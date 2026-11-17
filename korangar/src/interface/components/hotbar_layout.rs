//! Hotbar layout math (Matthew t274u).
//!
//! The hotbar window can be resized from every edge and corner (the #145
//! machinery). The grid then picks the row and column count that gives the
//! largest square slots for the region it gets, so a one-row bar made narrow
//! wraps onto more rows, and the slots shrink or grow with the window. Each
//! slot has a label line under it (skill or item name), so a cell is
//! `slot` wide and `slot + label_height` tall. Everything here is pure so the
//! unit tests and the e2e harness can check it without a renderer.

/// Smallest drawn slot at UI scale 1, same floor as the item grids.
pub const HOTBAR_SLOT_MINIMUM: f32 = 24.0;
/// Largest drawn slot at UI scale 1.
pub const HOTBAR_SLOT_MAXIMUM: f32 = 64.0;
/// Space between neighbouring cells.
pub const HOTBAR_SLOT_GAP: f32 = 2.0;
/// Label font, as the historical pixel size scaled by the theme body font.
pub const HOTBAR_LABEL_FONT_PX: f32 = 10.0;
/// Hotkey hint font (top-left corner of a slot).
pub const HOTBAR_HINT_FONT_PX: f32 = 9.0;
/// Narrowest hotbar window. The four control buttons still fit.
pub const HOTBAR_WINDOW_MINIMUM_WIDTH: f32 = 180.0;
/// Widest hotbar window; slots cap at [`HOTBAR_SLOT_MAXIMUM`] well before.
pub const HOTBAR_WINDOW_MAXIMUM_WIDTH: f32 = 1_200.0;
/// Shortest hotbar window: title, buttons and one row of small slots.
pub const HOTBAR_WINDOW_MINIMUM_HEIGHT: f32 = 96.0;
/// Tallest hotbar window.
pub const HOTBAR_WINDOW_MAXIMUM_HEIGHT: f32 = 720.0;
/// First-open hotbar height in interface units (screens from 900px tall):
/// buttons, one row of 40px slots and their labels.
pub const HOTBAR_DEFAULT_HEIGHT: f32 = 124.0;
/// Text appended to a truncated label.
pub const LABEL_ELLIPSIS: &str = "..";

/// Grid shape for one hotbar region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HotbarMetrics {
    pub columns: usize,
    pub rows: usize,
    /// Side of the square slot.
    pub slot: f32,
    /// Height of the label line under each slot.
    pub label_height: f32,
}

impl HotbarMetrics {
    /// Height of one cell: the slot plus its label.
    pub fn cell_height(&self) -> f32 {
        self.slot + self.label_height
    }

    /// Height the whole grid needs.
    pub fn grid_height(&self) -> f32 {
        self.rows as f32 * self.cell_height() + self.rows.saturating_sub(1) as f32 * HOTBAR_SLOT_GAP
    }

    /// Width the whole grid needs.
    pub fn grid_width(&self) -> f32 {
        self.columns as f32 * self.slot + self.columns.saturating_sub(1) as f32 * HOTBAR_SLOT_GAP
    }

    /// Column and row of the `index`-th drawn slot (row-major).
    pub fn cell_position(&self, index: usize) -> (usize, usize) {
        let columns = self.columns.max(1);
        (index % columns, index / columns)
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value.max(0.0) } else { 0.0 }
}

/// Largest slot for `slots` cells in a `width` x `height` region.
///
/// Every column count from `slots` down to one is tried; each needs
/// `ceil(slots / columns)` rows. The slot is limited by the column width and,
/// when the region height is known, by the row height minus the label line.
/// The largest slot wins; on a tie the layout with more columns wins, so a
/// roomy region keeps the classic one-row bar. The result is floored and
/// clamped to [`HOTBAR_SLOT_MINIMUM`]..[`HOTBAR_SLOT_MAXIMUM`].
pub fn hotbar_metrics(slots: usize, width: f32, height: Option<f32>, label_height: f32) -> HotbarMetrics {
    let slots = slots.max(1);
    let width = finite_or_zero(width);
    let label_height = finite_or_zero(label_height);
    let height = height.map(finite_or_zero);

    let mut best: Option<(f32, usize, usize)> = None;
    for columns in (1..=slots).rev() {
        let rows = slots.div_ceil(columns);
        let by_width = (width - HOTBAR_SLOT_GAP * (columns - 1) as f32) / columns as f32;
        let by_height = match height {
            Some(height) => (height - HOTBAR_SLOT_GAP * (rows - 1) as f32) / rows as f32 - label_height,
            None => f32::INFINITY,
        };
        let slot = by_width.min(by_height).min(HOTBAR_SLOT_MAXIMUM);
        if best.is_none_or(|(best_slot, ..)| slot > best_slot + 0.01) {
            best = Some((slot, columns, rows));
        }
    }
    let (slot, columns, rows) = best.expect("at least one column count");
    HotbarMetrics {
        columns,
        rows,
        slot: slot.floor().clamp(HOTBAR_SLOT_MINIMUM, HOTBAR_SLOT_MAXIMUM),
        label_height,
    }
}

/// Visual slot indices the grid draws, in order: the visible F-key rows,
/// then the two quick slots when all four rows are shown.
pub fn drawn_visual_slots(visible_rows: u8, columns_per_row: usize, rows_total: usize, quick: &[u16]) -> Vec<u16> {
    let rows = usize::from(visible_rows).clamp(1, rows_total);
    let mut slots: Vec<u16> = (0..rows * columns_per_row).map(|visual| visual as u16).collect();
    if rows == rows_total {
        slots.extend_from_slice(quick);
    }
    slots
}

