//! Inventory grid view, headless (inventory review #2, #3, #7, #9, #12).
//!
//! [`offline_inventory_view`] builds a Turoran-like inventory, applies the
//! favorites ack Hercules sends (`ZC_INVENTORY_TAB` `0x0908`) as wire bytes,
//! encodes the request (`CZ_INVENTORY_TAB` `0x0907`), and reads back what
//! each tab, sort and search draws. The live check is
//! [`super::LogicSession::inventory_view_on_test`].

use korangar_networking::{InventoryItem, InventoryItemDetails, NoMetadata, decode_item_favorites};
use ragnarok_bytes::ByteWriter;
use ragnarok_packets::{
    EquipPosition, EquippableItemFlags, INVENTORY_TAB_FAVORITE, InventoryIndex, InventoryTabRequestPacket, ItemId, ItemOptions, PacketExt,
    RegularItemFlags,
};

use crate::state::inventory::Inventory;
pub use crate::state::inventory_view::{InventorySort, InventoryTab};
use crate::state::inventory_view::{WEIGHT_MARKS, favorite_drop, item_type, weight_fraction};
use crate::world::ResourceMetadata;

/// Server indexes drawn, in order, for one tab or sort.
pub type DrawnIndexes = Vec<u16>;

/// What the offline run observed.
#[derive(Clone, Debug, PartialEq)]
pub struct OfflineInventoryViewReport {
    pub tabs: Vec<(InventoryTab, DrawnIndexes)>,
    pub sorts: Vec<(InventorySort, DrawnIndexes)>,
    /// Indexes drawn faded for the search "poring" on the All tab.
    pub poring_dimmed: DrawnIndexes,
    /// Favorites decoded from the `0x0908` bytes.
    pub favorite_acks: Vec<(InventoryIndex, bool)>,
    /// `0x0907` bytes asking to favorite index 6.
    pub favorite_request: Vec<u8>,
    /// Drop of the Red Potion on Favorites, then on Usable once favorited.
    pub drops: (Option<bool>, Option<bool>),
    pub weight_marks: [f32; 2],
    pub weight_fill: f32,
}

fn metadata(name: &str, description: &str) -> ResourceMetadata {
    ResourceMetadata {
        texture: None,
        name: name.to_owned(),
        description: description.to_owned(),
    }
}

fn regular(index: u16, item_id: u32, kind: u8, amount: u16, worn: EquipPosition, name: &str) -> InventoryItem<ResourceMetadata> {
    InventoryItem {
        metadata: metadata(name, ""),
        index: InventoryIndex(index),
        item_id: ItemId(item_id),
        item_type: kind,
        slot: [0; 4],
        hire_expiration_date: 0,
        details: InventoryItemDetails::Regular {
            amount,
            equipped_position: worn,
            flags: RegularItemFlags::IDENTIFIED,
        },
    }
}

fn gear(index: u16, item_id: u32, name: &str, worn: bool, cards: [u32; 4]) -> InventoryItem<ResourceMetadata> {
    let option = || ItemOptions {
        index: 0,
        value: 0,
        parameter: 0,
    };
    InventoryItem {
        metadata: metadata(name, "A sharp blade."),
        index: InventoryIndex(index),
        item_id: ItemId(item_id),
        item_type: item_type::WEAPON,
        slot: cards,
        hire_expiration_date: 0,
        details: InventoryItemDetails::Equippable {
            equip_position: EquipPosition::RIGHT_HAND,
            equipped_position: if worn { EquipPosition::RIGHT_HAND } else { EquipPosition::empty() },
            bind_on_equip_type: 0,
            w_item_sprite_number: 0,
            option_count: 0,
            option_data: [option(), option(), option(), option(), option()],
            forge_data: None,
            refinement_level: 0,
            enchantment_level: 0,
            flags: EquippableItemFlags::IDENTIFIED,
        },
    }
}

