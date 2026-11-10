use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::settings::{GameSettings, GameSettingsPathExt};
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

#[derive(Default)]
pub struct GameSettingsWindow<A> {
    game_settings_path: A,
}

impl<A> GameSettingsWindow<A> {
    pub fn new(game_settings_path: A) -> Self {
        Self { game_settings_path }
    }
}

impl<A> CustomWindow<ClientState> for GameSettingsWindow<A>
where
    A: Path<ClientState, GameSettings>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::GameSettings)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let show_equipment = self.game_settings_path.show_equipment();
        let toggle_show_equipment = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            let enabled = !*state.get(&show_equipment);
            queue.queue(InputEvent::SetShowEquipment { enabled });
        };

        window! {
            title: client_state().localization().game_settings_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                crate::interface::windows::WindowGroupTabs::new(WindowClass::GameSettings),
                state_button! {
                    text: client_state().localization().auto_attack_button_text(),
                    state: self.game_settings_path.auto_attack(),
                    event: Toggle(self.game_settings_path.auto_attack()),
                },
                state_button! {
                    text: "Show my equipment",
                    state: show_equipment,
                    event: toggle_show_equipment,
                },
            ),
        }
    }
}
