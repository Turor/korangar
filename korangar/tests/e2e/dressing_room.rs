//! Dressing room. Offline Ctrl+clicks stack view sprites and send nothing.
//! The live case tries on every headgear the TEST character owns.

use korangar::e2e::{LiveConnect, LogicSession, no_preview_message, offline_dressing_room};

#[test]
fn dressing_room_stacks_headgear_and_sends_nothing() {
    let report = offline_dressing_room();
    assert_eq!(report.top_preview_id, 8);
    assert!(report.top_path.contains("악세사리"), "{}", report.top_path);
    assert!(report.top_path.contains("_고글"), "{}", report.top_path);
    assert_eq!(report.costume_preview_id, 5);
    assert!(report.costume_path.ends_with("여_리본"), "{}", report.costume_path);
    assert_eq!(report.top_kept_after_costume, 8);
    assert_eq!(report.potion_notice, no_preview_message());
    assert_eq!(report.top_after_potion, 8);
    assert_eq!(report.direction_before_reset, 2);
    assert_eq!(report.reset_preview_id, 0);
    assert_eq!(report.reset_shown_id, 2);
    assert_eq!(report.direction_after_reset, 0);
    assert!(report.outbound_empty);
    assert!(report.ctrl_inventory);
    assert!(report.ctrl_storage);
    assert!(report.ctrl_cart);
    assert!(!report.plain_click);
    assert!(!report.ctrl_equipment);
}

#[test]
fn dressing_room_on_test_headgear() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let report = session.dressing_room_on_test().unwrap();
    if report.note == "no headgear on TEST character" {
        eprintln!("no headgear on TEST character");
        return;
    }
    eprintln!(
        "E2E_DRESSING checked={} resolved={} missing={:?}",
        report.checked,
        report.resolved.len(),
        report.missing_view_ids
    );
    assert!(report.checked > 0);
    assert_eq!(report.checked, report.resolved.len() + report.missing_view_ids.len());
    assert!(report.resolved.iter().all(|path| path.contains(':')));
    assert!(report.resolved.iter().all(|path| !path.ends_with(':')));
}
