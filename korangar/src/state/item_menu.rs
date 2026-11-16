//! Right-click menu for an item slot.
//!
//! Rows follow Use or Equip, Details, Link to chat, Favorite, the open
//! transfer targets, then Drop. Link to chat stays hidden until G55's
//! `InputEvent::LinkItemInChat` exists ([`pending_link_event`]). Junk and
//! Lock stay hidden until G58 registers them in [`item_menu_extensions`].

use korangar_interface::element::StateElement;
use rust_state::RustState;

use super::inventory::ItemActivation;
use super::quick_move::{QuickMoveSource, QuickMoveWindows};
use super::vn_dialog::{ChoiceKeys, choice_key_outcome};
use crate::state::item_move::ItemMoveTarget;

/// One menu row. G58 adds [`Self::Junk`], [`Self::Lock`] and [`Self::Unlock`]
/// through [`item_menu_extensions`]; those variants are hidden while that
/// list is empty.
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
    /// G58 registers this through [`item_menu_extensions`]. Hidden until then.
    #[allow(dead_code)]
    Junk,
    /// G58 registers this through [`item_menu_extensions`]. Hidden until then.
    #[allow(dead_code)]
    Lock,
    /// G58 registers this through [`item_menu_extensions`]. Hidden until then.
    #[allow(dead_code)]
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
            Self::Junk => "Junk",
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
    Favorite { favorite: bool },
    Move(ItemMoveTarget),
    Mail,
    Drop,
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
        ItemMenuEntry::LinkToChat | ItemMenuEntry::Junk | ItemMenuEntry::Lock | ItemMenuEntry::Unlock => ItemMenuEffect::None,
    }
}

/// `Some("LinkItemInChat")` once G55's input event exists. `None` hides the
/// row.
pub fn pending_link_event(_index: u16) -> Option<&'static str> {
    None
}

/// Extra rows from features that are not in this tree yet.
///
/// G58 pushes [`ItemMenuEntry::Junk`] and [`ItemMenuEntry::Lock`] or
/// [`ItemMenuEntry::Unlock`] from here. An empty list hides those rows.
pub fn item_menu_extensions(_facts: &ItemMenuFacts) -> Vec<ItemMenuEntry> {
    Vec::new()
}

/// G58 handles Junk and Lock. `false` means the row did nothing.
pub fn handle_extension(entry: ItemMenuEntry, index: u16) -> bool {
    let _ = (entry, index);
    false
}

#[derive(Clone, Copy, Debug)]
pub struct ItemMenuFacts {
    pub origin: MenuOrigin,
    pub activation: ItemActivation,
    pub worn: bool,
    pub favorite: bool,
    pub locked: bool,
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

    pub fn entry_at(&self, index: usize) -> Option<ItemMenuEntry> {
        self.entries.get(index).map(|row| row.entry)
    }

    pub fn open(&mut self, title: String, origin: MenuOrigin, index: u16, entries: Vec<ItemMenuRow>) {
        self.open = true;
        self.title = title;
        self.origin = origin;
        self.index = index;
        self.entries = entries;
        self.focus = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.title.clear();
        self.entries.clear();
        self.focus = 0;
        self.index = 0;
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
        assert_eq!(labels(&rows), ["Use", "Details", "Favorite", "Drop"]);
        assert!(pending_link_event(1).is_none());
        assert!(
            item_menu_extensions(&facts(
                MenuOrigin::Inventory,
                ItemActivation::Use,
                false,
                false,
                false,
                QuickMoveWindows::default(),
            ))
            .is_empty()
        );
        assert!(!labels(&rows).contains(&"Link to chat"));
        assert!(!labels(&rows).contains(&"Junk"));
        assert!(!labels(&rows).contains(&"Lock"));
        assert_eq!(effect_of(rows[0].entry, MenuOrigin::Inventory), ItemMenuEffect::UseItem);
        assert_eq!(effect_of(rows[1].entry, MenuOrigin::Inventory), ItemMenuEffect::Details);
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
            "Unfavorite",
            "Move to storage",
            "Move to cart",
            "Move to trade",
            "Move to mail",
            "Drop",
        ]);
        let locked = build_item_menu(&facts(MenuOrigin::Inventory, ItemActivation::Use, false, false, true, windows));
        assert_eq!(labels(&locked), [
            "Use",
            "Details",
            "Favorite",
            "Move to storage",
            "Move to cart",
            "Drop"
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
        assert_eq!(labels(&worn), ["Unequip", "Details", "Favorite"]);
        let stored = build_item_menu(&facts(
            MenuOrigin::Storage,
            ItemActivation::Ignore,
            false,
            false,
            false,
            QuickMoveWindows::default(),
        ));
        assert_eq!(labels(&stored), ["Details", "Move to inventory"]);
        assert_eq!(
            effect_of(ItemMenuEntry::MoveToInventory, MenuOrigin::Storage),
            ItemMenuEffect::Move(crate::state::item_move::ItemMoveTarget::FromStorage)
        );
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
}
