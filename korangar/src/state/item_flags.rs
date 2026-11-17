//! Per-character new, junk, and lock marks.
//!
//! Inventory indexes change when the server moves a stack, so a mark is stored
//! on the item id, refine, and cards. The file is `client/item_flags.ron`, the
//! same kind of client config the hotbar and game settings use. Only the
//! active character's rows are loaded; other characters stay in the file.
//! Only junk and lock are saved. The new-item glow lasts for the session, so
//! the login list never glows. A file that does not parse is never written
//! over, and saves go through a temp file and a rename.
//!
//! When the map server announces item marks (`0x0EF1` bit 0), junk and lock
//! move to the server: they are kept per inventory or cart row, read from
//! `0x0EE8`, changed with `0x0EE1`, and the file is neither read for them nor
//! written. The server refuses drop, NPC sell, vend, trade, and mail of a
//! locked row on its own; the checks here only save a round trip. Storage
//! rows show no marks in that mode. The new-item glow stays client-only.

use std::path::{Path, PathBuf};
use std::time::Instant;

#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, print_debug};
use korangar_interface::element::StateElement;
use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::InventoryIndex;
use ron::ser::PrettyConfig;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

use crate::world::ResourceMetadata;

/// How long a new-item glow takes to fade after the first hover.
pub const NEW_GLOW_MS: u32 = 300;
/// Peak opacity of [`NEW_GLOW_MS`] while the item is still new.
pub const NEW_GLOW_ALPHA: u8 = 160;

const FILE_NAME: &str = "client/item_flags.ron";
/// A junk or lock edit is written this long after it is made, and right away
/// on logout, exit, and character change.
pub const SAVE_DEBOUNCE_MS: u32 = 2000;

/// One inventory or cart row as the server keys its marks. `index` is the
/// client index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkPlace {
    pub location: u8,
    pub index: u16,
}

impl MarkPlace {
    pub fn inventory(index: u16) -> Self {
        Self {
            location: ragnarok_packets::ITEM_MARK_INVENTORY,
            index,
        }
    }

    pub fn cart(index: u16) -> Self {
        Self {
            location: ragnarok_packets::ITEM_MARK_CART,
            index,
        }
    }

    /// A personal storage row. The server only lists and changes these while
    /// the storage is open.
    pub fn storage(index: u16) -> Self {
        Self {
            location: ragnarok_packets::ITEM_MARK_STORAGE,
            index,
        }
    }

    pub fn for_source(source: crate::interface::resource::ItemSource, index: u16) -> Option<Self> {
        use crate::interface::resource::ItemSource;
        match source {
            ItemSource::Inventory | ItemSource::Equipment { .. } | ItemSource::Hotbar { .. } => Some(Self::inventory(index)),
            ItemSource::Cart => Some(Self::cart(index)),
            ItemSource::Storage => Some(Self::storage(index)),
        }
    }
}

/// One `0x0EE1` the network loop still has to send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkRequest {
    pub place: MarkPlace,
    pub mask: u8,
    pub value: u8,
}

/// Marks kept on the server, by row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ServerMarks {
    rows: Vec<(MarkPlace, u8)>,
    outbox: Vec<MarkRequest>,
    /// File marks still have to be copied up after the first list.
    migrate: bool,
}

impl ServerMarks {
    fn get(&self, place: MarkPlace) -> u8 {
        self.rows.iter().find(|(row, _)| *row == place).map_or(0, |(_, marks)| *marks)
    }

    fn put(&mut self, place: MarkPlace, marks: u8) {
        self.rows.retain(|(row, _)| *row != place);
        if marks != 0 {
            self.rows.push((place, marks));
        }
    }
}

/// Stable identity of one stack. Two potions of the same id share a mark.
/// Two weapons with a different refine or a different card do not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemKey {
    pub item_id: u32,
    pub refine: u8,
    pub cards: [u32; 4],
}

impl ItemKey {
    pub fn new(item_id: u32, refine: u8, cards: [u32; 4]) -> Self {
        Self { item_id, refine, cards }
    }

    pub fn from_inventory<Meta>(item: &InventoryItem<Meta>) -> Self {
        Self {
            item_id: item.item_id.0,
            refine: refine_of(&item.details),
            cards: item.slot,
        }
    }
}

pub fn refine_of(details: &InventoryItemDetails) -> u8 {
    match details {
        InventoryItemDetails::Equippable { refinement_level, .. } => *refinement_level,
        InventoryItemDetails::Regular { .. } => 0,
    }
}

pub fn is_equipped(details: &InventoryItemDetails) -> bool {
    match details {
        InventoryItemDetails::Regular { equipped_position, .. } | InventoryItemDetails::Equippable { equipped_position, .. } => {
            !equipped_position.is_empty()
        }
    }
}

/// Worn gear, or the arrow stack the server equipped. A Regular row's
/// `equipped_position` is the slot the item fits, so it does not mean worn.
pub fn is_worn_stack<Meta>(item: &InventoryItem<Meta>, equipped_ammo: Option<InventoryIndex>) -> bool {
    match &item.details {
        InventoryItemDetails::Equippable { equipped_position, .. } => !equipped_position.is_empty(),
        InventoryItemDetails::Regular { .. } => equipped_ammo == Some(item.index),
    }
}

/// Display name, or `Item <id>` when the row has no name yet.
pub fn item_label(name: &str, item_id: u32) -> String {
    if name.is_empty() {
        format!("Item {item_id}")
    } else {
        name.to_owned()
    }
}

/// What the item box copies out before it draws, so the layout pass can
/// update flags without holding the inventory borrow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverNote {
    pub key: ItemKey,
    pub index: u16,
    pub name: String,
    pub place: Option<MarkPlace>,
}

impl HoverNote {
    pub fn from_item(item: &InventoryItem<ResourceMetadata>) -> Self {
        Self {
            key: ItemKey::from_inventory(item),
            index: item.index.0,
            name: item.metadata.name.clone(),
            place: None,
        }
    }

    /// [`Self::from_item`] for a slot of `source`.
    pub fn from_slot(item: &InventoryItem<ResourceMetadata>, source: crate::interface::resource::ItemSource) -> Self {
        Self {
            place: MarkPlace::for_source(source, item.index.0),
            ..Self::from_item(item)
        }
    }
}

