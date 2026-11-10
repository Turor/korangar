//! Quest list, add, and hunt-progress packets for PACKETVER >= 20181017.
//!
//! Hercules `questListType` / `questAddType` / `questUpdateType` are `0x0AFF`,
//! `0x0B0C`, and `0x0AFE` (`packets_struct.h`). Each list and add objective has
//! `huntIdent`, `huntIdent2`, then `mobType`, then `mob_id`. The 20150513
//! `QuestDetails` layout has no `huntIdent2`, so reading `0x0AFF` with it
//! shifts `mob_id`. `0x0AFE` is the kill update: `questID`, `huntIdent`,
//! `huntIdent2` (the objective index), `maxCount`, `count`. It has no mob id.

use ragnarok_bytes::{FromBytes, ToBytes};

use crate::{MapServer, Packet, ServerPacket};

/// One objective inside `ZC_ALL_QUEST_LIST4` (`packet_mission_info_sub`).
#[derive(Debug, Clone, ragnarok_bytes::ByteConvertable)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct QuestMissionInfo {
    pub hunt_ident: u32,
    pub hunt_ident2: u32,
    pub mob_type: u32,
    pub mob_id: u32,
    pub minimum_level: u16,
    pub maximum_level: u16,
    pub kill_count: u16,
    pub total_count: u16,
    #[length(24)]
    pub mob_name: String,
}

/// One quest inside `ZC_ALL_QUEST_LIST4` (`packet_quest_list_info`).
#[derive(Debug, Clone, ragnarok_bytes::ByteConvertable)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct QuestListInfo4 {
    pub quest_id: u32,
    pub active: u8,
    pub remaining_time: u32,
    pub expire_time: u32,
    #[new_derive]
    pub objective_count: u16,
    #[repeating(objective_count)]
    pub objective_details: Vec<QuestMissionInfo>,
}

/// `ZC_ALL_QUEST_LIST4` (`0x0AFF`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0AFF)]
#[variable_length]
pub struct QuestListPacket4 {
    #[new_derive]
    pub quest_count: u32,
    #[repeating(quest_count)]
    pub quests: Vec<QuestListInfo4>,
}

/// One objective inside `ZC_ADD_QUEST` `0x0B0C` (`packet_quest_hunt_sub`).
/// `hunt_count` is the current kill count. The total arrives on `0x08FE`.
#[derive(Debug, Clone, ragnarok_bytes::ByteConvertable)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct QuestHuntSub {
    pub hunt_ident: u32,
    pub hunt_ident2: u32,
    pub mob_type: u32,
    pub mob_id: u32,
    pub minimum_level: u16,
    pub maximum_level: u16,
    pub hunt_count: u16,
    #[length(24)]
    pub mob_name: String,
}

/// `ZC_ADD_QUEST` (`0x0B0C`). Always padded to three objectives. Framing length
/// is 155.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B0C)]
pub struct QuestAddedPacket4 {
    pub quest_id: u32,
    pub active: u8,
    pub start_time: u32,
    pub expire_time: u32,
    pub objective_count: u16,
    pub objective_details: [QuestHuntSub; 3],
}

/// One row of `ZC_UPDATE_MISSION_HUNT` (`0x0AFE`). `hunt_ident2` is the
/// objective index. There is no mob id on this packet.
#[derive(Debug, Clone, ragnarok_bytes::ByteConvertable, ragnarok_bytes::FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct QuestHuntProgress {
    pub quest_id: u32,
    pub hunt_ident: u32,
    pub hunt_ident2: u32,
    pub total_count: u16,
    pub current_count: u16,
}

/// `ZC_UPDATE_MISSION_HUNT` (`0x0AFE`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0AFE)]
#[variable_length]
pub struct HuntingQuestUpdatePacket4 {
    pub objective_count: u16,
    #[repeating_remaining]
    pub objective_details: Vec<QuestHuntProgress>,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::{
        HuntingQuestUpdatePacket4, QuestAddedPacket4, QuestHuntProgress, QuestHuntSub, QuestListInfo4, QuestListPacket4, QuestMissionInfo,
    };
    use crate::PacketExt;

