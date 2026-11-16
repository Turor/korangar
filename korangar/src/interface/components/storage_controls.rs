//! Saved-search chips for the storage window (inventory review #18). Click a
//! chip to apply its search; right-click it to forget it.

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use rust_state::{Path, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::interface::font_scale::from_theme_body;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::inventory::Inventory;
use crate::state::storage_tools::{MAX_SAVED_SEARCHES, StorageTools};
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::{ClientState, client_theme};

const CHIP_HEIGHT: f32 = 22.0;
const CHIP_MAX_WIDTH: f32 = 120.0;

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
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| BaseLayoutInfo {
            area: resolver.with_height(CHIP_HEIGHT),
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
        let saved = state.get(&self.tools_path).saved();
        let area = layout_info.area;
        if saved.is_empty() {
            layout.add_text(
                area,
                "No saved searches",
                font,
                *state.get(&tokens.muted()),
                *state.get(&tokens.muted()),
                HorizontalAlignment::Left { offset: 2.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            return;
        }

        let active = state.get(&self.storage_path).view().search().trim().to_lowercase();
        let gap = 4.0;
        let width = ((area.width - gap * (saved.len() as f32 - 1.0)) / saved.len() as f32).min(CHIP_MAX_WIDTH);
        let none = Color::rgba_u8(0, 0, 0, 0);
        let flat = ShadowPadding::diagonal(0.0, 0.0);
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
                *state.get(&tokens.text()),
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
    }
}
