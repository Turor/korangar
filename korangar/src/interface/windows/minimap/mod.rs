//! Always-on minimap. The map image comes from `유저인터페이스\map\<map>.bmp`.
//! A missing file keeps the themed square. The window has no close button.

mod logic;

use std::cell::Cell;
use std::sync::Arc;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, StateElement};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
#[cfg(test)]
use ragnarok_packets::AccountId;
use ragnarok_packets::{Direction, MvpMinimapEntry};
use rust_state::{Path, RustState, State};

use self::logic::{arrow_boxes, plot_cell};
pub(crate) use self::logic::{image_needs_load, map_window, missing_image_log, plot_marks, step_zoom};
use super::WindowClass;
use super::menu_bar::{MenuBarPress, menu_bar_entries, press_menu_bar};
use crate::graphics::{Color, CornerDiameter, ShadowPadding, Texture};
use crate::input::InputEvent;
use crate::interface::font_scale::from_theme_body;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::navigation::normalize_map_name;
use crate::state::theme::InterfaceThemePathExt;
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme, this_entity};

/// Square minimap in interface pixels.
pub(crate) const MINIMAP_SIZE: f32 = 180.0;
const DOT: f32 = 6.0;
const BUTTON: f32 = 16.0;

/// The `window!` macro only accepts a literal here. This constant is the same
/// value so tests can see that Escape and the menu bar must not close it.
pub const MINIMAP_CLOSABLE: bool = false;

