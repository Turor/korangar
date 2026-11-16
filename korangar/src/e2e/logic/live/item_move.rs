//! Live amount-prompt check on Hercules TEST: log in, enter the map, and run
//! the storage / cart / trade prompt on the character's real inventory rows.
//! Nothing is moved: TEST has no storage, cart or trade partner open for the
//! session, so the moves themselves are covered offline.

use std::time::{Duration, Instant};

use korangar_networking::{InventoryItem, InventoryItemDetails, NetworkEvent, NoMetadata};

use super::super::LogicSession;
use crate::state::item_move::{ItemMovePrompt, ItemMoveStep, ItemMoveTarget};

/// What the live check saw.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveItemMoveReport {
    pub inventory_rows: usize,
    /// (index, item id, amount) of the largest stack, when there is one.
    pub stack: Option<(u16, u32, u16)>,
    /// Prompt text per target for that stack.
    pub stack_prompts: Vec<String>,
    /// Default amount text for that stack.
    pub stack_default: String,
    /// The stack's own amount confirms; one more is refused.
    pub stack_full_confirms: bool,
    pub stack_over_refused: bool,
    /// (index, step) of a single item or equipment, when there is one.
    pub single: Option<(u16, ItemMoveStep)>,
}

fn amount_of(item: &InventoryItem<NoMetadata>) -> u16 {
    match &item.details {
        InventoryItemDetails::Regular { amount, .. } => *amount,
        InventoryItemDetails::Equippable { .. } => 1,
    }
}

impl LogicSession {
    /// Log in, enter the map, and run the prompt on real inventory rows. Only
    /// TEST.
    pub fn item_move_prompts_on_test(&mut self) -> Result<LiveItemMoveReport, String> {
        self.login_to_character_select()?;
        self.enter_world()?;

        // Inventory arrives after the map-loaded ack.
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
                    other => self.on_event(other)?,
                }
            }
            self.take_failure()?;
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut report = LiveItemMoveReport {
            inventory_rows: inventory.len(),
            ..LiveItemMoveReport::default()
        };

        if let Some(item) = inventory
            .iter()
            .filter(|item| amount_of(item) > 1)
            .max_by_key(|item| amount_of(item))
        {
            let amount = amount_of(item);
            let name = format!("item {}", item.item_id.0);
            report.stack = Some((item.index.0, item.item_id.0, amount));
            for target in [ItemMoveTarget::ToStorage, ItemMoveTarget::ToCart, ItemMoveTarget::ToTrade] {
                let mut prompt = ItemMovePrompt::default();
                if prompt.offer(target, item.index.0, amount, &name) != ItemMoveStep::AskAmount {
                    return Err(format!("{target:?}: a stack of {amount} did not ask"));
                }
                report.stack_default.clone_from(&prompt.amount_text);
                report.stack_prompts.push(prompt.prompt.clone());
                prompt.amount_text = (u32::from(amount) + 1).to_string();
                report.stack_over_refused = prompt.confirm().is_err();
                prompt.amount_text = amount.to_string();
                report.stack_full_confirms = prompt.confirm() == Ok((target, item.index.0, amount));
            }
        }

        if let Some(item) = inventory.iter().find(|item| amount_of(item) == 1) {
            let mut prompt = ItemMovePrompt::default();
            let step = prompt.offer(ItemMoveTarget::ToStorage, item.index.0, 1, "single");
            report.single = Some((item.index.0, step));
        }
        Ok(report)
    }
}
