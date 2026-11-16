//! Perfect-enchant window state.
//!
//! The server opens the UI with a group id (`ZC_UI_OPEN_V3` type 10). Costs,
//! target items, and success rates live in the client `EnchantList` table, the
//! same source roBrowser uses. Grade blessing and grade materials are a
//! different window.

use std::collections::HashMap;
use std::sync::OnceLock;

use korangar_interface::element::StateElement;
use korangar_loaders::FileLoader;
use korangar_networking::{InventoryItem, InventoryItemDetails};
use mlua::Lua;
use ragnarok_packets::EquippableItemFlags;
use rust_state::RustState;

use super::picker::PickerRow;
use crate::loaders::GameFileLoader;
use crate::world::{ResourceMetadata, execute_lua_chunk};

pub const ACTION_RANDOM: u8 = 0;
pub const ACTION_PERFECT: u8 = 1;
pub const ACTION_UPGRADE: u8 = 2;
pub const ACTION_RESET: u8 = 3;

/// What the Enchant button sends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnchantCommand {
    Random { group: i64, index: i16 },
    Perfect { group: i64, index: i16, item_id: u32 },
    Upgrade { group: i64, index: i16, slot: i16 },
    Reset { group: i64, index: i16 },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct EnchantMaterial {
    pub item_id: u32,
    pub count: i32,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct EnchantCost {
    pub zeny: i32,
    pub materials: Vec<EnchantMaterial>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct GradeBonus {
    pub grade: u8,
    pub bonus: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct RandomEnchant {
    pub grade: u8,
    pub item_id: u32,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct PerfectEnchant {
    pub item_id: u32,
    pub name: String,
    pub zeny: i32,
    pub materials: Vec<EnchantMaterial>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct UpgradeEnchant {
    pub from_id: u32,
    pub name: String,
    pub result_id: u32,
    pub result_name: String,
    pub zeny: i32,
    pub materials: Vec<EnchantMaterial>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct SlotRecipe {
    pub slot: u8,
    pub require: EnchantCost,
    pub success_rate: i32,
    pub grade_bonus: Vec<GradeBonus>,
    pub random: Vec<RandomEnchant>,
    pub perfect: Vec<PerfectEnchant>,
    pub upgrade: Vec<UpgradeEnchant>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct EnchantGroup {
    pub id: i64,
    pub slot_order: Vec<u8>,
    pub target_item_ids: Vec<u32>,
    pub min_refine: u8,
    pub min_grade: u8,
    pub allow_random_options: bool,
    pub reset_enabled: bool,
    pub reset_rate: i32,
    pub reset_cost: EnchantCost,
    pub caution: String,
    pub slots: Vec<SlotRecipe>,
}

/// One equip, copied so the window can rebuild without borrowing the inventory.
#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct ItemSnap {
    pub index: u16,
    pub item_id: u32,
    pub name: String,
    pub refine: u8,
    pub grade: u8,
    pub slot0: u32,
    pub slot1: u32,
    pub slot2: u32,
    pub slot3: u32,
    pub slot_count: u8,
    pub has_random_options: bool,
    pub equipped: bool,
    pub in_switch: bool,
    pub identified: bool,
    pub broken: bool,
}

/// Cards to copy onto the inventory item after message 3857.
///
/// Hercules writes `it->card[]` and does not send a new inventory list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnchantCardWrite {
    pub index: u16,
    pub slots: [u32; 4],
}

impl ItemSnap {
    pub fn slot(&self, index: u8) -> u32 {
        match index {
            0 => self.slot0,
            1 => self.slot1,
            2 => self.slot2,
            3 => self.slot3,
            _ => 0,
        }
    }

    pub fn set_slot(&mut self, index: u8, value: u32) {
        match index {
            0 => self.slot0 = value,
            1 => self.slot1 = value,
            2 => self.slot2 = value,
            3 => self.slot3 = value,
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct OwnedStack {
    pub item_id: u32,
    pub amount: u32,
}

#[derive(Clone, Debug, Default)]
pub struct EnchantCatalog {
    groups: Vec<EnchantGroup>,
    slot_counts: HashMap<u32, u8>,
}

impl EnchantCatalog {
    pub fn group(&self, id: i64) -> Option<&EnchantGroup> {
        self.groups.iter().find(|group| group.id == id)
    }

    pub fn slot_count(&self, item_id: u32) -> u8 {
        self.slot_counts.get(&item_id).copied().unwrap_or(0)
    }
}

static CATALOG: OnceLock<EnchantCatalog> = OnceLock::new();

/// Load `EnchantList` once. A missing file yields an empty catalog.
pub fn shared_catalog(loader: &GameFileLoader) -> &EnchantCatalog {
    CATALOG.get_or_init(|| load_catalog(loader))
}

pub fn item_label(snap: &ItemSnap) -> String {
    let name = if snap.name.is_empty() {
        format!("Item {}", snap.item_id)
    } else {
        snap.name.clone()
    };
    if snap.grade > 0 {
        format!("+{} {name} (grade {})", snap.refine, snap.grade)
    } else {
        format!("+{} {name}", snap.refine)
    }
}

pub fn snaps_from_inventory(
    items: &[InventoryItem<ResourceMetadata>],
    switch_indexes: &[u16],
    slot_counts: &HashMap<u32, u8>,
) -> Vec<ItemSnap> {
    items
        .iter()
        .filter_map(|item| {
            let InventoryItemDetails::Equippable {
                equipped_position,
                refinement_level,
                enchantment_level,
                option_data,
                flags,
                ..
            } = &item.details
            else {
                return None;
            };
            Some(ItemSnap {
                index: item.index.0,
                item_id: item.item_id.0,
                name: item.metadata.name.clone(),
                refine: *refinement_level,
                grade: *enchantment_level,
                slot0: item.slot[0],
                slot1: item.slot[1],
                slot2: item.slot[2],
                slot3: item.slot[3],
                slot_count: slot_counts.get(&item.item_id.0).copied().unwrap_or(0),
                has_random_options: option_data.iter().any(|option| option.index > 0),
                equipped: !equipped_position.is_empty(),
                in_switch: switch_indexes.contains(&item.index.0),
                identified: flags.contains(EquippableItemFlags::IDENTIFIED),
                broken: flags.contains(EquippableItemFlags::IS_BROKEN),
            })
        })
        .collect()
}

pub fn stock_from_inventory(items: &[InventoryItem<ResourceMetadata>]) -> Vec<OwnedStack> {
    let mut totals: HashMap<u32, u32> = HashMap::new();
    for item in items {
        let amount = match &item.details {
            InventoryItemDetails::Regular { amount, .. } => u32::from(*amount),
            InventoryItemDetails::Equippable { .. } => 1,
        };
        *totals.entry(item.item_id.0).or_default() += amount;
    }
    let mut stacks: Vec<_> = totals.into_iter().map(|(item_id, amount)| OwnedStack { item_id, amount }).collect();
    stacks.sort_by_key(|stack| stack.item_id);
    stacks
}

pub fn inventory_item_name(items: &[InventoryItem<ResourceMetadata>], item_id: u32) -> String {
    items
        .iter()
        .find(|item| item.item_id.0 == item_id && !item.metadata.name.is_empty())
        .map(|item| item.metadata.name.clone())
        .unwrap_or_else(|| format!("Item {item_id}"))
}

/// `rate` is the client unit where 100000 is 100%.
pub fn format_success(rate: i32) -> String {
    let rate = rate.clamp(0, 100_000);
    if rate % 1000 == 0 {
        format!("Success {}%", rate / 1000)
    } else {
        format!("Success {:.1}%", rate as f32 / 1000.0)
    }
}

pub fn format_zeny(zeny: i32) -> String {
    let negative = zeny < 0;
    let digits = zeny.unsigned_abs().to_string();
    let mut out = String::new();
    for (index, byte) in digits.bytes().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(byte as char);
    }
    let body: String = out.chars().rev().collect();
    if negative {
        format!("Cost -{body}z")
    } else {
        format!("Cost {body}z")
    }
}

pub fn random_success_rate(recipe: &SlotRecipe, grade: u8) -> i32 {
    let bonus = recipe
        .grade_bonus
        .iter()
        .find(|entry| entry.grade == grade)
        .map(|entry| entry.bonus)
        .unwrap_or(0);
    (recipe.success_rate + bonus).clamp(0, 100_000)
}

/// Next card index Hercules will enchant: `SlotOrder[count of filled SlotOrder
/// entries]`.
///
/// `slotCount` does not move this. A card sitting in `SlotOrder` counts as
/// filled.
pub fn next_enchant_slot(snap: &ItemSnap, group: &EnchantGroup) -> Option<u8> {
    let order = slot_order(group);
    let filled = order.iter().filter(|slot| snap.slot(**slot) != 0).count();
    order.get(filled).copied()
}

pub fn build_caution(group: &EnchantGroup) -> String {
    let raw = group.caution.trim();
    if raw.is_empty() {
        return String::new();
    }
    if raw.is_ascii() {
        return raw.to_owned();
    }
    let percents = percent_tokens(raw);
    let mut lines = Vec::new();
    if group.min_refine > 0 {
        lines.push(format!("Requires refine +{} or higher.", group.min_refine));
    }
    if let Some(rate) = percents.first() {
        lines.push(format!("Normal enchant success rate: {rate}"));
    }
    if let Some(rate) = percents.get(1) {
        lines.push(format!("Enchant reset success rate: {rate}"));
    }
    lines.join("\n")
}

/// Why an equip is not offered in the item list.
pub fn rejection(snap: &ItemSnap, group: &EnchantGroup) -> Option<&'static str> {
    if !snap.identified {
        return Some("Item must be identified.");
    }
    if snap.broken {
        return Some("Item attribute must be normal.");
    }
    if snap.equipped {
        return Some("Item must be in the inventory.");
    }
    if snap.in_switch {
        return Some("Item cannot be in the equipment switch.");
    }
    if !group.target_item_ids.contains(&snap.item_id) {
        return Some("Item is not valid for this enchant group.");
    }
    if snap.refine < group.min_refine {
        return Some("Refine level too low.");
    }
    if snap.grade < group.min_grade {
        return Some("Enchant grade too low.");
    }
    if !group.allow_random_options && snap.has_random_options {
        return Some("Random options not allowed.");
    }
    None
}

#[derive(Default, RustState, StateElement)]
pub struct EnchantWindowState {
    pub status: String,
    pub caution: String,
    pub action_label: String,
    pub chance: String,
    pub cost: String,
    pub requirement: String,
    pub hint: String,
    pub candidates: Vec<PickerRow>,
    pub choices: Vec<PickerRow>,
    pub slots: Vec<PickerRow>,
    pub materials: Vec<PickerRow>,
    group_id: i64,
    group: EnchantGroup,
    catalog_ready: bool,
    snaps: Vec<ItemSnap>,
    stock: Vec<OwnedStack>,
    action: u8,
    selected_index: Option<u16>,
    choice_item_id: Option<u32>,
    choice_slot: Option<i16>,
    active_slot: Option<u8>,
    /// Action that was actually sent. The player can change the window while
    /// the packet is in flight; message 3857 must use this snapshot.
    pending_action: Option<u8>,
    pending_index: u16,
    pending_slot: Option<u8>,
}

impl EnchantWindowState {
    pub fn open(&mut self, group_id: i64, group: Option<EnchantGroup>, snaps: Vec<ItemSnap>, stock: Vec<OwnedStack>) {
        *self = Self::default();
        self.group_id = group_id;
        self.snaps = snaps;
        self.stock = stock;
        match group {
            Some(group) => {
                self.caution = build_caution(&group);
                self.group = group;
                self.catalog_ready = true;
                self.status = "Choose an item to enchant.".to_owned();
            }
            None => {
                self.catalog_ready = false;
                self.status = format!("Enchant data is not loaded for group {group_id}.");
            }
        }
        self.rebuild();
        if self.catalog_ready && self.candidates.is_empty() {
            self.status = "No enchantable items.".to_owned();
        }
    }

    /// Replace the bag snapshot used for material checks. Call this from the
    /// live inventory before a submit, and again after a result or a delete.
    pub fn reload_stock(&mut self, stock: Vec<OwnedStack>) {
        self.stock = stock;
        self.rebuild();
    }

    #[cfg(test)]
    pub fn set_stock(&mut self, stock: Vec<OwnedStack>) {
        self.reload_stock(stock);
    }

    pub fn select(&mut self, index: u16) {
        if !self.candidates.iter().any(|row| row.index == index) {
            self.status = "That item cannot be enchanted.".to_owned();
            return;
        }
        self.selected_index = Some(index);
        self.choice_item_id = None;
        self.choice_slot = None;
        self.action = self.first_action().unwrap_or(ACTION_RANDOM);
        self.rebuild();
        if let Some(name) = self.snaps.iter().find(|snap| snap.index == index).map(item_label) {
            self.status = format!("Selected {name}.");
        }
    }

    pub fn set_action(&mut self, action: u8) {
        if self.selected_index.is_none() {
            self.status = "Choose an item first.".to_owned();
            return;
        }
        if !self.action_available(action) {
            self.status = "That enchant is not available for this item.".to_owned();
            return;
        }
        self.action = action;
        self.choice_item_id = None;
        self.choice_slot = None;
        self.rebuild();
    }

    pub fn pick_choice(&mut self, index: u16, item_id: u32) {
        match self.action {
            ACTION_PERFECT => self.choice_item_id = Some(item_id),
            ACTION_UPGRADE => self.choice_slot = i16::try_from(index).ok(),
            _ => return,
        }
        self.rebuild();
    }

    pub fn show_result(&mut self, message_id: i32, item_id: u32) -> Option<EnchantCardWrite> {
        let action = self.pending_action.take()?;
        let index = self.pending_index;
        let written_slot = self.pending_slot.take();
        self.status = korangar_networking::format_enchant_result(message_id, item_id);
        if message_id != 3857 {
            return None;
        }
        let snap = self.snaps.iter_mut().find(|snap| snap.index == index)?;
        match action {
            ACTION_RESET => {
                let order = slot_order(&self.group);
                for slot in order {
                    snap.set_slot(slot, 0);
                }
            }
            ACTION_RANDOM | ACTION_PERFECT | ACTION_UPGRADE => {
                if let Some(slot) = written_slot {
                    snap.set_slot(slot, item_id);
                }
            }
            _ => {}
        }
        let written = EnchantCardWrite {
            index,
            slots: [snap.slot0, snap.slot1, snap.slot2, snap.slot3],
        };
        self.choice_item_id = None;
        self.choice_slot = None;
        self.rebuild();
        Some(written)
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn command(&mut self) -> Result<EnchantCommand, &'static str> {
        // One request at a time. A short zeny purse gets no result packet, so
        // the lock stays until the NPC opens the window again.
        if self.pending_action.is_some() {
            return Err("Waiting for the enchant result. Close and speak to the NPC again if nothing arrives.");
        }
        if !self.catalog_ready {
            return Err("Enchant data is not loaded.");
        }
        let index = self.selected_index.ok_or("Choose an item to enchant.")?;
        // The enchant packets carry a raw index. The server subtracts 2, the same
        // adjustment `InventoryIndex` applies when it is written.
        let wire = i16::try_from(index)
            .ok()
            .and_then(|stored| stored.checked_add(2))
            .ok_or("Inventory index does not fit the enchant packet.")?;
        if self.requirement.starts_with("Missing") {
            return Err("Not enough materials.");
        }
        if !self.requirement.is_empty() {
            return Err("Requirements are not met.");
        }
        let group = self.group_id;
        let action = self.action;
        let pending_slot = match action {
            ACTION_RANDOM | ACTION_PERFECT => self.active_slot,
            ACTION_UPGRADE => self.choice_slot.and_then(|slot| u8::try_from(slot).ok()),
            _ => None,
        };
        let command = match action {
            ACTION_RANDOM => Ok(EnchantCommand::Random { group, index: wire }),
            ACTION_PERFECT => {
                let item_id = self.choice_item_id.ok_or("Choose a perfect enchant.")?;
                Ok(EnchantCommand::Perfect {
                    group,
                    index: wire,
                    item_id,
                })
            }
            ACTION_UPGRADE => {
                let slot = self.choice_slot.ok_or("Choose an enchant to upgrade.")?;
                Ok(EnchantCommand::Upgrade { group, index: wire, slot })
            }
            ACTION_RESET => Ok(EnchantCommand::Reset { group, index: wire }),
            _ => Err("Choose an enchant action."),
        };
        if command.is_ok() {
            self.pending_action = Some(action);
            self.pending_index = index;
            self.pending_slot = pending_slot;
        }
        command
    }

    fn first_action(&self) -> Option<u8> {
        [ACTION_RANDOM, ACTION_PERFECT, ACTION_UPGRADE, ACTION_RESET]
            .into_iter()
            .find(|action| self.action_available(*action))
    }

    fn selected_snap(&self) -> Option<&ItemSnap> {
        let index = self.selected_index?;
        self.snaps.iter().find(|snap| snap.index == index)
    }

    fn action_available(&self, action: u8) -> bool {
        let Some(snap) = self.selected_snap() else {
            return false;
        };
        if !self.catalog_ready {
            return false;
        }
        match action {
            ACTION_RANDOM => next_enchant_slot(snap, &self.group)
                .and_then(|slot| recipe(&self.group, slot))
                .is_some_and(|slot| !random_for(slot, snap.grade).is_empty()),
            ACTION_PERFECT => next_enchant_slot(snap, &self.group)
                .and_then(|slot| recipe(&self.group, slot))
                .is_some_and(|slot| !slot.perfect.is_empty()),
            ACTION_UPGRADE => !upgrade_choices(snap, &self.group).is_empty(),
            ACTION_RESET => self.group.reset_enabled && has_enchant(snap, &self.group),
            _ => false,
        }
    }

    fn owned(&self, item_id: u32) -> u32 {
        self.stock
            .iter()
            .find(|stack| stack.item_id == item_id)
            .map(|stack| stack.amount)
            .unwrap_or(0)
    }

    fn material_gap(&self, materials: &[EnchantMaterial]) -> Option<String> {
        let mut missing = Vec::new();
        for material in materials {
            let need = u32::try_from(material.count).unwrap_or(0);
            let have = self.owned(material.item_id);
            if have < need {
                let name = if material.name.is_empty() {
                    format!("Item {}", material.item_id)
                } else {
                    material.name.clone()
                };
                missing.push(format!("{name} x{need} (have {have})"));
            }
        }
        if missing.is_empty() {
            None
        } else {
            Some(format!("Missing {}", missing.join(", ")))
        }
    }

    fn rebuild(&mut self) {
        let selected = self.selected_index;
        let ready = self.catalog_ready;
        self.candidates = self
            .snaps
            .iter()
            .filter(|snap| ready && rejection(snap, &self.group).is_none())
            .map(|snap| PickerRow {
                index: snap.index,
                item_id: snap.item_id,
                label: item_label(snap),
                selected: Some(snap.index) == selected,
            })
            .collect();
        if selected.is_some() && !self.candidates.iter().any(|row| row.selected) {
            self.selected_index = None;
        }
        self.rebuild_details();
    }

    fn rebuild_details(&mut self) {
        self.choices.clear();
        self.slots.clear();
        self.materials.clear();
        self.chance.clear();
        self.cost.clear();
        self.requirement.clear();
        self.hint.clear();
        self.active_slot = None;
        self.action_label = format!("Action: {}", action_name(self.action));

        let Some(snap) = self.selected_snap().cloned() else {
            return;
        };
        if !self.action_available(self.action)
            && let Some(action) = self.first_action()
        {
            self.action = action;
            self.action_label = format!("Action: {}", action_name(action));
        }

        self.slots = slot_rows(&snap, &self.group, self.active_slot_for(&snap));

        match self.action {
            ACTION_RANDOM => self.fill_random(&snap),
            ACTION_PERFECT => self.fill_perfect(&snap),
            ACTION_UPGRADE => self.fill_upgrade(&snap),
            ACTION_RESET => self.fill_reset(&snap),
            _ => {}
        }
    }

    fn active_slot_for(&self, snap: &ItemSnap) -> Option<u8> {
        match self.action {
            ACTION_UPGRADE => self.choice_slot.map(|slot| slot as u8),
            ACTION_RANDOM | ACTION_PERFECT => next_enchant_slot(snap, &self.group),
            _ => None,
        }
    }

    fn fill_random(&mut self, snap: &ItemSnap) {
        self.hint = "Uses the next open slot.".to_owned();
        let Some(slot) = next_enchant_slot(snap, &self.group) else {
            self.requirement = "No open enchant slot.".to_owned();
            return;
        };
        self.active_slot = Some(slot);
        self.slots = slot_rows(snap, &self.group, Some(slot));
        let Some(recipe) = recipe(&self.group, slot) else {
            self.requirement = "This slot has no enchant recipe.".to_owned();
            return;
        };
        let rate = random_success_rate(recipe, snap.grade);
        self.chance = format_success(rate);
        self.cost = format_zeny(recipe.require.zeny);
        self.materials = material_rows(&recipe.require.materials);
        self.choices = random_for(recipe, snap.grade)
            .into_iter()
            .map(|entry| PickerRow {
                index: 0,
                item_id: entry.item_id,
                label: entry.name.clone(),
                selected: false,
            })
            .collect();
        if let Some(gap) = self.material_gap(&recipe.require.materials) {
            self.requirement = gap;
        }
    }

    fn fill_perfect(&mut self, snap: &ItemSnap) {
        self.hint = "Choose the enchant to place.".to_owned();
        let Some(slot) = next_enchant_slot(snap, &self.group) else {
            self.requirement = "No open enchant slot.".to_owned();
            return;
        };
        self.active_slot = Some(slot);
        self.slots = slot_rows(snap, &self.group, Some(slot));
        let Some(recipe) = recipe(&self.group, slot) else {
            self.requirement = "This slot has no perfect enchant.".to_owned();
            return;
        };
        self.choices = recipe
            .perfect
            .iter()
            .map(|entry| PickerRow {
                index: 0,
                item_id: entry.item_id,
                label: entry.name.clone(),
                selected: self.choice_item_id == Some(entry.item_id),
            })
            .collect();
        let Some(chosen) = recipe.perfect.iter().find(|entry| Some(entry.item_id) == self.choice_item_id) else {
            self.requirement = "Choose a perfect enchant.".to_owned();
            return;
        };
        self.chance = format_success(100_000);
        self.cost = format_zeny(chosen.zeny);
        self.materials = material_rows(&chosen.materials);
        if let Some(gap) = self.material_gap(&chosen.materials) {
            self.requirement = gap;
        }
    }

    fn fill_upgrade(&mut self, snap: &ItemSnap) {
        self.hint = "Choose an enchant to upgrade.".to_owned();
        let choices = upgrade_choices(snap, &self.group);
        if self.choice_slot.is_none()
            && let Some(first) = choices.first()
        {
            self.choice_slot = Some(i16::from(first.slot));
        }
        let selected = self.choice_slot;
        self.choices = choices
            .iter()
            .map(|choice| PickerRow {
                index: u16::from(choice.slot),
                item_id: choice.result_id,
                label: format!("Slot {}: {} -> {}", choice.slot + 1, choice.from_name, choice.result_name),
                selected: selected == Some(i16::from(choice.slot)),
            })
            .collect();
        let Some(choice) = choices.iter().find(|choice| selected == Some(i16::from(choice.slot))) else {
            self.requirement = "Choose an enchant to upgrade.".to_owned();
            return;
        };
        self.active_slot = Some(choice.slot);
        self.slots = slot_rows(snap, &self.group, Some(choice.slot));
        self.chance = format_success(100_000);
        self.cost = format_zeny(choice.zeny);
        self.materials = material_rows(&choice.materials);
        if let Some(gap) = self.material_gap(&choice.materials) {
            self.requirement = gap;
        }
    }

    fn fill_reset(&mut self, snap: &ItemSnap) {
        self.hint = "Resets every enchant slot.".to_owned();
        if !self.group.reset_enabled || !has_enchant(snap, &self.group) {
            self.requirement = "Nothing to reset.".to_owned();
            return;
        }
        self.chance = format_success(self.group.reset_rate);
        self.cost = format_zeny(self.group.reset_cost.zeny);
        self.materials = material_rows(&self.group.reset_cost.materials);
        if let Some(gap) = self.material_gap(&self.group.reset_cost.materials) {
            self.requirement = gap;
        }
    }
}

struct UpgradeChoice {
    slot: u8,
    from_name: String,
    result_id: u32,
    result_name: String,
    zeny: i32,
    materials: Vec<EnchantMaterial>,
}

fn upgrade_choices(snap: &ItemSnap, group: &EnchantGroup) -> Vec<UpgradeChoice> {
    let mut choices = Vec::new();
    for slot in slot_order(group) {
        if slot < snap.slot_count {
            continue;
        }
        let current = snap.slot(slot);
        if current == 0 {
            continue;
        }
        let Some(recipe) = recipe(group, slot) else {
            continue;
        };
        let Some(upgrade) = recipe.upgrade.iter().find(|entry| entry.from_id == current) else {
            continue;
        };
        choices.push(UpgradeChoice {
            slot,
            from_name: if upgrade.name.is_empty() {
                known_name(group, current)
            } else {
                upgrade.name.clone()
            },
            result_id: upgrade.result_id,
            result_name: upgrade.result_name.clone(),
            zeny: upgrade.zeny,
            materials: upgrade.materials.clone(),
        });
    }
    choices
}

fn material_rows(materials: &[EnchantMaterial]) -> Vec<PickerRow> {
    materials
        .iter()
        .map(|material| {
            let name = if material.name.is_empty() {
                format!("Item {}", material.item_id)
            } else {
                material.name.clone()
            };
            PickerRow {
                index: 0,
                item_id: material.item_id,
                label: format!("{name} x{}", material.count),
                selected: false,
            }
        })
        .collect()
}

fn slot_rows(snap: &ItemSnap, group: &EnchantGroup, active: Option<u8>) -> Vec<PickerRow> {
    (0..4)
        .map(|slot| {
            let value = snap.slot(slot);
            let locked = slot < snap.slot_count;
            let label = if value == 0 {
                if locked {
                    format!("Slot {}: card slot", slot + 1)
                } else {
                    format!("Slot {}: empty", slot + 1)
                }
            } else {
                let kind = if locked { "card" } else { "enchant" };
                format!("Slot {}: {kind} {}", slot + 1, known_name(group, value))
            };
            PickerRow {
                index: u16::from(slot),
                item_id: value,
                label,
                selected: active == Some(slot),
            }
        })
        .collect()
}

fn known_name(group: &EnchantGroup, item_id: u32) -> String {
    if item_id == 0 {
        return "Empty".to_owned();
    }
    for slot in &group.slots {
        for entry in &slot.random {
            if entry.item_id == item_id && !entry.name.is_empty() {
                return entry.name.clone();
            }
        }
        for entry in &slot.perfect {
            if entry.item_id == item_id && !entry.name.is_empty() {
                return entry.name.clone();
            }
        }
        for entry in &slot.upgrade {
            if entry.from_id == item_id && !entry.name.is_empty() {
                return entry.name.clone();
            }
            if entry.result_id == item_id && !entry.result_name.is_empty() {
                return entry.result_name.clone();
            }
        }
        for material in slot
            .require
            .materials
            .iter()
            .chain(slot.perfect.iter().flat_map(|entry| entry.materials.iter()))
            .chain(slot.upgrade.iter().flat_map(|entry| entry.materials.iter()))
        {
            if material.item_id == item_id && !material.name.is_empty() {
                return material.name.clone();
            }
        }
    }
    for material in &group.reset_cost.materials {
        if material.item_id == item_id && !material.name.is_empty() {
            return material.name.clone();
        }
    }
    format!("Item {item_id}")
}

fn has_enchant(snap: &ItemSnap, group: &EnchantGroup) -> bool {
    slot_order(group).into_iter().any(|slot| snap.slot(slot) != 0)
}

fn slot_order(group: &EnchantGroup) -> Vec<u8> {
    if group.slot_order.is_empty() {
        vec![0, 1, 2, 3]
    } else {
        group.slot_order.clone()
    }
}

fn recipe(group: &EnchantGroup, slot: u8) -> Option<&SlotRecipe> {
    group.slots.iter().find(|recipe| recipe.slot == slot)
}

fn random_for(recipe: &SlotRecipe, grade: u8) -> Vec<&RandomEnchant> {
    let specific: Vec<_> = recipe.random.iter().filter(|entry| entry.grade == grade).collect();
    if specific.is_empty() && grade != 0 {
        recipe.random.iter().filter(|entry| entry.grade == 0).collect()
    } else {
        specific
    }
}

fn action_name(action: u8) -> &'static str {
    match action {
        ACTION_PERFECT => "Perfect",
        ACTION_UPGRADE => "Upgrade",
        ACTION_RESET => "Reset",
        _ => "Normal",
    }
}

fn percent_tokens(raw: &str) -> Vec<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index].is_ascii_digit() {
            let start = index;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            let mut cursor = index;
            while cursor < chars.len() && chars[cursor].is_whitespace() {
                cursor += 1;
            }
            if cursor < chars.len() && chars[cursor] == '%' {
                tokens.push(
                    chars[start..=cursor]
                        .iter()
                        .filter(|character| !character.is_whitespace())
                        .collect(),
                );
                index = cursor + 1;
                continue;
            }
        }
        index += 1;
    }
    tokens
}

/// Globals the official client defines before it runs `EnchantList_f.lub`
/// (values as in roBrowser `DBManager.loadEnchantListFile`). Without them the
/// first `SetSlotOrder` compares nil with a number and the list stops at group
/// 1.
const ENCHANT_LUA_PRELUDE: &str = r#"
MAX_SLOT_NUM = 4
MAX_MATERIAL_NUM = 10
MAX_REFINE_LEVEL = 20
MAX_GRADE_LEVEL = 4
IS_CLIENT = true
MessageBox = function(message) return 1 end
C_GetSlotCount = function(item_db_name) return 0 end
"#;

fn enchant_lua() -> Lua {
    let lua = Lua::new();
    let _ = lua.load(ENCHANT_LUA_PRELUDE).set_name("enchant prelude").exec();
    lua
}

fn load_catalog(loader: &GameFileLoader) -> EnchantCatalog {
    let lua = enchant_lua();
    for (name, paths) in [
        (
            "ItemDBNameTbl.lub",
            &[
                "data\\luafiles514\\lua files\\ItemDBNameTbl.lub",
                "data/luafiles514/lua files/ItemDBNameTbl.lub",
            ][..],
        ),
        (
            "itemInfo.lua",
            &[
                "System\\itemInfo.turoran.renewal11.lua",
                "System/itemInfo.turoran.renewal11.lua",
                "data\\luafiles514\\lua files\\datainfo\\iteminfo.lub",
                "data/luafiles514/lua files/datainfo/iteminfo.lub",
            ][..],
        ),
        (
            "EnchantList_f.lub",
            &[
                "data\\luafiles514\\lua files\\Enchant\\EnchantList_f.lub",
                "data/luafiles514/lua files/Enchant/EnchantList_f.lub",
            ][..],
        ),
        (
            "EnchantList.lub",
            &[
                "data\\luafiles514\\lua files\\Enchant\\EnchantList.lub",
                "data/luafiles514/lua files/Enchant/EnchantList.lub",
            ][..],
        ),
    ] {
        if let Some(bytes) = paths.iter().find_map(|path| loader.get(path).ok()) {
            let _ = execute_lua_chunk(&lua, name, &bytes);
        }
    }
    catalog_from_lua(&lua)
}

#[cfg(test)]
fn catalog_from_source(source: &str) -> EnchantCatalog {
    let lua = enchant_lua();
    lua.load(source).exec().expect("enchant fixture");
    catalog_from_lua(&lua)
}

fn catalog_from_lua(lua: &Lua) -> EnchantCatalog {
    let globals = lua.globals();
    let mut aegis: HashMap<String, u32> = HashMap::new();
    if let Ok(table) = globals.get::<mlua::Table>("ItemDBNameTbl") {
        for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
            let Some(name) = lua_text(pair.0) else {
                continue;
            };
            if let Some(id) = lua_u32(Some(pair.1)) {
                aegis.insert(name, id);
            }
        }
    }

    let mut names = HashMap::new();
    let mut slot_counts = HashMap::new();
    if let Ok(table) = globals.get::<mlua::Table>("tbl") {
        for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
            let Some(id) = lua_u32(Some(pair.0)) else {
                continue;
            };
            let mlua::Value::Table(info) = pair.1 else {
                continue;
            };
            if let Some(count) = lua_u32(info.get::<Option<mlua::Value>>("slotCount").ok().flatten()) {
                slot_counts.insert(id, count.min(4) as u8);
            }
            if let Some(name) = info
                .get::<Option<mlua::Value>>("identifiedDisplayName")
                .ok()
                .flatten()
                .and_then(lua_text)
            {
                names.insert(id, name);
            }
        }
    }

    let mut groups = Vec::new();
    if let Ok(table) = globals.get::<mlua::Table>("Table") {
        for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
            let Some(id) = lua_i64(Some(pair.0)) else {
                continue;
            };
            let mlua::Value::Table(info) = pair.1 else {
                continue;
            };
            groups.push(parse_group(id, &info, &aegis, &names));
        }
    }
    groups.sort_by_key(|group| group.id);
    EnchantCatalog { groups, slot_counts }
}