/// One bag or cart stack the sell-junk and refusal checks look at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackRef {
    pub index: u16,
    pub item_id: u32,
    pub refine: u8,
    pub cards: [u32; 4],
    pub name: String,
    pub amount: u16,
    pub equipped: bool,
}

impl StackRef {
    pub fn key(&self) -> ItemKey {
        ItemKey::new(self.item_id, self.refine, self.cards)
    }

    /// Stacks are bag rows.
    pub fn place(&self) -> Option<MarkPlace> {
        Some(MarkPlace::inventory(self.index))
    }

    pub fn from_item(item: &InventoryItem<ResourceMetadata>, equipped_ammo: Option<InventoryIndex>) -> Self {
        Self {
            index: item.index.0,
            item_id: item.item_id.0,
            refine: refine_of(&item.details),
            cards: item.slot,
            name: item.metadata.name.clone(),
            amount: match &item.details {
                InventoryItemDetails::Regular { amount, .. } => *amount,
                InventoryItemDetails::Equippable { .. } => 1,
            },
            equipped: is_worn_stack(item, equipped_ammo),
        }
    }

    pub fn label(&self) -> String {
        item_label(&self.name, self.item_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlagSend {
    Drop { index: u16, amount: u16 },
    Sell { lines: Vec<(u16, u16)> },
    Trade { index: u16, amount: u16 },
    Mail { index: u16, count: u16 },
    Vend { index: u16 },
    Store { index: u16, amount: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagAttempt {
    pub send: Option<FlagSend>,
    pub messages: Vec<String>,
}

impl FlagAttempt {
    fn allow(send: FlagSend) -> Self {
        Self {
            send: Some(send),
            messages: Vec::new(),
        }
    }

    fn refuse(message: String) -> Self {
        Self {
            send: None,
            messages: vec![message],
        }
    }
}

/// A whole junk stack to put on the sell cart. `amount` is the full stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JunkAdd {
    pub index: u16,
    pub amount: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub(crate) struct ItemFlagEntry {
    item_id: u32,
    refine: u8,
    cards: [u32; 4],
    junk: bool,
    locked: bool,
    is_new: bool,
    /// Client clock when the hover fade started. Empty until the first hover.
    #[hidden_element]
    fade_from: Option<u32>,
}

impl ItemFlagEntry {
    fn matches(&self, key: ItemKey) -> bool {
        self.item_id == key.item_id && self.refine == key.refine && self.cards == key.cards
    }

    fn blank(key: ItemKey) -> Self {
        Self {
            item_id: key.item_id,
            refine: key.refine,
            cards: key.cards,
            junk: false,
            locked: false,
            is_new: false,
            fade_from: None,
        }
    }

    fn alive(&self) -> bool {
        self.junk || self.locked || self.is_new
    }

    /// Junk and lock are kept across logins; the glow is not.
    fn stored(&self) -> bool {
        self.junk || self.locked
    }
}

#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub(crate) struct HoveredItem {
    index: u16,
    item_id: u32,
    refine: u8,
    cards: [u32; 4],
    name: String,
    #[hidden_element]
    place: Option<MarkPlace>,
}

impl HoveredItem {
    fn key(&self) -> ItemKey {
        ItemKey::new(self.item_id, self.refine, self.cards)
    }
}

/// Glow, junk badge, and lock icon for one drawn slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlagAppearance {
    pub glow: u8,
    pub junk: bool,
    pub locked: bool,
}

impl FlagAppearance {
    pub fn visible(self) -> bool {
        self.glow > 0 || self.junk || self.locked
    }
}

#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct ItemFlags {
    character_id: u32,
    /// Set by the first inventory list. Arrivals before that are the login
    /// snapshot and do not glow.
    snapshot: bool,
    entries: Vec<ItemFlagEntry>,
    #[hidden_element]
    hover: Option<HoveredItem>,
    #[hidden_element]
    hover_fresh: bool,
    /// The item grid laid out since the last input frame.
    #[hidden_element]
    laid_out: bool,
    #[hidden_element]
    persist: bool,
    /// Client clock of the first junk or lock edit not yet on disk.
    #[hidden_element]
    dirty_at: Option<u32>,
    /// The file on disk did not parse at the last load or save.
    #[hidden_element]
    file_broken: bool,
    /// One chat line for the player, taken by the flow each frame.
    #[hidden_element]
    notice: Option<String>,
    /// Set once the map server announces item marks; junk and lock then
    /// live on the server.
    #[hidden_element]
    server: Option<ServerMarks>,
    /// This character's file marks were already copied to the server.
    #[hidden_element]
    migrated: bool,
}

/// What [`ItemFlags::save_to`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SaveOutcome {
    Written,
    /// The file on disk does not parse. It was left alone and copied aside.
    KeptBroken,
    Failed,
}

fn elapsed_ms() -> u32 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u32
}

impl ItemFlags {
    /// Which character a later [`Self::save_ron`] writes. Does not read the
    /// file.
    pub fn assign_character(&mut self, character_id: u32) {
        self.character_id = character_id;
    }

    /// Load this character's rows and remember later edits on disk.
    pub fn bind_character(&mut self, character_id: u32) {
        self.flush();
        let path = Path::new(FILE_NAME);
        let text = read_existing(path).unwrap_or_default();
        self.load_ron(character_id, &text);
        self.file_broken = false;
        if parse_file(&text).is_none() {
            self.note_broken_file(path);
        }
        self.dirty_at = None;
        self.persist = true;
        self.server = None;
        self.snapshot = false;
        self.hover = None;
        self.hover_fresh = false;
    }

    /// Replace the in-memory rows with this character's section of `text`.
    pub fn load_ron(&mut self, character_id: u32, text: &str) {
        self.character_id = character_id;
        let record = parse_file(text)
            .unwrap_or_default()
            .characters
            .into_iter()
            .find(|character| character.character_id == character_id);
        self.migrated = record.as_ref().is_some_and(|character| character.server_migrated);
        self.entries = record
            .map(|character| character.items.into_iter().filter_map(StoredFlag::into_entry).collect())
            .unwrap_or_default();
    }

