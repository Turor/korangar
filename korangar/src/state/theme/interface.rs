#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, Timer, print_debug};
use korangar_interface::components::button::ButtonTheme;
use korangar_interface::components::collapsible::CollapsibleTheme;
use korangar_interface::components::drop_down::DropDownTheme;
use korangar_interface::components::field::FieldTheme;
use korangar_interface::components::state_button::StateButtonTheme;
use korangar_interface::components::text::TextTheme;
use korangar_interface::components::text_box::TextBoxTheme;
use korangar_interface::element::StateElement;
use korangar_interface::layout::tooltip::TooltipTheme;
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{StateWindow, WindowTheme};
use korangar_networking::MessageColor;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

use crate::graphics::{Color, CornerDiameter, ScreenSize, ShadowPadding};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::settings::{IN_GAME_THEMES_PATH, MENU_THEMES_PATH, TURORAN_DARK_THEME_NAME, VERDANT_RUNE_THEME_NAME};
use crate::state::ClientState;

fn default_slot_border_color() -> Color {
    Color::rgb_u8(52, 72, 63)
}
fn default_slot_hovered_border_color() -> Color {
    Color::rgb_hex("CFAE5A")
}
fn default_slot_selected_border_color() -> Color {
    Color::rgb_hex("4CC3A0")
}

#[derive(Default, Debug, Clone, Copy)]
pub enum InterfaceThemeType {
    #[default]
    InGame,
    Menu,
}

#[derive(Serialize, Deserialize, RustState, StateElement)]
pub struct DebugButtonTheme {
    foreground_color: Color,
    hovered_background_color: Color,
}

#[derive(Serialize, Deserialize, RustState, StateElement)]
pub struct ChatTheme {
    window_color: Color,
    text_box_background_color: Color,
    /// Default color for `MessageColor::Broadcast`.
    broadcast_color: Color,
    /// Default color for `MessageColor::Server`.
    server_color: Color,
    /// Default color for `MessageColor::Error`.
    error_color: Color,
    /// Default color for `MessageColor::Information`.
    information_color: Color,
    /// Nearby public speech.
    public_color: Color,
    /// Whisper lines.
    whisper_color: Color,
    /// Party chat. roBrowser uses rgb(230, 215, 200) for other members.
    #[serde(default = "default_party_chat_color")]
    party_color: Color,
    /// Guild chat.
    guild_color: Color,
    /// Overhead speech bubble text.
    bubble_color: Color,
    /// Overhead speech bubble backing.
    bubble_background_color: Color,
    /// Announce banner backing.
    announce_background_color: Color,
}

impl ChatTheme {
    pub(crate) fn message_color(&self, color: MessageColor) -> Color {
        match color {
            MessageColor::Rgb { red, green, blue } => Color::rgb_u8(red, green, blue),
            MessageColor::Broadcast => self.broadcast_color,
            MessageColor::Server => self.server_color,
            MessageColor::Error => self.error_color,
            MessageColor::Information => self.information_color,
            MessageColor::Public => self.public_color,
            MessageColor::Whisper => self.whisper_color,
            MessageColor::Party => self.party_color,
            MessageColor::Guild => self.guild_color,
        }
    }

    pub(crate) fn broadcast_color(&self) -> Color {
        self.broadcast_color
    }

    pub(crate) fn bubble_color(&self) -> Color {
        self.bubble_color
    }

    pub(crate) fn bubble_background_color(&self) -> Color {
        self.bubble_background_color
    }

    pub(crate) fn announce_background_color(&self) -> Color {
        self.announce_background_color
    }
}

fn default_party_chat_color() -> Color {
    Color::rgb_u8(230, 215, 200)
}

#[derive(Serialize, Deserialize, RustState, StateElement)]
pub struct SkillTreeTheme {
    /// Background color for an occupied slot.
    slot_background_color: Color,
    slot_outline: ShadowPadding,
    /// Color used for showing level or skill requirements, both outlines and
    /// text.
    requirement_color: Color,
    /// Outline color of a hovered slot.
    hover_color: Color,
    /// Corner diameter of the occupied and unoccupied slots.
    slot_corner_diameter: CornerDiameter,
    unoccupied_slot_color: Color,
    unoccupied_slot_outline: ShadowPadding,
    unlearned_skill_color: Color,
    name_font_size: FontSize,
    name_text_color: Color,
    highlight_color: Color,
    points_color: Color,
    /// Color used for coloring pending skll points.
    pending_points_color: Color,
    points_font_size: FontSize,
    arrow_color: Color,
    /// Color used when the selected skill level is lower than the learned skill
    /// level.
    lower_level_color: Color,
    /// Default socket frame for equipment, inventory, and hotbar slots.
    #[serde(default = "default_slot_border_color")]
    slot_border_color: Color,
    /// Socket frame when the pointer is over the slot (gold / amber hover).
    #[serde(default = "default_slot_hovered_border_color")]
    slot_hovered_border_color: Color,
    /// Socket frame for a selected or dragged slot.
    #[serde(default = "default_slot_selected_border_color")]
    slot_selected_border_color: Color,
}

#[derive(Serialize, Deserialize, RustState, StateElement)]
pub struct GlobalTheme {
    /// Color indicating that a grabbed resource can be dropped in the
    /// highlighted area.
    drop_area_color: Color,
    /// Color indicating that a grabbed resource is inside the highlighted area
    /// and can be dropped there.
    hovered_drop_area_color: Color,
    /// Alpha to multiply the area color by to get the fill color of the area.
    /// The original color will be used for the outline only.
    fill_alpha: f32,
    /// Outline for the highlighted area.
    drop_area_outline: ShadowPadding,
}

/// Colors for the basic-info HUD, inventory footer, and experience bars.
#[derive(Serialize, Deserialize, RustState, StateElement, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct BasicInfoTheme {
    pub base_exp_color: Color,
    pub job_exp_color: Color,
    pub exp_track_color: Color,
    pub zeny_color: Color,
    pub weight_color: Color,
    pub weight_caution_color: Color,
    pub weight_over_color: Color,
}

