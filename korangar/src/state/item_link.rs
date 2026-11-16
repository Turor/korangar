//! Shift+click item links in the roBrowser `<ITEML>` form (packetver 20250716).
//!
//! Hercules relays chat text unchanged. `clif_validate_message` rejects only
//! CR, LF, and a run of 13 spaces; `clif_process_chat_message` does not strip
//! `<ITEML>`. `CHAT_SIZE_MAX` is 256 and `CHATBOX_SIZE` is 71. This client
//! keeps the existing 80-byte draft. A link that does not fit drops random
//! options from the end, then cards, then enchant grade. The item id, refine,
//! and view stay so the tag still parses. If that minimum does not fit,
//! nothing is inserted.

use std::cell::Cell;

use korangar_interface::Interface;
use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::{EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions, RegularItemFlags};
use rust_state::State;

use crate::graphics::Color;
use crate::input::InputEvent;
use crate::interface::windows::{ChatTextBox, ChatWindow, WindowClass, format_item_details, insert_chat_item_link};
use crate::state::{ClientState, ClientStatePathExt, client_state};
use crate::world::ResourceMetadata;

/// Chat draft cap in UTF-8 bytes. See the module comment for the Hercules caps.
pub const CHAT_DRAFT_LIMIT: usize = 80;

const ALPHABET: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const OPEN_TAG: &str = "<ITEML>";
const CLOSE_TAG: &str = "</ITEML>";

thread_local! {
    static SHIFT_HELD: Cell<bool> = const { Cell::new(false) };
}

/// Record Shift for the click that is about to be dispatched.
pub fn set_shift_held(held: bool) {
    SHIFT_HELD.with(|cell| cell.set(held));
}

/// Shift+left-click links an item instead of dragging it.
pub fn shift_links() -> bool {
    SHIFT_HELD.with(Cell::get)
}

/// What a left click on an item slot does with the current Shift state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemClickAction {
    Link,
    Drag,
}

pub fn item_click_action() -> ItemClickAction {
    match shift_links() {
        true => ItemClickAction::Link,
        false => ItemClickAction::Drag,
    }
}

/// Name, slot count, and description used to label a link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemIdentity {
    pub name: String,
    pub slots: u8,
    pub description: String,
}

/// One random option carried by a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedOption {
    pub index: u32,
    pub parameter: u32,
    pub value: u32,
}

/// A parsed `<ITEML>` body. Fields are the values roBrowser and Hercules share.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedItemLink {
    pub item_id: u32,
    pub refine: u32,
    pub grade: u32,
    pub view: u32,
    pub location: u32,
    pub equip: bool,
    pub cards: [u32; 4],
    pub options: Vec<ParsedOption>,
}

/// One painted run of a chat line. `accent` is the theme accent token, not a
/// color.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatSegment {
    pub text: String,
    pub accent: bool,
    pub link: Option<ParsedItemLink>,
}

struct LinkParts {
    location: u32,
    equip: bool,
    item_id: u32,
    refine: u32,
    view: u32,
    grade: Option<u32>,
    cards: Vec<u32>,
    options: Vec<ParsedOption>,
}

/// English table first, then a name already on the inventory, storage, or cart.
pub fn identity_for(state: &State<ClientState>, item_id: u32) -> Option<ItemIdentity> {
    if let Some(row) = crate::world::english_item_identity(item_id)
        && !row.name.is_empty()
    {
        let row: crate::world::EnglishItemIdentity = row;
        return Some(ItemIdentity {
            name: row.name,
            slots: row.slots,
            description: row.description,
        });
    }
    let id = ItemId(item_id);
    for found in [
        state.follow(client_state().inventory()).find_by_item_id(id),
        state.follow(client_state().storage()).find_by_item_id(id),
        state.follow(client_state().cart()).find_by_item_id(id),
    ] {
        if let Some(identity) = identity_from_item(found) {
            return Some(identity);
        }
    }
    None
}

fn identity_from_item(item: Option<&InventoryItem<ResourceMetadata>>) -> Option<ItemIdentity> {
    let item = item?;
    if item.metadata.name.is_empty() {
        return None;
    }
    Some(ItemIdentity {
        name: item.metadata.name.clone(),
        slots: 0,
        description: item.metadata.description.clone(),
    })
}

