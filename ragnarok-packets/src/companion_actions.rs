//! Pet capture, egg hatching, rename, homunculus movement, and companion
//! skills.
//!
//! Layouts follow Hercules `packets_struct.h` / `clif.c` for PACKETVER 20250716
//! and the lengths in `framing_20250716.rs`. Homunculus skill-up is not in this
//! file: it reuses `CZ_UPGRADE_SKILLLEVEL` (`0x0112`). `0x0237` is
//! `CZ_RANKING_PK` (2 bytes) and is not a skill id packet.

use ragnarok_bytes::{ByteConvertable, FixedByteSize, FromBytes, ToBytes};
#[cfg(not(feature = "derive"))]
use ragnarok_macros::{ClientPacket, MapServer, Packet, ServerPacket};

#[cfg(feature = "derive")]
use crate::{ClientPacket, MapServer, Packet, ServerPacket};

/// `ZC_START_CAPTURE` (`0x019E`). Header only. The client enters capture mode.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x019E)]
pub struct StartCapturePacket {}

/// `CZ_TRYCAPTURE_MONSTER` (`0x019F`). 6 bytes: target account id.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x019F)]
pub struct TryCaptureMonsterPacket {
    pub target_id: u32,
}

/// `ZC_TRYCAPTURE_MONSTER` (`0x01A0`). 3 bytes. `0` fail, `1` success.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01A0)]
pub struct TryCaptureResultPacket {
    pub result: u8,
}

/// `CZ_RENAME_PET` (`0x01A5`). 26 bytes. The name is `NAME_LENGTH` (24).
#[derive(Debug, Clone, PartialEq, Eq, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01A5)]
pub struct RenamePetPacket {
    #[length(24)]
    pub name: String,
}

/// One inventory index from `ZC_PETEGG_LIST`. Hercules sends `inventory + 2`.
#[derive(Debug, Clone, PartialEq, Eq, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct PetEggWireIndex {
    pub index: u16,
}

/// `ZC_PETEGG_LIST` (`0x01A6`). `01a6 <len>.W { <index>.W }*`.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01A6)]
#[variable_length]
pub struct PetEggListPacket {
    #[repeating_remaining]
    pub indices: Vec<PetEggWireIndex>,
}

/// `CZ_SELECT_PETEGG` (`0x01A7`). 4 bytes. The client echoes the list index.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01A7)]
pub struct SelectPetEggPacket {
    pub index: u16,
}

/// `CZ_RENAME_MER` (`0x0231`). 26 bytes. Homunculus name, `NAME_LENGTH` (24).
#[derive(Debug, Clone, PartialEq, Eq, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0231)]
pub struct RenameHomunculusPacket {
    #[length(24)]
    pub name: String,
}

/// `ZC_CHANGESTATE_MER` (`0x0230`). 12 bytes.
/// `0230 <type>.B <state>.B <id>.L <data>.L`.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0230)]
pub struct HomunculusStatePacket {
    pub kind: i8,
    pub state: i8,
    pub entity_id: u32,
    pub value: i32,
}

/// `CZ_REQUEST_MOVENPC` (`0x0232`). 9 bytes: id plus 3 packed position bytes.
/// Standby does not use this packet.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0232)]
pub struct MoveNpcPacket {
    pub entity_id: u32,
    pub position: [u8; 3],
}

/// `CZ_REQUEST_ACTNPC` (`0x0233`). 11 bytes. `action` is always 0 for attack.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0233)]
pub struct ActNpcPacket {
    pub entity_id: u32,
    pub target_id: u32,
    pub action: u8,
}

/// `CZ_REQUEST_MOVETOOWNER` (`0x0234`). 6 bytes. Follow the owner.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0234)]
pub struct MoveToOwnerPacket {
    pub entity_id: u32,
}

