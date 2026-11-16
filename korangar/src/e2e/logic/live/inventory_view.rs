//! Live view check on Hercules TEST: log in, enter the map, split the real
//! inventory into tabs, then favorite one stack and put it back. The only
//! server change is the favorite flag, and it is reverted before returning.

use std::time::{Duration, Instant};

use korangar_networking::NetworkEvent;
use ragnarok_packets::InventoryIndex;

use super::super::LogicSession;
use crate::e2e::inventory_view::{InventorySort, InventoryTab, drawn, with_blank_metadata};
use crate::state::inventory::Inventory;
use crate::state::inventory_view::{is_favorite, is_worn};

/// What the live check saw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveInventoryViewReport {
    pub inventory_rows: usize,
    pub worn: usize,
    /// Stacks drawn per tab, in [`InventoryTab::ALL`] order.
    pub tab_counts: [usize; 5],
    /// The stack that was toggled and the acks for set and revert.
    pub toggled: Option<InventoryIndex>,
    pub set_ack: Option<bool>,
    pub revert_ack: Option<bool>,
}

impl LogicSession {
    fn pump_inventory_events(&mut self, until: Duration, mut stop: impl FnMut(&NetworkEvent) -> bool) -> Result<Vec<NetworkEvent>, String> {
        let deadline = Instant::now() + until;
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            let drained = {
                let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) else {
                    return Err("logic mode has no network".to_owned());
                };
                net.get_events(events);
                events.drain().collect::<Vec<_>>()
            };
            let mut done = false;
            for event in drained {
                done |= stop(&event);
                match event {
                    event @ (NetworkEvent::SetInventory { .. } | NetworkEvent::ItemFavorite { .. }) => seen.push(event),
                    other => self.on_event(other)?,
                }
            }
            self.take_failure()?;
            if done {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(seen)
    }

    fn favorite_round(&mut self, index: InventoryIndex, favorite: bool) -> Result<Option<bool>, String> {
        let Some(net) = self.net.as_mut() else {
            return Err("logic mode has no network".to_owned());
        };
        net.set_item_favorite(index, favorite).map_err(|_| "not connected".to_owned())?;
        let seen = self.pump_inventory_events(
            Duration::from_secs(3),
            |event| matches!(event, NetworkEvent::ItemFavorite { index: ack, .. } if *ack == index),
        )?;
        Ok(seen.into_iter().find_map(|event| match event {
            NetworkEvent::ItemFavorite { index: ack, favorite } if ack == index => Some(favorite),
            _ => None,
        }))
    }

    /// Only TEST. Favorites one unworn stack, then restores its flag.
    pub fn inventory_view_on_test(&mut self) -> Result<LiveInventoryViewReport, String> {
        self.login_to_character_select()?;
        self.enter_world()?;

        let mut items = Vec::new();
        for event in self.pump_inventory_events(Duration::from_secs(5), |_| false)? {
            if let NetworkEvent::SetInventory { items: list } = event {
                items = with_blank_metadata(list);
            }
        }

        let worn = items.iter().filter(|item| is_worn(item)).count();
        let target = items.iter().find(|item| !is_worn(item)).map(|item| (item.index, is_favorite(item)));
        let mut inventory = Inventory::from_items(items.clone());
        let tab_counts = InventoryTab::ALL.map(|tab| drawn(&mut inventory, tab, InventorySort::Server, 0).len());

        let (mut set_ack, mut revert_ack) = (None, None);
        if let Some((index, was_favorite)) = target {
            set_ack = self.favorite_round(index, !was_favorite)?;
            revert_ack = self.favorite_round(index, was_favorite)?;
        }

        Ok(LiveInventoryViewReport {
            inventory_rows: items.len(),
            worn,
            tab_counts,
            toggled: target.map(|(index, _)| index),
            set_ack,
            revert_ack,
        })
    }
}
