//! HUD status icons. Offline cases drive status packets through the same
//! `StatusEffects` calls as the client and parse fixture `stateicon` tables.
//! `KORANGAR_E2E_STATEICON_DIR` points at real client `stateicon/*.lub`
//! files. The live case reads the statuses Hercules TEST sends on map entry.

use std::path::PathBuf;
use std::time::Duration;

use korangar::e2e::{LiveConnect, LogicSession, Phase, StatusStrip};
use korangar_networking::StatusChangeState;

const PLAYER: u32 = 2_000_051;
const SOMEONE_ELSE: u32 = 2_000_052;

const EFST_IDS: &[u8] = b"EFST_IDs = { EFST_PROVOKE = 0, EFST_BLESSING = 10, EFST_INC_AGI = 12, EFST_RIDING = 27 }";

const IMAGE_INFO: &[u8] = br#"
PRIORITY_WHITE = 4
StateIconImgList = {
    [PRIORITY_WHITE] = {
        [EFST_IDs.EFST_BLESSING] = "BLESSING.TGA",
        [EFST_IDs.EFST_INC_AGI] = "INC_AGI.TGA",
        [EFST_IDs.EFST_FROM_A_NEWER_CLIENT] = "NEWER.TGA",
    },
}
"#;

const ICON_INFO: &[u8] = br#"
COLOR_TITLE_BUFF = { 155, 202, 155 }
COLOR_TIME = { 255, 176, 98 }
StateIconList = {}
StateIconList[EFST_IDs.EFST_BLESSING] = {
    haveTimeLimit = 1,
    posTimeLimitStr = 2,
    descript = {
        { "Blessing", COLOR_TITLE_BUFF },
        { "%s", COLOR_TIME },
        { "Increase DEX, INT, STR" },
        { "Increases HIT" },
    },
}
"#;

fn strip() -> StatusStrip {
    let mut strip = StatusStrip::with_tables(EFST_IDS, Some(IMAGE_INFO), Some(ICON_INFO)).unwrap();
    strip.set_local_entity(PLAYER);
    strip
}

#[test]
fn status_icons_show_art_tooltip_and_countdown() {
    let mut strip = strip();
    strip.on_status_change(PLAYER, 10, StatusChangeState::Active, 120_000, 120_000, 0);
    strip.on_status_change(SOMEONE_ELSE, 12, StatusChangeState::Active, 60_000, 60_000, 0);
    strip.on_status_change(PLAYER, 46, StatusChangeState::Active, 0, 0, 0);
    assert_eq!(strip.local_packets(), 2);

    let view = strip.view(0);
    assert_eq!(view.iter().map(|icon| icon.status_index).collect::<Vec<_>>(), vec![10, 46]);
    let blessing = &view[0];
    assert_eq!(blessing.icon_file.as_deref(), Some("BLESSING.TGA"));
    assert_eq!(blessing.name, "Blessing");
    assert_eq!(blessing.tooltip, "Blessing\nIncrease DEX, INT, STR\nIncreases HIT");
    assert_eq!(blessing.time, "2m");
    let sweep = blessing.sweep.unwrap();
    assert!(!sweep.urgent && sweep.fraction.abs() < 0.001);

    // A flag status without art anywhere keeps the letter chip and has no
    // countdown.
    let postdelay = &view[1];
    assert_eq!(postdelay.icon_file, None);
    assert_eq!(postdelay.name, "Postdelay");
    assert!(postdelay.tooltip.starts_with("Postdelay"));
    assert_eq!(postdelay.time, "");
    assert_eq!(postdelay.sweep, None);

    // The last minute is urgent and the sweep covers the elapsed share.
    let later = strip.view(90_000);
    assert_eq!(later[0].time, "30s");
    let sweep = later[0].sweep.unwrap();
    assert!(sweep.urgent && (sweep.fraction - 0.5).abs() < 0.001);

    // A refresh restarts the countdown and keeps the resolved icon.
    strip.on_status_change(PLAYER, 10, StatusChangeState::StillActive, 240_000, 240_000, 90_000);
    let refreshed = strip.view(90_000);
    assert_eq!(refreshed[0].time, "4m");
    assert_eq!(refreshed[0].icon_file.as_deref(), Some("BLESSING.TGA"));
}

