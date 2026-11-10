//! Party packets for PACKETVER 20250716 RE.
//!
//! Layouts follow Hercules `packets_struct.h` / `clif.c`. On this packetver
//! `ZC_GROUP_LIST` is `0x0AE5` and `ZC_ADD_MEMBER_TO_GROUP` is `0x0AE4`.
//! Invite-by-name is `CZ_PARTY_JOIN_REQ` `0x02C4` (shuffle_re restores it for
//! PACKETVER >= 20211117). Leader notification is `0x07FC`.

use ragnarok_bytes::{ByteConvertable, FixedByteSize, FromBytes, ToBytes};
use ragnarok_macros::{ClientPacket, MapServer, Packet, ServerPacket};

use crate::{AccountId, CharacterId};

/// `CZ_MAKE_GROUP2` (`0x01E8`): name, item pickup, item division. 28 bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01E8)]
pub struct CreatePartyPacket {
    #[length(24)]
    pub name: String,
    pub item_pickup: u8,
    pub item_division: u8,
}

/// `CZ_PARTY_JOIN_REQ` (`0x02C4`): character name. 26 bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02C4)]
pub struct PartyInviteRequestPacket {
    #[length(24)]
    pub character_name: String,
}

/// `CZ_REQ_LEAVE_GROUP` (`0x0100`). 2 bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0100)]
pub struct LeavePartyPacket {}

/// `CZ_REQ_EXPEL_GROUP_MEMBER` (`0x0103`): account id and character name. 30
/// bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0103)]
pub struct ExpelPartyMemberPacket {
    pub account_id: AccountId,
    #[length(24)]
    pub character_name: String,
}

/// `CZ_GROUPINFO_CHANGE_V2` (`0x07D7`). 8 bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x07D7)]
pub struct ChangePartyOptionsPacket {
    pub experience_share: u32,
    pub item_pickup: u8,
    pub item_division: u8,
}

/// `CZ_CHANGE_GROUP_MASTER` (`0x07DA`). 6 bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x07DA)]
pub struct ChangePartyLeaderPacket {
    pub account_id: AccountId,
}

/// `CZ_REQUEST_CHAT_PARTY` (`0x0108`). Length matches global chat.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0108)]
#[variable_length]
pub struct PartyMessagePacket {
    #[length_remaining_off_by_one]
    pub message: String,
}

/// `CZ_PARTY_CONFIG` (`0x02C8`). `refuse_invite` 0 allows invites.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02C8)]
pub struct PartyConfigPacket {
    pub refuse_invite: u8,
}

/// `ZC_ACK_MAKE_GROUP` (`0x00FA`). 0 created, 1 name exists, 2 already in a
/// party, 3 cannot organize.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00FA)]
pub struct PartyCreateResultPacket {
    pub result: u8,
}

/// One `ZC_GROUP_LIST` member. 54 bytes on PACKETVER >= 20171207.
/// `leader` 0 is the party leader. `offline` 0 means online.
#[derive(Debug, Clone, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct PartyListMember {
    pub account_id: AccountId,
    pub character_id: CharacterId,
    #[length(24)]
    pub name: String,
    #[length(16)]
    pub map_name: String,
    pub leader: u8,
    pub offline: u8,
    pub job: i16,
    pub base_level: i16,
}

/// `ZC_GROUP_LIST` (`0x0AE5`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0AE5)]
#[variable_length]
pub struct PartyListPacket {
    #[length(24)]
    pub party_name: String,
    #[repeating_remaining]
    pub members: Vec<PartyListMember>,
}

/// `ZC_ADD_MEMBER_TO_GROUP` (`0x0AE4`). 89 bytes.
/// `leader` 0 is the party leader. `offline` 0 means online.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0AE4)]
pub struct PartyMemberAddedPacket {
    pub account_id: AccountId,
    pub character_id: CharacterId,
    pub leader: u32,
    pub job: i16,
    pub base_level: i16,
    pub x: i16,
    pub y: i16,
    pub offline: u8,
    #[length(24)]
    pub party_name: String,
    #[length(24)]
    pub player_name: String,
    #[length(16)]
    pub map_name: String,
    pub share_pickup: u8,
    pub share_loot: u8,
}

/// `ZC_DELETE_MEMBER_FROM_GROUP` (`0x0105`). 31 bytes.
/// `result` 0 leave, 1 expel, 2 cannot leave, 3 cannot expel.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0105)]
pub struct PartyMemberRemovedPacket {
    pub account_id: AccountId,
    #[length(24)]
    pub name: String,
    pub result: u8,
}

