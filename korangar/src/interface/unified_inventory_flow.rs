//! Unified inventory flow (Matthew t275u). `lib.rs` forwards storage and
//! cart events here; the model is `state::unified_inventory`.

use std::path::Path;

use korangar_networking::MessageColor;

use crate::Client;
use crate::interface::windows::{InventoryWindow, WindowClass};
use crate::state::unified_inventory::{CartHotkey, NO_CART_MESSAGE, PrefsStep, VIEW_PREFS_FILE, ViewPrefsFile, cart_hotkey};
use crate::state::{ChatMessage, ClientStatePathExt, client_state};

impl Client {
    /// Storage is open on the server and drawn as the inventory's second pane.
    pub(crate) fn storage_is_open(&self) -> bool {
        self.client_state.follow(client_state().unified_inventory()).storage_open()
    }

    /// The cart pane is on screen: Cart tab picked and the window open.
    pub(crate) fn cart_pane_is_open(&self) -> bool {
        self.client_state.follow(client_state().unified_inventory()).cart_view()
            && self.interface.is_window_with_class_open(WindowClass::Inventory)
    }

    fn open_inventory_window(&mut self) {
        if !self.interface.is_window_with_class_open(WindowClass::Inventory) {
            self.interface.open_window(InventoryWindow::new(client_state().inventory()));
        }
        self.interface.raise_window_classes(&[WindowClass::Inventory]);
    }

    /// `SetStorage`: split the inventory with storage on the right.
    pub(crate) fn open_storage_pane(&mut self) {
        self.client_state.follow_mut(client_state().unified_inventory()).open_storage();
        self.open_inventory_window();
    }

    /// Storage is done. `tell_server` sends `CZ_CLOSE_STORE`; it is `false`
    /// when the server already closed it (`ZC_CLOSE_STORE`). The inventory
    /// stays open in its single view.
    pub(crate) fn close_storage_pane(&mut self, tell_server: bool) {
        let was_open = self.client_state.follow_mut(client_state().unified_inventory()).close_storage();
        if was_open && tell_server {
            let _ = self.networking_system.close_storage();
        }
    }

    pub(crate) fn note_cart_equipped(&mut self) {
        self.client_state
            .follow_mut(client_state().unified_inventory())
            .set_cart_equipped(true);
    }

    /// `ZC_CARTOFF`: no Cart tab, no cart list.
    pub(crate) fn note_cart_removed(&mut self) {
        self.client_state.follow_mut(client_state().cart()).clear_list();
        self.client_state
            .follow_mut(client_state().unified_inventory())
            .set_cart_equipped(false);
    }

    /// Alt+W and the Cart menu entry.
    pub(crate) fn toggle_cart_pane(&mut self) {
        let inventory_open = self.interface.is_window_with_class_open(WindowClass::Inventory);
        let step = cart_hotkey(self.client_state.follow(client_state().unified_inventory()), inventory_open);
        match step {
            CartHotkey::NoCart => {
                self.client_state
                    .follow_mut(client_state().chat_messages())
                    .push(ChatMessage::new(NO_CART_MESSAGE.to_owned(), MessageColor::Error));
            }
            CartHotkey::OpenOnCart | CartHotkey::ShowCart => {
                self.client_state.follow_mut(client_state().unified_inventory()).show_cart(true);
                self.open_inventory_window();
            }
            CartHotkey::Close => {
                self.client_state.follow_mut(client_state().unified_inventory()).show_cart(false);
                self.interface.close_window_with_class(WindowClass::Inventory);
            }
        }
    }

    /// Once per frame. Closing the inventory (X, Escape, hotkey) while storage
    /// is open closes storage on the server, and the tab and sort are kept
    /// per character.
    pub(crate) fn sync_unified_inventory(&mut self) {
        if self.storage_is_open() && !self.interface.is_window_with_class_open(WindowClass::Inventory) {
            self.close_storage_pane(true);
        }
        self.sync_view_prefs();
    }

    fn sync_view_prefs(&mut self) {
        let character = self.client_state.follow(client_state().player_name()).clone();
        let current = self.client_state.follow(client_state().inventory()).view().controls();
        let path = Path::new(VIEW_PREFS_FILE);
        let step = self
            .client_state
            .follow_mut(client_state().unified_inventory())
            .prefs_mut()
            .step(&character, current, || ViewPrefsFile::load_checked(path).unwrap_or_default());
        match step {
            PrefsStep::Nothing => {}
            PrefsStep::Apply(controls) => {
                self.client_state
                    .follow_mut(client_state().inventory())
                    .view_mut()
                    .apply_controls(controls);
            }
            PrefsStep::Save(character, controls) => {
                // An unreadable file is left alone rather than replaced.
                if let Ok(mut file) = ViewPrefsFile::load_checked(path) {
                    file.by_character.insert(character, controls);
                    let _ = file.save(path);
                }
            }
        }
    }
}
