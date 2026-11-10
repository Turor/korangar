//! Item use and drop for PACKETVER 20250716 MAIN.
//!
//! Layouts follow Hercules `clif_useitemack` / `clif_parse_UseItem` /
//! `clif_parse_DropItem` and the 20250716 length table: `0x0439` (8),
//! `0x01C8` (15), `0x00A8` (7), `0x0363` (6), `0x00AF` (6). Inventory indexes
//! are stored as index+2 on the wire.

use crate::{AccountId, ClientPacket, InventoryIndex, ItemId, MapServer, Packet, ServerPacket};

/// `CZ_USE_ITEM2` (`0x0439`). The account id is on the wire; Hercules ignores
/// it.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0439)]
pub struct UseItemPacket {
    pub inventory_index: InventoryIndex,
    pub account_id: AccountId,
}

/// `ZC_USE_ITEM_ACK` (`0x00A8`). Older ack with no item id or account id.
/// `amount` is the count still in the inventory. `result` is 0 on failure.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00A8)]
pub struct UseItemAckPacket {
    pub inventory_index: InventoryIndex,
    pub amount: u16,
    pub result: u8,
}

/// `ZC_USE_ITEM_ACK2` (`0x01C8`). Sent for PACKETVER >= 3. On success Hercules
/// broadcasts it to the area, so the account id selects this client.
/// `amount` is the remaining stack, not the amount consumed.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01C8)]
pub struct UseItemAck2Packet {
    pub inventory_index: InventoryIndex,
    pub item_id: ItemId,
    pub account_id: AccountId,
    pub amount: u16,
    pub result: u8,
}

/// `CZ_ITEM_THROW2` (`0x0363`) for PACKETVER >= 20180307. `amount` is how many
/// to remove from the stack.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0363)]
pub struct DropItemPacket {
    pub inventory_index: InventoryIndex,
    pub amount: u16,
}

/// `ZC_ITEM_THROW_ACK` (`0x00AF`). `amount` is how many were removed. Zero is
/// a failure and must not change the inventory.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x00AF)]
pub struct DropItemAckPacket {
    pub inventory_index: InventoryIndex,
    pub amount: u16,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::{DropItemAckPacket, DropItemPacket, UseItemAck2Packet, UseItemAckPacket, UseItemPacket};
    use crate::{AccountId, InventoryIndex, ItemId, PacketExt};

    fn round_trip(bytes: &[u8], decode: impl FnOnce(&mut ByteReader<'_>) -> Vec<u8>) {
        let mut reader = ByteReader::without_metadata(bytes);
        let encoded = decode(&mut reader);
        assert!(reader.is_empty());
        assert_eq!(encoded, bytes);
    }

    #[test]
    fn use_item_packet_matches_hercules_layout() {
        let packet = UseItemPacket::new(InventoryIndex(4), AccountId(0x0102_0304));
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = [0x39, 0x04, 0x06, 0x00, 0x04, 0x03, 0x02, 0x01];
        assert_eq!(writer.as_slice(), &bytes);

        round_trip(&bytes, |reader| {
            let decoded = UseItemPacket::packet_from_bytes(reader).unwrap();
            assert_eq!(decoded.inventory_index, InventoryIndex(4));
            assert_eq!(decoded.account_id, AccountId(0x0102_0304));
            let mut writer = ByteWriter::new();
            decoded.packet_to_bytes(&mut writer).unwrap();
            writer.into_inner()
        });
    }

    #[test]
    fn use_item_ack2_matches_hercules_layout() {
        // index 1, item 501, account 9, 4 remaining, success.
        let bytes = [
            0xC8, 0x01, 0x03, 0x00, 0xF5, 0x01, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x04, 0x00, 0x01,
        ];
        assert_eq!(bytes.len(), 15);
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = UseItemAck2Packet::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(packet.inventory_index, InventoryIndex(1));
        assert_eq!(packet.item_id, ItemId(501));
        assert_eq!(packet.account_id, AccountId(9));
        assert_eq!(packet.amount, 4);
        assert_eq!(packet.result, 1);

        let failure = UseItemAck2Packet::new(InventoryIndex(1), ItemId(501), AccountId(9), 0, 0);
        let mut writer = ByteWriter::new();
        failure.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 15);
        assert_eq!(writer.as_slice()[14], 0);
    }

    #[test]
    fn legacy_use_item_ack_matches_hercules_layout() {
        let bytes = [0xA8, 0x00, 0x03, 0x00, 0x04, 0x00, 0x01];
        assert_eq!(bytes.len(), 7);
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = UseItemAckPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(packet.inventory_index, InventoryIndex(1));
        assert_eq!(packet.amount, 4);
        assert_eq!(packet.result, 1);
    }

    #[test]
    fn drop_item_packets_match_hercules_layout() {
        let request = DropItemPacket::new(InventoryIndex(3), 2);
        let mut writer = ByteWriter::new();
        request.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x63, 0x03, 0x05, 0x00, 0x02, 0x00]);

        let ack = [0xAF, 0x00, 0x05, 0x00, 0x02, 0x00];
        let mut reader = ByteReader::without_metadata(&ack);
        let packet = DropItemAckPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(packet.inventory_index, InventoryIndex(3));
        assert_eq!(packet.amount, 2);

        let failed = [0xAF, 0x00, 0x05, 0x00, 0x00, 0x00];
        let mut reader = ByteReader::without_metadata(&failed);
        let packet = DropItemAckPacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.amount, 0);
    }

    #[test]
    fn inventory_index_below_two_is_rejected() {
        let bytes = [0x39, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut reader = ByteReader::without_metadata(&bytes);
        assert!(UseItemPacket::packet_from_bytes(&mut reader).is_err());
    }
}
