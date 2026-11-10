use korangar_interface::window::{CustomWindow, Window};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::bank_roulette::RouletteWindowStatePathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

pub struct RouletteWindow;

impl RouletteWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for RouletteWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Roulette)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().roulette_window();

        window! {
            title: "Roulette",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            elements: (
                text! {
                    text: state_path.points(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.wheel(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.prizes(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! { text: "Open", event: InputEvent::OpenRoulette },
                button! { text: "Reload prizes", event: InputEvent::RequestRouletteInfo },
                button! { text: "Spin", event: InputEvent::SpinRoulette },
                button! { text: "Claim prize", event: InputEvent::ClaimRoulette },
                button! { text: "Close roulette", event: InputEvent::CloseRoulette },
            ),
        }
    }
}
