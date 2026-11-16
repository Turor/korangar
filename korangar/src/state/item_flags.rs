//! Per-character new, junk, and lock marks.
//!
//! Inventory indexes change when the server moves a stack, so a mark is stored
//! on the item id, refine, and cards. The file is `client/item_flags.ron`, the
//! same kind of client config the hotbar and game settings use. Only the
//! active character's rows are loaded; other characters stay in the file.

use std::time::Instant;

use korangar_interface::element::StateElement;
use korangar_networking::{InventoryItem, InventoryItemDetails};
use ron::ser::PrettyConfig;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

use crate::world::ResourceMetadata;

/// How long a new-item glow takes to fade after the first hover.
pub const NEW_GLOW_MS: u32 = 300;
/// Peak opacity of [`NEW_GLOW_MS`] while the item is still new.
pub const NEW_GLOW_ALPHA: u8 = 160;

const FILE_NAME: &str = "client/item_flags.ron";

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
}

impl HoverNote {
    pub fn from_item(item: &InventoryItem<ResourceMetadata>) -> Self {
        Self {
            key: ItemKey::from_inventory(item),
            index: item.index.0,
            name: item.metadata.name.clone(),
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

    pub fn from_item(item: &InventoryItem<ResourceMetadata>) -> Self {
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
            equipped: is_equipped(&item.details),
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
}

#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub(crate) struct HoveredItem {
    index: u16,
    item_id: u32,
    refine: u8,
    cards: [u32; 4],
    name: String,
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
        let text = std::fs::read_to_string(FILE_NAME).unwrap_or_default();
        self.load_ron(character_id, &text);
        self.persist = true;
        self.snapshot = false;
        self.hover = None;
        self.hover_fresh = false;
    }

    /// Replace the in-memory rows with this character's section of `text`.
    pub fn load_ron(&mut self, character_id: u32, text: &str) {
        self.character_id = character_id;
        self.entries = parse_file(text)
            .characters
            .into_iter()
            .find(|character| character.character_id == character_id)
            .map(|character| character.items.into_iter().filter_map(StoredFlag::into_entry).collect())
            .unwrap_or_default();
    }

    /// File text with this character replaced and every other character kept.
    pub fn save_ron(&self, previous: &str) -> String {
        let mut file = parse_file(previous);
        file.characters.retain(|character| character.character_id != self.character_id);
        let items = self
            .entries
            .iter()
            .filter(|entry| entry.alive())
            .map(StoredFlag::from_entry)
            .collect::<Vec<_>>();
        if !items.is_empty() {
            file.characters.push(CharacterRecord {
                character_id: self.character_id,
                items,
            });
        }
        file.characters.sort_by_key(|character| character.character_id);
        ron::ser::to_string_pretty(&file, PrettyConfig::new()).unwrap_or_else(|_| "()".to_owned())
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
        self.persist_if_needed();
    }

    pub fn is_new(&self, key: ItemKey) -> bool {
        self.entry(key).is_some_and(|entry| entry.is_new)
    }

    pub fn is_junk(&self, key: ItemKey) -> bool {
        self.entry(key).is_some_and(|entry| entry.junk)
    }

    pub fn is_locked(&self, key: ItemKey) -> bool {
        self.entry(key).is_some_and(|entry| entry.locked)
    }

    pub fn set_junk(&mut self, key: ItemKey, junk: bool) {
        self.entry_mut(key).junk = junk;
        self.prune();
        self.persist_if_needed();
    }

    pub fn set_locked(&mut self, key: ItemKey, locked: bool) {
        self.entry_mut(key).locked = locked;
        self.prune();
        self.persist_if_needed();
    }

    pub fn refusal(&self, key: ItemKey, name: &str) -> Option<String> {
        self.is_locked(key).then(|| format!("{} is locked", item_label(name, key.item_id)))
    }

    /// Called once per input frame, before hotkeys. A grid that was not drawn
    /// last frame drops its hover, so J and L do nothing in the world.
    pub fn begin_input_frame(&mut self) {
        self.advance_to(elapsed_ms());
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

    pub fn note_hover(&mut self, key: ItemKey, index: u16, name: String) {
        self.note_hover_at(key, index, name, elapsed_ms());
    }

    pub fn note_hover_at(&mut self, key: ItemKey, index: u16, name: String, now: u32) {
        self.hover = Some(HoveredItem {
            index,
            item_id: key.item_id,
            refine: key.refine,
            cards: key.cards,
            name,
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
        let Some(key) = self.fresh_hover_key() else {
            return false;
        };
        let junk = !self.is_junk(key);
        self.set_junk(key, junk);
        true
    }

    pub fn toggle_hovered_lock(&mut self) -> bool {
        let Some(key) = self.fresh_hover_key() else {
            return false;
        };
        let locked = !self.is_locked(key);
        self.set_locked(key, locked);
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
            self.persist_if_needed();
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

    pub fn appearance(&self, key: ItemKey, now: u32) -> FlagAppearance {
        FlagAppearance {
            glow: self.glow_alpha_at(key, now),
            junk: self.is_junk(key),
            locked: self.is_locked(key),
        }
    }

    /// Glow, junk, and lock for the overlay, using the client clock.
    pub fn appearance_now(&self, key: ItemKey) -> FlagAppearance {
        self.appearance(key, elapsed_ms())
    }

    fn fresh_hover_key(&self) -> Option<ItemKey> {
        self.hover_fresh.then(|| self.hover.as_ref().map(HoveredItem::key)).flatten()
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

    fn persist_if_needed(&self) {
        if !self.persist {
            return;
        }
        let previous = std::fs::read_to_string(FILE_NAME).unwrap_or_default();
        let text = self.save_ron(&previous);
        let _ = std::fs::create_dir_all("client");
        let _ = std::fs::write(FILE_NAME, text);
    }
}

/// Applies a Junk / Lock pick from the item menu.
pub fn apply_toggle(flags: &mut ItemFlags, key: ItemKey, toggle: crate::state::item_menu::FlagToggle) {
    match toggle {
        crate::state::item_menu::FlagToggle::Junk(junk) => flags.set_junk(key, junk),
        crate::state::item_menu::FlagToggle::Lock(locked) => flags.set_locked(key, locked),
    }
}

/// Junk stacks the NPC will buy, minus locked and equipped rows.
/// Locked junk is named in the returned messages and is not added.
pub fn plan_sell_junk(flags: &ItemFlags, stacks: &[StackRef], sellable: &[(u16, u16)]) -> (Vec<JunkAdd>, Vec<String>) {
    let mut adds = Vec::new();
    let mut messages = Vec::new();
    for stack in stacks {
        if !flags.is_junk(stack.key()) {
            continue;
        }
        if stack.equipped {
            continue;
        }
        if flags.is_locked(stack.key()) {
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
    if let Some(message) = flags.refusal(stack.key(), &stack.name) {
        return FlagAttempt::refuse(message);
    }
    FlagAttempt::allow(FlagSend::Drop {
        index: stack.index,
        amount,
    })
}

pub fn attempt_trade(flags: &ItemFlags, stack: &StackRef, amount: u16) -> FlagAttempt {
    if let Some(message) = flags.refusal(stack.key(), &stack.name) {
        return FlagAttempt::refuse(message);
    }
    FlagAttempt::allow(FlagSend::Trade {
        index: stack.index,
        amount,
    })
}

pub fn attempt_mail(flags: &ItemFlags, stack: &StackRef, count: u16) -> FlagAttempt {
    if let Some(message) = flags.refusal(stack.key(), &stack.name) {
        return FlagAttempt::refuse(message);
    }
    FlagAttempt::allow(FlagSend::Mail { index: stack.index, count })
}

pub fn attempt_vend(flags: &ItemFlags, stack: &StackRef) -> FlagAttempt {
    if let Some(message) = flags.refusal(stack.key(), &stack.name) {
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
        if let Some(message) = flags.refusal(stack.key(), &stack.name) {
            messages.push(message);
        }
    }
    if !messages.is_empty() {
        return FlagAttempt { send: None, messages };
    }
    FlagAttempt::allow(FlagSend::Sell { lines: lines.to_vec() })
}

pub fn refusal_for_index(flags: &ItemFlags, items: &[InventoryItem<ResourceMetadata>], index: u16) -> Option<String> {
    let item = items.iter().find(|item| item.index.0 == index)?;
    flags.refusal(ItemKey::from_inventory(item), &item.metadata.name)
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredFlag {
    item_id: u32,
    refine: u8,
    cards: [u32; 4],
    junk: bool,
    locked: bool,
    #[serde(default)]
    is_new: bool,
}

impl StoredFlag {
    fn from_entry(entry: &ItemFlagEntry) -> Self {
        Self {
            item_id: entry.item_id,
            refine: entry.refine,
            cards: entry.cards,
            junk: entry.junk,
            locked: entry.locked,
            is_new: entry.is_new,
        }
    }

    fn into_entry(self) -> Option<ItemFlagEntry> {
        let entry = ItemFlagEntry {
            item_id: self.item_id,
            refine: self.refine,
            cards: self.cards,
            junk: self.junk,
            locked: self.locked,
            is_new: self.is_new,
            fade_from: None,
        };
        entry.alive().then_some(entry)
    }
}

fn parse_file(text: &str) -> FlagFile {
    if text.trim().is_empty() {
        return FlagFile::default();
    }
    ron::from_str(text).unwrap_or_default()
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
        let text = flags.save_ron(previous);

        let mut loaded = ItemFlags::default();
        loaded.load_ron(4, &text);
        assert!(loaded.is_junk(ItemKey::new(501, 0, [0; 4])));
        assert!(loaded.is_locked(ItemKey::new(1201, 0, [0; 4])));
        assert!(loaded.is_new(ItemKey::new(512, 0, [0; 4])));
        assert!(!loaded.is_junk(ItemKey::new(909, 0, [0; 4])));

        let mut other = ItemFlags::default();
        other.load_ron(9, &text);
        assert!(other.is_junk(ItemKey::new(909, 0, [0; 4])));
        assert!(!other.is_locked(ItemKey::new(1201, 0, [0; 4])));
    }
}
