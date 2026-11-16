//! Keyboard navigation for the inventory grid (inventory review #17).
//!
//! Same model as the NPC dialog choices: arrows move a cursor over the drawn
//! cells, Home / End jump to the first / last stack, Enter uses or equips the
//! stack under the cursor (the double-click action). The cursor is a view
//! position, so it follows the current tab, sort and search; the action
//! always uses that cell's server index. Korangar has no gamepad input
//! layer yet, so a controller drives this through the same key events.

use ragnarok_packets::InventoryIndex;

use crate::interface::components::item_grid::GRID_COLUMNS;
use crate::state::inventory_view::InventoryView;

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

/// One frame of input as the grid sees it.
#[derive(Clone, Debug, Default)]
pub struct GridFrame {
    pub inventory_open: bool,
    /// A text box has focus, or the NPC dialog took the key this frame.
    pub keys_taken: bool,
    /// The NPC dialog waits for Next / Close / a choice / an input.
    pub dialog_waiting: bool,
    /// The right-click item menu is open; its Up/Down own the arrows.
    pub menu_open: bool,
    pub mouse_clicked: bool,
    pub escape: bool,
    /// Any other key went down (letters, F-keys, hotbar, Space, Tab, ...).
    pub other_key: bool,
    pub keys: Vec<GridKey>,
    pub enter: bool,
}

/// Whether grid keys apply this frame at all.
pub fn grid_keys_allowed(inventory_open: bool, keys_taken: bool, dialog_waiting: bool, menu_open: bool) -> bool {
    inventory_open && !keys_taken && !dialog_waiting && !menu_open
}

/// Applies one frame to the view and returns the stack Enter uses, if any.
///
/// Enter belongs to the grid only while it is armed: a grid key was the
/// last input. A mouse click, Escape, any other key, focus elsewhere, an
/// open item menu or a waiting dialog disarms it, and so does using a
/// stack, so a later Enter goes to chat. Closing the inventory also drops
/// the cursor.
pub fn grid_frame(view: &mut InventoryView, frame: &GridFrame) -> Option<InventoryIndex> {
    if !frame.inventory_open {
        view.clear_cursor();
        return None;
    }
    if frame.mouse_clicked
        || frame.escape
        || frame.other_key
        || !grid_keys_allowed(frame.inventory_open, frame.keys_taken, frame.dialog_waiting, frame.menu_open)
    {
        view.disarm_cursor();
        return None;
    }
    for key in &frame.keys {
        view.move_cursor(*key);
    }
    if frame.enter && frame.keys.is_empty() && view.cursor_armed() {
        let used = view.cursor_item().map(|item| item.index);
        // One Enter, one use: the next Enter goes to chat again.
        view.disarm_cursor();
        return used;
    }
    None
}

/// Enter may still focus chat this frame: neither the NPC dialog nor the
/// grid took it. `lib.rs` checks this first in the Enter-focuses-chat rule.
pub fn enter_free_for_chat(grid_took_enter: bool, dialog_ate_confirm: bool) -> bool {
    !grid_took_enter && !dialog_ate_confirm
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
    fn keys_are_ignored_when_closed_focused_or_in_a_dialog() {
        assert!(grid_keys_allowed(true, false, false, false));
        assert!(!grid_keys_allowed(false, false, false, false));
        assert!(!grid_keys_allowed(true, true, false, false));
        assert!(!grid_keys_allowed(true, false, true, false));
        assert!(!grid_keys_allowed(true, false, false, true));
        assert!(enter_free_for_chat(false, false));
        assert!(!enter_free_for_chat(true, false));
        assert!(!enter_free_for_chat(false, true));
    }

    #[test]
    fn reveal_fraction_is_the_cursor_row() {
        assert_eq!(row_fraction(0, 4), Some((0.0, 0.25)));
        assert_eq!(row_fraction(35, 4), Some((0.75, 1.0)));
        assert_eq!(row_fraction(0, 0), None);
    }
}