/// `<ITEML>` payload for this stack, in the roBrowser 20200724 layout.
pub fn encode_item_link<Meta>(item: &InventoryItem<Meta>) -> String {
    encode_parts(&parts_from_item(item))
}

/// Append a link at the end of the draft. The chat box has no mid-line caret.
pub fn append_link_text<Meta>(draft: &str, item: &InventoryItem<Meta>) -> String {
    let leading = usize::from(!draft.is_empty() && !draft.ends_with(' '));
    let budget = CHAT_DRAFT_LIMIT.saturating_sub(draft.len().saturating_add(leading).saturating_add(1));
    let Some(link) = link_that_fits(parts_from_item(item), budget) else {
        return draft.to_owned();
    };
    let mut out = String::with_capacity(draft.len() + leading + link.len() + 1);
    out.push_str(draft);
    if leading == 1 {
        out.push(' ');
    }
    out.push_str(&link);
    out.push(' ');
    out
}

/// Open the chat window when it is closed and append the link to the draft.
pub fn link_item_in_chat(
    state: &mut State<ClientState>,
    interface: &mut Interface<'static, ClientState>,
    item: InventoryItem<ResourceMetadata>,
) {
    insert_chat_item_link(state, &item);
    if !interface.is_window_with_class_open(WindowClass::Chat) {
        interface.open_window(ChatWindow::new(client_state().chat_window(), client_state().chat_messages()));
    }
    interface.request_focus(ChatTextBox);
}

/// Split a chat line into plain runs and item links. Malformed tags stay plain.
pub fn chat_segments(text: &str, resolve: impl Fn(u32) -> Option<ItemIdentity>) -> Vec<ChatSegment> {
    let mut segments = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    let flush = |plain: &mut String, segments: &mut Vec<ChatSegment>| {
        if plain.is_empty() {
            return;
        }
        segments.push(ChatSegment {
            text: std::mem::take(plain),
            accent: false,
            link: None,
        });
    };
    loop {
        let Some(open) = find_ascii_ci(rest, OPEN_TAG) else {
            plain.push_str(rest);
            break;
        };
        plain.push_str(&rest[..open]);
        let after_open = open + OPEN_TAG.len();
        let Some(close) = find_ascii_ci(&rest[after_open..], CLOSE_TAG) else {
            plain.push_str(&rest[open..]);
            break;
        };
        let inner = &rest[after_open..after_open + close];
        let after = after_open + close + CLOSE_TAG.len();
        match parse_body(inner) {
            Some(link) => {
                flush(&mut plain, &mut segments);
                let identity = resolve(link.item_id);
                segments.push(ChatSegment {
                    text: link.display_label(identity.as_ref()),
                    accent: true,
                    link: Some(link),
                });
            }
            None => plain.push_str(&rest[open..after]),
        }
        rest = &rest[after..];
    }
    flush(&mut plain, &mut segments);
    segments
}

/// First well-formed link in `text`.
pub fn parse_item_link(text: &str) -> Option<ParsedItemLink> {
    chat_segments(text, |_| None).into_iter().find_map(|segment| segment.link)
}

impl ParsedItemLink {
    /// `[+7 Knife [3]]` when the table knows the name and slot count.
    /// Unknown ids are `[Item #id]`.
    pub fn display_label(&self, identity: Option<&ItemIdentity>) -> String {
        let Some(identity) = identity.filter(|identity| !identity.name.is_empty()) else {
            return format!("[Item #{}]", self.item_id);
        };
        let mut name = identity.name.clone();
        if identity.slots > 0 && !name.contains('[') {
            name = format!("{name} [{}]", identity.slots);
        }
        if self.refine > 0 {
            name = format!("+{0} {name}", self.refine);
        }
        format!("[{name}]")
    }

