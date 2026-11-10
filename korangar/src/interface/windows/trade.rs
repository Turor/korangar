use korangar_interface::MouseMode;
use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{DropHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, TextThemePathExt, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::InventoryItemDetails;
use rust_state::{Path, State};

use super::WindowClass;
use crate::graphics::Color;
use crate::input::{InputEvent, MouseInputMode};
use crate::interface::components::item_box::{paint_item_slot, slot_border_and_fill};
use crate::interface::resource::ItemSource;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::trade::{TradeOfferLine, TradeState, TradeStatePathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

pub struct TradeRequestWindow {
    partner_name: String,
    level: i16,
}

impl TradeRequestWindow {
    pub fn new(partner_name: String, level: i16) -> Self {
        Self { partner_name, level }
    }
}

impl CustomWindow<ClientState> for TradeRequestWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::TradeRequest)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let partner_name = self.partner_name;
        let level = self.level;

        window! {
            title: "Trade request",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            elements: (
                text! {
                    text: format!("{partner_name} (lv {level}) wants to trade"),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Decline",
                            event: InputEvent::DeclineTrade,
                        },
                        button! {
                            text: "Accept",
                            event: InputEvent::AcceptTrade,
                        },
                    ),
                },
            ),
        }
    }
}

const OFFER_ROW: f32 = 28.0;

struct TradeOfferIcons<A> {
    lines_path: A,
}

impl<A> TradeOfferIcons<A> {
    fn new(lines_path: A) -> Self {
        Self { lines_path }
    }
}

impl<A> Element<ClientState> for TradeOfferIcons<A>
where
    A: Path<ClientState, Vec<TradeOfferLine>>,
{
    type LayoutInfo = Area;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let count = state.get(&self.lines_path).len();
            let height = if count == 0 { 20.0 } else { count as f32 * OFFER_ROW };
            resolver.with_height(height)
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let lines = state.get(&self.lines_path);
        if lines.is_empty() {
            layout.add_text(
                *layout_info,
                "(nothing)",
                FontSize(12.0),
                *state.get(&client_theme().tokens().muted()),
                *state.get(&client_theme().tokens().gold()),
                HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            return;
        }

        for (index, line) in lines.iter().enumerate() {
            let row = Area {
                left: layout_info.left,
                top: layout_info.top + index as f32 * OFFER_ROW,
                width: layout_info.width,
                height: OFFER_ROW,
            };
            let icon = Area {
                left: row.left,
                top: row.top + 2.0,
                width: 24.0,
                height: 24.0,
            };
            let hovered = icon.check().run(layout);
            let (border, fill) = slot_border_and_fill(state, hovered, false);
            paint_item_slot(layout, icon, border, fill, 4.0, true);
            if let Some(texture) = line.texture.as_ref() {
                layout.add_texture(icon, texture.clone(), Color::WHITE, false);
            }
            layout.add_text(
                Area {
                    left: row.left + 30.0,
                    top: row.top,
                    width: (row.width - 30.0).max(1.0),
                    height: row.height,
                },
                &line.label,
                FontSize(12.0),
                *state.get(&client_theme().text().color()),
                *state.get(&client_theme().tokens().gold()),
                HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if hovered {
                struct OfferTip;
                layout.add_tooltip(&line.label, OfferTip.tooltip_id());
            }
        }
    }
}

struct TradeDropZone;

impl Element<ClientState> for TradeDropZone {
    type LayoutInfo = Area;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| resolver.with_height(36.0))
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let moving = matches!(layout.get_mouse_mode(), MouseMode::Custom {
            mode: MouseInputMode::MoveItem {
                source: ItemSource::Inventory,
                ..
            },
        });
        let hovered = if moving {
            let hovered = layout_info.check().any_mouse_mode().run(layout);
            if hovered {
                layout.set_hovered();
            }
            hovered
        } else {
            layout_info.check().run(layout)
        };
        let (border, fill) = slot_border_and_fill(state, hovered, moving);
        paint_item_slot(layout, *layout_info, border, fill, 6.0, true);
        layout.add_text(
            *layout_info,
            "Drop an inventory item",
            FontSize(12.0),
            *state.get(&client_theme().text().color()),
            *state.get(&client_theme().tokens().gold()),
            HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        if hovered {
            layout.register_drop_handler(&TradeInventoryDrop);
        }
    }
}

struct TradeInventoryDrop;

impl DropHandler<ClientState> for TradeInventoryDrop {
    fn handle_drop(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>, mouse_mode: &MouseMode<ClientState>) {
        let MouseMode::Custom {
            mode: MouseInputMode::MoveItem { source, item },
        } = mouse_mode
        else {
            return;
        };
        if *source != ItemSource::Inventory {
            return;
        }
        let amount = match &item.details {
            InventoryItemDetails::Regular { amount, .. } => *amount,
            InventoryItemDetails::Equippable { .. } => 1,
        };
        let index = item.index.0;
        if state.get(&client_state().trade()).can_note_item_add(index, u32::from(amount)) {
            queue.queue(InputEvent::TradeAddItem { index, amount });
        }
    }
}

pub struct TradeWindow<A> {
    trade_path: A,
}

impl<A> TradeWindow<A> {
    pub fn new(trade_path: A) -> Self {
        Self { trade_path }
    }
}

impl<A> CustomWindow<ClientState> for TradeWindow<A>
where
    A: Path<ClientState, TradeState>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Trade)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        struct ZenyField;

        let trade_path = self.trade_path;
        let add_zeny = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            let zeny_draft = trade_path.zeny_draft();
            let draft = state.get(&zeny_draft);
            let Ok(amount) = draft.parse::<u32>() else {
                return;
            };
            if amount > 0 {
                queue.queue(InputEvent::TradeAddZeny { amount });
            }
        };

        let deal_disabled = ComputedSelector::new_default(move |state: &ClientState| {
            let trade = trade_path.follow(state).expect("trade state");
            !(trade.self_locked && trade.partner_locked) || trade.self_committed
        });

        window! {
            title: "Trade",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            resizable: true,
            minimum_height: 280.0,
            elements: (
                text! {
                    text: "Their offer",
                },
                TradeOfferIcons::new(trade_path.partner_lines()),
                text! {
                    text: "Your offer",
                },
                TradeOfferIcons::new(trade_path.own_lines()),
                text! {
                    text: trade_path.status(),
                    overflow_behavior: OverflowBehavior::LineBreak,
                },
                text_box! {
                    ghost_text: "Zeny to offer",
                    state: trade_path.zeny_draft(),
                    input_handler: DefaultHandler::<_, _, 10>::new(trade_path.zeny_draft(), add_zeny),
                    focus_id: ZenyField,
                },
                button! {
                    text: "Add zeny",
                    event: add_zeny,
                    disabled: trade_path.self_locked(),
                },
                text! {
                    text: "Drag an item from the inventory",
                },
                TradeDropZone,
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Cancel",
                            event: InputEvent::TradeCancel,
                        },
                        button! {
                            text: "Confirm",
                            event: InputEvent::TradeConfirm,
                            disabled: trade_path.self_locked(),
                        },
                        button! {
                            text: "Deal",
                            event: InputEvent::TradeDeal,
                            disabled: deal_disabled,
                        },
                    ),
                },
            ),
        }
    }
}
