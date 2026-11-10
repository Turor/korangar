//! Open, search, report, and close this client's player shops.
//!
//! Packet sends stay in `korangar-networking`. This module only turns cart and
//! bag rows into the setup window and applies the server's own-list and sold
//! report onto the report window.

use korangar_networking::{InventoryItemDetails, ItemQuantity, MessageColor, NoMetadata, OwnShopLine, SearchStoreHit, ShopItem};
use ragnarok_packets::{ItemId, Price};

use crate::Client;
use crate::interface::windows::{SearchStoreWindow, VendingReportWindow, VendingSetupWindow, WindowClass};
use crate::state::inventory::InventoryPathExt;
use crate::state::item_finder::MISSING_CATALOG_STATUS;
use crate::state::player_shops::{
    self, BagSource, SEARCH_ITEM_LIMIT, ShopOpenRequest, ShopReportLine, ShopReportStatePathExt, ShopSearchStatePathExt,
    ShopSetupStatePathExt,
};
use crate::state::{ChatMessage, ClientStatePathExt, client_state};
use crate::world::ResourceMetadata;

impl Client {
    pub(crate) fn on_player_shop_sign(&mut self, account_id: ragnarok_packets::AccountId, shop_name: String, buying_store: bool) {
        let shops = self.client_state.follow_mut(client_state().player_shops());
        if let Some(existing) = shops.iter_mut().find(|shop| shop.account_id == account_id) {
            existing.buying_store = buying_store;
            existing.shop_name = shop_name;
        } else {
            shops.push(crate::state::ShopSign {
                account_id,
                shop_name,
                buying_store,
            });
        }
    }

    pub(crate) fn on_vending_prepare(&mut self, item_count: u16) {
        let candidates = self.bag_candidates(false);
        self.client_state
            .follow_mut(client_state().shop_setup())
            .start_vending(item_count, candidates);
        self.reopen_shop_setup();
        self.shop_chat(
            format!("You can list {item_count} items in a vending shop."),
            MessageColor::Information,
        );
    }

    pub(crate) fn on_buying_prepare(&mut self, item_count: u8) {
        let candidates = self.bag_candidates(true);
        self.client_state
            .follow_mut(client_state().shop_setup())
            .start_buying(item_count, candidates);
        self.reopen_shop_setup();
        self.shop_chat(
            format!("You can list {item_count} items in a buying store."),
            MessageColor::Information,
        );
    }

    pub(crate) fn on_vending_open_result(&mut self, result: u8) {
        let message = match result {
            0 | 2 => return,
            1 => "Could not open the vending shop.".to_owned(),
            3 => "You cannot open a shop on this cell.".to_owned(),
            other => format!("Could not open the vending shop (code {other})."),
        };
        *self.client_state.follow_mut(client_state().shop_setup().status()) = message.clone();
        self.shop_chat(message, MessageColor::Error);
    }

    pub(crate) fn on_own_shop_opened(&mut self, buying_store: bool, lines: Vec<OwnShopLine>) {
        let stock = lines
            .into_iter()
            .map(|line| {
                let name = self.shop_item_name(line.item_id, line.price, line.amount);
                ShopReportLine {
                    label: format!("{name} x{} {}z", line.amount, line.price),
                    name,
                    index: line.index,
                    item_id: line.item_id.0,
                    amount: line.amount,
                    price: line.price,
                }
            })
            .collect::<Vec<_>>();
        let count = stock.len();
        self.client_state
            .follow_mut(client_state().shop_report())
            .set_stock(buying_store, stock);
        self.interface.close_window_with_class(WindowClass::VendingSetup);
        self.interface.close_window_with_class(WindowClass::VendingReport);
        self.interface.open_window(VendingReportWindow::new(client_state().shop_report()));
        let kind = if buying_store { "buying store" } else { "vending shop" };
        self.shop_chat(format!("Opened {kind} with {count} items."), MessageColor::Information);
    }

    pub(crate) fn on_vending_sold(&mut self, index: u16, amount: u16, zeny: u32, date: u32) {
        self.client_state
            .follow_mut(client_state().shop_report())
            .note_sale(index, amount, zeny, date);
    }

    pub(crate) fn on_search_store_open(&mut self, remaining_uses: u8) {
        self.client_state.follow_mut(client_state().shop_search()).begin(remaining_uses);
        self.open_search_store_window();
        self.shop_chat(
            format!("Shop search is available ({remaining_uses} uses left)."),
            MessageColor::Information,
        );
    }

