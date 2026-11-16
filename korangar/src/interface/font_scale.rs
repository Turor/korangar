//! Global UI font scale (80–150%).
//!
//! The player setting is a percent. Chrome row heights are multiplied once
//! when a theme is loaded. Glyph measurement multiplies the same factor inside
//! the UI text layouter and strips it from the font stored on the instruction;
//! the interface renderer applies it again when drawing. World text does not
//! read this factor.

use std::cell::Cell;

use korangar_interface::element::StateElement;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

pub const FONT_SCALE_MIN: u16 = 80;
pub const FONT_SCALE_MAX: u16 = 150;
pub const FONT_SCALE_DEFAULT: u16 = 100;
const MAX_DIGITS: usize = 3;

thread_local! {
    static ACTIVE_FACTOR: Cell<f32> = const { Cell::new(1.0) };
}

/// Persisted font-scale percent plus the text-box draft.
///
/// The draft can be a partial number while the player is typing. The percent
/// stays on the last applied value until the draft is a finished number.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, RustState, StateElement)]
pub struct FontScaleSetting {
    #[serde(default = "default_percent")]
    pub percent: u16,
    #[serde(default = "default_draft")]
    pub draft: String,
}

fn default_percent() -> u16 {
    FONT_SCALE_DEFAULT
}

fn default_draft() -> String {
    FONT_SCALE_DEFAULT.to_string()
}

impl Default for FontScaleSetting {
    fn default() -> Self {
        Self {
            percent: FONT_SCALE_DEFAULT,
            draft: default_draft(),
        }
    }
}

impl FontScaleSetting {
    /// Snap a loaded file onto a canonical percent and matching draft.
    pub fn normalize(&mut self) {
        self.percent = clamp_font_scale_percent(self.percent);
        self.draft = self.percent.to_string();
    }
}

pub fn clamp_font_scale_percent(value: u16) -> u16 {
    value.clamp(FONT_SCALE_MIN, FONT_SCALE_MAX)
}

/// Multiplier for theme font sizes and chrome heights. Out-of-range percents
/// clamp to 0.8–1.5.
pub fn font_scale_factor(percent: u16) -> f32 {
    clamp_font_scale_percent(percent) as f32 / 100.0
}

pub fn slider_percent(t: f32) -> u16 {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    let span = (FONT_SCALE_MAX - FONT_SCALE_MIN) as f32;
    (FONT_SCALE_MIN as f32 + t * span).round() as u16
}

pub fn percent_to_slider_t(percent: u16) -> f32 {
    let percent = clamp_font_scale_percent(percent);
    (percent - FONT_SCALE_MIN) as f32 / (FONT_SCALE_MAX - FONT_SCALE_MIN) as f32
}

/// Slider position → percent and the canonical text-box string.
pub fn apply_slider_font_scale(t: f32) -> (u16, String) {
    let percent = slider_percent(t);
    (percent, percent.to_string())
}

/// A continuation of `digits` (0 to 3 total digits) can land inside 80–150.
fn can_reach_range(digits: &str) -> bool {
    let len = digits.len();
    if len == 0 || len > MAX_DIGITS {
        return false;
    }
    let Ok(base) = digits.parse::<u32>() else {
        return false;
    };
    let extra_max = MAX_DIGITS - len;
    for extra in 0..=extra_max {
        let factor = 10u32.pow(extra as u32);
        let start = base * factor;
        let end = start + factor - 1;
        if start <= FONT_SCALE_MAX as u32 && end >= FONT_SCALE_MIN as u32 {
            return true;
        }
    }
    false
}

