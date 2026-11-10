//! Barter and expanded barter shops. The list shows the item, its stock, and
//! the zeny or materials it costs. Amount buttons fill the cart. Purchase
//! sends the exchange.

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::event::ClickHandler;
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, PathExt, State, VecIndexExt};

use super::WindowClass;
use crate::graphics::Color;
use crate::input::InputEvent;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::barter::{BarterCartLine, BarterOffer, BarterShopStatePathExt, cost_label, offer_stock_text};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

const ROW_HEIGHT: f32 = 58.0;

struct OfferList {
    elements: Vec<ElementBox<ClientState>>,
}

impl OfferList {
    fn new() -> Self {
        Self { elements: Vec::new() }
    }
}

struct AddToCart {
    index: usize,
    amount: u32,
}

impl ClickHandler<ClientState> for AddToCart {
    fn handle_click(&self, state: &State<ClientState>, _: &mut korangar_interface::event::EventQueue<ClientState>) {
        let index = self.index;
        let amount = self.amount;
        state.update_value_with(client_state().barter(), move |shop| {
            shop.add_to_cart(index, amount);
        });
    }
}

struct OfferRow<A, B> {
    offer_path: A,
    children: B,
    name: String,
    stock: String,
    cost: String,
}

impl<A, B> OfferRow<A, B> {
    fn new(offer_path: A, children: B) -> Self {
        Self {
            offer_path,
            children,
            name: String::new(),
            stock: String::new(),
            cost: String::new(),
        }
    }
}

struct OfferLayout<C> {
    area: Area,
    icon: Area,
    name_area: Area,
    stock_area: Area,
    cost_area: Area,
    children: C,
}

