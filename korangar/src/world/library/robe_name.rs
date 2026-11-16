//! View-id to robe sprite folder (`RobeNameTable`).
//!
//! roBrowser loads `spriterobeid.lub` and `spriterobename.lub`, then uses
//! `RobeNameTable[id]` as the folder under `로브`. The files are optional.

use hashbrown::HashMap;
use mlua::Lua;

use super::LuaExt;
use super::lua::game_string;
use crate::loaders::GameFileLoader;

pub(super) fn load_robe_names(game_file_loader: &GameFileLoader) -> HashMap<u16, String> {
    let groups: &[&[&str]] = &[
        &[
            "data\\luafiles514\\lua files\\datainfo\\spriterobeid.lub",
            "data\\luafiles514\\lua files\\datainfo\\spriterobeid.lua",
        ],
        &[
            "data\\luafiles514\\lua files\\datainfo\\spriterobename.lub",
            "data\\luafiles514\\lua files\\datainfo\\spriterobename.lua",
        ],
    ];

    let Ok(state) = Lua::load_from_game_files_with_fallbacks(game_file_loader, groups) else {
        return HashMap::new();
    };

    robe_names_from(&state)
}

/// The same table from in-memory `spriterobeid` and `spriterobename` chunks,
/// in that order.
#[cfg(test)]
fn parse_robe_names(robe_id: &[u8], robe_name: &[u8]) -> mlua::Result<HashMap<u16, String>> {
    let state = Lua::new();
    super::lua::execute_chunk(&state, "spriterobeid.lub", robe_id)?;
    super::lua::execute_chunk(&state, "spriterobename.lub", robe_name)?;
    Ok(robe_names_from(&state))
}

fn robe_names_from(state: &Lua) -> HashMap<u16, String> {
    let Ok(table) = state.globals().get::<mlua::Table>("RobeNameTable") else {
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
        if !name.is_empty() && name != "LAST" {
            names.insert(id as u16, name);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::parse_robe_names;

    #[test]
    fn robe_rows_key_by_view_id_and_skip_bad_rows() {
        let ids = b"SPRITE_ROBE_IDs = { ROBE_WINGS = 1 }";
        let names = br#"RobeNameTable = {
            [SPRITE_ROBE_IDs.ROBE_WINGS] = "angel_wing",
            [2] = "_\176\237\177\219",
            [0] = "zero",
            [999] = "LAST",
            [4] = "",
        }"#;
        let table = parse_robe_names(ids, names).unwrap();
        assert_eq!(table.get(&1).map(String::as_str), Some("angel_wing"));
        assert_eq!(table.get(&2).map(String::as_str), Some("_고글"));
        assert_eq!(table.len(), 2);
    }
}
