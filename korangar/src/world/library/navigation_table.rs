//! Navigation tables from `lua files/navigation/navi_*_krpri.lub`, the same
//! files roBrowser's Navigation window searches and routes with.
//!
//! - `Navi_Link`: `{ map, id, type, sprite, name, "", x, y, dest_map, dest_x,
//!   dest_y }`. Type 200 is a warp portal, 201 an NPC warp, 202-205 services
//!   (Kafra, airship and the like).
//! - `Navi_Npc`: `{ map, id, type, class, name, "", x, y }`.
//! - `Navi_Mob`: `{ map, spawn_id, type, class, name, sprite, level, info }`.

use korangar_loaders::FileLoader;
use mlua::Lua;

use super::lua::{execute_chunk, game_string, value_u32};

const LINK_FILES: &[&str] = &[
    "data\\luafiles514\\lua files\\navigation\\navi_link_krpri.lub",
    "data/luafiles514/lua files/navigation/navi_link_krpri.lub",
];
const NPC_FILES: &[&str] = &[
    "data\\luafiles514\\lua files\\navigation\\navi_npc_krpri.lub",
    "data/luafiles514/lua files/navigation/navi_npc_krpri.lub",
];
const MOB_FILES: &[&str] = &[
    "data\\luafiles514\\lua files\\navigation\\navi_mob_krpri.lub",
    "data/luafiles514/lua files/navigation/navi_mob_krpri.lub",
];

/// A warp or service link from one map to another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaviLink {
    pub map: String,
    pub id: u32,
    pub kind: u32,
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub dest_map: String,
    pub dest_x: u16,
    pub dest_y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaviNpc {
    pub map: String,
    pub id: u32,
    pub name: String,
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaviMob {
    pub map: String,
    pub mob_id: u32,
    pub name: String,
    pub level: u32,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct NavigationTables {
    pub links: Vec<NaviLink>,
    pub npcs: Vec<NaviNpc>,
    pub mobs: Vec<NaviMob>,
}

impl std::fmt::Debug for NavigationTables {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NavigationTables")
            .field("links", &self.links.len())
            .field("npcs", &self.npcs.len())
            .field("mobs", &self.mobs.len())
            .finish()
    }
}

impl NavigationTables {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty() && self.npcs.is_empty() && self.mobs.is_empty()
    }
}

/// Missing or broken files leave that part empty. Navigation then falls back
/// to same-map guidance only.
pub fn load_navigation_tables(loader: &impl FileLoader) -> NavigationTables {
    let mut tables = NavigationTables::default();
    if let Some(lua) = run_first(loader, LINK_FILES) {
        tables.links = parse_links(&lua);
    }
    if let Some(lua) = run_first(loader, NPC_FILES) {
        tables.npcs = parse_npcs(&lua);
    }
    if let Some(lua) = run_first(loader, MOB_FILES) {
        tables.mobs = parse_mobs(&lua);
    }
    tables
}

fn run_first(loader: &impl FileLoader, files: &[&str]) -> Option<Lua> {
    files.iter().find_map(|file| {
        let data = loader.get(file).ok()?;
        let lua = Lua::new();
        execute_chunk(&lua, file, &data).ok()?;
        Some(lua)
    })
}

/// `prontera.gat` / `PRONTERA` → `prontera`.
pub fn navi_map_name(name: &str) -> String {
    let trimmed = name.trim().trim_matches('\0');
    let lower = trimmed.to_ascii_lowercase();
    lower.strip_suffix(".gat").map(str::to_owned).unwrap_or(lower)
}

fn rows(lua: &Lua, global: &str) -> Vec<mlua::Table> {
    let Ok(table) = lua.globals().get::<mlua::Table>(global) else {
        return Vec::new();
    };
    table.sequence_values::<mlua::Table>().flatten().collect()
}

fn field_text(row: &mlua::Table, index: usize) -> String {
    match row.get::<mlua::Value>(index) {
        Ok(mlua::Value::String(value)) => game_string(value).map(|text| text.trim().to_owned()).unwrap_or_default(),
        _ => String::new(),
    }
}

fn field_u32(row: &mlua::Table, index: usize) -> Option<u32> {
    row.get::<mlua::Value>(index).ok().and_then(value_u32)
}

fn field_cell(row: &mlua::Table, index: usize) -> u16 {
    field_u32(row, index).and_then(|value| u16::try_from(value).ok()).unwrap_or(0)
}

