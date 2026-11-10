//! Guild actions for Hercules PACKETVER 20250716 MAIN.
//!
//! Layouts follow `clif.c` / `packets_struct.h` and the 20250716 length table.
//! `0x0167` is `ZC_RESULT_MAKE_GUILD` (create result), not a skill level-up.
//! Guild skill level-up reuses `CZ_UPGRADE_SKILLLEVEL` (`0x0112`).
//!
//! On this packetver the emblem image is `ZC_GUILD_EMBLEM_IMG` `0x0B36`
//! (chunked) and the overhead id is `ZC_CHANGE_GUILD` `0x0B47`. Legacy
//! `0x0152` and `0x01B4` are still parsed.

#[cfg(feature = "derive")]
use crate::{ClientPacket, MapServer, Packet, ServerPacket};
#[cfg(not(feature = "derive"))]
use ragnarok_macros::{ClientPacket, MapServer, Packet, ServerPacket};

use ragnarok_bytes::{ByteConvertable, FixedByteSize, FromBytes, ToBytes};

use crate::{AccountId, CharacterId, SkillId};

pub const GUILD_MENU_SKILLS: u32 = 3;
pub const GUILD_NAME_LENGTH: usize = 24;
pub const GUILD_REASON_LENGTH: usize = 40;

/// `CZ_REQ_MAKE_GUILD` (`0x0165`). 30 bytes: char id plus a 24-byte name.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0165)]
pub struct CreateGuildPacket {
    pub character_id: CharacterId,
    #[length(24)]
    pub name: String,
}

/// `CZ_REQ_JOIN_GUILD` (`0x0168`). 14 bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0168)]
pub struct GuildInviteByAccountPacket {
    pub target_account_id: AccountId,
    pub inviter_account_id: AccountId,
    pub inviter_character_id: CharacterId,
}

/// `CZ_REQ_JOIN_GUILD2` (`0x0916`). 26 bytes. The 20250716 name invite.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0916)]
pub struct GuildInviteByNamePacket {
    #[length(24)]
    pub name: String,
}

/// `CZ_JOIN_GUILD` (`0x016B`). Answer 0 refuses, 1 accepts.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x016B)]
pub struct GuildInviteReplyPacket {
    pub guild_id: u32,
    pub answer: i32,
}

/// `CZ_REQ_LEAVE_GUILD` (`0x0159`). 54 bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0159)]
pub struct LeaveGuildPacket {
    pub guild_id: u32,
    pub account_id: AccountId,
    pub character_id: CharacterId,
    #[length(40)]
    pub reason: String,
}

/// `CZ_REQ_BAN_GUILD` (`0x015B`). 54 bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x015B)]
pub struct ExpelGuildPacket {
    pub guild_id: u32,
    pub account_id: AccountId,
    pub character_id: CharacterId,
    #[length(40)]
    pub reason: String,
}

/// `CZ_REQ_GUILD_EMBLEM_IMG` (`0x0151`). 6 bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0151)]
pub struct RequestGuildEmblemPacket {
    pub guild_id: u32,
}

/// `CZ_REGISTER_GUILD_EMBLEM_IMG` (`0x0153`). Zlib-compressed 24×24 BMP.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0153)]
#[variable_length]
pub struct UploadGuildEmblemPacket {
    #[length_remaining]
    pub emblem: Vec<u8>,
}

/// One `CZ_REG_CHANGE_GUILD_POSITIONINFO` / `ZC_ACK_CHANGE_GUILD_POSITIONINFO` row (40 bytes).
#[derive(Debug, Clone, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct GuildPositionEdit {
    pub position_id: i32,
    pub mode: i32,
    pub ranking: i32,
    pub pay_rate: i32,
    #[length(24)]
    pub name: String,
}

/// `CZ_REG_CHANGE_GUILD_POSITIONINFO` (`0x0161`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0161)]
#[variable_length]
pub struct ChangeGuildPositionsPacket {
    #[repeating_remaining]
    pub positions: Vec<GuildPositionEdit>,
}

