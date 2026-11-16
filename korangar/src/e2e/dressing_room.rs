//! Dressing-room checks. Offline clicks never touch the network. The live
//! check logs into Hercules TEST and resolves each owned headgear through the
//! game-file loader.

use korangar_networking::{InventoryItem, InventoryItemDetails, NoMetadata};
use ragnarok_packets::{EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions, RegularItemFlags};

use crate::interface::resource::ItemSource;
use crate::interface::windows::queues_preview;
use crate::state::dressing_room::{self, DressingRoom, ItemView, NO_PREVIEW, PreviewSlot, WornLook};

/// One Ctrl+click sequence on a fixed character. No packets leave the client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineDressingReport {
    pub top_preview_id: u16,
    pub top_path: String,
    pub costume_preview_id: u16,
    pub costume_path: String,
    pub top_kept_after_costume: u16,
    pub potion_notice: String,
    pub top_after_potion: u16,
    pub reset_preview_id: u16,
    pub reset_shown_id: u16,
    pub direction_before_reset: u8,
    pub direction_after_reset: u8,
    pub outbound_empty: bool,
    pub ctrl_inventory: bool,
    pub ctrl_storage: bool,
    pub ctrl_cart: bool,
    pub plain_click: bool,
    pub ctrl_equipment: bool,
}

/// Headgear the TEST character owns, resolved through the archives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveDressingReport {
    /// `no headgear on TEST character` when the bag, storage, cart, and worn
    /// look have none.
    pub note: String,
    pub checked: usize,
    pub resolved: Vec<String>,
    pub missing_view_ids: Vec<u16>,
}

pub fn offline_dressing_room() -> OfflineDressingReport {
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
    let top = slot(&room, PreviewSlot::HeadTop);
    room.apply_item(&worn, &equip(2, EquipPosition::COSTUME_HEAD_MIDDLE, 5), suffix, exists);
    let costume = slot(&room, PreviewSlot::CostumeHeadMid);
    let top_kept = slot(&room, PreviewSlot::HeadTop).preview_id;
    room.apply_item(&worn, &potion(), suffix, exists);
    let potion_notice = room.notice().to_owned();
    let top_after_potion = slot(&room, PreviewSlot::HeadTop).preview_id;
    room.rotate(1);
    room.rotate(1);
    let direction_before_reset = room.direction();
    room.reset();
    let reset = slot(&room, PreviewSlot::HeadTop);
    room.clear_loaded();

    OfflineDressingReport {
        top_preview_id: top.preview_id,
        top_path: top.sprite_path,
        costume_preview_id: costume.preview_id,
        costume_path: costume.sprite_path,
        top_kept_after_costume: top_kept,
        potion_notice,
        top_after_potion,
        reset_preview_id: reset.preview_id,
        reset_shown_id: reset.shown_id,
        direction_before_reset,
        direction_after_reset: room.direction(),
        outbound_empty: room.outbound_packets().is_empty(),
        ctrl_inventory: queues_preview(true, ItemSource::Inventory),
        ctrl_storage: queues_preview(true, ItemSource::Storage),
        ctrl_cart: queues_preview(true, ItemSource::Cart),
        plain_click: queues_preview(false, ItemSource::Inventory),
        ctrl_equipment: queues_preview(true, ItemSource::Equipment {
            position: EquipPosition::HEAD_TOP,
        }),
    }
}

fn slot(room: &DressingRoom, slot: PreviewSlot) -> dressing_room::SlotView {
    let name = slot.name();
    room.slot_views().into_iter().find(|view| view.slot == name).expect("slot")
}

fn suffix(view_id: u16) -> Option<String> {
    match view_id {
        5 => Some("_리본".to_owned()),
        8 => Some("_고글".to_owned()),
        _ => None,
    }
}

fn exists(path: &str) -> bool {
    path.ends_with("여_리본") || path.ends_with("여_고글")
}

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

/// View ids the live login should try on. Inventory rows come first; worn
/// headgear from the character list fills in when the bag has none.
pub fn headgear_to_preview(items: &[InventoryItem<NoMetadata>], worn_heads: [u16; 3]) -> Vec<ItemView> {
    let mut views: Vec<ItemView> = items
        .iter()
        .map(dressing_room::item_view)
        .filter(|view| view.view_id > 0 && view.slot.is_some_and(PreviewSlot::is_head))
        .collect();
    if views.is_empty() {
        let slots = [PreviewSlot::HeadBottom, PreviewSlot::HeadTop, PreviewSlot::HeadMid];
        for (slot, view_id) in slots.into_iter().zip(worn_heads) {
            if view_id > 0 {
                views.push(ItemView { slot: Some(slot), view_id });
            }
        }
    }
    views
}

pub fn no_preview_message() -> &'static str {
    NO_PREVIEW
}