impl Default for BasicInfoTheme {
    fn default() -> Self {
        Self::from_tokens(&crate::state::theme::ThemeTokens::turoran_dark())
    }
}

impl BasicInfoTheme {
    pub fn from_tokens(tokens: &crate::state::theme::ThemeTokens) -> Self {
        Self {
            base_exp_color: tokens.sp,
            job_exp_color: tokens.gold,
            exp_track_color: tokens.raised.with_alpha_u8(230),
            zeny_color: tokens.gold,
            weight_color: tokens.text,
            weight_caution_color: tokens.hover,
            weight_over_color: tokens.accent2,
        }
    }
}

#[derive(Serialize, Deserialize, RustState, StateElement, StateWindow)]
#[window_title("Theme Inspector")]
pub struct InterfaceTheme {
    /// Named palette shared by HUD, Gear, inventory, minimap, tooltips, and the
    /// world overlay.
    #[serde(default)]
    pub tokens: crate::state::theme::ThemeTokens,
    #[hidden_element]
    pub window: WindowTheme<ClientState>,
    #[hidden_element]
    pub text: TextTheme<ClientState>,
    #[hidden_element]
    pub button: ButtonTheme<ClientState>,
    #[hidden_element]
    pub state_button: StateButtonTheme<ClientState>,
    #[hidden_element]
    pub text_box: TextBoxTheme<ClientState>,
    #[hidden_element]
    pub collapsible: CollapsibleTheme<ClientState>,
    #[hidden_element]
    pub drop_down: DropDownTheme<ClientState>,
    #[hidden_element]
    pub field: FieldTheme<ClientState>,
    #[hidden_element]
    pub tooltip: TooltipTheme<ClientState>,
    pub debug_button: DebugButtonTheme,
    pub chat: ChatTheme,
    pub skill_tree: SkillTreeTheme,
    pub global: GlobalTheme,
    /// Experience bars, zeny, and weight. Missing from older theme files.
    #[serde(default)]
    pub basic_info: BasicInfoTheme,
}

