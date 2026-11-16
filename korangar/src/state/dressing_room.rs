//! Client-only dressing room. Ctrl+click layers one view sprite on the
//! player's own body. Nothing here is written to the server.
//!
//! Headgear paths use [`headgear_sprite_candidates`] and
//! [`player_look_part_files`], the same accessory-id lookup the world
//! renderer uses. Garments use `RobeNameTable` (static `RobeTable` when the
//! lub has no row). Weapons and shields use `WeaponNameTable`, `ShieldTable`,
//! and `WeaponJobTable`, always under `인간족` or `방패`. A view id whose
//! sprite files are missing stays in the slot list and the window shows
//! [`NO_PREVIEW`]; the part is left out of the sprite so it never draws a
//! black box.

use std::sync::Arc;

use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::{EquipPosition, JobId, Sex};

use super::view_names::{shield_file_name, static_robe_name, weapon_file_suffix};
use super::vn_dialog::headgear_sprite_candidates;
use crate::graphics::Texture;
use crate::world::{
    AnimationData, Player, PlayerLook, hair_palette_grf_path, hair_palette_id, is_doram_job, player_look_part_files, weapon_job_sprite_name,
};

pub const NO_PREVIEW: &str = "No preview for this item";

const SLOT_COUNT: usize = 10;

const HEAD_BOTTOM: usize = 0;
const HEAD_MID: usize = 1;
const HEAD_TOP: usize = 2;
const COSTUME_HEAD_BOTTOM: usize = 3;
const COSTUME_HEAD_MID: usize = 4;
const COSTUME_HEAD_TOP: usize = 5;
const GARMENT: usize = 6;
const COSTUME_GARMENT: usize = 7;
const WEAPON: usize = 8;
const SHIELD: usize = 9;

const SLOT_NAMES: [&str; SLOT_COUNT] = [
    "head_bottom",
    "head_mid",
    "head_top",
    "costume_head_bottom",
    "costume_head_mid",
    "costume_head_top",
    "garment",
    "costume_garment",
    "weapon",
    "shield",
];

/// Creation previews use `0xE1000000` and the styling shop uses `0xF0000000`.
const DRESSING_PREVIEW_BASE: u32 = 0xE200_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewSlot {
    HeadBottom,
    HeadMid,
    HeadTop,
    CostumeHeadBottom,
    CostumeHeadMid,
    CostumeHeadTop,
    Garment,
    CostumeGarment,
    Weapon,
    Shield,
}

impl PreviewSlot {
    fn index(self) -> usize {
        match self {
            Self::HeadBottom => HEAD_BOTTOM,
            Self::HeadMid => HEAD_MID,
            Self::HeadTop => HEAD_TOP,
            Self::CostumeHeadBottom => COSTUME_HEAD_BOTTOM,
            Self::CostumeHeadMid => COSTUME_HEAD_MID,
            Self::CostumeHeadTop => COSTUME_HEAD_TOP,
            Self::Garment => GARMENT,
            Self::CostumeGarment => COSTUME_GARMENT,
            Self::Weapon => WEAPON,
            Self::Shield => SHIELD,
        }
    }

    fn from_index(index: usize) -> Option<Self> {
        Some(match index {
            HEAD_BOTTOM => Self::HeadBottom,
            HEAD_MID => Self::HeadMid,
            HEAD_TOP => Self::HeadTop,
            COSTUME_HEAD_BOTTOM => Self::CostumeHeadBottom,
            COSTUME_HEAD_MID => Self::CostumeHeadMid,
            COSTUME_HEAD_TOP => Self::CostumeHeadTop,
            GARMENT => Self::Garment,
            COSTUME_GARMENT => Self::CostumeGarment,
            WEAPON => Self::Weapon,
            SHIELD => Self::Shield,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        SLOT_NAMES[self.index()]
    }

    pub fn is_head(self) -> bool {
        matches!(
            self,
            Self::HeadBottom | Self::HeadMid | Self::HeadTop | Self::CostumeHeadBottom | Self::CostumeHeadMid | Self::CostumeHeadTop
        )
    }
}

/// One slot after a click or a reset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotView {
    pub slot: &'static str,
    /// View id layered by Ctrl+click. Zero when this slot is not being tried
    /// on.
    pub preview_id: u16,
    /// View id the portrait uses for this slot (preview, or worn when reset).
    pub shown_id: u16,
    /// Sprite path without an extension. Empty when no file was found.
    pub sprite_path: String,
}

/// What the local character is already wearing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WornLook {
    pub job_id: u16,
    pub female: bool,
    pub hair_style: u16,
    pub hair_color: u16,
    pub head_bottom: u16,
    pub head_top: u16,
    pub head_mid: u16,
    pub garment: u16,
    pub costume_garment: u16,
    pub weapon: u16,
    pub shield: u16,
}

impl Default for WornLook {
    fn default() -> Self {
        Self {
            job_id: 0,
            female: false,
            hair_style: 1,
            hair_color: 0,
            head_bottom: 0,
            head_top: 0,
            head_mid: 0,
            garment: 0,
            costume_garment: 0,
            weapon: 0,
            shield: 0,
        }
    }
}

