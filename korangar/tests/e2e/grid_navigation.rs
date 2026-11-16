//! Keyboard / controller grid navigation (inventory review #17). The offline
//! case runs the cursor model, the Enter target under a sort, clamping and
//! the scroll reveal; the live case runs the cursor over the TEST
//! character's real inventory without using anything.

use korangar::e2e::{LiveConnect, LogicSession, offline_grid_navigation};

#[test]
fn grid_navigation_offline() {
    let report = offline_grid_navigation().unwrap();

    // Right, Right, Down, Down, Down (no row below 21 reaches col 1 of a
    // 25-stack grid, and 21 is already on the last row), Left, Up, End,
    // Home, Up (stops at the top).
    assert_eq!(report.steps, [
        Some(0),
        Some(1),
        Some(11),
        Some(21),
        Some(21),
        Some(20),
        Some(10),
        Some(24),
        Some(0),
        Some(0)
    ]);
    // Name sort draws Apple 8, Jellopy 6, Red Potion 2: Enter uses the
    // drawn stack's server index, not the position.
    assert_eq!(report.enter_indexes, [Some(2), Some(8)]);
    assert_eq!(report.focused, [false, true, false]);
    // Red Potion used up under the cursor: it moves to the new last stack.
    assert_eq!(report.after_use, Some(1));
    assert_eq!(report.after_tab, None);
    assert_eq!(report.reveal, [Some((0.0, 0.1)), Some((0.3, 0.4))]);
}

#[test]
fn grid_navigation_on_test_character() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let report = session.grid_navigation_on_test().unwrap();
    eprintln!(
        "E2E_GRID_NAV rows={} end={:?} home={:?}",
        report.inventory_rows, report.end_index, report.home_index
    );
    assert!(report.inventory_rows > 0, "the TEST character's inventory never arrived");
    assert_eq!(report.end_index, report.last_drawn);
    assert_eq!(report.home_index, report.first_drawn);
}
