//! Zoom window, marker mapping, and the facing arrow for the minimap.
//!
//! Zoom 0 is the full map and matches [`korangar_networking::minimap_point`].
//! Higher levels crop a fraction of the map around the player. Gat Y grows
//! north; screen Y grows down.

use ragnarok_packets::{Direction, TilePosition};

use crate::state::navigation::MinimapMark;

/// Zoom steps. Index 0 shows the whole map.
pub const ZOOM_LEVEL_COUNT: u8 = 4;
pub const ZOOM_SPANS: [f32; 4] = [1.0, 0.5, 0.25, 0.125];

const _: () = assert!(ZOOM_LEVEL_COUNT >= 3);

/// Cells this close to the crop edge still draw.
const EDGE: f32 = 0.01;

/// Visible fraction of the map. Origins are fractions of the map, screen Y
/// down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapWindow {
    pub origin_x: f32,
    pub origin_y: f32,
    pub span: f32,
}

/// One viewpoint mark in canvas pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlottedMark {
    pub slot: u8,
    pub x: f32,
    pub y: f32,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// One box of the player arrow, offset from the player point. Screen Y grows
/// down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrowBox {
    pub dx: f32,
    pub dy: f32,
    pub size: f32,
}

pub fn step_zoom(level: u8, delta: i8) -> u8 {
    let level = i16::from(level.min(ZOOM_LEVEL_COUNT - 1));
    let next = level + i16::from(delta);
    next.clamp(0, i16::from(ZOOM_LEVEL_COUNT - 1)) as u8
}

/// Crop centered on the player. The origin clamps so the window stays on the
/// map.
pub fn map_window(level: u8, map_width: u16, map_height: u16, player_x: u16, player_y: u16) -> MapWindow {
    let level = usize::from(level.min(ZOOM_LEVEL_COUNT - 1));
    let span = ZOOM_SPANS[level];
    if map_width == 0 || map_height == 0 || span >= 1.0 {
        return MapWindow {
            origin_x: 0.0,
            origin_y: 0.0,
            span: 1.0,
        };
    }
    let px = (f32::from(player_x) + 0.5) / f32::from(map_width);
    let py = 1.0 - (f32::from(player_y) + 0.5) / f32::from(map_height);
    let max_origin = (1.0 - span).max(0.0);
    MapWindow {
        origin_x: (px - span * 0.5).clamp(0.0, max_origin),
        origin_y: (py - span * 0.5).clamp(0.0, max_origin),
        span,
    }
}

/// Canvas point for one cell. `None` when the map size is unknown, the cell is
/// off the map, or the cell is outside the zoom window.
pub fn plot_cell(window: MapWindow, map_width: u16, map_height: u16, x: u16, y: u16, size: f32) -> Option<(f32, f32)> {
    if map_width == 0 || map_height == 0 || size <= 0.0 || x >= map_width || y >= map_height {
        return None;
    }
    if window.span >= 1.0 {
        return korangar_networking::minimap_point(map_width, map_height, TilePosition { x, y }, size);
    }
    let fx = (f32::from(x) + 0.5) / f32::from(map_width);
    let fy = 1.0 - (f32::from(y) + 0.5) / f32::from(map_height);
    let sx = (fx - window.origin_x) / window.span * size;
    let sy = (fy - window.origin_y) / window.span * size;
    if sx < -EDGE || sy < -EDGE || sx > size + EDGE || sy > size + EDGE {
        return None;
    }
    Some((sx.clamp(0.0, size), sy.clamp(0.0, size)))
}

pub fn plot_marks(marks: &[MinimapMark], window: MapWindow, map_width: u16, map_height: u16, size: f32) -> Vec<PlottedMark> {
    let mut plotted = Vec::new();
    for mark in marks {
        let Ok(x) = u16::try_from(mark.x) else { continue };
        let Ok(y) = u16::try_from(mark.y) else { continue };
        let Some((px, py)) = plot_cell(window, map_width, map_height, x, y, size) else {
            continue;
        };
        plotted.push(PlottedMark {
            slot: mark.slot,
            x: px,
            y: py,
            red: mark.red,
            green: mark.green,
            blue: mark.blue,
        });
    }
    plotted
}

/// Texture rectangle for a zoomed map image. The caller clips it to the canvas.
pub fn zoomed_image_area(left: f32, top: f32, size: f32, window: MapWindow) -> (f32, f32, f32, f32) {
    let span = window.span.max(f32::EPSILON);
    let width = size / span;
    let height = size / span;
    (left - window.origin_x * width, top - window.origin_y * height, width, height)
}

fn facing_unit(direction: Direction) -> (f32, f32) {
    let (x, y): (f32, f32) = match direction {
        Direction::North => (0.0, -1.0),
        Direction::NorthEast => (1.0, -1.0),
        Direction::East => (1.0, 0.0),
        Direction::SouthEast => (1.0, 1.0),
        Direction::South => (0.0, 1.0),
        Direction::SouthWest => (-1.0, 1.0),
        Direction::West => (-1.0, 0.0),
        Direction::NorthWest => (-1.0, -1.0),
    };
    let length = (x * x + y * y).sqrt();
    (x / length, y / length)
}

