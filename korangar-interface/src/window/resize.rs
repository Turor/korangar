//! Edge and corner resizing for windows.
//!
//! Window positions are screen pixels and window sizes are interface units
//! (screen pixels divided by the interface scaling), the same split the rest
//! of the window code uses. Everything here is pure math so the client tests
//! and the e2e harness can run it without a renderer.

use crate::layout::ResizeMode;
use crate::layout::area::Area;

/// Thickness of the edge grab strips, in interface units.
pub const RESIZE_EDGE: f32 = 5.0;
/// Side of the corner grab squares, in interface units.
pub const RESIZE_CORNER: f32 = 14.0;

/// Size limits of one window, in interface units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeLimits {
    pub minimum_width: f32,
    pub maximum_width: f32,
    pub minimum_height: f32,
    pub maximum_height: f32,
}

impl ResizeLimits {
    /// A maximum below its minimum is raised to the minimum, and negative or
    /// non-finite minimums become zero.
    pub fn new(minimum_width: f32, maximum_width: f32, minimum_height: f32, maximum_height: f32) -> Self {
        let sane = |value: f32| if value.is_finite() { value.max(0.0) } else { 0.0 };
        let minimum_width = sane(minimum_width);
        let minimum_height = sane(minimum_height);
        Self {
            minimum_width,
            maximum_width: maximum_width.max(minimum_width),
            minimum_height,
            maximum_height: maximum_height.max(minimum_height),
        }
    }

    /// No limits beyond "not negative".
    pub fn unbounded() -> Self {
        Self::new(0.0, f32::MAX, 0.0, f32::MAX)
    }

    /// The width can change at all.
    pub fn allows_width(&self) -> bool {
        self.maximum_width > self.minimum_width
    }

    pub fn clamp_width(&self, width: f32) -> f32 {
        width.clamp(self.minimum_width, self.maximum_width)
    }

    pub fn clamp_height(&self, height: f32) -> f32 {
        height.clamp(self.minimum_height, self.maximum_height)
    }
}

/// The eight grab areas of a window: corners first so they win where they
/// overlap the edges. `area` is the window as laid out (interface units
/// relative to its screen position, like every [`Area`] in a window layout).
pub fn resize_handle_areas(area: Area) -> [(ResizeMode, Area); 8] {
    let corner = RESIZE_CORNER.min(area.width / 2.0).min(area.height / 2.0).max(0.0);
    let edge = RESIZE_EDGE.min(corner).max(0.0);
    let right = area.left + area.width;
    let bottom = area.top + area.height;
    let square = |left: f32, top: f32| Area {
        left,
        top,
        width: corner,
        height: corner,
    };
    let inner_width = (area.width - corner * 2.0).max(0.0);
    let inner_height = (area.height - corner * 2.0).max(0.0);
    [
        (ResizeMode::TOP_LEFT, square(area.left, area.top)),
        (ResizeMode::TOP_RIGHT, square(right - corner, area.top)),
        (ResizeMode::BOTTOM_LEFT, square(area.left, bottom - corner)),
        (ResizeMode::BOTTOM_RIGHT, square(right - corner, bottom - corner)),
        (ResizeMode::LEFT, Area {
            left: area.left,
            top: area.top + corner,
            width: edge,
            height: inner_height,
        }),
        (ResizeMode::RIGHT, Area {
            left: right - edge,
            top: area.top + corner,
            width: edge,
            height: inner_height,
        }),
        (ResizeMode::TOP, Area {
            left: area.left + corner,
            top: area.top,
            width: inner_width,
            height: edge,
        }),
        (ResizeMode::BOTTOM, Area {
            left: area.left + corner,
            top: bottom - edge,
            width: inner_width,
            height: edge,
        }),
    ]
}

fn contains(area: Area, x: f32, y: f32) -> bool {
    x >= area.left && y >= area.top && x <= area.left + area.width && y <= area.top + area.height
}

/// The handle under `(x, y)` (layout coordinates), restricted to the axes
/// the window allows. Corners of a window that only grows one way resize
/// that axis.
pub fn resize_handle_at(area: Area, x: f32, y: f32, horizontal: bool, vertical: bool) -> Option<ResizeMode> {
    resize_handle_areas(area)
        .into_iter()
        .filter(|(_, handle)| contains(*handle, x, y))
        .find_map(|(mode, _)| mode.restricted(horizontal, vertical))
}

