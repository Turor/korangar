//! Tab strip, sort label and weight bar for the item grids (inventory review
//! #2, #7, #9, #12). The rules live in `state::inventory_view`.

use std::cell::{Cell, UnsafeCell};

use korangar_interface::MouseMode;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::event::{ClickHandler, DropHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use rust_state::{Path, PathExt, Selector, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::{InputEvent, MouseInputMode};
use crate::interface::font_scale::from_theme_body;
use crate::interface::resource::ItemSource;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::inventory::Inventory;
use crate::state::inventory_view::{InventorySort, InventoryTab, WEIGHT_MARKS, favorite_drop, weight_fraction};
use crate::state::theme::{BasicInfoThemePathExt, InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::{ClientState, client_theme};
use crate::world::{WeightPressure, weight_pressure};

const TAB_HEIGHT: f32 = 24.0;
const WEIGHT_BAR_HEIGHT: f32 = 8.0;

struct TabClick<P> {
    inventory_path: P,
    tab: InventoryTab,
}

impl<P> ClickHandler<ClientState> for TabClick<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    fn handle_click(&self, state: &State<ClientState>, _: &mut EventQueue<ClientState>) {
        let tab = self.tab;
        state.update_value_with(self.inventory_path, move |inventory: &mut Inventory| {
            inventory.view_mut().set_tab(tab)
        });
    }
}

struct TabDrop {
    tab: InventoryTab,
}

impl DropHandler<ClientState> for TabDrop {
    fn handle_drop(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>, mouse_mode: &MouseMode<ClientState>) {
        if let MouseMode::Custom {
            mode: MouseInputMode::MoveItem {
                source: ItemSource::Inventory,
                item,
            },
        } = mouse_mode
            && let Some(favorite) = favorite_drop(self.tab, item)
        {
            queue.queue(InputEvent::SetItemFavorite {
                index: item.index,
                favorite,
            });
        }
    }
}

/// One inventory tab. Click selects it; dropping an inventory item on
/// Favorites favorites it and on any other tab unfavorites it. The active
/// tab uses the gold accent; borders are otherwise neutral.
pub struct TabButton<P> {
    inventory_path: P,
    tab: InventoryTab,
    click: TabClick<P>,
    drop: TabDrop,
}

impl<P: Copy> TabButton<P> {
    pub fn new(inventory_path: P, tab: InventoryTab) -> Self {
        Self {
            inventory_path,
            tab,
            click: TabClick { inventory_path, tab },
            drop: TabDrop { tab },
        }
    }
}

impl<P> Element<ClientState> for TabButton<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| BaseLayoutInfo {
            area: resolver.with_height(TAB_HEIGHT),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let is_move_mode = matches!(layout.get_mouse_mode(), MouseMode::Custom {
            mode: MouseInputMode::MoveItem { .. },
        });
        let is_hovered = if is_move_mode {
            let hovered = layout_info.area.check().any_mouse_mode().run(layout);
            if hovered {
                layout.set_hovered();
            }
            hovered
        } else {
            layout_info.area.check().run(layout)
        };
        let is_active = state.get(&self.inventory_path).view().tab() == self.tab;

        let tokens = client_theme().tokens();
        let gold = *state.get(&tokens.gold());
        let border = match (is_active, is_hovered) {
            (true, _) => gold,
            (false, true) => *state.get(&tokens.hover()),
            (false, false) => *state.get(&tokens.border()),
        };
        let fill = *state.get(&tokens.raised());
        let none = Color::rgba_u8(0, 0, 0, 0);
        layout.add_rectangle(
            layout_info.area,
            CornerDiameter::uniform(4.0),
            border,
            none,
            ShadowPadding::diagonal(0.0, 0.0),
        );
        layout.add_rectangle(
            Area {
                left: layout_info.area.left + 1.0,
                top: layout_info.area.top + 1.0,
                width: (layout_info.area.width - 2.0).max(1.0),
                height: (layout_info.area.height - 2.0).max(1.0),
            },
            CornerDiameter::uniform(4.0),
            fill,
            none,
            ShadowPadding::diagonal(0.0, 0.0),
        );
        let text_color = if is_active { gold } else { *state.get(&tokens.text()) };
        layout.add_text(
            layout_info.area,
            self.tab.label(),
            FontSize(from_theme_body(*state.get(&tokens.font_size()), 13.0)),
            text_color,
            gold,
            HorizontalAlignment::Center { offset: 0.0, border: 2.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );

        if is_hovered {
            match is_move_mode {
                true => layout.register_drop_handler(&self.drop),
                false => layout.register_click_handler(MouseButton::Left, &self.click),
            }
        }
    }
}

/// "Sort: Name" on the cycle button.
pub struct SortLabel<P> {
    inventory_path: P,
    last: Cell<Option<InventorySort>>,
    text: UnsafeCell<String>,
}

impl<P> SortLabel<P> {
    pub fn new(inventory_path: P) -> Self {
        Self {
            inventory_path,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<P> Selector<ClientState, String> for SortLabel<P>
where
    P: Path<ClientState, Inventory>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let sort = self.inventory_path.follow_safe(state).view().sort();
        if self.last.get() != Some(sort) {
            unsafe {
                *self.text.get() = sort.label().to_owned();
            }
            self.last.set(Some(sort));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

/// Advances the display sort of one grid.
pub fn cycle_sort<P>(inventory_path: P) -> impl Fn(&State<ClientState>, &mut EventQueue<ClientState>) + 'static
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
        state.update_value_with(inventory_path, |inventory: &mut Inventory| {
            let next = inventory.view().sort().next();
            inventory.view_mut().set_sort(next);
        });
    }
}

/// Carried weight as a bar. Fill uses the weight status colours; tick marks
/// sit at 50% (no natural HP/SP regen) and 90% (no attacks or skills).
pub struct WeightBar<A, B> {
    current: A,
    maximum: B,
}

impl<A, B> WeightBar<A, B> {
    pub fn new(current: A, maximum: B) -> Self {
        Self { current, maximum }
    }
}

impl<A, B> Element<ClientState> for WeightBar<A, B>
where
    A: Path<ClientState, u32>,
    B: Path<ClientState, u32>,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| BaseLayoutInfo {
            area: resolver.with_height(WEIGHT_BAR_HEIGHT),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let current = *state.get(&self.current);
        let maximum = *state.get(&self.maximum);
        let area = layout_info.area;
        let tokens = client_theme().tokens();
        let weight = client_theme().basic_info();
        let fill = match weight_pressure(current, maximum) {
            WeightPressure::Normal => *state.get(&weight.weight_color()),
            WeightPressure::Caution => *state.get(&weight.weight_caution_color()),
            WeightPressure::Overweight => *state.get(&weight.weight_over_color()),
        };
        let none = Color::rgba_u8(0, 0, 0, 0);
        let flat = ShadowPadding::diagonal(0.0, 0.0);
        layout.add_rectangle(area, CornerDiameter::uniform(3.0), *state.get(&tokens.raised()), none, flat);
        let width = area.width * weight_fraction(current, maximum);
        if width > 0.0 {
            layout.add_rectangle(Area { width, ..area }, CornerDiameter::uniform(3.0), fill, none, flat);
        }
        let mark = *state.get(&tokens.muted());
        for fraction in WEIGHT_MARKS {
            layout.add_rectangle(
                Area {
                    left: area.left + area.width * fraction - 1.0,
                    top: area.top,
                    width: 2.0,
                    height: area.height,
                },
                CornerDiameter::uniform(0.0),
                mark,
                none,
                flat,
            );
        }
    }
}
