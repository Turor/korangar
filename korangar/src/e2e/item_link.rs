//! Shift+click `<ITEML>` links, headless.
//!
//! [`offline_item_link`] drives `InputSystem` the way a Shift+left click does,
//! appends the link with the same helper the client match arm calls, and
//! encodes the public chat packet. The live check is
//! [`super::LogicSession::item_link_on_test`].

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use ragnarok_bytes::ByteWriter;
use ragnarok_packets::version_20190530::GlobalMessagePacket;
use ragnarok_packets::{ClientTick, PacketExt};
use rust_state::State;
use winit::keyboard::KeyCode;

use crate::graphics::Color;
use crate::input::{InputEvent, InputSystem};
use crate::interface::windows::current_chat_draft;
use crate::loaders::GameFileLoader;
use crate::settings::GraphicsSettings;
use crate::state::ClientState;
use crate::state::item_link::{self, CHAT_DRAFT_LIMIT, ItemIdentity};

const PINNED_ID: u32 = 1201;
const PINNED_REFINE: u8 = 7;

/// What the offline run observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineItemLinkReport {
    pub draft: String,
    pub payload: String,
    pub packet: Vec<u8>,
    /// Shift+left click asks for a link. A click without Shift still drags.
    pub shift_links: bool,
    pub plain_click_drags: bool,
    pub label: String,
    pub accent: bool,
    pub item_id: u32,
    pub refine: u32,
    pub cards: [u32; 4],
    pub details_title: String,
    pub malformed_plain: bool,
    /// Bracketed label seen on public, party, guild, and whisper lines.
    pub channel_labels: Vec<String>,
}

fn knife() -> korangar_networking::InventoryItem<()> {
    use korangar_networking::InventoryItemDetails;
    use ragnarok_packets::{EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions};

    let empty = || ItemOptions {
        index: 0,
        value: 0,
        parameter: 0,
    };
    korangar_networking::InventoryItem {
        metadata: (),
        index: InventoryIndex(4),
        item_id: ItemId(PINNED_ID),
        item_type: 0,
        slot: [1, 0, 0, 0],
        hire_expiration_date: 0,
        details: InventoryItemDetails::Equippable {
            equip_position: EquipPosition::from_bits_truncate(2),
            equipped_position: EquipPosition::empty(),
            bind_on_equip_type: 0,
            w_item_sprite_number: 0,
            option_count: 0,
            option_data: [empty(), empty(), empty(), empty(), empty()],
            forge_data: None,
            refinement_level: PINNED_REFINE,
            enchantment_level: 0,
            flags: EquippableItemFlags::IDENTIFIED,
        },
    }
}

fn knife_name(item_id: u32) -> Option<ItemIdentity> {
    (item_id == PINNED_ID).then(|| ItemIdentity {
        name: "Knife".to_owned(),
        slots: 3,
        description: "A simple knife.".to_owned(),
    })
}

fn shift_click_state() -> (bool, bool) {
    let mut input = InputSystem::new(Arc::new(AtomicU64::new(0)));
    input.expire_synthesized();
    input.synthesize_key(KeyCode::ShiftLeft, false, false, false);
    input.synthesize_click(8.0, 8.0, korangar_interface::layout::MouseButton::Left);
    let report = input.update_delta(ClientTick(1));
    let shift = input.key_down(KeyCode::ShiftLeft);
    let clicked = report.mouse_click == Some(korangar_interface::layout::MouseButton::Left);
    item_link::set_shift_held(shift);
    let links = item_link::item_click_action() == item_link::ItemClickAction::Link;
    item_link::set_shift_held(false);
    input.expire_synthesized();
    input.synthesize_click(8.0, 8.0, korangar_interface::layout::MouseButton::Left);
    let plain = input.update_delta(ClientTick(1_000));
    let plain_shift = input.key_down(KeyCode::ShiftLeft);
    item_link::set_shift_held(plain_shift);
    let drags = item_link::item_click_action() == item_link::ItemClickAction::Drag
        && plain.mouse_click == Some(korangar_interface::layout::MouseButton::Left);
    item_link::set_shift_held(false);
    (links && clicked && shift, drags)
}

fn encode_chat(message: String) -> Result<Vec<u8>, String> {
    let mut writer = ByteWriter::new();
    GlobalMessagePacket { message }
        .packet_to_bytes(&mut writer)
        .map_err(|_| "chat packet did not encode".to_owned())?;
    Ok(writer.into_inner())
}

