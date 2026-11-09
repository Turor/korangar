use korangar_interface::element::StateElement;
use korangar_networking::{EquipSwitchSlot, InventoryItem, assign_slot, replace_slots};
use ragnarok_packets::{EquipPosition, EquippableSwitchItemInformation, InventoryIndex};
use rust_state::RustState;

use crate::world::ResourceMetadata;

/// One named snapshot of the alternate equipment set.
#[derive(Clone, Default, RustState, StateElement)]
pub struct EquipLoadout {
    name: String,
    slots: Vec<EquipSwitchSlot>,
}

impl EquipLoadout {
    pub fn slots(&self) -> &[EquipSwitchSlot] {
        &self.slots
    }
}

/// Alternate equipment set plus locally named copies of that set.
///
/// Worn equipment stays on [`Inventory`](super::inventory::Inventory). The
/// server still has a single switch set (`slots`). `loadouts` are client-side
/// snapshots that Gear can write back with the switch add/remove packets.
#[derive(Default, RustState, StateElement)]
pub struct EquipSwitch {
    slots: Vec<EquipSwitchSlot>,
    loadouts: Vec<EquipLoadout>,
    draft_name: String,
    selected: usize,
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

    pub fn slots(&self) -> &[EquipSwitchSlot] {
        &self.slots
    }

    /// Remember which saved set the Load button applies.
    pub fn select(&mut self, index: usize) {
        if index < self.loadouts.len() {
            self.selected = index;
            self.draft_name.clone_from(&self.loadouts[index].name);
        }
    }

    /// Store the live switch set under the draft name, or `Set N` when blank.
    /// A matching name is replaced.
    pub fn save_loadout(&mut self) {
        let name = {
            let trimmed = self.draft_name.trim();
            if trimmed.is_empty() {
                format!("Set {}", self.loadouts.len() + 1)
            } else {
                trimmed.to_string()
            }
        };
        if let Some(existing) = self.loadouts.iter_mut().find(|set| set.name == name) {
            existing.slots.clone_from(&self.slots);
        } else {
            self.loadouts.push(EquipLoadout {
                name,
                slots: self.slots.clone(),
            });
        }
    }

    /// Rename the selected set from the draft. Blank drafts are ignored.
    pub fn rename_selected(&mut self) {
        let trimmed = self.draft_name.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(set) = self.loadouts.get_mut(self.selected) {
            set.name = trimmed.to_string();
        }
    }

    pub fn loadout(&self, index: usize) -> Option<&EquipLoadout> {
        self.loadouts.get(index)
    }
}

/// Packets required to make the live switch set match `desired`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchDelta {
    pub remove: Vec<InventoryIndex>,
    pub add: Vec<(InventoryIndex, EquipPosition)>,
}

pub fn switch_delta(current: &[EquipSwitchSlot], desired: &[EquipSwitchSlot]) -> SwitchDelta {
    let mut remove = Vec::new();
    let mut add = Vec::new();
    for slot in current {
        let unchanged = desired.iter().any(|other| other.index == slot.index && other.position == slot.position);
        if !unchanged {
            remove.push(slot.index);
        }
    }
    for slot in desired {
        let unchanged = current.iter().any(|other| other.index == slot.index && other.position == slot.position);
        if !unchanged {
            add.push((slot.index, slot.position));
        }
    }
    SwitchDelta { remove, add }
}

fn item_name(items: &[InventoryItem<ResourceMetadata>], index: InventoryIndex) -> Option<String> {
    items.iter().find(|item| item.index == index).map(|item| item.metadata.name.clone())
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{EquipPosition, InventoryIndex};

    use korangar_networking::EquipSwitchSlot;

    use super::{EquipSwitch, switch_delta};

    fn slot(index: u16, position: EquipPosition) -> EquipSwitchSlot {
        EquipSwitchSlot {
            index: InventoryIndex(index),
            position,
            label: format!("{index}"),
        }
    }

    #[test]
    fn save_rename_and_replace_a_named_set() {
        let mut equip = EquipSwitch {
            slots: vec![slot(3, EquipPosition::ARMOR)],
            ..EquipSwitch::default()
        };
        equip.save_loadout();
        assert_eq!(equip.loadouts[0].name, "Set 1");
        assert_eq!(equip.loadouts[0].slots.len(), 1);

        equip.select(0);
        equip.draft_name = "Town".to_string();
        equip.rename_selected();
        assert_eq!(equip.loadouts[0].name, "Town");

        equip.slots = vec![slot(9, EquipPosition::SHOES)];
        equip.draft_name = "Town".to_string();
        equip.save_loadout();
        assert_eq!(equip.loadouts.len(), 1);
        assert_eq!(equip.loadouts[0].slots[0].index, InventoryIndex(9));
    }

    #[test]
    fn switch_delta_removes_extras_and_adds_missing() {
        let current = vec![slot(1, EquipPosition::ARMOR), slot(2, EquipPosition::RIGHT_HAND)];
        let desired = vec![slot(2, EquipPosition::LEFT_HAND), slot(4, EquipPosition::SHOES)];
        let delta = switch_delta(&current, &desired);
        assert_eq!(delta.remove, vec![InventoryIndex(1), InventoryIndex(2)]);
        assert_eq!(
            delta.add,
            vec![
                (InventoryIndex(2), EquipPosition::LEFT_HAND),
                (InventoryIndex(4), EquipPosition::SHOES),
            ]
        );
    }
}