/// Tip, middle, then tail. The tip is the small box in the facing direction.
pub fn arrow_boxes(direction: Direction) -> [ArrowBox; 3] {
    let (x, y) = facing_unit(direction);
    [
        ArrowBox {
            dx: x * 8.0,
            dy: y * 8.0,
            size: 4.0,
        },
        ArrowBox {
            dx: x * 2.0,
            dy: y * 2.0,
            size: 6.0,
        },
        ArrowBox {
            dx: x * -5.0,
            dy: y * -5.0,
            size: 8.0,
        },
    ]
}

pub fn missing_image_log(file: &str) -> String {
    format!("minimap: no image for {file}; keeping the flat square")
}

/// `Settled` when there is nothing to load. `Needed` when the file differs from
/// the path remembered for this map.
pub fn image_needs_load(file: &str, loaded: &str) -> bool {
    !file.is_empty() && file != loaded
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{Direction, TilePosition};

    use super::{ZOOM_LEVEL_COUNT, arrow_boxes, map_window, plot_cell, plot_marks, step_zoom, zoomed_image_area};
    use crate::state::navigation::{MinimapMarkUpdate, Navigation};

    #[test]
    fn zoom_zero_matches_the_full_map_point() {
        let window = map_window(0, 100, 100, 50, 50);
        let size = 100.0;
        for (x, y) in [(0, 0), (99, 99), (50, 50)] {
            let plotted = plot_cell(window, 100, 100, x, y, size).unwrap();
            let expected = korangar_networking::minimap_point(100, 100, TilePosition { x, y }, size).unwrap();
            assert_eq!(plotted, expected);
        }
    }

    #[test]
    fn each_zoom_centers_the_player_until_the_crop_hits_the_edge() {
        for level in 1..ZOOM_LEVEL_COUNT {
            let window = map_window(level, 100, 100, 50, 50);
            let point = plot_cell(window, 100, 100, 50, 50, 100.0).unwrap();
            assert!(
                (point.0 - 50.0).abs() < 0.05 && (point.1 - 50.0).abs() < 0.05,
                "level {level} plotted {point:?}"
            );
        }
        let clamped = map_window(1, 100, 100, 0, 0);
        let corner = plot_cell(clamped, 100, 100, 0, 0, 100.0).unwrap();
        assert!(corner.0 < 50.0, "clamped origin keeps the corner left of center");
        assert!(plot_cell(clamped, 100, 100, 99, 99, 100.0).is_none());
        assert!(plot_cell(map_window(0, 0, 0, 0, 0), 0, 0, 0, 0, 100.0).is_none());
    }

    #[test]
    fn zoom_steps_stay_inside_the_level_list() {
        assert_eq!(step_zoom(0, 1), 1);
        assert_eq!(step_zoom(0, -1), 0);
        assert_eq!(step_zoom(ZOOM_LEVEL_COUNT - 1, 1), ZOOM_LEVEL_COUNT - 1);
        assert_eq!(step_zoom(2, -1), 1);
    }

    #[test]
    fn zoomed_image_grows_around_the_crop() {
        let window = map_window(1, 100, 100, 50, 50);
        let (left, top, width, height) = zoomed_image_area(10.0, 20.0, 100.0, window);
        assert!((width - 200.0).abs() < 0.01);
        assert!((height - 200.0).abs() < 0.01);
        assert!(left < 10.0);
        assert!(top < 20.0);
    }

    #[test]
    fn marks_plot_with_packet_color_and_clear_on_remove_or_map_change() {
        let mut navigation = Navigation::default();
        navigation.on_map_change("prontera.gat", 50, 50);
        navigation.apply_mark(MinimapMarkUpdate {
            npc_id: 7,
            kind: 1,
            x: 10,
            y: 20,
            slot: 3,
            red: 10,
            green: 20,
            blue: 30,
            now: 0,
        });
        let window = map_window(0, 100, 100, 50, 50);
        let plotted = plot_marks(navigation.marks(), window, 100, 100, 100.0);
        assert_eq!(plotted.len(), 1);
        assert_eq!(plotted[0].slot, 3);
        assert_eq!((plotted[0].red, plotted[0].green, plotted[0].blue), (10, 20, 30));

        navigation.apply_mark(MinimapMarkUpdate {
            npc_id: 7,
            kind: 2,
            x: 0,
            y: 0,
            slot: 3,
            red: 0,
            green: 0,
            blue: 0,
            now: 0,
        });
        assert!(plot_marks(navigation.marks(), window, 100, 100, 100.0).is_empty());

        navigation.apply_mark(MinimapMarkUpdate {
            npc_id: 7,
            kind: 1,
            x: 10,
            y: 20,
            slot: 3,
            red: 1,
            green: 2,
            blue: 3,
            now: 0,
        });
        navigation.on_map_change("geffen.gat", 1, 1);
        assert!(navigation.marks().is_empty());
    }

    #[test]
    fn arrow_tip_follows_facing() {
        let north = arrow_boxes(Direction::North);
        assert!(north[0].dy < north[2].dy, "north tip is above the tail");
        let east = arrow_boxes(Direction::East);
        assert!(east[0].dx > east[2].dx, "east tip is to the right of the tail");
        assert!(north[0].size < north[2].size);
    }
}
