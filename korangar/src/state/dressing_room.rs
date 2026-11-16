//! Client-only dressing room. Ctrl+click layers one view sprite on the
//! player's own body. Nothing here is written to the server.
//!
//! Headgear paths use [`headgear_sprite_candidates`] and
//! [`player_look_part_files`], the same accessory-id lookup the world
//! renderer uses. Garment, shield, and weapon parts use the roBrowser file
//! layout (`로브`, `방패`, `인간족` / `도람족`). A view id whose sprite files
//! are missing stays in the slot list and the window shows
//! [`NO_PREVIEW`]; the part is left out of the sprite so it never draws a
//! black box.

use std::sync::Arc;

use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::{EquipPosition, JobId, Sex};

use super::vn_dialog::headgear_sprite_candidates;
use crate::world::{AnimationData, Player, PlayerLook, player_look_part_files};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemView {
    pub slot: Option<PreviewSlot>,
    pub view_id: u16,
}

/// Headgear, costume headgear, garment, shield, or weapon. Other locations
/// (armor, shoes, a potion) have no preview slot.
pub fn preview_slot(position: EquipPosition) -> Option<PreviewSlot> {
    const ORDER: &[(EquipPosition, PreviewSlot)] = &[
        (EquipPosition::COSTUME_HEAD_LOWER, PreviewSlot::CostumeHeadBottom),
        (EquipPosition::COSTUME_HEAD_MIDDLE, PreviewSlot::CostumeHeadMid),
        (EquipPosition::COSTUME_HEAD_TOP, PreviewSlot::CostumeHeadTop),
        (EquipPosition::COSTUME_GARMENT, PreviewSlot::CostumeGarment),
        (EquipPosition::HEAD_LOWER, PreviewSlot::HeadBottom),
        (EquipPosition::HEAD_MIDDLE, PreviewSlot::HeadMid),
        (EquipPosition::HEAD_TOP, PreviewSlot::HeadTop),
        (EquipPosition::GARMENT, PreviewSlot::Garment),
        (EquipPosition::RIGHT_HAND, PreviewSlot::Weapon),
        (EquipPosition::LEFT_RIGHT_HAND, PreviewSlot::Weapon),
        (EquipPosition::LEFT_HAND, PreviewSlot::Shield),
    ];
    ORDER.iter().find(|(flag, _)| position.contains(*flag)).map(|(_, slot)| *slot)
}

