use korangar_interface::application::Size;
use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, StateElement};
use korangar_interface::event::ClickHandler;
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
use crate::state::chat::{ChannelMaskPathExt, ChatChannel, ChatFilterState, ChatFilterStatePathExt, ChatTab};
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::{ChatThemePathExt, InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ChatMessage, ClientState, ClientStatePathExt, client_state, client_theme};

const MAXIMUM_CHAT_MESSAGE_LENGTH: usize = 80;
const MAXIMUM_WHISPER_NAME_LENGTH: usize = 23;

fn themed_message_color(state: &State<ClientState>, color: MessageColor) -> Color {
    match color {
        MessageColor::Rgb { red, green, blue } => Color::rgb_u8(red, green, blue),
        MessageColor::Broadcast => *state.get(&client_theme().chat().broadcast_color()),
        MessageColor::Server => *state.get(&client_theme().chat().server_color()),
        MessageColor::Error => *state.get(&client_theme().chat().error_color()),
        MessageColor::Information => *state.get(&client_theme().chat().information_color()),
        MessageColor::Public => *state.get(&client_theme().chat().public_color()),
        MessageColor::Whisper => *state.get(&client_theme().chat().whisper_color()),
        MessageColor::Party => *state.get(&client_theme().chat().party_color()),
        MessageColor::Guild => *state.get(&client_theme().chat().guild_color()),
    }
}

/// ZST for getting the focus id of the chat text box. This is only needed to
/// focus the chat when pressing enter.
pub struct ChatTextBox;

/// Focus id for the whisper-target name box. Enter leaves the field.
struct WhisperTargetTextBox;

struct ChatLayoutInfo {
    area: Area,
    // TODO: Don't allocate this every frame.
    message_heights: Vec<f32>,
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

impl<A, B> Element<ClientState> for ChatElement<A, B>
where
    A: Path<ClientState, Vec<ChatMessage>>,
    B: Path<ClientState, ChatWindowState>,
{
    type LayoutInfo = ChatLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let chat_messages = visible_messages(state, &self.chat_messages_path, &self.chat_window_state);
            let message_spacing = 5.0;
            let font_size = *state.get(&client_theme().text().font_size());
            let highlight = *state.get(&client_theme().text().highlight_color());

            let mut total_height = 0.0;
            let message_heights = chat_messages
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

                    if total_height != 0.0 {
                        total_height += message_spacing;
                    }

                    total_height += size.height();

                    size.height()
                })
                .collect();

            let area = resolver.with_height(total_height);

            Self::LayoutInfo { area, message_heights }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let chat_messages = visible_messages(state, &self.chat_messages_path, &self.chat_window_state);
        let message_spacing = 5.0;
        let font_size = *state.get(&client_theme().text().font_size());
        let highlight = *state.get(&client_theme().text().highlight_color());

        let mut offset = 0.0;
        chat_messages
            .iter()
            .zip(layout_info.message_heights.iter())
            .for_each(|(chat_message, message_height)| {
                let color = themed_message_color(state, chat_message.color);

                if offset != 0.0 {
                    offset += message_spacing;
                }

                let text_area = Area {
                    left: layout_info.area.left,
                    top: layout_info.area.top + offset,
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

                offset += message_height;
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
    /// Name typed in the whisper target box. Empty sends public chat.
    whisper_target: String,
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

    pub(crate) fn begin_whisper(&mut self, target: String, text: String) -> (String, String) {
        crate::state::chat::remember_whisper_target(&mut self.recent_whisper_targets, &target);
        self.whisper_target.clone_from(&target);
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
        let whisper_name_action = move |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(Event::Unfocus);
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
            minimum_height: 220.0,
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
                text! { text: label_path },
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
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text_box! {
                            ghost_text: "Whisper to",
                            state: whisper_target_path,
                            input_handler: DefaultHandler::<_, _, MAXIMUM_WHISPER_NAME_LENGTH>::new(whisper_target_path, whisper_name_action),
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
                scroll_view! {
                    follow: true,
                    children: ChatElement::new(messages_path, window_state),
                },
            ),
        }
    }
}
