//! Turoran homunculus and pet stable (`0x0EF0`–`0x0EF2`, `0x0EF6`,
//! `0x0EF8`–`0x0EFA`).
//!
//! Layouts follow the Turoran browser builders. `0x0EF7` is unused.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    StableHomunculusEntry, StableHomunculusListPacket, StableOwnedListPacket, StablePetEntry, StablePetListPacket, StableResultPacket,
};

use crate::event::{NetworkEvent, NetworkEventList};

pub fn register_stable_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: StableHomunculusListPacket| NetworkEvent::StableHomunculusList {
        active_id: packet.active_homunculus_id,
        summary: format_homunculus_list(packet.active_homunculus_id, &packet.homunculi),
    })?;
    packet_handler.register(|packet: StablePetListPacket| NetworkEvent::StablePetList {
        active_id: packet.active_pet_id,
        summary: format_pet_list(packet.active_pet_id, &packet.pets),
    })?;
    packet_handler.register(|packet: StableOwnedListPacket| NetworkEvent::StableOwnedList {
        summary: format_owned_list(&packet.items),
    })?;
    packet_handler.register(|packet: StableResultPacket| NetworkEvent::StableResult {
        result: packet.result,
        id: packet.id,
        summary: format_stable_result(packet.result, packet.id),
    })?;
    Ok(())
}

pub fn parse_stable_id(text: &str) -> Option<i32> {
    text.trim().parse().ok()
}

pub fn parse_stable_action(text: &str) -> Option<i16> {
    text.trim().parse().ok()
}

pub fn format_homunculus_list(active_id: i32, entries: &[StableHomunculusEntry]) -> String {
    if entries.is_empty() {
        return format!("Active homunculus {active_id}. None stored.");
    }
    let mut lines = vec![format!("Active homunculus {active_id}")];
    for entry in entries {
        lines.push(format!(
            "#{id} {name} class {class} (prev {prev}) lv {level} intimacy {intimacy} hunger {hunger} vaporize {vaporize} rename {rename}",
            id = entry.homunculus_id,
            name = entry.name,
            class = entry.class_id,
            prev = entry.previous_class,
            level = entry.level,
            intimacy = entry.intimacy,
            hunger = entry.hunger,
            vaporize = entry.vaporize,
            rename = entry.rename_flag,
        ));
    }
    lines.join("\n")
}

pub fn format_pet_list(active_id: i32, entries: &[StablePetEntry]) -> String {
    if entries.is_empty() {
        return format!("Active pet {active_id}. None stored.");
    }
    let mut lines = vec![format!("Active pet {active_id}")];
    for entry in entries {
        lines.push(format!(
            "#{id} {name} class {class} lv {level} intimacy {intimacy} hunger {hunger} egg {egg} food {food} owner {owner}",
            id = entry.pet_id,
            name = entry.name,
            class = entry.class_id,
            level = entry.level,
            intimacy = entry.intimacy,
            hunger = entry.hunger,
            egg = entry.egg_item_id,
            food = entry.food_item_id,
            owner = entry.owner_name,
        ));
    }
    lines.join("\n")
}

pub fn format_owned_list(items: &[ragnarok_packets::StableOwnedEntry]) -> String {
    if items.is_empty() {
        return "No evolution materials.".to_owned();
    }
    items
        .iter()
        .map(|item| format!("item {id} owned {owned}", id = item.item_id, owned = item.owned))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn format_stable_result(result: i16, id: i32) -> String {
    format!("Stable result {result} (id {id})")
}

#[cfg(test)]
mod tests {
    use super::{format_stable_result, parse_stable_action, parse_stable_id};

    #[test]
    fn parsers_keep_signed_ids_and_actions() {
        assert_eq!(parse_stable_id(" -4 "), Some(-4));
        assert_eq!(parse_stable_id("nope"), None);
        assert_eq!(parse_stable_action("2"), Some(2));
        assert_eq!(parse_stable_action("99999"), None);
        assert_eq!(format_stable_result(1, 9), "Stable result 1 (id 9)");
    }
}
