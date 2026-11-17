//! New-item glow, junk, and lock. `lib.rs` forwards the network and input
//! events here. Storage, cart, and equip stay allowed for a locked stack.

use korangar_networking::{InventoryItem, MessageColor, SellItem};
use ragnarok_packets::{InventoryIndex, SoldItemInformation};

use crate::Client;
use crate::interface::windows::{ItemDropAmountBox, ItemDropWindow, WindowClass};
use crate::state::inventory::{GroundDrop, InventoryPathExt, ground_drop};
use crate::state::item_flags::{self, FlagSend, ItemKey, JunkAdd, StackRef};
use crate::state::item_menu::{FlagToggle, ItemSlotFlags};
use crate::state::{ChatMessage, ClientStatePathExt, client_state};
use crate::world::ResourceMetadata;

impl Client {
    /// Junk / lock flags of a stack, for the right-click menu.
    pub(crate) fn slot_flags<Meta>(&self, item: &InventoryItem<Meta>) -> ItemSlotFlags {
        let flags = self.client_state.follow(client_state().item_flags());
        let key = ItemKey::from_inventory(item);
        ItemSlotFlags {
            junk: flags.is_junk(key),
            locked: flags.is_locked(key),
        }
    }

    /// A Junk / Lock row picked from the item menu. The row is re-read by
    /// index; a stack that is gone is left alone.
    pub(crate) fn apply_menu_flag(&mut self, index: InventoryIndex, toggle: FlagToggle) {
        let Some(key) = self
            .client_state
            .follow(client_state().inventory().items())
            .iter()
            .find(|item| item.index == index)
            .map(ItemKey::from_inventory)
        else {
            return;
        };
        item_flags::apply_toggle(self.client_state.follow_mut(client_state().item_flags()), key, toggle);
    }

    pub(crate) fn bind_item_flags(&mut self, character_id: u32) {
        self.client_state
            .follow_mut(client_state().item_flags())
            .bind_character(character_id);
    }

    pub(crate) fn note_inventory_snapshot(&mut self) {
        self.client_state.follow_mut(client_state().item_flags()).note_snapshot();
    }

    /// Pickup, cart add, or storage take-out. Ignored until the login list.
    pub(crate) fn note_item_arrival(&mut self, key: ItemKey) {
        self.client_state.follow_mut(client_state().item_flags()).note_arrival(key);
    }

    pub(crate) fn begin_item_flag_frame(&mut self) {
        let notice = {
            let flags = self.client_state.follow_mut(client_state().item_flags());
            flags.begin_input_frame();
            flags.take_notice()
        };
        if let Some(notice) = notice {
            self.item_flag_notice(notice);
        }
    }

    /// Save pending junk and lock edits. Logout and exit call this.
    pub(crate) fn flush_item_flags(&mut self) {
        self.client_state.follow_mut(client_state().item_flags()).flush();
    }

    pub(crate) fn on_toggle_hovered_junk(&mut self) {
        self.client_state.follow_mut(client_state().item_flags()).toggle_hovered_junk();
    }

    pub(crate) fn on_toggle_hovered_lock(&mut self) {
        self.client_state.follow_mut(client_state().item_flags()).toggle_hovered_lock();
    }

    pub(crate) fn item_flag_notice(&mut self, text: String) {
        self.client_state
            .follow_mut(client_state().chat_messages())
            .push(ChatMessage::new(text, MessageColor::Error));
    }

    /// `Some` when this inventory index is locked, with the chat line.
    pub(crate) fn locked_inventory_message(&self, index: u16) -> Option<String> {
        let flags = self.client_state.follow(client_state().item_flags());
        let items = self.client_state.follow(client_state().inventory().items());
        item_flags::refusal_for_index(flags, items, index)
    }