fn parse_group(id: i64, info: &mlua::Table, aegis: &HashMap<String, u32>, names: &HashMap<u32, String>) -> EnchantGroup {
    let mut slot_order = Vec::new();
    if let Ok(Some(order)) = info.get::<Option<mlua::Table>>("SlotOrder") {
        for value in order.sequence_values::<mlua::Value>().flatten() {
            if let Some(slot) = lua_u32(Some(value)) {
                slot_order.push(slot.min(255) as u8);
            }
        }
    }
    let mut target_item_ids = Vec::new();
    if let Ok(Some(targets)) = info.get::<Option<mlua::Table>>("TargetItemTbl") {
        for value in targets.sequence_values::<mlua::Value>().flatten() {
            let (item_id, _) = resolve_item(value, aegis, names);
            if item_id != 0 {
                target_item_ids.push(item_id);
            }
        }
    }
    let (min_refine, min_grade) = info
        .get::<Option<mlua::Table>>("Condition")
        .ok()
        .flatten()
        .map(|condition| {
            (
                lua_u32(condition.get("MinRefine").ok()).unwrap_or(0).min(255) as u8,
                lua_u32(condition.get("MinGrade").ok()).unwrap_or(0).min(255) as u8,
            )
        })
        .unwrap_or((0, 0));
    let allow_random_options = match info.get::<Option<mlua::Value>>("bApproveRandomOpt").ok().flatten() {
        Some(value) => lua_bool(Some(value), true),
        None => true,
    };
    let (reset_enabled, reset_rate, reset_cost) = info
        .get::<Option<mlua::Table>>("Reset")
        .ok()
        .flatten()
        .map(|reset| {
            (
                lua_bool(reset.get("bReset").ok(), false),
                lua_i32(reset.get("Rate").ok()),
                EnchantCost {
                    zeny: lua_i32(reset.get("Zeny").ok()),
                    materials: materials_from(reset.get("MatTbl").ok(), aegis, names),
                },
            )
        })
        .unwrap_or((false, 0, EnchantCost::default()));
    let caution = info
        .get::<Option<mlua::Value>>("CautionMsg")
        .ok()
        .flatten()
        .and_then(lua_text)
        .unwrap_or_default();
    let mut slots = Vec::new();
    if let Ok(Some(slot_table)) = info.get::<Option<mlua::Table>>("Slot") {
        for pair in slot_table.pairs::<mlua::Value, mlua::Value>().flatten() {
            let Some(slot) = lua_u32(Some(pair.0)) else {
                continue;
            };
            let mlua::Value::Table(slot_info) = pair.1 else {
                continue;
            };
            slots.push(parse_slot(slot.min(255) as u8, &slot_info, aegis, names));
        }
    }
    slots.sort_by_key(|slot| slot.slot);
    EnchantGroup {
        id,
        slot_order,
        target_item_ids,
        min_refine,
        min_grade,
        allow_random_options,
        reset_enabled,
        reset_rate,
        reset_cost,
        caution,
        slots,
    }
}