/// One `CZ_REQ_CHANGE_MEMBERPOS` / `ZC_ACK_REQ_CHANGE_MEMBERS` row (12 bytes).
#[derive(Debug, Clone, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct GuildMemberPositionEdit {
    pub account_id: AccountId,
    pub character_id: CharacterId,
    pub position_id: i32,
}

/// `CZ_REQ_CHANGE_MEMBERPOS` (`0x0155`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0155)]
#[variable_length]
pub struct ChangeGuildMemberPositionsPacket {
    #[repeating_remaining]
    pub members: Vec<GuildMemberPositionEdit>,
}

/// `CZ_GUILD_CHAT` (`0x017E`). Text is `Name : message`, same as public chat.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x017E)]
#[variable_length]
pub struct GuildChatPacket {
    #[length_remaining_off_by_one]
    pub message: String,
}

/// `CZ_REQ_DELETE_RELATED_GUILD` (`0x0183`). 10 bytes. Relation 0 ally, 1 enemy.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0183)]
pub struct BreakGuildRelationPacket {
    pub guild_id: u32,
    pub opposition: i32,
}

/// `ZC_RESULT_MAKE_GUILD` (`0x0167`). 3 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0167)]
pub struct GuildMakeResultPacket {
    pub result: u8,
}

/// `ZC_ACK_REQ_JOIN_GUILD` (`0x0169`). 3 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0169)]
pub struct GuildInviteResultPacket {
    pub result: u8,
}

/// `ZC_REQ_JOIN_GUILD` (`0x016A`). 30 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x016A)]
pub struct GuildInviteRequestPacket {
    pub guild_id: u32,
    #[length(24)]
    pub guild_name: String,
}

/// One `ZC_MYGUILD_BASIC_INFO` row (32 bytes). `relation` 0 is ally, 1 is enemy.
#[derive(Debug, Clone, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct GuildRelationRow {
    pub relation: i32,
    pub guild_id: i32,
    #[length(24)]
    pub name: String,
}

/// `ZC_MYGUILD_BASIC_INFO` (`0x014C`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x014C)]
#[variable_length]
pub struct GuildRelationsPacket {
    #[repeating_remaining]
    pub relations: Vec<GuildRelationRow>,
}

/// `ZC_DELETE_RELATED_GUILD` (`0x0184`). 10 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0184)]
pub struct GuildRelationRemovedPacket {
    pub guild_id: u32,
    pub relation: i32,
}

/// `ZC_ADD_RELATED_GUILD` (`0x0185`). 34 bytes. Hercules compiles the sender out;
/// the layout is still accepted.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0185)]
pub struct GuildRelationAddedPacket {
    pub relation: i32,
    pub guild_id: i32,
    #[length(24)]
    pub name: String,
}

/// One `ZC_POSITION_INFO` row (16 bytes).
#[derive(Debug, Clone, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct GuildPositionRightsRow {
    pub position_id: i32,
    pub mode: i32,
    pub ranking: i32,
    pub pay_rate: i32,
}

/// `ZC_POSITION_INFO` (`0x0160`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0160)]
#[variable_length]
pub struct GuildPositionInfoPacket {
    #[repeating_remaining]
    pub positions: Vec<GuildPositionRightsRow>,
}

/// `ZC_ACK_CHANGE_GUILD_POSITIONINFO` (`0x0174`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0174)]
#[variable_length]
pub struct GuildPositionChangedPacket {
    #[repeating_remaining]
    pub positions: Vec<GuildPositionEdit>,
}

/// `ZC_ACK_REQ_CHANGE_MEMBERS` (`0x0156`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0156)]
#[variable_length]
pub struct GuildMemberPositionChangedPacket {
    #[repeating_remaining]
    pub members: Vec<GuildMemberPositionEdit>,
}

/// One `ZC_GUILD_SKILLINFO` row (37 bytes).
#[derive(Debug, Clone, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct GuildSkillRow {
    pub skill_id: SkillId,
    pub kind: i32,
    pub level: u16,
    pub sp: u16,
    pub range: u16,
    #[length(24)]
    pub name: String,
    pub upgradable: u8,
}