/// Equip location and view sprite (`wItemSpriteNumber`) of one item.
///
/// `slot` is the first matching location. `also` holds every other location
/// the same view id is written into. A headpiece with both top and mid bits
/// fills both slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemView {
    pub slot: Option<PreviewSlot>,
    pub also: [Option<PreviewSlot>; 9],
    pub view_id: u16,
}

impl ItemView {
    pub fn only(slot: Option<PreviewSlot>, view_id: u16) -> Self {
        Self {
            slot,
            also: [None; 9],
            view_id,
        }
    }
}

/// Every preview slot this equip location covers. Head bits all match.
/// A two-handed weapon is a weapon, not a shield.
pub fn position_slots(position: EquipPosition) -> Vec<PreviewSlot> {
    if position.is_empty() {
        return Vec::new();
    }
    let mut slots = Vec::new();
    const HEADS: &[(EquipPosition, PreviewSlot)] = &[
        (EquipPosition::COSTUME_HEAD_LOWER, PreviewSlot::CostumeHeadBottom),
        (EquipPosition::COSTUME_HEAD_MIDDLE, PreviewSlot::CostumeHeadMid),
        (EquipPosition::COSTUME_HEAD_TOP, PreviewSlot::CostumeHeadTop),
        (EquipPosition::HEAD_LOWER, PreviewSlot::HeadBottom),
        (EquipPosition::HEAD_MIDDLE, PreviewSlot::HeadMid),
        (EquipPosition::HEAD_TOP, PreviewSlot::HeadTop),
    ];
    for (flag, slot) in HEADS {
        if position.contains(*flag) {
            slots.push(*slot);
        }
    }
    if position.contains(EquipPosition::COSTUME_GARMENT) {
        slots.push(PreviewSlot::CostumeGarment);
    }
    if position.contains(EquipPosition::GARMENT) {
        slots.push(PreviewSlot::Garment);
    }
    let two_hand = position.contains(EquipPosition::LEFT_RIGHT_HAND)
        || (position.contains(EquipPosition::RIGHT_HAND) && position.contains(EquipPosition::LEFT_HAND));
    if position.contains(EquipPosition::RIGHT_HAND) || position.contains(EquipPosition::LEFT_RIGHT_HAND) {
        slots.push(PreviewSlot::Weapon);
    }
    if position.contains(EquipPosition::LEFT_HAND) && !two_hand {
        slots.push(PreviewSlot::Shield);
    }
    slots
}

fn view_from_slots(slots: Vec<PreviewSlot>, view_id: u16) -> ItemView {
    let mut also = [None; 9];
    for (index, slot) in slots.iter().skip(1).take(also.len()).enumerate() {
        also[index] = Some(*slot);
    }
    ItemView {
        slot: slots.first().copied(),
        also,
        view_id,
    }
}

fn written_slots(view: &ItemView) -> Vec<PreviewSlot> {
    let mut slots = Vec::new();
    if let Some(slot) = view.slot {
        slots.push(slot);
    }
    for slot in view.also.into_iter().flatten() {
        if !slots.contains(&slot) {
            slots.push(slot);
        }
    }
    slots
}

pub fn item_view<T>(item: &InventoryItem<T>) -> ItemView {
    match &item.details {
        InventoryItemDetails::Equippable {
            equip_position,
            w_item_sprite_number,
            ..
        } => view_from_slots(position_slots(*equip_position), *w_item_sprite_number),
        InventoryItemDetails::Regular { .. } => ItemView::only(None, 0),
    }
}

pub fn is_dressing_preview(entity_id: u32) -> bool {
    entity_id & 0xFF00_0000 == DRESSING_PREVIEW_BASE
}

/// Worn job, hair, and headgear from the player sprite, plus garment / weapon /
/// shield view ids from items that are actually equipped.
pub fn worn_look_from_player<T>(player: Option<&Player>, items: &[InventoryItem<T>]) -> WornLook {
    let mut look = WornLook::default();
    if let Some(player) = player {
        let common = player.get_common();
        look.job_id = common.job_id.0;
        look.female = common.sex == Sex::Female;
        look.hair_style = u16::try_from(player.hair_id).unwrap_or(1);
        look.hair_color = player.hair_color;
        look.head_bottom = player.head_bottom;
        look.head_top = player.head_top;
        look.head_mid = player.head_mid;
    }
    look.garment = equipped_view(items, PreviewSlot::Garment);
    look.costume_garment = equipped_view(items, PreviewSlot::CostumeGarment);
    look.weapon = equipped_view(items, PreviewSlot::Weapon);
    look.shield = equipped_view(items, PreviewSlot::Shield);
    if player.is_none() {
        look.head_bottom = equipped_view(items, PreviewSlot::HeadBottom);
        look.head_top = equipped_view(items, PreviewSlot::HeadTop);
        look.head_mid = equipped_view(items, PreviewSlot::HeadMid);
    }
    look
}

fn equipped_view<T>(items: &[InventoryItem<T>], slot: PreviewSlot) -> u16 {
    items
        .iter()
        .filter_map(|item| {
            let InventoryItemDetails::Equippable {
                equipped_position,
                w_item_sprite_number,
                ..
            } = &item.details
            else {
                return None;
            };
            if equipped_position.is_empty() || !position_slots(*equipped_position).contains(&slot) {
                return None;
            }
            Some(*w_item_sprite_number)
        })
        .find(|view_id| *view_id > 0)
        .unwrap_or(0)
}

