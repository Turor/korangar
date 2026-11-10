//! Party list, membership, options, HP, position, and chat for packetver
//! 20250716.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    PartyChatPacket, PartyCreateResultPacket, PartyExpSharePacket, PartyInviteResultPacket, PartyLeaderChangedPacket, PartyListPacket,
    PartyMemberAddedPacket, PartyMemberAlivePacket, PartyMemberHealthPacket, PartyMemberJobLevelPacket, PartyMemberPositionPacket,
    PartyMemberRemovedPacket, PartyOptionsPacket,
};

use crate::event::{NetworkEvent, NetworkEventList};
use crate::message::MessageColor;
use crate::party::{PartyRosterEntry, PartyUpdate};

pub fn register_party_packets<Callback>(packet_handler: &mut PacketHandler<NetworkEventList, Callback>) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: PartyCreateResultPacket| NetworkEvent::Party(PartyUpdate::CreateResult(packet.result)))?;
    packet_handler.register(|packet: PartyListPacket| {
        NetworkEvent::Party(PartyUpdate::Roster {
            name: packet.party_name,
            members: packet
                .members
                .into_iter()
                .map(|member| PartyRosterEntry {
                    account_id: member.account_id,
                    character_id: member.character_id,
                    name: member.name,
                    map_name: member.map_name,
                    leader: member.leader == 0,
                    online: member.offline == 0,
                    job: member.job,
                    base_level: member.base_level,
                    x: 0,
                    y: 0,
                    has_position: false,
                })
                .collect(),
        })
    })?;
    packet_handler.register(|packet: PartyMemberAddedPacket| {
        NetworkEvent::Party(PartyUpdate::MemberAdded {
            party_name: packet.party_name,
            share_pickup: packet.share_pickup != 0,
            share_loot: packet.share_loot != 0,
            member: PartyRosterEntry {
                account_id: packet.account_id,
                character_id: packet.character_id,
                name: packet.player_name,
                map_name: packet.map_name,
                leader: packet.leader == 0,
                online: packet.offline == 0,
                job: packet.job,
                base_level: packet.base_level,
                x: packet.x,
                y: packet.y,
                has_position: packet.x >= 0 && packet.y >= 0,
            },
        })
    })?;
    packet_handler.register(|packet: PartyMemberRemovedPacket| {
        NetworkEvent::Party(PartyUpdate::MemberRemoved {
            account_id: packet.account_id,
            name: packet.name,
            result: packet.result,
        })
    })?;
    packet_handler.register(|packet: PartyOptionsPacket| {
        NetworkEvent::Party(PartyUpdate::Options {
            experience_share: packet.experience_share,
            item_pickup: packet.item_pickup != 0,
            item_division: packet.item_division != 0,
        })
    })?;
    packet_handler.register(|packet: PartyExpSharePacket| NetworkEvent::Party(PartyUpdate::ExperienceShare(packet.experience_share)))?;
    packet_handler.register(|packet: PartyLeaderChangedPacket| {
        NetworkEvent::Party(PartyUpdate::Leader {
            previous: packet.previous_leader,
            new_leader: packet.new_leader,
        })
    })?;
    packet_handler.register(|packet: PartyMemberHealthPacket| {
        NetworkEvent::Party(PartyUpdate::Health {
            account_id: packet.account_id,
            health: packet.health,
            maximum: packet.maximum_health,
        })
    })?;
    packet_handler.register(|packet: PartyMemberPositionPacket| {
        NetworkEvent::Party(PartyUpdate::Position {
            account_id: packet.account_id,
            x: packet.x,
            y: packet.y,
        })
    })?;
    packet_handler.register(|packet: PartyMemberJobLevelPacket| {
        NetworkEvent::Party(PartyUpdate::JobLevel {
            account_id: packet.account_id,
            job: packet.job,
            level: packet.base_level,
        })
    })?;
    packet_handler.register(|packet: PartyMemberAlivePacket| {
        NetworkEvent::Party(PartyUpdate::Alive {
            account_id: packet.account_id,
            dead: packet.dead != 0,
        })
    })?;
    packet_handler.register(|packet: PartyInviteResultPacket| {
        NetworkEvent::Party(PartyUpdate::InviteResult {
            name: packet.character_name,
            result: packet.result,
        })
    })?;
    packet_handler.register(|packet: PartyChatPacket| NetworkEvent::ChatMessage {
        text: packet.message,
        color: MessageColor::Party,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};
    use ragnarok_packets::handler::{HandlerResult, NoPacketCallback, PacketHandler};
    use ragnarok_packets::{AccountId, PacketExt, PartyMemberHealthPacket};

    use super::register_party_packets;
    use crate::NetworkEvent;
    use crate::event::NetworkEventList;
    use crate::message::MessageColor;
    use crate::party::PartyUpdate;

    fn process(bytes: &[u8]) -> Vec<NetworkEvent> {
        let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::with_callback(NoPacketCallback).without_callbacks();
        register_party_packets(&mut handler).unwrap();
        let mut reader = ByteReader::without_metadata(bytes);
        match handler.process_one(&mut reader) {
            HandlerResult::Ok(events) => events.0,
            HandlerResult::UnhandledPacket => panic!("party packet was not handled"),
            HandlerResult::PacketCutOff => panic!("party packet was cut off"),
            HandlerResult::InternalError(_) => panic!("party packet failed inside the handler"),
        }
    }

    #[test]
    fn health_packet_becomes_party_update() {
        let mut writer = ByteWriter::new();
        PartyMemberHealthPacket {
            account_id: AccountId(7),
            health: 40,
            maximum_health: 100,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        let events = process(writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::Party(PartyUpdate::Health {
            account_id: AccountId(7),
            health: 40,
            maximum: 100,
        })]));
    }

    #[test]
    fn party_chat_uses_party_color() {
        let mut writer = ByteWriter::new();
        ragnarok_packets::PartyChatPacket {
            account_id: AccountId(3),
            message: "Nova : hi".to_owned(),
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        let events = process(writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::ChatMessage { text, color: MessageColor::Party }] if text == "Nova : hi"));
    }
}
