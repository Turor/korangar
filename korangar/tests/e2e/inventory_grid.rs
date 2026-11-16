//! Scrolling Inventory, Storage and Cart grids (inventory review #1). The
//! offline case decodes Turoran's cap packets from wire bytes and sizes every
//! grid; the live case reads the TEST character's real cap and item list.

use korangar::e2e::{GridView, ItemCapacity, ItemCapacityList, LiveConnect, LogicSession, offline_inventory_grid};

#[test]
fn inventory_grid_scrolls_to_server_caps_offline() {
    let report = offline_inventory_grid().unwrap();
    assert_eq!(report.capacities, [
        ItemCapacity {
            list: ItemCapacityList::Inventory,
            used: None,
            maximum: 600,
        },
        ItemCapacity {
            list: ItemCapacityList::Storage,
            used: Some(37),
            maximum: 600,
        },
        ItemCapacity {
            list: ItemCapacityList::Cart,
            used: Some(12),
            maximum: 100,
        },
    ]);

    assert_eq!(report.inventory, GridView {
        capacity: Some(600),
        cells: 600,
        rows: 60,
        count: "Items 41 / 600".to_owned(),
        last_item_visible: true,
    });
    assert_eq!(report.storage.cells, 600);
    assert_eq!(report.storage.count, "Items 37 / 600");
    assert_eq!(report.cart.cells, 100);
    assert_eq!(report.cart.count, "Items 12 / 100");

    // 16000 cells would be too many to draw: used stacks plus one spare row.
    assert_eq!(report.huge_storage.cells, 150);
    assert!(report.huge_storage.last_item_visible);

    // Before 0x0B18 arrives the grid still covers Hercules' fixed 100.
    assert_eq!(report.inventory_before_cap.cells, 100);
    assert!(
        report.inventory_before_cap.last_item_visible,
        "stack 41 was hidden by the old 40-cell grid"
    );
}

#[test]
fn inventory_grid_on_test_character() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let report = session.inventory_grid_on_test().unwrap();
    eprintln!(
        "E2E_INVENTORY_GRID rows={} capacity={:?} cells={} count={:?}",
        report.inventory_rows, report.inventory_capacity, report.grid.cells, report.grid.count
    );
    assert!(report.inventory_rows > 0, "the TEST character's inventory never arrived");
    let capacity = report.inventory_capacity.expect("TEST never sent ZC_EXTEND_BODYITEM_SIZE");
    assert!(capacity >= 100);
    assert!(report.grid.cells >= report.inventory_rows);
    assert!(report.grid.cells >= capacity as usize);
    assert!(report.grid.last_item_visible);
}
