//! Basic quest journal fed by typed quest network events.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::graphics::Color;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::ClientState;
use crate::state::quest_log::QuestLog;
use crate::state::theme::InterfaceThemeType;

struct QuestJournalElement<A> {
    quest_log_path: A,
}

impl<A> QuestJournalElement<A> {
    fn new(quest_log_path: A) -> Self {
        Self { quest_log_path }
    }
}

struct QuestJournalLayoutInfo {
    area: Area,
    row_height: f32,
}

impl<A> Element<ClientState> for QuestJournalElement<A>
where
    A: Path<ClientState, QuestLog>,
{
    type LayoutInfo = QuestJournalLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let quests = state.get(&self.quest_log_path).quests();
            let row_height = 18.0;
            // One summary row per quest plus one row per objective.
            let rows = quests.iter().map(|quest| 1 + quest.objectives.len()).sum::<usize>().max(1);
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
        let quests = state.get(&self.quest_log_path).quests();
        if quests.is_empty() {
            layout.add_text(
                layout_info.area,
                "No quests",
                FontSize(12.0),
                Color::rgb_u8(160, 164, 176),
                Color::rgb_u8(255, 160, 60),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            return;
        }

        let mut row = 0usize;
        for quest in quests {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + layout_info.row_height * row as f32,
                width: layout_info.area.width,
                height: layout_info.row_height,
            };
            layout.add_text(
                area,
                quest.summary.as_str(),
                FontSize(12.0),
                Color::rgb_u8(235, 200, 120),
                Color::rgb_u8(255, 160, 60),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            row += 1;
            for objective in &quest.objectives {
                let area = Area {
                    left: layout_info.area.left,
                    top: layout_info.area.top + layout_info.row_height * row as f32,
                    width: layout_info.area.width,
                    height: layout_info.row_height,
                };
                layout.add_text(
                    area,
                    objective.summary.as_str(),
                    FontSize(12.0),
                    Color::rgb_u8(200, 204, 214),
                    Color::rgb_u8(255, 160, 60),
                    HorizontalAlignment::Left { offset: 14.0, border: 2.0 },
                    VerticalAlignment::Center { offset: 0.0 },
                    OverflowBehavior::Shrink,
                );
                row += 1;
            }
        }
    }
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
    A: Path<ClientState, QuestLog>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::QuestJournal)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Quests",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 240.0,
            maximum_width: 420.0,
            minimum_height: 140.0,
            maximum_height: 520.0,
            elements: (
                scroll_view! {
                    children: QuestJournalElement::new(self.quest_log_path),
                },
            ),
        }
    }
}
