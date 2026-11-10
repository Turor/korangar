use std::cmp::Ordering;

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, State, VecIndexExt};

use super::WindowClass;
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::ClientState;
use crate::state::theme::InterfaceThemeType;
use crate::state::trade::{TradeInventoryOffer, TradeInventoryOfferPathExt, TradeState, TradeStatePathExt};

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

struct TradeOfferButtons<A> {
    offers_path: A,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> TradeOfferButtons<A> {
    fn new(offers_path: A) -> Self {
        Self {
            offers_path,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for TradeOfferButtons<A>
where
    A: Path<ClientState, Vec<TradeInventoryOffer>> + Copy,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            use korangar_interface::prelude::*;

            let offers = state.get(&self.offers_path);
            match offers.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(offers.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..offers.len() {
                        let offer_path = self.offers_path.index(index).manually_asserted();
                        self.elements.push(ErasedElement::new(button! {
                            text: offer_path.label(),
                            event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                let TradeInventoryOffer { index, amount, .. } = state.get(&offer_path);
                                queue.queue(InputEvent::TradeAddItem {
                                    index: *index,
                                    amount: *amount,
                                });
                            },
                        }));
                    }
                }
            }

            self.elements.iter_mut().enumerate().for_each(|(index, element)| {
                element.create_layout_info(state, store.child_store(index as u64), resolver);
            });
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        self.elements.iter().enumerate().for_each(|(index, element)| {
            element.lay_out(state, store.child_store(index as u64), &(), layout);
        });
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
                text! {
                    text: trade_path.partner_summary(),
                    overflow_behavior: OverflowBehavior::LineBreak,
                    height: 48.0,
                },
                text! {
                    text: "Your offer",
                },
                text! {
                    text: trade_path.own_summary(),
                    overflow_behavior: OverflowBehavior::LineBreak,
                    height: 48.0,
                },
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
                    text: "Offer an inventory item",
                },
                TradeOfferButtons::new(trade_path.inventory_offers()),
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
