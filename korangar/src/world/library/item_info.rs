use hashbrown::HashMap;
use mlua::Lua;
use ragnarok_packets::ItemId;

use super::{HashMapExt, ItemName, ItemResource, Library, LuaExt, Table, game_string};
use crate::loaders::GameFileLoader;

#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub(super) identified_name: ItemName,
    pub(super) unidentified_name: ItemName,
    pub(super) identified_resource: ItemResource,
    pub(super) unidentified_resource: ItemResource,
    /// Description lines with `<NAVI>…<INFO>…</INFO></NAVI>` reduced to the label.
    identified_description: Vec<String>,
    unidentified_description: Vec<String>,
}

impl ItemInfo {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn identified_description(&self) -> &[String] {
        &self.identified_description
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn unidentified_description(&self) -> &[String] {
        &self.unidentified_description
    }
}

/// Strip Gravity navigation markup the way Turoran roBrowser does (#127 / #27):
/// `<NAVI>label<INFO>…</INFO></NAVI>` → `label` (case-insensitive tags).
pub(super) fn strip_navi_tags(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let open = "<navi>";
    let info = "<info>";
    let close = "</navi>";
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(rel_start) = lower[cursor..].find(open) {
        let start = cursor + rel_start;
        out.push_str(&input[cursor..start]);

        let after_open = start + open.len();
        let Some(rel_close) = lower[after_open..].find(close) else {
            out.push_str(&input[start..]);
            return out;
        };
        let close_at = after_open + rel_close;
        let inner = &input[after_open..close_at];
        let inner_lower = &lower[after_open..close_at];

        if let Some(info_at) = inner_lower.find(info) {
            out.push_str(inner[..info_at].trim_end());
        } else {
            out.push_str(inner);
        }

        cursor = close_at + close.len();
    }

    out.push_str(&input[cursor..]);
    out
}

fn optional_game_string(value: Option<mlua::String>) -> mlua::Result<Option<String>> {
    value.map(game_string).transpose().map(|opt| opt.map(|text| strip_navi_tags(&text)))
}

fn description_lines(table: &mlua::Table, key: &str) -> mlua::Result<Vec<String>> {
    let Ok(lines) = table.get::<mlua::Table>(key) else {
        return Ok(Vec::new());
    };

    let mut result = Vec::new();
    for value in lines.sequence_values::<mlua::String>() {
        let line = game_string(value?)?;
        result.push(strip_navi_tags(&line));
    }
    Ok(result)
}

impl Table for ItemInfo {
    type Key<'a> = ItemId;
    type Storage = HashMap<ItemId, Self>;

    fn load(game_file_loader: &GameFileLoader) -> mlua::Result<Self::Storage> {
        // Match roBrowser DBManager: Turoran renewal itemInfo first, stock iteminfo fallback.
        let state = Lua::load_from_game_files_with_fallbacks(game_file_loader, &[&[
            "System\\itemInfo.turoran.renewal11.lua",
            "System/itemInfo.turoran.renewal11.lua",
            "data\\luafiles514\\lua files\\datainfo\\iteminfo.lub",
        ]])?;

        let globals = state.globals();
        let mut result = HashMap::new();

        if let Ok(table) = globals.get::<mlua::Table>("tbl") {
            for (item_id, item_table) in table.pairs::<u32, mlua::Table>().flatten() {
                let info = ItemInfo {
                    identified_name: ItemName::from_option(optional_game_string(
                        item_table.get::<Option<mlua::String>>("identifiedDisplayName")?,
                    )?),
                    unidentified_name: ItemName::from_option(optional_game_string(
                        item_table.get::<Option<mlua::String>>("unidentifiedDisplayName")?,
                    )?),
                    identified_resource: ItemResource::from_option(
                        item_table
                            .get::<Option<mlua::String>>("identifiedResourceName")?
                            .map(game_string)
                            .transpose()?,
                    ),
                    unidentified_resource: ItemResource::from_option(
                        item_table
                            .get::<Option<mlua::String>>("unidentifiedResourceName")?
                            .map(game_string)
                            .transpose()?,
                    ),
                    identified_description: description_lines(&item_table, "identifiedDescriptionName")?,
                    unidentified_description: description_lines(&item_table, "unidentifiedDescriptionName")?,
                };

                result.insert(ItemId(item_id), info);
            }
        }

        Ok(result.compact())
    }

    fn try_get<'a, 'b>(library: &'a Library, key: Self::Key<'b>) -> Option<&'a Self> {
        library.item_info_table.get(&key)
    }

    fn get<'a, 'b>(library: &'a Library, key: Self::Key<'b>) -> &'a Self {
        use std::sync::LazyLock;
        static DEFAULT: LazyLock<ItemInfo> = LazyLock::new(|| ItemInfo {
            identified_name: ItemName::not_found_value(),
            unidentified_name: ItemName::not_found_value(),
            identified_resource: ItemResource::not_found_value(),
            unidentified_resource: ItemResource::not_found_value(),
            identified_description: Vec::new(),
            unidentified_description: Vec::new(),
        });
        Self::try_get(library, key).unwrap_or(&DEFAULT)
    }
}

