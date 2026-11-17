//! Hotbar layout math (Matthew t274u, t280u).
//!
//! The bar is a stack of rows of nine slots and never wraps. Row 0 is the
//! F-key row at the bottom; `+` adds the number row above it, then the Q row,
//! then the A row. Every slot has a hotkey strip under it.
//!
//! Sizing: the slot side is the smaller of what the width and the height
//! allow (each with the minimum padding), clamped to
//! [`HOTBAR_SLOT_MINIMUM`]..[`HOTBAR_SLOT_MAXIMUM`]. Width left over becomes
//! even padding between slots and height left over becomes even padding
//! between rows, so widening alone spreads the slots out, heightening alone
//! spreads the rows out, and the slots only grow when both allow it. Adding a
//! row runs the same rule, so the slots shrink to make room.
//!
//! The grid never asks for more height than it is given unless even the
//! minimum slots do not fit, so the window height does not feed back into
//! itself (the "keeps growing taller" bug). Everything here is pure so the
//! unit tests and the e2e harness check it without a renderer.

use crate::state::hotbar::{HOTBAR_COLUMNS, HOTBAR_ROWS};

/// Smallest drawn slot at UI scale 1, same floor as the item grids.
pub const HOTBAR_SLOT_MINIMUM: f32 = 24.0;
/// Largest drawn slot at UI scale 1.
pub const HOTBAR_SLOT_MAXIMUM: f32 = 64.0;
/// Minimum padding between neighbouring slots and rows.
pub const HOTBAR_SLOT_GAP: f32 = 2.0;
/// Hotkey font in the strip under each slot.
pub const HOTBAR_HINT_FONT_PX: f32 = 9.0;
/// Width of the `+` / `-` column at the right end of the F-key row.
pub const HOTBAR_ROW_CONTROLS_WIDTH: f32 = 14.0;
/// Height of the bare strip at the top of the window that moves it (the
/// window has no title text).
pub const HOTBAR_GRAB_HEIGHT: f32 = 8.0;
/// Window border on the sides and the bottom.
pub const HOTBAR_WINDOW_BORDER: f32 = 3.0;
/// Hotkey strip height used for the window limits (font 9 at scale 1).
const LIMIT_HINT_HEIGHT: f32 = 11.0;
/// Narrowest hotbar window: nine minimum slots, minimum padding and the
/// row controls.
pub const HOTBAR_WINDOW_MINIMUM_WIDTH: f32 = HOTBAR_WINDOW_BORDER * 2.0
    + HOTBAR_COLUMNS as f32 * HOTBAR_SLOT_MINIMUM
    + (HOTBAR_COLUMNS as f32 - 1.0) * HOTBAR_SLOT_GAP
    + HOTBAR_SLOT_GAP
    + HOTBAR_ROW_CONTROLS_WIDTH;
/// Widest hotbar window.
pub const HOTBAR_WINDOW_MAXIMUM_WIDTH: f32 = 1_200.0;
/// Shortest hotbar window: one row of minimum slots and its hotkey strip.
pub const HOTBAR_WINDOW_MINIMUM_HEIGHT: f32 = HOTBAR_GRAB_HEIGHT + HOTBAR_WINDOW_BORDER + HOTBAR_SLOT_MINIMUM + LIMIT_HINT_HEIGHT;
/// Tallest hotbar window.
pub const HOTBAR_WINDOW_MAXIMUM_HEIGHT: f32 = 720.0;
/// First-open hotbar height: one row of 40px slots and the hotkey strip.
pub const HOTBAR_DEFAULT_HEIGHT: f32 = HOTBAR_GRAB_HEIGHT + HOTBAR_WINDOW_BORDER + 40.0 + LIMIT_HINT_HEIGHT;

/// Hotkey strip height for a hint font of `font_px` (already scaled).
pub fn hint_strip_height(font_px: f32) -> f32 {
    finite_or_zero(font_px).ceil() + 1.0
}

/// Slot geometry for one hotbar region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HotbarMetrics {
    /// Always [`HOTBAR_COLUMNS`]: rows never wrap.
    pub columns: usize,
    /// Rows shown, 1 to [`HOTBAR_ROWS`].
    pub rows: usize,
    /// Side of the square slot.
    pub slot: f32,
    /// Height of the hotkey strip under each slot.
    pub hint_height: f32,
    /// Padding between neighbouring slots in a row.
    pub gap_x: f32,
    /// Padding between rows (0 with one row).
    pub gap_y: f32,
    /// Space above the first row (one row is centred vertically).
    pub top: f32,
    /// Height the grid takes: the region height, or more when even the
    /// minimum slots do not fit.
    pub height: f32,
}

impl HotbarMetrics {
    /// Height of one row: the slot plus its hotkey strip.
    pub fn cell_height(&self) -> f32 {
        self.slot + self.hint_height
    }

    /// Width of the slots and the padding between them (no controls).
    pub fn grid_width(&self) -> f32 {
        self.columns as f32 * self.slot + self.columns.saturating_sub(1) as f32 * self.gap_x
    }

