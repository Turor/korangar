//! Live grid check on Hercules TEST: log in, enter the map, and size the
//! inventory grid from the cap and item list the server really sends. Nothing
//! is moved.

use std::time::{Duration, Instant};

use korangar_networking::{ItemCapacityList, NetworkEvent};

use super::super::LogicSession;
use crate::e2e::inventory_grid::{GridView, grid_view};

/// What the live check saw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveInventoryGridReport {
    pub inventory_rows: usize,
    /// `ZC_EXTEND_BODYITEM_SIZE` turned into a slot count.
    pub inventory_capacity: Option<u32>,
    pub grid: GridView,
}

impl LogicSession {
    /// Log in, enter the map, and wait for the inventory list and cap. Only
    /// TEST.
    pub fn inventory_grid_on_test(&mut self) -> Result<LiveInventoryGridReport, String> {
        self.login_to_character_select()?;
        self.enter_world()?;

        let mut rows = 0;
        let inventory_cap = |session: &Self| {
            session
                .item_capacities
                .iter()
                .rev()
                .find(|cap| cap.list == ItemCapacityList::Inventory)
                .map(|cap| cap.maximum)
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let drained = {
                let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) else {
                    return Err("logic mode has no network".to_owned());
                };
                net.get_events(events);
                events.drain().collect::<Vec<_>>()
            };
            for event in drained {
                match event {
                    NetworkEvent::SetInventory { items } => rows = items.len(),
                    other => self.on_event(other)?,
                }
            }
            self.take_failure()?;
            std::thread::sleep(Duration::from_millis(10));
        }

        let capacity = inventory_cap(self);
        Ok(LiveInventoryGridReport {
            inventory_rows: rows,
            inventory_capacity: capacity,
            grid: grid_view(rows, capacity),
        })
    }
}
