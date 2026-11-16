//! Storage tabs, saved searches and "deposit matching" (inventory review
//! #14, #18).
//!
//! Storage tabs reuse the grid view (`state::inventory_view`) on the storage
//! list; the server's storage order is untouched. Saved searches are local,
//! per character, in `client/storage_searches.ron`. Deposit matching sends
//! one `CZ_MOVE_ITEM_FROM_BODY_TO_STORE` per stack, spaced by
//! [`DEPOSIT_INTERVAL`], for every inventory stack the storage tab and
//! search would show. Worn gear, the equipped ammo stack and favorites never
//! move: the queue holds indexes only and each one is checked against the
//! live inventory right before it is sent.

use std::collections::{BTreeMap, VecDeque};
use std::path::Path as FilePath;
use std::time::{Duration, Instant};

use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::InventoryIndex;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

use crate::state::inventory_view::{InventoryTab, is_favorite, is_worn, normalize_query, search_matches, type_tab};
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
    /// Missing file is empty; an unreadable or invalid file is an error so
    /// a later save does not overwrite other characters' lists.
    pub fn load_checked(path: &FilePath) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => ron::from_str(&text).map_err(|error| error.to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn load(path: &FilePath) -> Self {
        Self::load_checked(path).unwrap_or_default()
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

/// Never deposited: worn gear, the equipped ammo stack and favorites.
///
/// A regular row's `equipped_position` is the item's equip slot from the
/// item database (`WearState`), not whether it is worn, so ammo is only
/// protected when its index is the one `ZC_EQUIP_ARROW` named.
pub fn is_protected<Meta>(item: &InventoryItem<Meta>, equipped_ammo: Option<InventoryIndex>) -> bool {
    is_worn(item) || is_favorite(item) || equipped_ammo == Some(item.index)
}

fn stack_amount<Meta>(item: &InventoryItem<Meta>) -> u16 {
    match &item.details {
        InventoryItemDetails::Regular { amount, .. } => *amount,
        InventoryItemDetails::Equippable { .. } => 1,
    }
}

/// Inventory stacks the storage `tab` and `query` would show, in server
/// order. The Favorites tab plans nothing: favorites never move.
pub fn deposit_plan<'a>(
    inventory: &[InventoryItem<ResourceMetadata>],
    equipped_ammo: Option<InventoryIndex>,
    tab: InventoryTab,
    query: &str,
    card_name: impl Fn(u32) -> Option<&'a str> + Copy,
) -> Vec<InventoryIndex> {
    if tab == InventoryTab::Favorites {
        return Vec::new();
    }
    let query = normalize_query(query);
    let mut plan: Vec<InventoryIndex> = inventory
        .iter()
        .filter(|item| !is_protected(item, equipped_ammo))
        .filter(|item| tab == InventoryTab::All || type_tab(item.item_type) == tab)
        .filter(|item| search_matches(item, &query, card_name))
        .filter(|item| stack_amount(item) > 0)
        .map(|item| item.index)
        .collect();
    plan.sort_by_key(|index| index.0);
    plan
}

/// Whole-stack amount to send for `index` now, or `None` when the row is
/// gone, became worn, favorite or the equipped ammo since it was queued.
pub fn live_deposit_amount<Meta>(
    inventory: &[InventoryItem<Meta>],
    index: InventoryIndex,
    equipped_ammo: Option<InventoryIndex>,
) -> Option<u16> {
    let item = inventory.iter().find(|item| item.index == index)?;
    let amount = stack_amount(item);
    (!is_protected(item, equipped_ammo) && amount > 0).then_some(amount)
}

/// Saved searches for the character in storage, and the running deposit.
#[derive(Default, RustState)]
pub struct StorageTools {
    character: String,
    saved: Vec<String>,
    pending: VecDeque<InventoryIndex>,
    planned: usize,
    sent: usize,
    skipped: usize,
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

    /// Picks the saved list for `character` from the file, at most
    /// [`MAX_SAVED_SEARCHES`] entries.
    pub fn load_for(&mut self, character: &str, file: &SavedSearchFile) {
        self.character = character.to_owned();
        self.saved = file.by_character.get(character).cloned().unwrap_or_default();
        self.saved.truncate(MAX_SAVED_SEARCHES);
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

    pub fn start_deposit(&mut self, plan: Vec<InventoryIndex>) {
        self.planned = plan.len();
        self.pending = plan.into();
        self.sent = 0;
        self.skipped = 0;
        self.last_sent = None;
        self.status = match self.planned {
            0 => "Nothing to deposit".to_owned(),
            planned => format!("Depositing 0 / {planned}"),
        };
    }

    pub fn cancel_deposit(&mut self) {
        if self.is_depositing() {
            self.status = format!("Deposit stopped after {} of {} stacks", self.sent, self.planned);
        }
        self.pending.clear();
    }

    /// The next queued index once [`DEPOSIT_INTERVAL`] has passed since the
    /// last send. Report the outcome with [`Self::mark_sent`],
    /// [`Self::mark_skipped`] or [`Self::fail`].
    pub fn pop_due(&mut self, now: Instant) -> Option<InventoryIndex> {
        if self.last_sent.is_some_and(|last| now.duration_since(last) < DEPOSIT_INTERVAL) {
            return None;
        }
        self.pending.pop_front()
    }

    pub fn mark_sent(&mut self, now: Instant) {
        self.sent += 1;
        self.last_sent = Some(now);
        self.update_status();
    }

    pub fn mark_skipped(&mut self) {
        self.skipped += 1;
        self.update_status();
    }

    /// The send failed: stop and say how far it got.
    pub fn fail(&mut self, reason: &str) {
        self.pending.clear();
        self.status = format!("Deposit failed after {} of {} stacks: {reason}", self.sent, self.planned);
    }

    fn update_status(&mut self) {
        let done = self.sent + self.skipped;
        self.status = match (self.pending.is_empty(), self.skipped) {
            (false, _) => format!("Depositing {done} / {}", self.planned),
            (true, 0) => format!("Deposited {} stacks", self.sent),
            (true, skipped) => format!("Deposited {} stacks, {skipped} skipped", self.sent),
        };
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
    fn deposits_are_spaced_and_counted() {
        let mut tools = StorageTools::default();
        tools.start_deposit(vec![InventoryIndex(2), InventoryIndex(3), InventoryIndex(4)]);
        let start = Instant::now();
        assert_eq!(tools.pop_due(start), Some(InventoryIndex(2)));
        tools.mark_sent(start);
        assert_eq!(tools.pop_due(start + Duration::from_millis(50)), None);
        assert_eq!(tools.pop_due(start + DEPOSIT_INTERVAL), Some(InventoryIndex(3)));
        tools.mark_skipped();
        assert_eq!(tools.pop_due(start + DEPOSIT_INTERVAL), Some(InventoryIndex(4)));
        tools.mark_sent(start + DEPOSIT_INTERVAL);
        assert_eq!(tools.status(), "Deposited 2 stacks, 1 skipped");
        assert!(!tools.is_depositing());
    }

    #[test]
    fn a_failed_send_stops_the_queue() {
        let mut tools = StorageTools::default();
        tools.start_deposit(vec![InventoryIndex(2), InventoryIndex(3)]);
        let _ = tools.pop_due(Instant::now());
        tools.fail("not connected");
        assert!(!tools.is_depositing());
        assert_eq!(tools.status(), "Deposit failed after 0 of 2 stacks: not connected");
    }

    #[test]
    fn per_character_lists_round_trip_and_clamp() {
        let mut file = SavedSearchFile::default();
        let mut tools = StorageTools::default();
        tools.load_for("Alice", &file);
        tools.save_search("ore");
        tools.store_into(&mut file);
        tools.load_for("Bob", &file);
        assert!(tools.saved().is_empty());
        tools.load_for("Alice", &file);
        assert_eq!(tools.saved(), ["ore"]);

        file.by_character
            .insert("Carol".to_owned(), (0..12).map(|index| format!("q{index}")).collect());
        tools.load_for("Carol", &file);
        assert_eq!(tools.saved().len(), MAX_SAVED_SEARCHES);
    }

    #[test]
    fn corrupt_files_are_errors_and_missing_files_are_empty() {
        let dir = std::env::temp_dir().join(format!("korangar-storage-tools-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("storage_searches.ron");
        assert_eq!(SavedSearchFile::load_checked(&path), Ok(SavedSearchFile::default()));
        std::fs::write(&path, "not ron {").unwrap();
        assert!(SavedSearchFile::load_checked(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
