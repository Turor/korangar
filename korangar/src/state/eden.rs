//! Eden Group board: mission bands, merit shop, and `0x0EF3` button packets.
//!
//! A buy has no amount field on the wire. One click of Buy asks how many
//! stacks, and each stack is its own `0x0EF3` action 4 for that shop slot.

use std::sync::Arc;

use korangar_interface::element::StateElement;
use korangar_networking::format_eden_result;
pub use korangar_networking::{EdenButton, eden_button_packets};
use ragnarok_packets::{EdenQuestEntry, EdenShopEntry, RequestEdenPacket};
use rust_state::RustState;

use crate::graphics::Texture;

pub const EDEN_PANE_MISSIONS: u8 = 0;
pub const EDEN_PANE_SHOP: u8 = 1;
/// Server accepts one stack per buy packet. Cap a single prompt so a typo
/// cannot flood the map server.
pub const EDEN_BUY_MAX: u16 = 99;

pub const EDEN_STATE_AVAILABLE: u8 = 0;
pub const EDEN_STATE_ACTIVE: u8 = 1;
pub const EDEN_STATE_TURNIN: u8 = 2;
pub const EDEN_STATE_COOLDOWN: u8 = 3;

/// One mission row. Buttons are derived from [`mission_actions`].
#[derive(Clone, Debug, RustState, StateElement)]
pub struct EdenMission {
    pub quest_id: i32,
    pub name: String,
    pub state: u8,
    pub state_label: String,
    pub min_level: i16,
    pub max_level: i16,
    pub progress: String,
    /// Server hint, painted under the hunt line so that line keeps the theme
    /// font size.
    pub hint: String,
    pub rewards: String,
    pub can_accept: bool,
    pub can_complete: bool,
    pub can_abandon: bool,
}

/// Missions that share a level band, in server order.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct EdenBand {
    pub label: String,
    pub min_level: i16,
    pub max_level: i16,
    pub missions: Vec<EdenMission>,
}

/// One merit-shop cell. `texture` is filled when the item icon loads.
#[derive(Debug, RustState, StateElement)]
pub struct EdenShopOffer {
    pub slot: i32,
    pub category: u8,
    pub category_label: String,
    pub item_id: i32,
    pub name: String,
    pub stack: i16,
    pub min_level: u8,
    pub price_merit: i32,
    pub price_zeny: i32,
    pub cost: String,
    pub detail: String,
    pub locked: bool,
    #[hidden_element]
    pub texture: Option<Arc<Texture>>,
}

#[derive(RustState, StateElement)]
pub struct EdenWindowState {
    headline: String,
    status: String,
    base_level: i16,
    merit: i32,
    pane: u8,
    bands: Vec<EdenBand>,
    offers: Vec<EdenShopOffer>,
    /// Shop slot waiting for an amount. Negative when the prompt is closed.
    buy_slot: i32,
    buy_prompt: String,
    buy_amount_text: String,
    buy_ghost: String,
}

impl Default for EdenWindowState {
    fn default() -> Self {
        Self {
            headline: "Eden Group".to_owned(),
            status: "Open the Eden board to load missions and the merit shop.".to_owned(),
            base_level: 1,
            merit: 0,
            pane: EDEN_PANE_MISSIONS,
            bands: Vec::new(),
            offers: Vec::new(),
            buy_slot: -1,
            buy_prompt: String::new(),
            buy_amount_text: String::new(),
            buy_ghost: "stacks".to_owned(),
        }
    }
}

impl EdenWindowState {
    pub(crate) fn bands(&self) -> &[EdenBand] {
        &self.bands
    }

    pub(crate) fn offers(&self) -> &[EdenShopOffer] {
        &self.offers
    }

    pub(crate) fn pane(&self) -> u8 {
        self.pane
    }

    pub(crate) fn base_level(&self) -> i16 {
        self.base_level
    }

