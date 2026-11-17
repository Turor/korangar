//! Hotbar (Matthew t274u): drag the corner in, check the bar wraps onto more
//! rows with smaller slots, the size survives a save and reload, and the
//! cooldown and cast timeline shows casting, then the after-cast delay on
//! every skill, then the used skill cooling down.

use korangar::e2e::{CastPhaseView, SlotDelayView, offline_hotbar_pointer, offline_hotbar_scenario};
use korangar_interface::layout::ResizeMode;

#[test]
fn hotbar_corner_drag_reflows_and_cooldowns_follow_the_cast_offline() {
    let directory = std::env::temp_dir().join(format!("korangar-e2e-hotbar-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let report = offline_hotbar_scenario(&directory).unwrap();
    std::fs::remove_dir_all(&directory).ok();

    for (label, window) in [
        ("before", report.before),
        ("narrowed", report.narrowed),
        ("reloaded", report.reloaded),
    ] {
        eprintln!(
            "E2E_HOTBAR {label} size={}x{} grid={}x{} slot={}",
            window.width, window.height, window.metrics.columns, window.metrics.rows, window.metrics.slot
        );
    }
    assert_eq!(report.grabbed, Some(ResizeMode::TOP_RIGHT));
    assert_eq!((report.before.metrics.columns, report.before.metrics.rows), (9, 1));
    assert!(report.narrowed.width < report.before.width);
    assert!(report.narrowed.height > report.before.height);
    assert!(report.narrowed.metrics.rows >= 2, "{:?}", report.narrowed);
    assert_eq!(report.reloaded, report.narrowed);
    assert!(report.reset_forgets);

    // Review-1 B2: 18 slots in the smallest window wrap instead of spilling
    // out sideways, and the window grows down to hold them.
    for (label, window) in [
        ("minimum-18", report.minimum_18),
        ("font-default", report.font_default),
        ("font-150", report.font_scaled),
    ] {
        eprintln!(
            "E2E_HOTBAR {label} size={}x{} laid_out_height={} region_width={} grid={}x{} grid_width={} slot={} label={}",
            window.width,
            window.height,
            window.laid_out_height,
            window.region_width,
            window.metrics.columns,
            window.metrics.rows,
            window.metrics.grid_width(),
            window.metrics.slot,
            window.metrics.label_height
        );
        assert!(window.metrics.grid_width() <= window.region_width + 0.01, "{label}: {window:?}");
        assert!(window.laid_out_height >= window.height, "{label}: {window:?}");
    }
    let minimum = report.minimum_18;
    assert!(minimum.metrics.columns * minimum.metrics.rows >= 18);
    assert!(minimum.metrics.rows >= 3, "{minimum:?}");
    assert!(
        minimum.laid_out_height > minimum.height,
        "the window grows to hold the rows: {minimum:?}"
    );
    // Font scale 150%: the label line is taller, so the slots get smaller (or
    // wrap), and everything still fits.
    assert!(report.font_scaled.metrics.label_height > report.font_default.metrics.label_height);
    assert!(report.font_scaled.metrics.slot <= report.font_default.metrics.slot);

    let phases: Vec<_> = report.timeline.iter().map(|point| point.bar).collect();
    assert_eq!(phases, [
        Some(CastPhaseView::Casting),
        Some(CastPhaseView::AfterCast),
        None,
        None
    ]);
    let casting = &report.timeline[0];
    assert!(casting.used_skill.is_none() && casting.other_skill.is_none());
    let after_cast = &report.timeline[1];
    assert!(matches!(after_cast.used_skill, Some(SlotDelayView::Cooldown(view)) if view.remaining_seconds() == 3));
    assert!(matches!(after_cast.other_skill, Some(SlotDelayView::AfterCast(_))));
    let cooling = &report.timeline[2];
    assert!(matches!(cooling.used_skill, Some(SlotDelayView::Cooldown(view)) if view.remaining_seconds() == 2));
    assert!(cooling.other_skill.is_none());
    let ready = &report.timeline[3];
    assert!(ready.used_skill.is_none() && ready.other_skill.is_none());
}

/// Matthew t278u: right-click opens info and keeps the slot, a drop on a
/// slot swaps, Escape cancels, a drop off the bar clears.
#[test]
fn hotbar_right_click_opens_info_and_dragging_off_clears_offline() {
    const SKILL: u8 = 1;
    const ITEM: u8 = 2;
    const EMPTY: u8 = 0;
    let report = offline_hotbar_pointer();
    eprintln!("E2E_HOTBAR_POINTER {report:?}");
    assert_eq!(report.right_click_titles, [
        Some("Endure".to_owned()),
        Some("Red Potion".to_owned()),
        None
    ]);
    assert_eq!(report.right_click_kinds, ["skill description", "item details"]);
    assert_eq!(report.after_right_click, [SKILL, ITEM], "right-click must not clear");
    assert_eq!(report.after_swap, [ITEM, SKILL]);
    assert_eq!(report.swap_updates, [(1, true), (0, false)]);
    assert!(report.escape_cancels);
    assert!(!report.cleared_after_escape, "Escape leaves the slot alone");
    assert_eq!(report.cleared_off_bar, [0, 1]);
    assert_eq!(report.after_drag_off, [EMPTY, EMPTY]);
}
