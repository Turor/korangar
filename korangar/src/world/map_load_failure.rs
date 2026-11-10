//! What to do when a map cannot be loaded.
//!
//! A failed map load used to be dropped after a log line, which left the
//! player on a black screen with no way out. Now the player sees which map
//! failed and is sent back to character selection.

/// What the client does after a failed map load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapLoadFailure {
    /// Text for the error window.
    pub message: String,
    /// Leave the map server so character selection comes back.
    pub return_to_character_selection: bool,
}

pub fn on_map_load_failed(map_name: &str, map_server_connected: bool) -> MapLoadFailure {
    let map_name = map_name.trim_end_matches(".rsw");
    match map_server_connected {
        true => MapLoadFailure {
            message: format!(
                "The map \"{map_name}\" could not be loaded: its files are missing from the client data. Returning to character selection."
            ),
            return_to_character_selection: true,
        },
        false => MapLoadFailure {
            message: format!("The map \"{map_name}\" could not be loaded: its files are missing from the client data."),
            return_to_character_selection: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_in_game_failure_names_the_map_and_returns_to_character_selection() {
        let failure = on_map_load_failed("int_land", true);
        assert!(failure.message.contains("\"int_land\""));
        assert!(failure.message.contains("character selection"));
        assert!(failure.return_to_character_selection);
    }

    #[test]
    fn a_menu_map_failure_only_reports() {
        let failure = on_map_load_failed("new_zone01.rsw", false);
        assert!(failure.message.contains("\"new_zone01\""));
        assert!(!failure.return_to_character_selection);
    }
}