#[derive(Default, korangar_interface::element::StateElement, rust_state::RustState)]
pub struct DressingRoom {
    pub ctrl_down: bool,
    direction: u8,
    hide_other_headgear: bool,
    notice: String,
    job_id: u16,
    female: bool,
    hair_style: u16,
    hair_color: u16,
    worn_head_bottom: u16,
    worn_head_top: u16,
    worn_head_mid: u16,
    worn_garment: u16,
    worn_costume_garment: u16,
    worn_weapon: u16,
    worn_shield: u16,
    #[hidden_element]
    preview_ids: Vec<u16>,
    #[hidden_element]
    preview_paths: Vec<String>,
    #[hidden_element]
    worn_head_paths: Vec<String>,
    #[hidden_element]
    worn_extra_paths: Vec<String>,
    #[hidden_element]
    suffix_ids: Vec<u16>,
    #[hidden_element]
    suffix_names: Vec<String>,
    #[hidden_element]
    part_files: Vec<String>,
    #[hidden_element]
    requested_key: String,
    #[hidden_element]
    pending_entity: u32,
    #[hidden_element]
    generation: u32,
    #[hidden_element]
    preview: Option<Arc<AnimationData>>,
    /// Dyed head frames. Empty keeps the sprite's built-in palette.
    #[hidden_element]
    head_dye: Vec<Arc<Texture>>,
    #[hidden_element]
    head_dye_key: String,
}

impl DressingRoom {
    pub fn set_ctrl(&mut self, down: bool) {
        self.ctrl_down = down;
    }

    pub fn direction(&self) -> u8 {
        self.direction
    }

    pub fn hide_other_headgear(&self) -> bool {
        self.hide_other_headgear
    }

    pub fn notice(&self) -> &str {
        &self.notice
    }

    pub fn preview(&self) -> Option<&AnimationData> {
        self.preview.as_deref()
    }

    #[cfg(test)]
    fn part_files(&self) -> &[String] {
        &self.part_files
    }

