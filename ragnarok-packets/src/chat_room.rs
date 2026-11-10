//! Chat room packets (`0x00D5`..`0x00E3`). Layouts follow Hercules
//! `clif.c`; they are the same on every supported packetver.

use ragnarok_bytes::{ByteConvertable, FixedByteSize, FromBytes, ToBytes};
use ragnarok_macros::{ClientPacket, MapServer, Packet, ServerPacket};

use crate::AccountId;

/// Server-side chat room id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct ChatRoomId(pub u32);

/// `CZ_CREATE_CHATROOM` (`0x00D5`). `public` is 1 for an open room.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00D5)]
#[variable_length]
pub struct CreateChatRoomPacket {
    pub limit: u16,
    pub public: u8,
    #[length(8)]
    pub password: String,
    #[length_remaining]
    pub title: String,
}

/// `ZC_ACK_CREATE_CHATROOM` (`0x00D6`). 0 is success.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00D6)]
pub struct CreateChatRoomResultPacket {
    pub result: u8,
}

/// `ZC_ROOM_NEWENTRY` (`0x00D7`): the board over a room owner.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00D7)]
#[variable_length]
pub struct ChatRoomEntryPacket {
    pub owner_id: AccountId,
    pub chat_id: ChatRoomId,
    pub limit: u16,
    pub users: u16,
    pub public: u8,
    #[length_remaining]
    pub title: String,
}

/// `ZC_DESTROY_ROOM` (`0x00D8`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00D8)]
pub struct ChatRoomDestroyedPacket {
    pub chat_id: ChatRoomId,
}

/// `CZ_REQ_ENTER_ROOM` (`0x00D9`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00D9)]
pub struct EnterChatRoomPacket {
    pub chat_id: ChatRoomId,
    #[length(8)]
    pub password: String,
}

/// `ZC_REFUSE_ENTER_ROOM` (`0x00DA`). 0 room full, 1 wrong password, 2
/// kicked, 3 success (unused), 4 no zeny, 5 level too low, 6 level too high,
/// 7 job not allowed.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00DA)]
pub struct ChatRoomRefusedPacket {
    pub result: u8,
}

/// One member row of `ZC_ENTER_ROOM`. `role` 0 is the owner.
#[derive(Debug, Clone, ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct ChatRoomMemberInformation {
    pub role: u32,
    #[length(24)]
    pub name: String,
}

/// `ZC_ENTER_ROOM` (`0x00DB`): the member list after joining or creating.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00DB)]
#[variable_length]
pub struct ChatRoomEnteredPacket {
    pub chat_id: ChatRoomId,
    #[repeating_remaining]
    pub members: Vec<ChatRoomMemberInformation>,
}

/// `ZC_MEMBER_NEWENTRY` (`0x00DC`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00DC)]
pub struct ChatRoomMemberJoinedPacket {
    pub users: u16,
    #[length(24)]
    pub name: String,
}

/// `ZC_MEMBER_EXIT` (`0x00DD`). `kicked` is 1 when the owner expelled them.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00DD)]
pub struct ChatRoomMemberLeftPacket {
    pub users: u16,
    #[length(24)]
    pub name: String,
    pub kicked: u8,
}

/// `CZ_CHANGE_CHATROOM` (`0x00DE`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00DE)]
#[variable_length]
pub struct ChangeChatRoomPacket {
    pub limit: u16,
    pub public: u8,
    #[length(8)]
    pub password: String,
    #[length_remaining]
    pub title: String,
}

/// `ZC_CHANGE_CHATROOM` (`0x00DF`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00DF)]
#[variable_length]
pub struct ChatRoomChangedPacket {
    pub owner_id: AccountId,
    pub chat_id: ChatRoomId,
    pub limit: u16,
    pub users: u16,
    pub public: u8,
    #[length_remaining]
    pub title: String,
}

/// `CZ_REQ_ROLE_CHANGE` (`0x00E0`): hand the room to `name` (role 0).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00E0)]
pub struct ChangeChatRoomOwnerPacket {
    pub role: u32,
    #[length(24)]
    pub name: String,
}

/// `ZC_ROLE_CHANGE` (`0x00E1`). `role` 0 is the new owner.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00E1)]
pub struct ChatRoomRoleChangedPacket {
    pub role: u32,
    #[length(24)]
    pub name: String,
}

