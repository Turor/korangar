#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, Timer, print_debug};
use korangar_interface::element::StateElement;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

use crate::graphics::{Color, ScreenPosition, ScreenSize};
use crate::loaders::FontSize;

#[derive(Serialize, Deserialize, RustState, StateElement)]
pub struct OverlayTheme {
    pub foreground_color: Color,
    pub text_offset: ScreenPosition,
    pub font_size: FontSize,
}

impl Default for OverlayTheme {
    fn default() -> Self {
        Self {
            foreground_color: Color::monochrome_u8(220),
            text_offset: ScreenPosition { left: 20.0, top: 10.0 },
            font_size: FontSize(24.0),
        }
    }
}

#[derive(Serialize, Deserialize, RustState, StateElement)]
pub struct StatusBarTheme {
    pub background_color: Color,
    pub player_health_color: Color,
    pub enemy_health_color: Color,
    pub spell_point_color: Color,
    pub activity_point_color: Color,
    pub player_bar_width: f32,
    pub enemy_bar_width: f32,
    pub health_height: f32,
    pub enemy_health_height: f32,
    pub spell_point_height: f32,
    pub activity_point_height: f32,
    pub border_size: ScreenSize,
    pub gap: f32,
}

impl Default for StatusBarTheme {
    fn default() -> Self {
        Self {
            background_color: Color::monochrome_u8(40),
            player_health_color: Color::rgb_u8(67, 163, 83),
            enemy_health_color: Color::rgb_u8(206, 49, 116),
            spell_point_color: Color::rgb_u8(0, 129, 163),
            activity_point_color: Color::rgb_u8(218, 145, 81),
            player_bar_width: 85.0,
            enemy_bar_width: 60.0,
            health_height: 8.0,
            enemy_health_height: 6.0,
            spell_point_height: 4.0,
            activity_point_height: 4.0,
            border_size: ScreenSize { width: 2.0, height: 1.0 },
            gap: 1.0,
        }
    }
}

#[derive(Serialize, Deserialize, RustState, StateElement)]
pub struct IndicatorTheme {
    pub walking: Color,
}

impl Default for IndicatorTheme {
    fn default() -> Self {
        Self {
            walking: Color::rgba_u8(0, 255, 170, 170),
        }
    }
}

#[derive(Serialize, Deserialize, RustState, StateElement)]
pub struct CursorTheme {
    pub color: Color,
}

impl Default for CursorTheme {
    fn default() -> Self {
        Self {
            color: Color::monochrome_u8(255),
        }
    }
}

#[derive(Default, Serialize, Deserialize, RustState, StateElement)]
pub struct WorldTheme {
    pub overlay: OverlayTheme,
    pub status_bar: StatusBarTheme,
    pub indicator: IndicatorTheme,
    pub cursor: CursorTheme,
}

impl WorldTheme {
    /// Built-in Turoran dark world overlay (HP/SP/AP frames).
    ///
    /// Used when the selected world theme name is `turoran_dark` and no
    /// overriding `client/world_themes/turoran_dark.ron` is present.
    pub fn turoran_dark() -> Self {
        Self {
            overlay: OverlayTheme {
                foreground_color: Color::rgb_u8(230, 232, 240),
                text_offset: ScreenPosition { left: 18.0, top: 10.0 },
                font_size: FontSize(22.0),
            },
            status_bar: StatusBarTheme {
                background_color: Color::rgba_u8(10, 12, 18, 220),
                player_health_color: Color::rgb_u8(62, 168, 96),
                enemy_health_color: Color::rgb_u8(198, 64, 88),
                spell_point_color: Color::rgb_u8(64, 156, 210),
                activity_point_color: Color::rgb_u8(220, 150, 72),
                player_bar_width: 96.0,
                enemy_bar_width: 64.0,
                health_height: 9.0,
                enemy_health_height: 6.0,
                spell_point_height: 5.0,
                activity_point_height: 5.0,
                border_size: ScreenSize { width: 3.0, height: 2.0 },
                gap: 2.0,
            },
            indicator: IndicatorTheme {
                walking: Color::rgba_u8(80, 220, 190, 180),
            },
            cursor: CursorTheme {
                color: Color::rgb_u8(235, 236, 240),
            },
        }
    }

    /// Verdant Rune world overlay. HP, SP, and gold activity use the interface
    /// palette. Used when the world theme name is `verdant_rune` and no
    /// overriding RON file is present.
    pub fn verdant_rune() -> Self {
        Self {
            overlay: OverlayTheme {
                foreground_color: Color::rgb_hex("E3EDE6"),
                text_offset: ScreenPosition { left: 18.0, top: 10.0 },
                font_size: FontSize(22.0),
            },
            status_bar: StatusBarTheme {
                background_color: Color::rgb_hex("0F1714").with_alpha_u8(220),
                player_health_color: Color::rgb_hex("D2524A"),
                enemy_health_color: Color::rgb_hex("E0708A"),
                spell_point_color: Color::rgb_hex("4A86D8"),
                activity_point_color: Color::rgb_hex("CFAE5A"),
                player_bar_width: 96.0,
                enemy_bar_width: 64.0,
                health_height: 9.0,
                enemy_health_height: 6.0,
                spell_point_height: 5.0,
                activity_point_height: 5.0,
                border_size: ScreenSize { width: 3.0, height: 2.0 },
                gap: 2.0,
            },
            indicator: IndicatorTheme {
                walking: Color::rgb_hex("4CC3A0").with_alpha_u8(180),
            },
            cursor: CursorTheme {
                color: Color::rgb_hex("E3EDE6"),
            },
        }
    }