    /// Width of the grid plus the `+` / `-` column.
    pub fn total_width(&self) -> f32 {
        self.grid_width() + HOTBAR_SLOT_GAP + HOTBAR_ROW_CONTROLS_WIDTH
    }

    /// Top-left of the slot in `column` of visual `row` (row 0 = F-keys,
    /// drawn at the bottom), relative to the region.
    pub fn slot_origin(&self, column: usize, row: usize) -> (f32, f32) {
        let row_from_top = self.rows.saturating_sub(1).saturating_sub(row.min(self.rows.saturating_sub(1)));
        (
            column as f32 * (self.slot + self.gap_x),
            self.top + row_from_top as f32 * (self.cell_height() + self.gap_y),
        )
    }

    /// Left edge of the `+` / `-` column, relative to the region.
    pub fn controls_left(&self) -> f32 {
        self.grid_width() + HOTBAR_SLOT_GAP
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value.max(0.0) } else { 0.0 }
}

/// Lay out `rows` rows of nine slots in a `width` x `height` region (height
/// `None` while the window has no height yet).
pub fn hotbar_metrics(rows: usize, width: f32, height: Option<f32>, hint_height: f32) -> HotbarMetrics {
    let rows = rows.clamp(1, HOTBAR_ROWS);
    let columns = HOTBAR_COLUMNS;
    let width = finite_or_zero(width);
    let hint_height = finite_or_zero(hint_height);
    let height = height.map(finite_or_zero);

    let usable_width = (width - HOTBAR_SLOT_GAP - HOTBAR_ROW_CONTROLS_WIDTH).max(0.0);
    let fit_width = (usable_width - HOTBAR_SLOT_GAP * (columns - 1) as f32) / columns as f32;
    let fit_height = match height {
        Some(height) => (height - HOTBAR_SLOT_GAP * (rows - 1) as f32) / rows as f32 - hint_height,
        None => f32::INFINITY,
    };
    let slot = fit_width.min(fit_height).min(HOTBAR_SLOT_MAXIMUM).floor().max(HOTBAR_SLOT_MINIMUM);

    let gap_x = ((usable_width - columns as f32 * slot) / (columns - 1) as f32).max(HOTBAR_SLOT_GAP);
    let cell_height = slot + hint_height;
    let needed = rows as f32 * cell_height + (rows - 1) as f32 * HOTBAR_SLOT_GAP;
    let region_height = height.map_or(needed, |height| height.max(needed));
    let (gap_y, top) = match rows {
        1 => (0.0, ((region_height - cell_height) / 2.0).max(0.0).floor()),
        _ => (
            ((region_height - rows as f32 * cell_height) / (rows - 1) as f32).max(HOTBAR_SLOT_GAP),
            0.0,
        ),
    };
    HotbarMetrics {
        columns,
        rows,
        slot,
        hint_height,
        gap_x,
        gap_y,
        top,
        height: region_height,
    }
}

/// Visual slot of `column` in visual `row` (row 0 = F-keys).
pub fn visual_slot(row: usize, column: usize) -> u16 {
    (row * HOTBAR_COLUMNS + column) as u16
}

/// Whether a skill slot shows its level: only skills with more than one
/// level (Matthew t280u).
pub fn shows_skill_level(maximum_level: u16) -> bool {
    maximum_level > 1
}

/// Compact hotkey hint for a slot: `F1`, `A-F1`, `C-F1`, `S-F1`, `Q`.
pub fn compact_hint(key: &str, alt: bool, ctrl: bool, shift: bool) -> String {
    let mut hint = String::new();
    for (held, prefix) in [(alt, "A-"), (ctrl, "C-"), (shift, "S-")] {
        if held {
            hint.push_str(prefix);
        }
    }
    hint.push_str(key);
    hint
}

