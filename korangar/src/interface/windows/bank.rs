use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::State;

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::bank_roulette::BankWindowStatePathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

struct BankAmountBox;

pub struct BankWindow;

impl BankWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for BankWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Bank)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().bank_window();
        let ignore = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Bank",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            elements: (
                text! {
                    text: state_path.vault_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.pocket_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.history(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: state_path.amount_ghost(),
                    state: state_path.amount_text(),
                    input_handler: DefaultHandler::<_, _, 12>::new(state_path.amount_text(), ignore),
                    focus_id: BankAmountBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! { text: "Refresh", event: InputEvent::RefreshBank },
                button! { text: "+100,000", event: InputEvent::BankAdd100k },
                button! { text: "+1,000,000", event: InputEvent::BankAdd1m },
                button! { text: "+10,000,000", event: InputEvent::BankAdd10m },
                button! { text: "Max", event: InputEvent::BankSetMax },
                button! { text: "Deposit", event: InputEvent::DepositBank },
                button! { text: "Withdraw", event: InputEvent::WithdrawBank },
                button! { text: "Close bank", event: InputEvent::CloseBank },
            ),
        }
    }
}
