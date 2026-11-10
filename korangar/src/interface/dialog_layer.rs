//! Keeps an open NPC dialog in front of the HUD.
//!
//! While a dialog is open its band (and an NPC input box) sits above every HUD
//! window, and the bottom menu bar is hidden. The band spans the bottom of the
//! screen where the bar docks, and a click anywhere advances the dialog, so a
//! bar peeking out from under the band would only be a dead target. The bar
//! comes back where it was (its anchor is in the window cache) as soon as the
//! dialog closes.

use korangar_interface::Interface;

use crate::interface::windows::{MenuBarWindow, WindowClass};
use crate::state::ClientState;

/// Front-to-back order of the dialog windows: the input box above the band.
pub const DIALOG_STACK: [WindowClass; 2] = [WindowClass::Dialog, WindowClass::NpcInput];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuBarAction {
    Keep,
    Hide,
    Restore,
}

/// What to do with the menu bar this frame. `hidden_by_dialog` is true when
/// the dialog layer closed the bar and still owes it back.
pub fn menu_bar_action(dialog_open: bool, bar_open: bool, hidden_by_dialog: bool) -> (MenuBarAction, bool) {
    match (dialog_open, bar_open, hidden_by_dialog) {
        (true, true, _) => (MenuBarAction::Hide, true),
        (true, false, hidden) => (MenuBarAction::Keep, hidden),
        (false, false, true) => (MenuBarAction::Restore, false),
        // The bar is open again (for example a fresh login opened it) or was
        // never ours to restore.
        (false, ..) => (MenuBarAction::Keep, false),
    }
}

#[derive(Debug, Default)]
pub struct DialogLayer {
    menu_bar_hidden: bool,
}

impl DialogLayer {
    /// Call once per frame before layout.
    pub fn sync(&mut self, interface: &mut Interface<'static, ClientState>) {
        let dialog_open = interface.is_window_with_class_open(WindowClass::Dialog);
        let bar_open = interface.is_window_with_class_open(WindowClass::MenuBar);
        let (action, hidden) = menu_bar_action(dialog_open, bar_open, self.menu_bar_hidden);
        self.menu_bar_hidden = hidden;

        match action {
            MenuBarAction::Hide => interface.close_window_with_class(WindowClass::MenuBar),
            MenuBarAction::Restore => interface.open_window(MenuBarWindow),
            MenuBarAction::Keep => {}
        }

        if dialog_open {
            interface.raise_window_classes(&DIALOG_STACK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_a_dialog_hides_the_bar_and_closing_it_brings_it_back() {
        let (action, hidden) = menu_bar_action(true, true, false);
        assert_eq!(action, MenuBarAction::Hide);
        assert!(hidden);

        // While the dialog stays open nothing else happens.
        assert_eq!(menu_bar_action(true, false, hidden), (MenuBarAction::Keep, true));

        assert_eq!(menu_bar_action(false, false, hidden), (MenuBarAction::Restore, false));
    }

    #[test]
    fn a_bar_the_dialog_did_not_hide_is_left_alone() {
        assert_eq!(menu_bar_action(false, false, false), (MenuBarAction::Keep, false));
        assert_eq!(menu_bar_action(false, true, false), (MenuBarAction::Keep, false));
        assert_eq!(menu_bar_action(true, false, false), (MenuBarAction::Keep, false));
    }

    #[test]
    fn a_bar_reopened_elsewhere_clears_the_debt() {
        assert_eq!(menu_bar_action(false, true, true), (MenuBarAction::Keep, false));
    }

    #[test]
    fn the_input_box_stays_above_the_band() {
        let dialog = DIALOG_STACK.iter().position(|class| *class == WindowClass::Dialog);
        let input = DIALOG_STACK.iter().position(|class| *class == WindowClass::NpcInput);
        assert!(dialog < input);
    }
}
