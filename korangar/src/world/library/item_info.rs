use hashbrown::HashMap;
use korangar_loaders::FileLoader;
use mlua::Lua;
use ragnarok_packets::ItemId;

use super::{HashMapExt, ItemName, ItemResource, Library, Table, decode_game_bytes, execute_chunk, game_string};
use crate::loaders::GameFileLoader;

/// Stock client itemInfo, then legacy `translation.grf` text tables, then the
/// Turoran English renewal overlay. Later sources win field by field.
/// `GameFileLoader` already returns the highest-priority archive copy of each
/// path (translation.grf above data.grf).
const BASE_ITEM_INFO_LUA: &[&str] = &[
    "data\\luafiles514\\lua files\\datainfo\\iteminfo.lub",
    "System\\itemInfo.lub",
    "System\\itemInfo.lua",
    "System\\itemInfo_true.lub",
    "System\\itemInfo_true.lua",
    "System\\itemInfo_sak.lub",
    "System\\itemInfo_sak.lua",
    "System\\itemInfo_Sakray.lub",
    "System\\itemInfo_Sakray.lua",
];

const NAME_TABLES: &[(&str, bool)] = &[
    ("data\\num2itemdisplaynametable.txt", false),
    ("data\\idnum2itemdisplaynametable.txt", true),
];

const DESCRIPTION_TABLES: &[(&str, bool)] = &[("data\\num2itemdesctable.txt", false), ("data\\idnum2itemdesctable.txt", true)];

const OVERLAY_ITEM_INFO_LUA: &[&str] = &["System\\itemInfo.turoran.renewal11.lua", "System\\itemInfo.turoran.renewal11.lub"];

#[cfg_attr(test, allow(dead_code))]
const SAMPLE_ITEM_IDS: &[u32] = &[500, 501, 1201, 4001];

#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub(super) identified_name: ItemName,
    pub(super) unidentified_name: ItemName,
    pub(super) identified_resource: ItemResource,
    pub(super) unidentified_resource: ItemResource,
    /// Description lines with `<NAVI>…<INFO>…</INFO></NAVI>` reduced to the
    /// label.
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

/// Longest match first so `<item>` does not eat `<itemlink>` / `<iteml>`.
const MARKUP_TAGS: &[(&str, &str)] = &[
    ("<itemlink>", "</itemlink>"),
    ("<iteml>", "</iteml>"),
    ("<item>", "</item>"),
    ("<navi>", "</navi>"),
];

/// Strip Gravity item markup the way Turoran roBrowser does (#127 / #27):
/// `<NAVI|ITEM|ITEMLINK>label<INFO>…</INFO></…>` → `label`.
/// `^RRGGBB` color codes stay in the string so the font layouter renders them
/// as colors.
pub(super) fn strip_navi_tags(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some((start, open, close)) = find_markup(&lower, cursor) {
        out.push_str(&input[cursor..start]);

        let after_open = start + open.len();
        let Some(rel_close) = lower[after_open..].find(close) else {
            out.push_str(&input[start..]);
            return out;
        };
        let close_at = after_open + rel_close;
        let inner = &input[after_open..close_at];
        let inner_lower = &lower[after_open..close_at];

        if let Some(info_at) = inner_lower.find("<info>") {
            out.push_str(inner[..info_at].trim_end());
        } else {
            out.push_str(inner);
        }

        cursor = close_at + close.len();
    }

    out.push_str(&input[cursor..]);
    out
}

