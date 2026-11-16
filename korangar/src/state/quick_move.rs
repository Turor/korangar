//! Alt+right-click: send the stack to the open transfer window.
//!
//! Priority while the item is in the inventory: storage, then cart, then
//! trade, then mail compose. A storage or cart slot always comes back to the
//! inventory. Stacks above 1 still go through [`super::item_move`] (or the
//! mail amount prompt); this module only picks the target.

use super::item_move::ItemMoveTarget;

/// Shown when no storage, cart, trade or mail window is open.
pub const QUICK_MOVE_NO_TARGET: &str = "Open storage, cart, trade or mail to quick-move";

/// Trade and mail refuse a locked stack (`state::item_flags`).
pub const QUICK_MOVE_LOCKED: &str = "That item is locked.";

/// Worn gear is not a transfer. The player unequips it first.
pub const QUICK_MOVE_WORN: &str = "Unequip it before moving it.";

/// Which list the clicked slot belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickMoveSource {
    Inventory,
    Storage,
    Cart,
}

/// Transfer windows that are open this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuickMoveWindows {
    pub storage: bool,
    pub cart: bool,
    pub trade: bool,
    pub mail: bool,
}

/// What Alt+right-click should do. `Move` reuses the #132 amount prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickMoveDecision {
    Move(ItemMoveTarget),
    /// Inventory stack onto the open mail form (`0x0A04`, mail amount prompt).
    Mail,
    /// Chat line. Nothing is sent.
    Hint(&'static str),
}

/// Right-click versus Alt+right-click. The item slot always reports a right
/// click; `alt` is the modifier captured with that click.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotSecondary {
    QuickMove,
    OpenMenu,
    Ignore,
}

pub fn classify_secondary(right: bool, alt: bool) -> SlotSecondary {
    if !right {
        SlotSecondary::Ignore
    } else if alt {
        SlotSecondary::QuickMove
    } else {
        SlotSecondary::OpenMenu
    }
}

/// `locked` is only consulted for trade and mail. Storage wins over a lock.
pub fn decide_quick_move(source: QuickMoveSource, windows: QuickMoveWindows, locked: bool, worn: bool) -> QuickMoveDecision {
    if worn {
        return QuickMoveDecision::Hint(QUICK_MOVE_WORN);
    }
    let decision = match source {
        QuickMoveSource::Storage => QuickMoveDecision::Move(ItemMoveTarget::FromStorage),
        QuickMoveSource::Cart => QuickMoveDecision::Move(ItemMoveTarget::FromCart),
        QuickMoveSource::Inventory => {
            if windows.storage {
                QuickMoveDecision::Move(ItemMoveTarget::ToStorage)
            } else if windows.cart {
                QuickMoveDecision::Move(ItemMoveTarget::ToCart)
            } else if windows.trade {
                QuickMoveDecision::Move(ItemMoveTarget::ToTrade)
            } else if windows.mail {
                QuickMoveDecision::Mail
            } else {
                QuickMoveDecision::Hint(QUICK_MOVE_NO_TARGET)
            }
        }
    };
    match decision {
        QuickMoveDecision::Move(ItemMoveTarget::ToTrade) | QuickMoveDecision::Mail if locked => QuickMoveDecision::Hint(QUICK_MOVE_LOCKED),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        QUICK_MOVE_LOCKED, QUICK_MOVE_NO_TARGET, QUICK_MOVE_WORN, QuickMoveDecision, QuickMoveSource, QuickMoveWindows, SlotSecondary,
        classify_secondary, decide_quick_move,
    };
    use crate::state::item_move::ItemMoveTarget;

    fn windows(storage: bool, cart: bool, trade: bool, mail: bool) -> QuickMoveWindows {
        QuickMoveWindows {
            storage,
            cart,
            trade,
            mail,
        }
    }

    #[test]
    fn alt_right_is_quick_move_and_plain_right_opens_the_menu() {
        assert_eq!(classify_secondary(true, true), SlotSecondary::QuickMove);
        assert_eq!(classify_secondary(true, false), SlotSecondary::OpenMenu);
        assert_eq!(classify_secondary(false, true), SlotSecondary::Ignore);
    }

    #[test]
    fn inventory_priority_is_storage_then_cart_then_trade_then_mail() {
        let inventory = QuickMoveSource::Inventory;
        assert_eq!(
            decide_quick_move(inventory, windows(true, true, true, true), false, false),
            QuickMoveDecision::Move(ItemMoveTarget::ToStorage)
        );
        assert_eq!(
            decide_quick_move(inventory, windows(false, true, true, true), false, false),
            QuickMoveDecision::Move(ItemMoveTarget::ToCart)
        );
        assert_eq!(
            decide_quick_move(inventory, windows(false, false, true, true), false, false),
            QuickMoveDecision::Move(ItemMoveTarget::ToTrade)
        );
        assert_eq!(
            decide_quick_move(inventory, windows(false, false, false, true), false, false),
            QuickMoveDecision::Mail
        );
        assert_eq!(
            decide_quick_move(inventory, windows(false, false, false, false), false, false),
            QuickMoveDecision::Hint(QUICK_MOVE_NO_TARGET)
        );
    }

    #[test]
    fn storage_and_cart_slots_return_to_the_inventory() {
        assert_eq!(
            decide_quick_move(QuickMoveSource::Storage, windows(false, true, true, true), true, false),
            QuickMoveDecision::Move(ItemMoveTarget::FromStorage)
        );
        assert_eq!(
            decide_quick_move(QuickMoveSource::Cart, windows(true, false, false, false), false, false),
            QuickMoveDecision::Move(ItemMoveTarget::FromCart)
        );
    }

    #[test]
    fn locked_items_skip_trade_and_mail_but_not_storage() {
        let inventory = QuickMoveSource::Inventory;
        assert_eq!(
            decide_quick_move(inventory, windows(true, false, true, true), true, false),
            QuickMoveDecision::Move(ItemMoveTarget::ToStorage)
        );
        assert_eq!(
            decide_quick_move(inventory, windows(false, false, true, true), true, false),
            QuickMoveDecision::Hint(QUICK_MOVE_LOCKED)
        );
        assert_eq!(
            decide_quick_move(inventory, windows(false, false, false, true), true, false),
            QuickMoveDecision::Hint(QUICK_MOVE_LOCKED)
        );
    }

    #[test]
    fn worn_gear_is_not_moved() {
        assert_eq!(
            decide_quick_move(QuickMoveSource::Inventory, windows(true, false, false, false), false, true),
            QuickMoveDecision::Hint(QUICK_MOVE_WORN)
        );
    }
}
