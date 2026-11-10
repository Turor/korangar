//! World map window.
//!
//! Textures are loaded from the GRF (`data/texture/유저인터페이스/…`) when the
//! window is open. Unlocked Warpra squares send `0x0EF4` action warp
//! immediately. Locked squares are dimmed and do not buy or confirm.

mod data;
mod logic;

use std::cell::Cell;
use std::sync::Arc;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, StateElement};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{ButtonThemePathExt, HorizontalAlignment, TextThemePathExt, VerticalAlignment, WindowThemePathExt};
use korangar_interface::window::{CustomWindow, Window};
pub use logic::should_close_after_result;
use rust_state::{Path, RustState, State};

use self::data::WorldMapSection;
use self::logic::{
    BASE_HEIGHT, BASE_WIDTH, ClickDecision, DEFAULT_EPISODE, FloorChoice, OverlayClass, entry_for_map, floor_click, hover_caption,
    map_thumbnail_path, overlay_class, region_texture_path, regions_for_episode, scale_rect, section_click, section_rect, section_ref,
    visible_sections,
};
use super::WindowClass;
use super::warpra::WarpraWindowStatePathExt;
use crate::graphics::{Color, CornerDiameter, ShadowPadding, Texture};
use crate::input::InputEvent;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::navigation::NavigationPathExt;
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme, this_player};

const RAIL_WIDTH: f32 = 168.0;
const RAIL_GAP: f32 = 8.0;
const CAPTION_HEIGHT: f32 = 72.0;
const REGION_BUTTON_HEIGHT: f32 = 28.0;
const WINDOW_WIDTH: f32 = 1080.0;
const WINDOW_HEIGHT: f32 = 900.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClickTarget {
    None,
    Region(u8),
    Section(u16),
    Floor(u16),
    CloseFloors,
}

/// Region, hover, and dungeon-floor picker for the world map.
#[derive(RustState, StateElement)]
pub struct WorldMapWindowState {
    episode: f32,
    region_index: u8,
    status: String,
    hover_caption: String,
    hover_map_id: String,
    show_floors: bool,
    floor_title: String,
    floors: Vec<FloorChoice>,
    loaded_region: String,
    loaded_hover: String,
    #[hidden_element]
    region_texture: Option<Arc<Texture>>,
    #[hidden_element]
    hover_texture: Option<Arc<Texture>>,
}

impl Default for WorldMapWindowState {
    fn default() -> Self {
        Self {
            episode: DEFAULT_EPISODE,
            region_index: 0,
            status: "World Map. Unlocked squares warp immediately. Locked squares stay closed.".to_owned(),
            hover_caption: String::new(),
            hover_map_id: String::new(),
            show_floors: false,
            floor_title: String::new(),
            floors: Vec::new(),
            loaded_region: String::new(),
            loaded_hover: String::new(),
            region_texture: None,
            hover_texture: None,
        }
    }
}

impl WorldMapWindowState {
    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn close_floors(&mut self) {
        self.show_floors = false;
        self.floors.clear();
        self.floor_title.clear();
    }

    pub fn on_catalog_cleared(&mut self) {
        self.close_floors();
        self.status = "Warpra catalog cleared.".to_owned();
    }

    pub fn prepare_open(&mut self) {
        self.close_floors();
        self.loaded_region.clear();
        self.loaded_hover.clear();
        self.region_texture = None;
        self.hover_texture = None;
    }

    pub fn apply_decision(&mut self, decision: ClickDecision) {
        match decision {
            ClickDecision::Waiting => {
                self.status = "Loading Warpra destinations…".to_owned();
            }
            ClickDecision::Missing { map_id } => {
                self.status = format!("No Warpra destination for {map_id}.");
            }
            ClickDecision::Ignore { reason } => {
                if !reason.is_empty() {
                    self.status = reason;
                }
            }
            ClickDecision::Warp { .. } => {
                self.close_floors();
                self.status = "Warping…".to_owned();
            }
            ClickDecision::OpenFloors { title, floors } => {
                self.floor_title = title;
                self.floors = floors;
                self.show_floors = true;
                self.status = "Choose an unlocked floor. Locked floors stay closed.".to_owned();
            }
        }
    }