    pub fn load(name: &str) -> Self {
        use crate::settings::{TURORAN_DARK_THEME_NAME, VERDANT_RUNE_THEME_NAME, WORLD_THEMES_PATH};

        #[cfg(feature = "debug")]
        let timer = Timer::new("Load theme");

        let path = format!("{WORLD_THEMES_PATH}/{name}.ron");

        #[cfg(feature = "debug")]
        print_debug!("loading theme from file {}", path.magenta());

        let theme = std::fs::read_to_string(&path)
            .ok()
            .and_then(|data| ron::from_str(&data).ok())
            .unwrap_or_else(|| {
                if name == TURORAN_DARK_THEME_NAME {
                    return WorldTheme::turoran_dark();
                }
                if name == VERDANT_RUNE_THEME_NAME {
                    return WorldTheme::verdant_rune();
                }
                #[cfg(feature = "debug")]
                print_debug!("[{}] failed to load theme {}", "error".red(), name.magenta());
                WorldTheme::default()
            });

        #[cfg(feature = "debug")]
        timer.stop();

        theme
    }

    #[cfg(feature = "debug")]
    pub fn save(&self, name: &str) {
        use crate::settings::WORLD_THEMES_PATH;

        let timer = Timer::new("Save theme");

        let path = format!("{WORLD_THEMES_PATH}/{name}.ron");

        print_debug!("saving theme to file {}", path.magenta());

        let data = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new()).unwrap();

        if let Err(error) = std::fs::write(path, data) {
            print_debug!(
                "[{}] failed to save theme to {}: {:?}",
                "error".red(),
                name.magenta(),
                error.red()
            );
        }

        timer.stop();
    }
}

#[cfg(test)]
mod turoran_dark_world_theme_tests {
    use super::WorldTheme;
    use crate::settings::TURORAN_DARK_THEME_NAME;

    #[test]
    fn turoran_dark_status_bar_is_framed_and_distinct() {
        let theme = WorldTheme::turoran_dark();
        let default = WorldTheme::default();
        assert!(theme.status_bar.border_size.width >= default.status_bar.border_size.width);
        assert!(theme.status_bar.player_bar_width >= default.status_bar.player_bar_width);
        assert_ne!(
            theme.status_bar.background_color.alpha_as_u8(),
            255,
            "player frame should stay translucent"
        );
        assert_ne!(theme.status_bar.player_health_color, default.status_bar.player_health_color);
    }

    #[test]
    fn load_falls_back_to_built_in_turoran_dark() {
        let loaded = WorldTheme::load(TURORAN_DARK_THEME_NAME);
        let expected = WorldTheme::turoran_dark();
        assert_eq!(loaded.status_bar.player_bar_width, expected.status_bar.player_bar_width);
        assert_eq!(
            loaded.status_bar.background_color.alpha_as_u8(),
            expected.status_bar.background_color.alpha_as_u8()
        );
    }
}

#[cfg(test)]
mod verdant_rune_world_theme_tests {
    use super::WorldTheme;
    use crate::settings::VERDANT_RUNE_THEME_NAME;

    #[test]
    fn verdant_rune_status_uses_palette() {
        let theme = WorldTheme::verdant_rune();
        assert_eq!(theme.status_bar.player_health_color.red_as_u8(), 210);
        assert_eq!(theme.status_bar.player_health_color.green_as_u8(), 82);
        assert_eq!(theme.status_bar.player_health_color.blue_as_u8(), 74);
        assert_eq!(theme.status_bar.spell_point_color.red_as_u8(), 74);
        assert_eq!(theme.status_bar.spell_point_color.green_as_u8(), 134);
        assert_eq!(theme.status_bar.spell_point_color.blue_as_u8(), 216);
        assert_eq!(theme.status_bar.activity_point_color.red_as_u8(), 207);
        assert_ne!(
            theme.status_bar.player_health_color,
            WorldTheme::default().status_bar.player_health_color
        );
        assert_ne!(
            theme.status_bar.player_health_color,
            WorldTheme::turoran_dark().status_bar.player_health_color
        );
    }

    #[test]
    fn load_falls_back_to_built_in_verdant_rune() {
        let loaded = WorldTheme::load(VERDANT_RUNE_THEME_NAME);
        let expected = WorldTheme::verdant_rune();
        assert_eq!(
            loaded.status_bar.player_health_color.red_as_u8(),
            expected.status_bar.player_health_color.red_as_u8()
        );
        assert_eq!(
            loaded.status_bar.spell_point_color.blue_as_u8(),
            expected.status_bar.spell_point_color.blue_as_u8()
        );
    }
}