    /// Inventory row the pinned details window and the tooltip already know.
    pub fn to_inventory_item(&self, identity: Option<&ItemIdentity>) -> InventoryItem<ResourceMetadata> {
        let name = identity.map(|identity| identity.name.clone()).unwrap_or_default();
        let description = identity.map(|identity| identity.description.clone()).unwrap_or_default();
        let gear = self.equip
            || self.refine > 0
            || self.grade > 0
            || self.location != 0
            || self.cards.iter().any(|card| *card > 0)
            || self.options.iter().any(|option| option.index > 0);
        let item_type = if self.equip {
            5
        } else if gear {
            4
        } else {
            0
        };
        let details = if gear {
            let mut option_data = [zero_option(), zero_option(), zero_option(), zero_option(), zero_option()];
            let mut count = 0u8;
            for option in self.options.iter().take(5) {
                if option.index == 0 {
                    continue;
                }
                option_data[count as usize] = ItemOptions {
                    index: option.index.min(u32::from(u16::MAX)) as u16,
                    value: option.value.min(u32::from(u16::MAX)) as u16,
                    parameter: option.parameter.min(u32::from(u8::MAX)) as u8,
                };
                count += 1;
            }
            InventoryItemDetails::Equippable {
                equip_position: EquipPosition::from_bits_truncate(self.location),
                equipped_position: EquipPosition::empty(),
                bind_on_equip_type: 0,
                w_item_sprite_number: self.view.min(u32::from(u16::MAX)) as u16,
                option_count: count,
                option_data,
                forge_data: None,
                refinement_level: self.refine.min(u32::from(u8::MAX)) as u8,
                enchantment_level: self.grade.min(u32::from(u8::MAX)) as u8,
                flags: EquippableItemFlags::IDENTIFIED,
            }
        } else {
            InventoryItemDetails::Regular {
                amount: 1,
                equipped_position: EquipPosition::empty(),
                flags: RegularItemFlags::IDENTIFIED,
            }
        };
        InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name,
                description,
            },
            index: InventoryIndex(0),
            item_id: ItemId(self.item_id),
            item_type,
            slot: self.cards,
            hire_expiration_date: 0,
            details,
        }
    }

    pub(crate) fn pinned_details(&self, identity: Option<&ItemIdentity>, gold: Color) -> (u64, String, String) {
        let title = self.display_label(identity);
        let item = self.to_inventory_item(identity);
        let body = format_item_details(&item, gold);
        (details_key(self), title, body)
    }

    /// The same window a right-click on a real slot opens.
    pub fn open_details(&self, identity: Option<&ItemIdentity>, gold: Color) -> InputEvent {
        let (key, title, body) = self.pinned_details(identity, gold);
        InputEvent::OpenItemDetails { key, title, body }
    }
}

fn details_key(link: &ParsedItemLink) -> u64 {
    let mut key = u64::from(link.item_id);
    key = key.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ u64::from(link.refine);
    for card in link.cards {
        key = key.wrapping_mul(0x0100_0000_01B3) ^ u64::from(card);
    }
    key.wrapping_mul(0x0100_0000_01B3) ^ u64::from(link.grade)
}

fn zero_option() -> ItemOptions {
    ItemOptions {
        index: 0,
        value: 0,
        parameter: 0,
    }
}

fn parts_from_item<Meta>(item: &InventoryItem<Meta>) -> LinkParts {
    let (location, refine, view, grade, options) = match &item.details {
        InventoryItemDetails::Regular { equipped_position, .. } => (equipped_position.bits(), 0, 0, None, Vec::new()),
        InventoryItemDetails::Equippable {
            equip_position,
            refinement_level,
            enchantment_level,
            w_item_sprite_number,
            option_data,
            ..
        } => {
            let options = option_data
                .iter()
                .filter(|option| option.index > 0)
                .map(|option| ParsedOption {
                    index: u32::from(option.index),
                    parameter: u32::from(option.parameter),
                    value: u32::from(option.value),
                })
                .collect();
            (
                equip_position.bits(),
                u32::from(*refinement_level),
                u32::from(*w_item_sprite_number),
                (*enchantment_level > 0).then_some(u32::from(*enchantment_level)),
                options,
            )
        }
    };
    LinkParts {
        location,
        equip: item.item_type == 5,
        item_id: item.item_id.0,
        refine,
        view,
        grade,
        cards: item.slot.into_iter().filter(|card| *card > 0).collect(),
        options,
    }
}

