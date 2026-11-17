//! Hotbar pointer actions (Matthew t278u): what a drag released off the bar,
//! Escape during a drag, and a right-click on a slot do. Kept pure so the
//! unit and e2e tests can check them without a window or a server.
//!
//! - A drag from a hotbar slot that ends anywhere outside the hotbar, and is
//!   not taken by a window that accepts it (another slot, the inventory, the
//!   skill tree, ...), clears the slot. The clear sends
//!   `CZ_SHORTCUT_KEY_CHANGE2` (`0x0B21`) with an unbound hotkey, like every
//!   other hotbar change.
//! - Dropping on a slot keeps the move / swap. Dropping on the hotbar window
//!   between slots changes nothing. Escape cancels the drag.
//! - Right-click opens the skill description (the #144 skill-tree path) or the
//!   pinned item details the inventory opens. It no longer clears.

use korangar_interface::MouseMode;
use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::{EquipPosition, HotbarSlot, HotbarTab, InventoryIndex, ItemId, RegularItemFlags, SkillLevel};

use crate::input::{InputEvent, MouseInputMode};
use crate::interface::components::item_tooltip::{ItemTooltipText, pinned_details_body};
use crate::interface::resource::{ItemSource, SkillSource};
use crate::interface::windows::item_details_key;
use crate::state::ClientState;
use crate::state::hotbar::{CELL_ITEM, CELL_SKILL, HotbarCell, cell_item_id, cell_skill_level};
use crate::state::skills::LearnableSkill;
use crate::world::{ResourceMetadata, compose_skill_description};

/// Where a drag was released, as the frame saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DragRelease {
    /// The pointer was over a window.
    pub over_interface: bool,
    /// That window is the hotbar.
    pub over_hotbar: bool,
    /// A drop handler under the pointer took the drag (queued an event).
    pub accepted: bool,
}

/// Hotbar slot the drag came from, if the mouse mode is a hotbar drag.
pub fn hotbar_drag_source(mouse_mode: &MouseMode<ClientState>) -> Option<(HotbarTab, HotbarSlot)> {
    match mouse_mode {
        MouseMode::Custom {
            mode: MouseInputMode::MoveItem {
                source: ItemSource::Hotbar { tab, slot },
                ..
            },
        }
        | MouseMode::Custom {
            mode: MouseInputMode::MoveSkill {
                source: SkillSource::Hotbar { tab, slot },
                ..
            },
        } => Some((*tab, *slot)),
        _ => None,
    }
}

/// Slot to clear when a hotbar drag is released: off every window (the
/// world), or over a window that is not the hotbar and did not take it.
pub fn slot_to_clear_on_release(dragged: Option<(HotbarTab, HotbarSlot)>, release: DragRelease) -> Option<(HotbarTab, HotbarSlot)> {
    let dragged = dragged?;
    let removes = !release.over_interface || (!release.over_hotbar && !release.accepted);
    removes.then_some(dragged)
}

/// Escape while an item or skill is held drops the drag and does nothing
/// else (it does not also close a window).
pub fn escape_cancels_drag(mouse_mode: &MouseMode<ClientState>) -> bool {
    matches!(mouse_mode, MouseMode::Custom {
        mode: MouseInputMode::MoveItem { .. } | MouseInputMode::MoveSkill { .. },
    })
}

/// Skill description for a hotbar skill, the event the skill tree's
/// right-click queues.
pub fn skill_info_event(skill: &LearnableSkill, level: SkillLevel) -> InputEvent {
    InputEvent::ToggleSkillDescription {
        skill_id: skill.skill_id,
        title: skill.skill_name.clone(),
        body: compose_skill_description(&skill.description, level.0, skill.maximum_level.0, &skill.spell_point_costs),
    }
}

/// Pinned item details, keyed like the inventory's so a second right-click
/// (from either place) raises the same window.
pub fn item_info_event(item: &InventoryItem<ResourceMetadata>, text: &ItemTooltipText) -> InputEvent {
    InputEvent::OpenItemDetails {
        key: item_details_key(ItemSource::Inventory, item),
        title: item.metadata.name.clone(),
        body: pinned_details_body(text),
    }
}

/// The inventory stack a hotbar item cell points at, or a stand-in built
/// from the cell when none is held (a used-up stack still shows as `0`). The
/// stand-in lets such a slot be dragged off and its details opened.
pub fn hotbar_item(cell: &HotbarCell, held: Option<&InventoryItem<ResourceMetadata>>) -> Option<InventoryItem<ResourceMetadata>> {
    let item_id = cell_item_id(cell)?;
    if let Some(item) = held {
        return Some(item.clone());
    }
    Some(InventoryItem {
        metadata: ResourceMetadata {
            texture: cell.item_texture.clone(),
            name: cell.item_name.clone(),
            description: String::new(),
        },
        index: InventoryIndex(u16::MAX),
        item_id,
        item_type: 3,
        slot: [0; 4],
        hire_expiration_date: 0,
        details: InventoryItemDetails::Regular {
            amount: 0,
            equipped_position: EquipPosition::empty(),
            flags: RegularItemFlags::IDENTIFIED,
        },
    })
}

