//! Route search and path guide for the Navigation window (roBrowser
//! `UI/Components/Navigation` + `MapPathFinder`).
//!
//! Search runs over `Navi_Npc` / `Navi_Mob`. Routes run Dijkstra over
//! `Navi_Link`, fewest warps first and then fewest walked cells, from the
//! player's cell to the destination. The guide line names the next waypoint
//! on the current map with its distance and compass direction.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

use hashbrown::HashMap;
use korangar_interface::element::StateElement;
use rust_state::RustState;

use super::{Navigation, normalize_map_name};
use crate::world::{NaviLink, NavigationTables};

/// Warp portals and NPC warps, roBrowser's default.
pub const WALK_LINK_KINDS: &[u32] = &[200, 201];
/// With the Services box ticked: Kafra, airship and other paid links too.
pub const SERVICE_LINK_KINDS: &[u32] = &[200, 201, 202, 203, 204, 205];
/// Search needs at least this many characters, like roBrowser.
pub const MIN_QUERY: usize = 2;
const MAX_HITS: usize = 40;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchKind {
    #[default]
    All,
    Npc,
    Mob,
}

impl SearchKind {
    pub fn cycle(self) -> Self {
        match self {
            Self::All => Self::Npc,
            Self::Npc => Self::Mob,
            Self::Mob => Self::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Npc => "NPC",
            Self::Mob => "MOB",
        }
    }
}

/// One search result row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub is_monster: bool,
    pub name: String,
    pub map: String,
    pub x: u16,
    pub y: u16,
    /// Monster rows have no cell; the route ends anywhere on the map.
    pub has_cell: bool,
    pub label: String,
}

/// One leg of a route. `via` is the next map when the step is a warp.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteStep {
    pub map: String,
    pub x: u16,
    pub y: u16,
    pub via: Option<String>,
    pub has_cell: bool,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteDestination {
    pub map: String,
    pub x: u16,
    pub y: u16,
    pub has_cell: bool,
    pub name: String,
    /// Picked on the world map: reaching the map is the whole goal.
    pub whole_map: bool,
}

pub fn cell_distance(from: (u16, u16), to: (u16, u16)) -> u32 {
    let dx = (i32::from(from.0) - i32::from(to.0)).unsigned_abs();
    let dy = (i32::from(from.1) - i32::from(to.1)).unsigned_abs();
    dx.max(dy)
}

/// Eight-way compass name from `from` to `to`. Cell Y grows north.
pub fn compass(from: (u16, u16), to: (u16, u16)) -> &'static str {
    let dx = f32::from(to.0) - f32::from(from.0);
    let dy = f32::from(to.1) - f32::from(from.1);
    if dx == 0.0 && dy == 0.0 {
        return "here";
    }
    let angle = dy.atan2(dx).to_degrees();
    let sector = (((angle + 360.0 + 22.5) % 360.0) / 45.0) as usize;
    ["E", "NE", "N", "NW", "W", "SW", "S", "SE"][sector.min(7)]
}

/// Case-insensitive substring search, at most 40 rows. ALL also matches map
/// names (the official NPC table is Korean, map names are not); then NPCs,
/// then monsters, each spawn map listed once.
pub fn search(tables: &NavigationTables, query: &str, kind: SearchKind) -> Vec<SearchHit> {
    let query = query.trim().to_lowercase();
    if query.chars().count() < MIN_QUERY {
        return Vec::new();
    }
    let mut hits = Vec::new();
    if kind == SearchKind::All {
        let mut maps: Vec<&str> = tables
            .links
            .iter()
            .flat_map(|link| [link.map.as_str(), link.dest_map.as_str()])
            .filter(|map| map.contains(&query))
            .collect();
        maps.sort_unstable();
        maps.dedup();
        for map in maps.into_iter().take(MAX_HITS / 4) {
            hits.push(SearchHit {
                is_monster: false,
                name: map.to_owned(),
                map: map.to_owned(),
                x: 0,
                y: 0,
                has_cell: false,
                label: format!("[MAP] {map}"),
            });
        }
    }
    if kind != SearchKind::Mob {
        for npc in tables.npcs.iter().filter(|npc| npc.name.to_lowercase().contains(&query)) {
            hits.push(SearchHit {
                is_monster: false,
                name: npc.name.clone(),
                map: npc.map.clone(),
                x: npc.x,
                y: npc.y,
                has_cell: true,
                label: format!("[NPC] {} — {} {},{}", npc.name, npc.map, npc.x, npc.y),
            });
            if hits.len() >= MAX_HITS {
                return hits;
            }
        }
    }
    if kind != SearchKind::Npc {
        let mut seen: Vec<(u32, &str)> = Vec::new();
        for mob in tables.mobs.iter().filter(|mob| mob.name.to_lowercase().contains(&query)) {
            if seen.contains(&(mob.mob_id, mob.map.as_str())) {
                continue;
            }
            seen.push((mob.mob_id, mob.map.as_str()));
            let level = if mob.level > 0 {
                format!(" Lv {}", mob.level)
            } else {
                String::new()
            };
            hits.push(SearchHit {
                is_monster: true,
                name: mob.name.clone(),
                map: mob.map.clone(),
                x: 0,
                y: 0,
                has_cell: false,
                label: format!("[MOB] {}{level} — {}", mob.name, mob.map),
            });
            if hits.len() >= MAX_HITS {
                break;
            }
        }
    }
    hits
}