fn parse_slot(slot: u8, info: &mlua::Table, aegis: &HashMap<String, u32>, names: &HashMap<u32, String>) -> SlotRecipe {
    let require = info
        .get::<Option<mlua::Table>>("RequireTbl")
        .ok()
        .flatten()
        .map(|require| EnchantCost {
            zeny: lua_i32(require.get("Zeny").ok()),
            materials: materials_from(require.get("MatTbl").ok(), aegis, names),
        })
        .unwrap_or_default();
    let mut grade_bonus = Vec::new();
    if let Ok(Some(bonuses)) = info.get::<Option<mlua::Table>>("GradeBonusTbl") {
        for pair in bonuses.pairs::<mlua::Value, mlua::Value>().flatten() {
            if let Some(grade) = lua_u32(Some(pair.0)) {
                grade_bonus.push(GradeBonus {
                    grade: grade.min(255) as u8,
                    bonus: lua_i32(Some(pair.1)),
                });
            }
        }
    }
    let mut random = Vec::new();
    if let Ok(Some(rates)) = info.get::<Option<mlua::Table>>("EnchantRateTbl") {
        for pair in rates.pairs::<mlua::Value, mlua::Value>().flatten() {
            let Some(grade) = lua_u32(Some(pair.0)) else {
                continue;
            };
            let mlua::Value::Table(items) = pair.1 else {
                continue;
            };
            for item in items.pairs::<mlua::Value, mlua::Value>().flatten() {
                let (item_id, name) = resolve_item(item.0, aegis, names);
                random.push(RandomEnchant {
                    grade: grade.min(255) as u8,
                    item_id,
                    name,
                });
            }
        }
    }
    let mut perfect = Vec::new();
    if let Ok(Some(perfects)) = info.get::<Option<mlua::Table>>("PerfectECTbl") {
        for pair in perfects.pairs::<mlua::Value, mlua::Value>().flatten() {
            let (item_id, name) = resolve_item(pair.0, aegis, names);
            let mlua::Value::Table(entry) = pair.1 else {
                continue;
            };
            perfect.push(PerfectEnchant {
                item_id,
                name,
                zeny: lua_i32(entry.get("Zeny").ok()),
                materials: materials_from(entry.get("MatTbl").ok(), aegis, names),
            });
        }
    }
    let mut upgrade = Vec::new();
    if let Ok(Some(upgrades)) = info.get::<Option<mlua::Table>>("UpgradeECTbl") {
        for pair in upgrades.pairs::<mlua::Value, mlua::Value>().flatten() {
            let (from_id, name) = resolve_item(pair.0, aegis, names);
            let mlua::Value::Table(entry) = pair.1 else {
                continue;
            };
            let (result_id, result_name) = entry
                .get::<Option<mlua::Value>>("ResultItemDB")
                .ok()
                .flatten()
                .map(|value| resolve_item(value, aegis, names))
                .unwrap_or((0, String::new()));
            upgrade.push(UpgradeEnchant {
                from_id,
                name,
                result_id,
                result_name,
                zeny: lua_i32(entry.get("Zeny").ok()),
                materials: materials_from(entry.get("MatTbl").ok(), aegis, names),
            });
        }
    }
    SlotRecipe {
        slot,
        require,
        success_rate: lua_i32(info.get("SuccessRate").ok()),
        grade_bonus,
        random,
        perfect,
        upgrade,
    }
}

