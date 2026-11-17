//! Skill cooldowns and the global after-cast delay (Matthew t274u).
//!
//! Sources on PACKETVER 20250716 (Hercules TEST):
//! - `ZC_SKILL_POSTDELAY` (`0x043D`): one skill's cooldown. `DelayTM` is a
//!   duration in milliseconds from now (Hercules `clif_skill_cooldown` sends
//!   the `blockpc_start` duration), not a tick.
//! - `ZC_SKILL_POSTDELAY_LIST2` (`0x0985`): every running cooldown on login,
//!   `{SKID, totalDelay, remainDelay}` per entry (`clif_skill_cooldown_list`).
//! - After-cast delay: Hercules has no dedicated packet. When
//!   `display_status_timers` is on (TEST: `conf/map/battle/client.conf`) it
//!   sends the status icon `EFST_POSTDELAY` (46) through `ZC_MSG_STATE_CHANGE`
//!   (`0x0983`) with the `skill->delay_fix` duration at `skill_castend_id` /
//!   `skill_castend_pos`. roBrowser does the same: `StatusConst.POSTDELAY`
//!   calls `ShortCut.setGlobalSkillDelay(RemainMS)`.
//!
//! Times are client-tick milliseconds and compared with wrapping arithmetic.

use hashbrown::HashMap;
use korangar_interface::element::StateElement;
use ragnarok_packets::{ClientTick, SkillId};
use rust_state::RustState;

/// `EFST_POSTDELAY` / `SI_POSTDELAY`: the after-cast delay status icon.
pub const EFST_POSTDELAY: u16 = 46;

/// One running timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cooldown {
    pub started_at: u32,
    pub total_ms: u32,
}

/// A timer as drawn at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CooldownView {
    pub remaining_ms: u32,
    pub total_ms: u32,
    /// 1.0 right after the skill was used, 0.0 when it is ready.
    pub fraction_remaining: f32,
}

impl CooldownView {
    /// Whole seconds left, rounded up (`3` for 2.1 s).
    pub fn remaining_seconds(&self) -> u32 {
        self.remaining_ms.div_ceil(1000).max(1)
    }
}

impl Cooldown {
    /// A timer with `remaining_ms` left out of `total_ms` at `now`.
    pub fn new(remaining_ms: u32, total_ms: u32, now: u32) -> Self {
        let total_ms = total_ms.max(remaining_ms);
        Self {
            started_at: now.wrapping_sub(total_ms - remaining_ms),
            total_ms,
        }
    }

    pub fn view(&self, now: u32) -> Option<CooldownView> {
        let elapsed = now.wrapping_sub(self.started_at);
        // A tick before the start (clock adjust) counts as just started.
        let elapsed = if elapsed > u32::MAX / 2 { 0 } else { elapsed };
        if self.total_ms == 0 || elapsed >= self.total_ms {
            return None;
        }
        let remaining_ms = self.total_ms - elapsed;
        Some(CooldownView {
            remaining_ms,
            total_ms: self.total_ms,
            fraction_remaining: remaining_ms as f32 / self.total_ms as f32,
        })
    }

    fn ends_at(&self) -> u32 {
        self.started_at.wrapping_add(self.total_ms)
    }
}

/// What a hotbar skill slot shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SlotDelay {
    /// The skill's own cooldown (`0x043D` / `0x0985`): sweep plus seconds.
    Cooldown(CooldownView),
    /// Only the global after-cast delay: a dimmed sweep.
    AfterCast(CooldownView),
}

impl SlotDelay {
    pub fn view(&self) -> CooldownView {
        match self {
            Self::Cooldown(view) | Self::AfterCast(view) => *view,
        }
    }
}

/// Per-skill cooldowns and the global after-cast delay.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct SkillCooldowns {
    #[hidden_element]
    by_skill: HashMap<SkillId, Cooldown>,
    #[hidden_element]
    after_cast: Option<Cooldown>,
}

impl SkillCooldowns {
    /// `0x043D`: `skill_id` cools down for `delay_ms` from `now`. Zero clears
    /// it.
    pub fn set(&mut self, skill_id: SkillId, delay_ms: u32, now: ClientTick) {
        self.set_remaining(skill_id, delay_ms, delay_ms, now);
    }

