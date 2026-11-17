//! Inventory grid view (inventory review #2, #3, #7, #9, #12): tabs that hide
//! worn gear, the favorites tab and its packets, display sort, search dimming
//! with card names, and the weight bar marks. The live case splits the TEST
//! character's real inventory and round-trips one favorite flag.

use korangar::e2e::{InventorySort, InventoryTab, LiveConnect, LogicSession, offline_inventory_view};
use ragnarok_packets::InventoryIndex;

#[test]
fn inventory_view_tabs_sort_search_and_favorites_offline() {
    let report = offline_inventory_view().unwrap();

    // Worn Knife (3) is hidden; equipped arrows (5) stay under Ammo; the
    // favorited Poring Card (7) moves to Favorites only, so Cards is empty.
    assert_eq!(report.tabs, [
        (InventoryTab::All, vec![2, 4, 5, 6, 7]),
        (InventoryTab::Usable, vec![2]),
        (InventoryTab::Equip, vec![4]),
        (InventoryTab::Ammo, vec![5]),
        (InventoryTab::Etc, vec![6]),
        (InventoryTab::Cards, vec![]),
        (InventoryTab::Favorites, vec![7]),
    ]);

    assert_eq!(report.sorts, [
        // Type follows the tab order: usable, equip, ammo, etc, cards.
        (InventorySort::Type, vec![2, 4, 5, 6, 7]),
        (InventorySort::Name, vec![5, 6, 4, 7, 2]),
        // Stack weight: potions 210.0, arrows 50.0, knife 40.0, jellopy 12.0, card 1.0.
        (InventorySort::Weight, vec![2, 5, 4, 6, 7]),
        (InventorySort::Newest, vec![7, 6, 5, 4, 2]),
        (InventorySort::Amount, vec![5, 2, 6, 4, 7]),
    ]);

    // A-Z, Recent and Zeny, then each reversed by a second click. Zeny is
    // sell price x amount: knife 250, potions 150, jellopy 36, card 10; the
    // arrows have no price and stay last both ways.
    assert_eq!(report.buttons, [
        (InventorySort::Name, false, vec![5, 6, 4, 7, 2]),
        (InventorySort::Name, true, vec![2, 7, 4, 6, 5]),
        (InventorySort::Newest, false, vec![7, 6, 5, 4, 2]),
        (InventorySort::Newest, true, vec![2, 4, 5, 6, 7]),
        (InventorySort::Zeny, false, vec![4, 2, 6, 7, 5]),
        (InventorySort::Zeny, true, vec![7, 6, 2, 4, 5]),
    ]);

    // "poring" hits the card and the Knife holding a Poring Card.
    assert_eq!(report.poring_dimmed, [2, 5, 6]);

    assert_eq!(report.favorite_acks, [(InventoryIndex(7), true)]);
    assert_eq!(report.favorite_request, [0x07, 0x09, 8, 0, 0]);
    assert_eq!(report.drops, (Some(true), Some(false)));

    assert_eq!(report.weight_marks, [0.5, 0.9]);
    assert!((report.weight_fill - 0.9).abs() < 1e-6);
}

#[test]
fn inventory_view_splits_the_test_inventory_live() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let report = session.inventory_view_on_test().unwrap();
    eprintln!(
        "E2E_INVENTORY_VIEW rows={} worn={} tabs={:?} toggled={:?} set={:?} revert={:?}",
        report.inventory_rows, report.worn, report.tab_counts, report.toggled, report.set_ack, report.revert_ack
    );
    assert!(report.inventory_rows > 0, "the TEST character's inventory never arrived");
    let [all, usable, equip, ammo, etc, cards, favorites] = report.tab_counts;
    assert_eq!(all, report.inventory_rows - report.worn, "All hides only worn gear");
    assert_eq!(
        usable + equip + ammo + etc + cards + favorites,
        all,
        "every unworn stack is in exactly one tab"
    );
    assert!(report.toggled.is_some());
    assert!(report.set_ack.is_some(), "TEST never answered CZ_INVENTORY_TAB");
    assert_eq!(
        report.set_ack.map(|favorite| !favorite),
        report.revert_ack,
        "favorite flag was restored"
    );
}
