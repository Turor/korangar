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

    // Worn knife 3 and favorite card 7 never move. Arrows 5 carry the ammo
    // slot as WearState like every ammo row; they stay only while
    // ZC_EQUIP_ARROW named their index.
    assert_eq!(report.plans, [
        (InventoryTab::All, "", None, vec![2, 4, 5, 6, 8]),
        (InventoryTab::All, "", Some(5), vec![2, 4, 6, 8]),
        (InventoryTab::Usable, "", None, vec![2, 8]),
        (InventoryTab::Etc, "", None, vec![5, 6]),
        (InventoryTab::Etc, "", Some(5), vec![6]),
        // The knife holds a Poring Card; the card itself is a favorite.
        (InventoryTab::All, "poring", None, vec![4]),
        (InventoryTab::Favorites, "", None, vec![]),
    ]);

    assert_eq!(report.throttled, [Some(2), None, Some(6)]);
    // Checked against the live row at send time: current amount, then not
    // at all once it is a favorite or gone.
    assert_eq!(report.send_time, [Some(12), Some(10), None, None]);
    assert_eq!(report.ammo_tracking, (Some(5), None));
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
        "E2E_STORAGE_PLAN rows={} worn={} ammo={:?} protected={} planned={}",
        report.inventory_rows, report.worn, report.equipped_ammo, report.protected, report.planned
    );
    assert!(report.inventory_rows > 0, "the TEST character's inventory never arrived");
    assert_eq!(report.planned, report.inventory_rows - report.protected);
    assert!(report.protected >= report.worn, "worn gear is always protected");
}
