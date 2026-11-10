//! Outgoing guild actions. Incoming packets are registered in
//! `packet_versions::guild`.

use ragnarok_packets::handler::PacketCallback;
use ragnarok_packets::{
    AccountId, BreakGuildRelationPacket, ChangeGuildMemberPositionsPacket, ChangeGuildPositionsPacket, CharacterId, CreateGuildPacket,
    ExpelGuildPacket, GuildChatPacket, GuildInviteByAccountPacket, GuildInviteByNamePacket, GuildInviteReplyPacket,
    GuildMemberPositionEdit, GuildPositionEdit, LeaveGuildPacket, RequestGuildEmblemPacket, UploadGuildEmblemPacket,
};

use crate::server::NotConnectedError;
use crate::{NetworkingSystem, SupportedPacketVersion};

impl<Callback> NetworkingSystem<Callback>
where
    Callback: PacketCallback + Send,
{
    /// `CZ_REQ_MAKE_GUILD` (`0x0165`). Hercules ignores the character id and
    /// uses the session character.
    pub fn create_guild(&mut self, character_id: CharacterId, name: String) -> Result<(), NotConnectedError> {
        let name = super::clip_guild_text(name, 24);
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CreateGuildPacket { character_id, name })
            }
        }
    }

    /// `CZ_REQ_JOIN_GUILD2` (`0x0916`).
    pub fn invite_guild_by_name(&mut self, name: String) -> Result<(), NotConnectedError> {
        let name = super::clip_guild_text(name, 24);
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(GuildInviteByNamePacket { name })
            }
        }
    }

    /// `CZ_REQ_JOIN_GUILD` (`0x0168`). Used when the target account id is
    /// already known. The guild window invites by name.
    pub fn invite_guild_by_account(
        &mut self,
        target_account_id: AccountId,
        inviter_account_id: AccountId,
        inviter_character_id: CharacterId,
    ) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(GuildInviteByAccountPacket {
                    target_account_id,
                    inviter_account_id,
                    inviter_character_id,
                }),
        }
    }

    /// `CZ_JOIN_GUILD` (`0x016B`). `answer` 0 refuses, 1 accepts.
    pub fn reply_guild_invite(&mut self, guild_id: u32, answer: i32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(GuildInviteReplyPacket { guild_id, answer })
            }
        }
    }

    /// `CZ_REQ_LEAVE_GUILD` (`0x0159`).
    pub fn leave_guild(
        &mut self,
        guild_id: u32,
        account_id: AccountId,
        character_id: CharacterId,
        reason: String,
    ) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(LeaveGuildPacket {
                    guild_id,
                    account_id,
                    character_id,
                    reason: super::clip_guild_text(reason, 40),
                }),
        }
    }

    /// `CZ_REQ_BAN_GUILD` (`0x015B`).
    pub fn expel_guild_member(
        &mut self,
        guild_id: u32,
        account_id: AccountId,
        character_id: CharacterId,
        reason: String,
    ) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(ExpelGuildPacket {
                    guild_id,
                    account_id,
                    character_id,
                    reason: super::clip_guild_text(reason, 40),
                }),
        }
    }

    /// `CZ_REG_CHANGE_GUILD_POSITIONINFO` (`0x0161`).
    pub fn change_guild_positions(&mut self, positions: Vec<GuildPositionEdit>) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ChangeGuildPositionsPacket { positions })
            }
        }
    }

    /// `CZ_REQ_CHANGE_MEMBERPOS` (`0x0155`). One row with position 0 transfers
    /// guild master.
    pub fn change_guild_member_position(
        &mut self,
        account_id: AccountId,
        character_id: CharacterId,
        position_id: i32,
    ) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(ChangeGuildMemberPositionsPacket {
                    members: vec![GuildMemberPositionEdit {
                        account_id,
                        character_id,
                        position_id,
                    }],
                }),
        }
    }

    /// `CZ_REQ_GUILD_EMBLEM_IMG` (`0x0151`).
    pub fn request_guild_emblem(&mut self, guild_id: u32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(RequestGuildEmblemPacket { guild_id })
            }
        }
    }

    /// `CZ_REGISTER_GUILD_EMBLEM_IMG` (`0x0153`). `emblem` is zlib-compressed
    /// BMP.
    pub fn upload_guild_emblem(&mut self, emblem: Vec<u8>) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(UploadGuildEmblemPacket { emblem })
            }
        }
    }

    /// `CZ_GUILD_CHAT` (`0x017E`). `text` is the message body. The speaker name
    /// must match the character or Hercules drops the packet.
    pub fn send_guild_chat(&mut self, player_name: &str, text: &str) -> Result<(), NotConnectedError> {
        let text = super::clip_guild_text(text.to_owned(), 70);
        let message = format!("{player_name} : {text}");
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(GuildChatPacket { message })
            }
        }
    }

    /// `CZ_REQ_DELETE_RELATED_GUILD` (`0x0183`).
    pub fn break_guild_relation(&mut self, guild_id: u32, opposition: i32) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(BreakGuildRelationPacket { guild_id, opposition })
            }
        }
    }
}
