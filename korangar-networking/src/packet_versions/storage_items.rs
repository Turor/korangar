//! Storage and cart deltas for the unified inventory.
//!
//! - `0x00F6` `ZC_DELETE_ITEM_FROM_STORE`: shared by every map server.
//! - `0x0A0A` / `0x0B44` `ZC_ADD_ITEM_TO_STORE` and `0x0B45`
//!   `ZC_ADD_ITEM_TO_CART`: the 2014-2020 and the PACKETVER_MAIN_NUM >=
//!   20200916 layouts. TEST's PACKETVER 20250716 build sends `0x0B44` and
//!   `0x0B45`; only the 20250716 handler registers these
//!   ([`register_item_add_packets`]), because older framings disagree on the
//!   `0x0A0A` size.
//!
//! Indexes decode through [`InventoryIndex`] like the storage list, so a
//! delta for storage row 0 (wire 1) finds the list's row 0.

use ragnarok_bytes::ByteReader;
use ragnarok_packets::handler::{DuplicateHandlerError, HandlerResult, NoPacketCallback, PacketCallback, PacketHandler};
use ragnarok_packets::{
    AddItemToCart2Packet, AddItemToStore2Packet, AddItemToStorePacket, DeleteItemFromStorePacket, EquipPosition, EquippableItemFlags,
    InventoryIndex, ItemId, ItemOptions, RegularItemFlags,
};

use crate::event::NetworkEventList;
use crate::{InventoryItem, InventoryItemDetails, NetworkEvent, NoMetadata};

/// Item types Hercules sends as stackable rows (healing, usable, etc, card,
/// ammo, delay-consume, cash). The rest are equipment rows.
pub fn stackable_item_type(item_type: u8) -> bool {
    matches!(item_type, 0 | 2 | 3 | 6 | 10 | 11 | 18)
}

/// One row from an add packet.
#[allow(clippy::too_many_arguments)]
pub fn added_item(
    index: InventoryIndex,
    amount: i32,
    item_id: ItemId,
    item_type: u8,
    identified: bool,
    damaged: bool,
    refine: u8,
    cards: [u32; 4],
    option_data: [ItemOptions; 5],
) -> InventoryItem<NoMetadata> {
    let details = match stackable_item_type(item_type) {
        true => InventoryItemDetails::Regular {
            amount: u16::try_from(amount.max(0)).unwrap_or(u16::MAX),
            equipped_position: EquipPosition::empty(),
            flags: {
                let mut flags = RegularItemFlags::empty();
                flags.set(RegularItemFlags::IDENTIFIED, identified);
                flags
            },
        },
        false => InventoryItemDetails::Equippable {
            equip_position: EquipPosition::empty(),
            equipped_position: EquipPosition::empty(),
            bind_on_equip_type: 0,
            w_item_sprite_number: 0,
            option_count: option_data.iter().filter(|option| option.index != 0).count() as u8,
            option_data,
            forge_data: None,
            refinement_level: refine,
            enchantment_level: 0,
            flags: {
                let mut flags = EquippableItemFlags::empty();
                flags.set(EquippableItemFlags::IDENTIFIED, identified);
                flags.set(EquippableItemFlags::IS_BROKEN, damaged);
                flags
            },
        },
    };
    InventoryItem {
        metadata: NoMetadata,
        index,
        item_id,
        item_type,
        slot: cards,
        hire_expiration_date: 0,
        details,
    }
}

/// `0x00F6`, for every map server.
pub fn register_storage_delete_packet<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: DeleteItemFromStorePacket| NetworkEvent::StorageItemRemoved {
        index: packet.index,
        amount: u16::try_from(packet.amount.max(0)).unwrap_or(u16::MAX),
    })?;
    Ok(())
}

