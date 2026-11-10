use std::sync::Arc;

use korangar_interface::element::StateElement;
use korangar_networking::{InventoryItem, InventoryItemDetails, NoMetadata};
use ragnarok_packets::{AccountId, EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, RegularItemFlags};
use rust_state::RustState;

use crate::graphics::Texture;
use crate::loaders::AsyncLoader;
use crate::world::ResourceMetadata;

#[derive(Default, RustState, StateElement)]
pub struct Inventory {
    // TODO: Unhide this.
    #[hidden_element]
    items: Vec<InventoryItem<ResourceMetadata>>,
    /// Item ids that have appeared in this inventory snapshot. A later delete
    /// keeps the id so the hotbar can show a live stack of 0.
    #[hidden_element]
    seen_item_ids: Vec<u32>,
}

impl Inventory {
    pub fn stacked_amount(&self, item_id: ItemId) -> u16 {
        self.items
            .iter()
            .filter(|item| item.item_id == item_id)
            .map(|item| match &item.details {
                InventoryItemDetails::Regular { amount, .. } => *amount,
                InventoryItemDetails::Equippable { .. } => 1,
            })
            .fold(0, u16::saturating_add)
    }

    pub fn find_by_item_id(&self, item_id: ItemId) -> Option<&InventoryItem<ResourceMetadata>> {
        self.items.iter().find(|item| item.item_id == item_id)
    }

    /// `Some` once this id has been in the inventory, including a stack of 0
    /// after it was removed. `None` means the hotkey count is the only source.
    pub fn known_amount(&self, item_id: ItemId) -> Option<u16> {
        self.seen_item_ids.contains(&item_id.0).then(|| self.stacked_amount(item_id))
    }

    fn remember(&mut self, item_id: ItemId) {
        if !self.seen_item_ids.contains(&item_id.0) {
            self.seen_item_ids.push(item_id.0);
        }
    }

    fn replace_seen(&mut self, item_ids: impl IntoIterator<Item = ItemId>) {
        self.seen_item_ids.clear();
        for item_id in item_ids {
            self.remember(item_id);
        }
    }

    /// Names and icons for hotbar item slots. The hotkey packet has no texture.
    pub fn item_visuals(&self) -> Vec<(u32, String, Option<Arc<Texture>>)> {
        self.items
            .iter()
            .map(|item| (item.item_id.0, item.metadata.name.clone(), item.metadata.texture.clone()))
            .collect()
    }

    pub fn fill(&mut self, async_loader: &AsyncLoader, items: Vec<InventoryItem<NoMetadata>>) {
        self.items = items
            .into_iter()
            .map(|item| async_loader.request_inventory_item_metadata_load(item))
            .collect();
        let seen = self.items.iter().map(|item| item.item_id).collect::<Vec<_>>();
        self.replace_seen(seen);
    }

    /// Returns the name and icon so a hotbar slot bound before this item
    /// arrived can pick up a cached texture. A later sprite load fills the
    /// icon when this returns `None`.
    pub fn add_item(&mut self, async_loader: &AsyncLoader, item: InventoryItem<NoMetadata>) -> (u32, String, Option<Arc<Texture>>) {
        if let Some(position) = self.items.iter().position(|inventory_item| inventory_item.index == item.index) {
            let found_item = &mut self.items[position];
            let InventoryItemDetails::Regular { amount, .. } = &mut found_item.details else {
                panic!();
            };

            let InventoryItemDetails::Regular { amount: added_amount, .. } = item.details else {
                panic!();
            };

            *amount += added_amount;
            let item_id = found_item.item_id;
            let visual = (item_id.0, found_item.metadata.name.clone(), found_item.metadata.texture.clone());
            self.remember(item_id);
            return visual;
        }

        let item = async_loader.request_inventory_item_metadata_load(item);
        let item_id = item.item_id;
        let visual = (item_id.0, item.metadata.name.clone(), item.metadata.texture.clone());
        self.remember(item_id);
        self.items.push(item);
        visual
    }

    pub fn update_item_sprite(&mut self, item_id: ItemId, texture: Arc<Texture>) {
        self.items.iter_mut().filter(|item| item.item_id == item_id).for_each(|item| {
            item.metadata.texture = Some(texture.clone());
        });
    }