fn link_verb(link: &NaviLink) -> String {
    match link.kind {
        200 => "warp portal".to_owned(),
        201 if link.name.is_empty() => "NPC warp".to_owned(),
        201 => format!("NPC warp ({})", link.name),
        _ if link.name.is_empty() => "service".to_owned(),
        _ => format!("service ({})", link.name),
    }
}

fn final_step(destination: &RouteDestination) -> RouteStep {
    let label = if destination.whole_map {
        format!("{}: arrive in {}", destination.map, destination.name)
    } else if destination.has_cell {
        format!(
            "{}: {} at {},{}",
            destination.map, destination.name, destination.x, destination.y
        )
    } else {
        format!("{}: {} (anywhere on the map)", destination.map, destination.name)
    };
    RouteStep {
        map: destination.map.clone(),
        x: destination.x,
        y: destination.y,
        via: None,
        has_cell: destination.has_cell,
        label,
    }
}

/// Fewest warps, then fewest walked cells. `None` when no link chain of the
/// allowed kinds reaches the destination map.
pub fn find_route(links: &[NaviLink], start: (&str, u16, u16), destination: &RouteDestination, kinds: &[u32]) -> Option<Vec<RouteStep>> {
    let start_map = normalize_map_name(start.0);
    let end_map = normalize_map_name(&destination.map);
    if start_map == end_map {
        return Some(vec![final_step(destination)]);
    }

    let mut by_map: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, link) in links.iter().enumerate() {
        if kinds.contains(&link.kind) && link.map != link.dest_map {
            by_map.entry(link.map.as_str()).or_default().push(index);
        }
    }

    struct Best {
        hops: u32,
        cells: u32,
        arrival: (u16, u16),
        previous: Option<(String, usize)>,
    }
    let mut best: HashMap<String, Best> = HashMap::new();
    best.insert(start_map.clone(), Best {
        hops: 0,
        cells: 0,
        arrival: (start.1, start.2),
        previous: None,
    });
    let mut queue = BinaryHeap::new();
    queue.push(Reverse((0u32, 0u32, start_map.clone())));

    while let Some(Reverse((hops, cells, map))) = queue.pop() {
        let Some(current) = best.get(&map) else { continue };
        if (current.hops, current.cells) < (hops, cells) {
            continue;
        }
        if map == end_map {
            break;
        }
        let arrival = current.arrival;
        let Some(outgoing) = by_map.get(map.as_str()) else { continue };
        for &index in outgoing {
            let link = &links[index];
            let next = (hops + 1, cells + cell_distance(arrival, (link.x, link.y)));
            let better = best.get(&link.dest_map).is_none_or(|known| next < (known.hops, known.cells));
            if better {
                best.insert(link.dest_map.clone(), Best {
                    hops: next.0,
                    cells: next.1,
                    arrival: (link.dest_x, link.dest_y),
                    previous: Some((map.clone(), index)),
                });
                queue.push(Reverse((next.0, next.1, link.dest_map.clone())));
            }
        }
    }

    best.get(&end_map)?;
    let mut legs = Vec::new();
    let mut cursor = end_map.clone();
    while let Some((previous, index)) = best.get(&cursor).and_then(|entry| entry.previous.clone()) {
        legs.push(index);
        cursor = previous;
        if legs.len() > links.len() {
            return None;
        }
    }
    legs.reverse();
    let mut steps: Vec<RouteStep> = legs
        .into_iter()
        .map(|index| {
            let link = &links[index];
            RouteStep {
                map: link.map.clone(),
                x: link.x,
                y: link.y,
                via: Some(link.dest_map.clone()),
                has_cell: true,
                label: format!(
                    "{}: {} at {},{} to {}",
                    link.map,
                    link_verb(link),
                    link.x,
                    link.y,
                    link.dest_map
                ),
            }
        })
        .collect();
    steps.push(final_step(destination));
    Some(steps)
}

