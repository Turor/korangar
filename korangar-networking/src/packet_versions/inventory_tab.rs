//! Favorites tab ack (`ZC_INVENTORY_TAB` `0x0908`), shared by every supported
//! map server. The request is [`crate::NetworkingSystem::set_item_favorite`].

use ragnarok_bytes::ByteReader;
use ragnarok_packets::handler::{DuplicateHandlerError, HandlerResult, NoPacketCallback, PacketCallback, PacketHandler};
use ragnarok_packets::{INVENTORY_TAB_FAVORITE, InventoryIndex, InventoryTabPacket};

use crate::NetworkEvent;
use crate::event::NetworkEventList;

pub fn register_inventory_tab_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: InventoryTabPacket| NetworkEvent::ItemFavorite {
        index: packet.inventory_index,
        favorite: packet.tab == INVENTORY_TAB_FAVORITE,
    })?;
    Ok(())
}

/// Runs raw map-server bytes through this handler only (e2e harness).
pub fn decode_item_favorites(bytes: &[u8]) -> Result<Vec<(InventoryIndex, bool)>, String> {
    let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::with_callback(NoPacketCallback);
    register_inventory_tab_packets(&mut handler).map_err(|_| "duplicate inventory tab handler".to_owned())?;
    let mut reader = ByteReader::without_metadata(bytes);
    let mut favorites = Vec::new();
    while !reader.is_empty() {
        match handler.process_one(&mut reader) {
            HandlerResult::Ok(NetworkEventList(events)) => favorites.extend(events.into_iter().filter_map(|event| match event {
                NetworkEvent::ItemFavorite { index, favorite } => Some((index, favorite)),
                _ => None,
            })),
            _ => return Err("inventory tab packet was not handled".to_owned()),
        }
    }
    Ok(favorites)
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::InventoryIndex;

    use super::decode_item_favorites;

    #[test]
    fn favorite_and_unfavorite_acks_decode() {
        let bytes = [0x08, 0x09, 11, 0, 0, 0x08, 0x09, 6, 0, 1];
        assert_eq!(decode_item_favorites(&bytes).unwrap(), [
            (InventoryIndex(9), true),
            (InventoryIndex(4), false)
        ]);
    }
}
