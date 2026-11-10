//! Turoran Eden Group board (`0x0EF3`, `0x0EFB`–`0x0EFD`).
//!
//! Layouts follow the Turoran browser builders. Actions are list (0), accept
//! (1), complete (2), abandon (3), and buy (4). Buy puts the shop slot in the
//! quest-id field.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{EdenQuestEntry, EdenQuestListPacket, EdenResultPacket, EdenShopEntry, EdenShopPacket};

use crate::event::{NetworkEvent, NetworkEventList};

pub const EDEN_ACTION_LIST: i16 = 0;
pub const EDEN_ACTION_ACCEPT: i16 = 1;
pub const EDEN_ACTION_COMPLETE: i16 = 2;
pub const EDEN_ACTION_ABANDON: i16 = 3;
pub const EDEN_ACTION_BUY: i16 = 4;

pub fn register_eden_packets<Callback>(packet_handler: &mut PacketHandler<NetworkEventList, Callback>) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: EdenQuestListPacket| NetworkEvent::EdenQuestList {
        base_level: packet.base_level,
        merit: packet.merit,
        summary: format_quest_list(packet.base_level, packet.merit, &packet.quests),
    })?;
    packet_handler.register(|packet: EdenResultPacket| NetworkEvent::EdenResult {
        result: packet.result,
        quest_id: packet.quest_id,
        summary: format_eden_result(packet.result, packet.quest_id),
    })?;
    packet_handler.register(|packet: EdenShopPacket| NetworkEvent::EdenShop {
        merit: packet.merit,
        summary: format_shop(packet.merit, &packet.items),
    })?;
    Ok(())
}

pub fn parse_eden_id(text: &str) -> Option<i32> {
    text.trim().parse().ok()
}

pub fn format_quest_list(base_level: i16, merit: i32, quests: &[EdenQuestEntry]) -> String {
    let mut lines = vec![format!("Base level {base_level}. Merit {merit}.")];
    if quests.is_empty() {
        lines.push("No Eden missions.".to_owned());
        return lines.join("\n");
    }
    for quest in quests {
        lines.push(format!(
            "#{id} {name} [{state}] lv {min}-{max} hunt {have}/{need} mob {mob} item {item} {held}/{required} map {map} ({spawns}) exp \
             {base}/{job} reward {reward}x{amount} merit {merit} — {hint}",
            id = quest.quest_id,
            name = quest.name,
            state = quest_state_label(quest.state),
            min = quest.min_level,
            max = quest.max_level,
            have = quest.hunt_count,
            need = quest.hunt_max,
            mob = quest.mob_id,
            item = quest.item_id,
            held = quest.item_have,
            required = quest.item_need,
            map = quest.map,
            spawns = quest.spawn_count,
            base = quest.base_exp,
            job = quest.job_exp,
            reward = quest.reward_item,
            amount = quest.reward_item_amount,
            merit = quest.merit,
            hint = quest.hint,
        ));
    }
    lines.join("\n")
}

pub fn quest_state_label(state: u8) -> &'static str {
    match state {
        0 => "available",
        1 => "in progress",
        2 => "ready",
        3 => "cooldown",
        _ => "unknown",
    }
}

pub fn shop_category_label(category: u8) -> &'static str {
    match category {
        0 => "supply",
        1 => "gear",
        2 => "upgrade",
        _ => "other",
    }
}

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
    use ragnarok_packets::EdenShopEntry;

    use super::{format_eden_result, format_shop, parse_eden_id, quest_state_label};

    #[test]
    fn parsers_and_result_text() {
        assert_eq!(parse_eden_id(" 12 "), Some(12));
        assert_eq!(parse_eden_id("-3"), Some(-3));
        assert_eq!(parse_eden_id("slot"), None);
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
