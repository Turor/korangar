//! Data-driven interface and world theme tokens.
//!
//! One file per theme (`themes/<name>.ron` or `.toml`) is expanded into the
//! widget [`InterfaceTheme`] and [`WorldTheme`]. UI code should read these
//! tokens instead of hard-coded colors.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use korangar_interface::element::StateElement;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

use super::{OverlayTheme, StatusBarTheme, WorldTheme};
use crate::graphics::{Color, ScreenPosition, ScreenSize};
use crate::loaders::FontSize;
use crate::settings::{
    ARCANE_VIOLET_THEME_NAME, EMBER_FORGE_THEME_NAME, IN_GAME_THEMES_PATH, TURORAN_DARK_THEME_NAME, VERDANT_RUNE_THEME_NAME,
    WORLD_THEMES_PATH, theme_search_dirs,
};

/// Named palette used by HUD, Gear, inventory, minimap, tooltips, and the world
/// overlay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, RustState, StateElement)]
pub struct ThemeTokens {
    pub name: String,
    pub bg: Color,
    pub panel: Color,
    pub raised: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent2: Color,
    /// Slot hover border. Gold / amber, never the default slot border.
    pub hover: Color,
    pub hp: Color,
    pub sp: Color,
    /// Refine, rarity, and other small gold accents only.
    pub gold: Color,
    pub slot_size: f32,
    pub corner_radius: f32,
    pub font_size: f32,
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self::verdant_rune()
    }
}

impl ThemeTokens {
    pub fn verdant_rune() -> Self {
        parse_theme_file(include_str!("../../../../themes/verdant_rune.ron")).expect("bundled verdant_rune")
    }

    pub fn ember_forge() -> Self {
        parse_theme_file(include_str!("../../../../themes/ember_forge.ron")).expect("bundled ember_forge")
    }

    pub fn arcane_violet() -> Self {
        parse_theme_file(include_str!("../../../../themes/arcane_violet.ron")).expect("bundled arcane_violet")
    }

    pub fn turoran_dark() -> Self {
        Self {
            name: "Turoran Dark".to_owned(),
            bg: Color::rgb_u8(12, 14, 20),
            panel: Color::rgba_u8(18, 20, 28, 255),
            raised: Color::rgb_u8(28, 32, 42),
            border: Color::rgb_u8(58, 66, 82),
            text: Color::rgb_u8(235, 236, 240),
            muted: Color::rgb_u8(176, 180, 192),
            accent: Color::rgb_u8(255, 152, 48),
            accent2: Color::rgb_u8(198, 64, 88),
            hover: Color::rgb_u8(255, 170, 60),
            hp: Color::rgb_u8(62, 168, 96),
            sp: Color::rgb_u8(64, 156, 210),
            gold: Color::rgb_u8(220, 150, 72),
            slot_size: 44.0,
            corner_radius: 10.0,
            font_size: 13.0,
        }
    }

    pub fn builtin(name: &str) -> Option<Self> {
        match name {
            VERDANT_RUNE_THEME_NAME => Some(Self::verdant_rune()),
            EMBER_FORGE_THEME_NAME => Some(Self::ember_forge()),
            ARCANE_VIOLET_THEME_NAME => Some(Self::arcane_violet()),
            TURORAN_DARK_THEME_NAME => Some(Self::turoran_dark()),
            _ => None,
        }
    }
}

/// On-disk token file. Colors are `#RRGGBB` strings so the file is easy to
/// edit.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThemeFile {
    pub name: String,
    pub bg: String,
    pub panel: String,
    pub raised: String,
    pub border: String,
    pub text: String,
    pub muted: String,
    pub accent: String,
    pub accent2: String,
    pub hover: String,
    pub hp: String,
    pub sp: String,
    pub gold: String,
    #[serde(default = "default_slot_size")]
    pub slot_size: f32,
    #[serde(default = "default_corner_radius")]
    pub corner_radius: f32,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
}

fn default_slot_size() -> f32 {
    44.0
}
fn default_corner_radius() -> f32 {
    8.0
}
fn default_font_size() -> f32 {
    13.0
}

