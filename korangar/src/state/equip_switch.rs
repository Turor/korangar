use korangar_interface::element::StateElement;
use korangar_networking::{EquipSwitchSlot, InventoryItem, assign_slot, replace_slots};
use ragnarok_packets::{EquipPosition, EquippableSwitchItemInformation, InventoryIndex};
use rust_state::RustState;

use crate::world::ResourceMetadata;

/// Alternate equipment set. Worn equipment stays on
/// [`Inventory`](super::inventory::Inventory).
#[derive(Default, RustState, StateElement)]
pub struct EquipSwitch {
    slots: Vec<EquipSwitchSlot>,
}

impl EquipSwitch {
    pub fn replace(&mut self, entries: Vec<EquippableSwitchItemInformation>, items: &[InventoryItem<ResourceMetadata>]) {
        self.slots = replace_slots(entries, |index| item_name(items, index));
    }

    pub fn assign(&mut self, index: InventoryIndex, position: EquipPosition, items: &[InventoryItem<ResourceMetadata>]) {
        assign_slot(&mut self.slots, index, position, item_name(items, index));
    }

    pub fn clear(&mut self) {
        self.slots.clear();
    }
}

fn item_name(items: &[InventoryItem<ResourceMetadata>], index: InventoryIndex) -> Option<String> {
    items.iter().find(|item| item.index == index).map(|item| item.metadata.name.clone())
}
