//! Achievement and attendance list.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::graphics::Color;
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::ClientState;
use crate::state::achievements::Achievements;
use crate::state::theme::InterfaceThemeType;

struct AchievementListElement<A> {
    path: A,
}

struct AchievementListLayoutInfo {
    area: Area,
    row_height: f32,
}

impl<A> Element<ClientState> for AchievementListElement<A>
where
    A: Path<ClientState, Achievements>,
{
    type LayoutInfo = AchievementListLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let log = state.get(&self.path);
            let rows = log.lines().len().max(1);
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
        let log = state.get(&self.path);
        for (row, line) in log.lines().iter().enumerate() {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + layout_info.row_height * row as f32,
                width: layout_info.area.width,
                height: layout_info.row_height,
            };
            layout.add_text(
                area,
                line.as_str(),
                FontSize(12.0),
                Color::rgb_u8(220, 214, 196),
                Color::rgb_u8(255, 160, 60),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        }
    }
}

pub struct AchievementsWindow<A> {
    path: A,
}

impl<A> AchievementsWindow<A> {
    pub fn new(path: A) -> Self {
        Self { path }
    }
}

impl<A> CustomWindow<ClientState> for AchievementsWindow<A>
where
    A: Path<ClientState, Achievements>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Achievements)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Achievements",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 260.0,
            maximum_width: 440.0,
            minimum_height: 160.0,
            maximum_height: 520.0,
            elements: (
                button! {
                    text: "Claim attendance",
                    event: InputEvent::RequestAttendance,
                },
                button! {
                    text: "Claim reward",
                    event: InputEvent::ClaimAchievementReward,
                },
                scroll_view! {
                    children: AchievementListElement { path: self.path },
                },
            ),
        }
    }
}
