//! Live server-kept lock on Hercules TEST.
//!
//! The first session locks one bag row with `0x0EE1` and logs out. A second
//! session logs in again, reads the lock back from `0x0EE8`, sends a real
//! ground drop that the map server has to refuse, and then clears the lock so
//! the TEST character is left as it was.

use std::time::{Duration, Instant};

use korangar_networking::{InventoryItem, NetworkEvent, NoMetadata};
use ragnarok_packets::{ITEM_MARK_INVENTORY, ITEM_MARK_LOCK, InventoryIndex, TURORAN_FEATURE_ITEM_MARKS};

use super::super::LogicSession;
use crate::state::item_flags::is_equipped;

/// The row the first session locked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveLockedRow {
    pub index: u16,
    pub item_id: u32,
    pub result: u8,
    pub marks: u8,
}

/// What the second session saw.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveServerMarksReport {
    pub listed_locked: bool,
    pub row_present: bool,
    pub refusal: String,
    pub removed_after_drop: bool,
    pub cleared: bool,
}

/// The map server line for a refused locked row. Server text may carry its
/// terminating NUL.
fn is_lock_refusal(line: &str) -> bool {
    line.trim_end_matches(char::from(0)).ends_with("is locked.")
}

#[derive(Default)]
struct MarkWatch {
    features: Option<u32>,
    inventory: Vec<InventoryItem<NoMetadata>>,
    list: Option<Vec<korangar_networking::ItemMark>>,
    results: Vec<(u8, u16, u8, u8)>,
    chat: Vec<String>,
    removed: Vec<u16>,
}

impl LogicSession {
    fn pump_marks(&mut self, watch: &mut MarkWatch, budget: Duration, mut done: impl FnMut(&MarkWatch) -> bool) -> Result<bool, String> {
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
                match event {
                    NetworkEvent::SetInventory { items } => watch.inventory = items,
                    NetworkEvent::TuroranFeatures { features } => watch.features = Some(features),
                    NetworkEvent::ItemMarksList { entries } => watch.list = Some(entries),
                    NetworkEvent::ItemMarkResult {
                        location,
                        index,
                        marks,
                        result,
                    } => watch.results.push((location, index, marks, result)),
                    NetworkEvent::ChatMessage { text, color } => {
                        watch.chat.push(text.clone());
                        self.on_event(NetworkEvent::ChatMessage { text, color })?;
                    }
                    NetworkEvent::InventoryItemRemoved { index, amount, reason } => {
                        watch.removed.push(index.0);
                        self.on_event(NetworkEvent::InventoryItemRemoved { index, amount, reason })?;
                    }
                    other => self.on_event(other)?,
                }
            }
            self.take_failure()?;
            if done(watch) {
                return Ok(true);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(false)
    }

    /// Log in, wait for the features notice and the bag, say hello, and wait
    /// for the mark list.
    fn enter_with_marks(&mut self) -> Result<MarkWatch, String> {
        self.login_to_character_select()?;
        self.enter_world()?;
        let mut watch = MarkWatch::default();
        self.pump_marks(&mut watch, Duration::from_secs(8), |watch| {
            watch.features.is_some() && !watch.inventory.is_empty()
        })?;
        let features = watch.features.ok_or("no 0x0EF1 features notice from the map server")?;
        if features & TURORAN_FEATURE_ITEM_MARKS == 0 {
            return Err(format!("features {features:#x} do not include item marks"));
        }
        if watch.inventory.is_empty() {
            return Err("inventory never arrived".to_owned());
        }
        self.net
            .as_mut()
            .ok_or("logic mode has no network")?
            .send_item_marks_hello()
            .map_err(|_| "map server is not connected".to_owned())?;
        if !self.pump_marks(&mut watch, Duration::from_secs(5), |watch| watch.list.is_some())? {
            return Err("no 0x0EE8 list after hello".to_owned());
        }
        Ok(watch)
    }

    fn set_lock(&mut self, watch: &mut MarkWatch, index: u16, locked: bool) -> Result<(u8, u8), String> {
        watch.results.clear();
        self.net
            .as_mut()
            .ok_or("logic mode has no network")?
            .set_item_mark(
                ITEM_MARK_INVENTORY,
                index,
                ITEM_MARK_LOCK,
                if locked { ITEM_MARK_LOCK } else { 0 },
            )
            .map_err(|_| "map server is not connected".to_owned())?;
        if !self.pump_marks(watch, Duration::from_secs(5), |watch| {
            watch.results.iter().any(|result| result.1 == index)
        })? {
            return Err("no 0x0EE9 result".to_owned());
        }
        let (_, _, marks, result) = *watch.results.iter().find(|result| result.1 == index).expect("checked above");
        Ok((marks, result))
    }

    /// First session: lock the first bag row that is not worn.
    pub fn server_marks_lock_on_test(&mut self) -> Result<LiveLockedRow, String> {
        let mut watch = self.enter_with_marks()?;
        let item = watch
            .inventory
            .iter()
            .find(|item| !is_equipped(&item.details))
            .cloned()
            .ok_or("no bag row that is not worn")?;
        let (marks, result) = self.set_lock(&mut watch, item.index.0, true)?;
        Ok(LiveLockedRow {
            index: item.index.0,
            item_id: item.item_id.0,
            result,
            marks,
        })
    }

