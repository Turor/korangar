//! Slot caps for the inventory, Kafra/guild storage and the cart.
//!
//! Layouts follow Hercules `clif.c`:
//! - `ZC_NOTIFY_STOREITEM_COUNTINFO` (`0x00F2`, 6 bytes) from
//!   `clif_updatestorageamount`: current and maximum storage stacks.
//! - `ZC_NOTIFY_CARTITEM_COUNTINFO` (`0x0121`, 14 bytes) from
//!   `clif_updatestatus(SP_CARTINFO)`: stack count, `MAX_CART`, weight and
//!   maximum weight.
//!
//! `ZC_EXTEND_BODYITEM_SIZE` (`0x0B18`) is [`crate::Packet0b18`].
//!
//! The two list ends the unified inventory needs:
//! - `ZC_CLOSE_STORE` (`0x00F8`, 2 bytes) from `clif_storageclose`: storage is
//!   closed on the server.
//! - `ZC_CARTOFF` (`0x012B`, 2 bytes) from `clif_clearcart`: the cart is gone.
//!
//! Storage and cart deltas (Hercules `clif_storageitemadded`,
//! `clif_storageitemremoved`, `clif_cart_additem`). Their index is the
//! server row + 1, which [`InventoryIndex`] decodes the same way as the
//! storage list, so storage row 0 matches its list entry:
//! - `ZC_DELETE_ITEM_FROM_STORE` (`0x00F6`, 8 bytes).
//! - `ZC_ADD_ITEM_TO_STORE` `0x0A0A` (57 bytes, PACKETVER 2014-08 to 2020-09)
//!   and `0x0B44` (73 bytes, PACKETVER_MAIN_NUM >= 20200916, which is what
//!   TEST's PACKETVER 20250716 build sends).
//! - `ZC_ADD_ITEM_TO_CART` `0x0B45` (73 bytes), the newer twin of `0x0A0B`.

use crate::{InventoryIndex, ItemId, ItemOptions, MapServer, Packet, ServerPacket};

/// `ZC_NOTIFY_STOREITEM_COUNTINFO` (`0x00F2`). 6 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00F2)]
pub struct StorageItemCountPacket {
    pub current_amount: u16,
    pub maximum_amount: u16,
}

/// `ZC_NOTIFY_CARTITEM_COUNTINFO` (`0x0121`). 14 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0121)]
pub struct CartItemCountPacket {
    pub current_count: u16,
    pub maximum_count: u16,
    pub current_weight: u32,
    pub maximum_weight: u32,
}

/// `ZC_CLOSE_STORE` (`0x00F8`). 2 bytes.
#[derive(Debug, Clone, Default, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00F8)]
pub struct StorageClosedPacket {}

/// `ZC_CARTOFF` (`0x012B`). 2 bytes.
#[derive(Debug, Clone, Default, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x012B)]
pub struct CartOffPacket {}

/// `ZC_DELETE_ITEM_FROM_STORE` (`0x00F6`). 8 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00F6)]
pub struct DeleteItemFromStorePacket {
    pub index: InventoryIndex,
    pub amount: i32,
}

/// `ZC_ADD_ITEM_TO_STORE` (`0x0A0A`). 57 bytes, the same layout as the cart's
/// `0x0A0B`.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A0A)]
pub struct AddItemToStorePacket {
    pub index: InventoryIndex,
    pub amount: i32,
    pub item_id: ItemId,
    pub item_type: u8,
    pub identified: u8,
    pub damaged: u8,
    pub refine: u8,
    pub cards: [u32; 4],
    pub option_data: [ItemOptions; 5],
}

/// `ZC_ADD_ITEM_TO_STORE` (`0x0B44`). 73 bytes: forge data, then refine and
/// grade after the options.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B44)]
pub struct AddItemToStore2Packet {
    pub index: InventoryIndex,
    pub amount: i32,
    pub item_id: ItemId,
    pub item_type: u8,
    pub identified: u8,
    pub damaged: u8,
    pub cards: [u32; 4],
    pub option_data: [ItemOptions; 5],
    pub forge_data: [ItemOptions; 3],
    pub refine: u8,
    pub grade: u8,
}