    pub fn desired_region_texture(&self) -> String {
        let regions = regions_for_episode(self.episode);
        let region = regions
            .get(usize::from(self.region_index))
            .or_else(|| regions.first())
            .map(|region| region.id)
            .unwrap_or("worldmap.jpg");
        region_texture_path(region)
    }

    pub fn desired_hover_texture(&self) -> String {
        if self.hover_map_id.is_empty() {
            String::new()
        } else {
            map_thumbnail_path(&self.hover_map_id)
        }
    }

    pub fn loaded_region(&self) -> &str {
        &self.loaded_region
    }

    pub fn loaded_hover(&self) -> &str {
        &self.loaded_hover
    }

    pub fn holds_textures(&self) -> bool {
        self.region_texture.is_some() || self.hover_texture.is_some() || !self.loaded_region.is_empty() || !self.loaded_hover.is_empty()
    }

    pub fn install_region_texture(&mut self, path: String, texture: Option<Arc<Texture>>) {
        self.loaded_region = path;
        self.region_texture = texture;
    }

    pub fn install_hover_texture(&mut self, path: String, texture: Option<Arc<Texture>>) {
        self.loaded_hover = path;
        self.hover_texture = texture;
    }

    pub fn drop_textures(&mut self) {
        self.loaded_region.clear();
        self.loaded_hover.clear();
        self.region_texture = None;
        self.hover_texture = None;
    }
}

struct Metrics {
    height: f32,
    rail: Area,
    map: Area,
    caption: Area,
    thumb: Area,
}

fn metrics(width: f32) -> Metrics {
    let map_width = (width - RAIL_WIDTH - RAIL_GAP).max(200.0);
    let map_height = map_width * (BASE_HEIGHT / BASE_WIDTH);
    let height = map_height + RAIL_GAP + CAPTION_HEIGHT;
    Metrics {
        height,
        rail: Area {
            left: 0.0,
            top: 0.0,
            width: RAIL_WIDTH,
            height: map_height,
        },
        map: Area {
            left: RAIL_WIDTH + RAIL_GAP,
            top: 0.0,
            width: map_width,
            height: map_height,
        },
        caption: Area {
            left: 0.0,
            top: map_height + RAIL_GAP,
            width: (width - 80.0).max(120.0),
            height: CAPTION_HEIGHT,
        },
        thumb: Area {
            left: (width - 72.0).max(0.0),
            top: map_height + RAIL_GAP,
            width: 72.0,
            height: 72.0,
        },
    }
}

fn offset_area(origin: Area, local: Area) -> Area {
    Area {
        left: origin.left + local.left,
        top: origin.top + local.top,
        width: local.width,
        height: local.height,
    }
}

fn stroke(layout: &mut WindowLayout<ClientState>, area: Area, color: Color, thickness: f32) {
    let horizontal = thickness.min(area.height);
    let vertical = thickness.min(area.width);
    layout.add_rectangle(
        Area {
            left: area.left,
            top: area.top,
            width: area.width,
            height: horizontal,
        },
        CornerDiameter::default(),
        color,
        Color::TRANSPARENT,
        ShadowPadding::default(),
    );
    layout.add_rectangle(
        Area {
            left: area.left,
            top: area.top + area.height - horizontal,
            width: area.width,
            height: horizontal,
        },
        CornerDiameter::default(),
        color,
        Color::TRANSPARENT,
        ShadowPadding::default(),
    );
    layout.add_rectangle(
        Area {
            left: area.left,
            top: area.top,
            width: vertical,
            height: area.height,
        },
        CornerDiameter::default(),
        color,
        Color::TRANSPARENT,
        ShadowPadding::default(),
    );
    layout.add_rectangle(
        Area {
            left: area.left + area.width - vertical,
            top: area.top,
            width: vertical,
            height: area.height,
        },
        CornerDiameter::default(),
        color,
        Color::TRANSPARENT,
        ShadowPadding::default(),
    );
}

struct MapLayout {
    area: Area,
}

struct WorldMapView<W> {
    click: Cell<ClickTarget>,
    world_map: W,
}