/// Shared catalog handle. `Debug` prints table sizes only.
#[derive(Clone, Default)]
pub struct TablesHandle(Option<Arc<NavigationTables>>);

impl std::fmt::Debug for TablesHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(tables) => tables.fmt(formatter),
            None => formatter.write_str("NavigationTables(unloaded)"),
        }
    }
}

/// Window state for search, the active route and the guide line.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct NavigationRoute {
    /// Search text box contents.
    pub query: String,
    /// "3 results", "Route: 2 warps", or why there is none.
    pub status: String,
    /// Next waypoint with distance and direction.
    pub guide: String,
    #[hidden_element]
    kind: SearchKind,
    #[hidden_element]
    services: bool,
    #[hidden_element]
    hits: Vec<SearchHit>,
    #[hidden_element]
    steps: Vec<RouteStep>,
    #[hidden_element]
    destination: Option<RouteDestination>,
    #[hidden_element]
    tables: TablesHandle,
}

impl NavigationRoute {
    #[cfg(test)]
    pub fn services(&self) -> bool {
        self.services
    }

    pub fn hits(&self) -> &[SearchHit] {
        &self.hits
    }

    pub fn steps(&self) -> &[RouteStep] {
        &self.steps
    }

    #[cfg(test)]
    pub fn destination(&self) -> Option<&RouteDestination> {
        self.destination.as_ref()
    }

    #[cfg(test)]
    pub fn has_tables(&self) -> bool {
        self.tables.0.as_ref().is_some_and(|tables| !tables.is_empty())
    }

    /// `ALL · Services off` for the filter row.
    pub fn filter_line(&self) -> String {
        format!(
            "Search {} · Services {}",
            self.kind.label(),
            if self.services { "on" } else { "off" }
        )
    }

    /// Steps on `map`, in route order.
    pub fn steps_on<'a>(&'a self, map: &'a str) -> impl Iterator<Item = &'a RouteStep> + 'a {
        self.steps.iter().filter(move |step| step.map == map)
    }

    /// The tables survive logout; everything else resets.
    fn cleared(&self) -> Self {
        Self {
            tables: self.tables.clone(),
            services: self.services,
            kind: self.kind,
            ..Self::default()
        }
    }
}

impl Navigation {
    pub fn route(&self) -> &NavigationRoute {
        &self.route
    }

    pub fn attach_navigation_tables(&mut self, tables: Arc<NavigationTables>) {
        self.route.tables = TablesHandle(Some(tables));
    }

    /// `clear` keeps the loaded tables and the filter choices.
    pub(super) fn cleared_route(&self) -> NavigationRoute {
        self.route.cleared()
    }

    pub fn set_route_query(&mut self, query: &str) {
        self.route.query = query.to_owned();
    }

    pub fn search_route_targets(&mut self) {
        let route = &mut self.route;
        let Some(tables) = route.tables.0.clone() else {
            route.hits.clear();
            route.status = "No navigation data in the client files".to_owned();
            return;
        };
        if route.query.trim().chars().count() < MIN_QUERY {
            route.hits.clear();
            route.status = format!("Type at least {MIN_QUERY} letters");
            return;
        }
        route.hits = search(&tables, &route.query, route.kind);
        route.status = match route.hits.len() {
            0 => format!("No {} matches for \"{}\"", route.kind.label(), route.query.trim()),
            1 => "1 result".to_owned(),
            count => format!("{count} results"),
        };
    }

