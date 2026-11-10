use std::sync::Arc;

use korangar_interface::element::StateElement;
use korangar_networking::{InventoryItem, InventoryItemDetails, NetworkingSystem};
use ragnarok_packets::handler::PacketCallback;
use ragnarok_packets::{
    EntityId, EquipPosition, HotbarSlot, HotbarTab, HotkeyData, HotkeyType, ItemId, SkillId, SkillLevel, SkillType, TilePosition,
};
use rust_state::RustState;

use crate::graphics::{PickerTarget, Texture};
use crate::loaders::Sprite;
use crate::state::inventory::{ItemActivation, item_activation};
use crate::state::skills::{LearnableSkill, LearnedSkill};
use crate::world::{Actions, ResourceMetadata};

/// Hercules `MAX_HOTKEYS` for packetver 20250716 (RE): 38 slots per tab.
pub const HOTBAR_SLOTS: usize = 38;
/// `MAX_HOTKEYS_DB` is two tabs (`MAX_HOTKEYS * 2`).
pub const HOTBAR_TABS: usize = 2;
pub const HOTBAR_ROWS: usize = 4;
pub const HOTBAR_COLUMNS: usize = 9;
/// Slots 0..35 are the 4×9 bar. 36 and 37 are the two quick slots.
pub const HOTBAR_GRID_SLOTS: usize = HOTBAR_ROWS * HOTBAR_COLUMNS;

pub const CELL_EMPTY: u8 = 0;
pub const CELL_SKILL: u8 = 1;
pub const CELL_ITEM: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InventoryHint {
    Consumable,
    Equipped,
    Unequipped,
    /// Present in the inventory, but not a usable or equippable item.
    Inactive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotbarDispatch {
    None,
    CastSkill { skill_id: u16, level: u16 },
    UseItem { item_id: u32 },
    Equip { item_id: u32 },
    Unequip { item_id: u32 },
}

/// Decide what a hotbar press does. Skills cast. Consumables become `UseItem`.
/// Equipment equips or unequips. An item the inventory would ignore does
/// nothing.
pub fn dispatch_hotbar(kind: u8, skill_id: u16, level: u16, item_id: u32, stack: Option<InventoryHint>) -> HotbarDispatch {
    match kind {
        CELL_SKILL => HotbarDispatch::CastSkill { skill_id, level },
        CELL_ITEM => match stack {
            Some(InventoryHint::Consumable) | None => HotbarDispatch::UseItem { item_id },
            Some(InventoryHint::Unequipped) => HotbarDispatch::Equip { item_id },
            Some(InventoryHint::Equipped) => HotbarDispatch::Unequip { item_id },
            Some(InventoryHint::Inactive) => HotbarDispatch::None,
        },
        _ => HotbarDispatch::None,
    }
}

/// What pressing a hotbar item does. This follows [`item_activation`]: potions
/// are used, identified ammo and equipment are equipped, and cards or
/// unidentified ammo are ignored. Gear that is already worn is taken off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotbarItemPress {
    Use,
    Equip { position: EquipPosition },
    Unequip,
    Ignore,
}

pub fn hotbar_item_press<Meta>(item: &InventoryItem<Meta>) -> HotbarItemPress {
    if let InventoryItemDetails::Equippable { equipped_position, .. } = &item.details
        && !equipped_position.is_empty()
    {
        return HotbarItemPress::Unequip;
    }

    match item_activation(item) {
        ItemActivation::Use => HotbarItemPress::Use,
        ItemActivation::Equip { position } => HotbarItemPress::Equip { position },
        // Card insert is an inventory action. A hotbar key does not pick a target item.
        ItemActivation::Ignore | ItemActivation::InsertCard => HotbarItemPress::Ignore,
    }
}