struct RecordingHost {
    opened: bool,
}

impl item_link::ChatInputHost for RecordingHost {
    fn open_chat_input(&mut self) {
        self.opened = true;
    }
}

fn draft_state() -> Result<State<ClientState>, String> {
    let loader = GameFileLoader::default();
    loader
        .load_archives_from_settings()
        .map_err(|error| format!("game archives: {error}"))?;
    Ok(State::new(ClientState::new(
        &loader,
        GraphicsSettings::new(),
        #[cfg(feature = "debug")]
        crate::networking::PacketHistory::new().0,
    )))
}

/// Shift+click path, outbound `0x00F3` bytes, and inbound link segments.
pub fn offline_item_link() -> Result<OfflineItemLinkReport, String> {
    let (shift_links, plain_click_drags) = shift_click_state();
    let item = knife();
    let event = InputEvent::LinkItemInChat { item: owned_knife() };
    let mut state = draft_state()?;
    let mut host = RecordingHost { opened: false };
    match event {
        InputEvent::LinkItemInChat { item } => item_link::link_item_in_chat(&mut state, &mut host, item),
        _ => return Err("link event was not LinkItemInChat".to_owned()),
    }
    state.apply().map_err(|_| "chat draft update did not apply".to_owned())?;
    if !host.opened {
        return Err("link did not open the chat input".to_owned());
    }
    let draft = current_chat_draft(&state);
    let payload = item_link::encode_item_link(&item);
    if item_link::parse_item_link(&payload).is_none() {
        return Err("encoded link did not parse".to_owned());
    }
    if !draft.contains(&payload) {
        return Err("draft is missing the item link".to_owned());
    }
    if draft.len() > CHAT_DRAFT_LIMIT {
        return Err("draft is longer than the chat box".to_owned());
    }
    let packet = encode_chat(format!("Tester : {draft}"))?;
    if packet.len() < 2 || packet[0] != 0xF3 || packet[1] != 0x00 {
        return Err("outbound chat header is not 0x00F3".to_owned());
    }
    if !packet.windows(payload.len()).any(|window| window == payload.as_bytes()) {
        return Err("outbound chat bytes do not carry the link".to_owned());
    }

    let segments = item_link::chat_segments(&format!("Other : {payload}"), knife_name);
    let link_segment = segments
        .iter()
        .find(|segment| segment.accent)
        .ok_or_else(|| "inbound line has no accent link".to_owned())?;
    let parsed = link_segment.link.clone().ok_or_else(|| "accent run has no item".to_owned())?;
    let gold = Color::rgb(1.0, 0.8, 0.2);
    let details = parsed.open_details(knife_name(parsed.item_id).as_ref(), gold);
    let details_title = match details {
        InputEvent::OpenItemDetails { title, .. } => title,
        _ => return Err("click did not queue OpenItemDetails".to_owned()),
    };
    let malformed = item_link::chat_segments("<ITEML>not-a-link</ITEML>", |_| None);
    let malformed_plain = malformed.len() == 1 && malformed[0].link.is_none() && !malformed[0].accent;

    let mut channel_labels = Vec::new();
    for prefix in ["Other : ", "[Party] ", "[Guild] ", "Friend : "] {
        let line = format!("{prefix}{payload}");
        let segments = item_link::chat_segments(&line, knife_name);
        let label = segments
            .iter()
            .find(|segment| segment.accent)
            .map(|segment| segment.text.clone())
            .ok_or_else(|| format!("{prefix} line has no link"))?;
        channel_labels.push(label);
    }

    Ok(OfflineItemLinkReport {
        draft,
        payload,
        packet,
        shift_links,
        plain_click_drags,
        label: link_segment.text.clone(),
        accent: link_segment.accent,
        item_id: parsed.item_id,
        refine: parsed.refine,
        cards: parsed.cards,
        details_title,
        malformed_plain,
        channel_labels,
    })
}

fn owned_knife() -> korangar_networking::InventoryItem<crate::world::ResourceMetadata> {
    let source = knife();
    korangar_networking::InventoryItem {
        metadata: crate::world::ResourceMetadata {
            texture: None,
            name: "Knife".to_owned(),
            description: "A simple knife.".to_owned(),
        },
        index: source.index,
        item_id: source.item_id,
        item_type: source.item_type,
        slot: source.slot,
        hire_expiration_date: source.hire_expiration_date,
        details: source.details,
    }
}