    /// `0x0985` entry: `remaining_ms` of `total_ms` left.
    pub fn set_remaining(&mut self, skill_id: SkillId, remaining_ms: u32, total_ms: u32, now: ClientTick) {
        if remaining_ms == 0 {
            self.by_skill.remove(&skill_id);
        } else {
            self.by_skill.insert(skill_id, Cooldown::new(remaining_ms, total_ms, now.0));
        }
    }

    /// `EFST_POSTDELAY` started: every skill waits `remaining_ms`. Like
    /// roBrowser, a shorter delay does not cut a longer running one short.
    pub fn start_after_cast(&mut self, remaining_ms: u32, total_ms: u32, now: ClientTick) {
        if remaining_ms == 0 {
            return;
        }
        let next = Cooldown::new(remaining_ms, total_ms, now.0);
        let keep_current = self
            .after_cast
            .is_some_and(|current| current.view(now.0).is_some() && current.ends_at().wrapping_sub(next.ends_at()) < u32::MAX / 2);
        if !keep_current {
            self.after_cast = Some(next);
        }
    }

    /// `EFST_POSTDELAY` ended.
    pub fn end_after_cast(&mut self) {
        self.after_cast = None;
    }

    pub fn clear(&mut self) {
        self.by_skill.clear();
        self.after_cast = None;
    }

    pub fn skill(&self, skill_id: SkillId, now: ClientTick) -> Option<CooldownView> {
        self.by_skill.get(&skill_id)?.view(now.0)
    }

    pub fn after_cast(&self, now: ClientTick) -> Option<CooldownView> {
        self.after_cast?.view(now.0)
    }

    /// The overlay for a hotbar skill slot: its own cooldown when that ends
    /// later than the after-cast delay, else the after-cast delay.
    pub fn slot_delay(&self, skill_id: SkillId, now: ClientTick) -> Option<SlotDelay> {
        match (self.skill(skill_id, now), self.after_cast(now)) {
            (Some(own), Some(global)) if global.remaining_ms > own.remaining_ms => Some(SlotDelay::AfterCast(global)),
            (Some(own), _) => Some(SlotDelay::Cooldown(own)),
            (None, Some(global)) => Some(SlotDelay::AfterCast(global)),
            (None, None) => None,
        }
    }

    /// Forget timers that have run out.
    pub fn prune(&mut self, now: ClientTick) {
        self.by_skill.retain(|_, cooldown| cooldown.view(now.0).is_some());
        if self.after_cast.is_some_and(|cooldown| cooldown.view(now.0).is_none()) {
            self.after_cast = None;
        }
    }