/// Classifies a bag item for [`dispatch_hotbar`]. Identified ammo is equipment,
/// not a consumable. Cards and unidentified ammo are
/// [`InventoryHint::Inactive`].
pub fn inventory_hint(item: &InventoryItem<ResourceMetadata>) -> InventoryHint {
    match hotbar_item_press(item) {
        HotbarItemPress::Use => InventoryHint::Consumable,
        HotbarItemPress::Equip { .. } => InventoryHint::Unequipped,
        HotbarItemPress::Unequip => InventoryHint::Equipped,
        HotbarItemPress::Ignore => InventoryHint::Inactive,
    }
}

/// Map a visual row (what F1 / Alt+F1 / Ctrl+F1 / Shift+F1 hit) through the
/// server row shift onto a stored slot in the active tab.
pub fn stored_slot(visual: u16, row_shift: u8) -> u16 {
    if visual >= HOTBAR_GRID_SLOTS as u16 {
        return visual.min((HOTBAR_SLOTS - 1) as u16);
    }
    let row = visual / HOTBAR_COLUMNS as u16;
    let column = visual % HOTBAR_COLUMNS as u16;
    let stored_row = (row + u16::from(row_shift % HOTBAR_ROWS as u8)) % HOTBAR_ROWS as u16;
    stored_row * HOTBAR_COLUMNS as u16 + column
}

#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct HotbarCell {
    /// [`CELL_EMPTY`], [`CELL_SKILL`], or [`CELL_ITEM`].
    pub kind: u8,
    pub skill: Option<LearnableSkill>,
    pub item_id: u32,
    pub amount: u16,
    pub item_name: String,
    #[hidden_element]
    pub item_texture: Option<Arc<Texture>>,
}

impl HotbarCell {
    pub fn to_hotkey(&self) -> HotkeyData {
        match self.kind {
            CELL_SKILL => {
                let skill = self.skill.as_ref();
                HotkeyData {
                    hotkey_type: HotkeyType::Skill,
                    item_or_skill_id: skill.map(|skill| u32::from(skill.skill_id.0)).unwrap_or(0),
                    quantity_or_skill_level: skill.map(|skill| skill.maximum_level.0).unwrap_or(0),
                }
            }
            CELL_ITEM => HotkeyData::item(self.item_id, self.amount),
            CELL_EMPTY => HotkeyData::UNBOUND,
            _ => HotkeyData::UNBOUND,
        }
    }
}

#[derive(RustState, StateElement)]
pub struct Hotbar {
    cells: [HotbarCell; HOTBAR_TABS * HOTBAR_SLOTS],
    active_tab: u8,
    visible_rows: u8,
    row_shift: [u8; HOTBAR_TABS],
    sitting: bool,
}

impl Default for Hotbar {
    fn default() -> Self {
        Self {
            cells: std::array::from_fn(|_| HotbarCell::default()),
            active_tab: 0,
            visible_rows: 1,
            row_shift: [0; HOTBAR_TABS],
            sitting: false,
        }
    }
}

impl Hotbar {
    pub fn active_tab(&self) -> u8 {
        self.active_tab
    }

    pub fn visible_rows(&self) -> u8 {
        self.visible_rows.clamp(1, HOTBAR_ROWS as u8)
    }

    pub fn row_shift(&self, tab: u8) -> u8 {
        self.row_shift.get(tab as usize).copied().unwrap_or(0) % HOTBAR_ROWS as u8
    }

    /// Returns `true` when the player should sit down.
    pub fn toggle_sitting(&mut self) -> bool {
        self.sitting = !self.sitting;
        self.sitting
    }

    pub fn mark_standing(&mut self) {
        self.sitting = false;
    }

    pub fn switch_tab(&mut self) {
        self.active_tab = if self.active_tab == 0 { 1 } else { 0 };
    }

    pub fn cycle_visible_rows(&mut self) {
        self.visible_rows = match self.visible_rows() {
            1 => 2,
            2 => 3,
            3 => 4,
            _ => 1,
        };
    }

