//! Live Rodex round trip on Hercules TEST: open the write form, attach and
//! detach an item, mail it with 1 zeny to the session's own character, read
//! it back, take the zeny and the item, and delete the mail.
//!
//! Events are drained here so the mail replies are not lost to the generic
//! session handler; everything else still goes through it.

use std::time::{Duration, Instant};

use korangar_networking::{InventoryItem, InventoryItemDetails, NetworkEvent, NoMetadata};
use ragnarok_packets::EquipPosition;

use super::super::LogicSession;

/// What the live round trip saw. Item fields are `None` when the character
/// had nothing the server would attach.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailRoundTrip {
    pub own_name: String,
    pub write_allowed: bool,
    /// (inventory index, item id, `0x0A05`/`0x0B3F` result) per attach try.
    pub attach_tries: Vec<(u16, u32, u8)>,
    /// (item id, position) of gear taken off so it could be mailed, when
    /// nothing in the bag was mailable. It is put back on at the end.
    pub unequipped: Option<(u32, EquipPosition)>,
    pub reequipped: bool,
    /// (index, item id) that attached with result 0.
    pub attached: Option<(u16, u32)>,
    /// `0x0A07` removed flag and weight after taking it off.
    pub detached: Option<(bool, u16)>,
    pub receiver_id: u32,
    pub receiver_name: String,
    /// `0x09ED` result of the send that went out, and whether it carried the
    /// item.
    pub send_result: Option<u8>,
    pub sent_with_item: bool,
    pub mail_id: Option<i64>,
    pub read_zeny: Option<i64>,
    pub read_items: Vec<(u32, u16)>,
    pub take_zeny_result: Option<u8>,
    pub take_items_result: Option<u8>,
    pub deleted: bool,
}

/// (mail id, zeny, item rows) of a read mail.
type ReadMail = (i64, i64, Vec<(u32, u16)>);

#[derive(Default)]
struct Seen {
    inventory: Vec<InventoryItem<NoMetadata>>,
    opened: Option<bool>,
    receiver: Option<(u32, String)>,
    attached: Option<(u8, Option<u16>, u32)>,
    detached: Option<(bool, u16)>,
    send_result: Option<u8>,
    list: Option<Vec<(i64, String)>>,
    read: Option<ReadMail>,
    zeny: Option<u8>,
    items: Option<u8>,
    deleted: Option<i64>,
    equip_change: Option<(u16, EquipPosition)>,
}

impl Seen {
    /// Keep the mail replies; hand everything else back.
    fn observe(&mut self, event: NetworkEvent) -> Option<NetworkEvent> {
        match event {
            NetworkEvent::SetInventory { items } => self.inventory = items,
            NetworkEvent::IventoryItemAdded { item } => {
                self.inventory.retain(|known| known.index != item.index);
                self.inventory.push(item);
            }
            NetworkEvent::InventoryItemRemoved { index, amount, .. } => {
                if let Some(position) = self.inventory.iter().position(|item| item.index == index) {
                    if let InventoryItemDetails::Regular { amount: held, .. } = &mut self.inventory[position].details
                        && *held > amount
                    {
                        *held -= amount;
                    } else {
                        self.inventory.remove(position);
                    }
                }
            }
            NetworkEvent::UpdateEquippedPosition { index, equipped_position } => {
                if let Some(item) = self.inventory.iter_mut().find(|item| item.index == index) {
                    match &mut item.details {
                        InventoryItemDetails::Regular { equipped_position: at, .. }
                        | InventoryItemDetails::Equippable { equipped_position: at, .. } => *at = equipped_position,
                    }
                }
                self.equip_change = Some((index.0, equipped_position));
            }
            NetworkEvent::RodexWriteOpened { allowed, .. } => self.opened = Some(allowed),
            NetworkEvent::RodexReceiver { character_id, name, .. } => self.receiver = Some((character_id, name)),
            NetworkEvent::RodexItemAttached {
                result, index, item_id, ..
            } => self.attached = Some((result, index, item_id)),
            NetworkEvent::RodexItemDetached { removed, weight, .. } => self.detached = Some((removed, weight)),
            NetworkEvent::RodexWriteResult { result, .. } => self.send_result = Some(result),
            NetworkEvent::RodexMailList { mails, .. } => {
                self.list = Some(mails.into_iter().map(|mail| (mail.mail_id, mail.title)).collect());
            }
            NetworkEvent::RodexRead {
                mail_id,
                zeny,
                attachments,
                ..
            } => {
                let items = attachments.iter().map(|row| (row.item_id, row.amount)).collect();
                self.read = Some((mail_id, zeny, items));
            }
            NetworkEvent::RodexZeny { result, .. } => self.zeny = Some(result),
            NetworkEvent::RodexItems { result, .. } => self.items = Some(result),
            NetworkEvent::RodexDelete { mail_id, .. } => self.deleted = Some(mail_id),
            other => return Some(other),
        }
        None
    }
}

