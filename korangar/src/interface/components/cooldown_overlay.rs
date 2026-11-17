//! Cooldown overlay shared by the hotbar, the skill tree and `SkillBox`
//! (Matthew t274u). This is the overlay that used to live only in the unused
//! `SkillBox`, moved here so every skill slot draws the same thing.
//!
//! A skill on its own cooldown (`0x043D` / `0x0985`) gets a dark vertical
//! sweep that drains toward the bottom plus the remaining seconds. During the
//! global after-cast delay (`EFST_POSTDELAY`) every skill gets a dimmer sweep
//! with no number, like roBrowser's `ShortCut.setGlobalSkillDelay`.
//! Colors come from the `hotbar` theme section.

use std::sync::LazyLock;

use korangar_interface::layout::WindowLayout;
use korangar_interface::layout::area::Area;
use ragnarok_packets::SkillId;
use rust_state::State;

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::interface::font_scale::from_theme_body;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::skill_cooldowns::SlotDelay;
use crate::state::theme::{HotbarThemePathExt, InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

/// Remaining-time font, as the historical pixel size.
pub const COOLDOWN_FONT_PX: f32 = 14.0;

static COOLDOWN_TEXT: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut texts: Vec<String> = (0..60).map(|seconds| seconds.to_string()).collect();
    texts.extend((1..60).map(|minutes| format!("{minutes}m")));
    texts.extend((1..=24).map(|hours| format!("{hours}h")));
    texts.push("24h+".to_owned());
    texts
});

/// Short remaining-time text: `7`, `3m`, `2h` (rounded up), `24h+`.
pub fn cooldown_text(seconds: u32) -> &'static str {
    let index = match seconds {
        0..60 => seconds as usize,
        60..3_600 => 59 + seconds.div_ceil(60).min(59) as usize,
        _ => 59 + 59 + seconds.div_ceil(3_600).min(25) as usize,
    };
    COOLDOWN_TEXT[index.min(COOLDOWN_TEXT.len() - 1)].as_str()
}

/// Part of `area` the sweep covers: the bottom `fraction_remaining` of it.
pub fn sweep_area(area: Area, fraction_remaining: f32) -> Area {
    let fraction = if fraction_remaining.is_finite() {
        fraction_remaining.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let height = area.height * fraction;
    Area {
        left: area.left,
        top: area.top + area.height - height,
        width: area.width,
        height,
    }
}

/// Draw the overlay for `delay` over `area`.
pub fn add_delay_overlay<'a>(layout: &mut WindowLayout<'a, ClientState>, state: &'a State<ClientState>, area: Area, delay: SlotDelay) {
    let (color, text) = match delay {
        SlotDelay::Cooldown(view) => (
            *state.get(&client_theme().hotbar().cooldown_color()),
            Some(cooldown_text(view.remaining_seconds())),
        ),
        SlotDelay::AfterCast(_) => (*state.get(&client_theme().hotbar().after_cast_color()), None),
    };
    let sweep = sweep_area(area, delay.view().fraction_remaining);
    if sweep.height > 0.0 {
        layout.add_rectangle(
            sweep,
            CornerDiameter::uniform(0.0),
            color,
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );
    }
    if let Some(text) = text {
        use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
        layout.add_text(
            area,
            text,
            FontSize(from_theme_body(
                *state.get(&client_theme().tokens().font_size()),
                COOLDOWN_FONT_PX,
            )),
            *state.get(&client_theme().hotbar().cooldown_text_color()),
            *state.get(&client_theme().hotbar().cooldown_text_color()),
            HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
    }
}

/// Overlay for `skill_id` at the current client tick, if it is waiting.
pub fn add_skill_cooldown<'a>(layout: &mut WindowLayout<'a, ClientState>, state: &'a State<ClientState>, area: Area, skill_id: SkillId) {
    let now = *state.get(&client_state().client_tick());
    if let Some(delay) = state.get(&client_state().skill_cooldowns()).slot_delay(skill_id, now) {
        add_delay_overlay(layout, state, area, delay);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_text_is_short() {
        assert_eq!(cooldown_text(0), "0");
        assert_eq!(cooldown_text(7), "7");
        assert_eq!(cooldown_text(59), "59");
        assert_eq!(cooldown_text(60), "1m");
        assert_eq!(cooldown_text(61), "2m");
        assert_eq!(cooldown_text(3_599), "59m");
        assert_eq!(cooldown_text(3_600), "1h");
        assert_eq!(cooldown_text(3_601), "2h");
        assert_eq!(cooldown_text(86_400), "24h");
        assert_eq!(cooldown_text(u32::MAX), "24h+");
    }

    #[test]
    fn sweep_drains_toward_the_bottom() {
        let area = Area {
            left: 10.0,
            top: 20.0,
            width: 40.0,
            height: 40.0,
        };
        let full = sweep_area(area, 1.0);
        assert_eq!((full.top, full.height), (20.0, 40.0));
        let quarter = sweep_area(area, 0.25);
        assert_eq!((quarter.top, quarter.height), (50.0, 10.0));
        assert_eq!(quarter.left, 10.0);
        assert_eq!(sweep_area(area, 0.0).height, 0.0);
        assert_eq!(sweep_area(area, f32::NAN).height, 0.0);
        assert_eq!(sweep_area(area, 3.0).height, 40.0);
    }
}
