//! Own-shop, buying-store, and search-store packets for PACKETVER 20250716.
//!
//! Hercules registers open-vending as `CZ_REQ_OPENSTORE2` (`0x01B2`), not the
//! older `0x012F` body. Open-buying is `CZ_REQ_OPEN_BUYING_STORE` (`0x0811`,
//! variable). `0x0815` is only the 2-byte close. Sold rows are
//! `ZC_DELETEITEM_FROM_MCSTORE2` (`0x09E5`, 18 bytes). The legacy own-list
//! header `0x0136` is not this report; 20250716 sends the own list as `0x0B40`.
//!
//! Cart and inventory indexes on the wire are the stored index plus 2.

use crate::*;

/// Stored item index to the index Hercules expects (`index + 2`).
pub fn stored_index_to_wire(stored: u16) -> u16 {
    stored.saturating_add(2)
}

/// Same shift for the buying-store sell packet, which carries a signed index.
pub fn stored_index_to_wire_i16(stored: i16) -> i16 {
    stored.saturating_add(2)
}

/// Open this client's vend (`CZ_REQ_OPENSTORE2`, `0x01B2`).
///
/// `result` is 1 to open and 0 to cancel. Item `inventory_index` is the wire
/// index (stored index + 2).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01B2)]
#[variable_length]
pub struct OpenVendingShop2Packet {
    #[length(80)]
    pub shop_name: String,
    pub result: u8,
    #[repeating_remaining]
    pub items: Vec<OpenVendingItemInformation>,
}

/// One wanted item in `CZ_REQ_OPEN_BUYING_STORE` (`0x0811`). Item ids are `u32`
/// for every packet version this client speaks (main branch, 20181121+).
#[derive(Debug, Clone, FixedByteSize, ByteConvertable)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct OpenBuyingStoreItem {
    pub item_id: ItemId,
    pub amount: u16,
    pub price: Price,
}

/// Open or cancel a buying store (`CZ_REQ_OPEN_BUYING_STORE`, `0x0811`).
///
/// `result` is 1 to open and 0 to leave the setup without opening.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0811)]
#[variable_length]
pub struct OpenBuyingStorePacket {
    pub zeny_limit: u32,
    pub result: u8,
    #[length(80)]
    pub shop_name: String,
    #[repeating_remaining]
    pub items: Vec<OpenBuyingStoreItem>,
}

/// Close this client's buying store (`CZ_REQ_CLOSE_BUYING_STORE`, `0x0815`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0815)]
pub struct CloseBuyingStorePacket {}

/// The server tells this client how many buying-store slots it may fill
/// (`ZC_OPEN_BUYING_STORE`, `0x0810`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0810)]
pub struct BuyingStorePreparePacket {
    pub item_count: u8,
}

/// Search shops (`CZ_SEARCH_STORE_INFO`, `0x0835`).
///
/// `search_type` 0 is vending (selling shops) and 1 is buying stores. A price
/// of 0 means that side is unlimited. `cards` stays empty for the name search.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0835)]
#[variable_length]
pub struct SearchStoreInfoPacket {
    pub search_type: u8,
    pub max_price: u32,
    pub min_price: u32,
    #[new_derive]
    pub item_count: u8,
    #[new_derive]
    pub card_count: u8,
    #[repeating(item_count)]
    pub items: Vec<ItemId>,
    #[repeating(card_count)]
    pub cards: Vec<ItemId>,
}

/// Ask for the next search-store page (`CZ_SEARCH_STORE_INFO_NEXT`, `0x0838`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0838)]
pub struct SearchStoreNextPagePacket {}

/// Close the search store (`CZ_CLOSE_SEARCH_STORE_INFO`, `0x083B`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x083B)]
pub struct SearchStoreClosePacket {}

/// One item left this client's vend (`ZC_DELETEITEM_FROM_MCSTORE2`, `0x09E5`).
///
/// `index` is the wire cart index (stored index + 2), matching the own-list
/// index on `0x0B40`.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x09E5)]
pub struct VendingSoldReportPacket {
    pub index: u16,
    pub amount: u16,
    pub buyer_gid: u32,
    pub date: u32,
    pub zeny: u32,
}

/// The server answered an open-vend request (`ZC_ACK_OPENSTORE2`, `0x0A28`).
///
/// `result` 0 is success. 1 is a failure message, 2 is silent, 3 is a bad cell.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A28)]
pub struct OpenVendingAckPacket {
    pub result: u8,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;

