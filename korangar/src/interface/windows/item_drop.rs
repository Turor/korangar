use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::State;

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::item_drop::ItemDropPromptPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

pub struct ItemDropAmountBox;

pub struct ItemDropWindow;

impl ItemDropWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for ItemDropWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ItemDrop)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let prompt = client_state().item_drop();
        let ignore = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Drop item",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: prompt.prompt(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: prompt.amount_ghost(),
                    state: prompt.amount_text(),
                    input_handler: DefaultHandler::<_, _, 12>::new(prompt.amount_text(), ignore),
                    focus_id: ItemDropAmountBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Cancel",
                            event: InputEvent::CancelItemDrop,
                        },
                        button! {
                            text: "OK",
                            event: InputEvent::ConfirmItemDrop,
                        },
                    ),
                },
            ),
        }
    }
}
