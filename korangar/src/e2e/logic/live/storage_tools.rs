//! Live deposit plan on Hercules TEST: log in, enter the map, and plan
//! "Deposit matching" for the All tab on the real inventory. Nothing is
//! moved; TEST has no storage open for the session.

use std::time::{Duration, Instant};

use korangar_networking::NetworkEvent;

use super::super::LogicSession;
use crate::e2e::inventory_view::with_blank_metadata;
use crate::state::inventory_view::InventoryTab;
use crate::state::storage_tools::{deposit_plan, is_protected};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveStoragePlanReport {
    pub inventory_rows: usize,
    /// Worn, equipped-ammo or favorite stacks.
    pub protected: usize,
    pub planned: usize,
}

impl LogicSession {
    /// Only TEST. Plans, does not send.
    pub fn storage_deposit_plan_on_test(&mut self) -> Result<LiveStoragePlanReport, String> {
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

        let plan = deposit_plan(&items, InventoryTab::All, "", |_| None);
        Ok(LiveStoragePlanReport {
            inventory_rows: items.len(),
            protected: items.iter().filter(|item| is_protected(item)).count(),
            planned: plan.len(),
        })
    }
}
