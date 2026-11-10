//! Signboard and town-info rows are map labels. They must not become entity job
//! ids.

use hashbrown::HashMap;
use korangar_loaders::FileLoader;
use mlua::{Lua, Value};

use super::lua::{execute_chunk, game_string};
use super::{Library, Table};
use crate::loaders::GameFileLoader;
use crate::settings::GameSettings;

const SIGNBOARD_PATH: &str = "data\\luafiles514\\lua files\\signboardlist.lub";
const TOWNINFO_PATH: &str = "System\\Towninfo.lub";

/// `KORANGAR_ENABLE_SIGNBOARDS` overrides the saved game setting.
/// An unset env uses the setting. A missing setting defaults to on.
/// An unrecognized env value stays off.
pub fn resolve_signboards_enabled(env_value: Option<&str>, settings_flag: Option<bool>) -> bool {
    if let Some(value) = env_value {
        return matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on");
    }
    settings_flag.unwrap_or(true)
}

pub fn signboards_enabled() -> bool {
    if let Ok(value) = std::env::var("KORANGAR_ENABLE_SIGNBOARDS") {
        return resolve_signboards_enabled(Some(value.as_str()), None);
    }
    // `GameSettings` writes the settings file on drop, so read the flag through a
    // probe.
    resolve_signboards_enabled(None, GameSettings::read_enable_signboards())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignboardIconKind {
    None,
    Bitmap,
    Sprite,
    Signboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TownServiceKind {
    ToolDealer,
    WeaponDealer,
    ArmorDealer,
    Smith,
    Guide,
    Inn,
    Kafra,
    StylingShop,
}

impl TownServiceKind {
    fn from_type_id(type_id: i32) -> Option<Self> {
        Some(match type_id {
            0 => Self::ToolDealer,
            1 => Self::WeaponDealer,
            2 => Self::ArmorDealer,
            3 => Self::Smith,
            4 => Self::Guide,
            5 => Self::Inn,
            6 => Self::Kafra,
            7 => Self::StylingShop,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignboardLabel {
    pub map: String,
    pub x: i32,
    pub y: i32,
    pub height: i32,
    pub icon_kind: SignboardIconKind,
    pub icon_path: Option<String>,
    pub caption: Option<String>,
    pub color: Option<String>,
}

impl SignboardLabel {
    /// Rows must not become entity job ids.
    pub fn vendor_job_id(&self) -> Option<u16> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TownServiceMarker {
    pub map: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub kind: TownServiceKind,
}

impl TownServiceMarker {
    pub fn vendor_job_id(&self) -> Option<u16> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MapSignboards {
    labels: Vec<SignboardLabel>,
    town_services: Vec<TownServiceMarker>,
}

impl MapSignboards {
    pub fn vendor_job_id_count(&self) -> usize {
        self.labels.iter().filter(|label| label.vendor_job_id().is_some()).count()
            + self.town_services.iter().filter(|marker| marker.vendor_job_id().is_some()).count()
    }
}

impl Table for MapSignboards {
    type Key<'a> = &'a str;
    type Storage = HashMap<String, Self>;

    fn load(game_file_loader: &GameFileLoader) -> mlua::Result<Self::Storage> {
        let enabled = signboards_enabled();
        let (labels, town_services) = if enabled {
            let labels = match game_file_loader.get(SIGNBOARD_PATH) {
                Ok(bytes) => parse_signboard_bytes(&bytes).unwrap_or_default(),
                Err(_) => Vec::new(),
            };
            let town_services = match game_file_loader.get(TOWNINFO_PATH) {
                Ok(bytes) => parse_towninfo_bytes(&bytes).unwrap_or_default(),
                Err(_) => Vec::new(),
            };
            (labels, town_services)
        } else {
            (Vec::new(), Vec::new())
        };
        Ok(publish_markers(enabled, labels, town_services))
    }

    fn try_get<'a, 'b>(library: &'a Library, key: Self::Key<'b>) -> Option<&'a Self> {
        library.signboard_table.get(normalize_map_name(key))
    }

    fn get<'a, 'b>(library: &'a Library, key: Self::Key<'b>) -> &'a Self {
        static DEFAULT: std::sync::OnceLock<MapSignboards> = std::sync::OnceLock::new();
        Self::try_get(library, key).unwrap_or_else(|| DEFAULT.get_or_init(MapSignboards::default))
    }
}

pub(crate) fn publish_markers(
    enabled: bool,
    labels: Vec<SignboardLabel>,
    town_services: Vec<TownServiceMarker>,
) -> HashMap<String, MapSignboards> {
    if !enabled {
        return HashMap::new();
    }
    group_markers(labels, town_services)
}

fn group_markers(labels: Vec<SignboardLabel>, town_services: Vec<TownServiceMarker>) -> HashMap<String, MapSignboards> {
    let mut grouped: HashMap<String, MapSignboards> = HashMap::new();
    for label in labels {
        grouped.entry(label.map.clone()).or_default().labels.push(label);
    }
    for marker in town_services {
        grouped.entry(marker.map.clone()).or_default().town_services.push(marker);
    }
    grouped
}

fn normalize_map_name(name: &str) -> &str {
    name.strip_suffix(".rsw").unwrap_or(name)
}

pub(crate) fn parse_signboard_bytes(bytes: &[u8]) -> mlua::Result<Vec<SignboardLabel>> {
    let state = Lua::new();
    execute_chunk(&state, SIGNBOARD_PATH, bytes)?;
    let globals = state.globals();
    let Ok(table) = globals.get::<mlua::Table>("SignBoardList") else {
        return Ok(Vec::new());
    };
    parse_signboard_table(&table)
}

pub(crate) fn parse_towninfo_bytes(bytes: &[u8]) -> mlua::Result<Vec<TownServiceMarker>> {
    let state = Lua::new();
    execute_chunk(&state, TOWNINFO_PATH, bytes)?;
    let globals = state.globals();
    let Ok(table) = globals.get::<mlua::Table>("mapNPCInfoTable") else {
        return Ok(Vec::new());
    };
    parse_towninfo_table(&table)
}

fn parse_signboard_table(table: &mlua::Table) -> mlua::Result<Vec<SignboardLabel>> {
    let mut labels = Vec::new();
    for row in table.sequence_values::<mlua::Table>().flatten() {
        let Some(label) = signboard_row(&row)? else {
            continue;
        };
        labels.push(label);
    }
    Ok(labels)
}

fn signboard_row(row: &mlua::Table) -> mlua::Result<Option<SignboardLabel>> {
    let Some(map) = lua_string(row, 1)? else {
        return Ok(None);
    };
    if map.eq_ignore_ascii_case("default") {
        return Ok(None);
    }
    let Some(x) = lua_i32(row, 2) else {
        return Ok(None);
    };
    let Some(y) = lua_i32(row, 3) else {
        return Ok(None);
    };
    let Some(icon_kind) = lua_i32(row, 5).and_then(icon_kind) else {
        return Ok(None);
    };
    if matches!(icon_kind, SignboardIconKind::None) {
        return Ok(None);
    }

    Ok(Some(SignboardLabel {
        map: map.to_ascii_lowercase(),
        x,
        y,
        height: lua_i32(row, 4).unwrap_or(0),
        icon_kind,
        icon_path: lua_string(row, 6)?,
        caption: lua_string(row, 7)?,
        color: lua_string(row, 8)?,
    }))
}

fn icon_kind(type_id: i32) -> Option<SignboardIconKind> {
    Some(match type_id {
        0 => SignboardIconKind::None,
        1 => SignboardIconKind::Bitmap,
        2 => SignboardIconKind::Sprite,
        3 => SignboardIconKind::Signboard,
        _ => return None,
    })
}

fn parse_towninfo_table(table: &mlua::Table) -> mlua::Result<Vec<TownServiceMarker>> {
    let mut markers = Vec::new();
    for pair in table.pairs::<Value, mlua::Table>().flatten() {
        let (key, rows) = pair;
        let Value::String(map_name) = key else {
            continue;
        };
        let map = game_string(map_name)?.to_ascii_lowercase();
        if map.is_empty() || map == "default" {
            continue;
        }
        for row in rows.sequence_values::<mlua::Table>().flatten() {
            let Some(marker) = town_row(&map, &row)? else {
                continue;
            };
            markers.push(marker);
        }
    }
    Ok(markers)
}

fn town_row(map: &str, row: &mlua::Table) -> mlua::Result<Option<TownServiceMarker>> {
    let Some(name) = named_string(row, "name")? else {
        return Ok(None);
    };
    if name.is_empty() {
        return Ok(None);
    }
    let Some(x) = named_i32(row, "X") else {
        return Ok(None);
    };
    let Some(y) = named_i32(row, "Y") else {
        return Ok(None);
    };
    // Unknown TYPE values are dropped. They are not NPC class ids and must not
    // become a spawned vendor.
    let Some(kind) = named_i32(row, "TYPE").and_then(TownServiceKind::from_type_id) else {
        return Ok(None);
    };
    Ok(Some(TownServiceMarker {
        map: map.to_owned(),
        name,
        x,
        y,
        kind,
    }))
}

fn lua_string(table: &mlua::Table, index: i32) -> mlua::Result<Option<String>> {
    match table.get::<Value>(index)? {
        Value::String(value) => Ok(Some(game_string(value)?)),
        Value::Nil => Ok(None),
        _ => Ok(None),
    }
}

fn named_string(table: &mlua::Table, name: &str) -> mlua::Result<Option<String>> {
    match table.get::<Value>(name)? {
        Value::String(value) => Ok(Some(game_string(value)?)),
        _ => Ok(None),
    }
}

fn lua_i32(table: &mlua::Table, index: i32) -> Option<i32> {
    value_i32(table.get::<Value>(index).ok()?)
}

fn named_i32(table: &mlua::Table, name: &str) -> Option<i32> {
    value_i32(table.get::<Value>(name).ok()?)
}

fn value_i32(value: Value) -> Option<i32> {
    match value {
        Value::Integer(value) => i32::try_from(value).ok(),
        Value::Number(value) if value.is_finite() => Some(value as i32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMBEDDED_SIGNBOARDS: &[u8] = include_bytes!("../../../archive/data/luafiles514/lua files/signboardlist.lub");
    const EMBEDDED_TOWNINFO: &[u8] = include_bytes!("../../../archive/System/Towninfo.lub");

    const SAMPLE_SIGNBOARDS: &str = r##"
        IT_NONE = 0
        IT_BMP = 1
        IT_SPRITE = 2
        IT_SIGNBOARD = 3
        SignBoardList = {
            { "Default", 157, 278, 0, IT_BMP, "ui\\collection\\flower.bmp", "Cannot enter", "#0x009EBDFF" },
            { "prontera", 82, 108, 0, IT_BMP, "ui\\information\\over_market.bmp" },
            { "prontera", 124, 76, 10, IT_SPRITE, "npc\\vending_box.spr", "Shop lookalike", "#0x00FFFFFF" },
            { "geffen", 132, 66, 10, IT_SIGNBOARD, "ui\\item\\suffragium.bmp", "Eden Warper", "#0x00FFFFFF" },
            { "alberta", 1, 2, 0, IT_NONE, "ignored.bmp" },
            { "payon", 3, 4, 0, 99, "not-a-sign.bmp", "dropped" },
        }
    "##;

    const SAMPLE_TOWNINFO: &str = r#"
        mapNPCInfoTable = {
            prontera = {
                { name = [=[Kafra Employee]=], X = 146, Y = 89, TYPE = 6 },
                { name = [=[Fake Vendor]=], X = 10, Y = 11, TYPE = 999 },
                { name = [=[Guide]=], X = 154, Y = 187, TYPE = 4 },
            },
            gef_fild00 = {
                { name = [=[Tool Dealer]=], X = 1, Y = 2, TYPE = 0 },
            },
        }
    "#;

    #[test]
    fn signboards_default_on_and_env_overrides() {
        assert!(resolve_signboards_enabled(None, None));
        assert!(resolve_signboards_enabled(None, Some(true)));
        assert!(!resolve_signboards_enabled(None, Some(false)));
        assert!(!resolve_signboards_enabled(Some("false"), Some(true)));
        assert!(!resolve_signboards_enabled(Some(""), Some(true)));
        assert!(resolve_signboards_enabled(Some("1"), Some(false)));
        assert!(resolve_signboards_enabled(Some(" YES "), None));
    }

    #[test]
    fn embedded_real_rows_are_labels_without_vendor_jobs() {
        let labels = parse_signboard_bytes(EMBEDDED_SIGNBOARDS).expect("signboardlist.lub");
        let towns = parse_towninfo_bytes(EMBEDDED_TOWNINFO).expect("Towninfo.lub");
        assert!(labels.len() > 10, "expected ROenglishRE caption rows");
        assert!(towns.len() > 10, "expected real town service markers");
        assert!(labels.iter().all(|label| label.vendor_job_id().is_none()));
        assert!(towns.iter().all(|marker| marker.vendor_job_id().is_none()));
        assert!(labels.iter().all(|label| label.map != "default"));

        let published = publish_markers(true, labels, towns);
        let prontera = published.get("prontera").expect("prontera labels");
        assert!(
            prontera
                .labels
                .iter()
                .any(|label| { label.icon_kind == SignboardIconKind::Signboard && label.caption.as_deref() == Some("  Eden Warper") })
        );
        assert!(prontera.town_services.iter().any(|marker| {
            marker.kind == TownServiceKind::Kafra && marker.name == "Kafra Employee" && marker.x == 146 && marker.y == 89
        }));
        assert!(prontera.town_services.iter().any(|marker| marker.kind == TownServiceKind::Guide));
        assert!(
            prontera
                .town_services
                .iter()
                .any(|marker| marker.kind == TownServiceKind::ToolDealer)
        );
        assert_eq!(prontera.vendor_job_id_count(), 0);

        let total_jobs: usize = published.values().map(MapSignboards::vendor_job_id_count).sum();
        assert_eq!(total_jobs, 0);
    }

    #[test]
    fn disabled_flag_drops_parsed_renewal_rows() {
        let labels = parse_signboard_bytes(SAMPLE_SIGNBOARDS.as_bytes()).unwrap();
        let towns = parse_towninfo_bytes(SAMPLE_TOWNINFO.as_bytes()).unwrap();
        assert_eq!(labels.len(), 3);
        assert_eq!(towns.len(), 3);
        assert!(publish_markers(false, labels, towns).is_empty());
    }

    #[test]
    fn sprite_rows_are_labels_and_unknown_town_types_are_dropped() {
        let labels = parse_signboard_bytes(SAMPLE_SIGNBOARDS.as_bytes()).unwrap();
        let towns = parse_towninfo_bytes(SAMPLE_TOWNINFO.as_bytes()).unwrap();
        let published = publish_markers(true, labels, towns);

        let prontera = published.get("prontera").unwrap();
        assert_eq!(prontera.labels.len(), 2);
        assert!(
            prontera
                .labels
                .iter()
                .any(|label| { label.icon_kind == SignboardIconKind::Sprite && label.caption.as_deref() == Some("Shop lookalike") })
        );
        assert!(prontera.labels.iter().all(|label| label.vendor_job_id().is_none()));
        assert_eq!(prontera.town_services.len(), 2);
        assert!(prontera.town_services.iter().all(|marker| marker.name != "Fake Vendor"));
        assert!(prontera.town_services.iter().any(|marker| marker.kind == TownServiceKind::Kafra));
        assert_eq!(prontera.vendor_job_id_count(), 0);

        let geffen = published.get("geffen").unwrap();
        assert_eq!(geffen.labels[0].icon_kind, SignboardIconKind::Signboard);
        assert_eq!(geffen.labels[0].height, 10);
        assert!(published.get("alberta").is_none());
        assert!(published.get("payon").is_none());
        assert!(published.get("default").is_none());
        assert_eq!(
            published.get("gef_fild00").unwrap().town_services[0].kind,
            TownServiceKind::ToolDealer
        );
    }
}