    pub(crate) fn buying(&self) -> bool {
        self.buy_slot >= 0
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn set_pane(&mut self, pane: u8) {
        self.pane = if pane == EDEN_PANE_SHOP {
            EDEN_PANE_SHOP
        } else {
            EDEN_PANE_MISSIONS
        };
        if self.pane != EDEN_PANE_SHOP {
            self.clear_buy();
        }
        self.refresh_headline();
    }

    pub fn apply_quests(&mut self, base_level: i16, merit: i32, quests: &[EdenQuestEntry], names: &[(i32, String)]) {
        self.base_level = base_level;
        self.merit = merit;
        let missions = quests.iter().map(|quest| mission_from_entry(quest, names)).collect();
        self.bands = group_missions(missions);
        self.refresh_locks();
        self.refresh_headline();
        self.keep_buy_if_present();
    }

    pub fn apply_shop(&mut self, merit: i32, items: Vec<(EdenShopEntry, String, Option<Arc<Texture>>)>) {
        self.merit = merit;
        self.offers = items
            .into_iter()
            .map(|(entry, name, texture)| offer_from_entry(entry, name, texture, self.base_level))
            .collect();
        self.offers.sort_by(|left, right| {
            left.category
                .cmp(&right.category)
                .then(left.min_level.cmp(&right.min_level))
                .then(left.slot.cmp(&right.slot))
        });
        self.refresh_headline();
        self.keep_buy_if_present();
    }

    pub fn apply_result(&mut self, result: i16, quest_id: i32) {
        self.status = format_eden_result(result, quest_id);
        if result == 0 {
            self.clear_buy();
        }
    }

    pub fn update_item_sprite(&mut self, item_id: i32, texture: Arc<Texture>) {
        for offer in &mut self.offers {
            if offer.item_id == item_id {
                offer.texture = Some(texture.clone());
            }
        }
    }

    pub fn begin_buy(&mut self, slot: i32) {
        let Some(offer) = self.offers.iter().find(|offer| offer.slot == slot) else {
            self.set_status("That shop item is no longer listed.".to_owned());
            return;
        };
        if offer.locked {
            self.set_status(format!("{} needs base level {}.", offer.name, offer.min_level));
            return;
        }
        self.pane = EDEN_PANE_SHOP;
        self.buy_slot = slot;
        self.buy_amount_text = "1".to_owned();
        self.buy_prompt = format!(
            "Buy {} ({}) for {} each. How many stacks?",
            offer.name, offer.detail, offer.cost
        );
        self.refresh_headline();
    }

    pub fn clear_buy(&mut self) {
        self.buy_slot = -1;
        self.buy_prompt.clear();
        self.buy_amount_text.clear();
    }

    /// Packets for the amount prompt. The caller sends each one.
    pub fn confirm_buy(&self) -> Result<Vec<RequestEdenPacket>, &'static str> {
        if self.buy_slot < 0 {
            return Err("Choose an item in the merit shop.");
        }
        let offer = self
            .offers
            .iter()
            .find(|offer| offer.slot == self.buy_slot)
            .ok_or("That shop item is no longer listed.")?;
        if offer.locked {
            return Err("Your base level is too low for that item.");
        }
        let amount = parse_buy_amount(&self.buy_amount_text)?;
        if offer.price_merit > 0 {
            let affordable = self.merit / offer.price_merit;
            if i32::from(amount) > affordable {
                return Err("Not enough Eden Merit Badges for that many stacks.");
            }
        }
        Ok(eden_button_packets(EdenButton::Buy { slot: offer.slot, amount }))
    }

    fn refresh_headline(&mut self) {
        let pane = if self.pane == EDEN_PANE_SHOP { "Merit shop" } else { "Missions" };
        self.headline = format!("{pane} · base level {} · Merit Badges {}", self.base_level, self.merit);
    }

    fn refresh_locks(&mut self) {
        for offer in &mut self.offers {
            offer.locked = self.base_level < i16::from(offer.min_level);
        }
    }

