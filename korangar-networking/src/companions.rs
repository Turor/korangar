//! Capture mode, companion rename checks, egg indexes, and skill-up bytes.
//!
//! The pet and homunculus panels call these helpers. Packet layouts live in
//! `ragnarok_packets::companion_actions`.

use ragnarok_bytes::ByteWriter;
use ragnarok_packets::{LevelUpSkillPacket, PacketExt, SkillId};

/// UTF-8 byte limit for `CZ_RENAME_PET` and `CZ_RENAME_MER` (`NAME_LENGTH`).
pub const COMPANION_NAME_BYTES: usize = 24;

pub const CAPTURE_IDLE: u8 = 0;
pub const CAPTURE_ARMED: u8 = 1;
pub const CAPTURE_SUCCEEDED: u8 = 2;
pub const CAPTURE_FAILED: u8 = 3;

/// `ZC_CHANGESTATE_MER` state: intimacy.
pub const HOM_STATE_INTIMACY: i8 = 1;
/// `ZC_CHANGESTATE_MER` state: hunger.
pub const HOM_STATE_HUNGER: i8 = 2;

/// One skill row after the 37-byte list packet is parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompanionSkillSnapshot {
    pub skill_id: u16,
    pub skill_type: i32,
    pub level: i16,
    pub sp_cost: i16,
    pub attack_range: i16,
    pub name: String,
    pub upgradable: bool,
}

/// Server packets that are not the existing pet/homun/merc property blocks.
#[derive(Debug)]
pub enum CompanionNotice {
    /// `ZC_START_CAPTURE` (`0x019E`).
    StartCapture,
    /// `ZC_TRYCAPTURE_MONSTER` (`0x01A0`).
    CaptureResult { success: bool },
    /// `ZC_PETEGG_LIST` (`0x01A6`). Values are the wire indexes (`inventory +
    /// 2`).
    PetEggs { indices: Vec<u16> },
    /// `ZC_CHANGESTATE_MER` (`0x0230`).
    HomunculusState { kind: i8, entity_id: u32, value: i32 },
    /// `ZC_HOSKILLINFO_LIST` (`0x0235`).
    HomunculusSkills { skills: Vec<CompanionSkillSnapshot> },
    /// `ZC_HOSKILLINFO_UPDATE` (`0x0239`).
    HomunculusSkillUpdate {
        skill_id: u16,
        level: i16,
        sp_cost: i16,
        attack_range: i16,
        upgradable: bool,
    },
    /// `ZC_MER_SKILLINFO_LIST` (`0x029D`).
    MercenarySkills { skills: Vec<CompanionSkillSnapshot> },
    /// `ZC_MER_SKILLINFO_UPDATE` (`0x029E`).
    MercenarySkillUpdate {
        skill_id: u16,
        level: i16,
        sp_cost: i16,
        attack_range: i16,
        upgradable: bool,
    },
}

pub fn skill_snapshot(
    skill_id: u16,
    skill_type: i32,
    level: i16,
    sp_cost: i16,
    attack_range: i16,
    name: String,
    upgradable: u8,
) -> CompanionSkillSnapshot {
    CompanionSkillSnapshot {
        skill_id,
        skill_type,
        level,
        sp_cost,
        attack_range,
        name,
        upgradable: upgradable != 0,
    }
}

pub fn capture_is_armed(phase: u8) -> bool {
    phase == CAPTURE_ARMED
}

/// `ZC_START_CAPTURE` arms capture. A later start replaces a finished attempt.
pub fn capture_arm() -> u8 {
    CAPTURE_ARMED
}

/// Monster click while armed sends `0x019F` and stays armed until the result
/// or Escape. A click that is not a monster does not send and does not disarm.
pub fn capture_click(phase: u8, monster: bool) -> (u8, bool) {
    if phase == CAPTURE_ARMED && monster {
        (CAPTURE_ARMED, true)
    } else {
        (phase, false)
    }
}

/// Escape cancels only an armed capture. Success and failure stay put.
pub fn capture_cancel(phase: u8) -> Option<u8> {
    if phase == CAPTURE_ARMED { Some(CAPTURE_IDLE) } else { None }
}

pub fn capture_result(success: bool) -> u8 {
    if success { CAPTURE_SUCCEEDED } else { CAPTURE_FAILED }
}

/// Trim, reject empty and control bytes, and keep the UTF-8 size at 24 bytes.
pub fn validate_companion_name(name: &str) -> Result<String, &'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Enter a name.");
    }
    if trimmed.chars().any(|character| character.is_control()) {
        return Err("Name has invalid characters.");
    }
    if trimmed.len() > COMPANION_NAME_BYTES {
        return Err("Name is longer than 24 bytes.");
    }
    Ok(trimmed.to_owned())
}

