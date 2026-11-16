//! Inventory, storage and cart slot caps, shared by every supported map
//! server.
//!
//! - `0x0B18` `ZC_EXTEND_BODYITEM_SIZE`: slots beyond Hercules'
//!   `FIXED_INVENTORY_SIZE` (100). Turoran sends 500, so 600 in total.
//! - `0x00F2` `ZC_NOTIFY_STOREITEM_COUNTINFO`: storage stacks and the cap.
//! - `0x0121` `ZC_NOTIFY_CARTITEM_COUNTINFO`: cart stacks and `MAX_CART`.

use ragnarok_bytes::ByteReader;
use ragnarok_packets::handler::{DuplicateHandlerError, HandlerResult, NoPacketCallback, PacketCallback, PacketHandler};
use ragnarok_packets::{CartItemCountPacket, Packet0b18, StorageItemCountPacket};

use crate::NetworkEvent;
use crate::event::NetworkEventList;

/// Hercules `FIXED_INVENTORY_SIZE`. `0x0B18` reports slots past this.
pub const FIXED_INVENTORY_SIZE: u32 = 100;

/// Which item list a cap belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemCapacityList {
    Inventory,
    Storage,
    Cart,
}

/// A slot cap from the map server. `used` is the server's own stack count
/// when the packet carries one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemCapacity {
    pub list: ItemCapacityList,
    pub used: Option<u16>,
    pub maximum: u32,
}

impl ItemCapacity {
    pub fn inventory(expansion_size: u16) -> Self {
        Self {
            list: ItemCapacityList::Inventory,
            used: None,
            maximum: FIXED_INVENTORY_SIZE + u32::from(expansion_size),
        }
    }
}

pub fn register_item_capacity_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: Packet0b18| NetworkEvent::ItemCapacity(ItemCapacity::inventory(packet.expansion_size)))?;
    packet_handler.register(|packet: StorageItemCountPacket| {
        NetworkEvent::ItemCapacity(ItemCapacity {
            list: ItemCapacityList::Storage,
            used: Some(packet.current_amount),
            maximum: u32::from(packet.maximum_amount),
        })
    })?;
    packet_handler.register(|packet: CartItemCountPacket| {
        NetworkEvent::ItemCapacity(ItemCapacity {
            list: ItemCapacityList::Cart,
            used: Some(packet.current_count),
            maximum: u32::from(packet.maximum_count),
        })
    })?;
    Ok(())
}

/// Runs raw map-server bytes through these handlers only. Used by the e2e
/// harness to check the wire layouts without a socket.
pub fn decode_item_capacity(bytes: &[u8]) -> Result<Vec<ItemCapacity>, String> {
    let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::with_callback(NoPacketCallback);
    register_item_capacity_packets(&mut handler).map_err(|_| "duplicate capacity handler".to_owned())?;
    let mut reader = ByteReader::without_metadata(bytes);
    let mut capacities = Vec::new();
    while !reader.is_empty() {
        match handler.process_one(&mut reader) {
            HandlerResult::Ok(NetworkEventList(events)) => capacities.extend(events.into_iter().filter_map(|event| match event {
                NetworkEvent::ItemCapacity(capacity) => Some(capacity),
                _ => None,
            })),
            _ => return Err("capacity packet was not handled".to_owned()),
        }
    }
    Ok(capacities)
}

#[cfg(test)]
mod tests {
    use super::{ItemCapacity, ItemCapacityList, decode_item_capacity};

    #[test]
    fn turoran_caps_decode_from_wire_bytes() {
        let bytes = [
            0x18, 0x0B, 0xF4, 0x01, // 0x0B18: expansion 500
            0xF2, 0x00, 37, 0, 0x58, 0x02, // 0x00F2: 37 / 600
            0x21, 0x01, 12, 0, 100, 0, 0x10, 0x27, 0, 0, 0x40, 0x9C, 0, 0, // 0x0121: 12 / 100
        ];
        let capacities = decode_item_capacity(&bytes).unwrap();
        assert_eq!(capacities, [
            ItemCapacity {
                list: ItemCapacityList::Inventory,
                used: None,
                maximum: 600,
            },
            ItemCapacity {
                list: ItemCapacityList::Storage,
                used: Some(37),
                maximum: 600,
            },
            ItemCapacity {
                list: ItemCapacityList::Cart,
                used: Some(12),
                maximum: 100,
            },
        ]);
    }

    #[test]
    fn no_expansion_is_the_fixed_hundred() {
        assert_eq!(ItemCapacity::inventory(0).maximum, 100);
    }
}
