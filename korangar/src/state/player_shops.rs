//! Draft, search, and sales state for this client's own shops.
//!
//! Counts and prices follow Hercules `MAX_VENDING` (12), `vending_max_value`
//! (10_000_000), `MAX_BUYINGSTORE_SLOTS` (5), `BUYINGSTORE_MAX_PRICE`
//! (99_990_000), and `BUYINGSTORE_MAX_AMOUNT` (9999). The UI rejects a price of
//! 0. Hercules caps an over-max vending price instead of rejecting it, and it
//! accepts a vending price of 0; the setup window still refuses both so a shop
//! cannot list a free or uncapped line. Buying-store prices of 0 are rejected
//! by the server. Pocket zeny is not tracked here, so a buying limit above 0
//! is checked locally and the server still rejects a limit above the pocket.

use korangar_interface::element::StateElement;
use rust_state::RustState;

/// Hercules `MAX_VENDING`.
pub const VENDING_MAX_ITEMS: usize = 12;
/// Hercules `battle_config.vending_max_value` default.
pub const VENDING_MAX_PRICE: u32 = 10_000_000;
/// Hercules `MAX_BUYINGSTORE_SLOTS`.
pub const BUYING_STORE_MAX_SLOTS: usize = 5;
/// Hercules `BUYINGSTORE_MAX_PRICE`.
pub const BUYING_STORE_MAX_PRICE: u32 = 99_990_000;
/// Hercules `BUYINGSTORE_MAX_AMOUNT`.
pub const BUYING_STORE_MAX_AMOUNT: u16 = 9999;
/// Name-search item cap. The search packet count is a single byte.
pub const SEARCH_ITEM_LIMIT: usize = 10;
/// Shop title bytes on the wire, excluding the trailing NUL inside the 80.
pub const SHOP_TITLE_LIMIT: usize = 79;
/// `selected_line` value when no row is selected.
pub const NO_SELECTION: u16 = u16::MAX;

/// One cart or bag stack the setup window can list.
#[derive(Clone, Debug)]
pub struct BagSource {
    pub name: String,
    pub stored_index: u16,
    pub item_id: u32,
    pub amount: u16,
    pub stackable: bool,
}

/// A row offered in the setup window before it is edited.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct ShopCandidate {
    pub label: String,
    pub name: String,
    pub stored_index: u16,
    pub item_id: u32,
    pub available: u16,
}

/// One editable shop line. `stored_index` is the inventory index before `+ 2`.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct ShopDraftLine {
    pub label: String,
    pub name: String,
    pub stored_index: u16,
    pub item_id: u32,
    pub available: u16,
    pub amount: u16,
    pub price: u32,
    pub listed: bool,
}

/// A catalog or bag hit the search window can add.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct ShopSearchPick {
    pub label: String,
    pub item_id: u32,
    pub name: String,
}

/// One line of this client's open shop.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct ShopReportLine {
    pub label: String,
    pub name: String,
    pub index: u16,
    pub item_id: u32,
    pub amount: u16,
    pub price: u32,
}

/// Validated open request. Vending indexes are still stored indexes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShopOpenRequest {
    Vending {
        title: String,
        items: Vec<(u16, u16, u32)>,
    },
    Buying {
        title: String,
        zeny_limit: u32,
        items: Vec<(u32, u16, u32)>,
    },
}

/// Validated search request. Prices of 0 mean that side is unlimited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopSearchRequest {
    pub buying: bool,
    pub min_price: u32,
    pub max_price: u32,
    pub item_ids: Vec<u32>,
}

/// Vending or buying-store setup window.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct ShopSetupState {
    pub buying: bool,
    pub max_items: u16,
    pub title: String,
    pub amount_text: String,
    pub price_text: String,
    pub zeny_text: String,
    pub status: String,
    pub summary: String,
    #[hidden_element]
    pub selected_line: u16,
    pub lines: Vec<ShopDraftLine>,
}

impl Default for ShopSetupState {
    fn default() -> Self {
        Self {
            buying: false,
            max_items: 0,
            title: String::new(),
            amount_text: String::new(),
            price_text: String::new(),
            zeny_text: String::new(),
            status: String::new(),
            summary: String::new(),
            selected_line: NO_SELECTION,
            lines: Vec::new(),
        }
    }
}

