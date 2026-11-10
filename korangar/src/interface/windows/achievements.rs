//! Achievement and attendance list.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::client_theme;
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
            let rows = log.lines().len().saturating_sub(1).max(1);
            let row_height = 28.0;
            let area = resolver.with_height(36.0 + row_height * rows as f32);
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
        let lines = log.lines();
        let rank = lines.first().map(String::as_str).unwrap_or("");
        let header = Area {
            left: layout_info.area.left,
            top: layout_info.area.top,
            width: layout_info.area.width,
            height: 16.0,
        };
        layout.add_text(
            header,
            rank,
            FontSize(12.0),
            *state.get(&client_theme().tokens().text()),
            *state.get(&client_theme().tokens().accent()),
            HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        let bar = Area {
            left: layout_info.area.left + 4.0,
            top: layout_info.area.top + 18.0,
            width: (layout_info.area.width - 8.0).max(1.0),
            height: 8.0,
        };
        paint_bar(layout, bar, fraction_after(rank, "exp "), *state.get(&client_theme().tokens().accent()), *state.get(&client_theme().tokens().raised()));

        let rows: Vec<&str> = lines.iter().skip(1).map(String::as_str).collect();
        let rows = if rows.is_empty() { vec!["No achievements"] } else { rows };
        for (index, row) in rows.iter().enumerate() {
            let top = layout_info.area.top + 36.0 + layout_info.row_height * index as f32;
            let text_area = Area {
                left: layout_info.area.left,
                top,
                width: layout_info.area.width,
                height: 14.0,
            };
            let claimable = row.contains("complete") && !row.contains("rewarded");
            let color = if claimable {
                *state.get(&client_theme().tokens().gold())
            } else {
                *state.get(&client_theme().tokens().text())
            };
            layout.add_text(
                text_area,
                row,
                FontSize(12.0),
                color,
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            let row_bar = Area {
                left: layout_info.area.left + 4.0,
                top: top + 16.0,
                width: (layout_info.area.width - 8.0).max(1.0),
                height: 6.0,
            };
            paint_bar(layout, row_bar, fraction_in_parens(row), *state.get(&client_theme().tokens().accent2()), *state.get(&client_theme().tokens().raised()));
        }
    }
}

/// `exp 25/100` inside a rank line. Missing numbers draw an empty bar.
fn fraction_after(text: &str, marker: &str) -> f32 {
    let Some(start) = text.find(marker) else {
        return 0.0;
    };
    fraction_in_parens(&format!(
        "({})",
        text[start + marker.len()..].split_whitespace().next().unwrap_or("0/1")
    ))
}

fn fraction_in_parens(text: &str) -> f32 {
    let Some(open) = text.rfind('(') else {
        return 0.0;
    };
    let Some(close) = text[open..].find(')') else {
        return 0.0;
    };
    let body = &text[open + 1..open + close];
    let mut parts = body.split('/');
    let done = parts.next().and_then(|value| value.trim().parse::<f32>().ok()).unwrap_or(0.0);
    let total = parts.next().and_then(|value| value.trim().parse::<f32>().ok()).unwrap_or(1.0);
    if total <= 0.0 { 0.0 } else { (done / total).clamp(0.0, 1.0) }
}

fn paint_bar(layout: &mut WindowLayout<ClientState>, area: Area, fill: f32, color: Color, track: Color) {
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(2.0),
        track,
        Color::TRANSPARENT,
        ShadowPadding::uniform(0.0),
    );
    let width = (area.width * fill.clamp(0.0, 1.0)).max(0.0);
    if width > 0.0 {
        layout.add_rectangle(
            Area {
                left: area.left,
                top: area.top,
                width,
                height: area.height,
            },
            CornerDiameter::uniform(2.0),
            color,
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );
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
                    text: "Filter",
                    event: InputEvent::CycleAchievementFilter,
                },
                button! {
                    text: "Category",
                    event: InputEvent::CycleAchievementCategory,
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