/// Last `0x0EF5` snapshot, the loaded map, and the minimap image.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct MvpMinimapState {
    flags: u8,
    map_width: u16,
    map_height: u16,
    markers: Vec<MvpMinimapEntry>,
    summary: String,
    /// Visible window title: the map display name, falling back to the map id.
    title: String,
    /// "Prontera (132, 98)" — rewritten as the player walks.
    caption: String,
    player_x: u16,
    player_y: u16,
    /// Map id from the last map change, matched against party member maps.
    map_id: String,
    /// `map\<id>.bmp` last settled (ready or confirmed missing).
    #[hidden_element]
    loaded_image: String,
    #[hidden_element]
    texture_missing: bool,
    /// Map files already logged as missing. A return trip does not log again.
    #[hidden_element]
    logged_missing: Vec<String>,
    #[hidden_element]
    map_texture: Option<Arc<Texture>>,
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
            loaded_image: String::new(),
            texture_missing: false,
            logged_missing: Vec::new(),
            map_texture: None,
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
        if self.map_id != map_id {
            self.loaded_image.clear();
            self.map_texture = None;
            self.texture_missing = false;
        }
        self.map_id.clear();
        self.map_id.push_str(map_id);
    }

    pub fn set_player_cell(&mut self, x: u16, y: u16) {
        self.player_x = x;
        self.player_y = y;
        self.refresh_caption();
    }

    pub fn caption(&self) -> &str {
        &self.caption
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn player_cell(&self) -> (u16, u16) {
        (self.player_x, self.player_y)
    }

    pub fn map_id(&self) -> &str {
        &self.map_id
    }

    pub fn map_size(&self) -> (u16, u16) {
        (self.map_width, self.map_height)
    }

    /// File passed to the interface texture loader, relative to the interface
    /// directory.
    pub fn image_file(&self) -> String {
        let name = normalize_map_name(&self.map_id);
        if name.is_empty() {
            return String::new();
        }
        format!("map\\{name}.bmp")
    }

    pub fn loaded_image(&self) -> &str {
        &self.loaded_image
    }

    pub fn texture_missing(&self) -> bool {
        self.texture_missing
    }

    /// No image is installed, so the themed square shows through.
    #[cfg(test)]
    pub fn shows_flat_square(&self) -> bool {
        self.map_texture.is_none()
    }

    /// Remember a confirmed miss. `true` the first time this file is logged.
    pub fn record_missing(&mut self, file: &str) -> bool {
        self.loaded_image = file.to_owned();
        self.texture_missing = true;
        self.map_texture = None;
        if self.logged_missing.iter().any(|known| known == file) {
            return false;
        }
        self.logged_missing.push(file.to_owned());
        true
    }

    pub fn record_ready(&mut self, file: &str) {
        self.loaded_image = file.to_owned();
        self.texture_missing = false;
    }

    pub fn install_image(&mut self, texture: Arc<Texture>) {
        self.map_texture = Some(texture);
    }

    /// A local file was present but did not read. Leave the path unset so the
    /// next frame retries.
    pub fn note_image_retry(&mut self) {
        self.map_texture = None;
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

struct MarkerColors {
    raised: Color,
    border: Color,
    text: Color,
    muted: Color,
    accent: Color,
    accent2: Color,
    hover: Color,
    hp: Color,
    gold: Color,
    font_size: f32,
    shadow: Color,
}

pub struct MinimapLayout {
    area: Area,
    mvp_dots: Vec<(f32, f32)>,
    party_dots: Vec<(f32, f32)>,
    player: Option<(f32, f32)>,
    facing: Direction,
    marks: Vec<logic::PlottedMark>,
    origin_x: f32,
    origin_y: f32,
    span: f32,
}

/// Draws the loaded map as a square and plots marks, party, MVPs, and the
/// player.
pub struct MvpMinimapView<A> {
    board_path: A,
    zoom_delta: Cell<i8>,
}

impl<A> MvpMinimapView<A> {
    pub fn new(board_path: A) -> Self {
        Self {
            board_path,
            zoom_delta: Cell::new(0),
        }
    }
}

impl<A> ClickHandler<ClientState> for MvpMinimapView<A> {
    fn handle_click(&self, state: &State<ClientState>, _queue: &mut EventQueue<ClientState>) {
        let delta = self.zoom_delta.get();
        if delta == 0 {
            return;
        }
        state.update_value_with(client_state().interface_settings(), move |settings| {
            settings.minimap_zoom = step_zoom(settings.minimap_zoom, delta);
        });
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
        let zoom = state.get(&root.interface_settings()).minimap_zoom;
        let (map_width, map_height, board_x, board_y, map_id, markers, extra_lines) = {
            let board = state.get(&self.board_path);
            let extra_lines = if board.summary.is_empty() {
                1.0
            } else {
                board.summary.lines().count().max(1) as f32 + 1.0
            };
            (
                board.map_width,
                board.map_height,
                board.player_x,
                board.player_y,
                board.map_id.clone(),
                board.markers.clone(),
                extra_lines,
            )
        };
        let (cell_x, cell_y, facing) = match this_entity().follow(state.get(&root)) {
            Some(player) => {
                let tile = player.get_tile_position();
                (tile.x, tile.y, player.get_direction())
            }
            None => (board_x, board_y, Direction::North),
        };
        let window = map_window(zoom, map_width, map_height, cell_x, cell_y);
        let mvp_dots = markers
            .iter()
            .filter_map(|marker| {
                plot_cell(
                    window,
                    map_width,
                    map_height,
                    marker.position.x,
                    marker.position.y,
                    MINIMAP_SIZE,
                )
            })
            .collect();
        let party_dots = {
            let positions = state.get(&root.party()).minimap_positions(&map_id);
            positions
                .iter()
                .filter_map(|&(_, x, y)| {
                    let x = u16::try_from(x).ok()?;
                    let y = u16::try_from(y).ok()?;
                    plot_cell(window, map_width, map_height, x, y, MINIMAP_SIZE)
                })
                .collect()
        };
        let player = plot_cell(window, map_width, map_height, cell_x, cell_y, MINIMAP_SIZE);
        let marks = plot_marks(
            state.get(&root.navigation()).marks(),
            window,
            map_width,
            map_height,
            MINIMAP_SIZE,
        );
        let area = with_single_resolver(resolvers, |resolver| {
            resolver.with_height(MINIMAP_SIZE + 8.0 + extra_lines * 16.0)
        });
        MinimapLayout {
            area,
            mvp_dots,
            party_dots,
            player,
            facing,
            marks,
            origin_x: window.origin_x,
            origin_y: window.origin_y,
            span: window.span,
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
        let colors = MarkerColors {
            raised: tokens.raised,
            border: tokens.border,
            text: tokens.text,
            muted: tokens.muted,
            accent: tokens.accent,
            accent2: tokens.accent2,
            hover: tokens.hover,
            hp: tokens.hp,
            gold: tokens.gold,
            font_size: tokens.font_size,
            shadow: tokens.bg.with_alpha_u8(80),
        };
        let zoom = state.get(&client_state().interface_settings()).minimap_zoom;
        let board = state.get(&self.board_path);
        let texture = board.map_texture.clone();
        let caption = board.caption.as_str();

        let canvas = Area {
            left: info.area.left,
            top: info.area.top,
            width: MINIMAP_SIZE,
            height: MINIMAP_SIZE,
        };
        layout.add_rectangle(
            canvas,
            CornerDiameter::uniform(4.0),
            colors.raised,
            colors.shadow,
            ShadowPadding::diagonal(1.0, 2.0),
        );

        let image_window = logic::MapWindow {
            origin_x: info.origin_x,
            origin_y: info.origin_y,
            span: info.span,
        };
        layout.with_clip(canvas, |layout| {
            if let Some(texture) = texture.clone() {
                let (left, top, width, height) = logic::zoomed_image_area(canvas.left, canvas.top, MINIMAP_SIZE, image_window);
                layout.add_texture(Area { left, top, width, height }, texture, Color::WHITE, true);
            }
            // Rectangles in this layer draw before the texture, so markers use the next
            // layer.
            layout.with_layer(|layout| {
                for mark in &info.marks {
                    paint_dot(
                        layout,
                        canvas,
                        mark.x,
                        mark.y,
                        DOT + 2.0,
                        Color::rgb_u8(mark.red, mark.green, mark.blue),
                        colors.border,
                    );
                }
                for (x, y) in &info.party_dots {
                    paint_dot(layout, canvas, *x, *y, DOT, colors.hp, colors.border);
                }
                for (x, y) in &info.mvp_dots {
                    paint_dot(layout, canvas, *x, *y, DOT, colors.accent2, colors.border);
                }
                if let Some((x, y)) = info.player {
                    for piece in arrow_boxes(info.facing) {
                        paint_dot(
                            layout,
                            canvas,
                            x + piece.dx,
                            y + piece.dy,
                            piece.size,
                            colors.accent,
                            colors.border,
                        );
                    }
                }
                paint_zoom_buttons(layout, canvas, zoom, &colors, &self.zoom_delta, self);
            });
        });

        let text_area = Area {
            left: info.area.left,
            top: info.area.top + MINIMAP_SIZE + 4.0,
            width: info.area.width,
            height: info.area.height - MINIMAP_SIZE - 4.0,
        };
        layout.add_text(
            text_area,
            caption,
            FontSize(from_theme_body(colors.font_size, 14.0)),
            colors.text,
            colors.gold,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Top { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
    }
}

fn paint_dot(layout: &mut WindowLayout<ClientState>, canvas: Area, x: f32, y: f32, size: f32, fill: Color, outline: Color) {
    let outline_size = size + 2.0;
    layout.add_rectangle(
        centered(canvas, x, y, outline_size),
        CornerDiameter::uniform(outline_size / 2.0),
        outline,
        Color::TRANSPARENT,
        ShadowPadding::default(),
    );
    layout.add_rectangle(
        centered(canvas, x, y, size),
        CornerDiameter::uniform(size / 2.0),
        fill,
        Color::TRANSPARENT,
        ShadowPadding::default(),
    );
}

fn centered(canvas: Area, x: f32, y: f32, size: f32) -> Area {
    Area {
        left: canvas.left + x - size / 2.0,
        top: canvas.top + y - size / 2.0,
        width: size,
        height: size,
    }
}

fn paint_zoom_buttons<'a, A>(
    layout: &mut WindowLayout<'a, ClientState>,
    canvas: Area,
    zoom: u8,
    colors: &MarkerColors,
    zoom_delta: &Cell<i8>,
    handler: &'a MvpMinimapView<A>,
) where
    A: Path<ClientState, MvpMinimapState>,
{
    let plus = Area {
        left: canvas.left + canvas.width - BUTTON - 4.0,
        top: canvas.top + canvas.height - BUTTON - 4.0,
        width: BUTTON,
        height: BUTTON,
    };
    let minus = Area {
        left: plus.left - BUTTON - 2.0,
        top: plus.top,
        width: BUTTON,
        height: BUTTON,
    };
    let minus_enabled = zoom > 0;
    let plus_enabled = zoom + 1 < logic::ZOOM_LEVEL_COUNT;
    let minus_hover = minus.check().run(layout);
    let plus_hover = plus.check().run(layout);
    let delta = if minus_hover && minus_enabled {
        -1
    } else if plus_hover && plus_enabled {
        1
    } else {
        0
    };
    zoom_delta.set(delta);
    if delta != 0 {
        layout.register_click_handler(MouseButton::Left, handler);
    }
    let font = FontSize(from_theme_body(colors.font_size, 12.0));
    paint_zoom_button(layout, minus, minus_hover, minus_enabled, "-", colors, font);
    paint_zoom_button(layout, plus, plus_hover, plus_enabled, "+", colors, font);
}

fn paint_zoom_button<'a>(
    layout: &mut WindowLayout<'a, ClientState>,
    area: Area,
    hovered: bool,
    enabled: bool,
    label: &'a str,
    colors: &MarkerColors,
    font: FontSize,
) {
    let fill = if !enabled {
        colors.muted
    } else if hovered {
        colors.hover
    } else {
        colors.raised
    };
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(3.0),
        colors.border,
        Color::TRANSPARENT,
        ShadowPadding::default(),
    );
    let inset = Area {
        left: area.left + 1.0,
        top: area.top + 1.0,
        width: (area.width - 2.0).max(1.0),
        height: (area.height - 2.0).max(1.0),
    };
    layout.add_rectangle(
        inset,
        CornerDiameter::uniform(2.0),
        fill,
        Color::TRANSPARENT,
        ShadowPadding::default(),
    );
    layout.add_text(
        area,
        label,
        font,
        colors.text,
        Color::TRANSPARENT,
        HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
}

/// Canvas points for party member cells at zoom 0. Cells outside the map are
/// dropped.
#[cfg(test)]
fn party_minimap_dots(positions: &[(AccountId, i16, i16)], map_width: u16, map_height: u16, size: f32) -> Vec<(f32, f32)> {
    let window = map_window(0, map_width, map_height, 0, 0);
    positions
        .iter()
        .filter_map(|&(_, x, y)| {
            let x = u16::try_from(x).ok()?;
            let y = u16::try_from(y).ok()?;
            plot_cell(window, map_width, map_height, x, y, size)
        })
        .collect()
}

/// `true` when a menu-bar press would remove the minimap window.
pub fn menu_bar_closes_minimap(minimap_open: bool) -> bool {
    for (index, entry) in menu_bar_entries().iter().enumerate() {
        if entry.class != WindowClass::MvpMinimap {
            continue;
        }
        let Some(press) = press_menu_bar(index, |class| minimap_open && class == WindowClass::MvpMinimap) else {
            continue;
        };
        match press {
            MenuBarPress::Close(WindowClass::MvpMinimap) => return true,
            MenuBarPress::Close(_) => {}
            MenuBarPress::CloseAll(classes) if classes.contains(&WindowClass::MvpMinimap) => return true,
            MenuBarPress::CloseAll(_) => {}
            MenuBarPress::Dispatch(event) if matches!(event.as_ref(), InputEvent::ToggleMvpMinimap) => {}
            MenuBarPress::Dispatch(_) => {}
            MenuBarPress::OpenGroup(_) => {}
        }
    }
    false
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
            closable: false,
            minimum_width: 220.0,
            elements: (
                MvpMinimapView::new(client_state().mvp_minimap()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::AccountId;

    use super::{MvpMinimapState, party_minimap_dots};

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

    #[test]
    fn missing_texture_keeps_the_flat_square_and_logs_once_per_map() {
        let mut board = MvpMinimapState::default();
        board.set_map_id("prontera.gat");
        let file = board.image_file();
        assert_eq!(file, "map\\prontera.bmp");
        assert!(super::image_needs_load(&file, board.loaded_image()));
        assert!(board.record_missing(&file));
        assert!(!board.record_missing(&file));
        assert!(board.shows_flat_square());
        assert!(board.texture_missing());
        assert!(!super::image_needs_load(&file, board.loaded_image()));
        assert_eq!(
            super::missing_image_log(&file),
            "minimap: no image for map\\prontera.bmp; keeping the flat square"
        );

        board.set_map_id("geffen.gat");
        assert!(board.loaded_image().is_empty());
        assert!(!board.texture_missing());
        board.set_map_id("prontera.gat");
        assert!(!board.record_missing(&file));

        board.record_ready(&file);
        assert!(!board.texture_missing());
        assert!(board.shows_flat_square());
        board.set_map_id("prontera.gat");
        assert_eq!(board.loaded_image(), file);
    }

    #[test]
    fn caption_keeps_the_map_name_and_cell() {
        let mut board = MvpMinimapState::default();
        board.set_display_name("Prontera");
        board.set_player_cell(132, 98);
        assert_eq!(board.caption(), "Prontera (132, 98)");
        assert_eq!(board.player_cell(), (132, 98));
    }
}
