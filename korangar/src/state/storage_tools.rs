//! Storage tabs, saved searches and "deposit matching" (inventory review
//! #14, #18).
//!
//! Storage tabs reuse the grid view (`state::inventory_view`) on the storage
//! list; the server's storage order is untouched. Saved searches are local,
//! per character, in `client/storage_searches.ron`. Deposit matching sends
//! one `CZ_MOVE_ITEM_FROM_BODY_TO_STORE` per stack, spaced by
//! [`DEPOSIT_INTERVAL`], for every inventory stack the storage tab and
//! search would show. Worn gear, equipped ammo and favorites never move.

use std::collections::{BTreeMap, VecDeque};
use std::path::Path as FilePath;
use std::time::{Duration, Instant};

use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::InventoryIndex;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

use crate::state::inventory_view::{InventoryTab, is_favorite, normalize_query, search_matches, type_tab};
use crate::world::ResourceMetadata;

/// Saved searches kept per character.
pub const MAX_SAVED_SEARCHES: usize = 8;
/// Gap between two deposit requests. Hercules handles each move on its own;
/// the spacing keeps a 100-stack deposit from flooding the map server.
pub const DEPOSIT_INTERVAL: Duration = Duration::from_millis(200);
pub const SAVED_SEARCHES_FILE: &str = "client/storage_searches.ron";

/// On-disk form: character name to saved queries.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSearchFile {
    pub by_character: BTreeMap<String, Vec<String>>,
}

impl SavedSearchFile {
    pub fn load(path: &FilePath) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| ron::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &FilePath) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|error| error.to_string())?;
        std::fs::write(path, text).map_err(|error| error.to_string())
    }
}

/// Adds `query` (trimmed) to `saved`. Empty queries, case-insensitive
/// duplicates and a full list are refused.
pub fn add_saved_search(saved: &mut Vec<String>, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() || saved.len() >= MAX_SAVED_SEARCHES || saved.iter().any(|known| known.eq_ignore_ascii_case(query)) {
        return false;
    }
    saved.push(query.to_owned());
    true
}

pub fn remove_saved_search(saved: &mut Vec<String>, query: &str) -> bool {
    let before = saved.len();
    saved.retain(|known| !known.eq_ignore_ascii_case(query.trim()));
    saved.len() != before
}

/// Never deposited: worn gear, equipped ammo and favorites.
pub fn is_protected<Meta>(item: &InventoryItem<Meta>) -> bool {
    let equipped = match &item.details {
        InventoryItemDetails::Regular { equipped_position, .. } => !equipped_position.is_empty(),
        InventoryItemDetails::Equippable { equipped_position, .. } => !equipped_position.is_empty(),
    };
    equipped || is_favorite(item)
}

/// Inventory stacks the storage `tab` and `query` would show, in server
/// order, with the whole stack amount.
pub fn deposit_plan<'a>(
    inventory: &[InventoryItem<ResourceMetadata>],
    tab: InventoryTab,
    query: &str,
    card_name: impl Fn(u32) -> Option<&'a str> + Copy,
) -> Vec<(InventoryIndex, u16)> {
    let query = normalize_query(query);
    let mut plan: Vec<(InventoryIndex, u16)> = inventory
        .iter()
        .filter(|item| !is_protected(item))
        .filter(|item| matches!(tab, InventoryTab::All | InventoryTab::Favorites) || type_tab(item.item_type) == tab)
        .filter(|item| search_matches(item, &query, card_name))
        .map(|item| {
            let amount = match &item.details {
                InventoryItemDetails::Regular { amount, .. } => *amount,
                InventoryItemDetails::Equippable { .. } => 1,
            };
            (item.index, amount)
        })
        .filter(|(_, amount)| *amount > 0)
        .collect();
    plan.sort_by_key(|(index, _)| index.0);
    plan
}

/// Saved searches for the character in storage, and the running deposit.
#[derive(Default, RustState)]
pub struct StorageTools {
    character: String,
    saved: Vec<String>,
    pending: VecDeque<(InventoryIndex, u16)>,
    planned: usize,
    last_sent: Option<Instant>,
    status: String,
}

