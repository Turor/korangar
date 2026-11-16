//! World-map routing and achievement reward names in logic mode.
//!
//! [`map_route_scenario`] runs the window state the world map's right-click
//! drives (`InputEvent::NavigateToMap` → [`Navigation::route_to_map`]) on
//! `navi_*` fixtures. [`reward_detail_scenario`] builds the reward names the
//! client resolves at startup ([`RewardNames::resolve`] with a parsed
//! `titletable.lub` and fixture item and skill names) and reports the
//! Achievements detail panel and the reward icon it would request.

use std::sync::Arc;

use hashbrown::HashMap;

use crate::state::achievements::{Achievements, RewardNames};
use crate::state::navigation::Navigation;
use crate::world::{AchievementCatalog, AttendanceTable, merge_achievement_table, navigation_tables_from_source, parse_title_table};

/// Navigation window text after one step of a world-map route.
#[derive(Clone, Debug, Default)]
pub struct MapRouteReport {
    pub status: String,
    pub steps: Vec<String>,
    pub guide: String,
}

/// Right-click `map_id` (labelled `name`) on the world map while standing at
/// `start`, then follow `moves`. One report before the first move and one
/// after each.
pub fn map_route_scenario(
    navi: (&str, &str, &str),
    start: (&str, u16, u16),
    map_id: &str,
    name: &str,
    moves: &[(&str, u16, u16)],
) -> Result<Vec<MapRouteReport>, String> {
    let (links, npcs, mobs) = navi;
    let tables = navigation_tables_from_source(links, npcs, mobs).map_err(|error| error.to_string())?;
    let mut navigation = Navigation::default();
    navigation.attach_navigation_tables(Arc::new(tables));
    navigation.on_map_change(start.0, start.1, start.2);
    navigation.route_to_map(map_id, name);
    let mut reports = vec![report(&navigation)];
    for (map, x, y) in moves {
        if *map == navigation.map_name() {
            navigation.set_player_cell(*x, *y);
        } else {
            navigation.on_map_change(map, *x, *y);
        }
        reports.push(report(&navigation));
    }
    Ok(reports)
}

fn report(navigation: &Navigation) -> MapRouteReport {
    MapRouteReport {
        status: navigation.route().status.clone(),
        steps: navigation.route().steps().iter().map(|step| step.label.clone()).collect(),
        guide: navigation.route().guide.clone(),
    }
}

/// Detail panel lines plus the reward item whose icon the window would ask
/// for.
#[derive(Clone, Debug, Default)]
pub struct RewardDetailReport {
    pub detail: Vec<String>,
    pub reward_item: Option<u32>,
    pub missing_icons: Vec<u32>,
}

/// `achievement_tbl` and `title_tbl` fixtures, item and skill names, then
/// select `achievement_id`.
pub fn reward_detail_scenario(
    achievement_source: &str,
    title_source: &str,
    item_names: &[(u32, &str)],
    skill_names: &[(u32, &str)],
    achievement_id: u32,
) -> Result<RewardDetailReport, String> {
    let mut catalog = AchievementCatalog::new();
    let lua = mlua::Lua::new();
    crate::world::execute_lua_chunk(&lua, "achievement_list.lub", achievement_source.as_bytes()).map_err(|error| error.to_string())?;
    merge_achievement_table(&lua, &mut catalog);
    let titles = parse_title_table(title_source.as_bytes()).map_err(|error| error.to_string())?;
    let items: HashMap<u32, &str> = item_names.iter().copied().collect();
    let skills: HashMap<u32, &str> = skill_names.iter().copied().collect();
    let names = RewardNames::resolve(
        &catalog,
        |id| items.get(&id).map(|name| (*name).to_owned()),
        |id| titles.get(&id).cloned(),
        |id| skills.get(&id).map(|name| (*name).to_owned()),
    );
    let mut achievements = Achievements::default();
    achievements.attach_reward_names(names);
    achievements.attach_tables(Arc::new(catalog), Arc::new(AttendanceTable::default()));
    achievements.select_achievement(achievement_id);
    Ok(RewardDetailReport {
        detail: achievements.detail().to_vec(),
        reward_item: achievements.selected_reward_item(),
        missing_icons: achievements.missing_item_icons(),
    })
}
