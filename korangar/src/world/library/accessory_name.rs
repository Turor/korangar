//! View-id to accessory sprite suffix (`AccNameTable`).
//!
//! roBrowser builds hat paths as `악세사리/{sex}/{sex}{AccNameTable[id]}`.
//! The table is optional: a client without the lub files still draws the
//! body and head.

use hashbrown::HashMap;
use mlua::Lua;

use super::LuaExt;
use super::lua::game_string;
use crate::loaders::GameFileLoader;

pub(super) fn load_accessory_names(game_file_loader: &GameFileLoader) -> HashMap<u16, String> {
    let groups: &[&[&str]] = &[
        &[
            "data\\luafiles514\\lua files\\datainfo\\accessoryid.lub",
            "data\\luafiles514\\lua files\\datainfo\\accessoryid.lua",
        ],
        &[
            "data\\luafiles514\\lua files\\datainfo\\accname.lub",
            "data\\luafiles514\\lua files\\datainfo\\accname.lua",
        ],
    ];

    let Ok(state) = Lua::load_from_game_files_with_fallbacks(game_file_loader, groups) else {
        return HashMap::new();
    };

    accessory_names_from(&state)
}

/// The same table from in-memory `accessoryid` and `accname` chunks, in that
/// order. Used by the e2e harness with fixture tables.
pub fn parse_accessory_names(accessory_id: &[u8], accname: &[u8]) -> mlua::Result<HashMap<u16, String>> {
    let state = Lua::new();
    super::lua::execute_chunk(&state, "accessoryid.lub", accessory_id)?;
    super::lua::execute_chunk(&state, "accname.lub", accname)?;
    Ok(accessory_names_from(&state))
}

fn accessory_names_from(state: &Lua) -> HashMap<u16, String> {
    let Ok(table) = state.globals().get::<mlua::Table>("AccNameTable") else {
        return HashMap::new();
    };

    let mut names = HashMap::new();
    for (key, value) in table.pairs::<mlua::Value, mlua::Value>().flatten() {
        let mlua::Value::Integer(id) = key else {
            continue;
        };
        if id <= 0 || id > i64::from(u16::MAX) {
            continue;
        }
        let mlua::Value::String(text) = value else {
            continue;
        };
        let Ok(name) = game_string(text) else {
            continue;
        };
        if !name.is_empty() {
            names.insert(id as u16, name);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::parse_accessory_names;

    #[test]
    fn accname_rows_key_by_view_id_and_skip_bad_rows() {
        let ids = b"ACCESSORY_IDs = { ACCESSORY_GOGGLE = 1, ACCESSORY_RIBBON = 17 }";
        let names = br#"AccNameTable = {
            [ACCESSORY_IDs.ACCESSORY_GOGGLE] = "_\176\237\177\219",
            [ACCESSORY_IDs.ACCESSORY_RIBBON] = "_ribbon",
            [0] = "_zero",
            [-3] = "_negative",
            [5] = "",
        }"#;
        let table = parse_accessory_names(ids, names).unwrap();
        assert_eq!(table.get(&1).map(String::as_str), Some("_고글"));
        assert_eq!(table.get(&17).map(String::as_str), Some("_ribbon"));
        assert_eq!(table.len(), 2);
    }
}