    /// Bytes this preview would put on the wire. Closing, rotating, and
    /// trying items on never produce any.
    pub fn outbound_packets(&self) -> &'static [u8] {
        &[]
    }

    pub fn slot_views(&self) -> Vec<SlotView> {
        (0..SLOT_COUNT)
            .map(|index| {
                let (shown_id, sprite_path) = self.shown(index);
                SlotView {
                    slot: SLOT_NAMES[index],
                    preview_id: self.preview_ids.get(index).copied().unwrap_or(0),
                    shown_id,
                    sprite_path,
                }
            })
            .collect()
    }

    /// Layer `item` on `worn`. A later click replaces only its own slots.
    pub fn apply_item<T, S, E>(&mut self, worn: &WornLook, item: &InventoryItem<T>, suffix: S, exists: E)
    where
        S: Fn(u16) -> Option<String>,
        E: Fn(&str) -> bool,
    {
        self.apply_item_with_robes(worn, item, suffix, |_| None, exists);
    }

    /// Same as [`Self::apply_item`], with `RobeNameTable` overriding the static
    /// robe names.
    pub fn apply_item_with_robes<T, S, R, E>(&mut self, worn: &WornLook, item: &InventoryItem<T>, suffix: S, robe: R, exists: E)
    where
        S: Fn(u16) -> Option<String>,
        R: Fn(u16) -> Option<String>,
        E: Fn(&str) -> bool,
    {
        self.apply_view_with_robes(worn, item_view(item), suffix, robe, exists);
    }

    pub fn apply_view<S, E>(&mut self, worn: &WornLook, view: ItemView, suffix: S, exists: E)
    where
        S: Fn(u16) -> Option<String>,
        E: Fn(&str) -> bool,
    {
        self.apply_view_with_robes(worn, view, suffix, |_| None, exists);
    }

    fn apply_view_with_robes<S, R, E>(&mut self, worn: &WornLook, view: ItemView, suffix: S, robe: R, exists: E)
    where
        S: Fn(u16) -> Option<String>,
        R: Fn(u16) -> Option<String>,
        E: Fn(&str) -> bool,
    {
        self.ensure();
        self.copy_worn(worn);
        for view_id in [worn.head_bottom, worn.head_top, worn.head_mid, view.view_id] {
            self.remember_suffix(view_id, &suffix);
        }
        self.resolve_worn_paths(&robe, &exists);
        let slots = written_slots(&view);
        if view.view_id > 0 && !slots.is_empty() {
            let mut found = false;
            for slot in slots {
                let index = slot.index();
                self.preview_ids[index] = view.view_id;
                let path = self.resolve_path(slot, view.view_id, &robe, &exists);
                if !path.is_empty() {
                    found = true;
                }
                self.preview_paths[index] = path;
            }
            if found {
                self.notice.clear();
            } else {
                self.notice = NO_PREVIEW.to_owned();
            }
        } else {
            self.notice = NO_PREVIEW.to_owned();
        }
        self.rebuild_parts();
    }

    pub fn rotate(&mut self, delta: i8) {
        let next = i16::from(self.direction) + i16::from(delta);
        self.direction = next.rem_euclid(8) as u8;
    }

    /// Drop every tried-on piece and face the default direction.
    pub fn reset(&mut self) {
        self.ensure();
        self.direction = 0;
        self.hide_other_headgear = false;
        self.notice.clear();
        self.preview_ids.fill(0);
        for path in &mut self.preview_paths {
            path.clear();
        }
        self.rebuild_parts();
    }

    pub fn toggle_hide_other_headgear(&mut self) {
        self.hide_other_headgear = !self.hide_other_headgear;
        self.rebuild_parts();
    }

    /// Drop the loaded portrait. The tried-on ids stay; no packet is sent.
    pub fn clear_loaded(&mut self) {
        self.preview = None;
        self.requested_key.clear();
        self.pending_entity = 0;
        self.head_dye.clear();
        self.head_dye_key.clear();
    }

    /// Indexed head frames dyed with the player's hair colour. Empty means
    /// palette 0, or a missing palette file: paint the built-in palette.
    pub fn head_dye(&self) -> &[Arc<Texture>] {
        &self.head_dye
    }

    pub fn head_dye_key(&self) -> &str {
        &self.head_dye_key
    }

    pub fn set_head_dye(&mut self, key: String, frames: Vec<Arc<Texture>>) {
        self.head_dye_key = key;
        self.head_dye = frames;
    }

    /// Sprite and palette paths for the head, when the colour is not 0.
    ///
    /// The sprite file uses the style index. The palette uses `HairIndexTable`
    /// (`hair_palette_id`), which is not that index. Style 0 is looked up as 0.
    pub fn head_dye_request(&self) -> Option<HeadDyeRequest> {
        if self.hair_color == 0 {
            return None;
        }
        let head = self.part_files.get(1)?;
        if head.is_empty() || head.starts_with('?') {
            return None;
        }
        let doram = is_doram_job(JobId(self.job_id));
        let palette_id = hair_palette_id(self.sex(), doram, Some(usize::from(self.hair_style)));
        Some(HeadDyeRequest {
            sprite: format!("{head}.spr"),
            palette: hair_palette_grf_path(doram, self.female, palette_id, self.hair_color),
        })
    }

    pub fn take_load_request(&mut self) -> Option<(u32, Vec<String>)> {
        if self.part_files.is_empty() {
            return None;
        }
        let key = self.part_files.join("\n");
        if self.requested_key == key {
            return None;
        }
        self.requested_key = key;
        self.generation = self.generation.wrapping_add(1);
        let id = DRESSING_PREVIEW_BASE | (self.generation & 0x00FF_FFFF);
        self.pending_entity = id;
        Some((id, self.part_files.clone()))
    }

    pub fn store_animation(&mut self, entity_id: u32, animation: Arc<AnimationData>) -> bool {
        if entity_id != self.pending_entity || entity_id == 0 {
            return false;
        }
        self.preview = Some(animation);
        true
    }

    fn ensure(&mut self) {
        self.preview_ids.resize(SLOT_COUNT, 0);
        self.preview_paths.resize(SLOT_COUNT, String::new());
        self.worn_head_paths.resize(3, String::new());
        self.worn_extra_paths.resize(4, String::new());
    }

    fn copy_worn(&mut self, worn: &WornLook) {
        self.job_id = worn.job_id;
        self.female = worn.female;
        self.hair_style = worn.hair_style;
        self.hair_color = worn.hair_color;
        self.worn_head_bottom = worn.head_bottom;
        self.worn_head_top = worn.head_top;
        self.worn_head_mid = worn.head_mid;
        self.worn_garment = worn.garment;
        self.worn_costume_garment = worn.costume_garment;
        self.worn_weapon = worn.weapon;
        self.worn_shield = worn.shield;
    }

    fn remember_suffix<S>(&mut self, view_id: u16, suffix: &S)
    where
        S: Fn(u16) -> Option<String>,
    {
        if view_id == 0 {
            return;
        }
        let name = suffix(view_id).unwrap_or_default();
        if let Some(index) = self.suffix_ids.iter().position(|id| *id == view_id) {
            self.suffix_names[index] = name;
            return;
        }
        self.suffix_ids.push(view_id);
        self.suffix_names.push(name);
    }

    fn suffix_owned(&self, view_id: u16) -> Option<String> {
        let index = self.suffix_ids.iter().position(|id| *id == view_id)?;
        let name = self.suffix_names.get(index)?.clone();
        (!name.is_empty()).then_some(name)
    }

    fn resolve_worn_paths<R, E>(&mut self, robe: &R, exists: &E)
    where
        R: Fn(u16) -> Option<String>,
        E: Fn(&str) -> bool,
    {
        let heads = [self.worn_head_bottom, self.worn_head_mid, self.worn_head_top];
        for (index, view_id) in heads.into_iter().enumerate() {
            let slot = PreviewSlot::from_index(index).unwrap();
            self.worn_head_paths[index] = self.resolve_path(slot, view_id, robe, exists);
        }
        let extras = [
            (PreviewSlot::Garment, self.worn_garment),
            (PreviewSlot::Weapon, self.worn_weapon),
            (PreviewSlot::Shield, self.worn_shield),
            (PreviewSlot::CostumeGarment, self.worn_costume_garment),
        ];
        for (index, (slot, view_id)) in extras.into_iter().enumerate() {
            self.worn_extra_paths[index] = self.resolve_path(slot, view_id, robe, exists);
        }
    }

    fn resolve_path<R, E>(&self, slot: PreviewSlot, view_id: u16, robe: &R, exists: &E) -> String
    where
        R: Fn(u16) -> Option<String>,
        E: Fn(&str) -> bool,
    {
        if view_id == 0 {
            return String::new();
        }
        let sex_name = sex_folder(self.female);
        let (class_name, tribe) = self.job_and_tribe();
        let candidates = if slot.is_head() {
            headgear_sprite_candidates(view_id, sex_name, self.suffix_owned(view_id).as_deref())
        } else if matches!(slot, PreviewSlot::Garment | PreviewSlot::CostumeGarment) {
            match robe_label(view_id, robe) {
                Some(name) => garment_candidates(&name, sex_name, &class_name, tribe == "도람족"),
                None => Vec::new(),
            }
        } else if slot == PreviewSlot::Weapon {
            let job = weapon_job_sprite_name(JobId(self.job_id));
            weapon_candidates(view_id, sex_name, job)
        } else {
            let job = weapon_job_sprite_name(JobId(self.job_id));
            shield_candidates(view_id, sex_name, job)
        };
        candidates.into_iter().find(|path| exists(path)).unwrap_or_default()
    }

    fn job_and_tribe(&self) -> (String, String) {
        let sex = self.sex();
        let look = PlayerLook {
            hair: self.hair_style,
            head_bottom: 0,
            head_top: 0,
            head_mid: 0,
        };
        let files = player_look_part_files(JobId(self.job_id), sex, &look, |_| None);
        let body = files.first().map(String::as_str).unwrap_or("");
        let tribe = body.split('\\').next().unwrap_or("인간족").to_owned();
        let file = body.rsplit('\\').next().unwrap_or("초보자_남");
        let marker = format!("_{}", sex_folder(self.female));
        let job = file.strip_suffix(&marker).unwrap_or(file).to_owned();
        (job, tribe)
    }

    fn sex(&self) -> Sex {
        if self.female { Sex::Female } else { Sex::Male }
    }

    fn shown(&self, index: usize) -> (u16, String) {
        let preview_id = self.preview_ids.get(index).copied().unwrap_or(0);
        let preview_path = self.preview_paths.get(index).cloned().unwrap_or_default();
        if preview_id > 0 {
            return (preview_id, preview_path);
        }
        let Some(slot) = PreviewSlot::from_index(index) else {
            return (0, String::new());
        };
        if slot.is_head() && self.hide_other_headgear {
            return (0, String::new());
        }
        let (worn_id, worn_path) = match slot {
            PreviewSlot::HeadBottom => (self.worn_head_bottom, self.worn_head_paths.first().cloned().unwrap_or_default()),
            PreviewSlot::HeadMid => (self.worn_head_mid, self.worn_head_paths.get(1).cloned().unwrap_or_default()),
            PreviewSlot::HeadTop => (self.worn_head_top, self.worn_head_paths.get(2).cloned().unwrap_or_default()),
            PreviewSlot::Garment => (self.worn_garment, self.worn_extra_paths.first().cloned().unwrap_or_default()),
            PreviewSlot::Weapon => (self.worn_weapon, self.worn_extra_paths.get(1).cloned().unwrap_or_default()),
            PreviewSlot::Shield => (self.worn_shield, self.worn_extra_paths.get(2).cloned().unwrap_or_default()),
            PreviewSlot::CostumeHeadBottom | PreviewSlot::CostumeHeadMid | PreviewSlot::CostumeHeadTop => (0, String::new()),
            PreviewSlot::CostumeGarment => (
                self.worn_costume_garment,
                self.worn_extra_paths.get(3).cloned().unwrap_or_default(),
            ),
        };
        (worn_id, worn_path)
    }

    fn visual_head(&self, costume_index: usize, regular_index: usize, worn: u16) -> u16 {
        let costume = self.preview_ids.get(costume_index).copied().unwrap_or(0);
        if costume > 0 {
            return costume;
        }
        let preview = self.preview_ids.get(regular_index).copied().unwrap_or(0);
        if preview > 0 {
            return preview;
        }
        if self.hide_other_headgear { 0 } else { worn }
    }

    fn extra_path(&self, preview_index: usize, worn_index: usize) -> String {
        let preview_id = self.preview_ids.get(preview_index).copied().unwrap_or(0);
        if preview_id > 0 {
            return self.preview_paths.get(preview_index).cloned().unwrap_or_default();
        }
        let worn_id = match worn_index {
            0 => self.worn_garment,
            1 => self.worn_weapon,
            _ => self.worn_shield,
        };
        if worn_id == 0 {
            String::new()
        } else {
            self.worn_extra_paths.get(worn_index).cloned().unwrap_or_default()
        }
    }

    fn rebuild_parts(&mut self) {
        let look = PlayerLook {
            hair: self.hair_style,
            head_bottom: self.visual_head(COSTUME_HEAD_BOTTOM, HEAD_BOTTOM, self.worn_head_bottom),
            head_top: self.visual_head(COSTUME_HEAD_TOP, HEAD_TOP, self.worn_head_top),
            head_mid: self.visual_head(COSTUME_HEAD_MID, HEAD_MID, self.worn_head_mid),
        };
        let mut files = player_look_part_files(JobId(self.job_id), self.sex(), &look, |view_id| self.suffix_owned(view_id));
        let costume_garment = self.preview_ids.get(COSTUME_GARMENT).copied().unwrap_or(0);
        let garment_path = if costume_garment > 0 {
            self.preview_paths.get(COSTUME_GARMENT).cloned().unwrap_or_default()
        } else if self.worn_costume_garment > 0 && self.preview_ids.get(GARMENT).copied().unwrap_or(0) == 0 {
            self.worn_extra_paths.get(3).cloned().unwrap_or_default()
        } else {
            self.extra_path(GARMENT, 0)
        };
        for path in [garment_path, self.extra_path(WEAPON, 1), self.extra_path(SHIELD, 2)] {
            if !path.is_empty() {
                files.push(format!("?{path}"));
            }
        }
        let key = files.join("\n");
        if self.requested_key != key {
            self.preview = None;
        }
        self.part_files = files;
    }
}

