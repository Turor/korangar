//! Right-click menu and Alt+right-click quick-move.
//!
//! Amounts reuse [`super::item_move_flow`]. Mail stacks use the existing rodex
//! amount window (`MailAttachItem { amount: None }`), not a new move target.

use korangar_interface::window::{Anchor, AnchorPoint};
use korangar_networking::{InventoryItem, MessageColor};
use ragnarok_packets::{EquipPosition, HotbarSlot, HotbarTab, InventoryIndex};

use crate::Client;
use crate::graphics::{ScreenPosition, ScreenSize};
use crate::input::InputEvent;
use crate::interface::components::item_tooltip::{inventory_tooltips, pinned_details_body};
use crate::interface::resource::ItemSource;
use crate::interface::windows::{ItemMenuWindow, WindowClass, item_details_key};
use crate::state::inventory::{InventoryPathExt, item_activation};
use crate::state::inventory_view::{is_favorite, is_worn};
use crate::state::item_menu::{
    ItemMenuEffect, ItemMenuFacts, ItemMenuRow, MenuOrigin, build_item_menu, effect_of, handle_extension, pending_link_event,
};
use crate::state::item_move::ItemMoveTarget;
use crate::state::quick_move::{
    QUICK_MOVE_NO_TARGET, QUICK_MOVE_WORN, QuickMoveDecision, QuickMoveWindows, SlotSecondary, classify_secondary, decide_quick_move,
    item_is_locked,
};
use crate::state::{ChatMessage, ClientStatePathExt, client_state};
use crate::world::ResourceMetadata;

fn menu_origin(source: ItemSource) -> MenuOrigin {
    match source {
        ItemSource::Inventory => MenuOrigin::Inventory,
        ItemSource::Storage => MenuOrigin::Storage,
        ItemSource::Cart => MenuOrigin::Cart,
        ItemSource::Equipment { position } => MenuOrigin::Equipment {
            position_bits: position.bits(),
        },
        ItemSource::Hotbar { tab, slot } => MenuOrigin::Hotbar { tab: tab.0, slot: slot.0 },
    }
}

/// What Alt+right-click or a plain right-click should do.
/// [`Client::on_item_slot_secondary`] and the offline fixture both apply this.
pub(crate) enum SlotReaction {
    Ignored,
    Hint(&'static str),
    Move(ItemMoveTarget),
    Mail {
        index: u16,
    },
    OpenMenu {
        title: String,
        origin: MenuOrigin,
        index: u16,
        rows: Vec<ItemMenuRow>,
    },
}

pub(crate) fn react_to_secondary<Meta>(
    source: ItemSource,
    item: &InventoryItem<Meta>,
    name: &str,
    alt: bool,
    windows: QuickMoveWindows,
) -> SlotReaction {
    let origin = menu_origin(source);
    match classify_secondary(true, alt) {
        SlotSecondary::Ignore => SlotReaction::Ignored,
        SlotSecondary::QuickMove => {
            let Some(quick_source) = origin.quick_source() else {
                let hint = if is_worn(item) { QUICK_MOVE_WORN } else { QUICK_MOVE_NO_TARGET };
                return SlotReaction::Hint(hint);
            };
            match decide_quick_move(quick_source, windows, item_is_locked(item), is_worn(item)) {
                QuickMoveDecision::Move(target) => SlotReaction::Move(target),
                QuickMoveDecision::Mail => SlotReaction::Mail { index: item.index.0 },
                QuickMoveDecision::Hint(text) => SlotReaction::Hint(text),
            }
        }
        SlotSecondary::OpenMenu => {
            let worn = is_worn(item) || matches!(origin, MenuOrigin::Equipment { .. });
            let facts = ItemMenuFacts {
                origin,
                activation: item_activation(item),
                worn,
                favorite: is_favorite(item),
                locked: item_is_locked(item),
                windows,
                link_to_chat: pending_link_event(item.index.0).is_some(),
            };
            let title = match name.trim() {
                "" => "Item".to_owned(),
                name => name.to_owned(),
            };
            SlotReaction::OpenMenu {
                title,
                origin,
                index: item.index.0,
                rows: build_item_menu(&facts),
            }
        }
    }
}

fn source_of(origin: MenuOrigin) -> ItemSource {
    match origin {
        MenuOrigin::Inventory => ItemSource::Inventory,
        MenuOrigin::Storage => ItemSource::Storage,
        MenuOrigin::Cart => ItemSource::Cart,
        MenuOrigin::Equipment { position_bits } => ItemSource::Equipment {
            position: EquipPosition::from_bits_truncate(position_bits),
        },
        MenuOrigin::Hotbar { tab, slot } => ItemSource::Hotbar {
            tab: HotbarTab(tab),
            slot: HotbarSlot(slot),
        },
    }
}

impl Client {
    pub(crate) fn transfer_windows(&self) -> QuickMoveWindows {
        QuickMoveWindows {
            storage: self.interface.is_window_with_class_open(WindowClass::Storage),
            cart: self.interface.is_window_with_class_open(WindowClass::Cart),
            trade: self.interface.is_window_with_class_open(WindowClass::Trade),
            mail: self.interface.is_window_with_class_open(WindowClass::MailCompose),
        }
    }