/// `ZC_GUILD_SKILLINFO` (`0x0162`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0162)]
#[variable_length]
pub struct GuildSkillListPacket {
    pub skill_points: u16,
    #[repeating_remaining]
    pub skills: Vec<GuildSkillRow>,
}

/// `ZC_GUILD_CHAT` (`0x017F`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x017F)]
#[variable_length]
pub struct GuildChatReceivedPacket {
    #[length_remaining]
    pub message: String,
}

/// `ZC_GUILD_EMBLEM_IMG` (`0x0B36`) for PACKETVER_MAIN_NUM >= 20190821.
/// `result`: 0 clear, 1 add a chunk, 2 complete.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B36)]
#[variable_length]
pub struct GuildEmblemImagePacket {
    pub result: u16,
    pub guild_id: i32,
    pub emblem_id: u32,
    #[length_remaining]
    pub emblem_data: Vec<u8>,
}

/// Legacy `ZC_GUILD_EMBLEM_IMG` (`0x0152`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0152)]
#[variable_length]
pub struct LegacyGuildEmblemImagePacket {
    pub guild_id: i32,
    pub emblem_id: u32,
    #[length_remaining]
    pub emblem_data: Vec<u8>,
}

/// `ZC_CHANGE_GUILD` (`0x0B47`). 14 bytes. Sent to everyone except the subject.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B47)]
pub struct ChangeGuildPacket {
    pub guild_id: i32,
    pub emblem_id: u32,
    pub account_id: AccountId,
}

/// Legacy `ZC_CHANGE_GUILD` (`0x01B4`). 12 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01B4)]
pub struct LegacyChangeGuildPacket {
    pub account_id: AccountId,
    pub guild_id: i32,
    pub emblem_id: u16,
}

/// `ZC_MEMBER_ADD` (`0x0182`). 106 bytes. Classic row with a 50-byte intro.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0182)]
pub struct GuildMemberAddedPacket {
    pub account_id: AccountId,
    pub character_id: CharacterId,
    pub hair: i16,
    pub hair_color: i16,
    pub sex: i16,
    pub job: i16,
    pub level: i16,
    pub contribution_exp: i32,
    pub current_state: i32,
    pub position_id: i32,
    #[length(50)]
    pub intro: String,
    #[length(24)]
    pub name: String,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;
    use crate::PacketExt;

    fn round_trip_len<T: PacketExt>(packet: &T, expected: usize) -> Vec<u8> {
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.into_inner();
        assert_eq!(bytes.len(), expected, "{:04X}", T::HEADER.0);
        bytes
    }