    pub fn set_server_row_shift(&mut self, tab: HotbarTab, shift: u8) {
        if let Some(slot) = self.row_shift.get_mut(tab.0 as usize) {
            *slot = shift % HOTBAR_ROWS as u8;
        }
    }

    pub fn rotate_active_row<Callback>(&mut self, networking_system: &mut NetworkingSystem<Callback>)
    where
        Callback: PacketCallback + Send,
    {
        let tab = self.active_tab;
        let next = (self.row_shift(tab) + 1) % HOTBAR_ROWS as u8;
        self.row_shift[tab as usize] = next;
        let _ = networking_system.set_hotkey_row_shift(HotbarTab(u16::from(tab)), next);
    }

    pub fn cell(&self, tab: HotbarTab, slot: HotbarSlot) -> Option<&HotbarCell> {
        self.index(tab, slot).map(|index| &self.cells[index])
    }

    pub fn skill_in_slot(&self, tab: HotbarTab, slot: HotbarSlot) -> Option<&LearnableSkill> {
        self.cell(tab, slot).and_then(|cell| match cell.kind {
            CELL_SKILL => cell.skill.as_ref(),
            _ => None,
        })
    }

    pub fn visual_to_stored(&self, visual: u16) -> (HotbarTab, HotbarSlot) {
        let tab = HotbarTab(u16::from(self.active_tab));
        let slot = HotbarSlot(stored_slot(visual, self.row_shift(self.active_tab)));
        (tab, slot)
    }

    pub fn clear_local(&mut self, tab: HotbarTab, slot: HotbarSlot) {
        if let Some(index) = self.index(tab, slot) {
            self.cells[index] = HotbarCell::default();
        }
    }

    pub fn set_skill_local(&mut self, tab: HotbarTab, slot: HotbarSlot, skill: LearnableSkill) {
        if let Some(index) = self.index(tab, slot) {
            self.cells[index] = HotbarCell {
                kind: CELL_SKILL,
                skill: Some(skill),
                item_id: 0,
                amount: 0,
                item_name: String::new(),
                item_texture: None,
            };
        }
    }

    pub fn set_item_local(&mut self, tab: HotbarTab, slot: HotbarSlot, item_id: u32, amount: u16) {
        if let Some(index) = self.index(tab, slot) {
            self.cells[index] = HotbarCell {
                kind: CELL_ITEM,
                skill: None,
                item_id,
                amount,
                item_name: String::new(),
                item_texture: None,
            };
        }
    }

    pub fn bind_skill<Callback>(
        &mut self,
        networking_system: &mut NetworkingSystem<Callback>,
        tab: HotbarTab,
        slot: HotbarSlot,
        skill: LearnableSkill,
    ) where
        Callback: PacketCallback + Send,
    {
        let data = HotkeyData::skill(u32::from(skill.skill_id.0), skill.maximum_level.0);
        let _ = networking_system.set_hotkey_data(tab, slot, data);
        self.set_skill_local(tab, slot, skill);
    }

    pub fn bind_item<Callback>(
        &mut self,
        networking_system: &mut NetworkingSystem<Callback>,
        tab: HotbarTab,
        slot: HotbarSlot,
        item: &InventoryItem<ResourceMetadata>,
    ) where
        Callback: PacketCallback + Send,
    {
        let amount = match &item.details {
            InventoryItemDetails::Regular { amount, .. } => *amount,
            InventoryItemDetails::Equippable { .. } => 1,
        };
        let _ = networking_system.set_hotkey_data(tab, slot, HotkeyData::item(item.item_id.0, amount));
        if let Some(index) = self.index(tab, slot) {
            self.cells[index] = HotbarCell {
                kind: CELL_ITEM,
                skill: None,
                item_id: item.item_id.0,
                amount,
                item_name: item.metadata.name.clone(),
                item_texture: item.metadata.texture.clone(),
            };
        }
    }