/// One window layout pass, as `korangar-interface` runs it for the hotbar:
/// the grab strip and the bottom border around the grid, which gets the rest
/// of the window height. Returns the height the window reports back.
pub fn laid_out_window_height(window_height: f32, rows: usize, width: f32, hint_height: f32) -> f32 {
    let chrome = HOTBAR_GRAB_HEIGHT + HOTBAR_WINDOW_BORDER;
    let region_width = width - HOTBAR_WINDOW_BORDER * 2.0;
    let metrics = hotbar_metrics(rows, region_width, Some(window_height - chrome), hint_height);
    (chrome + metrics.height).clamp(HOTBAR_WINDOW_MINIMUM_HEIGHT, HOTBAR_WINDOW_MAXIMUM_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HINT: f32 = 11.0;

    fn fits(metrics: &HotbarMetrics, width: f32) {
        assert_eq!(metrics.columns, HOTBAR_COLUMNS, "never wraps: {metrics:?}");
        assert!(metrics.total_width() <= width + 0.01, "{metrics:?} in {width}");
    }

    #[test]
    fn repeated_layout_passes_keep_the_window_height() {
        // Regression (t280u): the window took the grid height back as its
        // size each frame and grew without bound.
        for rows in 1..=HOTBAR_ROWS {
            for (width, height) in [(420.0, 66.0), (600.0, 200.0), (300.0, 300.0), (1_000.0, 90.0)] {
                let mut window_height = height;
                let first = laid_out_window_height(window_height, rows, width, HINT);
                for _ in 0..200 {
                    window_height = laid_out_window_height(window_height, rows, width, HINT);
                }
                assert_eq!(window_height, first, "rows {rows} at {width}x{height}");
                assert!(
                    first <= height.max(HOTBAR_WINDOW_MINIMUM_HEIGHT) || rows > 1,
                    "{rows} {width}x{height}: {first}"
                );
            }
        }
    }

    #[test]
    fn a_window_with_room_reports_its_own_height() {
        assert_eq!(laid_out_window_height(200.0, 2, 600.0, HINT), 200.0);
        assert_eq!(laid_out_window_height(66.0, 1, 420.0, HINT), 66.0);
    }

    #[test]
    fn widening_alone_adds_padding_between_slots() {
        let narrow = hotbar_metrics(1, 420.0, Some(40.0), HINT);
        let wide = hotbar_metrics(1, 800.0, Some(40.0), HINT);
        assert_eq!(narrow.slot, wide.slot);
        assert!(wide.gap_x > narrow.gap_x, "{narrow:?} {wide:?}");
        fits(&wide, 800.0);
    }

    #[test]
    fn heightening_alone_adds_padding_between_rows() {
        let short = hotbar_metrics(2, 420.0, Some(120.0), HINT);
        let tall = hotbar_metrics(2, 420.0, Some(300.0), HINT);
        assert_eq!(short.slot, tall.slot, "{short:?} {tall:?}");
        assert!(tall.gap_y > short.gap_y);
        assert!(tall.height <= 300.0);
    }

    #[test]
    fn slots_grow_only_when_both_dimensions_allow() {
        let base = hotbar_metrics(1, 420.0, Some(40.0), HINT);
        let both = hotbar_metrics(1, 600.0, Some(70.0), HINT);
        assert!(both.slot > base.slot, "{base:?} {both:?}");
        let capped = hotbar_metrics(1, 5_000.0, Some(5_000.0), HINT);
        assert_eq!(capped.slot, HOTBAR_SLOT_MAXIMUM);
    }

    #[test]
    fn adding_a_row_shrinks_the_slots_and_removing_it_restores_them() {
        let one = hotbar_metrics(1, 600.0, Some(100.0), HINT);
        let two = hotbar_metrics(2, 600.0, Some(100.0), HINT);
        let four = hotbar_metrics(4, 600.0, Some(100.0), HINT);
        assert!(two.slot < one.slot, "{one:?} {two:?}");
        assert!(four.slot <= two.slot);
        assert_eq!(hotbar_metrics(1, 600.0, Some(100.0), HINT), one);
        for metrics in [one, two, four] {
            assert!(metrics.height <= 100.0 || metrics.slot == HOTBAR_SLOT_MINIMUM, "{metrics:?}");
        }
    }

    #[test]
    fn rows_never_wrap_even_at_the_minimum() {
        for rows in 1..=HOTBAR_ROWS {
            for width in [HOTBAR_WINDOW_MINIMUM_WIDTH, 300.0, 700.0] {
                let region = width - HOTBAR_WINDOW_BORDER * 2.0;
                let metrics = hotbar_metrics(rows, region, Some(10.0), HINT);
                fits(&metrics, region);
                assert_eq!(metrics.rows, rows);
                assert_eq!(metrics.slot, HOTBAR_SLOT_MINIMUM);
                // Too short: the grid asks for the height the minimum slots
                // need, once, and nothing more.
                assert_eq!(
                    metrics.height,
                    rows as f32 * (HOTBAR_SLOT_MINIMUM + HINT) + (rows - 1) as f32 * HOTBAR_SLOT_GAP
                );
            }
        }
    }

    #[test]
    fn f_keys_are_the_bottom_row() {
        let metrics = hotbar_metrics(3, 600.0, Some(200.0), HINT);
        let (_, f_row) = metrics.slot_origin(0, 0);
        let (_, number_row) = metrics.slot_origin(0, 1);
        let (_, q_row) = metrics.slot_origin(0, 2);
        assert!(q_row < number_row && number_row < f_row);
        assert_eq!(metrics.slot_origin(8, 0).0, 8.0 * (metrics.slot + metrics.gap_x));
        assert!(metrics.controls_left() + HOTBAR_ROW_CONTROLS_WIDTH <= 600.0 + 0.01);
    }

    #[test]
    fn only_multi_level_skills_show_their_level() {
        assert!(!shows_skill_level(1));
        assert!(!shows_skill_level(0));
        assert!(shows_skill_level(10));
    }

    #[test]
    fn hints_are_compact() {
        assert_eq!(compact_hint("F1", false, false, false), "F1");
        assert_eq!(compact_hint("F9", true, false, false), "A-F9");
        assert_eq!(compact_hint("F2", false, true, true), "C-S-F2");
        assert_eq!(compact_hint("Q", false, false, false), "Q");
    }
}
