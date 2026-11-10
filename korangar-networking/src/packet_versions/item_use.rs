//! Item use and drop acks shared by every supported map server.
//! `0x01C8` is the PACKETVER >= 3 ack. `0x00A8` is the older ack Hercules can
//! still emit. `0x00AF` reports how many were dropped; zero is a failure.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{DropItemAckPacket, ItemId, UseItemAck2Packet, UseItemAckPacket};

use crate::NetworkEvent;
use crate::event::NetworkEventList;

pub fn register_item_use_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: UseItemAck2Packet| NetworkEvent::ItemUseAck {
        index: packet.inventory_index,
        item_id: packet.item_id,
        account_id: Some(packet.account_id),
        amount: packet.amount,
        success: packet.result != 0,
    })?;
    packet_handler.register(|packet: UseItemAckPacket| NetworkEvent::ItemUseAck {
        index: packet.inventory_index,
        item_id: ItemId(0),
        account_id: None,
        amount: packet.amount,
        success: packet.result != 0,
    })?;
    packet_handler.register(|packet: DropItemAckPacket| NetworkEvent::ItemThrowAck {
        index: packet.inventory_index,
        amount: packet.amount,
    })?;
    Ok(())
}