fn sex_folder(female: bool) -> &'static str {
    if female { "여" } else { "남" }
}

/// Palette request for the head sprite. Palette 0 is not a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadDyeRequest {
    pub sprite: String,
    pub palette: String,
}

fn robe_label<R>(view_id: u16, robe: &R) -> Option<String>
where
    R: Fn(u16) -> Option<String>,
{
    robe(view_id)
        .filter(|name| !name.is_empty())
        .or_else(|| static_robe_name(view_id).map(str::to_owned))
}

fn garment_candidates(name: &str, sex_name: &str, class_name: &str, doram: bool) -> Vec<String> {
    let mut paths = vec![format!("로브\\{name}\\{sex_name}\\{class_name}_{sex_name}")];
    if doram {
        paths.push(format!("로브\\{name}\\{name}_doram"));
    } else {
        paths.push(format!("로브\\{name}\\{name}"));
    }
    paths
}

fn weapon_candidates(view_id: u16, sex_name: &str, job: &str) -> Vec<String> {
    let suffix = weapon_file_suffix(view_id);
    vec![format!("인간족\\{job}\\{job}_{sex_name}{suffix}")]
}

fn shield_candidates(view_id: u16, sex_name: &str, job: &str) -> Vec<String> {
    let name = shield_file_name(view_id);
    vec![format!("방패\\{job}\\{job}_{sex_name}_{name}")]
}

