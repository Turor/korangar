//! Quest journal: active, inactive, and cooldown tabs.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::graphics::{CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::quest_log::{JournalLine, JournalLineKind, QuestFilter, QuestLog, QuestLogPathExt};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, client_theme};

struct QuestRowClick<A> {
    quest_log_path: A,
    quest_id: u32,
    kind: JournalLineKind,
    make_active: bool,
}

impl<A> ClickHandler<ClientState> for QuestRowClick<A>
where
    A: Path<ClientState, QuestLog>,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        match self.kind {
            JournalLineKind::Title => {
                let quest_log_path = self.quest_log_path;
                let quest_id = self.quest_id;
                state.update_value_with(quest_log_path, move |quest_log| quest_log.toggle_selected(quest_id));
            }
            JournalLineKind::Toggle => {
                queue.queue(InputEvent::SetQuestActive {
                    quest_id: self.quest_id,
                    active: self.make_active,
                });
            }
            JournalLineKind::Body | JournalLineKind::Empty => {}
        }
    }
}

struct QuestJournalElement<A> {
    quest_log_path: A,
    lines: Vec<JournalLine>,
    clicks: Vec<QuestRowClick<A>>,
}

impl<A> QuestJournalElement<A>
where
    A: Copy,
{
    fn new(quest_log_path: A) -> Self {
        Self {
            quest_log_path,
            lines: Vec::new(),
            clicks: Vec::new(),
        }
    }
}

struct QuestJournalLayoutInfo {
    area: Area,
    row_height: f32,
}

impl<A> Element<ClientState> for QuestJournalElement<A>
where
    A: Path<ClientState, QuestLog> + Copy,
{
    type LayoutInfo = QuestJournalLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let now = unix_now();
        let quest_log_path = self.quest_log_path;
        self.lines = state.get(&quest_log_path).journal_lines(now);
        self.clicks = self
            .lines
            .iter()
            .map(|line| QuestRowClick {
                quest_log_path: self.quest_log_path,
                quest_id: line.quest_id,
                kind: line.kind,
                make_active: line.make_active,
            })
            .collect();
        let token_path = client_theme().tokens();
        let font_size = state.get(&token_path).font_size;
        let row_height = font_size + 8.0;
        with_single_resolver(resolvers, |resolver| {
            let rows = self.lines.len().max(1) as f32;
            let area = resolver.with_height(row_height * rows);
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
        let token_path = client_theme().tokens();
        let tokens = state.get(&token_path);
        let font_size = FontSize(tokens.font_size);
        for (index, line) in self.lines.iter().enumerate() {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + layout_info.row_height * index as f32,
                width: layout_info.area.width,
                height: layout_info.row_height,
            };
            let hovered = area.check().run(layout);
            let border = if line.selected {
                tokens.accent
            } else if hovered {
                tokens.hover
            } else {
                tokens.border
            };
            if line.kind == JournalLineKind::Title || line.kind == JournalLineKind::Toggle {
                layout.add_rectangle(
                    area,
                    CornerDiameter::uniform(tokens.corner_radius),
                    tokens.raised,
                    border,
                    ShadowPadding::uniform(1.0),
                );
            }
            let color = match line.kind {
                JournalLineKind::Title => tokens.text,
                JournalLineKind::Toggle => tokens.accent,
                JournalLineKind::Empty => tokens.muted,
                JournalLineKind::Body => tokens.muted,
            };
            let offset = if line.kind == JournalLineKind::Body { 14.0 } else { 4.0 };
            layout.add_text(
                area,
                line.text.as_str(),
                font_size,
                color,
                tokens.hover,
                HorizontalAlignment::Left { offset, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::LineBreak,
            );
            if hovered && matches!(line.kind, JournalLineKind::Title | JournalLineKind::Toggle) {
                layout.register_click_handler(MouseButton::Left, &self.clicks[index]);
            }
        }
    }
}

fn unix_now() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as u32)
        .unwrap_or(0)
}

pub struct QuestJournalWindow<A> {
    quest_log_path: A,
}

impl<A> QuestJournalWindow<A> {
    pub fn new(quest_log_path: A) -> Self {
        Self { quest_log_path }
    }
}

impl<A> CustomWindow<ClientState> for QuestJournalWindow<A>
where
    A: Path<ClientState, QuestLog> + Copy,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::QuestJournal)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let quest_log_path = self.quest_log_path;
        window! {
            title: "Quests",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 260.0,
            maximum_width: 460.0,
            minimum_height: 180.0,
            maximum_height: 560.0,
            elements: (
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Active",
                            event: move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                state.update_value(quest_log_path.filter(), QuestFilter::Active);
                            },
                        },
                        button! {
                            text: "Inactive",
                            event: move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                state.update_value(quest_log_path.filter(), QuestFilter::Inactive);
                            },
                        },
                        button! {
                            text: "Cooldown",
                            event: move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                state.update_value(quest_log_path.filter(), QuestFilter::Cooldown);
                            },
                        },
                    ),
                },
                scroll_view! {
                    children: QuestJournalElement::new(quest_log_path),
                },
            ),
        }
    }
}
