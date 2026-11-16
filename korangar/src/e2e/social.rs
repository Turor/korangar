//! Clan, attendance, achievements and navigation scenarios.
//!
//! Offline entry points run the client's own parsers and state reducers on
//! Lua fixtures shaped like the official tables, so the e2e binary checks the
//! same code the windows draw from. The live attendance check is
//! [`LogicSession::request_attendance_live`].

use std::sync::Arc;

use crate::state::achievements::Achievements;
use crate::state::chat::{OutgoingChat, clan_chat_line, parse_outgoing_chat};
use crate::state::navigation::Navigation;
use crate::world::{AchievementCatalog, AttendanceTable, merge_achievement_table, navigation_tables_from_source, parse_attendance_chunk};

/// What the Navigation window would show.
#[derive(Clone, Debug, Default)]
pub struct NavigationReport {
    pub status: String,
    pub hits: Vec<String>,
    pub steps: Vec<String>,
    pub guide: String,
}

/// Parses the three `navi_*` fixtures, searches `query`, routes to the first
/// hit from `start` and returns the window text.
pub fn navigation_scenario(
    links: &str,
    npcs: &str,
    mobs: &str,
    start: (&str, u16, u16),
    query: &str,
    services: bool,
) -> Result<NavigationReport, String> {
    let tables = navigation_tables_from_source(links, npcs, mobs).map_err(|error| error.to_string())?;
    let mut navigation = Navigation::default();
    navigation.attach_navigation_tables(Arc::new(tables));
    navigation.on_map_change(start.0, start.1, start.2);
    if services {
        navigation.toggle_route_services();
    }
    navigation.set_route_query(query);
    navigation.search_route_targets();
    let hits = navigation.route().hits().iter().map(|hit| hit.label.clone()).collect();
    if !navigation.route().hits().is_empty() {
        navigation.choose_route_target(0);
    }
    Ok(report(&navigation, hits))
}

/// Moves the player in an existing scenario: a new map or a new cell.
pub fn navigation_follow(
    links: &str,
    npcs: &str,
    mobs: &str,
    start: (&str, u16, u16),
    query: &str,
    moves: &[(&str, u16, u16)],
) -> Result<Vec<NavigationReport>, String> {
    let tables = navigation_tables_from_source(links, npcs, mobs).map_err(|error| error.to_string())?;
    let mut navigation = Navigation::default();
    navigation.attach_navigation_tables(Arc::new(tables));
    navigation.on_map_change(start.0, start.1, start.2);
    navigation.set_route_query(query);
    navigation.search_route_targets();
    navigation.choose_route_target(0);
    let mut reports = vec![report(&navigation, Vec::new())];
    for (map, x, y) in moves {
        if *map == navigation.map_name() {
            navigation.set_player_cell(*x, *y);
        } else {
            navigation.on_map_change(map, *x, *y);
        }
        reports.push(report(&navigation, Vec::new()));
    }
    Ok(reports)
}

fn report(navigation: &Navigation, hits: Vec<String>) -> NavigationReport {
    NavigationReport {
        status: navigation.route().status.clone(),
        hits,
        steps: navigation.route().steps().iter().map(|step| step.label.clone()).collect(),
        guide: navigation.route().guide.clone(),
    }
}

/// What the Achievements window would show.
#[derive(Clone, Debug, Default)]
pub struct AchievementReport {
    pub header: String,
    pub rows: Vec<String>,
    pub detail: Vec<String>,
    pub gauges: Vec<String>,
    pub claimable: Option<u32>,
}

/// `entries` are `(id, completed, rewarded)` as `ZC_ACHIEVEMENT_LIST` sends.
/// `tab` is `(major, minor)`; `select` picks a row for the detail panel.
pub fn achievement_scenario(
    base: &str,
    overlay: &str,
    entries: &[(u32, bool, bool)],
    tab: (u8, Option<u8>),
    select: Option<u32>,
) -> Result<AchievementReport, String> {
    let mut catalog = AchievementCatalog::new();
    for (name, source) in [("achievement_list.lub", base), ("achievements.lub", overlay)] {
        let lua = mlua::Lua::new();
        crate::world::execute_lua_chunk(&lua, name, source.as_bytes()).map_err(|error| error.to_string())?;
        merge_achievement_table(&lua, &mut catalog);
    }
    let mut achievements = Achievements::default();
    achievements.attach_tables(Arc::new(catalog), Arc::new(AttendanceTable::default()));
    let data = entries
        .iter()
        .map(|(id, completed, rewarded)| ragnarok_packets::AchievementData {
            acheivement_id: *id,
            is_completed: u8::from(*completed),
            objectives: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            completion_timestamp: 0,
            got_rewarded: u8::from(*rewarded),
        })
        .collect();
    achievements.replace_list(0, 1, 0, 10, data);
    achievements.select_category(tab.0, tab.1);
    if let Some(id) = select {
        achievements.select_achievement(id);
    }
    Ok(AchievementReport {
        header: achievements.lines().first().cloned().unwrap_or_default(),
        rows: achievements.rows().iter().map(|row| row.text.clone()).collect(),
        detail: achievements.detail().to_vec(),
        gauges: achievements
            .category_progress()
            .iter()
            .map(|gauge| format!("{} {}/{}", gauge.name, gauge.completed, gauge.total))
            .collect(),
        claimable: achievements.claimable_id(),
    })
}

/// Calendar cell labels (`Day N\nItem xA`) and states for a CheckAttendance
/// fixture and a `ZC_UI_OPEN` type 7 payload.
pub fn attendance_scenario(check_attendance: &str, days_attended: u32, claimed_today: bool) -> (Vec<String>, Vec<String>, String) {
    let table = parse_attendance_chunk("CheckAttendance.lub", check_attendance.as_bytes());
    let mut achievements = Achievements::default();
    achievements.attach_tables(Arc::new(AchievementCatalog::new()), Arc::new(table));
    achievements.open_attendance(days_attended, claimed_today, &|item_id| format!("Item {item_id}"));
    let labels = achievements.attendance_cells().iter().map(|cell| cell.label.clone()).collect();
    let states = achievements
        .attendance_cells()
        .iter()
        .map(|cell| format!("{:?}", cell.state))
        .collect();
    (labels, states, achievements.attendance_status.clone())
}

/// `/cl` routing and the incoming chat line, as the chat window sees them.
pub fn clan_chat_scenario(typed: &str, incoming: &str) -> (Option<String>, String) {
    let outgoing = match parse_outgoing_chat(typed, "", false, false) {
        OutgoingChat::Clan(text) => Some(text),
        _ => None,
    };
    (outgoing, clan_chat_line(incoming))
}
