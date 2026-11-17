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

use crate::{MapServer, Packet, ServerPacket};

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

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::{CartItemCountPacket, CartOffPacket, StorageClosedPacket, StorageItemCountPacket};

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
