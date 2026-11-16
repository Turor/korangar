//! Item reform, Laphine synthesis, and Laphine upgrade.
//!
//! The server packet only names the tool that opened the window. Eligible
//! gear, material counts, and the result preview come from the client tables
//! roBrowser reads (`ItemReformSystem.lub`, `lapineddukddakbox.lub` /
//! `LaphineSynthesisList.lub`, `lapineupgradebox.lub` /
//! `LaphineUpgradeList.lub`). When those files are not in the GRF, the window
//! stays on what the packet named and says the result is decided by the server.

use std::collections::HashMap;
use std::sync::OnceLock;

use korangar_interface::element::StateElement;
use korangar_loaders::FileLoader;
use korangar_networking::{InventoryItem, InventoryItemDetails};
use mlua::Lua;
use ragnarok_bytes::ByteWriter;
use ragnarok_packets::{
    InventoryIndex, ItemId, ItemReformClosePacket, ItemReformRequestPacket, LaphineSynthesisClosePacket, LaphineSynthesisMaterial,
    LaphineSynthesisRequestPacket, LaphineUpgradeClosePacket, LaphineUpgradeRequestPacket, Packet, PacketExt,
};
use rust_state::RustState;

use super::picker::PickerRow;
use crate::loaders::GameFileLoader;
use crate::world::{ResourceMetadata, execute_lua_chunk};

const RESULT_UNKNOWN: &str = "Result decided by server";

/// One inventory stack the workshop windows can offer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkshopSnap {
    pub index: u16,
    pub item_id: u32,
    pub name: String,
    pub amount: u16,
    pub refine: u8,
    pub option_count: u8,
    pub slot_count: u8,
    pub cards: Vec<u32>,
    pub card_names: Vec<String>,
    pub equip: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReformMaterialReq {
    pub item_id: u32,
    pub name: String,
    pub amount: u16,
}

