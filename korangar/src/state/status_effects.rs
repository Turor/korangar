use std::sync::Arc;

use korangar_interface::element::StateElement;
use ragnarok_packets::ClientTick;
use rust_state::RustState;

use crate::graphics::Texture;
use crate::state::status_names::{status_effect_description, status_effect_name};
use crate::world::StatusIconInfo;

/// Seconds left after which the countdown sweep turns to the urgent color
/// (roBrowser `StatusIcons.renderStatus` uses the last minute).
pub const URGENT_REMAINING_MILLISECONDS: u32 = 60_000;

/// Hercules sends `Left = 9999` (and `Total = -1`) for a status without a
/// time limit. roBrowser `StatusIcons.update` treats 9999 as infinite too.
pub const UNLIMITED_REMAINING_MILLISECONDS: u32 = 9999;

/// `(remaining, total)` with Hercules' "no time limit" encodings mapped to
/// `(0, 0)`.
pub fn normalize_duration(remaining_milliseconds: u32, total_duration_milliseconds: u32) -> (u32, u32) {
    if remaining_milliseconds == UNLIMITED_REMAINING_MILLISECONDS || total_duration_milliseconds == u32::MAX {
        (0, 0)
    } else {
        (remaining_milliseconds, total_duration_milliseconds)
    }
}

/// One timed (or flag) status currently on the local player.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct ActiveStatusEffect {
    pub status_index: u16,
    pub remaining_milliseconds: u32,
    pub total_duration_milliseconds: u32,
    /// Client tick when the status was last refreshed.
    pub updated_at_tick: u32,
    /// Preformatted chip label (`Blessing 5s`) as of the last refresh.
    pub label: String,
    /// Icon art from `data\texture\effect\` (`stateiconimginfo.lub`).
    #[hidden_element]
    pub icon: Option<Arc<Texture>>,
    /// Hover text: title and description lines from `stateiconinfo.lub`, or
    /// the built-in name table.
    pub tooltip: String,
    /// Set once the icon lookup ran, so a status without art is not probed on
    /// every refresh.
    pub presentation_resolved: bool,
}

/// Local-player status effects driven by `NetworkEvent::StatusChange*`.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct StatusEffects {
    effects: Vec<ActiveStatusEffect>,
}

fn format_label(status_index: u16, remaining_milliseconds: u32) -> String {
    let name = status_effect_name(status_index);
    if remaining_milliseconds == 0 {
        name
    } else {
        let seconds = remaining_milliseconds.div_ceil(1000).max(1);
        format!("{name} {seconds}s")
    }
}

/// Name for the status from the client tables, falling back to the built-in
/// list (`SC#N` when unknown).
pub fn status_display_name(status_index: u16, info: Option<&StatusIconInfo>) -> String {
    info.and_then(|info| info.title.clone())
        .unwrap_or_else(|| status_effect_name(status_index))
}

/// Tooltip for an icon: the name, then the description lines.
pub fn status_tooltip(status_index: u16, info: Option<&StatusIconInfo>) -> String {
    let name = status_display_name(status_index, info);
    let description = match info {
        Some(info) if !info.description.is_empty() => info.description.join("\n"),
        _ => status_effect_description(status_index),
    };
    if description.is_empty() || description == name {
        name
    } else {
        format!("{name}\n{description}")
    }
}

/// Path for [`crate::loaders::TextureLoader`], which prefixes
/// `data\texture\`.
pub fn status_icon_texture_path(icon_name: &str) -> String {
    format!("effect\\{}", icon_name.trim())
}

/// Milliseconds left on `effect` at `now`. `0` means no time limit.
pub fn remaining_ms(effect: &ActiveStatusEffect, now: ClientTick) -> u32 {
    if effect.remaining_milliseconds == 0 {
        return 0;
    }
    let elapsed = now.0.saturating_sub(effect.updated_at_tick);
    effect.remaining_milliseconds.saturating_sub(elapsed).max(1)
}

/// Countdown sweep drawn over an icon, like roBrowser's pie overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CountdownSweep {
    /// Share of the icon covered, `0.0..=1.0`.
    pub fraction: f32,
    /// The last minute: drawn in the urgent color.
    pub urgent: bool,
}