/// What a right-click on this slot opens. Empty slots open nothing.
pub fn hotbar_info_event(
    cell: Option<&HotbarCell>,
    held: impl FnOnce(ItemId) -> Option<InventoryItem<ResourceMetadata>>,
    item_text: impl FnOnce(&InventoryItem<ResourceMetadata>) -> ItemTooltipText,
) -> Option<InputEvent> {
    let cell = cell?;
    match cell.kind {
        CELL_SKILL => cell.skill.as_ref().map(|skill| skill_info_event(skill, cell_skill_level(cell))),
        CELL_ITEM => {
            let held = held(ItemId(cell.item_id));
            let item = hotbar_item(cell, held.as_ref())?;
            Some(item_info_event(&item, &item_text(&item)))
        }
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod tests {

    use ragnarok_packets::{ClientTick, SkillId};

    use super::*;
    use crate::state::skills::SkillAcquisition;
    use crate::world::SpriteAnimationState;

    pub(crate) fn bash() -> LearnableSkill {
        LearnableSkill {
            skill_id: SkillId(5),
            maximum_level: SkillLevel(10),
            file_name: "sm_bash".to_owned(),
            skill_name: "Bash".to_owned(),
            description: "Hits one enemy hard.".to_owned(),
            spell_point_costs: vec![8; 10],
            can_select_level: true,
            acquisition: SkillAcquisition::Job,
            required_skills: Default::default(),
            required_for_skills: Default::default(),
            sprite: None,
            actions: None,
            animation_state: SpriteAnimationState::new(ClientTick(0)),
        }
    }

    pub(crate) fn skill_cell(skill: LearnableSkill) -> HotbarCell {
        HotbarCell {
            kind: CELL_SKILL,
            skill: Some(skill),
            item_id: 0,
            amount: 0,
            item_name: String::new(),
            item_texture: None,
        }
    }

    pub(crate) fn item_cell(item_id: u32, name: &str) -> HotbarCell {
        HotbarCell {
            kind: CELL_ITEM,
            skill: None,
            item_id,
            amount: 5,
            item_name: name.to_owned(),
            item_texture: None,
        }
    }

    fn red_potion(index: u16) -> InventoryItem<ResourceMetadata> {
        let mut item = hotbar_item(&item_cell(501, "Red Potion"), None).unwrap();
        item.index = InventoryIndex(index);
        item.details = InventoryItemDetails::Regular {
            amount: 12,
            equipped_position: EquipPosition::empty(),
            flags: RegularItemFlags::IDENTIFIED,
        };
        item
    }

    fn text(body: &str) -> ItemTooltipText {
        ItemTooltipText {
            body: body.to_owned(),
            compare: String::new(),
        }
    }

    const SLOT: (HotbarTab, HotbarSlot) = (HotbarTab(0), HotbarSlot(3));

    fn release(over_interface: bool, over_hotbar: bool, accepted: bool) -> DragRelease {
        DragRelease {
            over_interface,
            over_hotbar,
            accepted,
        }
    }

    #[test]
    fn a_drag_off_the_bar_clears_the_slot() {
        // Released over the world.
        assert_eq!(slot_to_clear_on_release(Some(SLOT), release(false, false, false)), Some(SLOT));
        // Released over another window that does not take hotbar drags.
        assert_eq!(slot_to_clear_on_release(Some(SLOT), release(true, false, false)), Some(SLOT));
    }

    #[test]
    fn a_drop_that_is_taken_or_lands_on_the_bar_keeps_the_slot() {
        // Another hotbar slot took it (move / swap) or the bar itself.
        assert_eq!(slot_to_clear_on_release(Some(SLOT), release(true, true, true)), None);
        assert_eq!(slot_to_clear_on_release(Some(SLOT), release(true, true, false)), None);
        // A window that accepts it (inventory or skill tree clear it there;
        // equipment or others decide for themselves).
        assert_eq!(slot_to_clear_on_release(Some(SLOT), release(true, false, true)), None);
        // Not a hotbar drag (or cancelled with Escape): nothing to clear.
        assert_eq!(slot_to_clear_on_release(None, release(false, false, false)), None);
    }

    #[test]
    fn escape_cancels_only_a_held_item_or_skill() {
        let skill = MouseMode::Custom {
            mode: MouseInputMode::MoveSkill {
                skill: bash(),
                source: SkillSource::Hotbar { tab: SLOT.0, slot: SLOT.1 },
            },
        };
        assert!(escape_cancels_drag(&skill));
        assert_eq!(hotbar_drag_source(&skill), Some(SLOT));
        let item = MouseMode::Custom {
            mode: MouseInputMode::MoveItem {
                item: red_potion(2),
                source: ItemSource::Inventory,
            },
        };
        assert!(escape_cancels_drag(&item));
        assert_eq!(hotbar_drag_source(&item), None, "an inventory drag is not a hotbar drag");
        assert!(!escape_cancels_drag(&MouseMode::Default));
        assert_eq!(hotbar_drag_source(&MouseMode::Default), None);
    }

    #[test]
    fn right_click_on_a_skill_opens_its_description() {
        let cell = skill_cell(bash());
        let event = hotbar_info_event(Some(&cell), |_| panic!("no item lookup for a skill"), |_| unreachable!());
        let Some(InputEvent::ToggleSkillDescription { skill_id, title, body }) = event else {
            panic!("expected the skill description, got {event:?}");
        };
        assert_eq!(skill_id, SkillId(5));
        assert_eq!(title, "Bash");
        assert!(body.starts_with("Level: 10 / 10"), "{body}");
        assert!(body.contains("Hits one enemy hard."), "{body}");
    }

    #[test]
    fn right_click_on_an_item_opens_the_inventory_details() {
        let cell = item_cell(501, "Red Potion");
        let held = red_potion(7);
        let event = hotbar_info_event(
            Some(&cell),
            |item_id| (item_id == ItemId(501)).then(|| held.clone()),
            |item| text(&format!("{} details", item.metadata.name)),
        );
        let Some(InputEvent::OpenItemDetails { key, title, body }) = event else {
            panic!("expected item details, got {event:?}");
        };
        assert_eq!(title, "Red Potion");
        assert_eq!(body, "Red Potion details");
        assert_eq!(
            key,
            item_details_key(ItemSource::Inventory, &held),
            "same window as the inventory's right-click"
        );
    }

    #[test]
    fn a_used_up_item_still_has_details_and_can_be_dragged() {
        let cell = item_cell(501, "Red Potion");
        let event = hotbar_info_event(Some(&cell), |_| None, |item| text(&item.metadata.name));
        assert!(matches!(event, Some(InputEvent::OpenItemDetails { ref title, .. }) if title == "Red Potion"));
        let stand_in = hotbar_item(&cell, None).unwrap();
        assert_eq!(stand_in.item_id, ItemId(501));
        assert!(matches!(stand_in.details, InventoryItemDetails::Regular { amount: 0, .. }));
    }

    #[test]
    fn a_drop_on_another_slot_swaps_and_sends_both_hotkeys() {
        use crate::state::hotbar::Hotbar;

        let mut hotbar = Hotbar::default();
        hotbar.set_skill_local(HotbarTab(0), HotbarSlot(0), bash());
        hotbar.set_item_local(HotbarTab(0), HotbarSlot(4), 501, 5);
        let updates = hotbar
            .swap_local(HotbarTab(0), HotbarSlot(0), HotbarTab(0), HotbarSlot(4))
            .expect("two different slots swap");
        assert_eq!(hotbar.cell(HotbarTab(0), HotbarSlot(4)).unwrap().kind, CELL_SKILL);
        assert_eq!(hotbar.cell(HotbarTab(0), HotbarSlot(0)).unwrap().kind, CELL_ITEM);
        let [(tab, slot, moved), (_, back_slot, back)] = updates;
        assert_eq!((tab, slot), (HotbarTab(0), HotbarSlot(4)));
        assert!(moved.is_skill() && moved.item_or_skill_id == 5);
        assert_eq!(back_slot, HotbarSlot(0));
        assert!(!back.is_skill() && back.item_or_skill_id == 501);
        // Dropping back onto the same slot changes nothing and sends nothing.
        assert!(
            hotbar
                .swap_local(HotbarTab(0), HotbarSlot(4), HotbarTab(0), HotbarSlot(4))
                .is_none()
        );
    }

    #[test]
    fn the_clear_sends_an_unbound_shortcut_key_change() {
        use ragnarok_bytes::ByteWriter;
        use ragnarok_packets::{HotkeyData, PacketExt, SetHotkeyData2Packet};

        // `Hotbar::clear_slot`, which `ClearHotbar` runs, sends this packet.
        let mut writer = ByteWriter::new();
        SetHotkeyData2Packet::new(SLOT.0, SLOT.1, HotkeyData::UNBOUND)
            .packet_to_bytes(&mut writer)
            .unwrap();
        assert_eq!(writer.as_slice(), &[0x21, 0x0B, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn right_click_on_an_empty_slot_opens_nothing() {
        assert!(hotbar_info_event(None, |_| None, |_| unreachable!()).is_none());
        let empty = HotbarCell {
            kind: crate::state::hotbar::CELL_EMPTY,
            ..item_cell(0, "")
        };
        assert!(hotbar_info_event(Some(&empty), |_| None, |_| unreachable!()).is_none());
    }
}
