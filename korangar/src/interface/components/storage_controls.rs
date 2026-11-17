//! The storage tool row of the unified inventory: saved-search chips
//! (inventory review #18) and the Save / Deposit / Close buttons on one line,
//! so the split keeps its item rows. Click a chip to apply its search;
//! right-click it to forget it. The last tool status replaces the
//! "No saved searches" hint and fills what the chips leave free.

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{ButtonThemePathExt, HorizontalAlignment, VerticalAlignment};
use korangar_interface::theme::{ThemePathGetter, theme};
use rust_state::{Path, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::interface::components::unified_inventory::storage_tool_row_height;
use crate::interface::font_scale::from_theme_body;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::inventory::Inventory;
use crate::state::storage_tools::{MAX_SAVED_SEARCHES, StorageTools};
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::{ClientState, client_theme};

const CHIP_MAX_WIDTH: f32 = 120.0;
const ACTION_GAP: f32 = 4.0;

/// The row's buttons, right to left of the chips.
const ACTIONS: [(&str, ToolAction); 3] = [
    ("Save search", ToolAction::Save),
    ("Deposit", ToolAction::Deposit),
    ("Close", ToolAction::Close),
];

#[derive(Clone, Copy)]
enum ToolAction {
    Save,
    Deposit,
    Close,
}

impl ClickHandler<ClientState> for ToolAction {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(match self {
            ToolAction::Save => InputEvent::SaveStorageSearch,
            ToolAction::Deposit => InputEvent::DepositMatchingToStorage,
            ToolAction::Close => InputEvent::CloseStorage,
        });
    }
}

/// Width of one action button: a sixth of the row, kept readable.
fn action_width(row_width: f32) -> f32 {
    (row_width / 6.0).clamp(44.0, 84.0)
}

struct ChipClick<T, S> {
    tools_path: T,
    storage_path: S,
    slot: usize,
    forget: bool,
}

impl<T, S> ClickHandler<ClientState> for ChipClick<T, S>
where
    T: Path<ClientState, StorageTools> + Copy + 'static,
    S: Path<ClientState, Inventory> + Copy + 'static,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        let Some(query) = state.get(&self.tools_path).saved().get(self.slot).cloned() else {
            return;
        };
        match self.forget {
            true => queue.queue(InputEvent::ForgetStorageSearch { query }),
            false => state.update_value_with(self.storage_path, move |storage: &mut Inventory| {
                storage.view_mut().set_search(query.clone());
            }),
        }
    }
}

pub struct SavedSearchStrip<T, S> {
    tools_path: T,
    storage_path: S,
    apply: Vec<ChipClick<T, S>>,
    forget: Vec<ChipClick<T, S>>,
}

impl<T: Copy, S: Copy> SavedSearchStrip<T, S> {
    pub fn new(tools_path: T, storage_path: S) -> Self {
        let chip = |slot, forget| ChipClick {
            tools_path,
            storage_path,
            slot,
            forget,
        };
        Self {
            tools_path,
            storage_path,
            apply: (0..MAX_SAVED_SEARCHES).map(|slot| chip(slot, false)).collect(),
            forget: (0..MAX_SAVED_SEARCHES).map(|slot| chip(slot, true)).collect(),
        }
    }
}

