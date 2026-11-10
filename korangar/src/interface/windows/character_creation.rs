use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, PathExt, State};

use super::character_appearance::CharacterAppearance;
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

const MINIMUM_NAME_LENGTH: usize = 4;
const MAXIMUM_NAME_LENGTH: usize = 24;

pub struct CharacterCreationWindow<A> {
    character_name_path: A,
}

impl<A> CharacterCreationWindow<A> {
    pub fn new(character_name_path: A) -> Self {
        Self { character_name_path }
    }
}

impl<A> CustomWindow<ClientState> for CharacterCreationWindow<A>
where
    A: Path<ClientState, String>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::CharacterCreation)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        struct CharacterName;

        let name_path = self.character_name_path;
        let disabled = ComputedSelector::new_default(move |state: &ClientState| name_path.follow_safe(state).len() < MINIMUM_NAME_LENGTH);

        let create_action = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            let name = state.get(&name_path).clone();
            let screen_path = client_state().character_screen();
            let fields = state.get(&screen_path).packet_fields(&name);
            queue.queue(InputEvent::CreateCharacter {
                slot: fields.slot as usize,
                name: fields.name,
                hair_color: fields.hair_color,
                hair_style: fields.hair_style,
                start_job: fields.start_job,
                sex: fields.sex,
            });
        };

        window! {
            title: client_state().localization().create_character_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::Menu,
            closable: true,
            minimum_width: 560.0,
            maximum_width: 640.0,
            elements: (
                text_box! {
                    ghost_text: client_state().localization().character_name_text(),
                    state: name_path,
                    input_handler: DefaultHandler::<_, _, MAXIMUM_NAME_LENGTH>::new(name_path, create_action),
                    focus_id: CharacterName,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                CharacterAppearance::new(),
                button! {
                    text: client_state().localization().create_character_button_text(),
                    disabled,
                    disabled_tooltip: client_state().localization().create_character_button_tooltip(),
                    event: create_action,
                }
            ),
        }
    }
}
