/// What Escape should do, given the current interface focus and open windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeAction {
    /// A text field or other element is focused — drop focus first.
    Unfocus,
    /// Close the topmost closable window. Chat and the hotbar stay open.
    CloseTop,
    /// No closable window is open — toggle the menu, matching the old bind.
    ToggleMenu,
}

pub fn escape_action(interface_has_focus: bool, has_closable_window: bool) -> EscapeAction {
    if interface_has_focus {
        EscapeAction::Unfocus
    } else if has_closable_window {
        EscapeAction::CloseTop
    } else {
        EscapeAction::ToggleMenu
    }
}

#[cfg(test)]
mod tests {
    use super::{EscapeAction, escape_action};

    #[test]
    fn escape_unfocuses_before_closing_windows() {
        assert_eq!(escape_action(true, true), EscapeAction::Unfocus);
        assert_eq!(escape_action(true, false), EscapeAction::Unfocus);
    }

    #[test]
    fn escape_closes_the_top_closable_window() {
        assert_eq!(escape_action(false, true), EscapeAction::CloseTop);
    }

    #[test]
    fn escape_opens_the_menu_when_nothing_is_closable() {
        assert_eq!(escape_action(false, false), EscapeAction::ToggleMenu);
    }
}
