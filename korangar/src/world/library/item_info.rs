use std::io::Read;

use hashbrown::HashMap;
use korangar_loaders::FileLoader;
use mlua::Lua;
use ragnarok_packets::ItemId;

use super::{HashMapExt, ItemName, ItemResource, Library, Table, decode_game_bytes, execute_chunk, game_string};
use crate::loaders::GameFileLoader;

/// Stock client itemInfo, then legacy `translation.grf` text tables, then the
/// shipped English table, then a mounted Turoran renewal overlay. Later
/// sources win field by field. `GameFileLoader` already returns the
/// highest-priority archive copy of each path (the `archive/` folder and
/// `translation.grf` above `data.grf`).
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

/// Compact English table generated from `itemInfo.turoran.renewal11.lua`.
/// `FolderArchive` serves `iteminfo.english.v1.txt.gz` under this path.
const ENGLISH_ITEM_TABLE: &str = "System\\iteminfo.english.v1.txt";

/// Same bytes as the archive file, so a runtime that does not mount this
/// checkout's `archive/` (the TEST headless profile) still has English names.
const EMBEDDED_ENGLISH_ITEM_TABLE_GZ: &[u8] = include_bytes!("../../../archive/System/iteminfo.english.v1.txt.gz");

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
    /// `slotCount` from the English table or a mounted itemInfo overlay.
    slot_count: Option<u8>,
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn slot_count(&self) -> Option<u8> {
        self.slot_count
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

/// Test entry that skips the embedded English snapshot so fixtures stay
/// isolated.
#[cfg(test)]
fn load_item_infos(game_file_loader: &impl FileLoader) -> mlua::Result<HashMap<ItemId, ItemInfo>> {
    load_item_infos_with(game_file_loader, false)
}

fn load_item_infos_with(game_file_loader: &impl FileLoader, embed_english: bool) -> mlua::Result<HashMap<ItemId, ItemInfo>> {
    let mut result = HashMap::new();
    let mut saw_file = false;

    for path in BASE_ITEM_INFO_LUA {
        saw_file |= merge_lua_file(game_file_loader, path, false, &mut result);
    }
    for (path, identified) in NAME_TABLES {
        saw_file |= merge_name_table(game_file_loader, path, *identified, &mut result);
    }
    for (path, identified) in DESCRIPTION_TABLES {
        saw_file |= merge_description_table(game_file_loader, path, *identified, &mut result);
    }

    // Turoran overlay beats translation.grf tables, which beat Korean iteminfo.
    // A mounted copy of the compact table beats the binary snapshot so a newer
    // archive file can ship without rebuilding. The renewal lua, when present,
    // is the same source and wins over both.
    let saw_english_file = merge_english_file(game_file_loader, ENGLISH_ITEM_TABLE, &mut result);
    saw_file |= saw_english_file;
    if embed_english && !saw_english_file {
        let bytes = decode_gzip(EMBEDDED_ENGLISH_ITEM_TABLE_GZ)
            .map_err(|error| mlua::Error::RuntimeError(format!("embedded english item table: {error}")))?;
        merge_english_bytes(&bytes, &mut result)?;
        saw_file = true;
    }
    for path in OVERLAY_ITEM_INFO_LUA {
        saw_file |= merge_lua_file(game_file_loader, path, true, &mut result);
    }

    if !saw_file {
        return Err(mlua::Error::RuntimeError(
            "Cannot read itemInfo (tried iteminfo.lub, idnum2itemdisplaynametable.txt, iteminfo.english.v1.txt, and \
             itemInfo.turoran.renewal11.lua)"
                .into(),
        ));
    }

    log_sample_item_names(&result);
    Ok(result.compact())
}

fn merge_lua_file(loader: &impl FileLoader, path: &str, turoran_overlay: bool, into: &mut HashMap<ItemId, ItemInfo>) -> bool {
    let data = match loader.get(path) {
        Ok(data) => data,
        Err(_) => return false,
    };
    match items_from_lua(path, &data) {
        Ok(items) => {
            for (item_id, info) in items {
                overlay_item(into, item_id, info, turoran_overlay);
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
        slot_count: read_slot_count(item_table),
    })
}

fn read_slot_count(table: &mlua::Table) -> Option<u8> {
    if let Ok(value) = table.get::<u32>("slotCount") {
        return Some(value.min(255) as u8);
    }
    let Ok(value) = table.get::<f64>("slotCount") else {
        return None;
    };
    if value.is_finite() && (0.0..256.0).contains(&value) {
        Some(value as u8)
    } else {
        None
    }
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
        slot_count: None,
    }
}

fn is_description_stub(lines: &[String]) -> bool {
    lines.is_empty() || (lines.len() == 1 && (lines[0].is_empty() || lines[0] == "..."))
}

fn overlay_item(into: &mut HashMap<ItemId, ItemInfo>, item_id: ItemId, mut next: ItemInfo, turoran_overlay: bool) {
    // `{ "" }` and `{ "..." }` on the Turoran overlay are official stubs.
    // Keeping the previous unidentified text leaves a Korean blurb under the
    // English name. Fall back to the English identified description, or leave
    // it empty. Stock iteminfo does not clear a description it never replaced.
    if turoran_overlay && is_description_stub(&next.unidentified_description) {
        next.unidentified_description = if is_description_stub(&next.identified_description) {
            Vec::new()
        } else {
            next.identified_description.clone()
        };
    }

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
    // A settled Turoran stub is either the English identified text or empty.
    // Empty still replaces a previous Hangul blurb. Other sources skip stubs.
    if turoran_overlay || !is_description_stub(&next.unidentified_description) {
        current.unidentified_description = next.unidentified_description;
    }
    if let Some(slot_count) = next.slot_count {
        current.slot_count = Some(slot_count);
    }
}

struct EnglishRow {
    id: u32,
    slot_count: u8,
    identified_name: String,
    unidentified_name: String,
    identified_description: Vec<String>,
    unidentified_description: Vec<String>,
}

fn merge_english_file(loader: &impl FileLoader, path: &str, into: &mut HashMap<ItemId, ItemInfo>) -> bool {
    let Ok(data) = loader.get(path) else {
        return false;
    };
    match merge_english_bytes(&data, into) {
        Ok(()) => true,
        Err(error) => {
            eprintln!("item-name-glyph skip {path}: {error}");
            false
        }
    }
}

fn merge_english_bytes(bytes: &[u8], into: &mut HashMap<ItemId, ItemInfo>) -> mlua::Result<()> {
    let text = decode_game_bytes(bytes).map_err(|_| mlua::Error::RuntimeError("english item table is not UTF-8".into()))?;
    let rows = parse_english_table(&text).map_err(mlua::Error::RuntimeError)?;
    for row in rows {
        let item_id = ItemId(row.id);
        overlay_item(into, item_id, item_from_english_row(row), true);
    }
    Ok(())
}

fn item_from_english_row(row: EnglishRow) -> ItemInfo {
    ItemInfo {
        identified_name: ItemName::from_option(optional_text(row.identified_name)),
        unidentified_name: ItemName::from_option(optional_text(row.unidentified_name)),
        identified_resource: ItemResource::not_found_value(),
        unidentified_resource: ItemResource::not_found_value(),
        identified_description: row.identified_description,
        unidentified_description: row.unidentified_description,
        slot_count: Some(row.slot_count),
    }
}

fn optional_text(text: String) -> Option<String> {
    if text.is_empty() { None } else { Some(text) }
}

fn parse_english_table(text: &str) -> Result<Vec<EnglishRow>, String> {
    let mut lines = text.lines();
    let header = lines.next().unwrap_or("").trim();
    if header != "KEN1" {
        return Err(format!("english item table header {header:?}, expected KEN1"));
    }

    let mut rows = Vec::new();
    for (index, line) in lines.enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6 {
            return Err(format!("english item table line {}: expected 6 fields", index + 2));
        }
        let id = fields[0]
            .parse::<u32>()
            .map_err(|_| format!("english item table line {}: bad id", index + 2))?;
        let slot_count = fields[1]
            .parse::<u8>()
            .map_err(|_| format!("english item table line {}: bad slot count", index + 2))?;
        rows.push(EnglishRow {
            id,
            slot_count,
            identified_name: strip_navi_tags(&unescape_field(fields[2])),
            unidentified_name: strip_navi_tags(&unescape_field(fields[3])),
            identified_description: description_from_field(fields[4]),
            unidentified_description: description_from_field(fields[5]),
        });
    }
    Ok(rows)
}

