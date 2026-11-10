//! Cast-bar timing. Rendering lives in the client; this type only tracks state.
//!
//! roBrowser shows the bar from `ZC_USESKILL_ACK` (`0x0B1A` on this profile) when `delayTime > 0`,
//! except skills with `hideCastBar`. Auto Counter has no delay and still shows a
//! one-second bar. `ZC_DISPEL` (`0x01B9`) clears it. That is the packet Hercules
//! sends for a damage interrupt (`clif_skillcastcancel`). The client stop packet
//! `CZ_USE_SKILL_END` (`0x0B11`) clears the local bar as well.

use super::table;

/// `SkillConst.KN_AUTOCOUNTER`. The server sends `delay_time = 0`.
const KN_AUTOCOUNTER: u16 = 61;
const AUTOCOUNTER_CAST_MS: u32 = 1000;

#[derive(Clone, Debug, PartialEq)]
pub struct CastBar {
    pub entity_id: u32,
    pub skill_id: u16,
    pub started_at_ms: u32,
    pub delay_ms: u32,
    pub interruptible: bool,
}

impl CastBar {
    pub fn progress(&self, now_ms: u32) -> f32 {
        if self.delay_ms == 0 {
            return 1.0;
        }
        let elapsed = now_ms.wrapping_sub(self.started_at_ms);
        if elapsed >= self.delay_ms {
            1.0
        } else {
            elapsed as f32 / self.delay_ms as f32
        }
    }
}

/// Delay to show, or nothing when the bar stays hidden.
pub fn visible_cast_delay(skill_id: u16, delay_ms: u32) -> Option<u32> {
    if table::skill_visual(skill_id).is_some_and(|visual| visual.hide_cast_bar) {
        return None;
    }
    if delay_ms > 0 {
        return Some(delay_ms);
    }
    if skill_id == KN_AUTOCOUNTER {
        return Some(AUTOCOUNTER_CAST_MS);
    }
    None
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CastBars {
    bars: Vec<CastBar>,
}

impl CastBars {
    pub fn begin(&mut self, entity_id: u32, skill_id: u16, delay_ms: u32, now_ms: u32) {
        self.begin_with(entity_id, skill_id, delay_ms, now_ms, true);
    }

    pub fn begin_with(&mut self, entity_id: u32, skill_id: u16, delay_ms: u32, now_ms: u32, interruptible: bool) {
        self.cancel(entity_id);
        let Some(delay_ms) = visible_cast_delay(skill_id, delay_ms) else {
            return;
        };
        self.bars.push(CastBar {
            entity_id,
            skill_id,
            started_at_ms: now_ms,
            delay_ms,
            interruptible,
        });
    }

    /// `ZC_DISPEL` / local `0x0B11`.
    pub fn cancel(&mut self, entity_id: u32) {
        self.bars.retain(|bar| bar.entity_id != entity_id);
    }

    /// Drop bars whose cast time has elapsed.
    pub fn expire(&mut self, now_ms: u32) {
        self.bars.retain(|bar| bar.progress(now_ms) < 1.0);
    }

    /// A hit on the caster. The client does not call this: Hercules sends
    /// `0x01B9` only when `skillcastcancel` is set. Kept so tests can show an
    /// uninterruptible bar surviving a hit.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn interrupt_for_damage(&mut self, entity_id: u32) {
        self.bars.retain(|bar| bar.entity_id != entity_id || !bar.interruptible);
    }

    #[cfg(test)]
    pub fn progress(&self, entity_id: u32, now_ms: u32) -> Option<f32> {
        let bar = self.bars.iter().find(|bar| bar.entity_id == entity_id)?;
        let progress = bar.progress(now_ms);
        (progress < 1.0).then_some(progress)
    }

    pub fn iter_visible(&self, now_ms: u32) -> impl Iterator<Item = (u32, f32)> + '_ {
        self.bars.iter().filter_map(move |bar| {
            let progress = bar.progress(now_ms);
            (progress < 1.0).then_some((bar.entity_id, progress))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cast_bar_timing_advances_and_finishes() {
        let mut bars = CastBars::default();
        bars.begin(9, 5, 1_000, 500);
        assert_eq!(bars.progress(9, 500), Some(0.0));
        let halfway = bars.progress(9, 1_000).unwrap();
        assert!((halfway - 0.5).abs() < f32::EPSILON);
        assert!(bars.progress(9, 1_500).is_none());
        assert_eq!(bars.bars.len(), 1);
        bars.expire(1_500);
        assert!(bars.bars.is_empty());
        assert!(bars.iter_visible(1_500).next().is_none());
    }

    #[test]
    fn zero_delay_is_hidden_except_auto_counter() {
        let mut bars = CastBars::default();
        bars.begin(1, 5, 0, 0);
        assert!(bars.progress(1, 0).is_none());

        bars.begin(2, KN_AUTOCOUNTER, 0, 10);
        assert_eq!(bars.progress(2, 10), Some(0.0));
        assert!(bars.progress(2, 10 + AUTOCOUNTER_CAST_MS).is_none());
    }

    #[test]
    fn hidden_cast_bar_skills_do_not_show() {
        let mut bars = CastBars::default();
        bars.begin(3, 57, 800, 0);
        assert!(bars.progress(3, 100).is_none());
    }

    #[test]
    fn dispel_and_damage_clear_the_bar() {
        let mut bars = CastBars::default();
        bars.begin(4, 14, 2_000, 0);
        bars.cancel(4);
        assert!(bars.progress(4, 100).is_none());

        bars.begin(4, 14, 2_000, 0);
        bars.interrupt_for_damage(4);
        assert!(bars.progress(4, 100).is_none());

        bars.begin_with(4, 14, 2_000, 0, false);
        bars.interrupt_for_damage(4);
        assert!(bars.progress(4, 100).is_some());
        bars.cancel(4);
        assert!(bars.progress(4, 100).is_none());
    }
}