    /// File text with this character replaced and every other character kept.
    /// `None` when `previous` does not parse, so a broken file is never
    /// written over.
    pub fn save_ron(&self, previous: &str) -> Option<String> {
        let mut file = parse_file(previous)?;
        file.characters.retain(|character| character.character_id != self.character_id);
        let items = self
            .entries
            .iter()
            .filter(|entry| entry.stored())
            .map(StoredFlag::from_entry)
            .collect::<Vec<_>>();
        if !items.is_empty() || self.migrated {
            file.characters.push(CharacterRecord {
                character_id: self.character_id,
                items,
                server_migrated: self.migrated,
            });
        }
        file.characters.sort_by_key(|character| character.character_id);
        ron::ser::to_string_pretty(&file, PrettyConfig::new()).ok()
    }

    /// The login inventory list. Items in it do not become new.
    pub fn note_snapshot(&mut self) {
        self.snapshot = true;
    }

    /// `ItemPickup`, `CartItemAdded`, or a storage take-out after the snapshot.
    pub fn note_arrival(&mut self, key: ItemKey) {
        if !self.snapshot {
            return;
        }
        let entry = self.entry_mut(key);
        entry.is_new = true;
        entry.fade_from = None;
    }

    pub fn is_new(&self, key: ItemKey) -> bool {
        self.entry(key).is_some_and(|entry| entry.is_new)
    }

    /// The map server keeps junk and lock. Pending file edits are written
    /// first; after this the file is not written again for this character.
    pub fn enable_server_marks(&mut self) {
        self.flush();
        self.persist = false;
        self.dirty_at = None;
        if self.server.is_none() {
            self.server = Some(ServerMarks {
                migrate: !self.migrated,
                ..ServerMarks::default()
            });
        }
    }

    /// The file marks still wait to be copied to the server.
    pub fn migration_pending(&self) -> bool {
        self.server.as_ref().is_some_and(|server| server.migrate)
    }

    /// Once per character, after the first server list: copy file lock and
    /// junk onto the rows still present. Bits are only ever set, never
    /// cleared. The file records that it was done and is not used for marks
    /// again.
    pub fn migrate_file_marks(&mut self, rows: &[(MarkPlace, ItemKey)]) -> usize {
        let mut sent = 0;
        let wanted = rows
            .iter()
            .filter_map(|(place, key)| {
                let entry = self.entry(*key)?;
                let mut bits = 0;
                if entry.locked {
                    bits |= ragnarok_packets::ITEM_MARK_LOCK;
                }
                if entry.junk {
                    bits |= ragnarok_packets::ITEM_MARK_JUNK;
                }
                (bits != 0).then_some((*place, bits))
            })
            .collect::<Vec<_>>();
        let Some(server) = self.server.as_mut() else {
            return 0;
        };
        if !server.migrate {
            return 0;
        }
        server.migrate = false;
        for (place, bits) in wanted {
            let current = server.get(place);
            let missing = bits & !current;
            if missing == 0 {
                continue;
            }
            server.put(place, current | missing);
            server.outbox.push(MarkRequest {
                place,
                mask: missing,
                value: missing,
            });
            sent += 1;
        }
        self.migrated = true;
        self.record_migration();
        sent
    }

    /// Write the migrated flag; the marks in the file are kept for servers
    /// without item marks.
    fn record_migration(&mut self) {
        if self.character_id == 0 {
            return;
        }
        self.save_to(Path::new(FILE_NAME));
    }

    #[cfg(test)]
    pub fn server_marks_enabled(&self) -> bool {
        self.server.is_some()
    }

    /// `0x0EE8`: every marked row. Rows not listed have no marks.
    pub fn apply_server_list(&mut self, entries: impl IntoIterator<Item = (MarkPlace, u8)>) {
        let Some(server) = self.server.as_mut() else {
            return;
        };
        server.rows.clear();
        for (place, marks) in entries {
            server.put(
                place,
                marks & (ragnarok_packets::ITEM_MARK_LOCK | ragnarok_packets::ITEM_MARK_JUNK),
            );
        }
    }

    /// `0x0EE9`: the row's marks after a change. Result 1 is a row that is
    /// gone.
    pub fn apply_server_result(&mut self, place: MarkPlace, marks: u8, result: u8) {
        let Some(server) = self.server.as_mut() else {
            return;
        };
        let marks = if result == 1 { 0 } else { marks };
        server.put(
            place,
            marks & (ragnarok_packets::ITEM_MARK_LOCK | ragnarok_packets::ITEM_MARK_JUNK),
        );
    }

    /// The row was used up or filled by something new; the server cleared it.
    pub fn clear_place(&mut self, place: MarkPlace) {
        if let Some(server) = self.server.as_mut() {
            server.put(place, 0);
        }
    }

    /// `0x0EE1` requests made since the last call.
    pub fn take_mark_requests(&mut self) -> Vec<MarkRequest> {
        self.server
            .as_mut()
            .map(|server| std::mem::take(&mut server.outbox))
            .unwrap_or_default()
    }

    fn server_bit(&self, place: Option<MarkPlace>, bit: u8) -> Option<bool> {
        let server = self.server.as_ref()?;
        Some(place.is_some_and(|place| server.get(place) & bit != 0))
    }

    fn set_server_bit(&mut self, place: Option<MarkPlace>, bit: u8, on: bool) {
        let (Some(server), Some(place)) = (self.server.as_mut(), place) else {
            return;
        };
        let current = server.get(place);
        let next = if on { current | bit } else { current & !bit };
        if next == current {
            return;
        }
        server.put(place, next);
        server.outbox.push(MarkRequest {
            place,
            mask: bit,
            value: if on { bit } else { 0 },
        });
    }

    /// Junk on this row: the server row when the server keeps marks,
    /// otherwise the stack key.
    pub fn junk_at(&self, key: ItemKey, place: Option<MarkPlace>) -> bool {
        self.server_bit(place, ragnarok_packets::ITEM_MARK_JUNK)
            .unwrap_or_else(|| self.entry(key).is_some_and(|entry| entry.junk))
    }

    pub fn locked_at(&self, key: ItemKey, place: Option<MarkPlace>) -> bool {
        self.server_bit(place, ragnarok_packets::ITEM_MARK_LOCK)
            .unwrap_or_else(|| self.entry(key).is_some_and(|entry| entry.locked))
    }

