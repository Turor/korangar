use std::path::PathBuf;

#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, print_debug};
use korangar_interface::element::StateElement;
use ron::ser::PrettyConfig;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

use crate::loaders::Scaling;
use crate::state::localization::Language;

/// This theme name includes a zero byte so that it can not point to an actual
/// file. This is guaranteed to fail to load which will automatically fall back
/// to the default theme. The only issue is that loading the default theme will
/// cause an error to appear when running Korangar with debug features.
pub const DEFAULT_THEME_NAME: &str = "^000001default^000000\0";
/// Built-in selectable Turoran dark in-game HUD theme.
///
/// Always offered in Interface Settings even when no RON file is present.
/// A matching `client/in_game_themes/turoran_dark.ron` overrides the built-in
/// palette when present.
pub const TURORAN_DARK_THEME_NAME: &str = "turoran_dark";
/// Built-in selectable Verdant Rune in-game HUD theme.
///
/// Always offered in Interface Settings even when no RON file is present.
/// A matching `client/in_game_themes/verdant_rune.ron` (or world theme file)
/// overrides the built-in palette when present. First-run defaults and
/// `KORANGAR_HEADLESS_AUTO` screenshots use this theme.
pub const VERDANT_RUNE_THEME_NAME: &str = "verdant_rune";
/// Built-in Ember Forge theme (token file `themes/ember_forge.ron`).
pub const EMBER_FORGE_THEME_NAME: &str = "ember_forge";
/// Built-in Arcane Violet theme (token file `themes/arcane_violet.ron`).
pub const ARCANE_VIOLET_THEME_NAME: &str = "arcane_violet";

/// Linux: `$XDG_CONFIG_HOME/korangar/themes` or `~/.config/korangar/themes`.
/// Windows: `%APPDATA%\korangar\themes`. Override with `KORANGAR_THEMES_DIR`.
pub fn user_themes_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("KORANGAR_THEMES_DIR") {
        return PathBuf::from(path);
    }
    if cfg!(windows)
        && let Some(appdata) = std::env::var_os("APPDATA")
    {
        return PathBuf::from(appdata).join("korangar").join("themes");
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("korangar").join("themes");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config").join("korangar").join("themes");
    }
    PathBuf::from("themes")
}

fn bundled_theme_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        dirs.push(parent.join("themes"));
    }
    dirs.push(PathBuf::from("themes"));
    dirs.push(PathBuf::from("client/themes"));
    dirs
}

/// Directories searched for `<name>.ron` / `<name>.toml`, user config first.
pub fn theme_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![user_themes_dir()];
    dirs.extend(bundled_theme_dirs());
    dirs
}

fn headless_auto_session() -> bool {
    std::env::var_os("KORANGAR_HEADLESS_AUTO").is_some()
}

/// Theme name to load. Headless screenshots use [`VERDANT_RUNE_THEME_NAME`].
/// The returned name must not be stored on [`InterfaceSettings`]: that type
/// saves itself on drop.
pub fn session_theme_name(saved: &str) -> &str {
    theme_name_to_load(saved, headless_auto_session())
}

fn theme_name_to_load(saved: &str, headless: bool) -> &str {
    if headless { VERDANT_RUNE_THEME_NAME } else { saved }
}

fn should_persist_interface_settings(headless: bool) -> bool {
    !headless
}
pub const MENU_THEMES_PATH: &str = "client/menu_themes";
pub const IN_GAME_THEMES_PATH: &str = "client/in_game_themes";
pub const WORLD_THEMES_PATH: &str = "client/world_themes";

#[derive(Clone, Serialize, Deserialize, RustState, StateElement)]
pub struct InterfaceSettings {
    pub language: Language,
    pub scaling: Scaling,
    pub menu_theme: String,
    pub in_game_theme: String,
    pub world_theme: String,
}

impl Default for InterfaceSettings {
    fn default() -> Self {
        Self {
            language: Language::English,
            scaling: Scaling::new(1.0),
            menu_theme: DEFAULT_THEME_NAME.to_string(),
            in_game_theme: VERDANT_RUNE_THEME_NAME.to_string(),
            world_theme: VERDANT_RUNE_THEME_NAME.to_string(),
        }
    }
}

