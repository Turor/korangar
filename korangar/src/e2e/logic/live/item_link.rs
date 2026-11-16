//! Live item-link check on Hercules TEST: log in, encode one real inventory
//! row, send it on public chat, and parse the echo. Nothing is moved.

use std::time::{Duration, Instant};

use korangar_networking::{InventoryItem, InventoryItemDetails, NetworkEvent, NoMetadata};

use super::super::LogicSession;
use crate::e2e::actions::send_public_chat;
use crate::state::item_link::{self, chat_segments};

fn refine_of(item: &InventoryItem<NoMetadata>) -> u32 {
    match &item.details {
        InventoryItemDetails::Equippable { refinement_level, .. } => u32::from(*refinement_level),
        InventoryItemDetails::Regular { .. } => 0,
    }
}

impl LogicSession {
    /// Log in, link one inventory row into public chat, and check the echo.
    /// Only TEST. An empty inventory after a real login is an error.
    pub fn item_link_on_test(&mut self) -> Result<(), String> {
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
        if inventory.is_empty() {
            return Err("logged in on TEST but the inventory list stayed empty".to_owned());
        }

        let item = inventory.iter().find(|item| refine_of(item) > 0).unwrap_or(&inventory[0]).clone();
        let expected_id = item.item_id.0;
        let expected_refine = refine_of(&item);
        let payload = item_link::encode_item_link(&item);
        let name = self
            .character_names()
            .first()
            .cloned()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "no character name after entering the world".to_owned())?;
        {
            let Some(net) = self.net.as_mut() else {
                return Err("logic mode has no network".to_owned());
            };
            send_public_chat(net, &name, &payload).map_err(|_| "could not send public chat".to_owned())?;
        }
        if !self.sent_headers().contains(&0x00F3) {
            return Err("public chat packet 0x00F3 was not sent".to_owned());
        }

        let echo_deadline = Instant::now() + Duration::from_secs(8);
        let mut echoed = false;
        let mut saw_id = None;
        let mut saw_refine = None;
        while Instant::now() < echo_deadline && !echoed {
            let drained = {
                let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) else {
                    return Err("logic mode has no network".to_owned());
                };
                net.get_events(events);
                events.drain().collect::<Vec<_>>()
            };
            for event in drained {
                let text = match &event {
                    NetworkEvent::ChatMessage { text, .. }
                    | NetworkEvent::EntitySpeech { text, .. }
                    | NetworkEvent::BroadcastMessage { text, .. }
                    | NetworkEvent::WhisperReceived { text, .. } => Some(text.clone()),
                    NetworkEvent::GuildChatMessage { text, .. } | NetworkEvent::ClanChatMessage { text, .. } => Some(text.clone()),
                    _ => None,
                };
                self.on_event(event)?;
                if let Some(text) = text
                    && text.contains(&payload)
                {
                    let parsed = chat_segments(&text, |_| None).into_iter().find_map(|segment| segment.link);
                    if let Some(parsed) = parsed {
                        saw_id = Some(parsed.item_id);
                        saw_refine = Some(parsed.refine);
                        echoed = true;
                    }
                }
            }
            self.take_failure()?;
            if !echoed {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        if !echoed {
            return Err("TEST chat echo did not include the item link".to_owned());
        }
        if saw_id != Some(expected_id) || saw_refine != Some(expected_refine) {
            return Err(format!(
                "echo item {:?} +{:?} did not match sent {expected_id} +{expected_refine}",
                saw_id, saw_refine
            ));
        }
        Ok(())
    }
}
