//! Storage, cart and trade amount prompt. The offline case runs every drop
//! target through the prompt and checks the wire request; the live case runs
//! the prompt on the TEST character's real inventory without moving anything.

use korangar::e2e::{ItemMoveStep, ItemMoveTarget, LiveConnect, LogicSession, offline_item_moves};

#[test]
fn item_move_prompt_and_requests_offline() {
    let report = offline_item_moves().unwrap();
    assert_eq!(report.scripted, ["ConfirmItemMove", "CancelItemMove"]);

    let expected = [
        (ItemMoveTarget::ToStorage, "Store how many Red Potion? (1-30)", vec![
            0x64, 0x03, 11, 0, 12, 0, 0, 0,
        ]),
        (
            ItemMoveTarget::FromStorage,
            "Take how many Red Potion out of storage? (1-30)",
            vec![0x65, 0x03, 11, 0, 12, 0, 0, 0],
        ),
        (ItemMoveTarget::ToCart, "Put how many Red Potion in the cart? (1-30)", vec![
            0x26, 0x01, 11, 0, 12, 0, 0, 0,
        ]),
        (
            ItemMoveTarget::FromCart,
            "Take how many Red Potion from the cart? (1-30)",
            vec![0x27, 0x01, 11, 0, 12, 0, 0, 0],
        ),
        (ItemMoveTarget::ToTrade, "Trade how many Red Potion? (1-30)", vec![
            0xE8, 0x00, 11, 0, 12, 0, 0, 0,
        ]),
    ];
    assert_eq!(report.cases.len(), expected.len());
    for (case, (target, prompt, request)) in report.cases.iter().zip(expected) {
        assert_eq!(case.target, target);
        assert_eq!(case.prompt, prompt);
        assert_eq!(case.default_amount, "30", "{target:?} starts at the whole stack");
        assert_eq!(case.rejected, "Enter an amount from 1 to 30.");
        assert_eq!(case.request, request, "{target:?}: index 9 goes out as 11, amount 12");
    }

    assert_eq!(report.single, ItemMoveStep::Send {
        target: ItemMoveTarget::ToStorage,
        index: 4,
        amount: 1,
    });
    assert_eq!(report.single_request, [0x64, 0x03, 6, 0, 1, 0, 0, 0]);
    assert!(report.cancelled_cleanly);
}

#[test]
fn item_move_prompt_on_test_inventory() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let report = session.item_move_prompts_on_test().unwrap();
    eprintln!(
        "E2E_ITEM_MOVE rows={} stack={:?} default={:?} prompts={:?} single={:?}",
        report.inventory_rows, report.stack, report.stack_default, report.stack_prompts, report.single
    );
    assert!(report.inventory_rows > 0, "the TEST character's inventory never arrived");
    if let Some((_, item_id, amount)) = report.stack {
        assert_eq!(report.stack_default, amount.to_string());
        assert_eq!(report.stack_prompts.len(), 3);
        assert!(report.stack_prompts[0].ends_with(&format!("(1-{amount})")));
        assert!(report.stack_prompts[0].contains(&format!("item {item_id}")));
        assert!(report.stack_full_confirms);
        assert!(report.stack_over_refused);
    }
    if let Some((index, step)) = report.single {
        assert_eq!(step, ItemMoveStep::Send {
            target: ItemMoveTarget::ToStorage,
            index,
            amount: 1,
        });
    }
    assert!(report.stack.is_some() || report.single.is_some());
}