impl InterfaceSettings {
    const FILE_NAME: &'static str = "client/interface_settings.ron";

    pub fn new() -> Self {
        Self::load().unwrap_or_else(|| {
            #[cfg(feature = "debug")]
            print_debug!("failed to load interface settings from {}", Self::FILE_NAME.magenta());

            Default::default()
        })
    }

    pub fn load() -> Option<Self> {
        #[cfg(feature = "debug")]
        print_debug!("loading interface settings from {}", Self::FILE_NAME.magenta());

        std::fs::read_to_string(Self::FILE_NAME)
            .ok()
            .and_then(|data| ron::from_str(&data).ok())
    }

    pub fn save(&self) {
        // Headless only borrows Verdant Rune in memory. Never rewrite the file.
        if !should_persist_interface_settings(headless_auto_session()) {
            return;
        }

        #[cfg(feature = "debug")]
        print_debug!("saving interface settings to {}", Self::FILE_NAME.magenta());

        let data = ron::ser::to_string_pretty(self, PrettyConfig::new()).unwrap();

        if let Err(_error) = std::fs::write(Self::FILE_NAME, data) {
            #[cfg(feature = "debug")]
            print_debug!(
                "failed to save interface settings to {}: {:?}",
                Self::FILE_NAME.magenta(),
                _error.red()
            );
        }
    }
}

impl Drop for InterfaceSettings {
    fn drop(&mut self) {
        self.save();
    }
}

#[derive(RustState, StateElement)]
pub struct InterfaceSettingsCapabilities {
    languages: Vec<Language>,
    scalings: Vec<Scaling>,
    menu_themes: Vec<String>,
    in_game_themes: Vec<String>,
    world_themes: Vec<String>,
}

impl InterfaceSettingsCapabilities {
    fn load_themes(directory: &str) -> Vec<String> {
        let mut themes = vec![DEFAULT_THEME_NAME.to_string()];

        if directory == IN_GAME_THEMES_PATH || directory == WORLD_THEMES_PATH {
            for name in [
                VERDANT_RUNE_THEME_NAME,
                EMBER_FORGE_THEME_NAME,
                ARCANE_VIOLET_THEME_NAME,
                TURORAN_DARK_THEME_NAME,
            ] {
                if !themes.iter().any(|existing| existing == name) {
                    themes.push(name.to_string());
                }
            }
            for dir in theme_search_dirs() {
                let Ok(entries) = std::fs::read_dir(dir) else { continue };
                for entry in entries.filter_map(|entry| entry.ok()) {
                    let file_name = entry.file_name();
                    let Some(stem) = file_name.to_str().and_then(|name| {
                        name.strip_suffix(".ron")
                            .or_else(|| name.strip_suffix(".toml"))
                            .map(ToOwned::to_owned)
                    }) else {
                        continue;
                    };
                    if !stem.is_empty() && !themes.iter().any(|existing| existing == &stem) {
                        themes.push(stem);
                    }
                }
            }
        }

        if let Ok(entries) = std::fs::read_dir(directory) {
            for name in entries.filter_map(|entry| entry.ok()).filter_map(|entry| {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                name.strip_suffix(".ron")
                    .or_else(|| name.strip_suffix(".toml"))
                    .map(ToOwned::to_owned)
            }) {
                if !themes.iter().any(|existing| existing == &name) {
                    themes.push(name);
                }
            }
        }

        // Sort themes excluding the default since we always want that to be first.
        if themes.len() > 1 {
            themes[1..].sort_unstable();
        }

        themes
    }

    #[cfg(test)]
    fn in_game_theme_names_for_test() -> Vec<String> {
        Self::load_themes(IN_GAME_THEMES_PATH)
    }
}

