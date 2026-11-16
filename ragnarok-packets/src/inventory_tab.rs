//! Favorites tab (`PlaceETCTab`) for inventory items.
//!
//! Layouts follow Hercules `clif.c`:
//! - `CZ_INVENTORY_TAB` (`0x0907`, 5 bytes) parsed by `clif_parse_MoveItem`:
//!   `<index>.W <type>.B`, type 0 moves the item to the favorites tab and 1
//!   moves it back.
//! - `ZC_INVENTORY_TAB` (`0x0908`, 5 bytes) from `clif_favorite_item`:
//!   `<index>.W <type>.B`, type 0 means the item is now a favorite.
//!
//! The flag itself also arrives in the item lists as the `PlaceETCTab` bit
//! ([`crate::RegularItemFlags::IN_ETC_TAB`],
//! [`crate::EquippableItemFlags::IN_ETC_TAB`]).

use crate::{ClientPacket, InventoryIndex, MapServer, Packet, ServerPacket};

/// Wire value for "in the favorites tab".
pub const INVENTORY_TAB_FAVORITE: u8 = 0;
/// Wire value for "in the normal tabs".
pub const INVENTORY_TAB_NORMAL: u8 = 1;

/// `CZ_INVENTORY_TAB` (`0x0907`). 5 bytes.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0907)]
pub struct InventoryTabRequestPacket {
    pub inventory_index: InventoryIndex,
    pub tab: u8,
}

/// `ZC_INVENTORY_TAB` (`0x0908`). 5 bytes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0908)]
pub struct InventoryTabPacket {
    pub inventory_index: InventoryIndex,
    pub tab: u8,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::{INVENTORY_TAB_FAVORITE, INVENTORY_TAB_NORMAL, InventoryTabPacket, InventoryTabRequestPacket};
    use crate::{EquippableItemFlags, InventoryIndex, PacketExt, RegularItemFlags};

    #[test]
    fn request_matches_hercules_layout() {
        // RFIFOW(fd,2)-2 is the index; RFIFOB(fd,4) == 0 sets favorite.
        let mut writer = ByteWriter::new();
        InventoryTabRequestPacket::new(InventoryIndex(9), INVENTORY_TAB_FAVORITE)
            .packet_to_bytes(&mut writer)
            .unwrap();
        assert_eq!(writer.as_slice(), &[0x07, 0x09, 11, 0, 0]);
    }

    #[test]
    fn ack_matches_hercules_layout() {
        // WFIFOW(fd,2) = index+2; WFIFOB(fd,4) = favorite ? 0 : 1.
        let bytes = [0x08, 0x09, 11, 0, 1];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = InventoryTabPacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.inventory_index, InventoryIndex(9));
        assert_eq!(packet.tab, INVENTORY_TAB_NORMAL);
    }

    #[test]
    fn place_etc_tab_bits_match_the_item_list_bitfields() {
        // EQUIPITEM_INFO: IsIdentified:1, IsDamaged:1, PlaceETCTab:1.
        assert_eq!(EquippableItemFlags::IN_ETC_TAB.bits(), 0b100);
        // NORMALITEM_INFO: IsIdentified:1, PlaceETCTab:1.
        assert_eq!(RegularItemFlags::IN_ETC_TAB.bits(), 0b10);
    }
}
