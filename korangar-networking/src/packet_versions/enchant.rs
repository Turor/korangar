//! Perfect-enchant UI (`0x0B9A` type 10, `0x0B9B`–`0x0BA0`).
//!
//! Headers are stable on 20250716 MAIN and on the Turoran 20190530 backport.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{ENCHANT_UI_TYPE, EnchantResponsePacket, UiOpenV3Packet};

use crate::event::{NetworkEvent, NetworkEventList};

pub fn register_enchant_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: UiOpenV3Packet| {
        if packet.ui_type == ENCHANT_UI_TYPE {
            Some(NetworkEvent::OpenEnchant {
                enchant_group: packet.data,
            })
        } else {
            None
        }
    })?;
    packet_handler.register(|packet: EnchantResponsePacket| NetworkEvent::EnchantResult {
        message_id: packet.message_id,
        item_id: packet.item_id,
    })?;
    Ok(())
}

/// Inventory index typed into the enchant window. Keeps the signed wire value.
pub fn parse_enchant_index(text: &str) -> Option<i16> {
    text.trim().parse().ok()
}

pub fn parse_enchant_item_id(text: &str) -> Option<u32> {
    text.trim().parse().ok()
}

pub fn parse_enchant_slot(text: &str) -> Option<i16> {
    text.trim().parse().ok()
}

/// Short status line for `ZC_RESPONSE_ENCHANT`.
/// Message 3857 is success and 3858 is failure on the 2025 client string table.
pub fn format_enchant_result(message_id: i32, item_id: u32) -> String {
    let outcome = match message_id {
        3857 => "Enchantment successful",
        3858 => "Enchantment failed",
        _ => "Enchant result",
    };
    format!("{outcome} (message {message_id}, item {item_id})")
}

#[cfg(test)]
mod tests {
    use super::{format_enchant_result, parse_enchant_index, parse_enchant_item_id, parse_enchant_slot};

    #[test]
    fn parsers_keep_signed_indexes_and_full_item_ids() {
        assert_eq!(parse_enchant_index(" -1 "), Some(-1));
        assert_eq!(parse_enchant_index("99999"), None);
        assert_eq!(parse_enchant_item_id("4001"), Some(4001));
        assert_eq!(parse_enchant_slot("3"), Some(3));
        assert!(format_enchant_result(3857, 4001).contains("successful"));
        assert!(format_enchant_result(9, 1).contains("message 9"));
    }
}