    fn item_for_slot(&self, source: ItemSource, index: InventoryIndex) -> Option<InventoryItem<ResourceMetadata>> {
        let items = match source {
            ItemSource::Storage => self.client_state.follow(client_state().storage().items()),
            ItemSource::Cart => self.client_state.follow(client_state().cart().items()),
            ItemSource::Inventory | ItemSource::Equipment { .. } | ItemSource::Hotbar { .. } => {
                self.client_state.follow(client_state().inventory().items())
            }
        };
        items.iter().find(|item| item.index == index).cloned()
    }

    fn item_menu_hint(&mut self, text: &str) {
        self.client_state
            .follow_mut(client_state().chat_messages())
            .push(ChatMessage::new(text.to_owned(), MessageColor::Information));
    }

    /// Right-click on a filled slot. `alt` selects quick-move.
    pub(crate) fn on_item_slot_secondary(&mut self, source: ItemSource, index: InventoryIndex, alt: bool) {
        let Some(item) = self.item_for_slot(source, index) else {
            return;
        };
        let reaction = react_to_secondary(source, &item, &item.metadata.name, alt, self.transfer_windows());
        match reaction {
            SlotReaction::Ignored => {}
            SlotReaction::Hint(text) => self.item_menu_hint(text),
            SlotReaction::Move(target) => self.offer_item_move(target, &item),
            SlotReaction::Mail { index } => {
                self.input_event_buffer.push(InputEvent::MailAttachItem { index, amount: None });
            }
            SlotReaction::OpenMenu {
                title,
                origin,
                index,
                rows,
            } => self.present_item_menu(title, origin, index, rows),
        }
    }

    fn present_item_menu(&mut self, title: String, origin: MenuOrigin, index: u16, rows: Vec<ItemMenuRow>) {
        self.client_state
            .follow_mut(client_state().item_menu())
            .open(title, origin, index, rows);
        if !self.interface.is_window_with_class_open(WindowClass::ItemMenu) {
            self.interface.open_window(ItemMenuWindow::new());
        }
        let (left, top) = self.client_state.follow(client_state().item_menu()).cursor();
        self.interface.redock_window_class(
            WindowClass::ItemMenu,
            Anchor::at(AnchorPoint::TopLeft, ScreenPosition { left, top }),
            ScreenSize {
                width: 200.0,
                height: f32::MAX,
            },
        );
        self.interface.unfocus();
    }

    pub(crate) fn pick_item_menu(&mut self, index: usize) {
        let (entry, origin, item_index) = {
            let menu = self.client_state.follow(client_state().item_menu());
            let Some(entry) = menu.entry_at(index) else {
                return;
            };
            (entry, menu.origin(), menu.index())
        };
        if handle_extension(entry, item_index) {
            self.dismiss_item_menu();
            return;
        }
        let effect = effect_of(entry, origin);
        self.dismiss_item_menu();
        let index = InventoryIndex(item_index);
        match effect {
            ItemMenuEffect::UseItem => self.input_event_buffer.push(InputEvent::UseItem { index }),
            ItemMenuEffect::Unequip => {
                let _ = self.networking_system.request_item_unequip(index);
            }
            ItemMenuEffect::Details => self.open_menu_details(origin, index),
            ItemMenuEffect::Favorite { favorite } => {
                self.input_event_buffer.push(InputEvent::SetItemFavorite { index, favorite });
            }
            ItemMenuEffect::Move(target) => {
                if let Some(item) = self.item_for_slot(source_of(origin), index) {
                    self.offer_item_move(target, &item);
                }
            }
            ItemMenuEffect::Mail => {
                self.input_event_buffer.push(InputEvent::MailAttachItem {
                    index: item_index,
                    amount: None,
                });
            }
            ItemMenuEffect::Link => {
                if let Some(item) = self.item_for_slot(source_of(origin), index) {
                    self.input_event_buffer.push(InputEvent::LinkItemInChat { item });
                }
            }
            ItemMenuEffect::Drop => {
                if let Some(item) = self.item_for_slot(source_of(origin), index) {
                    self.input_event_buffer.push(InputEvent::DropItem { item });
                }
            }
            ItemMenuEffect::None => {}
        }
    }

    fn open_menu_details(&mut self, origin: MenuOrigin, index: InventoryIndex) {
        let Some(item) = self.item_for_slot(source_of(origin), index) else {
            return;
        };
        let source = source_of(origin);
        let body = pinned_details_body(&inventory_tooltips(&self.client_state, &item, source));
        self.input_event_buffer.push(InputEvent::OpenItemDetails {
            key: item_details_key(source, &item),
            title: item.metadata.name.clone(),
            body,
        });
    }

    pub(crate) fn dismiss_item_menu(&mut self) {
        self.client_state.follow_mut(client_state().item_menu()).close();
        self.interface.close_window_with_class(WindowClass::ItemMenu);
    }

    /// Title-bar and Escape close the window without telling the menu. The
    /// next frame drops the rows, and a closed menu drops a leftover window.
    pub(crate) fn sync_item_menu(&mut self) {
        let open = self.client_state.follow(client_state().item_menu()).is_open();
        let window_open = self.interface.is_window_with_class_open(WindowClass::ItemMenu);
        if open && !window_open {
            self.client_state.follow_mut(client_state().item_menu()).close();
        } else if window_open && !open {
            self.interface.close_window_with_class(WindowClass::ItemMenu);
        }
    }
}
