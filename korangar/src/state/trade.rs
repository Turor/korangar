use std::sync::Arc;

use korangar_interface::element::StateElement;
use rust_state::RustState;

use crate::graphics::Texture;

/// One line shown on a side of the trade window.
#[derive(Clone, Default, RustState, StateElement)]
pub struct TradeOfferLine {
    pub label: String,
    pub item_id: u32,
    #[hidden_element]
    pub texture: Option<Arc<Texture>>,
}

/// An inventory stack the player can place into the trade.
#[derive(Clone, Default, RustState, StateElement)]
pub struct TradeInventoryOffer {
    /// Stored inventory index (wire index minus 2).
    pub index: u16,
    pub item_id: u32,
    pub label: String,
    pub amount: u16,
    #[hidden_element]
    pub texture: Option<Arc<Texture>>,
}

/// An item add waiting for `ZC_ACK_ADD_EXCHANGE_ITEM`.
///
/// Zeny is not tracked here. Hercules replaces `deal.zeny` and does not ack it.
/// Index 0 on that ack is the dummy success from `trade_tradeok`.
#[derive(Clone, Default, RustState, StateElement)]
pub struct PendingTradeAdd {
    /// Wire inventory index (`stored + 2`). Never zero.
    pub index: u16,
    pub item_id: u32,
    pub label: String,
    pub amount: u32,
    #[hidden_element]
    pub texture: Option<Arc<Texture>>,
}

/// Both sides of an in-progress player trade.
#[derive(Default, RustState, StateElement)]
pub struct TradeState {
    pub partner_name: String,
    pub partner_level: i16,
    pub partner_lines: Vec<TradeOfferLine>,
    pub own_lines: Vec<TradeOfferLine>,
    pub partner_summary: String,
    pub own_summary: String,
    pub status: String,
    pub zeny_draft: String,
    pub inventory_offers: Vec<TradeInventoryOffer>,
    pub self_locked: bool,
    pub partner_locked: bool,
    pub self_committed: bool,
    pub partner_zeny: u32,
    pub own_zeny: u32,
    /// In-flight item adds. One entry per inventory index, so a silent
    /// over-stack reject on one stack does not block the rest of the window.
    pub pending_adds: Vec<PendingTradeAdd>,
}

impl TradeState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn refresh_summaries(&mut self) {
        let name = if self.partner_name.is_empty() {
            "Partner".to_owned()
        } else {
            self.partner_name.clone()
        };
        let partner_items = if self.partner_lines.is_empty() {
            "(nothing)".to_owned()
        } else {
            self.partner_lines
                .iter()
                .map(|line| line.label.clone())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let own_items = if self.own_lines.is_empty() {
            "(nothing)".to_owned()
        } else {
            self.own_lines.iter().map(|line| line.label.clone()).collect::<Vec<_>>().join(", ")
        };
        self.partner_summary = format!(
            "{name} (lv {}) — zeny {} — {partner_items}",
            self.partner_level, self.partner_zeny
        );
        self.own_summary = format!("You — zeny {} — {own_items}", self.own_zeny);
        self.status = if self.self_committed {
            "Waiting for the trade to finish.".to_owned()
        } else if self.self_locked && self.partner_locked {
            "Both sides locked. Press Deal.".to_owned()
        } else if self.self_locked {
            "You locked your offer.".to_owned()
        } else if self.partner_locked {
            "Partner locked. Confirm when ready.".to_owned()
        } else {
            "Add items or zeny, then confirm.".to_owned()
        };
    }

    /// Partner zeny on `ZC_ADD_EXCHANGE_ITEM` is the absolute `deal.zeny`.
    pub fn set_partner_zeny(&mut self, amount: u32) {
        self.partner_zeny = amount;
        self.refresh_summaries();
    }

    /// Our zeny offer replaces the previous total. The server does not ack it.
    pub fn offer_own_zeny(&mut self, amount: u32) {
        self.own_zeny = amount;
        self.zeny_draft.clear();
        self.refresh_summaries();
    }

    /// True when this stack can be sent. A stack already in flight, or an
    /// amount above the offer button, is refused. The server drops the latter
    /// with no ack.
    pub fn can_note_item_add(&self, stored_index: u16, amount: u32) -> bool {
        if self.self_locked || amount == 0 {
            return false;
        }
        let wire = stored_index.saturating_add(2);
        if wire == 0 || self.pending_adds.iter().any(|pending| pending.index == wire) {
            return false;
        }
        match self.inventory_offers.iter().find(|offer| offer.index == stored_index) {
            Some(offer) => u32::from(offer.amount) >= amount,
            None => false,
        }
    }

    /// Record an item add after the packet was sent.
    pub fn note_item_add(&mut self, stored_index: u16, label: String, amount: u32) -> bool {
        if !self.can_note_item_add(stored_index, amount) {
            return false;
        }
        let (item_id, texture) = self
            .inventory_offers
            .iter()
            .find(|offer| offer.index == stored_index)
            .map(|offer| (offer.item_id, offer.texture.clone()))
            .unwrap_or((0, None));
        self.pending_adds.push(PendingTradeAdd {
            index: stored_index.saturating_add(2),
            item_id,
            label,
            amount,
            texture,
        });
        true
    }