/// Parse `#RRGGBB` or `RRGGBB`.
pub fn parse_hex_color(value: &str) -> Option<Color> {
    let trimmed = value
        .trim()
        .trim_start_matches('#')
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if trimmed.len() != 6 {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&trimmed[range], 16).ok();
    Some(Color::rgb_u8(channel(0..2)?, channel(2..4)?, channel(4..6)?))
}

impl ThemeFile {
    pub fn into_tokens(self) -> Option<ThemeTokens> {
        Some(ThemeTokens {
            name: self.name,
            bg: parse_hex_color(&self.bg)?,
            panel: parse_hex_color(&self.panel)?,
            raised: parse_hex_color(&self.raised)?,
            border: parse_hex_color(&self.border)?,
            text: parse_hex_color(&self.text)?,
            muted: parse_hex_color(&self.muted)?,
            accent: parse_hex_color(&self.accent)?,
            accent2: parse_hex_color(&self.accent2)?,
            hover: parse_hex_color(&self.hover)?,
            hp: parse_hex_color(&self.hp)?,
            sp: parse_hex_color(&self.sp)?,
            gold: parse_hex_color(&self.gold)?,
            slot_size: if self.slot_size > 0.0 { self.slot_size } else { 44.0 },
            corner_radius: self.corner_radius.max(0.0),
            font_size: if self.font_size > 0.0 { self.font_size } else { 13.0 },
        })
    }
}

pub fn parse_theme_file(source: &str) -> Option<ThemeTokens> {
    let trimmed = source.trim_start();
    if trimmed.starts_with("ThemeFile") || trimmed.starts_with("//") || trimmed.starts_with("/*") {
        return ron::from_str::<ThemeFile>(source).ok().and_then(ThemeFile::into_tokens);
    }
    parse_theme_toml(source).and_then(ThemeFile::into_tokens)
}

/// Minimal TOML reader for the flat theme schema (`key = "value"` / `key =
/// 44.0`).
fn parse_theme_toml(source: &str) -> Option<ThemeFile> {
    let mut name = None::<String>;
    let mut strings = std::collections::HashMap::<String, String>::new();
    let mut nums = std::collections::HashMap::<String, f32>::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, rest) = line.split_once('=')?;
        let key = key.trim();
        let rest = rest.trim().trim_end_matches(',').trim();
        if let Some(inner) = rest.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            if key == "name" {
                name = Some(inner.to_owned());
            } else {
                strings.insert(key.to_owned(), inner.to_owned());
            }
        } else if let Ok(value) = rest.parse::<f32>() {
            nums.insert(key.to_owned(), value);
        }
    }
    Some(ThemeFile {
        name: name?,
        bg: strings.remove("bg")?,
        panel: strings.remove("panel")?,
        raised: strings.remove("raised")?,
        border: strings.remove("border")?,
        text: strings.remove("text")?,
        muted: strings.remove("muted")?,
        accent: strings.remove("accent")?,
        accent2: strings.remove("accent2")?,
        hover: strings.remove("hover")?,
        hp: strings.remove("hp")?,
        sp: strings.remove("sp")?,
        gold: strings.remove("gold")?,
        slot_size: nums.get("slot_size").copied().unwrap_or(44.0),
        corner_radius: nums.get("corner_radius").copied().unwrap_or(8.0),
        font_size: nums.get("font_size").copied().unwrap_or(13.0),
    })
}

