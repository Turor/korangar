use std::cell::RefCell;

use korangar_interface::application::Size;
use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut, Persistent, PersistentExt};
use korangar_interface::element::{Element, StateElement};
use korangar_interface::event::{ClickHandler, Event, EventQueue, InputHandler, ScrollHandler};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, TextThemePathExt, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::MessageColor;
use rust_state::{Path, RustState, State};

use super::WindowClass;
use crate::graphics::Color;
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::chat::{
    CHAT_LINE_SPACING, ChannelMaskPathExt, ChatChannel, ChatFilterState, ChatFilterStatePathExt, ChatTab, ListScroll, scroll_list,
    stacked_content_height, sync_list_scroll,
};
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::{ChatThemePathExt, InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ChatMessage, ClientState, ClientStatePathExt, client_state, client_theme};

const MAXIMUM_CHAT_MESSAGE_LENGTH: usize = 80;
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

/// Focus id for the whisper-target name box. Enter leaves the field.
struct WhisperTargetTextBox;

struct ChatLayoutInfo {
    area: Area,
    offset: f32,
    // TODO: Don't allocate this every frame.
    message_heights: Vec<f32>,
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

            let message_heights: Vec<f32> = chat_messages
                .iter()
                .map(|chat_message| {
                    let color = themed_message_color(state, chat_message.color);
                    let (size, _) = resolver.get_text_dimensions(
                        &chat_message.text,
                        color,
                        highlight,
                        font_size,
                        HorizontalAlignment::Left { offset: 5.0, border: 3.0 },
                        OverflowBehavior::LineBreak,
                    );
                    size.height()
                })
                .collect();

            let content_height = stacked_content_height(&message_heights, CHAT_LINE_SPACING);
            let available = resolver.push_available_area();
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

            Self::LayoutInfo {
                area,
                offset,
                message_heights,
            }
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
        let scroll_offset = layout_info.offset;

        layout.with_clip(layout_info.area, |layout| {
            let mut line_top = 0.0;
            chat_messages
                .iter()
                .zip(layout_info.message_heights.iter())
                .for_each(|(chat_message, message_height)| {
                    let color = themed_message_color(state, chat_message.color);

                    if line_top != 0.0 {
                        line_top += CHAT_LINE_SPACING;
                    }

                    let text_area = Area {
                        left: layout_info.area.left,
                        top: layout_info.area.top + line_top - scroll_offset,
                        width: layout_info.area.width,
                        height: *message_height,
                    };

                    layout.add_text(
                        text_area,
                        &chat_message.text,
                        font_size,
                        color,
                        highlight,
                        HorizontalAlignment::Left { offset: 5.0, border: 3.0 },
                        VerticalAlignment::Center { offset: 0.0 },
                        OverflowBehavior::LineBreak,
                    );

                    line_top += message_height;
                });
        });
    }
}

#[derive(Clone, Copy)]
struct SelectTab<F, L> {
    filters_path: F,
    label_path: L,
    tab: ChatTab,
}

impl<F, L> ClickHandler<ClientState> for SelectTab<F, L>
where
    F: Path<ClientState, ChatFilterState> + Copy,
    L: Path<ClientState, String> + Copy,
{
    fn handle_click(&self, state: &State<ClientState>, _: &mut korangar_interface::event::EventQueue<ClientState>) {
        let filters_path = self.filters_path;
        let label_path = self.label_path;
        let tab = self.tab;
        state.update_value_with(filters_path, move |filters| filters.select(tab));
        state.update_value(label_path, tab.label().to_string());
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
            active_tab_label: ChatTab::All.label().to_string(),
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

    pub(crate) fn begin_whisper(&mut self, target: String, text: String) -> (String, String) {
        crate::state::chat::remember_whisper_target(&mut self.recent_whisper_targets, &target);
        crate::state::chat::disarm_sticky_whisper(&mut self.whisper_target, &mut self.whisper_disarm_stamp, &target);
        self.pending_whisper_name.clone_from(&target);
        self.pending_whisper_text.clone_from(&text);
        (target, text)
    }

    pub(crate) fn recent_whisper_targets_mut(&mut self) -> &mut Vec<String> {
        &mut self.recent_whisper_targets
    }

    pub(crate) fn pending_whisper_mut(&mut self) -> (&mut Vec<String>, &mut String, &mut String) {
        (
            &mut self.recent_whisper_targets,
            &mut self.pending_whisper_name,
            &mut self.pending_whisper_text,
        )
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

        let select_tab = |tab: ChatTab| SelectTab {
            filters_path,
            label_path,
            tab,
        };
        let toggle_channel = |channel: ChatChannel| ToggleChannel { filters_path, channel };

        window! {
            title: client_state().localization().chat_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            background_color: client_theme().chat().window_color(),
            resizable: true,
            border: 3.0,
            gaps: 2.0,
            title_gap: 0.0,
            minimum_height: 96.0,
            maximum_height: 800.0,
            elements: (
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! { text: "All", event: select_tab(ChatTab::All) },
                        button! { text: "Whisper", event: select_tab(ChatTab::Whisper) },
                        button! { text: "Party", event: select_tab(ChatTab::Party) },
                        button! { text: "Guild", event: select_tab(ChatTab::Guild) },
                        button! { text: "System", event: select_tab(ChatTab::System) },
                    ),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text! { text: label_path, overflow_behavior: OverflowBehavior::Shrink },
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
                            input_handler: DefaultHandler::<_, _, MAXIMUM_CHAT_MESSAGE_LENGTH>::new(current_text_path, send_action),
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
