//! List windows for identify, card insert, warp memo, auto spell, crafting,
//! Whitesmith refine, and change cart.
//!
//! Confirm and cancel return a [`SelectionCommand`]. The caller sends the
//! packet. Tests do not open a socket.

use std::sync::Arc;

use korangar_interface::element::StateElement;
use rust_state::RustState;

use crate::graphics::Texture;

/// `MC_CHANGECART` in Hercules `e_skill` (`NV_BASIC` is 1).
pub const CHANGE_CART_SKILL: u16 = 154;

const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub enum SelectionKind {
    #[default]
    Identify,
    CardInsert,
    Warp,
    AutoSpell,
    ShadowSpell,
    Arrow,
    Produce,
    Making,
    WeaponRefine,
    ChangeCart,
}

#[derive(Clone, Debug, RustState, StateElement)]
pub struct SelectionRow {
    pub label: String,
    /// Real inventory index. `u16::MAX` when the row is not an inventory item.
    pub inventory_index: u16,
    /// Skill id or cart type.
    pub value: i32,
    /// Wire map name for warp rows.
    pub map_name: String,
    pub item_id: u32,
    pub material0: u32,
    pub material1: u32,
    pub material2: u32,
    #[hidden_element]
    pub texture: Option<Arc<Texture>>,
}

impl SelectionRow {
    pub fn named(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            inventory_index: u16::MAX,
            value: 0,
            map_name: String::new(),
            item_id: 0,
            material0: 0,
            material1: 0,
            material2: 0,
            texture: None,
        }
    }

    pub fn inventory(label: impl Into<String>, inventory_index: u16, item_id: u32, texture: Option<Arc<Texture>>) -> Self {
        Self {
            inventory_index,
            item_id,
            texture,
            ..Self::named(label)
        }
    }

    pub fn catalog(label: impl Into<String>, item_id: u32, texture: Option<Arc<Texture>>) -> Self {
        Self {
            item_id,
            texture,
            ..Self::named(label)
        }
    }
}

/// What OK or Cancel should put on the wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionCommand {
    /// `CZ_REQ_ITEMIDENTIFY`. `0xFFFF` cancels.
    Identify { wire_index: u16 },
    /// `CZ_REQ_ITEMCOMPOSITION`. Indexes are the real inventory indexes.
    InsertCard { card_index: u16, equip_index: u16 },
    /// `CZ_SELECT_WARPPOINT`. `map_name` is the 16-byte wire name.
    Warp { skill_id: u16, map_name: String },
    /// `CZ_SELECTAUTOSPELL`.
    AutoSpell { skill_id: i32 },
    /// `CZ_SKILL_SELECT_RESPONSE`. Skill id 0 cancels.
    Shadow { flag: i32, skill_id: i16 },
    /// `CZ_REQ_MAKINGARROW`. Item id 0 cancels.
    Arrow { item_id: u32 },
    /// `CZ_REQMAKINGITEM`. Item id 0 cancels and still clears the menu.
    Produce { item_id: u32, material: [u32; 3] },
    /// `CZ_REQ_MAKINGITEM`.
    Making { make_type: u16, item_id: u32 },
    /// `CZ_REQ_WEAPONREFINE`. Zero cancels. A choice is the inventory index plus 2.
    WeaponRefine { wire_index: i32 },
    /// `CZ_REQ_CHANGECART`.
    ChangeCart { cart_type: u16 },
    /// Card insert and change cart send nothing on cancel.
    Silent,
}

#[derive(Clone, Debug, RustState, StateElement)]
pub struct ItemSkillSelectionState {
    pub kind: SelectionKind,
    pub title: String,
    pub status: String,
    pub skill_id: u16,
    pub card_index: u16,
    pub make_type: u16,
    pub shadow_flag: i32,
    pub selected: u32,
    pub rows: Vec<SelectionRow>,
}

impl Default for ItemSkillSelectionState {
    fn default() -> Self {
        Self {
            kind: SelectionKind::Identify,
            title: String::new(),
            status: String::new(),
            skill_id: 0,
            card_index: 0,
            make_type: 0,
            shadow_flag: 0,
            selected: NONE,
            rows: Vec::new(),
        }
    }
}