    pub(crate) fn on_search_store_results(&mut self, first_page: bool, next_page: bool, uses_remaining: u8, hits: Vec<SearchStoreHit>) {
        if first_page {
            self.client_state.follow_mut(client_state().search_store_hits()).clear();
        }
        let mut rows = Vec::with_capacity(hits.len());
        for hit in hits {
            let name = self.shop_item_name(hit.item_id, hit.price.0, hit.amount);
            rows.push(crate::state::SearchStoreRow {
                label: format!("{} — {name}  x{}  {}z", hit.shop_name, hit.amount, hit.price.0),
                account_id: hit.account_id,
                store_id: hit.store_id,
                item_id: hit.item_id,
            });
        }
        self.client_state.follow_mut(client_state().search_store_hits()).extend(rows);
        self.client_state
            .follow_mut(client_state().shop_search())
            .note_page(next_page, uses_remaining);
        self.open_search_store_window();
        self.shop_chat(
            format!("Shop search results ({uses_remaining} uses left)."),
            MessageColor::Information,
        );
    }

    pub(crate) fn select_shop_draft(&mut self, index: usize) {
        self.client_state.follow_mut(client_state().shop_setup()).select(index);
    }

    pub(crate) fn commit_shop_draft(&mut self) {
        let _ = self.client_state.follow_mut(client_state().shop_setup()).commit_selected();
    }

    pub(crate) fn unlist_shop_draft(&mut self) {
        self.client_state.follow_mut(client_state().shop_setup()).unlist_selected();
    }

    pub(crate) fn submit_shop_setup(&mut self) {
        let request = {
            let setup = self.client_state.follow_mut(client_state().shop_setup());
            let request = setup.open_request();
            if request.is_ok() {
                setup.status = "Opening the shop.".to_owned();
            }
            request
        };
        match request {
            Ok(ShopOpenRequest::Vending { title, items }) => {
                let _ = self.networking_system.open_vending_shop(&title, &items);
            }
            Ok(ShopOpenRequest::Buying { title, zeny_limit, items }) => {
                let _ = self.networking_system.open_buying_store(&title, zeny_limit, &items);
            }
            Err(_) => {}
        }
    }

    pub(crate) fn cancel_shop_setup(&mut self) {
        self.send_shop_setup_cancel();
        self.interface.close_window_with_class(WindowClass::VendingSetup);
    }

    pub(crate) fn send_shop_setup_cancel(&mut self) {
        let buying = *self.client_state.follow(client_state().shop_setup().buying());
        if buying {
            let _ = self.networking_system.cancel_open_buying_store();
        } else {
            let _ = self.networking_system.cancel_open_vending_shop();
        }
    }

    pub(crate) fn close_own_shop(&mut self) {
        self.send_close_own_shop();
        self.interface.close_window_with_class(WindowClass::VendingReport);
    }

    fn send_close_own_shop(&mut self) {
        let buying = *self.client_state.follow(client_state().shop_report().buying());
        if buying {
            let _ = self.networking_system.close_buying_store();
        } else {
            let _ = self.networking_system.close_vending_shop();
        }
    }

    /// The title-bar X removes the window inside the interface and does not
    /// emit an input event. Escape closes through
    /// [`Self::on_close_top_window`]. Either way, a dismissed setup,
    /// search, or report must tell the server.
    pub(crate) fn on_shop_windows_dismissed(&mut self, setup_was_open: bool, search_was_open: bool, report_was_open: bool) {
        if setup_was_open && !self.interface.is_window_with_class_open(WindowClass::VendingSetup) {
            self.send_shop_setup_cancel();
        }
        if search_was_open && !self.interface.is_window_with_class_open(WindowClass::SearchStore) {
            let _ = self.networking_system.close_search_store();
        }
        if report_was_open && !self.interface.is_window_with_class_open(WindowClass::VendingReport) {
            self.send_close_own_shop();
        }
    }

    pub(crate) fn suggest_shop_search(&mut self) {
        let query = self.client_state.follow(client_state().shop_search().query()).clone();
        let catalog_empty = self.client_state.follow(client_state().item_finder()).catalog_is_empty();
        let mut picks = self
            .client_state
            .follow(client_state().item_finder())
            .name_matches(&query, SEARCH_ITEM_LIMIT);
        let bag = self.bag_name_pairs();
        let bag_hits = player_shops::match_item_names(bag.iter().map(|(id, name)| (*id, name.as_str())), &query, SEARCH_ITEM_LIMIT);
        player_shops::extend_name_matches(&mut picks, bag_hits, SEARCH_ITEM_LIMIT);
        let trimmed = query.trim();
        let short_text = trimmed.chars().count() < 2 && !trimmed.bytes().all(|byte| byte.is_ascii_digit());
        let status = if picks.is_empty() && catalog_empty {
            format!("{MISSING_CATALOG_STATUS} No inventory or cart name matched.")
        } else if picks.is_empty() && short_text {
            "Type at least 2 letters, or an item id.".to_owned()
        } else if picks.is_empty() {
            "No matching items.".to_owned()
        } else if catalog_empty {
            format!("{MISSING_CATALOG_STATUS} Showing inventory and cart names.")
        } else {
            format!("{} matching items.", picks.len())
        };
        let search = self.client_state.follow_mut(client_state().shop_search());
        search.set_matches(picks);
        search.status = status;
    }

