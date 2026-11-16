//! View rules for the Inventory, Storage and Cart grids (inventory review
//! #2, #3, #7, #9, #12): category tabs, hidden worn gear, favorites, display
//! sort, search dimming and the weight bar marks.
//!
//! Everything here is view-only. Server inventory indexes never change; the
//! grid just draws [`view_order`] of the item list.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::{EquippableItemFlags, RegularItemFlags};
use rust_state::{Path, RustState, State};

use crate::state::inventory::Inventory;
use crate::state::item_finder::ItemFinderState;
use crate::state::{ClientState, ClientStatePathExt, client_state};
use crate::world::ResourceMetadata;

/// Hercules item types as the client receives them (`itemtype()` in clif.c).
pub mod item_type {
    pub const HEALING: u8 = 0;
    pub const USABLE: u8 = 2;
    pub const ETC: u8 = 3;
    pub const ARMOR: u8 = 4;
    pub const WEAPON: u8 = 5;
    pub const CARD: u8 = 6;
    pub const PET_EGG: u8 = 7;
    pub const PET_ARMOR: u8 = 8;
    pub const DELAY_CONSUME: u8 = 11;
    pub const SHADOW_GEAR: u8 = 12;
    pub const CASH: u8 = 18;
}

/// Grid tab. `All` is the default; the rest follow roBrowser's
/// Usable / Equip / Etc / Favorites tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InventoryTab {
    #[default]
    All,
    Usable,
    Equip,
    Etc,
    Favorites,
}

impl InventoryTab {
    pub const ALL: [InventoryTab; 5] = [Self::All, Self::Usable, Self::Equip, Self::Etc, Self::Favorites];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Usable => "Usable",
            Self::Equip => "Equip",
            Self::Etc => "Etc",
            Self::Favorites => "Favorites",
        }
    }
}

/// Display sort. `Server` keeps the server's order; the others only change
/// the drawn order. There is no separate Compact: the grid never has gaps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InventorySort {
    #[default]
    Server,
    Type,
    Name,
    Weight,
    Newest,
    Amount,
}

impl InventorySort {
    pub fn next(self) -> Self {
        match self {
            Self::Server => Self::Type,
            Self::Type => Self::Name,
            Self::Name => Self::Weight,
            Self::Weight => Self::Newest,
            Self::Newest => Self::Amount,
            Self::Amount => Self::Server,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Server => "Sort: Default",
            Self::Type => "Sort: Type",
            Self::Name => "Sort: Name",
            Self::Weight => "Sort: Weight",
            Self::Newest => "Sort: Newest",
            Self::Amount => "Sort: Amount",
        }
    }
}

/// Category tab for an item type (roBrowser `getItemTab`).
pub fn type_tab(item_type: u8) -> InventoryTab {
    use item_type::*;
    match item_type {
        HEALING | USABLE | DELAY_CONSUME | CASH => InventoryTab::Usable,
        WEAPON | ARMOR | SHADOW_GEAR | PET_EGG | PET_ARMOR => InventoryTab::Equip,
        _ => InventoryTab::Etc,
    }
}

/// `PlaceETCTab` from the item list or a later `ZC_INVENTORY_TAB`.
pub fn is_favorite<Meta>(item: &InventoryItem<Meta>) -> bool {
    match &item.details {
        InventoryItemDetails::Regular { flags, .. } => flags.contains(RegularItemFlags::IN_ETC_TAB),
        InventoryItemDetails::Equippable { flags, .. } => flags.contains(EquippableItemFlags::IN_ETC_TAB),
    }
}

pub fn set_favorite<Meta>(item: &mut InventoryItem<Meta>, favorite: bool) {
    match &mut item.details {
        InventoryItemDetails::Regular { flags, .. } => flags.set(RegularItemFlags::IN_ETC_TAB, favorite),
        InventoryItemDetails::Equippable { flags, .. } => flags.set(EquippableItemFlags::IN_ETC_TAB, favorite),
    }
}

