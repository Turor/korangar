//! Item reform and laphine windows through injected packets and input.

use korangar::e2e::{LiveConnect, LogicSession, WorkshopView};

fn le_u16(header: u16) -> [u8; 2] {
    header.to_le_bytes()
}

fn open_packet(header: u16, item_id: u32) -> Vec<u8> {
    let mut bytes = le_u16(header).to_vec();
    bytes.extend(item_id.to_le_bytes());
    bytes
}

fn reform_result(index: u16, result: i8) -> Vec<u8> {
    let mut bytes = le_u16(0x0B92).to_vec();
    bytes.extend((index + 2).to_le_bytes());
    bytes.push(result as u8);
    bytes
}

fn synthesis_result(result: i16) -> Vec<u8> {
    let mut bytes = le_u16(0x0A50).to_vec();
    bytes.extend(result.to_le_bytes());
    bytes
}

fn upgrade_result(result: u16) -> Vec<u8> {
    let mut bytes = le_u16(0x0AB7).to_vec();
    bytes.extend(result.to_le_bytes());
    bytes
}

const TABLES: &str = r#"
    ItemDBNameTbl = { Knife = 1201, Dagger = 1202, Oridecon = 984, Hammer = 4001 }
    ReformInfo = {
        [1] = {
            BaseItem = "Knife",
            ResultItem = "Dagger",
            NeedRefineMin = 5,
            NeedRefineMax = 10,
            NeedOptionNumMin = 0,
            IsEmptySocket = true,
            ChangeRefineValue = 1,
            RandomOptionCode = "",
            PreserveSocketItem = false,
            PreserveGrade = true,
            Material = { Oridecon = 2 },
            InformationString = { "Keeps nothing else." },
        },
    }
    ReformItemList = { Hammer = { 1 } }
    tblLapineDdukddakBox = {
        sources = {
            [1] = {
                ItemID = 7210,
                NeedCount = 1,
                NeedRefineMin = 0,
                NeedRefineMax = 20,
                NeedSource_String = "One ore",
                SourceItems = { { "Oridecon", 2, 984 } },
            },
        },
    }
    tblLapineUpgradeBox = {
        targets = {
            [1] = {
                ItemID = 7211,
                NeedRefineMin = 7,
                NeedRefineMax = 10,
                NeedSource_String = "+7 to +10",
                NeedOptionNumMin = 0,
                NotSocketEnchantItem = 1,
                TargetItems = { { "Knife", 1201 } },
            },
        },
    }
"#;

fn session() -> LogicSession {
    let mut session = LogicSession::offline();
    session.load_workshop_lua(TABLES).expect("workshop tables");
    session.seed_workshop_equip(2, 1201, "Knife", 7, 0);
    session.seed_workshop_equip(3, 1201, "Knife", 3, 0);
    session.seed_workshop_equip(5, 1201, "Knife", 9, 4001);
    session.seed_workshop_stack(4, 984, "Oridecon", 5);
    session
}

fn assert_window(view: &WorkshopView, name: &str) {
    assert!(view.windows.iter().any(|window| window == name), "windows {:?}", view.windows);
}

fn assert_window_closed(view: &WorkshopView, name: &str) {
    assert!(view.windows.iter().all(|window| window != name), "windows {:?}", view.windows);
}

#[test]
fn offline_reform_filters_and_encodes() {
    let mut session = session();
    session.feed_workshop_packet(&open_packet(0x0B8F, 4001)).unwrap();
    let view = session.workshop_view();
    assert_window(&view, "ItemReform");
    assert_eq!(view.reform_catalyst, "Hammer");
    assert_eq!(view.reform_candidates.len(), 1);
    assert!(view.reform_candidates[0].contains("+7 Knife"));
    assert!(!view.reform_enabled);

    session.apply_line("event ItemReformSelect(3)").unwrap();
    assert!(!session.workshop_view().reform_enabled);
    session.apply_line("event ItemReformSubmit").unwrap();
    assert!(session.workshop_view().outbound.is_empty());

    session.apply_line("event ItemReformSelect(2)").unwrap();
    let view = session.workshop_view();
    assert!(view.reform_enabled);
    assert!(view.reform_preview.contains("+8 Dagger"));
    assert!(view.reform_kept.contains("cards cleared"));
    assert!(view.reform_materials.iter().any(|row| row.contains("Oridecon 5/2")));

    session.apply_line("event ItemReformSubmit").unwrap();
    let request = [0x91, 0x0B, 0xA1, 0x0F, 0x00, 0x00, 0x04, 0x00];
    assert_eq!(session.workshop_view().outbound, vec![request.to_vec()]);

    session.feed_workshop_packet(&reform_result(2, 1)).unwrap();
    let view = session.workshop_view();
    assert_window(&view, "ItemReform");
    assert!(view.reform_status.contains("failed"));
    assert!(view.chat.iter().any(|line| line.contains("failed")));
    assert_eq!(view.outbound.len(), 1);

    session.feed_workshop_packet(&reform_result(2, 0)).unwrap();
    let view = session.workshop_view();
    assert_window_closed(&view, "ItemReform");
    assert!(view.chat.iter().any(|line| line.contains("succeeded")));
    assert_eq!(view.outbound.last().unwrap(), &[0x90, 0x0B]);

    session.feed_workshop_packet(&open_packet(0x0B8F, 4001)).unwrap();
    session.apply_line("close itemreform").unwrap();
    let view = session.workshop_view();
    assert_window_closed(&view, "ItemReform");
    assert_eq!(view.outbound.last().unwrap(), &[0x90, 0x0B]);
}

