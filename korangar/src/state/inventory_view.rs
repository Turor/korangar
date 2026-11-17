//! View rules for the Inventory, Storage and Cart grids (inventory review
//! #2, #3, #7, #9, #12): category tabs, hidden worn gear, favorites, display
//! sort, search dimming and the weight bar marks.
//!
//! The unified inventory (Matthew t275u) draws the cart and storage beside
//! the bag; one set of tabs, one search box and one sort (A-Z, Recent,
//! Zeny, each reversible) drive both panes, see
//! [`InventoryView::follow_controls`].
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
    pub const AMMO: u8 = 10;
    pub const DELAY_CONSUME: u8 = 11;
    /// `IT_SELECTPACKAGE` (Hercules `mmo.h`): a box the player opens to pick
    /// an item, so it is used like a consumable.
    pub const SELECT_PACKAGE: u8 = 12;
    pub const CASH: u8 = 18;
}

/// Grid tab. `All` is the default; the rest follow the official client's
/// and roBrowser's storage tabs (Usable / Equip / Ammo / Etc / Cards) plus
/// Favorites (the server-side `PlaceETCTab` flag, `ZC_INVENTORY_TAB`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum InventoryTab {
    #[default]
    All,
    Usable,
    Equip,
    Ammo,
    Etc,
    Cards,
    Favorites,
}

impl InventoryTab {
    pub const ALL: [InventoryTab; 7] = [
        Self::All,
        Self::Usable,
        Self::Equip,
        Self::Ammo,
        Self::Etc,
        Self::Cards,
        Self::Favorites,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Usable => "Usable",
            Self::Equip => "Equip",
            Self::Ammo => "Ammo",
            Self::Etc => "Etc",
            Self::Cards => "Cards",
            Self::Favorites => "Favorites",
        }
    }
}

/// Display sort. `Server` keeps the server's order; the others only change
/// the drawn order. There is no separate Compact: the grid never has gaps.
///
/// The window offers A-Z ([`Self::Name`]), Recent ([`Self::Newest`]) and
/// Zeny ([`Self::Zeny`]); see [`click_sort`]. `Type`, `Weight` and `Amount`
/// stay for the headless checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum InventorySort {
    #[default]
    Server,
    Type,
    Name,
    Weight,
    Newest,
    Amount,
    /// NPC sell value of the whole stack, highest first.
    Zeny,
}

/// The three sort buttons, left to right.
pub const SORT_BUTTONS: [InventorySort; 3] = [InventorySort::Name, InventorySort::Newest, InventorySort::Zeny];

impl InventorySort {
    /// Button text. The direction is drawn as an arrow next to it.
    pub fn button_label(self) -> &'static str {
        match self {
            Self::Server => "Default",
            Self::Type => "Type",
            Self::Name => "A-Z",
            Self::Weight => "Weight",
            Self::Newest => "Recent",
            Self::Amount => "Amount",
            Self::Zeny => "Zeny",
        }
    }

    /// The arrow points up when the drawn list starts from the low end
    /// (A first, oldest first, cheapest first).
    pub fn arrow_up(self, reversed: bool) -> bool {
        match self {
            Self::Name => !reversed,
            // Recent and Zeny start from the newest / most valuable stack.
            Self::Newest | Self::Zeny => reversed,
            _ => !reversed,
        }
    }
}

/// A click on a sort button. A new button starts in its natural direction,
/// a second click reverses it and a third goes back to the server order.
pub fn click_sort(current: InventorySort, reversed: bool, clicked: InventorySort) -> (InventorySort, bool) {
    match (current == clicked, reversed) {
        (false, _) => (clicked, false),
        (true, false) => (clicked, true),
        (true, true) => (InventorySort::Server, false),
    }
}

/// Category tab for an item type (roBrowser `getItemTab`). Pet eggs and pet
/// armor are gear, cash-shop consumables are usable.
pub fn type_tab(item_type: u8) -> InventoryTab {
    use item_type::*;
    match item_type {
        HEALING | USABLE | DELAY_CONSUME | CASH | SELECT_PACKAGE => InventoryTab::Usable,
        WEAPON | ARMOR | PET_EGG | PET_ARMOR => InventoryTab::Equip,
        AMMO => InventoryTab::Ammo,
        CARD => InventoryTab::Cards,
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
        InventoryTab::Ammo => 2,
        InventoryTab::Etc => 3,
        _ => 4,
    }
}