impl<A, B> Element<ClientState> for OfferRow<A, B>
where
    A: Path<ClientState, BarterOffer>,
    B: Element<ClientState>,
{
    type LayoutInfo = OfferLayout<B::LayoutInfo>;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let offer = state.get(&self.offer_path);
        self.name.clone_from(&offer.name);
        self.stock = offer_stock_text(offer);
        self.cost = cost_label(offer.zeny, &offer.currencies, 1);

        with_single_resolver(resolvers, |resolver| {
            let (area, (icon, name_area, stock_area, cost_area, children)) = resolver.with_derived(3.0, 3.0, |resolver| {
                let area = resolver.with_height(ROW_HEIGHT);
                let icon = Area {
                    left: area.left,
                    top: area.top,
                    width: 34.0,
                    height: 34.0,
                };
                let name_area = Area {
                    left: area.left + 40.0,
                    top: area.top,
                    width: (area.width - 40.0).max(0.0),
                    height: 20.0,
                };
                let stock_area = Area {
                    left: area.left + 40.0,
                    top: area.top,
                    width: (area.width - 40.0).max(0.0),
                    height: 20.0,
                };
                let cost_area = Area {
                    left: area.left + 40.0,
                    top: area.top + 22.0,
                    width: (area.width - 40.0).max(0.0),
                    height: 34.0,
                };
                let children = self.children.create_layout_info(state, store, resolver);
                (icon, name_area, stock_area, cost_area, children)
            });
            OfferLayout {
                area,
                icon,
                name_area,
                stock_area,
                cost_area,
                children,
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let window_path = client_theme().window();
        let text_path = client_theme().text();
        let window = state.get(&window_path);
        let text = state.get(&text_path);
        layout.add_rectangle(
            layout_info.area,
            window.corner_diameter,
            window.background_color,
            window.shadow_color,
            window.shadow_padding,
        );
        let offer = state.get(&self.offer_path);
        if let Some(texture) = &offer.texture {
            layout.add_texture(layout_info.icon, texture.clone(), Color::WHITE, false);
        }
        let mut cursor = layout_info.icon.left + 40.0;
        for currency in offer.currencies.iter().take(4) {
            if let Some(texture) = &currency.texture {
                let icon = Area {
                    left: cursor,
                    top: layout_info.area.top + 36.0,
                    width: 18.0,
                    height: 18.0,
                };
                layout.add_texture(icon, texture.clone(), Color::WHITE, false);
                cursor += 20.0;
            }
        }
        layout.add_text(
            layout_info.name_area,
            &self.name,
            text.font_size,
            text.color,
            text.highlight_color,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Top { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        layout.add_text(
            layout_info.stock_area,
            &self.stock,
            FontSize(text.font_size.0.min(14.0)),
            text.color,
            text.highlight_color,
            HorizontalAlignment::Right { offset: 4.0, border: 0.0 },
            VerticalAlignment::Top { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        layout.add_text(
            layout_info.cost_area,
            &self.cost,
            FontSize(text.font_size.0.min(14.0)),
            text.color,
            text.highlight_color,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Top { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        self.children.lay_out(state, store, &layout_info.children, layout);
    }
}

impl Element<ClientState> for OfferList {
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            use korangar_interface::prelude::*;

            let offers_path = client_state().barter().offers();
            let offers = state.get(&offers_path);
            match offers.len().cmp(&self.elements.len()) {
                std::cmp::Ordering::Less => self.elements.truncate(offers.len()),
                std::cmp::Ordering::Equal => {}
                std::cmp::Ordering::Greater => {
                    for index in self.elements.len()..offers.len() {
                        let offer_path = client_state().barter().offers().index(index).manually_asserted();
                        let cannot_add = move |amount: u32| {
                            ComputedSelector::new_default(move |state: &ClientState| {
                                !client_state().barter().follow_safe(state).can_add(index, amount)
                            })
                        };
                        let buttons = split! {
                            gaps: theme().window().gaps(),
                            children: (
                                button! {
                                    text: "+1",
                                    disabled: cannot_add(1),
                                    event: AddToCart { index, amount: 1 },
                                },
                                button! {
                                    text: "+10",
                                    disabled: cannot_add(10),
                                    event: AddToCart { index, amount: 10 },
                                },
                                button! {
                                    text: "+100",
                                    disabled: cannot_add(100),
                                    event: AddToCart { index, amount: 100 },
                                },
                            ),
                        };
                        self.elements.push(ErasedElement::new(OfferRow::new(offer_path, buttons)));
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

struct RemoveFromCart {
    index: usize,
}

impl ClickHandler<ClientState> for RemoveFromCart {
    fn handle_click(&self, state: &State<ClientState>, _: &mut korangar_interface::event::EventQueue<ClientState>) {
        let index = self.index;
        state.update_value_with(client_state().barter(), move |shop| {
            shop.remove_cart_line(index);
        });
    }
}

struct CartRow<A, B> {
    line_path: A,
    children: B,
    label: String,
}

impl<A, B> CartRow<A, B> {
    fn new(line_path: A, children: B) -> Self {
        Self {
            line_path,
            children,
            label: String::new(),
        }
    }
}

struct CartLayout<C> {
    area: Area,
    icon: Area,
    text_area: Area,
    children: C,
}

impl<A, B> Element<ClientState> for CartRow<A, B>
where
    A: Path<ClientState, BarterCartLine>,
    B: Element<ClientState>,
{
    type LayoutInfo = CartLayout<B::LayoutInfo>;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let line = state.get(&self.line_path);
        self.label = format!(
            "{} x{} — {}",
            line.name,
            line.amount,
            cost_label(line.zeny, &line.currencies, line.amount)
        );
        with_single_resolver(resolvers, |resolver| {
            let (area, (icon, text_area, children)) = resolver.with_derived(2.0, 2.0, |resolver| {
                let area = resolver.with_height(28.0);
                let icon = Area {
                    left: area.left,
                    top: area.top,
                    width: 24.0,
                    height: 24.0,
                };
                let text_area = Area {
                    left: area.left + 28.0,
                    top: area.top,
                    width: (area.width - 28.0).max(0.0),
                    height: 28.0,
                };
                let children = self.children.create_layout_info(state, store, resolver);
                (icon, text_area, children)
            });
            CartLayout {
                area,
                icon,
                text_area,
                children,
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let window_path = client_theme().window();
        let text_path = client_theme().text();
        let window = state.get(&window_path);
        let text = state.get(&text_path);
        layout.add_rectangle(
            layout_info.area,
            window.corner_diameter,
            window.background_color,
            window.shadow_color,
            window.shadow_padding,
        );
        let line = state.get(&self.line_path);
        if let Some(texture) = &line.texture {
            layout.add_texture(layout_info.icon, texture.clone(), Color::WHITE, false);
        }
        layout.add_text(
            layout_info.text_area,
            &self.label,
            text.font_size,
            text.color,
            text.highlight_color,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        self.children.lay_out(state, store, &layout_info.children, layout);
    }
}

struct CartList {
    elements: Vec<ElementBox<ClientState>>,
}

impl CartList {
    fn new() -> Self {
        Self { elements: Vec::new() }
    }
}

impl Element<ClientState> for CartList {
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            use korangar_interface::prelude::*;

            let cart_path = client_state().barter().cart();
            let cart = state.get(&cart_path);
            match cart.len().cmp(&self.elements.len()) {
                std::cmp::Ordering::Less => self.elements.truncate(cart.len()),
                std::cmp::Ordering::Equal => {}
                std::cmp::Ordering::Greater => {
                    for index in self.elements.len()..cart.len() {
                        let line_path = client_state().barter().cart().index(index).manually_asserted();
                        let button = button! {
                            text: "Remove",
                            event: RemoveFromCart { index },
                        };
                        self.elements.push(ErasedElement::new(CartRow::new(line_path, button)));
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

pub struct BarterWindow;

impl BarterWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for BarterWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Barter)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let shop = client_state().barter();
        let purchase_blocked =
            ComputedSelector::new_default(|state: &ClientState| client_state().barter().follow_safe(state).purchase_blocked());

        window! {
            title: "Barter",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            elements: (
                text! {
                    text: shop.heading(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                scroll_view! {
                    children: OfferList::new(),
                },
                text! { text: "Cart" },
                scroll_view! {
                    children: CartList::new(),
                },
                text! {
                    text: shop.cart_summary(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Purchase",
                            disabled: purchase_blocked,
                            event: InputEvent::PurchaseBarter,
                        },
                        button! {
                            text: "Close",
                            event: InputEvent::CloseBarter,
                        },
                    ),
                },
                text! {
                    text: shop.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
            ),
        }
    }
}
