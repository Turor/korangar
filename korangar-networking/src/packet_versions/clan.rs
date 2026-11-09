//! Clan info (`ZC_CLANINFO`, `0x098A`), online counts
//! (`ZC_NOTIFY_CLAN_CONNECTINFO`, `0x0988`), and leave ack
//! (`ZC_ACK_CLAN_LEAVE`, `0x0989`).

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{ClanInfoPacket, ClanLeaveAckPacket, ClanOnlineCountPacket};

use crate::event::{NetworkEvent, NetworkEventList};

pub fn register_clan_packets<Callback>(packet_handler: &mut PacketHandler<NetworkEventList, Callback>) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: ClanInfoPacket| NetworkEvent::ClanInfo {
        clan_id: packet.clan_id,
        name: packet.clan_name,
        master_name: packet.clan_master,
        map_name: packet.clan_map,
        alliances: packet.aliances.into_iter().map(|entry| entry.name).collect(),
        antagonists: packet.antagonists.into_iter().map(|entry| entry.name).collect(),
    })?;
    packet_handler.register(|packet: ClanOnlineCountPacket| NetworkEvent::ClanOnlineCount {
        online_members: packet.online_members,
        maximum_members: packet.maximum_members,
    })?;
    packet_handler.register(|_packet: ClanLeaveAckPacket| NetworkEvent::ClanLeft)?;
    Ok(())
}
