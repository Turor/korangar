use std::cmp::Ordering;

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, State, VecIndexExt};

use super::WindowClass;
use crate::input::InputEvent;
use crate::state::player_shops::{ShopSearchPick, ShopSearchPickPathExt, ShopSearchState, ShopSearchStatePathExt};
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

struct ShopPickList<A> {
    picks_path: A,
    remove: bool,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> ShopPickList<A> {
    fn new(picks_path: A, remove: bool) -> Self {
        Self {
            picks_path,
            remove,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for ShopPickList<A>
where
    A: Path<ClientState, Vec<ShopSearchPick>> + Copy,
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

            let picks = state.get(&self.picks_path);
            match picks.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(picks.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..picks.len() {
                        let pick_path = self.picks_path.index(index).manually_asserted();
                        let remove = self.remove;
                        let caption: &'static str = if remove { "Remove" } else { "Add" };
                        self.elements.push(ErasedElement::new(split! {
                            children: (
                                text! { text: pick_path.label() },
                                button! {
                                    text: caption,
                                    event: move |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                        if remove {
                                            queue.queue(InputEvent::RemoveShopSearchItem { index });
                                        } else {
                                            queue.queue(InputEvent::AddShopSearchItem { index });
                                        }
                                    },
                                },
                            ),
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

pub struct SearchStoreWindow<Rows, Search> {
    rows_path: Rows,
    search_path: Search,
}

impl<Rows, Search> SearchStoreWindow<Rows, Search> {
    pub fn new(rows_path: Rows, search_path: Search) -> Self {
        Self { rows_path, search_path }
    }
}

impl<Rows, Search> CustomWindow<ClientState> for SearchStoreWindow<Rows, Search>
where
    Rows: Path<ClientState, Vec<crate::state::SearchStoreRow>>,
    Search: Path<ClientState, ShopSearchState>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::SearchStore)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        struct QueryBox;
        struct MinBox;
        struct MaxBox;

        let search_path = self.search_path;
        let suggest = |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::SuggestShopSearch);
        };

        window! {
            title: "Shop search",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            minimum_width: 420.0,
            elements: (
                text! { text: search_path.mode_label() },
                text! { text: search_path.status() },
                text_box! {
                    ghost_text: "Item name",
                    state: search_path.query(),
                    input_handler: DefaultHandler::<_, _, 32>::new(search_path.query(), suggest),
                    focus_id: QueryBox,
                },
                split! {
                    children: (
                        text_box! {
                            ghost_text: "Min zeny",
                            state: search_path.min_text(),
                            input_handler: DefaultHandler::<_, _, 10>::new(search_path.min_text(), suggest),
                            focus_id: MinBox,
                        },
                        text_box! {
                            ghost_text: "Max zeny",
                            state: search_path.max_text(),
                            input_handler: DefaultHandler::<_, _, 10>::new(search_path.max_text(), suggest),
                            focus_id: MaxBox,
                        },
                    ),
                },
                split! {
                    children: (
                        button! {
                            text: "Selling shops",
                            event: InputEvent::SetShopSearchBuying { buying: false },
                        },
                        button! {
                            text: "Buying shops",
                            event: InputEvent::SetShopSearchBuying { buying: true },
                        },
                        button! { text: "Find items", event: InputEvent::SuggestShopSearch },
                    ),
                },
                text! { text: "Matching items" },
                scroll_view! {
                    children: ShopPickList::new(search_path.matches(), false),
                },
                text! { text: "Selected items" },
                scroll_view! {
                    children: ShopPickList::new(search_path.selected(), true),
                },
                split! {
                    children: (
                        button! { text: "Search", event: InputEvent::SubmitShopSearch },
                        button! { text: "Next", event: InputEvent::ShopSearchNextPage },
                        button! { text: "Close", event: InputEvent::CloseShopSearch },
                    ),
                },
                scroll_view! {
                    children: SearchList::new(self.rows_path),
                },
            ),
        }
    }
}
