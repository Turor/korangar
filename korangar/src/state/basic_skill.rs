//! Chat text for `ZC_ACK_TOUSESKILL` (`0x0110`) failures on Basic Skill.
//!
//! Hercules gates trading, emotions, sitting, chat rooms, and party creation
//! on `NV_BASIC` (`basic_skill_check`). It answers with `clif_skill_fail(sd, 1,
//! USESKILL_FAIL_LEVEL, btype, 0)` and nothing else, so without this text a
//! novice's "Create party" silently did nothing (G04B). roBrowser shows the
//! matching msgstringtable line (`Skill.js` `onSkillResult`).

/// `NV_BASIC`.
const BASIC_SKILL_ID: u16 = 1;
/// `USESKILL_FAIL_LEVEL`.
const FAIL_LEVEL: u8 = 0;

/// Error line for a failed `0x0110`, or `None` when the failure is not a
/// Basic Skill level gate.
pub fn basic_skill_failure_text(skill_id: u16, cause: u8, btype: i32) -> Option<&'static str> {
    if skill_id != BASIC_SKILL_ID || cause != FAIL_LEVEL {
        return None;
    }
    // `btype` is what Hercules passes from each `check_basicskill` site.
    Some(match btype {
        0 => "Basic Skill level 1 is required to trade.",
        1 => "Basic Skill level 2 is required to use emotions.",
        2 => "Basic Skill level 3 is required to sit.",
        3 => "Basic Skill level 4 is required to open a chat room.",
        4 => "Basic Skill level 7 is required to create a party.",
        _ => "Your Basic Skill level is too low.",
    })
}

#[cfg(test)]
mod tests {
    use super::basic_skill_failure_text;

    #[test]
    fn party_creation_gate_has_a_message() {
        // Hercules `clif_parse_CreateParty2`: skill_fail(sd, 1, USESKILL_FAIL_LEVEL, 4,
        // 0).
        assert_eq!(
            basic_skill_failure_text(1, 0, 4),
            Some("Basic Skill level 7 is required to create a party.")
        );
    }

    #[test]
    fn other_basic_skill_gates_have_messages() {
        assert!(basic_skill_failure_text(1, 0, 0).unwrap().contains("trade"));
        assert!(basic_skill_failure_text(1, 0, 2).unwrap().contains("sit"));
        assert!(basic_skill_failure_text(1, 0, 9).is_some());
    }

    #[test]
    fn unrelated_failures_stay_silent() {
        // Other skills and causes keep the existing no-spam behavior.
        assert_eq!(basic_skill_failure_text(28, 0, 4), None);
        assert_eq!(basic_skill_failure_text(1, 1, 4), None);
    }
}