/// One homunculus or mercenary skill row (37 bytes), shared by `0x0235` and
/// `0x029D`.
///
/// `<skill id>.W <type>.L <level>.W <sp>.W <range>.W <name>.24B <upgradable>.B`
#[derive(Debug, Clone, PartialEq, Eq, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct CompanionSkillEntry {
    pub skill_id: u16,
    pub skill_type: i32,
    pub level: i16,
    pub sp_cost: i16,
    pub attack_range: i16,
    #[length(24)]
    pub name: String,
    pub upgradable: u8,
}

/// `ZC_HOSKILLINFO_LIST` (`0x0235`).
#[derive(Debug, Clone, PartialEq, Eq, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0235)]
#[variable_length]
pub struct HomunculusSkillListPacket {
    #[repeating_remaining]
    pub skills: Vec<CompanionSkillEntry>,
}

/// `ZC_HOSKILLINFO_UPDATE` (`0x0239`). 11 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0239)]
pub struct HomunculusSkillUpdatePacket {
    pub skill_id: u16,
    pub level: i16,
    pub sp_cost: i16,
    pub attack_range: i16,
    pub upgradable: u8,
}

/// `ZC_MER_SKILLINFO_LIST` (`0x029D`). Same 37-byte rows as the homunculus
/// list.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x029D)]
#[variable_length]
pub struct MercenarySkillListPacket {
    #[repeating_remaining]
    pub skills: Vec<CompanionSkillEntry>,
}

/// `ZC_MER_SKILLINFO_UPDATE` (`0x029E`). 11 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x029E)]
pub struct MercenarySkillUpdatePacket {
    pub skill_id: u16,
    pub level: i16,
    pub sp_cost: i16,
    pub attack_range: i16,
    pub upgradable: u8,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;
    use crate::PacketExt;