    /// Like [`Self::remove_item`], but a missing stack is ignored.
    pub fn remove_item_if_present(&mut self, index: InventoryIndex, remove_amount: u16) {
        if self.items.iter().any(|item| item.index == index) {
            self.remove_item(index, remove_amount);
        }
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

    pub fn mark_identified(&mut self, index: InventoryIndex) -> Option<ItemId> {
        let item = self.items.iter_mut().find(|item| item.index == index)?;
        match &mut item.details {
            InventoryItemDetails::Regular { flags, .. } => {
                flags.insert(RegularItemFlags::IDENTIFIED);
            }
            InventoryItemDetails::Equippable { flags, .. } => {
                flags.insert(EquippableItemFlags::IDENTIFIED);
            }
        }
        Some(item.item_id)
    }

    /// Replace the unidentified name, description, and icon after a successful
    /// identify.
    pub fn set_identified_presentation(&mut self, index: InventoryIndex, name: String, description: String, texture: Option<Arc<Texture>>) {
        let Some(item) = self.items.iter_mut().find(|item| item.index == index) else {
            return;
        };
        item.metadata.name = name;
        item.metadata.description = description;
        if texture.is_some() {
            item.metadata.texture = texture;
        }
    }

    pub fn refinement_level(&self, index: InventoryIndex) -> Option<u8> {
        let item = self.items.iter().find(|item| item.index == index)?;
        match &item.details {
            InventoryItemDetails::Equippable { refinement_level, .. } => Some(*refinement_level),
            InventoryItemDetails::Regular { .. } => None,
        }
    }

    /// `pc_insert_card` writes the card id into the first empty card slot.
    /// The composition ack does not resend the inventory list.
    pub fn insert_card(&mut self, equip_index: InventoryIndex, card_item_id: u32) -> bool {
        if card_item_id == 0 {
            return false;
        }
        let Some(item) = self.items.iter_mut().find(|item| item.index == equip_index) else {
            return false;
        };
        let Some(slot) = item.slot.iter_mut().find(|slot| **slot == 0) else {
            return false;
        };
        *slot = card_item_id;
        true
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

    /// `ZC_USE_ITEM_ACK` remaining count. Zero removes the item. A missing
    /// index is left alone. Equippable rows are not rewritten to a stack count.
    pub fn apply_use_ack(&mut self, index: InventoryIndex, remaining: u16) -> bool {
        let Some(position) = self.items.iter().position(|item| item.index == index) else {
            return false;
        };

        if remaining == 0 {
            self.items.remove(position);
            return true;
        }

        let InventoryItemDetails::Regular { amount, .. } = &mut self.items[position].details else {
            return false;
        };
        *amount = remaining;
        true
    }

    /// `ZC_ITEM_THROW_ACK`. `removed` is the count taken off the stack. Zero
    /// fails and does not delete the item. A missing index does not panic.
    pub fn apply_throw_ack(&mut self, index: InventoryIndex, removed: u16) -> ThrowUpdate {
        let Some(position) = self.items.iter().position(|item| item.index == index) else {
            return ThrowUpdate::Missing;
        };

        if removed == 0 {
            return ThrowUpdate::Failed;
        }

        if let InventoryItemDetails::Regular { amount, .. } = &mut self.items[position].details
            && *amount > removed
        {
            *amount -= removed;
            return ThrowUpdate::Updated;
        }

        self.items.remove(position);
        ThrowUpdate::Updated
    }
}

/// What a double-click on an inventory cell should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemActivation {
    Use,
    Equip {
        position: EquipPosition,
    },
    /// Item type 6. Sends `CZ_REQ_ITEMCOMPOSITION_LIST` instead of
    /// `CZ_USE_ITEM`.
    InsertCard,
    Ignore,
}

/// Drag onto the world. Stacks larger than one ask for an amount first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundDrop {
    Now(u16),
    Ask { max: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrowUpdate {
    Failed,
    Missing,
    Updated,
}

/// `0x01C8` is an area broadcast. `0x00A8` has no account id and is this
/// client.
pub fn use_ack_targets_self(our_account: Option<AccountId>, packet_account: Option<AccountId>) -> bool {
    packet_account.is_none_or(|id| our_account == Some(id))
}

/// roBrowser `ItemType`: healing 0, usable 2, delayed consume 11, type 18 are
/// used. Weapon, armor, shadow, pet armor, and ammo (4, 5, 8, 12, 10) equip
/// when identified. Ammo that arrived as a regular stack equips in the ammo
/// slot.
pub fn item_activation<Meta>(item: &InventoryItem<Meta>) -> ItemActivation {
    match item.item_type {
        0 | 2 | 11 | 18 => ItemActivation::Use,
        6 => ItemActivation::InsertCard,
        4 | 5 | 8 | 12 => equip_position(item),
        10 => match equip_position(item) {
            ItemActivation::Ignore => {
                let InventoryItemDetails::Regular { flags, .. } = &item.details else {
                    return ItemActivation::Ignore;
                };
                if flags.contains(RegularItemFlags::IDENTIFIED) {
                    ItemActivation::Equip {
                        position: EquipPosition::AMMO,
                    }
                } else {
                    ItemActivation::Ignore
                }
            }
            equipped => equipped,
        },
        _ => ItemActivation::Ignore,
    }
}

fn equip_position<Meta>(item: &InventoryItem<Meta>) -> ItemActivation {
    let InventoryItemDetails::Equippable { equip_position, flags, .. } = &item.details else {
        return ItemActivation::Ignore;
    };

    if flags.contains(EquippableItemFlags::IDENTIFIED) && !flags.contains(EquippableItemFlags::IS_BROKEN) && !equip_position.is_empty() {
        ItemActivation::Equip { position: *equip_position }
    } else {
        ItemActivation::Ignore
    }
}

pub fn ground_drop<Meta>(item: &InventoryItem<Meta>) -> Option<GroundDrop> {
    match &item.details {
        InventoryItemDetails::Regular { amount, .. } if *amount > 1 => Some(GroundDrop::Ask { max: *amount }),
        InventoryItemDetails::Regular { amount: 1, .. } => Some(GroundDrop::Now(1)),
        InventoryItemDetails::Regular { .. } => None,
        InventoryItemDetails::Equippable { .. } => Some(GroundDrop::Now(1)),
    }
}

#[cfg(test)]
mod tests {
    use korangar_networking::{InventoryItem, InventoryItemDetails};
    use ragnarok_packets::{AccountId, EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions, RegularItemFlags};

    use super::{GroundDrop, Inventory, ItemActivation, ThrowUpdate, ground_drop, item_activation, use_ack_targets_self};
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
                description: String::new(),
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

    fn regular(index: u16, item_type: u8, amount: u16, identified: bool) -> InventoryItem<ResourceMetadata> {
        InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name: "Red Potion".to_owned(),
                description: String::new(),
            },
            index: InventoryIndex(index),
            item_id: ItemId(501),
            item_type,
            slot: [0; 4],
            hire_expiration_date: 0,
            details: InventoryItemDetails::Regular {
                amount,
                equipped_position: EquipPosition::empty(),
                flags: if identified {
                    RegularItemFlags::IDENTIFIED
                } else {
                    RegularItemFlags::empty()
                },
            },
        }
    }

