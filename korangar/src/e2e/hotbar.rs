//! Hotbar, headless (Matthew t274u).
//!
//! [`offline_hotbar_scenario`] opens the hotbar at its first-open size,
//! drags its top-right corner in (narrower, taller) with the scripted input
//! path and the same [`resize_window_area`] the interface uses, and sizes the
//! slot grid for each region with [`hotbar_metrics`]: the one-row bar wraps
//! onto more rows. It saves and reloads the size through `window_cache.ron`
//! and checks a reset forgets it. It then plays the cooldown and cast
//! timeline the client builds from `0x0B1A`, `0x043D` and `EFST_POSTDELAY`
//! through [`SkillCooldowns`] and [`CastBars`].
//!
//! Review-1 added two sizes: 18 slots (two rows shown) in the smallest window
//! the limits allow, and the first-open bar at the largest global font scale.
//! Both must keep the grid inside the window width.

use std::mem::ManuallyDrop;
use std::path::Path;

use korangar_interface::application::WindowCache as _;
use korangar_interface::layout::ResizeMode;
use korangar_interface::layout::area::Area;
use korangar_interface::window::{ResizeLimits, resize_handle_areas, resize_handle_at, resize_window_area};
use ragnarok_packets::{ClientTick, SkillId};

use super::resizable_windows::{RESIZE_SCREEN, scripted_drag};
use crate::graphics::{ScreenPosition, ScreenSize};
use crate::interface::components::hotbar_layout::{
    HOTBAR_DEFAULT_HEIGHT, HOTBAR_LABEL_FONT_PX, HOTBAR_WINDOW_MAXIMUM_HEIGHT, HOTBAR_WINDOW_MAXIMUM_WIDTH, HOTBAR_WINDOW_MINIMUM_HEIGHT,
    HOTBAR_WINDOW_MINIMUM_WIDTH, HotbarMetrics, hotbar_metrics,
};
use crate::interface::font_scale::{FONT_SCALE_MAX, font_scale_factor, from_theme_body};
use crate::interface::windows::{WindowCache, WindowClass};
use crate::state::skill_cooldowns::{SkillCooldowns, SlotDelay};
use crate::state::theme::InterfaceTheme;
use crate::world::effects::{CastBars, CastPhase};

/// Corner drag: 220px narrower and 120px taller.
pub const HOTBAR_NARROW_DRAG: (f32, f32) = (-220.0, -120.0);

/// One hotbar window as drawn and its grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HotbarSnapshot {
    pub width: f32,
    pub height: f32,
    /// Width the grid gets inside the window borders.
    pub region_width: f32,
    /// Window height after the grid grew it (never below `height`).
    pub laid_out_height: f32,
    pub metrics: HotbarMetrics,
}

/// One moment of the cooldown timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelinePoint {
    pub at_ms: u32,
    /// Overlay on the used skill (`Cooldown` with seconds, or `AfterCast`).
    pub used_skill: Option<SlotDelay>,
    /// Overlay on another skill (only the after-cast dim).
    pub other_skill: Option<SlotDelay>,
    /// Phase of the bar under the player, if one shows.
    pub bar: Option<CastPhase>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OfflineHotbarReport {
    pub grabbed: Option<ResizeMode>,
    pub before: HotbarSnapshot,
    pub narrowed: HotbarSnapshot,
    pub reloaded: HotbarSnapshot,
    pub reset_forgets: bool,
    /// 18 slots in the minimum 180x96 window.
    pub minimum_18: HotbarSnapshot,
    /// First-open 420x124 bar at the largest font scale.
    pub font_scaled: HotbarSnapshot,
    /// Same bar at the default font scale, for comparison.
    pub font_default: HotbarSnapshot,
    pub timeline: Vec<TimelinePoint>,
}

/// Hotbar window limits (`hotbar.rs`).
pub fn hotbar_limits() -> ResizeLimits {
    ResizeLimits::new(
        HOTBAR_WINDOW_MINIMUM_WIDTH,
        HOTBAR_WINDOW_MAXIMUM_WIDTH,
        HOTBAR_WINDOW_MINIMUM_HEIGHT,
        HOTBAR_WINDOW_MAXIMUM_HEIGHT,
    )
}

/// Grid for a hotbar window of this size: the window border on both sides,
/// the title and the button row above, one row of nine slots shown.
pub fn hotbar_window_grid(width: f32, height: f32) -> HotbarMetrics {
    hotbar_window_snapshot(9, width, height, 1.0).metrics
}

/// `slots` drawn in a window of this size with the global font scale at
/// `font_factor`, as `hotbar_grid.rs` lays it out (label line
/// `ceil(font * factor) + 2`, the grid may grow the window downwards).
pub fn hotbar_window_snapshot(slots: usize, width: f32, height: f32, font_factor: f32) -> HotbarSnapshot {
    let theme = InterfaceTheme::turoran_dark();
    let window = &theme.window;
    let chrome = window.title_height + window.title_gap + window.border + theme.button.height + window.gaps;
    let region_width = width - window.border * 2.0;
    let grid_height = height - chrome;
    let label_height = (from_theme_body(theme.tokens.font_size, HOTBAR_LABEL_FONT_PX) * font_factor).ceil() + 2.0;
    let metrics = hotbar_metrics(slots, region_width, Some(grid_height), label_height);
    HotbarSnapshot {
        width,
        height,
        region_width,
        laid_out_height: (chrome + metrics.grid_height().max(grid_height)).clamp(height, HOTBAR_WINDOW_MAXIMUM_HEIGHT),
        metrics,
    }
}

fn snapshot(area: Area) -> HotbarSnapshot {
    hotbar_window_snapshot(9, area.width, area.height, 1.0)
}