impl ItemSkillSelectionState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn remember_card(&mut self, index: u16) {
        self.card_index = index;
        self.kind = SelectionKind::CardInsert;
    }

    pub fn show(&mut self, kind: SelectionKind, title: impl Into<String>, rows: Vec<SelectionRow>) {
        let skill_id = self.skill_id;
        let card_index = self.card_index;
        let make_type = self.make_type;
        let shadow_flag = self.shadow_flag;
        let selected = if rows.is_empty() { NONE } else { 0 };
        *self = Self {
            kind,
            title: title.into(),
            status: String::new(),
            skill_id,
            card_index,
            make_type,
            shadow_flag,
            selected,
            rows,
        };
    }

    pub fn select(&mut self, index: u32) {
        if (index as usize) < self.rows.len() {
            self.selected = index;
        }
    }

    pub fn confirm(&self) -> Option<SelectionCommand> {
        let row = self.rows.get(self.selected as usize)?;
        Some(match self.kind {
            SelectionKind::Identify => SelectionCommand::Identify {
                wire_index: row.inventory_index.saturating_add(2),
            },
            SelectionKind::CardInsert => SelectionCommand::InsertCard {
                card_index: self.card_index,
                equip_index: row.inventory_index,
            },
            SelectionKind::Warp => SelectionCommand::Warp {
                skill_id: self.skill_id,
                map_name: row.map_name.clone(),
            },
            SelectionKind::AutoSpell => SelectionCommand::AutoSpell { skill_id: row.value },
            SelectionKind::ShadowSpell => SelectionCommand::Shadow {
                flag: self.shadow_flag,
                skill_id: i16::try_from(row.value).unwrap_or(0),
            },
            SelectionKind::Arrow => SelectionCommand::Arrow { item_id: row.item_id },
            SelectionKind::Produce => SelectionCommand::Produce {
                item_id: row.item_id,
                material: [row.material0, row.material1, row.material2],
            },
            SelectionKind::Making => SelectionCommand::Making {
                make_type: self.make_type,
                item_id: row.item_id,
            },
            SelectionKind::WeaponRefine => SelectionCommand::WeaponRefine {
                wire_index: i32::from(row.inventory_index.saturating_add(2)),
            },
            SelectionKind::ChangeCart => SelectionCommand::ChangeCart {
                cart_type: u16::try_from(row.value).unwrap_or(1),
            },
        })
    }

    pub fn cancel(&self) -> SelectionCommand {
        match self.kind {
            SelectionKind::Identify => SelectionCommand::Identify { wire_index: 0xFFFF },
            SelectionKind::CardInsert | SelectionKind::ChangeCart => SelectionCommand::Silent,
            SelectionKind::Warp => SelectionCommand::Warp {
                skill_id: self.skill_id,
                map_name: "cancel".to_owned(),
            },
            SelectionKind::AutoSpell => SelectionCommand::AutoSpell { skill_id: 0 },
            SelectionKind::ShadowSpell => SelectionCommand::Shadow {
                flag: self.shadow_flag,
                skill_id: 0,
            },
            SelectionKind::Arrow => SelectionCommand::Arrow { item_id: 0 },
            SelectionKind::Produce => SelectionCommand::Produce {
                item_id: 0,
                material: [0, 0, 0],
            },
            SelectionKind::Making => SelectionCommand::Making {
                make_type: self.make_type,
                item_id: 0,
            },
            SelectionKind::WeaponRefine => SelectionCommand::WeaponRefine { wire_index: 0 },
        }
    }

    /// Whitesmith refine stays open so the player can try another weapon.
    /// Hercules keeps the menu skill until the index is zero.
    pub fn closes_after_confirm(&self) -> bool {
        self.kind != SelectionKind::WeaponRefine
    }

    pub fn note_item_texture(&mut self, item_id: u32, texture: Arc<Texture>) {
        for row in &mut self.rows {
            if row.item_id == item_id {
                row.texture = Some(texture.clone());
            }
        }
    }

    pub fn note_weapon_refine_success(&mut self, item_id: u32) -> Option<u16> {
        let row = self.rows.iter_mut().find(|row| row.item_id == item_id)?;
        row.label = bump_refine_label(&row.label);
        Some(row.inventory_index)
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }
}

/// Carts the official client offers. Type 1 is always listed. Later carts
/// require a base level strictly above the gate.
pub fn change_cart_types(base_level: i32) -> Vec<u16> {
    let mut types = vec![1];
    for (gate, cart) in [(40, 2), (65, 3), (80, 4), (90, 5), (100, 6), (111, 7), (121, 8), (131, 9)] {
        if base_level > gate {
            types.push(cart);
        }
    }
    types
}

