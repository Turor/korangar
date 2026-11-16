//! Rodex mail with attachments. The offline case drives the write form on
//! Hercules-shaped acks; the live case mails an item and 1 zeny to the TEST
//! character itself and takes them back.

use korangar::e2e::{LiveConnect, LogicSession, offline_mail_exchange};

#[test]
fn mail_attach_detach_and_send_offline() {
    let report = offline_mail_exchange().unwrap();
    assert_eq!(report.scripted, [
        "ComposeMailTo { receiver: \"bob\" }",
        "MailAttachItem { index: 3, amount: None }",
        "ConfirmMailAttachAmount",
        "MailDetachItem { index: 3 }",
        "SendMail",
    ]);
    assert!(report.asked_amount, "a stack of 5 asks for an amount first");
    assert_eq!(report.add_request, [0x04, 0x0A, 5, 0, 2, 0], "0x0A04 carries index + 2");
    assert_eq!(report.attached, [(3, 2)]);
    assert_eq!(report.summary, "Items 1/5, weight 14/2000");
    assert_eq!(report.fee_text, "Fee 2,520 z, total 3,520 z");
    assert_eq!(report.remove_request, [0x06, 0x0A, 5, 0, 2, 0]);
    assert_eq!(report.attached_after_remove, 0);
    assert_eq!(report.send_receiver, "Bob", "the send uses the name the server confirmed");
    assert_eq!(report.send_header, 0x0A6E);
    assert_eq!(report.send_zeny, 1000);
    assert!(report.closed_on_success);
    assert_eq!(report.read_attachments, [(501, 1), (1201, 1)]);
}

#[test]
fn mail_round_trip_on_test() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let trip = session.mail_round_trip().unwrap();
    eprintln!(
        "E2E_MAIL tries={:?} attached={:?} detached={:?} unequipped={:?} send={:?} with_item={} read_zeny={:?} items={:?} take_zeny={:?} \
         take_items={:?} deleted={}",
        trip.attach_tries,
        trip.attached,
        trip.detached,
        trip.unequipped,
        trip.send_result,
        trip.sent_with_item,
        trip.read_zeny,
        trip.read_items,
        trip.take_zeny_result,
        trip.take_items_result,
        trip.deleted
    );
    assert!(trip.write_allowed);
    assert!(trip.receiver_id != 0);
    assert!(trip.receiver_name.eq_ignore_ascii_case(&trip.own_name));
    if let Some((_, item_id)) = trip.attached {
        assert!(matches!(trip.detached, Some((true, _))), "taking the item off failed");
        assert!(!trip.attach_tries.iter().any(|(_, id, result)| *id == item_id && *result != 0));
    }
    // Kafra tickets are nomail on TEST.
    assert!(
        trip.attach_tries
            .iter()
            .filter(|(_, id, _)| (7060..=7061).contains(id))
            .all(|(_, _, result)| *result == 4)
    );
    assert_eq!(trip.send_result, Some(0), "the self mail was refused");
    assert_eq!(trip.read_zeny, Some(1));
    if trip.sent_with_item {
        let (_, item_id) = trip.attached.unwrap();
        assert_eq!(trip.read_items, [(item_id, 1)]);
        assert_eq!(trip.take_items_result, Some(0));
    }
    assert_eq!(trip.take_zeny_result, Some(0));
    assert!(trip.deleted);
    if trip.unequipped.is_some() {
        assert!(trip.reequipped, "the gear taken off for the mail was not put back on");
    }
}