/// Linux: `$XDG_CONFIG_HOME/korangar/themes` or `~/.config/korangar/themes`.
fn theme_file_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    for ext in ["ron", "toml"] {
        let path = dir.join(format!("{name}.{ext}"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Resolve the first token file for `name`, if any.
pub fn resolve_theme_path(name: &str) -> Option<PathBuf> {
    theme_search_dirs().into_iter().find_map(|dir| theme_file_in_dir(&dir, name))
}

pub fn load_tokens_from_path(path: &Path) -> Option<ThemeTokens> {
    let data = fs::read_to_string(path).ok()?;
    parse_theme_file(&data)
}

pub fn load_theme_tokens(name: &str) -> Option<ThemeTokens> {
    if let Some(path) = resolve_theme_path(name)
        && let Some(tokens) = load_tokens_from_path(&path)
    {
        return Some(tokens);
    }
    ThemeTokens::builtin(name)
}

pub fn theme_file_mtime(name: &str) -> Option<SystemTime> {
    resolve_theme_path(name).and_then(|path| fs::metadata(path).ok()?.modified().ok())
}

/// Theme ids discovered on disk plus the built-in names.
#[allow(dead_code)]
pub fn discover_theme_names() -> Vec<String> {
    let mut names = vec![
        VERDANT_RUNE_THEME_NAME.to_string(),
        EMBER_FORGE_THEME_NAME.to_string(),
        ARCANE_VIOLET_THEME_NAME.to_string(),
        TURORAN_DARK_THEME_NAME.to_string(),
    ];
    for dir in theme_search_dirs() {
        let Ok(entries) = fs::read_dir(dir) else { continue };
        for entry in entries.filter_map(|entry| entry.ok()) {
            let file_name = entry.file_name();
            let Some(stem) = file_name.to_str().and_then(|name| {
                name.strip_suffix(".ron")
                    .or_else(|| name.strip_suffix(".toml"))
                    .map(ToOwned::to_owned)
            }) else {
                continue;
            };
            if !stem.is_empty() && !names.iter().any(|existing| existing == &stem) {
                names.push(stem);
            }
        }
    }
    // Keep older full-widget dumps discoverable too.
    for dir in [IN_GAME_THEMES_PATH, WORLD_THEMES_PATH] {
        let Ok(entries) = fs::read_dir(dir) else { continue };
        for entry in entries.filter_map(|entry| entry.ok()) {
            let file_name = entry.file_name();
            let Some(stem) = file_name.to_str().and_then(|name| name.strip_suffix(".ron").map(ToOwned::to_owned)) else {
                continue;
            };
            if !stem.is_empty() && !names.iter().any(|existing| existing == &stem) {
                names.push(stem);
            }
        }
    }
    names
}

impl WorldTheme {
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        Self {
            overlay: OverlayTheme {
                foreground_color: tokens.text,
                text_offset: ScreenPosition { left: 18.0, top: 10.0 },
                font_size: FontSize(22.0),
            },
            status_bar: StatusBarTheme {
                background_color: tokens.bg.with_alpha_u8(220),
                player_health_color: tokens.hp,
                enemy_health_color: tokens.accent2,
                spell_point_color: tokens.sp,
                activity_point_color: tokens.gold,
                player_bar_width: 96.0,
                enemy_bar_width: 64.0,
                health_height: 9.0,
                enemy_health_height: 6.0,
                spell_point_height: 5.0,
                activity_point_height: 5.0,
                border_size: ScreenSize { width: 3.0, height: 2.0 },
                gap: 2.0,
            },
            indicator: super::IndicatorTheme {
                walking: tokens.accent.with_alpha_u8(180),
            },
            cursor: super::CursorTheme { color: tokens.text },
        }
    }
}

/// Contrast ratio of two opaque colors (WCAG).
#[cfg(test)]
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
fn relative_luminance(color: Color) -> f32 {
    fn lin(channel: f32) -> f32 {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * lin(color.red) + 0.7152 * lin(color.green) + 0.0722 * lin(color.blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_token_files_parse() {
        for (name, tokens) in [
            ("verdant", ThemeTokens::verdant_rune()),
            ("ember", ThemeTokens::ember_forge()),
            ("arcane", ThemeTokens::arcane_violet()),
        ] {
            assert!(!tokens.name.is_empty(), "{name}");
            assert_eq!(tokens.slot_size, 44.0, "{name}");
        }
        let verdant = ThemeTokens::verdant_rune();
        assert_eq!(verdant.border.red_as_u8(), 0x34);
        assert_eq!(verdant.border.green_as_u8(), 0x48);
        assert_eq!(verdant.border.blue_as_u8(), 0x3F);
        assert_ne!(verdant.border, verdant.gold);
        assert_ne!(verdant.border, verdant.hover);
    }

    #[test]
    fn ember_and_arcane_unlisted_tokens_meet_aa_against_panel() {
        let ember = ThemeTokens::ember_forge();
        assert_eq!(ember.border.red_as_u8(), 0x3A);
        assert_eq!(ember.accent.red_as_u8(), 0xD9);
        assert_eq!(ember.hover, ember.gold);
        for (label, color) in [
            ("muted", ember.muted),
            ("hp", ember.hp),
            ("sp", ember.sp),
            ("hover", ember.hover),
            ("text", ember.text),
        ] {
            let ratio = contrast_ratio(color, ember.panel);
            assert!(ratio >= 4.5, "ember {label} contrast {ratio:.2}");
        }

        let arcane = ThemeTokens::arcane_violet();
        assert_eq!(arcane.accent.red_as_u8(), 0x9B);
        assert_eq!(arcane.gold.red_as_u8(), 0xE2);
        assert_eq!(arcane.hover, arcane.gold);
        for (label, color) in [
            ("muted", arcane.muted),
            ("hp", arcane.hp),
            ("sp", arcane.sp),
            ("hover", arcane.hover),
            ("text", arcane.text),
        ] {
            let ratio = contrast_ratio(color, arcane.panel);
            assert!(ratio >= 4.5, "arcane {label} contrast {ratio:.2}");
        }
    }

    #[test]
    fn toml_and_ron_round_trip_the_same_tokens() {
        let ron = r###"ThemeFile(
            name: "Test",
            bg: "#12151C",
            panel: "#1B2029",
            raised: "#252B36",
            border: "#3A4250",
            text: "#E6E1D6",
            muted: "#A39B8C",
            accent: "#D9A441",
            accent2: "#4F8FBF",
            hover: "#D9A441",
            hp: "#E2785C",
            sp: "#5C9ED6",
            gold: "#D9A441",
            slot_size: 44.0,
            corner_radius: 8.0,
            font_size: 13.0,
        )"###;
        let toml = r###"
name = "Test"
bg = "#12151C"
panel = "#1B2029"
raised = "#252B36"
border = "#3A4250"
text = "#E6E1D6"
muted = "#A39B8C"
accent = "#D9A441"
accent2 = "#4F8FBF"
hover = "#D9A441"
hp = "#E2785C"
sp = "#5C9ED6"
gold = "#D9A441"
slot_size = 44.0
corner_radius = 8.0
font_size = 13.0
"###;
        let a = parse_theme_file(ron).expect("ron");
        let b = parse_theme_file(toml).expect("toml");
        assert_eq!(a, b);
    }

    #[test]
    fn user_theme_dir_overrides_bundled() {
        let root = std::env::temp_dir().join(format!("korangar-theme-override-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("verdant_rune.ron"),
            r###"ThemeFile(
                name: "Override",
                bg: "#010101",
                panel: "#020202",
                raised: "#030303",
                border: "#3A4250",
                text: "#E6E1D6",
                muted: "#A39B8C",
                accent: "#D9A441",
                accent2: "#4F8FBF",
                hover: "#D9A441",
                hp: "#E2785C",
                sp: "#5C9ED6",
                gold: "#D9A441",
            )"###,
        )
        .unwrap();
        let previous = std::env::var_os("KORANGAR_THEMES_DIR");
        unsafe { std::env::set_var("KORANGAR_THEMES_DIR", &root) };
        let loaded = load_theme_tokens(VERDANT_RUNE_THEME_NAME).expect("override");
        match previous {
            Some(value) => unsafe { std::env::set_var("KORANGAR_THEMES_DIR", value) },
            None => unsafe { std::env::remove_var("KORANGAR_THEMES_DIR") },
        }
        let _ = fs::remove_dir_all(&root);
        assert_eq!(loaded.name, "Override");
        assert_eq!(loaded.bg.red_as_u8(), 1);
    }

    #[test]
    fn discover_theme_names_includes_bundled() {
        let names = discover_theme_names();
        assert!(
            names
                .iter()
                .any(|name| name == VERDANT_RUNE_THEME_NAME || name == EMBER_FORGE_THEME_NAME)
        );
    }
}
