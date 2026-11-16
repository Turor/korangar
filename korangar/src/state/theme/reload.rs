//! Decide when the in-memory interface theme should be rebuilt.
//!
//! A new theme name, a changed token file, a font-scale percent, or an
//! explicit reload replaces the themes already stored on the client. The
//! replacement happens in the frame loop, without restarting the process.

use std::time::SystemTime;

pub fn theme_reload_due(
    loaded_name: &str,
    loaded_mtime: Option<SystemTime>,
    loaded_font_percent: u16,
    selected_name: &str,
    selected_mtime: Option<SystemTime>,
    selected_font_percent: u16,
    force: bool,
) -> bool {
    force || loaded_name != selected_name || loaded_mtime != selected_mtime || loaded_font_percent != selected_font_percent
}

#[cfg(test)]
mod tests {
    use super::theme_reload_due;
    use crate::settings::{EMBER_FORGE_THEME_NAME, VERDANT_RUNE_THEME_NAME};
    use crate::state::theme::{InterfaceTheme, InterfaceThemeType};

    #[test]
    fn theme_change_updates_in_memory_tokens_without_a_restart() {
        let verdant = InterfaceTheme::load(InterfaceThemeType::InGame, VERDANT_RUNE_THEME_NAME);
        let ember = InterfaceTheme::load(InterfaceThemeType::InGame, EMBER_FORGE_THEME_NAME);
        assert_ne!(verdant.tokens.name, ember.tokens.name);
        assert_ne!(verdant.tokens.border, ember.tokens.border);
        assert_ne!(verdant.window.background_color, ember.window.background_color);

        assert!(!theme_reload_due(
            VERDANT_RUNE_THEME_NAME,
            None,
            100,
            VERDANT_RUNE_THEME_NAME,
            None,
            100,
            false
        ));
        assert!(theme_reload_due(
            VERDANT_RUNE_THEME_NAME,
            None,
            100,
            EMBER_FORGE_THEME_NAME,
            None,
            100,
            false
        ));
        assert!(theme_reload_due(
            VERDANT_RUNE_THEME_NAME,
            None,
            100,
            VERDANT_RUNE_THEME_NAME,
            None,
            120,
            false
        ));
        assert!(theme_reload_due(
            VERDANT_RUNE_THEME_NAME,
            None,
            100,
            VERDANT_RUNE_THEME_NAME,
            None,
            100,
            true
        ));
    }
}
