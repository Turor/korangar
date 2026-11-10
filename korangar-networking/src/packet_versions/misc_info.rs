//! Sense, read book, memorial dungeon, battleground, reputation, and
//! ammunition notices. Each packet becomes one [`NetworkEvent`]; the text
//! helpers here are shared with the client windows and unit tests.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    AccountId, AmmunitionActionPacket, AmmunitionActionType, BattlefieldCampInfoPacket, BattlefieldChatPacket, BattlefieldHp2Packet,
    BattlefieldHpPacket, BattlefieldPointsPacket, BattlefieldPositionPacket, EquipAmmunitionPacket, MemorialDungeonInfoPacket,
    MemorialDungeonNotifyPacket, MemorialDungeonSubscriptionInfoPacket, MemorialDungeonSubscriptionNotifyPacket, MonsterInfoPacket,
    ReadBookPacket, ReputationEntry, ReputationPacket,
};

use crate::event::{NetworkEvent, NetworkEventList};

/// One memorial dungeon packet (`0x02CB`–`0x02CE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemorialDungeonUpdate {
    /// Queued for creation at `priority` (1 is next).
    Subscription { name: String, priority: u16 },
    /// New queue position. Hercules sends a negative (wrapped) value when the
    /// reservation is cancelled.
    Priority { priority: u16 },
    /// Created. Unix seconds; zero means that limit is not running.
    Info {
        name: String,
        destroy_date: u32,
        enter_time_out_date: u32,
    },
    /// State change; see [`memorial_dungeon_notice`].
    Notify { notify_type: u32 },
}

/// One battleground packet (`0x02DC`–`0x02E0`, `0x0A0E`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattlefieldUpdate {
    Chat {
        account_id: AccountId,
        name: String,
        message: String,
    },
    Camp {
        account_id: AccountId,
        name: String,
        camp: u16,
    },
    Points {
        camp_a: u16,
        camp_b: u16,
    },
    Position {
        account_id: AccountId,
        name: String,
        class: u16,
        x: u16,
        y: u16,
    },
    Hp {
        account_id: AccountId,
        name: Option<String>,
        hp: i32,
        maximum_hp: i32,
    },
}

pub fn register_misc_info_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: MonsterInfoPacket| NetworkEvent::MonsterInfo { info: packet })?;
    packet_handler.register(|packet: ReadBookPacket| NetworkEvent::ReadBook {
        book_item_id: packet.book_item_id,
        page: packet.page,
    })?;
    packet_handler.register(|packet: MemorialDungeonSubscriptionInfoPacket| {
        NetworkEvent::MemorialDungeon(MemorialDungeonUpdate::Subscription {
            name: packet.name,
            priority: packet.priority,
        })
    })?;
    packet_handler.register(|packet: MemorialDungeonSubscriptionNotifyPacket| {
        NetworkEvent::MemorialDungeon(MemorialDungeonUpdate::Priority { priority: packet.priority })
    })?;
    packet_handler.register(|packet: MemorialDungeonInfoPacket| {
        NetworkEvent::MemorialDungeon(MemorialDungeonUpdate::Info {
            name: packet.name,
            destroy_date: packet.destroy_date,
            enter_time_out_date: packet.enter_time_out_date,
        })
    })?;
    packet_handler.register(|packet: MemorialDungeonNotifyPacket| {
        NetworkEvent::MemorialDungeon(MemorialDungeonUpdate::Notify {
            notify_type: packet.notify_type,
        })
    })?;
    packet_handler.register(|packet: BattlefieldChatPacket| {
        NetworkEvent::Battlefield(BattlefieldUpdate::Chat {
            account_id: packet.account_id,
            name: packet.name,
            message: packet.message,
        })
    })?;
    packet_handler.register(|packet: BattlefieldCampInfoPacket| {
        NetworkEvent::Battlefield(BattlefieldUpdate::Camp {
            account_id: packet.account_id,
            name: packet.name,
            camp: packet.camp,
        })
    })?;
    packet_handler.register(|packet: BattlefieldPointsPacket| {
        NetworkEvent::Battlefield(BattlefieldUpdate::Points {
            camp_a: packet.camp_a,
            camp_b: packet.camp_b,
        })
    })?;
    packet_handler.register(|packet: BattlefieldPositionPacket| {
        NetworkEvent::Battlefield(BattlefieldUpdate::Position {
            account_id: packet.account_id,
            name: packet.name,
            class: packet.class,
            x: packet.x,
            y: packet.y,
        })
    })?;
    packet_handler.register(|packet: BattlefieldHpPacket| {
        NetworkEvent::Battlefield(BattlefieldUpdate::Hp {
            account_id: packet.account_id,
            name: Some(packet.name),
            hp: i32::from(packet.hp),
            maximum_hp: i32::from(packet.maximum_hp),
        })
    })?;
    packet_handler.register(|packet: BattlefieldHp2Packet| {
        NetworkEvent::Battlefield(BattlefieldUpdate::Hp {
            account_id: packet.account_id,
            name: None,
            hp: packet.hp,
            maximum_hp: packet.maximum_hp,
        })
    })?;
    packet_handler.register(|packet: ReputationPacket| NetworkEvent::Reputation {
        success: packet.success != 0,
        entries: packet.entries,
    })?;
    packet_handler.register(|packet: EquipAmmunitionPacket| NetworkEvent::AmmunitionEquipped {
        index: packet.inventory_index,
    })?;
    packet_handler.register(|packet: AmmunitionActionPacket| NetworkEvent::AmmunitionAction {
        action: packet.action_type,
    })?;
    Ok(())
}