    pub fn clear_slot<Callback>(&mut self, networking_system: &mut NetworkingSystem<Callback>, tab: HotbarTab, slot: HotbarSlot)
    where
        Callback: PacketCallback + Send,
    {
        let _ = networking_system.set_hotkey_data(tab, slot, HotkeyData::UNBOUND);
        self.clear_local(tab, slot);
    }

    pub fn swap_slots<Callback>(
        &mut self,
        networking_system: &mut NetworkingSystem<Callback>,
        source_tab: HotbarTab,
        source_slot: HotbarSlot,
        destination_tab: HotbarTab,
        destination_slot: HotbarSlot,
    ) where
        Callback: PacketCallback + Send,
    {
        let Some(source) = self.index(source_tab, source_slot) else {
            return;
        };
        let Some(destination) = self.index(destination_tab, destination_slot) else {
            return;
        };
        if source == destination {
            return;
        }
        self.cells.swap(source, destination);
        let first = self.cells[destination].to_hotkey();
        let second = self.cells[source].to_hotkey();
        let _ = networking_system.set_hotkey_data(destination_tab, destination_slot, first);
        let _ = networking_system.set_hotkey_data(source_tab, source_slot, second);
    }

    pub fn apply_skill_sprite(&mut self, skill_id: SkillId, sprite: Arc<Sprite>) {
        for cell in &mut self.cells {
            if cell.kind == CELL_SKILL
                && let Some(skill) = cell.skill.as_mut()
                && skill.skill_id == skill_id
            {
                skill.sprite = Some(sprite.clone());
            }
        }
    }

    pub fn apply_item_visuals(&mut self, visuals: &[(u32, String, Option<Arc<Texture>>)]) {
        for cell in &mut self.cells {
            if cell.kind != CELL_ITEM {
                continue;
            }
            if let Some((_, name, texture)) = visuals.iter().find(|(item_id, ..)| *item_id == cell.item_id) {
                if !name.is_empty() {
                    cell.item_name.clone_from(name);
                }
                // A snapshot taken before the sprite load finishes has no icon.
                // Leave a texture that already arrived.
                if let Some(texture) = texture {
                    cell.item_texture = Some(texture.clone());
                }
            }
        }
    }

    /// Copy a finished item icon onto every item slot with that id.
    pub fn apply_item_sprite(&mut self, item_id: ItemId, texture: Arc<Texture>) {
        for cell in &mut self.cells {
            if cell.kind == CELL_ITEM && cell.item_id == item_id.0 {
                cell.item_texture = Some(texture.clone());
            }
        }
    }

    pub fn apply_skill_actions(&mut self, skill_id: SkillId, actions: Arc<Actions>) {
        for cell in &mut self.cells {
            if cell.kind == CELL_SKILL
                && let Some(skill) = cell.skill.as_mut()
                && skill.skill_id == skill_id
            {
                skill.actions = Some(actions.clone());
            }
        }
    }

    fn index(&self, tab: HotbarTab, slot: HotbarSlot) -> Option<usize> {
        let tab = tab.0 as usize;
        let slot = slot.0 as usize;
        if tab < HOTBAR_TABS && slot < HOTBAR_SLOTS {
            Some(tab * HOTBAR_SLOTS + slot)
        } else {
            None
        }
    }
}

/// Item id carried by a hotbar cell, used when the inventory lookup needs it.
pub fn cell_item_id(cell: &HotbarCell) -> Option<ItemId> {
    (cell.kind == CELL_ITEM).then_some(ItemId(cell.item_id))
}

/// Saved hotkey count until the inventory has contained this item. After that,
/// the live stack is shown, including 0.
pub fn hotbar_stack_label(saved: u16, known: Option<u16>) -> u16 {
    known.unwrap_or(saved)
}

/// Skill level carried by a hotbar cell.
pub fn cell_skill_level(cell: &HotbarCell) -> SkillLevel {
    cell.skill.as_ref().map(|skill| skill.maximum_level).unwrap_or(SkillLevel(0))
}