fn save(cache: &mut WindowCache, area: Area) {
    let mut anchor = cache
        .get_window_state(WindowClass::Hotbar, RESIZE_SCREEN)
        .map(|(anchor, _)| anchor)
        .unwrap_or_default();
    let size = ScreenSize {
        width: area.width,
        height: area.height,
    };
    anchor.update(
        RESIZE_SCREEN,
        ScreenPosition {
            left: area.left,
            top: area.top,
        },
        size,
    );
    cache.update_anchor(WindowClass::Hotbar, anchor);
    cache.update_size(WindowClass::Hotbar, size);
}

fn restore(cache: &WindowCache) -> Result<Area, String> {
    let (anchor, size) = cache
        .get_window_state(WindowClass::Hotbar, RESIZE_SCREEN)
        .ok_or("no Hotbar state")?;
    let position = anchor.to_position(RESIZE_SCREEN);
    Ok(Area {
        left: position.left,
        top: position.top,
        width: size.width,
        height: size.height,
    })
}

/// Cooldown and cast timeline for one cast of `skill` (cast 1 s, after-cast
/// 0.5 s, cooldown 3 s), sampled at fixed times.
pub fn cooldown_timeline() -> Vec<TimelinePoint> {
    const PLAYER: u32 = 150_000;
    const SKILL: SkillId = SkillId(19);
    const OTHER: SkillId = SkillId(14);
    let mut cooldowns = SkillCooldowns::default();
    let mut bars = CastBars::default();
    let mut timeline = Vec::new();
    let mut sample = |at_ms: u32, cooldowns: &SkillCooldowns, bars: &CastBars| {
        timeline.push(TimelinePoint {
            at_ms,
            used_skill: cooldowns.slot_delay(SKILL, ClientTick(at_ms)),
            other_skill: cooldowns.slot_delay(OTHER, ClientTick(at_ms)),
            bar: bars.iter_visible(at_ms).find(|bar| bar.entity_id == PLAYER).map(|bar| bar.phase),
        });
    };
    // 0x0B1A: cast time 1000 ms.
    bars.begin(PLAYER, SKILL.0, 1_000, 0);
    sample(500, &cooldowns, &bars);
    // Cast ends: 0x043D (3000 ms) and EFST_POSTDELAY (500 ms) arrive.
    cooldowns.set(SKILL, 3_000, ClientTick(1_000));
    cooldowns.start_after_cast(500, 500, ClientTick(1_000));
    bars.begin_after_cast(PLAYER, 500, 1_000);
    sample(1_200, &cooldowns, &bars);
    sample(2_000, &cooldowns, &bars);
    bars.expire(4_100);
    cooldowns.prune(ClientTick(4_100));
    sample(4_100, &cooldowns, &bars);
    timeline
}

/// Run the scenario. `directory` holds the saved cache file.
pub fn offline_hotbar_scenario(directory: &Path) -> Result<OfflineHotbarReport, String> {
    let mut cache = ManuallyDrop::new(WindowCache::default());
    let limits = hotbar_limits();
    let start = Area {
        left: (RESIZE_SCREEN.width - 420.0) / 2.0,
        top: RESIZE_SCREEN.height - HOTBAR_DEFAULT_HEIGHT - 8.0,
        width: limits.clamp_width(420.0),
        height: limits.clamp_height(HOTBAR_DEFAULT_HEIGHT),
    };
    let before = snapshot(start);

    let border = InterfaceTheme::turoran_dark().window.border;
    let (_, corner) = resize_handle_areas(start, border)
        .into_iter()
        .find(|(mode, _)| *mode == ResizeMode::TOP_RIGHT)
        .ok_or("no top-right handle")?;
    let press = (corner.left + corner.width / 2.0, corner.top + corner.height / 2.0);
    let mode = resize_handle_at(start, border, press.0, press.1, limits.allows_width(), true).ok_or("the corner is not a resize handle")?;
    let deltas = scripted_drag(press, (press.0 + HOTBAR_NARROW_DRAG.0, press.1 + HOTBAR_NARROW_DRAG.1));
    let area = deltas.iter().fold(start, |area, delta| {
        resize_window_area(
            area,
            mode,
            delta.width,
            delta.height,
            1.0,
            limits,
            RESIZE_SCREEN.width,
            RESIZE_SCREEN.height,
        )
    });
    let narrowed = snapshot(area);

    save(&mut cache, area);
    let path = directory.join("window_cache.ron");
    cache.save_to(&path).map_err(|error| format!("save {}: {error}", path.display()))?;
    let reloaded_cache = ManuallyDrop::new(WindowCache::load_from(&path).ok_or("saved window cache did not load")?);
    let reloaded = snapshot(restore(&reloaded_cache)?);

    let mut reset_cache = ManuallyDrop::new(WindowCache::load_from(&path).ok_or("saved window cache did not load")?);
    reset_cache.forget_all();
    let reset_forgets = !reset_cache.has_saved_state(WindowClass::Hotbar);

    Ok(OfflineHotbarReport {
        grabbed: Some(mode),
        before,
        narrowed,
        reloaded,
        reset_forgets,
        minimum_18: hotbar_window_snapshot(18, HOTBAR_WINDOW_MINIMUM_WIDTH, HOTBAR_WINDOW_MINIMUM_HEIGHT, 1.0),
        font_scaled: hotbar_window_snapshot(9, 420.0, HOTBAR_DEFAULT_HEIGHT, font_scale_factor(FONT_SCALE_MAX)),
        font_default: hotbar_window_snapshot(9, 420.0, HOTBAR_DEFAULT_HEIGHT, 1.0),
        timeline: cooldown_timeline(),
    })
}
