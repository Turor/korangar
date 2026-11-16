//! Right-click menu for an item slot.
//!
//! Rows follow Use or Equip, Details, Link to chat, Favorite, the open
//! transfer targets, then Drop. Link to chat is shown because
//! [`crate::input::InputEvent::LinkItemInChat`] exists. Junk and Lock come
//! last, from [`item_menu_extensions`] (item flags, G58).

use korangar_interface::element::StateElement;
use rust_state::RustState;

use super::inventory::ItemActivation;
use super::quick_move::{QuickMoveSource, QuickMoveWindows};
use super::vn_dialog::{ChoiceKeys, choice_key_outcome};
use crate::state::item_move::ItemMoveTarget;

/// One menu row. Junk / Not junk and Lock / Unlock come from
/// [`item_menu_extensions`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemMenuEntry {
    Use,
    Equip,
    Unequip,
    InsertCard,
    Details,
    LinkToChat,
    Favorite,
    Unfavorite,
    MoveToStorage,
    MoveToCart,
    MoveToTrade,
    MoveToMail,
    MoveToInventory,
    Drop,
    Junk,
    Unjunk,
    Lock,
    Unlock,
}

impl ItemMenuEntry {
    pub fn label(self) -> &'static str {
        match self {
            Self::Use => "Use",
            Self::Equip => "Equip",
            Self::Unequip => "Unequip",
            Self::InsertCard => "Insert card",
            Self::Details => "Details",
            Self::LinkToChat => "Link to chat",
            Self::Favorite => "Favorite",
            Self::Unfavorite => "Unfavorite",
            Self::MoveToStorage => "Move to storage",
            Self::MoveToCart => "Move to cart",
            Self::MoveToTrade => "Move to trade",
            Self::MoveToMail => "Move to mail",
            Self::MoveToInventory => "Move to inventory",
            Self::Drop => "Drop",
            Self::Junk => "Mark junk",
            Self::Unjunk => "Not junk",
            Self::Lock => "Lock",
            Self::Unlock => "Unlock",
        }
    }
}

/// Where the slot lives. Equipment and hotbar keep enough to rebuild the
/// details-window key without storing an
/// [`crate::interface::resource::ItemSource`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuOrigin {
    #[default]
    Inventory,
    Storage,
    Cart,
    Equipment {
        position_bits: u32,
    },
    Hotbar {
        tab: u16,
        slot: u16,
    },
}

impl MenuOrigin {
    /// Alt+right-click target list. Gear slots are not a transfer source.
    pub fn quick_source(self) -> Option<QuickMoveSource> {
        match self {
            Self::Inventory | Self::Hotbar { .. } => Some(QuickMoveSource::Inventory),
            Self::Storage => Some(QuickMoveSource::Storage),
            Self::Cart => Some(QuickMoveSource::Cart),
            Self::Equipment { .. } => None,
        }
    }
}

/// What picking a row does. `None` is an extension that is not merged yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemMenuEffect {
    UseItem,
    Unequip,
    Details,
    Favorite {
        favorite: bool,
    },
    Move(ItemMoveTarget),
    Mail,
    Drop,
    /// Queue [`crate::input::InputEvent::LinkItemInChat`].
    Link,
    None,
}