/// Worn gear lives in the Gear window, not the grid. Equipped ammo stays
/// visible, as in roBrowser.
pub fn is_worn<Meta>(item: &InventoryItem<Meta>) -> bool {
    match &item.details {
        InventoryItemDetails::Equippable { equipped_position, .. } => !equipped_position.is_empty(),
        InventoryItemDetails::Regular { .. } => false,
    }
}

pub fn shows_in_tab<Meta>(item: &InventoryItem<Meta>, tab: InventoryTab) -> bool {
    if is_worn(item) {
        return false;
    }
    match tab {
        InventoryTab::All => true,
        InventoryTab::Favorites => is_favorite(item),
        tab => !is_favorite(item) && type_tab(item.item_type) == tab,
    }
}

fn amount_of<Meta>(item: &InventoryItem<Meta>) -> u16 {
    match &item.details {
        InventoryItemDetails::Regular { amount, .. } => *amount,
        InventoryItemDetails::Equippable { .. } => 1,
    }
}

fn type_rank(item_type: u8) -> u8 {
    match type_tab(item_type) {
        InventoryTab::Usable => 0,
        InventoryTab::Equip => 1,
        _ => 2,
    }
}

/// Positions into `items` in drawn order for this tab and sort. The item
/// list is kept in arrival order, so "newest" is the list reversed. `weight`
/// is the per-piece weight from the Item Finder catalog; unknown items sort
/// last.
pub fn view_order(
    items: &[InventoryItem<ResourceMetadata>],
    tab: InventoryTab,
    sort: InventorySort,
    weight: impl Fn(u32) -> Option<i32>,
) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).filter(|&position| shows_in_tab(&items[position], tab)).collect();
    match sort {
        InventorySort::Server => order.sort_by_key(|&position| items[position].index.0),
        InventorySort::Type => order.sort_by_key(|&position| {
            let item = &items[position];
            (type_rank(item.item_type), item.item_type, item.item_id.0, item.index.0)
        }),
        InventorySort::Name => order.sort_by_cached_key(|&position| {
            let item = &items[position];
            (item.metadata.name.to_lowercase(), item.index.0)
        }),
        InventorySort::Weight => order.sort_by_key(|&position| {
            let item = &items[position];
            let total = weight(item.item_id.0).map(|weight| i64::from(weight) * i64::from(amount_of(item)));
            (std::cmp::Reverse(total), item.index.0)
        }),
        InventorySort::Newest => order.reverse(),
        InventorySort::Amount => order.sort_by_key(|&position| {
            let item = &items[position];
            (std::cmp::Reverse(amount_of(item)), item.index.0)
        }),
    }
    order
}

/// Card ids in an item's slots. Forged and named items use the slots for
/// other data (`0x00FF`, `0x00FE`, `0xFF00`), which never match a card.
pub fn card_ids<Meta>(item: &InventoryItem<Meta>) -> impl Iterator<Item = u32> + '_ {
    let special = matches!(item.slot[0], 0x00FF | 0x00FE | 0xFF00);
    item.slot.iter().copied().filter(move |&id| !special && id != 0)
}

/// Search hit on name, description, a slotted card's name, or an exact item
/// id. `query` must already be trimmed and lowercased; empty matches all.
pub fn search_matches<'a>(item: &InventoryItem<ResourceMetadata>, query: &str, card_name: impl Fn(u32) -> Option<&'a str>) -> bool {
    if query.is_empty() {
        return true;
    }
    if query.parse::<u32>().is_ok_and(|id| id == item.item_id.0) {
        return true;
    }
    item.metadata.name.to_lowercase().contains(query)
        || item.metadata.description.to_lowercase().contains(query)
        || card_ids(item).any(|id| card_name(id).is_some_and(|name| name.to_lowercase().contains(query)))
}

pub fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
}