    /// Second session: the lock came back from the server, a real drop is
    /// refused there, and the lock is cleared again.
    pub fn server_marks_check_after_relog(&mut self, row: &LiveLockedRow) -> Result<LiveServerMarksReport, String> {
        let mut watch = self.enter_with_marks()?;
        let mut report = LiveServerMarksReport {
            listed_locked: watch.list.as_ref().is_some_and(|list| {
                list.iter()
                    .any(|entry| entry.location == ITEM_MARK_INVENTORY && entry.index.0 == row.index && entry.marks & ITEM_MARK_LOCK != 0)
            }),
            row_present: watch
                .inventory
                .iter()
                .any(|item| item.index.0 == row.index && item.item_id.0 == row.item_id),
            ..LiveServerMarksReport::default()
        };
        if !(report.row_present && report.listed_locked) {
            // Never drop an item whose lock is not confirmed. Clear and fail.
            let _ = self.set_lock(&mut watch, row.index, false);
            return Err(format!(
                "lock not confirmed after relog (present={}, listed={}); no drop sent",
                report.row_present, report.listed_locked
            ));
        }
        watch.chat.clear();
        watch.removed.clear();
        self.net
            .as_mut()
            .ok_or("logic mode has no network")?
            .drop_item(InventoryIndex(row.index), 1)
            .map_err(|_| "map server is not connected".to_owned())?;
        self.pump_marks(&mut watch, Duration::from_secs(3), |watch| {
            watch.chat.iter().any(|line| is_lock_refusal(line))
        })?;
        report.refusal = watch
            .chat
            .iter()
            .find(|line| is_lock_refusal(line))
            .map(|line| line.trim_end_matches(char::from(0)).to_owned())
            .unwrap_or_default();

        report.removed_after_drop = watch.removed.contains(&row.index);
        let (marks, result) = self.set_lock(&mut watch, row.index, false)?;
        report.cleared = result == 0 && marks & ITEM_MARK_LOCK == 0;
        Ok(report)
    }
}

/// Offline: the client side of server marks with no socket.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OfflineServerMarksReport {
    pub file_mark_ignored: bool,
    pub listed_locked: bool,
    pub drop_refusal: String,
    pub toggle_bytes: Vec<u8>,
    pub result_applied: bool,
    pub fresh_row_cleared: bool,
}

/// Server mode on [`crate::state::item_flags::ItemFlags`]: the list from
/// `0x0EE8` drives lock and junk, a toggle becomes one `0x0EE1`, and a
/// stack that leaves its row loses the mark.
pub fn offline_server_item_marks() -> OfflineServerMarksReport {
    use ragnarok_bytes::ByteWriter;
    use ragnarok_packets::{ITEM_MARK_JUNK, PacketExt, SetItemMarkPacket};

    use crate::state::item_flags::{self, ItemFlags, ItemKey, MarkPlace, StackRef};

    let knife = StackRef {
        index: 4,
        item_id: 1201,
        refine: 0,
        cards: [0; 4],
        name: "Knife".to_owned(),
        amount: 1,
        equipped: false,
    };
    let mut flags = ItemFlags::default();
    flags.set_locked(knife.key(), true);
    flags.enable_server_marks();
    let file_mark_ignored = item_flags::attempt_drop(&flags, &knife, 1).send.is_some();

    flags.apply_server_list([(MarkPlace::inventory(4), ITEM_MARK_LOCK)]);
    let listed_locked = flags.locked_at(knife.key(), knife.place());
    let drop_refusal = item_flags::attempt_drop(&flags, &knife, 1)
        .messages
        .into_iter()
        .next()
        .unwrap_or_default();

    flags.set_junk_at(ItemKey::new(501, 0, [0; 4]), Some(MarkPlace::inventory(2)), true);
    let mut toggle_bytes = Vec::new();
    for request in flags.take_mark_requests() {
        let mut writer = ByteWriter::new();
        let packet = SetItemMarkPacket {
            location: request.place.location,
            index: ragnarok_packets::InventoryIndex(request.place.index),
            mask: request.mask,
            value: request.value,
        };
        if packet.packet_to_bytes(&mut writer).is_ok() {
            toggle_bytes.extend_from_slice(writer.as_slice());
        }
    }
    flags.apply_server_result(MarkPlace::inventory(2), ITEM_MARK_JUNK | ITEM_MARK_LOCK, 0);
    let result_applied = flags.locked_at(ItemKey::new(501, 0, [0; 4]), Some(MarkPlace::inventory(2)));

    flags.clear_place(MarkPlace::inventory(4));
    let fresh_row_cleared = item_flags::attempt_drop(&flags, &knife, 1).send.is_some();

    OfflineServerMarksReport {
        file_mark_ignored,
        listed_locked,
        drop_refusal,
        toggle_bytes,
        result_applied,
        fresh_row_cleared,
    }
}