    fn empty_hunt(mob_id: u32, name: &str) -> QuestHuntSub {
        QuestHuntSub {
            hunt_ident: 501,
            hunt_ident2: 0,
            mob_type: 1,
            mob_id,
            minimum_level: 1,
            maximum_level: 99,
            hunt_count: 2,
            mob_name: name.to_owned(),
        }
    }

    #[test]
    fn quest_list_4_keeps_mob_id_after_hunt_ident2() {
        let packet = QuestListPacket4 {
            quest_count: 1,
            quests: vec![QuestListInfo4 {
                quest_id: 501,
                active: 1,
                remaining_time: 11,
                expire_time: 90,
                objective_count: 1,
                objective_details: vec![QuestMissionInfo {
                    hunt_ident: 501,
                    hunt_ident2: 0,
                    mob_type: 1,
                    mob_id: 1002,
                    minimum_level: 1,
                    maximum_level: 99,
                    kill_count: 2,
                    total_count: 5,
                    mob_name: "Poring".to_owned(),
                }],
            }],
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.into_inner();
        assert_eq!(&bytes[..2], &[0xFF, 0x0A]);
        // header 2 + length 2 + count 4 + quest 15 + objective 48
        assert_eq!(bytes.len(), 71);

        let mut reader = ByteReader::without_metadata(&bytes);
        let decoded = QuestListPacket4::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        let objective = &decoded.quests[0].objective_details[0];
        assert_eq!(objective.hunt_ident2, 0);
        assert_eq!(objective.mob_type, 1);
        assert_eq!(objective.mob_id, 1002);
        assert_eq!(objective.kill_count, 2);
        assert_eq!(objective.total_count, 5);
        assert_eq!(objective.mob_name, "Poring");
    }

    #[test]
    fn quest_add_4_is_155_bytes_and_mob_id_is_not_shifted() {
        let packet = QuestAddedPacket4 {
            quest_id: 501,
            active: 1,
            start_time: 11,
            expire_time: 90,
            objective_count: 1,
            objective_details: [empty_hunt(1002, "Poring"), empty_hunt(0, ""), empty_hunt(0, "")],
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.into_inner();
        assert_eq!(bytes.len(), 155);
        assert_eq!(&bytes[..2], &[0x0C, 0x0B]);
        // header 2 + quest id 4 + active 1 + times 8 + count 2 + huntIdent 4 +
        // huntIdent2 4 + mobType 4
        assert_eq!(u32::from_le_bytes(bytes[29..33].try_into().unwrap()), 1002);

        let mut reader = ByteReader::without_metadata(&bytes);
        let decoded = QuestAddedPacket4::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(decoded.objective_details[0].mob_id, 1002);
        assert_eq!(decoded.objective_details[0].hunt_ident2, 0);
        assert_eq!(decoded.objective_details[0].mob_name, "Poring");
    }

    #[test]
    fn hunt_update_4_uses_the_objective_index_and_has_no_mob_id() {
        let packet = HuntingQuestUpdatePacket4 {
            objective_count: 1,
            objective_details: vec![QuestHuntProgress {
                quest_id: 501,
                hunt_ident: 501,
                hunt_ident2: 1,
                total_count: 5,
                current_count: 3,
            }],
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.into_inner();
        // header 2 + length 2 + count 2 + 16-byte row
        assert_eq!(bytes.len(), 22);
        assert_eq!(&bytes[..2], &[0xFE, 0x0A]);

        let mut reader = ByteReader::without_metadata(&bytes);
        let decoded = HuntingQuestUpdatePacket4::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(decoded.objective_details[0].hunt_ident2, 1);
        assert_eq!(decoded.objective_details[0].current_count, 3);
        assert_eq!(decoded.objective_details[0].total_count, 5);
    }
}
