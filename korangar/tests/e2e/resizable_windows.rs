//! Resizable windows (Matthew t272u): drag the Inventory's bottom-right
//! corner out and back in, check the grid reflows (more and larger squares,
//! then fewer and smaller ones), and check the new size and position
//! survive a save and reload of the window cache.

use korangar::e2e::{GROW_DRAG, RESIZE_SCREEN, SHRINK_DRAG, inventory_limits, offline_window_resize, scripted_drag, window_grid};
use korangar_interface::layout::ResizeMode;

#[test]
fn inventory_corner_drag_reflows_the_grid_and_persists_offline() {
    let directory = std::env::temp_dir().join(format!("korangar-e2e-resize-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let report = offline_window_resize(&directory).unwrap();
    std::fs::remove_dir_all(&directory).ok();

    for (label, window) in [("before", report.before), ("grown", report.grown), ("shrunk", report.shrunk)] {
        eprintln!(
            "E2E_RESIZE {label} at=({},{}) size={}x{} columns={} slot={}",
            window.left, window.top, window.width, window.height, window.grid.columns, window.grid.slot
        );
    }

    assert_eq!(
        report.grabbed,
        Some(ResizeMode::BOTTOM_RIGHT),
        "the corner must grab both edges"
    );
    assert_eq!(report.drag_frames, 2, "each scripted drag reports one move");

    // The corner followed the pointer (within the limits); the top-left corner
    // stayed put.
    let limits = inventory_limits();
    for window in [report.grown, report.shrunk] {
        assert_eq!((window.left, window.top), (report.before.left, report.before.top));
    }
    let expected_grown = (
        limits
            .clamp_width(report.before.width + GROW_DRAG.0)
            .min(RESIZE_SCREEN.width - report.before.left),
        limits
            .clamp_height(report.before.height + GROW_DRAG.1)
            .min(RESIZE_SCREEN.height - report.before.top),
    );
    assert_eq!((report.grown.width, report.grown.height), expected_grown);
    let expected_shrunk = (
        limits.clamp_width(report.grown.width + SHRINK_DRAG.0),
        limits.clamp_height(report.grown.height + SHRINK_DRAG.1),
    );
    assert_eq!((report.shrunk.width, report.shrunk.height), expected_shrunk);

    // A wider window gives more and larger squares, a narrower one fewer and
    // smaller squares.
    assert!(report.grown.grid.columns > report.before.grid.columns, "{report:?}");
    assert!(report.grown.grid.slot > report.before.grid.slot, "{report:?}");
    assert!(report.shrunk.grid.columns < report.grown.grid.columns, "{report:?}");
    assert!(report.shrunk.grid.slot < report.grown.grid.slot, "{report:?}");

    // Size, position and therefore the grid survive a reload.
    assert_eq!(report.reloaded, report.shrunk);

    // The scenario moves the window to (500, 300) first. The top-left corner
    // stops at the screen edge (or where the maximum height stops it) and the
    // far corner stays.
    let bottom = 300.0 + report.shrunk.height;
    assert_eq!(report.grown_top_left.left, 0.0);
    assert!(
        (report.grown_top_left.top - (bottom - report.grown_top_left.height).max(0.0)).abs() < 0.5,
        "the bottom edge moved: {report:?}"
    );
    assert!(
        (report.grown_top_left.width - (500.0 + report.shrunk.width)).abs() < 0.5,
        "the right edge moved: {report:?}"
    );
    assert!((report.grown_top_left.height - (300.0 + report.shrunk.height).min(limits.maximum_height)).abs() < 0.5);
    assert!(report.grown_top_left.width <= RESIZE_SCREEN.width);
    assert!(report.grown_top_left.grid.columns > report.shrunk.grid.columns);

    assert!(report.reset_forgets, "reset window layout kept the Inventory size");
}

#[test]
fn grid_grows_with_the_window_offline() {
    let widths = [240.0, 320.0, 420.0, 560.0, 800.0];
    let grids: Vec<_> = widths.iter().map(|width| window_grid(*width)).collect();
    for pair in grids.windows(2) {
        assert!(pair[0].slot <= pair[1].slot, "{grids:?}");
    }
    assert!(grids[4].columns > grids[0].columns, "{grids:?}");
    assert!(grids[0].slot >= 24.0 && grids[4].slot <= 48.0);

    let deltas = scripted_drag((100.0, 100.0), (60.0, 140.0));
    let total = deltas
        .iter()
        .fold((0.0, 0.0), |sum, delta| (sum.0 + delta.width, sum.1 + delta.height));
    assert_eq!(total, (-40.0, 40.0));
}
