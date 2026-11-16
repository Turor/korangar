//! Live tooltip check on Hercules TEST: log in, read the real inventory, and
//! build a tooltip for every row. Nothing is equipped or moved.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use korangar_networking::{InventoryItem, InventoryItemDetails, NetworkEvent, NoMetadata};
use ragnarok_packets::{EquipPosition, InventoryIndex};

use super::super::LogicSession;
use crate::state::item_compare::{LineRole, compare_targets, item_tooltip_lines, tooltip_plain};
use crate::state::item_finder::ItemFinderState;
use crate::world::ResourceMetadata;

/// What the live check saw. Names stay in the sample compare only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveItemTooltipReport {
    pub inventory_seen: bool,
    pub rows: usize,
    pub built: usize,
    pub worn: usize,
    pub compare_pairs: usize,
    /// Worn name and the compare block, when an unworn equip overlaps a worn
    /// slot.
    pub sample_compare: Option<(String, String)>,
    pub note: String,
}

impl LogicSession {
    /// Log in, enter the map, and build tooltips for the TEST character's
    /// inventory.
    pub fn item_tooltips_on_test(&mut self) -> Result<LiveItemTooltipReport, String> {
        self.login_to_character_select()?;
        self.enter_world()?;

        let mut inventory: Vec<InventoryItem<NoMetadata>> = Vec::new();
        let mut seen = false;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !seen {
            let drained = {
                let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) else {
                    return Err("logic mode has no network".to_owned());
                };
                net.get_events(events);
                events.drain().collect::<Vec<_>>()
            };
            for event in drained {
                match event {
                    NetworkEvent::SetInventory { items } => {
                        inventory = items;
                        seen = true;
                    }
                    NetworkEvent::IventoryItemAdded { item } => add_item(&mut inventory, item),
                    NetworkEvent::InventoryItemRemoved { index, amount, .. } => remove_item(&mut inventory, index, amount),
                    NetworkEvent::UpdateEquippedPosition { index, equipped_position } => {
                        set_equipped(&mut inventory, index, equipped_position);
                    }
                    other => self.on_event(other)?,
                }
            }
            self.take_failure()?;
            std::thread::sleep(Duration::from_millis(10));
        }
        if !seen {
            return Err("inventory never arrived".to_owned());
        }

        let mut finder = ItemFinderState::default();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("archive/System/itemfinder.turoran.v8.json");
        let catalog_loaded = match std::fs::read(&path) {
            Ok(bytes) => {
                finder.load_bytes(Some(&bytes), None);
                !finder.catalog_is_empty()
            }
            Err(_) => false,
        };
        let catalog = finder.tooltip_catalog();
        let presented: Vec<InventoryItem<ResourceMetadata>> = inventory.iter().cloned().map(|item| present(item, catalog)).collect();

        let mut report = LiveItemTooltipReport {
            inventory_seen: true,
            rows: presented.len(),
            worn: presented.iter().filter(|item| equipped_bits(item) != 0).count(),
            note: if presented.is_empty() {
                "inventory empty".to_owned()
            } else if catalog_loaded {
                String::new()
            } else {
                "item finder catalog was not in the archive; names fell back to item ids".to_owned()
            },
            ..LiveItemTooltipReport::default()
        };

        for item in &presented {
            let lines = item_tooltip_lines(item, catalog, &presented);
            let body = tooltip_plain(&lines, LineRole::Item);
            if body.is_empty() {
                return Err(format!("item {} produced no tooltip", item.item_id.0));
            }
            report.built += 1;
            let targets = compare_targets(item, &presented);
            if targets.is_empty() {
                continue;
            }
            report.compare_pairs += 1;
            if report.sample_compare.is_none() {
                let worn_name = targets[0].metadata.name.clone();
                report.sample_compare = Some((worn_name, tooltip_plain(&lines, LineRole::Compare)));
            }
        }
        if report.compare_pairs == 0 && report.note.is_empty() {
            report.note = "no unworn equippable shares a slot with worn gear".to_owned();
        }
        Ok(report)
    }
}

fn present(item: InventoryItem<NoMetadata>, catalog: &crate::state::item_compare::ItemTooltipCatalog) -> InventoryItem<ResourceMetadata> {
    let identified = item.is_identified();
    let facts = catalog.get(item.item_id.0);
    let name = if !identified {
        "Unknown item".to_owned()
    } else if let Some(facts) = facts.filter(|facts| !facts.name.is_empty()) {
        facts.name.clone()
    } else {
        format!("Item {}", item.item_id.0)
    };
    InventoryItem {
        metadata: ResourceMetadata {
            texture: None,
            name,
            description: String::new(),
        },
        index: item.index,
        item_id: item.item_id,
        item_type: item.item_type,
        slot: item.slot,
        hire_expiration_date: item.hire_expiration_date,
        details: item.details,
    }
}

fn equipped_bits(item: &InventoryItem<ResourceMetadata>) -> u32 {
    match &item.details {
        InventoryItemDetails::Equippable { equipped_position, .. } => equipped_position.bits(),
        InventoryItemDetails::Regular { equipped_position, .. } => equipped_position.bits(),
    }
}

fn add_item(items: &mut Vec<InventoryItem<NoMetadata>>, item: InventoryItem<NoMetadata>) {
    if let Some(found) = items.iter_mut().find(|row| row.index == item.index) {
        if let (InventoryItemDetails::Regular { amount, .. }, InventoryItemDetails::Regular { amount: added, .. }) =
            (&mut found.details, &item.details)
        {
            *amount = amount.saturating_add(*added);
        }
        return;
    }
    items.push(item);
}

fn remove_item(items: &mut Vec<InventoryItem<NoMetadata>>, index: InventoryIndex, remove_amount: u16) {
    let Some(position) = items.iter().position(|item| item.index == index) else {
        return;
    };
    if let InventoryItemDetails::Regular { amount, .. } = &mut items[position].details
        && *amount > remove_amount
    {
        *amount -= remove_amount;
        return;
    }
    items.remove(position);
}

fn set_equipped(items: &mut [InventoryItem<NoMetadata>], index: InventoryIndex, position: EquipPosition) {
    let Some(item) = items.iter_mut().find(|item| item.index == index) else {
        return;
    };
    if let InventoryItemDetails::Equippable { equipped_position, .. } = &mut item.details {
        *equipped_position = position;
    }
}