    fn encode<T: PacketExt>(packet: &T) -> Vec<u8> {
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        writer.as_slice().to_vec()
    }

    #[test]
    fn stored_index_is_shifted_by_two_on_the_wire() {
        assert_eq!(stored_index_to_wire(0), 2);
        assert_eq!(stored_index_to_wire(2), 4);
        assert_eq!(stored_index_to_wire(u16::MAX), u16::MAX);
        assert_eq!(stored_index_to_wire_i16(5), 7);
    }

    #[test]
    fn open_vending_shop2_matches_hercules_layout() {
        let packet = OpenVendingShop2Packet {
            shop_name: "Pot".to_owned(),
            result: 1,
            items: vec![OpenVendingItemInformation {
                inventory_index: 4,
                amount: 2,
                price: Price(100),
            }],
        };
        let bytes = encode(&packet);
        assert_eq!(&bytes[..2], &[0xB2, 0x01]);
        assert_eq!(bytes.len(), 93);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 93);
        assert_eq!(&bytes[4..7], b"Pot");
        assert_eq!(bytes[83], 0);
        assert_eq!(bytes[84], 1);
        assert_eq!(&bytes[85..93], &[0x04, 0x00, 0x02, 0x00, 0x64, 0x00, 0x00, 0x00]);

        let mut reader = ByteReader::without_metadata(&bytes);
        let parsed = OpenVendingShop2Packet::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.shop_name, "Pot");
        assert_eq!(parsed.result, 1);
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].inventory_index, 4);
        assert_eq!(parsed.items[0].amount, 2);
        assert_eq!(parsed.items[0].price, Price(100));
    }

    #[test]
    fn open_buying_store_matches_hercules_layout() {
        assert_eq!(OpenBuyingStoreItem::size_in_bytes(), 10);
        let packet = OpenBuyingStorePacket {
            zeny_limit: 10_000,
            result: 1,
            shop_name: "Hi".to_owned(),
            items: vec![OpenBuyingStoreItem {
                item_id: ItemId(501),
                amount: 3,
                price: Price(50),
            }],
        };
        let bytes = encode(&packet);
        assert_eq!(&bytes[..2], &[0x11, 0x08]);
        assert_eq!(bytes.len(), 99);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 99);
        assert_eq!(&bytes[4..8], &10_000u32.to_le_bytes());
        assert_eq!(bytes[8], 1);
        assert_eq!(&bytes[9..11], b"Hi");
        assert_eq!(bytes[88], 0);
        assert_eq!(&bytes[89..93], &501u32.to_le_bytes());
        assert_eq!(&bytes[93..95], &3u16.to_le_bytes());
        assert_eq!(&bytes[95..99], &50u32.to_le_bytes());
    }

    #[test]
    fn search_store_query_next_and_close_match_framing() {
        let packet = SearchStoreInfoPacket {
            search_type: 0,
            max_price: 10,
            min_price: 1,
            item_count: 1,
            card_count: 0,
            items: vec![ItemId(1750)],
            cards: Vec::new(),
        };
        let bytes = encode(&packet);
        assert_eq!(&bytes[..2], &[0x35, 0x08]);
        assert_eq!(bytes.len(), 19);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 19);
        assert_eq!(bytes[4], 0);
        assert_eq!(&bytes[5..9], &10u32.to_le_bytes());
        assert_eq!(&bytes[9..13], &1u32.to_le_bytes());
        assert_eq!(bytes[13], 1);
        assert_eq!(bytes[14], 0);
        assert_eq!(&bytes[15..19], &1750u32.to_le_bytes());

        let mut reader = ByteReader::without_metadata(&bytes);
        let parsed = SearchStoreInfoPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.search_type, 0);
        assert_eq!(parsed.items, vec![ItemId(1750)]);
        assert!(parsed.cards.is_empty());

        assert_eq!(&encode(&SearchStoreNextPagePacket {})[..], &[0x38, 0x08]);
        assert_eq!(&encode(&SearchStoreClosePacket {})[..], &[0x3B, 0x08]);
        assert_eq!(&encode(&CloseBuyingStorePacket {})[..], &[0x15, 0x08]);
    }

    #[test]
    fn sold_report_prepare_and_open_ack_match_framing() {
        let sold = [
            0xE5, 0x09, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00,
        ];
        assert_eq!(sold.len(), 18);
        let mut reader = ByteReader::without_metadata(&sold);
        let parsed = VendingSoldReportPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.index, 4);
        assert_eq!(parsed.amount, 1);
        assert_eq!(parsed.zeny, 100);

        let prepare = [0x10, 0x08, 0x03];
        let mut reader = ByteReader::without_metadata(&prepare);
        let parsed = BuyingStorePreparePacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.item_count, 3);

        let ack = [0x28, 0x0A, 0x01];
        let mut reader = ByteReader::without_metadata(&ack);
        let parsed = OpenVendingAckPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.result, 1);
    }

    #[test]
    fn sell_to_buying_store_writes_the_shifted_index() {
        let packet = SellToBuyingStorePacket {
            account_id: AccountId(1),
            store_id: 2,
            items: vec![SellToBuyingStoreInformation {
                inventory_index: stored_index_to_wire_i16(5),
                item_id: ItemId(501),
                amount: 1,
            }],
        };
        let bytes = encode(&packet);
        assert_eq!(&bytes[..2], &[0x19, 0x08]);
        assert_eq!(bytes.len(), 20);
        assert_eq!(u16::from_le_bytes([bytes[12], bytes[13]]), 7);
    }

    #[test]
    fn vend_list_items_include_three_forge_options() {
        use ragnarok_bytes::FixedByteSize;

        assert_eq!(VendingShopItem::size_in_bytes(), 79);
        assert_eq!(OwnVendingItem::size_in_bytes(), 73);

        fn shop_item(price: u32, index: i16, forge_index: u16, refine: u8) -> Vec<u8> {
            let mut item = Vec::new();
            item.extend(price.to_le_bytes());
            item.extend(4u16.to_le_bytes());
            item.extend(index.to_le_bytes());
            item.push(0);
            item.extend(501u32.to_le_bytes());
            item.push(1);
            item.push(0);
            item.extend([0u8; 16]);
            item.extend([0u8; 25]);
            item.extend(forge_index.to_le_bytes());
            item.extend([0u8; 3]);
            item.extend([0u8; 10]);
            item.extend(0u32.to_le_bytes());
            item.extend(8u16.to_le_bytes());
            item.push(refine);
            item.push(2);
            assert_eq!(item.len(), 79);
            item
        }

        fn own_item(price: u32, index: i16, forge_index: u16, refine: u8) -> Vec<u8> {
            let mut item = Vec::new();
            item.extend(price.to_le_bytes());
            item.extend(index.to_le_bytes());
            item.extend(2i16.to_le_bytes());
            item.push(0);
            item.extend(501u32.to_le_bytes());
            item.push(1);
            item.push(0);
            item.extend([0u8; 16]);
            item.extend([0u8; 25]);
            item.extend(forge_index.to_le_bytes());
            item.extend([0u8; 3]);
            item.extend([0u8; 10]);
            item.push(refine);
            item.push(2);
            assert_eq!(item.len(), 73);
            item
        }

        fn frame(header: u16, prefix: &[u8], items: &[Vec<u8>]) -> Vec<u8> {
            let len = 4 + prefix.len() + items.iter().map(Vec::len).sum::<usize>();
            let mut bytes = Vec::new();
            bytes.extend(header.to_le_bytes());
            bytes.extend((len as u16).to_le_bytes());
            bytes.extend(prefix);
            for item in items {
                bytes.extend(item);
            }
            bytes
        }

        let one_shop = frame(0x0B3D, &[9, 0, 0, 0, 77, 0, 0, 0], &[shop_item(120, 3, 100, 9)]);
        assert_eq!(one_shop.len(), 4 + 8 + 79);
        let two_shops = frame(0x0B3D, &[9, 0, 0, 0, 77, 0, 0, 0], &[
            shop_item(120, 3, 100, 9),
            shop_item(121, 4, 101, 8),
        ]);
        assert_eq!(two_shops.len(), 4 + 8 + 79 * 2);
        let mut reader = ByteReader::without_metadata(&two_shops);
        let parsed = VendingShopListPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.account_id, AccountId(9));
        assert_eq!(parsed.vender_id, 77);
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].price, Price(120));
        assert_eq!(parsed.items[0].index, 3);
        assert_eq!(parsed.items[0].item_id, ItemId(501));
        assert_eq!(parsed.items[0].forge_data[0].index, 100);
        assert_eq!(parsed.items[0].view_sprite, 8);
        assert_eq!(parsed.items[0].refine, 9);
        assert_eq!(parsed.items[1].price, Price(121));
        assert_eq!(parsed.items[1].index, 4);
        assert_eq!(parsed.items[1].forge_data[0].index, 101);
        assert_eq!(parsed.items[1].refine, 8);
        let mut reader = ByteReader::without_metadata(&one_shop);
        assert_eq!(VendingShopListPacket::packet_from_bytes(&mut reader).unwrap().items.len(), 1);

        let one_own = frame(0x0B40, &[9, 0, 0, 0], &[own_item(50, 4, 200, 7)]);
        assert_eq!(one_own.len(), 4 + 4 + 73);
        let two_own = frame(0x0B40, &[9, 0, 0, 0], &[own_item(50, 4, 200, 7), own_item(51, 5, 201, 6)]);
        assert_eq!(two_own.len(), 4 + 4 + 73 * 2);
        let mut reader = ByteReader::without_metadata(&two_own);
        let parsed = OwnVendingListPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].price, Price(50));
        assert_eq!(parsed.items[0].index, 4);
        assert_eq!(parsed.items[0].amount, 2);
        assert_eq!(parsed.items[0].forge_data[0].index, 200);
        assert_eq!(parsed.items[0].refine, 7);
        assert_eq!(parsed.items[1].price, Price(51));
        assert_eq!(parsed.items[1].index, 5);
        assert_eq!(parsed.items[1].forge_data[0].index, 201);
        assert_eq!(parsed.items[1].refine, 6);
        let mut reader = ByteReader::without_metadata(&one_own);
        assert_eq!(OwnVendingListPacket::packet_from_bytes(&mut reader).unwrap().items.len(), 1);
    }

    #[test]
    fn search_store_hits_include_three_forge_options() {
        use ragnarok_bytes::FixedByteSize;

        assert_eq!(SearchStoreHitInformation::size_in_bytes(), 157);

        fn hit(store_id: u32, forge_index: u16, refine: u8) -> Vec<u8> {
            let mut row = Vec::new();
            row.extend(store_id.to_le_bytes());
            row.extend(4u32.to_le_bytes());
            let mut name = b"Arrow shop".to_vec();
            name.resize(80, 0);
            row.extend(name);
            row.extend(1750u32.to_le_bytes());
            row.push(0);
            row.extend(10u32.to_le_bytes());
            row.extend(99u16.to_le_bytes());
            row.extend([0u8; 16]);
            row.extend([0u8; 25]);
            row.extend(forge_index.to_le_bytes());
            row.extend([0u8; 3]);
            row.extend([0u8; 10]);
            row.push(refine);
            row.push(1);
            assert_eq!(row.len(), 157);
            row
        }

        fn frame(hits: &[Vec<u8>]) -> Vec<u8> {
            let len = 7 + hits.iter().map(Vec::len).sum::<usize>();
            let mut bytes = Vec::new();
            bytes.extend(0x0B64u16.to_le_bytes());
            bytes.extend((len as u16).to_le_bytes());
            bytes.push(1);
            bytes.push(0);
            bytes.push(2);
            for hit in hits {
                bytes.extend(hit);
            }
            bytes
        }

        let one = frame(&[hit(15, 100, 7)]);
        assert_eq!(one.len(), 7 + 157);
        let two = frame(&[hit(15, 100, 7), hit(16, 101, 6)]);
        assert_eq!(two.len(), 7 + 157 * 2);
        let mut reader = ByteReader::without_metadata(&two);
        let parsed = SearchStoreResultPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.hits.len(), 2);
        assert_eq!(parsed.hits[0].forge_data[0].index, 100);
        assert_eq!(parsed.hits[0].refine, 7);
        assert_eq!(parsed.hits[0].shop_name, "Arrow shop");
        assert_eq!(parsed.hits[1].store_id, 16);
        assert_eq!(parsed.hits[1].forge_data[0].index, 101);
        assert_eq!(parsed.hits[1].refine, 6);
        let mut reader = ByteReader::without_metadata(&one);
        assert_eq!(SearchStoreResultPacket::packet_from_bytes(&mut reader).unwrap().hits.len(), 1);
    }
}