/// Shorten `text` so `measure` reports at most `max_width`, ending in
/// [`LABEL_ELLIPSIS`]. Returns an empty string when not even one character
/// and the ellipsis fit.
pub fn ellipsize(text: &str, max_width: f32, measure: impl Fn(&str) -> f32) -> String {
    let text = text.trim();
    if text.is_empty() || !max_width.is_finite() || max_width <= 0.0 {
        return String::new();
    }
    if measure(text) <= max_width {
        return text.to_owned();
    }
    let characters: Vec<char> = text.chars().collect();
    for keep in (1..characters.len()).rev() {
        let mut candidate: String = characters[..keep].iter().collect();
        let trimmed = candidate.trim_end().len();
        candidate.truncate(trimmed);
        candidate.push_str(LABEL_ELLIPSIS);
        if measure(&candidate) <= max_width {
            return candidate;
        }
    }
    String::new()
}

/// Compact hotkey hint for a slot corner: `F1`, `A-F1`, `C-F1`, `S-F1`.
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

#[cfg(test)]
mod tests {
    use super::*;

    const LABEL: f32 = 12.0;

    #[test]
    fn roomy_region_keeps_one_row_of_nine() {
        let metrics = hotbar_metrics(9, 9.0 * 40.0 + 8.0 * HOTBAR_SLOT_GAP, Some(60.0), LABEL);
        assert_eq!((metrics.columns, metrics.rows), (9, 1));
        assert_eq!(metrics.slot, 40.0);
        assert!(metrics.grid_width() <= 9.0 * 40.0 + 8.0 * HOTBAR_SLOT_GAP + 0.01);
        assert!(metrics.grid_height() <= 60.0);
    }

    #[test]
    fn narrow_tall_region_wraps_onto_more_rows() {
        // 150 wide, 130 tall: nine in a row would be ~15px, 5x2 gives 28px.
        let metrics = hotbar_metrics(9, 150.0, Some(130.0), LABEL);
        assert!(metrics.rows >= 2, "{metrics:?}");
        assert!(metrics.columns * metrics.rows >= 9);
        assert!(metrics.slot >= HOTBAR_SLOT_MINIMUM);
    }

    #[test]
    fn slots_shrink_and_grow_with_the_region() {
        let small = hotbar_metrics(9, 300.0, Some(50.0), LABEL);
        let large = hotbar_metrics(9, 600.0, Some(80.0), LABEL);
        assert!(large.slot > small.slot, "{small:?} {large:?}");
        assert_eq!(small.columns, 9);
        assert_eq!(large.columns, 9);
    }

    #[test]
    fn slot_size_is_clamped() {
        assert_eq!(hotbar_metrics(9, 5_000.0, Some(5_000.0), LABEL).slot, HOTBAR_SLOT_MAXIMUM);
        assert_eq!(hotbar_metrics(9, 10.0, Some(10.0), LABEL).slot, HOTBAR_SLOT_MINIMUM);
        assert_eq!(hotbar_metrics(9, f32::NAN, None, LABEL).slot, HOTBAR_SLOT_MINIMUM);
        // Unknown height: width alone decides, ties prefer more columns.
        let unbounded = hotbar_metrics(9, 2_000.0, None, LABEL);
        assert_eq!((unbounded.columns, unbounded.rows), (9, 1));
    }

    #[test]
    fn four_rows_plus_quick_slots_fill_a_square_region() {
        let metrics = hotbar_metrics(38, 400.0, Some(400.0), LABEL);
        assert!(metrics.columns * metrics.rows >= 38);
        assert!(metrics.grid_width() <= 400.0 + 0.01);
        assert!(metrics.grid_height() <= 400.0 + 0.01);
        assert_eq!(metrics.cell_position(0), (0, 0));
        assert_eq!(metrics.cell_position(metrics.columns), (0, 1));
    }

    #[test]
    fn drawn_slots_follow_visible_rows() {
        assert_eq!(drawn_visual_slots(1, 9, 4, &[36, 37]), (0..9).collect::<Vec<u16>>());
        assert_eq!(drawn_visual_slots(2, 9, 4, &[36, 37]).len(), 18);
        let all = drawn_visual_slots(4, 9, 4, &[36, 37]);
        assert_eq!(all.len(), 38);
        assert_eq!(&all[36..], &[36, 37]);
        assert_eq!(drawn_visual_slots(0, 9, 4, &[36, 37]).len(), 9);
    }

    fn monospace(text: &str) -> f32 {
        text.chars().count() as f32 * 6.0
    }

    #[test]
    fn labels_are_ellipsized_to_the_slot_width() {
        assert_eq!(ellipsize("Bash", 40.0, monospace), "Bash");
        assert_eq!(ellipsize("Magnum Break", 40.0, monospace), "Magn..");
        assert!(monospace(&ellipsize("Magnum Break", 40.0, monospace)) <= 40.0);
        // Trailing spaces are dropped before the ellipsis.
        assert_eq!(ellipsize("Red Potion", 36.0, monospace), "Red..");
        assert_eq!(ellipsize("Heal", 6.0, monospace), "");
        assert_eq!(ellipsize("", 40.0, monospace), "");
        assert_eq!(ellipsize("Heal", f32::NAN, monospace), "");
        // Multi-byte names are cut on character boundaries.
        assert_eq!(ellipsize("ÄÖÜäöü", 24.0, monospace), "ÄÖ..");
    }

    #[test]
    fn hints_are_compact() {
        assert_eq!(compact_hint("F1", false, false, false), "F1");
        assert_eq!(compact_hint("F9", true, false, false), "A-F9");
        assert_eq!(compact_hint("F2", false, true, true), "C-S-F2");
    }
}
