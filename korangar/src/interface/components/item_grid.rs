//! Scrolling item grid for Inventory, Storage and Cart.
//!
//! The grid used to be a fixed 4x10 array, so stacks past the 40th could not
//! be seen or dragged. It now has one row per ten slots up to the server's cap
//! (600 inventory slots and 100 cart slots on Turoran) and scrolls inside the
//! window. Storage caps that are too large to draw (Hercules sends
//! `MAX_STORAGE`, 16000) show the used stacks plus one empty row.

use std::cell::{Cell, UnsafeCell};

use korangar_components::item_box;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use rust_state::{Path, PathExt, Selector, State, VecIndexExt};

use crate::interface::resource::ItemSource;
use crate::state::ClientState;
use crate::state::inventory::{Inventory, InventoryPathExt};
use crate::state::inventory_view::{InventoryTab, InventoryViewPathExt};

/// Slots per grid row.
pub const GRID_COLUMNS: usize = 10;
/// The grid never shrinks below the old 4x10 look.
pub const GRID_MINIMUM_ROWS: usize = 4;
/// Caps above this are not drawn in full; the grid shows the used stacks and
/// one spare row instead.
pub const GRID_FULL_CAP_LIMIT: usize = 1_000;

/// Inventory cap before `ZC_EXTEND_BODYITEM_SIZE` arrives (Hercules
/// `FIXED_INVENTORY_SIZE`).
pub const INVENTORY_FALLBACK_CAPACITY: u32 = korangar_networking::FIXED_INVENTORY_SIZE;
/// Cart cap before `ZC_NOTIFY_CARTITEM_COUNTINFO` arrives (Hercules
/// `MAX_CART`).
pub const CART_FALLBACK_CAPACITY: u32 = 100;

/// Number of cells the grid draws, always whole rows.
pub fn grid_cells(used: usize, capacity: Option<u32>) -> usize {
    let cells = match capacity.map(|capacity| capacity as usize) {
        Some(capacity) if capacity <= GRID_FULL_CAP_LIMIT => capacity.max(used),
        _ => used + GRID_COLUMNS,
    };
    cells.div_ceil(GRID_COLUMNS).max(GRID_MINIMUM_ROWS) * GRID_COLUMNS
}

/// "Items 12 / 600", or "Items 12" when the cap is unknown.
pub fn item_count_text(used: usize, capacity: Option<u32>) -> String {
    match capacity {
        Some(maximum) => format!("Items {used} / {maximum}"),
        None => format!("Items {used}"),
    }
}

/// Cells for one tab. `All` keeps the server cap; the other tabs only need
/// their own stacks plus a spare row.
pub fn tab_cells(tab: InventoryTab, used: usize, shown: usize, capacity: Option<u32>) -> usize {
    match tab {
        InventoryTab::All => grid_cells(used, capacity),
        _ => grid_cells(shown, None),
    }
}

/// Rows of [`item_box`]es, one per ten slots, drawing the inventory view
/// (tab, sort, search) rather than the raw server list. Rows are rebuilt
/// only when the row count changes.
pub struct ItemGrid<P> {
    inventory_path: P,
    source: ItemSource,
    fallback_capacity: Option<u32>,
    rows: Vec<ElementBox<ClientState>>,
}

impl<P> ItemGrid<P> {
    pub fn new(inventory_path: P, source: ItemSource, fallback_capacity: Option<u32>) -> Self {
        Self {
            inventory_path,
            source,
            fallback_capacity,
            rows: Vec::new(),
        }
    }
}

impl<P> Element<ClientState> for ItemGrid<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            use korangar_interface::prelude::*;

            let inventory = state.get(&self.inventory_path);
            let capacity = inventory.capacity().or(self.fallback_capacity);
            let rows = tab_cells(inventory.view().tab(), inventory.item_count(), inventory.view().len(), capacity) / GRID_COLUMNS;

            self.rows.truncate(rows);
            for row in self.rows.len()..rows {
                let inventory_path = self.inventory_path;
                let items_path = inventory_path.view().items();
                let source = self.source;
                self.rows.push(ErasedElement::new(split! {
                    gaps: theme().window().gaps(),
                    children: std::array::from_fn::<_, GRID_COLUMNS, _>(|column| {
                        let position = row * GRID_COLUMNS + column;
                        item_box! {
                            item_path: items_path.index(position),
                            source: source,
                            dim: Some(Box::new(move |state: &State<ClientState>| {
                                state.get(&inventory_path).view().is_search_miss(position)
                            })),
                            focused: Some(Box::new(move |state: &State<ClientState>| {
                                state.get(&inventory_path).view().cursor() == Some(position)
                            })),
                        }
                    }),
                }));
            }

            self.rows.iter_mut().enumerate().for_each(|(index, row)| {
                row.create_layout_info(state, store.child_store(index as u64), resolver);
            });
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        self.rows.iter().enumerate().for_each(|(index, row)| {
            row.lay_out(state, store.child_store(index as u64), &(), layout);
        });
    }
}

