use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::StateElement;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{format_enchant_result, parse_enchant_index, parse_enchant_item_id, parse_enchant_slot};
use ragnarok_packets::ItemId;
use rust_state::{RustState, State};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Fields the enchant window edits. The group id is stored as text so all 64
/// bits survive.
#[derive(RustState, StateElement)]
pub struct EnchantWindowState {
    enchant_group: String,
    index_text: String,
    material_text: String,
    slot_text: String,
    status: String,
    index_ghost: String,
    material_ghost: String,
    slot_ghost: String,
}

impl Default for EnchantWindowState {
    fn default() -> Self {
        Self {
            enchant_group: "0".to_owned(),
            index_text: String::new(),
            material_text: String::new(),
            slot_text: "0".to_owned(),
            status: String::new(),
            index_ghost: "index".to_owned(),
            material_ghost: "item id".to_owned(),
            slot_ghost: "slot".to_owned(),
        }
    }
}

impl EnchantWindowState {
    pub fn open(&mut self, enchant_group: i64) {
        self.enchant_group = enchant_group.to_string();
        self.status = "Enter an inventory index, then enchant. Close sends CZ_CLOSE_UI_ENCHANT.".to_owned();
    }

    pub fn show_result(&mut self, message_id: i32, item_id: ItemId) {
        self.status = format_enchant_result(message_id, item_id.0);
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    /// Group, inventory index, material item id, and upgrade slot.
    pub fn request(&self) -> Result<(i64, i16, u32, i16), &'static str> {
        let enchant_group = self
            .enchant_group
            .trim()
            .parse::<i64>()
            .map_err(|_| "Enchant group is not a 64-bit id.")?;
        let inventory_index = parse_enchant_index(&self.index_text).ok_or("Enter an inventory index.")?;
        let material = parse_enchant_item_id(&self.material_text).unwrap_or(0);
        let slot = parse_enchant_slot(&self.slot_text).ok_or("Enter an enchant slot.")?;
        Ok((enchant_group, inventory_index, material, slot))
    }
}

struct IndexBox;
struct MaterialBox;
struct SlotBox;

pub struct EnchantWindow;

impl EnchantWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for EnchantWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Enchant)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().enchant_window();
        let index_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};
        let material_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};
        let slot_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Enchant",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            // Title-bar close would drop the window without CZ_CLOSE_UI_ENCHANT.
            closable: false,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Enchant group" },
                text! {
                    text: state_path.enchant_group(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Inventory index" },
                text_box! {
                    ghost_text: state_path.index_ghost(),
                    state: state_path.index_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.index_text(), index_action),
                    focus_id: IndexBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Material item id (perfect enchant)" },
                text_box! {
                    ghost_text: state_path.material_ghost(),
                    state: state_path.material_text(),
                    input_handler: DefaultHandler::<_, _, 12>::new(state_path.material_text(), material_action),
                    focus_id: MaterialBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Slot (upgrade)" },
                text_box! {
                    ghost_text: state_path.slot_ghost(),
                    state: state_path.slot_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.slot_text(), slot_action),
                    focus_id: SlotBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Random enchant",
                    event: InputEvent::RequestRandomEnchant,
                },
                button! {
                    text: "Perfect enchant",
                    event: InputEvent::RequestPerfectEnchant,
                },
                button! {
                    text: "Upgrade enchant",
                    event: InputEvent::RequestUpgradeEnchant,
                },
                button! {
                    text: "Reset enchant",
                    event: InputEvent::RequestResetEnchant,
                },
                button! {
                    text: "Close",
                    event: InputEvent::CloseEnchant,
                },
            ),
        }
    }
}
