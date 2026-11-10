//! Barter shop the player is looking at. Offers come from the server. The cart
//! is local until Purchase is pressed.

use std::sync::Arc;

use korangar_networking::{BarterOfferData, InventoryItemDetails, ItemQuantity, NetworkingSystem, NoMetadata, ShopItem};
use ragnarok_packets::{InventoryIndex, ItemId, Price};
use rust_state::{PathExt, RustState, State};

use super::inventory::InventoryPathExt;
use super::{ClientState, ClientStatePathExt, client_state};
use crate::graphics::Texture;
use crate::loaders::AsyncLoader;
use crate::world::ResourceMetadata;

/// `u32::MAX` stock is the server's unlimited quantity (`qty == -1`).
pub const UNLIMITED_STOCK: u32 = u32::MAX;

const CART_CAP: u32 = 9_999;

/// One material for a single copy of an offer.
#[derive(Debug, Clone)]
pub struct BarterCurrency {
    pub item_id: ItemId,
    pub name: String,
    pub texture: Option<Arc<Texture>>,
    pub amount: u32,
    pub refine_level: u16,
}

/// One thing the NPC will trade.
#[derive(Debug, Clone)]
pub struct BarterOffer {
    pub item_id: ItemId,
    pub name: String,
    pub texture: Option<Arc<Texture>>,
    /// Server item type. The list shows the item name; the type stays on the
    /// offer.
    #[allow(dead_code)]
    pub item_type: u16,
    pub stock: u32,
    pub weight: u32,
    pub shop_index: u32,
    pub zeny: u32,
    pub currencies: Vec<BarterCurrency>,
}

/// A line the player has put in the cart. Keyed by shop index, because the
/// same item can be sold for two different prices.
#[derive(Debug, Clone)]
pub struct BarterCartLine {
    pub shop_index: u32,
    pub item_id: ItemId,
    pub name: String,
    pub texture: Option<Arc<Texture>>,
    pub amount: u32,
    pub zeny: u32,
    pub currencies: Vec<BarterCurrency>,
}

/// One stack the player is holding, used to pick a currency slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldStack {
    pub index: InventoryIndex,
    pub item_id: ItemId,
    pub amount: u32,
    pub refine: u16,
}

/// A purchase line ready for the packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedPurchase {
    pub item_id: ItemId,
    pub amount: u32,
    pub shop_index: u32,
    /// Set for simple barter. The wire value is this index plus 2.
    pub inventory_index: Option<InventoryIndex>,
}

#[derive(Debug, Clone, RustState)]
pub struct BarterShopState {
    open: bool,
    /// Escape or the title-bar close still has to tell the server.
    close_pending: bool,
    expanded: bool,
    pending: bool,
    heading: String,
    status: String,
    cart_summary: String,
    offers: Vec<BarterOffer>,
    cart: Vec<BarterCartLine>,
}

impl Default for BarterShopState {
    fn default() -> Self {
        Self {
            open: false,
            close_pending: false,
            expanded: false,
            pending: false,
            heading: "Barter".to_owned(),
            status: String::new(),
            cart_summary: "Cart is empty.".to_owned(),
            offers: Vec::new(),
            cart: Vec::new(),
        }
    }
}

impl BarterShopState {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn open_shop(&mut self, expanded: bool, offers: Vec<BarterOffer>) {
        self.expanded = expanded;
        self.offers = offers;
        self.cart.clear();
        self.pending = false;
        self.open = true;
        self.close_pending = true;
        self.heading = if expanded {
            "Expanded barter".to_owned()
        } else {
            "Barter".to_owned()
        };
        self.status = "Choose an amount, then purchase.".to_owned();
        self.refresh_summary();
    }

    pub fn can_add(&self, offer_index: usize, amount: u32) -> bool {
        let Some(offer) = self.offers.get(offer_index) else {
            return false;
        };
        room(offer, &self.cart) >= amount && amount > 0
    }

    pub fn purchase_blocked(&self) -> bool {
        self.cart.is_empty() || self.pending || !self.open
    }

