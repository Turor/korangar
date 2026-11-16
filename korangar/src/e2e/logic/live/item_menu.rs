//! Live item-menu check on Hercules TEST: log in, enter the map, right-click
//! a real inventory row, then Alt+right-click with no transfer window.
//!
//! The Alt+click runs [`react_to_secondary`]. A move reaction is offered
//! through the client's amount prompt and, when that sends immediately, is
//! written with the same networking calls the client uses. A hint sends
//! nothing, so the outbound count stays put.

use std::time::{Duration, Instant};

use korangar_networking::{InventoryItem, NetworkEvent, NoMetadata};
use ragnarok_packets::InventoryIndex;

use super::super::LogicSession;
use crate::interface::item_menu_flow::{SlotReaction, react_to_secondary};
use crate::interface::item_move_flow::{offer_amount, stack_amount};
use crate::interface::resource::ItemSource;
use crate::state::item_move::{ItemMovePrompt, ItemMoveStep, ItemMoveTarget};
use crate::state::quick_move::{QUICK_MOVE_NO_TARGET, QuickMoveWindows, SlotSecondary, classify_secondary};

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
    /// Log in, right-click one real inventory row, and Alt+right-click it with
    /// no storage, cart, trade, or mail window. Only TEST.
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

        let item = inventory[0].clone();
        let (right, alt_down) = self.sample_right_click(false);
        if classify_secondary(right, alt_down) != SlotSecondary::OpenMenu {
            return Err("right-click did not open the menu".to_owned());
        }
        let reaction = react_to_secondary(ItemSource::Inventory, &item, "Item", false, QuickMoveWindows::default(), false);
        let SlotReaction::OpenMenu { rows, .. } = reaction else {
            return Err("right-click did not build a menu".to_owned());
        };
        let labels = rows.into_iter().map(|row| row.label).collect::<Vec<_>>();
        let has_details = labels.iter().any(|label| label == "Details");

        let quick_item = inventory
            .iter()
            .find(|item| !crate::state::inventory_view::is_worn(item))
            .unwrap_or(&item);
        let (right, alt_down) = self.sample_right_click(true);
        if classify_secondary(right, alt_down) != SlotSecondary::QuickMove {
            return Err("Alt+right-click did not quick-move".to_owned());
        }
        let sent_before = self.sent_header_count();
        let reaction = react_to_secondary(
            ItemSource::Inventory,
            quick_item,
            "Item",
            true,
            QuickMoveWindows::default(),
            false,
        );
        let quick_move_hint = match reaction {
            SlotReaction::Hint(text) => text.to_owned(),
            SlotReaction::Move(target) => {
                self.send_quick_move(target, quick_item)?;
                format!("sent {target:?}")
            }
            SlotReaction::Mail { .. } => "sent mail".to_owned(),
            SlotReaction::Ignored => "ignored".to_owned(),
            SlotReaction::OpenMenu { .. } => "opened a menu".to_owned(),
        };
        let sent_after = self.sent_header_count();
        if quick_move_hint != QUICK_MOVE_NO_TARGET {
            return Err(format!("quick-move with nothing open was {quick_move_hint}"));
        }

        Ok(LiveItemMenuReport {
            inventory_rows: inventory.len(),
            labels,
            has_details,
            quick_move_hint,
            sent_before,
            sent_after,
        })
    }

    /// Immediate send for a quick-move that did not ask for an amount.
    fn send_quick_move<Meta>(&mut self, target: ItemMoveTarget, item: &InventoryItem<Meta>) -> Result<(), String> {
        let mut prompt = ItemMovePrompt::default();
        let step = offer_amount(&mut prompt, target, item.index.0, stack_amount(item), "Item");
        let ItemMoveStep::Send { target, index, amount } = step else {
            return Ok(());
        };
        let net = self.net.as_mut().ok_or("logic mode has no network")?;
        let item_index = InventoryIndex(index);
        let sent = match target {
            ItemMoveTarget::ToStorage => net.move_item_to_storage(item_index, u32::from(amount)),
            ItemMoveTarget::FromStorage => net.move_item_from_storage(item_index, u32::from(amount)),
            ItemMoveTarget::ToCart => net.move_item_to_cart(item_index, i32::from(amount)),
            ItemMoveTarget::FromCart => net.move_item_from_cart(item_index, i32::from(amount)),
            ItemMoveTarget::ToTrade => net.add_trade_item(Some(item_index), u32::from(amount)),
        };
        sent.map_err(|_| "map server is not connected".to_owned())
    }
}
