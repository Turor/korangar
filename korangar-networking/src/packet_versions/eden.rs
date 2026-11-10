//! Turoran Eden Group board (`0x0EF3`, `0x0EFB`–`0x0EFD`).
//!
//! Layouts follow the Turoran browser builders. Actions are list (0), accept
//! (1), complete (2), abandon (3), and buy (4). Buy puts the shop slot in the
//! quest-id field.

#[cfg(test)]
use ragnarok_packets::EdenShopEntry;
use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{EdenQuestListPacket, EdenResultPacket, EdenShopPacket, RequestEdenPacket};

use crate::event::{NetworkEvent, NetworkEventList};

pub const EDEN_ACTION_LIST: i16 = 0;
pub const EDEN_ACTION_ACCEPT: i16 = 1;
pub const EDEN_ACTION_COMPLETE: i16 = 2;
pub const EDEN_ACTION_ABANDON: i16 = 3;
pub const EDEN_ACTION_BUY: i16 = 4;

/// One Eden board button. Buy has no amount on the wire, so each stack is its
/// own packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdenButton {
    Accept(i32),
    Complete(i32),
    Abandon(i32),
    Buy { slot: i32, amount: u16 },
}

/// `0x0EF3` packets for one button. A buy of `amount` stacks is `amount`
/// copies.
pub fn eden_button_packets(button: EdenButton) -> Vec<RequestEdenPacket> {
    match button {
        EdenButton::Accept(quest_id) => vec![RequestEdenPacket {
            action: EDEN_ACTION_ACCEPT,
            quest_id,
        }],
        EdenButton::Complete(quest_id) => vec![RequestEdenPacket {
            action: EDEN_ACTION_COMPLETE,
            quest_id,
        }],
        EdenButton::Abandon(quest_id) => vec![RequestEdenPacket {
            action: EDEN_ACTION_ABANDON,
            quest_id,
        }],
        EdenButton::Buy { slot, amount } => (0..amount)
            .map(|_| RequestEdenPacket {
                action: EDEN_ACTION_BUY,
                quest_id: slot,
            })
            .collect(),
    }
}

/// Eight-byte `0x0EF3` body: opcode, action, quest id or shop slot.
pub fn encode_eden_packet(action: i16, quest_id: i32) -> [u8; 8] {
    let mut bytes = [0u8; 8];
    bytes[0..2].copy_from_slice(&0x0EF3u16.to_le_bytes());
    bytes[2..4].copy_from_slice(&action.to_le_bytes());
    bytes[4..8].copy_from_slice(&quest_id.to_le_bytes());
    bytes
}

pub fn register_eden_packets<Callback>(packet_handler: &mut PacketHandler<NetworkEventList, Callback>) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: EdenQuestListPacket| NetworkEvent::EdenQuestList {
        base_level: packet.base_level,
        merit: packet.merit,
        quests: packet.quests,
    })?;
    packet_handler.register(|packet: EdenResultPacket| NetworkEvent::EdenResult {
        result: packet.result,
        quest_id: packet.quest_id,
        summary: format_eden_result(packet.result, packet.quest_id),
    })?;
    packet_handler.register(|packet: EdenShopPacket| NetworkEvent::EdenShop {
        merit: packet.merit,
        items: packet.items,
    })?;
    Ok(())
}

#[cfg(test)]
pub fn quest_state_label(state: u8) -> &'static str {
    match state {
        0 => "available",
        1 => "in progress",
        2 => "ready",
        3 => "cooldown",
        _ => "unknown",
    }
}

#[cfg(test)]
pub fn shop_category_label(category: u8) -> &'static str {
    match category {
        0 => "supply",
        1 => "gear",
        2 => "upgrade",
        _ => "other",
    }
}

#[cfg(test)]
pub fn format_shop(merit: i32, items: &[EdenShopEntry]) -> String {
    let mut lines = vec![format!("Merit {merit}.")];
    if items.is_empty() {
        lines.push("Nothing in the Eden shop.".to_owned());
        return lines.join("\n");
    }
    for item in items {
        let price = match (item.price_merit > 0, item.price_zeny > 0) {
            (true, true) => format!("{} merit + {} zeny", item.price_merit, item.price_zeny),
            (true, false) => format!("{} merit", item.price_merit),
            (false, _) => format!("{} zeny", item.price_zeny),
        };
        lines.push(format!(
            "slot {slot} {category} item {item} x{amount} lv {level}+ {price}",
            slot = item.slot,
            category = shop_category_label(item.category),
            item = item.item_id,
            amount = item.amount,
            level = item.min_level,
            price = price,
        ));
    }
    lines.join("\n")
}

pub fn format_eden_result(result: i16, quest_id: i32) -> String {
    let reason = match result {
        0 => "ok",
        1 => "Eden Group could not process that request",
        2 => "not available at your base level",
        3 => "finish or abandon the current Eden mission first",
        4 => "you are not on that mission",
        5 => "the hunt is not finished yet",
        6 => "missing the required items",
        7 => "that mission is still on cooldown",
        8 => "not enough inventory space for the reward",
        9 => "not enough Eden Merit Badges",
        _ => "unknown result",
    };
    format!("Eden result {result} ({reason}) quest {quest_id}")
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::ByteWriter;
    use ragnarok_packets::{EdenShopEntry, PacketExt, RequestEdenPacket};

    use super::{EdenButton, eden_button_packets, encode_eden_packet, format_eden_result, format_shop, quest_state_label};

    #[test]
    fn each_button_encodes_its_own_packet() {
        let cases = [
            (EdenButton::Accept(44), 1i16, 44i32, 1usize),
            (EdenButton::Complete(44), 2, 44, 1),
            (EdenButton::Abandon(44), 3, 44, 1),
            (EdenButton::Buy { slot: 3, amount: 2 }, 4, 3, 2),
        ];
        for (button, action, quest_id, count) in cases {
            let packets = eden_button_packets(button);
            assert_eq!(packets.len(), count);
            for packet in &packets {
                assert_eq!(packet.action, action);
                assert_eq!(packet.quest_id, quest_id);
                let mut writer = ByteWriter::new();
                packet.packet_to_bytes(&mut writer).unwrap();
                assert_eq!(writer.as_slice(), encode_eden_packet(action, quest_id));
            }
        }
        let direct = RequestEdenPacket { action: 4, quest_id: 3 };
        let mut writer = ByteWriter::new();
        direct.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 8);
        assert_eq!(writer.as_slice(), encode_eden_packet(4, 3));
    }

    #[test]
    fn parsers_and_result_text() {
        assert_eq!(quest_state_label(2), "ready");
        assert!(format_eden_result(9, 4).contains("Merit"));
        assert!(format_eden_result(0, 8).contains("ok"));

        let both = EdenShopEntry {
            slot: 1,
            category: 0,
            min_level: 1,
            item_id: 501,
            amount: 1,
            price_merit: 4,
            price_zeny: 100,
        };
        let merit_only = EdenShopEntry {
            price_zeny: 0,
            ..both.clone()
        };
        let zeny_only = EdenShopEntry {
            price_merit: 0,
            price_zeny: 100,
            ..both.clone()
        };
        let summary = format_shop(4, &[both, merit_only, zeny_only]);
        assert!(summary.contains("4 merit + 100 zeny"));
        assert!(summary.contains("4 merit"));
        assert!(summary.contains("100 zeny"));
    }
}
