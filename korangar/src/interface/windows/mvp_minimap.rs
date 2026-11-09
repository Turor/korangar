use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, StateElement};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::minimap_point;
use ragnarok_packets::MvpMinimapEntry;
use rust_state::{Path, RustState, State};

use super::WindowClass;
use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state, this_entity};

/// Square minimap in interface pixels.
const MINIMAP_SIZE: f32 = 180.0;
const DOT: f32 = 6.0;

/// Last `0x0EF5` snapshot and the loaded map's tile size.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct MvpMinimapState {
    flags: u8,
    map_width: u16,
    map_height: u16,
    markers: Vec<MvpMinimapEntry>,
    summary: String,
}

impl Default for MvpMinimapState {
    fn default() -> Self {
        Self {
            flags: 0,
            map_width: 0,
            map_height: 0,
            markers: Vec::new(),
            summary: "No alive MVPs.".to_owned(),
        }
    }
}

impl MvpMinimapState {
    pub fn replace(&mut self, flags: u8, markers: Vec<MvpMinimapEntry>) {
        self.flags = flags;
        self.summary = korangar_networking::format_mvp_markers(&markers);
        self.markers = markers;
    }

    pub fn clear_markers(&mut self) {
        self.flags = 0;
        self.markers.clear();
        self.summary = "No alive MVPs.".to_owned();
    }

    pub fn set_map_size(&mut self, width: u16, height: u16) {
        self.map_width = width;
        self.map_height = height;
    }
}

pub struct MinimapLayout {
    area: Area,
    mvp_dots: Vec<(f32, f32)>,
    player_dot: Option<(f32, f32)>,
}

/// Draws the loaded map as a square and plots alive MVPs plus the player.
pub struct MvpMinimapView<A> {
    board_path: A,
}

impl<A> MvpMinimapView<A> {
    pub fn new(board_path: A) -> Self {
        Self { board_path }
    }
}

impl<A> Element<ClientState> for MvpMinimapView<A>
where
    A: Path<ClientState, MvpMinimapState>,
{
    type LayoutInfo = MinimapLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let root = client_state();
        let (map_width, map_height, mvp_dots, lines) = {
            let board = state.get(&self.board_path);
            let mvp_dots = board
                .markers
                .iter()
                .filter_map(|marker| minimap_point(board.map_width, board.map_height, marker.position, MINIMAP_SIZE))
                .collect();
            let lines = board.summary.lines().count().max(1) as f32;
            (board.map_width, board.map_height, mvp_dots, lines)
        };
        let player_dot = this_entity()
            .follow(state.get(&root))
            .and_then(|player| minimap_point(map_width, map_height, player.get_tile_position(), MINIMAP_SIZE));
        let area = with_single_resolver(resolvers, |resolver| resolver.with_height(MINIMAP_SIZE + 8.0 + lines * 16.0));
        MinimapLayout {
            area,
            mvp_dots,
            player_dot,
        }
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _store: ElementStore<'a>,
        info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let canvas = Area {
            left: info.area.left,
            top: info.area.top,
            width: MINIMAP_SIZE,
            height: MINIMAP_SIZE,
        };
        layout.add_rectangle(
            canvas,
            CornerDiameter::uniform(4.0),
            Color::rgb_u8(28, 40, 32),
            Color::rgba_u8(0, 0, 0, 80),
            ShadowPadding::diagonal(1.0, 2.0),
        );

        for (x, y) in &info.mvp_dots {
            layout.add_rectangle(
                Area {
                    left: canvas.left + x - DOT / 2.0,
                    top: canvas.top + y - DOT / 2.0,
                    width: DOT,
                    height: DOT,
                },
                CornerDiameter::uniform(DOT / 2.0),
                Color::rgb_u8(220, 48, 48),
                Color::rgba_u8(0, 0, 0, 0),
                ShadowPadding::default(),
            );
        }

        if let Some((x, y)) = info.player_dot {
            layout.add_rectangle(
                Area {
                    left: canvas.left + x - DOT / 2.0,
                    top: canvas.top + y - DOT / 2.0,
                    width: DOT,
                    height: DOT,
                },
                CornerDiameter::uniform(DOT / 2.0),
                Color::rgb_u8(80, 170, 255),
                Color::rgba_u8(0, 0, 0, 0),
                ShadowPadding::default(),
            );
        }

        let text_area = Area {
            left: info.area.left,
            top: info.area.top + MINIMAP_SIZE + 4.0,
            width: info.area.width,
            height: info.area.height - MINIMAP_SIZE - 4.0,
        };
        layout.add_text(
            text_area,
            state.get(&self.board_path).summary.as_str(),
            FontSize(14.0),
            Color::monochrome_u8(220),
            Color::rgb_u8(40, 40, 40),
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Top { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
    }
}

pub struct MvpMinimapWindow;

impl CustomWindow<ClientState> for MvpMinimapWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::MvpMinimap)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "MVP map",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 220.0,
            elements: (
                MvpMinimapView::new(client_state().mvp_minimap()),
            ),
        }
    }
}