    pub fn add_to_cart(&mut self, offer_index: usize, amount: u32) -> bool {
        let Some(offer) = self.offers.get(offer_index) else {
            return false;
        };
        let add = amount.min(room(offer, &self.cart));
        if add == 0 {
            return false;
        }
        let shop_index = offer.shop_index;
        if let Some(line) = self.cart.iter_mut().find(|line| line.shop_index == shop_index) {
            line.amount += add;
        } else {
            self.cart.push(BarterCartLine {
                shop_index,
                item_id: offer.item_id,
                name: offer.name.clone(),
                texture: offer.texture.clone(),
                amount: add,
                zeny: offer.zeny,
                currencies: offer.currencies.clone(),
            });
        }
        self.refresh_summary();
        true
    }

    pub fn remove_cart_line(&mut self, index: usize) {
        if index < self.cart.len() {
            self.cart.remove(index);
            self.refresh_summary();
        }
    }

    pub fn mark_pending(&mut self) {
        self.pending = true;
        self.status = "Exchanging...".to_owned();
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    /// The Close button already sent the packet.
    pub fn mark_button_close(&mut self) {
        self.close_pending = false;
        self.open = false;
        self.pending = false;
        self.cart.clear();
        self.refresh_summary();
    }

    /// Title-bar or Escape removed the window without the Close button.
    pub fn take_unsolicited_close(&mut self, window_still_open: bool) -> bool {
        if self.close_pending && !window_still_open {
            self.close_pending = false;
            self.open = false;
            self.pending = false;
            true
        } else {
            false
        }
    }

    pub fn apply_icon(&mut self, item_id: ItemId, texture: Arc<Texture>) {
        for offer in &mut self.offers {
            if offer.item_id == item_id {
                offer.texture = Some(texture.clone());
            }
            for currency in &mut offer.currencies {
                if currency.item_id == item_id {
                    currency.texture = Some(texture.clone());
                }
            }
        }
        for line in &mut self.cart {
            if line.item_id == item_id {
                line.texture = Some(texture.clone());
            }
            for currency in &mut line.currencies {
                if currency.item_id == item_id {
                    currency.texture = Some(texture.clone());
                }
            }
        }
    }

    fn refresh_summary(&mut self) {
        if self.cart.is_empty() {
            self.cart_summary = "Cart is empty.".to_owned();
            return;
        }
        let mut lines = Vec::new();
        let mut zeny = 0u64;
        for line in &self.cart {
            zeny += u64::from(line.zeny) * u64::from(line.amount);
            lines.push(format!(
                "{} x{} ({})",
                line.name,
                line.amount,
                cost_label(line.zeny, &line.currencies, line.amount)
            ));
        }
        if zeny > 0 {
            lines.push(format!("Zeny {zeny}"));
        }
        self.cart_summary = lines.join("\n");
    }
}

fn room(offer: &BarterOffer, cart: &[BarterCartLine]) -> u32 {
    let in_cart = cart
        .iter()
        .find(|line| line.shop_index == offer.shop_index)
        .map(|line| line.amount)
        .unwrap_or(0);
    let cap = if offer.stock == UNLIMITED_STOCK { CART_CAP } else { offer.stock };
    cap.saturating_sub(in_cart)
}

/// Text for the materials and zeny of `multiplier` purchases.
pub fn cost_label(zeny: u32, currencies: &[BarterCurrency], multiplier: u32) -> String {
    let mut parts = Vec::new();
    let zeny_total = u64::from(zeny) * u64::from(multiplier);
    if zeny_total > 0 {
        parts.push(format!("{zeny_total}z"));
    }
    for currency in currencies {
        let amount = u64::from(currency.amount) * u64::from(multiplier);
        if currency.refine_level > 0 {
            parts.push(format!("+{} {} x{amount}", currency.refine_level, currency.name));
        } else {
            parts.push(format!("{} x{amount}", currency.name));
        }
    }
    if parts.is_empty() {
        "no extra cost".to_owned()
    } else {
        parts.join(", ")
    }
}

pub fn stock_label(stock: u32) -> String {
    if stock == UNLIMITED_STOCK {
        "unlimited".to_owned()
    } else {
        format!("stock {stock}")
    }
}

/// Stock plus the item weight. Hercules sends `id->weight * 10` on this
/// packetver.
pub fn offer_stock_text(offer: &BarterOffer) -> String {
    let stock = stock_label(offer.stock);
    let weight = offer.weight / 10;
    if weight == 0 {
        stock
    } else {
        format!("{stock} · weight {weight}")
    }
}

/// Build the purchase packet lines. Simple barter needs one inventory slot
/// that holds the whole currency cost. Expanded barter checks the totals and
/// lets the server pick the slots.
pub fn plan_purchase(shop: &BarterShopState, held: &[HeldStack]) -> Result<Vec<PlannedPurchase>, String> {
    if shop.cart.is_empty() {
        return Err("The cart is empty.".to_owned());
    }
    if shop.expanded {
        plan_expanded(shop, held)
    } else {
        plan_simple(shop, held)
    }
}

fn plan_simple(shop: &BarterShopState, held: &[HeldStack]) -> Result<Vec<PlannedPurchase>, String> {
    let mut reserved: Vec<(InventoryIndex, u32)> = Vec::new();
    let mut planned = Vec::with_capacity(shop.cart.len());
    for line in &shop.cart {
        let Some(currency) = line.currencies.first() else {
            return Err(format!("{} has no material.", line.name));
        };
        let need = u64::from(currency.amount) * u64::from(line.amount);
        if need == 0 || need > u64::from(u32::MAX) {
            return Err(format!("Not enough {}.", currency.name));
        }
        let need = need as u32;
        let slot = held.iter().find(|stack| {
            if stack.item_id != currency.item_id || stack.amount < need {
                return false;
            }
            let used = reserved
                .iter()
                .find(|(index, _)| *index == stack.index)
                .map(|(_, used)| *used)
                .unwrap_or(0);
            stack.amount.saturating_sub(used) >= need
        });
        let Some(slot) = slot else {
            return Err(format!("Not enough {}.", currency.name));
        };
        if let Some((_, used)) = reserved.iter_mut().find(|(index, _)| *index == slot.index) {
            *used += need;
        } else {
            reserved.push((slot.index, need));
        }
        planned.push(PlannedPurchase {
            item_id: line.item_id,
            amount: line.amount,
            shop_index: line.shop_index,
            inventory_index: Some(slot.index),
        });
    }
    Ok(planned)
}

fn plan_expanded(shop: &BarterShopState, held: &[HeldStack]) -> Result<Vec<PlannedPurchase>, String> {
    let mut need: Vec<(ItemId, u16, u32, String)> = Vec::new();
    for line in &shop.cart {
        for currency in &line.currencies {
            let amount = u64::from(currency.amount) * u64::from(line.amount);
            if amount > u64::from(u32::MAX) {
                return Err(format!("Not enough {}.", currency.name));
            }
            let amount = amount as u32;
            if let Some(entry) = need
                .iter_mut()
                .find(|(item_id, refine, ..)| *item_id == currency.item_id && *refine == currency.refine_level)
            {
                entry.2 = entry.2.saturating_add(amount);
            } else {
                need.push((currency.item_id, currency.refine_level, amount, currency.name.clone()));
            }
        }
    }
    for (item_id, refine, amount, name) in &need {
        let have = held
            .iter()
            .filter(|stack| stack.item_id == *item_id && (*refine == 0 || stack.refine == *refine))
            .map(|stack| u64::from(stack.amount))
            .sum::<u64>();
        if have < u64::from(*amount) {
            return Err(format!("Not enough {name}."));
        }
    }
    Ok(shop
        .cart
        .iter()
        .map(|line| PlannedPurchase {
            item_id: line.item_id,
            amount: line.amount,
            shop_index: line.shop_index,
            inventory_index: None,
        })
        .collect())
}

fn named_shop_item(loader: &AsyncLoader, item_id: ItemId, item_type: u16) -> ShopItem<ResourceMetadata> {
    loader.request_shop_item_metadata_load(ShopItem {
        metadata: NoMetadata,
        item_id,
        item_type: u8::try_from(item_type).unwrap_or(0),
        price: Price(0),
        quantity: ItemQuantity::Fixed(1),
        weight: 0,
        location: 0,
    })
}

/// Turn a catalog packet into offers with names and icons.
pub fn offers_from_catalog(loader: &AsyncLoader, offers: Vec<BarterOfferData>) -> Vec<BarterOffer> {
    offers
        .into_iter()
        .map(|offer| {
            let reward = named_shop_item(loader, offer.item_id, offer.item_type);
            let currencies = offer
                .currencies
                .into_iter()
                .map(|currency| {
                    let named = named_shop_item(loader, currency.item_id, currency.item_type);
                    BarterCurrency {
                        item_id: currency.item_id,
                        name: named.metadata.name,
                        texture: named.metadata.texture,
                        amount: currency.amount,
                        refine_level: currency.refine_level,
                    }
                })
                .collect();
            BarterOffer {
                item_id: offer.item_id,
                name: reward.metadata.name,
                texture: reward.metadata.texture,
                item_type: offer.item_type,
                stock: offer.stock,
                weight: offer.weight,
                shop_index: offer.shop_index,
                zeny: offer.zeny,
                currencies,
            }
        })
        .collect()
}

/// Plan the cart against the inventory and send the exchange.
pub fn submit_barter_purchase<Callback>(networking: &mut NetworkingSystem<Callback>, state: &mut State<ClientState>) {
    let held = state
        .follow(client_state().inventory().items())
        .iter()
        .map(|item| {
            let (amount, refine) = match &item.details {
                InventoryItemDetails::Regular { amount, .. } => (u32::from(*amount), 0),
                InventoryItemDetails::Equippable { refinement_level, .. } => (1, u16::from(*refinement_level)),
            };
            HeldStack {
                index: item.index,
                item_id: item.item_id,
                amount,
                refine,
            }
        })
        .collect::<Vec<_>>();
    let outcome = {
        let shop = state.follow(client_state().barter());
        if shop.purchase_blocked() {
            None
        } else {
            Some((shop.is_expanded(), plan_purchase(shop, &held)))
        }
    };
    if let Some((expanded, result)) = outcome {
        match result {
            Ok(lines) => {
                state.follow_mut(client_state().barter()).mark_pending();
                let wire = lines
                    .into_iter()
                    .map(|line| (line.item_id, line.shop_index, line.amount, line.inventory_index))
                    .collect();
                let _ = networking.purchase_barter(expanded, wire);
            }
            Err(message) => {
                state.follow_mut(client_state().barter()).set_status(message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{InventoryIndex, ItemId};

    use super::{
        BarterCartLine, BarterCurrency, BarterOffer, BarterShopState, HeldStack, cost_label, offer_stock_text, plan_purchase, stock_label,
    };

    fn currency(item_id: u32, name: &str, amount: u32, refine: u16) -> BarterCurrency {
        BarterCurrency {
            item_id: ItemId(item_id),
            name: name.to_owned(),
            texture: None,
            amount,
            refine_level: refine,
        }
    }

    fn offer(item_id: u32, name: &str, stock: u32, shop_index: u32, zeny: u32, currencies: Vec<BarterCurrency>) -> BarterOffer {
        BarterOffer {
            item_id: ItemId(item_id),
            name: name.to_owned(),
            texture: None,
            item_type: 0,
            stock,
            weight: 10,
            shop_index,
            zeny,
            currencies,
        }
    }

    #[test]
    fn cart_stops_at_stock_and_keeps_separate_prices() {
        let mut shop = BarterShopState::default();
        shop.open_shop(false, vec![
            offer(509, "White Herb", 5, 0, 0, vec![currency(501, "Red Potion", 2, 0)]),
            offer(509, "White Herb", 100, 3, 0, vec![currency(502, "Orange Potion", 1, 0)]),
        ]);
        assert!(shop.add_to_cart(0, 10));
        assert_eq!(shop.cart[0].amount, 5);
        assert!(!shop.can_add(0, 1));
        assert!(shop.add_to_cart(1, 2));
        assert_eq!(shop.cart.len(), 2);
        assert_eq!(shop.cart[1].shop_index, 3);
        assert_eq!(shop.cart[1].amount, 2);
        shop.remove_cart_line(0);
        assert_eq!(shop.cart.len(), 1);
        assert!(shop.can_add(0, 1));
    }

    #[test]
    fn unlimited_stock_caps_the_cart() {
        let mut shop = BarterShopState::default();
        shop.open_shop(false, vec![offer(512, "Apple", u32::MAX, 4, 0, vec![currency(
            502,
            "Orange Potion",
            1,
            0,
        )])]);
        assert!(shop.add_to_cart(0, 100));
        assert_eq!(shop.cart[0].amount, 100);
        assert_eq!(stock_label(u32::MAX), "unlimited");
        assert_eq!(cost_label(0, &shop.cart[0].currencies, 3), "Orange Potion x3");
        let shown = offer(512, "Apple", u32::MAX, 4, 0, Vec::new());
        assert_eq!(offer_stock_text(&shown), "unlimited · weight 1");
        let mut hidden = shown;
        hidden.weight = 9;
        assert_eq!(offer_stock_text(&hidden), "unlimited");
    }

    #[test]
    fn simple_purchase_uses_one_stack() {
        let mut shop = BarterShopState::default();
        shop.open_shop(false, vec![offer(509, "White Herb", 100, 0, 0, vec![currency(
            501,
            "Red Potion",
            2,
            0,
        )])]);
        shop.add_to_cart(0, 3);
        let held = vec![
            HeldStack {
                index: InventoryIndex(1),
                item_id: ItemId(501),
                amount: 4,
                refine: 0,
            },
            HeldStack {
                index: InventoryIndex(2),
                item_id: ItemId(501),
                amount: 10,
                refine: 0,
            },
        ];
        let error = plan_purchase(&shop, &held[..1]).unwrap_err();
        assert_eq!(error, "Not enough Red Potion.");
        let planned = plan_purchase(&shop, &held).unwrap();
        assert_eq!(planned[0].inventory_index, Some(InventoryIndex(2)));
        assert_eq!(planned[0].amount, 3);
        assert_eq!(planned[0].shop_index, 0);
    }

    #[test]
    fn expanded_purchase_sums_materials_and_refine() {
        let mut shop = BarterShopState::default();
        shop.open_shop(true, vec![offer(502, "Orange Potion", 100_000, 0, 2, vec![
            currency(513, "Banana", 2, 0),
            currency(512, "Apple", 1, 3),
        ])]);
        shop.add_to_cart(0, 2);
        let short = vec![HeldStack {
            index: InventoryIndex(0),
            item_id: ItemId(513),
            amount: 4,
            refine: 0,
        }];
        assert_eq!(plan_purchase(&shop, &short).unwrap_err(), "Not enough Apple.");
        let held = vec![short[0].clone(), HeldStack {
            index: InventoryIndex(1),
            item_id: ItemId(512),
            amount: 2,
            refine: 3,
        }];
        let planned = plan_purchase(&shop, &held).unwrap();
        assert!(planned[0].inventory_index.is_none());
        assert_eq!(planned[0].amount, 2);
        assert!(shop.cart_summary.contains("4z") || shop.cart_summary.contains("Zeny 4"));
        let _ = BarterCartLine {
            shop_index: 0,
            item_id: ItemId(502),
            name: "Orange Potion".to_owned(),
            texture: None,
            amount: 2,
            zeny: 2,
            currencies: Vec::new(),
        };
    }

    #[test]
    fn escape_close_is_reported_once() {
        let mut shop = BarterShopState::default();
        shop.open_shop(true, Vec::new());
        assert!(!shop.take_unsolicited_close(true));
        assert!(shop.take_unsolicited_close(false));
        assert!(!shop.take_unsolicited_close(false));
        shop.open_shop(false, Vec::new());
        shop.mark_button_close();
        assert!(!shop.take_unsolicited_close(false));
    }
}