pub fn warp_title(skill_id: u16) -> &'static str {
    match skill_id {
        26 => "Teleport",
        27 => "Warp Portal",
        _ => "Warp",
    }
}

pub fn making_title(make_type: u16) -> &'static str {
    match make_type {
        1 => "Cooking",
        2 => "Arrow Crafting",
        3 => "Elemental",
        4 => "Mixed Cooking",
        5 => "Bomb Making",
        6 => "Pharmacy",
        _ => "Create item",
    }
}

pub fn identify_message(success: bool, name: &str) -> String {
    if success {
        format!("Identified {name}.")
    } else {
        "Could not identify the item.".to_owned()
    }
}

pub fn card_message(success: bool) -> &'static str {
    if success {
        "The card was inserted."
    } else {
        "The card could not be inserted."
    }
}

pub fn produce_message(result: u16, name: &str) -> String {
    if result.is_multiple_of(2) {
        format!("{name} was created.")
    } else {
        format!("Failed to create {name}.")
    }
}

pub fn weapon_refine_message(result: i32, name: &str) -> String {
    match result {
        0 | 1 => format!("Weapon upgraded: {name}"),
        2 => format!("Cannot upgrade {name} until the refine skill level is higher."),
        3 => format!("Missing the item required to upgrade {name}."),
        other => format!("Weapon refine result {other} for {name}."),
    }
}

pub fn bump_refine_label(label: &str) -> String {
    let Some(rest) = label.strip_prefix('+') else {
        return label.to_owned();
    };
    let (number, name) = rest.split_once(' ').unwrap_or((rest, ""));
    let next = number.parse::<u32>().unwrap_or(0).saturating_add(1);
    if name.is_empty() {
        format!("+{next}")
    } else {
        format!("+{next} {name}")
    }
}

pub fn catalog_label(name: &str, item_id: u32) -> String {
    if name.is_empty() || name == "NOTFOUND" {
        format!("Item {item_id}")
    } else {
        name.to_owned()
    }
}

pub fn skill_label(name: &str, skill_id: i32) -> String {
    if name.is_empty() || name == "notfound" {
        format!("Skill {skill_id}")
    } else {
        name.to_owned()
    }
}