    pub fn set_junk_at(&mut self, key: ItemKey, place: Option<MarkPlace>, junk: bool) {
        if self.server.is_some() {
            self.set_server_bit(place, ragnarok_packets::ITEM_MARK_JUNK, junk);
            return;
        }
        let changed = self.is_junk(key) != junk;
        self.entry_mut(key).junk = junk;
        self.prune();
        if changed {
            self.mark_dirty(elapsed_ms());
        }
    }

    pub fn set_locked_at(&mut self, key: ItemKey, place: Option<MarkPlace>, locked: bool) {
        if self.server.is_some() {
            self.set_server_bit(place, ragnarok_packets::ITEM_MARK_LOCK, locked);
            return;
        }
        let changed = self.is_locked(key) != locked;
        self.entry_mut(key).locked = locked;
        self.prune();
        if changed {
            self.mark_dirty(elapsed_ms());
        }
    }

    pub fn refusal_at(&self, key: ItemKey, place: Option<MarkPlace>, name: &str) -> Option<String> {
        self.locked_at(key, place)
            .then(|| format!("{} is locked", item_label(name, key.item_id)))
    }

    /// Key-only reads and writes, for file mode. With server marks they see
    /// no row.
    pub fn is_junk(&self, key: ItemKey) -> bool {
        self.junk_at(key, None)
    }

    pub fn is_locked(&self, key: ItemKey) -> bool {
        self.locked_at(key, None)
    }

    pub fn set_junk(&mut self, key: ItemKey, junk: bool) {
        self.set_junk_at(key, None, junk);
    }

    pub fn set_locked(&mut self, key: ItemKey, locked: bool) {
        self.set_locked_at(key, None, locked);
    }

    /// Called once per input frame, before hotkeys. A grid that was not drawn
    /// last frame drops its hover, so J and L do nothing in the world.
    pub fn begin_input_frame(&mut self) {
        let now = elapsed_ms();
        self.advance_to(now);
        if self.save_due(now) {
            self.flush();
        }
        if !self.laid_out {
            self.hover = None;
            self.hover_fresh = false;
        }
        self.laid_out = false;
    }

    /// First slot of this frame clears the hover. Later slots only add to it.
    pub fn prepare_layout(&mut self) {
        if self.laid_out {
            return;
        }
        self.laid_out = true;
        self.hover = None;
        self.hover_fresh = false;
    }

    pub fn note_hover_at(&mut self, key: ItemKey, index: u16, name: String, now: u32) {
        self.note_hover_in(key, None, index, name, now);
    }

    /// A hover on a slot whose server row is `place`.
    pub fn note_hover_place(&mut self, key: ItemKey, place: Option<MarkPlace>, index: u16, name: String) {
        self.note_hover_in(key, place, index, name, elapsed_ms());
    }

    fn note_hover_in(&mut self, key: ItemKey, place: Option<MarkPlace>, index: u16, name: String, now: u32) {
        self.hover = Some(HoveredItem {
            index,
            item_id: key.item_id,
            refine: key.refine,
            cards: key.cards,
            name,
            place,
        });
        self.hover_fresh = true;
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.matches(key))
            && entry.is_new
            && entry.fade_from.is_none()
        {
            entry.fade_from = Some(now);
        }
    }

    pub fn toggle_hovered_junk(&mut self) -> bool {
        let Some((key, place)) = self.fresh_hover() else {
            return false;
        };
        let junk = !self.junk_at(key, place);
        self.set_junk_at(key, place, junk);
        true
    }

    pub fn toggle_hovered_lock(&mut self) -> bool {
        let Some((key, place)) = self.fresh_hover() else {
            return false;
        };
        let locked = !self.locked_at(key, place);
        self.set_locked_at(key, place, locked);
        true
    }

    pub fn advance_to(&mut self, now: u32) {
        let mut changed = false;
        for entry in &mut self.entries {
            if let Some(started) = entry.fade_from
                && entry.is_new
                && now.wrapping_sub(started) >= NEW_GLOW_MS
            {
                entry.is_new = false;
                entry.fade_from = None;
                changed = true;
            }
        }
        if changed {
            self.prune();
        }
    }

    pub fn glow_alpha_at(&self, key: ItemKey, now: u32) -> u8 {
        let Some(entry) = self.entry(key) else {
            return 0;
        };
        if !entry.is_new {
            return 0;
        }
        let Some(started) = entry.fade_from else {
            return NEW_GLOW_ALPHA;
        };
        let elapsed = now.wrapping_sub(started);
        if elapsed >= NEW_GLOW_MS {
            return 0;
        }
        let left = NEW_GLOW_MS - elapsed;
        (u32::from(NEW_GLOW_ALPHA) * left / NEW_GLOW_MS) as u8
    }

    pub fn appearance_at(&self, key: ItemKey, place: Option<MarkPlace>, now: u32) -> FlagAppearance {
        FlagAppearance {
            glow: self.glow_alpha_at(key, now),
            junk: self.junk_at(key, place),
            locked: self.locked_at(key, place),
        }
    }

    /// Glow, junk, and lock for the overlay, using the client clock.
    pub fn appearance_now_at(&self, key: ItemKey, place: Option<MarkPlace>) -> FlagAppearance {
        self.appearance_at(key, place, elapsed_ms())
    }

    fn fresh_hover(&self) -> Option<(ItemKey, Option<MarkPlace>)> {
        self.hover_fresh
            .then(|| self.hover.as_ref().map(|hover| (hover.key(), hover.place)))
            .flatten()
    }

    fn entry(&self, key: ItemKey) -> Option<&ItemFlagEntry> {
        self.entries.iter().find(|entry| entry.matches(key))
    }

    fn entry_mut(&mut self, key: ItemKey) -> &mut ItemFlagEntry {
        if let Some(position) = self.entries.iter().position(|entry| entry.matches(key)) {
            return &mut self.entries[position];
        }
        self.entries.push(ItemFlagEntry::blank(key));
        self.entries.last_mut().expect("just pushed")
    }

    fn prune(&mut self) {
        self.entries.retain(ItemFlagEntry::alive);
    }

    fn mark_dirty(&mut self, now: u32) {
        if self.dirty_at.is_none() {
            self.dirty_at = Some(now);
        }
    }

    /// Client clock of the first junk or lock edit not yet saved.
    #[cfg(test)]
    fn unsaved_since(&self) -> Option<u32> {
        self.dirty_at
    }

    /// A junk or lock edit has waited [`SAVE_DEBOUNCE_MS`].
    pub fn save_due(&self, now: u32) -> bool {
        self.dirty_at.is_some_and(|at| now.wrapping_sub(at) >= SAVE_DEBOUNCE_MS)
    }

    /// Write pending junk and lock edits now. Logout, exit, and character
    /// change call this; frames call it once the debounce has passed.
    pub fn flush(&mut self) {
        if !self.persist || self.dirty_at.is_none() {
            return;
        }
        self.dirty_at = None;
        self.save_to(Path::new(FILE_NAME));
    }

    /// A chat line about the file, once.
    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    /// Merge this character into the file at `path`. A file that does not
    /// parse is left as it is.
    pub(crate) fn save_to(&mut self, path: &Path) -> SaveOutcome {
        let Some(previous) = read_existing(path) else {
            return SaveOutcome::Failed;
        };
        let Some(text) = self.save_ron(&previous) else {
            self.note_broken_file(path);
            return SaveOutcome::KeptBroken;
        };
        match write_atomic(path, &text) {
            Ok(()) => {
                self.file_broken = false;
                SaveOutcome::Written
            }
            Err(_error) => {
                #[cfg(feature = "debug")]
                print_debug!("[{}] failed to save {}: {:?}", "error".red(), path.display(), _error);
                SaveOutcome::Failed
            }
        }
    }

    fn note_broken_file(&mut self, path: &Path) {
        if self.file_broken {
            return;
        }
        self.file_broken = true;
        let copy = suffixed(path, ".corrupt");
        let _ = std::fs::copy(path, &copy);
        #[cfg(feature = "debug")]
        print_debug!(
            "[{}] {} does not parse; kept a copy at {} and will not save over it",
            "warning".red(),
            path.display(),
            copy.display()
        );
        self.notice = Some(format!(
            "{} could not be read. A copy is at {}. Junk and lock marks will not be saved until the file is fixed or removed.",
            path.display(),
            copy.display()
        ));
    }
}