/// Unequipped stacks, ordinary items first. Kafra tickets are `nomail` on
/// TEST, so they go last.
fn attach_order(inventory: &[InventoryItem<NoMetadata>]) -> Vec<(u16, u32)> {
    let mut candidates: Vec<(u8, u16, u32)> = inventory
        .iter()
        .filter_map(|item| {
            let equipped = match &item.details {
                InventoryItemDetails::Regular {
                    amount, equipped_position, ..
                } => *amount == 0 || *equipped_position != EquipPosition::NONE,
                InventoryItemDetails::Equippable { equipped_position, .. } => *equipped_position != EquipPosition::NONE,
            };
            let rank = if (7060..=7061).contains(&item.item_id.0) { 1 } else { 0 };
            (!equipped).then_some((rank, item.index.0, item.item_id.0))
        })
        .collect();
    candidates.sort();
    candidates.into_iter().map(|(_, index, item_id)| (index, item_id)).collect()
}

const STEP: Duration = Duration::from_secs(6);

impl LogicSession {
    /// Log in, enter the map, and run the Rodex round trip. Only TEST.
    pub fn mail_round_trip(&mut self) -> Result<MailRoundTrip, String> {
        self.login_to_character_select()?;
        self.enter_world()?;
        let mut seen = Seen::default();
        let mut trip = MailRoundTrip {
            own_name: self.characters.first().cloned().unwrap_or_default(),
            ..MailRoundTrip::default()
        };
        if trip.own_name.is_empty() {
            return Err("the session has no character name".to_owned());
        }
        // Inventory arrives after the map-loaded ack.
        self.pump_mail(&mut seen, Duration::from_secs(4), None, |_| false)?;

        let result = self.mail_steps(&mut seen, &mut trip);
        if let Some(net) = self.net.as_mut() {
            let _ = net.cancel_rodex_write();
            let _ = net.close_rodex();
        }
        let restored = self.put_gear_back(&mut seen, &mut trip);
        result.and(restored).map(|()| trip)
    }

