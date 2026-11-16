//! Live item-menu check on Hercules TEST: log in, enter the map, and build
//! the menu for a real inventory row. Nothing is moved or used.

use std::time::{Duration, Instant};

use korangar_networking::{InventoryItem, NetworkEvent, NoMetadata};

use super::super::LogicSession;
use crate::state::inventory::item_activation;
use crate::state::inventory_view::{is_favorite, is_worn};
use crate::state::item_menu::{ItemMenuFacts, MenuOrigin, build_item_menu, pending_link_event};
use crate::state::quick_move::{QuickMoveDecision, QuickMoveSource, QuickMoveWindows, decide_quick_move, item_is_locked};

/// What the live check saw. No account secrets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveItemMenuReport {
    pub inventory_rows: usize,
    pub labels: Vec<String>,
    pub has_details: bool,
    pub quick_move_hint: String,
    pub sent_before: usize,
    pub sent_after: usize,
}

impl LogicSession {
    /// Log in and build the menu for one real inventory row. Only TEST.
    pub fn item_menu_on_test(&mut self) -> Result<LiveItemMenuReport, String> {
        self.login_to_character_select()?;
        self.enter_world()?;

        let mut inventory: Vec<InventoryItem<NoMetadata>> = Vec::new();
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
                    NetworkEvent::SetInventory { items } => inventory = items,
                    NetworkEvent::IventoryItemAdded { item } => {
                        if let Some(existing) = inventory.iter_mut().find(|row| row.index == item.index) {
                            *existing = item;
                        } else {
                            inventory.push(item);
                        }
                    }
                    other => self.on_event(other)?,
                }
            }
            self.take_failure()?;
            if !inventory.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        if inventory.is_empty() {
            return Err("the TEST character's inventory never arrived".to_owned());
        }

        let item = &inventory[0];
        let facts = ItemMenuFacts {
            origin: MenuOrigin::Inventory,
            activation: item_activation(item),
            worn: is_worn(item),
            favorite: is_favorite(item),
            locked: item_is_locked(item),
            windows: QuickMoveWindows::default(),
            link_to_chat: pending_link_event(item.index.0).is_some(),
        };
        let rows = build_item_menu(&facts);
        let labels = rows.into_iter().map(|row| row.label).collect::<Vec<_>>();
        let has_details = labels.iter().any(|label| label == "Details");

        let quick_item = inventory.iter().find(|item| !is_worn(item)).unwrap_or(item);
        let sent_before = self.sent_header_count();
        let decision = decide_quick_move(
            QuickMoveSource::Inventory,
            QuickMoveWindows::default(),
            item_is_locked(quick_item),
            is_worn(quick_item),
        );
        let sent_after = self.sent_header_count();
        let quick_move_hint = match decision {
            QuickMoveDecision::Hint(text) => text.to_owned(),
            other => return Err(format!("quick-move with nothing open was {other:?}")),
        };

        Ok(LiveItemMenuReport {
            inventory_rows: inventory.len(),
            labels,
            has_details,
            quick_move_hint,
            sent_before,
            sent_after,
        })
    }
}
