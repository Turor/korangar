//! Small map-server notices with their own windows or chat feedback:
//! Sense (`0x018C`), read book (`0x0294`), memorial dungeons
//! (`0x02CB`–`0x02CF`), and battlegrounds (`0x02DB`–`0x02E0`, `0x0A0E`).
//!
//! Layouts follow Hercules `packets_struct.h` and the 20250716 length table:
//! `0x018C` 29, `0x0294` 10, `0x02CB` 65, `0x02CC` 4, `0x02CD` 71, `0x02CE`
//! 10, `0x02CF` 6, `0x02DC` variable, `0x02DD` 32, `0x02DE` 6, `0x02DF` 36,
//! `0x02E0` 34, `0x0A0E` 14.

use ragnarok_bytes::{FromBytes, ToBytes};
use ragnarok_macros::{ClientPacket, MapServer, Packet, ServerPacket};

use crate::AccountId;

/// `ZC_MONSTER_INFO`: the Sense / Monster Information result.
///
/// `element` is the defense element (0 neutral … 9 undead). The nine
/// `element_damage` bytes are the damage percent taken from water, earth,
/// fire, wind, poison, holy, shadow, ghost, and undead attacks (Hercules
/// `attr_ratio(i + 1, …)`, negative values capped at 0).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x018C)]
pub struct MonsterInfoPacket {
    pub job: u16,
    pub level: u16,
    pub size: u16,
    pub hp: u32,
    pub defense: u16,
    pub race: u16,
    pub magic_defense: u16,
    pub element: u16,
    pub element_damage: [u8; 9],
}

/// `ZC_READ_BOOK`: open a book item at a page.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0294)]
pub struct ReadBookPacket {
    pub book_item_id: u32,
    pub page: u32,
}

/// `ZC_MEMORIALDUNGEON_SUBSCRIPTION_INFO`: the dungeon is queued for creation.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02CB)]
pub struct MemorialDungeonSubscriptionInfoPacket {
    #[length(61)]
    pub name: String,
    pub priority: u16,
}

/// `ZC_MEMORIALDUNGEON_SUBSCRIPTION_NOTIFY`: new place in the creation queue.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02CC)]
pub struct MemorialDungeonSubscriptionNotifyPacket {
    pub priority: u16,
}

/// `ZC_MEMORIALDUNGEON_INFO`: the dungeon exists. Both times are Unix
/// seconds; zero means no limit.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02CD)]
pub struct MemorialDungeonInfoPacket {
    #[length(61)]
    pub name: String,
    pub destroy_date: u32,
    pub enter_time_out_date: u32,
}

/// `ZC_MEMORIALDUNGEON_NOTIFY`: the dungeon changed state (see
/// `memorial_dungeon_notice`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02CE)]
pub struct MemorialDungeonNotifyPacket {
    pub notify_type: u32,
    pub enter_limit_date: u32,
}

/// `CZ_MEMORIALDUNGEON_COMMAND`. Hercules only handles `3`
/// (`COMMAND_MEMORIALDUNGEON_DESTROY_FORCE`); other values are ignored.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02CF)]
pub struct MemorialDungeonCommandPacket {
    pub command: u32,
}

/// `COMMAND_MEMORIALDUNGEON_DESTROY_FORCE`.
pub const MEMORIAL_DUNGEON_COMMAND_DESTROY: u32 = 3;

/// `ZC_BATTLEFIELD_CHAT`: team chat inside a battleground.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02DC)]
#[variable_length]
pub struct BattlefieldChatPacket {
    pub account_id: AccountId,
    #[length(24)]
    pub name: String,
    #[length_remaining]
    pub message: String,
}

/// `ZC_BATTLEFIELD_NOTIFY_CAMPINFO`: which camp a player is on.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02DD)]
pub struct BattlefieldCampInfoPacket {
    pub account_id: AccountId,
    #[length(24)]
    pub name: String,
    pub camp: u16,
}

/// `ZC_BATTLEFIELD_NOTIFY_POINT`: the score board.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02DE)]
pub struct BattlefieldPointsPacket {
    pub camp_a: u16,
    pub camp_b: u16,
}

/// `ZC_BATTLEFIELD_NOTIFY_POSITION`: a team mate on the minimap.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02DF)]
pub struct BattlefieldPositionPacket {
    pub account_id: AccountId,
    #[length(24)]
    pub name: String,
    pub class: u16,
    pub x: u16,
    pub y: u16,
}

