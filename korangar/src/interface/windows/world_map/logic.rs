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

/// Marker color role. Gold is the hover accent only. The current map uses the
/// theme highlight (accent). Locked destinations stay dim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerTone {
    None,
    Accent,
    Dim,
    Gold,
}

/// Fill and stroke for one square. This does not look at whether the region
/// texture has loaded, so markers stay visible on the opaque panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkerPaint {
    pub fill: MarkerTone,
    pub stroke: MarkerTone,
}

pub fn marker_paint(class: OverlayClass, is_current: bool, hovered: bool) -> MarkerPaint {
    let fill = match class {
        OverlayClass::Unlocked => MarkerTone::Accent,
        OverlayClass::Locked => MarkerTone::Dim,
        OverlayClass::Missing => MarkerTone::None,
    };
    let stroke = if hovered {
        MarkerTone::Gold
    } else if is_current {
        MarkerTone::Accent
    } else {
        MarkerTone::None
    };
    MarkerPaint { fill, stroke }
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

/// Hangul interface directory. GRF names decode to this, and the asset host
/// serves `data/texture/유저인터페이스/…` as UTF-8.
pub const INTERFACE_DIR_HANGUL: &str = "유저인터페이스";

/// CP949 bytes of [`INTERFACE_DIR_HANGUL`]. Folder extracts and roBrowser's
/// `DB.INTERFACE_PATH` keep those bytes as Latin-1 (`À¯ÀúÀÎÅÍÆäÀÌ½º`).
pub const INTERFACE_DIR_CP949: &[u8] = &[0xC0, 0xAF, 0xC0, 0xFA, 0xC0, 0xCE, 0xC5, 0xCD, 0xC6, 0xE4, 0xC0, 0xCC, 0xBD, 0xBA];

pub fn interface_dir_latin1() -> String {
    INTERFACE_DIR_CP949.iter().copied().map(char::from).collect()
}

/// Hangul path first, then the CP949 Latin-1 directory. `file` is the region
/// id (`worldmap.jpg`) or a thumbnail (`map\\prontera.bmp`).
pub fn interface_texture_candidates(file: &str) -> [String; 2] {
    [
        format!("{INTERFACE_DIR_HANGUL}\\{file}"),
        format!("{}\\{file}", interface_dir_latin1()),
    ]
}

/// GRF path relative to `data/texture/`, matching the interface texture loader.
pub fn region_texture_path(region_id: &str) -> String {
    interface_texture_candidates(region_id)[0].clone()
}

pub fn map_thumbnail_path(map_id: &str) -> String {
    interface_texture_candidates(&format!("map\\{map_id}.bmp"))[0].clone()
}

/// First candidate whose `data\\texture\\…` path `available` accepts.
///
/// Production loading uses [`select_interface_texture`], which does not treat
/// a remote manifest entry as a local file. Tests keep this helper to show
/// the old `file_exists` order.
#[cfg(test)]
pub fn choose_interface_texture(file: &str, mut available: impl FnMut(&str) -> bool) -> Option<String> {
    interface_texture_candidates(file)
        .into_iter()
        .find(|candidate| available(&format!("data\\texture\\{candidate}")))
}

/// How one interface image resolved.
///
/// `Retry` means a local file was present but the read failed. The caller must
/// not record that path and must not install the placeholder, so a later frame
/// can open the other candidate. `Missing` means both local candidates missed
/// and the remote read returned nothing; that absence is safe to remember so
/// the frame loop does not fetch again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InterfaceTextureAttempt {
    Ready(String),
    Retry,
    Missing,
}

/// Local archives first (Hangul, then CP949). `read` runs on a local hit to
/// confirm the bytes, and as the remote `get` only after both local candidates
/// miss. `local_exists` must not be true for a manifest-only path.
pub fn select_interface_texture(
    file: &str,
    mut local_exists: impl FnMut(&str) -> bool,
    mut read: impl FnMut(&str) -> bool,
) -> InterfaceTextureAttempt {
    let candidates = interface_texture_candidates(file);
    let mut local_read_failed = false;
    for candidate in &candidates {
        let archive = format!("data\\texture\\{candidate}");
        if !local_exists(&archive) {
            continue;
        }
        if read(&archive) {
            return InterfaceTextureAttempt::Ready(candidate.clone());
        }
        local_read_failed = true;
    }
    if local_read_failed {
        return InterfaceTextureAttempt::Retry;
    }
    for candidate in &candidates {
        let archive = format!("data\\texture\\{candidate}");
        if read(&archive) {
            return InterfaceTextureAttempt::Ready(candidate.clone());
        }
    }
    InterfaceTextureAttempt::Missing
}

