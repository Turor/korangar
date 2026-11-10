//! Chat channels, tab filters, whisper routing, overhead bubbles, and the
//! announce banner.
//!
//! Party (`MessageColor::Party` or [`ChatChannel::Party`]) and guild
//! (`MessageColor::Guild` or [`ChatChannel::Guild`]) are accepted here so G04
//! and the guild task can push lines without defining the channel again.
use std::fmt::Display;

use korangar_interface::element::StateElement;
use korangar_networking::MessageColor;
use ragnarok_packets::EntityId;
use rust_state::RustState;

use super::ChatMessage;

pub const BUBBLE_LIFETIME_MS: u32 = 5_000;
pub const ANNOUNCE_LIFETIME_MS: u32 = 12_000;
const MAX_RECENT_WHISPER_TARGETS: usize = 8;
const MAX_BUBBLES: usize = 32;

/// Where a chat line is routed. Independent of the color token so a whisper
/// failure can stay on the whisper tab while using the error color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, RustState)]
pub enum ChatChannel {
    Public,
    Whisper,
    Party,
    Guild,
    System,
    Announce,
}

impl Display for ChatChannel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Public => "Public",
            Self::Whisper => "Whisper",
            Self::Party => "Party",
            Self::Guild => "Guild",
            Self::System => "System",
            Self::Announce => "Announce",
        };
        formatter.write_str(label)
    }
}

impl ChatChannel {
    pub fn from_message_color(color: MessageColor) -> Self {
        match color {
            MessageColor::Rgb { .. } | MessageColor::Public => Self::Public,
            MessageColor::Whisper => Self::Whisper,
            MessageColor::Party => Self::Party,
            MessageColor::Guild => Self::Guild,
            MessageColor::Broadcast => Self::Announce,
            MessageColor::Server | MessageColor::Error | MessageColor::Information => Self::System,
        }
    }
}

/// Which channels a tab currently shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, RustState, StateElement)]
pub struct ChannelMask {
    pub public_chat: bool,
    pub whisper: bool,
    pub party: bool,
    pub guild: bool,
    pub system: bool,
    pub announce: bool,
}

impl ChannelMask {
    pub const fn all() -> Self {
        Self {
            public_chat: true,
            whisper: true,
            party: true,
            guild: true,
            system: true,
            announce: true,
        }
    }

    pub const fn only_whisper() -> Self {
        Self {
            public_chat: false,
            whisper: true,
            party: false,
            guild: false,
            system: false,
            announce: false,
        }
    }

    pub const fn only_party() -> Self {
        Self {
            public_chat: false,
            whisper: false,
            party: true,
            guild: false,
            system: false,
            announce: false,
        }
    }

    pub const fn only_guild() -> Self {
        Self {
            public_chat: false,
            whisper: false,
            party: false,
            guild: true,
            system: false,
            announce: false,
        }
    }

    pub const fn system_and_announce() -> Self {
        Self {
            public_chat: false,
            whisper: false,
            party: false,
            guild: false,
            system: true,
            announce: true,
        }
    }

    pub fn allows(self, channel: ChatChannel) -> bool {
        match channel {
            ChatChannel::Public => self.public_chat,
            ChatChannel::Whisper => self.whisper,
            ChatChannel::Party => self.party,
            ChatChannel::Guild => self.guild,
            ChatChannel::System => self.system,
            ChatChannel::Announce => self.announce,
        }
    }

    pub fn set(&mut self, channel: ChatChannel, enabled: bool) {
        match channel {
            ChatChannel::Public => self.public_chat = enabled,
            ChatChannel::Whisper => self.whisper = enabled,
            ChatChannel::Party => self.party = enabled,
            ChatChannel::Guild => self.guild = enabled,
            ChatChannel::System => self.system = enabled,
            ChatChannel::Announce => self.announce = enabled,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, RustState)]
pub enum ChatTab {
    #[default]
    All,
    Whisper,
    Party,
    Guild,
    System,
}

impl Display for ChatTab {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

impl ChatTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Whisper => "Whisper",
            Self::Party => "Party",
            Self::Guild => "Guild",
            Self::System => "System",
        }
    }
}

