//! Achievement window modeled on roBrowser `UI/Components/Achievement`: major
//! and minor category tabs, a summary page with per-tab gauges, a clickable
//! list with progress and points, and a detail panel with goals and rewards.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::achievements::{ACHIEVEMENT_MAJORS, AchievementRow, Achievements, CategoryProgress, OBJECTIVE_SLOTS};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ClientState, client_theme};

const ROW_HEIGHT: f32 = 30.0;
const TAB_HEIGHT: f32 = 20.0;
const LINE_HEIGHT: f32 = 16.0;

/// Queues one input event when its row or tab is clicked.
struct EventClick(InputEvent);

impl ClickHandler<ClientState> for EventClick {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(self.0.clone());
    }
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
            Area { width, ..area },
            CornerDiameter::uniform(2.0),
            color,
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );
    }
}

/// Minor tabs of the open major tab. Empty for Summary and Feat.
struct MinorTabs<A> {
    path: A,
}

struct MinorTabsLayout {
    area: Area,
    tabs: Vec<(&'static str, bool, EventClick)>,
}

impl<A> Element<ClientState> for MinorTabs<A>
where
    A: Path<ClientState, Achievements>,
{
    type LayoutInfo = MinorTabsLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let achievements = state.get(&self.path);
            let major = achievements.major();
            let minors: &[&str] = ACHIEVEMENT_MAJORS
                .iter()
                .find(|(id, ..)| *id == major)
                .map(|(_, _, minors)| *minors)
                .unwrap_or(&[]);
            let mut tabs = Vec::new();
            if !minors.is_empty() {
                tabs.push((
                    "All",
                    achievements.minor().is_none(),
                    EventClick(InputEvent::SelectAchievementCategory { major, minor: None }),
                ));
                for (index, name) in minors.iter().enumerate() {
                    let minor = index as u8;
                    tabs.push((
                        *name,
                        achievements.minor() == Some(minor),
                        EventClick(InputEvent::SelectAchievementCategory { major, minor: Some(minor) }),
                    ));
                }
            }
            let height = if tabs.is_empty() { 0.0 } else { TAB_HEIGHT };
            MinorTabsLayout {
                area: resolver.with_height(height),
                tabs,
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        if layout_info.tabs.is_empty() {
            return;
        }
        let width = layout_info.area.width / layout_info.tabs.len() as f32;
        for (index, (name, active, click)) in layout_info.tabs.iter().enumerate() {
            let area = Area {
                left: layout_info.area.left + width * index as f32,
                top: layout_info.area.top,
                width: (width - 2.0).max(1.0),
                height: TAB_HEIGHT,
            };
            let hovered = area.check().run(layout);
            let border = if *active {
                *state.get(&client_theme().tokens().gold())
            } else if hovered {
                *state.get(&client_theme().tokens().hover())
            } else {
                *state.get(&client_theme().tokens().border())
            };
            layout.add_rectangle(
                area,
                CornerDiameter::uniform(3.0),
                *state.get(&client_theme().tokens().raised()),
                border,
                ShadowPadding::uniform(1.0),
            );
            layout.add_text(
                area,
                name,
                FontSize(11.0),
                *state.get(&client_theme().tokens().text()),
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Center { offset: 0.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if hovered {
                layout.register_click_handler(MouseButton::Left, click);
            }
        }
    }
}

/// Rank, experience bar and, on the summary page, one gauge per tab.
struct RankHeader<A> {
    path: A,
}

struct RankHeaderLayout {
    area: Area,
    rank: String,
    fraction: f32,
    gauges: Vec<(CategoryProgress, String)>,
}

impl<A> Element<ClientState> for RankHeader<A>
where
    A: Path<ClientState, Achievements>,
{
    type LayoutInfo = RankHeaderLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let achievements = state.get(&self.path);
            let gauges = if achievements.major() == 0 {
                achievements
                    .category_progress()
                    .into_iter()
                    .map(|gauge| {
                        let text = format!("{} {}/{}", gauge.name, gauge.completed, gauge.total);
                        (gauge, text)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let height = 28.0 + gauges.len().div_ceil(2) as f32 * 26.0;
            RankHeaderLayout {
                area: resolver.with_height(height),
                rank: achievements.lines().first().cloned().unwrap_or_default(),
                fraction: achievements.experience_fraction(),
                gauges,
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let area = layout_info.area;
        layout.add_text(
            Area {
                height: LINE_HEIGHT,
                ..area
            },
            &layout_info.rank,
            FontSize(12.0),
            *state.get(&client_theme().tokens().text()),
            *state.get(&client_theme().tokens().accent()),
            HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        paint_bar(
            layout,
            Area {
                left: area.left + 4.0,
                top: area.top + 18.0,
                width: (area.width - 8.0).max(1.0),
                height: 6.0,
            },
            layout_info.fraction,
            *state.get(&client_theme().tokens().accent()),
            *state.get(&client_theme().tokens().raised()),
        );
        let half = area.width / 2.0;
        for (index, (gauge, text)) in layout_info.gauges.iter().enumerate() {
            let left = area.left + half * (index % 2) as f32;
            let top = area.top + 28.0 + 26.0 * (index / 2) as f32;
            layout.add_text(
                Area {
                    left,
                    top,
                    width: half,
                    height: 14.0,
                },
                text,
                FontSize(11.0),
                *state.get(&client_theme().tokens().text()),
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            let fill = if gauge.total == 0 {
                0.0
            } else {
                gauge.completed as f32 / gauge.total as f32
            };
            paint_bar(
                layout,
                Area {
                    left: left + 4.0,
                    top: top + 16.0,
                    width: (half - 8.0).max(1.0),
                    height: 5.0,
                },
                fill,
                *state.get(&client_theme().tokens().accent2()),
                *state.get(&client_theme().tokens().raised()),
            );
        }
    }
}

/// Clickable achievement rows. A click selects the row for the detail panel
/// and the Claim reward button.
struct AchievementList<A> {
    path: A,
}

struct AchievementListLayout {
    area: Area,
    rows: Vec<(AchievementRow, String, EventClick)>,
}

impl<A> Element<ClientState> for AchievementList<A>
where
    A: Path<ClientState, Achievements>,
{
    type LayoutInfo = AchievementListLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let rows: Vec<_> = state
                .get(&self.path)
                .rows()
                .iter()
                .map(|row| {
                    let points = if row.score > 0 {
                        format!("{} pts", row.score)
                    } else {
                        String::new()
                    };
                    (
                        row.clone(),
                        points,
                        EventClick(InputEvent::SelectAchievement {
                            achievement_id: row.achievement_id,
                        }),
                    )
                })
                .collect();
            let height = ROW_HEIGHT * rows.len().max(1) as f32;
            AchievementListLayout {
                area: resolver.with_height(height),
                rows,
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        if layout_info.rows.is_empty() {
            layout.add_text(
                Area {
                    height: ROW_HEIGHT,
                    ..layout_info.area
                },
                "No achievements",
                FontSize(12.0),
                *state.get(&client_theme().tokens().muted()),
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            return;
        }
        for (index, (row, points, click)) in layout_info.rows.iter().enumerate() {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + ROW_HEIGHT * index as f32,
                width: layout_info.area.width,
                height: ROW_HEIGHT - 2.0,
            };
            let hovered = area.check().run(layout);
            let border = if row.selected {
                *state.get(&client_theme().tokens().gold())
            } else if hovered {
                *state.get(&client_theme().tokens().hover())
            } else {
                *state.get(&client_theme().tokens().border())
            };
            layout.add_rectangle(
                area,
                CornerDiameter::uniform(3.0),
                *state.get(&client_theme().tokens().raised()),
                border,
                ShadowPadding::uniform(1.0),
            );
            let text_color = if row.claimable {
                *state.get(&client_theme().tokens().gold())
            } else if row.rewarded {
                *state.get(&client_theme().tokens().muted())
            } else {
                *state.get(&client_theme().tokens().text())
            };
            layout.add_text(
                Area {
                    left: area.left,
                    top: area.top + 1.0,
                    width: (area.width - 56.0).max(1.0),
                    height: 15.0,
                },
                &row.text,
                FontSize(12.0),
                text_color,
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            layout.add_text(
                Area {
                    left: area.left + area.width - 56.0,
                    top: area.top + 1.0,
                    width: 52.0,
                    height: 15.0,
                },
                points,
                FontSize(11.0),
                *state.get(&client_theme().tokens().muted()),
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Right { offset: 2.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            paint_bar(
                layout,
                Area {
                    left: area.left + 4.0,
                    top: area.top + 19.0,
                    width: (area.width - 8.0).max(1.0),
                    height: 5.0,
                },
                if row.claimable || row.rewarded {
                    1.0
                } else {
                    f32::from(row.objective_hits) / f32::from(OBJECTIVE_SLOTS)
                },
                *state.get(&client_theme().tokens().accent2()),
                *state.get(&client_theme().tokens().bg()),
            );
            if hovered {
                layout.register_click_handler(MouseButton::Left, click);
            }
        }
    }
}

/// Selected achievement: title, tab and points, description, goals, reward.
struct AchievementDetail<A> {
    path: A,
}

struct AchievementDetailLayout {
    area: Area,
    lines: Vec<String>,
}

impl<A> Element<ClientState> for AchievementDetail<A>
where
    A: Path<ClientState, Achievements>,
{
    type LayoutInfo = AchievementDetailLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let lines = state.get(&self.path).detail().to_vec();
            AchievementDetailLayout {
                area: resolver.with_height(LINE_HEIGHT * lines.len().max(1) as f32 + 6.0),
                lines,
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        layout.add_rectangle(
            layout_info.area,
            CornerDiameter::uniform(4.0),
            *state.get(&client_theme().tokens().raised()),
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );
        for (index, line) in layout_info.lines.iter().enumerate() {
            let color = if index == 0 {
                *state.get(&client_theme().tokens().gold())
            } else {
                *state.get(&client_theme().tokens().text())
            };
            layout.add_text(
                Area {
                    left: layout_info.area.left,
                    top: layout_info.area.top + 3.0 + LINE_HEIGHT * index as f32,
                    width: layout_info.area.width,
                    height: LINE_HEIGHT,
                },
                line,
                FontSize(if index == 0 { 13.0 } else { 12.0 }),
                color,
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Left { offset: 6.0, border: 2.0 },
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
    A: Path<ClientState, Achievements> + Copy,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Achievements)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let tab = |major: u8| InputEvent::SelectAchievementCategory { major, minor: None };

        window! {
            title: "Achievements",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 420.0,
            maximum_width: 640.0,
            minimum_height: 320.0,
            maximum_height: 720.0,
            elements: (
                crate::interface::windows::WindowGroupTabs::new(WindowClass::Achievements),
                split! {
                    children: (
                        button! { text: "Summary", event: tab(0), },
                        button! { text: "General", event: tab(1), },
                        button! { text: "Adventure", event: tab(2), },
                        button! { text: "Battle", event: tab(3), },
                    ),
                },
                split! {
                    children: (
                        button! { text: "Quest", event: tab(4), },
                        button! { text: "Memorial", event: tab(5), },
                        button! { text: "Feat", event: tab(6), },
                    ),
                },
                MinorTabs { path: self.path },
                RankHeader { path: self.path },
                split! {
                    children: (
                        button! { text: "Filter", event: InputEvent::CycleAchievementFilter, },
                        button! { text: "Claim reward", event: InputEvent::ClaimAchievementReward, },
                        button! { text: "Attendance", event: InputEvent::ToggleAttendanceWindow, },
                    ),
                },
                AchievementDetail { path: self.path },
                scroll_view! {
                    children: AchievementList { path: self.path },
                },
            ),
        }
    }
}
