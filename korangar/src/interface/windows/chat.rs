use std::cell::RefCell;

use korangar_interface::application::{Size, TextLayouter};
use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut, Persistent, PersistentExt};
use korangar_interface::element::{Element, StateElement};
use korangar_interface::event::{ClickHandler, Event, EventQueue, InputHandler, ScrollHandler};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{ButtonThemePathExt, HorizontalAlignment, TextThemePathExt, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::MessageColor;
use rust_state::{Path, RustState, State};

use super::WindowClass;
use crate::graphics::Color;
use crate::input::InputEvent;
use crate::interface::components::chat_item_link::{self, LaidChatLine};
use crate::loaders::OverflowBehavior;
use crate::state::chat::{
    CHAT_LINE_SPACING, ChannelMaskPathExt, ChatChannel, ChatFilterState, ChatFilterStatePathExt, ChatTab, ChatTabSlot, FIXED_CHAT_TABS,
    ListScroll, MAX_PARTNER_TABS, format_unread_badge, scroll_list, stacked_content_height, sync_list_scroll,
};
use crate::state::item_link::{self, CHAT_DRAFT_LIMIT};
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::{ChatThemePathExt, InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ChatMessage, ClientState, ClientStatePathExt, client_state, client_theme};

/// Left inset the old single `add_text` got from `offset: 5.0` plus `border:
/// 3.0`.
const CHAT_TEXT_INSET: f32 = 8.0;
const MAXIMUM_WHISPER_NAME_LENGTH: usize = 23;

fn ensure_readable(color: Color, panel: Color, text: Color) -> Color {
    fn luminance(color: Color) -> f32 {
        0.2126 * color.red + 0.7152 * color.green + 0.0722 * color.blue
    }
    let contrast = {
        let (hi, lo) = if luminance(color) > luminance(panel) {
            (luminance(color), luminance(panel))
        } else {
            (luminance(panel), luminance(color))
        };
        (hi + 0.05) / (lo + 0.05)
    };
    if contrast >= 4.5 {
        color
    } else {
        // Lift toward the theme text color so inherited RGB (old default blues
        // included) stays readable on every panel.
        Color::rgb(
            color.red * 0.35 + text.red * 0.65,
            color.green * 0.35 + text.green * 0.65,
            color.blue * 0.35 + text.blue * 0.65,
        )
    }
}

fn themed_message_color(state: &State<ClientState>, color: MessageColor) -> Color {
    let raw = state.get(&client_theme().chat()).message_color(color);
    ensure_readable(
        raw,
        *state.get(&client_theme().tokens().panel()),
        *state.get(&client_theme().tokens().text()),
    )
}

/// ZST for getting the focus id of the chat text box. This is only needed to
/// focus the chat when pressing enter.
pub struct ChatTextBox;

/// Append an item link at the end of the chat draft. The text box has no caret.
/// Chat draft. The path trait for `current_text` is private to this module.
pub(crate) fn current_chat_draft(state: &State<ClientState>) -> String {
    state.get(&client_state().chat_window().current_text()).clone()
}

pub(crate) fn insert_chat_item_link(
    state: &mut State<ClientState>,
    item: &korangar_networking::InventoryItem<crate::world::ResourceMetadata>,
) {
    let next = item_link::append_link_text(state.get(&client_state().chat_window().current_text()), item);
    state.update_value_with(client_state().chat_window().current_text(), move |draft| {
        draft.clone_from(&next);
    });
}

/// Focus id for the whisper-target name box. Enter leaves the field.
struct WhisperTargetTextBox;

struct ChatLayoutInfo {
    area: Area,
    offset: f32,
    // TODO: Don't allocate this every frame.
    lines: Vec<LaidChatLine>,
}

struct MessageListScrollData {
    inner: RefCell<MessageListScrollInner>,
}

struct MessageListScrollInner {
    scroll: ListScroll,
    content_height: f32,
    viewport: f32,
}

impl Default for MessageListScrollData {
    fn default() -> Self {
        Self {
            inner: RefCell::new(MessageListScrollInner {
                scroll: ListScroll::default(),
                content_height: 0.0,
                viewport: 0.0,
            }),
        }
    }
}

impl ScrollHandler<ClientState> for MessageListScrollData {
    fn handle_scroll(&self, _: &State<ClientState>, _: &mut EventQueue<ClientState>, delta: f32) -> bool {
        let mut inner = self.inner.borrow_mut();
        let next = scroll_list(inner.scroll, delta, inner.content_height, inner.viewport);
        let changed = (next.offset - inner.scroll.offset).abs() > f32::EPSILON || next.pinned != inner.scroll.pinned;
        inner.scroll = next;
        changed
    }
}

struct ChatElement<A, B> {
    chat_messages_path: A,
    chat_window_state: B,
}

impl<A, B> ChatElement<A, B> {
    fn new(chat_messages_path: A, chat_window_state: B) -> Self {
        Self {
            chat_messages_path,
            chat_window_state,
        }
    }
}

impl<A, B> Persistent for ChatElement<A, B> {
    type Data = MessageListScrollData;
}

impl<A, B> Element<ClientState> for ChatElement<A, B>
where
    A: Path<ClientState, Vec<ChatMessage>>,
    B: Path<ClientState, ChatWindowState>,
{
    type LayoutInfo = ChatLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let chat_messages = visible_messages(state, &self.chat_messages_path, &self.chat_window_state);
            let font_size = *state.get(&client_theme().text().font_size());
            let highlight = *state.get(&client_theme().text().highlight_color());
            let gold = *state.get(&client_theme().tokens().gold());
            let available = resolver.push_available_area();
            let content_width = (available.width - CHAT_TEXT_INSET).max(0.0);
            let lines: Vec<LaidChatLine> = {
                let layouter = resolver.get_text_layouter();
                chat_messages
                    .iter()
                    .map(|chat_message| {
                        let color = themed_message_color(state, chat_message.color);
                        let measure = |text: &str| {
                            let (size, _) =
                                layouter.get_text_dimensions(text, color, highlight, font_size, 1_000_000.0, OverflowBehavior::Shrink);
                            (size.width(), size.height())
                        };
                        chat_item_link::lay_out_chat_line(
                            &chat_message.text,
                            content_width,
                            gold,
                            |item_id| item_link::identity_for(state, item_id),
                            measure,
                        )
                    })
                    .collect()
            };
            let message_heights: Vec<f32> = lines.iter().map(|line| line.height).collect();

            let content_height = stacked_content_height(&message_heights, CHAT_LINE_SPACING);
            let viewport = match available.height {
                Some(height) => height.max(0.0),
                None => content_height,
            };

            let persistent = self.get_persistent_data(&store, ());
            let offset = {
                let mut inner = persistent.inner.borrow_mut();
                inner.content_height = content_height;
                inner.viewport = viewport;
                inner.scroll = sync_list_scroll(inner.scroll, content_height, viewport);
                inner.scroll.offset
            };

            let area = Area {
                left: available.left,
                top: available.top,
                width: available.width,
                height: viewport,
            };
            resolver.commit_used_height(viewport);

            Self::LayoutInfo { area, offset, lines }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let persistent = self.get_persistent_data(&store, ());
        if layout_info.area.check().dont_mark().run(layout) {
            layout.register_scroll_handler(persistent);
        }

        let chat_messages = visible_messages(state, &self.chat_messages_path, &self.chat_window_state);
        let font_size = *state.get(&client_theme().text().font_size());
        let highlight = *state.get(&client_theme().text().highlight_color());
        let accent = *state.get(&client_theme().tokens().accent());
        let scroll_offset = layout_info.offset;

        layout.with_clip(layout_info.area, |layout| {
            let mut line_top = 0.0;
            chat_messages.iter().zip(layout_info.lines.iter()).for_each(|(chat_message, line)| {
                let plain = themed_message_color(state, chat_message.color);

                if line_top != 0.0 {
                    line_top += CHAT_LINE_SPACING;
                }

                chat_item_link::paint_chat_line(layout, chat_item_link::ChatPaint {
                    line,
                    left: layout_info.area.left + CHAT_TEXT_INSET,
                    top: layout_info.area.top + line_top - scroll_offset,
                    font_size,
                    plain,
                    accent,
                    highlight,
                });

                line_top += line.height;
            });
        });
    }
}

