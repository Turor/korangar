//! Read-only equipment received from `ZC_EQUIPWIN_MICROSCOPE`.

use std::sync::Arc;

use korangar_interface::element::StateElement;
use ragnarok_packets::EquipPosition;
use rust_state::RustState;

use crate::graphics::Texture;

/// One worn item placed on the view-equip paperdoll.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct ViewedEquipItem {
    pub item_id: u32,
    pub location: u32,
    pub refine: u8,
    pub label: String,
    #[hidden_element]
    pub texture: Option<Arc<Texture>>,
}

#[derive(Clone, Default, RustState, StateElement)]
pub struct ViewEquipState {
    pub character_name: String,
    pub summary: String,
    pub items: Vec<ViewedEquipItem>,
}

impl ViewEquipState {
    pub fn replace(&mut self, name: String, job: i16, sex: u8, items: Vec<ViewedEquipItem>) {
        self.character_name = if name.is_empty() { "Equipment".to_owned() } else { name };
        self.summary = format!("{} · {}", super::party::job_label(job), sex_label(sex));
        self.items = items;
    }

    pub fn set_item_texture(&mut self, item_id: u32, texture: Arc<Texture>) {
        if item_id == 0 {
            return;
        }
        for item in &mut self.items {
            if item.item_id == item_id {
                item.texture = Some(texture.clone());
            }
        }
    }
}

fn sex_label(sex: u8) -> &'static str {
    match sex {
        0 => "Female",
        1 => "Male",
        _ => "Unknown",
    }
}

/// True when `location` should be drawn in `slot`.
///
/// A two-hand weapon and a paired accessory occupy the right/primary socket
/// only, so the same item is not painted in both hands.
pub fn occupies_slot(location: u32, slot: EquipPosition) -> bool {
    if slot.is_empty() {
        return false;
    }
    let loc = EquipPosition::from_bits_truncate(location);
    if loc.contains(EquipPosition::LEFT_RIGHT_HAND) {
        if slot == EquipPosition::RIGHT_HAND {
            return true;
        }
        if slot == EquipPosition::LEFT_HAND {
            return false;
        }
    }
    if loc.contains(EquipPosition::LEFT_RIGHT_ACCESSORY) {
        if slot == EquipPosition::RIGTH_ACCESSORY {
            return true;
        }
        if slot == EquipPosition::LEFT_ACCESSORY {
            return false;
        }
    }
    if loc.contains(EquipPosition::SHADOW_LEFT_RIGHT_ACCESSORY) {
        if slot == EquipPosition::SHADOW_RIGHT_ACCESSORY {
            return true;
        }
        if slot == EquipPosition::SHADOW_LEFT_ACCESSORY {
            return false;
        }
    }
    loc.contains(slot)
}

#[cfg(test)]
mod tests {

    #[test]
    fn worn_ring_and_dual_wield_masks_fill_their_own_socket() {
        use ragnarok_packets::EquipPosition;

        use super::occupies_slot;
        let left_ring = EquipPosition::LEFT_ACCESSORY.bits();
        let right_ring = EquipPosition::RIGTH_ACCESSORY.bits();
        assert!(occupies_slot(left_ring, EquipPosition::LEFT_ACCESSORY));
        assert!(!occupies_slot(left_ring, EquipPosition::RIGTH_ACCESSORY));
        assert!(occupies_slot(right_ring, EquipPosition::RIGTH_ACCESSORY));
        assert!(!occupies_slot(right_ring, EquipPosition::LEFT_ACCESSORY));
        let left_weapon = EquipPosition::LEFT_HAND.bits();
        assert!(occupies_slot(left_weapon, EquipPosition::LEFT_HAND));
        assert!(!occupies_slot(left_weapon, EquipPosition::RIGHT_HAND));
    }

    use super::*;

    #[test]
    fn two_hand_weapon_occupies_the_right_hand_only() {
        let location = EquipPosition::LEFT_RIGHT_HAND.bits();
        assert!(occupies_slot(location, EquipPosition::RIGHT_HAND));
        assert!(!occupies_slot(location, EquipPosition::LEFT_HAND));
    }

    #[test]
    fn armor_and_costume_stay_on_their_own_sockets() {
        assert!(occupies_slot(EquipPosition::ARMOR.bits(), EquipPosition::ARMOR));
        assert!(!occupies_slot(EquipPosition::ARMOR.bits(), EquipPosition::GARMENT));
        assert!(occupies_slot(
            EquipPosition::COSTUME_HEAD_TOP.bits(),
            EquipPosition::COSTUME_HEAD_TOP
        ));
        assert!(!occupies_slot(EquipPosition::COSTUME_HEAD_TOP.bits(), EquipPosition::HEAD_TOP));
    }

    #[test]
    fn paired_accessory_occupies_the_right_socket_only() {
        let location = EquipPosition::LEFT_RIGHT_ACCESSORY.bits();
        assert!(occupies_slot(location, EquipPosition::RIGTH_ACCESSORY));
        assert!(!occupies_slot(location, EquipPosition::LEFT_ACCESSORY));
        let shadow = EquipPosition::SHADOW_LEFT_RIGHT_ACCESSORY.bits();
        assert!(occupies_slot(shadow, EquipPosition::SHADOW_RIGHT_ACCESSORY));
        assert!(!occupies_slot(shadow, EquipPosition::SHADOW_LEFT_ACCESSORY));
    }
}
