//! Storage tabs, saved searches and deposit matching (inventory review #14,
//! #18). The offline case runs the storage tabs, deposit plans, throttle and
//! saved-search file; the live case plans a deposit on the TEST character's
//! real inventory without sending it.

use korangar::e2e::{InventoryTab, LiveConnect, LogicSession, offline_storage_tools};

#[test]
fn storage_tabs_saved_searches_and_deposit_offline() {
    let scratch = std::env::temp_dir().join(format!("korangar-e2e-storage-{}", std::process::id()));
    let report = offline_storage_tools(&scratch).unwrap();
    let _ = std::fs::remove_dir_all(&scratch);

    assert_eq!(report.storage_tabs, [
        (InventoryTab::All, vec![0, 1, 2]),
        (InventoryTab::Usable, vec![2]),
        (InventoryTab::Equip, vec![1]),
        (InventoryTab::Etc, vec![0]),
    ]);

    // Worn knife 3, equipped arrows 5 and favorite card 7 never move.
    assert_eq!(report.plans, [
        (InventoryTab::All, "", vec![(2, 30), (4, 1), (6, 12), (8, 4)]),
        (InventoryTab::Usable, "", vec![(2, 30), (8, 4)]),
        (InventoryTab::Etc, "", vec![(6, 12)]),
        // The knife holds a Poring Card; the card itself is a favorite.
        (InventoryTab::All, "poring", vec![(4, 1)]),
    ]);

    assert_eq!(report.throttled, [Some(2), None, Some(6)]);
    assert_eq!(report.deposit_status, "Deposited 2 stacks");

    assert_eq!(report.alice, ["card", "ore"]);
    assert!(report.bob.is_empty());
    assert!(report.save_refused_duplicate);
}

#[test]
fn storage_deposit_plan_on_test_character() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let report = session.storage_deposit_plan_on_test().unwrap();
    eprintln!(
        "E2E_STORAGE_PLAN rows={} protected={} planned={}",
        report.inventory_rows, report.protected, report.planned
    );
    assert!(report.inventory_rows > 0, "the TEST character's inventory never arrived");
    assert_eq!(report.planned, report.inventory_rows - report.protected);
}