/// `ZC_BATTLEFIELD_NOTIFY_HP` (16-bit HP).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02E0)]
pub struct BattlefieldHpPacket {
    pub account_id: AccountId,
    #[length(24)]
    pub name: String,
    pub hp: u16,
    pub maximum_hp: u16,
}

/// `ZC_BATTLEFIELD_NOTIFY_HP2` (32-bit HP, no name).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A0E)]
pub struct BattlefieldHp2Packet {
    pub account_id: AccountId,
    pub hp: i32,
    pub maximum_hp: i32,
}

/// `CZ_BATTLEFIELD_CHAT`: send to the battleground team.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02DB)]
#[variable_length]
pub struct BattlefieldChatRequestPacket {
    #[length_remaining]
    pub message: String,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;
    use crate::PacketExt;

    fn bytes_of(packet: &impl PacketExt) -> Vec<u8> {
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        writer.as_slice().to_vec()
    }

    #[test]
    fn wire_lengths_match_hercules() {
        let sense = MonsterInfoPacket {
            job: 1002,
            level: 1,
            size: 1,
            hp: 50,
            defense: 0,
            race: 2,
            magic_defense: 5,
            element: 21,
            element_damage: [100; 9],
        };
        assert_eq!(bytes_of(&sense).len(), 29);
        assert_eq!(
            bytes_of(&ReadBookPacket {
                book_item_id: 7001,
                page: 1
            })
            .len(),
            10
        );
        let subscription = MemorialDungeonSubscriptionInfoPacket {
            name: "Endless Tower".to_owned(),
            priority: 2,
        };
        assert_eq!(bytes_of(&subscription).len(), 65);
        assert_eq!(bytes_of(&MemorialDungeonSubscriptionNotifyPacket { priority: 1 }).len(), 4);
        let info = MemorialDungeonInfoPacket {
            name: "Endless Tower".to_owned(),
            destroy_date: 1,
            enter_time_out_date: 2,
        };
        assert_eq!(bytes_of(&info).len(), 71);
        assert_eq!(
            bytes_of(&MemorialDungeonNotifyPacket {
                notify_type: 1,
                enter_limit_date: 0
            })
            .len(),
            10
        );
        assert_eq!(
            bytes_of(&MemorialDungeonCommandPacket {
                command: MEMORIAL_DUNGEON_COMMAND_DESTROY
            })
            .len(),
            6
        );
        let camp = BattlefieldCampInfoPacket {
            account_id: AccountId(1),
            name: "a".to_owned(),
            camp: 1,
        };
        assert_eq!(bytes_of(&camp).len(), 32);
        assert_eq!(bytes_of(&BattlefieldPointsPacket { camp_a: 1, camp_b: 2 }).len(), 6);
        let position = BattlefieldPositionPacket {
            account_id: AccountId(1),
            name: "a".to_owned(),
            class: 0,
            x: 1,
            y: 2,
        };
        assert_eq!(bytes_of(&position).len(), 36);
        let hp = BattlefieldHpPacket {
            account_id: AccountId(1),
            name: "a".to_owned(),
            hp: 1,
            maximum_hp: 2,
        };
        assert_eq!(bytes_of(&hp).len(), 34);
        let hp2 = BattlefieldHp2Packet {
            account_id: AccountId(1),
            hp: 1,
            maximum_hp: 2,
        };
        assert_eq!(bytes_of(&hp2).len(), 14);
    }

    #[test]
    fn sense_round_trips() {
        let sense = MonsterInfoPacket {
            job: 1002,
            level: 3,
            size: 1,
            hp: 55,
            defense: 2,
            race: 2,
            magic_defense: 5,
            element: 21,
            element_damage: [25, 100, 90, 100, 100, 100, 100, 100, 100],
        };
        let bytes = bytes_of(&sense);
        let mut reader = ByteReader::without_metadata(&bytes);
        let parsed = MonsterInfoPacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(parsed.job, 1002);
        assert_eq!(parsed.hp, 55);
        assert_eq!(parsed.element_damage[0], 25);
    }

    #[test]
    fn battlefield_chat_reads_the_trailing_message() {
        let chat = BattlefieldChatPacket {
            account_id: AccountId(7),
            name: "Guard".to_owned(),
            message: "Guard : go".to_owned(),
        };
        let bytes = bytes_of(&chat);
        // opcode + length + account + name + message + NUL (Hercules sends one too).
        assert_eq!(bytes.len(), 2 + 2 + 4 + 24 + "Guard : go".len() + 1);
    }
}
