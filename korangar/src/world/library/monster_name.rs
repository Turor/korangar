//! Monster display names from `npcidentity.lub` + `jobname.lub`.
//!
//! The same identity table already names entity sprites. `JobNameTable` is the
//! paired display-name table roBrowser copies into `MonsterTable`.

use hashbrown::HashMap;
use mlua::Lua;

use super::LuaExt;
use super::lua::{game_string, value_u32};
use crate::loaders::GameFileLoader;

const IDENTITY_AND_NAMES: &[&[&str]] = &[
    &[
        "data\\luafiles514\\lua files\\datainfo\\npcidentity.lub",
        "data/luafiles514/lua files/datainfo/npcidentity.lub",
    ],
    &[
        "data\\luafiles514\\lua files\\datainfo\\jobname.lub",
        "data/luafiles514/lua files/datainfo/jobname.lub",
        "data\\luafiles514\\lua files\\datainfo\\jobname_f.lub",
        "data/luafiles514/lua files/datainfo/jobname_f.lub",
    ],
];

pub(super) fn load_monster_names(game_file_loader: &GameFileLoader) -> HashMap<u32, String> {
    let Ok(state) = Lua::load_from_game_files_with_fallbacks(game_file_loader, IDENTITY_AND_NAMES) else {
        return HashMap::new();
    };
    parse_job_name_table(&state)
}

pub fn parse_job_name_table(state: &Lua) -> HashMap<u32, String> {
    let Ok(table) = state.globals().get::<mlua::Table>("JobNameTable") else {
        return HashMap::new();
    };
    let mut names = HashMap::new();
    for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
        let Some(mob_id) = monster_key(state, pair.0) else { continue };
        let Some(name) = monster_label(pair.1) else { continue };
        if name.is_empty() {
            continue;
        }
        names.insert(mob_id, name);
    }
    names
}

fn monster_key(state: &Lua, key: mlua::Value) -> Option<u32> {
    if let Some(id) = value_u32(key.clone()) {
        return Some(id);
    }
    let mlua::Value::String(text) = key else {
        return None;
    };
    let name = game_string(text).ok()?;
    state.globals().get::<i64>(name.as_str()).ok().and_then(|id| u32::try_from(id).ok())
}

fn monster_label(value: mlua::Value) -> Option<String> {
    let mlua::Value::String(text) = value else {
        return None;
    };
    game_string(text).ok().map(|text| text.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::parse_job_name_table;
    use crate::world::library::lua::execute_chunk;

    #[test]
    fn job_name_table_uses_identity_constants() {
        let lua = Lua::new();
        execute_chunk(
            &lua,
            "jobname.lub",
            br#"
            JT_PORING = 1002
            JobNameTable = { [JT_PORING] = "Poring", [1113] = "Drops" }
            "#,
        )
        .unwrap();
        let names = parse_job_name_table(&lua);
        assert_eq!(names.get(&1002).map(String::as_str), Some("Poring"));
        assert_eq!(names.get(&1113).map(String::as_str), Some("Drops"));
    }
}