    fn round_trip<T>(packet: &T, len: usize, header: u16) -> Vec<u8>
    where
        T: PacketExt + PartialEq + std::fmt::Debug,
    {
        let mut writer = ByteWriter::new();
        assert_eq!(packet.packet_to_bytes(&mut writer).unwrap(), len);
        let bytes = writer.into_inner();
        assert_eq!(bytes.len(), len);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), header);
        let mut reader = ByteReader::without_metadata(&bytes);
        let parsed = T::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(&parsed, packet);
        bytes
    }

    fn skill(name: &str, upgradable: u8) -> CompanionSkillEntry {
        CompanionSkillEntry {
            skill_id: 8004,
            skill_type: 1,
            level: 3,
            sp_cost: 12,
            attack_range: 7,
            name: name.to_owned(),
            upgradable,
        }
    }

    #[test]
    fn capture_packets_match_hercules_bytes() {
        let bytes = round_trip(&StartCapturePacket {}, 2, 0x019E);
        assert_eq!(bytes, [0x9E, 0x01]);

        let bytes = round_trip(&TryCaptureMonsterPacket { target_id: 0x1234_5678 }, 6, 0x019F);
        assert_eq!(bytes, [0x9F, 0x01, 0x78, 0x56, 0x34, 0x12]);

        let bytes = round_trip(&TryCaptureResultPacket { result: 1 }, 3, 0x01A0);
        assert_eq!(bytes, [0xA0, 0x01, 0x01]);
        let fail = round_trip(&TryCaptureResultPacket { result: 0 }, 3, 0x01A0);
        assert_eq!(fail[2], 0);
    }

    #[test]
    fn rename_and_egg_packets_match_hercules_bytes() {
        let bytes = round_trip(&RenamePetPacket { name: "Picky".to_owned() }, 26, 0x01A5);
        assert_eq!(&bytes[2..7], b"Picky");
        assert!(bytes[7..26].iter().all(|byte| *byte == 0));

        let bytes = round_trip(&RenameHomunculusPacket { name: "Lif".to_owned() }, 26, 0x0231);
        assert_eq!(&bytes[2..5], b"Lif");
        assert!(bytes[5..26].iter().all(|byte| *byte == 0));

        let bytes = round_trip(
            &PetEggListPacket {
                indices: vec![PetEggWireIndex { index: 4 }, PetEggWireIndex { index: 9 }],
            },
            8,
            0x01A6,
        );
        assert_eq!(bytes, [0xA6, 0x01, 0x08, 0x00, 0x04, 0x00, 0x09, 0x00]);

        let bytes = round_trip(&SelectPetEggPacket { index: 4 }, 4, 0x01A7);
        assert_eq!(bytes, [0xA7, 0x01, 0x04, 0x00]);
    }

    #[test]
    fn homunculus_state_and_commands_match_hercules_bytes() {
        let bytes = round_trip(
            &HomunculusStatePacket {
                kind: 0,
                state: 2,
                entity_id: 0x10,
                value: 40,
            },
            12,
            0x0230,
        );
        assert_eq!(bytes[2], 0);
        assert_eq!(bytes[3], 2);
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 0x10);
        assert_eq!(i32::from_le_bytes(bytes[8..12].try_into().unwrap()), 40);

        let bytes = round_trip(&MoveToOwnerPacket { entity_id: 0x22 }, 6, 0x0234);
        assert_eq!(bytes, [0x34, 0x02, 0x22, 0x00, 0x00, 0x00]);

        let bytes = round_trip(
            &ActNpcPacket {
                entity_id: 0x22,
                target_id: 0x99,
                action: 0,
            },
            11,
            0x0233,
        );
        assert_eq!(bytes[0], 0x33);
        assert_eq!(bytes[1], 0x02);
        assert_eq!(u32::from_le_bytes(bytes[2..6].try_into().unwrap()), 0x22);
        assert_eq!(u32::from_le_bytes(bytes[6..10].try_into().unwrap()), 0x99);
        assert_eq!(bytes[10], 0);

        let bytes = round_trip(
            &MoveNpcPacket {
                entity_id: 0x22,
                position: [1, 2, 3],
            },
            9,
            0x0232,
        );
        assert_eq!(&bytes[6..9], &[1, 2, 3]);
    }

    #[test]
    fn skill_rows_are_37_bytes_and_updates_are_11() {
        let entry = skill("HLIF_HEAL", 1);
        let list = round_trip(
            &HomunculusSkillListPacket {
                skills: vec![entry.clone()],
            },
            4 + 37,
            0x0235,
        );
        assert_eq!(u16::from_le_bytes([list[2], list[3]]), 41);
        assert_eq!(u16::from_le_bytes([list[4], list[5]]), 8004);
        assert_eq!(i32::from_le_bytes(list[6..10].try_into().unwrap()), 1);
        assert_eq!(i16::from_le_bytes([list[10], list[11]]), 3);
        assert_eq!(i16::from_le_bytes([list[12], list[13]]), 12);
        assert_eq!(i16::from_le_bytes([list[14], list[15]]), 7);
        assert_eq!(&list[16..25], b"HLIF_HEAL");
        assert!(list[25..40].iter().all(|byte| *byte == 0));
        assert_eq!(list[40], 1);

        let merc = round_trip(
            &MercenarySkillListPacket {
                skills: vec![skill("MER_MAGNIFICAT", 0)],
            },
            41,
            0x029D,
        );
        assert_eq!(merc[40], 0);

        let update = round_trip(
            &HomunculusSkillUpdatePacket {
                skill_id: 8004,
                level: 4,
                sp_cost: 15,
                attack_range: 7,
                upgradable: 1,
            },
            11,
            0x0239,
        );
        assert_eq!(update, [0x39, 0x02, 0x44, 0x1F, 0x04, 0x00, 0x0F, 0x00, 0x07, 0x00, 0x01]);
        let merc_update = round_trip(
            &MercenarySkillUpdatePacket {
                skill_id: 8201,
                level: 1,
                sp_cost: 5,
                attack_range: 1,
                upgradable: 0,
            },
            11,
            0x029E,
        );
        assert_eq!(u16::from_le_bytes([merc_update[2], merc_update[3]]), 8201);
        assert_eq!(merc_update[10], 0);
    }
}