    #[test]
    fn fixed_guild_packets_match_the_20250716_length_table() {
        let created = round_trip_len(
            &CreateGuildPacket {
                character_id: CharacterId(77),
                name: "Wolves".to_owned(),
            },
            30,
        );
        assert_eq!(&created[0..2], &[0x65, 0x01]);
        assert_eq!(u32::from_le_bytes(created[2..6].try_into().unwrap()), 77);
        assert_eq!(&created[6..12], b"Wolves");

        assert_eq!(
            round_trip_len(
                &GuildInviteByNamePacket {
                    name: "Beta".to_owned(),
                },
                26,
            )[0..2],
            [0x16, 0x09]
        );
        assert_eq!(
            round_trip_len(
                &GuildInviteByAccountPacket {
                    target_account_id: AccountId(4),
                    inviter_account_id: AccountId(3),
                    inviter_character_id: CharacterId(9),
                },
                14,
            )
            .len(),
            14
        );
        let reply = round_trip_len(
            &GuildInviteReplyPacket {
                guild_id: 9,
                answer: 1,
            },
            10,
        );
        assert_eq!(reply[0], 0x6B);
        assert_eq!(i32::from_le_bytes(reply[6..10].try_into().unwrap()), 1);

        let leave = round_trip_len(
            &LeaveGuildPacket {
                guild_id: 9,
                account_id: AccountId(3),
                character_id: CharacterId(77),
                reason: "bye".to_owned(),
            },
            54,
        );
        assert_eq!(leave[0], 0x59);
        assert_eq!(&leave[14..17], b"bye");

        assert_eq!(
            round_trip_len(
                &ExpelGuildPacket {
                    guild_id: 9,
                    account_id: AccountId(8),
                    character_id: CharacterId(8),
                    reason: "out".to_owned(),
                },
                54,
            )[0],
            0x5B
        );
        assert_eq!(
            round_trip_len(&RequestGuildEmblemPacket { guild_id: 9 }, 6)[0..2],
            [0x51, 0x01]
        );
        assert_eq!(round_trip_len(&GuildMakeResultPacket { result: 3 }, 3), vec![0x67, 0x01, 3]);
        assert_eq!(round_trip_len(&GuildInviteResultPacket { result: 2 }, 3), vec![0x69, 0x01, 2]);

        let invite = round_trip_len(
            &GuildInviteRequestPacket {
                guild_id: 9,
                guild_name: "Wolves".to_owned(),
            },
            30,
        );
        assert_eq!(invite[0], 0x6A);
        assert_eq!(&invite[6..12], b"Wolves");

        assert_eq!(
            round_trip_len(
                &BreakGuildRelationPacket {
                    guild_id: 4,
                    opposition: 1,
                },
                10,
            )[0],
            0x83
        );
        assert_eq!(
            round_trip_len(
                &GuildRelationRemovedPacket {
                    guild_id: 4,
                    relation: 0,
                },
                10,
            )[0],
            0x84
        );
        assert_eq!(
            round_trip_len(
                &GuildRelationAddedPacket {
                    relation: 1,
                    guild_id: 4,
                    name: "Axes".to_owned(),
                },
                34,
            )[0],
            0x85
        );
        let changed = round_trip_len(
            &ChangeGuildPacket {
                guild_id: 9,
                emblem_id: 4,
                account_id: AccountId(3),
            },
            14,
        );
        assert_eq!(&changed[0..2], &[0x47, 0x0B]);
        assert_eq!(i32::from_le_bytes(changed[2..6].try_into().unwrap()), 9);
        assert_eq!(u32::from_le_bytes(changed[6..10].try_into().unwrap()), 4);
        assert_eq!(u32::from_le_bytes(changed[10..14].try_into().unwrap()), 3);

        let legacy = round_trip_len(
            &LegacyChangeGuildPacket {
                account_id: AccountId(3),
                guild_id: 9,
                emblem_id: 4,
            },
            12,
        );
        assert_eq!(&legacy[0..2], &[0xB4, 0x01]);
        assert_eq!(u16::from_le_bytes(legacy[10..12].try_into().unwrap()), 4);

        let added = round_trip_len(
            &GuildMemberAddedPacket {
                account_id: AccountId(8),
                character_id: CharacterId(8),
                hair: 1,
                hair_color: 2,
                sex: 0,
                job: 7,
                level: 10,
                contribution_exp: 5,
                current_state: 1,
                position_id: 1,
                intro: String::new(),
                name: "Beta".to_owned(),
            },
            106,
        );
        assert_eq!(added[0], 0x82);
        // Header 2 + 4+4+2+2+2+2+2+4+4+4 + intro 50 = 82. Name follows the intro.
        assert_eq!(&added[82..86], b"Beta");
    }

    #[test]
    fn variable_guild_packets_round_trip_hercules_rows() {
        let relations = GuildRelationsPacket {
            relations: vec![
                GuildRelationRow {
                    relation: 0,
                    guild_id: 4,
                    name: "Archers".to_owned(),
                },
                GuildRelationRow {
                    relation: 1,
                    guild_id: 5,
                    name: "Axes".to_owned(),
                },
            ],
        };
        let bytes = round_trip_len(&relations, 4 + 64);
        assert_eq!(&bytes[0..2], &[0x4C, 0x01]);
        let mut reader = ByteReader::without_metadata(&bytes);
        let parsed = GuildRelationsPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.relations[0].relation, 0);
        assert_eq!(parsed.relations[0].name, "Archers");
        assert_eq!(parsed.relations[1].guild_id, 5);