/// Live typing. Partial drafts keep `current_percent`. Finished numbers apply.
/// Digits that cannot grow into range clamp (below → 80, above → 150).
pub fn apply_typed_font_scale(current_percent: u16, draft: &str) -> (u16, String) {
    let current = clamp_font_scale_percent(current_percent);
    if draft.is_empty() {
        return (current, current.to_string());
    }
    if draft.len() > MAX_DIGITS || !draft.chars().all(|c| c.is_ascii_digit()) {
        return (current, current.to_string());
    }
    let Ok(value) = draft.parse::<u16>() else {
        return (current, current.to_string());
    };
    if (FONT_SCALE_MIN..=FONT_SCALE_MAX).contains(&value) {
        return (value, value.to_string());
    }
    if can_reach_range(draft) {
        return (current, draft.to_string());
    }
    let clamped = if value > FONT_SCALE_MAX { FONT_SCALE_MAX } else { FONT_SCALE_MIN };
    (clamped, clamped.to_string())
}

/// Enter commits a partial draft by clamping, and restores junk or empty text.
pub fn commit_font_scale_draft(current_percent: u16, draft: &str) -> (u16, String) {
    let current = clamp_font_scale_percent(current_percent);
    let digits = draft.trim();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return (current, current.to_string());
    }
    let Ok(value) = digits.parse::<u32>() else {
        return (current, current.to_string());
    };
    let clamped = value.clamp(FONT_SCALE_MIN as u32, FONT_SCALE_MAX as u32) as u16;
    (clamped, clamped.to_string())
}

pub fn set_active_factor(factor: f32) {
    let factor = if factor.is_finite() {
        factor.clamp(font_scale_factor(FONT_SCALE_MIN), font_scale_factor(FONT_SCALE_MAX))
    } else {
        1.0
    };
    ACTIVE_FACTOR.with(|cell| cell.set(factor));
}

pub fn active_factor() -> f32 {
    ACTIVE_FACTOR.with(|cell| cell.get())
}

/// Multiply a font size by the active player scale.
pub fn scale_font_size(size: f32) -> f32 {
    if !size.is_finite() || size <= 0.0 {
        return size;
    }
    let scaled = size * active_factor();
    if scaled.is_finite() && scaled > 0.0 { scaled } else { size }
}

/// Map a font measured at `input * factor` back into the caller's font space.
///
/// Shrink still comes through as a ratio. The player factor does not, so a
/// later draw can apply it once.
pub fn unscale_laid_out_font(input: f32, factor: f32, scaled_output: f32) -> f32 {
    let scaled_input = input * factor;
    if !scaled_input.is_finite() || scaled_input.abs() <= f32::EPSILON {
        return input;
    }
    let ratio = scaled_output / scaled_input;
    if ratio.is_finite() && ratio >= 0.0 { input * ratio } else { input }
}

