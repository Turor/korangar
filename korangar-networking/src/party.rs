//! Party updates and the map-server packets that send them.

use ragnarok_packets::handler::PacketCallback;
use ragnarok_packets::{
    AccountId, ChangePartyLeaderPacket, ChangePartyOptionsPacket, CharacterId, CreatePartyPacket, ExpelPartyMemberPacket, LeavePartyPacket,
    PartyConfigPacket, PartyInviteRequestPacket, PartyMessagePacket,
};

use crate::NetworkingSystem;
use crate::packet_versions::SupportedPacketVersion;
use crate::server::NotConnectedError;

/// One roster row from `ZC_GROUP_LIST` or `ZC_ADD_MEMBER_TO_GROUP`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRosterEntry {
    pub account_id: AccountId,
    pub character_id: CharacterId,
    pub name: String,
    pub map_name: String,
    pub leader: bool,
    pub online: bool,
    pub job: i16,
    pub base_level: i16,
    pub x: i16,
    pub y: i16,
    /// Set when this packet carried a map coordinate. List packets do not.
    pub has_position: bool,
}

/// Server party packet reduced to client state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyUpdate {
    CreateResult(u8),
    Roster {
        name: String,
        members: Vec<PartyRosterEntry>,
    },
    MemberAdded {
        party_name: String,
        share_pickup: bool,
        share_loot: bool,
        member: PartyRosterEntry,
    },
    MemberRemoved {
        account_id: AccountId,
        name: String,
        result: u8,
    },
    Options {
        experience_share: u32,
        item_pickup: bool,
        item_division: bool,
    },
    ExperienceShare(u32),
    Leader {
        previous: AccountId,
        new_leader: AccountId,
    },
    Health {
        account_id: AccountId,
        health: i32,
        maximum: i32,
    },
    Position {
        account_id: AccountId,
        x: i16,
        y: i16,
    },
    JobLevel {
        account_id: AccountId,
        job: i16,
        level: i16,
    },
    Alive {
        account_id: AccountId,
        dead: bool,
    },
    InviteResult {
        name: String,
        result: i32,
    },
}

fn clip_name(name: &str) -> String {
    name.chars().take(23).collect()
}

impl<Callback> NetworkingSystem<Callback>
where
    Callback: PacketCallback + Send,
{
    pub fn create_party(&mut self, name: String, item_pickup: bool, item_division: bool) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(CreatePartyPacket {
                    name: clip_name(&name),
                    item_pickup: u8::from(item_pickup),
                    item_division: u8::from(item_division),
                }),
        }
    }

    pub fn invite_to_party(&mut self, character_name: String) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(PartyInviteRequestPacket {
                    character_name: clip_name(&character_name),
                }),
        }
    }

    pub fn leave_party(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(LeavePartyPacket {})
            }
        }
    }

    pub fn expel_party_member(&mut self, account_id: AccountId, character_name: String) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(ExpelPartyMemberPacket {
                    account_id,
                    character_name: clip_name(&character_name),
                }),
        }
    }

    pub fn change_party_leader(&mut self, account_id: AccountId) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(ChangePartyLeaderPacket { account_id })
            }
        }
    }

    pub fn set_party_options(&mut self, experience_share: u32, item_pickup: bool, item_division: bool) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(ChangePartyOptionsPacket {
                    experience_share,
                    item_pickup: u8::from(item_pickup),
                    item_division: u8::from(item_division),
                }),
        }
    }

    pub fn set_party_invite_block(&mut self, refuse: bool) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => self
                .send_map_server_packet(PartyConfigPacket {
                    refuse_invite: u8::from(refuse),
                }),
        }
    }

    pub fn send_party_message(&mut self, player_name: &str, text: &str) -> Result<(), NotConnectedError> {
        let message = format!("{player_name} : {text}");
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(PartyMessagePacket { message })
            }
        }
    }
}