#[cfg(test)]
mod tests {
    use korangar_networking::NoMetadata;
    use ragnarok_packets::{EquippableItemFlags, InventoryIndex, ItemId, ItemOptions, RegularItemFlags};

    use super::*;

    fn options() -> [ItemOptions; 5] {
        [
            ItemOptions {
                index: 0,
                value: 0,
                parameter: 0,
            },
            ItemOptions {
                index: 0,
                value: 0,
                parameter: 0,
            },
            ItemOptions {
                index: 0,
                value: 0,
                parameter: 0,
            },
            ItemOptions {
                index: 0,
                value: 0,
                parameter: 0,
            },
            ItemOptions {
                index: 0,
                value: 0,
                parameter: 0,
            },
        ]
    }

    fn equip(index: u16, position: EquipPosition, view_id: u16) -> InventoryItem<NoMetadata> {
        InventoryItem {
            metadata: NoMetadata,
            index: InventoryIndex(index),
            item_id: ItemId(u32::from(index)),
            item_type: 5,
            slot: [0; 4],
            hire_expiration_date: 0,
            details: InventoryItemDetails::Equippable {
                equip_position: position,
                equipped_position: EquipPosition::NONE,
                bind_on_equip_type: 0,
                w_item_sprite_number: view_id,
                option_count: 0,
                option_data: options(),
                forge_data: None,
                refinement_level: 0,
                enchantment_level: 0,
                flags: EquippableItemFlags::empty(),
            },
        }
    }

    fn potion() -> InventoryItem<NoMetadata> {
        InventoryItem {
            metadata: NoMetadata,
            index: InventoryIndex(9),
            item_id: ItemId(501),
            item_type: 0,
            slot: [0; 4],
            hire_expiration_date: 0,
            details: InventoryItemDetails::Regular {
                amount: 5,
                equipped_position: EquipPosition::NONE,
                flags: RegularItemFlags::empty(),
            },
        }
    }

    fn suffix(view_id: u16) -> Option<String> {
        match view_id {
            5 => Some("_리본".to_owned()),
            8 => Some("_고글".to_owned()),
            _ => None,
        }
    }

    fn exists(path: &str) -> bool {
        path.ends_with("여_리본") || path.ends_with("여_고글") || path.ends_with("초보자_남_검")
    }

