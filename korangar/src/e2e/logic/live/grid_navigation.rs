//! Live grid navigation on Hercules TEST: log in, enter the map, run the
//! cursor over the real inventory and read the stack Enter would use.
//! Nothing is used or sent.

use std::time::{Duration, Instant};

use korangar_networking::NetworkEvent;

use super::super::LogicSession;
use crate::e2e::inventory_view::with_blank_metadata;
use crate::state::grid_nav::GridKey;
use crate::state::inventory::Inventory;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveGridNavReport {
    pub inventory_rows: usize,
    /// Server index under the cursor after End, and after Home.
    pub end_index: Option<u16>,
    pub home_index: Option<u16>,
    /// Server index of the last and first drawn stack.
    pub last_drawn: Option<u16>,
    pub first_drawn: Option<u16>,
}

impl LogicSession {
    /// Only TEST. Moves the cursor, never sends a use.
    pub fn grid_navigation_on_test(&mut self) -> Result<LiveGridNavReport, String> {
        self.login_to_character_select()?;
        self.enter_world()?;

        let mut items = Vec::new();
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
                    NetworkEvent::SetInventory { items: list } => items = with_blank_metadata(list),
                    other => self.on_event(other)?,
                }
            }
            self.take_failure()?;
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut inventory = Inventory::from_items(items);
        inventory.refresh_view(0, Vec::new());
        let cursor_index = |inventory: &Inventory| inventory.view().cursor_item().map(|item| item.index.0);
        inventory.view_mut().move_cursor(GridKey::End);
        let end_index = cursor_index(&inventory);
        inventory.view_mut().move_cursor(GridKey::Home);
        let home_index = cursor_index(&inventory);
        let drawn = inventory.view().items();
        Ok(LiveGridNavReport {
            inventory_rows: inventory.item_count(),
            end_index,
            home_index,
            last_drawn: drawn.last().map(|item| item.index.0),
            first_drawn: drawn.first().map(|item| item.index.0),
        })
    }
}