    fn keep_buy_if_present(&mut self) {
        if self.buy_slot >= 0 && !self.offers.iter().any(|offer| offer.slot == self.buy_slot) {
            self.clear_buy();
        }
    }
}

pub fn mission_state_label(state: u8) -> &'static str {
    match state {
        EDEN_STATE_AVAILABLE => "Available",
        EDEN_STATE_ACTIVE => "In progress",
        EDEN_STATE_TURNIN => "Ready",
        EDEN_STATE_COOLDOWN => "Cooldown",
        _ => "Unknown",
    }
}

pub fn mission_actions(state: u8) -> (bool, bool, bool) {
    match state {
        EDEN_STATE_AVAILABLE => (true, false, false),
        EDEN_STATE_ACTIVE => (false, false, true),
        EDEN_STATE_TURNIN => (false, true, true),
        _ => (false, false, false),
    }
}

pub fn level_band_label(min_level: i16, max_level: i16) -> String {
    if max_level >= 100 {
        format!("Lv {min_level}+")
    } else {
        format!("Lv {min_level}-{max_level}")
    }
}

pub fn shop_section_label(category: u8) -> &'static str {
    match category {
        0 => "Supplies",
        1 => "Gear",
        2 => "Upgrades",
        _ => "Other",
    }
}

pub fn named_item(names: &[(i32, String)], item_id: i32) -> &str {
    names
        .iter()
        .find(|entry| entry.0 == item_id)
        .map(|entry| entry.1.as_str())
        .unwrap_or("")
}

pub fn mission_progress(quest: &EdenQuestEntry, item_name: &str) -> String {
    let mut parts = Vec::new();
    if quest.hunt_max > 0 {
        parts.push(format!("Hunt {}/{}", quest.hunt_count, quest.hunt_max));
    }
    if quest.item_need > 0 {
        let name = display_item(item_name, quest.item_id);
        parts.push(format!("Bring {}/{} {name}", quest.item_have, quest.item_need));
    }
    let map = quest.map.trim();
    if !map.is_empty() {
        if quest.spawn_count > 0 {
            parts.push(format!("{map} ({})", quest.spawn_count));
        } else {
            parts.push(map.to_owned());
        }
    }
    parts.join(" · ")
}

pub fn mission_hint(quest: &EdenQuestEntry) -> String {
    quest.hint.trim().to_owned()
}

pub fn mission_rewards(quest: &EdenQuestEntry, reward_name: &str) -> String {
    let mut parts = Vec::new();
    if quest.base_exp > 0 {
        parts.push(format!("{} Base EXP", quest.base_exp));
    }
    if quest.job_exp > 0 {
        parts.push(format!("{} Job EXP", quest.job_exp));
    }
    if quest.reward_item > 0 && quest.reward_item_amount > 0 {
        let name = display_item(reward_name, quest.reward_item);
        parts.push(format!("{}x {name}", quest.reward_item_amount));
    }
    if quest.merit > 0 {
        parts.push(format!("{} Merit", quest.merit));
    }
    if parts.is_empty() {
        "Reward: none listed".to_owned()
    } else {
        format!("Reward: {}", parts.join(", "))
    }
}

pub fn cost_label(price_merit: i32, price_zeny: i32) -> String {
    match (price_merit > 0, price_zeny > 0) {
        (true, true) => format!("{price_merit} Merit + {price_zeny} z"),
        (true, false) => format!("{price_merit} Merit"),
        (false, true) => format!("{price_zeny} z"),
        (false, false) => "Free".to_owned(),
    }
}

pub fn parse_buy_amount(text: &str) -> Result<u16, &'static str> {
    let amount: u32 = text.trim().parse().map_err(|_| "Enter how many stacks to buy.")?;
    if amount == 0 {
        return Err("Enter at least 1 stack.");
    }
    if amount > u32::from(EDEN_BUY_MAX) {
        return Err("Buy at most 99 stacks at a time.");
    }
    Ok(amount as u16)
}

