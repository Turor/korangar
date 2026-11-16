//! Storage saved searches and "deposit matching" (inventory review #14,
//! #18). `lib.rs` forwards the storage events here; the model is
//! `state::storage_tools`.

use std::path::Path;
use std::time::Instant;

use crate::Client;
use crate::interface::windows::WindowClass;
use crate::state::inventory::InventoryPathExt;
use crate::state::storage_tools::{SAVED_SEARCHES_FILE, SavedSearchFile, deposit_plan};
use crate::state::{ClientStatePathExt, client_state};

impl Client {
    /// Storage opened: pick this character's saved searches.
    pub(crate) fn load_storage_searches(&mut self) {
        let character = self.client_state.follow(client_state().player_name()).clone();
        let file = SavedSearchFile::load(Path::new(SAVED_SEARCHES_FILE));
        self.client_state
            .follow_mut(client_state().storage_tools())
            .load_for(&character, &file);
    }

    fn persist_storage_searches(&mut self) {
        let path = Path::new(SAVED_SEARCHES_FILE);
        let mut file = SavedSearchFile::load(path);
        self.client_state.follow(client_state().storage_tools()).store_into(&mut file);
        let _ = file.save(path);
    }

    /// Saves the storage search box as a chip.
    pub(crate) fn save_storage_search(&mut self) {
        let query = self.client_state.follow(client_state().storage()).view().search().to_owned();
        if self.client_state.follow_mut(client_state().storage_tools()).save_search(&query) {
            self.persist_storage_searches();
        }
    }

    pub(crate) fn forget_storage_search(&mut self, query: &str) {
        if self.client_state.follow_mut(client_state().storage_tools()).forget_search(query) {
            self.persist_storage_searches();
        }
    }

    /// Queues every inventory stack the storage tab and search would show.
    pub(crate) fn start_storage_deposit(&mut self) {
        let plan = {
            let storage = self.client_state.follow(client_state().storage());
            let inventory = self.client_state.follow(client_state().inventory());
            let view = inventory.view();
            deposit_plan(
                self.client_state.follow(client_state().inventory().items()),
                storage.view().tab(),
                storage.view().search(),
                |id| view.card_name(id),
            )
        };
        self.client_state.follow_mut(client_state().storage_tools()).start_deposit(plan);
    }

    /// Sends the next queued deposit when it is due. Closing storage stops
    /// the queue.
    pub(crate) fn tick_storage_deposit(&mut self) {
        if !self.client_state.follow(client_state().storage_tools()).is_depositing() {
            return;
        }
        if !self.interface.is_window_with_class_open(WindowClass::Storage) {
            self.client_state.follow_mut(client_state().storage_tools()).cancel_deposit();
            return;
        }
        let next = self
            .client_state
            .follow_mut(client_state().storage_tools())
            .next_deposit(Instant::now());
        if let Some((index, amount)) = next {
            let _ = self.networking_system.move_item_to_storage(index, u32::from(amount));
        }
    }
}