impl<T, S> Element<ClientState> for SavedSearchStrip<T, S>
where
    T: Path<ClientState, StorageTools> + Copy + 'static,
    S: Path<ClientState, Inventory> + Copy + 'static,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let tab = (*state.get(&theme().button().height())).max(16.0);
        with_single_resolver(resolvers, |resolver| BaseLayoutInfo {
            area: resolver.with_height(storage_tool_row_height(tab)),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let tokens = client_theme().tokens();
        let font = FontSize(from_theme_body(*state.get(&tokens.font_size()), 12.0));
        let tools = state.get(&self.tools_path);
        let saved = tools.saved();
        let status = tools.status();
        let row = layout_info.area;
        let none = Color::rgba_u8(0, 0, 0, 0);
        let flat = ShadowPadding::diagonal(0.0, 0.0);
        let text = *state.get(&tokens.text());
        let muted = *state.get(&tokens.muted());

        // Buttons on the right.
        let button_width = action_width(row.width);
        let buttons_width = ACTIONS.len() as f32 * (button_width + ACTION_GAP);
        for (slot, (label, action)) in ACTIONS.iter().enumerate() {
            let cell = Area {
                left: row.left + row.width - buttons_width + ACTION_GAP + slot as f32 * (button_width + ACTION_GAP),
                width: button_width,
                ..row
            };
            let hovered = cell.check().run(layout);
            let outline = match hovered {
                true => *state.get(&tokens.accent_mid()),
                false => *state.get(&tokens.accent_dim()),
            };
            layout.add_rectangle(cell, CornerDiameter::uniform(4.0), outline, none, flat);
            layout.add_rectangle(
                Area {
                    left: cell.left + 1.0,
                    top: cell.top + 1.0,
                    width: (cell.width - 2.0).max(1.0),
                    height: (cell.height - 2.0).max(1.0),
                },
                CornerDiameter::uniform(4.0),
                *state.get(&tokens.raised()),
                none,
                flat,
            );
            layout.add_text(
                cell,
                label,
                font,
                if hovered { text } else { muted },
                *state.get(&tokens.accent()),
                HorizontalAlignment::Center { offset: 0.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if hovered {
                layout.register_click_handler(MouseButton::Left, action);
            }
        }

        // Chips, then the status, on the left.
        let area = Area {
            width: (row.width - buttons_width).max(1.0),
            ..row
        };
        let hint = |layout: &mut WindowLayout<'a, ClientState>, area: Area, message: &'a str| {
            layout.add_text(
                area,
                message,
                font,
                muted,
                muted,
                HorizontalAlignment::Left { offset: 2.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        };
        if saved.is_empty() {
            hint(layout, area, if status.is_empty() { "No saved searches" } else { status });
            return;
        }

        let active = state.get(&self.storage_path).view().search().trim().to_lowercase();
        let gap = 4.0;
        let width = ((area.width - gap * (saved.len() as f32 - 1.0)) / saved.len() as f32).min(CHIP_MAX_WIDTH);
        for (slot, query) in saved.iter().enumerate() {
            let chip = Area {
                left: area.left + slot as f32 * (width + gap),
                width,
                ..area
            };
            let hovered = chip.check().run(layout);
            let is_active = query.to_lowercase() == active;
            let border = match (is_active, hovered) {
                (true, _) => *state.get(&tokens.gold()),
                (false, true) => *state.get(&tokens.hover()),
                (false, false) => *state.get(&tokens.border()),
            };
            layout.add_rectangle(chip, CornerDiameter::uniform(10.0), border, none, flat);
            layout.add_rectangle(
                Area {
                    left: chip.left + 1.0,
                    top: chip.top + 1.0,
                    width: (chip.width - 2.0).max(1.0),
                    height: (chip.height - 2.0).max(1.0),
                },
                CornerDiameter::uniform(10.0),
                *state.get(&tokens.raised()),
                none,
                flat,
            );
            layout.add_text(
                chip,
                query,
                font,
                text,
                *state.get(&tokens.gold()),
                HorizontalAlignment::Center { offset: 0.0, border: 4.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if hovered {
                layout.register_click_handler(MouseButton::Left, &self.apply[slot]);
                layout.register_click_handler(MouseButton::Right, &self.forget[slot]);
            }
        }
        let used = saved.len() as f32 * (width + gap);
        if !status.is_empty() && area.width - used > 40.0 {
            hint(
                layout,
                Area {
                    left: area.left + used,
                    width: area.width - used,
                    ..area
                },
                status,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tool_buttons_fit_beside_the_chips() {
        // Even at the minimum window width the chips keep 40% of the row.
        for row in [240.0_f32, 320.0, 600.0] {
            let buttons = ACTIONS.len() as f32 * (action_width(row) + ACTION_GAP);
            assert!(buttons <= row * 0.6, "{row}: {buttons}");
        }
    }
}