    fn amount_of(inventory: &Inventory, index: u16) -> Option<u16> {
        inventory.items.iter().find(|item| item.index == InventoryIndex(index)).map(|item| {
            let InventoryItemDetails::Regular { amount, .. } = item.details else {
                panic!("expected a stack");
            };
            amount
        })
    }

    #[test]
    fn use_ack_sets_remaining_and_removes_at_zero() {
        let mut inventory = Inventory::default();
        inventory.items.push(regular(4, 0, 5, true));

        assert!(inventory.apply_use_ack(InventoryIndex(4), 4));
        assert_eq!(amount_of(&inventory, 4), Some(4));

        assert!(inventory.apply_use_ack(InventoryIndex(4), 0));
        assert!(amount_of(&inventory, 4).is_none());
        assert!(!inventory.apply_use_ack(InventoryIndex(4), 1));
    }

    #[test]
    fn use_ack_does_not_rewrite_an_equip() {
        let mut inventory = Inventory::default();
        inventory.items.push(equip(2, 0, 0));
        assert!(!inventory.apply_use_ack(InventoryIndex(2), 1));
        assert_eq!(inventory.items.len(), 1);
        assert!(inventory.apply_use_ack(InventoryIndex(2), 0));
        assert!(inventory.items.is_empty());
    }

    #[test]
    fn throw_ack_subtracts_removes_and_ignores_failure() {
        let mut inventory = Inventory::default();
        inventory.items.push(regular(3, 0, 5, true));
        inventory.items.push(equip(8, 0, 0));

        assert_eq!(inventory.apply_throw_ack(InventoryIndex(3), 0), ThrowUpdate::Failed);
        assert_eq!(amount_of(&inventory, 3), Some(5));

        assert_eq!(inventory.apply_throw_ack(InventoryIndex(3), 2), ThrowUpdate::Updated);
        assert_eq!(amount_of(&inventory, 3), Some(3));

        assert_eq!(inventory.apply_throw_ack(InventoryIndex(3), 3), ThrowUpdate::Updated);
        assert!(amount_of(&inventory, 3).is_none());

        assert_eq!(inventory.apply_throw_ack(InventoryIndex(8), 0), ThrowUpdate::Failed);
        assert_eq!(inventory.items.len(), 1);
        assert_eq!(inventory.apply_throw_ack(InventoryIndex(8), 1), ThrowUpdate::Updated);
        assert!(inventory.items.is_empty());

        assert_eq!(inventory.apply_throw_ack(InventoryIndex(8), 1), ThrowUpdate::Missing);
    }