fn link_that_fits(mut parts: LinkParts, budget: usize) -> Option<String> {
    loop {
        let encoded = encode_parts(&parts);
        if encoded.len() <= budget {
            return Some(encoded);
        }
        if parts.options.pop().is_some() {
            continue;
        }
        if parts.cards.pop().is_some() {
            continue;
        }
        if parts.grade.take().is_some() {
            continue;
        }
        return None;
    }
}

fn encode_parts(parts: &LinkParts) -> String {
    let mut data = pad(parts.location, 5);
    data.push(if parts.equip { '1' } else { '0' });
    data.push_str(&encode_base62(parts.item_id));
    if parts.refine > 0 {
        data.push('%');
        data.push_str(&pad(parts.refine, 2));
    }
    // View is always present from packetver 20161116 so later fields stay aligned.
    data.push('&');
    data.push_str(&pad(parts.view, 2));
    if let Some(grade) = parts.grade.filter(|grade| *grade > 0) {
        data.push('\'');
        data.push_str(&pad(grade, 2));
    }
    for card in &parts.cards {
        if *card == 0 {
            continue;
        }
        data.push(')');
        data.push_str(&pad(*card, 2));
    }
    for option in &parts.options {
        if option.index == 0 {
            continue;
        }
        data.push('+');
        data.push_str(&pad(option.index, 2));
        data.push(',');
        data.push_str(&pad(option.parameter, 2));
        data.push('-');
        data.push_str(&pad(option.value, 2));
    }
    format!("{OPEN_TAG}{data}{CLOSE_TAG}")
}

fn encode_base62(mut number: u32) -> String {
    if number == 0 {
        return String::new();
    }
    let alphabet = ALPHABET.as_bytes();
    let mut bytes = Vec::new();
    while number > 0 {
        bytes.push(alphabet[(number % 62) as usize]);
        number /= 62;
    }
    bytes.reverse();
    String::from_utf8(bytes).unwrap_or_default()
}

fn pad(number: u32, width: usize) -> String {
    let raw = encode_base62(number);
    if raw.len() >= width { raw } else { format!("{raw:0>width$}") }
}

fn decode_base62(text: &str) -> Option<u32> {
    if text.is_empty() {
        return Some(0);
    }
    let mut value: u64 = 0;
    for byte in text.bytes() {
        let digit = ALPHABET.bytes().position(|candidate| candidate == byte)?;
        value = value.checked_mul(62)?.checked_add(digit as u64)?;
        if value > u64::from(u32::MAX) {
            return None;
        }
    }
    Some(value as u32)
}

fn is_alphabet(byte: u8) -> bool {
    ALPHABET.as_bytes().contains(&byte)
}

fn alphabet_end(data: &str, start: usize) -> usize {
    let bytes = data.as_bytes();
    let mut pos = start;
    while pos < bytes.len() && is_alphabet(bytes[pos]) {
        pos += 1;
    }
    pos
}

fn take_number(data: &str, pos: &mut usize) -> Option<u32> {
    let end = alphabet_end(data, *pos);
    if end == *pos {
        return None;
    }
    let value = decode_base62(&data[*pos..end])?;
    *pos = end;
    Some(value)
}