    pub fn set_item_texture(&mut self, item_id: u32, texture: Arc<Texture>) {
        if item_id == 0 {
            return;
        }
        for line in self.partner_lines.iter_mut().chain(self.own_lines.iter_mut()) {
            if line.item_id == item_id {
                line.texture = Some(texture.clone());
            }
        }
        for offer in &mut self.inventory_offers {
            if offer.item_id == item_id {
                offer.texture = Some(texture.clone());
            }
        }
    }

    /// Apply `ZC_ACK_ADD_EXCHANGE_ITEM`.
    ///
    /// Index 0 is ignored. `trade_tradeok` sends that dummy success, and
    /// booking it as zeny drops whatever item add was still pending.
    /// A matching success removes the inventory offer so the button goes away.
    pub fn apply_add_result(&mut self, index: u16, result: u8) -> Option<&'static str> {
        if index == 0 {
            return None;
        }
        let position = self.pending_adds.iter().position(|pending| pending.index == index)?;
        let pending = self.pending_adds.remove(position);
        if result != 0 {
            self.refresh_summaries();
            return Some(match result {
                1 => "That offer is too heavy.",
                2 => "The trade was cancelled.",
                3 => "That amount is too high.",
                _ => "Could not add that to the trade.",
            });
        }
        if !pending.label.is_empty() {
            self.own_lines.push(TradeOfferLine {
                label: pending.label,
                item_id: pending.item_id,
                texture: pending.texture,
            });
        }
        let stored = index.saturating_sub(2);
        if let Some(offer_at) = self.inventory_offers.iter().position(|offer| offer.index == stored) {
            let offer_amount = u32::from(self.inventory_offers[offer_at].amount);
            if pending.amount >= offer_amount {
                self.inventory_offers.remove(offer_at);
            } else {
                let left = (offer_amount - pending.amount) as u16;
                let offer = &mut self.inventory_offers[offer_at];
                offer.amount = left;
                if let Some((name, _)) = offer.label.rsplit_once(" x") {
                    offer.label = format!("{name} x{left}");
                }
            }
        }
        self.refresh_summaries();
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trade_partner_zeny_replaces_the_previous_total() {
        let mut trade = TradeState::default();
        trade.set_partner_zeny(1_000);
        trade.set_partner_zeny(500);
        assert_eq!(trade.partner_zeny, 500);
        assert!(trade.partner_summary.contains("zeny 500"));
    }

    #[test]
    fn trade_own_zeny_replaces_and_does_not_wait_for_an_ack() {
        let mut trade = TradeState::default();
        trade.offer_own_zeny(1_000);
        trade.offer_own_zeny(500);
        assert_eq!(trade.own_zeny, 500);
        assert!(trade.pending_adds.is_empty());
    }

    #[test]
    fn trade_dummy_zero_ack_leaves_a_pending_item_alone() {
        let mut trade = TradeState::default();
        trade.inventory_offers.push(TradeInventoryOffer {
            index: 3,
            item_id: 512,
            label: "Apple x5".to_owned(),
            amount: 5,
            texture: None,
        });
        assert!(trade.note_item_add(3, "Apple x5".to_owned(), 5));
        assert!(trade.apply_add_result(0, 0).is_none());
        assert!(trade.own_lines.is_empty());
        assert_eq!(trade.own_zeny, 0);
        assert_eq!(trade.pending_adds.len(), 1);
        assert_eq!(trade.inventory_offers.len(), 1);
    }

    #[test]
    fn trade_successful_item_ack_removes_the_offer() {
        let mut trade = TradeState::default();
        trade.inventory_offers.push(TradeInventoryOffer {
            index: 3,
            item_id: 512,
            label: "Apple x5".to_owned(),
            amount: 5,
            texture: None,
        });
        trade.inventory_offers.push(TradeInventoryOffer {
            index: 4,
            item_id: 501,
            label: "Potion x2".to_owned(),
            amount: 2,
            texture: None,
        });
        assert!(trade.note_item_add(3, "Apple x5".to_owned(), 5));
        assert!(!trade.note_item_add(3, "Apple x5".to_owned(), 5));
        assert!(trade.note_item_add(4, "Potion x2".to_owned(), 2));
        assert!(trade.apply_add_result(5, 0).is_none());
        assert_eq!(trade.own_lines.len(), 1);
        assert_eq!(trade.own_lines[0].item_id, 512);
        assert_eq!(trade.inventory_offers.len(), 1);
        assert_eq!(trade.inventory_offers[0].index, 4);
        assert_eq!(trade.pending_adds.len(), 1);
    }

    #[test]
    fn trade_over_stack_is_refused_without_a_pending_add() {
        let mut trade = TradeState::default();
        trade.inventory_offers.push(TradeInventoryOffer {
            index: 3,
            item_id: 512,
            label: "Apple x5".to_owned(),
            amount: 5,
            texture: None,
        });
        assert!(!trade.note_item_add(3, "Apple x5".to_owned(), 9));
        assert!(trade.pending_adds.is_empty());
    }
}
