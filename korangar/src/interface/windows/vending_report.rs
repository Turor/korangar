use std::cmp::Ordering;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, State, VecIndexExt};

use super::WindowClass;
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::ClientState;
use crate::state::player_shops::{ShopReportLine, ShopReportLinePathExt, ShopReportState, ShopReportStatePathExt};
use crate::state::theme::InterfaceThemeType;

struct ShopStockList<A> {
    stock_path: A,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> ShopStockList<A> {
    fn new(stock_path: A) -> Self {
        Self {
            stock_path,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for ShopStockList<A>
where
    A: Path<ClientState, Vec<ShopReportLine>> + Copy,
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

            let stock = state.get(&self.stock_path);
            match stock.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(stock.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..stock.len() {
                        let line_path = self.stock_path.index(index).manually_asserted();
                        self.elements.push(ErasedElement::new(text! { text: line_path.label() }));
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

pub struct VendingReportWindow<A> {
    report_path: A,
}

impl<A> VendingReportWindow<A> {
    pub fn new(report_path: A) -> Self {
        Self { report_path }
    }
}

impl<A> CustomWindow<ClientState> for VendingReportWindow<A>
where
    A: Path<ClientState, ShopReportState>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::VendingReport)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let report_path = self.report_path;

        window! {
            title: "My shop",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            minimum_width: 320.0,
            elements: (
                text! { text: report_path.status() },
                scroll_view! {
                    children: ShopStockList::new(report_path.stock()),
                },
                text! { text: "Sales" },
                text! {
                    text: report_path.sales_log(),
                    overflow_behavior: OverflowBehavior::LineBreak,
                    height: 120.0,
                },
                button! { text: "Close shop", event: InputEvent::CloseOwnShop },
            ),
        }
    }
}