pub fn parse_links(lua: &Lua) -> Vec<NaviLink> {
    rows(lua, "Navi_Link")
        .iter()
        .filter_map(|row| {
            let map = navi_map_name(&field_text(row, 1));
            let dest_map = navi_map_name(&field_text(row, 9));
            if map.is_empty() || dest_map.is_empty() {
                return None;
            }
            Some(NaviLink {
                map,
                id: field_u32(row, 2).unwrap_or(0),
                kind: field_u32(row, 3).unwrap_or(0),
                name: field_text(row, 5),
                x: field_cell(row, 7),
                y: field_cell(row, 8),
                dest_map,
                dest_x: field_cell(row, 10),
                dest_y: field_cell(row, 11),
            })
        })
        .collect()
}

pub fn parse_npcs(lua: &Lua) -> Vec<NaviNpc> {
    rows(lua, "Navi_Npc")
        .iter()
        .filter_map(|row| {
            let map = navi_map_name(&field_text(row, 1));
            let name = field_text(row, 5);
            if map.is_empty() || name.is_empty() {
                return None;
            }
            Some(NaviNpc {
                map,
                id: field_u32(row, 2).unwrap_or(0),
                name,
                x: field_cell(row, 7),
                y: field_cell(row, 8),
            })
        })
        .collect()
}

pub fn parse_mobs(lua: &Lua) -> Vec<NaviMob> {
    rows(lua, "Navi_Mob")
        .iter()
        .filter_map(|row| {
            let map = navi_map_name(&field_text(row, 1));
            let name = field_text(row, 5);
            if map.is_empty() || name.is_empty() {
                return None;
            }
            Some(NaviMob {
                map,
                // Official rows pack extra flags above the 16-bit class id.
                mob_id: field_u32(row, 4).unwrap_or(0) & 0xFFFF,
                name,
                level: field_u32(row, 7).unwrap_or(0),
            })
        })
        .collect()
}

/// The official tables name monsters in Korean. Use the client's display
/// names (`JobNameTable`) when it has one for the class.
pub fn apply_monster_names(tables: &mut NavigationTables, names: &hashbrown::HashMap<u32, String>) {
    for mob in &mut tables.mobs {
        if let Some(name) = names.get(&mob.mob_id).filter(|name| !name.is_empty()) {
            mob.name = name.clone();
        }
    }
}

/// Parse all three tables from Lua source (tests and e2e fixtures).
pub fn navigation_tables_from_source(links: &str, npcs: &str, mobs: &str) -> mlua::Result<NavigationTables> {
    let mut tables = NavigationTables::default();
    for (name, source) in [("navi_link.lub", links), ("navi_npc.lub", npcs), ("navi_mob.lub", mobs)] {
        let lua = Lua::new();
        execute_chunk(&lua, name, source.as_bytes())?;
        match name {
            "navi_link.lub" => tables.links = parse_links(&lua),
            "navi_npc.lub" => tables.npcs = parse_npcs(&lua),
            _ => tables.mobs = parse_mobs(&lua),
        }
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::{navi_map_name, navigation_tables_from_source};

    #[test]
    fn parses_official_row_shapes() {
        let tables = navigation_tables_from_source(
            r#"Navi_Link = { { "prontera", 5001, 200, 99999, "", "", 156, 22, "prt_fild08", 170, 375 } }"#,
            r#"Navi_Npc = { { "prontera", 1, 101, 86, "Kafra Employee", "", 146, 89 }, { "prontera", 2, 101, 86, "", "", 1, 1 } }"#,
            r#"Navi_Mob = { { "prt_fild08", 1, 300, 1002, "Poring", "PORING", 1, 70 } }"#,
        )
        .unwrap();
        assert_eq!(tables.links.len(), 1);
        let link = &tables.links[0];
        assert_eq!((link.map.as_str(), link.kind, link.x, link.y), ("prontera", 200, 156, 22));
        assert_eq!((link.dest_map.as_str(), link.dest_x, link.dest_y), ("prt_fild08", 170, 375));
        // Nameless NPC rows are skipped, like roBrowser's search.
        assert_eq!(tables.npcs.len(), 1);
        assert_eq!((tables.npcs[0].x, tables.npcs[0].y), (146, 89));
        assert_eq!((tables.mobs[0].mob_id, tables.mobs[0].level), (1002, 1));
        let mut renamed = tables.clone();
        let names = hashbrown::HashMap::from([(1002, "Poring EN".to_owned())]);
        super::apply_monster_names(&mut renamed, &names);
        assert_eq!(renamed.mobs[0].name, "Poring EN");
        let packed = navigation_tables_from_source(
            "Navi_Link = {}",
            "Navi_Npc = {}",
            r#"Navi_Mob = { { "abbey01", 1, 300, 3736588, "x", "X", 61, 1 } }"#,
        )
        .unwrap();
        assert_eq!(packed.mobs[0].mob_id, 1036);
        assert_eq!(navi_map_name("Prontera.GAT"), "prontera");
    }
}
