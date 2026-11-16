//! World headgear. Offline cases run appear packets and `ZC_SPRITE_CHANGE`
//! looks through the client's look state and part-file builder with a
//! fixture `accname` table. The live case reads the looks Hercules TEST sends
//! for the pool account's characters and for players on the map.

use std::time::Duration;

use korangar::e2e::{HeadgearScene, LiveConnect, LogicSession, Phase};
use korangar_networking::{EntityData, NetworkEvent};
use ragnarok_packets::{AccountId, Direction, EntityAppearPacket, EntityId, JobId, Sex, WorldPosition};

const ACCESSORY_IDS: &[u8] = b"ACCESSORY_IDs = { ACCESSORY_GOGGLE = 1, ACCESSORY_RIBBON = 17, ACCESSORY_GLASSES = 12 }";
// `_고글` in CP949, like the shipped table.
const ACCNAME: &[u8] = br#"AccNameTable = {
    [ACCESSORY_IDs.ACCESSORY_GOGGLE] = "_\176\237\177\219",
    [ACCESSORY_IDs.ACCESSORY_RIBBON] = "_ribbon",
    [ACCESSORY_IDs.ACCESSORY_GLASSES] = "_glasses",
}"#;

const SWORDSMAN: u32 = 2_000_101;
const ACOLYTE: u32 = 2_000_102;
const PORING: u32 = 110_001;

fn appear(entity_id: u32, job: u16, sex: Sex, head: u16, [bottom, top, mid]: [u16; 3]) -> NetworkEvent {
    let packet = EntityAppearPacket {
        object_type: 0,
        entity_id: EntityId(entity_id),
        group_id: 0,
        movement_speed: 150,
        body_state: 0,
        health_state: 0,
        effect_state: 0,
        job_id: JobId(job),
        head,
        weapon: 0,
        shield: 0,
        accessory: bottom,
        accessory2: top,
        accessory3: mid,
        head_palette: 0,
        body_palette: 0,
        head_direction: 0,
        robe: 0,
        guild_id: 0,
        emblem_version: 0,
        honor: 0,
        virtue: 0,
        is_pk_mode_on: 0,
        sex,
        position: WorldPosition {
            x: 150,
            y: 150,
            direction: Direction::South,
        },
        x_size: 0,
        y_size: 0,
        c_level: 1,
        font: 0,
        maximum_health_points: 100,
        health_points: 100,
        is_boss: 0,
        body: 0,
        name: String::new(),
    };
    NetworkEvent::AddEntity {
        entity_data: EntityData::from(packet),
    }
}

fn scene() -> HeadgearScene {
    HeadgearScene::with_tables(ACCESSORY_IDS, ACCNAME).expect("fixture accname tables")
}

#[test]
fn headgear_from_appear_packets_follows_the_head_in_draw_order() {
    let mut scene = scene();
    assert_eq!(scene.accessory_names().get(&1).map(String::as_str), Some("_고글"));

    // Swordsman wearing goggles (top), glasses (mid) and nothing on the
    // bottom slot; an acolyte with a ribbon that also covers mid; a poring.
    scene.observe(&appear(SWORDSMAN, 1, Sex::Male, 5, [0, 1, 12]));
    scene.observe(&appear(ACOLYTE, 4, Sex::Female, 2, [0, 17, 17]));
    scene.observe(&appear(PORING, 1002, Sex::Male, 0, [0, 0, 0]));
    assert_eq!(scene.players().len(), 2, "monsters carry no look");

    let swordsman = scene.part_files(SWORDSMAN).unwrap();
    assert_eq!(swordsman.len(), 4);
    assert_eq!(swordsman[0], "인간족\\몸통\\남\\검사_남");
    assert_eq!(
        swordsman[1], "인간족\\머리통\\남\\5_남",
        "other players wear their own hair style"
    );
    // roBrowser order after the head: middle, then top.
    assert!(swordsman[2].starts_with("?악세사리\\남\\남_glasses|"));
    assert!(swordsman[3].starts_with("?악세사리\\남\\남_고글|"));

    let acolyte = scene.part_files(ACOLYTE).unwrap();
    assert_eq!(acolyte.len(), 3, "a top+mid ribbon is drawn once");
    assert!(acolyte[2].starts_with("?악세사리\\여\\여_ribbon|"));

    // The loader keeps only headgear whose sprite files exist and never draws
    // the fallback sprite for a missing hat.
    let composed = scene
        .composed_parts(SWORDSMAN, |path| {
            !path.ends_with("_glasses") && !path.ends_with("_12") && !path.ends_with("남12")
        })
        .unwrap();
    assert_eq!(composed, vec![
        "인간족\\몸통\\남\\검사_남".to_owned(),
        "인간족\\머리통\\남\\5_남".to_owned(),
        "악세사리\\남\\남_고글".to_owned(),
    ]);
}