/// Active tab plus the filter stored for every tab.
///
/// [`Self::shown`] mirrors the active tab so the chat window can bind
/// checkboxes without swapping paths when the player changes tabs.
#[derive(Debug, Clone, PartialEq, Eq, RustState, StateElement)]
pub struct ChatFilterState {
    pub active: ChatTab,
    pub shown: ChannelMask,
    pub all: ChannelMask,
    pub whisper: ChannelMask,
    pub party: ChannelMask,
    pub guild: ChannelMask,
    pub system: ChannelMask,
}

impl Default for ChatFilterState {
    fn default() -> Self {
        let all = ChannelMask::all();
        Self {
            active: ChatTab::All,
            shown: all,
            all,
            whisper: ChannelMask::only_whisper(),
            party: ChannelMask::only_party(),
            guild: ChannelMask::only_guild(),
            system: ChannelMask::system_and_announce(),
        }
    }
}

impl ChatFilterState {
    pub fn select(&mut self, tab: ChatTab) {
        self.active = tab;
        self.shown = self.stored(tab);
    }

    pub fn toggle(&mut self, channel: ChatChannel) {
        let enabled = !self.shown.allows(channel);
        self.shown.set(channel, enabled);
        match self.active {
            ChatTab::All => self.all = self.shown,
            ChatTab::Whisper => self.whisper = self.shown,
            ChatTab::Party => self.party = self.shown,
            ChatTab::Guild => self.guild = self.shown,
            ChatTab::System => self.system = self.shown,
        }
    }

    pub fn allows_message(&self, message: &ChatMessage) -> bool {
        self.shown.allows(message.channel)
    }