/// Left rail, gap, and the caption row under the map. Shared with the window
/// so hit testing uses the same rectangles the layout draws.
pub const RAIL_WIDTH: f32 = 168.0;
pub const RAIL_GAP: f32 = 8.0;
pub const CAPTION_HEIGHT: f32 = 72.0;

/// Region buttons stack at this stride when the rail is tall enough.
pub const REGION_BUTTON_HEIGHT: f32 = 28.0;
pub const REGION_BUTTON_GAP: f32 = 4.0;
pub const REGION_BUTTON_STRIDE: f32 = REGION_BUTTON_HEIGHT + REGION_BUTTON_GAP;

/// One region button, relative to the top of the rail.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionButtonRect {
    pub top: f32,
    pub height: f32,
}

/// Keep every region button inside the rail. The natural 32px stride is used
/// when `count * stride` fits. A short rail (interface scale 2 on 1024×576)
/// shrinks the stride so the stack ends at the rail bottom, one gap above the
/// caption. A point on the caption is not a region.
pub fn region_buttons(rail_height: f32, region_count: usize) -> Vec<RegionButtonRect> {
    if region_count == 0 || rail_height <= 0.0 {
        return Vec::new();
    }
    let count = region_count as f32;
    let natural_bottom = (count - 1.0) * REGION_BUTTON_STRIDE + REGION_BUTTON_HEIGHT;
    if natural_bottom <= rail_height {
        return (0..region_count)
            .map(|index| RegionButtonRect {
                top: index as f32 * REGION_BUTTON_STRIDE,
                height: REGION_BUTTON_HEIGHT,
            })
            .collect();
    }
    let slot = rail_height / count;
    let gap = REGION_BUTTON_GAP.min(slot * 0.25);
    let height = (slot - gap).max(0.0);
    (0..region_count)
        .map(|index| {
            let top = index as f32 * slot;
            RegionButtonRect {
                top,
                height: height.min((rail_height - top).max(0.0)),
            }
        })
        .collect()
}

/// Region under a point in the same space as `rail`. Outside the rail,
/// including the caption row, this is `None`. The window lays the same rects
/// out and ignores a button that would leave the rail, so a caption click
/// cannot select a region.
#[cfg(test)]
pub fn region_button_index(rail: BaseRect, region_count: usize, x: f32, y: f32) -> Option<usize> {
    if !contains(rail, x, y) {
        return None;
    }
    let local_y = y - rail.top;
    region_buttons(rail.height, region_count)
        .into_iter()
        .enumerate()
        .find(|(_, button)| local_y >= button.top && local_y < button.top + button.height)
        .map(|(index, _)| index)
}

/// Gap between the window and the viewport edge.
pub const VIEWPORT_MARGIN: f32 = 12.0;

/// `Anchor::initialize` places a new window's top a quarter of the way down
/// the screen. The fit keeps the caption above the bottom margin from there.
pub const DEFAULT_ANCHOR_TOP_RATIO: f32 = 0.25;

/// Window chrome from `InterfaceTheme::from_tokens` (title 22, gap 2, border
/// 8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldMapChrome {
    pub title_height: f32,
    pub title_gap: f32,
    pub border: f32,
    pub margin: f32,
    pub scaling: f32,
}

impl WorldMapChrome {
    #[cfg(test)]
    pub const fn stock() -> Self {
        Self {
            title_height: 22.0,
            title_gap: 2.0,
            border: 8.0,
            margin: VIEWPORT_MARGIN,
            scaling: 1.0,
        }
    }
}

/// Placed world-map frame in layout pixels. `map` is the art rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldMapFit {
    pub window_width: f32,
    pub window_height: f32,
    pub window_left: f32,
    pub window_top: f32,
    pub map: BaseRect,
    pub caption_bottom: f32,
}