fn materials_from(value: Option<mlua::Value>, aegis: &HashMap<String, u32>, names: &HashMap<u32, String>) -> Vec<EnchantMaterial> {
    let Some(mlua::Value::Table(table)) = value else {
        return Vec::new();
    };
    let mut materials = Vec::new();
    for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
        let count = lua_i32(Some(pair.1.clone()));
        let (item_id, name) = resolve_item(pair.0, aegis, names);
        materials.push(EnchantMaterial { item_id, count, name });
    }
    materials.sort_by_key(|material| material.item_id);
    materials
}

fn resolve_item(value: mlua::Value, aegis: &HashMap<String, u32>, names: &HashMap<u32, String>) -> (u32, String) {
    if let Some(id) = lua_u32(Some(value.clone())) {
        let name = names.get(&id).cloned().unwrap_or_else(|| format!("Item {id}"));
        return (id, name);
    }
    let Some(aegis_name) = lua_text(value) else {
        return (0, String::new());
    };
    let item_id = aegis.get(&aegis_name).copied().unwrap_or(0);
    let name = names.get(&item_id).cloned().unwrap_or(aegis_name);
    (item_id, name)
}

fn lua_text(value: mlua::Value) -> Option<String> {
    let mlua::Value::String(text) = value else {
        return None;
    };
    let bytes = text.as_bytes();
    if let Ok(utf8) = std::str::from_utf8(&bytes) {
        return Some(utf8.to_owned());
    }
    encoding_rs::EUC_KR
        .decode_without_bom_handling_and_without_replacement(&bytes)
        .map(|text| text.into_owned())
}

