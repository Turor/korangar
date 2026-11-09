use std::cmp::Ordering;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, State, VecIndexExt};

use super::WindowClass;
use crate::input::InputEvent;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, VendorRowPathExt, client_state};

struct VendorList<A> {
    rows_path: A,
    buying: bool,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> VendorList<A> {
    fn new(rows_path: A) -> Self {
        Self {
            rows_path,
            buying: false,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for VendorList<A>
where
    A: Path<ClientState, Vec<crate::state::VendorRow>>,
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

            let rows = state.get(&self.rows_path);
            let buying = *state.get(&client_state().vendor_buying());
            if buying != self.buying {
                self.elements.clear();
                self.buying = buying;
            }

            match rows.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(rows.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    let button_text = if buying { "Sell 1" } else { "Buy 1" };
                    for index in self.elements.len()..rows.len() {
                        let row_path = self.rows_path.index(index).manually_asserted();
                        let label_path = row_path.label();
                        self.elements.push(ErasedElement::new(split! {
                            children: (
                                text! { text: label_path },
                                button! {
                                    text: button_text,
                                    event: move |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                        queue.queue(InputEvent::VendorTrade { list_index: index });
                                    },
                                },
                            ),
                        }));
                    }
                }
            }

            self.elements
                .iter_mut()
                .enumerate()
                .for_each(|(index, element)| element.create_layout_info(state, store.child_store(index as u64), resolver));
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        self.elements
            .iter()
            .enumerate()
            .for_each(|(index, element)| element.lay_out(state, store.child_store(index as u64), &(), layout));
    }
}

pub struct VendorWindow<A> {
    rows_path: A,
}

impl<A> VendorWindow<A> {
    pub fn new(rows_path: A) -> Self {
        Self { rows_path }
    }
}

impl<A> CustomWindow<ClientState> for VendorWindow<A>
where
    A: Path<ClientState, Vec<crate::state::VendorRow>>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Vendor)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Vendor",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            elements: (
                scroll_view! {
                    children: VendorList::new(self.rows_path),
                },
            ),
        }
    }
}
