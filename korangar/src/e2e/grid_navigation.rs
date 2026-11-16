//! Inventory keyboard navigation, headless (inventory review #17). The live
//! check is [`super::LogicSession::grid_navigation_on_test`].

use korangar_networking::InventoryItemDetails;
use ragnarok_packets::{EquipPosition, InventoryIndex};

use crate::e2e::inventory_view::regular;
use crate::interface::components::item_grid::{GRID_COLUMNS, tab_cells};
pub use crate::state::grid_nav::GridKey;
use crate::state::grid_nav::{row_fraction, step_cursor};
use crate::state::inventory::Inventory;
use crate::state::inventory_view::{InventorySort, InventoryTab, item_type};

/// What the offline run observed.
#[derive(Clone, Debug, PartialEq)]
pub struct OfflineGridNavReport {
    /// Cursor after each key of [`KEY_RUN`] over 25 stacks.
    pub steps: Vec<Option<usize>>,
    /// Server index Enter would use: End, then Home, under the Name sort.
    pub enter_indexes: Vec<Option<u16>>,
    /// Cursor after the last drawn stack is used up.
    pub after_use: Option<usize>,
    /// Cursor after switching to another tab.
    pub after_tab: Option<usize>,
    /// Which cells draw the focus border (cursor at position 1 of 3).
    pub focused: Vec<bool>,
    /// Row scroll fraction for the cursor at 0 and at 35 on a 100-slot grid.
    pub reveal: Vec<Option<(f32, f32)>>,
}

pub const KEY_RUN: [GridKey; 10] = [
    GridKey::Right,
    GridKey::Right,
    GridKey::Down,
    GridKey::Down,
    GridKey::Down,
    GridKey::Left,
    GridKey::Up,
    GridKey::End,
    GridKey::Home,
    GridKey::Up,
];

fn enter_index(inventory: &Inventory) -> Option<u16> {
    inventory.view().cursor_item().map(|item| item.index.0)
}

pub fn offline_grid_navigation() -> Result<OfflineGridNavReport, String> {
    let mut cursor = None;
    let steps = KEY_RUN
        .iter()
        .map(|key| {
            cursor = step_cursor(cursor, 25, *key);
            cursor
        })
        .collect();

    let mut inventory = Inventory::from_items(vec![
        regular(2, 501, item_type::HEALING, 3, EquipPosition::empty(), "Red Potion"),
        regular(6, 909, item_type::ETC, 12, EquipPosition::empty(), "Jellopy"),
        regular(8, 512, item_type::HEALING, 1, EquipPosition::empty(), "Apple"),
    ]);
    inventory.view_mut().set_sort(InventorySort::Name);
    inventory.refresh_view(0, Vec::new());

    inventory.view_mut().move_cursor(GridKey::End);
    let mut enter_indexes = vec![enter_index(&inventory)];
    inventory.view_mut().move_cursor(GridKey::Home);
    enter_indexes.push(enter_index(&inventory));

    inventory.view_mut().move_cursor(GridKey::Right);
    let focused = (0..3).map(|position| inventory.view().cursor() == Some(position)).collect();

    // Red Potion (last under Name) used up while the cursor is on it.
    inventory.view_mut().move_cursor(GridKey::End);
    let Some(item) = inventory.view().cursor_item() else {
        return Err("no cursor item".to_owned());
    };
    let InventoryItemDetails::Regular { .. } = item.details else {
        return Err("unexpected row".to_owned());
    };
    inventory.apply_use_ack(InventoryIndex(2), 0);
    inventory.refresh_view(0, Vec::new());
    let after_use = inventory.view().cursor();

    inventory.view_mut().set_tab(InventoryTab::Etc);
    inventory.refresh_view(0, Vec::new());
    let after_tab = inventory.view().cursor();

    let rows = tab_cells(InventoryTab::All, 40, 40, Some(100)) / GRID_COLUMNS;
    let reveal = vec![row_fraction(0, rows), row_fraction(35, rows)];

    Ok(OfflineGridNavReport {
        steps,
        enter_indexes,
        after_use,
        after_tab,
        focused,
        reveal,
    })
}