#[test]
fn status_icons_end_and_expire() {
    let mut strip = strip();
    strip.on_status_change(PLAYER, 10, StatusChangeState::Active, 5_000, 5_000, 0);
    strip.on_status_change(PLAYER, 12, StatusChangeState::Active, 60_000, 60_000, 0);
    strip.on_status_change(PLAYER, 27, StatusChangeState::Active, 0, 0, 0);
    strip.on_status_change(PLAYER, 12, StatusChangeState::Ended, 0, 0, 1_000);
    assert_eq!(
        strip.view(1_000).iter().map(|icon| icon.status_index).collect::<Vec<_>>(),
        vec![10, 27]
    );
    // No end packet for Blessing: the timer runs out on the client.
    strip.expire(5_000);
    assert_eq!(
        strip.view(5_000).iter().map(|icon| icon.status_index).collect::<Vec<_>>(),
        vec![27]
    );
    // Another player's end packet does not touch ours.
    strip.on_status_change(SOMEONE_ELSE, 27, StatusChangeState::Ended, 0, 0, 6_000);
    assert_eq!(strip.view(6_000).len(), 1);
}

#[test]
fn status_icon_tables_survive_unknown_and_broken_entries() {
    let strip = StatusStrip::with_tables(EFST_IDS, Some(IMAGE_INFO), Some(b"StateIconList = nil + 1")).unwrap();
    // Images still load when the description chunk fails, and the newer EFST
    // name does not drop the whole image table.
    assert_eq!(strip.table()[&10].icon.as_deref(), Some("BLESSING.TGA"));
    assert_eq!(strip.table()[&12].icon.as_deref(), Some("INC_AGI.TGA"));
    assert_eq!(strip.table()[&10].title, None);
    // Riding is not in the fixture lub; the built-in table has its art.
    assert_eq!(strip.table()[&27].icon.as_deref(), Some("라이딩.tga"));
    assert!(StatusStrip::with_tables(b"EFST_IDs = {", None, None).is_err());
}

/// Real client tables: `KORANGAR_E2E_STATEICON_DIR` holds `efstids.lub`,
/// `stateiconimginfo.lub` and `stateiconinfo.lub` (bytecode or source).
#[test]
fn real_client_stateicon_tables_when_present() {
    let Some(directory) = std::env::var_os("KORANGAR_E2E_STATEICON_DIR").map(PathBuf::from) else {
        eprintln!("E2E_SKIP set KORANGAR_E2E_STATEICON_DIR to parse real stateicon tables");
        return;
    };
    let read = |name: &str| std::fs::read(directory.join(name)).ok();
    let efst_ids = read("efstids.lub").expect("efstids.lub");
    let strip = StatusStrip::with_tables(
        &efst_ids,
        read("stateiconimginfo.lub").as_deref(),
        read("stateiconinfo.lub").as_deref(),
    )
    .unwrap();
    let table = strip.table();
    let with_icons = table.values().filter(|info| info.icon.is_some()).count();
    let with_titles = table.values().filter(|info| info.title.is_some()).count();
    eprintln!("E2E_STATEICON entries={} icons={with_icons} titles={with_titles}", table.len());
    assert!(with_icons > 700, "only {with_icons} icons");
    assert!(with_titles > 100, "only {with_titles} titles");
    // Blessing's art is not in kRO's lub; the built-in table supplies it.
    let blessing = &table[&10];
    assert_eq!(blessing.icon.as_deref(), Some("블레싱.tga"));
    assert_eq!(blessing.title.as_deref(), Some("Blessing"));
    assert!(blessing.has_time_limit);
}

#[test]
fn live_status_icons_on_map_entry() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let tick = session
        .enter_world_and_watch_statuses(Duration::from_secs(6))
        .expect("enter TEST and read statuses");
    assert_eq!(session.phase(), Phase::InWorld);
    let strip = session.status_strip();
    let view = strip.view(tick);
    let summary: Vec<String> = view
        .iter()
        .map(|icon| format!("{}:{}:{}", icon.status_index, icon.name, icon.time))
        .collect();
    eprintln!(
        "E2E_LIVE status_packets={} icons={} {:?}",
        strip.local_packets(),
        view.len(),
        summary
    );
    for icon in &view {
        assert!(!icon.name.is_empty());
        assert!(icon.tooltip.starts_with(&icon.name));
        assert_eq!(icon.sweep.is_some(), !icon.time.is_empty());
    }
}
