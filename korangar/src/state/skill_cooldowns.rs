use hashbrown::HashMap;
use korangar_interface::element::StateElement;
use ragnarok_packets::{ClientTick, SkillId};
use rust_state::RustState;

/// Per-skill cooldown deadlines from `NetworkEvent::SkillCooldown`.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct SkillCooldowns {
    #[hidden_element]
    until_by_skill: HashMap<SkillId, ClientTick>,
}

impl SkillCooldowns {
    pub fn set(&mut self, skill_id: SkillId, until: ClientTick) {
        if until.0 == 0 {
            self.until_by_skill.remove(&skill_id);
        } else {
            self.until_by_skill.insert(skill_id, until);
        }
    }

    pub fn clear(&mut self) {
        self.until_by_skill.clear();
    }

    /// Remaining whole seconds for `skill_id` at `now`, if still cooling down.
    pub fn remaining_seconds(&self, skill_id: SkillId, now: ClientTick) -> Option<u32> {
        let until = self.until_by_skill.get(&skill_id)?;
        if until.0 <= now.0 {
            return None;
        }
        let remaining_ms = until.0.saturating_sub(now.0);
        Some(remaining_ms.div_ceil(1000).max(1))
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{ClientTick, SkillId};

    use super::SkillCooldowns;

    #[test]
    fn remaining_seconds_respects_deadline() {
        let mut cds = SkillCooldowns::default();
        cds.set(SkillId(42), ClientTick(5000));
        assert_eq!(cds.remaining_seconds(SkillId(42), ClientTick(2000)), Some(3));
        assert_eq!(cds.remaining_seconds(SkillId(42), ClientTick(5000)), None);
        assert_eq!(cds.remaining_seconds(SkillId(1), ClientTick(2000)), None);
        cds.set(SkillId(42), ClientTick(0));
        assert_eq!(cds.remaining_seconds(SkillId(42), ClientTick(2000)), None);
    }
}
