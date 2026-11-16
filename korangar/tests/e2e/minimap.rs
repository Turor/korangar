//! Minimap: always open, zoom, viewpoint marks, and hide-interface.

use korangar::e2e::{LiveConnect, LogicSession, MinimapHarness};

fn live_session() -> Option<LogicSession> {
    match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            None
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => Some(session),
    }
}

#[test]
fn minimap_stays_open_zooms_marks_and_hides() {
    let mut harness = MinimapHarness::new();
    assert!(!harness.view().open);

    harness.enter_map("prontera.gat", 132, 98);
    let opened = harness.view();
    assert!(opened.open);
    assert!(!opened.closable);
    assert!(!opened.hidden);
    assert_eq!(opened.caption, "prontera (132, 98)");
    assert_eq!((opened.player_x, opened.player_y), (132, 98));
    assert!(opened.map_key.eq_ignore_ascii_case("prontera"));

    assert!(harness.press_escape());
    assert!(harness.view().open);
    harness.press_menu_bar();
    assert!(harness.view().open);

    let before = harness.view().zoom;
    assert!(harness.click_zoom_in());
    assert!(harness.view().zoom > before);

    harness.feed_mark(1, 4, 132, 98, (255, 16, 32));
    let marked = harness.view();
    assert!(
        marked
            .marks
            .iter()
            .any(|mark| mark.slot == 4 && mark.red == 255 && mark.green == 16 && mark.blue == 32),
        "viewpoint mark should be on the minimap"
    );
    harness.feed_mark(2, 4, 0, 0, (0, 0, 0));
    assert!(harness.view().marks.iter().all(|mark| mark.slot != 4));

    harness.feed_mark(1, 4, 132, 98, (1, 2, 3));
    harness.enter_map("geffen.gat", 10, 20);
    let warped = harness.view();
    assert!(warped.open);
    assert!(warped.marks.is_empty());
    assert!(warped.caption.contains("(10, 20)"));

    assert!(harness.press_hide_interface());
    assert!(harness.view().hidden);
    assert!(harness.view().open);
    assert!(harness.press_hide_interface());
    assert!(!harness.view().hidden);
    assert!(harness.view().open);
}

#[test]
fn minimap_is_open_on_test_with_the_current_map() {
    let Some(mut session) = live_session() else {
        return;
    };
    session.enter_world_for_minimap().expect("enter world");
    let view = session.minimap_view();
    assert!(view.open, "minimap opens after entering the world");
    assert!(!view.closable);
    assert!(!view.map_key.is_empty());
    let caption = view.caption.to_ascii_lowercase();
    let key = view.map_key.to_ascii_lowercase();
    assert!(
        caption.contains(&key) || view.title.to_ascii_lowercase().contains(&key),
        "caption {:?} should name {}",
        view.caption,
        view.map_key
    );
    assert_eq!(session.character_cell(), Some((view.player_x, view.player_y)));
}
