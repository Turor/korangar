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
    /// `status.hp_bar`: status-panel HP fill. Green unless the theme says
    /// otherwise.
    pub status_hp_bar: Color,
    /// `status.sp_bar`: status-panel SP fill. Defaults to `sp`.
    pub status_sp_bar: Color,
    /// `status.base_exp_bar`: Base EXP fill. Defaults to `accent`.
    pub status_base_exp_bar: Color,
    /// `status.job_exp_bar`: Job EXP fill. Defaults to `accent2`.
    pub status_job_exp_bar: Color,
    /// `window_alpha`: opacity of window and chat backgrounds, 0.0–1.0.
    /// Defaults to 1.0 (opaque).
    #[serde(default = "default_window_alpha")]
    pub window_alpha: f32,
}

/// Status-panel HP fill when a theme file has no `status.hp_bar`.
pub const DEFAULT_STATUS_HP_BAR: &str = "#4CAF50";

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
            status_hp_bar: Color::rgb_u8(62, 168, 96),
            status_sp_bar: Color::rgb_u8(64, 156, 210),
            status_base_exp_bar: Color::rgb_u8(255, 152, 48),
            status_job_exp_bar: Color::rgb_u8(198, 64, 88),
            window_alpha: 1.0,
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
    /// Window and chat background opacity (1.0 = opaque).
    #[serde(default = "default_window_alpha")]
    pub window_alpha: f32,
    /// Optional `status: (...)` block (TOML: `[status]` or `status.key`).
    #[serde(default)]
    pub status: StatusThemeFile,
}

/// Status-panel bar colors. Every key is optional.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct StatusThemeFile {
    pub hp_bar: Option<String>,
    pub sp_bar: Option<String>,
    pub base_exp_bar: Option<String>,
    pub job_exp_bar: Option<String>,
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
fn default_window_alpha() -> f32 {
    1.0
}

impl ThemeTokens {
    /// `window_alpha` as a color alpha byte.
    pub fn window_alpha_u8(&self) -> u8 {
        (self.window_alpha.clamp(0.0, 1.0) * 255.0).round() as u8
    }
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
        let sp = parse_hex_color(&self.sp)?;
        let accent = parse_hex_color(&self.accent)?;
        let accent2 = parse_hex_color(&self.accent2)?;
        let optional = |value: &Option<String>, fallback: Color| -> Option<Color> {
            match value {
                Some(value) => parse_hex_color(value),
                None => Some(fallback),
            }
        };
        let default_hp = parse_hex_color(DEFAULT_STATUS_HP_BAR)?;
        let status_hp_bar = optional(&self.status.hp_bar, default_hp)?;
        let status_sp_bar = optional(&self.status.sp_bar, sp)?;
        let status_base_exp_bar = optional(&self.status.base_exp_bar, accent)?;
        let status_job_exp_bar = optional(&self.status.job_exp_bar, accent2)?;
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
            status_hp_bar,
            status_sp_bar,
            status_base_exp_bar,
            status_job_exp_bar,
            window_alpha: self.window_alpha.clamp(0.0, 1.0),
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
    let mut section = String::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            section = header.trim().to_owned();
            continue;
        }
        let (key, rest) = line.split_once('=')?;
        let key = key.trim();
        let qualified;
        let key = if section.is_empty() {
            key
        } else {
            qualified = format!("{section}.{key}");
            qualified.as_str()
        };
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
        window_alpha: nums.get("window_alpha").copied().unwrap_or(1.0),
        status: StatusThemeFile {
            hp_bar: strings.remove("status.hp_bar"),
            sp_bar: strings.remove("status.sp_bar"),
            base_exp_bar: strings.remove("status.base_exp_bar"),
            job_exp_bar: strings.remove("status.job_exp_bar"),
        },
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
    resolve_theme_path_in(&theme_search_dirs(), name)
}

/// Resolve the first token file for `name` in `dirs`, searched in order.
fn resolve_theme_path_in(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    dirs.iter().find_map(|dir| theme_file_in_dir(dir, name))
}

pub fn load_tokens_from_path(path: &Path) -> Option<ThemeTokens> {
    let data = fs::read_to_string(path).ok()?;
    parse_theme_file(&data)
}

pub fn load_theme_tokens(name: &str) -> Option<ThemeTokens> {
    load_theme_tokens_in(&theme_search_dirs(), name)
}

