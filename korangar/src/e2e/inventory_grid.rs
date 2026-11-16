//! Scrolling Inventory, Storage and Cart grids, headless.
//!
//! [`offline_inventory_grid`] feeds the three cap packets Hercules sends
//! (`0x0B18`, `0x00F2`, `0x0121`) through the map-server handlers as wire
//! bytes, stores each cap the way the client does, and sizes every grid. The
//! live check is [`super::LogicSession::inventory_grid_on_test`].

use korangar_networking::{ItemCapacity, ItemCapacityList, decode_item_capacity};

use crate::interface::components::item_grid::{
    CART_FALLBACK_CAPACITY, GRID_COLUMNS, INVENTORY_FALLBACK_CAPACITY, grid_cells, item_count_text,
};
use crate::state::inventory::Inventory;

/// How one window's grid is sized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridView {
    pub capacity: Option<u32>,
    pub cells: usize,
    pub rows: usize,
    pub count: String,
    /// Grid index of the last used stack is drawn (the old grid stopped at 40).
    pub last_item_visible: bool,
}

/// Sizes a grid for `used` stacks the way
/// [`crate::interface::components::item_grid::ItemGrid`] does.
pub fn grid_view(used: usize, capacity: Option<u32>) -> GridView {
    let cells = grid_cells(used, capacity);
    GridView {
        capacity,
        cells,
        rows: cells / GRID_COLUMNS,
        count: item_count_text(used, capacity),
        last_item_visible: used == 0 || used - 1 < cells,
    }
}

/// What the offline run observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineInventoryGridReport {
    pub capacities: Vec<ItemCapacity>,
    /// 41 stacks in a 600-slot inventory.
    pub inventory: GridView,
    /// 37 stacks in 600-slot storage.
    pub storage: GridView,
    /// 12 stacks in a 100-slot cart.
    pub cart: GridView,
    /// 137 stacks when the server sends `MAX_STORAGE` 16000.
    pub huge_storage: GridView,
    /// 41 stacks before any cap packet arrived.
    pub inventory_before_cap: GridView,
}

/// Turoran's caps as Hercules puts them on the wire.
pub const TURORAN_CAP_BYTES: [u8; 24] = [
    0x18, 0x0B, 0xF4, 0x01, // ZC_EXTEND_BODYITEM_SIZE: 500 over 100
    0xF2, 0x00, 37, 0, 0x58, 0x02, // ZC_NOTIFY_STOREITEM_COUNTINFO: 37 / 600
    0x21, 0x01, 12, 0, 100, 0, 0x10, 0x27, 0, 0, 0x40, 0x9C, 0, 0, // ZC_NOTIFY_CARTITEM_COUNTINFO: 12 / 100
];

pub fn offline_inventory_grid() -> Result<OfflineInventoryGridReport, String> {
    let capacities = decode_item_capacity(&TURORAN_CAP_BYTES)?;

    let mut inventory = Inventory::default();
    let mut storage = Inventory::default();
    let mut cart = Inventory::default();
    let inventory_before_cap = grid_view(41, inventory.capacity().or(Some(INVENTORY_FALLBACK_CAPACITY)));
    for capacity in &capacities {
        match capacity.list {
            ItemCapacityList::Inventory => inventory.set_capacity(capacity.maximum),
            ItemCapacityList::Storage => storage.set_capacity(capacity.maximum),
            ItemCapacityList::Cart => cart.set_capacity(capacity.maximum),
        }
    }

    let mut huge = Inventory::default();
    huge.set_capacity(16_000);

    Ok(OfflineInventoryGridReport {
        capacities,
        inventory: grid_view(41, inventory.capacity().or(Some(INVENTORY_FALLBACK_CAPACITY))),
        storage: grid_view(37, storage.capacity()),
        cart: grid_view(12, cart.capacity().or(Some(CART_FALLBACK_CAPACITY))),
        huge_storage: grid_view(137, huge.capacity()),
        inventory_before_cap,
    })
}