    fn mail_steps(&mut self, seen: &mut Seen, trip: &mut MailRoundTrip) -> Result<(), String> {
        self.open_write(seen)?;
        trip.write_allowed = true;

        for (index, item_id) in attach_order(&seen.inventory).into_iter().take(6) {
            seen.attached = None;
            self.mail_net()?.add_rodex_item(index, 1).map_err(not_connected)?;
            self.pump_mail(seen, STEP, Some("the add-item ack"), |seen| seen.attached.is_some())?;
            let (result, acked, _) = seen.attached.ok_or("no add ack")?;
            trip.attach_tries.push((index, item_id, result));
            if result == 0 {
                if acked != Some(index) {
                    return Err(format!("add ack names index {acked:?}, sent {index}"));
                }
                trip.attached = Some((index, item_id));
                break;
            }
        }

        if trip.attached.is_none() {
            self.attach_worn_gear(seen, trip)?;
        }

        if let Some((index, _)) = trip.attached {
            seen.detached = None;
            self.mail_net()?.remove_rodex_item(index, 1).map_err(not_connected)?;
            self.pump_mail(seen, STEP, Some("the remove-item ack"), |seen| seen.detached.is_some())?;
            trip.detached = seen.detached;
            // Attach it again for the send.
            seen.attached = None;
            self.mail_net()?.add_rodex_item(index, 1).map_err(not_connected)?;
            self.pump_mail(seen, STEP, Some("the add-item ack"), |seen| seen.attached.is_some())?;
            if seen.attached.map(|(result, ..)| result) != Some(0) {
                return Err("the item did not attach a second time".to_owned());
            }
        }

        let title = format!("e2e mail {}", std::process::id());
        let mut result = self.check_and_send(seen, trip, &title)?;
        trip.sent_with_item = trip.attached.is_some();
        if result != 0 && trip.attached.is_some() {
            // Not enough zeny for the 2500 fee: Hercules cleaned the form, send
            // the zeny alone.
            self.open_write(seen)?;
            result = self.check_and_send(seen, trip, &title)?;
            trip.sent_with_item = false;
        }
        trip.send_result = Some(result);
        if result != 0 {
            return Ok(());
        }

        std::thread::sleep(Duration::from_millis(800));
        seen.list = None;
        self.mail_net()?.open_rodex(0).map_err(not_connected)?;
        self.pump_mail(seen, STEP, Some("the inbox page"), |seen| {
            seen.list
                .as_ref()
                .is_some_and(|mails| mails.iter().any(|(_, mail_title)| *mail_title == title))
        })?;
        let mail_id = seen
            .list
            .as_ref()
            .and_then(|mails| mails.iter().find(|(_, mail_title)| *mail_title == title))
            .map(|(mail_id, _)| *mail_id)
            .ok_or("the sent mail is not in the inbox")?;
        trip.mail_id = Some(mail_id);

        self.mail_net()?.read_rodex(0, mail_id).map_err(not_connected)?;
        self.pump_mail(seen, STEP, Some("the read mail"), |seen| {
            seen.read.as_ref().is_some_and(|(id, ..)| *id == mail_id)
        })?;
        if let Some((_, zeny, items)) = seen.read.clone() {
            trip.read_zeny = Some(zeny);
            trip.read_items = items;
        }

        self.mail_net()?.take_rodex_zeny(0, mail_id).map_err(not_connected)?;
        self.pump_mail(seen, STEP, Some("the take-zeny ack"), |seen| seen.zeny.is_some())?;
        trip.take_zeny_result = seen.zeny;
        if !trip.read_items.is_empty() {
            self.mail_net()?.take_rodex_items(0, mail_id).map_err(not_connected)?;
            self.pump_mail(seen, STEP, Some("the take-items ack"), |seen| seen.items.is_some())?;
            trip.take_items_result = seen.items;
        }
        self.mail_net()?.delete_rodex(0, mail_id).map_err(not_connected)?;
        self.pump_mail(seen, STEP, Some("the delete ack"), |seen| seen.deleted == Some(mail_id))?;
        trip.deleted = true;
        Ok(())
    }

    /// Take off the lowest-slot worn piece and attach it (new TEST characters
    /// carry only worn gear and `nomail` tickets).
    fn attach_worn_gear(&mut self, seen: &mut Seen, trip: &mut MailRoundTrip) -> Result<(), String> {
        let worn = seen
            .inventory
            .iter()
            .filter_map(|item| match &item.details {
                InventoryItemDetails::Equippable { equipped_position, .. } if *equipped_position != EquipPosition::NONE => {
                    Some((equipped_position.bits(), item.index.0, item.item_id.0, *equipped_position))
                }
                _ => None,
            })
            .min_by_key(|(bits, index, ..)| (*bits, *index));
        let Some((_, index, item_id, position)) = worn else {
            return Ok(());
        };
        seen.equip_change = None;
        self.mail_net()?
            .request_item_unequip(ragnarok_packets::InventoryIndex(index))
            .map_err(not_connected)?;
        self.pump_mail(seen, STEP, Some("the unequip ack"), |seen| seen.equip_change.is_some())?;
        trip.unequipped = Some((item_id, position));
        seen.attached = None;
        self.mail_net()?.add_rodex_item(index, 1).map_err(not_connected)?;
        self.pump_mail(seen, STEP, Some("the add-item ack"), |seen| seen.attached.is_some())?;
        let (result, acked, _) = seen.attached.ok_or("no add ack")?;
        trip.attach_tries.push((index, item_id, result));
        if result == 0 {
            if acked != Some(index) {
                return Err(format!("add ack names index {acked:?}, sent {index}"));
            }
            trip.attached = Some((index, item_id));
        }
        Ok(())
    }

