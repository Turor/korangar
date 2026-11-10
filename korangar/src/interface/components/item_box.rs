use korangar_interface::MouseMode;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::event::{ClickHandler, DropHandler, Event, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{ButtonThemePathExt, HorizontalAlignment, TextThemePathExt, VerticalAlignment, WindowThemePathExt};
use korangar_networking::{InventoryItem, InventoryItemDetails};
use rust_state::{Path, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::{InputEvent, MouseInputMode};
use crate::interface::resource::ItemSource;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::{ClientState, client_theme};
use crate::world::ResourceMetadata;

#[derive(Default)]
struct AmountDisplay {
    amount: u16,
    string: Option<String>,
}

impl AmountDisplay {
    fn update(&mut self, new_amount: u16) {
        if self.string.is_none() || self.amount != new_amount {
            self.string = Some(new_amount.to_string());
            self.amount = new_amount;
        }
    }
}

struct ItemBoxHandler<P> {
    item_path: P,
    source: ItemSource,
}

impl<P> ItemBoxHandler<P> {
    fn new(item_path: P, source: ItemSource) -> Self {
        Self { item_path, source }
    }
}

impl<P> ClickHandler<ClientState> for ItemBoxHandler<P>
where
    P: Path<ClientState, InventoryItem<ResourceMetadata>, false>,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        // Unwrapping here is fine since we only register the handler if the slot has a
        // item.
        let item = state.try_get(&self.item_path).unwrap().clone();

        queue.queue(Event::SetMouseMode {
            mouse_mode: MouseMode::Custom {
                mode: MouseInputMode::MoveItem { item, source: self.source },
            },
        });
    }
}

impl<P> DropHandler<ClientState> for ItemBoxHandler<P>
where
    P: Path<ClientState, InventoryItem<ResourceMetadata>, false>,
{
    fn handle_drop(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>, mouse_mode: &MouseMode<ClientState>) {
        if let MouseMode::Custom {
            mode: MouseInputMode::MoveItem { source, item },
        } = mouse_mode
        {
            queue.queue(InputEvent::MoveItem {
                source: *source,
                destination: self.source,
                item: item.clone(),
            });
        }
    }
}

pub struct ItemBox<A> {
    item_path: A,
    handler: ItemBoxHandler<A>,
    amount_display: AmountDisplay,
    /// `0.0` keeps the historical 40px-tall stretched cell. A positive value is
    /// a square icon frame of that side, centered in the cell.
    side: f32,
    /// Native icon size inside `side`. `0.0` stretches the texture to the cell.
    icon_side: f32,
    tooltip: &'static str,
}

impl<A> ItemBox<A>
where
    A: Copy,
{
    /// This function is supposed to be called from a component macro
    /// and not intended to be called manually.
    #[inline(always)]
    pub fn component_new(item_path: A, source: ItemSource, side: f32, icon_side: f32, tooltip: &'static str) -> Self {
        Self {
            item_path,
            handler: ItemBoxHandler::new(item_path, source),
            amount_display: AmountDisplay::default(),
            side,
            icon_side,
            tooltip,
        }
    }
}

impl<A> Element<ClientState> for ItemBox<A>
where
    A: Path<ClientState, InventoryItem<ResourceMetadata>, false>,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let area = if self.side > 0.0 {
                let row = resolver.with_height(self.side);
                // Requested side is the socket. A narrower parent used to shrink Gear
                // slots (44 became ~36 when the window was scaled to 1024).
                let side = self.side;
                Area {
                    left: row.left + (row.width - side) / 2.0,
                    top: row.top,
                    width: side,
                    height: side,
                }
            } else {
                resolver.with_height(40.0)
            };

            if let Some(item) = state.try_get(&self.item_path)
                && item.metadata.texture.as_ref().is_some()
                && let InventoryItemDetails::Regular { amount, .. } = &item.details
            {
                self.amount_display.update(*amount);
            }

            Self::LayoutInfo { area }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        use crate::state::theme::{InterfaceThemePathExt, SkillTreeThemePathExt};
        let (is_hovered, background_color) = match layout.get_mouse_mode() {
            MouseMode::Custom {
                mode: MouseInputMode::MoveItem { .. },
            } => match layout_info.area.check().any_mouse_mode().run(layout) {
                true => {
                    // Since we are not in default mouse mode we need to mark the window as
                    // hovered.
                    layout.set_hovered();

                    (true, *state.get(&client_theme().button().highlight_color()))
                }
                false => (false, *state.get(&client_theme().skill_tree().highlight_color())),
            },
            _ => match layout_info.area.check().run(layout) {
                true => (true, *state.get(&client_theme().button().hovered_background_color())),
                false => (false, *state.get(&client_theme().skill_tree().slot_background_color())),
            },
        };

        let corner = match self.side > 0.0 {
            true => 4.0,
            false => 20.0,
        };
        if self.side > 0.0 {
            layout.add_rectangle(
                layout_info.area,
                CornerDiameter::uniform(corner),
                *state.get(&client_theme().window().closest_anchor_color()),
                Color::rgba_u8(0, 0, 0, 140),
                ShadowPadding::diagonal(0.0, 0.0),
            );
            let inset = 2.0;
            layout.add_rectangle(
                Area {
                    left: layout_info.area.left + inset,
                    top: layout_info.area.top + inset,
                    width: (layout_info.area.width - inset * 2.0).max(1.0),
                    height: (layout_info.area.height - inset * 2.0).max(1.0),
                },
                CornerDiameter::uniform(corner),
                background_color,
                Color::rgba_u8(0, 0, 0, 0),
                ShadowPadding::diagonal(0.0, 0.0),
            );
        } else {
            layout.add_rectangle(
                layout_info.area,
                CornerDiameter::uniform(corner),
                background_color,
                Color::rgba_u8(0, 0, 0, 100),
                ShadowPadding::diagonal(2.0, 5.0),
            );
        }

        if is_hovered {
            layout.register_drop_handler(&self.handler);
            if !self.tooltip.is_empty() {
                struct SlotTooltip;
                layout.add_tooltip(self.tooltip, SlotTooltip.tooltip_id());
            }
        }

        if let Some(item) = state.try_get(&self.item_path)
            && let Some(texture) = item.metadata.texture.as_ref()
        {
            let texture_size = if self.icon_side > 0.0 {
                self.icon_side.min(layout_info.area.width).min(layout_info.area.height)
            } else {
                layout_info.area.width.min(layout_info.area.height)
            };
            let texture_area = Area {
                left: layout_info.area.left + (layout_info.area.width - texture_size) / 2.0,
                top: layout_info.area.top + (layout_info.area.height - texture_size) / 2.0,
                width: texture_size,
                height: texture_size,
            };

            layout.add_texture(texture_area, texture.clone(), Color::WHITE, false);

            if is_hovered {
                layout.register_click_handler(MouseButton::Left, &self.handler);
            }

            if matches!(item.details, InventoryItemDetails::Regular { .. }) {
                layout.add_text(
                    layout_info.area,
                    self.amount_display.string.as_ref().unwrap(),
                    // TODO: Put this in the theme
                    FontSize(12.0),
                    // TODO: Put this in the theme
                    *state.get(&client_theme().text().color()),
                    *state.get(&client_theme().text().highlight_color()),
                    // TODO: Put this in the theme
                    HorizontalAlignment::Right { offset: 3.0, border: 3.0 },
                    // TODO: Put this in the theme
                    VerticalAlignment::Bottom { offset: 3.0 },
                    OverflowBehavior::Shrink,
                );
            }
        }
    }
}