/// NPC sell value of a whole stack, if the price table knows the item.
pub fn stack_value<Meta>(item: &InventoryItem<Meta>, sell_price: Option<u32>) -> Option<u64> {
    sell_price.map(|price| u64::from(price) * u64::from(amount_of(item)))
}

/// Positions into `items` in drawn order for this tab and sort. The item
/// list is kept in arrival order, so "newest" is the list reversed. `weight`
/// is the per-piece weight from the Item Finder catalog; unknown items sort
/// last.
#[cfg(test)]
pub fn view_order(
    items: &[InventoryItem<ResourceMetadata>],
    tab: InventoryTab,
    sort: InventorySort,
    weight: impl Fn(u32) -> Option<i32>,
) -> Vec<usize> {
    view_order_sorted(items, tab, sort, false, weight, |_| None)
}

/// [`view_order`] with a direction and the NPC sell price per piece.
/// `reversed` flips the order; stacks the price table does not know stay
/// last under Zeny either way.
pub fn view_order_sorted(
    items: &[InventoryItem<ResourceMetadata>],
    tab: InventoryTab,
    sort: InventorySort,
    reversed: bool,
    weight: impl Fn(u32) -> Option<i32>,
    sell_price: impl Fn(u32) -> Option<u32>,
) -> Vec<usize> {
    if sort == InventorySort::Zeny {
        let mut order: Vec<usize> = (0..items.len()).filter(|&position| shows_in_tab(&items[position], tab)).collect();
        order.sort_by_key(|&position| {
            let item = &items[position];
            let value = stack_value(item, sell_price(item.item_id.0));
            let direction = match (value, reversed) {
                (None, _) => (1, 0),
                (Some(value), false) => (0, u64::MAX - value),
                (Some(value), true) => (0, value),
            };
            (direction, item.index.server_order())
        });
        return order;
    }
    let mut order = view_order_unreversed(items, tab, sort, weight);
    if reversed && sort != InventorySort::Server {
        order.reverse();
    }
    order
}

