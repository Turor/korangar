//! Hit testing and Warpra overlay decisions for the world map.

use korangar_interface::element::StateElement;
use korangar_networking::warpra_level_requirement;
use ragnarok_packets::{WARPRA_ACTION_WARP, WarpraDestinationEntry};
use rust_state::RustState;

use super::data::{REGIONS, WorldMapRegion, WorldMapSection};

/// Turoran world map includes maps through episode 12. New World starts at
/// 13.1.
pub const DEFAULT_EPISODE: f32 = 12.0;

/// World-map image space. Section rectangles are authored in this space.
pub const BASE_WIDTH: f32 = 1280.0;
pub const BASE_HEIGHT: f32 = 1024.0;

const DUNGEON_KIND: u8 = 2;

/// A rectangle in either base space or screen space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BaseRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

/// One visible world-map square passed into click resolution.
#[derive(Clone, Copy, Debug)]
pub struct MapSection<'a> {
    pub id: &'a str,
    pub name: &'a str,
    /// World-map `type` from the section table. The ported table leaves this 0;
    /// dungeon grouping uses the Warpra catalog kind instead.
    #[allow(dead_code)]
    pub kind: u8,
    pub index: Option<u16>,
    pub moblevel: Option<&'a str>,
}

/// One dungeon floor row in the picker.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct FloorChoice {
    pub name: String,
    pub map_id: String,
    pub kind: i16,
    pub location_id: i16,
    pub unlocked: bool,
    pub level: Option<u32>,
}

/// What a world-map click does. There is no buy or confirm variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClickDecision {
    /// The catalog has not arrived yet.
    Waiting,
    /// The square is not a Warpra destination.
    Missing { map_id: String },
    /// Locked, or otherwise not actionable. `reason` is hover or status text.
    Ignore { reason: String },
    /// Send `0x0EF4` with [`WARPRA_ACTION_WARP`].
    Warp { kind: i16, location_id: i16 },
    /// More than one floor. The caller draws the list and warps on an unlocked
    /// row.
    OpenFloors { title: String, floors: Vec<FloorChoice> },
}

/// How a catalog row paints on top of a square.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayClass {
    Missing,
    Unlocked,
    Locked,
}

/// `true` when `episode` is inside `[ep_from, ep_to)`.
pub fn episode_includes(episode: f32, ep_from: f32, ep_to: f32) -> bool {
    episode >= ep_from && episode < ep_to
}

pub fn regions_for_episode(episode: f32) -> Vec<&'static WorldMapRegion> {
    REGIONS
        .iter()
        .filter(|region| episode_includes(episode, region.ep_from, region.ep_to))
        .collect()
}

pub fn visible_sections(region: &WorldMapRegion, episode: f32) -> Vec<&'static WorldMapSection> {
    region
        .sections
        .iter()
        .filter(|section| episode_includes(episode, section.ep_from, section.ep_to))
        .collect()
}

pub fn section_ref(section: &WorldMapSection) -> MapSection<'_> {
    MapSection {
        id: section.id,
        name: section.name,
        kind: section.kind,
        index: section.index,
        moblevel: section.moblevel,
    }
}

/// Strip a trailing `.gat` or `.rsw` and compare map ids case-insensitively.
pub fn normalize_map_id(map_id: &str) -> String {
    let trimmed = map_id.trim().trim_matches('\0');
    let without_gat = trimmed
        .strip_suffix(".gat")
        .or_else(|| trimmed.strip_suffix(".GAT"))
        .unwrap_or(trimmed);
    let without_ext = without_gat
        .strip_suffix(".rsw")
        .or_else(|| without_gat.strip_suffix(".RSW"))
        .unwrap_or(without_gat);
    without_ext.to_ascii_lowercase()
}

pub fn maps_match(left: &str, right: &str) -> bool {
    normalize_map_id(left) == normalize_map_id(right)
}

