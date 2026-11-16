//! Attendance calendar (roBrowser `CheckAttendance`): one cell per day with
//! the reward icon, name and amount. Claimed days are filled, today outlined.

use std::sync::Arc;

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use ragnarok_packets::ItemId;
use rust_state::State;

use crate::graphics::{Color, CornerDiameter, ShadowPadding, Texture};
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{AsyncLoader, FontSize, ImageType, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::achievements::{AchievementsPathExt, AttendanceCellState};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};
use crate::world::{ItemResource, ItemResourceKey, Library, Table, inventory_icon_texture_path};

/// Days per calendar row.
pub const ATTENDANCE_COLUMNS: usize = 5;
const CELL_HEIGHT: f32 = 58.0;
const CELL_GAP: f32 = 4.0;
const ICON_SIZE: f32 = 24.0;

/// Ask for the reward icons the calendar still lacks. Icons already in the
/// texture cache land at once; the rest arrive with the item sprite event.
pub fn request_attendance_icons(state: &mut State<ClientState>, library: &Library, loader: &AsyncLoader) {
    let missing = state.follow(client_state().achievements()).missing_attendance_icons();
    for item_id in missing {
        let item = ItemId(item_id);
        let Some(resource) = ItemResource::try_get(library, ItemResourceKey {
            item_id: item,
            is_identified: true,
        }) else {
            continue;
        };
        let Ok(path) = inventory_icon_texture_path(resource.as_str()) else {
            continue;
        };
        if let Some(texture) = loader.request_item_sprite_load(item, resource.as_str(), &path, ImageType::Color) {
            state.follow_mut(client_state().achievements()).note_item_texture(item_id, texture);
        }
    }
}

struct AttendanceGrid;

struct AttendanceGridLayout {
    area: Area,
    cells: Vec<(crate::state::achievements::AttendanceCell, Option<Arc<Texture>>)>,
    corner: CornerDiameter,
    shadow: Color,
    shadow_padding: ShadowPadding,
    accent: Color,
    gold: Color,
    raised: Color,
    bg: Color,
    text: Color,
    highlight: Color,
}

impl Element<ClientState> for AttendanceGrid {
    type LayoutInfo = AttendanceGridLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let client = client_state();
            let achievements_path = client.achievements();
            let achievements = state.get(&achievements_path);
            let cells: Vec<_> = achievements
                .attendance_cells()
                .iter()
                .map(|cell| (cell.clone(), achievements.item_icon(cell.item_id)))
                .collect();
            let rows = cells.len().max(1).div_ceil(ATTENDANCE_COLUMNS);
            let area = resolver.with_height(rows as f32 * (CELL_HEIGHT + CELL_GAP));
            let theme_path = client_theme();
            let theme = state.get(&theme_path);
            AttendanceGridLayout {
                area,
                cells,
                corner: theme.button.corner_diameter,
                shadow: theme.button.shadow_color,
                shadow_padding: theme.button.shadow_padding,
                accent: theme.tokens.accent,
                gold: theme.tokens.gold,
                raised: theme.tokens.raised,
                bg: theme.tokens.bg,
                text: theme.tokens.text,
                highlight: theme.button.highlight_color,
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        _: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let area = layout_info.area;
        let width = (area.width - CELL_GAP * (ATTENDANCE_COLUMNS as f32 - 1.0)) / ATTENDANCE_COLUMNS as f32;
        let corner = layout_info.corner;
        let shadow = layout_info.shadow;
        let shadow_padding = layout_info.shadow_padding;
        let accent = layout_info.accent;
        let gold = layout_info.gold;
        let raised = layout_info.raised;
        let bg = layout_info.bg;
        let text = layout_info.text;
        let highlight = layout_info.highlight;

        for (index, (cell, icon)) in layout_info.cells.iter().enumerate() {
            let column = index % ATTENDANCE_COLUMNS;
            let row = index / ATTENDANCE_COLUMNS;
            let cell_area = Area {
                left: area.left + column as f32 * (width + CELL_GAP),
                top: area.top + row as f32 * (CELL_HEIGHT + CELL_GAP),
                width,
                height: CELL_HEIGHT,
            };
            // Claimed days use the accent fill, today the gold fill, later days
            // the raised panel color. All three are theme keys.
            let (background, foreground) = match cell.state {
                AttendanceCellState::Claimed => (accent, bg),
                AttendanceCellState::Today => (gold, bg),
                AttendanceCellState::Upcoming => (raised, text),
            };
            layout.add_rectangle(cell_area, corner, background, shadow, shadow_padding);
            let text_top = if let Some(texture) = icon {
                layout.add_texture(
                    Area {
                        left: cell_area.left + (cell_area.width - ICON_SIZE) / 2.0,
                        top: cell_area.top + 3.0,
                        width: ICON_SIZE,
                        height: ICON_SIZE,
                    },
                    texture.clone(),
                    Color::WHITE,
                    false,
                );
                ICON_SIZE + 3.0
            } else {
                0.0
            };
            layout.add_text(
                Area {
                    left: cell_area.left,
                    top: cell_area.top + text_top,
                    width: cell_area.width,
                    height: cell_area.height - text_top,
                },
                cell.label.as_str(),
                FontSize(11.0),
                foreground,
                highlight,
                HorizontalAlignment::Center { offset: 0.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        }
    }
}

pub struct AttendanceWindow;

impl CustomWindow<ClientState> for AttendanceWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Attendance)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Attendance",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 360.0,
            maximum_width: 520.0,
            elements: (
                text! {
                    text: client_state().achievements().attendance_status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                AttendanceGrid,
                button! {
                    text: "Claim today's reward",
                    event: InputEvent::RequestAttendance,
                },
            ),
        }
    }
}