impl Default for InterfaceSettingsCapabilities {
    fn default() -> Self {
        Self {
            // TODO: Don't hardcode this, load it from the disk instead.
            languages: vec![Language::English, Language::German],
            scalings: vec![
                Scaling::new(0.5),
                Scaling::new(0.6),
                Scaling::new(0.7),
                Scaling::new(0.8),
                Scaling::new(0.9),
                Scaling::new(1.0),
                Scaling::new(1.1),
                Scaling::new(1.2),
                Scaling::new(1.3),
                Scaling::new(1.4),
                Scaling::new(1.5),
                Scaling::new(1.6),
                Scaling::new(1.7),
                Scaling::new(1.8),
                Scaling::new(1.9),
                Scaling::new(2.0),
            ],
            menu_themes: Self::load_themes(MENU_THEMES_PATH),
            in_game_themes: Self::load_themes(IN_GAME_THEMES_PATH),
            world_themes: Self::load_themes(WORLD_THEMES_PATH),
        }
    }
}

#[cfg(test)]
mod interface_settings_theme_tests {
    use super::{DEFAULT_THEME_NAME, InterfaceSettings, InterfaceSettingsCapabilities, TURORAN_DARK_THEME_NAME, VERDANT_RUNE_THEME_NAME};

    #[test]
    fn in_game_theme_capabilities_include_turoran_dark() {
        let themes = InterfaceSettingsCapabilities::in_game_theme_names_for_test();
        assert_eq!(themes.first().map(String::as_str), Some(DEFAULT_THEME_NAME));
        assert!(
            themes.iter().any(|name| name == TURORAN_DARK_THEME_NAME),
            "expected {TURORAN_DARK_THEME_NAME} in {themes:?}"
        );
        assert!(
            themes.iter().any(|name| name == VERDANT_RUNE_THEME_NAME),
            "expected {VERDANT_RUNE_THEME_NAME} in {themes:?}"
        );
        assert!(
            themes.iter().any(|name| name == super::EMBER_FORGE_THEME_NAME),
            "expected ember_forge in {themes:?}"
        );
        assert!(
            themes.iter().any(|name| name == super::ARCANE_VIOLET_THEME_NAME),
            "expected arcane_violet in {themes:?}"
        );
    }

    #[test]
    fn headless_loads_verdant_rune_without_persisting() {
        assert_eq!(
            super::theme_name_to_load(TURORAN_DARK_THEME_NAME, true),
            VERDANT_RUNE_THEME_NAME
        );
        assert_eq!(
            super::theme_name_to_load(TURORAN_DARK_THEME_NAME, false),
            TURORAN_DARK_THEME_NAME
        );
        assert!(!super::should_persist_interface_settings(true));
        assert!(super::should_persist_interface_settings(false));
    }

    #[test]
    fn first_run_defaults_to_verdant_rune() {
        use std::mem::ManuallyDrop;

        let settings = ManuallyDrop::new(InterfaceSettings::default());
        assert_eq!(settings.in_game_theme, VERDANT_RUNE_THEME_NAME);
        assert_eq!(settings.world_theme, VERDANT_RUNE_THEME_NAME);
        assert_eq!(settings.menu_theme, DEFAULT_THEME_NAME);
    }

    #[test]
    fn interface_settings_preserve_turoran_dark_selection() {
        use std::mem::ManuallyDrop;

        use crate::loaders::Scaling;
        use crate::state::localization::Language;

        let settings = ManuallyDrop::new(InterfaceSettings {
            language: Language::English,
            scaling: Scaling::new(1.0),
            menu_theme: DEFAULT_THEME_NAME.to_string(),
            in_game_theme: TURORAN_DARK_THEME_NAME.to_string(),
            world_theme: DEFAULT_THEME_NAME.to_string(),
        });

        let encoded = ron::ser::to_string_pretty(&*settings, ron::ser::PrettyConfig::new()).expect("serialize settings");
        let decoded = ManuallyDrop::new(ron::from_str::<InterfaceSettings>(&encoded).expect("deserialize settings"));

        assert_eq!(decoded.in_game_theme, TURORAN_DARK_THEME_NAME);
        assert_eq!(decoded.menu_theme, settings.menu_theme);
        assert_eq!(decoded.world_theme, settings.world_theme);
        assert!(matches!(decoded.language, Language::English));
        assert_eq!(decoded.scaling.get_factor(), settings.scaling.get_factor());
    }
}
