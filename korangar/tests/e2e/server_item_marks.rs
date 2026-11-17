//! Lock and junk kept on the Hercules server (`0x0EF1` notice, `0x0EE0`
//! hello, `0x0EE1` set, `0x0EE8` list, `0x0EE9` result). The offline case
//! drives the client state with no socket. The live case locks a real TEST
//! row, logs in again, reads the lock back, has the map server refuse a
//! drop, and clears the lock.

use korangar::e2e::{LiveConnect, LogicSession, offline_server_item_marks};

#[test]
fn server_item_marks_offline() {
    let report = offline_server_item_marks();
    assert!(
        report.file_mark_ignored,
        "file marks must not apply once the server keeps marks"
    );
    assert!(report.listed_locked);
    assert_eq!(report.drop_refusal, "Knife is locked");
    assert_eq!(report.toggle_bytes, vec![0xE1, 0x0E, 0, 4, 0, 2, 2]);
    assert!(report.result_applied);
    assert!(report.fresh_row_cleared);
}

fn live_session() -> Option<LogicSession> {
    match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            None
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => Some(session),
    }
}

#[test]
fn server_item_marks_lock_survives_relog_on_test() {
    let Some(mut first) = live_session() else {
        return;
    };
    let row = first.server_marks_lock_on_test().unwrap();
    drop(first);
    eprintln!(
        "E2E_SERVER_MARKS locked index={} item={} result={} marks={}",
        row.index, row.item_id, row.result, row.marks
    );
    assert_eq!(row.result, 0);
    assert_ne!(row.marks & 0x01, 0);

    std::thread::sleep(std::time::Duration::from_secs(2));
    let Some(mut second) = live_session() else {
        panic!("TEST went away between sessions");
    };
    // Errors, without dropping, when the lock did not survive the relog.
    let report = second.server_marks_check_after_relog(&row).unwrap();
    eprintln!(
        "E2E_SERVER_MARKS relog listed={} present={} refusal={:?} removed={} cleared={}",
        report.listed_locked, report.row_present, report.refusal, report.removed_after_drop, report.cleared
    );
    assert!(report.row_present, "locked row moved between sessions");
    assert!(report.listed_locked, "lock did not come back after relog");
    assert!(report.refusal.ends_with("is locked."), "map server did not refuse the drop");
    assert!(!report.removed_after_drop);
    assert!(report.cleared);
}
