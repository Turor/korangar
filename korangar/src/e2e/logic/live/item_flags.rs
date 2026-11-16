//! Live lock and junk check on Hercules TEST.
//!
//! The marks stay in memory. This does not bind the character file, so the
//! TEST character's `client/item_flags.ron` is left alone. The refused drop
//! is not given to the socket.

use std::time::{Duration, Instant};

use korangar_networking::{InventoryItem, InventoryItemDetails, NetworkEvent, NoMetadata};

use super::super::LogicSession;
use crate::state::item_flags::{self, ItemFlags, ItemKey, StackRef, is_equipped, refine_of};

/// What the live check saw. Names may be empty when the row has no metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveItemFlagsReport {
    pub inventory_rows: usize,
    pub item_id: u32,
    pub refusal: String,
    pub sent_before: usize,
    pub sent_after: usize,
    pub cleared: bool,
}

fn amount_of(item: &InventoryItem<NoMetadata>) -> u16 {
    match &item.details {
        InventoryItemDetails::Regular { amount, .. } => *amount,
        InventoryItemDetails::Equippable { .. } => 1,
    }
}

fn sent_len(session: &LogicSession) -> usize {
    session.sent.headers.lock().map(|headers| headers.len()).unwrap_or(0)
}

impl LogicSession {
    /// Log in, mark the first inventory row junk and locked, and refuse a
    /// ground drop without writing a packet. Then clear the marks.
    pub fn item_flags_on_test(&mut self) -> Result<LiveItemFlagsReport, String> {
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
                    other => self.on_event(other)?,
                }
            }
            self.take_failure()?;
            if !inventory.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut report = LiveItemFlagsReport {
            inventory_rows: inventory.len(),
            ..LiveItemFlagsReport::default()
        };
        let Some(item) = inventory.first() else {
            return Ok(report);
        };

        let key = ItemKey::from_inventory(item);
        let mut flags = ItemFlags::default();
        flags.set_junk(key, true);
        flags.set_locked(key, true);
        let stack = StackRef {
            index: item.index.0,
            item_id: item.item_id.0,
            refine: refine_of(&item.details),
            cards: item.slot,
            name: String::new(),
            amount: amount_of(item),
            equipped: is_equipped(&item.details),
        };
        report.item_id = item.item_id.0;
        report.sent_before = sent_len(self);
        let attempt = item_flags::attempt_drop(&flags, &stack, 1);
        report.refusal = attempt.messages.into_iter().next().unwrap_or_default();
        if attempt.send.is_some() {
            return Err("locked row was allowed to drop".to_owned());
        }
        report.sent_after = sent_len(self);

        flags.set_junk(key, false);
        flags.set_locked(key, false);
        report.cleared = !flags.is_junk(key) && !flags.is_locked(key);
        Ok(report)
    }
}