fn description_from_field(field: &str) -> Vec<String> {
    if field.is_empty() {
        return Vec::new();
    }
    unescape_field(field).split('\n').map(strip_navi_tags).collect()
}

fn unescape_field(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn decode_gzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = flate2::read::GzDecoder::new(bytes);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).map_err(|error| error.to_string())?;
    Ok(decoded)
}

fn log_sample_item_names(table: &HashMap<ItemId, ItemInfo>) {
    #[cfg(not(test))]
    {
        // Startup used to print these four names on every launch. Keep the
        // sample behind an explicit debug flag.
        if std::env::var("KORANGAR_DEBUG_ITEM_NAMES").ok().as_deref() != Some("1") {
            return;
        }
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
    }
    #[cfg(test)]
    let _ = table;
}

impl Table for ItemInfo {
    type Key<'a> = ItemId;
    type Storage = HashMap<ItemId, Self>;

    fn load(game_file_loader: &GameFileLoader) -> mlua::Result<Self::Storage> {
        load_item_infos_with(game_file_loader, true)
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
            slot_count: None,
        });
        Self::try_get(library, key).unwrap_or(&DEFAULT)
    }
}

#[cfg(test)]
mod tests {
    use super::strip_navi_tags;

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
            slot_count: None,
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
                    b"tbl = { [500] = { identifiedDisplayName = \"STOCK\", unidentifiedDisplayName = \"STOCK\", identifiedResourceName = \"x\", unidentifiedResourceName = \"x\", unidentifiedDescriptionName = { \"\xed\x95\x9c\xea\xb8\x80\" } } }",
                ),
            ],
        };

        let table = super::load_item_infos(&loader).unwrap();
        let item = table.get(&ragnarok_packets::ItemId(500)).unwrap();
        assert_eq!(item.identified_name.as_str(), "Choco");
        assert_eq!(item.identified_description(), ["[Shop]", "Tasty."]);
        assert_eq!(item.unidentified_description(), ["[Shop]", "Tasty."]);
        assert!(item.unidentified_description().iter().all(|line| line != "한글"));
    }

    #[test]
    fn stock_iteminfo_is_used_when_turoran_file_is_absent() {
        let loader = MemoryLoader {
            files: &[(
                "data\\luafiles514\\lua files\\datainfo\\iteminfo.lub",
                b"tbl = { [501] = { identifiedDisplayName = \"Red Potion\", unidentifiedDisplayName = \"Potion\", identifiedResourceName = \"red\", unidentifiedResourceName = \"pot\" } }",
            )],
        };
        let table = super::load_item_infos(&loader).unwrap();
        let item = table.get(&ragnarok_packets::ItemId(501)).unwrap();
        assert_eq!(item.identified_name.as_str(), "Red Potion");
        assert_eq!(item.unidentified_name.as_str(), "Potion");
        assert_eq!(item.identified_resource.as_str(), "red");
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
        // `{ "..." }` is a stub, so the Hangul blurb is replaced by the English text.
        assert_eq!(potion.unidentified_description(), potion.identified_description());

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

    #[test]
    fn precedence_is_overlay_then_english_table_then_translation_then_korean() {
        let korean = br#"tbl = {
            [501] = {
                identifiedDisplayName = "STOCK",
                unidentifiedDisplayName = "STOCK",
                identifiedResourceName = "red",
                unidentifiedResourceName = "pot",
                identifiedDescriptionName = { "korean-identified" },
                unidentifiedDescriptionName = { "korean-unidentified" },
            },
            [1201] = {
                identifiedDisplayName = "STOCK",
                unidentifiedDisplayName = "STOCK",
                identifiedResourceName = "knife",
                unidentifiedResourceName = "knife",
                identifiedDescriptionName = { "korean-knife" },
                unidentifiedDescriptionName = { "korean-knife-unidentified" },
            },
            [999] = {
                identifiedDisplayName = "Only Korean",
                unidentifiedDisplayName = "Only Korean",
                identifiedResourceName = "apple",
                unidentifiedResourceName = "apple",
                identifiedDescriptionName = { "korean-apple" },
                unidentifiedDescriptionName = { "korean-apple" },
            },
        }"#;
        let english = b"KEN1\n\
501\t0\tRed Potion\tRed Potion\tA bottle of potion.\\n^0000CCType:^000000 Restorative\t\n\
1201\t4\tKnife\tUnidentified Dagger\tA simple knife.\tCan be identified by using a ^990099Magnifier^000000.\n";
        let overlay = br#"tbl = {
            [501] = {
                identifiedDisplayName = "<NAVI>Overlay Potion<INFO>501</INFO></NAVI>",
                unidentifiedDisplayName = "Overlay Potion",
                identifiedResourceName = "redpot",
                unidentifiedResourceName = "pot",
                identifiedDescriptionName = { "From the mounted overlay." },
                unidentifiedDescriptionName = { "" },
                slotCount = 0,
            },
        }"#;
        let loader = OwnedLoader {
            files: vec![
                ("data\\luafiles514\\lua files\\datainfo\\iteminfo.lub".into(), korean.to_vec()),
                (
                    "data\\idnum2itemdisplaynametable.txt".into(),
                    b"501#From_Table#\n1201#From_Table#\n".to_vec(),
                ),
                ("System\\iteminfo.english.v1.txt".into(), english.to_vec()),
                ("System\\itemInfo.turoran.renewal11.lua".into(), overlay.to_vec()),
            ],
        };

        let table = super::load_item_infos(&loader).unwrap();

        let potion = table.get(&ragnarok_packets::ItemId(501)).unwrap();
        assert_eq!(potion.identified_name.as_str(), "Overlay Potion");
        assert_eq!(potion.identified_description(), ["From the mounted overlay."]);
        assert_eq!(potion.unidentified_description(), ["From the mounted overlay."]);
        assert_eq!(potion.identified_resource.as_str(), "redpot");
        assert_eq!(potion.slot_count(), Some(0));
        assert!(potion.unidentified_description().iter().all(|line| line != "korean-unidentified"));

        let knife = table.get(&ragnarok_packets::ItemId(1201)).unwrap();
        assert_eq!(knife.identified_name.as_str(), "Knife");
        assert_eq!(knife.unidentified_name.as_str(), "Unidentified Dagger");
        assert_eq!(knife.identified_description(), ["A simple knife."]);
        assert_eq!(knife.unidentified_description(), [
            "Can be identified by using a ^990099Magnifier^000000."
        ]);
        assert_eq!(knife.identified_resource.as_str(), "knife");
        assert_eq!(knife.slot_count(), Some(4));
        assert!(knife.unidentified_description().iter().all(|line| !line.contains("korean")));

        let apple = table.get(&ragnarok_packets::ItemId(999)).unwrap();
        assert_eq!(apple.identified_name.as_str(), "Only Korean");
        assert_eq!(apple.identified_description(), ["korean-apple"]);
    }

    #[test]
    fn unidentified_stub_drops_hangul_blurb_under_english_name() {
        let korean = cp949_lua_name("\u{be68}\u{ac04}\u{d3ec}\u{c158}");
        let english = b"KEN1\n501\t0\tRed Potion\tRed Potion\tA bottle of potion made from ground Red Herbs.\t\n";
        let loader = OwnedLoader {
            files: vec![
                ("data\\luafiles514\\lua files\\datainfo\\iteminfo.lub".into(), korean),
                ("System\\iteminfo.english.v1.txt".into(), english.to_vec()),
            ],
        };
        let table = super::load_item_infos(&loader).unwrap();
        let potion = table.get(&ragnarok_packets::ItemId(501)).unwrap();
        assert_eq!(potion.identified_name.as_str(), "Red Potion");
        assert_eq!(potion.identified_description(), [
            "A bottle of potion made from ground Red Herbs."
        ]);
        assert_eq!(potion.unidentified_description(), potion.identified_description());
        assert!(
            potion
                .unidentified_description()
                .iter()
                .all(|line| !line.chars().any(|c| ('\u{ac00}'..='\u{d7a3}').contains(&c)))
        );
        assert_eq!(potion.identified_resource.as_str(), "red");
    }

    #[test]
    fn packaged_english_table_names_potion_knife_card_and_cotton_shirt() {
        let bytes = super::decode_gzip(super::EMBEDDED_ENGLISH_ITEM_TABLE_GZ).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let rows = super::parse_english_table(&text).unwrap();
        let row = |id: u32| rows.iter().find(|row| row.id == id).unwrap_or_else(|| panic!("missing {id}"));

        let potion = row(501);
        assert_eq!(potion.identified_name, "Red Potion");
        assert!(potion.identified_description.iter().any(|line| line.contains("Red Herbs")));
        assert!(potion.unidentified_description.is_empty());

        let knife = row(1201);
        assert_eq!(knife.identified_name, "Knife");
        assert_eq!(knife.unidentified_name, "Unidentified Dagger");
        assert_eq!(knife.slot_count, 4);
        assert!(knife.identified_description.iter().any(|line| line.contains("Dagger")));

        let card = row(4001);
        assert_eq!(card.identified_name, "Poring Card");
        assert!(card.identified_description.iter().any(|line| line.contains("LUK")));
        assert!(card.unidentified_description.is_empty());

        let shirt = row(2301);
        assert_eq!(shirt.identified_name, "Cotton Shirt");
        assert_eq!(shirt.unidentified_name, "Unidentified Armor");
        assert_eq!(shirt.slot_count, 1);
        assert!(shirt.unidentified_description.iter().any(|line| line.contains("Magnifier")));
    }

    #[test]
    fn embedded_english_table_overrides_korean_when_archive_copy_is_absent() {
        let korean = br#"tbl = { [501] = { identifiedDisplayName = "STOCK", unidentifiedDisplayName = "STOCK", identifiedResourceName = "red", unidentifiedResourceName = "pot", identifiedDescriptionName = { "korean" }, unidentifiedDescriptionName = { "korean-unidentified" } } }"#;
        let loader = OwnedLoader {
            files: vec![("data\\luafiles514\\lua files\\datainfo\\iteminfo.lub".into(), korean.to_vec())],
        };
        let table = super::load_item_infos_with(&loader, true).unwrap();
        let potion = table.get(&ragnarok_packets::ItemId(501)).unwrap();
        assert_eq!(potion.identified_name.as_str(), "Red Potion");
        assert!(potion.identified_description().iter().any(|line| line.contains("Red Herbs")));
        assert!(potion.unidentified_description().iter().all(|line| line != "korean-unidentified"));
        assert_eq!(potion.identified_resource.as_str(), "red");
    }
}