/// The file text, empty when there is no file yet, `None` when it exists but
/// cannot be read.
fn read_existing(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(String::new()),
        Err(_) => None,
    }
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Write next to `path` and rename into place, so a cut-off write leaves the
/// old file whole.
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let temp = suffixed(path, ".tmp");
    {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&temp, path)
}

/// Applies a Junk / Lock pick from the item menu.
pub fn apply_toggle(flags: &mut ItemFlags, key: ItemKey, toggle: crate::state::item_menu::FlagToggle) {
    apply_toggle_at(flags, key, None, toggle);
}

/// [`apply_toggle`] on the row at `place`.
pub fn apply_toggle_at(flags: &mut ItemFlags, key: ItemKey, place: Option<MarkPlace>, toggle: crate::state::item_menu::FlagToggle) {
    match toggle {
        crate::state::item_menu::FlagToggle::Junk(junk) => flags.set_junk_at(key, place, junk),
        crate::state::item_menu::FlagToggle::Lock(locked) => flags.set_locked_at(key, place, locked),
    }
}

/// Junk stacks the NPC will buy, minus locked and equipped rows.
/// Locked junk is named in the returned messages and is not added.
pub fn plan_sell_junk(flags: &ItemFlags, stacks: &[StackRef], sellable: &[(u16, u16)]) -> (Vec<JunkAdd>, Vec<String>) {
    let mut adds = Vec::new();
    let mut messages = Vec::new();
    for stack in stacks {
        if !flags.junk_at(stack.key(), stack.place()) {
            continue;
        }
        if stack.equipped {
            continue;
        }
        if flags.locked_at(stack.key(), stack.place()) {
            messages.push(format!("{} is locked", stack.label()));
            continue;
        }
        let Some((_, amount)) = sellable.iter().find(|(index, _)| *index == stack.index) else {
            continue;
        };
        if *amount == 0 {
            continue;
        }
        adds.push(JunkAdd {
            index: stack.index,
            amount: *amount,
        });
    }
    (adds, messages)
}

pub fn attempt_drop(flags: &ItemFlags, stack: &StackRef, amount: u16) -> FlagAttempt {
    if let Some(message) = flags.refusal_at(stack.key(), stack.place(), &stack.name) {
        return FlagAttempt::refuse(message);
    }
    FlagAttempt::allow(FlagSend::Drop {
        index: stack.index,
        amount,
    })
}

pub fn attempt_trade(flags: &ItemFlags, stack: &StackRef, amount: u16) -> FlagAttempt {
    if let Some(message) = flags.refusal_at(stack.key(), stack.place(), &stack.name) {
        return FlagAttempt::refuse(message);
    }
    FlagAttempt::allow(FlagSend::Trade {
        index: stack.index,
        amount,
    })
}

pub fn attempt_mail(flags: &ItemFlags, stack: &StackRef, count: u16) -> FlagAttempt {
    if let Some(message) = flags.refusal_at(stack.key(), stack.place(), &stack.name) {
        return FlagAttempt::refuse(message);
    }
    FlagAttempt::allow(FlagSend::Mail { index: stack.index, count })
}

pub fn attempt_vend(flags: &ItemFlags, stack: &StackRef) -> FlagAttempt {
    if let Some(message) = flags.refusal_at(stack.key(), stack.place(), &stack.name) {
        return FlagAttempt::refuse(message);
    }
    FlagAttempt::allow(FlagSend::Vend { index: stack.index })
}

/// Storage and cart moves stay allowed for a locked stack.
pub fn attempt_store(stack: &StackRef, amount: u16) -> FlagAttempt {
    FlagAttempt::allow(FlagSend::Store {
        index: stack.index,
        amount,
    })
}

/// NPC sell of the cart. One locked line refuses the whole send.
pub fn attempt_sell(flags: &ItemFlags, stacks: &[StackRef], lines: &[(u16, u16)]) -> FlagAttempt {
    let mut messages = Vec::new();
    for (index, _) in lines {
        let Some(stack) = stacks.iter().find(|stack| stack.index == *index) else {
            continue;
        };
        if let Some(message) = flags.refusal_at(stack.key(), stack.place(), &stack.name) {
            messages.push(message);
        }
    }
    if !messages.is_empty() {
        return FlagAttempt { send: None, messages };
    }
    FlagAttempt::allow(FlagSend::Sell { lines: lines.to_vec() })
}

