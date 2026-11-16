//! roBrowser's amount `InputBox` for moving part of a stack into or out of
//! storage, the cart, or a trade. The model is `state::item_move`.

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::State;

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::item_move::ItemMovePromptPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

pub struct ItemMoveAmountBox;

pub struct ItemMoveAmountWindow;

impl CustomWindow<ClientState> for ItemMoveAmountWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ItemMoveAmount)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let prompt = client_state().item_move();
        let confirm = |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::ConfirmItemMove);
        };

        window! {
            title: "Move items",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: prompt.prompt(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: "amount",
                    state: prompt.amount_text(),
                    input_handler: DefaultHandler::<_, _, 5>::new(prompt.amount_text(), confirm),
                    focus_id: ItemMoveAmountBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Cancel",
                            event: InputEvent::CancelItemMove,
                        },
                        button! {
                            text: "OK",
                            event: InputEvent::ConfirmItemMove,
                        },
                    ),
                },
            ),
        }
    }
}
