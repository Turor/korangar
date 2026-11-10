//! NPC number and string input, modeled on the roBrowser input box.
//!
//! A number shows up and down. A string is a single text box. OK and Cancel
//! both answer the script. Enter submits the focused box.

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::step_number;
use rust_state::State;

use super::WindowClass;
use super::dialog::DialogWindowStatePathExt;
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Byte cap for one NPC input. Hercules copies at most `CHATBOX_SIZE` bytes.
const NPC_INPUT_LIMIT: usize = 240;

/// Focus id for the NPC input box. One type so the frame can focus it by id.
pub struct NpcInputBox;

pub struct NpcInputWindow;

impl NpcInputWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for NpcInputWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::NpcInput)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let text_path = client_state().dialog_window().input_text();
        let submit_key = |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::SubmitNpcInput);
        };
        let submit_text = |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::SubmitNpcInput);
        };
        let submit_button = |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::SubmitNpcInput);
        };
        let cancel = |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::CancelNpcInput);
        };
        let step_up = move |state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {
            state.update_value_with(text_path, |current| {
                *current = step_number(current, 1);
            });
        };
        let step_down = move |state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {
            state.update_value_with(text_path, |current| {
                *current = step_number(current, -1);
            });
        };

        window! {
            title: "Input",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            minimum_width: 280.0,
            closable: true,
            elements: (
                either! {
                    selector: client_state().dialog_window().input_number(),
                    on_true: split! {
                        gaps: theme().window().gaps(),
                        children: (
                            text_box! {
                                ghost_text: "Number",
                                state: text_path,
                                input_handler: DefaultHandler::<_, _, NPC_INPUT_LIMIT>::new(text_path, submit_key),
                                focus_id: NpcInputBox,
                                overflow_behavior: OverflowBehavior::Shrink,
                            },
                            button! { text: "Up", event: step_up },
                            button! { text: "Down", event: step_down },
                        ),
                    },
                    on_false: text_box! {
                        ghost_text: "Text",
                        state: text_path,
                        input_handler: DefaultHandler::<_, _, NPC_INPUT_LIMIT>::new(text_path, submit_text),
                        focus_id: NpcInputBox,
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                },
                button! { text: "OK", event: submit_button },
                button! { text: "Cancel", event: cancel },
            ),
        }
    }
}
