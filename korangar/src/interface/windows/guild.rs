//! Guild window: basic info, members, and the notice.

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, StateElement};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, PathExt, RustState, State};

use crate::graphics::Color;
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::ClientState;
use crate::state::guild::{GuildState, GuildStatePathExt};
use crate::state::theme::InterfaceThemeType;

const SUBJECT_LIMIT: usize = 60;
const NOTICE_LIMIT: usize = 120;

struct GuildBody<A> {
    guild_path: A,
}

impl<A> GuildBody<A> {
    fn new(guild_path: A) -> Self {
        Self { guild_path }
    }
}

struct GuildBodyLayout {
    area: Area,
    row_height: f32,
}

impl<A> Element<ClientState> for GuildBody<A>
where
    A: Path<ClientState, GuildState>,
{
    type LayoutInfo = GuildBodyLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let rows = state.get(&self.guild_path).rows.len().max(1);
            let row_height = 18.0;
            let area = resolver.with_height(row_height * rows as f32);
            Self::LayoutInfo { area, row_height }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let lines = &state.get(&self.guild_path).rows;

        for (index, line) in lines.iter().enumerate() {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + layout_info.row_height * index as f32,
                width: layout_info.area.width,
                height: layout_info.row_height,
            };
            let color = if line == "Members" {
                Color::rgb_u8(235, 200, 120)
            } else {
                Color::rgb_u8(220, 224, 232)
            };
            layout.add_text(
                area,
                line.as_str(),
                FontSize(12.0),
                color,
                Color::rgb_u8(255, 160, 60),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        }
    }
}

/// Drafts for the notice editor. The server copy lives on [`GuildState`].
#[derive(Default, RustState, StateElement)]
pub struct GuildWindowState {
    pub subject: String,
    pub notice: String,
}

impl GuildWindowState {
    pub fn clear(&mut self) {
        self.subject.clear();
        self.notice.clear();
    }
}

pub struct GuildWindow<A, B> {
    window_state_path: A,
    guild_path: B,
}

impl<A, B> GuildWindow<A, B> {
    pub fn new(window_state_path: A, guild_path: B) -> Self {
        Self {
            window_state_path,
            guild_path,
        }
    }
}

impl<A, B> CustomWindow<ClientState> for GuildWindow<A, B>
where
    A: Path<ClientState, GuildWindowState>,
    B: Path<ClientState, GuildState>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Guild)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        struct SubjectBox;
        struct NoticeBox;

        let subject_path = self.window_state_path.subject();
        let notice_path = self.window_state_path.notice();
        let save_notice = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            let subject = state.get(&subject_path).clone();
            let notice = state.get(&notice_path).clone();
            queue.queue(InputEvent::UpdateGuildNotice { subject, notice });
        };

        window! {
            title: "Guild",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            minimum_width: 420.0,
            maximum_width: 640.0,
            closable: true,
            elements: (
                button! {
                    text: "Refresh",
                    event: InputEvent::RequestGuildWindow,
                },
                either! {
                    selector: self.guild_path.joined(),
                    on_true: split! {
                        gaps: theme().window().gaps(),
                        children: (
                            GuildBody::new(self.guild_path),
                            text! { text: "Subject", },
                            text_box! {
                                ghost_text: "Guild subject",
                                state: self.window_state_path.subject(),
                                input_handler: DefaultHandler::<_, _, SUBJECT_LIMIT>::new(self.window_state_path.subject(), save_notice),
                                focus_id: SubjectBox,
                            },
                            text! { text: "Notice", },
                            text_box! {
                                ghost_text: "Guild notice",
                                state: self.window_state_path.notice(),
                                input_handler: DefaultHandler::<_, _, NOTICE_LIMIT>::new(self.window_state_path.notice(), save_notice),
                                focus_id: NoticeBox,
                            },
                            button! {
                                text: "Save notice",
                                disabled: ComputedSelector::new_default(move |state: &ClientState| {
                                    !*self.guild_path.is_master().follow_safe(state)
                                }),
                                event: save_notice,
                            },
                        ),
                    },
                    on_false: text! { text: "You are not in a guild.", },
                },
            ),
        }
    }
}