        let rights = GuildPositionInfoPacket {
            positions: vec![GuildPositionRightsRow {
                position_id: 1,
                mode: 0x001,
                ranking: 1,
                pay_rate: 10,
            }],
        };
        let bytes = round_trip_len(&rights, 4 + 16);
        assert_eq!(bytes[0], 0x60);

        let edit = GuildPositionEdit {
            position_id: 1,
            mode: 0x011,
            ranking: 1,
            pay_rate: 15,
            name: "Raider".to_owned(),
        };
        let bytes = round_trip_len(
            &ChangeGuildPositionsPacket {
                positions: vec![edit.clone()],
            },
            4 + 40,
        );
        assert_eq!(&bytes[0..2], &[0x61, 0x01]);
        assert_eq!(&bytes[20..26], b"Raider");
        let changed = round_trip_len(&GuildPositionChangedPacket { positions: vec![edit] }, 44);
        assert_eq!(changed[0], 0x74);
        assert_eq!(u16::from_le_bytes([changed[2], changed[3]]), 44);

        let member = GuildMemberPositionEdit {
            account_id: AccountId(8),
            character_id: CharacterId(8),
            position_id: 2,
        };
        assert_eq!(
            round_trip_len(
                &ChangeGuildMemberPositionsPacket {
                    members: vec![member.clone()],
                },
                16,
            )[0],
            0x55
        );
        let moved = round_trip_len(&GuildMemberPositionChangedPacket { members: vec![member] }, 16);
        assert_eq!(moved[0], 0x56);
        assert_eq!(i32::from_le_bytes(moved[12..16].try_into().unwrap()), 2);

        let skills = GuildSkillListPacket {
            skill_points: 3,
            skills: vec![GuildSkillRow {
                skill_id: SkillId(10000),
                kind: 0,
                level: 1,
                sp: 0,
                range: 1,
                name: "Emergency Call".to_owned(),
                upgradable: 1,
            }],
        };
        let bytes = round_trip_len(&skills, 4 + 2 + 37);
        assert_eq!(&bytes[0..2], &[0x62, 0x01]);
        assert_eq!(u16::from_le_bytes(bytes[4..6].try_into().unwrap()), 3);
        assert_eq!(u16::from_le_bytes(bytes[6..8].try_into().unwrap()), 10000);
        assert_eq!(bytes[bytes.len() - 1], 1);

        let mut chat = vec![0x7F, 0x01, 0, 0];
        chat.extend_from_slice(b"Alpha : hello\0");
        let length = chat.len() as u16;
        chat[2..4].copy_from_slice(&length.to_le_bytes());
        let mut reader = ByteReader::without_metadata(&chat);
        let parsed = GuildChatReceivedPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.message, "Alpha : hello");

        let mut emblem = vec![0x36, 0x0B, 0, 0, 1, 0];
        emblem.extend_from_slice(&9i32.to_le_bytes());
        emblem.extend_from_slice(&4u32.to_le_bytes());
        emblem.extend_from_slice(&[0x78, 0x9C]);
        let length = emblem.len() as u16;
        emblem[2..4].copy_from_slice(&length.to_le_bytes());
        let mut reader = ByteReader::without_metadata(&emblem);
        let parsed = GuildEmblemImagePacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.result, 1);
        assert_eq!(parsed.guild_id, 9);
        assert_eq!(parsed.emblem_id, 4);
        assert_eq!(parsed.emblem_data, vec![0x78, 0x9C]);

        let upload = round_trip_len(
            &UploadGuildEmblemPacket {
                emblem: vec![1, 2, 3, 4],
            },
            8,
        );
        assert_eq!(&upload[0..2], &[0x53, 0x01]);
        assert_eq!(&upload[4..], &[1, 2, 3, 4]);
    }
}
