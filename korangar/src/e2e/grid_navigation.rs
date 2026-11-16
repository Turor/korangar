//! Inventory keyboard navigation, headless (inventory review #17). The live
//! check is [`super::LogicSession::grid_navigation_on_test`].

use korangar_networking::InventoryItemDetails;
use ragnarok_packets::{EquipPosition, InventoryIndex};

use crate::e2e::inventory_view::regular;
use crate::interface::components::item_grid::{GRID_COLUMNS, tab_cells};
use crate::interface::item_menu_flow::{SlotReaction, react_to_secondary};
use crate::interface::resource::ItemSource;
pub use crate::state::grid_nav::GridKey;
use crate::state::grid_nav::{GridFrame, enter_free_for_chat, grid_frame, row_fraction, step_cursor};
use crate::state::inventory::Inventory;
use crate::state::inventory_view::{InventorySort, InventoryTab, item_type};
use crate::state::quick_move::QuickMoveWindows;

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
    /// One Enter frame with chat open, as (stack used, chat focused), in
    /// order: right after End; after a mouse click; after Escape; after
    /// closing and reopening the inventory; while an NPC dialog waits;
    /// while a text box has focus.
    pub enter_frames: Vec<(Option<u16>, bool)>,
    /// Cursor after the inventory closed and reopened.
    pub cursor_after_reopen: Option<usize>,
    /// Armed, then two Enter frames: (stack used, chat focused) each.
    pub enter_twice: Vec<(Option<u16>, bool)>,
    /// Armed, an F1 / letter press, then Enter.
    pub other_key_then_enter: (Option<u16>, bool),
    /// Cursor before and after ArrowRight while the item menu is open,
    /// and whether that armed Enter.
    pub arrows_with_menu: (Option<usize>, Option<usize>, bool),
    /// Equipped arrows with storage open: Alt+right-click result, and the
    /// right-click menu labels.
    pub ammo_quick_move: String,
    pub ammo_menu: Vec<String>,
}

fn open_frame() -> GridFrame {
    GridFrame {
        inventory_open: true,
        ..GridFrame::default()
    }
}

/// What one frame does with Enter and chat open: the stack the grid uses
/// and whether chat takes focus (`lib.rs` rule).
fn enter_frame(inventory: &mut Inventory, frame: GridFrame) -> (Option<u16>, bool) {
    let used = grid_frame(inventory.view_mut(), &GridFrame { enter: true, ..frame });
    let chat_open = true;
    (
        used.map(|index| index.0),
        enter_free_for_chat(used.is_some(), false) && chat_open,
    )
}

fn armed(inventory: &mut Inventory) {
    grid_frame(inventory.view_mut(), &GridFrame {
        keys: vec![GridKey::End],
        ..open_frame()
    });
}

fn grid_enter_frames(inventory: &mut Inventory) -> (Vec<(Option<u16>, bool)>, Option<usize>) {
    let mut frames = Vec::new();
    armed(inventory);
    frames.push(enter_frame(inventory, open_frame()));

    armed(inventory);
    grid_frame(inventory.view_mut(), &GridFrame {
        mouse_clicked: true,
        ..open_frame()
    });
    frames.push(enter_frame(inventory, open_frame()));

    armed(inventory);
    grid_frame(inventory.view_mut(), &GridFrame {
        escape: true,
        ..open_frame()
    });
    frames.push(enter_frame(inventory, open_frame()));

    armed(inventory);
    grid_frame(inventory.view_mut(), &GridFrame::default());
    let cursor_after_reopen = inventory.view().cursor();
    frames.push(enter_frame(inventory, open_frame()));

    armed(inventory);
    frames.push(enter_frame(inventory, GridFrame {
        dialog_waiting: true,
        ..open_frame()
    }));

    armed(inventory);
    frames.push(enter_frame(inventory, GridFrame {
        keys_taken: true,
        ..open_frame()
    }));
    (frames, cursor_after_reopen)
}

/// (stack used, chat focused) for one Enter frame.
type EnterOutcome = (Option<u16>, bool);
/// Cursor before / after menu arrows, and whether they armed Enter.
type MenuArrows = (Option<usize>, Option<usize>, bool);

fn repeat_frames(inventory: &mut Inventory) -> (Vec<EnterOutcome>, EnterOutcome, MenuArrows) {
    armed(inventory);
    let enter_twice = vec![enter_frame(inventory, open_frame()), enter_frame(inventory, open_frame())];

    armed(inventory);
    grid_frame(inventory.view_mut(), &GridFrame {
        other_key: true,
        ..open_frame()
    });
    let other_key_then_enter = enter_frame(inventory, open_frame());

    inventory.view_mut().clear_cursor();
    grid_frame(inventory.view_mut(), &GridFrame {
        keys: vec![GridKey::Home],
        ..open_frame()
    });
    inventory.view_mut().disarm_cursor();
    let before = inventory.view().cursor();
    grid_frame(inventory.view_mut(), &GridFrame {
        keys: vec![GridKey::Right],
        menu_open: true,
        ..open_frame()
    });
    let after = inventory.view().cursor();
    let armed_by_menu_arrows = inventory.view().cursor_armed();
    (enter_twice, other_key_then_enter, (before, after, armed_by_menu_arrows))
}

fn equipped_ammo_reactions() -> (String, Vec<String>) {
    let arrows = regular(5, 1750, 10, 300, EquipPosition::AMMO, "Arrow");
    let windows = QuickMoveWindows {
        storage: true,
        ..QuickMoveWindows::default()
    };
    let quick = match react_to_secondary(ItemSource::Inventory, &arrows, "Arrow", true, windows, true) {
        SlotReaction::Hint(text) => format!("hint: {text}"),
        SlotReaction::Move(_) => "move".to_owned(),
        _ => "other".to_owned(),
    };
    let labels = match react_to_secondary(ItemSource::Inventory, &arrows, "Arrow", false, windows, true) {
        SlotReaction::OpenMenu { rows, .. } => rows.into_iter().map(|row| row.label).collect(),
        _ => Vec::new(),
    };
    (quick, labels)
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

    let (enter_frames, cursor_after_reopen) = grid_enter_frames(&mut inventory);
    let (enter_twice, other_key_then_enter, arrows_with_menu) = repeat_frames(&mut inventory);
    let (ammo_quick_move, ammo_menu) = equipped_ammo_reactions();

    let rows = tab_cells(InventoryTab::All, 40, 40, Some(100)) / GRID_COLUMNS;
    let reveal = vec![row_fraction(0, rows), row_fraction(35, rows)];

    Ok(OfflineGridNavReport {
        steps,
        enter_indexes,
        after_use,
        after_tab,
        focused,
        reveal,
        enter_frames,
        cursor_after_reopen,
        enter_twice,
        other_key_then_enter,
        arrows_with_menu,
        ammo_quick_move,
        ammo_menu,
    })
}
