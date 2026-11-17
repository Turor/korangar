//! Resizable windows, headless (Matthew t272u).
//!
//! [`offline_window_resize`] grabs the Inventory window's bottom-right
//! corner with scripted `InputSystem` drags (out, then back in), applies
//! every drag delta through
//! the same [`resize_window_area`] the interface uses, sizes the item grid
//! for each width with [`grid_metrics`], and saves and reloads the window
//! cache file. It also drags the top-left corner far off screen and checks
//! the clamp, and checks that a reset forgets the saved layout.

use std::mem::ManuallyDrop;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use korangar_interface::application::WindowCache as _;
use korangar_interface::layout::ResizeMode;
use korangar_interface::layout::area::Area;
use korangar_interface::window::{ResizeLimits, resize_handle_areas, resize_handle_at, resize_window_area};
use ragnarok_packets::ClientTick;

use crate::graphics::{ScreenPosition, ScreenSize};
use crate::input::InputSystem;
use crate::interface::components::item_grid::{GridMetrics, grid_metrics};
use crate::interface::windows::resizable::{GRID_WINDOW_MAXIMUM_WIDTH, GRID_WINDOW_MINIMUM_WIDTH};
use crate::interface::windows::{WindowCache, WindowClass};
use crate::state::theme::InterfaceTheme;

/// Client size the scenario runs at.
pub const RESIZE_SCREEN: ScreenSize = ScreenSize {
    width: 1280.0,
    height: 720.0,
};
/// First drag: the corner goes down and to the right.
pub const GROW_DRAG: (f32, f32) = (360.0, 180.0);
/// Second drag: the corner comes back up and to the left.
pub const SHRINK_DRAG: (f32, f32) = (-240.0, -120.0);

/// One window as drawn: position in screen pixels, size in interface units,
/// and the grid it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSnapshot {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub grid: GridMetrics,
}

/// What the offline run observed.
#[derive(Clone, Debug, PartialEq)]
pub struct OfflineResizeReport {
    /// Handle under the pointer where the drag started.
    pub grabbed: Option<ResizeMode>,
    /// Drag deltas the input system reported.
    pub drag_frames: usize,
    /// First-open preset as drawn (size clamped to the window limits).
    pub before: WindowSnapshot,
    /// After [`GROW_DRAG`].
    pub grown: WindowSnapshot,
    /// After [`SHRINK_DRAG`] from [`Self::grown`].
    pub shrunk: WindowSnapshot,
    /// The shrunk window read back from the saved cache file.
    pub reloaded: WindowSnapshot,
    /// The shrunk window moved to (500, 300), then its top-left corner
    /// dragged 5000 px up and left.
    pub grown_top_left: WindowSnapshot,
    /// No saved state for the Inventory after a reset.
    pub reset_forgets: bool,
}

/// Inventory window limits (`inventory.rs`).
pub fn inventory_limits() -> ResizeLimits {
    ResizeLimits::new(GRID_WINDOW_MINIMUM_WIDTH, GRID_WINDOW_MAXIMUM_WIDTH, 240.0, 720.0)
}

/// Grid inside a window `width` wide: the window border on both sides, the
/// window gaps between slots (as `with_derived_borderless` and the item grid
/// do).
pub fn window_grid(width: f32) -> GridMetrics {
    let theme = InterfaceTheme::turoran_dark();
    grid_metrics(width - theme.window.border * 2.0, theme.window.gaps)
}

fn snapshot(area: Area) -> WindowSnapshot {
    WindowSnapshot {
        left: area.left,
        top: area.top,
        width: area.width,
        height: area.height,
        grid: window_grid(area.width),
    }
}

/// Press at `start`, move to `end`, release, through the scripted input
/// path. Returns the drag deltas the frames reported.
pub fn scripted_drag(start: (f32, f32), end: (f32, f32)) -> Vec<ScreenSize> {
    let mut input = InputSystem::new(Arc::new(AtomicU64::new(0)));
    let mut deltas = Vec::new();
    for frame in 0..4u32 {
        input.expire_synthesized();
        input.advance_synthetic_drag();
        if frame == 0 {
            input.begin_synthetic_drag(start.0, start.1, end.0, end.1);
        }
        let report = input.update_delta(ClientTick(frame * 16));
        if let Some(delta) = report.drag
            && (delta.width != 0.0 || delta.height != 0.0)
        {
            deltas.push(delta);
        }
    }
    deltas
}

