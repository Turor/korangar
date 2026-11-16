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
/// Gap between stacked chat lines. The message pane and its tests share this.
pub const CHAT_LINE_SPACING: f32 = 5.0;
/// How close to the bottom (pixels) still counts as pinned, matching
/// roBrowser's `shouldScrollDownBeforeAdd` tolerance.
pub const LIST_FOLLOW_SLOP: f32 = 5.0;
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

/// Fixed channel tabs plus one slot per whisper partner.
///
/// Partner tabs are session-only. The generic Whisper tab stays and shows
/// every whisper.
pub const FIXED_CHAT_TABS: usize = 5;
/// Oldest inactive partner tab is recycled once this many are open.
pub const MAX_PARTNER_TABS: usize = 8;
/// Unread badges render this value as `99+`.
pub const UNREAD_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Default, PartialEq, Eq, RustState)]
pub enum ChatTab {
    #[default]
    All,
    Whisper,
    Party,
    Guild,
    System,
    Partner(String),
}

impl Display for ChatTab {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label())
    }
}

impl ChatTab {
    /// Chat window title for this tab, such as `Chat - Party`.
    pub fn window_title(&self) -> String {
        format!("Chat - {}", self.label())
    }

    pub fn label(&self) -> String {
        match self {
            Self::All => "All".to_owned(),
            Self::Whisper => "Whisper".to_owned(),
            Self::Party => "Party".to_owned(),
            Self::Guild => "Guild".to_owned(),
            Self::System => "System".to_owned(),
            Self::Partner(name) => name.clone(),
        }
    }

    pub fn partner_name(&self) -> Option<&str> {
        match self {
            Self::Partner(name) => Some(name.as_str()),
            _ => None,
        }
    }
}

/// One closable whisper conversation. `seen` is the first chat-log index that
/// is still unread.
#[derive(Debug, Clone, PartialEq, Eq, RustState)]
pub struct PartnerTab {
    pub name: String,
    pub mask: ChannelMask,
    pub seen: usize,
    /// Newer tabs and the tab the player just opened have a higher stamp.
    pub stamp: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FixedSeen {
    all: usize,
    whisper: usize,
    party: usize,
    guild: usize,
    system: usize,
}

/// One row the chat tab strip draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTabSlot {
    pub slot: usize,
    pub tab: ChatTab,
    pub label: String,
    pub unread: u32,
    pub closable: bool,
    pub selected: bool,
}

/// Active tab plus the filter stored for every fixed tab.
///
/// [`Self::shown`] mirrors the active tab so the chat window can bind
/// checkboxes without swapping paths when the player changes tabs. Partner
/// tabs keep their own whisper mask and are not edited by those checkboxes.
#[derive(Debug, Clone, PartialEq, Eq, RustState, StateElement)]
pub struct ChatFilterState {
    #[hidden_element]
    pub active: ChatTab,
    pub shown: ChannelMask,
    pub all: ChannelMask,
    pub whisper: ChannelMask,
    pub party: ChannelMask,
    pub guild: ChannelMask,
    pub system: ChannelMask,
    #[hidden_element]
    pub partners: Vec<PartnerTab>,
    #[hidden_element]
    seen: FixedSeen,
    #[hidden_element]
    partner_clock: u64,
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
            partners: Vec::new(),
            seen: FixedSeen::default(),
            partner_clock: 0,
        }
    }
}

impl ChatFilterState {
    /// Select `tab` and treat every line already in the log as read on both
    /// the tab being left and the tab being opened.
    pub fn select_at(&mut self, tab: ChatTab, message_len: usize) {
        let tab = self.canonical(tab);
        self.mark_seen(&self.active.clone(), message_len);
        self.active = tab;
        self.mark_seen(&self.active.clone(), message_len);
        if let Some(name) = self.active.partner_name().map(str::to_owned) {
            self.touch_partner(&name);
        }
        self.shown = self.mask_of(&self.active);
    }

    #[cfg(test)]
    pub fn select(&mut self, tab: ChatTab) {
        self.select_at(tab, 0);
    }

