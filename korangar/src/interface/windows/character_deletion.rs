//! Six-character confirmation for `0x0829`. This server compares those bytes
//! to `delete`.

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{PathExt, State};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::character_screen::{CharacterScreenPathExt, confirmation_bytes};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

const CONFIRMATION_LEN: usize = 6;

pub struct CharacterDeletionWindow;

impl CustomWindow<ClientState> for CharacterDeletionWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::CharacterDeletion)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        struct ConfirmationField;

        let confirmation = client_state().character_screen().delete_confirmation();
        let disabled = ComputedSelector::new_default(|state: &ClientState| {
            let screen = client_state().character_screen().follow_safe(state);
            screen.pending_delete_id().is_none() || confirmation_bytes(&screen.delete_confirmation).is_none()
        });

        let confirm = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            let screen_path = client_state().character_screen();
            let screen = state.get(&screen_path);
            let Some(character_id) = screen.pending_delete_id() else {
                return;
            };
            let Some(_) = confirmation_bytes(&screen.delete_confirmation) else {
                return;
            };
            queue.queue(InputEvent::AcceptCharacterDeletion {
                character_id,
                confirmation: screen.delete_confirmation.clone(),
            });
        };

        window! {
            title: "Confirm deletion",
            class: Self::window_class(),
            theme: InterfaceThemeType::Menu,
            closable: true,
            minimum_width: 360.0,
            maximum_width: 420.0,
            elements: (
                text! {
                    text: "Type delete.",
                },
                text_box! {
                    ghost_text: "delete",
                    state: confirmation,
                    input_handler: DefaultHandler::<_, _, CONFIRMATION_LEN>::new(confirmation, confirm),
                    focus_id: ConfirmationField,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Confirm deletion",
                    disabled,
                    event: confirm,
                },
            ),
        }
    }
}
