//! Map weather, zone badge, and gat cell changes.
//!
//! Offline cases feed Hercules byte layouts through the logic session. The
//! live case logs into TEST on a normal map. This tree has no weather
//! mapflag a normal character can walk to.

use korangar::e2e::LogicSession;

/// `0x0091` change to `prontera.gat`.
const CHANGE_PRONTERA: &[u8] = &[
    0x91, 0x00, b'p', b'r', b'o', b'n', b't', b'e', b'r', b'a', b'.', b'g', b'a', b't', 0, 0, 0, 0, 0x00, 0x00, 0x64, 0x00,
];

/// `0x0091` change to `iz_int.gat`.
const CHANGE_IZ_INT: &[u8] = &[
    0x91, 0x00, b'i', b'z', b'_', b'i', b'n', b't', b'.', b'g', b'a', b't', 0, 0, 0, 0, 0, 0, 0x10, 0x00, 0x10, 0x00,
];

/// `0x01F3` snow, effect 162.
const SNOW: &[u8] = &[0xF3, 0x01, 0x02, 0x00, 0x00, 0x00, 0xA2, 0x00, 0x00, 0x00];

/// `0x01F3` effect 299 (`EF_THROWITEM2`), which is not weather.
const THROW_ITEM: &[u8] = &[0xF3, 0x01, 0x02, 0x00, 0x00, 0x00, 0x2B, 0x01, 0x00, 0x00];

/// `0x099B` type `0x28`, PvP flag.
const PVP: &[u8] = &[0x9B, 0x09, 0x28, 0x00, 0x01, 0x00, 0x00, 0x00];

/// `0x099B` type `0x28`, GvG and siege flags.
const GVG_SIEGE: &[u8] = &[0x9B, 0x09, 0x28, 0x00, 0x06, 0x00, 0x00, 0x00];

/// `0x099B` type `0x28`, no flags.
const PLAIN_PROPERTY: &[u8] = &[0x9B, 0x09, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00];

/// `0x01D6` map type 19, battlefield.
const BATTLEFIELD: &[u8] = &[0xD6, 0x01, 0x13, 0x00];

/// `0x0192` gat type 5 at (3, 0) on `prontera.gat`.
const ICE_WALL: &[u8] = &[
    0x92, 0x01, 0x03, 0x00, 0x00, 0x00, 0x05, 0x00, b'p', b'r', b'o', b'n', b't', b'e', b'r', b'a', b'.', b'g', b'a', b't', 0, 0, 0, 0,
];

#[test]
fn weather_starts_on_effect_and_clears_on_map_change() {
    let mut session = LogicSession::offline();
    session.feed_map_bytes(CHANGE_PRONTERA).unwrap();
    assert_eq!(session.loaded_map_name(), "prontera");
    session.feed_map_bytes(SNOW).unwrap();
    session.feed_map_bytes(THROW_ITEM).unwrap();
    assert_eq!(session.active_weather(), vec![korangar::e2e::WeatherKind::Snow]);
    assert_eq!(session.visible_weather(), vec![korangar::e2e::WeatherKind::Snow]);

    session.feed_map_bytes(CHANGE_IZ_INT).unwrap();
    assert_eq!(session.loaded_map_name(), "iz_int");
    assert!(session.active_weather().is_empty());
    assert!(session.visible_weather().is_empty());
    assert_eq!(session.map_badge(), None);
}

#[test]
fn show_weather_toggle_suppresses_the_visible_set() {
    let mut session = LogicSession::offline();
    session.feed_map_bytes(CHANGE_PRONTERA).unwrap();
    session.feed_map_bytes(SNOW).unwrap();
    session.set_show_weather(false);
    assert!(session.visible_weather().is_empty());
    assert_eq!(session.active_weather(), vec![korangar::e2e::WeatherKind::Snow]);
    session.set_show_weather(true);
    assert_eq!(session.visible_weather(), vec![korangar::e2e::WeatherKind::Snow]);
}

#[test]
fn map_property_packets_set_the_badge() {
    let mut session = LogicSession::offline();
    session.feed_map_bytes(CHANGE_PRONTERA).unwrap();
    session.feed_map_bytes(PLAIN_PROPERTY).unwrap();
    assert_eq!(session.map_badge(), None);
    session.feed_map_bytes(PVP).unwrap();
    assert_eq!(session.map_badge().as_deref(), Some("PvP"));
    session.feed_map_bytes(GVG_SIEGE).unwrap();
    assert_eq!(session.map_badge().as_deref(), Some("GvG"));
    session.feed_map_bytes(PLAIN_PROPERTY).unwrap();
    session.feed_map_bytes(BATTLEFIELD).unwrap();
    assert_eq!(session.map_badge().as_deref(), Some("BG"));
}

#[test]
fn cell_change_blocks_the_pathfinder() {
    let mut session = LogicSession::offline();
    session.feed_map_bytes(CHANGE_PRONTERA).unwrap();
    assert!(session.corridor_path_open());
    assert!(session.cell_is_walkable(3, 0));
    session.feed_map_bytes(ICE_WALL).unwrap();
    assert!(!session.cell_is_walkable(3, 0));
    assert!(!session.corridor_path_open());
}

#[test]
fn live_normal_map_has_no_weather_or_pvp_badge() {
    let Some(mut session) = super::live_session() else {
        return;
    };
    session.enter_world_for_weather().expect("enter world");
    let map = session.map_name().to_owned();
    assert!(session.active_weather().is_empty(), "weather on {map}");
    assert!(session.visible_weather().is_empty(), "visible weather on {map}");
    assert_eq!(session.map_badge(), None, "badge on {map}");
    eprintln!("weather map not reachable on TEST without GM");
}