/// Hercules `clif_arrow_fail` messages (MsgStringTable 242–245).
pub fn ammunition_message(action: &AmmunitionActionType) -> &'static str {
    match action {
        AmmunitionActionType::EquipProperAmmunitionFirst => "Please equip the proper ammunition first.",
        AmmunitionActionType::WeightLimitExceeded1 => "You can't attack or use skills because your weight limit has been exceeded.",
        AmmunitionActionType::WeightLimitExceeded2 => "You can't use skills because your weight limit has been exceeded.",
        AmmunitionActionType::Equipped => "Ammunition has been equipped.",
    }
}

/// `true` for the messages that report a failure.
pub fn ammunition_is_error(action: &AmmunitionActionType) -> bool {
    !matches!(action, AmmunitionActionType::Equipped)
}

/// Hercules `clif_instance` `INSTANCE_WND_INFO_DESTROY` messages.
pub fn memorial_dungeon_notice(notify_type: u32) -> Option<&'static str> {
    match notify_type {
        1 => Some("The Memorial Dungeon expired; it has been destroyed."),
        2 => Some("The Memorial Dungeon's entry time limit expired; it has been destroyed."),
        3 => Some("The Memorial Dungeon has been removed."),
        4 => Some("The Memorial Dungeon could not be created."),
        _ => None,
    }
}

/// `true` when a notice ends the dungeon, so the window closes.
pub fn memorial_dungeon_notice_closes(notify_type: u32) -> bool {
    (1..=4).contains(&notify_type)
}

/// `h:mm:ss` until `deadline` (Unix seconds), or `None` when the limit is not
/// running (zero).
pub fn format_remaining(deadline: u32, now_unix: u64) -> Option<String> {
    if deadline == 0 {
        return None;
    }
    let remaining = u64::from(deadline).saturating_sub(now_unix);
    Some(format!(
        "{}:{:02}:{:02}",
        remaining / 3600,
        (remaining / 60) % 60,
        remaining % 60
    ))
}

const SIZE_NAMES: [&str; 3] = ["Small", "Medium", "Large"];
const RACE_NAMES: [&str; 12] = [
    "Formless",
    "Undead",
    "Brute",
    "Plant",
    "Insect",
    "Fish",
    "Demon",
    "Demi-Human",
    "Angel",
    "Dragon",
    "Player",
    "Doram",
];
pub const ELEMENT_NAMES: [&str; 10] = [
    "Neutral", "Water", "Earth", "Fire", "Wind", "Poison", "Holy", "Shadow", "Ghost", "Undead",
];

