//! Scrolling item grid for Inventory, Storage and Cart.
//!
//! The grid used to be a fixed 4x10 array, so stacks past the 40th could not
//! be seen or dragged. It now has one row per ten slots up to the server's cap
//! (600 inventory slots and 100 cart slots on Turoran) and scrolls inside the
//! window. Storage caps that are too large to draw (Hercules sends
//! `MAX_STORAGE`, 16000) show the used stacks plus one empty row.
//!
//! The grid flows with the window: [`grid_metrics`] picks the column count
//! and a square slot size from the width the window gives it, so dragging a
//! window corner reflows the slots and a narrower window draws smaller
//! squares. Rows scroll.

use std::cell::{Cell, UnsafeCell};
use std::sync::atomic::{AtomicUsize, Ordering};

use korangar_components::item_box;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::area::PartialArea;
use korangar_interface::layout::{Resolver, Resolvers, WindowLayout, with_single_resolver};
use rust_state::{Path, PathExt, Selector, State, VecIndexExt};

use crate::interface::resource::ItemSource;
use crate::state::ClientState;
use crate::state::inventory::{Inventory, InventoryPathExt};
use crate::state::inventory_view::{InventoryTab, InventoryViewPathExt};

/// Slots per row of the server-cap math ([`grid_cells`] rounds to whole
/// rows of this many). The drawn grid picks its own column count from the
/// window width, see [`grid_metrics`].
pub const GRID_COLUMNS: usize = 10;
/// Smallest slot side in interface units (still legible at UI scale 1).
pub const GRID_SLOT_MINIMUM: f32 = 24.0;
/// Largest slot side in interface units.
pub const GRID_SLOT_MAXIMUM: f32 = 48.0;
/// The slot side aims at the grid width divided by this, so the squares grow
/// and shrink with the window.
pub const GRID_SLOT_WIDTH_DIVISOR: f32 = 12.0;
/// Most columns drawn, however wide the window is.
pub const GRID_MAXIMUM_COLUMNS: usize = 20;

/// Columns and slot side for one grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridMetrics {
    pub columns: usize,
    /// Square slot side in interface units, whole pixels.
    pub slot: f32,
}

impl GridMetrics {
    /// Rows needed for `cells` slots.
    pub fn rows(&self, cells: usize) -> usize {
        cells.div_ceil(self.columns.max(1))
    }

    /// Width of one full row with `gaps` between the slots.
    pub fn row_width(&self, gaps: f32) -> f32 {
        self.columns as f32 * self.slot + self.columns.saturating_sub(1) as f32 * gaps
    }

    /// Height of `rows` rows with `gaps` between them.
    pub fn height(&self, rows: usize, gaps: f32) -> f32 {
        rows as f32 * self.slot + rows.saturating_sub(1) as f32 * gaps
    }
}

/// Lay a grid out in `width` interface units with `gaps` between slots.
///
/// The slot side is `width / GRID_SLOT_WIDTH_DIVISOR` in whole pixels, kept
/// between [`GRID_SLOT_MINIMUM`] and [`GRID_SLOT_MAXIMUM`], so the squares
/// grow and shrink smoothly with the window and never jump back when a
/// column is added. The row holds as many of those squares as fit (up to
/// [`GRID_MAXIMUM_COLUMNS`]); the few pixels left over are split around the
/// row by the grid.
pub fn grid_metrics(width: f32, gaps: f32) -> GridMetrics {
    let width = if width.is_finite() { width.max(0.0) } else { 0.0 };
    let gaps = if gaps.is_finite() { gaps.max(0.0) } else { 0.0 };
    let slot = (width / GRID_SLOT_WIDTH_DIVISOR)
        .clamp(GRID_SLOT_MINIMUM, GRID_SLOT_MAXIMUM)
        .floor();
    let columns = (((width + gaps) / (slot + gaps)).floor() as usize).clamp(1, GRID_MAXIMUM_COLUMNS);
    GridMetrics { columns, slot }
}

/// Columns the inventory grid drew last frame. Arrow keys step by a whole
/// row of this many.
static INVENTORY_COLUMNS: AtomicUsize = AtomicUsize::new(GRID_COLUMNS);

/// See [`INVENTORY_COLUMNS`].
pub fn inventory_columns() -> usize {
    INVENTORY_COLUMNS.load(Ordering::Relaxed).max(1)
}
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

/// Square [`item_box`]es in rows that flow with the window width, drawing
/// the inventory view (tab, sort, search) rather than the raw server list.
/// Cells are rebuilt only when the cell count or the slot size changes.
pub struct ItemGrid<P> {
    inventory_path: P,
    source: ItemSource,
    fallback_capacity: Option<u32>,
    cells: Vec<ElementBox<ClientState>>,
    /// Slot side the cells were built with.
    built_slot: f32,
}

