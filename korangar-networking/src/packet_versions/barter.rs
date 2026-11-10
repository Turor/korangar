//! Barter and expanded barter catalogs, purchases, and the NPC buy result.
//!
//! Catalog opcodes follow Hercules 20250716 MAIN (`0x0B78`, `0x0B79`). The
//! older `0x0B0E` and `0x0B56` layouts map to the same offers. Purchase
//! result is `0x00CA` (`clif_npc_buy_result`). Success for a barter exchange
//! is result `12`.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    BarterShop2021Packet, BarterShopItem, BarterShopPacket, ExpandedBarterItem, ExpandedBarterShop2025Packet, ExpandedBarterShopPacket,
    ItemId, NpcBuyResultPacket,
};

use crate::event::{NetworkEvent, NetworkEventList};

/// One material required for a single purchase of a barter offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarterCurrencyData {
    pub item_id: ItemId,
    pub amount: u32,
    /// Zero means any refine (Hercules sends `0` for script refine `-1`).
    pub refine_level: u16,
    pub item_type: u16,
}

/// One row in a barter or expanded barter shop.
///
/// `stock == u32::MAX` is unlimited. `zeny` is zero for a simple barter row,
/// which always has exactly one currency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarterOfferData {
    pub item_id: ItemId,
    pub item_type: u16,
    pub stock: u32,
    pub weight: u32,
    pub shop_index: u32,
    pub zeny: u32,
    pub currencies: Vec<BarterCurrencyData>,
}

pub fn register_barter_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: BarterShopPacket| NetworkEvent::BarterShopOpened {
        expanded: false,
        offers: simple_offers(packet.items),
    })?;
    packet_handler.register(|packet: BarterShop2021Packet| NetworkEvent::BarterShopOpened {
        expanded: false,
        offers: simple_offers(packet.items),
    })?;
    packet_handler.register(|packet: ExpandedBarterShopPacket| NetworkEvent::BarterShopOpened {
        expanded: true,
        offers: expanded_offers(packet.items),
    })?;
    packet_handler.register(|packet: ExpandedBarterShop2025Packet| NetworkEvent::BarterShopOpened {
        expanded: true,
        offers: expanded_offers(packet.items),
    })?;
    packet_handler.register(|packet: NpcBuyResultPacket| NetworkEvent::NpcBuyResult { result: packet.result })?;
    Ok(())
}

fn simple_offers(items: Vec<BarterShopItem>) -> Vec<BarterOfferData> {
    items
        .into_iter()
        .map(|item| BarterOfferData {
            item_id: item.item_id,
            item_type: u16::from(item.item_type),
            stock: item.amount,
            weight: item.weight,
            shop_index: item.shop_index,
            zeny: 0,
            currencies: vec![BarterCurrencyData {
                item_id: item.currency_id,
                amount: item.currency_amount,
                refine_level: 0,
                item_type: 0,
            }],
        })
        .collect()
}

fn expanded_offers(items: Vec<ExpandedBarterItem>) -> Vec<BarterOfferData> {
    items
        .into_iter()
        .map(|item| BarterOfferData {
            item_id: item.item_id,
            item_type: item.item_type,
            stock: item.amount,
            weight: item.weight,
            shop_index: item.shop_index,
            zeny: item.zeny,
            currencies: item
                .currencies
                .into_iter()
                .map(|currency| BarterCurrencyData {
                    item_id: currency.item_id,
                    amount: currency.amount,
                    refine_level: currency.refine_level,
                    item_type: currency.item_type,
                })
                .collect(),
        })
        .collect()
}

/// Player-facing text for `ZC_PC_PURCHASE_RESULT`. Result `12` is the barter
/// success code Hercules returns from `npc_barter_buylist`.
pub fn barter_result_text(result: u8) -> &'static str {
    match result {
        0 | 12 => "The exchange was successful.",
        1 => "You cannot carry any more of that item.",
        2 => "You are over the weight limit.",
        3 => "Not enough zeny or inventory space.",
        11 => "The exchange failed.",
        13 => "That item is out of stock.",
        14 => "You do not have enough materials.",
        _ => "The exchange failed.",
    }
}

pub fn barter_result_ok(result: u8) -> bool {
    result == 0 || result == 12
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};
    use ragnarok_packets::handler::{HandlerResult, NoPacketCallback, PacketHandler};
    use ragnarok_packets::{
        BarterShop2021Packet, BarterShopItem, ExpandedBarterCurrency, ExpandedBarterItem, ExpandedBarterShop2025Packet, ItemId, PacketExt,
    };

    use super::{barter_result_ok, barter_result_text, register_barter_packets};
    use crate::event::{NetworkEvent, NetworkEventList};

    fn feed(bytes: &[u8]) -> NetworkEvent {
        let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::default();
        register_barter_packets(&mut handler).unwrap();
        let mut reader = ByteReader::without_metadata(bytes);
        let HandlerResult::Ok(events) = handler.process_one(&mut reader) else {
            panic!("barter packet was not handled");
        };
        assert!(reader.is_empty());
        assert_eq!(events.0.len(), 1);
        events.0.into_iter().next().unwrap()
    }

    #[test]
    fn catalog_packets_become_offers() {
        let simple = BarterShop2021Packet {
            items: vec![BarterShopItem {
                item_id: ItemId(509),
                item_type: 0,
                amount: 100,
                currency_id: ItemId(501),
                currency_amount: 2,
                weight: 70,
                shop_index: 3,
                view_sprite: 0,
                location: 0,
            }],
        };
        let mut writer = ByteWriter::new();
        simple.packet_to_bytes(&mut writer).unwrap();
        match feed(writer.as_slice()) {
            NetworkEvent::BarterShopOpened { expanded, offers } => {
                assert!(!expanded);
                assert_eq!(offers[0].item_id, ItemId(509));
                assert_eq!(offers[0].shop_index, 3);
                assert_eq!(offers[0].zeny, 0);
                assert_eq!(offers[0].currencies[0].item_id, ItemId(501));
                assert_eq!(offers[0].currencies[0].amount, 2);
            }
            other => panic!("unexpected {other:?}"),
        }

        let expanded = ExpandedBarterShop2025Packet {
            items: vec![ExpandedBarterItem {
                item_id: ItemId(502),
                item_type: 0,
                amount: u32::MAX,
                weight: 10,
                shop_index: 1,
                zeny: 20,
                view_sprite: 0,
                location: 0,
                refine_level: 0,
                currencies: vec![ExpandedBarterCurrency {
                    item_id: ItemId(513),
                    refine_level: 3,
                    amount: 2,
                    item_type: 0,
                }],
            }],
        };
        let mut writer = ByteWriter::new();
        expanded.packet_to_bytes(&mut writer).unwrap();
        match feed(writer.as_slice()) {
            NetworkEvent::BarterShopOpened { expanded, offers } => {
                assert!(expanded);
                assert_eq!(offers[0].stock, u32::MAX);
                assert_eq!(offers[0].zeny, 20);
                assert_eq!(offers[0].currencies[0].refine_level, 3);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn buy_result_twelve_is_success() {
        match feed(&[0xCA, 0x00, 12]) {
            NetworkEvent::NpcBuyResult { result } => {
                assert!(barter_result_ok(result));
                assert_eq!(barter_result_text(result), "The exchange was successful.");
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(!barter_result_ok(14));
        assert_eq!(barter_result_text(14), "You do not have enough materials.");
    }
}