fn table_name(table: &[&'static str], value: u16) -> String {
    table
        .get(usize::from(value))
        .map(|name| (*name).to_owned())
        .unwrap_or_else(|| format!("Unknown ({value})"))
}

/// Sense window rows. `monster_name` comes from the client job table.
pub fn format_sense(info: &MonsterInfoPacket, monster_name: &str) -> String {
    let mut lines = vec![
        format!("{monster_name} (id {})", info.job),
        format!("Level: {}", info.level),
        format!("HP: {}", info.hp),
        format!("Size: {}", table_name(&SIZE_NAMES, info.size)),
        format!("Race: {}", table_name(&RACE_NAMES, info.race)),
        format!("Element: {}", table_name(&ELEMENT_NAMES, info.element)),
        format!("DEF: {}   MDEF: {}", info.defense, info.magic_defense),
        "Damage taken from:".to_owned(),
    ];
    let mut row = Vec::new();
    for (index, percent) in info.element_damage.iter().enumerate() {
        row.push(format!("{} {percent}%", ELEMENT_NAMES[index + 1]));
        if row.len() == 3 {
            lines.push(row.join("   "));
            row.clear();
        }
    }
    if !row.is_empty() {
        lines.push(row.join("   "));
    }
    lines.join("\n")
}

/// Reputation rows (`0x0B8D`).
pub fn format_reputation(success: bool, entries: &[ReputationEntry]) -> String {
    if !success {
        return "Reputation is not available.".to_owned();
    }
    if entries.is_empty() {
        return "No reputation yet.".to_owned();
    }
    entries
        .iter()
        .map(|entry| format!("Group {}: {} points", entry.reputation_type, entry.points))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poring() -> MonsterInfoPacket {
        MonsterInfoPacket {
            job: 1002,
            level: 1,
            size: 1,
            hp: 50,
            defense: 0,
            race: 3,
            magic_defense: 5,
            element: 1,
            element_damage: [25, 100, 90, 100, 100, 100, 100, 100, 100],
        }
    }

    #[test]
    fn sense_lists_stats_and_element_table() {
        let text = format_sense(&poring(), "Poring");
        assert!(text.starts_with("Poring (id 1002)"));
        assert!(text.contains("Size: Medium"));
        assert!(text.contains("Race: Plant"));
        assert!(text.contains("Element: Water"));
        assert!(text.contains("Water 25%"));
        assert!(text.contains("Fire 90%"));
        assert!(text.contains("Undead 100%"));
        assert_eq!(text.lines().count(), 8 + 3);
    }

    #[test]
    fn sense_survives_unknown_codes() {
        let mut info = poring();
        info.size = 9;
        info.race = 40;
        info.element = 12;
        let text = format_sense(&info, "Odd");
        assert!(text.contains("Size: Unknown (9)"));
        assert!(text.contains("Race: Unknown (40)"));
        assert!(text.contains("Element: Unknown (12)"));
    }

    #[test]
    fn memorial_notices_follow_hercules() {
        assert_eq!(memorial_dungeon_notice(0), None);
        assert!(memorial_dungeon_notice(1).unwrap().contains("expired"));
        assert!(memorial_dungeon_notice(2).unwrap().contains("entry time limit"));
        assert!(memorial_dungeon_notice(3).unwrap().contains("removed"));
        assert!(memorial_dungeon_notice_closes(4));
        assert!(!memorial_dungeon_notice_closes(0));
    }

    #[test]
    fn remaining_time_counts_down_and_stops_at_zero() {
        assert_eq!(format_remaining(0, 100), None);
        assert_eq!(format_remaining(100 + 3725, 100).as_deref(), Some("1:02:05"));
        assert_eq!(format_remaining(50, 100).as_deref(), Some("0:00:00"));
    }

    #[test]
    fn ammunition_messages_match_the_string_table() {
        assert_eq!(
            ammunition_message(&AmmunitionActionType::EquipProperAmmunitionFirst),
            "Please equip the proper ammunition first."
        );
        assert!(ammunition_is_error(&AmmunitionActionType::WeightLimitExceeded1));
        assert!(!ammunition_is_error(&AmmunitionActionType::Equipped));
    }

    #[test]
    fn reputation_rows() {
        assert_eq!(format_reputation(false, &[]), "Reputation is not available.");
        assert_eq!(format_reputation(true, &[]), "No reputation yet.");
        let rows = format_reputation(true, &[ReputationEntry {
            reputation_type: 1,
            points: -20,
        }]);
        assert_eq!(rows, "Group 1: -20 points");
    }
}