    #[test]
    fn stacking_reset_and_a_potion_send_nothing() {
        let worn = WornLook {
            job_id: 0,
            female: true,
            hair_style: 2,
            hair_color: 4,
            head_bottom: 1,
            head_top: 2,
            head_mid: 0,
            garment: 0,
            costume_garment: 0,
            weapon: 0,
            shield: 0,
        };
        let mut room = DressingRoom::default();
        room.apply_item(&worn, &equip(1, EquipPosition::HEAD_TOP, 8), suffix, exists);
        room.apply_item(&worn, &equip(2, EquipPosition::COSTUME_HEAD_MIDDLE, 5), suffix, exists);
        let views = room.slot_views();
        let top = views.iter().find(|slot| slot.slot == "head_top").unwrap();
        let costume = views.iter().find(|slot| slot.slot == "costume_head_mid").unwrap();
        assert_eq!(top.preview_id, 8);
        assert!(top.sprite_path.contains("악세사리"));
        assert!(top.sprite_path.contains("_고글"));
        assert_eq!(costume.preview_id, 5);
        assert!(costume.sprite_path.ends_with("여_리본"));

        room.apply_item(&worn, &potion(), suffix, exists);
        assert_eq!(room.notice(), NO_PREVIEW);
        assert_eq!(
            room.slot_views().iter().find(|slot| slot.slot == "head_top").unwrap().preview_id,
            8
        );

        room.rotate(1);
        room.rotate(1);
        assert_eq!(room.direction(), 2);
        room.rotate(-1);
        assert_eq!(room.direction(), 1);
        room.reset();
        let top = room.slot_views().into_iter().find(|slot| slot.slot == "head_top").unwrap();
        assert_eq!(top.preview_id, 0);
        assert_eq!(top.shown_id, 2);
        assert_eq!(room.direction(), 0);
        assert!(room.outbound_packets().is_empty());
        room.clear_loaded();
        assert!(room.outbound_packets().is_empty());
    }

    #[test]
    fn a_missing_sprite_file_keeps_the_view_id_and_skips_the_part() {
        let worn = WornLook {
            female: false,
            hair_style: 1,
            ..WornLook::default()
        };
        let mut room = DressingRoom::default();
        room.apply_view(&worn, ItemView::only(Some(PreviewSlot::HeadTop), 9), suffix, |_| false);
        let top = room.slot_views().into_iter().find(|slot| slot.slot == "head_top").unwrap();
        assert_eq!(top.preview_id, 9);
        assert!(top.sprite_path.is_empty());
        assert_eq!(room.notice(), NO_PREVIEW);
        assert!(
            room.part_files()
                .iter()
                .filter(|file| file.contains("남_9"))
                .all(|file| file.starts_with('?'))
        );

        room.apply_view(&worn, ItemView::only(Some(PreviewSlot::Weapon), 3), suffix, exists);
        let weapon = room.slot_views().into_iter().find(|slot| slot.slot == "weapon").unwrap();
        assert_eq!(weapon.preview_id, 3);
        assert_eq!(weapon.sprite_path, "인간족\\초보자\\초보자_남_검");
        assert!(!weapon.sprite_path.contains("초보자_남_3"));
        assert!(
            room.part_files()
                .iter()
                .any(|file| file.starts_with('?') && file.contains("초보자_남_검"))
        );
    }

    #[test]
    fn hide_other_headgear_drops_worn_hats_and_keeps_the_preview() {
        let worn = WornLook {
            head_bottom: 4,
            head_top: 2,
            ..WornLook::default()
        };
        let mut room = DressingRoom::default();
        room.apply_view(&worn, ItemView::only(Some(PreviewSlot::HeadTop), 8), suffix, exists);
        room.toggle_hide_other_headgear();
        let views = room.slot_views();
        let top = views.iter().find(|slot| slot.slot == "head_top").unwrap();
        let bottom = views.iter().find(|slot| slot.slot == "head_bottom").unwrap();
        assert_eq!(top.shown_id, 8);
        assert_eq!(bottom.shown_id, 0);
        assert!(room.hide_other_headgear());
    }

    #[test]
    fn gear_paths_use_the_sprite_name_tables() {
        let mut room = DressingRoom::default();
        let male = WornLook {
            female: false,
            hair_style: 1,
            ..WornLook::default()
        };
        let hit = |path: &str| {
            path.ends_with("초보자_남_검")
                || path.ends_with("초보자_남_가드")
                || path.ends_with("기사_남_검")
                || path.contains("천사날개")
                || path.contains("summoner_남_검")
        };
        room.apply_item(&male, &equip(1, EquipPosition::RIGHT_HAND, 2), suffix, hit);
        let sword = room.slot_views().into_iter().find(|slot| slot.slot == "weapon").unwrap();
        assert_eq!(sword.sprite_path, "인간족\\초보자\\초보자_남_검");

        room.apply_item(&male, &equip(2, EquipPosition::LEFT_HAND, 1), suffix, hit);
        let guard = room.slot_views().into_iter().find(|slot| slot.slot == "shield").unwrap();
        assert_eq!(guard.sprite_path, "방패\\초보자\\초보자_남_가드");
        room.apply_item(&male, &equip(3, EquipPosition::LEFT_HAND, 9), suffix, hit);
        let unknown_shield = room.slot_views().into_iter().find(|slot| slot.slot == "shield").unwrap();
        assert_eq!(unknown_shield.sprite_path, "방패\\초보자\\초보자_남_가드");
        assert!(!unknown_shield.sprite_path.contains("_9"));

        let rune = WornLook { job_id: 4054, ..male };
        room.apply_item(&rune, &equip(4, EquipPosition::RIGHT_HAND, 2), suffix, hit);
        let rune_sword = room.slot_views().into_iter().find(|slot| slot.slot == "weapon").unwrap();
        assert_eq!(rune_sword.sprite_path, "인간족\\기사\\기사_남_검");
        assert!(!rune_sword.sprite_path.contains("룬나이트"));

        let dragon = WornLook { job_id: 4252, ..male };
        room.apply_item(&dragon, &equip(5, EquipPosition::RIGHT_HAND, 2), suffix, hit);
        let dragon_sword = room.slot_views().into_iter().find(|slot| slot.slot == "weapon").unwrap();
        assert_eq!(dragon_sword.sprite_path, "인간족\\기사\\기사_남_검");

        let doram = WornLook { job_id: 4218, ..male };
        room.apply_item(&doram, &equip(6, EquipPosition::RIGHT_HAND, 2), suffix, hit);
        let doram_sword = room.slot_views().into_iter().find(|slot| slot.slot == "weapon").unwrap();
        assert_eq!(doram_sword.sprite_path, "인간족\\summoner\\summoner_남_검");
        assert!(!doram_sword.sprite_path.contains("도람족"));

        let female = WornLook {
            female: true,
            hair_style: 2,
            ..WornLook::default()
        };
        room.apply_item(&female, &equip(7, EquipPosition::GARMENT, 1), suffix, hit);
        let robe = room.slot_views().into_iter().find(|slot| slot.slot == "garment").unwrap();
        assert_eq!(robe.sprite_path, "로브\\천사날개\\여\\초보자_여");

        room.apply_item_with_robes(
            &female,
            &equip(8, EquipPosition::COSTUME_GARMENT, 4),
            suffix,
            |_| Some("custom_cape".to_owned()),
            |path| path.contains("custom_cape"),
        );
        let costume = room.slot_views().into_iter().find(|slot| slot.slot == "costume_garment").unwrap();
        assert_eq!(costume.sprite_path, "로브\\custom_cape\\여\\초보자_여");

        room.apply_item(&female, &equip(9, EquipPosition::GARMENT, 9000), suffix, |_| false);
        let missing = room.slot_views().into_iter().find(|slot| slot.slot == "garment").unwrap();
        assert_eq!(missing.preview_id, 9000);
        assert!(missing.sprite_path.is_empty());
        assert_eq!(room.notice(), NO_PREVIEW);
    }