impl<W> WorldMapView<W> {
    fn new(world_map: W) -> Self {
        Self {
            click: Cell::new(ClickTarget::None),
            world_map,
        }
    }
}

impl<W> ClickHandler<ClientState> for WorldMapView<W> {
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        let world_map = client_state().world_map_window();
        match self.click.get() {
            ClickTarget::None => {}
            ClickTarget::CloseFloors => {
                state.update_value_with(world_map, |window| window.close_floors());
            }
            ClickTarget::Region(index) => {
                state.update_value_with(world_map, move |window| {
                    window.region_index = index;
                    window.close_floors();
                    window.hover_map_id.clear();
                    window.hover_caption.clear();
                });
            }
            ClickTarget::Floor(index) => {
                let floors = state.get(&world_map.floors()).clone();
                if let Some(floor) = floors.get(usize::from(index)) {
                    let decision = Cell::new(Some(floor_click(floor)));
                    let warp = match decision.take() {
                        Some(ClickDecision::Warp { kind, location_id }) => {
                            decision.set(Some(ClickDecision::Warp { kind, location_id }));
                            Some((kind, location_id))
                        }
                        other => {
                            decision.set(other);
                            None
                        }
                    };
                    state.update_value_with(world_map, move |window| {
                        if let Some(decision) = decision.take() {
                            window.apply_decision(decision);
                        }
                    });
                    if let Some((kind, location_id)) = warp {
                        queue.queue(InputEvent::WorldMapWarp { kind, location_id });
                    }
                }
            }
            ClickTarget::Section(index) => {
                let episode = *state.get(&world_map.episode());
                let region_index = *state.get(&world_map.region_index());
                let entries = state.get(&client_state().warpra_window().destinations()).clone();
                let regions = regions_for_episode(episode);
                let Some(region) = regions.get(usize::from(region_index)).copied().or_else(|| regions.first().copied()) else {
                    return;
                };
                let sections: Vec<_> = visible_sections(region, episode).iter().copied().map(section_ref).collect();
                let decision = Cell::new(Some(section_click(&sections, &entries, usize::from(index))));
                let warp = match decision.take() {
                    Some(ClickDecision::Warp { kind, location_id }) => {
                        decision.set(Some(ClickDecision::Warp { kind, location_id }));
                        Some((kind, location_id))
                    }
                    other => {
                        decision.set(other);
                        None
                    }
                };
                state.update_value_with(world_map, move |window| {
                    if let Some(decision) = decision.take() {
                        window.apply_decision(decision);
                    }
                });
                if let Some((kind, location_id)) = warp {
                    queue.queue(InputEvent::WorldMapWarp { kind, location_id });
                }
            }
        }
    }
}