    pub fn cycle_route_search_kind(&mut self) {
        self.route.kind = self.route.kind.cycle();
        if !self.route.hits.is_empty() {
            self.search_route_targets();
        }
    }

    pub fn toggle_route_services(&mut self) {
        self.route.services = !self.route.services;
        self.refresh_route();
    }

    pub fn choose_route_target(&mut self, index: usize) {
        let Some(hit) = self.route.hits.get(index).cloned() else {
            return;
        };
        self.route.destination = Some(RouteDestination {
            map: hit.map,
            x: hit.x,
            y: hit.y,
            has_cell: hit.has_cell,
            name: hit.name,
            whole_map: false,
        });
        self.refresh_route();
    }

    /// World map right-click: route to `map` itself, arriving anywhere on it.
    /// `name` is the world map's label for the square (`Prontera`).
    pub fn route_to_map(&mut self, map: &str, name: &str) {
        let map = normalize_map_name(map);
        if map.is_empty() {
            return;
        }
        let name = if name.trim().is_empty() {
            map.clone()
        } else {
            name.trim().to_owned()
        };
        self.route.destination = Some(RouteDestination {
            map,
            x: 0,
            y: 0,
            has_cell: false,
            name,
            whole_map: true,
        });
        self.refresh_route();
    }

    /// Drops the route and the server target.
    pub fn clear_route(&mut self) {
        self.route.destination = None;
        self.route.steps.clear();
        self.route.guide.clear();
        self.route.status = "Route cleared".to_owned();
        self.target = None;
    }

    /// `0x08E2` target. Service flags turn on paid links for this route.
    pub(super) fn route_to_server_target(&mut self) {
        let Some(target) = self.target.clone() else { return };
        let has_cell = target.target_type != 3 && (target.x != 0 || target.y != 0);
        let name = if target.monster_id != 0 {
            format!("monster {}", target.monster_id)
        } else {
            "navigation target".to_owned()
        };
        if target.service_flags != 0 {
            self.route.services = true;
        }
        self.route.destination = Some(RouteDestination {
            map: normalize_map_name(&target.map_name),
            x: target.x,
            y: target.y,
            has_cell,
            name,
            whole_map: false,
        });
        self.refresh_route();
    }

    /// Recompute from the player's map and cell. Called on map change.
    pub(super) fn refresh_route(&mut self) {
        let Some(destination) = self.route.destination.clone() else {
            self.route.steps.clear();
            self.route.guide.clear();
            return;
        };
        if self.map_name.is_empty() {
            self.route.steps = vec![final_step(&destination)];
            self.route.status = "Waiting for the current map".to_owned();
            self.refresh_guide();
            return;
        }
        let kinds = if self.route.services { SERVICE_LINK_KINDS } else { WALK_LINK_KINDS };
        let links: &[NaviLink] = self.route.tables.0.as_deref().map(|tables| tables.links.as_slice()).unwrap_or(&[]);
        match find_route(links, (&self.map_name, self.player_x, self.player_y), &destination, kinds) {
            Some(steps) => {
                let warps = steps.len() - 1;
                self.route.status = match warps {
                    0 => format!("Route: on this map ({})", destination.name),
                    1 => format!("Route: 1 warp to {}", destination.map),
                    count => format!("Route: {count} warps to {}", destination.map),
                };
                self.route.steps = steps;
            }
            None => {
                self.route.steps.clear();
                self.route.status = if self.route.services {
                    format!("No known route from {} to {}", self.map_name, destination.map)
                } else {
                    format!("No walking route from {} to {}; try Services", self.map_name, destination.map)
                };
            }
        }
        self.refresh_guide();
    }

    /// Next waypoint on the current map. Called when the player's cell moves.
    pub(super) fn refresh_guide(&mut self) {
        if self.route.destination.is_none() {
            self.route.guide.clear();
            return;
        }
        let player = (self.player_x, self.player_y);
        let next = self.route.steps.iter().find(|step| step.map == self.map_name);
        self.route.guide = match next {
            None if self.route.steps.is_empty() => String::new(),
            None => format!("Off route on {}; head back to {}", self.map_name, self.route.steps[0].map),
            Some(step) if !step.has_cell && self.route.destination.as_ref().is_some_and(|destination| destination.whole_map) => {
                format!("Arrived: {} ({})", final_name(&self.route), step.map)
            }
            Some(step) if !step.has_cell => format!("You are on {}. Look for {}.", step.map, final_name(&self.route)),
            Some(step) => {
                let distance = cell_distance(player, (step.x, step.y));
                let direction = compass(player, (step.x, step.y));
                match &step.via {
                    Some(dest) => format!("Next: warp at {},{} to {dest} — {distance} cells {direction}", step.x, step.y),
                    None if distance <= 2 => format!("Arrived: {}", final_name(&self.route)),
                    None => format!("Destination {},{} — {distance} cells {direction}", step.x, step.y),
                }
            }
        };
    }
}