/// New window area after dragging `mode` by `(delta_x, delta_y)` screen
/// pixels.
///
/// `area.left`/`area.top` are screen pixels, `area.width`/`area.height`
/// interface units. The dragged edges follow the pointer; the opposite edges
/// stay put. Sizes stay inside `limits`, and the window never grows past the
/// screen edges (a window already larger than the screen is not shrunk by a
/// drag on the other axis).
#[allow(clippy::too_many_arguments)]
pub fn resize_window_area(
    area: Area,
    mode: ResizeMode,
    delta_x: f32,
    delta_y: f32,
    scaling: f32,
    limits: ResizeLimits,
    screen_width: f32,
    screen_height: f32,
) -> Area {
    let scaling = if scaling.is_finite() && scaling > 0.0 { scaling } else { 1.0 };
    let mut result = area;

    let (left, width) = resize_axis(
        area.left,
        area.width,
        mode.left,
        mode.right,
        delta_x,
        scaling,
        limits.minimum_width,
        limits.maximum_width,
        screen_width,
    );
    result.left = left;
    result.width = width;

    let (top, height) = resize_axis(
        area.top,
        area.height,
        mode.top,
        mode.bottom,
        delta_y,
        scaling,
        limits.minimum_height,
        limits.maximum_height,
        screen_height,
    );
    result.top = top;
    result.height = height;

    result
}

