use std::sync::Arc;

use korangar_interface::element::StateElement;
use korangar_networking::{InventoryItem, InventoryItemDetails, NoMetadata};
use ragnarok_packets::{EquipPosition, InventoryIndex, ItemId};
use rust_state::RustState;

use crate::graphics::Texture;
use crate::loaders::AsyncLoader;
use crate::world::ResourceMetadata;

#[derive(Default, RustState, StateElement)]
pub struct Inventory {
    // TODO: Unhide this.
    #[hidden_element]
    items: Vec<InventoryItem<ResourceMetadata>>,
}

impl Inventory {
    pub fn fill(&mut self, async_loader: &AsyncLoader, items: Vec<InventoryItem<NoMetadata>>) {
        self.items = items
            .into_iter()
            .map(|item| async_loader.request_inventory_item_metadata_load(item))
            .collect();
    }

    pub fn add_item(&mut self, async_loader: &AsyncLoader, item: InventoryItem<NoMetadata>) {
        if let Some(found_item) = self.items.iter_mut().find(|inventory_item| inventory_item.index == item.index) {
            let InventoryItemDetails::Regular { amount, .. } = &mut found_item.details else {
                panic!();
            };

            let InventoryItemDetails::Regular { amount: added_amount, .. } = item.details else {
                panic!();
            };

            *amount += added_amount;
        } else {
            let item = async_loader.request_inventory_item_metadata_load(item);

            self.items.push(item);
        }
    }

    pub fn update_item_sprite(&mut self, item_id: ItemId, texture: Arc<Texture>) {
        self.items.iter_mut().filter(|item| item.item_id == item_id).for_each(|item| {
            item.metadata.texture = Some(texture.clone());
        });
    }

    pub fn remove_item(&mut self, index: InventoryIndex, remove_amount: u16) {
        let position = self
            .items
            .iter()
            .position(|item| item.index == index)
            .expect("item not in inventory");

        if let InventoryItemDetails::Regular { amount, .. } = &mut self.items[position].details
            && *amount > remove_amount
        {
            *amount -= remove_amount;
            return;
        }

        self.items.remove(position);
    }

    pub fn set_refinement(&mut self, index: InventoryIndex, refine_level: u8) {
        let Some(item) = self.items.iter_mut().find(|item| item.index == index) else {
            return;
        };
        let InventoryItemDetails::Equippable { refinement_level, .. } = &mut item.details else {
            return;
        };
        *refinement_level = refine_level;
    }

    /// Apply `ZC_GRADE_ENCHANT_ACK` to the local equip.
    ///
    /// rAthena writes the new grade in the ack and does not send a follow-up
    /// item packet. Result 0 also forces refine to 0. Result 3 is removed by
    /// the existing delete-item packet, so this leaves the stack alone.
    /// Result 2 lowers refine by a server-side `downgrade_amount` that is not
    /// on the wire; refine stays stale until the next full inventory list.
    /// The grade field is still written.
    pub fn apply_enchant_grade(&mut self, index: InventoryIndex, grade: i16, result: i32) {
        if result == 3 {
            return;
        }
        let Some(item) = self.items.iter_mut().find(|item| item.index == index) else {
            return;
        };
        let InventoryItemDetails::Equippable {
            refinement_level,
            enchantment_level,
            ..
        } = &mut item.details
        else {
            return;
        };
        *enchantment_level = u8::try_from(grade).unwrap_or(0);
        if result == 0 {
            *refinement_level = 0;
        }
    }

    pub fn update_equipped_position(&mut self, index: InventoryIndex, new_equipped_position: EquipPosition) {
        let item = self.items.iter_mut().find(|item| item.index == index).unwrap();

        let InventoryItemDetails::Equippable { equipped_position, .. } = &mut item.details else {
            // This can happen for ammunition for example.
            return;
        };

        *equipped_position = new_equipped_position;
    }
}

#[cfg(test)]
mod tests {
    use korangar_networking::{InventoryItem, InventoryItemDetails};
    use ragnarok_packets::{EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions};

    use super::Inventory;
    use crate::world::ResourceMetadata;

    fn equip(index: u16, refine: u8, grade: u8) -> InventoryItem<ResourceMetadata> {
        let option = || ItemOptions {
            index: 0,
            value: 0,
            parameter: 0,
        };
        InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name: String::new(),
            },
            index: InventoryIndex(index),
            item_id: ItemId(1),
            item_type: 5,
            slot: [0; 4],
            hire_expiration_date: 0,
            details: InventoryItemDetails::Equippable {
                equip_position: EquipPosition::empty(),
                equipped_position: EquipPosition::empty(),
                bind_on_equip_type: 0,
                w_item_sprite_number: 0,
                option_count: 0,
                option_data: [option(), option(), option(), option(), option()],
                forge_data: None,
                refinement_level: refine,
                enchantment_level: grade,
                flags: EquippableItemFlags::empty(),
            },
        }
    }

    fn levels(inventory: &Inventory, index: u16) -> (u8, u8) {
        let item = inventory.items.iter().find(|item| item.index == InventoryIndex(index)).unwrap();
        let InventoryItemDetails::Equippable {
            refinement_level,
            enchantment_level,
            ..
        } = item.details
        else {
            panic!("expected an equip");
        };
        (refinement_level, enchantment_level)
    }

    #[test]
    fn grade_ack_updates_grade_and_clears_refine_on_success() {
        let mut inventory = Inventory::default();
        inventory.items.push(equip(4, 10, 1));

        inventory.apply_enchant_grade(InventoryIndex(4), 2, 0);
        assert_eq!(levels(&inventory, 4), (0, 2));

        inventory.items[0] = equip(4, 9, 2);
        inventory.apply_enchant_grade(InventoryIndex(4), 2, 2);
        assert_eq!(levels(&inventory, 4), (9, 2));

        inventory.apply_enchant_grade(InventoryIndex(4), 0, 3);
        assert_eq!(levels(&inventory, 4), (9, 2));
    }
}
