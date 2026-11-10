use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, StateElement};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::minimap_point;
use ragnarok_packets::{AccountId, MvpMinimapEntry, TilePosition};
use rust_state::{Path, RustState, State};

use super::WindowClass;
use crate::state::theme::{ThemeTokensPathExt};
use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::theme::InterfaceThemePathExt;
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme, this_entity};

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
    /// Visible window title: the map display name, falling back to `iz_int`.
    title: String,
    /// "Prontera (132, 98)" — rewritten as the player walks.
    caption: String,
    player_x: u16,
    player_y: u16,
    /// Map id from the last map change, matched against party member maps.
    map_id: String,
}

impl Default for MvpMinimapState {
    fn default() -> Self {
        Self {
            flags: 0,
            map_width: 0,
            map_height: 0,
            markers: Vec::new(),
            summary: String::new(),
            title: "Minimap".to_owned(),
            caption: String::new(),
            player_x: 0,
            player_y: 0,
            map_id: String::new(),
        }
    }
}

impl MvpMinimapState {
    pub fn replace(&mut self, flags: u8, markers: Vec<MvpMinimapEntry>) {
        self.flags = flags;
        self.summary = if markers.is_empty() {
            String::new()
        } else {
            korangar_networking::format_mvp_markers(&markers)
        };
        self.markers = markers;
        self.refresh_caption();
    }

    pub fn clear_markers(&mut self) {
        self.flags = 0;
        self.markers.clear();
        self.summary = String::new();
        self.refresh_caption();
    }

    pub fn set_map_size(&mut self, width: u16, height: u16) {
        self.map_width = width;
        self.map_height = height;
    }

    pub fn set_display_name(&mut self, display_name: impl Into<String>) {
        let display_name = display_name.into();
        self.title = display_name;
        self.refresh_caption();
    }

    pub fn set_map_id(&mut self, map_id: &str) {
        self.map_id.clear();
        self.map_id.push_str(map_id);
    }

    pub fn set_player_cell(&mut self, x: u16, y: u16) {
        self.player_x = x;
        self.player_y = y;
        self.refresh_caption();
    }

    fn refresh_caption(&mut self) {
        let head = format!("{} ({}, {})", self.title, self.player_x, self.player_y);
        self.caption = if self.summary.is_empty() {
            head
        } else {
            format!("{head}\n{}", self.summary)
        };
    }
}

pub struct MinimapLayout {
    area: Area,
    mvp_dots: Vec<(f32, f32)>,
    party_dots: Vec<(f32, f32)>,
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
        let (map_width, map_height, mvp_dots, extra_lines) = {
            let board = state.get(&self.board_path);
            let mvp_dots = board
                .markers
                .iter()
                .filter_map(|marker| minimap_point(board.map_width, board.map_height, marker.position, MINIMAP_SIZE))
                .collect();
            let extra_lines = if board.summary.is_empty() {
                1.0
            } else {
                board.summary.lines().count().max(1) as f32 + 1.0
            };
            (board.map_width, board.map_height, mvp_dots, extra_lines)
        };
        let party_dots = {
            let board = state.get(&self.board_path);
            let positions = state.get(&root.party()).minimap_positions(&board.map_id);
            party_minimap_dots(&positions, map_width, map_height, MINIMAP_SIZE)
        };
        let player_dot = this_entity()
            .follow(state.get(&root))
            .and_then(|player| minimap_point(map_width, map_height, player.get_tile_position(), MINIMAP_SIZE));
        let area = with_single_resolver(resolvers, |resolver| {
            resolver.with_height(MINIMAP_SIZE + 8.0 + extra_lines * 16.0)
        });
        MinimapLayout {
            area,
            mvp_dots,
            party_dots,
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
        let token_path = client_theme().tokens();
        let tokens = state.get(&token_path);
        let canvas = Area {
            left: info.area.left,
            top: info.area.top,
            width: MINIMAP_SIZE,
            height: MINIMAP_SIZE,
        };
        layout.add_rectangle(
            canvas,
            CornerDiameter::uniform(4.0),
            tokens.raised,
            state.get(&client_theme().tokens().bg()).with_alpha_u8(80),
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
                tokens.accent2,
                Color::rgba_u8(0, 0, 0, 0),
                ShadowPadding::default(),
            );
        }

        // Party members from `0x0107`, under the local player's dot.
        for (x, y) in &info.party_dots {
            layout.add_rectangle(
                Area {
                    left: canvas.left + x - DOT / 2.0,
                    top: canvas.top + y - DOT / 2.0,
                    width: DOT,
                    height: DOT,
                },
                CornerDiameter::uniform(DOT / 2.0),
                tokens.hp,
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
                tokens.accent,
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
        let board = state.get(&self.board_path);
        layout.add_text(
            text_area,
            board.caption.as_str(),
            FontSize(14.0),
            tokens.text,
            tokens.gold,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Top { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
    }
}

/// Canvas points for party member cells. Cells outside the map are dropped.
fn party_minimap_dots(positions: &[(AccountId, i16, i16)], map_width: u16, map_height: u16, size: f32) -> Vec<(f32, f32)> {
    positions
        .iter()
        .filter_map(|&(_, x, y)| {
            let position = TilePosition {
                x: u16::try_from(x).ok()?,
                y: u16::try_from(y).ok()?,
            };
            minimap_point(map_width, map_height, position, size)
        })
        .collect()
}

pub struct MvpMinimapWindow;

impl CustomWindow<ClientState> for MvpMinimapWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::MvpMinimap)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: client_state().mvp_minimap().title(),
            class: Self::window_class(),
            theme: crate::state::theme::InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 220.0,
            elements: (
                MvpMinimapView::new(client_state().mvp_minimap()),
            ),
        }
    }
}

#[cfg(test)]
mod party_dot_tests {
    use ragnarok_packets::AccountId;

    use super::party_minimap_dots;

    #[test]
    fn party_cells_map_onto_the_canvas() {
        let dots = party_minimap_dots(&[(AccountId(2), 0, 0), (AccountId(3), 99, 99)], 100, 100, 100.0);
        assert_eq!(dots, vec![(0.5, 99.5), (99.5, 0.5)]);
    }

    #[test]
    fn party_cells_off_the_map_are_dropped() {
        let dots = party_minimap_dots(&[(AccountId(2), -1, 5), (AccountId(3), 100, 5)], 100, 100, 100.0);
        assert!(dots.is_empty());
        assert!(party_minimap_dots(&[(AccountId(2), 5, 5)], 0, 0, 100.0).is_empty());
    }
}