/// Accept roBrowser (omit zeros) and Hercules `clif_format_itemlink` (always
/// four cards, always view and grade). Separator set is packetver >= 20200724.
fn parse_body(data: &str) -> Option<ParsedItemLink> {
    if data.len() < 6 || !data.is_char_boundary(5) || !data.is_char_boundary(6) {
        return None;
    }
    let bytes = data.as_bytes();
    if !bytes[..5].iter().all(|byte| is_alphabet(*byte)) {
        return None;
    }
    let location = decode_base62(&data[..5])?;
    let equip = match bytes[5] {
        b'0' => false,
        b'1' => true,
        _ => return None,
    };
    let mut pos = 6;
    let id_end = alphabet_end(data, pos);
    let item_id = decode_base62(&data[pos..id_end])?;
    pos = id_end;

    let mut refine = 0u32;
    let mut view = 0u32;
    let mut grade = 0u32;
    let mut cards = [0u32; 4];
    let mut card_index = 0usize;
    let mut options = Vec::new();
    while pos < bytes.len() {
        let separator = bytes[pos];
        pos += 1;
        match separator {
            b'%' => refine = take_number(data, &mut pos)?,
            b'&' => view = take_number(data, &mut pos)?,
            b'\'' => grade = take_number(data, &mut pos)?,
            b')' => {
                if card_index >= 4 {
                    return None;
                }
                cards[card_index] = take_number(data, &mut pos)?;
                card_index += 1;
            }
            b'+' => {
                if options.len() >= 5 {
                    return None;
                }
                let index = take_number(data, &mut pos)?;
                if pos >= bytes.len() || bytes[pos] != b',' {
                    return None;
                }
                pos += 1;
                let parameter = take_number(data, &mut pos)?;
                if pos >= bytes.len() || bytes[pos] != b'-' {
                    return None;
                }
                pos += 1;
                let value = take_number(data, &mut pos)?;
                if index > 0 {
                    options.push(ParsedOption { index, parameter, value });
                }
            }
            _ => return None,
        }
    }
    Some(ParsedItemLink {
        item_id,
        refine,
        grade,
        view,
        location,
        equip,
        cards,
        options,
    })
}