pub(crate) const ROLLING_CUTTER_ID: SkillId = SkillId(2036);

/// Cast a skill that is already bound to a hotbar slot.
pub fn cast_bound_skill<Callback>(
    networking_system: &mut NetworkingSystem<Callback>,
    learnable_skill: &LearnableSkill,
    learned_skill: &LearnedSkill,
    mouse_target: PickerTarget,
    player_entity_id: EntityId,
) where
    Callback: PacketCallback + Send,
{
    let skill_id = learnable_skill.skill_id;
    let skill_level = learnable_skill.maximum_level;
    match learned_skill.skill_type {
        SkillType::Passive => {}
        SkillType::Attack => {
            if let PickerTarget::Entity(entity_id) = mouse_target {
                let _ = networking_system.cast_skill(skill_id, skill_level, entity_id);
            }
        }
        SkillType::Ground | SkillType::Trap => {
            if let PickerTarget::Tile { x, y } = mouse_target {
                let _ = networking_system.cast_ground_skill(skill_id, skill_level, TilePosition { x, y });
            }
        }
        SkillType::SelfCast => match skill_id == ROLLING_CUTTER_ID {
            true => {
                let _ = networking_system.cast_channeling_skill(skill_id, skill_level, player_entity_id);
            }
            false => {
                let _ = networking_system.cast_skill(skill_id, skill_level, player_entity_id);
            }
        },
        SkillType::Support => {
            if let PickerTarget::Entity(entity_id) = mouse_target {
                let _ = networking_system.cast_skill(skill_id, skill_level, entity_id);
            } else {
                let _ = networking_system.cast_skill(skill_id, skill_level, player_entity_id);
            }
        }
    }
}

/// Stop Rolling Cutter when its hotbar key is released.
pub fn stop_bound_skill<Callback>(networking_system: &mut NetworkingSystem<Callback>, skill: &LearnableSkill)
where
    Callback: PacketCallback + Send,
{
    if skill.skill_id == ROLLING_CUTTER_ID {
        let _ = networking_system.stop_channeling_skill(skill.skill_id);
    }
}

#[cfg(test)]
mod tests {
    use korangar_networking::{InventoryItem, InventoryItemDetails};
    use ragnarok_packets::{EquipPosition, InventoryIndex, ItemId, RegularItemFlags};

    use super::*;
    use crate::world::ResourceMetadata;

    #[test]
    fn item_and_skill_dispatch_differ() {
        assert_eq!(dispatch_hotbar(CELL_SKILL, 12, 4, 0, None), HotbarDispatch::CastSkill {
            skill_id: 12,
            level: 4
        });
        assert_eq!(
            dispatch_hotbar(CELL_ITEM, 0, 0, 501, Some(InventoryHint::Consumable)),
            HotbarDispatch::UseItem { item_id: 501 }
        );
        assert_eq!(
            dispatch_hotbar(CELL_ITEM, 0, 0, 1201, Some(InventoryHint::Unequipped)),
            HotbarDispatch::Equip { item_id: 1201 }
        );
        assert_eq!(
            dispatch_hotbar(CELL_ITEM, 0, 0, 1201, Some(InventoryHint::Equipped)),
            HotbarDispatch::Unequip { item_id: 1201 }
        );
        assert_eq!(dispatch_hotbar(CELL_EMPTY, 1, 1, 1, None), HotbarDispatch::None);
        assert_eq!(
            dispatch_hotbar(CELL_ITEM, 0, 0, 4001, Some(InventoryHint::Inactive)),
            HotbarDispatch::None
        );
    }