/// Converts a server item list for the view (names are not needed for the
/// tab split).
pub fn with_blank_metadata(items: Vec<InventoryItem<NoMetadata>>) -> Vec<InventoryItem<ResourceMetadata>> {
    items
        .into_iter()
        .map(|item| InventoryItem {
            metadata: metadata("", ""),
            index: item.index,
            item_id: item.item_id,
            item_type: item.item_type,
            slot: item.slot,
            hire_expiration_date: item.hire_expiration_date,
            details: item.details,
        })
        .collect()
}

/// Server indexes `inventory` draws for a tab and sort.
pub fn drawn(inventory: &mut Inventory, tab: InventoryTab, sort: InventorySort, catalog_len: usize) -> DrawnIndexes {
    inventory.view_mut().set_tab(tab);
    inventory.view_mut().set_sort(sort);
    inventory.refresh_view(catalog_len, Vec::new());
    inventory.view().items().iter().map(|item| item.index.0).collect()
}

/// `ZC_INVENTORY_TAB` for index 7 (Poring Card) moving into favorites.
pub const FAVORITE_ACK_BYTES: [u8; 5] = [0x08, 0x09, 9, 0, INVENTORY_TAB_FAVORITE];

pub fn offline_inventory_view() -> Result<OfflineInventoryViewReport, String> {
    let mut inventory = Inventory::from_items(vec![
        regular(2, 501, item_type::HEALING, 30, EquipPosition::empty(), "Red Potion"),
        gear(3, 1201, "Knife", true, [0; 4]),
        gear(4, 1202, "Knife", false, [4001, 0, 0, 0]),
        regular(5, 1750, 10, 500, EquipPosition::AMMO, "Arrow"),
        regular(6, 909, item_type::ETC, 12, EquipPosition::empty(), "Jellopy"),
        regular(7, 4001, item_type::CARD, 1, EquipPosition::empty(), "Poring Card"),
    ]);

    let favorite_acks = decode_item_favorites(&FAVORITE_ACK_BYTES)?;
    for (index, favorite) in &favorite_acks {
        inventory.set_favorite(*index, *favorite);
    }

    let mut writer = ByteWriter::new();
    InventoryTabRequestPacket::new(InventoryIndex(6), INVENTORY_TAB_FAVORITE)
        .packet_to_bytes(&mut writer)
        .map_err(|error| format!("{error:?}"))?;

    // Catalog answers: Poring Card is a card name, weights in tenths.
    let catalog = vec![
        (4001, Some(("Poring Card".to_owned(), 10))),
        (501, Some(("Red Potion".to_owned(), 70))),
        (909, Some(("Jellopy".to_owned(), 10))),
        (1202, Some(("Knife".to_owned(), 400))),
        (1750, Some(("Arrow".to_owned(), 1))),
    ];
    inventory.refresh_view(5, catalog);

    let tabs = InventoryTab::ALL
        .into_iter()
        .map(|tab| (tab, drawn(&mut inventory, tab, InventorySort::Server, 5)))
        .collect();
    let sorts = [
        InventorySort::Type,
        InventorySort::Name,
        InventorySort::Weight,
        InventorySort::Newest,
        InventorySort::Amount,
    ]
    .into_iter()
    .map(|sort| (sort, drawn(&mut inventory, InventoryTab::All, sort, 5)))
    .collect();

    inventory.view_mut().set_search(" Poring ");
    let all = drawn(&mut inventory, InventoryTab::All, InventorySort::Server, 5);
    let poring_dimmed = all
        .iter()
        .enumerate()
        .filter(|(position, _)| inventory.view().is_search_miss(*position))
        .map(|(_, index)| *index)
        .collect();

    let mut potion = regular(2, 501, item_type::HEALING, 30, EquipPosition::empty(), "Red Potion");
    let first = favorite_drop(InventoryTab::Favorites, &potion);
    crate::state::inventory_view::set_favorite(&mut potion, true);
    let second = favorite_drop(InventoryTab::Usable, &potion);

    Ok(OfflineInventoryViewReport {
        tabs,
        sorts,
        poring_dimmed,
        favorite_acks,
        favorite_request: writer.as_slice().to_vec(),
        drops: (first, second),
        weight_marks: WEIGHT_MARKS,
        weight_fill: weight_fraction(1_620, 1_800),
    })
}
