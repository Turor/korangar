//! Outgoing chat room actions. Incoming packets are registered in
//! `packet_versions::chat_room`.

use ragnarok_packets::handler::PacketCallback;
use ragnarok_packets::{
    ChangeChatRoomOwnerPacket, ChangeChatRoomPacket, ChatRoomId, CreateChatRoomPacket, EnterChatRoomPacket, ExpelChatRoomMemberPacket,
    LeaveChatRoomPacket,
};

use crate::server::NotConnectedError;
use crate::{NetworkingSystem, SupportedPacketVersion};

/// Room title limit on Hercules (`CHATROOM_TITLE_SIZE` minus the NUL).
pub const CHAT_ROOM_TITLE_LENGTH: usize = 36;
/// Room password limit (`CHATROOM_PASS_SIZE` minus the NUL).
pub const CHAT_ROOM_PASSWORD_LENGTH: usize = 7;
/// Member limits Hercules accepts.
pub const CHAT_ROOM_LIMITS: std::ops::RangeInclusive<u16> = 2..=20;

/// Trim and cut to `max` bytes on a character boundary.
pub fn clip_chat_room_text(text: &str, max: usize) -> String {
    let text = text.trim();
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

impl<Callback> NetworkingSystem<Callback>
where
    Callback: PacketCallback + Send,
{
    /// `CZ_CREATE_CHATROOM` (`0x00D5`).
    pub fn create_chat_room(&mut self, title: &str, limit: u16, public: bool, password: &str) -> Result<(), NotConnectedError> {
        let packet = CreateChatRoomPacket {
            limit: limit.clamp(*CHAT_ROOM_LIMITS.start(), *CHAT_ROOM_LIMITS.end()),
            public: u8::from(public),
            password: clip_chat_room_text(password, CHAT_ROOM_PASSWORD_LENGTH),
            title: clip_chat_room_text(title, CHAT_ROOM_TITLE_LENGTH),
        };
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(packet)
            }
        }
    }

    /// `CZ_CHANGE_CHATROOM` (`0x00DE`). Owner only.
    pub fn change_chat_room(&mut self, title: &str, limit: u16, public: bool, password: &str) -> Result<(), NotConnectedError> {
        let packet = ChangeChatRoomPacket {
            limit: limit.clamp(*CHAT_ROOM_LIMITS.start(), *CHAT_ROOM_LIMITS.end()),
            public: u8::from(public),
            password: clip_chat_room_text(password, CHAT_ROOM_PASSWORD_LENGTH),
            title: clip_chat_room_text(title, CHAT_ROOM_TITLE_LENGTH),
        };
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(packet)
            }
        }
    }

    /// `CZ_REQ_ENTER_ROOM` (`0x00D9`).
    pub fn enter_chat_room(&mut self, chat_id: ChatRoomId, password: &str) -> Result<(), NotConnectedError> {
        let password = clip_chat_room_text(password, CHAT_ROOM_PASSWORD_LENGTH);
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(EnterChatRoomPacket { chat_id, password })
            }
        }
    }

    /// `CZ_REQ_ROLE_CHANGE` (`0x00E0`): hand the room to `name`.
    pub fn give_chat_room_ownership(&mut self, name: &str) -> Result<(), NotConnectedError> {
        let name = clip_chat_room_text(name, 23);
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ChangeChatRoomOwnerPacket { role: 0, name })
            }
        }
    }

    /// `CZ_REQ_EXPEL_MEMBER` (`0x00E2`). Owner only.
    pub fn expel_chat_room_member(&mut self, name: &str) -> Result<(), NotConnectedError> {
        let name = clip_chat_room_text(name, 23);
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ExpelChatRoomMemberPacket { name })
            }
        }
    }

    /// `CZ_EXIT_ROOM` (`0x00E3`).
    pub fn leave_chat_room(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(LeaveChatRoomPacket {})
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::clip_chat_room_text;

    #[test]
    fn room_text_is_trimmed_and_clipped_on_a_character_boundary() {
        assert_eq!(clip_chat_room_text("  Selling cards  ", 36), "Selling cards");
        assert_eq!(clip_chat_room_text("abcdefghij", 7), "abcdefg");
        assert_eq!(clip_chat_room_text("가나다", 4), "가");
    }
}
