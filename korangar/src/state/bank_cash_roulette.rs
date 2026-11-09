use korangar_interface::element::StateElement;
use korangar_networking::{CashShopOffer, RoulettePrize};
use rust_state::RustState;

/// Parse a positive zeny, cash amount, item id, or tab from a text box.
pub fn parse_positive_u32(text: &str) -> Option<u32> {
    let value = text.trim().parse::<u32>().ok()?;
    (value > 0).then_some(value)
}

pub fn bank_reason_label(reason: i16) -> &'static str {
    match reason {
        0 => "ok",
        1 => "failed",
        2 => "not enough zeny",
        3 => "vault is full",
        _ => "unknown",
    }
}

pub fn cash_shop_result_label(result: u16) -> &'static str {
    match result {
        0 => "purchased",
        1 => "no shop",
        2 => "shop error",
        3 => "over weight",
        4 => "busy",
        5 => "bad item",
        6 => "not enough points",
        7 => "too many items",
        8 => "partial purchase",
        _ => "unknown",
    }
}

pub fn roulette_generate_label(result: u8) -> &'static str {
    match result {
        0 => "advanced",
        1 => "failed",
        2 => "not enough points",
        3 => "losing cell",
        _ => "unknown",
    }
}

fn format_prizes(prizes: &[RoulettePrize]) -> String {
    if prizes.is_empty() {
        return "No prizes.".to_owned();
    }
    prizes
        .iter()
        .take(42)
        .map(|prize| {
            format!(
                "row {} col {} item {} x{}",
                prize.row, prize.position, prize.item_id, prize.count
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_offers(items: &[CashShopOffer]) -> String {
    if items.is_empty() {
        return "This tab is empty.".to_owned();
    }
    items
        .iter()
        .map(|item| format!("item {} — {} points", item.item_id, item.price))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(RustState, StateElement)]
pub struct BankWindowState {
    pub status: String,
    pub amount_text: String,
    pub amount_ghost: String,
}

impl Default for BankWindowState {
    fn default() -> Self {
        Self {
            status: "Open the bank to read the vault.".to_owned(),
            amount_text: String::new(),
            amount_ghost: "zeny".to_owned(),
        }
    }
}

impl BankWindowState {
    pub fn amount(&self) -> Result<i32, &'static str> {
        let value = parse_positive_u32(&self.amount_text).ok_or("Enter a zeny amount.")?;
        i32::try_from(value).map_err(|_| "Amount is too large.")
    }

    pub fn note_open(&mut self) {
        self.status = "Bank is open.".to_owned();
    }

    pub fn note_balance(&mut self, money: i64, reason: i16) {
        self.status = format!("Vault {money} zeny ({})", bank_reason_label(reason));
    }

    pub fn note_deposit(&mut self, reason: i16, money: i64, zeny: i32) {
        self.status = format!("Deposit {} — vault {money}, pocket {zeny}", bank_reason_label(reason));
    }

    pub fn note_withdraw(&mut self, reason: i16, money: i64, zeny: i32) {
        self.status = format!("Withdraw {} — vault {money}, pocket {zeny}", bank_reason_label(reason));
    }

    pub fn note_closed(&mut self) {
        self.status = "Bank closed.".to_owned();
    }
}

#[derive(RustState, StateElement)]
pub struct CashShopWindowState {
    pub status: String,
    pub catalog: String,
    pub tab_text: String,
    pub tab_ghost: String,
    pub item_text: String,
    pub item_ghost: String,
    pub amount_text: String,
    pub amount_ghost: String,
    pub kafra_text: String,
    pub kafra_ghost: String,
    pub cash_points: u32,
    pub kafra_points: u32,
}

impl Default for CashShopWindowState {
    fn default() -> Self {
        Self {
            status: "Cash shop is off until Game settings enables it.".to_owned(),
            catalog: String::new(),
            tab_text: "0".to_owned(),
            tab_ghost: "tab".to_owned(),
            item_text: String::new(),
            item_ghost: "item id".to_owned(),
            amount_text: "1".to_owned(),
            amount_ghost: "amount".to_owned(),
            kafra_text: "0".to_owned(),
            kafra_ghost: "kafra points to spend".to_owned(),
            cash_points: 0,
            kafra_points: 0,
        }
    }
}

impl CashShopWindowState {
    pub fn tab(&self) -> Result<u16, &'static str> {
        let value = self.tab_text.trim().parse::<u32>().map_err(|_| "Enter a tab.")?;
        u16::try_from(value).map_err(|_| "Tab is too large.")
    }

    pub fn item_id(&self) -> Result<u32, &'static str> {
        parse_positive_u32(&self.item_text).ok_or("Enter an item id.")
    }

    pub fn amount(&self) -> Result<u32, &'static str> {
        parse_positive_u32(&self.amount_text).ok_or("Enter an amount.")
    }

    pub fn kafra_spend(&self) -> Result<u32, &'static str> {
        self.kafra_text.trim().parse::<u32>().map_err(|_| "Enter kafra points, or 0.")
    }

    pub fn note_open(&mut self, cash_points: u32, kafra_points: u32, tab: u32) {
        self.cash_points = cash_points;
        self.kafra_points = kafra_points;
        self.status = format!("Cash {cash_points}, kafra {kafra_points}, tab {tab}");
    }

    pub fn note_items(&mut self, tab: u32, items: &[CashShopOffer]) {
        self.catalog = format_offers(items);
        self.status = format!(
            "Tab {tab}: {} item(s). Cash {}, kafra {}.",
            items.len(),
            self.cash_points,
            self.kafra_points
        );
    }

    pub fn note_buy(&mut self, item_id: u32, result: u16, cash_points: u32, kafra_points: u32) {
        self.cash_points = cash_points;
        self.kafra_points = kafra_points;
        self.status = format!(
            "Item {item_id} {} — cash {cash_points}, kafra {kafra_points}",
            cash_shop_result_label(result)
        );
    }
}