/// Drop blank warp slots. `Random` is a real destination. `label_for` turns
/// the wire map name into the row the player sees.
pub fn warp_destinations(maps: impl IntoIterator<Item = String>, mut label_for: impl FnMut(&str) -> String) -> Vec<SelectionRow> {
    maps.into_iter()
        .filter(|map| !map.is_empty())
        .map(|map| {
            let mut row = SelectionRow::named(label_for(&map));
            row.map_name = map;
            row
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ItemSkillSelectionState, SelectionCommand, SelectionKind, SelectionRow, bump_refine_label, change_cart_types, warp_destinations,
    };

    fn identify_state() -> ItemSkillSelectionState {
        let mut state = ItemSkillSelectionState::default();
        state.show(
            SelectionKind::Identify,
            "Identify",
            vec![SelectionRow::inventory("Unidentified Sword", 3, 1101, None)],
        );
        state
    }

    #[test]
    fn identify_confirm_uses_wire_index_and_cancel_is_ffff() {
        let state = identify_state();
        assert_eq!(state.confirm(), Some(SelectionCommand::Identify { wire_index: 5 }));
        assert_eq!(state.cancel(), SelectionCommand::Identify { wire_index: 0xFFFF });
    }

    #[test]
    fn card_cancel_is_silent_and_confirm_keeps_the_card_index() {
        let mut state = ItemSkillSelectionState::default();
        state.remember_card(4);
        state.show(
            SelectionKind::CardInsert,
            "Insert Card",
            vec![SelectionRow::inventory("Sword", 1, 1101, None)],
        );
        assert_eq!(state.confirm(), Some(SelectionCommand::InsertCard {
            card_index: 4,
            equip_index: 1,
        }));
        assert_eq!(state.cancel(), SelectionCommand::Silent);
    }

    #[test]
    fn warp_keeps_random_drops_blanks_and_cancel_sends_cancel() {
        let rows = warp_destinations(["prontera".to_owned(), String::new(), "Random".to_owned()], |map| map.to_owned());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].map_name, "Random");

        let mut state = ItemSkillSelectionState {
            skill_id: 27,
            ..Default::default()
        };
        state.show(SelectionKind::Warp, "Warp Portal", rows);
        state.select(1);
        assert_eq!(state.confirm(), Some(SelectionCommand::Warp {
            skill_id: 27,
            map_name: "Random".to_owned(),
        }));
        assert_eq!(state.cancel(), SelectionCommand::Warp {
            skill_id: 27,
            map_name: "cancel".to_owned(),
        });
    }

    #[test]
    fn crafting_cancel_sends_zero_and_produce_echoes_materials() {
        let mut state = ItemSkillSelectionState::default();
        let mut row = SelectionRow::catalog("Red Potion", 501, None);
        row.material0 = 713;
        state.show(SelectionKind::Produce, "Create item", vec![row]);
        assert_eq!(state.confirm(), Some(SelectionCommand::Produce {
            item_id: 501,
            material: [713, 0, 0],
        }));
        assert_eq!(state.cancel(), SelectionCommand::Produce {
            item_id: 0,
            material: [0, 0, 0],
        });

        state.make_type = 6;
        state.show(SelectionKind::Making, "Pharmacy", vec![SelectionRow::catalog("Potion", 501, None)]);
        assert_eq!(state.cancel(), SelectionCommand::Making {
            make_type: 6,
            item_id: 0,
        });

        state.show(SelectionKind::Arrow, "Arrow Crafting", vec![SelectionRow::catalog("Arrow", 1750, None)]);
        assert_eq!(state.cancel(), SelectionCommand::Arrow { item_id: 0 });
    }

    #[test]
    fn weapon_refine_confirm_stays_open_and_cancel_is_zero() {
        let mut state = ItemSkillSelectionState::default();
        let mut row = SelectionRow::inventory("+7 Sword", 2, 1201, None);
        row.label = "+7 Sword".to_owned();
        state.show(SelectionKind::WeaponRefine, "Refine Weapon", vec![row]);
        assert!(!state.closes_after_confirm());
        assert_eq!(state.confirm(), Some(SelectionCommand::WeaponRefine { wire_index: 4 }));
        assert_eq!(state.cancel(), SelectionCommand::WeaponRefine { wire_index: 0 });
        assert_eq!(state.note_weapon_refine_success(1201), Some(2));
        assert_eq!(state.rows[0].label, "+8 Sword");
        assert_eq!(bump_refine_label("Knife"), "Knife");
    }

    #[test]
    fn shadow_cancel_is_zero_and_autospell_cancel_is_zero() {
        let mut state = ItemSkillSelectionState {
            shadow_flag: 1,
            ..Default::default()
        };
        let mut row = SelectionRow::named("Fire Bolt");
        row.value = 19;
        state.show(SelectionKind::ShadowSpell, "Auto Shadow Spell", vec![row]);
        assert_eq!(state.cancel(), SelectionCommand::Shadow { flag: 1, skill_id: 0 });
        assert_eq!(state.confirm(), Some(SelectionCommand::Shadow { flag: 1, skill_id: 19 }));

        let mut spell = SelectionRow::named("Napalm Beat");
        spell.value = 11;
        state.show(SelectionKind::AutoSpell, "Auto Spell", vec![spell]);
        assert_eq!(state.cancel(), SelectionCommand::AutoSpell { skill_id: 0 });
    }

    #[test]
    fn change_cart_types_follow_base_level_gates() {
        assert_eq!(change_cart_types(1), vec![1]);
        assert_eq!(change_cart_types(40), vec![1]);
        assert_eq!(change_cart_types(41), vec![1, 2]);
        assert_eq!(change_cart_types(131), vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(change_cart_types(132), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);

        let mut state = ItemSkillSelectionState::default();
        let mut row = SelectionRow::named("Cart 2");
        row.value = 2;
        state.show(SelectionKind::ChangeCart, "Change Cart", vec![row]);
        assert_eq!(state.confirm(), Some(SelectionCommand::ChangeCart { cart_type: 2 }));
        assert_eq!(state.cancel(), SelectionCommand::Silent);
    }

    #[test]
    fn empty_list_and_out_of_range_select_do_not_confirm() {
        let mut state = ItemSkillSelectionState::default();
        state.show(SelectionKind::Identify, "Identify", Vec::new());
        assert!(state.confirm().is_none());
        state.select(3);
        assert_eq!(state.selected, u32::MAX);
    }
}