/// `CZ_REQ_EXPEL_MEMBER` (`0x00E2`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00E2)]
pub struct ExpelChatRoomMemberPacket {
    #[length(24)]
    pub name: String,
}

/// `CZ_EXIT_ROOM` (`0x00E3`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00E3)]
pub struct LeaveChatRoomPacket {}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;
    use crate::PacketExt;

    fn parse<T: crate::Packet>(bytes: &[u8]) -> T {
        let mut reader = ByteReader::without_metadata(bytes);
        let packet = T::packet_from_bytes(&mut reader).expect("parse chat room packet");
        assert!(reader.remaining_bytes().is_empty());
        packet
    }

    fn write<T: crate::Packet>(packet: &T) -> Vec<u8> {
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).expect("write chat room packet");
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
    fn create_room_carries_limit_password_and_title() {
        let bytes = write(&CreateChatRoomPacket {
            limit: 20,
            public: 1,
            password: String::new(),
            title: "Selling cards".to_owned(),
        });
        assert_eq!(&bytes[0..2], &0x00D5u16.to_le_bytes());
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]) as usize, bytes.len());
        assert_eq!(&bytes[4..6], &20u16.to_le_bytes());
        assert_eq!(bytes[6], 1);
        assert_eq!(&bytes[7..15], &[0; 8]);
        // The writer ends the title with a NUL; Hercules copies up to it.
        assert_eq!(&bytes[15..], b"Selling cards\0");
    }

    #[test]
    fn room_board_parses_owner_counts_and_title() {
        let mut bytes = Vec::new();
        bytes.extend(0x00D7u16.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes.extend(2000001u32.to_le_bytes());
        bytes.extend(7u32.to_le_bytes());
        bytes.extend(20u16.to_le_bytes());
        bytes.extend(3u16.to_le_bytes());
        bytes.push(0);
        bytes.extend(b"Party up");
        let len = bytes.len() as u16;
        bytes[2..4].copy_from_slice(&len.to_le_bytes());

        let packet = parse::<ChatRoomEntryPacket>(&bytes);
        assert_eq!(packet.owner_id, AccountId(2000001));
        assert_eq!(packet.chat_id, ChatRoomId(7));
        assert_eq!((packet.limit, packet.users, packet.public), (20, 3, 0));
        assert_eq!(packet.title, "Party up");
    }

    #[test]
    fn member_list_is_28_bytes_per_member() {
        let mut bytes = Vec::new();
        bytes.extend(0x00DBu16.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes.extend(7u32.to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        push_fixed(&mut bytes, "Owner", 24);
        bytes.extend(1u32.to_le_bytes());
        push_fixed(&mut bytes, "Guest", 24);
        let len = bytes.len() as u16;
        bytes[2..4].copy_from_slice(&len.to_le_bytes());
        assert_eq!(bytes.len(), 8 + 2 * 28);

        let packet = parse::<ChatRoomEnteredPacket>(&bytes);
        assert_eq!(packet.chat_id, ChatRoomId(7));
        assert_eq!(packet.members.len(), 2);
        assert_eq!((packet.members[0].role, packet.members[0].name.as_str()), (0, "Owner"));
        assert_eq!((packet.members[1].role, packet.members[1].name.as_str()), (1, "Guest"));
    }

    #[test]
    fn fixed_sizes_match_the_framing_table() {
        let enter = write(&EnterChatRoomPacket {
            chat_id: ChatRoomId(7),
            password: "secret".to_owned(),
        });
        assert_eq!(enter.len(), 14);
        let owner = write(&ChangeChatRoomOwnerPacket {
            role: 0,
            name: "Guest".to_owned(),
        });
        assert_eq!(owner.len(), 30);
        assert_eq!(write(&ExpelChatRoomMemberPacket { name: "Guest".to_owned() }).len(), 26);
        assert_eq!(write(&LeaveChatRoomPacket {}).len(), 2);

        let mut left = Vec::new();
        left.extend(0x00DDu16.to_le_bytes());
        left.extend(1u16.to_le_bytes());
        push_fixed(&mut left, "Guest", 24);
        left.push(1);
        assert_eq!(left.len(), 29);
        let packet = parse::<ChatRoomMemberLeftPacket>(&left);
        assert_eq!((packet.users, packet.name.as_str(), packet.kicked), (1, "Guest", 1));
    }
}
