//! Inventory grid keys (inventory review #17). `lib.rs` calls this once per
//! frame after the NPC dialog has had its keys; the model is
//! `state::grid_nav`.

use korangar_interface::Interface;
use rust_state::State;
use winit::keyboard::KeyCode;

use crate::input::{InputEvent, InputSystem};
use crate::interface::windows::WindowClass;
use crate::state::grid_nav::GridKey;
use crate::state::{ClientState, ClientStatePathExt, client_state};

const KEYS: [(KeyCode, GridKey); 6] = [
    (KeyCode::ArrowLeft, GridKey::Left),
    (KeyCode::ArrowRight, GridKey::Right),
    (KeyCode::ArrowUp, GridKey::Up),
    (KeyCode::ArrowDown, GridKey::Down),
    (KeyCode::Home, GridKey::Home),
    (KeyCode::End, GridKey::End),
];

/// Arrows / Home / End move the cursor in the open inventory; Enter uses or
/// equips the stack under it. Skipped while a text box has focus or an NPC
/// dialog is waiting for a key.
pub(crate) fn handle_inventory_grid_keys(
    input_system: &InputSystem,
    interface: &Interface<'static, ClientState>,
    state: &mut State<ClientState>,
    input_events: &mut Vec<InputEvent>,
    keys_taken: bool,
) {
    if keys_taken || !interface.is_window_with_class_open(WindowClass::Inventory) {
        return;
    }
    let dialog = state.follow(client_state().dialog_window());
    if dialog.is_awaiting_choice() || dialog.is_awaiting_next() || dialog.is_awaiting_close() || dialog.input_is_open() {
        return;
    }

    let pressed: Vec<GridKey> = KEYS
        .iter()
        .filter(|(code, _)| input_system.key_pressed(*code))
        .map(|(_, key)| *key)
        .collect();
    if !pressed.is_empty() {
        let inventory = state.follow_mut(client_state().inventory());
        for key in pressed {
            inventory.view_mut().move_cursor(key);
        }
    }

    if input_system.key_pressed(KeyCode::Enter) || input_system.key_pressed(KeyCode::NumpadEnter) {
        let view = state.follow(client_state().inventory()).view();
        if let Some(index) = view.cursor_item().map(|item| item.index) {
            input_events.push(InputEvent::UseItem { index });
        }
    }
}