/// `ZC_GROUPINFO_CHANGE_V2` (`0x07D8`). 8 bytes.
/// Experience share 0 off, 1 on, 2 locked.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x07D8)]
pub struct PartyOptionsPacket {
    pub experience_share: u32,
    pub item_pickup: u8,
    pub item_division: u8,
}

/// `ZC_CHANGE_GROUPEXPOPTION` (`0x0101`). 6 bytes. Experience share only.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0101)]
pub struct PartyExpSharePacket {
    pub experience_share: u32,
}

/// `ZC_CHANGE_GROUP_MASTER` (`0x07FC`). 10 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x07FC)]
pub struct PartyLeaderChangedPacket {
    pub previous_leader: AccountId,
    pub new_leader: AccountId,
}

/// `ZC_NOTIFY_HP_TO_GROUPM` (`0x080E`). 14 bytes. No SP on 20250716 RE.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x080E)]
pub struct PartyMemberHealthPacket {
    pub account_id: AccountId,
    pub health: i32,
    pub maximum_health: i32,
}

/// `ZC_NOTIFY_POSITION_TO_GROUPM` (`0x0107`). 10 bytes.
/// Negative coordinates remove the minimap mark.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0107)]
pub struct PartyMemberPositionPacket {
    pub account_id: AccountId,
    pub x: i16,
    pub y: i16,
}

/// `ZC_NOTIFY_CHAT_PARTY` (`0x0109`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0109)]
#[variable_length]
pub struct PartyChatPacket {
    pub account_id: AccountId,
    #[length_remaining]
    pub message: String,
}

/// `ZC_GROUP_ISALIVE` (`0x0AB2`). 7 bytes. `dead` 1 means the member is dead.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0AB2)]
pub struct PartyMemberAlivePacket {
    pub account_id: AccountId,
    pub dead: u8,
}

/// `ZC_NOTIFY_MEMBERINFO_TO_GROUPM` (`0x0ABD`). 10 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0ABD)]
pub struct PartyMemberJobLevelPacket {
    pub account_id: AccountId,
    pub job: i16,
    pub base_level: i16,
}

/// `ZC_PARTY_JOIN_REQ_ACK` (`0x02C5`). 30 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02C5)]
pub struct PartyInviteResultPacket {
    #[length(24)]
    pub character_name: String,
    pub result: i32,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;
    use crate::PacketExt;

    fn parse<T: crate::Packet>(bytes: &[u8]) -> T {
        let mut reader = ByteReader::without_metadata(bytes);
        let packet = T::packet_from_bytes(&mut reader).expect("parse party packet");
        assert!(reader.remaining_bytes().is_empty());
        packet
    }