/// Same 24-byte insert the text box uses. Returns whether the draft changed.
pub fn append_name_character(text: &mut String, character: char) -> bool {
    if character == '\u{8}' {
        text.pop().is_some()
    } else if character.is_control() {
        false
    } else if text.len().saturating_add(character.len_utf8()) <= COMPANION_NAME_BYTES {
        text.push(character);
        true
    } else {
        false
    }
}

/// Wire index from `ZC_PETEGG_LIST` is the inventory index plus 2.
pub fn egg_inventory_index(wire_index: u16) -> Option<u16> {
    wire_index.checked_sub(2)
}

/// `CZ_UPGRADE_SKILLLEVEL` (`0x0112`), 4 bytes. Homunculus skill ids are routed
/// to `homun->skillup` by `pc_skillup`.
///
/// `0x0237` is `CZ_RANKING_PK`. The 20250716 length table gives it 2 bytes and
/// no skill id, so skill-up must not send it.
pub fn homunculus_skill_up_bytes(skill_id: u16) -> Vec<u8> {
    let packet = LevelUpSkillPacket {
        skill_id: SkillId(skill_id),
    };
    let mut writer = ByteWriter::new();
    packet.packet_to_bytes(&mut writer).expect("0x0112 skill-up encodes");
    writer.into_inner()
}

/// Seconds left on a mercenary contract. `expire` is a unix timestamp.
pub fn mercenary_remaining(expire: i32, now: i64) -> String {
    if expire <= 0 {
        return "No contract.".to_owned();
    }
    let left = i64::from(expire) - now;
    if left <= 0 {
        return "Contract ended.".to_owned();
    }
    let hours = left / 3600;
    let minutes = (left % 3600) / 60;
    let seconds = left % 60;
    format!("{hours}:{minutes:02}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_mode_stays_armed_until_result_or_escape() {
        assert!(!capture_is_armed(CAPTURE_IDLE));
        let armed = capture_arm();
        assert!(capture_is_armed(armed));

        let (phase, send) = capture_click(armed, true);
        assert!(send);
        assert_eq!(phase, CAPTURE_ARMED);
        let (phase, send) = capture_click(phase, true);
        assert!(send);
        assert_eq!(phase, CAPTURE_ARMED);

        let (phase, send) = capture_click(phase, false);
        assert!(!send);
        assert_eq!(phase, CAPTURE_ARMED);

        assert_eq!(capture_cancel(phase), Some(CAPTURE_IDLE));
        assert_eq!(capture_cancel(CAPTURE_IDLE), None);
        assert_eq!(capture_cancel(CAPTURE_SUCCEEDED), None);

        assert_eq!(capture_result(true), CAPTURE_SUCCEEDED);
        assert_eq!(capture_result(false), CAPTURE_FAILED);
        let (phase, send) = capture_click(CAPTURE_FAILED, true);
        assert!(!send);
        assert_eq!(phase, CAPTURE_FAILED);
        assert_eq!(capture_arm(), CAPTURE_ARMED);
    }

    #[test]
    fn rename_rejects_empty_control_and_overlong_names() {
        assert_eq!(validate_companion_name("  Picky  ").as_deref(), Ok("Picky"));
        assert_eq!(
            validate_companion_name("abcdefghijabcdefghijabcd").as_deref(),
            Ok("abcdefghijabcdefghijabcd")
        );
        assert!(validate_companion_name("").is_err());
        assert!(validate_companion_name("   ").is_err());
        assert!(validate_companion_name("bad\nname").is_err());
        assert!(validate_companion_name("abcdefghijabcdefghijabcde").is_err());
        let hangul = "한".repeat(8);
        assert_eq!(hangul.len(), 24);
        assert!(validate_companion_name(&hangul).is_ok());
        assert!(validate_companion_name(&("한".repeat(9))).is_err());

        let mut draft = String::new();
        assert!(append_name_character(&mut draft, '한'));
        assert_eq!(draft, "한");
        assert!(append_name_character(&mut draft, '\u{8}'));
        assert!(draft.is_empty());
        for _ in 0..8 {
            assert!(append_name_character(&mut draft, '한'));
        }
        assert!(!append_name_character(&mut draft, '한'));
        assert_eq!(draft.len(), 24);
    }

    #[test]
    fn egg_wire_index_and_skill_up_bytes() {
        assert_eq!(egg_inventory_index(4), Some(2));
        assert_eq!(egg_inventory_index(2), Some(0));
        assert_eq!(egg_inventory_index(1), None);

        let bytes = homunculus_skill_up_bytes(8004);
        assert_eq!(bytes, [0x12, 0x01, 0x44, 0x1F]);
        assert_ne!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x0237);
    }

    #[test]
    fn mercenary_countdown_uses_the_expire_timestamp() {
        assert_eq!(mercenary_remaining(0, 100), "No contract.");
        assert_eq!(mercenary_remaining(1_000, 1_000), "Contract ended.");
        assert_eq!(mercenary_remaining(1_000, 900), "0:01:40");
        assert_eq!(mercenary_remaining(10_000, 1_000), "2:30:00");
    }
}