/// `ZC_ADD_ITEM_TO_CART` (`0x0B45`). 73 bytes, the layout of `0x0B44`.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B45)]
pub struct AddItemToCart2Packet {
    pub index: InventoryIndex,
    pub amount: i32,
    pub item_id: ItemId,
    pub item_type: u8,
    pub identified: u8,
    pub damaged: u8,
    pub cards: [u32; 4],
    pub option_data: [ItemOptions; 5],
    pub forge_data: [ItemOptions; 3],
    pub refine: u8,
    pub grade: u8,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::{
        AddItemToCart2Packet, AddItemToStore2Packet, AddItemToStorePacket, CartItemCountPacket, CartOffPacket, DeleteItemFromStorePacket,
        StorageClosedPacket, StorageItemCountPacket,
    };
    use crate::{InventoryIndex, ItemId, Packet};

    #[test]
    fn storage_deltas_match_hercules_sizes_and_storage_row_zero() {
        // Each layout consumes exactly Hercules' packet length.
        fn whole<P: Packet>(header: u16, length: usize) -> P {
            let mut bytes = header.to_le_bytes().to_vec();
            bytes.extend([1, 0]);
            bytes.resize(length, 0);
            let mut reader = ByteReader::without_metadata(&bytes);
            let packet = P::packet_from_bytes(&mut reader).unwrap();
            assert!(reader.is_empty(), "{header:#06X} left bytes over");
            packet
        }
        whole::<DeleteItemFromStorePacket>(0x00F6, 8);
        whole::<AddItemToStorePacket>(0x0A0A, 57);
        whole::<AddItemToStore2Packet>(0x0B44, 73);
        whole::<AddItemToCart2Packet>(0x0B45, 73);

        // 00f6 <index = row + 1>.W <amount>.L: row 0 is wire 1, which the
        // storage list also decodes to InventoryIndex(u16::MAX).
        let mut reader = ByteReader::without_metadata(&[0xF6, 0x00, 1, 0, 3, 0, 0, 0]);
        let packet = DeleteItemFromStorePacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!((packet.index, packet.amount), (InventoryIndex(u16::MAX), 3));

        let mut bytes = vec![0x44, 0x0B, 4, 0, 2, 0, 0, 0];
        bytes.extend(909u32.to_le_bytes());
        bytes.extend([3, 1, 0]);
        bytes.extend([0; 16 + 25 + 15]);
        bytes.extend([7, 0]);
        assert_eq!(bytes.len(), 73);
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = AddItemToStore2Packet::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.index, InventoryIndex(2));
        assert_eq!(
            (packet.amount, packet.item_id, packet.item_type, packet.refine),
            (2, ItemId(909), 3, 7)
        );
    }

    #[test]
    fn close_store_and_cart_off_are_bare_headers() {
        let mut reader = ByteReader::without_metadata(&[0xF8, 0x00]);
        StorageClosedPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        let mut reader = ByteReader::without_metadata(&[0x2B, 0x01]);
        CartOffPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        let mut writer = ByteWriter::new();
        CartOffPacket::default().packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x2B, 0x01]);
    }
    use crate::{Packet0b18, PacketExt};

    #[test]
    fn storage_count_matches_hercules_layout() {
        // WFIFOW(0) = 0xf2; WFIFOW(2) = amount; WFIFOW(4) = max_amount.
        let bytes = [0xF2, 0x00, 37, 0, 0x80, 0x3E];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = StorageItemCountPacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.current_amount, 37);
        assert_eq!(packet.maximum_amount, 16_000);

        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &bytes);
    }

    #[test]
    fn cart_count_matches_hercules_layout() {
        // cart_num, MAX_CART, cart_weight, cart_weight_max.
        let bytes = [0x21, 0x01, 12, 0, 100, 0, 0x10, 0x27, 0, 0, 0x40, 0x9C, 0, 0];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = CartItemCountPacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.current_count, 12);
        assert_eq!(packet.maximum_count, 100);
        assert_eq!(packet.current_weight, 10_000);
        assert_eq!(packet.maximum_weight, 40_000);
    }

    #[test]
    fn body_item_size_is_the_expansion_over_100() {
        // Turoran: 600 slots, FIXED_INVENTORY_SIZE 100, so the expansion is 500.
        let bytes = [0x18, 0x0B, 0xF4, 0x01];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = Packet0b18::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.expansion_size, 500);
    }
}