    fn regular_stack(item_type: u8, identified: bool) -> InventoryItem<ResourceMetadata> {
        InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name: String::new(),
                description: String::new(),
            },
            index: InventoryIndex(4),
            item_id: ItemId(1750),
            item_type,
            slot: [0; 4],
            hire_expiration_date: 0,
            details: InventoryItemDetails::Regular {
                amount: 10,
                equipped_position: EquipPosition::empty(),
                flags: if identified {
                    RegularItemFlags::IDENTIFIED
                } else {
                    RegularItemFlags::empty()
                },
            },
        }
    }

    #[test]
    fn identified_ammo_equips_and_unusable_items_are_ignored() {
        assert_eq!(hotbar_item_press(&regular_stack(10, true)), HotbarItemPress::Equip {
            position: EquipPosition::AMMO,
        });
        assert_eq!(hotbar_item_press(&regular_stack(10, false)), HotbarItemPress::Ignore);
        assert_eq!(hotbar_item_press(&regular_stack(6, true)), HotbarItemPress::Ignore);
        assert_eq!(hotbar_item_press(&regular_stack(0, true)), HotbarItemPress::Use);
        assert_eq!(inventory_hint(&regular_stack(10, true)), InventoryHint::Unequipped);
        assert_eq!(inventory_hint(&regular_stack(10, false)), InventoryHint::Inactive);
        assert_eq!(inventory_hint(&regular_stack(0, true)), InventoryHint::Consumable);
    }

    #[test]
    fn row_shift_moves_fkeys_onto_the_next_stored_row() {
        assert_eq!(stored_slot(0, 0), 0);
        assert_eq!(stored_slot(8, 0), 8);
        assert_eq!(stored_slot(0, 1), 9);
        assert_eq!(stored_slot(9, 1), 18);
        assert_eq!(stored_slot(0, 3), 27);
        assert_eq!(stored_slot(36, 2), 36);
    }

    #[test]
    fn hotkey_payload_keeps_the_is_skill_flag() {
        let mut hotbar = Hotbar::default();
        hotbar.set_item_local(HotbarTab(0), HotbarSlot(1), 501, 10);
        let item = hotbar.cell(HotbarTab(0), HotbarSlot(1)).unwrap().to_hotkey();
        assert!(!item.is_skill());
        assert_eq!(item.item_or_skill_id, 501);
        assert_eq!(item.quantity_or_skill_level, 10);

        hotbar.clear_local(HotbarTab(1), HotbarSlot(4));
        assert_eq!(
            hotbar.cell(HotbarTab(1), HotbarSlot(4)).unwrap().to_hotkey(),
            HotkeyData::UNBOUND
        );
    }

    #[test]
    fn stack_label_uses_zero_once_the_inventory_has_seen_the_item() {
        assert_eq!(hotbar_stack_label(10, None), 10);
        assert_eq!(hotbar_stack_label(10, Some(4)), 4);
        assert_eq!(hotbar_stack_label(10, Some(0)), 0);
    }

    #[test]
    fn item_visuals_name_empty_slots_and_do_not_clear_a_missing_icon() {
        let mut hotbar = Hotbar::default();
        hotbar.set_item_local(HotbarTab(0), HotbarSlot(0), 501, 10);
        hotbar.set_item_local(HotbarTab(1), HotbarSlot(2), 501, 3);
        hotbar.set_item_local(HotbarTab(0), HotbarSlot(1), 502, 1);
        hotbar.apply_item_visuals(&[(501, "Red Potion".to_owned(), None)]);
        assert_eq!(hotbar.cell(HotbarTab(0), HotbarSlot(0)).unwrap().item_name, "Red Potion");
        assert_eq!(hotbar.cell(HotbarTab(1), HotbarSlot(2)).unwrap().item_name, "Red Potion");
        assert!(hotbar.cell(HotbarTab(0), HotbarSlot(1)).unwrap().item_name.is_empty());
        assert!(hotbar.cell(HotbarTab(0), HotbarSlot(0)).unwrap().item_texture.is_none());

        hotbar.apply_item_visuals(&[(501, String::new(), None)]);
        assert_eq!(hotbar.cell(HotbarTab(0), HotbarSlot(0)).unwrap().item_name, "Red Potion");
    }
}