    /// Running cooldowns, sorted by skill, for the autopilot dump.
    pub fn describe(&self, now: ClientTick) -> String {
        let mut rows: Vec<(u16, CooldownView)> = self
            .by_skill
            .iter()
            .filter_map(|(skill_id, cooldown)| cooldown.view(now.0).map(|view| (skill_id.0, view)))
            .collect();
        rows.sort_by_key(|(skill_id, _)| *skill_id);
        let mut parts: Vec<String> = rows
            .into_iter()
            .map(|(skill_id, view)| format!("skill {skill_id} {}/{} ms", view.remaining_ms, view.total_ms))
            .collect();
        if let Some(view) = self.after_cast(now) {
            parts.push(format!("after_cast {}/{} ms", view.remaining_ms, view.total_ms));
        }
        format!("[{}]", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{ClientTick, SkillId};

    use super::*;

    #[test]
    fn postdelay_is_a_duration_from_now() {
        let mut cds = SkillCooldowns::default();
        cds.set(SkillId(42), 5_000, ClientTick(1_000_000));
        let view = cds.skill(SkillId(42), ClientTick(1_002_000)).unwrap();
        assert_eq!(view.remaining_ms, 3_000);
        assert_eq!(view.total_ms, 5_000);
        assert!((view.fraction_remaining - 0.6).abs() < 1e-6);
        assert_eq!(
            cds.skill(SkillId(42), ClientTick(1_002_000)).map(|view| view.remaining_seconds()),
            Some(3)
        );
        assert_eq!(
            cds.skill(SkillId(42), ClientTick(1_004_100)).map(|view| view.remaining_seconds()),
            Some(1)
        );
        assert!(cds.skill(SkillId(42), ClientTick(1_005_000)).is_none());
        assert!(cds.skill(SkillId(1), ClientTick(1_002_000)).is_none());
        cds.set(SkillId(42), 0, ClientTick(1_002_000));
        assert!(cds.skill(SkillId(42), ClientTick(1_002_000)).is_none());
    }

    #[test]
    fn login_list_keeps_the_elapsed_part() {
        let mut cds = SkillCooldowns::default();
        cds.set_remaining(SkillId(7), 2_500, 10_000, ClientTick(500));
        let view = cds.skill(SkillId(7), ClientTick(500)).unwrap();
        assert_eq!(view.remaining_ms, 2_500);
        assert!((view.fraction_remaining - 0.25).abs() < 1e-6);
        // A total smaller than the remainder is treated as the remainder.
        cds.set_remaining(SkillId(8), 3_000, 1_000, ClientTick(0));
        assert_eq!(cds.skill(SkillId(8), ClientTick(0)).unwrap().total_ms, 3_000);
    }

    #[test]
    fn timers_survive_tick_wraparound() {
        let mut cds = SkillCooldowns::default();
        cds.set(SkillId(3), 2_000, ClientTick(u32::MAX - 500));
        assert_eq!(cds.skill(SkillId(3), ClientTick(499)).unwrap().remaining_ms, 1_000);
        assert!(cds.skill(SkillId(3), ClientTick(1_500)).is_none());
    }

    #[test]
    fn after_cast_delay_dims_every_skill_and_keeps_the_longer_one() {
        let mut cds = SkillCooldowns::default();
        cds.start_after_cast(1_000, 1_000, ClientTick(100));
        assert!(matches!(cds.slot_delay(SkillId(5), ClientTick(600)), Some(SlotDelay::AfterCast(view)) if view.remaining_ms == 500));
        // A shorter delay does not cut it short.
        cds.start_after_cast(200, 200, ClientTick(600));
        assert_eq!(cds.after_cast(ClientTick(700)).unwrap().remaining_ms, 400);
        // A longer one replaces it.
        cds.start_after_cast(2_000, 2_000, ClientTick(700));
        assert_eq!(cds.after_cast(ClientTick(700)).unwrap().remaining_ms, 2_000);
        cds.end_after_cast();
        assert!(cds.slot_delay(SkillId(5), ClientTick(700)).is_none());
        cds.start_after_cast(0, 0, ClientTick(700));
        assert!(cds.after_cast(ClientTick(700)).is_none());
    }

    #[test]
    fn own_cooldown_wins_when_it_is_longer() {
        let mut cds = SkillCooldowns::default();
        cds.set(SkillId(9), 10_000, ClientTick(0));
        cds.start_after_cast(1_500, 1_500, ClientTick(0));
        assert!(matches!(cds.slot_delay(SkillId(9), ClientTick(100)), Some(SlotDelay::Cooldown(view)) if view.remaining_ms == 9_900));
        assert!(matches!(
            cds.slot_delay(SkillId(10), ClientTick(100)),
            Some(SlotDelay::AfterCast(_))
        ));
        cds.set(SkillId(11), 500, ClientTick(0));
        assert!(matches!(
            cds.slot_delay(SkillId(11), ClientTick(100)),
            Some(SlotDelay::AfterCast(_))
        ));
    }

    #[test]
    fn prune_and_describe() {
        let mut cds = SkillCooldowns::default();
        cds.set(SkillId(2), 1_000, ClientTick(0));
        cds.set(SkillId(1), 5_000, ClientTick(0));
        cds.start_after_cast(300, 300, ClientTick(0));
        assert_eq!(
            cds.describe(ClientTick(100)),
            "[skill 1 4900/5000 ms, skill 2 900/1000 ms, after_cast 200/300 ms]"
        );
        cds.prune(ClientTick(2_000));
        assert_eq!(cds.describe(ClientTick(2_000)), "[skill 1 3000/5000 ms]");
        cds.clear();
        assert_eq!(cds.describe(ClientTick(2_000)), "[]");
    }
}
