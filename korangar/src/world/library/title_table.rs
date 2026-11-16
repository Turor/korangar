//! Achievement title names (`titletable.lub`, `title_tbl`).
//!
//! roBrowser `DB.getTitleString` reads the same table for the achievement
//! reward overlay. The file is optional: without it a title reward shows its
//! id.

use hashbrown::HashMap;
use korangar_loaders::FileLoader;
use mlua::Lua;

use super::lua::{execute_chunk, game_string};

const TITLE_FILES: &[&str] = &[
    "data\\luafiles514\\lua files\\datainfo\\titletable.lub",
    "data\\luafiles514\\lua files\\datainfo\\titletable.lua",
];

pub fn load_title_table(loader: &impl FileLoader) -> HashMap<u32, String> {
    for file in TITLE_FILES {
        let Ok(data) = loader.get(file) else { continue };
        if let Ok(table) = parse_title_table(&data) {
            return table;
        }
    }
    HashMap::new()
}

/// `title_tbl = { [1000] = "Novice Adventurer", ... }`. Rows with a bad id or
/// an empty name are skipped.
pub fn parse_title_table(data: &[u8]) -> mlua::Result<HashMap<u32, String>> {
    let lua = Lua::new();
    execute_chunk(&lua, "titletable.lub", data)?;
    let mut titles = HashMap::new();
    let Ok(table) = lua.globals().get::<mlua::Table>("title_tbl") else {
        return Ok(titles);
    };
    for (key, value) in table.pairs::<mlua::Value, mlua::Value>().flatten() {
        let mlua::Value::Integer(id) = key else { continue };
        let Ok(id) = u32::try_from(id) else { continue };
        let mlua::Value::String(text) = value else { continue };
        let Ok(name) = game_string(text) else { continue };
        let name = name.trim().to_owned();
        if id != 0 && !name.is_empty() {
            titles.insert(id, name);
        }
    }
    Ok(titles)
}

#[cfg(test)]
mod tests {
    use super::parse_title_table;

    #[test]
    fn title_rows_key_by_id_and_skip_bad_rows() {
        let titles = parse_title_table(
            br#"title_tbl = {
                [1000] = "Novice Adventurer",
                [1001] = " Hero ",
                [0] = "zero",
                [-1] = "negative",
                [1002] = "",
                name = "not a row",
            }"#,
        )
        .unwrap();
        assert_eq!(titles.get(&1000).map(String::as_str), Some("Novice Adventurer"));
        assert_eq!(titles.get(&1001).map(String::as_str), Some("Hero"));
        assert_eq!(titles.len(), 2);
        assert!(parse_title_table(b"title_tbl = {").is_err());
        assert!(parse_title_table(b"other = 1").unwrap().is_empty());
    }
}
