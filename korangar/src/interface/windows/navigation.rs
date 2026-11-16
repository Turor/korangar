//! Navigation window modeled on roBrowser `UI/Components/Navigation`: NPC and
//! monster search over the client's `navi_*` tables, a cross-map route, a
//! map canvas with the player, target, route waypoints, viewpoint pins and a
//! dotted guide to the next waypoint, plus the `0x08E2` destination.

use korangar_interface::components::text_box::DefaultHandler;
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
use crate::state::navigation::route::NavigationRoutePathExt;
use crate::state::navigation::{Navigation, NavigationPathExt, NavigationTarget};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ClientState, client_theme};

const MAP_SIZE: f32 = 180.0;
const ROW_HEIGHT: f32 = 16.0;
const HIT_HEIGHT: f32 = 18.0;
const QUERY_LIMIT: usize = 48;
/// Cell span used when every coordinate still fits. Larger pins expand it.
const BASE_CELLS: u32 = 256;
/// Dots in the guide line from the player to the next waypoint.
const GUIDE_DOTS: usize = 8;

/// Queues one input event when its row is clicked.
struct EventClick(InputEvent);

impl ClickHandler<ClientState> for EventClick {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(self.0.clone());
    }
}

struct NavigationElement<A> {
    navigation_path: A,
}

impl<A> NavigationElement<A> {
    fn new(navigation_path: A) -> Self {
        Self { navigation_path }
    }
}

struct NavigationLayout {
    area: Area,
}

fn target_on_map(navigation: &Navigation, target: &NavigationTarget) -> bool {
    crate::state::navigation::normalize_map_name(&target.map_name) == navigation.map_name() || navigation.map_name().is_empty()
}

fn canvas_cells(navigation: &Navigation) -> u32 {
    let mut max_cell = BASE_CELLS;
    let (player_x, player_y) = navigation.player_cell();
    max_cell = max_cell.max(player_x as u32).max(player_y as u32);
    if let Some(target) = navigation.target()
        && target_on_map(navigation, target)
    {
        max_cell = max_cell.max(target.x as u32).max(target.y as u32);
    }
    for step in navigation.route().steps_on(navigation.map_name()) {
        max_cell = max_cell.max(step.x as u32).max(step.y as u32);
    }
    for mark in navigation.marks() {
        max_cell = max_cell.max(mark.x).max(mark.y);
    }
    max_cell.max(1)
}

fn cell_point(map_area: Area, cells: u32, x: f32, y: f32) -> (f32, f32) {
    let cells = cells as f32;
    let left = map_area.left + (x / cells) * (map_area.width - 8.0) + 2.0;
    // Cell Y grows north; the window Y grows down.
    let top = map_area.top + ((cells - y) / cells) * (map_area.height - 8.0) + 2.0;
    (left, top)
}

fn pin_area(map_area: Area, cells: u32, x: u32, y: u32, size: f32) -> Area {
    let (left, top) = cell_point(map_area, cells, x as f32, y as f32);
    Area {
        left: left + (6.0 - size) / 2.0,
        top: top + (6.0 - size) / 2.0,
        width: size,
        height: size,
    }
}

fn paint_pin(layout: &mut WindowLayout<ClientState>, area: Area, color: Color) {
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(area.width / 2.0),
        color,
        Color::TRANSPARENT,
        ShadowPadding::uniform(0.0),
    );
}

/// Text rows under the canvas: status, target, guide, route steps, marks.
fn row_count(navigation: &Navigation) -> usize {
    let route = navigation.route();
    let guide = usize::from(!route.guide.is_empty());
    2 + guide + route.steps().len() + navigation.marks().len().max(1)
}