/// `location` is [`ragnarok_packets::ITEM_MARK_INVENTORY`] or
/// [`ragnarok_packets::ITEM_MARK_CART`], matching `items`.
pub fn refusal_for_index(flags: &ItemFlags, items: &[InventoryItem<ResourceMetadata>], index: u16, location: u8) -> Option<String> {
    let item = items.iter().find(|item| item.index.0 == index)?;
    let place = MarkPlace { location, index };
    flags.refusal_at(ItemKey::from_inventory(item), Some(place), &item.metadata.name)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct FlagFile {
    #[serde(default)]
    characters: Vec<CharacterRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CharacterRecord {
    character_id: u32,
    items: Vec<StoredFlag>,
    /// Lock and junk were copied to the server once; the server is the
    /// source from then on. Older files have no field.
    #[serde(default)]
    server_migrated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredFlag {
    item_id: u32,
    refine: u8,
    cards: [u32; 4],
    junk: bool,
    locked: bool,
}

impl StoredFlag {
    fn from_entry(entry: &ItemFlagEntry) -> Self {
        Self {
            item_id: entry.item_id,
            refine: entry.refine,
            cards: entry.cards,
            junk: entry.junk,
            locked: entry.locked,
        }
    }

    fn into_entry(self) -> Option<ItemFlagEntry> {
        let entry = ItemFlagEntry {
            item_id: self.item_id,
            refine: self.refine,
            cards: self.cards,
            junk: self.junk,
            locked: self.locked,
            is_new: false,
            fade_from: None,
        };
        entry.stored().then_some(entry)
    }
}

/// `None` when the text is not a flag file. Older files with an `is_new`
/// field still parse; the field is ignored.
fn parse_file(text: &str) -> Option<FlagFile> {
    if text.trim().is_empty() {
        return Some(FlagFile::default());
    }
    ron::from_str(text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(index: u16, item_id: u32, name: &str, amount: u16, refine: u8, equipped: bool) -> StackRef {
        StackRef {
            index,
            item_id,
            refine,
            cards: [0; 4],
            name: name.to_owned(),
            amount,
            equipped,
        }
    }

    #[test]
    fn server_marks_follow_rows_not_keys_and_queue_requests() {
        let mut flags = ItemFlags::default();
        let potion = ItemKey::new(501, 0, [0; 4]);
        flags.set_junk(potion, true);
        flags.enable_server_marks();
        assert!(flags.server_marks_enabled());
        // File marks are not used once the server keeps them.
        assert!(!flags.junk_at(potion, Some(MarkPlace::inventory(2))));
        assert!(flags.migration_pending());

        flags.apply_server_list([
            (MarkPlace::inventory(2), ragnarok_packets::ITEM_MARK_LOCK),
            (MarkPlace::cart(5), ragnarok_packets::ITEM_MARK_JUNK),
        ]);
        assert!(flags.locked_at(potion, Some(MarkPlace::inventory(2))));
        assert!(!flags.locked_at(potion, Some(MarkPlace::inventory(3))));
        assert!(!flags.locked_at(potion, None));
        assert!(flags.junk_at(potion, Some(MarkPlace::cart(5))));
        assert_eq!(
            flags.refusal_at(potion, Some(MarkPlace::inventory(2)), "Red Potion"),
            Some("Red Potion is locked".to_owned())
        );

        flags.set_locked_at(potion, Some(MarkPlace::inventory(2)), false);
        flags.set_locked_at(potion, Some(MarkPlace::inventory(2)), false);
        flags.set_junk_at(potion, Some(MarkPlace::inventory(3)), true);
        flags.set_junk_at(potion, None, true);
        assert_eq!(flags.take_mark_requests(), vec![
            MarkRequest {
                place: MarkPlace::inventory(2),
                mask: ragnarok_packets::ITEM_MARK_LOCK,
                value: 0,
            },
            MarkRequest {
                place: MarkPlace::inventory(3),
                mask: ragnarok_packets::ITEM_MARK_JUNK,
                value: ragnarok_packets::ITEM_MARK_JUNK,
            },
        ]);
        assert!(flags.take_mark_requests().is_empty());

        // The server answer wins; a gone row loses its marks.
        flags.apply_server_result(
            MarkPlace::inventory(3),
            ragnarok_packets::ITEM_MARK_LOCK | ragnarok_packets::ITEM_MARK_JUNK,
            0,
        );
        assert!(flags.locked_at(potion, Some(MarkPlace::inventory(3))));
        flags.apply_server_result(MarkPlace::inventory(3), 0, 1);
        assert!(!flags.junk_at(potion, Some(MarkPlace::inventory(3))));
        flags.clear_place(MarkPlace::cart(5));
        assert!(!flags.junk_at(potion, Some(MarkPlace::cart(5))));

        // Hover toggles go to the hovered row.
        flags.prepare_layout();
        flags.note_hover_place(potion, Some(MarkPlace::inventory(7)), 7, "Red Potion".to_owned());
        assert!(flags.toggle_hovered_lock());
        assert!(flags.locked_at(potion, Some(MarkPlace::inventory(7))));
        assert_eq!(flags.take_mark_requests().len(), 1);

        // A new character starts in file mode again.
        flags.assign_character(9);
        flags.server = None;
        assert!(flags.is_junk(potion));
    }

    #[test]
    fn file_marks_migrate_once_and_only_set_bits() {
        let knife = ItemKey::new(1201, 0, [0; 4]);
        let potion = ItemKey::new(501, 0, [0; 4]);
        let gone = ItemKey::new(909, 0, [0; 4]);
        let mut flags = ItemFlags::default();
        flags.set_locked(knife, true);
        flags.set_junk(knife, true);
        flags.set_junk(potion, true);
        flags.set_locked(gone, true);
        flags.enable_server_marks();
        assert!(flags.migration_pending());
        flags.apply_server_list([(MarkPlace::inventory(4), ragnarok_packets::ITEM_MARK_LOCK)]);

        let rows = [(MarkPlace::inventory(4), knife), (MarkPlace::cart(1), potion)];
        assert_eq!(flags.migrate_file_marks(&rows), 2);
        assert_eq!(flags.take_mark_requests(), vec![
            MarkRequest {
                place: MarkPlace::inventory(4),
                mask: ragnarok_packets::ITEM_MARK_JUNK,
                value: ragnarok_packets::ITEM_MARK_JUNK,
            },
            MarkRequest {
                place: MarkPlace::cart(1),
                mask: ragnarok_packets::ITEM_MARK_JUNK,
                value: ragnarok_packets::ITEM_MARK_JUNK,
            },
        ]);
        assert!(!flags.migration_pending());
        // Once only.
        assert_eq!(flags.migrate_file_marks(&rows), 0);

        // The file says so, keeps its marks, and a reload does not migrate.
        let text = flags.save_ron("").expect("saves");
        assert!(text.contains("server_migrated: true"));
        let mut loaded = ItemFlags::default();
        loaded.load_ron(0, &text);
        assert!(loaded.is_locked(knife));
        loaded.enable_server_marks();
        assert!(!loaded.migration_pending());
    }

    #[test]
    fn snapshot_does_not_glow_and_a_later_arrival_does() {
        let mut flags = ItemFlags::default();
        let potion = ItemKey::new(501, 0, [0; 4]);
        flags.note_arrival(potion);
        assert!(!flags.is_new(potion));
        assert_eq!(flags.glow_alpha_at(potion, 0), 0);

        flags.note_snapshot();
        flags.note_arrival(potion);
        assert!(flags.is_new(potion));
        assert_eq!(flags.glow_alpha_at(potion, 0), NEW_GLOW_ALPHA);

        flags.note_hover_at(potion, 1, "Red Potion".to_owned(), 1_000);
        assert_eq!(flags.glow_alpha_at(potion, 1_000), NEW_GLOW_ALPHA);
        assert_eq!(flags.glow_alpha_at(potion, 1_150), NEW_GLOW_ALPHA / 2);
        flags.advance_to(1_299);
        assert!(flags.is_new(potion));
        flags.advance_to(1_300);
        assert!(!flags.is_new(potion));
        assert_eq!(flags.glow_alpha_at(potion, 1_300), 0);
    }

    #[test]
    fn junk_sell_skips_locked_and_equipped_stacks() {
        let mut flags = ItemFlags::default();
        let potion = stack(1, 501, "Red Potion", 10, 0, false);
        let knife = stack(2, 1201, "Knife", 1, 0, false);
        let armor = stack(4, 2301, "Armor", 1, 0, true);
        flags.set_junk(potion.key(), true);
        flags.set_junk(knife.key(), true);
        flags.set_junk(armor.key(), true);
        flags.set_locked(knife.key(), true);

        let (adds, messages) = plan_sell_junk(&flags, &[potion, knife, armor], &[(1, 10), (2, 1), (4, 1)]);
        assert_eq!(adds, vec![JunkAdd { index: 1, amount: 10 }]);
        assert_eq!(messages, vec!["Knife is locked".to_owned()]);
    }

    #[test]
    fn lock_refuses_drop_trade_mail_sell_and_vend_but_not_storage() {
        let mut flags = ItemFlags::default();
        let knife = stack(2, 1201, "Knife", 1, 0, false);
        flags.set_locked(knife.key(), true);

        assert!(attempt_drop(&flags, &knife, 1).send.is_none());
        assert_eq!(attempt_trade(&flags, &knife, 1).messages, vec!["Knife is locked".to_owned()]);
        assert!(attempt_mail(&flags, &knife, 1).send.is_none());
        assert!(attempt_vend(&flags, &knife).send.is_none());
        assert!(attempt_sell(&flags, std::slice::from_ref(&knife), &[(2, 1)]).send.is_none());
        assert_eq!(attempt_store(&knife, 1).send, Some(FlagSend::Store { index: 2, amount: 1 }));

        let moved = stack(40, 1201, "Knife", 1, 0, false);
        assert_eq!(attempt_drop(&flags, &moved, 1).messages, vec!["Knife is locked".to_owned()]);
    }

    #[test]
    fn refine_and_cards_keep_separate_marks() {
        let mut flags = ItemFlags::default();
        let plain = ItemKey::new(1201, 0, [0; 4]);
        let refined = ItemKey::new(1201, 7, [0; 4]);
        let carded = ItemKey::new(1201, 0, [4001, 0, 0, 0]);
        flags.set_locked(plain, true);
        assert!(!flags.is_locked(refined));
        assert!(!flags.is_locked(carded));
    }

    #[test]
    fn flags_reload_for_the_same_character_only() {
        let mut flags = ItemFlags {
            character_id: 4,
            ..ItemFlags::default()
        };
        flags.set_junk(ItemKey::new(501, 0, [0; 4]), true);
        flags.set_locked(ItemKey::new(1201, 0, [0; 4]), true);
        flags.note_snapshot();
        flags.note_arrival(ItemKey::new(512, 0, [0; 4]));
        let previous = r#"(characters: [(character_id: 9, items: [(item_id: 909, refine: 0, cards: (0, 0, 0, 0), junk: true, locked: false, is_new: false)])])"#;
        let text = flags.save_ron(previous).expect("previous file parses");

        let mut loaded = ItemFlags::default();
        loaded.load_ron(4, &text);
        assert!(loaded.is_junk(ItemKey::new(501, 0, [0; 4])));
        assert!(loaded.is_locked(ItemKey::new(1201, 0, [0; 4])));
        assert!(!loaded.is_new(ItemKey::new(512, 0, [0; 4])));
        assert!(!loaded.is_junk(ItemKey::new(909, 0, [0; 4])));

        let mut other = ItemFlags::default();
        other.load_ron(9, &text);
        assert!(other.is_junk(ItemKey::new(909, 0, [0; 4])));
        assert!(!other.is_locked(ItemKey::new(1201, 0, [0; 4])));
    }

    fn arrows(index: u16, amount: u16) -> InventoryItem<ResourceMetadata> {
        use ragnarok_packets::{EquipPosition, ItemId, RegularItemFlags};

        InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name: "Arrow".to_owned(),
                description: String::new(),
            },
            index: InventoryIndex(index),
            item_id: ItemId(1750),
            item_type: 10,
            slot: [0; 4],
            hire_expiration_date: 0,
            details: InventoryItemDetails::Regular {
                amount,
                equipped_position: EquipPosition::AMMO,
                flags: RegularItemFlags::IDENTIFIED,
            },
        }
    }

    fn scratch_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("korangar-item-flags-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir.join("item_flags.ron")
    }

    #[test]
    fn new_glow_is_never_saved() {
        let mut flags = ItemFlags {
            character_id: 4,
            ..ItemFlags::default()
        };
        flags.note_snapshot();
        flags.note_arrival(ItemKey::new(512, 0, [0; 4]));
        assert!(flags.is_new(ItemKey::new(512, 0, [0; 4])));
        let text = flags.save_ron("").expect("empty file parses");
        assert!(!text.contains("is_new"));
        assert!(!text.contains("512"));

        let old = "(characters: [(character_id: 4, items: [(item_id: 512, refine: 0, cards: (0, 0, 0, 0), junk: false, locked: false, \
                   is_new: true), (item_id: 501, refine: 0, cards: (0, 0, 0, 0), junk: true, locked: false, is_new: true)])])";
        let mut loaded = ItemFlags::default();
        loaded.load_ron(4, old);
        assert!(!loaded.is_new(ItemKey::new(512, 0, [0; 4])));
        assert!(!loaded.is_new(ItemKey::new(501, 0, [0; 4])));
        assert!(loaded.is_junk(ItemKey::new(501, 0, [0; 4])));
        assert_eq!(loaded.glow_alpha_at(ItemKey::new(501, 0, [0; 4]), 0), 0);
    }

    #[test]
    fn broken_file_is_copied_aside_and_never_saved_over() {
        let path = scratch_file("broken");
        let broken = "(characters: [(character_id: 9, items: [(item_id: 909";
        std::fs::write(&path, broken).expect("write broken file");
        let mut flags = ItemFlags {
            character_id: 4,
            ..ItemFlags::default()
        };
        flags.set_locked(ItemKey::new(1201, 0, [0; 4]), true);

        assert!(flags.save_ron(broken).is_none());
        assert_eq!(flags.save_to(&path), SaveOutcome::KeptBroken);
        assert_eq!(std::fs::read_to_string(&path).expect("file kept"), broken);
        assert_eq!(std::fs::read_to_string(suffixed(&path, ".corrupt")).expect("copy kept"), broken);
        assert!(flags.take_notice().is_some_and(|notice| notice.contains(".corrupt")));

        assert_eq!(flags.save_to(&path), SaveOutcome::KeptBroken);
        assert!(flags.take_notice().is_none(), "the warning is given once");
        assert_eq!(std::fs::read_to_string(&path).expect("file kept"), broken);
        let _ = std::fs::remove_dir_all(path.parent().expect("scratch dir"));
    }

    #[test]
    fn save_goes_through_a_temp_file_and_keeps_other_characters() {
        let path = scratch_file("atomic");
        let previous =
            "(characters: [(character_id: 9, items: [(item_id: 909, refine: 0, cards: (0, 0, 0, 0), junk: true, locked: false)])])";
        std::fs::write(&path, previous).expect("write previous file");
        let mut flags = ItemFlags {
            character_id: 4,
            ..ItemFlags::default()
        };
        flags.set_locked(ItemKey::new(1201, 0, [0; 4]), true);

        assert_eq!(flags.save_to(&path), SaveOutcome::Written);
        assert!(!suffixed(&path, ".tmp").exists());
        assert!(!suffixed(&path, ".corrupt").exists());
        let text = std::fs::read_to_string(&path).expect("saved file");
        let mut loaded = ItemFlags::default();
        loaded.load_ron(4, &text);
        assert!(loaded.is_locked(ItemKey::new(1201, 0, [0; 4])));
        let mut other = ItemFlags::default();
        other.load_ron(9, &text);
        assert!(other.is_junk(ItemKey::new(909, 0, [0; 4])));
        let _ = std::fs::remove_dir_all(path.parent().expect("scratch dir"));
    }

    #[test]
    fn only_junk_and_lock_edits_are_saved_after_the_debounce() {
        let mut flags = ItemFlags::default();
        let pickup = ItemKey::new(512, 0, [0; 4]);
        flags.note_snapshot();
        flags.note_arrival(pickup);
        flags.note_hover_at(pickup, 1, String::new(), 0);
        flags.advance_to(NEW_GLOW_MS);
        assert_eq!(flags.unsaved_since(), None, "pickups and the glow fade do not save");

        let knife = ItemKey::new(1201, 0, [0; 4]);
        flags.set_locked(knife, true);
        let at = flags.unsaved_since().expect("lock waits to be saved");
        flags.set_junk(knife, true);
        assert_eq!(flags.unsaved_since(), Some(at), "a second edit does not push the save back");
        assert!(!flags.save_due(at.wrapping_add(SAVE_DEBOUNCE_MS - 1)));
        assert!(flags.save_due(at.wrapping_add(SAVE_DEBOUNCE_MS)));

        flags.flush();
        assert_eq!(flags.unsaved_since(), Some(at), "an unbound client never writes the file");

        flags.dirty_at = None;
        flags.set_locked(knife, true);
        assert_eq!(flags.unsaved_since(), None, "setting the same lock again is not an edit");
    }

    #[test]
    fn sell_junk_skips_only_the_equipped_arrow_stack() {
        let quiver = arrows(3, 200);
        let spare = arrows(4, 50);
        let mut flags = ItemFlags::default();
        flags.set_junk(ItemKey::from_inventory(&quiver), true);
        let stacks = [
            StackRef::from_item(&quiver, Some(InventoryIndex(3))),
            StackRef::from_item(&spare, Some(InventoryIndex(3))),
        ];
        assert!(stacks[0].equipped);
        assert!(!stacks[1].equipped);
        let (adds, messages) = plan_sell_junk(&flags, &stacks, &[(3, 200), (4, 50)]);
        assert_eq!(adds, vec![JunkAdd { index: 4, amount: 50 }]);
        assert!(messages.is_empty());

        assert!(!StackRef::from_item(&quiver, None).equipped);
        assert!(!is_worn_stack(&quiver, None));
    }
}