/// GRF path relative to `data/texture/`, matching the interface texture loader.
pub fn region_texture_path(region_id: &str) -> String {
    format!("유저인터페이스\\{region_id}")
}

pub fn map_thumbnail_path(map_id: &str) -> String {
    format!("유저인터페이스\\map\\{map_id}.bmp")
}

/// World-map section id → catalog map. Identity aliases are omitted; those
/// tiles already use the catalog id. `wmIndex` grouping is separate.
pub fn dungeon_alias(map_id: &str) -> Option<&'static str> {
    match map_id {
        "izlu2dun" => Some("iz_dun00"),
        "mosk_dun02" | "mosk_dun03" => Some("mosk_dun01"),
        "odin_tem02" | "odin_tem03" => Some("odin_tem01"),
        _ => None,
    }
}

pub fn scale_rect(map: BaseRect, section: BaseRect) -> BaseRect {
    let scale_x = map.width / BASE_WIDTH;
    let scale_y = map.height / BASE_HEIGHT;
    BaseRect {
        left: map.left + section.left * scale_x,
        top: map.top + section.top * scale_y,
        width: section.width * scale_x,
        height: section.height * scale_y,
    }
}

/// Half-open point test. The right and bottom edges belong to the next tile.
pub fn contains(rect: BaseRect, x: f32, y: f32) -> bool {
    x >= rect.left && y >= rect.top && x < rect.left + rect.width && y < rect.top + rect.height
}

/// Topmost section whose scaled rectangle contains the point.
///
/// Later rectangles win. The window click path uses this so a shared border is
/// not inside two tiles.
pub fn hit_section(map: BaseRect, sections: &[BaseRect], x: f32, y: f32) -> Option<usize> {
    sections
        .iter()
        .enumerate()
        .rev()
        .find(|(_, section)| contains(scale_rect(map, **section), x, y))
        .map(|(index, _)| index)
}

pub fn section_rect(section: &WorldMapSection) -> BaseRect {
    BaseRect {
        left: f32::from(section.left),
        top: f32::from(section.top),
        width: f32::from(section.width),
        height: f32::from(section.height),
    }
}

/// Close the world map after a successful warp, not after a buy or a failure.
pub fn should_close_after_result(result: i16, action: i16) -> bool {
    result == 0 && action == WARPRA_ACTION_WARP
}

