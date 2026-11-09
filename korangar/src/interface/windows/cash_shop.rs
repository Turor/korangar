use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::State;

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::bank_cash_roulette::CashShopWindowStatePathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

struct CashTabBox;
struct CashItemBox;
struct CashAmountBox;
struct CashKafraBox;

pub struct CashShopWindow;

impl CashShopWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for CashShopWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::CashShop)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().cash_shop_window();
        let ignore = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Cash shop",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.catalog(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: state_path.tab_ghost(),
                    state: state_path.tab_text(),
                    input_handler: DefaultHandler::<_, _, 4>::new(state_path.tab_text(), ignore),
                    focus_id: CashTabBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: state_path.item_ghost(),
                    state: state_path.item_text(),
                    input_handler: DefaultHandler::<_, _, 12>::new(state_path.item_text(), ignore),
                    focus_id: CashItemBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: state_path.amount_ghost(),
                    state: state_path.amount_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.amount_text(), ignore),
                    focus_id: CashAmountBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: state_path.kafra_ghost(),
                    state: state_path.kafra_text(),
                    input_handler: DefaultHandler::<_, _, 12>::new(state_path.kafra_text(), ignore),
                    focus_id: CashKafraBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! { text: "Open shop", event: InputEvent::OpenCashShop },
                button! { text: "Load tab", event: InputEvent::RequestCashShopTab },
                button! { text: "Buy", event: InputEvent::BuyCashShopItem },
                button! { text: "Close shop", event: InputEvent::CloseCashShop },
            ),
        }
    }
}