#[derive(RustState, StateElement)]
pub struct RouletteWindowState {
    pub status: String,
    pub prizes: String,
}

impl Default for RouletteWindowState {
    fn default() -> Self {
        Self {
            status: "Open the roulette to load points.".to_owned(),
            prizes: String::new(),
        }
    }
}

pub struct RouletteSpinView {
    pub result: i32,
    pub step: i32,
    pub index: i32,
    pub bonus_item_id: u32,
    pub gold: i32,
    pub silver: i32,
    pub bronze: i32,
}

impl RouletteWindowState {
    pub fn note_open(&mut self, spin: &RouletteSpinView) {
        self.status = format!(
            "Roulette result {}, stage {}, cell {}, bonus {}. Gold {}, silver {}, bronze {}.",
            spin.result, spin.step, spin.index, spin.bonus_item_id, spin.gold, spin.silver, spin.bronze
        );
    }

    pub fn note_prizes(&mut self, prizes: &[RoulettePrize]) {
        self.prizes = format_prizes(prizes);
    }

    pub fn note_spin(&mut self, spin: &RouletteSpinView) {
        self.status = format!(
            "Spin {} — stage {}, cell {}, bonus {}. Gold {}, silver {}, bronze {}.",
            roulette_generate_label(spin.result as u8),
            spin.step,
            spin.index,
            spin.bonus_item_id,
            spin.gold,
            spin.silver,
            spin.bronze
        );
    }

    pub fn note_claim(&mut self, result: u8, item_id: u32) {
        let outcome = if result == 0 { "received" } else { "not received" };
        self.status = format!("Prize {item_id} {outcome} (code {result}).");
    }

    pub fn note_closed(&mut self, result: u8) {
        self.status = format!("Roulette closed (code {result}).");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_reject_zero_and_blank() {
        assert_eq!(parse_positive_u32(" 12 "), Some(12));
        assert_eq!(parse_positive_u32("0"), None);
        assert_eq!(parse_positive_u32(""), None);
        assert!(BankWindowState::default().amount().is_err());
    }

    #[test]
    fn cash_shop_starts_disabled_in_the_window_copy() {
        let shop = CashShopWindowState::default();
        assert!(shop.status.contains("off"));
        assert_eq!(shop.tab().unwrap(), 0);
        assert!(shop.item_id().is_err());
    }
}
