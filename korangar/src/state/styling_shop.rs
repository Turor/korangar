//! Styling shop selection. Indexes sent to the server are 1-based positions in
//! the Hercules stylist list. Zero means "keep the current look".

use std::sync::Arc;

use korangar_interface::element::StateElement;
use ragnarok_packets::ItemId;
use rust_state::RustState;

use super::styling_catalog::{CLOTH_COLOR, HAIR, HAIR_COLOR, HEAD_BOTTOM, HEAD_MID, HEAD_TOP, StyleGoods};
use crate::world::{AnimationData, ItemName, ItemNameKey, Library};

/// Summoner jobs that use the Doram price rows.
///
/// 4218 and 4220 are the base Doram jobs. 4308 is Spirit Handler
/// (`MAPID_SPIRIT_HANDLER` still sits on `MAPID_SUMMONER`). 4315 is the
/// riding Doram id roBrowser treats the same way.
pub fn job_is_doram(job_id: u16) -> bool {
    matches!(job_id, 4218 | 4220 | 4308 | 4315)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StylePurchase {
    pub head_palette: i16,
    pub head_style: i16,
    pub body_palette: i16,
    pub top_accessory: i16,
    pub mid_accessory: i16,
    pub bottom_accessory: i16,
}

impl StylePurchase {
    pub fn is_empty(self) -> bool {
        self.head_palette == 0
            && self.head_style == 0
            && self.body_palette == 0
            && self.top_accessory == 0
            && self.mid_accessory == 0
            && self.bottom_accessory == 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleCost {
    pub zeny: i32,
    pub coupons: Vec<String>,
}

/// In-progress stylist UI. The preview sprite, when loaded, is the selected
/// hair on the current body. Worn colors stay on the player.
#[derive(Default, RustState, StateElement)]
pub struct StylingShopState {
    pub open: bool,
    /// Set while the server still thinks the shop is open.
    pub close_pending: bool,
    pub headgear_tab: bool,
    pub hair_style: u16,
    pub hair_color: u16,
    pub cloth_color: u16,
    pub head_top: u16,
    pub head_mid: u16,
    pub head_bottom: u16,
    pub tab_text: String,
    pub row_a: String,
    pub row_b: String,
    pub row_c: String,
    pub cost_text: String,
    pub status: String,
    /// Hair sprite id the preview load was asked for.
    pub preview_hair: u32,
    #[hidden_element]
    pub preview: Option<Arc<AnimationData>>,
}

impl StylingShopState {
    pub fn open_shop(&mut self, doram: bool, library: Option<&Library>) {
        self.open = true;
        self.close_pending = true;
        self.headgear_tab = false;
        self.clear_selection();
        self.status = "Choose a style.".to_owned();
        self.refresh(doram, library);
    }

    pub fn set_tab(&mut self, headgear: bool, doram: bool, library: Option<&Library>) {
        self.headgear_tab = headgear;
        self.refresh(doram, library);
    }

    pub fn step(&mut self, row: u8, delta: i8, doram: bool, library: Option<&Library>) {
        let list = row_list(self.headgear_tab, row);
        let visible = visible_options(list, doram);
        let selected = selection_mut(self, row);
        *selected = cycle(*selected, delta, &visible);
        self.refresh(doram, library);
    }

    pub fn reset(&mut self, doram: bool, library: Option<&Library>) {
        self.clear_selection();
        self.status = "Choose a style.".to_owned();
        self.refresh(doram, library);
    }

    pub fn note_result(&mut self, success: bool, doram: bool, library: Option<&Library>) {
        self.clear_selection();
        self.refresh(doram, library);
        self.status = if success {
            "Style saved.".to_owned()
        } else {
            "Not enough zeny or coupon.".to_owned()
        };
    }

    /// Escape or the title-bar close removed the window without the Close
    /// button.
    pub fn take_unsolicited_close(&mut self, window_still_open: bool) -> bool {
        if self.close_pending && !window_still_open {
            self.close_pending = false;
            self.open = false;
            true
        } else {
            false
        }
    }

    pub fn mark_button_close(&mut self) {
        self.close_pending = false;
        self.open = false;
    }

    pub fn purchase(&self) -> StylePurchase {
        StylePurchase {
            head_palette: self.hair_color as i16,
            head_style: self.hair_style as i16,
            body_palette: self.cloth_color as i16,
            top_accessory: self.head_top as i16,
            mid_accessory: self.head_mid as i16,
            bottom_accessory: self.head_bottom as i16,
        }
    }

    pub fn cost(&self, doram: bool) -> StyleCost {
        let mut zeny = 0;
        let mut coupons = Vec::new();
        for (list, index) in [
            (HAIR_COLOR, self.hair_color),
            (HAIR, self.hair_style),
            (CLOTH_COLOR, self.cloth_color),
            (HEAD_TOP, self.head_top),
            (HEAD_MID, self.head_mid),
            (HEAD_BOTTOM, self.head_bottom),
        ] {
            let Some(goods) = find_goods(list, index) else {
                continue;
            };
            match price_of(goods, doram) {
                Priced::Zeny(amount) => zeny += amount,
                Priced::Coupon(name) => coupons.push(name),
            }
        }
        StyleCost { zeny, coupons }
    }

    /// Hair id to draw. A kept style uses the hair the character already wears.
    pub fn preview_hair_id(&self, worn: usize) -> usize {
        find_goods(HAIR, self.hair_style)
            .filter(|goods| goods.goods_id > 0)
            .map(|goods| goods.goods_id as usize)
            .unwrap_or(worn)
    }

    pub fn begin_preview(&mut self, hair: u32) {
        if self.preview_hair != hair {
            self.preview = None;
            self.preview_hair = hair;
        }
    }

    pub fn clear_preview(&mut self) {
        self.preview = None;
        self.preview_hair = 0;
    }

    pub fn store_preview(&mut self, hair: u32, animation: Arc<AnimationData>) {
        if self.preview_hair == hair {
            self.preview = Some(animation);
        }
    }

    fn clear_selection(&mut self) {
        self.hair_style = 0;
        self.hair_color = 0;
        self.cloth_color = 0;
        self.head_top = 0;
        self.head_mid = 0;
        self.head_bottom = 0;
    }

    fn refresh(&mut self, doram: bool, library: Option<&Library>) {
        self.tab_text = if self.headgear_tab {
            "Headgear".to_owned()
        } else {
            "Hair".to_owned()
        };
        self.row_a = row_text(self.headgear_tab, 0, selection_of(self, 0), doram, library);
        self.row_b = row_text(self.headgear_tab, 1, selection_of(self, 1), doram, library);
        self.row_c = row_text(self.headgear_tab, 2, selection_of(self, 2), doram, library);
        let cost = self.cost(doram);
        self.cost_text = if cost.coupons.is_empty() {
            format!("Total: {} z", cost.zeny)
        } else {
            format!("Total: {} z + {}", cost.zeny, cost.coupons.join(" + "))
        };
    }
}

fn selection_of(state: &StylingShopState, row: u8) -> u16 {
    match (state.headgear_tab, row) {
        (false, 0) => state.hair_style,
        (false, 1) => state.hair_color,
        (false, _) => state.cloth_color,
        (true, 0) => state.head_top,
        (true, 1) => state.head_mid,
        (true, _) => state.head_bottom,
    }
}

fn selection_mut(state: &mut StylingShopState, row: u8) -> &mut u16 {
    match (state.headgear_tab, row) {
        (false, 0) => &mut state.hair_style,
        (false, 1) => &mut state.hair_color,
        (false, _) => &mut state.cloth_color,
        (true, 0) => &mut state.head_top,
        (true, 1) => &mut state.head_mid,
        (true, _) => &mut state.head_bottom,
    }
}

fn row_list(headgear: bool, row: u8) -> &'static [StyleGoods] {
    match (headgear, row) {
        (false, 0) => HAIR,
        (false, 1) => HAIR_COLOR,
        (false, _) => CLOTH_COLOR,
        (true, 0) => HEAD_TOP,
        (true, 1) => HEAD_MID,
        (true, _) => HEAD_BOTTOM,
    }
}

fn row_title(headgear: bool, row: u8) -> &'static str {
    match (headgear, row) {
        (false, 0) => "Hair style",
        (false, 1) => "Hair color",
        (false, _) => "Cloth color",
        (true, 0) => "Head top",
        (true, 1) => "Head mid",
        (true, _) => "Head bottom",
    }
}

fn visible_options(list: &[StyleGoods], doram: bool) -> Vec<&StyleGoods> {
    list.iter().filter(|goods| !doram || goods.allow_doram).collect()
}

fn find_goods(list: &[StyleGoods], server_index: u16) -> Option<&StyleGoods> {
    if server_index == 0 {
        return None;
    }
    list.iter().find(|goods| goods.server_index == server_index)
}

fn cycle(current: u16, delta: i8, visible: &[&StyleGoods]) -> u16 {
    if visible.is_empty() {
        return 0;
    }
    let mut slots = Vec::with_capacity(visible.len() + 1);
    slots.push(0);
    slots.extend(visible.iter().map(|goods| goods.server_index));
    let pos = slots.iter().position(|index| *index == current).unwrap_or(0);
    let next = (pos as i32 + i32::from(delta)).rem_euclid(slots.len() as i32);
    slots[next as usize]
}

enum Priced {
    Zeny(i32),
    Coupon(String),
}

fn price_of(goods: &StyleGoods, doram: bool) -> Priced {
    let (zeny, item_label, item_id, box_label, box_id) = if doram && goods.has_doram_cost {
        (
            goods.doram_zeny,
            goods.doram_item_label,
            goods.doram_item_id,
            goods.doram_box_label,
            goods.doram_box_id,
        )
    } else {
        (goods.zeny, goods.item_label, goods.item_id, goods.box_label, goods.box_id)
    };
    if zeny != 0 {
        return Priced::Zeny(zeny);
    }
    let coupon = if !item_label.is_empty() {
        item_label.to_owned()
    } else if item_id != 0 {
        format!("item {item_id}")
    } else if !box_label.is_empty() {
        box_label.to_owned()
    } else if box_id != 0 {
        format!("item {box_id}")
    } else {
        return Priced::Zeny(0);
    };
    Priced::Coupon(coupon)
}

fn goods_label(goods: &StyleGoods, library: Option<&Library>) -> String {
    if goods.goods_id > 0 {
        if let Some(name) = library.and_then(|library| item_name(library, goods.goods_id)) {
            return name;
        }
        return format!("#{}", goods.goods_id);
    }
    if !goods.label.is_empty() {
        return goods.label.replace('_', " ");
    }
    "style".to_owned()
}

fn item_name(library: &Library, id: i32) -> Option<String> {
    if id <= 0 {
        return None;
    }
    let name = library
        .get::<ItemName>(ItemNameKey {
            item_id: ItemId(id as u32),
            is_identified: true,
        })
        .to_string();
    (name != "NOTFOUND").then_some(name)
}

fn row_text(headgear: bool, row: u8, selected: u16, doram: bool, library: Option<&Library>) -> String {
    let title = row_title(headgear, row);
    let visible = visible_options(row_list(headgear, row), doram);
    let Some(goods) = visible.iter().copied().find(|goods| goods.server_index == selected) else {
        return format!("{title}: current");
    };
    let pos = visible.iter().position(|entry| entry.server_index == selected).unwrap_or(0) + 1;
    let name = goods_label(goods, library);
    let price = match price_of(goods, doram) {
        Priced::Zeny(0) => String::new(),
        Priced::Zeny(amount) => format!("{amount} z"),
        Priced::Coupon(name) => name,
    };
    if price.is_empty() {
        format!("{title}: {name} ({pos}/{})", visible.len())
    } else {
        format!("{title}: {name} ({pos}/{}) · {price}", visible.len())
    }
}

#[cfg(test)]
mod tests {
    use super::super::styling_catalog::{HAIR, HAIR_COLOR, HEAD_TOP};
    use super::*;

    #[test]
    fn catalog_indexes_follow_the_hercules_file() {
        for (offset, goods) in HAIR_COLOR.iter().enumerate() {
            assert_eq!(goods.server_index, (offset + 1) as u16);
        }
        assert_eq!(HAIR_COLOR[0].goods_id, 1);
        assert_eq!(HAIR_COLOR[0].zeny, 100_000);
        assert!(HAIR_COLOR[0].allow_doram);
        assert_eq!(HEAD_TOP[0].label, "Hat");
        assert_eq!(HEAD_TOP[0].zeny, 1000);
        assert_eq!(HEAD_TOP[0].server_index, 1);
        assert!(HAIR.iter().any(|goods| !goods.allow_doram));
        assert!(HAIR.iter().any(|goods| goods.has_doram_cost));
    }

    #[test]
    fn step_sends_server_index_and_prices_the_row() {
        let mut shop = StylingShopState::default();
        shop.open_shop(false, None);
        shop.step(0, 1, false, None);
        assert_eq!(shop.hair_style, 1);
        assert_eq!(shop.purchase().head_style, 1);
        assert!(!shop.purchase().is_empty());
        assert_eq!(shop.cost(false).zeny, 100_000);
        assert!(shop.row_a.contains("100000"));
        assert!(shop.row_a.contains("#1"));
        assert_eq!(shop.preview_hair_id(4), 1);

        shop.step(0, -1, false, None);
        assert_eq!(shop.hair_style, 0);
        assert!(shop.purchase().is_empty());
        assert_eq!(shop.preview_hair_id(4), 4);

        shop.step(0, -1, false, None);
        assert_eq!(shop.hair_style, HAIR.last().unwrap().server_index);
    }

    #[test]
    fn headgear_index_is_not_the_item_name() {
        let mut shop = StylingShopState::default();
        shop.open_shop(false, None);
        shop.set_tab(true, false, None);
        shop.step(0, 1, false, None);
        assert_eq!(shop.head_top, 1);
        assert_eq!(HEAD_TOP[0].goods_id, 0);
        assert_eq!(shop.purchase().top_accessory, 1);
        assert_eq!(shop.purchase().head_style, 0);
        assert!(shop.row_a.contains("Hat"));
        assert!(shop.tab_text.contains("Headgear"));
        assert_eq!(shop.cost(false).zeny, 1000);
    }

    #[test]
    fn doram_uses_the_coupon_and_skips_human_styles() {
        let human = visible_options(HAIR, false);
        let doram = visible_options(HAIR, true);
        assert!(doram.len() < human.len());
        assert!(doram.iter().all(|goods| goods.allow_doram));

        let coupon = HAIR.iter().find(|goods| goods.has_doram_cost).unwrap();
        assert!(matches!(price_of(coupon, true), Priced::Coupon(ref name) if name.contains("Coupon")));
        assert!(matches!(price_of(coupon, false), Priced::Zeny(100_000)));

        let mut shop = StylingShopState::default();
        shop.open_shop(true, None);
        for _ in 0..coupon.server_index {
            shop.step(0, 1, true, None);
        }
        assert_eq!(shop.hair_style, coupon.server_index);
        let cost = shop.cost(true);
        assert_eq!(cost.zeny, 0);
        assert!(cost.coupons.iter().any(|coupon| coupon.contains("Coupon")));
    }

    #[test]
    fn spirit_handler_4308_skips_human_styles_and_pays_the_coupon() {
        assert!(job_is_doram(4218));
        assert!(job_is_doram(4220));
        assert!(job_is_doram(4308));
        assert!(job_is_doram(4315));
        assert!(!job_is_doram(4217));
        assert!(!job_is_doram(4219));

        let doram = job_is_doram(4308);
        let visible = visible_options(HAIR, doram);
        assert!(HAIR.iter().any(|goods| !goods.allow_doram));
        assert!(visible.iter().all(|goods| goods.allow_doram));
        assert!(visible.len() < HAIR.len());

        let coupon = HAIR.iter().find(|goods| goods.has_doram_cost).unwrap();
        assert!(matches!(price_of(coupon, doram), Priced::Coupon(ref name) if name.contains("Coupon")));

        let mut shop = StylingShopState::default();
        shop.open_shop(doram, None);
        for _ in 0..coupon.server_index {
            shop.step(0, 1, doram, None);
        }
        assert_eq!(shop.hair_style, coupon.server_index);
        let cost = shop.cost(doram);
        assert_eq!(cost.zeny, 0);
        assert!(cost.coupons.iter().any(|name| name.contains("Coupon")));
    }

    #[test]
    fn result_clears_the_selection() {
        let mut shop = StylingShopState::default();
        shop.open_shop(false, None);
        shop.step(1, 1, false, None);
        assert_eq!(shop.hair_color, 1);
        shop.note_result(true, false, None);
        assert!(shop.purchase().is_empty());
        assert_eq!(shop.status, "Style saved.");
        shop.step(1, 1, false, None);
        shop.note_result(false, false, None);
        assert_eq!(shop.status, "Not enough zeny or coupon.");
        assert!(shop.purchase().is_empty());
    }

    #[test]
    fn title_bar_close_asks_for_one_packet() {
        let mut shop = StylingShopState::default();
        shop.open_shop(false, None);
        assert!(!shop.take_unsolicited_close(true));
        assert!(shop.take_unsolicited_close(false));
        assert!(!shop.take_unsolicited_close(false));

        shop.open_shop(false, None);
        shop.mark_button_close();
        assert!(!shop.take_unsolicited_close(false));
    }
}
