use std::cmp::Ordering;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, State, VecIndexExt};

use super::WindowClass;
use crate::input::InputEvent;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, SearchStoreRowPathExt};

struct SearchList<A> {
    rows_path: A,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> SearchList<A> {
    fn new(rows_path: A) -> Self {
        Self {
            rows_path,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for SearchList<A>
where
    A: Path<ClientState, Vec<crate::state::SearchStoreRow>>,
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

            match rows.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(rows.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..rows.len() {
                        let row_path = self.rows_path.index(index).manually_asserted();
                        let label_path = row_path.label();
                        self.elements.push(ErasedElement::new(split! {
                            children: (
                                text! { text: label_path },
                                button! {
                                    text: "Vending",
                                    event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                        let row = state.get(&row_path);
                                        queue.queue(InputEvent::OpenPlayerShop {
                                            account_id: row.account_id,
                                            buying_store: false,
                                        });
                                    },
                                },
                                button! {
                                    text: "Buying",
                                    event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                        let row = state.get(&row_path);
                                        queue.queue(InputEvent::OpenPlayerShop {
                                            account_id: row.account_id,
                                            buying_store: true,
                                        });
                                    },
                                },
                                button! {
                                    text: "Locate",
                                    event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                        let row = state.get(&row_path);
                                        queue.queue(InputEvent::LocateShop {
                                            account_id: row.account_id,
                                            store_id: row.store_id,
                                            item_id: row.item_id,
                                        });
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

pub struct SearchStoreWindow<A> {
    rows_path: A,
}

impl<A> SearchStoreWindow<A> {
    pub fn new(rows_path: A) -> Self {
        Self { rows_path }
    }
}

impl<A> CustomWindow<ClientState> for SearchStoreWindow<A>
where
    A: Path<ClientState, Vec<crate::state::SearchStoreRow>>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::SearchStore)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Shop search",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            elements: (
                scroll_view! {
                    children: SearchList::new(self.rows_path),
                },
            ),
        }
    }
}