fn center(area: Area) -> (f32, f32) {
    (area.left + area.width / 2.0, area.top + area.height / 2.0)
}

fn save_like_the_interface(cache: &mut WindowCache, area: Area) {
    let mut anchor = cache
        .get_window_state(WindowClass::Inventory, RESIZE_SCREEN)
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
    cache.update_anchor(WindowClass::Inventory, anchor);
    cache.update_size(WindowClass::Inventory, size);
}

fn restore(cache: &WindowCache) -> Result<Area, String> {
    let (anchor, size) = cache
        .get_window_state(WindowClass::Inventory, RESIZE_SCREEN)
        .ok_or("no Inventory state")?;
    let position = anchor.to_position(RESIZE_SCREEN);
    Ok(Area {
        left: position.left,
        top: position.top,
        width: size.width,
        height: size.height,
    })
}

/// Run the scenario. `directory` holds the saved cache file.
pub fn offline_window_resize(directory: &Path) -> Result<OfflineResizeReport, String> {
    // `WindowCache` saves to the client's own file when dropped; the test
    // copies never touch it.
    let mut cache = ManuallyDrop::new(WindowCache::default());
    let limits = inventory_limits();

    // The window draws its saved size clamped to its limits.
    let mut start = restore(&cache)?;
    start.width = limits.clamp_width(start.width);
    start.height = limits.clamp_height(start.height);
    let before = snapshot(start);

    let mut grabbed = None;
    let mut drag_frames = 0;
    let mut corner_drag = |area: Area, by: (f32, f32)| -> Result<Area, String> {
        let (_, corner) = resize_handle_areas(area)
            .into_iter()
            .find(|(mode, _)| *mode == ResizeMode::BOTTOM_RIGHT)
            .ok_or("no bottom-right handle")?;
        let press = center(corner);
        let mode = resize_handle_at(area, press.0, press.1, limits.allows_width(), true).ok_or("the corner is not a resize handle")?;
        grabbed = Some(mode);
        let deltas = scripted_drag(press, (press.0 + by.0, press.1 + by.1));
        drag_frames += deltas.len();
        Ok(deltas.iter().fold(area, |area, delta| {
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
        }))
    };
    let grown_area = corner_drag(start, GROW_DRAG)?;
    let area = corner_drag(grown_area, SHRINK_DRAG)?;
    let grown = snapshot(grown_area);
    let shrunk = snapshot(area);
    save_like_the_interface(&mut cache, area);

    let path = directory.join("window_cache.ron");
    cache.save_to(&path).map_err(|error| format!("save {}: {error}", path.display()))?;
    let reloaded_cache = ManuallyDrop::new(WindowCache::load_from(&path).ok_or("saved window cache did not load")?);
    let reloaded = snapshot(restore(&reloaded_cache)?);

    // Moved to the middle of the screen first so the drag has room.
    let middle = Area {
        left: 500.0,
        top: 300.0,
        ..area
    };
    let stretched = resize_window_area(
        middle,
        ResizeMode::TOP_LEFT,
        -5_000.0,
        -5_000.0,
        1.0,
        limits,
        RESIZE_SCREEN.width,
        RESIZE_SCREEN.height,
    );
    let grown_top_left = snapshot(stretched);

    let mut reset_cache = ManuallyDrop::new(WindowCache::load_from(&path).ok_or("saved window cache did not load")?);
    reset_cache.forget_all();
    let reset_forgets = !reset_cache.has_saved_state(WindowClass::Inventory);

    Ok(OfflineResizeReport {
        grabbed,
        drag_frames,
        before,
        grown,
        shrunk,
        reloaded,
        grown_top_left,
        reset_forgets,
    })
}