/// One reform the opening tool can perform.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReformRecipe {
    pub catalyst_id: u32,
    pub base_item: u32,
    pub result_item: u32,
    pub result_name: String,
    pub refine_min: u8,
    pub refine_max: u8,
    pub option_min: u8,
    pub empty_socket: bool,
    pub refine_delta: i16,
    pub preserve_cards: bool,
    pub preserve_grade: bool,
    pub replace_options: bool,
    pub materials: Vec<ReformMaterialReq>,
    pub info: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SynthesisSource {
    pub item_id: u32,
    pub name: String,
    pub count: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SynthesisRecipe {
    pub item_id: u32,
    pub need_count: u16,
    pub refine_min: u8,
    pub refine_max: u8,
    pub hint: String,
    pub sources: Vec<SynthesisSource>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpgradeTarget {
    pub item_id: u32,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpgradeRecipe {
    pub item_id: u32,
    pub refine_min: u8,
    pub refine_max: u8,
    pub option_min: u8,
    pub warn_sockets: bool,
    pub hint: String,
    pub targets: Vec<UpgradeTarget>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkshopCatalog {
    pub reforms: Vec<ReformRecipe>,
    pub synthesis: Vec<SynthesisRecipe>,
    pub upgrades: Vec<UpgradeRecipe>,
    pub names: HashMap<u32, String>,
    pub slot_counts: HashMap<u32, u8>,
    pub reform_loaded: bool,
    pub synthesis_loaded: bool,
    pub upgrade_loaded: bool,
}

impl WorkshopCatalog {
    pub fn name(&self, item_id: u32) -> String {
        self.names
            .get(&item_id)
            .cloned()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| format!("Item {item_id}"))
    }

    pub fn slot_count(&self, item_id: u32) -> u8 {
        self.slot_counts.get(&item_id).copied().unwrap_or(0)
    }

    pub fn reforms_for(&self, catalyst_id: u32) -> Vec<ReformRecipe> {
        self.reforms
            .iter()
            .filter(|recipe| recipe.catalyst_id == catalyst_id)
            .cloned()
            .collect()
    }

    pub fn synthesis_for(&self, item_id: u32) -> Option<SynthesisRecipe> {
        self.synthesis.iter().find(|recipe| recipe.item_id == item_id).cloned()
    }

    pub fn upgrade_for(&self, item_id: u32) -> Option<UpgradeRecipe> {
        self.upgrades.iter().find(|recipe| recipe.item_id == item_id).cloned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QueuedStack {
    index: u16,
    count: u16,
    item_id: u32,
}

pub fn format_reform_result(result: i8, index: u16) -> String {
    if result == 0 {
        format!("Item reform succeeded (index {index})")
    } else {
        format!("Item reform failed (result {result}, index {index})")
    }
}

pub fn format_laphine_result(kind: &str, result: i32) -> String {
    if result == 0 {
        format!("{kind} succeeded")
    } else {
        format!("{kind} failed (result {result})")
    }
}

pub fn snap_label(snap: &WorkshopSnap) -> String {
    let mut label = if snap.equip {
        format!("+{} {}", snap.refine, snap.name)
    } else if snap.amount > 1 {
        format!("{} x{}", snap.name, snap.amount)
    } else {
        snap.name.clone()
    };
    let mut parts = Vec::new();
    for (index, card) in snap.cards.iter().enumerate() {
        if *card == 0 {
            if snap.slot_count > 0 {
                parts.push("empty slot".to_owned());
            }
            continue;
        }
        let name = snap
            .card_names
            .get(index)
            .map(String::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or("card");
        parts.push(name.to_owned());
    }
    if !parts.is_empty() {
        label.push_str(" · ");
        label.push_str(&parts.join(", "));
    }
    label
}

pub fn reform_accepts(recipe: &ReformRecipe, snap: &WorkshopSnap) -> bool {
    if snap.item_id != recipe.base_item || !snap.equip {
        return false;
    }
    if snap.refine < recipe.refine_min || snap.refine > recipe.refine_max {
        return false;
    }
    if snap.option_count < recipe.option_min {
        return false;
    }
    if recipe.empty_socket && snap.cards.iter().any(|card| *card != 0) {
        return false;
    }
    true
}

fn owned_amount(snaps: &[WorkshopSnap], item_id: u32) -> u32 {
    snaps
        .iter()
        .filter(|snap| snap.item_id == item_id)
        .map(|snap| u32::from(snap.amount))
        .fold(0, u32::saturating_add)
}

fn reform_preview(recipe: &ReformRecipe, snap: &WorkshopSnap) -> (String, String) {
    if recipe.result_item == 0 && recipe.result_name.is_empty() {
        return (RESULT_UNKNOWN.to_owned(), String::new());
    }
    let refine = (i32::from(snap.refine) + i32::from(recipe.refine_delta)).clamp(0, 20) as u8;
    let result_name = if recipe.result_name.is_empty() {
        format!("Item {}", recipe.result_item)
    } else {
        recipe.result_name.clone()
    };
    let result = format!("+{refine} {result_name}");
    let refine_line = if recipe.refine_delta == 0 {
        format!("refine +{} kept", snap.refine)
    } else {
        format!("refine +{} becomes +{refine}", snap.refine)
    };
    let cards = if recipe.preserve_cards { "cards kept" } else { "cards cleared" };
    let options = if recipe.replace_options {
        "options replaced"
    } else {
        "options kept"
    };
    let grade = if recipe.preserve_grade { "grade kept" } else { "grade cleared" };
    (result, format!("{refine_line}, {cards}, {options}, {grade}"))
}

fn materials_ready(recipe: &ReformRecipe, snaps: &[WorkshopSnap]) -> bool {
    recipe
        .materials
        .iter()
        .all(|material| owned_amount(snaps, material.item_id) >= u32::from(material.amount))
}

fn material_rows(recipe: &ReformRecipe, snaps: &[WorkshopSnap]) -> Vec<PickerRow> {
    let mut materials = recipe.materials.clone();
    materials.sort_by_key(|material| material.item_id);
    materials
        .into_iter()
        .map(|material| {
            let owned = owned_amount(snaps, material.item_id);
            let name = if material.name.is_empty() {
                format!("Item {}", material.item_id)
            } else {
                material.name
            };
            PickerRow {
                index: 0,
                item_id: material.item_id,
                label: format!("{name} {owned}/{}", material.amount),
                selected: false,
            }
        })
        .collect()
}

pub fn synthesis_source_ok(recipe: &SynthesisRecipe, source: &SynthesisSource, snap: &WorkshopSnap) -> bool {
    if snap.item_id != source.item_id {
        return false;
    }
    if snap.equip {
        snap.refine >= recipe.refine_min && snap.refine <= recipe.refine_max
    } else {
        snap.amount >= source.count && source.count > 0
    }
}

pub fn upgrade_target_ok(recipe: &UpgradeRecipe, snap: &WorkshopSnap) -> bool {
    if !recipe.targets.iter().any(|target| target.item_id == snap.item_id) {
        return false;
    }
    if snap.equip && (snap.refine < recipe.refine_min || snap.refine > recipe.refine_max) {
        return false;
    }
    if recipe.option_min > 0 && snap.option_count < recipe.option_min {
        return false;
    }
    true
}

fn encode_packet<P: Packet>(packet: &P) -> Vec<u8> {
    let mut writer = ByteWriter::new();
    packet.packet_to_bytes(&mut writer).expect("workshop packet fits");
    writer.into_inner()
}

pub fn reform_request_bytes(item_id: u32, index: u16) -> Vec<u8> {
    encode_packet(&ItemReformRequestPacket {
        item_id: ItemId(item_id),
        index: InventoryIndex(index),
    })
}

pub fn reform_close_bytes() -> Vec<u8> {
    encode_packet(&ItemReformClosePacket {})
}

pub fn synthesis_request_bytes(item_id: u32, materials: &[(u16, u16)]) -> Vec<u8> {
    encode_packet(&LaphineSynthesisRequestPacket {
        item_id: ItemId(item_id),
        materials: materials
            .iter()
            .map(|(index, count)| LaphineSynthesisMaterial {
                index: InventoryIndex(*index),
                count: *count,
            })
            .collect(),
    })
}

pub fn synthesis_close_bytes() -> Vec<u8> {
    encode_packet(&LaphineSynthesisClosePacket {})
}

pub fn upgrade_request_bytes(item_id: u32, index: u16) -> Vec<u8> {
    encode_packet(&LaphineUpgradeRequestPacket {
        item_id: ItemId(item_id),
        index: InventoryIndex(index),
    })
}

pub fn upgrade_close_bytes() -> Vec<u8> {
    encode_packet(&LaphineUpgradeClosePacket {})
}

/// One submitted synthesis stack: inventory index and count.
pub type SynthesisStack = (u16, u16);

/// Item reform UI. The opening id is the tool from `ZC_OPEN_REFORM_UI`.
#[derive(Default, RustState, StateElement)]
pub struct ItemReformState {
    pub status: String,
    pub catalyst_name: String,
    pub result_preview: String,
    pub kept: String,
    pub details: String,
    pub catalyst: Vec<PickerRow>,
    pub candidates: Vec<PickerRow>,
    pub materials: Vec<PickerRow>,
    pub submit_enabled: bool,
    #[hidden_element]
    catalyst_item_id: u32,
    #[hidden_element]
    selected: Option<u16>,
    #[hidden_element]
    recipes: Vec<ReformRecipe>,
    #[hidden_element]
    snaps: Vec<WorkshopSnap>,
}

impl ItemReformState {
    pub fn open(&mut self, item_id: u32, snaps: &[WorkshopSnap], catalog: &WorkshopCatalog) {
        self.catalyst_item_id = item_id;
        self.snaps = snaps.to_vec();
        self.recipes = catalog.reforms_for(item_id);
        self.selected = None;
        self.catalyst_name = snaps
            .iter()
            .find(|snap| snap.item_id == item_id)
            .map(|snap| snap.name.clone())
            .unwrap_or_else(|| catalog.name(item_id));
        self.catalyst = vec![PickerRow {
            index: 0,
            item_id,
            label: self.catalyst_name.clone(),
            selected: false,
        }];
        self.rebuild(catalog.reform_loaded);
    }

    pub fn refresh_snaps(&mut self, snaps: &[WorkshopSnap]) {
        let loaded = !self.recipes.is_empty() || !self.status.contains("No reform list");
        self.snaps = snaps.to_vec();
        if self.selected.is_some_and(|index| !self.eligible().any(|snap| snap.index == index)) {
            self.selected = None;
        }
        self.rebuild(loaded);
    }

    pub fn select(&mut self, index: u16) {
        if !self.eligible().any(|snap| snap.index == index) {
            return;
        }
        self.selected = Some(index);
        self.rebuild(!self.recipes.is_empty() || !self.status.contains("No reform list"));
    }

    pub fn request(&self) -> Result<(u32, u16), &'static str> {
        if self.catalyst_item_id == 0 {
            return Err("Reform is not open.");
        }
        if self.recipes.is_empty() {
            return Err("No reform list is loaded.");
        }
        let Some(index) = self.selected else {
            return Err("Choose an item.");
        };
        if !self.submit_enabled {
            return Err("Not enough materials.");
        }
        Ok((self.catalyst_item_id, index))
    }

    /// Success (`0`) closes the window, matching roBrowser. Other results stay.
    pub fn note_result(&mut self, index: u16, result: i8) -> bool {
        self.status = format_reform_result(result, index);
        if result == 0 {
            self.selected = None;
            self.submit_enabled = false;
            self.candidates.clear();
            self.materials.clear();
            self.kept.clear();
            true
        } else {
            false
        }
    }

    fn eligible(&self) -> impl Iterator<Item = &WorkshopSnap> {
        self.snaps
            .iter()
            .filter(|snap| self.recipes.iter().any(|recipe| reform_accepts(recipe, snap)))
    }

    fn rebuild(&mut self, list_loaded: bool) {
        let selected = self.selected;
        self.candidates = self
            .eligible()
            .map(|snap| PickerRow {
                index: snap.index,
                item_id: snap.item_id,
                label: snap_label(snap),
                selected: Some(snap.index) == selected,
            })
            .collect();
        let Some(index) = selected else {
            self.materials.clear();
            self.kept.clear();
            self.details.clear();
            self.submit_enabled = false;
            if self.recipes.is_empty() {
                self.result_preview = RESULT_UNKNOWN.to_owned();
                self.status = if list_loaded {
                    format!("{} has no reform entry. {RESULT_UNKNOWN}.", self.catalyst_name)
                } else {
                    format!("No reform list is loaded. {RESULT_UNKNOWN}.")
                };
            } else if self.candidates.is_empty() {
                self.result_preview = "Select an item to reform.".to_owned();
                self.status = "No inventory item matches this reform.".to_owned();
            } else {
                self.result_preview = "Select an item to reform.".to_owned();
                self.status = "Choose an item to reform.".to_owned();
            }
            return;
        };
        let Some(snap) = self.snaps.iter().find(|snap| snap.index == index).cloned() else {
            return;
        };
        let Some(recipe) = self.recipes.iter().find(|recipe| reform_accepts(recipe, &snap)).cloned() else {
            return;
        };
        let (preview, kept) = reform_preview(&recipe, &snap);
        self.result_preview = preview;
        self.kept = kept;
        self.details = recipe.info.clone();
        self.materials = material_rows(&recipe, &self.snaps);
        self.submit_enabled = materials_ready(&recipe, &self.snaps);
        self.status = if self.submit_enabled {
            "Reform is ready.".to_owned()
        } else {
            "Not enough materials.".to_owned()
        };
    }
}

/// Laphine synthesis (LaphineSys).
#[derive(Default, RustState, StateElement)]
pub struct LaphineSynthesisState {
    pub status: String,
    pub box_name: String,
    pub hint: String,
    pub requirement: String,
    pub candidates: Vec<PickerRow>,
    pub chosen: Vec<PickerRow>,
    pub submit_enabled: bool,
    #[hidden_element]
    item_id: u32,
    #[hidden_element]
    recipe: Option<SynthesisRecipe>,
    #[hidden_element]
    snaps: Vec<WorkshopSnap>,
    #[hidden_element]
    queued: Vec<QueuedStack>,
}

impl LaphineSynthesisState {
    pub fn open(&mut self, item_id: u32, snaps: &[WorkshopSnap], catalog: &WorkshopCatalog) {
        self.item_id = item_id;
        self.snaps = snaps.to_vec();
        self.queued.clear();
        self.recipe = catalog.synthesis_for(item_id);
        self.box_name = snaps
            .iter()
            .find(|snap| snap.item_id == item_id)
            .map(|snap| snap.name.clone())
            .unwrap_or_else(|| catalog.name(item_id));
        self.hint = self.recipe.as_ref().map(|recipe| recipe.hint.clone()).unwrap_or_default();
        if self.recipe.is_none() {
            self.requirement = String::new();
            self.candidates.clear();
            self.chosen.clear();
            self.submit_enabled = false;
            self.status = if catalog.synthesis_loaded {
                format!("{} has no synthesis entry. {RESULT_UNKNOWN}.", self.box_name)
            } else {
                format!("No synthesis list is loaded. {RESULT_UNKNOWN}.")
            };
            return;
        }
        self.rebuild();
    }

    pub fn toggle(&mut self, index: u16) {
        let Some(recipe) = self.recipe.clone() else {
            return;
        };
        if let Some(position) = self.queued.iter().position(|row| row.index == index) {
            self.queued.remove(position);
            self.rebuild();
            return;
        }
        let Some(snap) = self.snaps.iter().find(|snap| snap.index == index) else {
            return;
        };
        let Some(source) = recipe.sources.iter().find(|source| synthesis_source_ok(&recipe, source, snap)) else {
            return;
        };
        if self.queued.len() >= usize::from(recipe.need_count) {
            self.status = format!("Synthesis needs {} material(s).", recipe.need_count);
            return;
        }
        let count = if snap.equip { 1 } else { source.count.min(snap.amount) };
        if count == 0 {
            return;
        }
        self.queued.push(QueuedStack {
            index,
            count,
            item_id: snap.item_id,
        });
        self.rebuild();
    }

    pub fn clear_rows(&mut self) {
        self.queued.clear();
        if self.recipe.is_some() {
            self.rebuild();
        } else {
            self.chosen.clear();
            self.submit_enabled = false;
            self.status = "Synthesis list cleared.".to_owned();
        }
    }

    pub fn stacks(&self) -> Vec<SynthesisStack> {
        self.queued.iter().map(|row| (row.index, row.count)).collect()
    }

    pub fn request(&self) -> Result<(u32, Vec<SynthesisStack>), &'static str> {
        if self.item_id == 0 {
            return Err("Synthesis is not open.");
        }
        if self.recipe.is_none() {
            return Err("No synthesis list is loaded.");
        }
        if !self.submit_enabled {
            return Err("Add every required material.");
        }
        Ok((self.item_id, self.stacks()))
    }

    /// Success (`0`) closes and clears the queue, matching roBrowser.
    pub fn note_result(&mut self, result: i16) -> bool {
        self.status = format_laphine_result("Laphine synthesis", i32::from(result));
        if result == 0 {
            self.queued.clear();
            self.chosen.clear();
            self.candidates.clear();
            self.submit_enabled = false;
            self.requirement.clear();
            true
        } else {
            false
        }
    }

    fn rebuild(&mut self) {
        let Some(recipe) = self.recipe.clone() else {
            return;
        };
        let queued = self.queued.clone();
        self.candidates = self
            .snaps
            .iter()
            .filter(|snap| recipe.sources.iter().any(|source| synthesis_source_ok(&recipe, source, snap)))
            .filter(|snap| !queued.iter().any(|row| row.index == snap.index))
            .map(|snap| PickerRow {
                index: snap.index,
                item_id: snap.item_id,
                label: snap_label(snap),
                selected: false,
            })
            .collect();
        self.chosen = queued
            .iter()
            .map(|row| {
                let snap = self.snaps.iter().find(|snap| snap.index == row.index);
                let name = snap.map(|snap| snap.name.as_str()).unwrap_or("Item");
                PickerRow {
                    index: row.index,
                    item_id: row.item_id,
                    label: format!("{name} x{}", row.count),
                    selected: true,
                }
            })
            .collect();
        let need = recipe.need_count;
        let have = u16::try_from(self.queued.len()).unwrap_or(u16::MAX);
        self.requirement = format!("Materials {have}/{need}");
        self.submit_enabled = have == need && need > 0;
        self.status = if self.submit_enabled {
            "Synthesis is ready.".to_owned()
        } else if self.candidates.is_empty() && self.queued.is_empty() {
            "No inventory item matches this synthesis.".to_owned()
        } else {
            self.requirement.clone()
        };
    }
}

/// Laphine upgrade (LaphineUpg).
#[derive(Default, RustState, StateElement)]
pub struct LaphineUpgradeState {
    pub status: String,
    pub box_name: String,
    pub result_preview: String,
    pub candidates: Vec<PickerRow>,
    pub submit_enabled: bool,
    #[hidden_element]
    item_id: u32,
    #[hidden_element]
    selected: Option<u16>,
    #[hidden_element]
    recipe: Option<UpgradeRecipe>,
    #[hidden_element]
    snaps: Vec<WorkshopSnap>,
}

impl LaphineUpgradeState {
    pub fn open(&mut self, item_id: u32, snaps: &[WorkshopSnap], catalog: &WorkshopCatalog) {
        self.item_id = item_id;
        self.snaps = snaps.to_vec();
        self.selected = None;
        self.recipe = catalog.upgrade_for(item_id);
        self.box_name = snaps
            .iter()
            .find(|snap| snap.item_id == item_id)
            .map(|snap| snap.name.clone())
            .unwrap_or_else(|| catalog.name(item_id));
        self.rebuild(catalog.upgrade_loaded);
    }

    pub fn select(&mut self, index: u16) {
        if self.recipe.is_none() {
            return;
        }
        if self.selected == Some(index) {
            self.selected = None;
        } else if self.candidates.iter().any(|row| row.index == index) || self.snaps.iter().any(|snap| snap.index == index) {
            let allowed = self
                .recipe
                .as_ref()
                .is_some_and(|recipe| self.snaps.iter().any(|snap| snap.index == index && upgrade_target_ok(recipe, snap)));
            if !allowed {
                return;
            }
            self.selected = Some(index);
        } else {
            return;
        }
        self.rebuild(true);
    }

    pub fn request(&self) -> Result<(u32, u16), &'static str> {
        if self.item_id == 0 {
            return Err("Upgrade is not open.");
        }
        if self.recipe.is_none() {
            return Err("No upgrade list is loaded.");
        }
        let Some(index) = self.selected else {
            return Err("Choose an item.");
        };
        if !self.submit_enabled {
            return Err("Choose an item.");
        }
        Ok((self.item_id, index))
    }

    /// Success (`0`) closes, matching roBrowser. Other results stay.
    pub fn note_result(&mut self, result: u16) -> bool {
        self.status = format_laphine_result("Laphine upgrade", i32::from(result));
        if result == 0 {
            self.selected = None;
            self.submit_enabled = false;
            self.candidates.clear();
            true
        } else {
            false
        }
    }

    fn rebuild(&mut self, list_loaded: bool) {
        let Some(recipe) = self.recipe.clone() else {
            self.candidates.clear();
            self.submit_enabled = false;
            self.result_preview = RESULT_UNKNOWN.to_owned();
            self.status = if list_loaded {
                format!("{} has no upgrade entry. {RESULT_UNKNOWN}.", self.box_name)
            } else {
                format!("No upgrade list is loaded. {RESULT_UNKNOWN}.")
            };
            return;
        };
        let selected = self.selected;
        self.candidates = self
            .snaps
            .iter()
            .filter(|snap| upgrade_target_ok(&recipe, snap))
            .map(|snap| PickerRow {
                index: snap.index,
                item_id: snap.item_id,
                label: snap_label(snap),
                selected: Some(snap.index) == selected,
            })
            .collect();
        let hint = if recipe.hint.is_empty() {
            RESULT_UNKNOWN.to_owned()
        } else {
            recipe.hint.clone()
        };
        match selected.and_then(|index| self.snaps.iter().find(|snap| snap.index == index)) {
            Some(snap) => {
                self.result_preview = format!("{} → {hint}", snap_label(snap));
                self.submit_enabled = true;
                self.status = if recipe.warn_sockets && snap.cards.iter().any(|card| *card != 0) {
                    "This item has sockets. The server may refuse it.".to_owned()
                } else {
                    "Upgrade is ready.".to_owned()
                };
            }
            None => {
                self.result_preview = hint;
                self.submit_enabled = false;
                self.status = if self.candidates.is_empty() {
                    "No inventory item matches this upgrade.".to_owned()
                } else {
                    "Choose an item to upgrade.".to_owned()
                };
            }
        }
    }
}

pub fn snaps_from_inventory(items: &[InventoryItem<ResourceMetadata>], catalog: &WorkshopCatalog) -> Vec<WorkshopSnap> {
    items
        .iter()
        .map(|item| {
            let (amount, refine, option_count, equip) = match &item.details {
                InventoryItemDetails::Regular { amount, .. } => (*amount, 0, 0, false),
                InventoryItemDetails::Equippable {
                    refinement_level,
                    option_data,
                    ..
                } => {
                    let options = option_data.iter().filter(|option| option.index != 0).count() as u8;
                    (1, *refinement_level, options, true)
                }
            };
            let name = if item.metadata.name.is_empty() {
                catalog.name(item.item_id.0)
            } else {
                item.metadata.name.clone()
            };
            let slot_count = catalog.slot_count(item.item_id.0);
            let limit = if slot_count == 0 { 4 } else { usize::from(slot_count).min(4) };
            let mut cards = Vec::new();
            let mut card_names = Vec::new();
            for card in item.slot.iter().take(limit) {
                if *card == 0 && slot_count == 0 {
                    continue;
                }
                cards.push(*card);
                if *card == 0 {
                    card_names.push(String::new());
                } else {
                    card_names.push(catalog.name(*card));
                }
            }
            WorkshopSnap {
                index: item.index.0,
                item_id: item.item_id.0,
                name,
                amount,
                refine,
                option_count,
                slot_count,
                cards,
                card_names,
                equip,
            }
        })
        .collect()
}

static CATALOG: OnceLock<WorkshopCatalog> = OnceLock::new();

pub fn shared_catalog(loader: &GameFileLoader) -> &'static WorkshopCatalog {
    CATALOG.get_or_init(|| load_catalog(loader))
}

pub fn loaded_catalog() -> &'static WorkshopCatalog {
    CATALOG.get().unwrap_or_else(|| {
        static EMPTY: OnceLock<WorkshopCatalog> = OnceLock::new();
        EMPTY.get_or_init(WorkshopCatalog::default)
    })
}

const REFORM_PATHS: &[&str] = &[
    "data\\luafiles514\\lua files\\ItemReform\\ItemReformSystem.lub",
    "data/luafiles514/lua files/ItemReform/ItemReformSystem.lub",
];

const SYNTHESIS_PATHS: &[&str] = &[
    "data\\luafiles514\\lua files\\datainfo\\lapineddukddakbox.lub",
    "data/luafiles514/lua files/datainfo/lapineddukddakbox.lub",
    "data\\luafiles514\\lua files\\datainfo\\LaphineSynthesisList.lub",
    "data/luafiles514/lua files/datainfo/LaphineSynthesisList.lub",
    "data\\luafiles514\\lua files\\LaphineSynthesisList.lub",
    "data/luafiles514/lua files/LaphineSynthesisList.lub",
];

const UPGRADE_PATHS: &[&str] = &[
    "data\\luafiles514\\lua files\\datainfo\\lapineupgradebox.lub",
    "data/luafiles514/lua files/datainfo/lapineupgradebox.lub",
    "data\\luafiles514\\lua files\\datainfo\\LaphineUpgradeList.lub",
    "data/luafiles514/lua files/datainfo/LaphineUpgradeList.lub",
    "data\\luafiles514\\lua files\\LaphineUpgradeList.lub",
    "data/luafiles514/lua files/LaphineUpgradeList.lub",
];

const NAME_PATHS: &[&str] = &[
    "data\\luafiles514\\lua files\\ItemDBNameTbl.lub",
    "data/luafiles514/lua files/ItemDBNameTbl.lub",
    "System\\itemInfo.turoran.renewal11.lua",
    "System/itemInfo.turoran.renewal11.lua",
    "data\\luafiles514\\lua files\\datainfo\\iteminfo.lub",
    "data/luafiles514/lua files/datainfo/iteminfo.lub",
];

fn load_catalog(loader: &GameFileLoader) -> WorkshopCatalog {
    let lua = Lua::new();
    for (name, paths) in [
        ("ItemDBNameTbl.lub", NAME_PATHS),
        ("ItemReformSystem.lub", REFORM_PATHS),
        ("LaphineSynthesisList.lub", SYNTHESIS_PATHS),
        ("LaphineUpgradeList.lub", UPGRADE_PATHS),
    ] {
        if let Some(bytes) = paths.iter().find_map(|path| loader.get(path).ok()) {
            let _ = execute_lua_chunk(&lua, name, &bytes);
        }
    }
    catalog_from_lua(&lua)
}

pub fn catalog_from_lua_source(source: &str) -> Result<WorkshopCatalog, String> {
    let lua = Lua::new();
    execute_lua_chunk(&lua, "workshop", source.as_bytes()).map_err(|error| error.to_string())?;
    Ok(catalog_from_lua(&lua))
}

fn catalog_from_lua(lua: &Lua) -> WorkshopCatalog {
    let globals = lua.globals();
    let (aegis, mut names, slot_counts) = read_names(&globals);
    let mut catalog = WorkshopCatalog {
        names: names.clone(),
        slot_counts,
        ..WorkshopCatalog::default()
    };
    if let Ok(Some(info)) = globals.get::<Option<mlua::Table>>("ReformInfo") {
        catalog.reform_loaded = true;
        let list = globals.get::<Option<mlua::Table>>("ReformItemList").ok().flatten();
        catalog.reforms = parse_reforms(&info, list.as_ref(), &aegis, &mut names);
    }
    if let Some(sources) = laphine_sources(&globals) {
        catalog.synthesis_loaded = true;
        catalog.synthesis = parse_synthesis(&sources, &aegis, &names);
    }
    if let Some(targets) = laphine_targets(&globals) {
        catalog.upgrade_loaded = true;
        catalog.upgrades = parse_upgrades(&targets, &aegis, &names);
    }
    catalog.names = names;
    catalog
}

fn laphine_sources(globals: &mlua::Table) -> Option<mlua::Table> {
    if let Ok(Some(table)) = globals.get::<Option<mlua::Table>>("tblLapineDdukddakBox")
        && let Ok(Some(sources)) = table.get::<Option<mlua::Table>>("sources")
    {
        return Some(sources);
    }
    globals.get::<Option<mlua::Table>>("LaphineSynthesisList").ok().flatten()
}

fn laphine_targets(globals: &mlua::Table) -> Option<mlua::Table> {
    if let Ok(Some(table)) = globals.get::<Option<mlua::Table>>("tblLapineUpgradeBox")
        && let Ok(Some(targets)) = table.get::<Option<mlua::Table>>("targets")
    {
        return Some(targets);
    }
    globals.get::<Option<mlua::Table>>("LaphineUpgradeList").ok().flatten()
}

fn read_names(globals: &mlua::Table) -> (HashMap<String, u32>, HashMap<u32, String>, HashMap<u32, u8>) {
    let mut aegis = HashMap::new();
    let mut names = HashMap::new();
    if let Ok(Some(table)) = globals.get::<Option<mlua::Table>>("ItemDBNameTbl") {
        for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
            match (
                lua_text(pair.0.clone()),
                lua_u32(Some(pair.1.clone())),
                lua_text(pair.1.clone()),
            ) {
                (Some(name), Some(id), _) => {
                    aegis.insert(name, id);
                }
                (Some(name), None, _) => {
                    if let Some(id) = lua_u32(Some(pair.0)) {
                        names.insert(id, name);
                    }
                }
                _ => {
                    if let (Some(id), Some(name)) = (lua_u32(Some(pair.0)), lua_text(pair.1)) {
                        names.insert(id, name);
                    }
                }
            }
        }
    }
    let mut slot_counts = HashMap::new();
    if let Ok(Some(table)) = globals.get::<Option<mlua::Table>>("tbl") {
        for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
            let Some(id) = lua_u32(Some(pair.0)) else {
                continue;
            };
            let mlua::Value::Table(info) = pair.1 else {
                continue;
            };
            if let Some(count) = lua_u32(info.get("slotCount").ok()) {
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
    (aegis, names, slot_counts)
}

fn parse_reforms(
    info: &mlua::Table,
    list: Option<&mlua::Table>,
    aegis: &HashMap<String, u32>,
    names: &mut HashMap<u32, String>,
) -> Vec<ReformRecipe> {
    let mut by_key: HashMap<String, ReformRecipe> = HashMap::new();
    for pair in info.pairs::<mlua::Value, mlua::Value>().flatten() {
        let mlua::Value::Table(row) = pair.1 else {
            continue;
        };
        let key = lua_key(&pair.0);
        if key.is_empty() {
            continue;
        }
        by_key.insert(key, parse_reform_row(&row, aegis, names));
    }
    let Some(list) = list else {
        return Vec::new();
    };
    let mut recipes = Vec::new();
    for pair in list.pairs::<mlua::Value, mlua::Value>().flatten() {
        let (catalyst_id, catalyst_name) = resolve_item(pair.0, aegis, names);
        if catalyst_id != 0 && !catalyst_name.is_empty() {
            names.entry(catalyst_id).or_insert(catalyst_name);
        }
        let mlua::Value::Table(keys) = pair.1 else {
            continue;
        };
        for value in keys.sequence_values::<mlua::Value>().flatten() {
            let key = lua_key(&value);
            let Some(mut recipe) = by_key.get(&key).cloned() else {
                continue;
            };
            recipe.catalyst_id = catalyst_id;
            if catalyst_id != 0 {
                recipes.push(recipe);
            }
        }
    }
    recipes.sort_by_key(|recipe| (recipe.catalyst_id, recipe.base_item, recipe.result_item));
    recipes
}

fn parse_reform_row(row: &mlua::Table, aegis: &HashMap<String, u32>, names: &mut HashMap<u32, String>) -> ReformRecipe {
    let (base_item, base_name) = row
        .get::<Option<mlua::Value>>("BaseItem")
        .ok()
        .flatten()
        .map(|value| resolve_item(value, aegis, names))
        .unwrap_or((0, String::new()));
    let (result_item, result_name) = row
        .get::<Option<mlua::Value>>("ResultItem")
        .ok()
        .flatten()
        .map(|value| resolve_item(value, aegis, names))
        .unwrap_or((0, String::new()));
    if base_item != 0 {
        names.entry(base_item).or_insert(base_name);
    }
    if result_item != 0 {
        names.entry(result_item).or_insert_with(|| result_name.clone());
    }
    let mut materials = Vec::new();
    if let Ok(Some(mlua::Value::Table(table))) = row.get::<Option<mlua::Value>>("Material") {
        for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
            let amount = lua_u32(Some(pair.1)).unwrap_or(0).min(u32::from(u16::MAX)) as u16;
            let (item_id, name) = resolve_item(pair.0, aegis, names);
            if item_id != 0 {
                names.entry(item_id).or_insert_with(|| name.clone());
            }
            if amount > 0 && (item_id != 0 || !name.is_empty()) {
                materials.push(ReformMaterialReq { item_id, name, amount });
            }
        }
    }
    let code = row
        .get::<Option<mlua::Value>>("RandomOptionCode")
        .ok()
        .flatten()
        .and_then(lua_text)
        .unwrap_or_default();
    ReformRecipe {
        catalyst_id: 0,
        base_item,
        result_item,
        result_name,
        refine_min: lua_u8(row.get("NeedRefineMin").ok()),
        refine_max: lua_u8(row.get("NeedRefineMax").ok()),
        option_min: lua_u8(row.get("NeedOptionNumMin").ok()),
        empty_socket: lua_bool(row.get("IsEmptySocket").ok(), false),
        refine_delta: lua_i16(row.get("ChangeRefineValue").ok()),
        preserve_cards: lua_bool(row.get("PreserveSocketItem").ok(), true),
        preserve_grade: lua_bool(row.get("PreserveGrade").ok(), true),
        replace_options: !code.is_empty(),
        materials,
        info: lua_info(row.get("InformationString").ok()),
    }
}

fn parse_synthesis(sources: &mlua::Table, aegis: &HashMap<String, u32>, names: &HashMap<u32, String>) -> Vec<SynthesisRecipe> {
    let mut recipes = Vec::new();
    for pair in sources.pairs::<mlua::Value, mlua::Value>().flatten() {
        let mlua::Value::Table(row) = pair.1 else {
            continue;
        };
        let (item_id, _) = row
            .get::<Option<mlua::Value>>("ItemID")
            .ok()
            .flatten()
            .map(|value| resolve_item(value, aegis, names))
            .unwrap_or((0, String::new()));
        if item_id == 0 {
            continue;
        }
        let mut source_items = Vec::new();
        if let Ok(Some(table)) = row.get::<Option<mlua::Table>>("SourceItems") {
            for value in table.sequence_values::<mlua::Value>().flatten() {
                let mlua::Value::Table(item) = value else {
                    continue;
                };
                let name = item.get::<mlua::Value>(1).ok().and_then(lua_text).unwrap_or_default();
                let count = lua_u32(item.get(2).ok()).unwrap_or(0).min(u32::from(u16::MAX)) as u16;
                let id = item
                    .get::<mlua::Value>(3)
                    .ok()
                    .map(|value| resolve_item(value, aegis, names).0)
                    .unwrap_or(0);
                if id != 0 && count > 0 {
                    source_items.push(SynthesisSource { item_id: id, name, count });
                }
            }
        }
        recipes.push(SynthesisRecipe {
            item_id,
            need_count: lua_u32(row.get("NeedCount").ok()).unwrap_or(0).min(u32::from(u16::MAX)) as u16,
            refine_min: lua_u8(row.get("NeedRefineMin").ok()),
            refine_max: lua_u8(row.get("NeedRefineMax").ok()),
            hint: row
                .get::<Option<mlua::Value>>("NeedSource_String")
                .ok()
                .flatten()
                .and_then(lua_text)
                .unwrap_or_default(),
            sources: source_items,
        });
    }
    recipes.sort_by_key(|recipe| recipe.item_id);
    recipes
}

fn parse_upgrades(targets: &mlua::Table, aegis: &HashMap<String, u32>, names: &HashMap<u32, String>) -> Vec<UpgradeRecipe> {
    let mut recipes = Vec::new();
    for pair in targets.pairs::<mlua::Value, mlua::Value>().flatten() {
        let mlua::Value::Table(row) = pair.1 else {
            continue;
        };
        let (item_id, _) = row
            .get::<Option<mlua::Value>>("ItemID")
            .ok()
            .flatten()
            .map(|value| resolve_item(value, aegis, names))
            .unwrap_or((0, String::new()));
        if item_id == 0 {
            continue;
        }
        let mut target_items = Vec::new();
        if let Ok(Some(table)) = row.get::<Option<mlua::Table>>("TargetItems") {
            for value in table.sequence_values::<mlua::Value>().flatten() {
                let mlua::Value::Table(item) = value else {
                    continue;
                };
                let name = item.get::<mlua::Value>(1).ok().and_then(lua_text).unwrap_or_default();
                let id = item
                    .get::<mlua::Value>(2)
                    .ok()
                    .map(|value| resolve_item(value, aegis, names).0)
                    .unwrap_or(0);
                if id != 0 {
                    target_items.push(UpgradeTarget { item_id: id, name });
                }
            }
        }
        recipes.push(UpgradeRecipe {
            item_id,
            refine_min: lua_u8(row.get("NeedRefineMin").ok()),
            refine_max: lua_u8(row.get("NeedRefineMax").ok()),
            option_min: lua_u8(row.get("NeedOptionNumMin").ok()),
            warn_sockets: lua_bool(row.get("NotSocketEnchantItem").ok(), false),
            hint: row
                .get::<Option<mlua::Value>>("NeedSource_String")
                .ok()
                .flatten()
                .and_then(lua_text)
                .unwrap_or_default(),
            targets: target_items,
        });
    }
    recipes.sort_by_key(|recipe| recipe.item_id);
    recipes
}

fn resolve_item(value: mlua::Value, aegis: &HashMap<String, u32>, names: &HashMap<u32, String>) -> (u32, String) {
    if let Some(id) = lua_u32(Some(value.clone())) {
        let name = names.get(&id).cloned().unwrap_or_else(|| format!("Item {id}"));
        return (id, name);
    }
    let Some(text) = lua_text(value) else {
        return (0, String::new());
    };
    if let Ok(id) = text.parse::<u32>() {
        let name = names.get(&id).cloned().unwrap_or_else(|| format!("Item {id}"));
        return (id, name);
    }
    let item_id = aegis.get(&text).copied().unwrap_or(0);
    let name = if item_id == 0 {
        text
    } else {
        names.get(&item_id).cloned().unwrap_or(text)
    };
    (item_id, name)
}

fn lua_info(value: Option<mlua::Value>) -> String {
    match value {
        Some(mlua::Value::Table(table)) => {
            let mut lines = Vec::new();
            for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
                if let Some(text) = lua_text(pair.1) {
                    lines.push((lua_key(&pair.0), text));
                }
            }
            lines.sort_by(|left, right| left.0.cmp(&right.0));
            lines.into_iter().map(|(_, text)| text).collect::<Vec<_>>().join("\n")
        }
        Some(other) => lua_text(other).unwrap_or_default(),
        None => String::new(),
    }
}

