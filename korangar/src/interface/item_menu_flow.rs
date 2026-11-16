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
use crate::interface::resource::ItemSource;
use crate::interface::windows::{ItemDetailsWindow, ItemMenuWindow, WindowClass, format_item_details, item_details_key};
use crate::state::inventory::{InventoryPathExt, item_activation};
use crate::state::inventory_view::{is_favorite, is_worn};
use crate::state::item_menu::{
    ItemMenuEffect, ItemMenuFacts, MenuOrigin, build_item_menu, effect_of, handle_extension, pending_link_event,
};
use crate::state::quick_move::{
    QUICK_MOVE_NO_TARGET, QUICK_MOVE_WORN, QuickMoveDecision, QuickMoveWindows, SlotSecondary, classify_secondary, decide_quick_move,
    item_is_locked,
};
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::{ChatMessage, ClientStatePathExt, client_state, client_theme};
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
        match classify_secondary(true, alt) {
            SlotSecondary::Ignore => {}
            SlotSecondary::QuickMove => self.quick_move_item(source, &item),
            SlotSecondary::OpenMenu => self.open_item_menu(source, &item),
        }
    }

    fn quick_move_item(&mut self, source: ItemSource, item: &InventoryItem<ResourceMetadata>) {
        let origin = menu_origin(source);
        let Some(quick_source) = origin.quick_source() else {
            let hint = if is_worn(item) { QUICK_MOVE_WORN } else { QUICK_MOVE_NO_TARGET };
            self.item_menu_hint(hint);
            return;
        };
        let decision = decide_quick_move(quick_source, self.transfer_windows(), item_is_locked(item), is_worn(item));
        match decision {
            QuickMoveDecision::Move(target) => self.offer_item_move(target, item),
            QuickMoveDecision::Mail => {
                self.input_event_buffer.push(InputEvent::MailAttachItem {
                    index: item.index.0,
                    amount: None,
                });
            }
            QuickMoveDecision::Hint(text) => self.item_menu_hint(text),
        }
    }

    fn open_item_menu(&mut self, source: ItemSource, item: &InventoryItem<ResourceMetadata>) {
        let origin = menu_origin(source);
        let worn = is_worn(item) || matches!(origin, MenuOrigin::Equipment { .. });
        let facts = ItemMenuFacts {
            origin,
            activation: item_activation(item),
            worn,
            favorite: is_favorite(item),
            locked: item_is_locked(item),
            windows: self.transfer_windows(),
            link_to_chat: pending_link_event(item.index.0).is_some(),
        };
        let entries = build_item_menu(&facts);
        let title = match item.metadata.name.trim() {
            "" => "Item".to_owned(),
            name => name.to_owned(),
        };
        self.client_state
            .follow_mut(client_state().item_menu())
            .open(title, origin, item.index.0, entries);
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
        let gold = *self.client_state.follow(client_theme().tokens().gold());
        let key = item_details_key(source_of(origin), &item);
        self.interface.open_or_raise_instance(key, ItemDetailsWindow {
            title: item.metadata.name.clone(),
            body: format_item_details(&item, gold),
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