pub fn group_missions(missions: Vec<EdenMission>) -> Vec<EdenBand> {
    let mut bands: Vec<EdenBand> = Vec::new();
    for mission in missions {
        if let Some(band) = bands
            .iter_mut()
            .find(|band| band.min_level == mission.min_level && band.max_level == mission.max_level)
        {
            band.missions.push(mission);
        } else {
            bands.push(EdenBand {
                label: level_band_label(mission.min_level, mission.max_level),
                min_level: mission.min_level,
                max_level: mission.max_level,
                missions: vec![mission],
            });
        }
    }
    bands
}

fn mission_from_entry(quest: &EdenQuestEntry, names: &[(i32, String)]) -> EdenMission {
    let (can_accept, can_complete, can_abandon) = mission_actions(quest.state);
    EdenMission {
        quest_id: quest.quest_id,
        name: quest.name.trim().to_owned(),
        state: quest.state,
        state_label: mission_state_label(quest.state).to_owned(),
        min_level: quest.min_level,
        max_level: quest.max_level,
        progress: mission_progress(quest, named_item(names, quest.item_id)),
        hint: mission_hint(quest),
        rewards: mission_rewards(quest, named_item(names, quest.reward_item)),
        can_accept,
        can_complete,
        can_abandon,
    }
}

fn offer_from_entry(entry: EdenShopEntry, name: String, texture: Option<Arc<Texture>>, base_level: i16) -> EdenShopOffer {
    let name = if name.trim().is_empty() || name == "NOTFOUND" {
        format!("#{}", entry.item_id)
    } else {
        name
    };
    EdenShopOffer {
        slot: i32::from(entry.slot),
        category: entry.category,
        category_label: shop_section_label(entry.category).to_owned(),
        item_id: entry.item_id,
        name,
        stack: entry.amount,
        min_level: entry.min_level,
        price_merit: entry.price_merit,
        price_zeny: entry.price_zeny,
        cost: cost_label(entry.price_merit, entry.price_zeny),
        detail: format!("{}x · Lv {}+", entry.amount.max(1), entry.min_level),
        locked: base_level < i16::from(entry.min_level),
        texture,
    }
}

