//! Pet capture, egg hatching, and companion panels.
//!
//! Offline cases feed Hercules packet bytes and drive the injected-input path.
//! The live case only checks the empty Pets panel: pool characters are not GMs.

use korangar::e2e::{CompanionSession, LiveConnect};
use ragnarok_bytes::ByteWriter;
use ragnarok_packets::{
    CompanionSkillEntry, HomunculusPropertyPacket, HomunculusSkillListPacket, PacketExt, PetEggListPacket, PetEggWireIndex,
    PetPropertyPacket, StartCapturePacket, TryCaptureResultPacket,
};

fn encode<T: PacketExt>(packet: &T) -> Vec<u8> {
    let mut writer = ByteWriter::new();
    packet.packet_to_bytes(&mut writer).expect("companion fixture encodes");
    writer.into_inner()
}

fn header(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

#[test]
fn capture_click_sends_trycapture_and_applies_the_result() {
    let mut session = CompanionSession::offline();
    session.feed(&encode(&StartCapturePacket {})).unwrap();
    let armed = session.view();
    assert!(armed.capture_armed);
    assert_eq!(armed.status, "Click a monster to tame it.");

    session.inject_monster_click(0x1234).unwrap();
    let packet = session.packet(0x019F).expect("0x019F");
    assert_eq!(packet, [0x9F, 0x01, 0x34, 0x12, 0x00, 0x00]);
    assert!(session.view().capture_armed, "capture stays armed until the result");

    session.feed(&encode(&TryCaptureResultPacket { result: 0 })).unwrap();
    let failed = session.view();
    assert!(!failed.capture_armed);
    assert_eq!(failed.status, "Taming failed.");
}

#[test]
fn egg_choice_sends_the_wire_index_and_close_sends_nothing() {
    let mut session = CompanionSession::offline();
    session
        .feed(&encode(&PetEggListPacket {
            indices: vec![PetEggWireIndex { index: 4 }, PetEggWireIndex { index: 9 }],
        }))
        .unwrap();
    assert_eq!(session.view().eggs, vec![(4, "Egg 4".to_owned()), (9, "Egg 9".to_owned())]);

    session.inject_egg_row(1).unwrap();
    assert_eq!(session.packet(0x01A7).expect("0x01A7"), [0xA7, 0x01, 0x09, 0x00]);
    assert!(session.view().eggs.is_empty());

    let mut closed = CompanionSession::offline();
    closed
        .feed(&encode(&PetEggListPacket {
            indices: vec![PetEggWireIndex { index: 4 }],
        }))
        .unwrap();
    closed.inject_close_eggs();
    assert!(closed.outbound().is_empty());
    assert!(closed.view().eggs.is_empty());
    assert_eq!(closed.view().status, "Egg hatching cancelled.");
}

#[test]
fn pet_bars_and_rename_use_the_injected_name() {
    let mut session = CompanionSession::offline();
    session
        .feed(&encode(&PetPropertyPacket {
            name: "Poring".to_owned(),
            renamed: 0,
            level: 4,
            hunger: 5,
            intimacy: 910,
            accessory_id: 12,
            class_id: 1002,
        }))
        .unwrap();
    let view = session.view();
    assert!(view.pet_known);
    assert_eq!(view.pet_name, "Poring");
    assert_eq!(view.pet_level, 4);
    assert_eq!(view.pet_hunger, 5);
    assert_eq!(view.pet_hunger_label, "Very Hungry");
    assert_eq!(view.pet_intimacy, 910);
    assert_eq!(view.pet_intimacy_label, "Loyal");
    assert!(view.pet_rename_allowed);
    assert!(!view.empty_pet);

    session.inject_pet_rename("Pip").unwrap();
    let packet = session.packet(0x01A5).expect("0x01A5");
    assert_eq!(packet.len(), 26);
    assert_eq!(header(packet), 0x01A5);
    assert_eq!(&packet[2..5], b"Pip");
    assert!(packet[5..].iter().all(|byte| *byte == 0));
}

#[test]
fn homunculus_panel_skill_up_uses_0112_and_rename_uses_0231() {
    let mut session = CompanionSession::offline();
    session
        .feed(&encode(&HomunculusPropertyPacket {
            name: "Lif".to_owned(),
            flags: 0,
            level: 50,
            hunger: 40,
            intimacy: 910,
            attack: 10,
            magic_attack: 11,
            hit: 12,
            critical: 13,
            defense: 14,
            magic_defense: 15,
            flee: 16,
            attack_motion: 17,
            health_points: 2000,
            maximum_health_points: 3000,
            spell_points: 100,
            maximum_spell_points: 200,
            experience: 10,
            next_experience: 100,
            skill_points: 2,
            attack_range: 3,
        }))
        .unwrap();
    session
        .feed(&encode(&HomunculusSkillListPacket {
            skills: vec![CompanionSkillEntry {
                skill_id: 8004,
                skill_type: 1,
                level: 3,
                sp_cost: 12,
                attack_range: 7,
                name: "HVAN_CAPRICE".to_owned(),
                upgradable: 1,
            }],
        }))
        .unwrap();

    let view = session.view();
    assert!(view.homunculus_known);
    assert_eq!(view.homunculus_name, "Lif");
    assert_eq!(view.homunculus_level, 50);
    assert_eq!((view.homunculus_hp, view.homunculus_max_hp), (2000, 3000));
    assert_eq!((view.homunculus_sp, view.homunculus_max_sp), (100, 200));
    assert_eq!(view.homunculus_exp, 10);
    assert_eq!(view.homunculus_next_exp, 100);
    assert_eq!(view.homunculus_hunger, 40);
    assert_eq!(view.homunculus_intimacy, 910);
    assert_eq!(view.homunculus_intimacy_label, "Loyal");
    assert_eq!(view.homunculus_skill_points, 2);
    assert!(view.homunculus_rename_allowed);
    assert_eq!(view.ai_mode, "Standby");
    let shown = session.homunculus_skill(0).expect("homunculus skill");
    assert_eq!(shown.skill_id, 8004);
    assert_eq!(shown.name, "HVAN_CAPRICE");
    assert!(shown.upgradable);
    assert!(!view.empty_homunculus);

    session.inject_skill_up(8004).unwrap();
    let skill = session.packet(0x0112).expect("0x0112");
    assert_eq!(skill, korangar_networking::homunculus_skill_up_bytes(8004));
    assert_eq!(skill, [0x12, 0x01, 0x44, 0x1F]);
    assert!(session.outbound().iter().all(|bytes| header(bytes) != 0x0237));
    assert_eq!(session.view().homunculus_skill_points, 1);

    session.inject_homunculus_rename("Amy").unwrap();
    let rename = session.packet(0x0231).expect("0x0231");
    assert_eq!(rename.len(), 26);
    assert_eq!(&rename[2..5], b"Amy");
    assert!(rename[5..].iter().all(|byte| *byte == 0));
}

#[test]
fn live_pets_panel_is_empty_without_a_companion() {
    let mut session = match CompanionSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    session.enter_world().expect("map entry");
    assert!(session.window_open());
    if session.note().saw_owned_companion() {
        eprintln!("pool character already has a companion packet; taming/hatch/homun actions still not reachable on TEST without GM");
    } else {
        let view = session.view();
        assert!(view.empty_pet && view.empty_homunculus && view.empty_mercenary && view.empty_elemental);
        assert!(view.eggs.is_empty());
        assert!(!view.capture_armed);
        eprintln!("taming/hatch/homun not reachable on TEST without GM");
    }
}