impl<P> ItemGrid<P> {
    pub fn new(inventory_path: P, source: ItemSource, fallback_capacity: Option<u32>) -> Self {
        Self {
            inventory_path,
            source,
            fallback_capacity,
            cells: Vec::new(),
            built_slot: 0.0,
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
            let cells = tab_cells(inventory.view().tab(), inventory.item_count(), inventory.view().len(), capacity);

            let gaps = *state.get(&theme().window().gaps());
            let metrics = grid_metrics(resolver.push_available_area().width, gaps);
            if matches!(self.source, ItemSource::Inventory) {
                INVENTORY_COLUMNS.store(metrics.columns, Ordering::Relaxed);
            }

            if (self.built_slot - metrics.slot).abs() > f32::EPSILON {
                self.cells.clear();
                self.built_slot = metrics.slot;
            }
            self.cells.truncate(cells);
            for position in self.cells.len()..cells {
                let inventory_path = self.inventory_path;
                let items_path = inventory_path.view().items();
                self.cells.push(ErasedElement::new(item_box! {
                    item_path: items_path.index(position),
                    source: self.source,
                    side: metrics.slot,
                    dim: Some(Box::new(move |state: &State<ClientState>| {
                        state.get(&inventory_path).view().is_search_miss(position)
                    })),
                    focused: Some(Box::new(move |state: &State<ClientState>| {
                        state.get(&inventory_path).view().cursor() == Some(position)
                    })),
                }));
            }

            for row in 0..metrics.rows(cells) {
                let row_area = resolver.with_height(metrics.slot);
                let inset = ((row_area.width - metrics.row_width(gaps)) / 2.0).max(0.0).floor();
                let text_layouter = resolver.get_text_layouter();
                for column in 0..metrics.columns {
                    let position = row * metrics.columns + column;
                    let Some(cell) = self.cells.get_mut(position) else {
                        break;
                    };
                    let mut cell_resolver = Resolver::new(
                        PartialArea {
                            left: row_area.left + inset + column as f32 * (metrics.slot + gaps),
                            top: row_area.top,
                            width: metrics.slot,
                            height: Some(metrics.slot),
                        },
                        0.0,
                        text_layouter,
                    );
                    cell.create_layout_info(state, store.child_store(position as u64), &mut cell_resolver);
                }
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        self.cells.iter().enumerate().for_each(|(index, cell)| {
            cell.lay_out(state, store.child_store(index as u64), &(), layout);
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
        let columns = inventory_columns();
        let rows = tab_cells(view.tab(), inventory.item_count(), view.len(), capacity).div_ceil(columns);
        let value = match view.cursor() {
            Some(cursor) if self.last.get() != Some((cursor, rows)) => {
                self.last.set(Some((cursor, rows)));
                crate::state::grid_nav::row_fraction_in(cursor, rows, columns)
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
    use super::{
        GRID_COLUMNS, GRID_MAXIMUM_COLUMNS, GRID_SLOT_MAXIMUM, GRID_SLOT_MINIMUM, GridMetrics, grid_cells, grid_metrics, item_count_text,
        tab_cells,
    };
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

    fn fills(metrics: GridMetrics, gaps: f32) -> f32 {
        metrics.row_width(gaps)
    }

    #[test]
    fn grid_columns_and_slot_follow_the_width() {
        let gaps = 4.0;
        let narrow = grid_metrics(280.0, gaps);
        let default = grid_metrics(400.0, gaps);
        let wide = grid_metrics(620.0, gaps);
        assert_eq!(narrow, GridMetrics { columns: 10, slot: 24.0 });
        assert_eq!(default, GridMetrics { columns: 10, slot: 33.0 });
        assert_eq!(wide, GridMetrics { columns: 12, slot: 48.0 });
        for (metrics, width) in [(narrow, 280.0), (default, 400.0), (wide, 620.0)] {
            assert!(fills(metrics, gaps) <= width + 0.01, "{metrics:?} overflows {width}");
            assert!(
                width - fills(metrics, gaps) < metrics.slot + gaps,
                "{metrics:?} leaves room for a column in {width}"
            );
        }
    }

    #[test]
    fn a_narrower_grid_never_has_larger_squares() {
        let mut previous = grid_metrics(0.0, 6.0);
        for width in 1..1_500 {
            let metrics = grid_metrics(width as f32, 6.0);
            assert!(metrics.slot >= previous.slot, "{width}: {previous:?} -> {metrics:?}");
            previous = metrics;
        }
    }

    #[test]
    fn grid_slots_stay_between_the_limits() {
        for width in (0..2_000).step_by(7) {
            let metrics = grid_metrics(width as f32, 4.0);
            assert!(metrics.columns >= 1 && metrics.columns <= GRID_MAXIMUM_COLUMNS);
            assert!(
                metrics.slot >= GRID_SLOT_MINIMUM && metrics.slot <= GRID_SLOT_MAXIMUM,
                "{width}: {metrics:?}"
            );
            assert_eq!(metrics.slot, metrics.slot.floor());
            if width as f32 >= GRID_SLOT_MINIMUM {
                assert!(fills(metrics, 4.0) <= width as f32 + 0.01, "{width}: {metrics:?}");
            }
        }
    }

    #[test]
    fn very_wide_grids_cap_columns_and_slot_size() {
        let metrics = grid_metrics(4_000.0, 4.0);
        assert_eq!(metrics.columns, GRID_MAXIMUM_COLUMNS);
        assert_eq!(metrics.slot, GRID_SLOT_MAXIMUM);
    }

    #[test]
    fn tiny_or_broken_widths_fall_back_to_one_minimum_slot() {
        assert_eq!(grid_metrics(0.0, 4.0), GridMetrics {
            columns: 1,
            slot: GRID_SLOT_MINIMUM
        });
        assert_eq!(grid_metrics(f32::NAN, f32::NAN).columns, 1);
        assert_eq!(grid_metrics(60.0, 4.0).columns, 2);
        assert_eq!(grid_metrics(20.0, 4.0).columns, 1);
    }

    #[test]
    fn rows_cover_every_cell_with_a_partial_last_row() {
        let metrics = GridMetrics { columns: 7, slot: 30.0 };
        assert_eq!(metrics.rows(600), 86);
        assert_eq!(metrics.rows(0), 0);
        assert_eq!(metrics.height(3, 4.0), 98.0);
    }

    #[test]
    fn count_text() {
        assert_eq!(item_count_text(12, Some(600)), "Items 12 / 600");
        assert_eq!(item_count_text(7, None), "Items 7");
    }
}