/// Drop of an inventory item on a tab: `Some(true)` asks the server to
/// favorite it (Favorites tab), `Some(false)` to unfavorite it (any other
/// tab), `None` when nothing changes. Same as dragging onto a roBrowser tab.
pub fn favorite_drop<Meta>(tab: InventoryTab, item: &InventoryItem<Meta>) -> Option<bool> {
    match (tab == InventoryTab::Favorites, is_favorite(item)) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

/// Catalog facts the view needs: a card's name and an item's weight.
type Lookup = Option<(String, i32)>;

/// Drawn state of one item grid: the filtered and sorted copy of the list,
/// which cells the search dims, and the tab / sort / search the user picked.
/// Rebuilt by [`InventoryView::refresh`] only when something changed.
#[derive(Default, RustState)]
pub struct InventoryView {
    items: Vec<InventoryItem<ResourceMetadata>>,
    misses: Vec<bool>,
    tab: InventoryTab,
    sort: InventorySort,
    search: String,
    lookups: HashMap<u32, Lookup>,
    lookup_catalog_len: usize,
    key: Option<u64>,
}

impl InventoryView {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn items(&self) -> &[InventoryItem<ResourceMetadata>] {
        &self.items
    }

    pub fn tab(&self) -> InventoryTab {
        self.tab
    }

    pub fn set_tab(&mut self, tab: InventoryTab) {
        self.tab = tab;
    }

    pub fn sort(&self) -> InventorySort {
        self.sort
    }

    pub fn set_sort(&mut self, sort: InventorySort) {
        self.sort = sort;
    }

    pub fn set_search(&mut self, search: impl Into<String>) {
        self.search = search.into();
    }

    /// The cell at this view position is drawn dimmed.
    pub fn is_search_miss(&self, position: usize) -> bool {
        self.misses.get(position).copied().unwrap_or(false)
    }

    /// Item and card ids the catalog has not been asked about yet.
    pub fn wanted_lookups(&self, items: &[InventoryItem<ResourceMetadata>], catalog_len: usize) -> Vec<u32> {
        let stale = catalog_len != self.lookup_catalog_len;
        let mut wanted: Vec<u32> = items
            .iter()
            .flat_map(|item| std::iter::once(item.item_id.0).chain(card_ids(item)))
            .filter(|id| stale || !self.lookups.contains_key(id))
            .collect();
        wanted.sort_unstable();
        wanted.dedup();
        wanted
    }

    /// Stores catalog answers and rebuilds the drawn list if anything changed.
    pub fn refresh(&mut self, items: &[InventoryItem<ResourceMetadata>], catalog_len: usize, resolved: Vec<(u32, Lookup)>) {
        if catalog_len != self.lookup_catalog_len {
            self.lookups.clear();
            self.lookup_catalog_len = catalog_len;
        }
        self.lookups.extend(resolved);

        let key = self.key_for(items);
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);

        let lookups = &self.lookups;
        let order = view_order(items, self.tab, self.sort, |id| {
            lookups.get(&id).and_then(|lookup| lookup.as_ref()).map(|(_, weight)| *weight)
        });
        let query = normalize_query(&self.search);
        let card_name = |id: u32| lookups.get(&id).and_then(|lookup| lookup.as_ref()).map(|(name, _)| name.as_str());
        self.misses = order
            .iter()
            .map(|&position| !search_matches(&items[position], &query, card_name))
            .collect();
        self.items = order.into_iter().map(|position| items[position].clone()).collect();
    }

    fn key_for(&self, items: &[InventoryItem<ResourceMetadata>]) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.tab, self.sort, &self.search, self.lookups.len(), self.lookup_catalog_len).hash(&mut hasher);
        for item in items {
            (item.index.0, item.item_id.0, item.item_type, item.slot, &item.metadata.name).hash(&mut hasher);
            item.metadata
                .texture
                .as_ref()
                .map(|texture| Arc::as_ptr(texture) as usize)
                .hash(&mut hasher);
            match &item.details {
                InventoryItemDetails::Regular {
                    amount,
                    equipped_position,
                    flags,
                } => (0u8, *amount, equipped_position.bits(), flags.bits()).hash(&mut hasher),
                InventoryItemDetails::Equippable {
                    equipped_position,
                    refinement_level,
                    enchantment_level,
                    flags,
                    ..
                } => (
                    1u8,
                    equipped_position.bits(),
                    *refinement_level,
                    *enchantment_level,
                    flags.bits(),
                )
                    .hash(&mut hasher),
            }
        }
        hasher.finish()
    }
}

