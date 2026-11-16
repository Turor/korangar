use korangar_interface::window::{CustomWindow, Window};
use rust_state::Path;

use crate::input::InputEvent;
use crate::interface::font_scale::FontScaleSettingPathExt;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::settings::{InterfaceSettings, InterfaceSettingsCapabilities, InterfaceSettingsCapabilitiesPathExt, InterfaceSettingsPathExt};
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

struct FontScalePercentBox;

pub struct InterfaceSettingsWindow<A, B> {
    settings_path: A,
    capabilities_path: B,
}

impl<A, B> InterfaceSettingsWindow<A, B> {
    pub fn new(settings_path: A, capabilities_path: B) -> Self {
        Self {
            settings_path,
            capabilities_path,
        }
    }
}

impl<A, B> CustomWindow<ClientState> for InterfaceSettingsWindow<A, B>
where
    A: Path<ClientState, InterfaceSettings>,
    B: Path<ClientState, InterfaceSettingsCapabilities>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::InterfaceSettings)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let elements = (
            crate::interface::windows::WindowGroupTabs::new(WindowClass::InterfaceSettings),
            split! {
                children: (
                    text! {
                        text: client_state().localization().language_text(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    drop_down! {
                        selected: self.settings_path.language(),
                        options: self.capabilities_path.languages(),
                    }
                )
            },
            split! {
                children: (
                    text! {
                        text: client_state().localization().scaling_text(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    drop_down! {
                        selected: self.settings_path.scaling(),
                        options: self.capabilities_path.scalings(),
                    }
                )
            },
            split! {
                children: (
                    text! {
                        text: client_state().localization().menu_theme_text(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    drop_down! {
                        selected: self.settings_path.menu_theme(),
                        options: self.capabilities_path.menu_themes(),
                    }
                )
            },
            split! {
                children: (
                    text! {
                        text: client_state().localization().in_game_theme_text(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    drop_down! {
                        selected: self.settings_path.in_game_theme(),
                        options: self.capabilities_path.in_game_themes(),
                    }
                )
            },
            split! {
                children: (
                    text! {
                        text: client_state().localization().world_theme_text(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    drop_down! {
                        selected: self.settings_path.world_theme(),
                        options: self.capabilities_path.world_themes(),
                    }
                )
            },
            split! {
                children: (
                    text! {
                        text: "Font scale %",
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    super::font_scale_control::FontScaleSlider::new(self.settings_path.font_scale()),
                    text_box! {
                        ghost_text: "100",
                        state: self.settings_path.font_scale().draft(),
                        input_handler: super::font_scale_control::FontScaleInput::new(self.settings_path.font_scale()),
                        focus_id: FontScalePercentBox,
                    },
                )
            },
            button! {
                text: "Reload theme",
                event: InputEvent::ReloadTheme,
            },
        );

        window! {
            title: client_state().localization().interface_settings_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements,
        }
    }
}
