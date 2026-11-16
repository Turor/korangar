//! Amount prompt for moving part of a stack into or out of storage, the cart,
//! or a trade.
//!
//! roBrowser opens its amount `InputBox` whenever a stack of more than one is
//! dropped on Storage, Cart or the trade window; a single item moves at once.
//! The box starts at the whole stack and only `1..=stack` is accepted.

use korangar_interface::element::StateElement;
use rust_state::RustState;

/// Where the item is going.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemMoveTarget {
    /// Inventory to Kafra or guild storage (`CZ_MOVE_ITEM_FROM_BODY_TO_STORE`).
    #[default]
    ToStorage,
    /// Storage to inventory (`CZ_MOVE_ITEM_FROM_STORE_TO_BODY`).
    FromStorage,
    /// Inventory to cart (`CZ_MOVE_ITEM_FROM_BODY_TO_CART`).
    ToCart,
    /// Cart to inventory (`CZ_MOVE_ITEM_FROM_CART_TO_BODY`).
    FromCart,
    /// Offer in the open trade (`CZ_ADD_EXCHANGE_ITEM`).
    ToTrade,
}

impl ItemMoveTarget {
    fn question(self, name: &str, max: u16) -> String {
        match self {
            Self::ToStorage => format!("Store how many {name}? (1-{max})"),
            Self::FromStorage => format!("Take how many {name} out of storage? (1-{max})"),
            Self::ToCart => format!("Put how many {name} in the cart? (1-{max})"),
            Self::FromCart => format!("Take how many {name} from the cart? (1-{max})"),
            Self::ToTrade => format!("Trade how many {name}? (1-{max})"),
        }
    }

    /// Whether the move only makes sense while storage is open.
    pub fn needs_storage(self) -> bool {
        matches!(self, Self::ToStorage | Self::FromStorage)
    }
}

/// Result of offering a stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemMoveStep {
    /// Send now: a single item or equipment.
    Send { target: ItemMoveTarget, index: u16, amount: u16 },
    /// The amount window should open.
    AskAmount,
    /// Nothing to move.
    Ignore,
}

/// Amount window state. At most one move is asked at a time; a new drop
/// replaces the question.
#[derive(Debug, Clone, Default, RustState, StateElement)]
pub struct ItemMovePrompt {
    #[hidden_element]
    target: Option<ItemMoveTarget>,
    #[hidden_element]
    index: u16,
    #[hidden_element]
    max_amount: u16,
    pub prompt: String,
    pub amount_text: String,
}

impl ItemMovePrompt {
    /// Offer `amount` of item `index` (inventory, storage or cart index,
    /// depending on `target`).
    pub fn offer(&mut self, target: ItemMoveTarget, index: u16, amount: u16, name: &str) -> ItemMoveStep {
        match amount {
            0 => ItemMoveStep::Ignore,
            1 => {
                self.cancel();
                ItemMoveStep::Send { target, index, amount: 1 }
            }
            max => {
                let name = match name.trim() {
                    "" => "items",
                    name => name,
                };
                self.target = Some(target);
                self.index = index;
                self.max_amount = max;
                self.prompt = target.question(name, max);
                self.amount_text = max.to_string();
                ItemMoveStep::AskAmount
            }
        }
    }

    pub fn is_asking(&self) -> bool {
        self.target.is_some()
    }

    pub fn target(&self) -> Option<ItemMoveTarget> {
        self.target
    }