fn find_markup(lower: &str, from: usize) -> Option<(usize, &'static str, &'static str)> {
    let mut best: Option<(usize, &'static str, &'static str)> = None;
    for &(open, close) in MARKUP_TAGS {
        if let Some(rel) = lower[from..].find(open) {
            let at = from + rel;
            let replace = match best {
                None => true,
                Some((pos, prev, _)) => at < pos || (at == pos && open.len() > prev.len()),
            };
            if replace {
                best = Some((at, open, close));
            }
        }
    }
    best
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

fn load_item_infos(game_file_loader: &impl FileLoader) -> mlua::Result<HashMap<ItemId, ItemInfo>> {
    let mut result = HashMap::new();
    let mut saw_file = false;

    for path in BASE_ITEM_INFO_LUA {
        saw_file |= merge_lua_file(game_file_loader, path, &mut result);
    }
    for (path, identified) in NAME_TABLES {
        saw_file |= merge_name_table(game_file_loader, path, *identified, &mut result);
    }
    for (path, identified) in DESCRIPTION_TABLES {
        saw_file |= merge_description_table(game_file_loader, path, *identified, &mut result);
    }
    for path in OVERLAY_ITEM_INFO_LUA {
        saw_file |= merge_lua_file(game_file_loader, path, &mut result);
    }

    if !saw_file {
        return Err(mlua::Error::RuntimeError(
            "Cannot read itemInfo (tried iteminfo.lub, idnum2itemdisplaynametable.txt, and itemInfo.turoran.renewal11.lua)".into(),
        ));
    }

    log_sample_item_names(&result);
    Ok(result.compact())
}

fn merge_lua_file(loader: &impl FileLoader, path: &str, into: &mut HashMap<ItemId, ItemInfo>) -> bool {
    let data = match loader.get(path) {
        Ok(data) => data,
        Err(_) => return false,
    };
    match items_from_lua(path, &data) {
        Ok(items) => {
            for (item_id, info) in items {
                overlay_item(into, item_id, info);
            }
            true
        }
        Err(error) => {
            eprintln!("item-name-glyph skip {path}: {error}");
            true
        }
    }
}

fn items_from_lua(path: &str, data: &[u8]) -> mlua::Result<Vec<(ItemId, ItemInfo)>> {
    let state = Lua::new();
    execute_chunk(&state, path, data)?;
    let Ok(table) = state.globals().get::<mlua::Table>("tbl") else {
        return Ok(Vec::new());
    };

    let mut items = Vec::new();
    for pair in table.pairs::<u32, mlua::Table>() {
        let Ok((item_id, item_table)) = pair else {
            continue;
        };
        let Ok(info) = item_from_lua_table(&item_table) else {
            continue;
        };
        items.push((ItemId(item_id), info));
    }
    Ok(items)
}

fn item_from_lua_table(item_table: &mlua::Table) -> mlua::Result<ItemInfo> {
    Ok(ItemInfo {
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
        identified_description: description_lines(item_table, "identifiedDescriptionName")?,
        unidentified_description: description_lines(item_table, "unidentifiedDescriptionName")?,
    })
}

fn merge_name_table(loader: &impl FileLoader, path: &str, identified: bool, into: &mut HashMap<ItemId, ItemInfo>) -> bool {
    let Some(rows) = read_hash_table(loader, path) else {
        return false;
    };
    for (item_id, value) in rows {
        let name = value.replace('_', " ");
        let entry = into.entry(ItemId(item_id)).or_insert_with(empty_item);
        let slot = if identified {
            &mut entry.identified_name
        } else {
            &mut entry.unidentified_name
        };
        *slot = ItemName::from_option(Some(strip_navi_tags(&name)));
    }
    true
}

fn merge_description_table(loader: &impl FileLoader, path: &str, identified: bool, into: &mut HashMap<ItemId, ItemInfo>) -> bool {
    let Some(rows) = read_hash_table(loader, path) else {
        return false;
    };
    for (item_id, value) in rows {
        let lines = description_lines_from_table(&value);
        if is_description_stub(&lines) {
            continue;
        }
        let entry = into.entry(ItemId(item_id)).or_insert_with(empty_item);
        let slot = if identified {
            &mut entry.identified_description
        } else {
            &mut entry.unidentified_description
        };
        *slot = lines;
    }
    true
}

fn read_hash_table(loader: &impl FileLoader, path: &str) -> Option<Vec<(u32, String)>> {
    let bytes = loader.get(path).ok()?;
    let text = decode_game_bytes(&bytes).ok()?;
    Some(parse_hash_table(&text))
}

/// Gravity `id#value#` rows. Display-name callers replace `_` themselves.
fn parse_hash_table(text: &str) -> Vec<(u32, String)> {
    let mut rows = Vec::new();
    for raw in text.split(['\n', '\r']) {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let mut parts = line.split('#');
        let Some(id_text) = parts.next() else {
            continue;
        };
        let Ok(item_id) = id_text.trim().parse::<u32>() else {
            continue;
        };
        let Some(value) = parts.next() else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        rows.push((item_id, value.to_owned()));
    }
    rows
}

fn description_lines_from_table(value: &str) -> Vec<String> {
    value
        .replace("\\n", "\n")
        .split('\n')
        .map(strip_navi_tags)
        .filter(|line| !line.is_empty())
        .collect()
}

fn empty_item() -> ItemInfo {
    ItemInfo {
        identified_name: ItemName::not_found_value(),
        unidentified_name: ItemName::not_found_value(),
        identified_resource: ItemResource::not_found_value(),
        unidentified_resource: ItemResource::not_found_value(),
        identified_description: Vec::new(),
        unidentified_description: Vec::new(),
    }
}

fn is_description_stub(lines: &[String]) -> bool {
    lines.is_empty() || (lines.len() == 1 && (lines[0].is_empty() || lines[0] == "..."))
}

fn overlay_item(into: &mut HashMap<ItemId, ItemInfo>, item_id: ItemId, next: ItemInfo) {
    let Some(current) = into.get_mut(&item_id) else {
        into.insert(item_id, next);
        return;
    };
    if !next.identified_name.is_placeholder() {
        current.identified_name = next.identified_name;
    }
    if !next.unidentified_name.is_placeholder() {
        current.unidentified_name = next.unidentified_name;
    }
    if !next.identified_resource.is_placeholder() {
        current.identified_resource = next.identified_resource;
    }
    if !next.unidentified_resource.is_placeholder() {
        current.unidentified_resource = next.unidentified_resource;
    }
    if !is_description_stub(&next.identified_description) {
        current.identified_description = next.identified_description;
    }
    if !is_description_stub(&next.unidentified_description) {
        current.unidentified_description = next.unidentified_description;
    }
}

fn log_sample_item_names(table: &HashMap<ItemId, ItemInfo>) {
    #[cfg(not(test))]
    for item_id in SAMPLE_ITEM_IDS {
        match table.get(&ItemId(*item_id)) {
            Some(info) => {
                let name = info.identified_name.as_str();
                eprintln!(
                    "item-name-glyph id={item_id} identified_name={name:?} utf8={:02x?}",
                    name.as_bytes()
                );
            }
            None => eprintln!("item-name-glyph id={item_id} missing"),
        }
    }
    #[cfg(test)]
    let _ = table;
}

impl Table for ItemInfo {
    type Key<'a> = ItemId;
    type Storage = HashMap<ItemId, Self>;

    fn load(game_file_loader: &GameFileLoader) -> mlua::Result<Self::Storage> {
        load_item_infos(game_file_loader)
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
    use mlua::Lua;

    use super::strip_navi_tags;
    use crate::world::library::{LuaExt, game_string};

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
        assert_eq!(
            strip_navi_tags("<ITEM>Red Potion<INFO>501</INFO></ITEM> and <ITEMLINK>Knife<INFO>1201</INFO></ITEMLINK>"),
            "Red Potion and Knife"
        );

        let info = super::ItemInfo {
            identified_name: crate::world::library::ItemName::from_option(Some("x".into())),
            unidentified_name: crate::world::library::ItemName::from_option(Some("x".into())),
            identified_resource: crate::world::library::ItemResource::from_option(Some("x".into())),
            unidentified_resource: crate::world::library::ItemResource::from_option(Some("x".into())),
            identified_description: vec![strip_navi_tags("<NAVI>[Shop]<INFO>prontera,1,1,0,100,0,0</INFO></NAVI>")],
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

    struct OwnedLoader {
        files: Vec<(String, Vec<u8>)>,
    }

    impl korangar_loaders::FileLoader for OwnedLoader {
        fn get(&self, path: &str) -> Result<Vec<u8>, korangar_loaders::FileNotFoundError> {
            self.files
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(path))
                .map(|(_, data)| data.clone())
                .ok_or_else(|| korangar_loaders::FileNotFoundError::new(path.to_owned()))
        }
    }

    fn cp949_lua_name(hangul: &str) -> Vec<u8> {
        let (bytes, _, had_errors) = encoding_rs::EUC_KR.encode(hangul);
        assert!(!had_errors);
        let mut source = b"tbl = { [501] = { identifiedDisplayName = \"".to_vec();
        source.extend_from_slice(&bytes);
        source.extend_from_slice(
            b"\", unidentifiedDisplayName = \"Potion\", identifiedResourceName = \"red\", unidentifiedResourceName = \"pot\", identifiedDescriptionName = { \"korean\" } } }",
        );
        source
    }

    #[test]
    fn english_overlay_and_translation_table_replace_cp949_names() {
        let korean = cp949_lua_name("빨간포션");
        let renewal = br#"tbl = {
            [501] = {
                identifiedDisplayName = "Red Potion",
                unidentifiedDisplayName = "Red Potion",
                identifiedResourceName = "redpot",
                unidentifiedResourceName = "pot",
                identifiedDescriptionName = {
                    "<ITEM>Red Potion<INFO>501</INFO></ITEM>",
                    "^990099A potion made from grinded Red Herbs.^000000"
                },
                unidentifiedDescriptionName = { "..." },
            },
            [1201] = {
                identifiedDisplayName = "Knife",
                unidentifiedDisplayName = "Unidentified Dagger",
                identifiedResourceName = "knife",
                unidentifiedResourceName = "knife",
                identifiedDescriptionName = { "A simple dagger." },
                unidentifiedDescriptionName = { "" },
            },
            [500] = {
                identifiedDisplayName = "<NAVI>Choco Gangjeong<INFO>prontera,1,1,0,100,0,0</INFO></NAVI>",
                unidentifiedDisplayName = "Choco Gangjeong",
                identifiedResourceName = "choco",
                unidentifiedResourceName = "choco",
                identifiedDescriptionName = { "Turoran custom." },
                unidentifiedDescriptionName = { "" },
            },
        }"#;
        let loader = OwnedLoader {
            files: vec![
                ("data\\luafiles514\\lua files\\datainfo\\iteminfo.lub".into(), korean),
                (
                    "data\\idnum2itemdisplaynametable.txt".into(),
                    b"1201#Knife#\n999#Apple#\n".to_vec(),
                ),
                ("System\\itemInfo.turoran.renewal11.lua".into(), renewal.to_vec()),
            ],
        };

        let table = super::load_item_infos(&loader).unwrap();
        let potion = table.get(&ragnarok_packets::ItemId(501)).unwrap();
        assert_eq!(potion.identified_name.as_str(), "Red Potion");
        assert_eq!(potion.identified_description(), [
            "Red Potion",
            "^990099A potion made from grinded Red Herbs.^000000"
        ]);
        assert!(potion.unidentified_description().is_empty() || potion.unidentified_description() == ["..."]);

        let knife = table.get(&ragnarok_packets::ItemId(1201)).unwrap();
        assert_eq!(knife.identified_name.as_str(), "Knife");
        assert_eq!(knife.identified_description(), ["A simple dagger."]);

        let custom = table.get(&ragnarok_packets::ItemId(500)).unwrap();
        assert_eq!(custom.identified_name.as_str(), "Choco Gangjeong");
        assert_eq!(custom.identified_description(), ["Turoran custom."]);

        let apple = table.get(&ragnarok_packets::ItemId(999)).unwrap();
        assert_eq!(apple.identified_name.as_str(), "Apple");
    }

    #[test]
    fn translation_table_replaces_underscores_and_cp949_lua_names() {
        let korean = cp949_lua_name("빨간포션");
        let loader = OwnedLoader {
            files: vec![
                ("data\\luafiles514\\lua files\\datainfo\\iteminfo.lub".into(), korean),
                ("data\\idnum2itemdisplaynametable.txt".into(), b"501#Red_Potion#\n".to_vec()),
                (
                    "data\\idnum2itemdesctable.txt".into(),
                    b"501#A red potion.\\n<NAVI>[Shop]<INFO>prontera,1,1,0,100,0,0</INFO></NAVI>#\n".to_vec(),
                ),
            ],
        };
        let table = super::load_item_infos(&loader).unwrap();
        let potion = table.get(&ragnarok_packets::ItemId(501)).unwrap();
        assert_eq!(potion.identified_name.as_str(), "Red Potion");
        assert_eq!(potion.identified_description(), ["A red potion.", "[Shop]"]);
        assert_eq!(potion.identified_resource.as_str(), "red");
    }
}