const TAB_SLOTS: usize = FIXED_CHAT_TABS + MAX_PARTNER_TABS;
const TAB_GAP: f32 = 4.0;
const TAB_PAD_X: f32 = 6.0;
const TAB_CLOSE_WIDTH: f32 = 14.0;
const TAB_MAX_WIDTH: f32 = 168.0;

struct SelectChatTab<W, M> {
    window: W,
    messages: M,
    slot: usize,
}

impl<W, M> ClickHandler<ClientState> for SelectChatTab<W, M>
where
    W: Path<ClientState, ChatWindowState> + Copy,
    M: Path<ClientState, Vec<ChatMessage>> + Copy,
{
    fn handle_click(&self, state: &State<ClientState>, _: &mut korangar_interface::event::EventQueue<ClientState>) {
        let len = state.get(&self.messages).len();
        let slot = self.slot;
        state.update_value_with(self.window, move |window| {
            if let Some(tab) = window.filters.tab_for_slot(slot) {
                window.select_tab(tab, len);
            }
        });
    }
}

struct ClosePartnerTab<W, M> {
    window: W,
    messages: M,
    slot: usize,
}

impl<W, M> ClickHandler<ClientState> for ClosePartnerTab<W, M>
where
    W: Path<ClientState, ChatWindowState> + Copy,
    M: Path<ClientState, Vec<ChatMessage>> + Copy,
{
    fn handle_click(&self, state: &State<ClientState>, _: &mut korangar_interface::event::EventQueue<ClientState>) {
        let len = state.get(&self.messages).len();
        let slot = self.slot;
        state.update_value_with(self.window, move |window| window.close_partner_tab(slot, len));
    }
}