pub fn overlay_class(sections: &[MapSection<'_>], entries: &[WarpraDestinationEntry], index: usize) -> OverlayClass {
    if entries.is_empty() {
        return OverlayClass::Missing;
    }
    match resolve_location(sections, entries, index) {
        None => OverlayClass::Missing,
        Some(location) => {
            let floors = relevant_floors(entries, location);
            if floors.is_empty() {
                if location.unlocked == 0 {
                    OverlayClass::Locked
                } else {
                    OverlayClass::Unlocked
                }
            } else if floors.iter().any(|floor| floor.unlocked != 0) {
                OverlayClass::Unlocked
            } else {
                OverlayClass::Locked
            }
        }
    }
}

pub fn hover_caption(section: &MapSection<'_>, class: OverlayClass, catalog_name: Option<&str>, player_level: Option<u32>) -> String {
    let map_id = normalize_map_id(section.id);
    let level = catalog_name
        .and_then(warpra_level_requirement)
        .or_else(|| warpra_level_requirement(section.name));
    let mob = section
        .moblevel
        .map(|level| format!(" Suggested level {level}."))
        .unwrap_or_default();
    match class {
        OverlayClass::Unlocked | OverlayClass::Missing => format!("{} ({map_id}).{mob}", section.name),
        OverlayClass::Locked => match (level, player_level) {
            (Some(need), Some(have)) if have < need => {
                format!(
                    "{} ({map_id}) — locked, requires base level {need} (you are {have}).{mob}",
                    section.name
                )
            }
            (Some(need), _) => format!("{} ({map_id}) — locked, requires base level {need}.{mob}", section.name),
            _ => format!("{} ({map_id}) — locked.{mob}", section.name),
        },
    }
}

pub fn section_click(sections: &[MapSection<'_>], entries: &[WarpraDestinationEntry], index: usize) -> ClickDecision {
    let Some(section) = sections.get(index) else {
        return ClickDecision::Ignore { reason: String::new() };
    };
    if entries.is_empty() {
        return ClickDecision::Waiting;
    }
    let Some(location) = resolve_location(sections, entries, index) else {
        return ClickDecision::Missing {
            map_id: normalize_map_id(section.id),
        };
    };
    let floors = relevant_floors(entries, location);
    if floors.len() > 1 {
        if floors.iter().all(|floor| floor.unlocked == 0) {
            return ClickDecision::Ignore {
                reason: locked_reason(section, Some(location)),
            };
        }
        let title = if location.group_name.is_empty() {
            location.name.clone()
        } else {
            location.group_name.clone()
        };
        return ClickDecision::OpenFloors {
            title,
            floors: floors.iter().copied().map(floor_choice).collect(),
        };
    }
    let target = floors.first().copied().unwrap_or(location);
    if target.unlocked == 0 {
        ClickDecision::Ignore {
            reason: locked_reason(section, Some(target)),
        }
    } else {
        ClickDecision::Warp {
            kind: i16::from(target.kind),
            location_id: target.id,
        }
    }
}

pub fn floor_click(floor: &FloorChoice) -> ClickDecision {
    if floor.unlocked {
        ClickDecision::Warp {
            kind: floor.kind,
            location_id: floor.location_id,
        }
    } else {
        let reason = match floor.level {
            Some(need) => format!("{} ({}) — locked, requires base level {need}.", floor.name, floor.map_id),
            None => format!("{} ({}) — locked.", floor.name, floor.map_id),
        };
        ClickDecision::Ignore { reason }
    }
}

fn floor_choice(entry: &WarpraDestinationEntry) -> FloorChoice {
    FloorChoice {
        name: entry.name.clone(),
        map_id: normalize_map_id(&entry.map),
        kind: i16::from(entry.kind),
        location_id: entry.id,
        unlocked: entry.unlocked != 0,
        level: warpra_level_requirement(&entry.name),
    }
}

fn locked_reason(section: &MapSection<'_>, location: Option<&WarpraDestinationEntry>) -> String {
    hover_caption(section, OverlayClass::Locked, location.map(|entry| entry.name.as_str()), None)
}

pub fn entry_for_map<'a>(entries: &'a [WarpraDestinationEntry], map_id: &str) -> Option<&'a WarpraDestinationEntry> {
    find_entry(entries, map_id)
}

fn find_entry<'a>(entries: &'a [WarpraDestinationEntry], map_id: &str) -> Option<&'a WarpraDestinationEntry> {
    let id = normalize_map_id(map_id);
    let lookup = dungeon_alias(&id).unwrap_or(id.as_str());
    entries
        .iter()
        .find(|entry| normalize_map_id(&entry.map) == lookup)
        .or_else(|| entries.iter().find(|entry| normalize_map_id(&entry.map) == id))
}

fn resolve_location<'a>(
    sections: &[MapSection<'_>],
    entries: &'a [WarpraDestinationEntry],
    index: usize,
) -> Option<&'a WarpraDestinationEntry> {
    let section = sections.get(index)?;
    let direct = find_entry(entries, section.id);
    let direct_is_dungeon = direct.is_some_and(|entry| entry.kind == DUNGEON_KIND);
    if direct_is_dungeon || section.index.is_none() {
        return direct;
    }
    // A buried entrance can share `index` with the Warpra floor that is actually
    // listed.
    if let Some(shared) = section.index {
        for other in sections {
            if other.index != Some(shared) {
                continue;
            }
            if let Some(entry) = find_entry(entries, other.id)
                && entry.kind == DUNGEON_KIND
            {
                return Some(entry);
            }
        }
    }
    direct
}