    fn write<T: crate::Packet>(packet: &T) -> Vec<u8> {
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).expect("write party packet");
        writer.into_inner()
    }

    fn push_fixed(buffer: &mut Vec<u8>, text: &str, size: usize) {
        let start = buffer.len();
        buffer.extend(text.as_bytes());
        buffer.push(0);
        assert!(buffer.len() - start <= size, "text does not fit");
        buffer.resize(start + size, 0);
    }

    #[test]
    fn group_list_one_member_is_82_bytes() {
        let mut body = Vec::new();
        push_fixed(&mut body, "Wolves", 24);
        body.extend(7u32.to_le_bytes());
        body.extend(9u32.to_le_bytes());
        push_fixed(&mut body, "Nova", 24);
        push_fixed(&mut body, "prontera.gat", 16);
        body.push(0);
        body.push(0);
        body.extend(7i16.to_le_bytes());
        body.extend(50i16.to_le_bytes());
        let packet_len = (4 + body.len()) as u16;
        let mut bytes = Vec::new();
        bytes.extend(0x0AE5u16.to_le_bytes());
        bytes.extend(packet_len.to_le_bytes());
        bytes.extend(body);
        assert_eq!(bytes.len(), 82);

        let packet = parse::<PartyListPacket>(&bytes);
        assert_eq!(packet.party_name, "Wolves");
        assert_eq!(packet.members.len(), 1);
        assert_eq!(packet.members[0].account_id, AccountId(7));
        assert_eq!(packet.members[0].character_id, CharacterId(9));
        assert_eq!(packet.members[0].name, "Nova");
        assert_eq!(packet.members[0].map_name, "prontera.gat");
        assert_eq!(packet.members[0].leader, 0);
        assert_eq!(packet.members[0].offline, 0);
        assert_eq!(packet.members[0].job, 7);
        assert_eq!(packet.members[0].base_level, 50);
    }

    #[test]
    fn add_member_is_89_bytes() {
        let mut bytes = Vec::new();
        bytes.extend(0x0AE4u16.to_le_bytes());
        bytes.extend(11u32.to_le_bytes());
        bytes.extend(22u32.to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(8i16.to_le_bytes());
        bytes.extend(40i16.to_le_bytes());
        bytes.extend(100i16.to_le_bytes());
        bytes.extend(120i16.to_le_bytes());
        bytes.push(0);
        push_fixed(&mut bytes, "Wolves", 24);
        push_fixed(&mut bytes, "Nova", 24);
        push_fixed(&mut bytes, "prontera.gat", 16);
        bytes.push(1);
        bytes.push(0);
        assert_eq!(bytes.len(), 89);

        let packet = parse::<PartyMemberAddedPacket>(&bytes);
        assert_eq!(packet.account_id, AccountId(11));
        assert_eq!(packet.character_id, CharacterId(22));
        assert_eq!(packet.leader, 0);
        assert_eq!(packet.job, 8);
        assert_eq!(packet.base_level, 40);
        assert_eq!(packet.x, 100);
        assert_eq!(packet.y, 120);
        assert_eq!(packet.offline, 0);
        assert_eq!(packet.party_name, "Wolves");
        assert_eq!(packet.player_name, "Nova");
        assert_eq!(packet.map_name, "prontera.gat");
        assert_eq!(packet.share_pickup, 1);
        assert_eq!(packet.share_loot, 0);
    }

    #[test]
    fn fixed_server_packets_match_hercules_sizes() {
        let removed = parse::<PartyMemberRemovedPacket>(&{
            let mut bytes = Vec::new();
            bytes.extend(0x0105u16.to_le_bytes());
            bytes.extend(4u32.to_le_bytes());
            push_fixed(&mut bytes, "Nova", 24);
            bytes.push(2);
            assert_eq!(bytes.len(), 31);
            bytes
        });
        assert_eq!(removed.account_id, AccountId(4));
        assert_eq!(removed.name, "Nova");
        assert_eq!(removed.result, 2);

        let health = parse::<PartyMemberHealthPacket>(&{
            let mut bytes = vec![0x0E, 0x08];
            bytes.extend(7u32.to_le_bytes());
            bytes.extend(40i32.to_le_bytes());
            bytes.extend(100i32.to_le_bytes());
            assert_eq!(bytes.len(), 14);
            bytes
        });
        assert_eq!(health.health, 40);
        assert_eq!(health.maximum_health, 100);

        let position = parse::<PartyMemberPositionPacket>(&{
            let mut bytes = vec![0x07, 0x01];
            bytes.extend(7u32.to_le_bytes());
            bytes.extend((-1i16).to_le_bytes());
            bytes.extend(12i16.to_le_bytes());
            assert_eq!(bytes.len(), 10);
            bytes
        });
        assert_eq!(position.x, -1);
        assert_eq!(position.y, 12);

        let options = parse::<PartyOptionsPacket>(&{
            let mut bytes = vec![0xD8, 0x07];
            bytes.extend(2u32.to_le_bytes());
            bytes.push(1);
            bytes.push(0);
            assert_eq!(bytes.len(), 8);
            bytes
        });
        assert_eq!(options.experience_share, 2);
        assert_eq!(options.item_pickup, 1);

        let exp = parse::<PartyExpSharePacket>(&{
            let mut bytes = vec![0x01, 0x01];
            bytes.extend(1u32.to_le_bytes());
            assert_eq!(bytes.len(), 6);
            bytes
        });
        assert_eq!(exp.experience_share, 1);

        let leader = parse::<PartyLeaderChangedPacket>(&{
            let mut bytes = vec![0xFC, 0x07];
            bytes.extend(1u32.to_le_bytes());
            bytes.extend(2u32.to_le_bytes());
            assert_eq!(bytes.len(), 10);
            bytes
        });
        assert_eq!(leader.previous_leader, AccountId(1));
        assert_eq!(leader.new_leader, AccountId(2));

        let alive = parse::<PartyMemberAlivePacket>(&{
            let mut bytes = vec![0xB2, 0x0A];
            bytes.extend(9u32.to_le_bytes());
            bytes.push(1);
            assert_eq!(bytes.len(), 7);
            bytes
        });
        assert_eq!(alive.dead, 1);

        let job = parse::<PartyMemberJobLevelPacket>(&{
            let mut bytes = vec![0xBD, 0x0A];
            bytes.extend(9u32.to_le_bytes());
            bytes.extend(14i16.to_le_bytes());
            bytes.extend(55i16.to_le_bytes());
            assert_eq!(bytes.len(), 10);
            bytes
        });
        assert_eq!(job.job, 14);
        assert_eq!(job.base_level, 55);

        let created = parse::<PartyCreateResultPacket>(&[0xFA, 0x00, 1]);
        assert_eq!(created.result, 1);

        let invite = parse::<PartyInviteResultPacket>(&{
            let mut bytes = vec![0xC5, 0x02];
            push_fixed(&mut bytes, "Nova", 24);
            bytes.extend(7i32.to_le_bytes());
            assert_eq!(bytes.len(), 30);
            bytes
        });
        assert_eq!(invite.character_name, "Nova");
        assert_eq!(invite.result, 7);
    }

    #[test]
    fn party_chat_keeps_account_and_message() {
        let message = "Nova : hello";
        let packet_len = (8 + message.len() + 1) as u16;
        let mut bytes = Vec::new();
        bytes.extend(0x0109u16.to_le_bytes());
        bytes.extend(packet_len.to_le_bytes());
        bytes.extend(15u32.to_le_bytes());
        bytes.extend(message.as_bytes());
        bytes.push(0);

        let packet = parse::<PartyChatPacket>(&bytes);
        assert_eq!(packet.account_id, AccountId(15));
        assert_eq!(packet.message, message);
    }

    #[test]
    fn client_packets_match_hercules_sizes() {
        let created = write(&CreatePartyPacket {
            name: "Wolves".to_owned(),
            item_pickup: 1,
            item_division: 0,
        });
        assert_eq!(created.len(), 28);
        assert_eq!(u16::from_le_bytes([created[0], created[1]]), 0x01E8);
        assert_eq!(created[26], 1);
        assert_eq!(created[27], 0);

        let invite = write(&PartyInviteRequestPacket {
            character_name: "Nova".to_owned(),
        });
        assert_eq!(invite.len(), 26);
        assert_eq!(u16::from_le_bytes([invite[0], invite[1]]), 0x02C4);
        assert_eq!(&invite[2..6], b"Nova");
        assert_eq!(invite[6], 0);

        let leave = write(&LeavePartyPacket {});
        assert_eq!(leave, [0x00, 0x01]);

        let expel = write(&ExpelPartyMemberPacket {
            account_id: AccountId(4),
            character_name: "Nova".to_owned(),
        });
        assert_eq!(expel.len(), 30);
        assert_eq!(u16::from_le_bytes([expel[0], expel[1]]), 0x0103);
        assert_eq!(u32::from_le_bytes(expel[2..6].try_into().unwrap()), 4);

        let options = write(&ChangePartyOptionsPacket {
            experience_share: 1,
            item_pickup: 1,
            item_division: 1,
        });
        assert_eq!(options.len(), 8);
        assert_eq!(u16::from_le_bytes([options[0], options[1]]), 0x07D7);
        assert_eq!(options[6], 1);
        assert_eq!(options[7], 1);

        let leader = write(&ChangePartyLeaderPacket { account_id: AccountId(8) });
        assert_eq!(leader.len(), 6);
        assert_eq!(u16::from_le_bytes([leader[0], leader[1]]), 0x07DA);
        assert_eq!(u32::from_le_bytes(leader[2..6].try_into().unwrap()), 8);

        let config = write(&PartyConfigPacket { refuse_invite: 1 });
        assert_eq!(config, [0xC8, 0x02, 1]);

        let chat = write(&PartyMessagePacket {
            message: "Nova : hello".to_owned(),
        });
        assert_eq!(u16::from_le_bytes([chat[0], chat[1]]), 0x0108);
        let declared = u16::from_le_bytes([chat[2], chat[3]]);
        assert_eq!(declared as usize, chat.len());
        assert!(chat.ends_with(&[0]));
        assert!(chat.windows(b"Nova : hello".len()).any(|window| window == b"Nova : hello"));
    }
}