#[test]
fn equipping_and_removing_headgear_reloads_that_player_only() {
    let mut scene = scene();
    scene.observe(&appear(SWORDSMAN, 1, Sex::Male, 5, [0, 0, 0]));
    scene.observe(&appear(ACOLYTE, 4, Sex::Female, 2, [0, 0, 0]));
    assert_eq!(scene.part_files(SWORDSMAN).unwrap().len(), 2);

    let account = AccountId(SWORDSMAN);
    scene.observe(&NetworkEvent::ChangeHeadTop {
        account_id: account,
        view_id: 1,
    });
    // The same look again (Hercules resends on refresh) does not reload.
    scene.observe(&NetworkEvent::ChangeHeadTop {
        account_id: account,
        view_id: 1,
    });
    scene.observe(&NetworkEvent::ChangeHeadBottom {
        account_id: account,
        view_id: 12,
    });
    // Colors never reload sprites.
    scene.observe(&NetworkEvent::ChangeHairColor {
        account_id: account,
        color: 3,
    });
    // A look for someone who already left the screen is ignored, not a panic.
    scene.observe(&NetworkEvent::ChangeHeadTop {
        account_id: AccountId(9_999_999),
        view_id: 1,
    });
    assert_eq!(scene.reloads(), &[SWORDSMAN, SWORDSMAN]);
    let files = scene.part_files(SWORDSMAN).unwrap();
    assert_eq!(files.len(), 4);
    assert!(files[2].starts_with("?악세사리\\남\\남_고글|"), "top before bottom");
    assert!(files[3].starts_with("?악세사리\\남\\남_glasses|"));

    scene.observe(&NetworkEvent::ChangeHeadTop {
        account_id: account,
        view_id: 0,
    });
    scene.observe(&NetworkEvent::ChangeHeadBottom {
        account_id: account,
        view_id: 0,
    });
    assert_eq!(
        scene.part_files(SWORDSMAN).unwrap().len(),
        2,
        "unequipped hats leave the sprite"
    );
    assert_eq!(scene.reloads().len(), 4);
    assert_eq!(scene.part_files(ACOLYTE).unwrap().len(), 2);
}

#[test]
fn live_headgear_looks_from_test() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    session
        .enter_world_and_watch_statuses(Duration::from_secs(5))
        .expect("enter TEST and read appear packets");
    assert_eq!(session.phase(), Phase::InWorld);
    let scene = session.headgear();
    assert!(!scene.characters().is_empty(), "TEST sent no character list");
    for character in scene.characters() {
        let files = scene.part_files_for(character);
        let worn = character.look.headgear().iter().filter(|view_id| **view_id > 0).count();
        eprintln!(
            "E2E_LIVE character job={} hair={} headgear={:?} parts={}",
            character.job_id.0,
            character.look.hair,
            character.look.headgear(),
            files.len()
        );
        assert!(files.len() >= 2);
        assert!(files.len() <= 2 + worn);
        assert!(files[2..].iter().all(|file| file.starts_with('?')));
    }
    for player in scene.players() {
        let files = scene.part_files_for(player);
        eprintln!(
            "E2E_LIVE player job={} headgear={:?} parts={}",
            player.job_id.0,
            player.look.headgear(),
            files.len()
        );
        assert!(files.len() >= 2);
    }
}