/// Search-store query. Results stay on `ClientState::search_store_hits`.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct ShopSearchState {
    pub query: String,
    pub min_text: String,
    pub max_text: String,
    pub buying: bool,
    pub mode_label: String,
    pub status: String,
    pub has_next_page: bool,
    pub matches: Vec<ShopSearchPick>,
    pub selected: Vec<ShopSearchPick>,
}

impl Default for ShopSearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            min_text: String::new(),
            max_text: String::new(),
            buying: false,
            mode_label: "Searching selling shops".to_owned(),
            status: String::new(),
            has_next_page: false,
            matches: Vec::new(),
            selected: Vec::new(),
        }
    }
}

/// Own-shop stock and the sales log filled by `0x09E5`.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct ShopReportState {
    pub buying: bool,
    pub status: String,
    pub stock: Vec<ShopReportLine>,
    pub sales_log: String,
}

/// Turn bag stacks into setup rows. Buying stores only take stackable items.
pub fn candidates_from_sources(sources: impl IntoIterator<Item = BagSource>, stackable_only: bool) -> Vec<ShopCandidate> {
    sources
        .into_iter()
        .filter(|source| !stackable_only || source.stackable)
        .map(|source| ShopCandidate {
            label: format!("{} x{}", source.name, source.amount),
            name: source.name,
            stored_index: source.stored_index,
            item_id: source.item_id,
            available: source.amount,
        })
        .collect()
}

