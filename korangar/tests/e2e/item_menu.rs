//! Right-click item menu and Alt+right-click quick-move. The offline case
//! checks injected input, the amount prompt, and the request bytes. The live
//! case builds the menu from the TEST character's inventory and sends nothing.

use korangar::e2e::{LiveConnect, LogicSession, offline_item_menu};

const QUICK_MOVE_NO_TARGET: &str = "Open storage, cart, trade or mail to quick-move";

#[test]
fn item_menu_and_quick_move_offline() {
    let report = offline_item_menu().unwrap();
    assert!(report.injected_alt_right, "Alt+right-click did not classify as quick-move");
    assert!(report.injected_right, "right-click did not classify as the menu");
    assert_eq!(report.scripted, [
        "ItemMenuPick { index: 0 }",
        "ItemMenuPick { index: 1 }",
        "DismissItemMenu",
        "ConfirmItemMove",
    ]);

    assert!(report.stack_prompt.starts_with("Store how many Red Potion? (1-30)"));
    assert_eq!(report.stack_default, "30");
    assert_eq!(report.stack_request, [0x64, 0x03, 0x0B, 0x00, 0x1E, 0x00, 0x00, 0x00]);
    assert_eq!(report.knife_request, [0x64, 0x03, 6, 0, 1, 0, 0, 0]);
    assert_eq!(report.gear_request, [0x64, 0x03, 5, 0, 1, 0, 0, 0]);
    assert_eq!(report.cart_request, [0x26, 0x01, 6, 0, 1, 0, 0, 0]);
    assert_eq!(report.no_target_hint, QUICK_MOVE_NO_TARGET);
    assert!(report.no_target_request.is_empty());

    // Junk and Lock come last (item flags, G58).
    assert_eq!(report.menu_labels, [
        "Use",
        "Details",
        "Link to chat",
        "Favorite",
        "Drop",
        "Mark junk",
        "Lock"
    ]);
    assert_eq!(report.use_request, [0x39, 0x04, 4, 0, 0, 0, 0, 0]);
    assert!(report.details_opened);
    assert!(report.details_body.contains("Red Potion"));
    assert!(
        report.details_body.contains("Weight: 7"),
        "details body missed the item-finder line: {}",
        report.details_body
    );
    assert_eq!(report.keyboard_pick, 1);
    assert!(report.link_queued, "Link to chat did not queue LinkItemInChat");
}

#[test]
fn item_menu_on_test_inventory() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let report = session.item_menu_on_test().unwrap();
    eprintln!(
        "E2E_ITEM_MENU rows={} labels={:?} hint={} sent={}/{}",
        report.inventory_rows, report.labels, report.quick_move_hint, report.sent_before, report.sent_after
    );
    assert!(report.inventory_rows > 0, "the TEST character's inventory never arrived");
    assert!(report.has_details, "menu did not include Details: {:?}", report.labels);
    assert!(
        report.labels.iter().any(|label| label == "Link to chat"),
        "menu hid Link to chat: {:?}",
        report.labels
    );
    assert!(
        !report
            .labels
            .iter()
            .any(|label| label == "Junk" || label == "Lock" || label == "Unlock")
    );
    assert_eq!(report.quick_move_hint, QUICK_MOVE_NO_TARGET);
    assert_eq!(report.sent_before, report.sent_after);
}