impl InterfaceTheme {
    /// Build the widget theme from a token file. Slot borders use `border`,
    /// never `gold` / `hover`.
    pub fn from_tokens(tokens: &crate::state::theme::ThemeTokens) -> Self {
        use crate::state::theme::ThemeTokens;
        let ThemeTokens {
            bg,
            panel,
            raised,
            border,
            text,
            muted,
            accent,
            accent2,
            hover,
            gold,
            sp,
            corner_radius,
            font_size,
            ..
        } = *tokens;
        let radius = CornerDiameter::uniform(corner_radius);
        let window_radius = CornerDiameter::uniform((corner_radius + 2.0).max(corner_radius));
        let body_font = FontSize(font_size);
        let title_font = FontSize(font_size + 1.0);
        let small_font = FontSize((font_size - 1.0).max(11.0));
        let tooltip_font = FontSize((font_size - 1.0).max(12.0));

        Self {
            tokens: tokens.clone(),
            window: WindowTheme {
                title_color: text,
                hovered_title_color: accent,
                background_color: panel.with_alpha_u8(215),
                highlight_color: accent,
                shadow_color: Color::rgba_u8(0, 0, 0, 120),
                shadow_padding: ShadowPadding::diagonal(4.0, 8.0),
                gaps: 6.0,
                border: 8.0,
                corner_diameter: window_radius,
                close_button_size: ScreenSize { width: 36.0, height: 18.0 },
                close_button_corner_diameter: radius,
                minimum_width: 280.0,
                maximum_width: 620.0,
                minimum_height: 36.0,
                maximum_height: 720.0,
                title_height: 22.0,
                title_gap: 2.0,
                font_size: title_font,
                horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 4.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::Shrink,
                anchor_color: border,
                closest_anchor_color: hover,
            },
            text: TextTheme {
                color: text,
                highlight_color: accent,
                height: font_size + 1.0,
                font_size: body_font,
                horizontal_alignment: HorizontalAlignment::Left { offset: 3.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            button: ButtonTheme {
                background_color: raised.with_alpha_u8(230),
                foreground_color: text,
                highlight_color: accent,
                hovered_background_color: border.with_alpha_u8(240),
                hovered_foreground_color: Color::rgb_u8(255, 255, 255),
                disabled_background_color: bg.with_alpha_u8(200),
                disabled_foreground_color: muted,
                shadow_color: Color::rgba_u8(0, 0, 0, 110),
                shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                height: 20.0,
                corner_diameter: radius,
                font_size: body_font,
                horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 4.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            state_button: StateButtonTheme {
                background_color: raised.with_alpha_u8(230),
                foreground_color: text,
                highlight_color: accent,
                hovered_background_color: border.with_alpha_u8(240),
                hovered_foreground_color: Color::rgb_u8(255, 255, 255),
                disabled_background_color: bg.with_alpha_u8(200),
                disabled_foreground_color: muted,
                checkbox_color: accent,
                hovered_checkbox_color: accent,
                disabled_checkbox_color: muted,
                shadow_color: Color::rgba_u8(0, 0, 0, 110),
                shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                height: 20.0,
                corner_diameter: radius,
                font_size: body_font,
                horizontal_alignment: HorizontalAlignment::Left { offset: 28.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            text_box: TextBoxTheme {
                background_color: bg.with_alpha_u8(230),
                foreground_color: text,
                highlight_color: accent,
                hovered_background_color: panel.with_alpha_u8(240),
                hovered_foreground_color: Color::rgb_u8(255, 255, 255),
                focused_background_color: raised.with_alpha_u8(245),
                focused_foreground_color: Color::rgb_u8(255, 255, 255),
                ghost_foreground_color: muted,
                hide_icon_color: muted,
                hovered_hide_icon_color: accent2,
                shadow_color: Color::rgba_u8(0, 0, 0, 110),
                shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                height: 20.0,
                corner_diameter: radius,
                font_size: body_font,
                horizontal_alignment: HorizontalAlignment::Left { offset: 12.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            collapsible: CollapsibleTheme {
                background_color: raised.with_alpha_u8(230),
                secondary_background_color: panel.with_alpha_u8(230),
                foreground_color: text,
                highlight_color: accent,
                hovered_foreground_color: accent,
                corner_diameter: radius,
                icon_color: muted,
                icon_size: 10.0,
                shadow_color: Color::rgba_u8(0, 0, 0, 110),
                shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                gaps: 4.0,
                border: 5.0,
                title_height: 20.0,
                font_size: body_font,
                horizontal_alignment: HorizontalAlignment::Left { offset: 0.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            drop_down: DropDownTheme {
                item_background_color: raised.with_alpha_u8(235),
                item_foreground_color: text,
                item_highlight_color: accent,
                item_hovered_background_color: border.with_alpha_u8(245),
                item_hovered_foreground_color: Color::rgb_u8(255, 255, 255),
                item_shadow_color: Color::rgba_u8(0, 0, 0, 110),
                item_shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                item_height: 20.0,
                item_corner_diameter: radius,
                item_font_size: body_font,
                item_horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 4.0 },
                item_vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                item_overflow_behavior: OverflowBehavior::Shrink,
                list_corner_diameter: radius,
                list_background_color: bg.with_alpha_u8(235),
                list_shadow_color: Color::rgba_u8(0, 0, 0, 120),
                list_shadow_padding: ShadowPadding::diagonal(4.0, 8.0),
                list_gaps: 3.0,
                list_border: 4.0,
                list_maximum_height: 500.0,
                button_background_color: raised.with_alpha_u8(230),
                button_foreground_color: text,
                button_highlight_color: accent,
                button_hovered_background_color: border.with_alpha_u8(240),
                button_hovered_foreground_color: Color::rgb_u8(255, 255, 255),
                button_shadow_color: Color::rgba_u8(0, 0, 0, 110),
                button_shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                button_height: 20.0,
                button_corner_diameter: radius,
                button_font_size: body_font,
                button_horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 4.0 },
                button_vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                button_overflow_behavior: OverflowBehavior::Shrink,
            },
            field: FieldTheme {
                background_color: raised.with_alpha_u8(230),
                foreground_color: text,
                highlight_color: accent,
                shadow_color: Color::rgba_u8(0, 0, 0, 110),
                shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                height: 20.0,
                corner_diameter: radius,
                font_size: body_font,
                horizontal_alignment: HorizontalAlignment::Left { offset: 5.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            tooltip: TooltipTheme {
                background_color: bg.with_alpha_u8(230),
                foreground_color: text,
                highlight_color: gold,
                shadow_color: Color::rgba_u8(0, 0, 0, 120),
                shadow_padding: ShadowPadding::diagonal(3.0, 6.0),
                font_size: tooltip_font,
                corner_diameter: radius,
                border: 4.0,
                gap: 3.0,
                mouse_offset: 14.0,
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            debug_button: DebugButtonTheme {
                foreground_color: accent2,
                hovered_background_color: gold,
            },
            chat: ChatTheme {
                window_color: panel.with_alpha_u8(205),
                text_box_background_color: bg.with_alpha_u8(190),
                broadcast_color: gold,
                server_color: sp,
                error_color: accent2,
                information_color: accent,
                public_color: text,
                whisper_color: sp,
                party_color: Color::rgb_u8(230, 215, 200),
                guild_color: accent,
                bubble_color: text,
                bubble_background_color: panel.with_alpha_u8(180),
                announce_background_color: bg.with_alpha_u8(170),
            },
            skill_tree: SkillTreeTheme {
                slot_background_color: raised.with_alpha_u8(230),
                slot_outline: ShadowPadding::uniform(2.0),
                requirement_color: accent2,
                hover_color: Color::WHITE,
                slot_corner_diameter: CornerDiameter::uniform((corner_radius + 4.0).max(4.0)),
                unoccupied_slot_color: bg.with_alpha_u8(170),
                unoccupied_slot_outline: ShadowPadding::uniform(2.0),
                unlearned_skill_color: muted,
                name_font_size: small_font,
                name_text_color: text,
                highlight_color: gold,
                points_color: text,
                pending_points_color: accent,
                points_font_size: small_font,
                arrow_color: muted,
                lower_level_color: accent2,
                slot_border_color: border,
                slot_hovered_border_color: hover,
                slot_selected_border_color: accent,
            },
            global: GlobalTheme {
                drop_area_color: gold.with_alpha_u8(110),
                hovered_drop_area_color: accent.with_alpha_u8(130),
                fill_alpha: 0.12,
                drop_area_outline: ShadowPadding::uniform(3.0),
            },
            basic_info: BasicInfoTheme::from_tokens(tokens),
        }
    }

    pub fn load(theme_type: InterfaceThemeType, name: &str) -> Self {
        #[cfg(feature = "debug")]
        let timer = Timer::new("Load theme");

        let path = match theme_type {
            InterfaceThemeType::InGame => format!("{IN_GAME_THEMES_PATH}/{name}.ron"),
            InterfaceThemeType::Menu => format!("{MENU_THEMES_PATH}/{name}.ron"),
        };

        #[cfg(feature = "debug")]
        print_debug!("loading theme from file {}", path.magenta());

        let theme = crate::state::theme::load_theme_tokens(name)
            .map(|tokens| Self::from_tokens(&tokens))
            .or_else(|| std::fs::read_to_string(&path).ok().and_then(|data| ron::from_str(&data).ok()))
            .unwrap_or_else(|| {
                #[cfg(feature = "debug")]
                print_debug!("[{}] failed to load theme {}", "error".red(), name.magenta());

                match (theme_type, name) {
                    (InterfaceThemeType::InGame, TURORAN_DARK_THEME_NAME) => Self::turoran_dark(),
                    (InterfaceThemeType::InGame, VERDANT_RUNE_THEME_NAME) => Self::verdant_rune(),
                    (InterfaceThemeType::InGame, crate::settings::EMBER_FORGE_THEME_NAME) => Self::ember_forge(),
                    (InterfaceThemeType::InGame, crate::settings::ARCANE_VIOLET_THEME_NAME) => Self::arcane_violet(),
                    (InterfaceThemeType::Menu, _) => Self::default_menu(),
                    (InterfaceThemeType::InGame, _) => Self::default_in_game(),
                }
            });

        #[cfg(feature = "debug")]
        timer.stop();

        theme
    }

    #[cfg(feature = "debug")]
    pub fn save(&self, theme_type: InterfaceThemeType, name: &str) {
        let timer = Timer::new("Save theme");

        let path = match theme_type {
            InterfaceThemeType::InGame => format!("{IN_GAME_THEMES_PATH}/{name}.ron"),
            InterfaceThemeType::Menu => format!("{MENU_THEMES_PATH}/{name}.ron"),
        };

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

    /// Original Turoran dark in-game HUD theme.
    ///
    /// Charcoal translucent panels, orange accents, and compact chrome intended
    /// for existing chat/hotbar/inventory/equipment/stats windows. This is an
    /// original Turoran palette inspired by modern dark HUD conventions, not a
    /// copy of any third-party artwork, logo, or proprietary theme assets.
    pub fn turoran_dark() -> Self {
        Self::from_tokens(&crate::state::theme::ThemeTokens::turoran_dark())
    }

    pub fn verdant_rune() -> Self {
        Self::from_tokens(&crate::state::theme::ThemeTokens::verdant_rune())
    }

    pub fn ember_forge() -> Self {
        Self::from_tokens(&crate::state::theme::ThemeTokens::ember_forge())
    }

    pub fn arcane_violet() -> Self {
        Self::from_tokens(&crate::state::theme::ThemeTokens::arcane_violet())
    }

    fn default_menu() -> Self {
        Self {
            tokens: crate::state::theme::ThemeTokens::default(),
            window: WindowTheme {
                title_color: Color::rgb_u8(200, 150, 150),
                hovered_title_color: Color::rgb_u8(250, 200, 200),
                background_color: Color::monochrome_u8(30),
                highlight_color: Color::rgb_u8(255, 160, 60),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(5.0, 10.0),
                gaps: 25.0,
                border: 30.0,
                corner_diameter: CornerDiameter::uniform(50.0),
                close_button_size: ScreenSize { width: 45.0, height: 35.0 },
                close_button_corner_diameter: CornerDiameter::uniform(25.0),
                minimum_width: 300.0,
                maximum_width: 600.0,
                minimum_height: 50.0,
                maximum_height: 700.0,
                title_height: 45.0,
                title_gap: 20.0,
                font_size: FontSize(20.0),
                horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 5.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::Shrink,
                anchor_color: Color::rgb_u8(130, 105, 160),
                closest_anchor_color: Color::rgb_u8(255, 175, 30),
            },
            text: TextTheme {
                color: Color::monochrome_u8(220),
                highlight_color: Color::rgb_u8(255, 160, 60),
                height: 15.0,
                font_size: FontSize(16.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 6.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            button: ButtonTheme {
                background_color: Color::monochrome_u8(80),
                foreground_color: Color::monochrome_u8(180),
                highlight_color: Color::rgb_u8(255, 160, 60),
                hovered_background_color: Color::monochrome_u8(120),
                hovered_foreground_color: Color::monochrome_u8(220),
                disabled_background_color: Color::monochrome_u8(50),
                disabled_foreground_color: Color::monochrome_u8(100),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                height: 30.0,
                corner_diameter: CornerDiameter::uniform(30.0),
                font_size: FontSize(16.0),
                horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 5.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            state_button: StateButtonTheme {
                background_color: Color::monochrome_u8(80),
                foreground_color: Color::monochrome_u8(180),
                highlight_color: Color::rgb_u8(255, 160, 60),
                hovered_background_color: Color::monochrome_u8(120),
                hovered_foreground_color: Color::monochrome_u8(220),
                disabled_background_color: Color::monochrome_u8(50),
                disabled_foreground_color: Color::monochrome_u8(100),
                checkbox_color: Color::rgb_u8(255, 100, 100),
                hovered_checkbox_color: Color::rgb_u8(255, 140, 140),
                disabled_checkbox_color: Color::monochrome_u8(180),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                height: 24.0,
                corner_diameter: CornerDiameter::uniform(24.0),
                font_size: FontSize(16.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 50.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            text_box: TextBoxTheme {
                background_color: Color::monochrome_u8(45),
                foreground_color: Color::monochrome_u8(180),
                highlight_color: Color::rgb_u8(255, 160, 60),
                hovered_background_color: Color::monochrome_u8(75),
                hovered_foreground_color: Color::monochrome_u8(220),
                focused_background_color: Color::monochrome_u8(120),
                focused_foreground_color: Color::monochrome_u8(255),
                ghost_foreground_color: Color::monochrome_u8(100),
                hide_icon_color: Color::rgb_u8(200, 180, 180),
                hovered_hide_icon_color: Color::rgb_u8(250, 200, 200),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                height: 30.0,
                corner_diameter: CornerDiameter::uniform(30.0),
                font_size: FontSize(16.0),
                horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 5.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            collapsible: CollapsibleTheme {
                background_color: Color::monochrome_u8(45),
                secondary_background_color: Color::monochrome_u8(30),
                foreground_color: Color::monochrome_u8(200),
                highlight_color: Color::rgb_u8(255, 160, 60),
                hovered_foreground_color: Color::rgb_u8(250, 200, 200),
                corner_diameter: CornerDiameter::uniform(20.0),
                icon_color: Color::monochrome_u8(170),
                icon_size: 15.0,
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                gaps: 5.0,
                border: 10.0,
                title_height: 30.0,
                font_size: FontSize(16.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 0.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            drop_down: DropDownTheme {
                item_background_color: Color::monochrome_u8(65),
                item_foreground_color: Color::monochrome_u8(180),
                item_highlight_color: Color::rgb_u8(255, 160, 60),
                item_hovered_background_color: Color::monochrome_u8(105),
                item_hovered_foreground_color: Color::monochrome_u8(220),
                item_shadow_color: Color::rgba_u8(0, 0, 0, 100),
                item_shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                item_height: 30.0,
                item_corner_diameter: CornerDiameter::uniform(30.0),
                item_font_size: FontSize(16.0),
                item_horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 5.0 },
                item_vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                item_overflow_behavior: OverflowBehavior::Shrink,
                list_corner_diameter: CornerDiameter::uniform(30.0),
                list_background_color: Color::monochrome_u8(40),
                list_shadow_color: Color::rgba_u8(0, 0, 0, 100),
                list_shadow_padding: ShadowPadding::diagonal(5.0, 10.0),
                list_gaps: 8.0,
                list_border: 5.0,
                list_maximum_height: 700.0,
                button_background_color: Color::monochrome_u8(80),
                button_foreground_color: Color::monochrome_u8(180),
                button_highlight_color: Color::rgb_u8(255, 160, 60),
                button_hovered_background_color: Color::monochrome_u8(120),
                button_hovered_foreground_color: Color::monochrome_u8(220),
                button_shadow_color: Color::rgba_u8(0, 0, 0, 100),
                button_shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                button_height: 30.0,
                button_corner_diameter: CornerDiameter::uniform(30.0),
                button_font_size: FontSize(16.0),
                button_horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 5.0 },
                button_vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                button_overflow_behavior: OverflowBehavior::Shrink,
            },
            field: FieldTheme {
                background_color: Color::monochrome_u8(80),
                foreground_color: Color::monochrome_u8(180),
                highlight_color: Color::rgb_u8(255, 160, 60),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                height: 30.0,
                corner_diameter: CornerDiameter::uniform(30.0),
                font_size: FontSize(16.0),
                horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 5.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            tooltip: TooltipTheme {
                background_color: Color::rgba_u8(15, 15, 15, 200),
                foreground_color: Color::monochrome_u8(235),
                highlight_color: Color::rgb_u8(255, 160, 60),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                font_size: FontSize(16.0),
                corner_diameter: CornerDiameter::uniform(8.0),
                border: 8.0,
                gap: 4.0,
                mouse_offset: 20.0,
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            debug_button: DebugButtonTheme {
                foreground_color: Color::rgb_u8(255, 167, 89),
                hovered_background_color: Color::rgb_u8(225, 199, 115),
            },
            chat: ChatTheme {
                window_color: Color::TRANSPARENT,
                text_box_background_color: Color::TRANSPARENT,
                broadcast_color: Color::rgb_u8(220, 200, 255),
                server_color: Color::rgb_u8(180, 220, 255),
                error_color: Color::rgb_u8(255, 120, 120),
                information_color: Color::rgb_u8(200, 230, 180),
                public_color: Color::rgb_u8(235, 236, 240),
                whisper_color: Color::rgb_u8(181, 222, 239),
                party_color: Color::rgb_u8(230, 215, 200),
                guild_color: Color::rgb_u8(170, 220, 140),
                bubble_color: Color::rgb_u8(235, 236, 240),
                bubble_background_color: Color::rgba_u8(0, 0, 0, 150),
                announce_background_color: Color::rgba_u8(0, 0, 0, 150),
            },
            skill_tree: SkillTreeTheme {
                slot_background_color: Color::monochrome_u8(40),
                slot_outline: ShadowPadding::uniform(3.0),
                requirement_color: Color::rgba_u8(200, 100, 200, 255),
                hover_color: Color::WHITE,
                slot_corner_diameter: CornerDiameter::uniform(20.0),
                unoccupied_slot_color: Color::rgba_u8(30, 30, 30, 160),
                unoccupied_slot_outline: ShadowPadding::uniform(2.0),
                unlearned_skill_color: Color::monochrome_u8(80),
                name_font_size: FontSize(12.0),
                name_text_color: Color::WHITE,
                highlight_color: Color::rgb_u8(255, 160, 60),
                points_color: Color::WHITE,
                pending_points_color: Color::rgb_u8(0, 255, 220),
                points_font_size: FontSize(12.0),
                arrow_color: Color::monochrome_u8(160),
                lower_level_color: Color::rgb_u8(255, 200, 255),
                slot_border_color: Color::rgb_u8(58, 66, 82),
                slot_hovered_border_color: Color::rgb_u8(255, 170, 60),
                slot_selected_border_color: Color::rgb_u8(80, 220, 200),
            },
            global: GlobalTheme {
                drop_area_color: Color::rgba_u8(255, 60, 200, 120),
                hovered_drop_area_color: Color::rgba_u8(0, 255, 200, 130),
                fill_alpha: 0.1,
                drop_area_outline: ShadowPadding::uniform(4.0),
            },
            basic_info: BasicInfoTheme::default(),
        }
    }

    fn default_in_game() -> Self {
        Self {
            tokens: crate::state::theme::ThemeTokens::default(),
            window: WindowTheme {
                title_color: Color::rgb_u8(185, 155, 155),
                hovered_title_color: Color::rgb_u8(240, 155, 155),
                background_color: Color::monochrome_u8(50),
                highlight_color: Color::rgb_u8(255, 160, 60),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(5.0, 10.0),
                gaps: 8.0,
                border: 10.0,
                corner_diameter: CornerDiameter::uniform(14.0),
                close_button_size: ScreenSize { width: 40.0, height: 18.0 },
                close_button_corner_diameter: CornerDiameter::uniform(12.0),
                minimum_width: 300.0,
                maximum_width: 600.0,
                minimum_height: 40.0,
                maximum_height: 700.0,
                title_height: 25.0,
                title_gap: 2.0,
                font_size: FontSize(15.0),
                horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 5.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::Shrink,
                anchor_color: Color::rgb_u8(140, 105, 130),
                closest_anchor_color: Color::rgb_u8(255, 175, 30),
            },
            text: TextTheme {
                color: Color::monochrome_u8(220),
                highlight_color: Color::rgb_u8(255, 160, 60),
                height: 14.0,
                font_size: FontSize(14.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 3.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            button: ButtonTheme {
                background_color: Color::monochrome_u8(120),
                foreground_color: Color::monochrome_u8(220),
                highlight_color: Color::rgb_u8(255, 160, 60),
                hovered_background_color: Color::monochrome_u8(150),
                hovered_foreground_color: Color::monochrome_u8(250),
                disabled_background_color: Color::monochrome_u8(100),
                disabled_foreground_color: Color::monochrome_u8(160),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                height: 20.0,
                corner_diameter: CornerDiameter::uniform(10.0),
                font_size: FontSize(14.0),
                horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 5.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            state_button: StateButtonTheme {
                background_color: Color::monochrome_u8(120),
                foreground_color: Color::monochrome_u8(220),
                highlight_color: Color::rgb_u8(255, 160, 60),
                hovered_background_color: Color::monochrome_u8(150),
                hovered_foreground_color: Color::monochrome_u8(250),
                disabled_background_color: Color::monochrome_u8(80),
                disabled_foreground_color: Color::monochrome_u8(120),
                checkbox_color: Color::monochrome_u8(200),
                hovered_checkbox_color: Color::monochrome_u8(250),
                disabled_checkbox_color: Color::monochrome_u8(120),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                height: 20.0,
                corner_diameter: CornerDiameter::uniform(10.0),
                font_size: FontSize(14.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 30.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            text_box: TextBoxTheme {
                background_color: Color::monochrome_u8(40),
                foreground_color: Color::monochrome_u8(220),
                highlight_color: Color::rgb_u8(255, 160, 60),
                hovered_background_color: Color::monochrome_u8(60),
                hovered_foreground_color: Color::monochrome_u8(250),
                focused_background_color: Color::monochrome_u8(110),
                focused_foreground_color: Color::monochrome_u8(255),
                ghost_foreground_color: Color::monochrome_u8(130),
                hide_icon_color: Color::monochrome_u8(180),
                hovered_hide_icon_color: Color::rgb_u8(250, 200, 200),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                height: 20.0,
                corner_diameter: CornerDiameter::uniform(10.0),
                font_size: FontSize(14.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 15.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            collapsible: CollapsibleTheme {
                background_color: Color::monochrome_u8(75),
                secondary_background_color: Color::monochrome_u8(55),
                foreground_color: Color::monochrome_u8(170),
                highlight_color: Color::rgb_u8(255, 160, 60),
                hovered_foreground_color: Color::rgb_u8(250, 200, 200),
                corner_diameter: CornerDiameter::uniform(10.0),
                icon_color: Color::monochrome_u8(120),
                icon_size: 10.0,
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                gaps: 4.0,
                border: 5.0,
                title_height: 20.0,
                font_size: FontSize(14.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 0.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            drop_down: DropDownTheme {
                item_background_color: Color::monochrome_u8(80),
                item_foreground_color: Color::monochrome_u8(180),
                item_highlight_color: Color::rgb_u8(255, 160, 60),
                item_hovered_background_color: Color::monochrome_u8(120),
                item_hovered_foreground_color: Color::monochrome_u8(220),
                item_shadow_color: Color::rgba_u8(0, 0, 0, 100),
                item_shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                item_height: 20.0,
                item_corner_diameter: CornerDiameter::uniform(10.0),
                item_font_size: FontSize(14.0),
                item_horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 5.0 },
                item_vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                item_overflow_behavior: OverflowBehavior::Shrink,
                list_corner_diameter: CornerDiameter::uniform(8.0),
                list_background_color: Color::monochrome_u8(40),
                list_shadow_color: Color::rgba_u8(0, 0, 0, 100),
                list_shadow_padding: ShadowPadding::diagonal(5.0, 10.0),
                list_gaps: 4.0,
                list_border: 4.0,
                list_maximum_height: 500.0,
                button_background_color: Color::monochrome_u8(120),
                button_foreground_color: Color::monochrome_u8(220),
                button_highlight_color: Color::rgb_u8(255, 160, 60),
                button_hovered_background_color: Color::monochrome_u8(150),
                button_hovered_foreground_color: Color::monochrome_u8(250),
                button_shadow_color: Color::rgba_u8(0, 0, 0, 100),
                button_shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                button_height: 20.0,
                button_corner_diameter: CornerDiameter::uniform(10.0),
                button_font_size: FontSize(14.0),
                button_horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 5.0 },
                button_vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                button_overflow_behavior: OverflowBehavior::Shrink,
            },
            field: FieldTheme {
                background_color: Color::monochrome_u8(100),
                foreground_color: Color::monochrome_u8(220),
                highlight_color: Color::rgb_u8(255, 160, 60),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                height: 20.0,
                corner_diameter: CornerDiameter::uniform(10.0),
                font_size: FontSize(14.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 10.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -2.0 },
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            tooltip: TooltipTheme {
                background_color: Color::rgba_u8(15, 15, 15, 200),
                foreground_color: Color::monochrome_u8(235),
                highlight_color: Color::rgb_u8(255, 160, 60),
                shadow_color: Color::rgba_u8(0, 0, 0, 100),
                shadow_padding: ShadowPadding::diagonal(2.0, 5.0),
                font_size: FontSize(14.0),
                corner_diameter: CornerDiameter::uniform(5.0),
                border: 4.0,
                gap: 3.0,
                mouse_offset: 16.0,
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            debug_button: DebugButtonTheme {
                foreground_color: Color::rgb_u8(255, 167, 89),
                hovered_background_color: Color::rgb_u8(225, 199, 115),
            },
            chat: ChatTheme {
                window_color: Color::rgba_u8(0, 0, 0, 200),
                text_box_background_color: Color::rgba_u8(0, 0, 0, 150),
                broadcast_color: Color::rgb_u8(220, 200, 255),
                server_color: Color::rgb_u8(180, 220, 255),
                error_color: Color::rgb_u8(255, 120, 120),
                information_color: Color::rgb_u8(200, 230, 180),
                public_color: Color::rgb_u8(235, 236, 240),
                whisper_color: Color::rgb_u8(181, 222, 239),
                party_color: Color::rgb_u8(230, 215, 200),
                guild_color: Color::rgb_u8(170, 220, 140),
                bubble_color: Color::rgb_u8(235, 236, 240),
                bubble_background_color: Color::rgba_u8(0, 0, 0, 160),
                announce_background_color: Color::rgba_u8(0, 0, 0, 160),
            },
            skill_tree: SkillTreeTheme {
                slot_background_color: Color::monochrome_u8(40),
                slot_outline: ShadowPadding::uniform(3.0),
                requirement_color: Color::rgba_u8(200, 100, 200, 255),
                hover_color: Color::WHITE,
                slot_corner_diameter: CornerDiameter::uniform(20.0),
                unoccupied_slot_color: Color::rgba_u8(30, 30, 30, 160),
                unoccupied_slot_outline: ShadowPadding::uniform(2.0),
                unlearned_skill_color: Color::monochrome_u8(80),
                name_font_size: FontSize(12.0),
                name_text_color: Color::WHITE,
                highlight_color: Color::rgb_u8(255, 160, 60),
                points_color: Color::WHITE,
                pending_points_color: Color::rgb_u8(0, 255, 220),
                points_font_size: FontSize(12.0),
                arrow_color: Color::monochrome_u8(160),
                lower_level_color: Color::rgb_u8(255, 200, 255),
                slot_border_color: Color::rgb_u8(58, 66, 82),
                slot_hovered_border_color: Color::rgb_u8(255, 170, 60),
                slot_selected_border_color: Color::rgb_u8(80, 220, 200),
            },
            global: GlobalTheme {
                drop_area_color: Color::rgba_u8(255, 60, 200, 120),
                hovered_drop_area_color: Color::rgba_u8(0, 255, 200, 130),
                fill_alpha: 0.1,
                drop_area_outline: ShadowPadding::uniform(4.0),
            },
            basic_info: BasicInfoTheme::default(),
        }
    }
}

#[cfg(test)]
mod turoran_dark_theme_tests {
    use super::{InterfaceTheme, InterfaceThemeType};
    use crate::settings::TURORAN_DARK_THEME_NAME;

    #[test]
    fn turoran_dark_roundtrips_through_ron() {
        let theme = InterfaceTheme::turoran_dark();
        let encoded = ron::ser::to_string_pretty(&theme, ron::ser::PrettyConfig::new()).expect("serialize theme");
        let decoded: InterfaceTheme = ron::from_str(&encoded).expect("deserialize theme");

        assert_eq!(
            decoded.window.background_color.alpha_as_u8(),
            theme.window.background_color.alpha_as_u8()
        );
        assert_eq!(decoded.window.highlight_color.red_as_u8(), 255);
        assert_eq!(decoded.window.highlight_color.green_as_u8(), 152);
        assert_eq!(decoded.window.highlight_color.blue_as_u8(), 48);
        assert_eq!(decoded.chat.window_color.alpha_as_u8(), theme.chat.window_color.alpha_as_u8());
        assert!(
            decoded.window.background_color.alpha_as_u8() < 255,
            "window should stay translucent"
        );
        assert!(decoded.chat.window_color.alpha_as_u8() < 255, "chat should stay translucent");
        assert_ne!(decoded.chat.error_color, decoded.chat.server_color);
        assert_ne!(decoded.chat.broadcast_color, decoded.chat.information_color);
        assert_eq!(decoded.chat.party_color.red_as_u8(), 230);
        assert_eq!(decoded.chat.party_color.green_as_u8(), 215);
        assert_eq!(decoded.chat.party_color.blue_as_u8(), 200);
    }

    #[test]
    fn load_falls_back_to_built_in_turoran_dark() {
        let loaded = InterfaceTheme::load(InterfaceThemeType::InGame, TURORAN_DARK_THEME_NAME);
        let expected = InterfaceTheme::turoran_dark();

        assert_eq!(
            loaded.window.background_color.red_as_u8(),
            expected.window.background_color.red_as_u8()
        );
        assert_eq!(
            loaded.window.background_color.green_as_u8(),
            expected.window.background_color.green_as_u8()
        );
        assert_eq!(
            loaded.window.background_color.blue_as_u8(),
            expected.window.background_color.blue_as_u8()
        );
        assert_eq!(
            loaded.window.background_color.alpha_as_u8(),
            expected.window.background_color.alpha_as_u8()
        );
        assert_eq!(
            loaded.window.highlight_color.red_as_u8(),
            expected.window.highlight_color.red_as_u8()
        );
        assert_eq!(loaded.chat.window_color.alpha_as_u8(), expected.chat.window_color.alpha_as_u8());
        assert_eq!(
            loaded.button.highlight_color.green_as_u8(),
            expected.button.highlight_color.green_as_u8()
        );
    }

    #[test]
    fn turoran_dark_differs_from_stock_in_game_default() {
        let turoran = InterfaceTheme::turoran_dark();
        let stock = InterfaceTheme::load(InterfaceThemeType::InGame, "^missing-stock-fallback^");

        assert_ne!(
            (
                turoran.window.background_color.red_as_u8(),
                turoran.window.background_color.green_as_u8(),
                turoran.window.background_color.blue_as_u8(),
                turoran.window.background_color.alpha_as_u8(),
            ),
            (
                stock.window.background_color.red_as_u8(),
                stock.window.background_color.green_as_u8(),
                stock.window.background_color.blue_as_u8(),
                stock.window.background_color.alpha_as_u8(),
            )
        );
        assert_ne!(
            turoran.window.highlight_color.green_as_u8(),
            stock.window.highlight_color.green_as_u8()
        );
    }
}

#[cfg(test)]
mod verdant_rune_theme_tests {
    use super::{InterfaceTheme, InterfaceThemeType};
    use crate::settings::{TURORAN_DARK_THEME_NAME, VERDANT_RUNE_THEME_NAME};

    #[test]
    fn verdant_rune_roundtrips_through_ron() {
        let theme = InterfaceTheme::verdant_rune();
        let encoded = ron::ser::to_string_pretty(&theme, ron::ser::PrettyConfig::new()).expect("serialize theme");
        let decoded: InterfaceTheme = ron::from_str(&encoded).expect("deserialize theme");

        assert_eq!(decoded.window.highlight_color.red_as_u8(), 76);
        assert_eq!(decoded.window.highlight_color.green_as_u8(), 195);
        assert_eq!(decoded.window.highlight_color.blue_as_u8(), 160);
        assert_eq!(decoded.text.color.red_as_u8(), 227);
        assert_eq!(decoded.text.color.green_as_u8(), 237);
        assert_eq!(decoded.text.color.blue_as_u8(), 230);
        assert_eq!(decoded.chat.error_color.red_as_u8(), 224);
        assert_eq!(decoded.chat.error_color.green_as_u8(), 112);
        assert_eq!(decoded.chat.error_color.blue_as_u8(), 138);
        assert_eq!(decoded.skill_tree.highlight_color.red_as_u8(), 207);
        assert_eq!(decoded.skill_tree.highlight_color.green_as_u8(), 174);
        assert_eq!(decoded.skill_tree.highlight_color.blue_as_u8(), 90);
        assert_eq!(
            decoded.window.background_color.alpha_as_u8(),
            theme.window.background_color.alpha_as_u8()
        );
        assert!(decoded.window.background_color.alpha_as_u8() < 255);
        assert!(decoded.chat.window_color.alpha_as_u8() < 255);
        assert_ne!(decoded.chat.error_color, decoded.chat.server_color);
    }

    #[test]
    fn load_falls_back_to_built_in_verdant_rune() {
        let loaded = InterfaceTheme::load(InterfaceThemeType::InGame, VERDANT_RUNE_THEME_NAME);
        let expected = InterfaceTheme::verdant_rune();

        assert_eq!(
            loaded.window.highlight_color.red_as_u8(),
            expected.window.highlight_color.red_as_u8()
        );
        assert_eq!(
            loaded.window.highlight_color.green_as_u8(),
            expected.window.highlight_color.green_as_u8()
        );
        assert_eq!(
            loaded.window.highlight_color.blue_as_u8(),
            expected.window.highlight_color.blue_as_u8()
        );
        assert_eq!(
            loaded.window.background_color.red_as_u8(),
            expected.window.background_color.red_as_u8()
        );
        assert_eq!(loaded.chat.error_color.red_as_u8(), expected.chat.error_color.red_as_u8());
        assert_eq!(
            loaded.button.highlight_color.green_as_u8(),
            expected.button.highlight_color.green_as_u8()
        );
    }

    #[test]
    fn verdant_rune_differs_from_turoran_dark_and_stock() {
        let verdant = InterfaceTheme::verdant_rune();
        let turoran = InterfaceTheme::load(InterfaceThemeType::InGame, TURORAN_DARK_THEME_NAME);
        let stock = InterfaceTheme::load(InterfaceThemeType::InGame, "^missing-stock-fallback^");

        assert_ne!(
            verdant.window.highlight_color.green_as_u8(),
            turoran.window.highlight_color.green_as_u8()
        );
        assert_ne!(
            verdant.window.background_color.red_as_u8(),
            turoran.window.background_color.red_as_u8()
        );
        assert_ne!(
            verdant.window.background_color.red_as_u8(),
            stock.window.background_color.red_as_u8()
        );
        assert_ne!(
            verdant.window.highlight_color.green_as_u8(),
            stock.window.highlight_color.green_as_u8()
        );
        assert_ne!(verdant.text.color.green_as_u8(), stock.text.color.green_as_u8());
    }
}

#[cfg(test)]
mod token_slot_border_tests {
    use super::InterfaceTheme;

    #[test]
    fn token_slot_borders_are_never_gold() {
        for theme in [
            InterfaceTheme::verdant_rune(),
            InterfaceTheme::turoran_dark(),
            InterfaceTheme::ember_forge(),
            InterfaceTheme::arcane_violet(),
        ] {
            assert_eq!(theme.skill_tree.slot_border_color, theme.tokens.border);
            assert_eq!(theme.skill_tree.slot_hovered_border_color, theme.tokens.hover);
            assert_eq!(theme.skill_tree.slot_selected_border_color, theme.tokens.accent);
            assert_ne!(theme.skill_tree.slot_border_color, theme.tokens.gold);
            assert_ne!(theme.skill_tree.slot_border_color, theme.tokens.hover);
            assert_eq!(
                theme.skill_tree.slot_background_color.red_as_u8(),
                theme.tokens.raised.red_as_u8()
            );
            assert_eq!(
                theme.skill_tree.slot_background_color.green_as_u8(),
                theme.tokens.raised.green_as_u8()
            );
            assert_eq!(
                theme.skill_tree.slot_background_color.blue_as_u8(),
                theme.tokens.raised.blue_as_u8()
            );
        }
    }
}