/// Scale the 1280×1024 art so the window, rail, caption, and hover label sit
/// on screen. Interface scaling grows the painted size from the window's
/// top-left, so a scale above 1 shrinks the layout size to match.
pub fn fit_world_map(viewport_w: f32, viewport_h: f32, chrome: WorldMapChrome) -> WorldMapFit {
    let scale = if chrome.scaling.is_finite() && chrome.scaling >= 0.5 {
        chrome.scaling
    } else {
        1.0
    };
    let (viewport_w, viewport_h) = if viewport_w < 64.0 || viewport_h < 64.0 {
        (1024.0, 576.0)
    } else {
        (viewport_w, viewport_h)
    };
    let margin = chrome.margin;
    let max_window_h = ((viewport_h * (1.0 - DEFAULT_ANCHOR_TOP_RATIO) - margin) / scale).max(180.0);
    let max_window_w = if scale <= 1.0 {
        (viewport_w - margin * 2.0).max(320.0)
    } else {
        ((viewport_w * 0.5 - margin) / (scale - 0.5)).max(320.0)
    };

    let max_content_w = (max_window_w - chrome.border * 2.0).max(RAIL_WIDTH + RAIL_GAP + 80.0);
    let max_content_h = (max_window_h - chrome.title_height - chrome.title_gap - chrome.border).max(CAPTION_HEIGHT + RAIL_GAP + 80.0);
    let map_w_from_w = max_content_w - RAIL_WIDTH - RAIL_GAP;
    let map_w_from_h = (max_content_h - RAIL_GAP - CAPTION_HEIGHT) * (BASE_WIDTH / BASE_HEIGHT);
    let map_width = map_w_from_w.min(map_w_from_h).max(64.0);
    let map_height = map_width * (BASE_HEIGHT / BASE_WIDTH);
    let content_width = map_width + RAIL_WIDTH + RAIL_GAP;
    let content_height = map_height + RAIL_GAP + CAPTION_HEIGHT;
    let window_width = content_width + chrome.border * 2.0;
    let window_height = chrome.title_height + chrome.title_gap + content_height + chrome.border;
    let window_left = (viewport_w - window_width) * 0.5;
    let window_top = viewport_h * DEFAULT_ANCHOR_TOP_RATIO;
    let map = BaseRect {
        left: window_left + chrome.border + RAIL_WIDTH + RAIL_GAP,
        top: window_top + chrome.title_height + chrome.title_gap,
        width: map_width,
        height: map_height,
    };
    let caption_bottom = map.top + map_height + RAIL_GAP + CAPTION_HEIGHT;
    WorldMapFit {
        window_width,
        window_height,
        window_left,
        window_top,
        map,
        caption_bottom,
    }
}

/// Painted right edge. Scaling grows the window from its top-left.
#[cfg(test)]
pub fn rendered_right(fit: WorldMapFit, scale: f32) -> f32 {
    fit.window_left + fit.window_width * scale
}