#[test]
fn offline_missing_reform_list_does_not_guess() {
    let mut session = LogicSession::offline();
    session.seed_workshop_equip(2, 1201, "Knife", 7, 0);
    session.feed_workshop_packet(&open_packet(0x0B8F, 4001)).unwrap();
    let view = session.workshop_view();
    assert!(view.reform_candidates.is_empty());
    assert!(!view.reform_enabled);
    assert!(view.reform_preview.contains("Result decided by server"));
    assert!(view.reform_status.contains("Result decided by server"));
    session.apply_line("event ItemReformSelect(2)").unwrap();
    session.apply_line("event ItemReformSubmit").unwrap();
    assert!(session.workshop_view().outbound.is_empty());
}

#[test]
fn offline_synthesis_toggles_and_encodes() {
    let mut session = session();
    session.feed_workshop_packet(&open_packet(0x0A4E, 7210)).unwrap();
    let view = session.workshop_view();
    assert_window(&view, "LaphineSynthesis");
    assert!(view.synthesis_requirement.contains("0/1"));
    assert_eq!(view.synthesis_candidates.len(), 1);
    assert!(view.synthesis_candidates[0].contains("Oridecon"));
    assert!(!view.synthesis_enabled);

    session.apply_line("event LaphineSynthesisToggle(2)").unwrap();
    assert!(session.workshop_view().synthesis_chosen.is_empty());
    session.apply_line("event LaphineSynthesisToggle(4)").unwrap();
    let view = session.workshop_view();
    assert!(view.synthesis_enabled);
    assert!(view.synthesis_chosen.iter().any(|row| row.contains("x2")));
    assert!(view.synthesis_candidates.is_empty());

    session.apply_line("event LaphineSynthesisToggle(4)").unwrap();
    assert!(!session.workshop_view().synthesis_enabled);
    session.apply_line("event LaphineSynthesisToggle(4)").unwrap();
    session.apply_line("event LaphineSynthesisSubmit").unwrap();
    let request = [0x4F, 0x0A, 0x0C, 0x00, 0x2A, 0x1C, 0x00, 0x00, 0x06, 0x00, 0x02, 0x00];
    assert_eq!(session.workshop_view().outbound, vec![request.to_vec()]);

    session.feed_workshop_packet(&synthesis_result(3)).unwrap();
    assert_window(&session.workshop_view(), "LaphineSynthesis");
    assert!(session.workshop_view().synthesis_status.contains("failed"));

    session.feed_workshop_packet(&synthesis_result(0)).unwrap();
    let view = session.workshop_view();
    assert_window_closed(&view, "LaphineSynthesis");
    assert!(view.chat.iter().any(|line| line.contains("succeeded")));
    assert_eq!(view.outbound.last().unwrap(), &[0x70, 0x0A]);
}

#[test]
fn offline_upgrade_selects_and_encodes() {
    let mut session = session();
    session.feed_workshop_packet(&open_packet(0x0AB4, 7211)).unwrap();
    let view = session.workshop_view();
    assert_window(&view, "LaphineUpgrade");
    assert!(view.upgrade_preview.contains("+7 to +10"));
    assert_eq!(view.upgrade_candidates.len(), 2);
    assert!(view.upgrade_candidates.iter().any(|row| row.contains("+7 Knife")));
    assert!(view.upgrade_candidates.iter().any(|row| row.contains("+9 Knife")));
    assert!(!view.upgrade_enabled);

    session.apply_line("event LaphineUpgradeSelect(3)").unwrap();
    assert!(!session.workshop_view().upgrade_enabled);
    session.apply_line("event LaphineUpgradeSelect(5)").unwrap();
    let view = session.workshop_view();
    assert!(view.upgrade_enabled);
    assert!(view.upgrade_status.contains("sockets"));
    assert!(view.upgrade_preview.contains("+9 Knife"));
    assert!(view.upgrade_preview.contains("+7 to +10"));

    session.apply_line("event LaphineUpgradeSelect(5)").unwrap();
    assert!(!session.workshop_view().upgrade_enabled);
    session.apply_line("event LaphineUpgradeSelect(5)").unwrap();
    session.apply_line("event LaphineUpgradeSubmit").unwrap();
    let request = [0xB6, 0x0A, 0x2B, 0x1C, 0x00, 0x00, 0x07, 0x00];
    assert_eq!(session.workshop_view().outbound, vec![request.to_vec()]);

    session.feed_workshop_packet(&upgrade_result(1)).unwrap();
    assert_window(&session.workshop_view(), "LaphineUpgrade");
    session.feed_workshop_packet(&upgrade_result(0)).unwrap();
    let view = session.workshop_view();
    assert_window_closed(&view, "LaphineUpgrade");
    assert!(view.chat.iter().any(|line| line.contains("succeeded")));
    assert_eq!(view.outbound.last().unwrap(), &[0xB5, 0x0A]);

    session.feed_workshop_packet(&open_packet(0x0AB4, 7211)).unwrap();
    session.apply_line("close laphineupgrade").unwrap();
    assert_window_closed(&session.workshop_view(), "LaphineUpgrade");
}

#[test]
fn live_item_workshop_is_not_reachable_without_gm() {
    match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => {
            let _session = session;
            eprintln!("E2E_SKIP not reachable on TEST without GM");
        }
    }
}