#[allow(clippy::too_many_arguments)]
fn resize_axis(
    start: f32,
    size: f32,
    move_start: bool,
    move_end: bool,
    delta: f32,
    scaling: f32,
    minimum: f32,
    maximum: f32,
    screen: f32,
) -> (f32, f32) {
    if move_end {
        // Right or bottom edge: the start stays, the end may not leave the screen.
        let room = ((screen - start) / scaling).max(minimum);
        let size = (size + delta / scaling).clamp(minimum, maximum.min(room).max(minimum));
        (start, size)
    } else if move_start {
        // Left or top edge: the end stays, the start may not go past zero.
        let end = start + size * scaling;
        let room = (end / scaling).max(minimum);
        let size = (size - delta / scaling).clamp(minimum, maximum.min(room).max(minimum));
        (end - size * scaling, size)
    } else {
        (start, size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (f32, f32) = (1280.0, 720.0);

    fn window() -> Area {
        Area {
            left: 400.0,
            top: 200.0,
            width: 300.0,
            height: 260.0,
        }
    }

    fn limits() -> ResizeLimits {
        ResizeLimits::new(200.0, 600.0, 150.0, 500.0)
    }

    fn resize(mode: ResizeMode, dx: f32, dy: f32) -> Area {
        resize_window_area(window(), mode, dx, dy, 1.0, limits(), SCREEN.0, SCREEN.1)
    }

    #[test]
    fn bottom_right_corner_grows_both_axes_and_keeps_the_origin() {
        let area = resize(ResizeMode::BOTTOM_RIGHT, 50.0, 40.0);
        assert_eq!((area.left, area.top, area.width, area.height), (400.0, 200.0, 350.0, 300.0));
    }

    #[test]
    fn top_left_corner_moves_the_origin_and_keeps_the_far_edges() {
        let area = resize(ResizeMode::TOP_LEFT, -30.0, -20.0);
        assert_eq!((area.left, area.top, area.width, area.height), (370.0, 180.0, 330.0, 280.0));
        assert_eq!(area.left + area.width, 700.0);
        assert_eq!(area.top + area.height, 460.0);
    }

    #[test]
    fn single_edges_change_one_axis() {
        let right = resize(ResizeMode::RIGHT, 25.0, 99.0);
        assert_eq!((right.width, right.height), (325.0, 260.0));
        let bottom = resize(ResizeMode::BOTTOM, 99.0, -60.0);
        assert_eq!((bottom.width, bottom.height), (300.0, 200.0));
        let left = resize(ResizeMode::LEFT, 40.0, 0.0);
        assert_eq!((left.left, left.width), (440.0, 260.0));
        let top = resize(ResizeMode::TOP, 0.0, 10.0);
        assert_eq!((top.top, top.height), (210.0, 250.0));
    }

    #[test]
    fn sizes_stop_at_the_minimum_and_maximum() {
        assert_eq!(resize(ResizeMode::RIGHT, -500.0, 0.0).width, 200.0);
        assert_eq!(resize(ResizeMode::RIGHT, 900.0, 0.0).width, 600.0);
        let shrunk = resize(ResizeMode::LEFT, 500.0, 0.0);
        assert_eq!(
            (shrunk.left, shrunk.width),
            (500.0, 200.0),
            "left edge stops when the minimum is hit"
        );
        assert_eq!(resize(ResizeMode::BOTTOM, 0.0, 900.0).height, 500.0);
    }

    #[test]
    fn drags_are_clamped_to_the_screen() {
        // Right edge: 400 + w <= 1280 even though the maximum is larger.
        let wide = ResizeLimits::new(200.0, 5_000.0, 150.0, 5_000.0);
        let area = resize_window_area(
            window(),
            ResizeMode::BOTTOM_RIGHT,
            5_000.0,
            5_000.0,
            1.0,
            wide,
            SCREEN.0,
            SCREEN.1,
        );
        assert_eq!((area.width, area.height), (880.0, 520.0));
        // Left/top edge: never past 0.
        let area = resize_window_area(
            window(),
            ResizeMode::TOP_LEFT,
            -5_000.0,
            -5_000.0,
            1.0,
            wide,
            SCREEN.0,
            SCREEN.1,
        );
        assert_eq!((area.left, area.top, area.width, area.height), (0.0, 0.0, 700.0, 460.0));
    }

    #[test]
    fn interface_scaling_converts_pointer_pixels_to_interface_units() {
        let wide = ResizeLimits::new(100.0, 5_000.0, 100.0, 5_000.0);
        let area = resize_window_area(window(), ResizeMode::BOTTOM_RIGHT, 40.0, -20.0, 2.0, wide, SCREEN.0, SCREEN.1);
        assert_eq!((area.width, area.height), (320.0, 250.0));
        // 260 units at 2x already reach the bottom of the 720 px screen.
        let area = resize_window_area(window(), ResizeMode::BOTTOM, 0.0, 20.0, 2.0, wide, SCREEN.0, SCREEN.1);
        assert_eq!(area.height, 260.0);
        let area = resize_window_area(window(), ResizeMode::LEFT, -40.0, 0.0, 2.0, wide, SCREEN.0, SCREEN.1);
        assert_eq!((area.left, area.width), (360.0, 320.0));
        // At 2x the right edge of a 300-unit window starting at 400 px ends at 1000 px.
        let area = resize_window_area(window(), ResizeMode::RIGHT, 5_000.0, 0.0, 2.0, wide, SCREEN.0, SCREEN.1);
        assert_eq!(area.width, 440.0);
    }

    #[test]
    fn limits_repair_inverted_or_broken_values() {
        let limits = ResizeLimits::new(300.0, 300.0, -5.0, f32::NAN);
        assert!(!limits.allows_width());
        assert_eq!(limits.minimum_height, 0.0);
        assert_eq!(limits.clamp_width(999.0), 300.0);
        let inverted = ResizeLimits::new(400.0, 100.0, 0.0, 10.0);
        assert_eq!(inverted.maximum_width, 400.0);
        assert!(ResizeLimits::unbounded().allows_width());
    }

    #[test]
    fn every_corner_and_edge_has_a_handle() {
        let area = window();
        let hit = |x: f32, y: f32| resize_handle_at(area, x, y, true, true);
        assert_eq!(hit(401.0, 201.0), Some(ResizeMode::TOP_LEFT));
        assert_eq!(hit(699.0, 201.0), Some(ResizeMode::TOP_RIGHT));
        assert_eq!(hit(401.0, 459.0), Some(ResizeMode::BOTTOM_LEFT));
        assert_eq!(hit(699.0, 459.0), Some(ResizeMode::BOTTOM_RIGHT));
        assert_eq!(hit(402.0, 330.0), Some(ResizeMode::LEFT));
        assert_eq!(hit(698.0, 330.0), Some(ResizeMode::RIGHT));
        assert_eq!(hit(550.0, 202.0), Some(ResizeMode::TOP));
        assert_eq!(hit(550.0, 458.0), Some(ResizeMode::BOTTOM));
        assert_eq!(hit(550.0, 330.0), None, "the middle of the window is not a handle");
        assert_eq!(hit(390.0, 330.0), None, "outside the window is not a handle");
    }

    #[test]
    fn handles_respect_the_axes_a_window_allows() {
        let area = window();
        // Fixed width, growing height: corners only resize vertically, side edges do
        // nothing.
        assert_eq!(resize_handle_at(area, 699.0, 459.0, false, true), Some(ResizeMode::BOTTOM));
        assert_eq!(resize_handle_at(area, 401.0, 201.0, false, true), Some(ResizeMode::TOP));
        assert_eq!(resize_handle_at(area, 698.0, 330.0, false, true), None);
        assert_eq!(resize_handle_at(area, 699.0, 459.0, false, false), None);
        assert_eq!(resize_handle_at(area, 699.0, 459.0, true, false), Some(ResizeMode::RIGHT));
    }

    #[test]
    fn tiny_windows_still_get_non_overlapping_handles() {
        let area = Area {
            left: 0.0,
            top: 0.0,
            width: 16.0,
            height: 16.0,
        };
        for (_, handle) in resize_handle_areas(area) {
            assert!(handle.width >= 0.0 && handle.height >= 0.0);
            assert!(handle.left >= 0.0 && handle.left + handle.width <= 16.0 + f32::EPSILON);
        }
    }
}