fn final_name(route: &NavigationRoute) -> &str {
    route
        .destination
        .as_ref()
        .map(|destination| destination.name.as_str())
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{RouteDestination, SERVICE_LINK_KINDS, SearchKind, WALK_LINK_KINDS, compass, find_route, search};
    use crate::state::navigation::Navigation;
    use crate::world::{NaviLink, NaviMob, NaviNpc, NavigationTables};

    fn link(map: &str, kind: u32, x: u16, y: u16, dest: &str, dx: u16, dy: u16) -> NaviLink {
        NaviLink {
            map: map.into(),
            id: 0,
            kind,
            name: String::new(),
            x,
            y,
            dest_map: dest.into(),
            dest_x: dx,
            dest_y: dy,
        }
    }

    fn tables() -> NavigationTables {
        NavigationTables {
            links: vec![
                link("prontera", 200, 156, 22, "prt_fild08", 170, 375),
                link("prt_fild08", 200, 170, 20, "izlude", 128, 200),
                // A longer chain that also reaches izlude.
                link("prontera", 200, 20, 200, "prt_fild05", 380, 200),
                link("prt_fild05", 200, 200, 20, "prt_fild06", 200, 380),
                link("prt_fild06", 200, 380, 200, "izlude", 20, 100),
                // A Kafra service straight to geffen.
                link("prontera", 202, 146, 89, "geffen", 120, 60),
            ],
            npcs: vec![NaviNpc {
                map: "izlude".into(),
                id: 1,
                name: "Kafra Employee".into(),
                x: 134,
                y: 87,
            }],
            mobs: vec![
                NaviMob {
                    map: "prt_fild08".into(),
                    mob_id: 1002,
                    name: "Poring".into(),
                    level: 1,
                },
                NaviMob {
                    map: "prt_fild08".into(),
                    mob_id: 1002,
                    name: "Poring".into(),
                    level: 1,
                },
            ],
        }
    }

    fn to(map: &str, x: u16, y: u16) -> RouteDestination {
        RouteDestination {
            map: map.into(),
            x,
            y,
            has_cell: true,
            name: "target".into(),
            whole_map: false,
        }
    }

    #[test]
    fn route_prefers_fewest_warps() {
        let tables = tables();
        let steps = find_route(&tables.links, ("prontera", 150, 150), &to("izlude", 134, 87), WALK_LINK_KINDS).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].via.as_deref(), Some("prt_fild08"));
        assert_eq!((steps[0].x, steps[0].y), (156, 22));
        assert_eq!(steps[1].via.as_deref(), Some("izlude"));
        assert_eq!(steps[2].via, None);
        assert_eq!((steps[2].x, steps[2].y), (134, 87));
    }

    #[test]
    fn services_open_paid_links() {
        let tables = tables();
        assert!(find_route(&tables.links, ("prontera", 150, 150), &to("geffen", 1, 1), WALK_LINK_KINDS).is_none());
        let steps = find_route(
            &tables.links,
            ("prontera.gat", 150, 150),
            &to("geffen", 1, 1),
            SERVICE_LINK_KINDS,
        )
        .unwrap();
        assert_eq!(steps.len(), 2);
        assert!(steps[0].label.contains("service"));
        let same = find_route(&tables.links, ("geffen", 1, 1), &to("geffen", 50, 50), WALK_LINK_KINDS).unwrap();
        assert_eq!(same.len(), 1);
    }

    #[test]
    fn search_filters_and_dedupes() {
        let tables = tables();
        assert!(search(&tables, "k", SearchKind::All).is_empty());
        let all = search(&tables, "KAF", SearchKind::All);
        assert_eq!(all.len(), 1);
        assert!(all[0].label.starts_with("[NPC] Kafra Employee"));
        assert!(search(&tables, "kafra", SearchKind::Mob).is_empty());
        let mobs = search(&tables, "pori", SearchKind::All);
        assert_eq!(mobs.len(), 1);
        assert!(mobs[0].is_monster && !mobs[0].has_cell);
        let maps = search(&tables, "fild0", SearchKind::All);
        assert_eq!(maps.iter().map(|hit| hit.label.as_str()).collect::<Vec<_>>(), vec![
            "[MAP] prt_fild05",
            "[MAP] prt_fild06",
            "[MAP] prt_fild08"
        ]);
        assert!(search(&tables, "fild0", SearchKind::Npc).is_empty());
    }

    #[test]
    fn compass_uses_north_up_cells() {
        assert_eq!(compass((10, 10), (10, 20)), "N");
        assert_eq!(compass((10, 10), (20, 20)), "NE");
        assert_eq!(compass((10, 10), (0, 10)), "W");
        assert_eq!(compass((10, 10), (10, 0)), "S");
        assert_eq!(compass((10, 10), (10, 10)), "here");
    }

    #[test]
    fn window_flow_search_choose_walk_and_warp() {
        let mut navigation = Navigation::default();
        navigation.attach_navigation_tables(Arc::new(tables()));
        navigation.on_map_change("prontera.gat", 150, 150);
        navigation.route.query = "kafra".into();
        navigation.search_route_targets();
        assert_eq!(navigation.route().hits().len(), 1);
        navigation.choose_route_target(0);
        assert_eq!(navigation.route().steps().len(), 3);
        assert!(navigation.route().status.contains("2 warps"));
        assert!(navigation.route().guide.starts_with("Next: warp at 156,22 to prt_fild08"));
        assert!(navigation.route().guide.ends_with("S"));

        navigation.on_map_change("prt_fild08", 170, 375);
        assert!(navigation.route().status.contains("1 warp"));
        assert!(navigation.route().guide.contains("to izlude"));

        navigation.on_map_change("izlude", 128, 200);
        assert!(navigation.route().guide.starts_with("Destination 134,87"));
        navigation.set_player_cell(134, 88);
        assert_eq!(navigation.route().guide, "Arrived: Kafra Employee");

        navigation.clear();
        assert!(navigation.route().has_tables());
        assert!(navigation.route().destination().is_none());
    }

    #[test]
    fn server_target_routes_and_enables_services() {
        let mut navigation = Navigation::default();
        navigation.attach_navigation_tables(Arc::new(tables()));
        navigation.on_map_change("prontera", 150, 150);
        navigation.set_navigation("geffen".into(), 120, 60, 0, 0, 4);
        assert!(navigation.route().services());
        assert_eq!(navigation.route().steps().len(), 2);
        navigation.clear_route();
        assert!(navigation.target().is_none());
        assert!(navigation.route().steps().is_empty());
    }

    #[test]
    fn world_map_route_reaches_the_map_and_then_arrives() {
        let mut navigation = Navigation::default();
        navigation.attach_navigation_tables(Arc::new(tables()));
        navigation.on_map_change("prontera", 150, 150);
        navigation.route_to_map("IZLUDE.gat", " Izlude ");
        let destination = navigation.route().destination().unwrap().clone();
        assert_eq!(destination.map, "izlude");
        assert_eq!(destination.name, "Izlude");
        assert!(destination.whole_map && !destination.has_cell);
        assert_eq!(navigation.route().steps().len(), 3);
        assert!(navigation.route().status.contains("2 warps to izlude"));
        assert_eq!(navigation.route().steps().last().unwrap().label, "izlude: arrive in Izlude");
        assert!(navigation.route().guide.starts_with("Next: warp at 156,22 to prt_fild08"));

        navigation.on_map_change("izlude", 128, 200);
        assert_eq!(navigation.route().guide, "Arrived: Izlude (izlude)");

        // An empty map id is ignored; a blank label falls back to the id.
        navigation.route_to_map("  ", "Nowhere");
        assert_eq!(navigation.route().destination().unwrap().map, "izlude");
        navigation.route_to_map("prt_fild08", "");
        assert_eq!(navigation.route().destination().unwrap().name, "prt_fild08");
    }
}
