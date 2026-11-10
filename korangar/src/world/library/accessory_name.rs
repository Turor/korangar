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