    #[test]
    fn activation_follows_ro_browser_item_types() {
        assert_eq!(item_activation(&regular(1, 0, 1, true)), ItemActivation::Use);
        assert_eq!(item_activation(&regular(1, 2, 1, true)), ItemActivation::Use);
        assert_eq!(item_activation(&regular(1, 11, 1, true)), ItemActivation::Use);
        assert_eq!(item_activation(&regular(1, 18, 1, true)), ItemActivation::Use);
        assert_eq!(item_activation(&regular(1, 3, 1, true)), ItemActivation::Ignore);
        assert_eq!(item_activation(&regular(1, 6, 1, true)), ItemActivation::InsertCard);

        let mut weapon = equip(1, 0, 0);
        if let InventoryItemDetails::Equippable { equip_position, flags, .. } = &mut weapon.details {
            *equip_position = EquipPosition::RIGHT_HAND;
            *flags = EquippableItemFlags::IDENTIFIED;
        }
        assert_eq!(item_activation(&weapon), ItemActivation::Equip {
            position: EquipPosition::RIGHT_HAND,
        });

        if let InventoryItemDetails::Equippable { flags, .. } = &mut weapon.details {
            *flags = EquippableItemFlags::empty();
        }
        assert_eq!(item_activation(&weapon), ItemActivation::Ignore);

        if let InventoryItemDetails::Equippable { flags, .. } = &mut weapon.details {
            *flags = EquippableItemFlags::IDENTIFIED | EquippableItemFlags::IS_BROKEN;
        }
        assert_eq!(item_activation(&weapon), ItemActivation::Ignore);

        assert_eq!(item_activation(&regular(1, 10, 20, true)), ItemActivation::Equip {
            position: EquipPosition::AMMO,
        });
        assert_eq!(item_activation(&regular(1, 10, 20, false)), ItemActivation::Ignore);
    }

    #[test]
    fn ground_drop_asks_only_for_a_stack() {
        assert_eq!(ground_drop(&regular(1, 0, 5, true)), Some(GroundDrop::Ask { max: 5 }));
        assert_eq!(ground_drop(&regular(1, 0, 1, true)), Some(GroundDrop::Now(1)));
        assert_eq!(ground_drop(&regular(1, 0, 0, true)), None);
        assert_eq!(ground_drop(&equip(1, 0, 0)), Some(GroundDrop::Now(1)));
    }

    #[test]
    fn use_ack_area_broadcast_is_filtered_by_account() {
        let ours = Some(AccountId(9));
        assert!(use_ack_targets_self(ours, None));
        assert!(use_ack_targets_self(ours, Some(AccountId(9))));
        assert!(!use_ack_targets_self(ours, Some(AccountId(4))));
        assert!(!use_ack_targets_self(None, Some(AccountId(9))));
    }

    #[test]
    fn insert_card_fills_the_first_empty_slot() {
        let mut inventory = Inventory::default();
        let mut weapon = equip(2, 0, 0);
        weapon.slot = [4001, 0, 0, 0];
        inventory.items.push(weapon);

        assert!(inventory.insert_card(InventoryIndex(2), 4121));
        assert_eq!(inventory.items[0].slot, [4001, 4121, 0, 0]);
        assert!(!inventory.insert_card(InventoryIndex(2), 0));
        assert_eq!(inventory.items[0].slot, [4001, 4121, 0, 0]);

        inventory.items[0].slot = [1, 2, 3, 4];
        assert!(!inventory.insert_card(InventoryIndex(2), 5));
        assert_eq!(inventory.items[0].slot, [1, 2, 3, 4]);
        assert!(!inventory.insert_card(InventoryIndex(9), 5));
    }

    #[test]
    fn known_amount_stays_zero_after_the_stack_is_removed() {
        let mut inventory = Inventory::default();
        assert!(inventory.known_amount(ItemId(501)).is_none());

        inventory.items.push(regular(1, 0, 10, true));
        inventory.remember(ItemId(501));
        assert_eq!(inventory.known_amount(ItemId(501)), Some(10));

        inventory.remove_item(InventoryIndex(1), 10);
        assert_eq!(inventory.known_amount(ItemId(501)), Some(0));
        assert!(inventory.known_amount(ItemId(502)).is_none());
    }

    #[test]
    fn a_new_inventory_snapshot_forgets_ids_that_are_not_in_it() {
        let mut inventory = Inventory::default();
        inventory.remember(ItemId(501));
        inventory.replace_seen([ItemId(502)]);
        assert!(inventory.known_amount(ItemId(501)).is_none());
        assert_eq!(inventory.known_amount(ItemId(502)), Some(0));
    }
}
