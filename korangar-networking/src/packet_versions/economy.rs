//! Player vending, buying stores, and shop search for PACKETVER layouts used by
//! 20220406 and 20250716 (main branch item ids and the 20200916+ headers).
//! NPC buy/sell (`ShopItemListPacket`, `BuyOrSellPacket`) stays on its own
//! path.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    BuyingStoreClosedPacket, BuyingStoreListPacket, BuyingStoreOpenFailedPacket, BuyingStoreSignPacket, OpenMarketPacket,
    OwnBuyingStoreListPacket, OwnVendingListPacket, SearchStoreFailedPacket, SearchStoreOpenPacket, SearchStoreResultPacket,
    SearchStoreSelectResultPacket, VendingPreparePacket, VendingPurchaseResultPacket, VendingShopClosedPacket, VendingShopListPacket,
    VendingShopSignPacket,
};

use crate::event::NetworkEventList;
use crate::items::{ItemQuantity, NoMetadata, ShopItem};
use crate::{NetworkEvent, SearchStoreHit};

pub fn open_market_event(packet: OpenMarketPacket) -> NetworkEvent {
    let items = packet
        .items
        .into_iter()
        .map(|item| ShopItem {
            metadata: NoMetadata,
            item_id: ragnarok_packets::ItemId(item.name_id),
            item_type: item.item_type,
            price: item.price,
            quantity: ItemQuantity::from(item.quantity),
            weight: item.weight,
            location: item.location,
        })
        .collect();

    NetworkEvent::OpenShop { items }
}

fn shop_item(
    item_id: ragnarok_packets::ItemId,
    item_type: u8,
    price: ragnarok_packets::Price,
    amount: u16,
    location: u32,
) -> ShopItem<NoMetadata> {
    ShopItem {
        metadata: NoMetadata,
        item_id,
        item_type,
        price,
        quantity: ItemQuantity::Fixed(u32::from(amount)),
        weight: 0,
        location,
    }
}

pub fn register_player_shop_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: VendingShopSignPacket| NetworkEvent::PlayerShopSign {
        account_id: packet.account_id,
        shop_name: packet.shop_name,
        buying_store: false,
    })?;
    packet_handler.register(|packet: VendingShopClosedPacket| NetworkEvent::PlayerShopClosed {
        account_id: packet.account_id,
        buying_store: false,
    })?;
    packet_handler.register(|packet: BuyingStoreSignPacket| NetworkEvent::PlayerShopSign {
        account_id: packet.account_id,
        shop_name: packet.shop_name,
        buying_store: true,
    })?;
    packet_handler.register(|packet: BuyingStoreClosedPacket| NetworkEvent::PlayerShopClosed {
        account_id: packet.account_id,
        buying_store: true,
    })?;
    packet_handler.register(|packet: VendingPreparePacket| NetworkEvent::VendingPrepare {
        item_count: packet.item_count,
    })?;
    packet_handler.register(|packet: VendingShopListPacket| {
        let mut items = Vec::with_capacity(packet.items.len());
        let mut indexes = Vec::with_capacity(packet.items.len());
        for item in packet.items {
            indexes.push(item.index.max(0) as u16);
            items.push(shop_item(item.item_id, item.item_type, item.price, item.amount, item.location));
        }
        NetworkEvent::PlayerShopOpened {
            account_id: packet.account_id,
            store_id: packet.vender_id,
            buying_store: false,
            items,
            indexes,
        }
    })?;
    packet_handler.register(|packet: VendingPurchaseResultPacket| NetworkEvent::VendingPurchaseResult {
        index: packet.index,
        amount: packet.amount,
        result: packet.result,
    })?;
    packet_handler.register(|packet: OwnVendingListPacket| NetworkEvent::OwnPlayerShopOpened {
        item_count: packet.items.len(),
        buying_store: false,
    })?;
    packet_handler.register(|packet: BuyingStoreListPacket| {
        let items = packet
            .items
            .into_iter()
            .map(|item| shop_item(item.item_id, item.item_type, item.price, item.amount, 0))
            .collect();
        NetworkEvent::PlayerShopOpened {
            account_id: packet.account_id,
            store_id: packet.store_id,
            buying_store: true,
            items,
            indexes: Vec::new(),
        }
    })?;
    packet_handler.register(|packet: OwnBuyingStoreListPacket| NetworkEvent::OwnPlayerShopOpened {
        item_count: packet.items.len(),
        buying_store: true,
    })?;
    packet_handler.register(|packet: BuyingStoreOpenFailedPacket| NetworkEvent::BuyingStoreOpenFailed { result: packet.result })?;
    packet_handler.register(|packet: SearchStoreOpenPacket| NetworkEvent::SearchStoreOpen {
        effect: packet.effect,
        remaining_uses: packet.remaining_uses,
    })?;
    packet_handler.register(|packet: SearchStoreFailedPacket| NetworkEvent::SearchStoreFailed { reason: packet.reason })?;
    packet_handler.register(|packet: SearchStoreResultPacket| {
        let hits = packet
            .hits
            .into_iter()
            .map(|hit| SearchStoreHit {
                store_id: hit.store_id,
                account_id: hit.account_id,
                shop_name: hit.shop_name,
                item_id: hit.item_id,
                price: hit.price,
                amount: hit.amount,
            })
            .collect();
        NetworkEvent::SearchStoreResults {
            first_page: packet.first_page != 0,
            next_page: packet.next_page != 0,
            uses_remaining: packet.uses_remaining,
            hits,
        }
    })?;
    packet_handler.register(|packet: SearchStoreSelectResultPacket| NetworkEvent::SearchStorePosition { x: packet.x, y: packet.y })?;
    Ok(())
}