    #[test]
    fn a_multi_bit_hat_fills_every_matching_head_slot() {
        let worn = WornLook {
            female: true,
            hair_style: 2,
            head_bottom: 4,
            ..WornLook::default()
        };
        let mut room = DressingRoom::default();
        room.apply_item(
            &worn,
            &equip(1, EquipPosition::HEAD_TOP | EquipPosition::HEAD_MIDDLE, 8),
            suffix,
            exists,
        );
        let views = room.slot_views();
        let top = views.iter().find(|slot| slot.slot == "head_top").unwrap();
        let mid = views.iter().find(|slot| slot.slot == "head_mid").unwrap();
        let bottom = views.iter().find(|slot| slot.slot == "head_bottom").unwrap();
        assert_eq!(top.preview_id, 8);
        assert_eq!(mid.preview_id, 8);
        assert_eq!(bottom.preview_id, 0);
        assert_eq!(room.part_files().iter().filter(|file| file.contains("_고글")).count(), 1);

        room.apply_item(
            &worn,
            &equip(2, EquipPosition::COSTUME_HEAD_TOP | EquipPosition::COSTUME_HEAD_MIDDLE, 5),
            suffix,
            exists,
        );
        let views = room.slot_views();
        assert_eq!(views.iter().find(|slot| slot.slot == "costume_head_top").unwrap().preview_id, 5);
        assert_eq!(views.iter().find(|slot| slot.slot == "costume_head_mid").unwrap().preview_id, 5);
        assert_eq!(views.iter().find(|slot| slot.slot == "head_top").unwrap().preview_id, 8);

        room.apply_item(&worn, &equip(3, EquipPosition::LEFT_RIGHT_HAND, 2), suffix, |path| {
            path.ends_with("초보자_여_검")
        });
        let views = room.slot_views();
        assert_eq!(views.iter().find(|slot| slot.slot == "weapon").unwrap().preview_id, 2);
        assert_eq!(views.iter().find(|slot| slot.slot == "shield").unwrap().preview_id, 0);
    }

    #[test]
    fn hair_color_requests_a_head_palette_and_zero_stays_plain() {
        let colored = WornLook {
            female: true,
            hair_style: 2,
            hair_color: 4,
            ..WornLook::default()
        };
        let mut room = DressingRoom::default();
        room.apply_view(&colored, ItemView::only(Some(PreviewSlot::HeadTop), 8), suffix, exists);
        let request = room.head_dye_request().expect("palette");
        assert!(request.sprite.ends_with("2_여.spr"), "{}", request.sprite);
        assert_eq!(request.palette, "data\\palette\\머리\\머리4_여_4.pal");

        let doram = WornLook {
            job_id: 4218,
            female: true,
            hair_style: 3,
            hair_color: 1,
            ..WornLook::default()
        };
        room.apply_view(&doram, ItemView::only(Some(PreviewSlot::HeadTop), 8), suffix, exists);
        let request = room.head_dye_request().expect("doram palette");
        assert!(request.sprite.ends_with("3_여.spr"), "{}", request.sprite);
        assert_eq!(request.palette, "data\\palette\\도람족\\머리\\머리3_여_1.pal");

        let plain = WornLook { hair_color: 0, ..colored };
        room.apply_view(&plain, ItemView::only(Some(PreviewSlot::HeadTop), 8), suffix, exists);
        assert!(room.head_dye_request().is_none());
    }
}