impl<A> Element<ClientState> for NavigationElement<A>
where
    A: Path<ClientState, Navigation>,
{
    type LayoutInfo = NavigationLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let navigation = state.get(&self.navigation_path);
            let height = MAP_SIZE + ROW_HEIGHT * row_count(navigation) as f32;
            Self::LayoutInfo {
                area: resolver.with_height(height),
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
        let navigation = state.get(&self.navigation_path);
        let route = navigation.route();
        let map_area = Area {
            left: layout_info.area.left,
            top: layout_info.area.top,
            width: layout_info.area.width.min(MAP_SIZE),
            height: MAP_SIZE,
        };
        layout.add_rectangle(
            map_area,
            CornerDiameter::uniform(4.0),
            *state.get(&client_theme().tokens().raised()),
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );

        let cells = canvas_cells(navigation);
        let (player_x, player_y) = navigation.player_cell();

        // Dotted guide from the player to the next waypoint on this map.
        if let Some(next) = route.steps_on(navigation.map_name()).find(|step| step.has_cell) {
            let accent2 = *state.get(&client_theme().tokens().accent2());
            for dot in 1..GUIDE_DOTS {
                let t = dot as f32 / GUIDE_DOTS as f32;
                let x = f32::from(player_x) + (f32::from(next.x) - f32::from(player_x)) * t;
                let y = f32::from(player_y) + (f32::from(next.y) - f32::from(player_y)) * t;
                let (left, top) = cell_point(map_area, cells, x, y);
                paint_pin(
                    layout,
                    Area {
                        left: left + 2.0,
                        top: top + 2.0,
                        width: 2.0,
                        height: 2.0,
                    },
                    accent2,
                );
            }
        }

        for step in route.steps_on(navigation.map_name()).filter(|step| step.has_cell) {
            let color = if step.via.is_some() {
                *state.get(&client_theme().tokens().accent2())
            } else {
                *state.get(&client_theme().tokens().gold())
            };
            paint_pin(layout, pin_area(map_area, cells, step.x as u32, step.y as u32, 8.0), color);
        }

        paint_pin(
            layout,
            pin_area(map_area, cells, player_x as u32, player_y as u32, 6.0),
            *state.get(&client_theme().tokens().accent()),
        );

        if let Some(target) = navigation.target()
            && target_on_map(navigation, target)
        {
            paint_pin(
                layout,
                pin_area(map_area, cells, target.x as u32, target.y as u32, 6.0),
                *state.get(&client_theme().tokens().gold()),
            );
        }

        for mark in navigation.marks() {
            paint_pin(
                layout,
                pin_area(map_area, cells, mark.x, mark.y, 6.0),
                Color::rgb_u8(mark.red, mark.green, mark.blue),
            );
        }

        let mut row = 0usize;
        let mut write = |row: usize, text: &'a str, color: Color| {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + MAP_SIZE + ROW_HEIGHT * row as f32,
                width: layout_info.area.width,
                height: ROW_HEIGHT,
            };
            layout.add_text(
                area,
                text,
                FontSize(12.0),
                color,
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        };

        let status_color = if navigation.map_name().is_empty() {
            *state.get(&client_theme().tokens().muted())
        } else {
            *state.get(&client_theme().tokens().text())
        };
        let status_line = if navigation.status_line().is_empty() {
            "Map unknown — you 0,0"
        } else {
            navigation.status_line()
        };
        write(row, status_line, status_color);
        row += 1;

        if let Some(NavigationTarget { summary, .. }) = navigation.target() {
            write(row, summary.as_str(), *state.get(&client_theme().tokens().gold()));
        } else {
            write(row, "No navigation target", *state.get(&client_theme().tokens().muted()));
        }
        row += 1;

        if !route.guide.is_empty() {
            write(row, route.guide.as_str(), *state.get(&client_theme().tokens().gold()));
            row += 1;
        }

        for step in route.steps() {
            let color = if step.map == navigation.map_name() {
                *state.get(&client_theme().tokens().text())
            } else {
                *state.get(&client_theme().tokens().muted())
            };
            write(row, step.label.as_str(), color);
            row += 1;
        }

        if navigation.marks().is_empty() {
            write(row, "No minimap marks", *state.get(&client_theme().tokens().muted()));
        } else {
            for mark in navigation.marks() {
                write(row, mark.summary.as_str(), Color::rgb_u8(mark.red, mark.green, mark.blue));
                row += 1;
            }
        }
    }
}

/// Filter line plus clickable search results. A click routes to that row.
struct NavigationHits<A> {
    navigation_path: A,
}

struct NavigationHitsLayout {
    area: Area,
    filter: String,
    hits: Vec<(String, EventClick)>,
}

impl<A> Element<ClientState> for NavigationHits<A>
where
    A: Path<ClientState, Navigation>,
{
    type LayoutInfo = NavigationHitsLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let route = state.get(&self.navigation_path).route();
            let hits: Vec<_> = route
                .hits()
                .iter()
                .enumerate()
                .map(|(index, hit)| (hit.label.clone(), EventClick(InputEvent::ChooseNavigationResult { index })))
                .collect();
            NavigationHitsLayout {
                area: resolver.with_height(ROW_HEIGHT + HIT_HEIGHT * hits.len() as f32),
                filter: route.filter_line(),
                hits,
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
        layout.add_text(
            Area {
                height: ROW_HEIGHT,
                ..layout_info.area
            },
            &layout_info.filter,
            FontSize(11.0),
            *state.get(&client_theme().tokens().muted()),
            *state.get(&client_theme().tokens().accent()),
            HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        for (index, (text, click)) in layout_info.hits.iter().enumerate() {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + ROW_HEIGHT + HIT_HEIGHT * index as f32,
                width: layout_info.area.width,
                height: HIT_HEIGHT - 2.0,
            };
            let hovered = area.check().run(layout);
            let border = if hovered {
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
                text,
                FontSize(12.0),
                *state.get(&client_theme().tokens().text()),
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if hovered {
                layout.register_click_handler(MouseButton::Left, click);
            }
        }
    }
}

pub struct NavigationWindow<A> {
    navigation_path: A,
}

impl<A> NavigationWindow<A> {
    pub fn new(navigation_path: A) -> Self {
        Self { navigation_path }
    }
}

impl<A> CustomWindow<ClientState> for NavigationWindow<A>
where
    A: Path<ClientState, Navigation> + Copy + 'static,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Navigation)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        struct QueryBox;

        let search = |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::SearchNavigation);
        };

        window! {
            title: "Navigation",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 300.0,
            maximum_width: 480.0,
            minimum_height: 320.0,
            maximum_height: 720.0,
            elements: (
                text_box! {
                    ghost_text: "Search NPC or monster",
                    state: self.navigation_path.route().query(),
                    input_handler: DefaultHandler::<_, _, QUERY_LIMIT>::new(self.navigation_path.route().query(), search),
                    focus_id: QueryBox,
                },
                split! {
                    children: (
                        button! { text: "Search", event: InputEvent::SearchNavigation, },
                        button! { text: "ALL/NPC/MOB", event: InputEvent::CycleNavigationSearchKind, },
                        button! { text: "Services", event: InputEvent::ToggleNavigationServices, },
                        button! { text: "Clear", event: InputEvent::ClearNavigationRoute, },
                    ),
                },
                text! {
                    text: self.navigation_path.route().status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                scroll_view! {
                    children: (
                        NavigationHits { navigation_path: self.navigation_path },
                        NavigationElement::new(self.navigation_path),
                    ),
                },
            ),
        }
    }
}
