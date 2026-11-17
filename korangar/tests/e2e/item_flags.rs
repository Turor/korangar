//! New-item glow, Sell Junk, and item lock. The offline case runs without a
//! display. The live case marks one real TEST inventory row and refuses the
//! ground drop without sending it.

use korangar::e2e::{LiveConnect, LogicSession, offline_item_flag_menu, offline_item_flags};

#[test]
fn item_flags_glow_junk_and_lock_offline() {
    let report = offline_item_flags().unwrap();
    assert!(report.scripted.iter().any(|name| name.contains("ToggleHoveredJunk")));
    assert!(report.scripted.iter().any(|name| name.contains("ToggleHoveredLock")));
    assert!(report.scripted.iter().any(|name| name.contains("SellJunk")));
    assert!(report.scripted.iter().any(|name| name.contains("ConfirmItemDrop")));
    assert!(report.scripted.iter().any(|name| name.contains("TradeAddItem")));
    assert!(report.scripted.iter().any(|name| name.contains("MailAttachItem")));

    assert_eq!(report.snapshot_glow, 0);
    assert!(report.pickup_glow > 0);
    assert_eq!(report.faded_glow, 0);
    assert_eq!(report.sell_cart, vec![(1, 10), (2, 1)]);
    assert!(report.refused_outbound.is_empty());
    assert!(report.messages.iter().any(|message| message == "Knife is locked"));
    assert!(report.messages.iter().all(|message| !message.contains("Armor")));
    assert_eq!(report.storage_packet, vec![0x64, 0x03, 4, 0, 1, 0, 0, 0]);
    assert!(report.followed_locked);
    assert!(report.reloaded_junk);
    assert!(report.reloaded_locked);
    assert!(
        !report.reloaded_new,
        "the new-item glow is not saved, so the login list never glows"
    );
    assert!(report.other_character_kept);
}

#[test]
fn item_flags_junk_and_lock_from_the_item_menu() {
    let report = offline_item_flag_menu();
    assert!(
        report.before.ends_with(&["Mark junk".to_owned(), "Lock".to_owned()]),
        "{:?}",
        report.before
    );
    assert!(report.before.iter().any(|label| label == "Move to trade"));
    assert!(report.junk && report.locked);
    // Locked: Not junk / Unlock, and no trade row.
    assert!(
        report.after.ends_with(&["Not junk".to_owned(), "Unlock".to_owned()]),
        "{:?}",
        report.after
    );
    assert!(!report.after.iter().any(|label| label == "Move to trade"));
    assert_eq!(report.locked_quick_move, "hint: That item is locked.");
}

#[test]
fn item_flags_lock_on_test_inventory() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let report = session.item_flags_on_test().unwrap();
    eprintln!(
        "E2E_ITEM_FLAGS rows={} item={} refusal={} sent={}/{} cleared={}",
        report.inventory_rows, report.item_id, report.refusal, report.sent_before, report.sent_after, report.cleared
    );
    assert!(report.inventory_rows > 0, "inventory never arrived");
    assert!(report.refusal.ends_with("is locked"));
    assert_eq!(report.sent_before, report.sent_after);
    assert!(report.cleared);
}