/// Painted bottom edge, including the caption row.
#[cfg(test)]
pub fn rendered_bottom(fit: WorldMapFit, scale: f32) -> f32 {
    fit.window_top + fit.window_height * scale
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
    // Squares authored past the 1024-tall image must not steal caption or rail
    // clicks. A region switch only changes `sections`; the map rectangle stays.
    if !contains(map, x, y) {
        return None;
    }
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

    #[test]
    fn texture_paths_prefer_hangul_and_fall_back_to_the_cp949_directory() {
        assert_eq!(INTERFACE_DIR_HANGUL, "유저인터페이스");
        assert_eq!(interface_dir_latin1(), "À¯ÀúÀÎÅÍÆäÀÌ½º");
        let candidates = interface_texture_candidates("worldmap_localizing1.bmp");
        assert_eq!(candidates[0], "유저인터페이스\\worldmap_localizing1.bmp");
        assert_eq!(candidates[1], "À¯ÀúÀÎÅÍÆäÀÌ½º\\worldmap_localizing1.bmp");
        // Archive lookup lowercases. The Latin-1 directory still matches itself.
        assert_eq!(
            candidates[1].to_lowercase(),
            interface_dir_latin1().to_lowercase() + "\\worldmap_localizing1.bmp"
        );

        let hangul = choose_interface_texture("worldmap.jpg", |path| path.contains(INTERFACE_DIR_HANGUL));
        assert_eq!(hangul.as_deref(), Some("유저인터페이스\\worldmap.jpg"));

        let fallback = choose_interface_texture("WorldMap_Isgard.jpg", |path| path.contains('À'));
        assert_eq!(fallback.as_deref(), Some("À¯ÀúÀÎÅÍÆäÀÌ½º\\WorldMap_Isgard.jpg"));
        assert!(choose_interface_texture("worldmap_mob.bmp", |_| false).is_none());

        let thumb = choose_interface_texture("map\\prontera.bmp", |path| !path.contains('À'));
        assert_eq!(thumb.as_deref(), Some("유저인터페이스\\map\\prontera.bmp"));
    }

    #[test]
    fn manifest_hangul_hit_still_selects_local_cp949_when_the_hangul_read_fails() {
        let candidates = interface_texture_candidates("worldmap.jpg");
        let hangul = format!("data\\texture\\{}", candidates[0]);
        let latin1 = format!("data\\texture\\{}", candidates[1]);
        // `file_exists` is local archives OR the remote manifest. Hangul is only
        // advertised by the manifest; the CP949 directory is the local file.
        let manifest = [hangul.clone()];
        let local = [latin1.clone()];
        let file_exists = |path: &str| local.iter().any(|entry| entry == path) || manifest.iter().any(|entry| entry == path);
        assert!(file_exists(&hangul));
        assert!(!local.iter().any(|entry| entry == &hangul));

        let mut hangul_reads = 0;
        let selected = select_interface_texture(
            "worldmap.jpg",
            |path| local.iter().any(|entry| entry == path),
            |path| {
                if path == hangul {
                    hangul_reads += 1;
                    return false;
                }
                local.iter().any(|entry| entry == path)
            },
        );
        assert_eq!(selected, InterfaceTextureAttempt::Ready(candidates[1].clone()));
        assert_eq!(hangul_reads, 0, "remote get runs only after both local candidates miss");

        let manifest_probe = choose_interface_texture("worldmap.jpg", file_exists);
        assert_eq!(manifest_probe.as_deref(), Some(candidates[0].as_str()));

        let unreadable = select_interface_texture("worldmap.jpg", |path| path == hangul, |_| false);
        assert_eq!(unreadable, InterfaceTextureAttempt::Retry);
        let absent = select_interface_texture("worldmap.jpg", |_| false, |_| false);
        assert_eq!(absent, InterfaceTextureAttempt::Missing);
    }

    #[test]
    fn markers_stay_classified_without_a_region_texture() {
        assert_eq!(marker_paint(OverlayClass::Unlocked, false, false), MarkerPaint {
            fill: MarkerTone::Accent,
            stroke: MarkerTone::None,
        });
        assert_eq!(marker_paint(OverlayClass::Locked, false, false).fill, MarkerTone::Dim);
        assert_eq!(marker_paint(OverlayClass::Unlocked, true, false).stroke, MarkerTone::Accent);
        // Gold is hover only. The current map keeps the accent stroke.
        assert_eq!(marker_paint(OverlayClass::Unlocked, true, true).stroke, MarkerTone::Gold);
        assert_eq!(marker_paint(OverlayClass::Missing, true, false).fill, MarkerTone::None);
        assert_eq!(marker_paint(OverlayClass::Missing, true, false).stroke, MarkerTone::Accent);
        assert_eq!(marker_paint(OverlayClass::Locked, true, true).stroke, MarkerTone::Gold);
    }

    fn section_center(map: BaseRect, section: &super::super::data::WorldMapSection) -> (f32, f32) {
        let scaled = scale_rect(map, section_rect(section));
        (scaled.left + scaled.width * 0.5, scaled.top + scaled.height * 0.5)
    }

    #[test]
    fn fitted_window_keeps_caption_and_squares_inside_1024_and_1280() {
        let chrome = WorldMapChrome::stock();
        for (width, height) in [(1024.0_f32, 576.0_f32), (1280.0, 720.0)] {
            let fit = fit_world_map(width, height, chrome);
            assert!(
                fit.window_left + 0.51 >= chrome.margin,
                "{width}x{height} left {}",
                fit.window_left
            );
            assert!(fit.window_top >= 0.0);
            assert!(rendered_right(fit, 1.0) <= width - chrome.margin + 0.51);
            assert!(rendered_bottom(fit, 1.0) <= height - chrome.margin + 0.51);
            assert!(fit.caption_bottom <= fit.window_top + fit.window_height + 0.51);
            assert!(fit.caption_bottom <= height - chrome.margin + 0.51);
            let aspect = fit.map.width / fit.map.height;
            assert!((aspect - (BASE_WIDTH / BASE_HEIGHT)).abs() < 0.01);
        }

        // A zero viewport (state not yet resized) uses the smaller required size.
        let unknown = fit_world_map(0.0, 0.0, chrome);
        let small = fit_world_map(1024.0, 576.0, chrome);
        assert_eq!(unknown.window_width, small.window_width);
        assert_eq!(unknown.window_height, small.window_height);

        let wide = fit_world_map(1280.0, 720.0, WorldMapChrome { scaling: 2.0, ..chrome });
        assert!(rendered_right(wide, 2.0) <= 1280.0 - chrome.margin + 0.51);
        assert!(rendered_bottom(wide, 2.0) <= 720.0 - chrome.margin + 0.51);
    }

    #[test]
    fn scale_two_region_buttons_stay_inside_the_rail_and_above_the_caption() {
        let chrome = WorldMapChrome {
            scaling: 2.0,
            ..WorldMapChrome::stock()
        };
        let fit = fit_world_map(1024.0, 576.0, chrome);
        assert!(rendered_right(fit, 2.0) <= 1024.0 - chrome.margin + 0.51);
        assert!(rendered_bottom(fit, 2.0) <= 576.0 - chrome.margin + 0.51);

        let count = regions_for_episode(DEFAULT_EPISODE).len();
        assert_eq!(count, 4);
        let natural_fourth_center = 3.0 * REGION_BUTTON_STRIDE + REGION_BUTTON_HEIGHT * 0.5;
        let caption_top = fit.map.height + RAIL_GAP;
        assert!(
            natural_fourth_center >= caption_top,
            "the fixed 32px stack overlaps the caption at this size (map height {})",
            fit.map.height
        );

        let buttons = region_buttons(fit.map.height, count);
        assert_eq!(buttons.len(), count);
        let rail = BaseRect {
            left: fit.window_left + chrome.border,
            top: fit.map.top,
            width: RAIL_WIDTH,
            height: fit.map.height,
        };
        for (index, button) in buttons.iter().enumerate() {
            assert!(button.height > 0.0, "{index}");
            assert!(button.top >= -0.01, "{index}");
            let bottom = button.top + button.height;
            assert!(bottom <= fit.map.height + 0.01, "{index} bottom {bottom}");
            assert!(bottom <= caption_top + 0.01, "{index} overlaps the caption");
            let y = rail.top + button.top + button.height * 0.5;
            assert_eq!(region_button_index(rail, count, rail.left + 4.0, y), Some(index));
        }
        assert_eq!(
            region_button_index(rail, count, rail.left + 4.0, rail.top + natural_fourth_center),
            None,
            "a click on the caption must not select a region"
        );
        assert_eq!(
            region_button_index(rail, count, rail.left + 4.0, rail.top + caption_top + 4.0),
            None
        );
    }

    #[test]
    fn scaled_hit_testing_follows_the_region_switch() {
        let chrome = WorldMapChrome::stock();
        let fit = fit_world_map(1024.0, 576.0, chrome);
        let regions = regions_for_episode(DEFAULT_EPISODE);
        let midgard = regions.iter().copied().find(|region| region.name == "Midgard").unwrap();
        let eastern = regions.iter().copied().find(|region| region.name == "Eastern Kingdoms").unwrap();
        let midgard_sections = visible_sections(midgard, DEFAULT_EPISODE);
        let eastern_sections = visible_sections(eastern, DEFAULT_EPISODE);
        let midgard_rects: Vec<_> = midgard_sections.iter().copied().map(section_rect).collect();
        let eastern_rects: Vec<_> = eastern_sections.iter().copied().map(section_rect).collect();

        let prontera = midgard_sections.iter().position(|section| section.id == "prontera").unwrap();
        let (px, py) = section_center(fit.map, midgard_sections[prontera]);
        assert!(contains(fit.map, px, py));
        assert!(py <= 576.0 - chrome.margin);
        assert_eq!(hit_section(fit.map, &midgard_rects, px, py), Some(prontera));

        let amatsu = eastern_sections.iter().position(|section| section.id == "amatsu").unwrap();
        let (ax, ay) = section_center(fit.map, eastern_sections[amatsu]);
        assert!(contains(fit.map, ax, ay));
        assert!(ax >= chrome.margin && ax <= 1024.0 - chrome.margin);
        assert!(ay <= 576.0 - chrome.margin);
        assert_eq!(hit_section(fit.map, &eastern_rects, ax, ay), Some(amatsu));
        // The same point is not Prontera, and it is not the rail.
        assert_ne!(hit_section(fit.map, &midgard_rects, ax, ay), Some(prontera));
        let rail_x = fit.window_left + chrome.border + 4.0;
        assert_eq!(hit_section(fit.map, &eastern_rects, rail_x, fit.map.top + 8.0), None);

        let views: Vec<_> = eastern_sections.iter().copied().map(section_ref).collect();
        let entries = [entry(0, 9, 1, 0, "amatsu", "Amatsu", "")];
        assert!(matches!(section_click(&views, &entries, amatsu), ClickDecision::Warp {
            location_id: 9,
            ..
        }));
        // A locked eastern square still does not warp after the switch.
        let entries = [entry(0, 9, 0, 0, "amatsu", "Amatsu", "")];
        assert!(matches!(section_click(&views, &entries, amatsu), ClickDecision::Ignore { .. }));
    }
}