fn lua_key(value: &mlua::Value) -> String {
    if let Some(number) = lua_i64(Some(value.clone())) {
        return number.to_string();
    }
    lua_text(value.clone()).unwrap_or_default()
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
        mlua::Value::Number(number) if number.is_finite() => Some(number as i64),
        mlua::Value::String(text) => std::str::from_utf8(&text.as_bytes()).ok().and_then(|text| text.trim().parse().ok()),
        _ => None,
    }
}

fn lua_u32(value: Option<mlua::Value>) -> Option<u32> {
    lua_i64(value).and_then(|number| u32::try_from(number).ok())
}

fn lua_u8(value: Option<mlua::Value>) -> u8 {
    lua_u32(value).unwrap_or(0).min(u32::from(u8::MAX)) as u8
}

fn lua_i16(value: Option<mlua::Value>) -> i16 {
    lua_i64(value).and_then(|number| i16::try_from(number).ok()).unwrap_or(0)
}

fn lua_bool(value: Option<mlua::Value>, default: bool) -> bool {
    match value {
        Some(mlua::Value::Boolean(flag)) => flag,
        Some(mlua::Value::Integer(number)) => number != 0,
        Some(mlua::Value::Number(number)) => number != 0.0,
        Some(_) => default,
        None => default,
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{
        InventoryIndex, ItemId, ItemReformOpenPacket, ItemReformResultPacket, LaphineSynthesisOpenPacket, LaphineSynthesisResultPacket,
        LaphineUpgradeOpenPacket, LaphineUpgradeResultPacket,
    };

    use super::{
        ReformMaterialReq, ReformRecipe, SynthesisRecipe, SynthesisSource, UpgradeRecipe, UpgradeTarget, WorkshopCatalog, WorkshopSnap,
        catalog_from_lua_source, reform_accepts, reform_close_bytes, reform_request_bytes, synthesis_request_bytes, upgrade_request_bytes,
    };

    fn reform_open_bytes(item_id: u32) -> Vec<u8> {
        super::encode_packet(&ItemReformOpenPacket { item_id: ItemId(item_id) })
    }

    fn reform_result_bytes(index: u16, result: i8) -> Vec<u8> {
        super::encode_packet(&ItemReformResultPacket {
            index: InventoryIndex(index),
            result,
        })
    }

    fn synthesis_open_bytes(item_id: u32) -> Vec<u8> {
        super::encode_packet(&LaphineSynthesisOpenPacket { item_id: ItemId(item_id) })
    }

    fn synthesis_result_bytes(result: i16) -> Vec<u8> {
        super::encode_packet(&LaphineSynthesisResultPacket { result })
    }

    fn upgrade_open_bytes(item_id: u32) -> Vec<u8> {
        super::encode_packet(&LaphineUpgradeOpenPacket { item_id: ItemId(item_id) })
    }

    fn upgrade_result_bytes(result: u16) -> Vec<u8> {
        super::encode_packet(&LaphineUpgradeResultPacket { result })
    }

    fn knife(index: u16, refine: u8, card: u32) -> WorkshopSnap {
        WorkshopSnap {
            index,
            item_id: 1201,
            name: "Knife".to_owned(),
            amount: 1,
            refine,
            option_count: 0,
            slot_count: if card == 0 { 0 } else { 1 },
            cards: if card == 0 { Vec::new() } else { vec![card] },
            card_names: if card == 0 { Vec::new() } else { vec!["Poring Card".to_owned()] },
            equip: true,
        }
    }

    fn ore(index: u16, amount: u16) -> WorkshopSnap {
        WorkshopSnap {
            index,
            item_id: 984,
            name: "Oridecon".to_owned(),
            amount,
            ..WorkshopSnap::default()
        }
    }

    fn recipe() -> ReformRecipe {
        ReformRecipe {
            catalyst_id: 4001,
            base_item: 1201,
            result_item: 1202,
            result_name: "Dagger".to_owned(),
            refine_min: 5,
            refine_max: 10,
            option_min: 0,
            empty_socket: true,
            refine_delta: 0,
            preserve_cards: true,
            preserve_grade: true,
            replace_options: false,
            materials: vec![ReformMaterialReq {
                item_id: 984,
                name: "Oridecon".to_owned(),
                amount: 2,
            }],
            info: "Keeps refine.".to_owned(),
        }
    }

    fn catalog() -> WorkshopCatalog {
        let mut names = std::collections::HashMap::new();
        names.insert(4001, "Hammer".to_owned());
        names.insert(1201, "Knife".to_owned());
        names.insert(1202, "Dagger".to_owned());
        names.insert(984, "Oridecon".to_owned());
        WorkshopCatalog {
            reforms: vec![recipe()],
            names,
            reform_loaded: true,
            ..WorkshopCatalog::default()
        }
    }

    #[test]
    fn reform_filters_refine_sockets_and_materials() {
        let mut state = super::ItemReformState::default();
        let snaps = vec![knife(2, 7, 0), knife(4, 3, 0), knife(5, 7, 4001), ore(6, 2)];
        state.open(4001, &snaps, &catalog());
        assert_eq!(state.catalyst_name, "Hammer");
        assert_eq!(state.candidates.len(), 1);
        assert!(state.candidates[0].label.contains("+7 Knife"));
        assert!(!state.submit_enabled);

        state.select(4);
        assert!(!state.submit_enabled);
        state.select(5);
        assert!(!state.submit_enabled);
        state.select(2);
        assert!(state.submit_enabled);
        assert!(state.result_preview.contains("+7 Dagger"));
        assert!(state.kept.contains("refine +7 kept"));
        assert!(state.kept.contains("cards kept"));
        assert!(state.kept.contains("options kept"));
        assert!(state.materials[0].label.contains("2/2"));
        assert_eq!(state.request(), Ok((4001, 2)));

        let short = vec![knife(2, 7, 0), ore(6, 1)];
        state.open(4001, &short, &catalog());
        state.select(2);
        assert!(!state.submit_enabled);
        assert!(state.materials[0].label.contains("1/2"));
        assert!(state.request().is_err());
    }

    #[test]
    fn missing_reform_list_does_not_guess() {
        let mut state = super::ItemReformState::default();
        state.open(4001, &[knife(2, 7, 0)], &WorkshopCatalog::default());
        assert!(state.candidates.is_empty());
        assert!(!state.submit_enabled);
        assert_eq!(state.result_preview, "Result decided by server");
        assert!(state.status.contains("Result decided by server"));
        assert!(state.note_result(2, 0));
        assert!(state.status.contains("succeeded"));
        let mut state = super::ItemReformState::default();
        assert!(!state.note_result(2, 1));
        assert!(state.status.contains("failed"));
    }

    #[test]
    fn synthesis_toggles_up_to_the_requirement() {
        let recipe = SynthesisRecipe {
            item_id: 7210,
            need_count: 2,
            refine_min: 0,
            refine_max: 20,
            hint: "Two ores".to_owned(),
            sources: vec![SynthesisSource {
                item_id: 984,
                name: "Oridecon".to_owned(),
                count: 1,
            }],
        };
        let catalog = WorkshopCatalog {
            synthesis: vec![recipe],
            synthesis_loaded: true,
            ..WorkshopCatalog::default()
        };
        let mut state = super::LaphineSynthesisState::default();
        state.open(7210, &[ore(1, 1), ore(2, 1), ore(3, 0)], &catalog);
        assert_eq!(state.candidates.len(), 2);
        state.toggle(1);
        assert!(!state.submit_enabled);
        assert_eq!(state.stacks(), vec![(1, 1)]);
        state.toggle(1);
        assert!(state.stacks().is_empty());
        state.toggle(1);
        state.toggle(2);
        assert!(state.submit_enabled);
        assert_eq!(state.request().unwrap().1, vec![(1, 1), (2, 1)]);
        state.toggle(3);
        assert_eq!(state.stacks().len(), 2);
        assert!(state.note_result(0));
        assert!(state.stacks().is_empty());
        assert!(!state.note_result(5));
        assert!(state.status.contains("failed"));
    }

    #[test]
    fn upgrade_filters_refine_and_shows_the_range() {
        let recipe = UpgradeRecipe {
            item_id: 7211,
            refine_min: 7,
            refine_max: 10,
            option_min: 0,
            warn_sockets: true,
            hint: "+7 to +10".to_owned(),
            targets: vec![UpgradeTarget {
                item_id: 1201,
                name: "Knife".to_owned(),
            }],
        };
        let catalog = WorkshopCatalog {
            upgrades: vec![recipe],
            upgrade_loaded: true,
            ..WorkshopCatalog::default()
        };
        let mut state = super::LaphineUpgradeState::default();
        state.open(7211, &[knife(2, 9, 0), knife(3, 4, 0)], &catalog);
        assert_eq!(state.candidates.len(), 1);
        assert!(state.result_preview.contains("+7 to +10"));
        assert!(!state.submit_enabled);
        state.select(3);
        assert!(!state.submit_enabled);
        state.select(2);
        assert!(state.submit_enabled);
        assert!(state.result_preview.contains("+9 Knife"));
        assert_eq!(state.request(), Ok((7211, 2)));
        state.select(2);
        assert!(!state.submit_enabled);
    }

    #[test]
    fn request_bytes_match_the_hercules_layout() {
        let reform = reform_request_bytes(4001, 3);
        assert_eq!(&reform[..2], &0x0B91u16.to_le_bytes());
        assert_eq!(&reform[2..6], &4001u32.to_le_bytes());
        assert_eq!(&reform[6..8], &(3u16 + 2).to_le_bytes());
        assert_eq!(reform_close_bytes(), 0x0B90u16.to_le_bytes());

        let synthesis = synthesis_request_bytes(7210, &[(4, 2)]);
        assert_eq!(synthesis.len(), 12);
        assert_eq!(&synthesis[..2], &0x0A4Fu16.to_le_bytes());
        assert_eq!(&synthesis[2..4], &12u16.to_le_bytes());
        assert_eq!(&synthesis[4..8], &7210u32.to_le_bytes());
        assert_eq!(&synthesis[8..10], &(4u16 + 2).to_le_bytes());
        assert_eq!(&synthesis[10..12], &2u16.to_le_bytes());

        let upgrade = upgrade_request_bytes(7211, 1);
        assert_eq!(&upgrade[..2], &0x0AB6u16.to_le_bytes());
        assert_eq!(&upgrade[2..6], &7211u32.to_le_bytes());
        assert_eq!(&upgrade[6..8], &(1u16 + 2).to_le_bytes());

        assert_eq!(reform_open_bytes(4001), [0x8F, 0x0B, 0xA1, 0x0F, 0x00, 0x00]);
        assert_eq!(reform_result_bytes(3, 0), [0x92, 0x0B, 0x05, 0x00, 0x00]);
        assert_eq!(synthesis_open_bytes(7210), [0x4E, 0x0A, 0x2A, 0x1C, 0x00, 0x00]);
        assert_eq!(synthesis_result_bytes(0), [0x50, 0x0A, 0x00, 0x00]);
        assert_eq!(upgrade_open_bytes(7211), [0xB4, 0x0A, 0x2B, 0x1C, 0x00, 0x00]);
        assert_eq!(upgrade_result_bytes(1), [0xB7, 0x0A, 0x01, 0x00]);
    }

    #[test]
    fn lua_tables_link_the_opening_item() {
        let source = r#"
            ItemDBNameTbl = { Knife = 1201, Dagger = 1202, Oridecon = 984, Hammer = 4001 }
            ReformInfo = {
                [1] = {
                    BaseItem = "Knife",
                    ResultItem = "Dagger",
                    NeedRefineMin = 5,
                    NeedRefineMax = 10,
                    NeedOptionNumMin = 0,
                    IsEmptySocket = true,
                    ChangeRefineValue = 1,
                    RandomOptionCode = "",
                    PreserveSocketItem = false,
                    PreserveGrade = true,
                    Material = { Oridecon = 2 },
                    InformationString = { "Keeps nothing else." },
                },
            }
            ReformItemList = { Hammer = { 1 } }
            tblLapineDdukddakBox = {
                sources = {
                    [1] = {
                        ItemID = 7210,
                        NeedCount = 1,
                        NeedRefineMin = 0,
                        NeedRefineMax = 20,
                        NeedSource_String = "One ore",
                        SourceItems = { { "Oridecon", 2, 984 } },
                    },
                },
            }
            tblLapineUpgradeBox = {
                targets = {
                    [1] = {
                        ItemID = 7211,
                        NeedRefineMin = 7,
                        NeedRefineMax = 10,
                        NeedSource_String = "+7 to +10",
                        NeedOptionNumMin = 0,
                        NotSocketEnchantItem = 1,
                        TargetItems = { { "Knife", 1201 } },
                    },
                },
            }
        "#;
        let catalog = catalog_from_lua_source(source).unwrap();
        assert!(catalog.reform_loaded && catalog.synthesis_loaded && catalog.upgrade_loaded);
        let reform = &catalog.reforms_for(4001)[0];
        assert_eq!(reform.base_item, 1201);
        assert_eq!(reform.result_item, 1202);
        assert_eq!(reform.refine_delta, 1);
        assert!(!reform.preserve_cards);
        assert_eq!(reform.materials[0].amount, 2);
        let snap = knife(2, 7, 0);
        assert!(reform_accepts(reform, &snap));
        let synthesis = catalog.synthesis_for(7210).unwrap();
        assert_eq!(synthesis.sources[0].count, 2);
        assert_eq!(synthesis.need_count, 1);
        let upgrade = catalog.upgrade_for(7211).unwrap();
        assert!(upgrade.warn_sockets);
        assert_eq!(upgrade.targets[0].item_id, 1201);
    }

    #[test]
    fn card_names_stay_on_the_row_label() {
        let label = super::snap_label(&knife(2, 9, 4001));
        assert!(label.contains("+9 Knife"));
        assert!(label.contains("Poring Card"));
    }
}