    pub(crate) fn add_shop_search_item(&mut self, index: usize) {
        let _ = self.client_state.follow_mut(client_state().shop_search()).add_match(index);
    }

    pub(crate) fn remove_shop_search_item(&mut self, index: usize) {
        self.client_state.follow_mut(client_state().shop_search()).remove_selected(index);
    }

    pub(crate) fn set_shop_search_buying(&mut self, buying: bool) {
        self.client_state.follow_mut(client_state().shop_search()).set_buying(buying);
    }

    pub(crate) fn submit_shop_search(&mut self) {
        let request = self.client_state.follow_mut(client_state().shop_search()).validated_request();
        if let Ok(request) = request {
            let _ = self
                .networking_system
                .search_stores(request.buying, request.min_price, request.max_price, &request.item_ids);
        }
    }

    pub(crate) fn shop_search_next_page(&mut self) {
        let has_next = *self.client_state.follow(client_state().shop_search().has_next_page());
        if has_next {
            let _ = self.networking_system.search_store_next_page();
        }
    }

    pub(crate) fn close_shop_search(&mut self) {
        let _ = self.networking_system.close_search_store();
        self.interface.close_window_with_class(WindowClass::SearchStore);
    }

    pub(crate) fn on_close_top_window(&mut self) {
        let setup_open = self.interface.is_window_with_class_open(WindowClass::VendingSetup);
        let search_open = self.interface.is_window_with_class_open(WindowClass::SearchStore);
        let report_open = self.interface.is_window_with_class_open(WindowClass::VendingReport);
        self.interface.close_top_window(&self.client_state);
        self.on_shop_windows_dismissed(setup_open, search_open, report_open);
    }

    fn reopen_shop_setup(&mut self) {
        self.interface.close_window_with_class(WindowClass::VendingSetup);
        self.interface.open_window(VendingSetupWindow::new(client_state().shop_setup()));
    }

    fn open_search_store_window(&mut self) {
        if !self.interface.is_window_with_class_open(WindowClass::SearchStore) {
            self.interface.open_window(SearchStoreWindow::new(
                client_state().search_store_hits(),
                client_state().shop_search(),
            ));
        }
    }

    fn bag_candidates(&self, stackable_only: bool) -> Vec<player_shops::ShopCandidate> {
        if stackable_only {
            let items = self.client_state.follow(client_state().inventory().items());
            player_shops::candidates_from_sources(items.iter().map(source_from_item), true)
        } else {
            let items = self.client_state.follow(client_state().cart().items());
            player_shops::candidates_from_sources(items.iter().map(source_from_item), false)
        }
    }

    fn bag_name_pairs(&self) -> Vec<(u32, String)> {
        let mut pairs = Vec::new();
        for item in self.client_state.follow(client_state().inventory().items()).iter() {
            push_name(&mut pairs, item);
        }
        for item in self.client_state.follow(client_state().cart().items()).iter() {
            push_name(&mut pairs, item);
        }
        pairs
    }

    fn shop_item_name(&self, item_id: ItemId, price: u32, amount: u16) -> String {
        let named = self.async_loader.request_shop_item_metadata_load(ShopItem {
            metadata: NoMetadata,
            item_id,
            item_type: 0,
            price: Price(price),
            quantity: ItemQuantity::Fixed(u32::from(amount)),
            weight: 0,
            location: 0,
        });
        if named.metadata.name.is_empty() {
            format!("Item {}", item_id.0)
        } else {
            named.metadata.name
        }
    }

    fn shop_chat(&mut self, text: String, color: MessageColor) {
        self.client_state
            .follow_mut(client_state().chat_messages())
            .push(ChatMessage::new(text, color));
    }
}

fn source_from_item(item: &korangar_networking::InventoryItem<ResourceMetadata>) -> BagSource {
    let (amount, stackable) = match &item.details {
        InventoryItemDetails::Regular { amount, .. } => (*amount, true),
        InventoryItemDetails::Equippable { .. } => (1, false),
    };
    let name = if item.metadata.name.is_empty() {
        format!("Item {}", item.item_id.0)
    } else {
        item.metadata.name.clone()
    };
    BagSource {
        name,
        stored_index: item.index.0,
        item_id: item.item_id.0,
        amount,
        stackable,
    }
}

fn push_name(pairs: &mut Vec<(u32, String)>, item: &korangar_networking::InventoryItem<ResourceMetadata>) {
    if item.metadata.name.is_empty() {
        return;
    }
    let id = item.item_id.0;
    if pairs.iter().any(|(existing, _)| *existing == id) {
        return;
    }
    pairs.push((id, item.metadata.name.clone()));
}