fn display_item(name: &str, item_id: i32) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "NOTFOUND" {
        format!("#{item_id}")
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{EdenQuestEntry, EdenShopEntry};

    use super::{
        EdenButton, EdenWindowState, eden_button_packets, group_missions, level_band_label, mission_actions, mission_from_entry,
        parse_buy_amount,
    };

    fn quest(id: i32, state: u8, min_level: i16, max_level: i16) -> EdenQuestEntry {
        EdenQuestEntry {
            quest_id: id,
            state,
            min_level,
            max_level,
            mob_id: 1002,
            hunt_count: 3,
            hunt_max: 10,
            item_id: 909,
            item_have: 1,
            item_need: 2,
            base_exp: 100,
            job_exp: 50,
            reward_item: 501,
            reward_item_amount: 1,
            merit: 2,
            map: "prt_fild08".to_owned(),
            spawn_count: 4,
            name: format!("Quest {id}"),
            hint: "South of Prontera".to_owned(),
        }
    }

    #[test]
    fn missions_group_by_level_band_and_offer_one_button_set() {
        let names = [(909, "Jellopy".to_owned()), (501, "Red Potion".to_owned())];
        let rows = vec![
            mission_from_entry(&quest(1, 0, 11, 25), &names),
            mission_from_entry(&quest(2, 1, 41, 55), &names),
            mission_from_entry(&quest(3, 2, 11, 25), &names),
            mission_from_entry(&quest(4, 3, 91, 856), &names),
        ];
        assert!(rows[0].can_accept && !rows[0].can_complete && !rows[0].can_abandon);
        assert!(!rows[1].can_accept && rows[1].can_abandon && !rows[1].can_complete);
        assert!(rows[2].can_complete && rows[2].can_abandon && !rows[2].can_accept);
        assert!(!rows[3].can_accept && !rows[3].can_complete && !rows[3].can_abandon);
        assert!(rows[0].progress.contains("Hunt 3/10"));
        assert!(rows[0].progress.contains("Bring 1/2 Jellopy"));
        assert!(rows[0].progress.contains("prt_fild08 (4)"));
        assert!(!rows[0].progress.contains("South of Prontera"));
        assert_eq!(rows[0].hint, "South of Prontera");
        assert!(rows[0].rewards.contains("100 Base EXP"));
        assert!(rows[0].rewards.contains("1x Red Potion"));
        assert!(rows[0].rewards.contains("2 Merit"));

        let bands = group_missions(rows);
        assert_eq!(bands.len(), 3);
        assert_eq!(bands[0].label, "Lv 11-25");
        assert_eq!(bands[0].missions.len(), 2);
        assert_eq!(bands[1].label, "Lv 41-55");
        assert_eq!(bands[2].label, level_band_label(91, 856));
        assert_eq!(bands[2].label, "Lv 91+");
        assert_eq!(mission_actions(0), (true, false, false));
    }

    #[test]
    fn each_button_encodes_its_own_packet() {
        let accept = eden_button_packets(EdenButton::Accept(44));
        let complete = eden_button_packets(EdenButton::Complete(44));
        let abandon = eden_button_packets(EdenButton::Abandon(44));
        let buy = eden_button_packets(EdenButton::Buy { slot: 3, amount: 2 });

        assert_eq!(accept.len(), 1);
        assert_eq!(accept[0].action, 1);
        assert_eq!(accept[0].quest_id, 44);
        assert_eq!(complete[0].action, 2);
        assert_eq!(abandon[0].action, 3);
        assert_eq!(buy.len(), 2);
        assert!(buy.iter().all(|packet| packet.action == 4 && packet.quest_id == 3));
    }

    #[test]
    fn shop_buy_prompt_repeats_the_slot_packet() {
        let mut window = EdenWindowState::default();
        window.apply_quests(50, 9, &[], &[]);
        window.apply_shop(9, vec![(
            EdenShopEntry {
                slot: 3,
                category: 0,
                min_level: 1,
                item_id: 501,
                amount: 10,
                price_merit: 4,
                price_zeny: 0,
            },
            "Red Potion".to_owned(),
            None,
        )]);
        assert_eq!(window.offers[0].cost, "4 Merit");
        assert!(!window.offers[0].locked);
        assert!(window.offers[0].detail.contains("10x"));

        window.begin_buy(3);
        assert!(window.buying());
        window.buy_amount_text = "2".to_owned();
        let packets = window.confirm_buy().unwrap();
        assert_eq!(packets.len(), 2);
        assert!(packets.iter().all(|packet| packet.action == 4 && packet.quest_id == 3));
        window.clear_buy();
        assert!(!window.buying());
        assert!(window.confirm_buy().is_err(), "a second confirm does not send another buy");

        window.begin_buy(3);
        window.buy_amount_text = "0".to_owned();
        assert!(window.confirm_buy().is_err());
        assert!(window.buying(), "a rejected amount leaves the prompt open");
        window.buy_amount_text = "3".to_owned();
        assert!(window.confirm_buy().is_err(), "9 merit cannot buy 3 stacks at 4 each");
        assert!(window.buying());
        assert!(parse_buy_amount("100").is_err());

        window.apply_shop(9, vec![(
            EdenShopEntry {
                slot: 8,
                category: 1,
                min_level: 99,
                item_id: 15009,
                amount: 1,
                price_merit: 5,
                price_zeny: 100,
            },
            "Uniform".to_owned(),
            None,
        )]);
        assert!(window.offers[0].locked);
        assert!(window.offers[0].cost.contains("Merit"));
        assert!(window.offers[0].cost.contains("100 z"));
        window.begin_buy(8);
        assert!(!window.buying());
    }
}