/// Rebuilds the Inventory, Storage and Cart views. Called once per frame;
/// cheap when nothing changed.
pub fn refresh_inventory_views(state: &mut State<ClientState>) {
    refresh_one(state, client_state().inventory());
    refresh_one(state, client_state().storage());
    refresh_one(state, client_state().cart());
}

fn refresh_one<P>(state: &mut State<ClientState>, path: P)
where
    P: Path<ClientState, Inventory> + Copy,
{
    let finder_path = client_state().item_finder();
    let finder: &ItemFinderState = state.get(&finder_path);
    let catalog_len = finder.catalog_len();
    let wanted = state.get(&path).wanted_lookups(catalog_len);
    let finder: &ItemFinderState = state.get(&finder_path);
    let resolved = wanted
        .into_iter()
        .map(|id| {
            (
                id,
                finder.item_name_and_weight(id).map(|(name, weight)| (name.to_owned(), weight)),
            )
        })
        .collect();
    state.follow_mut(path).refresh_view(catalog_len, resolved);
}

/// Weight bar marks: from 50% HP/SP stop regenerating, from 90% you cannot
/// attack or use skills.
pub const WEIGHT_MARKS: [f32; 2] = [0.5, 0.9];

/// Fill fraction of the weight bar, clamped to `0.0..=1.0`.
pub fn weight_fraction(current: u32, maximum: u32) -> f32 {
    if maximum == 0 {
        return 0.0;
    }
    (current as f32 / maximum as f32).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use korangar_networking::{InventoryItem, InventoryItemDetails};
    use ragnarok_packets::{EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions, RegularItemFlags};

    use super::*;
    use crate::world::ResourceMetadata;

    pub(crate) fn regular(index: u16, item_id: u32, item_type: u8, amount: u16, name: &str) -> InventoryItem<ResourceMetadata> {
        InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name: name.to_owned(),
                description: String::new(),
            },
            index: InventoryIndex(index),
            item_id: ItemId(item_id),
            item_type,
            slot: [0; 4],
            hire_expiration_date: 0,
            details: InventoryItemDetails::Regular {
                amount,
                equipped_position: EquipPosition::empty(),
                flags: RegularItemFlags::IDENTIFIED,
            },
        }
    }

    pub(crate) fn gear(index: u16, item_id: u32, name: &str, worn: bool, cards: [u32; 4]) -> InventoryItem<ResourceMetadata> {
        let option = || ItemOptions {
            index: 0,
            value: 0,
            parameter: 0,
        };
        InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name: name.to_owned(),
                description: String::new(),
            },
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

    fn sample() -> Vec<InventoryItem<ResourceMetadata>> {
        let mut card = regular(5, 4001, item_type::CARD, 1, "Poring Card");
        set_favorite(&mut card, true);
        vec![
            regular(2, 501, item_type::HEALING, 30, "Red Potion"),
            gear(3, 1201, "Knife", true, [0; 4]),
            gear(4, 1202, "Knife", false, [4001, 0, 0, 0]),
            card,
            regular(6, 909, item_type::ETC, 12, "Jellopy"),
        ]
    }

    #[test]
    fn worn_gear_is_hidden_and_tabs_split_by_type() {
        let items = sample();
        let names = |tab| {
            view_order(&items, tab, InventorySort::Server, |_| None)
                .into_iter()
                .map(|position| items[position].index.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(InventoryTab::All), [2, 4, 5, 6]);
        assert_eq!(names(InventoryTab::Usable), [2]);
        assert_eq!(names(InventoryTab::Equip), [4]);
        assert_eq!(names(InventoryTab::Etc), [6]);
        assert_eq!(names(InventoryTab::Favorites), [5]);
    }

    #[test]
    fn sorts_are_view_only() {
        let items = sample();
        let order = |sort| {
            view_order(&items, InventoryTab::All, sort, |id| match id {
                501 => Some(70),
                909 => Some(10),
                1202 => Some(600),
                _ => None,
            })
            .into_iter()
            .map(|position| items[position].index.0)
            .collect::<Vec<_>>()
        };
        assert_eq!(order(InventorySort::Name), [6, 4, 5, 2]);
        assert_eq!(order(InventorySort::Newest), [6, 5, 4, 2]);
        assert_eq!(order(InventorySort::Amount), [2, 6, 4, 5]);
        assert_eq!(order(InventorySort::Type), [2, 4, 6, 5]);
        // Stack weight: potions 30 x 7.0, knife 60.0, jellopy 12 x 1.0, card unknown.
        assert_eq!(order(InventorySort::Weight), [2, 4, 6, 5]);
        assert_eq!(InventorySort::Amount.next(), InventorySort::Server);
    }

    #[test]
    fn search_covers_name_description_cards_and_ids() {
        let items = sample();
        let cards = |id: u32| (id == 4001).then_some("Poring Card");
        assert!(search_matches(&items[0], &normalize_query(" RED "), cards));
        assert!(search_matches(&items[2], "poring", cards), "slotted card name");
        assert!(!search_matches(&items[4], "poring", cards));
        assert!(search_matches(&items[4], "909", cards));
        assert!(search_matches(&items[4], "", cards));
    }

    #[test]
    fn forged_slots_are_not_cards() {
        let item = gear(7, 1201, "Knife", false, [0x00FF, 4001, 0, 0]);
        assert_eq!(card_ids(&item).count(), 0);
    }

    #[test]
    fn favorite_flag_round_trips_on_both_detail_kinds() {
        let mut potion = regular(2, 501, item_type::HEALING, 1, "Red Potion");
        let mut knife = gear(4, 1202, "Knife", false, [0; 4]);
        set_favorite(&mut potion, true);
        set_favorite(&mut knife, true);
        assert!(is_favorite(&potion) && is_favorite(&knife));
        set_favorite(&mut knife, false);
        assert!(!is_favorite(&knife));
    }

    #[test]
    fn dragging_onto_a_tab_toggles_favorite() {
        let mut potion = regular(2, 501, item_type::HEALING, 1, "Red Potion");
        assert_eq!(favorite_drop(InventoryTab::Favorites, &potion), Some(true));
        assert_eq!(favorite_drop(InventoryTab::Usable, &potion), None);
        set_favorite(&mut potion, true);
        assert_eq!(favorite_drop(InventoryTab::Favorites, &potion), None);
        assert_eq!(favorite_drop(InventoryTab::All, &potion), Some(false));
    }

    #[test]
    fn view_refresh_dims_misses_and_skips_unchanged_frames() {
        let items = sample();
        let mut view = InventoryView::default();
        assert_eq!(view.wanted_lookups(&items, 2), [501, 909, 1201, 1202, 4001]);
        view.set_search("poring");
        view.refresh(&items, 2, vec![(4001, Some(("Poring Card".to_owned(), 10))), (501, None)]);
        let drawn: Vec<u16> = view.items().iter().map(|item| item.index.0).collect();
        assert_eq!(drawn, [2, 4, 5, 6]);
        assert_eq!((0..4).map(|position| view.is_search_miss(position)).collect::<Vec<_>>(), [
            true, false, false, true
        ]);
        assert_eq!(view.wanted_lookups(&items, 2), [909, 1201, 1202]);
        let key = view.key;
        view.refresh(&items, 2, Vec::new());
        assert_eq!(view.key, key);
        view.set_tab(InventoryTab::Favorites);
        view.refresh(&items, 2, Vec::new());
        assert_eq!(view.len(), 1);
        assert_eq!(view.items()[0].index, InventoryIndex(5));
    }

    #[test]
    fn weight_marks_and_fraction() {
        assert_eq!(WEIGHT_MARKS, [0.5, 0.9]);
        assert_eq!(weight_fraction(0, 0), 0.0);
        assert_eq!(weight_fraction(450, 900), 0.5);
        assert_eq!(weight_fraction(1000, 900), 1.0);
    }
}