/// `None` for a status without a time limit. In the last minute the sweep
/// fills over that minute. Before that it fills over the time until the last
/// minute starts.
pub fn countdown_sweep(remaining_milliseconds: u32, total_duration_milliseconds: u32) -> Option<CountdownSweep> {
    if remaining_milliseconds == 0 {
        return None;
    }
    let total = total_duration_milliseconds.max(remaining_milliseconds);
    if remaining_milliseconds < URGENT_REMAINING_MILLISECONDS {
        let fraction = 1.0 - remaining_milliseconds as f32 / URGENT_REMAINING_MILLISECONDS as f32;
        return Some(CountdownSweep {
            fraction: fraction.clamp(0.0, 1.0),
            urgent: true,
        });
    }
    let span = total.saturating_sub(URGENT_REMAINING_MILLISECONDS);
    let fraction = match span {
        0 => 0.0,
        span => (total - remaining_milliseconds) as f32 / span as f32,
    };
    Some(CountdownSweep {
        fraction: fraction.clamp(0.0, 1.0),
        urgent: false,
    })
}

/// `5s`, `3m`, `2h`, or empty for no limit.
pub fn format_remaining(milliseconds: u32) -> String {
    if milliseconds == 0 {
        return String::new();
    }
    let seconds = milliseconds.div_ceil(1000).max(1);
    if seconds >= 3600 {
        format!("{}h", seconds / 3600)
    } else if seconds >= 60 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

impl StatusEffects {
    pub fn apply(&mut self, status_index: u16, remaining_milliseconds: u32, total_duration_milliseconds: u32, updated_at: ClientTick) {
        let (remaining_milliseconds, total_duration_milliseconds) = normalize_duration(remaining_milliseconds, total_duration_milliseconds);
        let label = format_label(status_index, remaining_milliseconds);
        if let Some(existing) = self.effects.iter_mut().find(|effect| effect.status_index == status_index) {
            existing.remaining_milliseconds = remaining_milliseconds;
            existing.total_duration_milliseconds = total_duration_milliseconds;
            existing.updated_at_tick = updated_at.0;
            existing.label = label;
            return;
        }
        self.effects.push(ActiveStatusEffect {
            status_index,
            remaining_milliseconds,
            total_duration_milliseconds,
            updated_at_tick: updated_at.0,
            label,
            icon: None,
            tooltip: status_tooltip(status_index, None),
            presentation_resolved: false,
        });
        self.effects.sort_by_key(|effect| effect.status_index);
    }

    /// True while `status_index` is active and its icon lookup has not run.
    pub fn needs_presentation(&self, status_index: u16) -> bool {
        self.effects
            .iter()
            .any(|effect| effect.status_index == status_index && !effect.presentation_resolved)
    }

    /// Store the icon and tooltip resolved from the client tables.
    pub fn set_presentation(&mut self, status_index: u16, icon: Option<Arc<Texture>>, tooltip: String) {
        if let Some(effect) = self.effects.iter_mut().find(|effect| effect.status_index == status_index) {
            effect.icon = icon;
            effect.tooltip = tooltip;
            effect.presentation_resolved = true;
        }
    }

    pub fn end(&mut self, status_index: u16) {
        self.effects.retain(|effect| effect.status_index != status_index);
    }

    /// Drop timed statuses whose countdown ran out without an end packet.
    pub fn expire(&mut self, now: ClientTick) {
        self.effects.retain(|effect| {
            effect.remaining_milliseconds == 0 || now.0.saturating_sub(effect.updated_at_tick) < effect.remaining_milliseconds
        });
    }

    pub fn clear(&mut self) {
        self.effects.clear();
    }

    pub fn effects(&self) -> &[ActiveStatusEffect] {
        &self.effects
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::ClientTick;

    use super::{StatusEffects, countdown_sweep, format_remaining, remaining_ms, status_icon_texture_path, status_tooltip};
    use crate::world::StatusIconInfo;

    #[test]
    fn apply_upserts_and_end_removes() {
        let mut bar = StatusEffects::default();
        bar.apply(10, 5000, 5000, ClientTick(100));
        bar.apply(5, 1000, 1000, ClientTick(100));
        assert_eq!(bar.effects().len(), 2);
        assert_eq!(bar.effects()[0].status_index, 5);
        assert!(bar.effects()[0].label.contains("Cloaking"));
        assert!(
            bar.effects()
                .iter()
                .find(|e| e.status_index == 10)
                .unwrap()
                .label
                .contains("Blessing")
        );
        bar.apply(10, 2000, 5000, ClientTick(150));
        assert_eq!(bar.effects().len(), 2);
        assert_eq!(
            bar.effects().iter().find(|e| e.status_index == 10).unwrap().remaining_milliseconds,
            2000
        );
        bar.end(5);
        assert_eq!(bar.effects().len(), 1);
        assert_eq!(bar.effects()[0].status_index, 10);
    }

    #[test]
    fn hercules_infinite_duration_is_a_flag_status() {
        let mut bar = StatusEffects::default();
        // `tick < 0` becomes Left 9999; Total stays -1.
        bar.apply(35, 9999, u32::MAX, ClientTick(0));
        bar.apply(40, 30_000, u32::MAX, ClientTick(0));
        assert!(bar.effects().iter().all(|effect| effect.remaining_milliseconds == 0));
        assert!(bar.effects().iter().all(|effect| effect.total_duration_milliseconds == 0));
        bar.expire(ClientTick(10_000_000));
        assert_eq!(bar.effects().len(), 2);
    }

    #[test]
    fn presentation_resolves_once_and_survives_refresh() {
        let mut bar = StatusEffects::default();
        assert!(!bar.needs_presentation(10));
        bar.apply(10, 5000, 5000, ClientTick(0));
        assert!(bar.needs_presentation(10));
        bar.set_presentation(10, None, "Blessing\nMore DEX".into());
        assert!(!bar.needs_presentation(10));
        bar.apply(10, 9000, 9000, ClientTick(10));
        assert!(!bar.needs_presentation(10));
        assert_eq!(bar.effects()[0].tooltip, "Blessing\nMore DEX");
    }

    #[test]
    fn expire_drops_finished_timers_but_keeps_flags() {
        let mut bar = StatusEffects::default();
        bar.apply(10, 1000, 1000, ClientTick(0));
        bar.apply(27, 0, 0, ClientTick(0));
        bar.expire(ClientTick(999));
        assert_eq!(bar.effects().len(), 2);
        bar.expire(ClientTick(1000));
        assert_eq!(bar.effects().len(), 1);
        assert_eq!(bar.effects()[0].status_index, 27);
    }

    #[test]
    fn remaining_time_counts_down_from_last_update() {
        let mut bar = StatusEffects::default();
        bar.apply(10, 5000, 5000, ClientTick(1000));
        let effect = &bar.effects()[0];
        assert_eq!(remaining_ms(effect, ClientTick(1000)), 5000);
        assert_eq!(remaining_ms(effect, ClientTick(3000)), 3000);
        // Still listed until the end packet or `expire`, so never reads as
        // "no limit".
        assert_eq!(remaining_ms(effect, ClientTick(8000)), 1);
        assert_eq!(format_remaining(1500), "2s");
        assert_eq!(format_remaining(60_000), "1m");
        assert_eq!(format_remaining(7_200_000), "2h");
        assert_eq!(format_remaining(0), "");
    }

    #[test]
    fn sweep_fills_toward_the_last_minute_then_turns_urgent() {
        assert_eq!(countdown_sweep(0, 0), None);
        let fresh = countdown_sweep(240_000, 240_000).unwrap();
        assert!(!fresh.urgent && fresh.fraction.abs() < 0.001);
        let half = countdown_sweep(150_000, 240_000).unwrap();
        assert!(!half.urgent && (half.fraction - 0.5).abs() < 0.001);
        let urgent = countdown_sweep(15_000, 240_000).unwrap();
        assert!(urgent.urgent && (urgent.fraction - 0.75).abs() < 0.001);
        // Short buffs are urgent from the start.
        let short = countdown_sweep(30_000, 30_000).unwrap();
        assert!(short.urgent && (short.fraction - 0.5).abs() < 0.001);
    }

    #[test]
    fn tooltip_prefers_client_text_and_falls_back_to_the_name_table() {
        let info = StatusIconInfo {
            icon: Some("BLESSING.TGA".into()),
            title: Some("Blessing".into()),
            description: vec!["Increase DEX, INT, STR".into(), "Increases HIT".into()],
            has_time_limit: true,
        };
        assert_eq!(
            status_tooltip(10, Some(&info)),
            "Blessing\nIncrease DEX, INT, STR\nIncreases HIT"
        );
        assert!(status_tooltip(10, None).starts_with("Blessing"));
        assert!(status_tooltip(60_000, None).starts_with("SC#"));
        assert_eq!(status_icon_texture_path(" BLESSING.TGA "), "effect\\BLESSING.TGA");
    }
}