fn relevant_floors<'a>(entries: &'a [WarpraDestinationEntry], location: &'a WarpraDestinationEntry) -> Vec<&'a WarpraDestinationEntry> {
    if location.kind != DUNGEON_KIND {
        return Vec::new();
    }
    let mut floors: Vec<_> = entries
        .iter()
        .filter(|entry| entry.kind == DUNGEON_KIND && entry.group == location.group)
        .collect();
    floors.sort_by_key(|entry| entry.id);
    floors
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::WARPRA_ACTION_BUY;

    use super::*;
    use crate::interface::windows::world_map::data::REGIONS;

    fn entry(kind: u8, id: i16, unlocked: u8, group: u8, map: &str, name: &str, group_name: &str) -> WarpraDestinationEntry {
        WarpraDestinationEntry {
            kind,
            id,
            unlocked,
            x: 10,
            y: 20,
            group,
            map: map.to_owned(),
            name: name.to_owned(),
            group_name: group_name.to_owned(),
        }
    }

    fn square<'a>(id: &'a str, name: &'a str, index: Option<u16>) -> MapSection<'a> {
        MapSection {
            id,
            name,
            kind: 0,
            index,
            moblevel: None,
        }
    }

    #[test]
    fn normalizes_map_ids() {
        assert_eq!(normalize_map_id("Payon.gat"), "payon");
        assert_eq!(normalize_map_id("iz_dun00.rsw"), "iz_dun00");
        assert_eq!(normalize_map_id("  prontera.GAT\0"), "prontera");
        assert_eq!(normalize_map_id("a.rsw.gat"), "a");
        assert!(maps_match("prontera.gat", "Prontera"));
        assert!(!maps_match("geffen", "prontera"));
    }

    #[test]
    fn episode_12_regions_skip_the_new_world() {
        let names: Vec<_> = regions_for_episode(DEFAULT_EPISODE).iter().map(|region| region.name).collect();
        assert_eq!(names, vec!["Midgard", "Eastern Kingdoms", "Far Lands", "Isgard"]);
        assert!(REGIONS.iter().any(|region| region.id == "worldmap_dimension.bmp"));
        assert!(regions_for_episode(14.0).iter().any(|region| region.id == "worldmap_dimension.bmp"));

        let far = regions_for_episode(DEFAULT_EPISODE)
            .into_iter()
            .find(|region| region.name == "Far Lands")
            .unwrap();
        let ids: Vec<_> = visible_sections(far, DEFAULT_EPISODE).iter().map(|section| section.id).collect();
        assert!(ids.contains(&"mosk_dun01"));
        assert!(!ids.contains(&"dew_dun01"));
    }

    #[test]
    fn hit_testing_uses_scaled_rects_and_prefers_the_top_square() {
        let map = BaseRect {
            left: 100.0,
            top: 40.0,
            width: 640.0,
            height: 512.0,
        };
        let sections = [
            BaseRect {
                left: 0.0,
                top: 0.0,
                width: 100.0,
                height: 80.0,
            },
            BaseRect {
                left: 50.0,
                top: 40.0,
                width: 40.0,
                height: 40.0,
            },
        ];
        let first = scale_rect(map, sections[0]);
        assert!(contains(first, first.left + 1.0, first.top + 1.0));
        assert!(!contains(first, first.left - 1.0, first.top));
        assert_eq!(hit_section(map, &sections, first.left + 1.0, first.top + 1.0), Some(0));

        let overlap = scale_rect(map, sections[1]);
        assert_eq!(hit_section(map, &sections, overlap.left + 1.0, overlap.top + 1.0), Some(1));

        let midgard = regions_for_episode(DEFAULT_EPISODE)[0];
        let visible = visible_sections(midgard, DEFAULT_EPISODE);
        let rects: Vec<_> = visible.iter().copied().map(section_rect).collect();
        let full = BaseRect {
            left: 0.0,
            top: 0.0,
            width: BASE_WIDTH,
            height: BASE_HEIGHT,
        };
        let prontera = visible.iter().position(|section| section.id == "prontera").unwrap();
        let prontera_rect = scale_rect(full, rects[prontera]);
        assert_eq!(
            hit_section(full, &rects, prontera_rect.left + 1.0, prontera_rect.top + 1.0),
            Some(prontera)
        );
        assert!(maps_match("prontera.gat", visible[prontera].id));
    }

    #[test]
    fn shared_border_is_not_inside_the_earlier_square() {
        let map = BaseRect {
            left: 0.0,
            top: 0.0,
            width: BASE_WIDTH,
            height: BASE_HEIGHT,
        };
        let sections = [BaseRect {
            left: 0.0,
            top: 0.0,
            width: 100.0,
            height: 50.0,
        }];
        assert_eq!(hit_section(map, &sections, 99.0, 10.0), Some(0));
        assert_eq!(hit_section(map, &sections, 100.0, 10.0), None);
        assert_eq!(hit_section(map, &sections, 10.0, 50.0), None);
    }

    #[test]
    fn overlapping_squares_keep_the_later_tile() {
        let map = BaseRect {
            left: 0.0,
            top: 0.0,
            width: BASE_WIDTH,
            height: BASE_HEIGHT,
        };
        let midgard = regions_for_episode(DEFAULT_EPISODE)
            .into_iter()
            .find(|region| region.name == "Midgard")
            .unwrap();
        let visible = visible_sections(midgard, DEFAULT_EPISODE);
        let rects: Vec<_> = visible.iter().copied().map(section_rect).collect();
        let yuno_field = visible.iter().position(|section| section.id == "yuno_fild05").unwrap();
        let yuno = visible.iter().position(|section| section.id == "yuno").unwrap();
        assert!(yuno > yuno_field);
        // Interior of the overlap (about 50×36): later `yuno` wins.
        assert_eq!(hit_section(map, &rects, 650.0, 90.0), Some(yuno));

        let forest = visible.iter().position(|section| section.id == "pay_fild01").unwrap();
        let guild = visible.iter().position(|section| section.id == "pay_gld").unwrap();
        assert!(guild > forest);
        assert_eq!(hit_section(map, &rects, 940.0, 810.0), Some(guild));
    }

    #[test]
    fn classifies_unlocked_locked_and_missing_squares() {
        let sections = [
            square("prontera", "Prontera", None),
            square("pay_dun00", "Payon Dungeon (Lv 55+)", None),
        ];
        let entries = [
            entry(0, 1, 1, 0, "prontera.gat", "Prontera", ""),
            entry(2, 4, 0, 9, "pay_dun00", "Payon Dungeon (Lv 55+)", "Payon"),
        ];
        assert_eq!(overlay_class(&sections, &entries, 0), OverlayClass::Unlocked);
        assert_eq!(overlay_class(&sections, &entries, 1), OverlayClass::Locked);
        assert_eq!(
            overlay_class(&[square("gef_fild00", "Geffen Field", None)], &entries, 0),
            OverlayClass::Missing
        );
        assert!(hover_caption(&sections[1], OverlayClass::Locked, Some("Payon Dungeon (Lv 55+)"), Some(40)).contains("base level 55"));
        assert!(hover_caption(&sections[1], OverlayClass::Locked, Some("Payon Dungeon (Lv 55+)"), Some(40)).contains("you are 40"));
        assert_eq!(warpra_level_requirement("Payon Dungeon (Lv 55+)"), Some(55));
    }

    #[test]
    fn unlocked_click_warps_and_locked_click_is_ignored() {
        let sections = [square("prontera", "Prontera", None), square("payon", "Payon", None)];
        let entries = [
            entry(0, 3, 1, 0, "prontera", "Prontera", ""),
            entry(0, 8, 0, 0, "payon", "Payon", ""),
        ];
        match section_click(&sections, &entries, 0) {
            ClickDecision::Warp { kind, location_id } => {
                assert_eq!(kind, 0);
                assert_eq!(location_id, 3);
                assert_eq!(WARPRA_ACTION_WARP, 1);
                assert_ne!(WARPRA_ACTION_WARP, WARPRA_ACTION_BUY);
            }
            other => panic!("expected warp, got {other:?}"),
        }
        assert!(matches!(section_click(&sections, &entries, 1), ClickDecision::Ignore { .. }));
        assert!(matches!(section_click(&sections, &[], 0), ClickDecision::Waiting));
    }

    #[test]
    fn dungeon_aliases_open_a_floor_list_and_locked_floors_do_not_warp() {
        let sections = [
            square("izlu2dun", "Byalan Island", None),
            square("odin_tem03", "Odin Temple F3", None),
            square("buried", "Buried entrance", Some(7)),
            square("tur_dun02", "Turtle Dungeon", Some(7)),
        ];
        let entries = [
            entry(2, 10, 1, 4, "iz_dun00.gat", "Byalan 1", "Byalan"),
            entry(2, 11, 0, 4, "iz_dun01", "Byalan 2 (Lv 90+)", "Byalan"),
            entry(2, 20, 1, 5, "odin_tem01", "Odin 1", "Odin"),
            entry(2, 21, 1, 5, "odin_tem02", "Odin 2", "Odin"),
            entry(2, 30, 0, 6, "tur_dun02", "Turtle", "Turtle"),
            entry(2, 31, 1, 6, "tur_dun03", "Turtle 2", "Turtle"),
        ];
        match section_click(&sections, &entries, 0) {
            ClickDecision::OpenFloors { title, floors } => {
                assert_eq!(title, "Byalan");
                assert_eq!(floors.len(), 2);
                assert!(floors[0].unlocked);
                assert!(!floors[1].unlocked);
                assert_eq!(floors[1].level, Some(90));
                assert!(matches!(floor_click(&floors[0]), ClickDecision::Warp { location_id: 10, .. }));
                assert!(matches!(floor_click(&floors[1]), ClickDecision::Ignore { .. }));
            }
            other => panic!("expected floors, got {other:?}"),
        }
        assert!(matches!(
            section_click(&sections, &entries, 1),
            ClickDecision::OpenFloors { .. }
        ));
        match section_click(&sections, &entries, 2) {
            ClickDecision::OpenFloors { floors, .. } => {
                assert_eq!(floors.len(), 2);
                assert_eq!(floors[0].location_id, 30);
            }
            other => panic!("expected grouped floors, got {other:?}"),
        }
    }

    #[test]
    fn a_single_unlocked_floor_warps_without_a_picker() {
        let sections = [square("izlu2dun", "Byalan Island", None)];
        let entries = [entry(2, 10, 1, 4, "iz_dun00", "Byalan 1", "Byalan")];
        assert!(matches!(section_click(&sections, &entries, 0), ClickDecision::Warp {
            location_id: 10,
            kind: 2
        }));
    }

    #[test]
    fn warp_success_closes_the_map_and_other_results_do_not() {
        assert!(should_close_after_result(0, WARPRA_ACTION_WARP));
        assert!(!should_close_after_result(0, WARPRA_ACTION_BUY));
        assert!(!should_close_after_result(6, WARPRA_ACTION_WARP));
        assert!(!should_close_after_result(2, WARPRA_ACTION_WARP));
        assert_eq!(region_texture_path("worldmap.jpg"), "유저인터페이스\\worldmap.jpg");
        assert_eq!(map_thumbnail_path("prontera"), "유저인터페이스\\map\\prontera.bmp");
    }
}
