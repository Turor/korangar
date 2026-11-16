//! Keyboard navigation for the inventory grid (inventory review #17).
//!
//! Same model as the NPC dialog choices: arrows move a cursor over the drawn
//! cells, Home / End jump to the first / last stack, Enter uses or equips the
//! stack under the cursor (the double-click action). The cursor is a view
//! position, so it follows the current tab, sort and search; the action
//! always uses that cell's server index. Korangar has no gamepad input
//! layer yet, so a controller drives this through the same key events.

use crate::interface::components::item_grid::GRID_COLUMNS;

/// One navigation key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridKey {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// Next cursor position over `len` drawn stacks. The first key press on an
/// empty cursor lands on the first stack (End on the last). Moves stop at
/// the edges; Down from a row whose column is past the end goes to the last
/// stack.
pub fn step_cursor(cursor: Option<usize>, len: usize, key: GridKey) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let last = len - 1;
    let Some(cursor) = cursor.map(|cursor| cursor.min(last)) else {
        return Some(if key == GridKey::End { last } else { 0 });
    };
    Some(match key {
        GridKey::Left => cursor.saturating_sub(1),
        GridKey::Right => (cursor + 1).min(last),
        GridKey::Up => cursor.checked_sub(GRID_COLUMNS).unwrap_or(cursor),
        GridKey::Down if cursor + GRID_COLUMNS <= last => cursor + GRID_COLUMNS,
        GridKey::Down if cursor / GRID_COLUMNS < last / GRID_COLUMNS => last,
        GridKey::Down => cursor,
        GridKey::Home => 0,
        GridKey::End => last,
    })
}

/// Scroll range of the cursor's row as fractions of the grid height, for
/// `scroll_view!`'s `reveal`.
pub fn row_fraction(cursor: usize, rows: usize) -> Option<(f32, f32)> {
    if rows == 0 {
        return None;
    }
    let row = (cursor / GRID_COLUMNS).min(rows - 1);
    Some((row as f32 / rows as f32, (row + 1) as f32 / rows as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_press_lands_on_an_edge() {
        assert_eq!(step_cursor(None, 0, GridKey::Right), None);
        assert_eq!(step_cursor(None, 5, GridKey::Down), Some(0));
        assert_eq!(step_cursor(None, 5, GridKey::End), Some(4));
    }

    #[test]
    fn moves_stop_at_the_edges() {
        assert_eq!(step_cursor(Some(0), 25, GridKey::Left), Some(0));
        assert_eq!(step_cursor(Some(9), 25, GridKey::Right), Some(10));
        assert_eq!(step_cursor(Some(24), 25, GridKey::Right), Some(24));
        assert_eq!(step_cursor(Some(3), 25, GridKey::Up), Some(3));
        assert_eq!(step_cursor(Some(13), 25, GridKey::Up), Some(3));
        assert_eq!(step_cursor(Some(3), 25, GridKey::Down), Some(13));
        // Column 8 does not exist on the last row (20..=24): go to 24.
        assert_eq!(step_cursor(Some(18), 25, GridKey::Down), Some(24));
        assert_eq!(step_cursor(Some(22), 25, GridKey::Down), Some(22));
    }

    #[test]
    fn a_stale_cursor_is_clamped() {
        assert_eq!(step_cursor(Some(40), 12, GridKey::Left), Some(10));
    }

    #[test]
    fn reveal_fraction_is_the_cursor_row() {
        assert_eq!(row_fraction(0, 4), Some((0.0, 0.25)));
        assert_eq!(row_fraction(35, 4), Some((0.75, 1.0)));
        assert_eq!(row_fraction(0, 0), None);
    }
}