    pub fn toggle(&mut self, channel: ChatChannel) {
        if self.active.partner_name().is_some() {
            return;
        }
        let enabled = !self.shown.allows(channel);
        self.shown.set(channel, enabled);
        match &self.active {
            ChatTab::All => self.all = self.shown,
            ChatTab::Whisper => self.whisper = self.shown,
            ChatTab::Party => self.party = self.shown,
            ChatTab::Guild => self.guild = self.shown,
            ChatTab::System => self.system = self.shown,
            ChatTab::Partner(_) => {}
        }
    }

    pub fn allows_message(&self, message: &ChatMessage) -> bool {
        self.tab_accepts(&self.active, message)
    }

    /// Open `name` or reuse the existing partner tab. A ninth tab replaces the
    /// oldest one that is not currently selected.
    pub fn ensure_partner(&mut self, name: &str, message_index: usize) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(index) = self.partners.iter().position(|partner| names_equal(&partner.name, name)) {
            let stamp = self.next_stamp();
            self.partners[index].stamp = stamp;
            return;
        }
        if self.partners.len() >= MAX_PARTNER_TABS
            && let Some(index) = self.oldest_inactive()
        {
            let stamp = self.next_stamp();
            let partner = &mut self.partners[index];
            partner.name = name.to_owned();
            partner.mask = ChannelMask::only_whisper();
            partner.seen = message_index;
            partner.stamp = stamp;
            return;
        }
        if self.partners.len() >= MAX_PARTNER_TABS {
            return;
        }
        let stamp = self.next_stamp();
        self.partners.push(PartnerTab {
            name: name.to_owned(),
            mask: ChannelMask::only_whisper(),
            seen: message_index,
            stamp,
        });
    }

    pub fn remove_partner(&mut self, index: usize) -> Option<String> {
        if index >= self.partners.len() {
            return None;
        }
        Some(self.partners.remove(index).name)
    }

    #[cfg(test)]
    pub fn partner_names(&self) -> Vec<String> {
        self.partners.iter().map(|partner| partner.name.clone()).collect()
    }

    pub fn tab_for_slot(&self, slot: usize) -> Option<ChatTab> {
        match slot {
            0 => Some(ChatTab::All),
            1 => Some(ChatTab::Whisper),
            2 => Some(ChatTab::Party),
            3 => Some(ChatTab::Guild),
            4 => Some(ChatTab::System),
            other => self
                .partners
                .get(other - FIXED_CHAT_TABS)
                .map(|partner| ChatTab::Partner(partner.name.clone())),
        }
    }

    pub fn tab_slots(&self, messages: &[ChatMessage], player_name: &str) -> Vec<ChatTabSlot> {
        let fixed = [ChatTab::All, ChatTab::Whisper, ChatTab::Party, ChatTab::Guild, ChatTab::System];
        let mut slots = Vec::with_capacity(fixed.len() + self.partners.len());
        for (index, tab) in fixed.into_iter().enumerate() {
            slots.push(self.slot(index, tab, false, messages, player_name));
        }
        for (index, partner) in self.partners.iter().enumerate() {
            let tab = ChatTab::Partner(partner.name.clone());
            slots.push(self.slot(FIXED_CHAT_TABS + index, tab, true, messages, player_name));
        }
        slots
    }

    /// Lines that arrived after this tab was last selected, that this tab
    /// shows, and that the player did not send.
    pub fn unread_count(&self, tab: &ChatTab, messages: &[ChatMessage], player_name: &str) -> u32 {
        if same_tab(&self.active, tab) {
            return 0;
        }
        let seen = self.seen_of(tab);
        messages
            .iter()
            .skip(seen)
            .filter(|message| self.tab_accepts(tab, message) && !is_own_outgoing_line(message, player_name))
            .count() as u32
    }

    fn slot(&self, index: usize, tab: ChatTab, closable: bool, messages: &[ChatMessage], player_name: &str) -> ChatTabSlot {
        let unread = self.unread_count(&tab, messages, player_name);
        ChatTabSlot {
            slot: index,
            label: tab.label(),
            selected: same_tab(&self.active, &tab),
            tab,
            unread,
            closable,
        }
    }

    fn canonical(&self, tab: ChatTab) -> ChatTab {
        match tab {
            ChatTab::Partner(name) => {
                let stored = self
                    .partners
                    .iter()
                    .find(|partner| names_equal(&partner.name, &name))
                    .map(|partner| partner.name.clone())
                    .unwrap_or(name);
                ChatTab::Partner(stored)
            }
            other => other,
        }
    }

    fn tab_accepts(&self, tab: &ChatTab, message: &ChatMessage) -> bool {
        match tab {
            ChatTab::Partner(name) => self.mask_of(tab).allows(message.channel) && whisper_for_partner(message, name),
            _ => self.mask_of(tab).allows(message.channel),
        }
    }

    fn mask_of(&self, tab: &ChatTab) -> ChannelMask {
        match tab {
            ChatTab::All => self.all,
            ChatTab::Whisper => self.whisper,
            ChatTab::Party => self.party,
            ChatTab::Guild => self.guild,
            ChatTab::System => self.system,
            ChatTab::Partner(name) => self
                .partners
                .iter()
                .find(|partner| names_equal(&partner.name, name))
                .map(|partner| partner.mask)
                .unwrap_or_else(ChannelMask::only_whisper),
        }
    }

    fn seen_of(&self, tab: &ChatTab) -> usize {
        match tab {
            ChatTab::All => self.seen.all,
            ChatTab::Whisper => self.seen.whisper,
            ChatTab::Party => self.seen.party,
            ChatTab::Guild => self.seen.guild,
            ChatTab::System => self.seen.system,
            ChatTab::Partner(name) => self
                .partners
                .iter()
                .find(|partner| names_equal(&partner.name, name))
                .map(|partner| partner.seen)
                .unwrap_or(usize::MAX),
        }
    }

    fn mark_seen(&mut self, tab: &ChatTab, message_len: usize) {
        match tab {
            ChatTab::All => self.seen.all = message_len,
            ChatTab::Whisper => self.seen.whisper = message_len,
            ChatTab::Party => self.seen.party = message_len,
            ChatTab::Guild => self.seen.guild = message_len,
            ChatTab::System => self.seen.system = message_len,
            ChatTab::Partner(name) => {
                if let Some(partner) = self.partners.iter_mut().find(|partner| names_equal(&partner.name, name)) {
                    partner.seen = message_len;
                }
            }
        }
    }

    fn touch_partner(&mut self, name: &str) {
        let stamp = self.next_stamp();
        if let Some(partner) = self.partners.iter_mut().find(|partner| names_equal(&partner.name, name)) {
            partner.stamp = stamp;
        }
    }

    fn oldest_inactive(&self) -> Option<usize> {
        let active = self.active.partner_name();
        self.partners
            .iter()
            .enumerate()
            .filter(|(_, partner)| active.is_none_or(|name| !names_equal(name, &partner.name)))
            .min_by_key(|(_, partner)| partner.stamp)
            .map(|(index, _)| index)
    }

    fn next_stamp(&mut self) -> u64 {
        self.partner_clock = self.partner_clock.saturating_add(1);
        self.partner_clock
    }
}

