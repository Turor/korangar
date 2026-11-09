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
use rust_state::RustState;
use serde::{Deserialize, Serialize};

use crate::graphics::{Color, CornerDiameter, ScreenSize, ShadowPadding};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::settings::{IN_GAME_THEMES_PATH, MENU_THEMES_PATH, TURORAN_DARK_THEME_NAME};
use crate::state::ClientState;

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

#[derive(Serialize, Deserialize, RustState, StateElement, StateWindow)]
#[window_title("Theme Inspector")]
pub struct InterfaceTheme {
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
}

impl InterfaceTheme {
    pub fn load(theme_type: InterfaceThemeType, name: &str) -> Self {
        #[cfg(feature = "debug")]
        let timer = Timer::new("Load theme");

        let path = match theme_type {
            InterfaceThemeType::InGame => format!("{IN_GAME_THEMES_PATH}/{name}.ron"),
            InterfaceThemeType::Menu => format!("{MENU_THEMES_PATH}/{name}.ron"),
        };

        #[cfg(feature = "debug")]
        print_debug!("loading theme from file {}", path.magenta());

        let theme = std::fs::read_to_string(&path)
            .ok()
            .and_then(|data| ron::from_str(&data).ok())
            .unwrap_or_else(|| {
                #[cfg(feature = "debug")]
                print_debug!("[{}] failed to load theme {}", "error".red(), name.magenta());

                match (theme_type, name) {
                    (InterfaceThemeType::InGame, TURORAN_DARK_THEME_NAME) => Self::turoran_dark(),
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
        Self {
            window: WindowTheme {
                title_color: Color::rgb_u8(235, 236, 240),
                hovered_title_color: Color::rgb_u8(255, 210, 160),
                background_color: Color::rgba_u8(18, 20, 28, 215),
                highlight_color: Color::rgb_u8(255, 152, 48),
                shadow_color: Color::rgba_u8(0, 0, 0, 120),
                shadow_padding: ShadowPadding::diagonal(4.0, 8.0),
                gaps: 6.0,
                border: 8.0,
                corner_diameter: CornerDiameter::uniform(10.0),
                close_button_size: ScreenSize { width: 36.0, height: 18.0 },
                close_button_corner_diameter: CornerDiameter::uniform(10.0),
                minimum_width: 280.0,
                maximum_width: 620.0,
                minimum_height: 36.0,
                maximum_height: 720.0,
                title_height: 22.0,
                title_gap: 2.0,
                font_size: FontSize(14.0),
                horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 4.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::Shrink,
                anchor_color: Color::rgb_u8(120, 130, 160),
                closest_anchor_color: Color::rgb_u8(255, 170, 60),
            },
            text: TextTheme {
                color: Color::rgb_u8(230, 232, 238),
                highlight_color: Color::rgb_u8(255, 152, 48),
                height: 14.0,
                font_size: FontSize(13.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 3.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            button: ButtonTheme {
                background_color: Color::rgba_u8(40, 44, 56, 230),
                foreground_color: Color::rgb_u8(235, 236, 240),
                highlight_color: Color::rgb_u8(255, 152, 48),
                hovered_background_color: Color::rgba_u8(58, 64, 80, 240),
                hovered_foreground_color: Color::rgb_u8(255, 255, 255),
                disabled_background_color: Color::rgba_u8(28, 30, 38, 200),
                disabled_foreground_color: Color::rgb_u8(140, 144, 156),
                shadow_color: Color::rgba_u8(0, 0, 0, 110),
                shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                height: 20.0,
                corner_diameter: CornerDiameter::uniform(8.0),
                font_size: FontSize(13.0),
                horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 4.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            state_button: StateButtonTheme {
                background_color: Color::rgba_u8(40, 44, 56, 230),
                foreground_color: Color::rgb_u8(235, 236, 240),
                highlight_color: Color::rgb_u8(255, 152, 48),
                hovered_background_color: Color::rgba_u8(58, 64, 80, 240),
                hovered_foreground_color: Color::rgb_u8(255, 255, 255),
                disabled_background_color: Color::rgba_u8(28, 30, 38, 200),
                disabled_foreground_color: Color::rgb_u8(140, 144, 156),
                checkbox_color: Color::rgb_u8(255, 152, 48),
                hovered_checkbox_color: Color::rgb_u8(255, 180, 96),
                disabled_checkbox_color: Color::rgb_u8(120, 124, 136),
                shadow_color: Color::rgba_u8(0, 0, 0, 110),
                shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                height: 20.0,
                corner_diameter: CornerDiameter::uniform(8.0),
                font_size: FontSize(13.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 28.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: 0.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            text_box: TextBoxTheme {
                background_color: Color::rgba_u8(12, 14, 20, 230),
                foreground_color: Color::rgb_u8(235, 236, 240),
                highlight_color: Color::rgb_u8(255, 152, 48),
                hovered_background_color: Color::rgba_u8(24, 28, 36, 240),
                hovered_foreground_color: Color::rgb_u8(255, 255, 255),
                focused_background_color: Color::rgba_u8(36, 42, 54, 245),
                focused_foreground_color: Color::rgb_u8(255, 255, 255),
                ghost_foreground_color: Color::rgb_u8(130, 136, 150),
                hide_icon_color: Color::rgb_u8(190, 194, 204),
                hovered_hide_icon_color: Color::rgb_u8(255, 200, 150),
                shadow_color: Color::rgba_u8(0, 0, 0, 110),
                shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                height: 20.0,
                corner_diameter: CornerDiameter::uniform(8.0),
                font_size: FontSize(13.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 12.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            collapsible: CollapsibleTheme {
                background_color: Color::rgba_u8(32, 36, 46, 230),
                secondary_background_color: Color::rgba_u8(22, 24, 32, 230),
                foreground_color: Color::rgb_u8(220, 224, 232),
                highlight_color: Color::rgb_u8(255, 152, 48),
                hovered_foreground_color: Color::rgb_u8(255, 210, 160),
                corner_diameter: CornerDiameter::uniform(8.0),
                icon_color: Color::rgb_u8(160, 168, 184),
                icon_size: 10.0,
                shadow_color: Color::rgba_u8(0, 0, 0, 110),
                shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                gaps: 4.0,
                border: 5.0,
                title_height: 20.0,
                font_size: FontSize(13.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 0.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            drop_down: DropDownTheme {
                item_background_color: Color::rgba_u8(40, 44, 56, 235),
                item_foreground_color: Color::rgb_u8(230, 232, 238),
                item_highlight_color: Color::rgb_u8(255, 152, 48),
                item_hovered_background_color: Color::rgba_u8(58, 64, 80, 245),
                item_hovered_foreground_color: Color::rgb_u8(255, 255, 255),
                item_shadow_color: Color::rgba_u8(0, 0, 0, 110),
                item_shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                item_height: 20.0,
                item_corner_diameter: CornerDiameter::uniform(8.0),
                item_font_size: FontSize(13.0),
                item_horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 4.0 },
                item_vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                item_overflow_behavior: OverflowBehavior::Shrink,
                list_corner_diameter: CornerDiameter::uniform(8.0),
                list_background_color: Color::rgba_u8(16, 18, 26, 235),
                list_shadow_color: Color::rgba_u8(0, 0, 0, 120),
                list_shadow_padding: ShadowPadding::diagonal(4.0, 8.0),
                list_gaps: 3.0,
                list_border: 4.0,
                list_maximum_height: 500.0,
                button_background_color: Color::rgba_u8(40, 44, 56, 230),
                button_foreground_color: Color::rgb_u8(235, 236, 240),
                button_highlight_color: Color::rgb_u8(255, 152, 48),
                button_hovered_background_color: Color::rgba_u8(58, 64, 80, 240),
                button_hovered_foreground_color: Color::rgb_u8(255, 255, 255),
                button_shadow_color: Color::rgba_u8(0, 0, 0, 110),
                button_shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                button_height: 20.0,
                button_corner_diameter: CornerDiameter::uniform(8.0),
                button_font_size: FontSize(13.0),
                button_horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 4.0 },
                button_vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                button_overflow_behavior: OverflowBehavior::Shrink,
            },
            field: FieldTheme {
                background_color: Color::rgba_u8(28, 32, 42, 230),
                foreground_color: Color::rgb_u8(230, 232, 238),
                highlight_color: Color::rgb_u8(255, 152, 48),
                shadow_color: Color::rgba_u8(0, 0, 0, 110),
                shadow_padding: ShadowPadding::diagonal(2.0, 4.0),
                height: 20.0,
                corner_diameter: CornerDiameter::uniform(8.0),
                font_size: FontSize(13.0),
                horizontal_alignment: HorizontalAlignment::Left { offset: 5.0, border: 3.0 },
                vertical_alignment: VerticalAlignment::Center { offset: -1.0 },
                overflow_behavior: OverflowBehavior::Shrink,
            },
            tooltip: TooltipTheme {
                background_color: Color::rgba_u8(12, 14, 20, 230),
                foreground_color: Color::rgb_u8(235, 236, 240),
                highlight_color: Color::rgb_u8(255, 152, 48),
                shadow_color: Color::rgba_u8(0, 0, 0, 120),
                shadow_padding: ShadowPadding::diagonal(3.0, 6.0),
                font_size: FontSize(12.0),
                corner_diameter: CornerDiameter::uniform(8.0),
                border: 4.0,
                gap: 3.0,
                mouse_offset: 14.0,
                overflow_behavior: OverflowBehavior::LineBreak,
            },
            debug_button: DebugButtonTheme {
                foreground_color: Color::rgb_u8(255, 170, 96),
                hovered_background_color: Color::rgb_u8(255, 200, 130),
            },
            chat: ChatTheme {
                window_color: Color::rgba_u8(12, 14, 20, 205),
                text_box_background_color: Color::rgba_u8(8, 10, 14, 190),
                broadcast_color: Color::rgb_u8(210, 190, 255),
                server_color: Color::rgb_u8(140, 200, 230),
                error_color: Color::rgb_u8(240, 110, 110),
                information_color: Color::rgb_u8(180, 220, 170),
            },
            skill_tree: SkillTreeTheme {
                slot_background_color: Color::rgba_u8(28, 32, 42, 230),
                slot_outline: ShadowPadding::uniform(2.0),
                requirement_color: Color::rgba_u8(180, 140, 220, 255),
                hover_color: Color::WHITE,
                slot_corner_diameter: CornerDiameter::uniform(12.0),
                unoccupied_slot_color: Color::rgba_u8(20, 22, 30, 170),
                unoccupied_slot_outline: ShadowPadding::uniform(2.0),
                unlearned_skill_color: Color::rgb_u8(90, 94, 108),
                name_font_size: FontSize(12.0),
                name_text_color: Color::rgb_u8(235, 236, 240),
                highlight_color: Color::rgb_u8(255, 152, 48),
                points_color: Color::rgb_u8(235, 236, 240),
                pending_points_color: Color::rgb_u8(80, 220, 200),
                points_font_size: FontSize(12.0),
                arrow_color: Color::rgb_u8(160, 168, 184),
                lower_level_color: Color::rgb_u8(220, 190, 255),
            },
            global: GlobalTheme {
                drop_area_color: Color::rgba_u8(255, 152, 48, 110),
                hovered_drop_area_color: Color::rgba_u8(80, 220, 200, 130),
                fill_alpha: 0.12,
                drop_area_outline: ShadowPadding::uniform(3.0),
            },
        }
    }

    fn default_menu() -> Self {
        Self {
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
            },
            global: GlobalTheme {
                drop_area_color: Color::rgba_u8(255, 60, 200, 120),
                hovered_drop_area_color: Color::rgba_u8(0, 255, 200, 130),
                fill_alpha: 0.1,
                drop_area_outline: ShadowPadding::uniform(4.0),
            },
        }
    }

    fn default_in_game() -> Self {
        Self {
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
            },
            global: GlobalTheme {
                drop_area_color: Color::rgba_u8(255, 60, 200, 120),
                hovered_drop_area_color: Color::rgba_u8(0, 255, 200, 130),
                fill_alpha: 0.1,
                drop_area_outline: ShadowPadding::uniform(4.0),
            },
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
