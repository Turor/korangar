use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::StateElement;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{parse_stable_action, parse_stable_id};
use rust_state::{RustState, State};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Stable board text and the ids the player asks the server to act on.
#[derive(RustState, StateElement)]
pub struct StableWindowState {
    status: String,
    homunculi: String,
    pets: String,
    owned: String,
    homunculus_id_text: String,
    pet_id_text: String,
    pet_action_text: String,
    homunculus_ghost: String,
    pet_ghost: String,
    action_ghost: String,
}

impl Default for StableWindowState {
    fn default() -> Self {
        Self {
            status: "Open the stable to load homunculi, pets, and evolution materials.".to_owned(),
            homunculi: String::new(),
            pets: String::new(),
            owned: String::new(),
            homunculus_id_text: String::new(),
            pet_id_text: String::new(),
            pet_action_text: "0".to_owned(),
            homunculus_ghost: "homunculus id".to_owned(),
            pet_ghost: "pet id".to_owned(),
            action_ghost: "pet action".to_owned(),
        }
    }
}

impl StableWindowState {
    pub fn show_homunculi(&mut self, summary: String) {
        self.homunculi = summary;
    }

    pub fn show_pets(&mut self, summary: String) {
        self.pets = summary;
    }

    pub fn show_owned(&mut self, summary: String) {
        self.owned = summary;
    }

    pub fn show_result(&mut self, summary: String) {
        self.status = summary;
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn homunculus_request(&self) -> Result<i32, &'static str> {
        parse_stable_id(&self.homunculus_id_text).ok_or("Enter a homunculus id.")
    }

    pub fn pet_request(&self) -> Result<(i16, i32), &'static str> {
        let action = parse_stable_action(&self.pet_action_text).ok_or("Enter a pet action.")?;
        let pet_id = parse_stable_id(&self.pet_id_text).ok_or("Enter a pet id.")?;
        Ok((action, pet_id))
    }
}

struct HomunculusBox;
struct PetBox;
struct ActionBox;

pub struct StableWindow;

impl StableWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for StableWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Stable)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().stable_window();
        let homunculus_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};
        let pet_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};
        let action_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Stable",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Homunculi" },
                text! {
                    text: state_path.homunculi(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Pets" },
                text! {
                    text: state_path.pets(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Evolution materials" },
                text! {
                    text: state_path.owned(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Homunculus id" },
                text_box! {
                    ghost_text: state_path.homunculus_ghost(),
                    state: state_path.homunculus_id_text(),
                    input_handler: DefaultHandler::<_, _, 12>::new(state_path.homunculus_id_text(), homunculus_action),
                    focus_id: HomunculusBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Request homunculus",
                    event: InputEvent::RequestStableHomunculus,
                },
                text! { text: "Pet action and id" },
                text_box! {
                    ghost_text: state_path.action_ghost(),
                    state: state_path.pet_action_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.pet_action_text(), action_action),
                    focus_id: ActionBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: state_path.pet_ghost(),
                    state: state_path.pet_id_text(),
                    input_handler: DefaultHandler::<_, _, 12>::new(state_path.pet_id_text(), pet_action),
                    focus_id: PetBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Request pet",
                    event: InputEvent::RequestStablePet,
                },
                button! {
                    text: "Refresh",
                    event: InputEvent::OpenStable,
                },
            ),
        }
    }
}
