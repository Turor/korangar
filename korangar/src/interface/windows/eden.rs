use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::StateElement;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::parse_eden_id;
use rust_state::{RustState, State};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Eden board text and the quest or shop slot the player asks the server about.
#[derive(RustState, StateElement)]
pub struct EdenWindowState {
    status: String,
    quests: String,
    shop: String,
    quest_id_text: String,
    shop_slot_text: String,
    quest_ghost: String,
    shop_ghost: String,
}

impl Default for EdenWindowState {
    fn default() -> Self {
        Self {
            status: "Open the Eden board to load missions and the merit shop.".to_owned(),
            quests: String::new(),
            shop: String::new(),
            quest_id_text: String::new(),
            shop_slot_text: String::new(),
            quest_ghost: "quest id".to_owned(),
            shop_ghost: "shop slot".to_owned(),
        }
    }
}

impl EdenWindowState {
    pub fn show_quests(&mut self, summary: String) {
        self.quests = summary;
    }

    pub fn show_shop(&mut self, summary: String) {
        self.shop = summary;
    }

    pub fn show_result(&mut self, summary: String) {
        self.status = summary;
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn quest_id(&self) -> Result<i32, &'static str> {
        parse_eden_id(&self.quest_id_text).ok_or("Enter a quest id.")
    }

    pub fn shop_slot(&self) -> Result<i32, &'static str> {
        parse_eden_id(&self.shop_slot_text).ok_or("Enter a shop slot.")
    }
}

struct QuestBox;
struct ShopBox;

pub struct EdenWindow;

impl EdenWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for EdenWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Eden)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().eden_window();
        let quest_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};
        let shop_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Eden Group",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Missions" },
                text! {
                    text: state_path.quests(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Merit shop" },
                text! {
                    text: state_path.shop(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Quest id" },
                text_box! {
                    ghost_text: state_path.quest_ghost(),
                    state: state_path.quest_id_text(),
                    input_handler: DefaultHandler::<_, _, 12>::new(state_path.quest_id_text(), quest_action),
                    focus_id: QuestBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Accept",
                    event: InputEvent::EdenAccept,
                },
                button! {
                    text: "Complete",
                    event: InputEvent::EdenComplete,
                },
                button! {
                    text: "Abandon",
                    event: InputEvent::EdenAbandon,
                },
                text! { text: "Shop slot" },
                text_box! {
                    ghost_text: state_path.shop_ghost(),
                    state: state_path.shop_slot_text(),
                    input_handler: DefaultHandler::<_, _, 12>::new(state_path.shop_slot_text(), shop_action),
                    focus_id: ShopBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Buy",
                    event: InputEvent::EdenBuy,
                },
                button! {
                    text: "Refresh",
                    event: InputEvent::OpenEden,
                },
            ),
        }
    }
}