/// Used / max line under the grid.
pub struct ItemCountLabel<P> {
    inventory_path: P,
    fallback_capacity: Option<u32>,
    last: Cell<Option<(usize, Option<u32>)>>,
    text: UnsafeCell<String>,
}

impl<P> ItemCountLabel<P> {
    pub fn new(inventory_path: P, fallback_capacity: Option<u32>) -> Self {
        Self {
            inventory_path,
            fallback_capacity,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<P> Selector<ClientState, String> for ItemCountLabel<P>
where
    P: Path<ClientState, Inventory>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let inventory = self.inventory_path.follow_safe(state);
        let key = (inventory.item_count(), inventory.capacity().or(self.fallback_capacity));
        if self.last.get() != Some(key) {
            unsafe {
                *self.text.get() = item_count_text(key.0, key.1);
            }
            self.last.set(Some(key));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

/// `scroll_view!` reveal for the keyboard cursor: the cursor row, once per
/// cursor move, so mouse scrolling still works between key presses.
pub struct CursorReveal<P> {
    inventory_path: P,
    fallback_capacity: Option<u32>,
    last: Cell<Option<(usize, usize)>>,
    value: UnsafeCell<Option<(f32, f32)>>,
}

impl<P> CursorReveal<P> {
    pub fn new(inventory_path: P, fallback_capacity: Option<u32>) -> Self {
        Self {
            inventory_path,
            fallback_capacity,
            last: Cell::default(),
            value: UnsafeCell::default(),
        }
    }
}

impl<P> Selector<ClientState, Option<(f32, f32)>> for CursorReveal<P>
where
    P: Path<ClientState, Inventory>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a Option<(f32, f32)>> {
        let inventory = self.inventory_path.follow_safe(state);
        let view = inventory.view();
        let capacity = inventory.capacity().or(self.fallback_capacity);
        let rows = tab_cells(view.tab(), inventory.item_count(), view.len(), capacity) / GRID_COLUMNS;
        let value = match view.cursor() {
            Some(cursor) if self.last.get() != Some((cursor, rows)) => {
                self.last.set(Some((cursor, rows)));
                crate::state::grid_nav::row_fraction(cursor, rows)
            }
            Some(_) => None,
            None => {
                self.last.set(None);
                None
            }
        };
        unsafe {
            *self.value.get() = value;
            Some(self.value.as_ref_unchecked())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GRID_COLUMNS, grid_cells, item_count_text, tab_cells};
    use crate::state::inventory_view::InventoryTab;

    #[test]
    fn turoran_inventory_draws_all_600_slots() {
        assert_eq!(grid_cells(0, Some(600)), 600);
        assert_eq!(grid_cells(41, Some(600)), 600);
    }

    #[test]
    fn cart_draws_its_100_slots() {
        assert_eq!(grid_cells(12, Some(100)), 100);
    }

    #[test]
    fn small_or_unknown_caps_keep_the_old_four_rows() {
        assert_eq!(grid_cells(0, None), 4 * GRID_COLUMNS);
        assert_eq!(grid_cells(3, Some(20)), 4 * GRID_COLUMNS);
    }

    #[test]
    fn huge_storage_cap_shows_used_plus_a_spare_row() {
        assert_eq!(grid_cells(137, Some(16_000)), 150);
        assert_eq!(grid_cells(140, None), 150);
    }

    #[test]
    fn more_items_than_the_cap_are_never_hidden() {
        assert_eq!(grid_cells(105, Some(100)), 110);
    }

    #[test]
    fn filtered_tabs_size_to_their_own_stacks() {
        assert_eq!(tab_cells(InventoryTab::All, 41, 38, Some(600)), 600);
        assert_eq!(tab_cells(InventoryTab::Usable, 41, 12, Some(600)), 40);
        assert_eq!(tab_cells(InventoryTab::Etc, 141, 75, Some(600)), 90);
    }

    #[test]
    fn count_text() {
        assert_eq!(item_count_text(12, Some(600)), "Items 12 / 600");
        assert_eq!(item_count_text(7, None), "Items 7");
    }
}