#[cfg(test)]
mod tests {
    use super::strip_navi_tags;
    use crate::world::library::{LuaExt, game_string};
    use mlua::Lua;

    struct MemoryLoader {
        files: &'static [(&'static str, &'static [u8])],
    }

    impl korangar_loaders::FileLoader for MemoryLoader {
        fn get(&self, path: &str) -> Result<Vec<u8>, korangar_loaders::FileNotFoundError> {
            self.files
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(path))
                .map(|(_, data)| data.to_vec())
                .ok_or_else(|| korangar_loaders::FileNotFoundError::new(path.to_owned()))
        }
    }

    #[test]
    fn strip_navi_keeps_label_and_drops_info() {
        assert_eq!(
            strip_navi_tags("<NAVI>[Aver De Dosh]<INFO>mal_in01,22,113,0,100,0,0</INFO></NAVI>"),
            "[Aver De Dosh]"
        );
        assert_eq!(
            strip_navi_tags("^ffffff<NAVI>^4D4DFF[Nyantime]^000000<INFO>malangdo,208,163,0,100,0,0</INFO></NAVI>^000000"),
            "^ffffff^4D4DFF[Nyantime]^000000^000000"
        );
        assert_eq!(strip_navi_tags("plain text"), "plain text");

        let info = super::ItemInfo {
            identified_name: crate::world::library::ItemName::from_option(Some("x".into())),
            unidentified_name: crate::world::library::ItemName::from_option(Some("x".into())),
            identified_resource: crate::world::library::ItemResource::from_option(Some("x".into())),
            unidentified_resource: crate::world::library::ItemResource::from_option(Some("x".into())),
            identified_description: vec![strip_navi_tags(
                "<NAVI>[Shop]<INFO>prontera,1,1,0,100,0,0</INFO></NAVI>",
            )],
            unidentified_description: vec![],
        };
        assert_eq!(info.identified_description(), ["[Shop]"]);
        assert!(info.unidentified_description().is_empty());
    }

    #[test]
    fn turoran_iteminfo_is_preferred_over_stock_and_descriptions_are_stripped() {
        let loader = MemoryLoader {
            files: &[
                (
                    "System\\itemInfo.turoran.renewal11.lua",
                    br#"tbl = {
                        [500] = {
                            identifiedDisplayName = "Choco",
                            unidentifiedDisplayName = "Choco",
                            identifiedResourceName = "choco",
                            unidentifiedResourceName = "choco",
                            identifiedDescriptionName = {
                                "<NAVI>[Shop]<INFO>prontera,1,1,0,100,0,0</INFO></NAVI>",
                                "Tasty."
                            },
                            unidentifiedDescriptionName = { "" },
                        }
                    }"#,
                ),
                (
                    "data\\luafiles514\\lua files\\datainfo\\iteminfo.lub",
                    b"tbl = { [500] = { identifiedDisplayName = \"STOCK\", unidentifiedDisplayName = \"STOCK\", identifiedResourceName = \"x\", unidentifiedResourceName = \"x\" } }",
                ),
            ],
        };

        let state = Lua::load_from_game_files_with_fallbacks(&loader, &[&[
            "System\\itemInfo.turoran.renewal11.lua",
            "System/itemInfo.turoran.renewal11.lua",
            "data\\luafiles514\\lua files\\datainfo\\iteminfo.lub",
        ]])
        .unwrap();
        let tbl: mlua::Table = state.globals().get("tbl").unwrap();
        let item: mlua::Table = tbl.get(500).unwrap();
        assert_eq!(game_string(item.get("identifiedDisplayName").unwrap()).unwrap(), "Choco");
        let lines: mlua::Table = item.get("identifiedDescriptionName").unwrap();
        let first = game_string(lines.get(1).unwrap()).unwrap();
        assert_eq!(strip_navi_tags(&first), "[Shop]");
    }

    #[test]
    fn stock_iteminfo_is_used_when_turoran_file_is_absent() {
        let loader = MemoryLoader {
            files: &[(
                "data\\luafiles514\\lua files\\datainfo\\iteminfo.lub",
                b"tbl = { [501] = { identifiedDisplayName = \"Red Potion\", unidentifiedDisplayName = \"Potion\", identifiedResourceName = \"red\", unidentifiedResourceName = \"pot\" } }",
            )],
        };
        let state = Lua::load_from_game_files_with_fallbacks(&loader, &[&[
            "System\\itemInfo.turoran.renewal11.lua",
            "System/itemInfo.turoran.renewal11.lua",
            "data\\luafiles514\\lua files\\datainfo\\iteminfo.lub",
        ]])
        .unwrap();
        let tbl: mlua::Table = state.globals().get("tbl").unwrap();
        let item: mlua::Table = tbl.get(501).unwrap();
        assert_eq!(game_string(item.get("identifiedDisplayName").unwrap()).unwrap(), "Red Potion");
    }
}