    /// Re-equip what [`Self::attach_worn_gear`] took off, wherever it is now.
    fn put_gear_back(&mut self, seen: &mut Seen, trip: &mut MailRoundTrip) -> Result<(), String> {
        let Some((item_id, position)) = trip.unequipped else {
            return Ok(());
        };
        let Some(index) = seen.inventory.iter().find(|item| item.item_id.0 == item_id).map(|item| item.index) else {
            return Err(format!("item {item_id} did not come back to the bag"));
        };
        seen.equip_change = None;
        self.mail_net()?.request_item_equip(index, position).map_err(not_connected)?;
        self.pump_mail(seen, STEP, Some("the equip ack"), |seen| {
            seen.equip_change
                .is_some_and(|(changed, at)| changed == index.0 && at != EquipPosition::NONE)
        })?;
        trip.reequipped = true;
        Ok(())
    }

    fn open_write(&mut self, seen: &mut Seen) -> Result<(), String> {
        seen.opened = None;
        // As the client does: cancel cleans the server's attachment slots.
        self.mail_net()?.cancel_rodex_write().map_err(not_connected)?;
        self.mail_net()?.open_rodex_write("").map_err(not_connected)?;
        self.pump_mail(seen, STEP, Some("the write-form ack"), |seen| seen.opened.is_some())?;
        match seen.opened {
            Some(true) => Ok(()),
            _ => Err("TEST refused the write form".to_owned()),
        }
    }

    fn check_and_send(&mut self, seen: &mut Seen, trip: &mut MailRoundTrip, title: &str) -> Result<u8, String> {
        seen.receiver = None;
        let own = trip.own_name.clone();
        self.mail_net()?.check_rodex_receiver(&own).map_err(not_connected)?;
        self.pump_mail(seen, STEP, Some("the receiver check"), |seen| seen.receiver.is_some())?;
        let (receiver_id, receiver_name) = seen.receiver.clone().ok_or("no receiver reply")?;
        trip.receiver_id = receiver_id;
        trip.receiver_name = receiver_name.trim_end_matches('\0').to_owned();
        if receiver_id == 0 {
            return Err(format!("TEST does not know {own}"));
        }
        seen.send_result = None;
        let receiver = trip.receiver_name.clone();
        self.mail_net()?
            .send_rodex_mail(&receiver, &own, receiver_id, 1, title, "Sent by the Korangar e2e harness.")
            .map_err(not_connected)?;
        self.pump_mail(seen, Duration::from_secs(10), Some("the send result"), |seen| {
            seen.send_result.is_some()
        })?;
        seen.send_result.ok_or_else(|| "no send result".to_owned())
    }

    fn mail_net(&mut self) -> Result<&mut korangar_networking::NetworkingSystem<super::SentHeaders>, String> {
        self.net.as_mut().ok_or_else(|| "logic mode has no network".to_owned())
    }

    /// Pump until `done`, keeping the mail replies in `seen`. `step: None`
    /// just pumps for `budget`.
    fn pump_mail(&mut self, seen: &mut Seen, budget: Duration, step: Option<&str>, done: impl Fn(&Seen) -> bool) -> Result<(), String> {
        let deadline = Instant::now() + budget;
        while Instant::now() < deadline {
            let drained = {
                let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) else {
                    return Err("logic mode has no network".to_owned());
                };
                net.get_events(events);
                events.drain().collect::<Vec<_>>()
            };
            for event in drained {
                if let Some(other) = seen.observe(event) {
                    self.on_event(other)?;
                }
            }
            self.take_failure()?;
            if step.is_some() && done(seen) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        match step {
            Some(step) => Err(self.timeout(step)),
            None => Ok(()),
        }
    }
}

fn not_connected<E>(_: E) -> String {
    "map server is not connected".to_owned()
}