fn view_order_unreversed(
    items: &[InventoryItem<ResourceMetadata>],
    tab: InventoryTab,
    sort: InventorySort,
    weight: impl Fn(u32) -> Option<i32>,
) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).filter(|&position| shows_in_tab(&items[position], tab)).collect();
    match sort {
        InventorySort::Server => order.sort_by_key(|&position| items[position].index.server_order()),
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
        // Handled by `view_order_sorted`; without prices it is the server order.
        InventorySort::Zeny => order.sort_by_key(|&position| items[position].index.server_order()),
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
    /// Second click on the active sort button.
    reversed: bool,
    search: String,
    lookups: HashMap<u32, Lookup>,
    /// NPC sell price per piece (`System/itemprices.turoran.v1.json`).
    prices: HashMap<u32, Option<u32>>,
    lookup_catalog_len: usize,
    key: Option<u64>,
    /// Keyboard cursor: a drawn position (`state::grid_nav`).
    cursor: Option<usize>,
    /// A grid key was the last input, so Enter belongs to the grid.
    cursor_armed: bool,
    /// Columns this view's grid drew last frame (`0` before the first
    /// layout). Written during layout, which only has `&self`.
    drawn_columns: std::cell::Cell<usize>,
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
        if self.tab != tab {
            // A cursor position means nothing on another tab.
            self.clear_cursor();
        }
        self.tab = tab;
    }

    pub fn sort(&self) -> InventorySort {
        self.sort
    }

    pub fn set_sort(&mut self, sort: InventorySort) {
        self.sort = sort;
    }

    pub fn reversed(&self) -> bool {
        self.reversed
    }

    pub fn set_reversed(&mut self, reversed: bool) {
        self.reversed = reversed;
    }

    /// A sort button was clicked (see [`click_sort`]).
    pub fn click_sort(&mut self, clicked: InventorySort) {
        let (sort, reversed) = click_sort(self.sort, self.reversed, clicked);
        self.sort = sort;
        self.reversed = reversed;
    }

    /// Tab, sort, direction and search, as persisted per character.
    pub fn controls(&self) -> ViewControls {
        ViewControls {
            tab: self.tab,
            sort: self.sort,
            reversed: self.reversed,
        }
    }

    pub fn apply_controls(&mut self, controls: ViewControls) {
        self.set_tab(controls.tab);
        self.sort = controls.sort;
        self.reversed = controls.reversed;
    }

    /// The cart and storage panes follow the bag's tab, sort and search.
    pub fn follow_controls(&mut self, controls: ViewControls, search: &str) {
        self.apply_controls(controls);
        if self.search != search {
            self.set_search(search);
        }
    }

    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    /// Columns the grid drew last frame; [`GRID_COLUMNS`] before the first
    /// layout.
    ///
    /// [`GRID_COLUMNS`]: crate::interface::components::item_grid::GRID_COLUMNS
    pub fn columns(&self) -> usize {
        match self.drawn_columns.get() {
            0 => crate::interface::components::item_grid::GRID_COLUMNS,
            columns => columns,
        }
    }

    /// Record the column count the grid laid out, so arrow keys step by a
    /// drawn row.
    pub fn note_columns(&self, columns: usize) {
        self.drawn_columns.set(columns);
    }

    pub fn move_cursor(&mut self, key: crate::state::grid_nav::GridKey) {
        self.cursor = crate::state::grid_nav::step_cursor_in(self.cursor, self.items.len(), key, self.columns());
        self.cursor_armed = self.cursor.is_some();
    }

    pub fn clear_cursor(&mut self) {
        self.cursor = None;
        self.cursor_armed = false;
    }

    pub fn cursor_armed(&self) -> bool {
        self.cursor_armed && self.cursor.is_some()
    }

    /// Keeps the highlighted cell but gives Enter back to chat.
    pub fn disarm_cursor(&mut self) {
        self.cursor_armed = false;
    }

    /// The stack under the keyboard cursor.
    pub fn cursor_item(&self) -> Option<&InventoryItem<ResourceMetadata>> {
        self.cursor.and_then(|cursor| self.items.get(cursor))
    }

    pub fn search(&self) -> &str {
        &self.search
    }

    /// Catalog name of a card slotted in one of this list's items.
    pub fn card_name(&self, id: u32) -> Option<&str> {
        self.lookups
            .get(&id)
            .and_then(|lookup| lookup.as_ref())
            .map(|(name, _)| name.as_str())
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
        self.refresh_priced(items, catalog_len, resolved, Vec::new());
    }

    /// [`Self::refresh`] with sell prices for the same ids.
    pub fn refresh_priced(
        &mut self,
        items: &[InventoryItem<ResourceMetadata>],
        catalog_len: usize,
        resolved: Vec<(u32, Lookup)>,
        prices: Vec<(u32, Option<u32>)>,
    ) {
        if catalog_len != self.lookup_catalog_len {
            self.lookups.clear();
            self.prices.clear();
            self.lookup_catalog_len = catalog_len;
        }
        self.lookups.extend(resolved);
        self.prices.extend(prices);

        let key = self.key_for(items);
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);

        let lookups = &self.lookups;
        let prices = &self.prices;
        let order = view_order_sorted(
            items,
            self.tab,
            self.sort,
            self.reversed,
            |id| lookups.get(&id).and_then(|lookup| lookup.as_ref()).map(|(_, weight)| *weight),
            |id| prices.get(&id).copied().flatten(),
        );
        let query = normalize_query(&self.search);
        let card_name = |id: u32| lookups.get(&id).and_then(|lookup| lookup.as_ref()).map(|(name, _)| name.as_str());
        self.misses = order
            .iter()
            .map(|&position| !search_matches(&items[position], &query, card_name))
            .collect();
        self.items = order.into_iter().map(|position| items[position].clone()).collect();
        self.cursor = match self.items.len() {
            0 => None,
            len => self.cursor.map(|cursor| cursor.min(len - 1)),
        };
    }

    fn key_for(&self, items: &[InventoryItem<ResourceMetadata>]) -> u64 {
        let mut hasher = DefaultHasher::new();
        (
            self.tab,
            self.sort,
            self.reversed,
            &self.search,
            self.lookups.len(),
            self.prices.len(),
            self.lookup_catalog_len,
        )
            .hash(&mut hasher);
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

/// Tab, sort and direction of one grid, saved per character in
/// `client/inventory_view.ron`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ViewControls {
    pub tab: InventoryTab,
    pub sort: InventorySort,
    pub reversed: bool,
}

/// Rebuilds the Inventory, Storage and Cart views. Called once per frame;
/// cheap when nothing changed. The storage and cart panes take the bag's
/// tab, sort and search first, so one set of controls filters both panes.
pub fn refresh_inventory_views(state: &mut State<ClientState>) {
    sync_pane_controls(state);
    refresh_one(state, client_state().inventory());
    refresh_one(state, client_state().storage());
    refresh_one(state, client_state().cart());
}

fn sync_pane_controls(state: &mut State<ClientState>) {
    sync_pane(state, client_state().storage());
    sync_pane(state, client_state().cart());
}