fn lua_i64(value: Option<mlua::Value>) -> Option<i64> {
    match value? {
        mlua::Value::Integer(number) => Some(number),
        mlua::Value::Number(number) => Some(number as i64),
        _ => None,
    }
}

fn lua_u32(value: Option<mlua::Value>) -> Option<u32> {
    let number = lua_i64(value)?;
    u32::try_from(number).ok()
}

fn lua_i32(value: Option<mlua::Value>) -> i32 {
    lua_i64(value).and_then(|number| i32::try_from(number).ok()).unwrap_or(0)
}

fn lua_bool(value: Option<mlua::Value>, default: bool) -> bool {
    match value {
        Some(mlua::Value::Boolean(flag)) => flag,
        Some(mlua::Value::Integer(number)) => number != 0,
        Some(mlua::Value::Number(number)) => number != 0.0,
        _ => default,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn prelude_lets_client_style_slot_order_run_past_the_first_group() {
        // Shape of the official EnchantList_f.lub: a vararg method that checks
        // `#arg` against MAX_SLOT_NUM before it stores SlotOrder.
        let catalog = super::catalog_from_source(
            r#"
Table = {}
CreateEnchantInfo = function()
    local info = {}
    info.SetSlotOrder = function(self, ...)
        if #arg < 1 or MAX_SLOT_NUM < #arg then
            MessageBox("bad slot order")
        end
        self.SlotOrder = {}
        for _, v in ipairs(arg) do
            if MAX_SLOT_NUM - 1 < v then MessageBox("slot out of range") end
            table.insert(self.SlotOrder, v)
        end
    end
    info.AddTargetItem = function(self, name)
        self.TargetItemTbl = self.TargetItemTbl or {}
        table.insert(self.TargetItemTbl, name)
    end
    return info
end
ItemDBNameTbl = { Hero_Insignia = 490163 }
Table[1] = CreateEnchantInfo()
Table[1]:SetSlotOrder(3, 2, 1)
Table[20] = CreateEnchantInfo()
Table[20]:SetSlotOrder(3, 2, 1)
Table[20]:AddTargetItem("Hero_Insignia")
"#,
        );
        let ids: Vec<i64> = catalog.groups.iter().map(|group| group.id).collect();
        assert_eq!(ids, vec![1, 20]);
        let hero = catalog.groups.iter().find(|group| group.id == 20).unwrap();
        assert_eq!(hero.slot_order, vec![3, 2, 1]);
        assert_eq!(hero.target_item_ids, vec![490163]);
    }

    use std::collections::HashMap;

    use korangar_networking::{InventoryItem, InventoryItemDetails};
    use ragnarok_bytes::ByteWriter;
    use ragnarok_packets::{
        EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions, PacketExt, RequestPerfectEnchantPacket,
        RequestRandomEnchantPacket, RequestResetEnchantPacket, RequestUpgradeEnchantPacket,
    };

    use super::*;
    use crate::world::ResourceMetadata;

    const FIXTURE: &str = r#"
        ItemDBNameTbl = { Knife = 1201, Bradium = 6224, Enchant_Stone = 4001, Enchant_Stone2 = 4002, Perfect_Stone = 4003 }
        tbl = {
            [1201] = { slotCount = 1, identifiedDisplayName = "Knife" },
            [6224] = { identifiedDisplayName = "Bradium" },
            [4001] = { identifiedDisplayName = "Enchant Stone" },
            [4002] = { identifiedDisplayName = "Enchant Stone II" },
            [4003] = { identifiedDisplayName = "Perfect Stone" },
        }
        Table = {
            [7] = {
                SlotOrder = {0, 1, 2, 3},
                TargetItemTbl = {"Knife"},
                Condition = { MinRefine = 7, MinGrade = 1 },
                bApproveRandomOpt = false,
                Reset = { bReset = true, Rate = 80000, Zeny = 1000, MatTbl = { Bradium = 1 } },
                CautionMsg = "가 50 % and 80%",
                Slot = {
                    [1] = {
                        RequireTbl = { Zeny = 500, MatTbl = { Bradium = 2 } },
                        SuccessRate = 50000,
                        GradeBonusTbl = { [1] = 5000 },
                        EnchantRateTbl = { [0] = { Enchant_Stone = 100 }, [1] = { Enchant_Stone = 80 } },
                        PerfectECTbl = { Perfect_Stone = { Zeny = 9000, MatTbl = { Bradium = 5 } } },
                        UpgradeECTbl = { Enchant_Stone = { ResultItemDB = "Enchant_Stone2", Zeny = 3000, MatTbl = { Bradium = 1 } } },
                    },
                },
            },
        }
    "#;

    fn opened() -> EnchantWindowState {
        let catalog = catalog_from_source(FIXTURE);
        let group = catalog.group(7).cloned().unwrap();
        let mut snap = ItemSnap {
            index: 4,
            item_id: 1201,
            name: "Knife".to_owned(),
            refine: 9,
            grade: 1,
            slot_count: catalog.slot_count(1201),
            identified: true,
            ..ItemSnap::default()
        };
        snap.set_slot(0, 99);
        let mut state = EnchantWindowState::default();
        state.open(7, Some(group), vec![snap], vec![OwnedStack { item_id: 6224, amount: 10 }]);
        state.select(4);
        state
    }

    #[test]
    fn lua_catalog_reads_targets_costs_and_slot_count() {
        let catalog = catalog_from_source(FIXTURE);
        let group = catalog.group(7).unwrap();
        assert_eq!(group.target_item_ids, vec![1201]);
        assert_eq!(group.min_refine, 7);
        assert_eq!(catalog.slot_count(1201), 1);
        let slot = group.slots.iter().find(|slot| slot.slot == 1).unwrap();
        assert_eq!(slot.require.zeny, 500);
        assert_eq!(slot.require.materials[0].name, "Bradium");
        assert_eq!(slot.perfect[0].item_id, 4003);
        assert_eq!(slot.upgrade[0].result_name, "Enchant Stone II");
        assert!(build_caution(group).contains("50%"));
        assert!(build_caution(group).contains("refine +7"));
    }

    #[test]
    fn eligibility_skips_worn_low_refine_and_the_wrong_item() {
        let group = catalog_from_source(FIXTURE).group(7).cloned().unwrap();
        let mut snap = ItemSnap {
            index: 4,
            item_id: 1201,
            refine: 9,
            grade: 1,
            identified: true,
            ..ItemSnap::default()
        };
        assert_eq!(rejection(&snap, &group), None);
        snap.equipped = true;
        assert_eq!(rejection(&snap, &group), Some("Item must be in the inventory."));
        snap.equipped = false;
        snap.refine = 6;
        assert_eq!(rejection(&snap, &group), Some("Refine level too low."));
        snap.refine = 9;
        snap.item_id = 9;
        assert!(rejection(&snap, &group).unwrap().contains("not valid"));
    }

    #[test]
    fn random_rate_adds_grade_bonus_and_skips_the_card_slot() {
        let mut state = opened();
        assert_eq!(state.action, ACTION_RANDOM);
        assert_eq!(state.chance, "Success 55%");
        assert_eq!(state.cost, "Cost 500z");
        assert!(state.slots[0].label.contains("card"));
        assert!(state.slots[1].selected);
        assert!(state.command().is_ok());
        state.show_result(3858, 0);
        state.set_stock(vec![OwnedStack { item_id: 6224, amount: 1 }]);
        assert!(state.requirement.starts_with("Missing"));
        assert!(matches!(state.command(), Err("Not enough materials.")));
    }

    #[test]
    fn perfect_upgrade_and_reset_encode_their_packets() {
        let mut state = opened();
        state.set_action(ACTION_PERFECT);
        assert!(state.command().is_err());
        state.pick_choice(0, 4003);
        let EnchantCommand::Perfect { group, index, item_id } = state.command().unwrap() else {
            panic!("perfect");
        };
        assert_eq!((group, index, item_id), (7, 6, 4003));
        let mut writer = ByteWriter::new();
        RequestPerfectEnchantPacket {
            enchant_group: group,
            inventory_index: index,
            item_id: ItemId(item_id),
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 16);
        assert_eq!(&writer.as_slice()[2..10], &7i64.to_le_bytes());
        assert_eq!(&writer.as_slice()[10..12], &6i16.to_le_bytes());

        state.show_result(3857, 4001);
        state.set_action(ACTION_UPGRADE);
        let EnchantCommand::Upgrade { slot, .. } = state.command().unwrap() else {
            panic!("upgrade");
        };
        assert_eq!(slot, 1);
        let mut writer = ByteWriter::new();
        RequestUpgradeEnchantPacket {
            enchant_group: 7,
            inventory_index: 6,
            slot,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 14);
        // The upgrade click is in flight. The result has to arrive before reset.
        state.show_result(3858, 0);

        state.set_action(ACTION_RESET);
        assert_eq!(state.chance, "Success 80%");
        let EnchantCommand::Reset { index, .. } = state.command().unwrap() else {
            panic!("reset");
        };
        let mut writer = ByteWriter::new();
        RequestResetEnchantPacket {
            enchant_group: 7,
            inventory_index: index,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 12);

        let mut writer = ByteWriter::new();
        RequestRandomEnchantPacket {
            enchant_group: 7,
            inventory_index: 6,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 12);
        assert_eq!(writer.as_slice()[0], 0x9B);
        assert_eq!(&writer.as_slice()[10..12], &6i16.to_le_bytes());
    }

    fn knife_item(slots: [u32; 4]) -> InventoryItem<ResourceMetadata> {
        let option = || ItemOptions {
            index: 0,
            value: 0,
            parameter: 0,
        };
        InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name: "Knife".to_owned(),
                description: String::new(),
            },
            index: InventoryIndex(4),
            item_id: ItemId(1201),
            item_type: 5,
            slot: slots,
            hire_expiration_date: 0,
            details: InventoryItemDetails::Equippable {
                equip_position: EquipPosition::empty(),
                equipped_position: EquipPosition::empty(),
                bind_on_equip_type: 0,
                w_item_sprite_number: 0,
                option_count: 0,
                option_data: [option(), option(), option(), option(), option()],
                forge_data: None,
                refinement_level: 9,
                enchantment_level: 1,
                flags: EquippableItemFlags::IDENTIFIED,
            },
        }
    }

    fn open_from(catalog: &EnchantCatalog, items: &[InventoryItem<ResourceMetadata>]) -> EnchantWindowState {
        let mut counts = HashMap::new();
        for item in items {
            counts.insert(item.item_id.0, catalog.slot_count(item.item_id.0));
        }
        let snaps = snaps_from_inventory(items, &[], &counts);
        let mut state = EnchantWindowState::default();
        state.open(7, catalog.group(7).cloned(), snaps, vec![OwnedStack {
            item_id: 6224,
            amount: 10,
        }]);
        state
    }

    #[test]
    fn success_is_written_onto_the_inventory_item_before_the_next_open() {
        let catalog = catalog_from_source(FIXTURE);
        let mut item = knife_item([99, 0, 0, 0]);
        let mut state = open_from(&catalog, &[item.clone()]);
        state.select(4);
        assert_eq!(state.active_slot, Some(1));
        assert!(matches!(state.command(), Ok(EnchantCommand::Random { .. })));
        let written = state.show_result(3857, 4001).expect("success writes cards");
        assert_eq!(written.index, 4);
        assert_eq!(written.slots, [99, 4001, 0, 0]);
        item.slot = written.slots;

        let mut again = open_from(&catalog, &[item]);
        again.select(4);
        let snap = again.snaps.iter().find(|snap| snap.index == 4).unwrap();
        assert_eq!(snap.slot_count, 1);
        assert_eq!(snap.slot1, 4001);
        // Slot 1 is the enchant just written. The next random enchant is slot 2,
        // which has no recipe, so the window offers an upgrade of that stone
        // instead of filling slot 1 a second time.
        assert_eq!(next_enchant_slot(snap, &again.group), Some(2));
        assert_eq!(again.slots[1].item_id, 4001);
        assert!(again.slots[1].label.contains("enchant"));
        assert!(again.slots[1].selected);
        assert_eq!(again.action, ACTION_UPGRADE);
        assert!(matches!(again.command(), Ok(EnchantCommand::Upgrade { slot: 1, index: 6, .. })));
        // That upgrade is the request in flight. A failed result releases the lock
        // without writing a second stone, so reset can be sent on its own.
        again.show_result(3858, 0);
        again.set_action(ACTION_RANDOM);
        assert_eq!(again.status, "That enchant is not available for this item.");
        assert_eq!(again.action, ACTION_UPGRADE);

        again.set_action(ACTION_RESET);
        assert!(matches!(again.command(), Ok(EnchantCommand::Reset { .. })));
        let cleared = again.show_result(3857, 0).expect("reset writes cards");
        // This fixture's SlotOrder is {0, 1, 2, 3}, so reset clears the card too.
        assert_eq!(cleared.slots, [0, 0, 0, 0]);
    }

    #[test]
    fn result_applies_the_request_that_was_sent() {
        let mut state = opened();
        assert!(matches!(state.command(), Ok(EnchantCommand::Random { index: 6, .. })));
        assert!(matches!(
            state.command(),
            Err("Waiting for the enchant result. Close and speak to the NPC again if nothing arrives.")
        ));
        state.set_action(ACTION_PERFECT);
        state.pick_choice(0, 4003);
        assert!(state.command().is_err());
        let written = state.show_result(3857, 4001).expect("pending random still applies");
        assert_eq!(written.slots, [99, 4001, 0, 0]);
        assert!(state.show_result(3857, 4002).is_none());
        assert_eq!(state.snaps.iter().find(|snap| snap.index == 4).unwrap().slot1, 4001);

        assert!(matches!(state.command(), Ok(EnchantCommand::Upgrade { slot: 1, .. })));
        state.set_action(ACTION_RESET);
        let upgraded = state.show_result(3857, 4002).expect("pending upgrade still applies");
        assert_eq!(upgraded.slots, [99, 4002, 0, 0]);

        assert!(matches!(state.command(), Ok(EnchantCommand::Reset { .. })));
        state.show_result(3858, 0);
        assert!(state.show_result(3857, 1).is_none());
        assert_eq!(state.snaps.iter().find(|snap| snap.index == 4).unwrap().slot1, 4002);
    }

    #[test]
    fn slot_order_counts_a_card_and_reset_clears_every_ordered_slot() {
        let source = r#"
            ItemDBNameTbl = { Knife = 1201, Bradium = 6224, Enchant_Stone = 4001 }
            tbl = {
                [1201] = { slotCount = 2, identifiedDisplayName = "Knife" },
                [6224] = { identifiedDisplayName = "Bradium" },
                [4001] = { identifiedDisplayName = "Enchant Stone" },
            }
            Table = {
                [7] = {
                    SlotOrder = {3, 2, 1},
                    TargetItemTbl = {"Knife"},
                    Reset = { bReset = true, Rate = 80000, Zeny = 1000, MatTbl = { Bradium = 1 } },
                    Slot = {
                        [2] = {
                            RequireTbl = { Zeny = 500, MatTbl = { Bradium = 1 } },
                            SuccessRate = 50000,
                            EnchantRateTbl = { [0] = { Enchant_Stone = 100 } },
                        },
                    },
                },
            }
        "#;
        let catalog = catalog_from_source(source);
        let item = knife_item([0, 501, 0, 0]);
        let mut state = open_from(&catalog, std::slice::from_ref(&item));
        state.select(4);
        let (slot_count, next) = {
            let snap = state.snaps.iter().find(|snap| snap.index == 4).unwrap();
            (snap.slot_count, next_enchant_slot(snap, &state.group))
        };
        assert_eq!(slot_count, 2);
        assert_eq!(next, Some(2));
        assert!(matches!(state.command(), Ok(EnchantCommand::Random { index: 6, .. })));
        let written = state.show_result(3857, 4001).expect("random writes SlotOrder[filled]");
        assert_eq!(written.slots, [0, 501, 4001, 0]);

        state.set_action(ACTION_RESET);
        assert!(matches!(state.command(), Ok(EnchantCommand::Reset { .. })));
        let cleared = state.show_result(3857, 0).expect("reset writes cards");
        assert_eq!(cleared.slots, [0, 0, 0, 0]);
    }
}
