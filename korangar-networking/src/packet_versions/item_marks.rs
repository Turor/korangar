//! Turoran item marks (`0x0EE0`, `0x0EE1`, `0x0EE8`, `0x0EE9`) and the
//! server features notice (`0x0EF1` from the map server).
//!
//! The server only sends the mark list after the client says hello, and the
//! client only says hello after the features notice has the item marks bit,
//! so neither side sends a packet the other cannot frame.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{ItemMarkResultPacket, ItemMarksListPacket, TuroranFeaturesPacket};

use crate::event::{ItemMark, NetworkEvent, NetworkEventList};

/// Version sent in `0x0EE0` and expected back in `0x0EE8`.
pub const ITEM_MARKS_VERSION: u16 = 1;

pub fn register_item_mark_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: TuroranFeaturesPacket| NetworkEvent::TuroranFeatures { features: packet.features })?;
    packet_handler.register(|packet: ItemMarksListPacket| NetworkEvent::ItemMarksList {
        entries: packet
            .entries
            .into_iter()
            .filter_map(|entry| {
                Some(ItemMark {
                    location: entry.location,
                    index: ragnarok_packets::InventoryIndex(entry.wire_index.checked_sub(2)?),
                    marks: entry.marks,
                })
            })
            .collect(),
    })?;
    packet_handler.register(|packet: ItemMarkResultPacket| NetworkEvent::ItemMarkResult {
        location: packet.location,
        index: packet.index.0,
        marks: packet.marks,
        result: packet.result,
    })?;
    Ok(())
}
