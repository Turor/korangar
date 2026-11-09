use korangar_interface::element::StateElement;
use rust_state::RustState;

/// One line shown on a side of the trade window.
#[derive(Clone, Default, RustState, StateElement)]
pub struct TradeOfferLine {
    pub label: String,
}

/// An inventory stack the player can place into the trade.
#[derive(Clone, Default, RustState, StateElement)]
pub struct TradeInventoryOffer {
    /// Stored inventory index (wire index minus 2).
    pub index: u16,
    pub label: String,
    pub amount: u16,
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
    /// Client index waiting for `ZC_ACK_ADD_EXCHANGE_ITEM`. Zero is zeny.
    pub pending_index: Option<u16>,
    pub pending_label: String,
    pub pending_amount: u32,
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
}
