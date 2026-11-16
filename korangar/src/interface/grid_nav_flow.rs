//! Inventory grid keys (inventory review #17). `lib.rs` calls this once per
//! frame after the NPC dialog has had its keys; the decisions are the pure
//! functions in `state::grid_nav`.

use korangar_interface::Interface;
use ragnarok_packets::InventoryIndex;
use rust_state::State;
use winit::keyboard::KeyCode;

use crate::input::{InputEvent, InputSystem};
use crate::interface::windows::WindowClass;
use crate::state::grid_nav::{GridFrame, GridKey, grid_frame};
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Keys the grid reads itself; anything else disarms Enter.
const OWN_KEYS: [KeyCode; 9] = [
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::Enter,
    KeyCode::NumpadEnter,
    KeyCode::Escape,
];

const KEYS: [(KeyCode, GridKey); 6] = [
    (KeyCode::ArrowLeft, GridKey::Left),
    (KeyCode::ArrowRight, GridKey::Right),
    (KeyCode::ArrowUp, GridKey::Up),
    (KeyCode::ArrowDown, GridKey::Down),
    (KeyCode::Home, GridKey::Home),
    (KeyCode::End, GridKey::End),
];

/// Arrows / Home / End move the cursor in the open inventory; Enter uses or
/// equips the stack under it only while a grid key was the last input.
/// Returns `true` when the grid took Enter, so chat must not also focus.
pub(crate) fn handle_inventory_grid_keys(
    input_system: &InputSystem,
    interface: &Interface<'static, ClientState>,
    state: &mut State<ClientState>,
    input_events: &mut Vec<InputEvent>,
    keys_taken: bool,
    mouse_clicked: bool,
) -> bool {
    let dialog = state.follow(client_state().dialog_window());
    let frame = GridFrame {
        inventory_open: interface.is_window_with_class_open(WindowClass::Inventory),
        keys_taken,
        dialog_waiting: dialog.is_awaiting_choice() || dialog.is_awaiting_next() || dialog.is_awaiting_close() || dialog.input_is_open(),
        menu_open: state.follow(client_state().item_menu()).is_open(),
        mouse_clicked,
        escape: input_system.key_pressed(KeyCode::Escape),
        other_key: input_system.other_key_pressed(&OWN_KEYS),
        keys: KEYS
            .iter()
            .filter(|(code, _)| input_system.key_pressed(*code))
            .map(|(_, key)| *key)
            .collect(),
        enter: input_system.key_pressed(KeyCode::Enter) || input_system.key_pressed(KeyCode::NumpadEnter),
    };

    let inventory = state.follow_mut(client_state().inventory());
    let used: Option<InventoryIndex> = grid_frame(inventory.view_mut(), &frame);
    if let Some(index) = used {
        input_events.push(InputEvent::UseItem { index });
    }
    used.is_some()
}