struct PlacedTab {
    area: Area,
    select_area: Area,
    close_area: Option<Area>,
    label_area: Area,
    badge_area: Option<Area>,
    label: String,
    badge: String,
    selected: bool,
    slot: usize,
}

struct ChatTabStripLayout {
    tabs: Vec<PlacedTab>,
}

struct ChatTabStrip<W, M> {
    window: W,
    messages: M,
    selects: [SelectChatTab<W, M>; TAB_SLOTS],
    closes: [ClosePartnerTab<W, M>; MAX_PARTNER_TABS],
}

impl<W, M> ChatTabStrip<W, M>
where
    W: Copy,
    M: Copy,
{
    fn new(window: W, messages: M) -> Self {
        Self {
            window,
            messages,
            selects: std::array::from_fn(|slot| SelectChatTab { window, messages, slot }),
            closes: std::array::from_fn(|slot| ClosePartnerTab { window, messages, slot }),
        }
    }
}

impl<W, M> Element<ClientState> for ChatTabStrip<W, M>
where
    W: Path<ClientState, ChatWindowState> + Copy,
    M: Path<ClientState, Vec<ChatMessage>> + Copy,
{
    type LayoutInfo = ChatTabStripLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let player_name = state.get(&client_state().player_name()).clone();
            let slots = {
                let messages = state.get(&self.messages);
                let window = state.get(&self.window);
                window.filters.tab_slots(messages, &player_name)
            };
            let font_size = *state.get(&client_theme().button().font_size());
            let text_color = *state.get(&client_theme().button().foreground_color());
            let accent = *state.get(&client_theme().tokens().accent());
            let highlight = *state.get(&client_theme().text().highlight_color());
            let row_height = *state.get(&client_theme().button().height());
            let available = resolver.push_available_area();
            // The measure closure borrows the resolver. Drop it before committing height.
            let (placed, height) = {
                let text_width = |text: &str, color: Color| {
                    resolver
                        .get_text_dimensions(
                            text,
                            color,
                            highlight,
                            font_size,
                            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                            OverflowBehavior::Shrink,
                        )
                        .0
                        .width()
                };

                let mut placed = Vec::with_capacity(slots.len());
                let mut x = 0.0;
                let mut y = 0.0;
                for slot in slots {
                    let badge = format_unread_badge(slot.unread).unwrap_or_default();
                    let label_width = text_width(&slot.label, text_color);
                    let badge_width = if badge.is_empty() { 0.0 } else { text_width(&badge, accent) };
                    let close_width = if slot.closable { TAB_CLOSE_WIDTH } else { 0.0 };
                    let badge_gap = if badge.is_empty() { 0.0 } else { TAB_GAP };
                    let mut chip_width = TAB_PAD_X + label_width + badge_gap + badge_width + close_width + TAB_PAD_X;
                    let mut shown_label = label_width;
                    if chip_width > TAB_MAX_WIDTH {
                        shown_label = (shown_label - (chip_width - TAB_MAX_WIDTH)).max(12.0);
                        chip_width = TAB_PAD_X + shown_label + badge_gap + badge_width + close_width + TAB_PAD_X;
                    }
                    if x > 0.0 && x + chip_width > available.width {
                        x = 0.0;
                        y += row_height + TAB_GAP;
                    }
                    let area = Area {
                        left: available.left + x,
                        top: available.top + y,
                        width: chip_width,
                        height: row_height,
                    };
                    let close_area = slot.closable.then_some(Area {
                        left: area.left + area.width - TAB_CLOSE_WIDTH,
                        top: area.top,
                        width: TAB_CLOSE_WIDTH,
                        height: area.height,
                    });
                    let select_width = area.width - close_area.map(|close| close.width).unwrap_or(0.0);
                    let select_area = Area {
                        left: area.left,
                        top: area.top,
                        width: select_width,
                        height: area.height,
                    };
                    let label_area = Area {
                        left: area.left + TAB_PAD_X,
                        top: area.top,
                        width: shown_label,
                        height: area.height,
                    };
                    let badge_area = if badge.is_empty() {
                        None
                    } else {
                        Some(Area {
                            left: label_area.left + shown_label + TAB_GAP,
                            top: area.top,
                            width: badge_width,
                            height: area.height,
                        })
                    };
                    placed.push(PlacedTab {
                        area,
                        select_area,
                        close_area,
                        label_area,
                        badge_area,
                        label: slot.label,
                        badge,
                        selected: slot.selected,
                        slot: slot.slot,
                    });
                    x += chip_width + TAB_GAP;
                }
                let height = if placed.is_empty() { row_height } else { y + row_height };
                (placed, height)
            };
            resolver.commit_used_height(height);
            Self::LayoutInfo { tabs: placed }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let accent = *state.get(&client_theme().tokens().accent());
        let highlight = *state.get(&client_theme().text().highlight_color());
        let font_size = *state.get(&client_theme().button().font_size());
        let corner = *state.get(&client_theme().button().corner_diameter());
        let shadow = *state.get(&client_theme().button().shadow_color());
        let shadow_padding = *state.get(&client_theme().button().shadow_padding());
        let idle_background = *state.get(&client_theme().button().background_color());
        let hot_background = *state.get(&client_theme().button().hovered_background_color());
        let idle_foreground = *state.get(&client_theme().button().foreground_color());
        let hot_foreground = *state.get(&client_theme().button().hovered_foreground_color());

        for tab in &layout_info.tabs {
            let select_hover = tab.select_area.check().run(layout);
            let close_hover = tab.close_area.is_some_and(|area| area.check().run(layout));
            let hot = tab.selected || select_hover || close_hover;
            layout.add_rectangle(
                tab.area,
                corner,
                if hot { hot_background } else { idle_background },
                shadow,
                shadow_padding,
            );
            if select_hover && let Some(handler) = self.selects.get(tab.slot) {
                layout.register_click_handler(MouseButton::Left, handler);
            }
            if close_hover && let Some(handler) = tab.close_area.and(self.closes.get(tab.slot - FIXED_CHAT_TABS)) {
                layout.register_click_handler(MouseButton::Left, handler);
            }
            layout.add_text(
                tab.label_area,
                &tab.label,
                font_size,
                if hot { hot_foreground } else { idle_foreground },
                highlight,
                HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if let Some(badge_area) = tab.badge_area {
                layout.add_text(
                    badge_area,
                    &tab.badge,
                    font_size,
                    accent,
                    highlight,
                    HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                    VerticalAlignment::Center { offset: 0.0 },
                    OverflowBehavior::Shrink,
                );
            }
            if let Some(close_area) = tab.close_area {
                layout.add_text(
                    close_area,
                    "x",
                    font_size,
                    if close_hover { accent } else { idle_foreground },
                    highlight,
                    HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                    VerticalAlignment::Center { offset: 0.0 },
                    OverflowBehavior::Shrink,
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ToggleChannel<F> {
    filters_path: F,
    channel: ChatChannel,
}

impl<F> ClickHandler<ClientState> for ToggleChannel<F>
where
    F: Path<ClientState, ChatFilterState> + Copy,
{
    fn handle_click(&self, state: &State<ClientState>, _: &mut korangar_interface::event::EventQueue<ClientState>) {
        let filters_path = self.filters_path;
        let channel = self.channel;
        state.update_value_with(filters_path, move |filters| filters.toggle(channel));
    }
}

struct WhisperTargetHandler<T, S> {
    text_path: T,
    stamp_path: S,
}

impl<T, S> InputHandler<ClientState> for WhisperTargetHandler<T, S>
where
    T: Path<ClientState, String> + Copy,
    S: Path<ClientState, String> + Copy,
{
    fn handle_character(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>, character: char) {
        if character == '\t' || character == '\r' || character == '\n' {
            state.update_value_with(self.stamp_path, crate::state::chat::note_whisper_field_edit);
            queue.queue(Event::Unfocus);
            return;
        }
        if character == '\u{1b}' {
            queue.queue(Event::Unfocus);
            return;
        }
        if character == '\u{8}' {
            state.update_value_with(self.text_path, |current_text| {
                if !current_text.is_empty() {
                    current_text.pop();
                }
            });
            state.update_value_with(self.stamp_path, crate::state::chat::note_whisper_field_edit);
            return;
        }
        if !character.is_control() {
            state.update_value_with(self.text_path, move |current_text| {
                if current_text.len().saturating_add(character.len_utf8()) <= MAXIMUM_WHISPER_NAME_LENGTH {
                    current_text.push(character);
                }
            });
            state.update_value_with(self.stamp_path, crate::state::chat::note_whisper_field_edit);
        }
    }
}

fn visible_messages<'a, A, B>(state: &'a State<ClientState>, messages_path: &'a A, window_path: &B) -> Vec<&'a ChatMessage>
where
    A: Path<ClientState, Vec<ChatMessage>>,
    B: Path<ClientState, ChatWindowState>,
{
    let filters_path = window_path.filters();
    let filters = state.get(&filters_path);
    state
        .get(messages_path)
        .iter()
        .filter(|message| filters.allows_message(message))
        .collect()
}

/// Internal state of the chat window.
#[derive(RustState, StateElement)]
pub struct ChatWindowState {
    current_text: String,
    /// Name shown in the whisper target box.
    whisper_target: String,
    /// When this equals [`Self::whisper_target`], plain chat stays public.
    /// `/w` sets both so the next line is not a whisper. Editing the box clears
    /// the stamp and arms whisper mode.
    whisper_disarm_stamp: String,
    /// Newest whisper names first. The dropdown writes
    /// [`Self::whisper_target`].
    recent_whisper_targets: Vec<String>,
    pending_whisper_name: String,
    pending_whisper_text: String,
    active_tab_label: String,
    filters: ChatFilterState,
}

impl Default for ChatWindowState {
    fn default() -> Self {
        Self {
            current_text: String::new(),
            whisper_target: String::new(),
            whisper_disarm_stamp: String::new(),
            recent_whisper_targets: Vec::new(),
            pending_whisper_name: String::new(),
            pending_whisper_text: String::new(),
            active_tab_label: ChatTab::All.window_title(),
            filters: ChatFilterState::default(),
        }
    }
}

impl ChatWindowState {
    pub(crate) fn whisper_target_text(&self) -> &str {
        &self.whisper_target
    }

    /// The Guild tab sends `0x017E` unless an armed whisper target or command
    /// wins.
    pub(crate) fn sends_guild_chat(&self) -> bool {
        self.filters.active == ChatTab::Guild
    }

    pub(crate) fn whisper_target_armed(&self) -> bool {
        crate::state::chat::whisper_target_armed(&self.whisper_target, &self.whisper_disarm_stamp)
    }

    pub(crate) fn active_tab(&self) -> &ChatTab {
        &self.filters.active
    }

    /// Context menu Whisper: create or reuse the partner tab and show it.
    pub(crate) fn focus_partner(&mut self, name: String, message_len: usize) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.filters.ensure_partner(name, message_len);
        let canonical = self
            .filters
            .partners
            .iter()
            .find(|partner| crate::state::chat::names_equal(&partner.name, name))
            .map(|partner| partner.name.clone())
            .unwrap_or_else(|| name.to_owned());
        self.select_tab(ChatTab::Partner(canonical), message_len);
    }

    /// Partner tabs arm whisper mode. Leaving one disarms it without clearing
    /// the name box, matching `/w`.
    pub(crate) fn select_tab(&mut self, tab: ChatTab, message_len: usize) {
        let leaving_partner = self.filters.active.partner_name().is_some();
        self.filters.select_at(tab, message_len);
        if let Some(name) = self.filters.active.partner_name() {
            let name = name.to_owned();
            crate::state::chat::remember_whisper_target(&mut self.recent_whisper_targets, &name);
            self.whisper_target.clone_from(&name);
            self.whisper_disarm_stamp.clear();
        } else if leaving_partner {
            let target = self.whisper_target.clone();
            crate::state::chat::disarm_sticky_whisper(&mut self.whisper_target, &mut self.whisper_disarm_stamp, &target);
        }
        self.active_tab_label = self.filters.active.window_title();
    }

    /// Drop a partner tab. The chat window stays open. Closing the active
    /// partner returns to the generic Whisper tab and disarms whisper mode.
    pub(crate) fn close_partner_tab(&mut self, index: usize, message_len: usize) {
        let Some(name) = self.filters.remove_partner(index) else {
            return;
        };
        let was_active = self
            .filters
            .active
            .partner_name()
            .is_some_and(|active| crate::state::chat::names_equal(active, &name));
        if was_active {
            self.select_tab(ChatTab::Whisper, message_len);
        }
    }

    pub(crate) fn begin_whisper(&mut self, target: String, text: String, message_len: usize) -> (String, String) {
        self.filters.ensure_partner(&target, message_len);
        let stay_armed = self
            .filters
            .active
            .partner_name()
            .is_some_and(|name| crate::state::chat::names_equal(name, &target));
        crate::state::chat::remember_whisper_target(&mut self.recent_whisper_targets, &target);
        if stay_armed {
            self.whisper_target.clone_from(&target);
            self.whisper_disarm_stamp.clear();
        } else {
            crate::state::chat::disarm_sticky_whisper(&mut self.whisper_target, &mut self.whisper_disarm_stamp, &target);
        }
        self.pending_whisper_name.clone_from(&target);
        self.pending_whisper_text.clone_from(&text);
        (target, text)
    }

    /// Same character rules as the chat text box. `Some` is the line Enter
    /// submits.
    pub(crate) fn apply_chat_character(&mut self, character: char) -> Option<String> {
        if character == '\t' || character == '\r' || character == '\n' {
            if self.current_text.is_empty() {
                return None;
            }
            return Some(std::mem::take(&mut self.current_text));
        }
        if character == '\u{8}' {
            self.current_text.pop();
            return None;
        }
        if !character.is_control() && self.current_text.len().saturating_add(character.len_utf8()) <= CHAT_DRAFT_LIMIT {
            self.current_text.push(character);
        }
        None
    }

    pub(crate) fn record_incoming_whisper(&mut self, messages: &mut Vec<ChatMessage>, sender: &str, text: &str) {
        crate::state::chat::record_whisper(messages, &mut self.recent_whisper_targets, &mut self.filters, sender, text);
    }

    pub(crate) fn record_whisper_ack(&mut self, messages: &mut Vec<ChatMessage>, result: u8) {
        crate::state::chat::record_whisper_ack(
            messages,
            &mut self.recent_whisper_targets,
            &mut self.filters,
            &mut self.pending_whisper_name,
            &mut self.pending_whisper_text,
            result,
        );
    }

    pub(crate) fn tab_slots(&self, messages: &[ChatMessage], player_name: &str) -> Vec<ChatTabSlot> {
        self.filters.tab_slots(messages, player_name)
    }

    /// Same selection the tab-strip click handler performs.
    pub(crate) fn select_slot(&mut self, slot: usize, message_len: usize) -> bool {
        let Some(tab) = self.filters.tab_for_slot(slot) else {
            return false;
        };
        self.select_tab(tab, message_len);
        true
    }

    pub(crate) fn close_named_partner(&mut self, name: &str, message_len: usize) -> bool {
        let Some(index) = self
            .filters
            .partners
            .iter()
            .position(|partner| crate::state::chat::names_equal(&partner.name, name))
        else {
            return false;
        };
        self.close_partner_tab(index, message_len);
        true
    }

    pub(crate) fn visible_lines(&self, messages: &[ChatMessage]) -> Vec<(String, MessageColor)> {
        messages
            .iter()
            .filter(|message| self.filters.allows_message(message))
            .map(|message| (message.text.clone(), message.color))
            .collect()
    }
}

pub struct ChatWindow<A, B> {
    chat_window_state: A,
    chat_messages_path: B,
}

impl<A, B> ChatWindow<A, B> {
    pub fn new(chat_window_state: A, chat_messages_path: B) -> Self {
        Self {
            chat_window_state,
            chat_messages_path,
        }
    }
}

impl<A, B> CustomWindow<ClientState> for ChatWindow<A, B>
where
    A: Path<ClientState, ChatWindowState> + Copy,
    B: Path<ClientState, Vec<ChatMessage>> + Copy,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Chat)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let window_state = self.chat_window_state;
        let current_text_path = window_state.current_text();
        let whisper_target_path = window_state.whisper_target();
        let recent_path = window_state.recent_whisper_targets();
        let filters_path = window_state.filters();
        let label_path = window_state.active_tab_label();
        let messages_path = self.chat_messages_path;

        let send_action = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            let text = state.get(&current_text_path);

            if !text.is_empty() {
                state.update_value_with(current_text_path, |current_text| current_text.clear());
                queue.queue(InputEvent::SendMessage { text: text.clone() });
                queue.queue(Event::Unfocus);
            }
        };
        let stamp_path = window_state.whisper_disarm_stamp();
        let whisper_name_input = WhisperTargetHandler {
            text_path: whisper_target_path,
            stamp_path,
        };

        let toggle_channel = |channel: ChatChannel| ToggleChannel { filters_path, channel };

        window! {
            // The title names the active tab, so it needs no row of its own.
            title: label_path,
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            background_color: client_theme().chat().window_color(),
            resizable: true,
            border: 3.0,
            gaps: 2.0,
            title_gap: 0.0,
            minimum_height: super::placement::CHAT_WINDOW_MIN_HEIGHT,
            maximum_height: 800.0,
            elements: (
                ChatTabStrip::new(window_state, messages_path),
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        state_button! {
                            text: "Pub",
                            state: filters_path.shown().public_chat(),
                            event: toggle_channel(ChatChannel::Public),
                        },
                        state_button! {
                            text: "Wis",
                            state: filters_path.shown().whisper(),
                            event: toggle_channel(ChatChannel::Whisper),
                        },
                        state_button! {
                            text: "Pty",
                            state: filters_path.shown().party(),
                            event: toggle_channel(ChatChannel::Party),
                        },
                        state_button! {
                            text: "Gld",
                            state: filters_path.shown().guild(),
                            event: toggle_channel(ChatChannel::Guild),
                        },
                        state_button! {
                            text: "Sys",
                            state: filters_path.shown().system(),
                            event: toggle_channel(ChatChannel::System),
                        },
                        state_button! {
                            text: "Ann",
                            state: filters_path.shown().announce(),
                            event: toggle_channel(ChatChannel::Announce),
                        },
                    ),
                },
                // roBrowser layout: name box and recents take the left half, the
                // message box the right half. One row instead of two leaves the
                // message list room for several lines under the Menu bar.
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        split! {
                            gaps: theme().window().gaps(),
                            children: (
                                text_box! {
                                    ghost_text: "Whisper to",
                                    state: whisper_target_path,
                                    input_handler: whisper_name_input,
                                    background_color: client_theme().chat().text_box_background_color(),
                                    focused_background_color: client_theme().text_box().focused_background_color(),
                                    focus_id: WhisperTargetTextBox,
                                },
                                drop_down! {
                                    options: recent_path,
                                    selected: whisper_target_path,
                                },
                            ),
                        },
                        text_box! {
                            ghost_text: client_state().localization().chat_text_box_message(),
                            state: current_text_path,
                            input_handler: DefaultHandler::<_, _, CHAT_DRAFT_LIMIT>::new(current_text_path, send_action),
                            background_color: client_theme().chat().text_box_background_color(),
                            focused_background_color: client_theme().text_box().focused_background_color(),
                            focus_id: ChatTextBox,
                        },
                    ),
                },
                ChatElement::new(messages_path, window_state),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::chat::{OutgoingChat, parse_outgoing_chat};

    #[test]
    fn partner_tab_arms_whisper_and_closing_it_disarms() {
        let mut window = ChatWindowState::default();
        window.focus_partner("Cara".to_owned(), 0);
        assert_eq!(window.active_tab().label(), "Cara");
        assert!(window.whisper_target_armed());
        assert_eq!(window.whisper_target_text(), "Cara");
        assert!(matches!(
            parse_outgoing_chat("hello", window.whisper_target_text(), window.whisper_target_armed(), window.sends_guild_chat()),
            OutgoingChat::Whisper { target, text } if target == "Cara" && text == "hello"
        ));

        let (target, text) = window.begin_whisper("Cara".to_owned(), "hello".to_owned(), 0);
        assert_eq!(target, "Cara");
        assert_eq!(text, "hello");
        assert!(window.whisper_target_armed(), "staying on the partner tab keeps whisper mode");

        window.select_tab(ChatTab::All, 0);
        assert!(!window.whisper_target_armed());
        assert_eq!(window.whisper_target_text(), "Cara");
        assert!(matches!(
            parse_outgoing_chat("public", window.whisper_target_text(), window.whisper_target_armed(), false),
            OutgoingChat::Public(text) if text == "public"
        ));

        window.select_tab(ChatTab::Partner("Cara".to_owned()), 0);
        assert!(window.whisper_target_armed());
        window.close_partner_tab(0, 0);
        assert_eq!(window.active_tab(), &ChatTab::Whisper);
        assert!(window.filters.partners.is_empty());
        assert!(!window.whisper_target_armed());
    }

    #[test]
    fn closing_an_inactive_partner_keeps_the_armed_tab() {
        let mut window = ChatWindowState::default();
        window.focus_partner("Cara".to_owned(), 0);
        window.filters.ensure_partner("Dana", 0);
        window.close_partner_tab(1, 0);
        assert_eq!(window.active_tab().label(), "Cara");
        assert!(window.whisper_target_armed());
        assert_eq!(window.filters.partner_names(), vec!["Cara".to_string()]);
    }

    #[test]
    fn typed_enter_submits_the_chat_box() {
        let mut window = ChatWindowState::default();
        assert!(window.apply_chat_character('h').is_none());
        assert!(window.apply_chat_character('i').is_none());
        assert_eq!(window.apply_chat_character('\r').as_deref(), Some("hi"));
        assert!(window.apply_chat_character('\r').is_none());
    }
}