pub fn effect_of(entry: ItemMenuEntry, origin: MenuOrigin) -> ItemMenuEffect {
    match entry {
        ItemMenuEntry::Use | ItemMenuEntry::Equip | ItemMenuEntry::InsertCard => ItemMenuEffect::UseItem,
        ItemMenuEntry::Unequip => ItemMenuEffect::Unequip,
        ItemMenuEntry::Details => ItemMenuEffect::Details,
        ItemMenuEntry::Favorite => ItemMenuEffect::Favorite { favorite: true },
        ItemMenuEntry::Unfavorite => ItemMenuEffect::Favorite { favorite: false },
        ItemMenuEntry::MoveToStorage => ItemMenuEffect::Move(ItemMoveTarget::ToStorage),
        ItemMenuEntry::MoveToCart => ItemMenuEffect::Move(ItemMoveTarget::ToCart),
        ItemMenuEntry::MoveToTrade => ItemMenuEffect::Move(ItemMoveTarget::ToTrade),
        ItemMenuEntry::MoveToMail => ItemMenuEffect::Mail,
        ItemMenuEntry::MoveToInventory => match origin {
            MenuOrigin::Storage => ItemMenuEffect::Move(ItemMoveTarget::FromStorage),
            MenuOrigin::Cart => ItemMenuEffect::Move(ItemMoveTarget::FromCart),
            _ => ItemMenuEffect::None,
        },
        ItemMenuEntry::Drop => ItemMenuEffect::Drop,
        ItemMenuEntry::LinkToChat => ItemMenuEffect::Link,
        ItemMenuEntry::Junk | ItemMenuEntry::Unjunk | ItemMenuEntry::Lock | ItemMenuEntry::Unlock => ItemMenuEffect::None,
    }
}

/// `Some` while [`crate::input::InputEvent::LinkItemInChat`] is in this tree.
/// `None` hides the row.
pub fn pending_link_event(_index: u16) -> Option<&'static str> {
    Some("LinkItemInChat")
}

/// Extra rows from features that are not in this tree yet.
///
/// Junk and lock rows for stacks the player carries (inventory, hotbar,
/// equipment). Storage and cart rows get none.
pub fn item_menu_extensions(facts: &ItemMenuFacts) -> Vec<ItemMenuEntry> {
    if matches!(facts.origin, MenuOrigin::Storage | MenuOrigin::Cart) {
        return Vec::new();
    }
    vec![
        if facts.junk { ItemMenuEntry::Unjunk } else { ItemMenuEntry::Junk },
        if facts.locked { ItemMenuEntry::Unlock } else { ItemMenuEntry::Lock },
    ]
}

/// A junk or lock row's new flag value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlagToggle {
    Junk(bool),
    Lock(bool),
}

/// The flag change for a junk / lock row; `None` for every other row.
pub fn extension_toggle(entry: ItemMenuEntry) -> Option<FlagToggle> {
    match entry {
        ItemMenuEntry::Junk => Some(FlagToggle::Junk(true)),
        ItemMenuEntry::Unjunk => Some(FlagToggle::Junk(false)),
        ItemMenuEntry::Lock => Some(FlagToggle::Lock(true)),
        ItemMenuEntry::Unlock => Some(FlagToggle::Lock(false)),
        _ => None,
    }
}