impl StorageTools {
    pub fn saved(&self) -> &[String] {
        &self.saved
    }

    pub fn character(&self) -> &str {
        &self.character
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    /// Picks the saved list for `character` from the file.
    pub fn load_for(&mut self, character: &str, file: &SavedSearchFile) {
        self.character = character.to_owned();
        self.saved = file.by_character.get(character).cloned().unwrap_or_default();
    }

    /// Writes the current list back into `file`.
    pub fn store_into(&self, file: &mut SavedSearchFile) {
        match self.saved.is_empty() {
            true => file.by_character.remove(&self.character),
            false => file.by_character.insert(self.character.clone(), self.saved.clone()),
        };
    }

    pub fn save_search(&mut self, query: &str) -> bool {
        add_saved_search(&mut self.saved, query)
    }

    pub fn forget_search(&mut self, query: &str) -> bool {
        remove_saved_search(&mut self.saved, query)
    }

    pub fn is_depositing(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn start_deposit(&mut self, plan: Vec<(InventoryIndex, u16)>) {
        self.planned = plan.len();
        self.pending = plan.into();
        self.last_sent = None;
        self.status = match self.planned {
            0 => "Nothing to deposit".to_owned(),
            planned => format!("Depositing 0 / {planned}"),
        };
    }

    pub fn cancel_deposit(&mut self) {
        if self.is_depositing() {
            self.status = format!("Deposit stopped at {} / {}", self.planned - self.pending.len(), self.planned);
        }
        self.pending.clear();
    }

    /// The next move to send at `now`, at most one per [`DEPOSIT_INTERVAL`].
    pub fn next_deposit(&mut self, now: Instant) -> Option<(InventoryIndex, u16)> {
        if self.last_sent.is_some_and(|last| now.duration_since(last) < DEPOSIT_INTERVAL) {
            return None;
        }
        let next = self.pending.pop_front()?;
        self.last_sent = Some(now);
        let done = self.planned - self.pending.len();
        self.status = match self.pending.is_empty() {
            true => format!("Deposited {done} stacks"),
            false => format!("Depositing {done} / {}", self.planned),
        };
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_searches_trim_dedup_and_cap() {
        let mut saved = Vec::new();
        assert!(add_saved_search(&mut saved, "  card "));
        assert!(!add_saved_search(&mut saved, "CARD"));
        assert!(!add_saved_search(&mut saved, "   "));
        for index in 1..MAX_SAVED_SEARCHES {
            assert!(add_saved_search(&mut saved, &format!("q{index}")));
        }
        assert!(!add_saved_search(&mut saved, "one more"));
        assert_eq!(saved[0], "card");
        assert!(remove_saved_search(&mut saved, "Card"));
        assert_eq!(saved.len(), MAX_SAVED_SEARCHES - 1);
    }

    #[test]
    fn deposits_are_spaced() {
        let mut tools = StorageTools::default();
        tools.start_deposit(vec![(InventoryIndex(2), 5), (InventoryIndex(4), 1)]);
        let start = Instant::now();
        assert_eq!(tools.next_deposit(start), Some((InventoryIndex(2), 5)));
        assert_eq!(tools.next_deposit(start + Duration::from_millis(50)), None);
        assert_eq!(tools.next_deposit(start + DEPOSIT_INTERVAL), Some((InventoryIndex(4), 1)));
        assert_eq!(tools.status(), "Deposited 2 stacks");
        assert!(!tools.is_depositing());
    }

    #[test]
    fn per_character_lists_round_trip() {
        let mut file = SavedSearchFile::default();
        let mut tools = StorageTools::default();
        tools.load_for("Alice", &file);
        tools.save_search("ore");
        tools.store_into(&mut file);
        tools.load_for("Bob", &file);
        assert!(tools.saved().is_empty());
        tools.load_for("Alice", &file);
        assert_eq!(tools.saved(), ["ore"]);
    }
}
