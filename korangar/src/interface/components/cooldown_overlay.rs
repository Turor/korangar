//! Cooldown overlay shared by the hotbar, the skill tree and `SkillBox`
//! (Matthew t274u). This is the overlay that used to live only in the unused
//! `SkillBox`, moved here so every skill slot draws the same thing.
//!
//! A skill on its own cooldown (`0x043D` / `0x0985`) gets a dark vertical
//! sweep that drains toward the bottom plus the remaining seconds. During the
//! global after-cast delay (`EFST_POSTDELAY`) every skill gets a dimmer sweep
//! with no number, like roBrowser's `ShortCut.setGlobalSkillDelay`.
//! Colors come from the `hotbar` theme section.
//!
//! Icons are sprites or textures (custom instructions), and inside one layer
//! every rectangle renders before them, so the sweep goes on its own layer
//! above the icon (review-1 B1). The digits get a one-pixel outline so they
//! stay readable over bright icons.

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

/// Offsets of the digit outline copies, drawn under the digits.
pub const OUTLINE_OFFSETS: [(f32, f32); 4] = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];

/// Colors and font for one overlay.
#[derive(Clone, Copy, Debug)]
pub struct OverlayStyle {
    pub sweep_color: Color,
    pub text_color: Color,
    pub outline_color: Color,
    pub font_size: FontSize,
}

/// Queue the sweep (and `text`, outlined) on a layer above everything the
/// caller queued so far for this slot, so it covers the icon.
pub fn push_overlay(
    layout: &mut WindowLayout<'_, ClientState>,
    area: Area,
    fraction_remaining: f32,
    text: Option<&'static str>,
    style: OverlayStyle,
) {
    use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};

    let sweep = sweep_area(area, fraction_remaining);
    layout.with_layer(|layout| {
        if sweep.height > 0.0 {
            layout.add_rectangle(
                sweep,
                CornerDiameter::uniform(0.0),
                style.sweep_color,
                Color::TRANSPARENT,
                ShadowPadding::uniform(0.0),
            );
        }
        let Some(text) = text else {
            return;
        };
        let copies = OUTLINE_OFFSETS
            .iter()
            .map(|&(dx, dy)| (dx, dy, style.outline_color))
            .chain(std::iter::once((0.0, 0.0, style.text_color)));
        for (dx, dy, color) in copies {
            layout.add_text(
                Area {
                    left: area.left + dx,
                    top: area.top + dy,
                    ..area
                },
                text,
                style.font_size,
                color,
                color,
                HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        }
    });
}

/// Draw the overlay for `delay` over `area`. Call it after the icon.
pub fn add_delay_overlay<'a>(layout: &mut WindowLayout<'a, ClientState>, state: &'a State<ClientState>, area: Area, delay: SlotDelay) {
    let (sweep_color, text) = match delay {
        SlotDelay::Cooldown(view) => (
            *state.get(&client_theme().hotbar().cooldown_color()),
            Some(cooldown_text(view.remaining_seconds())),
        ),
        SlotDelay::AfterCast(_) => (*state.get(&client_theme().hotbar().after_cast_color()), None),
    };
    let style = OverlayStyle {
        sweep_color,
        text_color: *state.get(&client_theme().hotbar().cooldown_text_color()),
        outline_color: *state.get(&client_theme().hotbar().cooldown_text_outline_color()),
        font_size: FontSize(from_theme_body(
            *state.get(&client_theme().tokens().font_size()),
            COOLDOWN_FONT_PX,
        )),
    };
    push_overlay(layout, area, delay.view().fraction_remaining, text, style);
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

    fn style() -> OverlayStyle {
        OverlayStyle {
            sweep_color: Color::BLACK.with_alpha_u8(185),
            text_color: Color::WHITE,
            outline_color: Color::BLACK,
            font_size: FontSize(14.0),
        }
    }

    const SLOT: Area = Area {
        left: 0.0,
        top: 0.0,
        width: 40.0,
        height: 40.0,
    };

    /// Review-1 B1: the sweep used to be a rectangle on the icon's layer, and
    /// rectangles render before sprites and textures, so solid icons hid it.
    /// The overlay must land on a layer above the one holding the icon.
    #[test]
    fn overlay_is_queued_above_the_icon() {
        let mut layout = korangar_interface::layout::WindowLayout::<ClientState>::default();
        // Stand-in for the slot frame and the icon, queued on the slot's layer
        // exactly where hotbar_grid and the skill tree queue them.
        layout.add_rectangle(
            SLOT,
            CornerDiameter::uniform(0.0),
            Color::WHITE,
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );
        let icon_layer = layout.current_layer_index();

        push_overlay(&mut layout, SLOT, 0.5, Some(cooldown_text(7)), style());
        assert_eq!(layout.current_layer_index(), icon_layer, "the overlay layer is popped again");

        let layers = layout.queued_instructions();
        let overlay_layer = layers
            .iter()
            .position(|layer| layer.texts > 0)
            .expect("the overlay queued its digits");
        assert!(
            overlay_layer > icon_layer,
            "overlay on layer {overlay_layer}, icon on {icon_layer}: {layers:?}"
        );
        assert_eq!(
            layers[icon_layer].rectangles, 1,
            "nothing of the overlay is on the icon layer: {layers:?}"
        );
        assert_eq!(layers[overlay_layer].rectangles, 1, "{layers:?}");
        assert_eq!(
            layers[overlay_layer].texts,
            OUTLINE_OFFSETS.len() + 1,
            "outline copies plus the digits"
        );
    }

    #[test]
    fn after_cast_overlay_has_no_digits_and_an_empty_sweep_draws_nothing() {
        let mut layout = korangar_interface::layout::WindowLayout::<ClientState>::default();
        push_overlay(&mut layout, SLOT, 0.25, None, style());
        let layers = layout.queued_instructions();
        assert_eq!(layers[0], Default::default());
        assert_eq!((layers[1].rectangles, layers[1].texts), (1, 0));

        let mut layout = korangar_interface::layout::WindowLayout::<ClientState>::default();
        push_overlay(&mut layout, SLOT, 0.0, None, style());
        assert!(
            layout
                .queued_instructions()
                .iter()
                .all(|layer| layer.rectangles == 0 && layer.texts == 0)
        );
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