pub fn item_view<T>(item: &InventoryItem<T>) -> ItemView {
    match &item.details {
        InventoryItemDetails::Equippable {
            equip_position,
            w_item_sprite_number,
            ..
        } => ItemView {
            slot: preview_slot(*equip_position),
            view_id: *w_item_sprite_number,
        },
        InventoryItemDetails::Regular { .. } => ItemView { slot: None, view_id: 0 },
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
            if equipped_position.is_empty() || preview_slot(*equipped_position) != Some(slot) {
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

    /// Layer `item` on `worn`. A later click replaces only its own slot.
    pub fn apply_item<T, S, E>(&mut self, worn: &WornLook, item: &InventoryItem<T>, suffix: S, exists: E)
    where
        S: Fn(u16) -> Option<String>,
        E: Fn(&str) -> bool,
    {
        self.apply_view(worn, item_view(item), suffix, exists);
    }

    pub fn apply_view<S, E>(&mut self, worn: &WornLook, view: ItemView, suffix: S, exists: E)
    where
        S: Fn(u16) -> Option<String>,
        E: Fn(&str) -> bool,
    {
        self.ensure();
        self.copy_worn(worn);
        for view_id in [worn.head_bottom, worn.head_top, worn.head_mid, view.view_id] {
            self.remember_suffix(view_id, &suffix);
        }
        self.resolve_worn_paths(&exists);
        match view.slot {
            Some(slot) if view.view_id > 0 => {
                let index = slot.index();
                self.preview_ids[index] = view.view_id;
                let path = self.resolve_path(slot, view.view_id, &exists);
                if path.is_empty() {
                    self.notice = NO_PREVIEW.to_owned();
                } else {
                    self.notice.clear();
                }
                self.preview_paths[index] = path;
            }
            _ => self.notice = NO_PREVIEW.to_owned(),
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

    fn resolve_worn_paths<E>(&mut self, exists: &E)
    where
        E: Fn(&str) -> bool,
    {
        let heads = [self.worn_head_bottom, self.worn_head_mid, self.worn_head_top];
        for (index, view_id) in heads.into_iter().enumerate() {
            let slot = PreviewSlot::from_index(index).unwrap();
            self.worn_head_paths[index] = self.resolve_path(slot, view_id, exists);
        }
        let extras = [
            (PreviewSlot::Garment, self.worn_garment),
            (PreviewSlot::Weapon, self.worn_weapon),
            (PreviewSlot::Shield, self.worn_shield),
            (PreviewSlot::CostumeGarment, self.worn_costume_garment),
        ];
        for (index, (slot, view_id)) in extras.into_iter().enumerate() {
            self.worn_extra_paths[index] = self.resolve_path(slot, view_id, exists);
        }
    }

    fn resolve_path<E>(&self, slot: PreviewSlot, view_id: u16, exists: &E) -> String
    where
        E: Fn(&str) -> bool,
    {
        if view_id == 0 {
            return String::new();
        }
        let sex_name = sex_folder(self.female);
        let (job, tribe) = self.job_and_tribe();
        let candidates = if slot.is_head() {
            headgear_sprite_candidates(view_id, sex_name, self.suffix_owned(view_id).as_deref())
        } else if matches!(slot, PreviewSlot::Garment | PreviewSlot::CostumeGarment) {
            garment_candidates(view_id, sex_name, &job)
        } else if slot == PreviewSlot::Weapon {
            weapon_candidates(view_id, sex_name, &job, &tribe)
        } else {
            shield_candidates(view_id, sex_name, &job)
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

fn garment_candidates(view_id: u16, sex_name: &str, job: &str) -> Vec<String> {
    vec![
        format!("로브\\{view_id}\\{sex_name}\\{job}_{sex_name}"),
        format!("로브\\_{view_id}\\{sex_name}\\{job}_{sex_name}"),
    ]
}

fn weapon_candidates(view_id: u16, sex_name: &str, job: &str, tribe: &str) -> Vec<String> {
    vec![
        format!("{tribe}\\{job}\\{job}_{sex_name}_{view_id}"),
        format!("{tribe}\\{job}\\{job}_{sex_name}{view_id}"),
    ]
}

fn shield_candidates(view_id: u16, sex_name: &str, job: &str) -> Vec<String> {
    vec![format!("방패\\{job}\\{job}_{sex_name}_{view_id}")]
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
        path.ends_with("여_리본") || path.ends_with("여_고글") || path.ends_with("초보자_남_3")
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
        room.apply_view(
            &worn,
            ItemView {
                slot: Some(PreviewSlot::HeadTop),
                view_id: 9,
            },
            suffix,
            |_| false,
        );
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

        room.apply_view(
            &worn,
            ItemView {
                slot: Some(PreviewSlot::Weapon),
                view_id: 3,
            },
            suffix,
            exists,
        );
        let weapon = room.slot_views().into_iter().find(|slot| slot.slot == "weapon").unwrap();
        assert_eq!(weapon.preview_id, 3);
        assert!(weapon.sprite_path.contains("초보자_남_3"));
        assert!(
            room.part_files()
                .iter()
                .any(|file| file.starts_with('?') && file.contains("초보자_남_3"))
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
        room.apply_view(
            &worn,
            ItemView {
                slot: Some(PreviewSlot::HeadTop),
                view_id: 8,
            },
            suffix,
            exists,
        );
        room.toggle_hide_other_headgear();
        let views = room.slot_views();
        let top = views.iter().find(|slot| slot.slot == "head_top").unwrap();
        let bottom = views.iter().find(|slot| slot.slot == "head_bottom").unwrap();
        assert_eq!(top.shown_id, 8);
        assert_eq!(bottom.shown_id, 0);
        assert!(room.hide_other_headgear());
    }
}