/// Copies the bag's tab, sort and search onto one pane when they differ.
fn sync_pane<P>(state: &mut State<ClientState>, pane: P)
where
    P: Path<ClientState, Inventory> + Copy,
{
    let bag = client_state().inventory();
    let differs = {
        let bag_view = state.get(&bag).view();
        let pane_view = state.get(&pane).view();
        bag_view.controls() != pane_view.controls() || bag_view.search() != pane_view.search()
    };
    if differs {
        let controls = state.get(&bag).view().controls();
        let search = state.get(&bag).view().search().to_owned();
        state.follow_mut(pane).view_mut().follow_controls(controls, &search);
    }
}

fn refresh_one<P>(state: &mut State<ClientState>, path: P)
where
    P: Path<ClientState, Inventory> + Copy,
{
    let finder_path = client_state().item_finder();
    let finder: &ItemFinderState = state.get(&finder_path);
    // A price table that loads later must also refresh the lookups.
    let generation = finder.catalog_len() + finder.price_count();
    let wanted = state.get(&path).wanted_lookups(generation);
    let finder: &ItemFinderState = state.get(&finder_path);
    let prices = wanted.iter().map(|&id| (id, finder.sell_price(id))).collect();
    let resolved = wanted
        .into_iter()
        .map(|id| {
            (
                id,
                finder.item_name_and_weight(id).map(|(name, weight)| (name.to_owned(), weight)),
            )
        })
        .collect();
    state.follow_mut(path).refresh_view_priced(generation, resolved, prices);
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
    }

    #[test]
    fn tabs_follow_the_official_storage_categories() {
        use item_type::*;
        assert_eq!(type_tab(HEALING), InventoryTab::Usable);
        assert_eq!(type_tab(USABLE), InventoryTab::Usable);
        assert_eq!(type_tab(DELAY_CONSUME), InventoryTab::Usable);
        assert_eq!(type_tab(CASH), InventoryTab::Usable);
        assert_eq!(type_tab(WEAPON), InventoryTab::Equip);
        assert_eq!(type_tab(ARMOR), InventoryTab::Equip);
        assert_eq!(type_tab(SELECT_PACKAGE), InventoryTab::Usable);
        assert_eq!(type_tab(PET_EGG), InventoryTab::Equip);
        assert_eq!(type_tab(PET_ARMOR), InventoryTab::Equip);
        assert_eq!(type_tab(AMMO), InventoryTab::Ammo);
        assert_eq!(type_tab(CARD), InventoryTab::Cards);
        assert_eq!(type_tab(ETC), InventoryTab::Etc);
        assert_eq!(type_tab(1), InventoryTab::Etc);
        assert_eq!(InventoryTab::ALL.map(InventoryTab::label), [
            "All",
            "Usable",
            "Equip",
            "Ammo",
            "Etc",
            "Cards",
            "Favorites"
        ]);
    }

    #[test]
    fn ammo_and_cards_get_their_own_tabs_and_favorites_win() {
        let mut favorite_arrow = regular(9, 1751, item_type::AMMO, 20, "Silver Arrow");
        set_favorite(&mut favorite_arrow, true);
        let items = vec![
            regular(2, 1750, item_type::AMMO, 500, "Arrow"),
            regular(3, 4001, item_type::CARD, 1, "Poring Card"),
            regular(4, 909, item_type::ETC, 3, "Jellopy"),
            favorite_arrow,
        ];
        let drawn = |tab| {
            view_order(&items, tab, InventorySort::Server, |_| None)
                .into_iter()
                .map(|position| items[position].index.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(drawn(InventoryTab::Ammo), [2]);
        assert_eq!(drawn(InventoryTab::Cards), [3]);
        assert_eq!(drawn(InventoryTab::Etc), [4]);
        assert_eq!(drawn(InventoryTab::Favorites), [9]);
        assert_eq!(drawn(InventoryTab::All), [2, 3, 4, 9]);
    }

    #[test]
    fn sort_buttons_reverse_then_reset() {
        use InventorySort::*;
        assert_eq!(SORT_BUTTONS, [Name, Newest, Zeny]);
        assert_eq!(click_sort(Server, false, Name), (Name, false));
        assert_eq!(click_sort(Name, false, Name), (Name, true));
        assert_eq!(click_sort(Name, true, Name), (Server, false));
        // Another button starts fresh in its natural direction.
        assert_eq!(click_sort(Name, true, Zeny), (Zeny, false));
        assert_eq!(click_sort(Zeny, false, Newest), (Newest, false));
        // Arrows: A-Z up, Z-A down; Recent and Zeny start from the top.
        assert!(Name.arrow_up(false) && !Name.arrow_up(true));
        assert!(!Newest.arrow_up(false) && Newest.arrow_up(true));
        assert!(!Zeny.arrow_up(false) && Zeny.arrow_up(true));
        assert_eq!(SORT_BUTTONS.map(InventorySort::button_label), ["A-Z", "Recent", "Zeny"]);
    }

    #[test]
    fn a_z_recent_and_zeny_orders_and_their_reversals() {
        let items = sample();
        let price = |id: u32| match id {
            501 => Some(25),  // 30 potions: 750
            909 => Some(3),   // 12 jellopy: 36
            1202 => Some(25), // knife: 25
            _ => None,        // card: unknown, always last
        };
        let order = |sort, reversed| {
            view_order_sorted(&items, InventoryTab::All, sort, reversed, |_| None, price)
                .into_iter()
                .map(|position| items[position].index.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(order(InventorySort::Name, false), [6, 4, 5, 2]);
        assert_eq!(order(InventorySort::Name, true), [2, 5, 4, 6]);
        assert_eq!(order(InventorySort::Newest, false), [6, 5, 4, 2]);
        assert_eq!(order(InventorySort::Newest, true), [2, 4, 5, 6]);
        assert_eq!(order(InventorySort::Zeny, false), [2, 6, 4, 5]);
        assert_eq!(order(InventorySort::Zeny, true), [4, 6, 2, 5]);
        // The server order has no direction.
        assert_eq!(order(InventorySort::Server, true), [2, 4, 5, 6]);
        assert_eq!(stack_value(&items[0], Some(25)), Some(750));
        assert_eq!(stack_value(&items[0], None), None);
    }

    #[test]
    fn view_controls_round_trip_and_panes_copy_the_bag() {
        let mut bag = InventoryView::default();
        bag.set_tab(InventoryTab::Cards);
        bag.click_sort(InventorySort::Zeny);
        bag.click_sort(InventorySort::Zeny);
        bag.set_search("poring");
        assert_eq!(bag.controls(), ViewControls {
            tab: InventoryTab::Cards,
            sort: InventorySort::Zeny,
            reversed: true,
        });
        let mut storage = InventoryView::default();
        storage.follow_controls(bag.controls(), bag.search());
        assert_eq!(storage.controls(), bag.controls());
        assert_eq!(storage.search(), "poring");

        let text = ron::to_string(&bag.controls()).unwrap();
        assert_eq!(ron::from_str::<ViewControls>(&text).unwrap(), bag.controls());
    }

    #[test]
    fn prices_rebuild_the_zeny_order() {
        let items = sample();
        let mut view = InventoryView::default();
        view.click_sort(InventorySort::Zeny);
        view.refresh_priced(&items, 1, Vec::new(), Vec::new());
        let before: Vec<u16> = view.items().iter().map(|item| item.index.0).collect();
        assert_eq!(before, [2, 4, 5, 6], "no prices: server order");
        view.refresh_priced(&items, 1, Vec::new(), vec![(909, Some(100))]);
        let after: Vec<u16> = view.items().iter().map(|item| item.index.0).collect();
        assert_eq!(after, [6, 2, 4, 5]);
    }

    #[test]
    fn zeny_sort_works_without_a_price_file_on_disk() {
        // No System/itemprices file in the data path (a binary-only
        // auto-update install): the built-in table still prices the stacks.
        let mut finder = crate::state::item_finder::ItemFinderState::default();
        finder.load_prices(None);
        assert!(finder.price_count() > 1_000);
        let items = sample();
        let prices: Vec<(u32, Option<u32>)> = items
            .iter()
            .map(|item| (item.item_id.0, finder.sell_price(item.item_id.0)))
            .collect();
        let mut view = InventoryView::default();
        view.click_sort(InventorySort::Zeny);
        view.refresh_priced(&items, 1, Vec::new(), prices);
        let order: Vec<u16> = view.items().iter().map(|item| item.index.0).collect();
        // Red Potion x30 at 25 z is the most valuable stack.
        assert_eq!(order.first(), Some(&2));
        assert_ne!(order, [2, 4, 5, 6], "not just the server order");
        let values: Vec<u64> = view
            .items()
            .iter()
            .map(|item| stack_value(item, finder.sell_price(item.item_id.0)).unwrap_or(0))
            .collect();
        assert!(values.windows(2).all(|pair| pair[0] >= pair[1]), "{order:?} {values:?}");
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