/// `0x0A0A`, `0x0B44` and `0x0B45` with their PACKETVER 20250716 sizes.
pub fn register_item_add_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: AddItemToStorePacket| NetworkEvent::StorageItemAdded {
        item: added_item(
            packet.index,
            packet.amount,
            packet.item_id,
            packet.item_type,
            packet.identified != 0,
            packet.damaged != 0,
            packet.refine,
            packet.cards,
            packet.option_data,
        ),
    })?;
    packet_handler.register(|packet: AddItemToStore2Packet| NetworkEvent::StorageItemAdded {
        item: added_item(
            packet.index,
            packet.amount,
            packet.item_id,
            packet.item_type,
            packet.identified != 0,
            packet.damaged != 0,
            packet.refine,
            packet.cards,
            packet.option_data,
        ),
    })?;
    packet_handler.register(|packet: AddItemToCart2Packet| NetworkEvent::CartItemAdded {
        item: added_item(
            packet.index,
            packet.amount,
            packet.item_id,
            packet.item_type,
            packet.identified != 0,
            packet.damaged != 0,
            packet.refine,
            packet.cards,
            packet.option_data,
        ),
    })?;
    Ok(())
}

/// Storage and cart deltas in raw PACKETVER 20250716 map-server bytes, for
/// tests and the e2e harness.
pub fn decode_item_deltas(bytes: &[u8]) -> Result<Vec<NetworkEvent>, String> {
    let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::with_callback(NoPacketCallback);
    register_storage_delete_packet(&mut handler).map_err(|_| "duplicate delta handler".to_owned())?;
    register_item_add_packets(&mut handler).map_err(|_| "duplicate delta handler".to_owned())?;
    let mut reader = ByteReader::without_metadata(bytes);
    let mut events = Vec::new();
    while !reader.is_empty() {
        match handler.process_one(&mut reader) {
            HandlerResult::Ok(NetworkEventList(list)) => events.extend(list),
            _ => return Err("item delta packet was not handled".to_owned()),
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{InventoryIndex, ItemId};

    use super::decode_item_deltas;
    use crate::{InventoryItemDetails, NetworkEvent};

    fn add(header: u16, wire_index: u16, amount: i32, item_id: u32, item_type: u8) -> Vec<u8> {
        let mut bytes = header.to_le_bytes().to_vec();
        bytes.extend(wire_index.to_le_bytes());
        bytes.extend(amount.to_le_bytes());
        bytes.extend(item_id.to_le_bytes());
        bytes.extend([item_type, 1, 0]);
        bytes.resize(73, 0);
        bytes
    }

    #[test]
    fn storage_add_and_delete_decode_with_storage_row_zero() {
        let mut bytes = add(0x0B44, 1, 2, 909, 3);
        bytes.extend([0xF6, 0x00, 1, 0, 2, 0, 0, 0]);
        bytes.extend(add(0x0B45, 3, 1, 1202, 5));
        let events = decode_item_deltas(&bytes).unwrap();
        match events.as_slice() {
            [
                NetworkEvent::StorageItemAdded { item },
                NetworkEvent::StorageItemRemoved { index, amount },
                NetworkEvent::CartItemAdded { item: knife },
            ] => {
                assert_eq!((item.index, item.item_id), (InventoryIndex(u16::MAX), ItemId(909)));
                assert!(matches!(item.details, InventoryItemDetails::Regular { amount: 2, .. }));
                assert_eq!((*index, *amount), (InventoryIndex(u16::MAX), 2));
                assert_eq!((knife.index, knife.item_id), (InventoryIndex(1), ItemId(1202)));
                assert!(matches!(knife.details, InventoryItemDetails::Equippable { .. }));
            }
            other => panic!("unexpected events {other:?}"),
        }

        let mut old = add(0x0A0A, 5, 4, 501, 0);
        old.truncate(57);
        let events = decode_item_deltas(&old).unwrap();
        assert!(matches!(events.as_slice(), [NetworkEvent::StorageItemAdded { item }] if item.index == InventoryIndex(3)));
    }
}
