//! Storage saved searches and "deposit matching" (inventory review #14,
//! #18). `lib.rs` forwards the storage events here; the model is
//! `state::storage_tools`.

use std::path::Path;
use std::time::Instant;

use crate::Client;
use crate::state::inventory::InventoryPathExt;
use crate::state::storage_tools::{SAVED_SEARCHES_FILE, SavedSearchFile, deposit_plan, live_deposit_amount};
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
        // An unreadable or invalid file is left alone rather than replaced
        // with this character's list only.
        let Ok(mut file) = SavedSearchFile::load_checked(path) else {
            return;
        };
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
                inventory.equipped_ammo(),
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
        if !self.storage_is_open() {
            self.client_state.follow_mut(client_state().storage_tools()).cancel_deposit();
            return;
        }
        let now = Instant::now();
        // Skipped entries cost no send, so the next one goes out this frame.
        while let Some(index) = self.client_state.follow_mut(client_state().storage_tools()).pop_due(now) {
            let amount = {
                let inventory = self.client_state.follow(client_state().inventory());
                live_deposit_amount(
                    self.client_state.follow(client_state().inventory().items()),
                    index,
                    inventory.equipped_ammo(),
                )
            };
            let tools = client_state().storage_tools();
            match amount {
                None => self.client_state.follow_mut(tools).mark_skipped(),
                Some(amount) => {
                    match self.networking_system.move_item_to_storage(index, u32::from(amount)) {
                        Ok(()) => self.client_state.follow_mut(tools).mark_sent(now),
                        Err(_) => self.client_state.follow_mut(tools).fail("not connected"),
                    }
                    return;
                }
            }
        }
    }
}