/// Local junk / lock flags of the clicked stack (`state::item_flags`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemSlotFlags {
    pub junk: bool,
    pub locked: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ItemMenuFacts {
    pub origin: MenuOrigin,
    pub activation: ItemActivation,
    pub worn: bool,
    pub favorite: bool,
    pub locked: bool,
    pub junk: bool,
    pub windows: QuickMoveWindows,
    /// True only when [`pending_link_event`] returns a name.
    pub link_to_chat: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct ItemMenuRow {
    pub label: String,
    #[hidden_element]
    pub entry: ItemMenuEntry,
}

fn row(entry: ItemMenuEntry) -> ItemMenuRow {
    ItemMenuRow {
        label: entry.label().to_owned(),
        entry,
    }
}

pub fn build_item_menu(facts: &ItemMenuFacts) -> Vec<ItemMenuRow> {
    let mut entries = Vec::new();
    let in_bag = matches!(facts.origin, MenuOrigin::Inventory | MenuOrigin::Hotbar { .. });
    if in_bag || matches!(facts.origin, MenuOrigin::Equipment { .. }) {
        match facts.activation {
            ItemActivation::Use => entries.push(ItemMenuEntry::Use),
            ItemActivation::Equip { .. } if facts.worn => entries.push(ItemMenuEntry::Unequip),
            ItemActivation::Equip { .. } => entries.push(ItemMenuEntry::Equip),
            ItemActivation::InsertCard => entries.push(ItemMenuEntry::InsertCard),
            ItemActivation::Ignore => {}
        }
        if facts.worn && !entries.contains(&ItemMenuEntry::Unequip) {
            entries.push(ItemMenuEntry::Unequip);
        }
    }

    entries.push(ItemMenuEntry::Details);
    if facts.link_to_chat {
        entries.push(ItemMenuEntry::LinkToChat);
    }
    if in_bag || matches!(facts.origin, MenuOrigin::Equipment { .. }) {
        entries.push(if facts.favorite {
            ItemMenuEntry::Unfavorite
        } else {
            ItemMenuEntry::Favorite
        });
    }

    if !facts.worn {
        match facts.origin {
            MenuOrigin::Inventory | MenuOrigin::Hotbar { .. } => {
                if facts.windows.storage {
                    entries.push(ItemMenuEntry::MoveToStorage);
                }
                if facts.windows.cart {
                    entries.push(ItemMenuEntry::MoveToCart);
                }
                if facts.windows.trade && !facts.locked {
                    entries.push(ItemMenuEntry::MoveToTrade);
                }
                if facts.windows.mail && !facts.locked {
                    entries.push(ItemMenuEntry::MoveToMail);
                }
            }
            MenuOrigin::Storage | MenuOrigin::Cart => entries.push(ItemMenuEntry::MoveToInventory),
            MenuOrigin::Equipment { .. } => {}
        }
    }

    if facts.origin == MenuOrigin::Inventory && !facts.worn {
        entries.push(ItemMenuEntry::Drop);
    }

    entries.extend(item_menu_extensions(facts));
    entries.into_iter().map(row).collect()
}

/// A pick still targets the row the menu opened on: same item id at that
/// index. A menu opened without an id (tests) is trusted.
pub fn menu_target_current(stored: Option<u32>, current: Option<u32>) -> bool {
    match stored {
        Some(stored) => current == Some(stored),
        None => current.is_some(),
    }
}

/// Popup state. `alt_held` is the modifier for the click that is about to land.
#[derive(Clone, Default, RustState, StateElement)]
pub struct ItemMenuState {
    #[hidden_element]
    open: bool,
    pub title: String,
    pub entries: Vec<ItemMenuRow>,
    pub focus: usize,
    #[hidden_element]
    origin: MenuOrigin,
    #[hidden_element]
    index: u16,
    /// Item id of the row the menu opened on, re-checked at pick time.
    #[hidden_element]
    item_id: Option<u32>,
    /// Alt is down. The item slot reads this when it handles right-click.
    pub alt_held: bool,
    #[hidden_element]
    cursor_x: f32,
    #[hidden_element]
    cursor_y: f32,
}

impl ItemMenuState {
    pub fn note_pointer(&mut self, x: f32, y: f32, alt: bool) {
        self.cursor_x = x;
        self.cursor_y = y;
        self.alt_held = alt;
    }

    pub fn cursor(&self) -> (f32, f32) {
        (self.cursor_x, self.cursor_y)
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn origin(&self) -> MenuOrigin {
        self.origin
    }

    pub fn index(&self) -> u16 {
        self.index
    }

    pub fn item_id(&self) -> Option<u32> {
        self.item_id
    }

    pub fn set_item_id(&mut self, item_id: u32) {
        self.item_id = Some(item_id);
    }

    pub fn entry_at(&self, index: usize) -> Option<ItemMenuEntry> {
        self.entries.get(index).map(|row| row.entry)
    }

    pub fn open(&mut self, title: String, origin: MenuOrigin, index: u16, entries: Vec<ItemMenuRow>) {
        self.open = true;
        self.title = title;
        self.origin = origin;
        self.index = index;
        self.item_id = None;
        self.entries = entries;
        self.focus = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.title.clear();
        self.entries.clear();
        self.focus = 0;
        self.index = 0;
        self.item_id = None;
        self.origin = MenuOrigin::Inventory;
    }

    /// Up/Down/Enter, plus the digit keys, matching NPC dialog menus.
    /// `Some` is the row to run.
    pub fn apply_keys(&mut self, keys: ChoiceKeys) -> Option<usize> {
        if !self.open || self.entries.is_empty() || !keys.any() {
            return None;
        }
        let (focus, pick) = choice_key_outcome(self.focus, self.entries.len(), keys);
        self.focus = focus;
        pick
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::EquipPosition;

    use super::{
        ItemMenuEntry, ItemMenuFacts, ItemMenuState, MenuOrigin, build_item_menu, effect_of, item_menu_extensions, pending_link_event,
    };
    use crate::state::inventory::ItemActivation;
    use crate::state::item_menu::ItemMenuEffect;
    use crate::state::quick_move::QuickMoveWindows;
    use crate::state::vn_dialog::ChoiceKeys;

    fn facts(
        origin: MenuOrigin,
        activation: ItemActivation,
        worn: bool,
        favorite: bool,
        locked: bool,
        windows: QuickMoveWindows,
    ) -> ItemMenuFacts {
        ItemMenuFacts {
            origin,
            activation,
            worn,
            favorite,
            locked,
            junk: false,
            windows,
            link_to_chat: pending_link_event(0).is_some(),
        }
    }

    fn labels(rows: &[super::ItemMenuRow]) -> Vec<&str> {
        rows.iter().map(|row| row.label.as_str()).collect()
    }

    #[test]
    fn potion_menu_is_use_details_favorite_and_drop() {
        let rows = build_item_menu(&facts(
            MenuOrigin::Inventory,
            ItemActivation::Use,
            false,
            false,
            false,
            QuickMoveWindows::default(),
        ));
        assert_eq!(labels(&rows), [
            "Use",
            "Details",
            "Link to chat",
            "Favorite",
            "Drop",
            "Mark junk",
            "Lock"
        ]);
        assert_eq!(pending_link_event(1), Some("LinkItemInChat"));
        assert_eq!(
            item_menu_extensions(&facts(
                MenuOrigin::Inventory,
                ItemActivation::Use,
                false,
                false,
                false,
                QuickMoveWindows::default(),
            )),
            [ItemMenuEntry::Junk, ItemMenuEntry::Lock]
        );
        assert_eq!(effect_of(rows[0].entry, MenuOrigin::Inventory), ItemMenuEffect::UseItem);
        assert_eq!(effect_of(rows[1].entry, MenuOrigin::Inventory), ItemMenuEffect::Details);
        assert_eq!(effect_of(rows[2].entry, MenuOrigin::Inventory), ItemMenuEffect::Link);
    }

    #[test]
    fn open_targets_are_the_only_move_rows_and_locks_hide_trade_and_mail() {
        let windows = QuickMoveWindows {
            storage: true,
            cart: true,
            trade: true,
            mail: true,
        };
        let rows = build_item_menu(&facts(MenuOrigin::Inventory, ItemActivation::Use, false, true, false, windows));
        assert_eq!(labels(&rows), [
            "Use",
            "Details",
            "Link to chat",
            "Unfavorite",
            "Move to storage",
            "Move to cart",
            "Move to trade",
            "Move to mail",
            "Drop",
            "Mark junk",
            "Lock",
        ]);
        let locked = build_item_menu(&facts(MenuOrigin::Inventory, ItemActivation::Use, false, false, true, windows));
        assert_eq!(labels(&locked), [
            "Use",
            "Details",
            "Link to chat",
            "Favorite",
            "Move to storage",
            "Move to cart",
            "Drop",
            "Mark junk",
            "Unlock",
        ]);
    }

    #[test]
    fn worn_gear_is_unequip_and_details_and_storage_goes_back() {
        let worn = build_item_menu(&facts(
            MenuOrigin::Equipment { position_bits: 2 },
            ItemActivation::Equip {
                position: EquipPosition::RIGHT_HAND,
            },
            true,
            false,
            false,
            QuickMoveWindows {
                storage: true,
                ..QuickMoveWindows::default()
            },
        ));
        assert_eq!(labels(&worn), [
            "Unequip",
            "Details",
            "Link to chat",
            "Favorite",
            "Mark junk",
            "Lock"
        ]);
        let stored = build_item_menu(&facts(
            MenuOrigin::Storage,
            ItemActivation::Ignore,
            false,
            false,
            false,
            QuickMoveWindows::default(),
        ));
        assert_eq!(labels(&stored), ["Details", "Link to chat", "Move to inventory"]);
        assert_eq!(
            effect_of(ItemMenuEntry::MoveToInventory, MenuOrigin::Storage),
            ItemMenuEffect::Move(crate::state::item_move::ItemMoveTarget::FromStorage)
        );
    }

    #[test]
    fn junk_and_lock_rows_toggle_the_flags() {
        let mut junk_facts = facts(
            MenuOrigin::Inventory,
            ItemActivation::Use,
            false,
            false,
            true,
            QuickMoveWindows::default(),
        );
        junk_facts.junk = true;
        assert_eq!(item_menu_extensions(&junk_facts), [
            ItemMenuEntry::Unjunk,
            ItemMenuEntry::Unlock
        ]);
        assert_eq!(labels(&build_item_menu(&junk_facts))[5..], ["Not junk", "Unlock"]);
        assert_eq!(
            super::extension_toggle(ItemMenuEntry::Junk),
            Some(super::FlagToggle::Junk(true))
        );
        assert_eq!(
            super::extension_toggle(ItemMenuEntry::Unjunk),
            Some(super::FlagToggle::Junk(false))
        );
        assert_eq!(
            super::extension_toggle(ItemMenuEntry::Lock),
            Some(super::FlagToggle::Lock(true))
        );
        assert_eq!(
            super::extension_toggle(ItemMenuEntry::Unlock),
            Some(super::FlagToggle::Lock(false))
        );
        assert_eq!(super::extension_toggle(ItemMenuEntry::Drop), None);
        let cart = facts(
            MenuOrigin::Cart,
            ItemActivation::Ignore,
            false,
            false,
            false,
            QuickMoveWindows::default(),
        );
        assert!(item_menu_extensions(&cart).is_empty());
    }

    #[test]
    fn arrow_down_and_enter_pick_the_next_row() {
        let rows = build_item_menu(&facts(
            MenuOrigin::Inventory,
            ItemActivation::Use,
            false,
            false,
            false,
            QuickMoveWindows::default(),
        ));
        let mut menu = ItemMenuState::default();
        menu.open("Red Potion".to_owned(), MenuOrigin::Inventory, 2, rows);
        assert!(
            menu.apply_keys(ChoiceKeys {
                next: true,
                ..ChoiceKeys::default()
            })
            .is_none()
        );
        assert_eq!(menu.focus, 1);
        assert_eq!(
            menu.apply_keys(ChoiceKeys {
                confirm: true,
                ..ChoiceKeys::default()
            }),
            Some(1)
        );
        assert_eq!(menu.entry_at(1), Some(ItemMenuEntry::Details));
        menu.close();
        assert!(!menu.is_open());
        assert!(
            menu.apply_keys(ChoiceKeys {
                confirm: true,
                ..ChoiceKeys::default()
            })
            .is_none()
        );
    }

    #[test]
    fn a_pick_on_a_reused_slot_is_dropped() {
        use super::menu_target_current;
        assert!(menu_target_current(Some(501), Some(501)));
        assert!(!menu_target_current(Some(501), Some(909)));
        assert!(!menu_target_current(Some(501), None));
        assert!(!menu_target_current(None, None));
    }
}
