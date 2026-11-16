//! Storage tabs, saved searches and deposit matching, headless (inventory
//! review #14, #18). The live check is
//! [`super::LogicSession::storage_deposit_plan_on_test`].

use std::time::Instant;

use ragnarok_packets::{EquipPosition, InventoryIndex};

use crate::e2e::inventory_view::{drawn, gear, regular};
use crate::state::inventory::Inventory;
use crate::state::inventory_view::{InventorySort, InventoryTab, item_type, set_favorite};
pub use crate::state::storage_tools::{DEPOSIT_INTERVAL, MAX_SAVED_SEARCHES, SavedSearchFile};
use crate::state::storage_tools::{StorageTools, deposit_plan};

/// (storage tab, search, planned (index, amount) moves).
pub type DepositCase = (InventoryTab, &'static str, Vec<(u16, u16)>);

/// What the offline run observed.
#[derive(Clone, Debug, PartialEq)]
pub struct OfflineStorageReport {
    /// Storage indexes per storage tab (All, Usable, Equip, Etc).
    pub storage_tabs: Vec<(InventoryTab, Vec<u16>)>,
    /// Inventory indexes "Deposit matching" queues per (tab, search).
    pub plans: Vec<DepositCase>,
    /// Indexes sent at 0 ms, 50 ms and one interval later.
    pub throttled: Vec<Option<u16>>,
    pub deposit_status: String,
    /// Saved searches for Alice and Bob after a save/load through a file.
    pub alice: Vec<String>,
    pub bob: Vec<String>,
    pub save_refused_duplicate: bool,
}

fn inventory() -> Inventory {
    let mut card = regular(7, 4001, item_type::CARD, 1, EquipPosition::empty(), "Poring Card");
    set_favorite(&mut card, true);
    let mut inventory = Inventory::from_items(vec![
        regular(2, 501, item_type::HEALING, 30, EquipPosition::empty(), "Red Potion"),
        gear(3, 1201, "Knife", true, [0; 4]),
        gear(4, 1202, "Knife", false, [4001, 0, 0, 0]),
        regular(5, 1750, 10, 500, EquipPosition::AMMO, "Arrow"),
        regular(6, 909, item_type::ETC, 12, EquipPosition::empty(), "Jellopy"),
        card,
        regular(8, 512, item_type::HEALING, 4, EquipPosition::empty(), "Apple"),
    ]);
    inventory.refresh_view(1, vec![(4001, Some(("Poring Card".to_owned(), 10)))]);
    inventory
}

pub fn offline_storage_tools(scratch: &std::path::Path) -> Result<OfflineStorageReport, String> {
    let mut storage = Inventory::from_items(vec![
        regular(0, 909, item_type::ETC, 200, EquipPosition::empty(), "Jellopy"),
        gear(1, 1202, "Knife", false, [0; 4]),
        regular(2, 501, item_type::HEALING, 5, EquipPosition::empty(), "Red Potion"),
    ]);
    let storage_tabs = [InventoryTab::All, InventoryTab::Usable, InventoryTab::Equip, InventoryTab::Etc]
        .into_iter()
        .map(|tab| (tab, drawn(&mut storage, tab, InventorySort::Server, 0)))
        .collect();

    let inventory = inventory();
    let items: Vec<_> = inventory.view().items().to_vec();
    let mut all_items = items.clone();
    // The view hides worn gear; the plan works on the raw list.
    all_items.insert(1, gear(3, 1201, "Knife", true, [0; 4]));
    let plans = [
        (InventoryTab::All, ""),
        (InventoryTab::Usable, ""),
        (InventoryTab::Etc, ""),
        (InventoryTab::All, "poring"),
    ]
    .into_iter()
    .map(|(tab, query)| {
        let plan = deposit_plan(&all_items, tab, query, |id| inventory.view().card_name(id))
            .into_iter()
            .map(|(index, amount)| (index.0, amount))
            .collect();
        (tab, query, plan)
    })
    .collect();

    let mut tools = StorageTools::default();
    tools.start_deposit(vec![(InventoryIndex(2), 30), (InventoryIndex(6), 12)]);
    let start = Instant::now();
    let throttled = [start, start + DEPOSIT_INTERVAL / 4, start + DEPOSIT_INTERVAL]
        .into_iter()
        .map(|now| tools.next_deposit(now).map(|(index, _)| index.0))
        .collect();
    let deposit_status = tools.status().to_owned();

    let path = scratch.join("storage_searches.ron");
    let mut alice = StorageTools::default();
    alice.load_for("Alice", &SavedSearchFile::load(&path));
    alice.save_search(" card ");
    alice.save_search("ore");
    let save_refused_duplicate = !alice.save_search("CARD");
    let mut file = SavedSearchFile::load(&path);
    alice.store_into(&mut file);
    file.save(&path)?;

    let reloaded = SavedSearchFile::load(&path);
    let mut bob = StorageTools::default();
    bob.load_for("Bob", &reloaded);
    let mut alice_again = StorageTools::default();
    alice_again.load_for("Alice", &reloaded);
    assert_eq!(alice_again.character(), "Alice");

    Ok(OfflineStorageReport {
        storage_tabs,
        plans,
        throttled,
        deposit_status,
        alice: alice_again.saved().to_vec(),
        bob: bob.saved().to_vec(),
        save_refused_duplicate,
    })
}