fn find_ascii_ci(haystack: &str, needle: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.is_empty() || bytes.len() < needle.len() {
        return None;
    }
    let last = bytes.len() - needle.len();
    for index in 0..=last {
        let end = index + needle.len();
        if bytes[index..end].eq_ignore_ascii_case(needle) && haystack.is_char_boundary(index) && haystack.is_char_boundary(end) {
            return Some(index);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINNED: &str = "<ITEML>000020jn%07&00)01</ITEML>";

    fn knife_identity(slots: u8) -> ItemIdentity {
        ItemIdentity {
            name: "Knife".to_owned(),
            slots,
            description: "A simple knife.".to_owned(),
        }
    }

    fn option(index: u16, parameter: u8, value: u16) -> ItemOptions {
        ItemOptions { index, parameter, value }
    }

    #[allow(clippy::too_many_arguments)]
    fn gear(
        item_id: u32,
        item_type: u8,
        location: u32,
        refine: u8,
        grade: u8,
        view: u16,
        cards: [u32; 4],
        options: [ItemOptions; 5],
    ) -> InventoryItem<()> {
        let option_count = options.iter().filter(|entry| entry.index > 0).count() as u8;
        InventoryItem {
            metadata: (),
            index: InventoryIndex(0),
            item_id: ItemId(item_id),
            item_type,
            slot: cards,
            hire_expiration_date: 0,
            details: InventoryItemDetails::Equippable {
                equip_position: EquipPosition::from_bits_truncate(location),
                equipped_position: EquipPosition::empty(),
                bind_on_equip_type: 0,
                w_item_sprite_number: view,
                option_count,
                option_data: options,
                forge_data: None,
                refinement_level: refine,
                enchantment_level: grade,
                flags: EquippableItemFlags::IDENTIFIED,
            },
        }
    }

    #[test]
    fn base62_roundtrip_and_zero() {
        assert_eq!(encode_base62(0), "");
        assert_eq!(decode_base62("").unwrap(), 0);
        assert_eq!(encode_base62(1201), "jn");
        assert_eq!(decode_base62("jn").unwrap(), 1201);
        assert_eq!(decode_base62("000jn").unwrap(), 1201);
        assert_eq!(pad(10, 2), "0a");
        assert!(decode_base62("!").is_none());
    }

    #[test]
    fn pinned_fixture_and_display_label() {
        let item = gear(1201, 0, 2, 7, 0, 0, [1, 0, 0, 0], [
            zero_option(),
            zero_option(),
            zero_option(),
            zero_option(),
            zero_option(),
        ]);
        assert_eq!(encode_item_link(&item), PINNED);
        let parsed = parse_item_link(PINNED).unwrap();
        assert_eq!(parsed.item_id, 1201);
        assert_eq!(parsed.refine, 7);
        assert_eq!(parsed.location, 2);
        assert!(!parsed.equip);
        assert_eq!(parsed.view, 0);
        assert_eq!(parsed.grade, 0);
        assert_eq!(parsed.cards, [1, 0, 0, 0]);
        let identity = knife_identity(3);
        assert_eq!(parsed.display_label(Some(&identity)), "[+7 Knife [3]]");
        assert_eq!(parsed.display_label(None), "[Item #1201]");
        assert_eq!(
            parse_item_link("<ITEML>00000085&00</ITEML>")
                .unwrap()
                .display_label(Some(&ItemIdentity {
                    name: "Red Potion".to_owned(),
                    slots: 0,
                    description: String::new(),
                })),
            "[Red Potion]"
        );
    }

    #[test]
    fn fuller_link_roundtrip_and_hercules_four_cards() {
        let item = gear(1201, 0, 0, 7, 2, 3, [1, 0, 0, 0], [
            option(5, 1, 10),
            zero_option(),
            zero_option(),
            zero_option(),
            zero_option(),
        ]);
        let encoded = encode_item_link(&item);
        assert_eq!(encoded, "<ITEML>000000jn%07&03'02)01+05,01-0a</ITEML>");
        let parsed = parse_item_link(&encoded).unwrap();
        assert_eq!(parsed.item_id, 1201);
        assert_eq!(parsed.refine, 7);
        assert_eq!(parsed.view, 3);
        assert_eq!(parsed.grade, 2);
        assert_eq!(parsed.cards[0], 1);
        assert_eq!(parsed.options, [ParsedOption {
            index: 5,
            parameter: 1,
            value: 10
        }]);

        let hercules = parse_item_link("<ITEML>000000jn%07&00'00)00)00)01)00</ITEML>").unwrap();
        assert_eq!(hercules.item_id, 1201);
        assert_eq!(hercules.refine, 7);
        assert_eq!(hercules.grade, 0);
        assert_eq!(hercules.cards, [0, 0, 1, 0]);

        let lower = parse_item_link("<iteml>000020jn%07&00)01</iteml>").unwrap();
        assert_eq!(lower.item_id, 1201);
        assert_eq!(lower.refine, 7);
    }

    #[test]
    fn truncation_drops_options_then_cards_then_grade() {
        let item = gear(1201, 5, 2, 7, 2, 0, [1, 2, 0, 0], [
            option(5, 1, 10),
            option(6, 1, 11),
            zero_option(),
            zero_option(),
            zero_option(),
        ]);
        let full = encode_item_link(&item);
        let fitted = link_that_fits(parts_from_item(&item), full.len() - 1).unwrap();
        let parsed = parse_item_link(&fitted).unwrap();
        assert_eq!(parsed.item_id, 1201);
        assert_eq!(parsed.refine, 7);
        assert!(parsed.options.len() < 2);

        // 31 bytes keeps id, refine, and view. One card or the grade needs more.
        let minimal = link_that_fits(parts_from_item(&item), 31).unwrap();
        let parsed = parse_item_link(&minimal).unwrap();
        assert_eq!(parsed.item_id, 1201);
        assert_eq!(parsed.refine, 7);
        assert!(parsed.options.is_empty());
        assert_eq!(parsed.cards, [0, 0, 0, 0]);
        assert_eq!(parsed.grade, 0);
        assert!(minimal.contains("&00"));
        assert!(link_that_fits(parts_from_item(&item), 8).is_none());

        let mut draft = "a".repeat(70);
        let unchanged = append_link_text(&draft, &item);
        assert_eq!(unchanged, draft);
        draft = "hello".to_owned();
        let appended = append_link_text(&draft, &item);
        assert!(appended.starts_with("hello "));
        assert!(appended.ends_with(' '));
        assert!(appended.len() <= CHAT_DRAFT_LIMIT);
        let parsed = parse_item_link(&appended).unwrap();
        assert_eq!(parsed.item_id, 1201);
        assert_eq!(parsed.refine, 7);
    }

    #[test]
    fn malformed_links_stay_plain_and_fuzz_does_not_panic() {
        let junk = [
            "",
            "<ITEML>",
            "</ITEML>",
            "<ITEML></ITEML>",
            "<ITEML>zzzz</ITEML>",
            "<ITEML>\u{1F600}</ITEML>",
            "<<<<",
            "no tags",
        ];
        for sample in junk {
            let segments = chat_segments(sample, |_| None);
            assert!(segments.iter().all(|segment| segment.link.is_none()));
        }
        let mixed = chat_segments("see <ITEML>000020jn%07&00)01</ITEML> and <ITEML>bad</ITEML>", |_| None);
        assert_eq!(mixed.iter().filter(|segment| segment.link.is_some()).count(), 1);
        assert!(mixed.iter().any(|segment| segment.text.contains("<ITEML>bad</ITEML>")));

        let mut state = 0x1234_5678u32;
        for _ in 0..300 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let len = (state % 64) as usize;
            let mut sample = String::new();
            let mut mixer = state;
            for _ in 0..len {
                mixer = mixer.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                sample.push(char::from_u32((mixer % 96) + 32).unwrap_or('?'));
            }
            let _ = chat_segments(&sample, |_| None);
            let _ = parse_item_link(&sample);
        }
    }

    #[test]
    fn english_table_names_knife_and_unknown_ids_stay_numbered() {
        let row = crate::world::english_item_identity(1201).expect("knife");
        assert_eq!(row.name, "Knife");
        assert!(row.slots > 0);
        let parsed = parse_item_link(PINNED).unwrap();
        let identity = ItemIdentity {
            name: row.name,
            slots: row.slots,
            description: row.description,
        };
        assert_eq!(parsed.display_label(Some(&identity)), "[+7 Knife [4]]");
        let unknown_item = gear(999_999, 0, 0, 0, 0, 0, [0, 0, 0, 0], [
            zero_option(),
            zero_option(),
            zero_option(),
            zero_option(),
            zero_option(),
        ]);
        let unknown = parse_item_link(&encode_item_link(&unknown_item)).unwrap();
        assert_eq!(unknown.item_id, 999_999);
        assert_eq!(unknown.display_label(None), "[Item #999999]");
    }

    #[test]
    fn click_opens_details_for_the_synthetic_item() {
        let parsed = parse_item_link(PINNED).unwrap();
        let identity = knife_identity(3);
        let gold = Color::rgb(1.0, 0.8, 0.2);
        let event = parsed.open_details(Some(&identity), gold);
        let InputEvent::OpenItemDetails { key, title, body } = event else {
            panic!("expected OpenItemDetails");
        };
        assert_eq!(title, "[+7 Knife [3]]");
        assert!(body.contains("Knife"));
        assert!(body.contains("+7"));
        let item = parsed.to_inventory_item(Some(&identity));
        assert_eq!(item.item_id.0, 1201);
        match item.details {
            InventoryItemDetails::Equippable { refinement_level, .. } => assert_eq!(refinement_level, 7),
            InventoryItemDetails::Regular { .. } => panic!("refine should make the row equippable"),
        }
        assert_eq!(item.slot[0], 1);
        let again = parsed.open_details(Some(&identity), gold);
        let InputEvent::OpenItemDetails { key: same, .. } = again else {
            panic!("expected OpenItemDetails");
        };
        assert_eq!(key, same);
    }

    #[test]
    fn shift_selects_link_and_a_plain_click_drags() {
        set_shift_held(true);
        assert_eq!(item_click_action(), ItemClickAction::Link);
        set_shift_held(false);
        assert_eq!(item_click_action(), ItemClickAction::Drag);
        assert!(!shift_links());
    }

    #[test]
    fn channel_prefixes_keep_one_accent_link() {
        let identity = knife_identity(3);
        for prefix in ["Other : ", "[Party] ", "[Guild] ", "Friend : "] {
            let line = format!("{prefix}{PINNED}");
            let segments = chat_segments(&line, |_| Some(identity.clone()));
            let link = segments.iter().find(|segment| segment.accent).unwrap();
            assert_eq!(link.text, "[+7 Knife [3]]");
            assert_eq!(link.link.as_ref().unwrap().item_id, 1201);
            assert_eq!(link.link.as_ref().unwrap().refine, 7);
            assert!(
                segments
                    .iter()
                    .any(|segment| !segment.accent && segment.text.contains(prefix.trim()))
            );
        }
    }
}