    fn stored(&self, tab: ChatTab) -> ChannelMask {
        match tab {
            ChatTab::All => self.all,
            ChatTab::Whisper => self.whisper,
            ChatTab::Party => self.party,
            ChatTab::Guild => self.guild,
            ChatTab::System => self.system,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutgoingChat {
    Public(String),
    Whisper { target: String, text: String },
    /// Guild channel. The caller formats `{name} : {text}` for `0x017E`.
    Guild(String),
}

/// `/w` and `/whisper`, then `/g` and `/guild`, then `name : text`, then the
/// whisper-target box. A guild chat tab sends guild chat when the text is not
/// a command and no whisper target is set. `@` commands stay public.
pub fn parse_outgoing_chat(raw: &str, whisper_target: &str, send_to_guild: bool) -> OutgoingChat {
    let trimmed = raw.trim();
    if let Some(whisper) = parse_whisper_command(trimmed) {
        return whisper;
    }
    if let Some(guild) = parse_guild_command(trimmed) {
        return guild;
    }
    if let Some((name, text)) = trimmed.split_once(" : ") {
        let name = name.trim();
        let text = text.trim();
        if !name.is_empty() && !text.is_empty() && !name.contains(' ') && !name.starts_with('/') && !name.starts_with('@') {
            return OutgoingChat::Whisper {
                target: name.to_string(),
                text: text.to_string(),
            };
        }
    }

    let target = whisper_target.trim();
    if !target.is_empty() && !trimmed.starts_with('/') && !trimmed.starts_with('@') {
        return OutgoingChat::Whisper {
            target: target.to_string(),
            text: trimmed.to_string(),
        };
    }

    if send_to_guild && !trimmed.is_empty() && !trimmed.starts_with('/') && !trimmed.starts_with('@') {
        return OutgoingChat::Guild(trimmed.to_string());
    }

    OutgoingChat::Public(trimmed.to_string())
}

fn parse_guild_command(trimmed: &str) -> Option<OutgoingChat> {
    let rest = trimmed
        .strip_prefix("/guild ")
        .or_else(|| trimmed.strip_prefix("/g "))
        .or_else(|| trimmed.strip_prefix("/G "))?;
    let text = rest.trim();
    if text.is_empty() { None } else { Some(OutgoingChat::Guild(text.to_string())) }
}

fn parse_whisper_command(trimmed: &str) -> Option<OutgoingChat> {
    let rest = trimmed
        .strip_prefix("/whisper ")
        .or_else(|| trimmed.strip_prefix("/w "))
        .or_else(|| trimmed.strip_prefix("/W "))?;
    let (name, text) = rest.split_once(' ')?;
    let name = name.trim();
    let text = text.trim();
    if name.is_empty() || text.is_empty() || name.contains(' ') {
        return None;
    }
    Some(OutgoingChat::Whisper {
        target: name.to_string(),
        text: text.to_string(),
    })
}

pub fn remember_whisper_target(recent: &mut Vec<String>, name: &str) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    recent.retain(|existing| !existing.eq_ignore_ascii_case(name));
    recent.insert(0, name.to_string());
    recent.truncate(MAX_RECENT_WHISPER_TARGETS);
}

pub fn strip_whisper_code(message: &str) -> &str {
    let bytes = message.as_bytes();
    if bytes.len() >= 3 && bytes[0] == b'|' && bytes[1].is_ascii_digit() && bytes[2].is_ascii_digit() {
        &message[3..]
    } else {
        message
    }
}

/// Text drawn above a character. Server speech is usually `Name : text`.
pub fn speech_bubble_text(message: &str) -> &str {
    message.split_once(" : ").map(|(_, rest)| rest.trim()).unwrap_or(message)
}

pub fn whisper_failure_line(name: &str, result: u8) -> String {
    match result {
        2 => format!("{name} is ignoring you."),
        3 => format!("{name} is ignoring everyone."),
        _ => format!("{name} is offline."),
    }
}

/// `true` once `now` has reached `until`, including equality. Wrapping client
/// ticks stay ordered as long as the span is under half the `u32` range.
pub fn tick_reached(now: u32, until: u32) -> bool {
    now.wrapping_sub(until) < u32::MAX / 2
}

#[derive(Debug, Clone, RustState, StateElement)]
pub struct ChatBubble {
    pub entity_id: EntityId,
    pub text: String,
    pub started: u32,
    pub until: u32,
}

impl ChatBubble {
    pub fn new(entity_id: EntityId, text: String, now: u32) -> Self {
        Self {
            entity_id,
            text,
            started: now,
            until: now.wrapping_add(BUBBLE_LIFETIME_MS),
        }
    }
}

pub fn upsert_bubble(bubbles: &mut Vec<ChatBubble>, entity_id: EntityId, text: String, now: u32) {
    if text.is_empty() {
        return;
    }
    if let Some(existing) = bubbles.iter_mut().find(|bubble| bubble.entity_id == entity_id) {
        existing.text = text;
        existing.started = now;
        existing.until = now.wrapping_add(BUBBLE_LIFETIME_MS);
        return;
    }
    bubbles.push(ChatBubble::new(entity_id, text, now));
    if bubbles.len() > MAX_BUBBLES {
        let extra = bubbles.len() - MAX_BUBBLES;
        bubbles.drain(0..extra);
    }
}

pub fn expire_bubbles(bubbles: &mut Vec<ChatBubble>, now: u32) {
    bubbles.retain(|bubble| !tick_reached(now, bubble.until));
}

#[derive(Debug, Clone, RustState, StateElement)]
pub struct AnnounceBanner {
    pub text: String,
    pub color: MessageColor,
    pub started: u32,
    pub until: u32,
}

pub fn set_announce(slot: &mut Option<AnnounceBanner>, text: String, color: MessageColor, now: u32) {
    *slot = Some(AnnounceBanner {
        text,
        color,
        started: now,
        until: now.wrapping_add(ANNOUNCE_LIFETIME_MS),
    });
}

pub fn expire_announce(slot: &mut Option<AnnounceBanner>, now: u32) {
    if slot.as_ref().is_some_and(|banner| tick_reached(now, banner.until)) {
        *slot = None;
    }
}

pub fn fade_alpha(started: u32, until: u32, now: u32) -> f32 {
    if tick_reached(now, until) {
        return 0.0;
    }
    let duration = until.wrapping_sub(started).max(1);
    let elapsed = now.wrapping_sub(started);
    if elapsed >= duration {
        return 0.0;
    }
    let remaining = duration - elapsed;
    let fade_window = (duration / 5).max(1);
    if remaining >= fade_window {
        1.0
    } else {
        remaining as f32 / fade_window as f32
    }
}

pub fn announce_scroll_left(started: u32, until: u32, now: u32, width: f32, text_width: f32) -> f32 {
    let duration = until.wrapping_sub(started).max(1);
    let elapsed = now.wrapping_sub(started).min(duration);
    let progress = elapsed as f32 / duration as f32;
    (1.0 - progress) * width - progress * text_width
}

pub fn record_entity_speech(
    messages: &mut Vec<ChatMessage>,
    bubbles: &mut Vec<ChatBubble>,
    entity_id: EntityId,
    text: String,
    color: MessageColor,
    now: u32,
) {
    let bubble = speech_bubble_text(&text).to_string();
    messages.push(ChatMessage::with_channel(text, color, ChatChannel::Public));
    upsert_bubble(bubbles, entity_id, bubble, now);
}

pub fn record_broadcast(
    messages: &mut Vec<ChatMessage>,
    announce: &mut Option<AnnounceBanner>,
    text: String,
    color: MessageColor,
    now: u32,
) {
    set_announce(announce, text.clone(), color, now);
    messages.push(ChatMessage::with_channel(text, color, ChatChannel::Announce));
}

pub fn record_whisper(messages: &mut Vec<ChatMessage>, recent: &mut Vec<String>, sender: &str, text: &str) {
    let sender = sender.trim();
    let text = strip_whisper_code(text).trim();
    if sender.is_empty() || text.is_empty() {
        return;
    }
    remember_whisper_target(recent, sender);
    messages.push(ChatMessage::new(format!("[From {sender}] : {text}"), MessageColor::Whisper));
}

pub fn record_whisper_ack(
    messages: &mut Vec<ChatMessage>,
    recent: &mut Vec<String>,
    pending_name: &mut String,
    pending_text: &mut String,
    result: u8,
) {
    let name = std::mem::take(pending_name);
    let text = std::mem::take(pending_text);
    if result == 0 {
        if name.is_empty() || text.is_empty() {
            return;
        }
        remember_whisper_target(recent, &name);
        messages.push(ChatMessage::new(format!("[To {name}] : {text}"), MessageColor::Whisper));
        return;
    }
    let who = if name.is_empty() { "Player".to_string() } else { name };
    messages.push(ChatMessage::with_channel(
        whisper_failure_line(&who, result),
        MessageColor::Error,
        ChatChannel::Whisper,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(color: MessageColor) -> ChatMessage {
        ChatMessage::new(color_label(color), color)
    }

    fn color_label(color: MessageColor) -> String {
        match color {
            MessageColor::Public => "public",
            MessageColor::Whisper => "whisper",
            MessageColor::Party => "party",
            MessageColor::Guild => "guild",
            MessageColor::Server => "system",
            MessageColor::Broadcast => "announce",
            _ => "other",
        }
        .to_string()
    }

    #[test]
    fn tabs_filter_party_and_guild_channels() {
        let mut filters = ChatFilterState::default();
        let public = message(MessageColor::Public);
        let whisper = message(MessageColor::Whisper);
        let party = message(MessageColor::Party);
        let guild = message(MessageColor::Guild);
        let system = message(MessageColor::Server);
        let announce = message(MessageColor::Broadcast);

        assert!(filters.allows_message(&public));
        assert!(filters.allows_message(&party));
        assert!(filters.allows_message(&announce));

        filters.select(ChatTab::Whisper);
        assert!(filters.allows_message(&whisper));
        assert!(!filters.allows_message(&public));
        assert!(!filters.allows_message(&party));

        filters.select(ChatTab::Party);
        assert!(filters.allows_message(&party));
        assert!(!filters.allows_message(&guild));
        filters.toggle(ChatChannel::Party);
        assert!(!filters.allows_message(&party));
        filters.select(ChatTab::All);
        assert!(filters.allows_message(&party));
        filters.select(ChatTab::Party);
        assert!(!filters.allows_message(&party));

        filters.select(ChatTab::Guild);
        assert!(filters.allows_message(&guild));
        filters.select(ChatTab::System);
        assert!(filters.allows_message(&system));
        assert!(filters.allows_message(&announce));
        assert!(!filters.allows_message(&public));
    }

    #[test]
    fn outgoing_chat_routes_whispers() {
        assert!(matches!(
            parse_outgoing_chat("hello", "", false),
            OutgoingChat::Public(text) if text == "hello"
        ));
        assert!(matches!(
            parse_outgoing_chat("/w Bob hi there", "", false),
            OutgoingChat::Whisper { target, text } if target == "Bob" && text == "hi there"
        ));
        assert!(matches!(
            parse_outgoing_chat("/whisper Cara ping", "", false),
            OutgoingChat::Whisper { target, text } if target == "Cara" && text == "ping"
        ));
        assert!(matches!(
            parse_outgoing_chat("Dana : secret", "", false),
            OutgoingChat::Whisper { target, text } if target == "Dana" && text == "secret"
        ));
        assert!(matches!(
            parse_outgoing_chat("hello", "Eve", true),
            OutgoingChat::Whisper { target, text } if target == "Eve" && text == "hello"
        ));
        assert!(matches!(parse_outgoing_chat("@broadcast hi", "Eve", true), OutgoingChat::Public(_)));
        assert!(matches!(parse_outgoing_chat("@guild", "", true), OutgoingChat::Public(_)));
        assert!(matches!(
            parse_outgoing_chat("/g rally at the castle", "", false),
            OutgoingChat::Guild(text) if text == "rally at the castle"
        ));
        assert!(matches!(
            parse_outgoing_chat("/guild on my way", "", false),
            OutgoingChat::Guild(text) if text == "on my way"
        ));
        assert!(matches!(
            parse_outgoing_chat("hello guild", "", true),
            OutgoingChat::Guild(text) if text == "hello guild"
        ));
        assert!(matches!(parse_outgoing_chat("/where", "", true), OutgoingChat::Public(_)));
    }

    #[test]
    fn bubbles_expire_and_replace_the_same_speaker() {
        let mut bubbles = Vec::new();
        upsert_bubble(&mut bubbles, EntityId(4), "first".to_string(), 1_000);
        upsert_bubble(&mut bubbles, EntityId(4), "second".to_string(), 1_500);
        upsert_bubble(&mut bubbles, EntityId(9), "other".to_string(), 1_500);
        assert_eq!(bubbles.len(), 2);
        assert_eq!(bubbles[0].text, "second");
        assert_eq!(bubbles[0].until, 1_500 + BUBBLE_LIFETIME_MS);

        expire_bubbles(&mut bubbles, 1_500 + BUBBLE_LIFETIME_MS - 1);
        assert_eq!(bubbles.len(), 2);
        expire_bubbles(&mut bubbles, 1_500 + BUBBLE_LIFETIME_MS);
        assert!(bubbles.is_empty());
    }

    #[test]
    fn announce_fades_near_the_end_and_scrolls() {
        let started = 100u32;
        let until = started + ANNOUNCE_LIFETIME_MS;
        assert_eq!(fade_alpha(started, until, started), 1.0);
        assert!(fade_alpha(started, until, until - 10) < 1.0);
        assert_eq!(fade_alpha(started, until, until), 0.0);

        let left = announce_scroll_left(started, until, started, 800.0, 200.0);
        assert!((left - 800.0).abs() < 0.01);
        let end = announce_scroll_left(started, until, until, 800.0, 200.0);
        assert!(end < 0.0);

        let mut slot = None;
        set_announce(&mut slot, "hi".to_string(), MessageColor::Broadcast, started);
        expire_announce(&mut slot, until - 1);
        assert!(slot.is_some());
        expire_announce(&mut slot, until);
        assert!(slot.is_none());
    }

    #[test]
    fn whisper_ack_and_receive_use_the_whisper_channel() {
        let mut messages = Vec::new();
        let mut recent = Vec::new();
        record_whisper(&mut messages, &mut recent, "Bob", "|00hello");
        assert_eq!(messages[0].text, "[From Bob] : hello");
        assert_eq!(messages[0].channel, ChatChannel::Whisper);
        assert_eq!(recent, vec!["Bob".to_string()]);

        let mut pending_name = "Cara".to_string();
        let mut pending_text = "yo".to_string();
        record_whisper_ack(&mut messages, &mut recent, &mut pending_name, &mut pending_text, 0);
        assert!(pending_name.is_empty());
        assert_eq!(messages[1].text, "[To Cara] : yo");
        assert_eq!(messages[1].channel, ChatChannel::Whisper);
        assert_eq!(recent[0], "Cara");

        let mut pending_name = "Bob".to_string();
        let mut pending_text = "again".to_string();
        record_whisper_ack(&mut messages, &mut recent, &mut pending_name, &mut pending_text, 2);
        assert_eq!(messages[2].channel, ChatChannel::Whisper);
        assert_eq!(messages[2].text, "Bob is ignoring you.");
        assert!(matches!(messages[2].color, MessageColor::Error));
    }
}