pub fn same_tab(left: &ChatTab, right: &ChatTab) -> bool {
    match (left, right) {
        (ChatTab::Partner(left), ChatTab::Partner(right)) => names_equal(left, right),
        (left, right) => left == right,
    }
}

pub fn names_equal(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

/// `None` when the count is zero. Values above 99 render as `99+`.
pub fn format_unread_badge(count: u32) -> Option<String> {
    match count {
        0 => None,
        1..=UNREAD_BADGE_CAP => Some(count.to_string()),
        _ => Some(format!("{UNREAD_BADGE_CAP}+")),
    }
}

/// The player's own public echo (`Name : text`) and the `[To name]` whisper
/// echo. Whisper failures are not outgoing lines.
pub fn is_own_outgoing_line(message: &ChatMessage, player_name: &str) -> bool {
    if message.channel == ChatChannel::Whisper
        && let Some(rest) = message.text.trim_start().strip_prefix("[To ")
        && rest.contains(']')
    {
        return true;
    }
    let player_name = player_name.trim();
    if player_name.is_empty() {
        return false;
    }
    matches!(message.text.strip_prefix(player_name), Some(rest) if rest.starts_with(" : "))
}

fn whisper_for_partner(message: &ChatMessage, name: &str) -> bool {
    whisper_partner_name(message).is_some_and(|partner| names_equal(partner, name))
}

pub fn whisper_partner_name(message: &ChatMessage) -> Option<&str> {
    if let Some(name) = message.partner.as_deref() {
        let name = name.trim();
        if !name.is_empty() {
            return Some(name);
        }
    }
    parse_whisper_partner(&message.text)
}

fn parse_whisper_partner(text: &str) -> Option<&str> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("[From ").or_else(|| text.strip_prefix("[To ")) {
        let name = rest.split_once(']')?.0.trim();
        if !name.is_empty() {
            return Some(name);
        }
    }
    const SUFFIXES: [&str; 3] = [" is ignoring you.", " is ignoring everyone.", " is offline."];
    for suffix in SUFFIXES {
        if let Some(name) = text.strip_suffix(suffix) {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutgoingChat {
    Public(String),
    Whisper {
        target: String,
        text: String,
    },
    /// Guild channel. The caller formats `{name} : {text}` for `0x017E`.
    Guild(String),
}

/// Scroll position of the chat message pane.
///
/// `offset` is the distance from the top of the stacked lines. `pinned` follows
/// roBrowser: new lines jump to the bottom until the player scrolls up, and
/// scrolling back to the bottom starts following again.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListScroll {
    pub offset: f32,
    pub pinned: bool,
}

impl Default for ListScroll {
    fn default() -> Self {
        Self { offset: 0.0, pinned: true }
    }
}

pub fn list_maximum(content_height: f32, viewport: f32) -> f32 {
    (content_height - viewport).max(0.0)
}

pub fn stacked_content_height(heights: &[f32], spacing: f32) -> f32 {
    if heights.is_empty() {
        return 0.0;
    }
    let gaps = spacing * (heights.len() - 1) as f32;
    heights.iter().sum::<f32>() + gaps
}

/// Snap to the newest lines while pinned, and clamp after the pane resizes.
pub fn sync_list_scroll(mut scroll: ListScroll, content_height: f32, viewport: f32) -> ListScroll {
    let maximum = list_maximum(content_height, viewport);
    if scroll.pinned || maximum <= LIST_FOLLOW_SLOP {
        return ListScroll {
            offset: maximum,
            pinned: true,
        };
    }
    scroll.offset = scroll.offset.clamp(0.0, maximum);
    if maximum - scroll.offset <= LIST_FOLLOW_SLOP {
        scroll.offset = maximum;
        scroll.pinned = true;
    }
    scroll
}

/// `delta` uses the interface wheel sign: positive moves toward older lines.
pub fn scroll_list(scroll: ListScroll, delta: f32, content_height: f32, viewport: f32) -> ListScroll {
    let maximum = list_maximum(content_height, viewport);
    if maximum <= LIST_FOLLOW_SLOP {
        return ListScroll { offset: 0.0, pinned: true };
    }
    if delta > 0.0 && scroll.offset <= 0.0 {
        return ListScroll {
            offset: 0.0,
            pinned: false,
        };
    }
    if delta < 0.0 && scroll.offset >= maximum - LIST_FOLLOW_SLOP {
        return ListScroll {
            offset: maximum,
            pinned: true,
        };
    }
    let offset = (scroll.offset - delta).clamp(0.0, maximum);
    let pinned = delta <= 0.0 && maximum - offset <= LIST_FOLLOW_SLOP;
    ListScroll {
        offset: if pinned { maximum } else { offset },
        pinned,
    }
}

/// Indexes whose line box overlaps the viewport. `offset` is the top of the
/// pane.
#[cfg(test)]
fn indexes_in_view(heights: &[f32], spacing: f32, offset: f32, viewport: f32) -> Vec<usize> {
    if viewport <= 0.0 {
        return Vec::new();
    }
    let mut start = 0.0;
    let mut visible = Vec::new();
    for (index, height) in heights.iter().copied().enumerate() {
        if index > 0 {
            start += spacing;
        }
        let end = start + height;
        if end > offset && start < offset + viewport {
            visible.push(index);
        }
        start = end;
    }
    visible
}

/// The whisper-to box routes plain chat only while the player is in whisper
/// mode. `/w` fills the box but stamps it disarmed, so the next plain line
/// stays public until the player edits that field (roBrowser treats the
/// username box as the mode, and does not arm it from a one-shot command).
pub fn whisper_target_armed(target: &str, disarm_stamp: &str) -> bool {
    let target = target.trim();
    !target.is_empty() && target != disarm_stamp.trim()
}

/// Remember `command_target` in the box without arming sticky whispers.
pub fn disarm_sticky_whisper(target_field: &mut String, disarm_stamp: &mut String, command_target: &str) {
    let command_target = command_target.trim();
    if command_target.is_empty() {
        return;
    }
    target_field.clear();
    target_field.push_str(command_target);
    disarm_stamp.clone_from(target_field);
}

pub fn note_whisper_field_edit(disarm_stamp: &mut String) {
    disarm_stamp.clear();
}

/// `/w name text`, `/whisper name text`, `name : text`, or an armed whisper
/// box.
///
/// `/w` and `/whisper`, then `/g` and `/guild`, then `name : text`, then an
/// armed whisper box. `/w` fills the box but does not arm it. A guild chat tab
/// sends guild chat when the text is not a command and whisper mode is off.
/// `@` commands stay public.
///
/// `whisper_armed` is the roBrowser username-field mode: the box is filled
/// because the player typed or picked a name.
pub fn parse_outgoing_chat(raw: &str, whisper_target: &str, whisper_armed: bool, send_to_guild: bool) -> OutgoingChat {
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
    if whisper_armed && !target.is_empty() && !trimmed.starts_with('/') && !trimmed.starts_with('@') {
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
    if text.is_empty() {
        None
    } else {
        Some(OutgoingChat::Guild(text.to_string()))
    }
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

/// 0x008E carries both server notices and the echo of the player's own
/// public line (`"<name> : text"`). roBrowser shows that echo as public chat,
/// so it is filed under Public, not System, and follows the Pub filter.
pub fn own_chat_echo_color(text: &str, player_name: &str, color: MessageColor) -> MessageColor {
    if !matches!(color, MessageColor::Server) || player_name.is_empty() {
        return color;
    }
    match text.strip_prefix(player_name) {
        Some(rest) if rest.starts_with(" : ") => MessageColor::Public,
        _ => color,
    }
}

pub fn record_whisper(messages: &mut Vec<ChatMessage>, recent: &mut Vec<String>, filters: &mut ChatFilterState, sender: &str, text: &str) {
    let sender = sender.trim();
    let text = strip_whisper_code(text).trim();
    if sender.is_empty() || text.is_empty() {
        return;
    }
    filters.ensure_partner(sender, messages.len());
    remember_whisper_target(recent, sender);
    messages.push(ChatMessage::with_partner(
        format!("[From {sender}] : {text}"),
        MessageColor::Whisper,
        ChatChannel::Whisper,
        Some(sender.to_owned()),
    ));
}

pub fn record_whisper_ack(
    messages: &mut Vec<ChatMessage>,
    recent: &mut Vec<String>,
    filters: &mut ChatFilterState,
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
        filters.ensure_partner(&name, messages.len());
        remember_whisper_target(recent, &name);
        messages.push(ChatMessage::with_partner(
            format!("[To {name}] : {text}"),
            MessageColor::Whisper,
            ChatChannel::Whisper,
            Some(name),
        ));
        return;
    }
    let who = if name.is_empty() { "Player".to_string() } else { name };
    if who != "Player" {
        filters.ensure_partner(&who, messages.len());
    }
    messages.push(ChatMessage::with_partner(
        whisper_failure_line(&who, result),
        MessageColor::Error,
        ChatChannel::Whisper,
        Some(who).filter(|name| name != "Player"),
    ));
}

#[cfg(test)]
mod tests {

    #[test]
    fn window_title_names_the_active_tab() {
        assert_eq!(ChatTab::All.window_title(), "Chat - All");
        assert_eq!(ChatTab::Party.window_title(), "Chat - Party");
    }

    #[test]
    fn own_public_echo_is_public_chat() {
        use super::own_chat_echo_color;
        assert!(matches!(
            own_chat_echo_color("kgap04 : hello", "kgap04", MessageColor::Server),
            MessageColor::Public
        ));
        assert!(matches!(
            own_chat_echo_color("kgap04x : hello", "kgap04", MessageColor::Server),
            MessageColor::Server
        ));
        assert!(matches!(
            own_chat_echo_color("Party created.", "kgap04", MessageColor::Server),
            MessageColor::Server
        ));
        assert!(matches!(
            own_chat_echo_color("kgap04 : hello", "", MessageColor::Server),
            MessageColor::Server
        ));
        assert!(matches!(
            own_chat_echo_color("kgap04 : hi", "kgap04", MessageColor::Party),
            MessageColor::Party
        ));
    }

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
            parse_outgoing_chat("hello", "", false, false),
            OutgoingChat::Public(text) if text == "hello"
        ));
        assert!(matches!(
            parse_outgoing_chat("/w Bob hi there", "", false, false),
            OutgoingChat::Whisper { target, text } if target == "Bob" && text == "hi there"
        ));
        assert!(matches!(
            parse_outgoing_chat("/whisper Cara ping", "", false, false),
            OutgoingChat::Whisper { target, text } if target == "Cara" && text == "ping"
        ));
        assert!(matches!(
            parse_outgoing_chat("/W Bob hi", "", false, false),
            OutgoingChat::Whisper { target, text } if target == "Bob" && text == "hi"
        ));
        assert!(matches!(
            parse_outgoing_chat("Dana : secret", "", false, false),
            OutgoingChat::Whisper { target, text } if target == "Dana" && text == "secret"
        ));
        assert!(matches!(
            parse_outgoing_chat("hello", "Eve", true, true),
            OutgoingChat::Whisper { target, text } if target == "Eve" && text == "hello"
        ));
        assert!(matches!(
            parse_outgoing_chat("@broadcast hi", "Eve", true, true),
            OutgoingChat::Public(_)
        ));
        assert!(matches!(
            parse_outgoing_chat("@guild", "", false, true),
            OutgoingChat::Public(_)
        ));
        assert!(matches!(
            parse_outgoing_chat("/g rally at the castle", "", false, false),
            OutgoingChat::Guild(text) if text == "rally at the castle"
        ));
        assert!(matches!(
            parse_outgoing_chat("/guild on my way", "", false, false),
            OutgoingChat::Guild(text) if text == "on my way"
        ));
        assert!(matches!(
            parse_outgoing_chat("hello guild", "", false, true),
            OutgoingChat::Guild(text) if text == "hello guild"
        ));
        assert!(matches!(
            parse_outgoing_chat("hello guild", "Eve", false, true),
            OutgoingChat::Guild(text) if text == "hello guild"
        ));
        assert!(matches!(
            parse_outgoing_chat("/where", "", false, true),
            OutgoingChat::Public(_)
        ));
        assert!(matches!(
            parse_outgoing_chat("/where", "Eve", true, false),
            OutgoingChat::Public(_)
        ));
    }

    #[test]
    fn whisper_command_does_not_hijack_the_next_plain_line() {
        let mut field = String::new();
        let mut stamp = String::new();
        let sent = parse_outgoing_chat("/w kgap01 hello from B", &field, whisper_target_armed(&field, &stamp), false);
        assert!(matches!(
            sent,
            OutgoingChat::Whisper { target, text } if target == "kgap01" && text == "hello from B"
        ));

        disarm_sticky_whisper(&mut field, &mut stamp, "kgap01");
        assert_eq!(field, "kgap01");
        assert!(!whisper_target_armed(&field, &stamp));
        assert!(matches!(
            parse_outgoing_chat("public bubble from B", &field, whisper_target_armed(&field, &stamp), false),
            OutgoingChat::Public(text) if text == "public bubble from B"
        ));

        note_whisper_field_edit(&mut stamp);
        assert!(whisper_target_armed(&field, &stamp));
        assert!(matches!(
            parse_outgoing_chat("secret", &field, whisper_target_armed(&field, &stamp), false),
            OutgoingChat::Whisper { target, text } if target == "kgap01" && text == "secret"
        ));
    }

    #[test]
    fn filled_box_without_whisper_mode_stays_public() {
        assert!(!whisper_target_armed("kgap01", "kgap01"));
        assert!(whisper_target_armed("kgap01", ""));
        assert!(whisper_target_armed("Cara", "kgap01"));
        assert!(!whisper_target_armed("  ", ""));
        assert!(matches!(
            parse_outgoing_chat("hello", "kgap01", false, false),
            OutgoingChat::Public(_)
        ));
    }

    #[test]
    fn pinned_list_shows_newest_lines_until_the_player_scrolls_up() {
        let heights = vec![16.0, 16.0, 16.0];
        let content = stacked_content_height(&heights, CHAT_LINE_SPACING);
        let viewport = 16.0;
        let pinned = sync_list_scroll(ListScroll::default(), content, viewport);
        assert!(pinned.pinned);
        assert_eq!(indexes_in_view(&heights, CHAT_LINE_SPACING, pinned.offset, viewport), vec![2]);

        let wider = sync_list_scroll(ListScroll::default(), content, 40.0);
        let shown = indexes_in_view(&heights, CHAT_LINE_SPACING, wider.offset, 40.0);
        assert_eq!(shown, vec![1, 2]);
        assert!(!shown.contains(&0));

        let reading_history = scroll_list(pinned, 30.0, content, viewport);
        assert!(!reading_history.pinned);
        assert!(reading_history.offset < pinned.offset);

        let grew = stacked_content_height(&[16.0, 16.0, 16.0, 16.0], CHAT_LINE_SPACING);
        let stayed = sync_list_scroll(reading_history, grew, viewport);
        assert!(!stayed.pinned);
        assert!((stayed.offset - reading_history.offset).abs() < 0.01);

        let back = scroll_list(stayed, -10_000.0, grew, viewport);
        assert!(back.pinned);
        let followed = sync_list_scroll(back, grew + 21.0, viewport);
        assert!(followed.pinned);
        assert!(followed.offset > back.offset);
        let newest = indexes_in_view(&[16.0; 5], CHAT_LINE_SPACING, followed.offset, viewport);
        assert_eq!(newest, vec![4]);
    }

    #[test]
    fn all_tab_pinned_view_keeps_the_welcome_line_above_new_lines() {
        let filters = ChatFilterState::default();
        let messages = [
            ChatMessage::new("Welcome to Korangar".to_string(), MessageColor::Server),
            ChatMessage::new("kgap02 : hello bubble from B".to_string(), MessageColor::Public),
            ChatMessage::with_channel("kgap01 is offline.".to_string(), MessageColor::Error, ChatChannel::Whisper),
        ];
        let shown: Vec<_> = messages.iter().filter(|message| filters.allows_message(message)).collect();
        assert_eq!(shown.len(), 3);
        let heights = vec![16.0; shown.len()];
        let content = stacked_content_height(&heights, CHAT_LINE_SPACING);
        let scroll = sync_list_scroll(ListScroll::default(), content, 18.0);
        let visible = indexes_in_view(&heights, CHAT_LINE_SPACING, scroll.offset, 18.0);
        assert_eq!(visible, vec![2]);
        assert_eq!(shown[2].text, "kgap01 is offline.");

        let mut whisper_tab = ChatFilterState::default();
        whisper_tab.select(ChatTab::Whisper);
        let whisper_lines: Vec<_> = messages.iter().filter(|message| whisper_tab.allows_message(message)).collect();
        assert_eq!(whisper_lines.len(), 1);
        assert_eq!(whisper_lines[0].text, "kgap01 is offline.");
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
        let mut filters = ChatFilterState::default();
        record_whisper(&mut messages, &mut recent, &mut filters, "Bob", "|00hello");
        assert_eq!(messages[0].text, "[From Bob] : hello");
        assert_eq!(messages[0].channel, ChatChannel::Whisper);
        assert_eq!(messages[0].partner.as_deref(), Some("Bob"));
        assert_eq!(recent, vec!["Bob".to_string()]);
        assert_eq!(filters.partner_names(), vec!["Bob".to_string()]);

        let mut pending_name = "Cara".to_string();
        let mut pending_text = "yo".to_string();
        record_whisper_ack(
            &mut messages,
            &mut recent,
            &mut filters,
            &mut pending_name,
            &mut pending_text,
            0,
        );
        assert!(pending_name.is_empty());
        assert_eq!(messages[1].text, "[To Cara] : yo");
        assert_eq!(messages[1].channel, ChatChannel::Whisper);
        assert!(is_own_outgoing_line(&messages[1], "anyone"));
        assert_eq!(recent[0], "Cara");

        let mut pending_name = "Bob".to_string();
        let mut pending_text = "again".to_string();
        record_whisper_ack(
            &mut messages,
            &mut recent,
            &mut filters,
            &mut pending_name,
            &mut pending_text,
            2,
        );
        assert_eq!(messages[2].channel, ChatChannel::Whisper);
        assert_eq!(messages[2].text, "Bob is ignoring you.");
        assert!(matches!(messages[2].color, MessageColor::Error));
        assert_eq!(messages[2].partner.as_deref(), Some("Bob"));
        filters.select_at(ChatTab::Partner("Bob".to_owned()), messages.len());
        assert!(filters.allows_message(&messages[2]));
        assert!(!is_own_outgoing_line(&messages[2], "Bob"));
    }

    #[test]
    fn unread_counts_skip_the_active_tab_and_own_lines() {
        let mut filters = ChatFilterState::default();
        let mut messages = Vec::new();
        messages.push(ChatMessage::new("Bob : hi".to_owned(), MessageColor::Public));
        messages.push(ChatMessage::new("rally".to_owned(), MessageColor::Party));
        messages.push(ChatMessage::new("notice".to_owned(), MessageColor::Server));
        assert_eq!(filters.unread_count(&ChatTab::All, &messages, "kgap04"), 0);
        assert_eq!(filters.unread_count(&ChatTab::Party, &messages, "kgap04"), 1);
        assert_eq!(filters.unread_count(&ChatTab::System, &messages, "kgap04"), 1);
        assert_eq!(filters.unread_count(&ChatTab::Whisper, &messages, "kgap04"), 0);
        assert_eq!(format_unread_badge(1).as_deref(), Some("1"));
        assert_eq!(format_unread_badge(99).as_deref(), Some("99"));
        assert_eq!(format_unread_badge(100).as_deref(), Some("99+"));
        assert_eq!(format_unread_badge(0), None);

        filters.select_at(ChatTab::Party, messages.len());
        assert_eq!(filters.unread_count(&ChatTab::Party, &messages, "kgap04"), 0);
        messages.push(ChatMessage::new("kgap04 : mine".to_owned(), MessageColor::Public));
        messages.push(ChatMessage::new("Cara : yo".to_owned(), MessageColor::Public));
        assert_eq!(filters.unread_count(&ChatTab::All, &messages, "kgap04"), 1);
        assert!(is_own_outgoing_line(messages.iter().rev().nth(1).unwrap(), "kgap04"));

        filters.select_at(ChatTab::All, messages.len());
        assert_eq!(filters.unread_count(&ChatTab::All, &messages, "kgap04"), 0);
        messages.push(ChatMessage::new("again".to_owned(), MessageColor::Party));
        assert_eq!(filters.unread_count(&ChatTab::Party, &messages, "kgap04"), 1);
    }

    #[test]
    fn partner_tabs_track_whispers_and_recycle_the_oldest_inactive() {
        let mut filters = ChatFilterState::default();
        let mut messages = Vec::new();
        let mut recent = Vec::new();
        record_whisper(&mut messages, &mut recent, &mut filters, "Cara", "secret");
        let cara = ChatTab::Partner("Cara".to_owned());
        assert_eq!(filters.unread_count(&cara, &messages, "kgap04"), 1);
        assert_eq!(filters.unread_count(&ChatTab::Whisper, &messages, "kgap04"), 1);
        assert_eq!(filters.unread_count(&ChatTab::All, &messages, "kgap04"), 0);

        filters.select_at(cara.clone(), messages.len());
        assert_eq!(filters.unread_count(&cara, &messages, "kgap04"), 0);
        assert!(filters.allows_message(&messages[0]));
        filters.select_at(ChatTab::All, messages.len());
        messages.push(ChatMessage::with_partner(
            "[To Cara] : hi".to_owned(),
            MessageColor::Whisper,
            ChatChannel::Whisper,
            Some("Cara".to_owned()),
        ));
        assert_eq!(filters.unread_count(&cara, &messages, "kgap04"), 0);
        record_whisper(&mut messages, &mut recent, &mut filters, "cara", "again");
        assert_eq!(filters.partner_names(), vec!["Cara".to_string()]);
        assert_eq!(filters.unread_count(&cara, &messages, "kgap04"), 1);

        let mut capped = ChatFilterState::default();
        for name in ["a", "b", "c", "d", "e", "f", "g", "h"] {
            capped.ensure_partner(name, 0);
        }
        capped.select_at(ChatTab::Partner("a".to_owned()), 0);
        capped.ensure_partner("i", 0);
        let names = capped.partner_names();
        assert_eq!(names.len(), MAX_PARTNER_TABS);
        assert!(names.iter().any(|name| name == "a"));
        assert!(names.iter().any(|name| name == "i"));
        assert!(!names.iter().any(|name| name == "b"));
        assert!(names.iter().any(|name| name == "h"));
    }
}