/// Case-insensitive name search. A numeric query is an exact item id.
/// Anything else needs at least two characters.
pub fn match_item_names<'a>(items: impl IntoIterator<Item = (u32, &'a str)>, query: &str, limit: usize) -> Vec<(u32, String)> {
    let query = query.trim();
    if query.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    if query.bytes().all(|byte| byte.is_ascii_digit()) {
        let Ok(id) = query.parse::<u32>() else {
            return out;
        };
        for (item_id, name) in items {
            if item_id == id {
                out.push((item_id, name.to_owned()));
                if out.len() == limit {
                    break;
                }
            }
        }
        return out;
    }
    if query.chars().count() < 2 {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    for (item_id, name) in items {
        if name.to_lowercase().contains(&needle) {
            out.push((item_id, name.to_owned()));
            if out.len() == limit {
                break;
            }
        }
    }
    out
}

/// Append name hits that are not already present, stopping at `limit`.
pub fn extend_name_matches(picks: &mut Vec<(u32, String)>, extra: impl IntoIterator<Item = (u32, String)>, limit: usize) {
    for (id, name) in extra {
        if picks.len() >= limit {
            break;
        }
        if picks.iter().any(|(existing, _)| *existing == id) {
            continue;
        }
        picks.push((id, name));
    }
}

impl ShopSetupState {
    pub fn start_vending(&mut self, item_count: u16, candidates: Vec<ShopCandidate>) {
        self.start(false, usize::from(item_count).min(VENDING_MAX_ITEMS), candidates);
    }

    pub fn start_buying(&mut self, item_count: u8, candidates: Vec<ShopCandidate>) {
        self.start(true, usize::from(item_count).min(BUYING_STORE_MAX_SLOTS), candidates);
    }

    fn start(&mut self, buying: bool, max_items: usize, candidates: Vec<ShopCandidate>) {
        self.buying = buying;
        self.max_items = max_items as u16;
        self.title.clear();
        self.amount_text.clear();
        self.price_text.clear();
        self.zeny_text.clear();
        self.selected_line = NO_SELECTION;
        self.lines = candidates
            .into_iter()
            .map(|candidate| {
                let mut line = ShopDraftLine {
                    label: String::new(),
                    name: candidate.name,
                    stored_index: candidate.stored_index,
                    item_id: candidate.item_id,
                    available: candidate.available,
                    amount: 0,
                    price: 0,
                    listed: false,
                };
                refresh_line(&mut line, buying);
                line
            })
            .collect();
        self.status = if self.max_items == 0 {
            "The server did not allow a shop.".to_owned()
        } else {
            String::new()
        };
        self.refresh_summary();
    }

    pub fn select(&mut self, index: usize) {
        let Some(line) = self.lines.get(index) else {
            self.status = "That item is not in the list.".to_owned();
            return;
        };
        self.selected_line = index as u16;
        self.amount_text = if line.listed && line.amount > 0 {
            line.amount.to_string()
        } else {
            "1".to_owned()
        };
        self.price_text = if line.listed && line.price > 0 {
            line.price.to_string()
        } else {
            String::new()
        };
    }

    pub fn commit_selected(&mut self) -> Result<(), String> {
        if self.selected_line == NO_SELECTION {
            return self.fail("Select an item first.");
        }
        let index = usize::from(self.selected_line);
        if index >= self.lines.len() {
            return self.fail("Select an item first.");
        }
        if self.max_items == 0 {
            return self.fail("The server did not allow a shop.");
        }
        let amount = match parse_u16(self.amount_text.trim(), "Amount") {
            Ok(amount) => amount,
            Err(message) => return self.fail(message),
        };
        let price_cap = if self.buying { BUYING_STORE_MAX_PRICE } else { VENDING_MAX_PRICE };
        let price = match parse_price(self.price_text.trim(), price_cap) {
            Ok(price) => price,
            Err(message) => return self.fail(message),
        };
        let available = self.lines[index].available;
        if amount == 0 {
            return self.fail("Amount must be at least 1.");
        }
        if self.buying {
            if amount > BUYING_STORE_MAX_AMOUNT {
                return self.fail("Buying stores can request at most 9999.");
            }
            if u32::from(available) + u32::from(amount) > u32::from(BUYING_STORE_MAX_AMOUNT) {
                return self.fail("You would hold more than 9999 of that item.");
            }
            let item_id = self.lines[index].item_id;
            if self
                .lines
                .iter()
                .enumerate()
                .any(|(other, line)| other != index && line.listed && line.item_id == item_id)
            {
                return self.fail("That item is already listed.");
            }
        } else if amount > available {
            return self.fail("Amount is higher than the cart stack.");
        }
        let listed = self.lines.iter().filter(|line| line.listed).count();
        if !self.lines[index].listed && listed >= usize::from(self.max_items) {
            return self.fail("The shop is already full.");
        }
        let buying = self.buying;
        let line = &mut self.lines[index];
        line.amount = amount;
        line.price = price;
        line.listed = true;
        refresh_line(line, buying);
        self.status.clear();
        self.refresh_summary();
        Ok(())
    }

    pub fn unlist_selected(&mut self) {
        if self.selected_line == NO_SELECTION {
            self.status = "Select an item first.".to_owned();
            return;
        }
        let index = usize::from(self.selected_line);
        let Some(line) = self.lines.get_mut(index) else {
            self.status = "Select an item first.".to_owned();
            return;
        };
        line.listed = false;
        line.amount = 0;
        line.price = 0;
        refresh_line(line, self.buying);
        self.refresh_summary();
    }

    pub fn open_request(&mut self) -> Result<ShopOpenRequest, String> {
        let title = self.title.trim();
        if title.is_empty() {
            return self.fail("Enter a shop title.");
        }
        if title.chars().count() > SHOP_TITLE_LIMIT {
            return self.fail("The shop title is too long.");
        }
        let listed: Vec<&ShopDraftLine> = self.lines.iter().filter(|line| line.listed).collect();
        if listed.is_empty() {
            return self.fail("List at least one item.");
        }
        if listed.len() > usize::from(self.max_items) {
            return self.fail("The shop is already full.");
        }
        let title = title.to_owned();
        if self.buying {
            let zeny_limit = match parse_zeny(self.zeny_text.trim()) {
                Ok(zeny) => zeny,
                Err(message) => return self.fail(message),
            };
            let mut seen = Vec::new();
            let mut items = Vec::with_capacity(listed.len());
            for line in listed {
                if line.price == 0 || line.price > BUYING_STORE_MAX_PRICE {
                    return self.fail("A listed price is outside the buying-store limit.");
                }
                if line.amount == 0 || line.amount > BUYING_STORE_MAX_AMOUNT {
                    return self.fail("A listed amount is outside the buying-store limit.");
                }
                if seen.contains(&line.item_id) {
                    return self.fail("That item is already listed.");
                }
                seen.push(line.item_id);
                items.push((line.item_id, line.amount, line.price));
            }
            Ok(ShopOpenRequest::Buying { title, zeny_limit, items })
        } else {
            let mut items = Vec::with_capacity(listed.len());
            for line in listed {
                if line.price == 0 || line.price > VENDING_MAX_PRICE {
                    return self.fail("A listed price is outside the vending limit.");
                }
                if line.amount == 0 || line.amount > line.available {
                    return self.fail("A listed amount is higher than the cart stack.");
                }
                items.push((line.stored_index, line.amount, line.price));
            }
            Ok(ShopOpenRequest::Vending { title, items })
        }
    }

    fn fail<T>(&mut self, message: impl Into<String>) -> Result<T, String> {
        let message = message.into();
        self.status = message.clone();
        Err(message)
    }

    fn refresh_summary(&mut self) {
        let listed = self.lines.iter().filter(|line| line.listed).count();
        let kind = if self.buying { "Buying store" } else { "Vending shop" };
        self.summary = format!("{kind}: {listed}/{} listed", self.max_items);
    }
}

impl ShopSearchState {
    pub fn begin(&mut self, uses: u8) {
        self.has_next_page = false;
        self.status = format!("Shop search is available ({uses} uses left).");
    }

    pub fn set_buying(&mut self, buying: bool) {
        self.buying = buying;
        self.mode_label = if buying {
            "Searching buying stores".to_owned()
        } else {
            "Searching selling shops".to_owned()
        };
    }

    pub fn set_matches(&mut self, picks: Vec<(u32, String)>) {
        self.matches = picks
            .into_iter()
            .map(|(item_id, name)| ShopSearchPick {
                label: name.clone(),
                item_id,
                name,
            })
            .collect();
    }

    pub fn add_match(&mut self, index: usize) -> Result<(), String> {
        let Some(pick) = self.matches.get(index).cloned() else {
            return self.fail("That item is not in the list.");
        };
        if self.selected.iter().any(|item| item.item_id == pick.item_id) {
            return self.fail("That item is already selected.");
        }
        if self.selected.len() >= SEARCH_ITEM_LIMIT {
            return self.fail(format!("Search can list at most {SEARCH_ITEM_LIMIT} items."));
        }
        self.selected.push(pick);
        self.status.clear();
        Ok(())
    }

    pub fn remove_selected(&mut self, index: usize) {
        if index < self.selected.len() {
            self.selected.remove(index);
        }
    }

    pub fn validated_request(&mut self) -> Result<ShopSearchRequest, String> {
        let min_price = match parse_optional_u32(self.min_text.trim(), "Minimum price") {
            Ok(price) => price,
            Err(message) => return self.fail(message),
        };
        let max_price = match parse_optional_u32(self.max_text.trim(), "Maximum price") {
            Ok(price) => price,
            Err(message) => return self.fail(message),
        };
        if max_price != 0 && min_price > max_price {
            return self.fail("Minimum price is above the maximum.");
        }
        if self.selected.is_empty() {
            return self.fail("Add at least one item.");
        }
        Ok(ShopSearchRequest {
            buying: self.buying,
            min_price,
            max_price,
            item_ids: self.selected.iter().map(|pick| pick.item_id).collect(),
        })
    }

    pub fn note_page(&mut self, next_page: bool, uses_remaining: u8) {
        self.has_next_page = next_page;
        self.status = format!("Shop search results ({uses_remaining} uses left).");
    }

    fn fail<T>(&mut self, message: impl Into<String>) -> Result<T, String> {
        let message = message.into();
        self.status = message.clone();
        Err(message)
    }
}

impl ShopReportState {
    pub fn set_stock(&mut self, buying: bool, stock: Vec<ShopReportLine>) {
        self.buying = buying;
        self.stock = stock;
        self.sales_log.clear();
        let kind = if buying { "buying store" } else { "vending shop" };
        self.status = format!("Opened {kind} with {} items.", self.stock.len());
    }

    pub fn note_sale(&mut self, index: u16, amount: u16, zeny: u32, date: u32) {
        let when = sale_when(date);
        let sold_name = self.stock.iter_mut().find(|line| line.index == index).map(|line| {
            line.amount = line.amount.saturating_sub(amount);
            line.label = format!("{} x{} {}z", line.name, line.amount, line.price);
            line.name.clone()
        });
        match sold_name {
            Some(name) => self.sales_log.push_str(&format!("Sold {name} x{amount} for {zeny}z{when}\n")),
            None => self.sales_log.push_str(&format!("Sold item {index} x{amount} for {zeny}z{when}\n")),
        }
    }
}

fn refresh_line(line: &mut ShopDraftLine, buying: bool) {
    let place = if buying { "Bag" } else { "Cart" };
    line.label = if line.listed {
        format!("Listed {} x{} {}z", line.name, line.amount, line.price)
    } else {
        format!("{place} {} x{}", line.name, line.available)
    };
}

fn parse_u16(text: &str, label: &str) -> Result<u16, String> {
    if text.is_empty() {
        return Err(format!("Enter an {label}."));
    }
    text.parse::<u16>().map_err(|_| format!("{label} must be a number."))
}

fn parse_price(text: &str, max: u32) -> Result<u32, String> {
    if text.is_empty() {
        return Err("Enter a price.".to_owned());
    }
    let price = text.parse::<u32>().map_err(|_| "Price must be a number.".to_owned())?;
    if price == 0 {
        return Err("Price must be at least 1.".to_owned());
    }
    if price > max {
        return Err(format!("Price must be at most {max}."));
    }
    Ok(price)
}

fn parse_optional_u32(text: &str, label: &str) -> Result<u32, String> {
    if text.is_empty() {
        return Ok(0);
    }
    text.parse::<u32>().map_err(|_| format!("{label} must be a number."))
}

fn parse_zeny(text: &str) -> Result<u32, String> {
    if text.is_empty() {
        return Err("Enter a zeny limit.".to_owned());
    }
    let zeny = text.parse::<u32>().map_err(|_| "Zeny limit must be a number.".to_owned())?;
    if zeny == 0 {
        return Err("Zeny limit must be at least 1.".to_owned());
    }
    if zeny > i32::MAX as u32 {
        return Err("Zeny limit is too high.".to_owned());
    }
    Ok(zeny)
}

fn sale_when(date: u32) -> String {
    if date == 0 {
        return String::new();
    }
    let Some(time) = chrono::DateTime::from_timestamp(i64::from(date), 0) else {
        return String::new();
    };
    format!(" ({})", time.format("%Y-%m-%d %H:%M"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(name: &str, stored: u16, item_id: u32, available: u16) -> ShopCandidate {
        ShopCandidate {
            label: name.to_owned(),
            name: name.to_owned(),
            stored_index: stored,
            item_id,
            available,
        }
    }

    fn list_line(setup: &mut ShopSetupState, index: usize, amount: &str, price: &str) -> Result<(), String> {
        setup.select(index);
        setup.amount_text = amount.to_owned();
        setup.price_text = price.to_owned();
        setup.commit_selected()
    }

    #[test]
    fn vending_prepare_count_limits_listed_items() {
        let mut setup = ShopSetupState::default();
        let candidates = (0..4)
            .map(|index| candidate(&format!("Item{index}"), index, 100 + u32::from(index), 5))
            .collect();
        setup.start_vending(3, candidates);
        assert_eq!(setup.max_items, 3);
        for index in 0..3 {
            assert!(list_line(&mut setup, index, "1", "10").is_ok());
        }
        let error = list_line(&mut setup, 3, "1", "10").unwrap_err();
        assert!(error.contains("full"));
    }

    #[test]
    fn prepare_counts_clamp_to_hercules_caps() {
        let mut setup = ShopSetupState::default();
        setup.start_vending(99, vec![candidate("Potion", 1, 501, 10)]);
        assert_eq!(setup.max_items, VENDING_MAX_ITEMS as u16);
        setup.start_vending(0, vec![candidate("Potion", 1, 501, 10)]);
        assert_eq!(setup.max_items, 0);
        assert!(setup.status.contains("did not allow"));
        setup.start_buying(99, vec![candidate("Potion", 1, 501, 10)]);
        assert_eq!(setup.max_items, BUYING_STORE_MAX_SLOTS as u16);
    }

    #[test]
    fn buying_candidates_skip_equipment() {
        let rows = candidates_from_sources(
            [
                BagSource {
                    name: "Sword".to_owned(),
                    stored_index: 1,
                    item_id: 1101,
                    amount: 1,
                    stackable: false,
                },
                BagSource {
                    name: "Potion".to_owned(),
                    stored_index: 2,
                    item_id: 501,
                    amount: 5,
                    stackable: true,
                },
            ],
            true,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].item_id, 501);
    }

    #[test]
    fn vending_price_and_amount_follow_the_cart_and_the_cap() {
        let mut setup = ShopSetupState::default();
        setup.start_vending(2, vec![candidate("Potion", 4, 501, 2)]);
        assert!(list_line(&mut setup, 0, "1", "0").is_err());
        assert!(list_line(&mut setup, 0, "1", "10000001").unwrap_err().contains("10000000"));
        assert!(list_line(&mut setup, 0, "3", "10").unwrap_err().contains("cart"));
        assert!(list_line(&mut setup, 0, "2", "10000000").is_ok());
        setup.title = "Potions".to_owned();
        assert_eq!(setup.open_request().unwrap(), ShopOpenRequest::Vending {
            title: "Potions".to_owned(),
            items: vec![(4, 2, 10_000_000)],
        });
    }

    #[test]
    fn buying_price_amount_zeny_and_duplicates_are_rejected() {
        let mut setup = ShopSetupState::default();
        setup.start_buying(5, vec![candidate("Potion", 1, 501, 10), candidate("Potion", 2, 501, 10)]);
        assert!(list_line(&mut setup, 0, "1", "0").is_err());
        assert!(list_line(&mut setup, 0, "1", "99990001").is_err());
        assert!(list_line(&mut setup, 0, "10000", "10").unwrap_err().contains("9999"));
        assert!(list_line(&mut setup, 0, "9990", "10").unwrap_err().contains("9999"));
        assert!(list_line(&mut setup, 0, "9", "99990000").is_ok());
        assert!(list_line(&mut setup, 1, "1", "10").unwrap_err().contains("already"));
        setup.title = "   ".to_owned();
        assert!(setup.open_request().unwrap_err().contains("title"));
        setup.title = "Buying".to_owned();
        setup.zeny_text = "0".to_owned();
        assert!(setup.open_request().unwrap_err().contains("Zeny"));
        setup.zeny_text = "5000".to_owned();
        assert_eq!(setup.open_request().unwrap(), ShopOpenRequest::Buying {
            title: "Buying".to_owned(),
            zeny_limit: 5000,
            items: vec![(501, 9, 99_990_000)],
        });
    }

    #[test]
    fn sold_report_decrements_the_matching_wire_index() {
        let mut report = ShopReportState::default();
        report.set_stock(false, vec![ShopReportLine {
            label: "Potion x4 100z".to_owned(),
            name: "Potion".to_owned(),
            index: 4,
            item_id: 501,
            amount: 4,
            price: 100,
        }]);
        report.note_sale(4, 1, 100, 0);
        assert_eq!(report.stock[0].amount, 3);
        assert!(report.stock[0].label.contains("x3"));
        assert!(report.sales_log.contains("Sold Potion x1 for 100z"));
        report.note_sale(9, 1, 20, 0);
        assert!(report.sales_log.contains("Sold item 9 x1 for 20z"));
    }

    #[test]
    fn search_requires_items_and_a_sane_price_range() {
        let mut search = ShopSearchState::default();
        assert!(search.validated_request().unwrap_err().contains("at least one"));
        search.set_matches(vec![(1750, "Arrow".to_owned())]);
        search.add_match(0).unwrap();
        search.min_text = "10".to_owned();
        search.max_text = "1".to_owned();
        assert!(search.validated_request().unwrap_err().contains("Minimum"));
        search.min_text.clear();
        search.max_text.clear();
        assert_eq!(search.validated_request().unwrap(), ShopSearchRequest {
            buying: false,
            min_price: 0,
            max_price: 0,
            item_ids: vec![1750],
        });
        search.set_buying(true);
        assert!(search.validated_request().unwrap().buying);
    }

    #[test]
    fn name_search_needs_two_characters_or_an_exact_id() {
        let catalog = [(501, "Red Potion"), (502, "Orange Potion"), (1750, "Arrow")];
        assert!(match_item_names(catalog, "p", 10).is_empty());
        let names = match_item_names(catalog, "po", 10);
        assert_eq!(names.len(), 2);
        assert_eq!(match_item_names(catalog, "501", 10), vec![(501, "Red Potion".to_owned())]);
        assert!(match_item_names(catalog, "50", 10).is_empty());
        assert!(match_item_names(catalog, "ARROW", 10).iter().any(|(id, _)| *id == 1750));
    }
}