impl<W> Element<ClientState> for WorldMapView<W>
where
    W: Path<ClientState, WorldMapWindowState>,
{
    type LayoutInfo = MapLayout;

    fn create_layout_info(
        &mut self,
        _state: &State<ClientState>,
        _store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let width = resolver.push_available_area().width;
            let height = metrics(width).height;
            MapLayout {
                area: resolver.with_height(height),
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _store: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let window = state.get(&self.world_map);
        let metrics = metrics(layout_info.area.width);
        let rail = offset_area(layout_info.area, metrics.rail);
        let map = offset_area(layout_info.area, metrics.map);
        let caption = offset_area(layout_info.area, metrics.caption);
        let thumb = offset_area(layout_info.area, metrics.thumb);

        let border = *state.get(&client_theme().window().anchor_color());
        let gold = *state.get(&client_theme().window().closest_anchor_color());
        let accent = *state.get(&client_theme().window().highlight_color());
        let text_color = *state.get(&client_theme().text().color());
        let muted = *state.get(&client_theme().button().disabled_foreground_color());
        let panel = *state.get(&client_theme().window().background_color());
        let raised = *state.get(&client_theme().button().background_color());

        let regions = regions_for_episode(window.episode);
        let region_index = usize::from(window.region_index).min(regions.len().saturating_sub(1));
        let region = regions.get(region_index).copied();
        let sections: Vec<&WorldMapSection> = region.map(|region| visible_sections(region, window.episode)).unwrap_or_default();
        let destinations = client_state().warpra_window().destinations();
        let entries = state.get(&destinations);
        let player_level = state.try_follow(this_player()).map(|player| player.base_level as u32);
        let map_name = client_state().navigation().map_name();
        let current_map = state.get(&map_name);

        layout.add_rectangle(
            map,
            CornerDiameter::uniform(4.0),
            panel,
            Color::TRANSPARENT,
            ShadowPadding::default(),
        );
        if let Some(texture) = window.region_texture.clone() {
            layout.add_texture(map, texture, Color::WHITE, true);
        }
        stroke(layout, map, border, 2.0);

        let map_rect = logic::BaseRect {
            left: map.left,
            top: map.top,
            width: map.width,
            height: map.height,
        };
        let pointer_on_picker = window.show_floors && floor_panel_area(map).check().run(layout);
        let mut hovered_section: Option<usize> = None;
        if !pointer_on_picker {
            // Later sections paint on top, so the last rectangle under the pointer wins.
            for (index, section) in sections.iter().enumerate() {
                let scaled = scale_rect(map_rect, section_rect(section));
                if area_from_rect(scaled).check().run(layout) {
                    hovered_section = Some(index);
                }
            }
        }

        let mut click = ClickTarget::None;
        let section_views: Vec<_> = sections.iter().copied().map(section_ref).collect();
        for (index, section) in sections.iter().enumerate() {
            let scaled = scale_rect(map_rect, section_rect(section));
            let area = area_from_rect(scaled);
            let class = overlay_class(&section_views, entries, index);
            if class == OverlayClass::Locked {
                layout.add_rectangle(
                    area,
                    CornerDiameter::default(),
                    Color::rgba_u8(8, 14, 12, 150),
                    Color::TRANSPARENT,
                    ShadowPadding::default(),
                );
            }
            let current = logic::maps_match(section.id, current_map);
            let hovered = hovered_section == Some(index);
            if hovered || current {
                let color = if hovered { gold } else { accent };
                stroke(layout, area, color, 2.0);
            }
            if hovered {
                let catalog_name = entry_for_map(entries, section.id).map(|entry| entry.name.as_str());
                let caption = hover_caption(&section_views[index], class, catalog_name, player_level);
                let map_id = logic::normalize_map_id(section.id);
                if state.get(&self.world_map.hover_caption()) != &caption {
                    state.update_value(self.world_map.hover_caption(), caption);
                }
                if state.get(&self.world_map.hover_map_id()) != &map_id {
                    state.update_value(self.world_map.hover_map_id(), map_id);
                }
                if class != OverlayClass::Locked {
                    click = ClickTarget::Section(index as u16);
                }
            }
        }
        if hovered_section.is_none() && !state.get(&self.world_map.hover_map_id()).is_empty() && !window.show_floors {
            state.update_value(self.world_map.hover_caption(), String::new());
            state.update_value(self.world_map.hover_map_id(), String::new());
        }

        for (index, region) in regions.iter().enumerate() {
            let button = Area {
                left: rail.left,
                top: rail.top + index as f32 * (REGION_BUTTON_HEIGHT + 4.0),
                width: rail.width,
                height: REGION_BUTTON_HEIGHT,
            };
            let hovered = button.check().run(layout);
            let selected = index == region_index;
            let background = if selected {
                accent
            } else if hovered {
                gold
            } else {
                raised
            };
            layout.add_rectangle(
                button,
                CornerDiameter::uniform(6.0),
                background,
                Color::TRANSPARENT,
                ShadowPadding::default(),
            );
            let foreground = if selected || hovered {
                Color::rgb_u8(15, 23, 20)
            } else {
                text_color
            };
            layout.add_text(
                button,
                region.name,
                FontSize(13.0),
                foreground,
                gold,
                HorizontalAlignment::Center { offset: 0.0, border: 4.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if hovered {
                click = ClickTarget::Region(index as u8);
            }
        }

        if window.show_floors {
            click = layout_floor_picker(layout, map, window, border, gold, text_color, muted, panel, click);
        }

        let line = if window.hover_caption.is_empty() {
            window.status.as_str()
        } else {
            window.hover_caption.as_str()
        };
        layout.add_text(
            caption,
            line,
            FontSize(13.0),
            text_color,
            gold,
            HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
            VerticalAlignment::Top { offset: 2.0 },
            OverflowBehavior::LineBreak,
        );
        layout.add_rectangle(
            thumb,
            CornerDiameter::uniform(4.0),
            border,
            Color::TRANSPARENT,
            ShadowPadding::default(),
        );
        if let Some(texture) = window.hover_texture.clone() {
            layout.add_texture(thumb, texture, Color::WHITE, false);
        }

        self.click.set(click);
        if click != ClickTarget::None {
            layout.register_click_handler(MouseButton::Left, self);
        }
    }
}

fn area_from_rect(rect: logic::BaseRect) -> Area {
    Area {
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
    }
}

fn floor_panel_area(map: Area) -> Area {
    let panel_width = 300.0_f32.min(map.width * 0.48);
    Area {
        left: map.left + map.width - panel_width - 8.0,
        top: map.top + 8.0,
        width: panel_width,
        height: map.height - 16.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_floor_picker<'a>(
    layout: &mut WindowLayout<'a, ClientState>,
    map: Area,
    window: &'a WorldMapWindowState,
    border: Color,
    gold: Color,
    text_color: Color,
    muted: Color,
    panel: Color,
    mut click: ClickTarget,
) -> ClickTarget {
    let panel_area = floor_panel_area(map);
    layout.add_rectangle(
        panel_area,
        CornerDiameter::uniform(8.0),
        panel,
        Color::rgba_u8(0, 0, 0, 80),
        ShadowPadding::uniform(2.0),
    );
    stroke(layout, panel_area, border, 2.0);
    let title_area = Area {
        left: panel_area.left + 6.0,
        top: panel_area.top + 4.0,
        width: panel_area.width - 64.0,
        height: 22.0,
    };
    let close_area = Area {
        left: panel_area.left + panel_area.width - 56.0,
        top: panel_area.top + 4.0,
        width: 48.0,
        height: 22.0,
    };
    layout.add_text(
        title_area,
        window.floor_title.as_str(),
        FontSize(13.0),
        text_color,
        gold,
        HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
    let close_hovered = close_area.check().run(layout);
    layout.add_text(
        close_area,
        "Close",
        FontSize(13.0),
        if close_hovered { gold } else { text_color },
        gold,
        HorizontalAlignment::Right { offset: 4.0, border: 2.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
    if close_hovered {
        click = ClickTarget::CloseFloors;
    }

    let rows = window.floors.len().max(1);
    let row_height = ((panel_area.height - 36.0) / rows as f32).clamp(16.0, 24.0);
    for (index, floor) in window.floors.iter().enumerate() {
        let row = Area {
            left: panel_area.left + 6.0,
            top: panel_area.top + 30.0 + index as f32 * row_height,
            width: panel_area.width - 12.0,
            height: row_height - 2.0,
        };
        if row.top + row.height > panel_area.top + panel_area.height {
            break;
        }
        let hovered = row.check().run(layout);
        let color = if !floor.unlocked {
            muted
        } else if hovered {
            gold
        } else {
            text_color
        };
        if hovered && floor.unlocked {
            layout.add_rectangle(
                row,
                CornerDiameter::uniform(4.0),
                Color::rgba_u8(207, 174, 90, 40),
                Color::TRANSPARENT,
                ShadowPadding::default(),
            );
            click = ClickTarget::Floor(index as u16);
        } else if !floor.unlocked {
            layout.add_rectangle(
                row,
                CornerDiameter::uniform(4.0),
                Color::rgba_u8(8, 14, 12, 90),
                Color::TRANSPARENT,
                ShadowPadding::default(),
            );
        }
        layout.add_text(
            row,
            floor.name.as_str(),
            FontSize(12.0),
            color,
            gold,
            HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
    }
    click
}

pub struct WorldMapWindow;

impl WorldMapWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for WorldMapWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::WorldMap)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "World Map",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: false,
            minimum_width: WINDOW_WIDTH,
            maximum_width: WINDOW_WIDTH,
            minimum_height: WINDOW_HEIGHT,
            maximum_height: WINDOW_HEIGHT,
            elements: (WorldMapView::new(client_state().world_map_window()),),
        }
    }
}