    /// The typed amount, or why it is refused. Leading zeros and surrounding
    /// spaces are fine (`" 007 "` is 7).
    pub fn amount(&self) -> Result<u16, String> {
        if self.target.is_none() {
            return Err("No item is waiting for an amount.".to_owned());
        }
        self.amount_text
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|amount| (1..=self.max_amount).contains(amount))
            .ok_or_else(|| format!("Enter an amount from 1 to {}.", self.max_amount))
    }

    /// The window's OK. Clears the question when the amount is valid.
    pub fn confirm(&mut self) -> Result<(ItemMoveTarget, u16, u16), String> {
        let amount = self.amount()?;
        let target = self.target.ok_or("No item is waiting for an amount.")?;
        let index = self.index;
        self.cancel();
        Ok((target, index, amount))
    }

    pub fn cancel(&mut self) {
        self.target = None;
        self.index = 0;
        self.max_amount = 0;
        self.prompt.clear();
        self.amount_text.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{ItemMovePrompt, ItemMoveStep, ItemMoveTarget};

    #[test]
    fn single_items_move_without_a_question() {
        let mut prompt = ItemMovePrompt::default();
        assert_eq!(prompt.offer(ItemMoveTarget::ToStorage, 4, 1, "Knife"), ItemMoveStep::Send {
            target: ItemMoveTarget::ToStorage,
            index: 4,
            amount: 1,
        });
        assert!(!prompt.is_asking());
        assert_eq!(prompt.offer(ItemMoveTarget::ToCart, 4, 0, "Knife"), ItemMoveStep::Ignore);
    }

    #[test]
    fn stacks_ask_and_accept_only_the_stack_range() {
        let mut prompt = ItemMovePrompt::default();
        assert_eq!(
            prompt.offer(ItemMoveTarget::FromStorage, 9, 30, "Red Potion"),
            ItemMoveStep::AskAmount
        );
        assert_eq!(prompt.prompt, "Take how many Red Potion out of storage? (1-30)");
        assert_eq!(prompt.amount_text, "30");
        assert_eq!(prompt.target(), Some(ItemMoveTarget::FromStorage));
        for text in ["", "0", "31", "x", "-1", "70000"] {
            prompt.amount_text = text.to_owned();
            assert_eq!(prompt.confirm(), Err("Enter an amount from 1 to 30.".to_owned()), "{text:?}");
            assert!(prompt.is_asking(), "a bad amount keeps the window open");
        }
        prompt.amount_text = " 007 ".to_owned();
        assert_eq!(prompt.confirm(), Ok((ItemMoveTarget::FromStorage, 9, 7)));
        assert!(!prompt.is_asking());
        assert!(prompt.confirm().is_err(), "a confirmed question cannot send twice");
    }

    #[test]
    fn every_target_words_its_question() {
        let cases = [
            (ItemMoveTarget::ToStorage, "Store how many Jellopy? (1-5)"),
            (ItemMoveTarget::FromStorage, "Take how many Jellopy out of storage? (1-5)"),
            (ItemMoveTarget::ToCart, "Put how many Jellopy in the cart? (1-5)"),
            (ItemMoveTarget::FromCart, "Take how many Jellopy from the cart? (1-5)"),
            (ItemMoveTarget::ToTrade, "Trade how many Jellopy? (1-5)"),
        ];
        for (target, text) in cases {
            let mut prompt = ItemMovePrompt::default();
            assert_eq!(prompt.offer(target, 2, 5, " Jellopy "), ItemMoveStep::AskAmount);
            assert_eq!(prompt.prompt, text);
        }
        let mut prompt = ItemMovePrompt::default();
        prompt.offer(ItemMoveTarget::ToTrade, 2, 5, "");
        assert_eq!(prompt.prompt, "Trade how many items? (1-5)");
    }

    #[test]
    fn a_new_drop_replaces_the_question_and_cancel_clears_it() {
        let mut prompt = ItemMovePrompt::default();
        prompt.offer(ItemMoveTarget::ToCart, 2, 5, "Jellopy");
        prompt.offer(ItemMoveTarget::ToTrade, 6, 12, "Apple");
        prompt.amount_text = "12".to_owned();
        assert_eq!(prompt.confirm(), Ok((ItemMoveTarget::ToTrade, 6, 12)));

        prompt.offer(ItemMoveTarget::FromCart, 3, 8, "Apple");
        prompt.cancel();
        assert!(!prompt.is_asking());
        assert!(prompt.prompt.is_empty());
        assert!(prompt.confirm().is_err());

        prompt.offer(ItemMoveTarget::ToStorage, 3, 8, "Apple");
        assert!(prompt.target().unwrap().needs_storage());
        assert_eq!(prompt.offer(ItemMoveTarget::ToCart, 5, 1, "Knife"), ItemMoveStep::Send {
            target: ItemMoveTarget::ToCart,
            index: 5,
            amount: 1,
        });
        assert!(!prompt.is_asking(), "a single-item drop clears an older question");
    }
}
