//! Shift+click item links (inventory review #11). The offline case drives the
//! injected Shift+click path and the public chat packet. The live case sends
//! one real TEST inventory row and checks the echo.

use korangar::e2e::{LiveConnect, LogicSession, offline_item_link};

#[test]
fn item_link_shift_click_roundtrip_offline() {
    let report = offline_item_link().unwrap();
    assert!(report.shift_links, "shift+click should link");
    assert!(report.plain_click_drags, "a click without shift should still drag");
    assert!(report.draft.contains(&report.payload));
    assert!(report.draft.contains("<ITEML>"));
    assert!(report.packet.windows(2).any(|header| header == [0xF3, 0x00]));
    assert!(
        report
            .packet
            .windows(report.payload.len())
            .any(|window| window == report.payload.as_bytes())
    );
    assert_eq!(report.label, "[+7 Knife [3]]");
    assert!(report.accent);
    assert_eq!(report.item_id, 1201);
    assert_eq!(report.refine, 7);
    assert_eq!(report.cards[0], 1);
    assert_eq!(report.details_title, "[+7 Knife [3]]");
    assert!(report.malformed_plain);
    assert_eq!(report.channel_labels, [
        "[+7 Knife [3]]",
        "[+7 Knife [3]]",
        "[+7 Knife [3]]",
        "[+7 Knife [3]]",
    ]);
}

#[test]
fn item_link_echo_on_test() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    session.item_link_on_test().unwrap();
    eprintln!("E2E_ITEM_LINK echo matched");
}
