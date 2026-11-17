//! Unified inventory flow (Matthew t275u). `lib.rs` forwards storage and
//! cart events here; the model is `state::unified_inventory`.

use std::path::Path;
use std::time::Instant;

use korangar_interface::Interface;
use korangar_networking::MessageColor;
use rust_state::State;

use crate::Client;
use crate::graphics::ScreenSize;
use crate::interface::components::unified_inventory::InventoryLayoutMetrics;
use crate::interface::windows::{INVENTORY_MAXIMUM_HEIGHT, InventoryWindow, WindowClass};
use crate::state::unified_inventory::{
    CartHotkey, FitStep, NO_CART_MESSAGE, PaneTab, PaneTabStep, PrefsStep, SPLIT_MINIMUM_ROWS, STORAGE_REQUEST_TIMEOUT, SecondPane,
    StorageKind, VIEW_PREFS_FILE, ViewPrefsFile, cart_hotkey,
};
use crate::state::{ChatMessage, ClientState, ClientStatePathExt, client_state, client_theme};

/// Space kept free above and below a grown window.
const SCREEN_MARGIN: f32 = 8.0;

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

    /// Map-server disconnect (also every character switch): drop the old
    /// character's cart and storage so a cartless character gets no Cart tab
    /// and Alt+W never shows the previous cart.
    pub(crate) fn reset_item_panes(&mut self) {
        self.client_state.follow_mut(client_state().unified_inventory()).reset_session();
        self.client_state.follow_mut(client_state().cart()).clear_list();
        self.client_state.follow_mut(client_state().storage()).clear_list();
    }

    /// See [`menu_class_open`].
    pub(crate) fn menu_class_open(&self, class: WindowClass) -> bool {
        menu_class_open(&self.interface, &self.client_state, class)
    }

    /// Inventory / Cart / Storage / Guild Storage tabs (t279u). Storage tabs
    /// send `@storage` / `@gstorage` like typed chat; the storage list that
    /// answers opens the split. A refusal ("Storage is available from base
    /// level 40.", "You are not in a guild.", "@storage failed.") arrives as
    /// a server message in chat, and the tab drops back after
    /// `STORAGE_REQUEST_TIMEOUT`.
    pub(crate) fn click_pane_tab(&mut self, tab: PaneTab) {
        let step = self
            .client_state
            .follow_mut(client_state().unified_inventory())
            .click_pane_tab(tab, Instant::now());
        match step {
            PaneTabStep::Nothing => {}
            PaneTabStep::CloseStorage => self.close_storage_pane(true),
            PaneTabStep::Request(kind) => self.send_storage_command(kind),
            PaneTabStep::CloseAndRequest(kind) => {
                self.close_storage_pane(true);
                self.send_storage_command(kind);
            }
        }
    }

    fn send_storage_command(&mut self, kind: StorageKind) {
        let player_name = self.client_state.follow(client_state().player_name()).clone();
        if self.networking_system.send_chat_message(&player_name, kind.command()).is_err() {
            self.client_state
                .follow_mut(client_state().unified_inventory())
                .expire_request(Instant::now() + STORAGE_REQUEST_TIMEOUT);
        }
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
            CartHotkey::ShowStorage => {
                self.client_state.follow_mut(client_state().unified_inventory()).show_cart(false);
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
        self.client_state
            .follow_mut(client_state().unified_inventory())
            .expire_request(Instant::now());
        self.sync_split_size();
        self.sync_view_prefs();
    }

    /// Grow the window when it splits so each half shows
    /// [`SPLIT_MINIMUM_ROWS`] (screen permitting), and give the height back
    /// when it joins again.
    fn sync_split_size(&mut self) {
        let Some(area) = self.interface.window_class_area(WindowClass::Inventory) else {
            self.client_state.follow_mut(client_state().unified_inventory()).fit_mut().forget();
            return;
        };
        let panes = self.client_state.follow(client_state().unified_inventory());
        let pane = panes.second_pane();
        let storage_tools = panes.storage_open();
        let metrics = InventoryLayoutMetrics::from_theme(self.client_state.follow(client_theme()));
        let needed = metrics.height_for_rows(SPLIT_MINIMUM_ROWS, area.width, true, storage_tools);
        let screen = self.interface.screen_size();
        let room = (screen.height - SCREEN_MARGIN * 2.0).min(INVENTORY_MAXIMUM_HEIGHT);
        let step = self
            .client_state
            .follow_mut(client_state().unified_inventory())
            .fit_mut()
            .step(pane, area.height, needed, room);
        if let FitStep::Resize(height) = step {
            self.interface
                .resize_window_class(WindowClass::Inventory, ScreenSize { width: area.width, height });
        }
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

/// Menu-bar highlight: Cart and Storage light while their pane shows in the
/// inventory window. A free function so the render pass can borrow the two
/// fields alone.
pub(crate) fn menu_class_open(interface: &Interface<'static, ClientState>, state: &State<ClientState>, class: WindowClass) -> bool {
    let inventory_open = || interface.is_window_with_class_open(WindowClass::Inventory);
    let panes = state.follow(client_state().unified_inventory());
    match class {
        WindowClass::Cart => panes.cart_view() && inventory_open(),
        WindowClass::Storage => panes.second_pane() == Some(SecondPane::Storage) && inventory_open(),
        _ => interface.is_window_with_class_open(class),
    }
}
