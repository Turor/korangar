//! Attendance calendar: one cell per day, claimed days filled, today outlined.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::State;

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::achievements::AttendanceCellState;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

/// Days per calendar row.
pub const ATTENDANCE_COLUMNS: usize = 5;
const CELL_HEIGHT: f32 = 46.0;
const CELL_GAP: f32 = 4.0;

struct AttendanceGrid;

struct AttendanceGridLayout {
    area: Area,
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
            let cells = state.get(&client_state().achievements()).attendance_cells().len().max(1);
            let rows = cells.div_ceil(ATTENDANCE_COLUMNS);
            let area = resolver.with_height(rows as f32 * (CELL_HEIGHT + CELL_GAP));
            AttendanceGridLayout { area }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let theme = state.get(&client_theme());
        let cells = state.get(&client_state().achievements()).attendance_cells();
        let area = layout_info.area;
        let width = (area.width - CELL_GAP * (ATTENDANCE_COLUMNS as f32 - 1.0)) / ATTENDANCE_COLUMNS as f32;
        let corner = theme.button.corner_diameter;
        let shadow = theme.button.shadow_color;
        let shadow_padding = theme.button.shadow_padding;

        for (index, cell) in cells.iter().enumerate() {
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
                AttendanceCellState::Claimed => (theme.tokens.accent, theme.tokens.bg),
                AttendanceCellState::Today => (theme.tokens.gold, theme.tokens.bg),
                AttendanceCellState::Upcoming => (theme.tokens.raised, theme.tokens.text),
            };
            layout.add_rectangle(cell_area, corner, background, shadow, shadow_padding);
            layout.add_text(
                cell_area,
                cell.label.as_str(),
                FontSize(11.0),
                foreground,
                theme.button.highlight_color,
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