/// Like [`load_theme_tokens`] with an explicit search path. Tests use this
/// instead of setting `KORANGAR_THEMES_DIR`, which is process-global and
/// leaks into tests that run in parallel.
fn load_theme_tokens_in(dirs: &[PathBuf], name: &str) -> Option<ThemeTokens> {
    if let Some(path) = resolve_theme_path_in(dirs, name)
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
                // Same green as the status panel (`status.hp_bar`).
                player_health_color: tokens.status_hp_bar,
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
        // Do not set KORANGAR_THEMES_DIR here: env vars are process-global and
        // made `load_falls_back_to_built_in_verdant_rune` load this override
        // when the tests ran in parallel.
        let mut dirs = vec![root.clone()];
        dirs.extend(theme_search_dirs().into_iter().skip(1));
        let loaded = load_theme_tokens_in(&dirs, VERDANT_RUNE_THEME_NAME).expect("override");
        let _ = fs::remove_dir_all(&root);
        assert_eq!(loaded.name, "Override");
        assert_eq!(loaded.bg.red_as_u8(), 1);
    }

    #[test]
    fn status_bar_keys_parse_and_default() {
        for tokens in [
            ThemeTokens::verdant_rune(),
            ThemeTokens::ember_forge(),
            ThemeTokens::arcane_violet(),
        ] {
            let name = tokens.name.clone();
            assert_ne!(tokens.status_base_exp_bar, tokens.status_job_exp_bar, "{name}");
            assert_ne!(tokens.status_base_exp_bar, tokens.status_hp_bar, "{name}");
            assert_ne!(tokens.status_job_exp_bar, tokens.status_hp_bar, "{name}");
            assert_ne!(tokens.status_base_exp_bar, tokens.status_sp_bar, "{name}");
            assert_ne!(tokens.status_job_exp_bar, tokens.status_sp_bar, "{name}");
            let hp = tokens.status_hp_bar;
            assert!(hp.green > hp.red && hp.green > hp.blue, "{name} HP bar must be green");
            assert_eq!(tokens.status_sp_bar, tokens.sp, "{name} SP stays the theme blue");
        }

        // No status block: green HP, SP = sp, Base = accent, Job = accent2.
        let bare = parse_theme_file(
            r###"ThemeFile(
                name: "Bare",
                bg: "#010101", panel: "#020202", raised: "#030303", border: "#3A4250",
                text: "#E6E1D6", muted: "#A39B8C", accent: "#D9A441", accent2: "#4F8FBF",
                hover: "#D9A441", hp: "#E2785C", sp: "#5C9ED6", gold: "#D9A441",
            )"###,
        )
        .expect("bare");
        assert_eq!(bare.status_hp_bar, parse_hex_color(DEFAULT_STATUS_HP_BAR).unwrap());
        assert_eq!(bare.status_sp_bar, bare.sp);
        assert_eq!(bare.status_base_exp_bar, bare.accent);
        assert_eq!(bare.status_job_exp_bar, bare.accent2);

        let toml = r###"
name = "Sectioned"
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

[status]
hp_bar = "#00FF00"
base_exp_bar = "#112233"
job_exp_bar = "#445566"
"###;
        let sectioned = parse_theme_file(toml).expect("toml status section");
        assert_eq!(sectioned.status_hp_bar, parse_hex_color("#00FF00").unwrap());
        assert_eq!(sectioned.status_base_exp_bar, parse_hex_color("#112233").unwrap());
        assert_eq!(sectioned.status_job_exp_bar, parse_hex_color("#445566").unwrap());
        assert_eq!(sectioned.status_sp_bar, sectioned.sp);

        let dotted = toml
            .replace("[status]\nhp_bar", "status.hp_bar")
            .replace("\nbase_exp_bar", "\nstatus.base_exp_bar");
        let dotted = parse_theme_file(&dotted).expect("dotted keys");
        assert_eq!(dotted.status_hp_bar, parse_hex_color("#00FF00").unwrap());
    }

    #[test]
    fn bundled_themes_have_opaque_windows() {
        for tokens in [
            ThemeTokens::verdant_rune(),
            ThemeTokens::ember_forge(),
            ThemeTokens::arcane_violet(),
        ] {
            assert_eq!(tokens.window_alpha, 1.0, "{}", tokens.name);
            assert_eq!(tokens.window_alpha_u8(), 255);
        }
        let toml = "name = \"T\"\nbg = \"#000000\"\npanel = \"#000000\"\nraised = \"#000000\"\nborder = \"#000000\"\ntext = \
                    \"#FFFFFF\"\nmuted = \"#888888\"\naccent = \"#FF0000\"\naccent2 = \"#00FF00\"\nhover = \"#FFFF00\"\nhp = \
                    \"#FF0000\"\nsp = \"#0000FF\"\ngold = \"#FFFF00\"\nwindow_alpha = 0.5\n";
        let half = parse_theme_file(toml).expect("toml");
        assert_eq!(half.window_alpha_u8(), 128);
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
