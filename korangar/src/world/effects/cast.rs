//! Cast-bar timing. Rendering lives in the client; this type only tracks state.
//!
//! roBrowser shows the bar from `ZC_USESKILL_ACK` (`0x0B1A` on this profile)
//! when `delayTime > 0`, except skills with `hideCastBar`. Auto Counter has no
//! delay and still shows a one-second bar. `ZC_DISPEL` (`0x01B9`) clears it.
//! That is the packet Hercules sends for a damage interrupt
//! (`clif_skillcastcancel`). The client stop packet `CZ_USE_SKILL_END`
//! (`0x0B11`) clears the local bar as well.
//!
//! After the cast, the local player's bar switches to the after-cast delay
//! (Matthew t274u). Hercules has no packet for that delay; with
//! `display_status_timers` it sends status icon `EFST_POSTDELAY` (46) on
//! `0x0983` with the `skill->delay_fix` duration, the same signal roBrowser
//! uses for `ShortCut.setGlobalSkillDelay`. That bar drains instead of
//! filling and is labelled, see [`CastPhase`].

use super::table;
use crate::state::theme::StatusBarTheme;

/// Gap between the local player's status bars and the cast bar under them.
pub const CAST_BAR_GAP: f32 = 3.0;
/// Phase label font under the local player's cast bar.
pub const CAST_LABEL_FONT_PX: f32 = 11.0;

/// Offset from the player's feet (projected) to the top of the cast bar fill,
/// in screen pixels. The status block starts 5px under the feet (see
/// `Player::render_status`) and is HP + SP + AP with two gaps, framed by
/// `border_size`; the cast bar sits under it with its own frame.
pub fn player_cast_bar_offset(theme: &StatusBarTheme) -> f32 {
    let status = theme.health_height + theme.spell_point_height + theme.activity_point_height + theme.gap * 2.0;
    5.0 + status + theme.border_size.height * 2.0 + CAST_BAR_GAP
}

/// `SkillConst.KN_AUTOCOUNTER`. The server sends `delay_time = 0`.
const KN_AUTOCOUNTER: u16 = 61;
const AUTOCOUNTER_CAST_MS: u32 = 1000;

/// Which wait a cast bar shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastPhase {
    /// `ZC_USESKILL_ACK` cast time. The bar fills.
    Casting,
    /// `EFST_POSTDELAY` after-cast delay. The bar drains.
    AfterCast,
}

impl CastPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Casting => "Casting",
            Self::AfterCast => "After-cast",
        }
    }

    /// How full the bar is drawn for `progress` (0.0 at the start, 1.0 done).
    pub fn fill(self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Casting => progress,
            Self::AfterCast => 1.0 - progress,
        }
    }
}

/// One visible bar, see [`CastBars::iter_visible`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibleCastBar {
    pub entity_id: u32,
    pub progress: f32,
    pub phase: CastPhase,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CastBar {
    pub entity_id: u32,
    pub skill_id: u16,
    pub started_at_ms: u32,
    pub delay_ms: u32,
    pub interruptible: bool,
    pub phase: CastPhase,
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
            phase: CastPhase::Casting,
        });
    }

    /// `EFST_POSTDELAY` on the local player: show the after-cast delay. It
    /// replaces a finished (or nearly finished) cast bar. Zero shows nothing.
    pub fn begin_after_cast(&mut self, entity_id: u32, delay_ms: u32, now_ms: u32) {
        if delay_ms == 0 {
            return;
        }
        self.cancel(entity_id);
        self.bars.push(CastBar {
            entity_id,
            skill_id: 0,
            started_at_ms: now_ms,
            delay_ms,
            interruptible: false,
            phase: CastPhase::AfterCast,
        });
    }

    /// `EFST_POSTDELAY` ended early: drop the after-cast bar only.
    pub fn end_after_cast(&mut self, entity_id: u32) {
        self.bars
            .retain(|bar| bar.entity_id != entity_id || bar.phase != CastPhase::AfterCast);
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

    pub fn iter_visible(&self, now_ms: u32) -> impl Iterator<Item = VisibleCastBar> + '_ {
        self.bars.iter().filter_map(move |bar| {
            let progress = bar.progress(now_ms);
            (progress < 1.0).then_some(VisibleCastBar {
                entity_id: bar.entity_id,
                progress,
                phase: bar.phase,
            })
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
    fn player_cast_bar_sits_under_the_status_bars() {
        let theme = StatusBarTheme::default();
        let status_bottom =
            5.0 + theme.health_height + theme.spell_point_height + theme.activity_point_height + theme.gap * 2.0 + theme.border_size.height;
        let bar_top = player_cast_bar_offset(&theme);
        assert!(bar_top - theme.border_size.height > status_bottom, "frames must not overlap");
        assert!((bar_top - theme.border_size.height - status_bottom - CAST_BAR_GAP).abs() < f32::EPSILON);
    }

    #[test]
    fn after_cast_follows_the_cast_and_drains() {
        let mut bars = CastBars::default();
        bars.begin(7, 14, 1_000, 0);
        let casting = bars.iter_visible(500).next().unwrap();
        assert_eq!(casting.phase, CastPhase::Casting);
        assert!((casting.phase.fill(casting.progress) - 0.5).abs() < f32::EPSILON);
        assert_eq!(casting.phase.label(), "Casting");

        bars.begin_after_cast(7, 2_000, 1_000);
        let visible: Vec<_> = bars.iter_visible(1_500).collect();
        assert_eq!(visible.len(), 1, "the after-cast bar replaces the cast bar");
        assert_eq!(visible[0].phase, CastPhase::AfterCast);
        assert!((visible[0].phase.fill(visible[0].progress) - 0.75).abs() < f32::EPSILON);
        assert_eq!(visible[0].phase.label(), "After-cast");

        // Damage does not cut the after-cast delay short; the status end does.
        bars.interrupt_for_damage(7);
        assert!(bars.progress(7, 1_500).is_some());
        bars.end_after_cast(7);
        assert!(bars.progress(7, 1_500).is_none());

        bars.begin_after_cast(7, 0, 0);
        assert!(bars.iter_visible(0).next().is_none());

        // Ending the after-cast delay keeps a new cast.
        bars.begin(7, 14, 1_000, 3_000);
        bars.end_after_cast(7);
        assert!(bars.progress(7, 3_100).is_some());
        assert!(bars.iter_visible(3_000 + 1_000).next().is_none());
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