    pub(crate) fn on_sell_junk(&mut self) {
        let stacks = self.inventory_stacks();
        let sellable = self.sellable_lines();
        let (adds, messages) = {
            let flags = self.client_state.follow(client_state().item_flags());
            item_flags::plan_sell_junk(flags, &stacks, &sellable)
        };
        for message in messages {
            self.item_flag_notice(message);
        }
        if adds.is_empty() {
            return;
        }
        let catalog = self.client_state.follow(client_state().sell_items()).clone();
        let cart = self.client_state.follow_mut(client_state().sell_cart());
        for add in adds {
            apply_junk_add(cart, &catalog, add);
        }
    }

    pub(crate) fn on_drop_item(&mut self, item: InventoryItem<ResourceMetadata>) {
        let equipped_ammo = self.client_state.follow(client_state().inventory()).equipped_ammo();
        let stack = StackRef::from_item(&item, equipped_ammo);
        let attempt = {
            let flags = self.client_state.follow(client_state().item_flags());
            item_flags::attempt_drop(flags, &stack, stack.amount)
        };
        if attempt.send.is_none() {
            for message in attempt.messages {
                self.item_flag_notice(message);
            }
            return;
        }
        match ground_drop(&item) {
            Some(GroundDrop::Now(amount)) => {
                let _ = self.networking_system.drop_item(item.index, amount);
            }
            Some(GroundDrop::Ask { max }) => {
                self.client_state
                    .follow_mut(client_state().item_drop())
                    .begin(item.index.0, max, &item.metadata.name);
                if !self.interface.is_window_with_class_open(WindowClass::ItemDrop) {
                    self.interface.open_window(ItemDropWindow::new());
                }
                self.interface.request_focus(ItemDropAmountBox);
            }
            None => {}
        }
    }

    pub(crate) fn on_confirm_item_drop(&mut self) {
        let (index, amount, message) = {
            let prompt = self.client_state.follow(client_state().item_drop());
            (prompt.index(), prompt.amount(), prompt.invalid_amount_message())
        };
        let Some(amount) = amount else {
            self.item_flag_notice(message);
            return;
        };
        if let Some(text) = self.locked_inventory_message(index) {
            self.item_flag_notice(text);
            return;
        }
        let _ = self.networking_system.drop_item(InventoryIndex(index), amount);
        self.interface.close_window_with_class(WindowClass::ItemDrop);
    }

    pub(crate) fn on_sell_items(&mut self, items: Vec<SoldItemInformation>) {
        let lines = items.iter().map(|item| (item.inventory_index.0, item.amount)).collect::<Vec<_>>();
        let stacks = self.inventory_stacks();
        let attempt = {
            let flags = self.client_state.follow(client_state().item_flags());
            item_flags::attempt_sell(flags, &stacks, &lines)
        };
        match attempt.send {
            Some(FlagSend::Sell { .. }) => {
                let _ = self.networking_system.sell_items(items);
            }
            _ => {
                for message in attempt.messages {
                    self.item_flag_notice(message);
                }
            }
        }
    }

    fn inventory_stacks(&self) -> Vec<StackRef> {
        let equipped_ammo = self.client_state.follow(client_state().inventory()).equipped_ammo();
        self.client_state
            .follow(client_state().inventory().items())
            .iter()
            .map(|item| StackRef::from_item(item, equipped_ammo))
            .collect()
    }

    fn sellable_lines(&self) -> Vec<(u16, u16)> {
        self.client_state
            .follow(client_state().sell_items())
            .iter()
            .map(|item| (item.inventory_index.0, item.metadata.1))
            .collect()
    }
}

/// Put the whole sellable amount on the cart. A line already there is replaced,
/// not added on top.
fn apply_junk_add(cart: &mut Vec<SellItem<(ResourceMetadata, u16)>>, catalog: &[SellItem<(ResourceMetadata, u16)>], add: JunkAdd) {
    let Some(source) = catalog.iter().find(|item| item.inventory_index.0 == add.index) else {
        return;
    };
    if let Some(line) = cart.iter_mut().find(|item| item.inventory_index.0 == add.index) {
        line.metadata.1 = add.amount;
        return;
    }
    let mut line = source.clone();
    line.metadata.1 = add.amount;
    cart.push(line);
}
