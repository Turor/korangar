//! Navigation window: destination from `0x08E2` and viewpoint pins from
//! `0x0144`.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::client_theme;
use crate::state::ClientState;
use crate::state::navigation::{Navigation, NavigationTarget};
use crate::state::theme::InterfaceThemeType;

const MAP_SIZE: f32 = 180.0;
const ROW_HEIGHT: f32 = 16.0;
/// Cell span used when every coordinate still fits. Larger pins expand it.
const BASE_CELLS: u32 = 256;

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

fn canvas_cells(navigation: &Navigation) -> u32 {
    let mut max_cell = BASE_CELLS;
    let (player_x, player_y) = navigation.player_cell();
    max_cell = max_cell.max(player_x as u32).max(player_y as u32);
    if let Some(target) = navigation.target() {
        let same_map =
            crate::state::navigation::normalize_map_name(&target.map_name) == navigation.map_name() || navigation.map_name().is_empty();
        if same_map {
            max_cell = max_cell.max(target.x as u32).max(target.y as u32);
        }
    }
    for mark in navigation.marks() {
        max_cell = max_cell.max(mark.x).max(mark.y);
    }
    max_cell.max(1)
}

fn pin_area(map_area: Area, cells: u32, x: u32, y: u32) -> Area {
    let cells = cells as f32;
    let left = map_area.left + (x as f32 / cells) * (map_area.width - 8.0) + 2.0;
    // Cell Y grows north; the window Y grows down.
    let top = map_area.top + ((cells - y as f32) / cells) * (map_area.height - 8.0) + 2.0;
    Area {
        left,
        top,
        width: 6.0,
        height: 6.0,
    }
}

fn paint_pin(layout: &mut WindowLayout<ClientState>, area: Area, color: Color) {
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(3.0),
        color,
        Color::TRANSPARENT,
        ShadowPadding::uniform(0.0),
    );
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
            let rows = 2 + navigation.marks().len().max(1);
            let height = MAP_SIZE + ROW_HEIGHT * rows as f32;
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
        paint_pin(
            layout,
            pin_area(map_area, cells, player_x as u32, player_y as u32),
            *state.get(&client_theme().tokens().accent()),
        );

        if let Some(target) = navigation.target() {
            let same_map =
                crate::state::navigation::normalize_map_name(&target.map_name) == navigation.map_name() || navigation.map_name().is_empty();
            if same_map {
                paint_pin(
                    layout,
                    pin_area(map_area, cells, target.x as u32, target.y as u32),
                    *state.get(&client_theme().tokens().gold()),
                );
            }
        }

        for mark in navigation.marks() {
            paint_pin(
                layout,
                pin_area(map_area, cells, mark.x, mark.y),
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
            row += 1;
        } else {
            write(row, "No navigation target", *state.get(&client_theme().tokens().muted()));
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
    A: Path<ClientState, Navigation>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Navigation)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Navigation",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 220.0,
            maximum_width: 420.0,
            minimum_height: 240.0,
            maximum_height: 560.0,
            elements: (
                scroll_view! {
                    children: NavigationElement::new(self.navigation_path),
                },
            ),
        }
    }
}