/// Historical pixel size as a multiple of the theme body font (13px at 100%).
/// Does not include the player scale; measurement applies that later.
pub fn from_theme_body(theme_font: f32, historical_px: f32) -> f32 {
    let theme_font = if theme_font.is_finite() && theme_font > 0.0 {
        theme_font
    } else {
        13.0
    };
    let historical_px = if historical_px.is_finite() && historical_px > 0.0 {
        historical_px
    } else {
        13.0
    };
    (theme_font * historical_px / 13.0).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factor_clamps_to_80_through_150() {
        assert!((font_scale_factor(80) - 0.8).abs() < f32::EPSILON);
        assert!((font_scale_factor(100) - 1.0).abs() < f32::EPSILON);
        assert!((font_scale_factor(150) - 1.5).abs() < f32::EPSILON);
        assert_eq!(font_scale_factor(0), font_scale_factor(80));
        assert_eq!(font_scale_factor(200), font_scale_factor(150));
        assert_eq!(clamp_font_scale_percent(79), 80);
        assert_eq!(clamp_font_scale_percent(151), 150);
    }

    #[test]
    fn slider_and_completed_text_stay_in_sync() {
        for percent in FONT_SCALE_MIN..=FONT_SCALE_MAX {
            let (got, draft) = apply_slider_font_scale(percent_to_slider_t(percent));
            assert_eq!(got, percent, "slider percent");
            assert_eq!(draft, percent.to_string());
            assert_eq!(draft.parse::<u16>().unwrap(), got);
        }
        assert_eq!(apply_slider_font_scale(0.0), (80, "80".to_string()));
        assert_eq!(apply_slider_font_scale(1.0), (150, "150".to_string()));
        assert_eq!(slider_percent(percent_to_slider_t(120)), 120);
    }

    #[test]
    fn partial_typed_values_do_not_move_the_percent() {
        for draft in ["0", "1", "8", "9", "12", "15"] {
            let (percent, text) = apply_typed_font_scale(100, draft);
            assert_eq!(percent, 100, "{draft}");
            assert_eq!(text, draft);
        }
    }

    #[test]
    fn finished_and_impossible_typed_values_apply_or_clamp() {
        assert_eq!(apply_typed_font_scale(100, "120"), (120, "120".to_string()));
        assert_eq!(apply_typed_font_scale(100, "80"), (80, "80".to_string()));
        assert_eq!(apply_typed_font_scale(100, "150"), (150, "150".to_string()));
        assert_eq!(apply_typed_font_scale(100, "080"), (80, "80".to_string()));
        assert_eq!(apply_typed_font_scale(100, "2"), (80, "80".to_string()));
        assert_eq!(apply_typed_font_scale(100, "7"), (80, "80".to_string()));
        assert_eq!(apply_typed_font_scale(100, "16"), (80, "80".to_string()));
        assert_eq!(apply_typed_font_scale(100, "50"), (80, "80".to_string()));
        assert_eq!(apply_typed_font_scale(100, "00"), (80, "80".to_string()));
        assert_eq!(apply_typed_font_scale(100, "200"), (150, "150".to_string()));
        assert_eq!(apply_typed_font_scale(100, "151"), (150, "150".to_string()));
        assert_eq!(apply_typed_font_scale(100, "999"), (150, "150".to_string()));
        assert_eq!(apply_typed_font_scale(120, ""), (120, "120".to_string()));
        assert_eq!(apply_typed_font_scale(110, "12a"), (110, "110".to_string()));
    }

    #[test]
    fn commit_clamps_partial_numbers_and_rejects_junk() {
        assert_eq!(commit_font_scale_draft(100, "12"), (80, "80".to_string()));
        assert_eq!(commit_font_scale_draft(100, "15"), (80, "80".to_string()));
        assert_eq!(commit_font_scale_draft(100, "200"), (150, "150".to_string()));
        assert_eq!(commit_font_scale_draft(100, "120"), (120, "120".to_string()));
        assert_eq!(commit_font_scale_draft(130, ""), (130, "130".to_string()));
        assert_eq!(commit_font_scale_draft(130, "nope"), (130, "130".to_string()));
    }

    #[test]
    fn active_factor_scales_measurement_and_restores() {
        set_active_factor(1.2);
        assert!((scale_font_size(13.0) - 15.6).abs() < 0.001);
        assert!((unscale_laid_out_font(13.0, 1.2, 15.6) - 13.0).abs() < 0.001);
        assert!((unscale_laid_out_font(13.0, 1.2, 7.8) - 6.5).abs() < 0.001);
        set_active_factor(4.0);
        assert!((active_factor() - 1.5).abs() < 0.001);
        set_active_factor(1.0);
        assert!((scale_font_size(13.0) - 13.0).abs() < 0.001);
    }

    #[test]
    fn theme_body_size_tracks_the_token_not_the_player_scale() {
        assert!((from_theme_body(13.0, 12.0) - 12.0).abs() < 0.001);
        assert!((from_theme_body(26.0, 12.0) - 24.0).abs() < 0.001);
        assert!((from_theme_body(13.0, 9.0) - 9.0).abs() < 0.001);
        set_active_factor(1.5);
        assert!((from_theme_body(13.0, 14.0) - 14.0).abs() < 0.001);
        set_active_factor(1.0);
    }
}
