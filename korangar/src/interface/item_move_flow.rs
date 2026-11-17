//! Storage, cart and trade moves with roBrowser's amount prompt. `lib.rs`
//! forwards the drops and the prompt's buttons here; the model is
//! `state::item_move`.

use korangar_networking::{InventoryItem, InventoryItemDetails, MessageColor};
use ragnarok_packets::InventoryIndex;

use crate::Client;
use crate::input::InputEvent;
use crate::interface::windows::{ItemMoveAmountBox, ItemMoveAmountWindow, WindowClass};
use crate::state::inventory::InventoryPathExt;
use crate::state::item_move::{ItemMoveStep, ItemMoveTarget};
use crate::state::{ChatMessage, ClientStatePathExt, client_state};
use crate::world::ResourceMetadata;

/// Stack size the drop offers; equipment is always 1.
pub(crate) fn stack_amount<Meta>(item: &InventoryItem<Meta>) -> u16 {
    match &item.details {
        InventoryItemDetails::Regular { amount, .. } => *amount,
        InventoryItemDetails::Equippable { .. } => 1,
    }
}

/// The amount prompt the client and the offline fixture both run.
pub(crate) fn offer_amount(
    prompt: &mut crate::state::item_move::ItemMovePrompt,
    target: ItemMoveTarget,
    index: u16,
    amount: u16,
    name: &str,
) -> ItemMoveStep {
    prompt.offer(target, index, amount, name)
}

/// OK on that prompt. The caller sends the returned move.
pub(crate) fn confirm_amount(prompt: &mut crate::state::item_move::ItemMovePrompt) -> Result<(ItemMoveTarget, u16, u16), String> {
    prompt.confirm()
}

impl Client {
    /// An item dragged between the inventory and storage or the cart.
    pub(crate) fn offer_item_move(&mut self, target: ItemMoveTarget, item: &InventoryItem<ResourceMetadata>) {
        let name = item.metadata.name.clone();
        self.offer_item_move_index(target, item.index.0, stack_amount(item), &name);
    }

    fn offer_item_move_index(&mut self, target: ItemMoveTarget, index: u16, amount: u16, name: &str) {
        let step = offer_amount(
            self.client_state.follow_mut(client_state().item_move()),
            target,
            index,
            amount,
            name,
        );
        match step {
            ItemMoveStep::Send { target, index, amount } => self.send_item_move(target, index, amount),
            ItemMoveStep::AskAmount => {
                if !self.interface.is_window_with_class_open(WindowClass::ItemMoveAmount) {
                    self.interface.open_window(ItemMoveAmountWindow);
                }
                self.interface.request_focus(ItemMoveAmountBox);
            }
            ItemMoveStep::Ignore => {}
        }
    }

    /// An inventory stack dropped on the trade window.
    fn offer_trade_item(&mut self, index: u16, amount: u16) {
        if let Some(message) = self.locked_inventory_message(index) {
            self.item_flag_notice(message);
            return;
        }
        let name = self
            .client_state
            .follow(client_state().inventory().items())
            .iter()
            .find(|item| item.index.0 == index)
            .map(|item| item.metadata.name.clone())
            .unwrap_or_default();
        self.offer_item_move_index(ItemMoveTarget::ToTrade, index, amount, &name);
    }

    fn send_item_move(&mut self, target: ItemMoveTarget, index: u16, amount: u16) {
        let item = InventoryIndex(index);
        let _ = match target {
            ItemMoveTarget::ToStorage => self.networking_system.move_item_to_storage(item, u32::from(amount)),
            ItemMoveTarget::FromStorage => self.networking_system.move_item_from_storage(item, u32::from(amount)),
            ItemMoveTarget::ToCart => self.networking_system.move_item_to_cart(item, i32::from(amount)),
            ItemMoveTarget::FromCart => self.networking_system.move_item_from_cart(item, i32::from(amount)),
            ItemMoveTarget::ToTrade => {
                self.trade_add_item(index, amount);
                Ok(())
            }
        };
    }

    /// Offer `amount` of inventory index `index` in the open trade.
    pub(crate) fn trade_add_item(&mut self, index: u16, amount: u16) {
        if let Some(message) = self.locked_inventory_message(index) {
            self.item_flag_notice(message);
            return;
        }
        let trade = self.client_state.follow(client_state().trade());
        if !trade.can_note_item_add(index, u32::from(amount)) {
            return;
        }
        let label = trade
            .inventory_offers
            .iter()
            .find(|offer| offer.index == index)
            .map(|offer| offer.label.clone())
            .unwrap_or_else(|| format!("Item {index}"));
        if self
            .networking_system
            .add_trade_item(Some(InventoryIndex(index)), u32::from(amount))
            .is_ok()
        {
            self.client_state
                .follow_mut(client_state().trade())
                .note_item_add(index, label, u32::from(amount));
        }
    }

    fn confirm_item_move(&mut self) {
        let confirmed = confirm_amount(self.client_state.follow_mut(client_state().item_move()));
        match confirmed {
            Ok((target, index, amount)) => {
                self.interface.close_window_with_class(WindowClass::ItemMoveAmount);
                self.send_item_move(target, index, amount);
            }
            Err(message) => {
                self.client_state
                    .follow_mut(client_state().chat_messages())
                    .push(ChatMessage::new(message, MessageColor::Error));
                self.interface.request_focus(ItemMoveAmountBox);
            }
        }
    }

    pub(crate) fn on_item_move_input(&mut self, event: InputEvent) {
        match event {
            InputEvent::OfferTradeItem { index, amount } => self.offer_trade_item(index, amount),
            InputEvent::ConfirmItemMove => self.confirm_item_move(),
            InputEvent::CancelItemMove => {
                self.client_state.follow_mut(client_state().item_move()).cancel();
                self.interface.close_window_with_class(WindowClass::ItemMoveAmount);
            }
            _ => {}
        }
    }

    /// Once per frame after the interface events. A question whose window
    /// was closed (title bar or Escape) is dropped, and so is one whose
    /// storage or trade closed under it.
    pub(crate) fn sync_item_move(&mut self) {
        let prompt = self.client_state.follow(client_state().item_move());
        if !prompt.is_asking() {
            return;
        }
        let Some(target) = prompt.target() else {
            return;
        };
        let prompt_open = self.interface.is_window_with_class_open(WindowClass::ItemMoveAmount);
        // The cart has no open/close handshake, so a cart question stays.
        let context_open = if target.needs_storage() {
            self.storage_is_open()
        } else if target == ItemMoveTarget::ToTrade {
            self.interface.is_window_with_class_open(WindowClass::Trade)
        } else {
            true
        };
        if !prompt_open || !context_open {
            self.client_state.follow_mut(client_state().item_move()).cancel();
            if prompt_open {
                self.interface.close_window_with_class(WindowClass::ItemMoveAmount);
            }
        }
    }
}
