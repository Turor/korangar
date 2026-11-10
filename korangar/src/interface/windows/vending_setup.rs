use std::cmp::Ordering;

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, State, VecIndexExt};

use super::WindowClass;
use crate::input::InputEvent;
use crate::state::ClientState;
use crate::state::player_shops::{ShopDraftLine, ShopDraftLinePathExt, ShopSetupState, ShopSetupStatePathExt};
use crate::state::theme::InterfaceThemeType;

struct ShopDraftList<A> {
    lines_path: A,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> ShopDraftList<A> {
    fn new(lines_path: A) -> Self {
        Self {
            lines_path,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for ShopDraftList<A>
where
    A: Path<ClientState, Vec<ShopDraftLine>> + Copy,
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

            let lines = state.get(&self.lines_path);
            match lines.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(lines.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..lines.len() {
                        let line_path = self.lines_path.index(index).manually_asserted();
                        self.elements.push(ErasedElement::new(button! {
                            text: line_path.label(),
                            event: move |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                queue.queue(InputEvent::SelectShopDraft { index });
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

pub struct VendingSetupWindow<A> {
    setup_path: A,
}

impl<A> VendingSetupWindow<A> {
    pub fn new(setup_path: A) -> Self {
        Self { setup_path }
    }
}

impl<A> CustomWindow<ClientState> for VendingSetupWindow<A>
where
    A: Path<ClientState, ShopSetupState>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::VendingSetup)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        struct TitleBox;
        struct AmountBox;
        struct PriceBox;
        struct ZenyBox;

        let setup_path = self.setup_path;
        let commit = |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::CommitShopDraft);
        };

        window! {
            title: "Open shop",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            minimum_width: 360.0,
            elements: (
                text! { text: setup_path.summary() },
                text! { text: setup_path.status() },
                text_box! {
                    ghost_text: "Shop title",
                    state: setup_path.title(),
                    input_handler: DefaultHandler::<_, _, 79>::new(setup_path.title(), commit),
                    focus_id: TitleBox,
                },
                text_box! {
                    ghost_text: "Zeny limit (buying store)",
                    state: setup_path.zeny_text(),
                    input_handler: DefaultHandler::<_, _, 10>::new(setup_path.zeny_text(), commit),
                    focus_id: ZenyBox,
                },
                scroll_view! {
                    children: ShopDraftList::new(setup_path.lines()),
                },
                text_box! {
                    ghost_text: "Amount",
                    state: setup_path.amount_text(),
                    input_handler: DefaultHandler::<_, _, 5>::new(setup_path.amount_text(), commit),
                    focus_id: AmountBox,
                },
                text_box! {
                    ghost_text: "Price",
                    state: setup_path.price_text(),
                    input_handler: DefaultHandler::<_, _, 10>::new(setup_path.price_text(), commit),
                    focus_id: PriceBox,
                },
                split! {
                    children: (
                        button! { text: "Add to shop", event: InputEvent::CommitShopDraft },
                        button! { text: "Remove", event: InputEvent::UnlistShopDraft },
                    ),
                },
                split! {
                    children: (
                        button! { text: "Open", event: InputEvent::SubmitShopSetup },
                        button! { text: "Cancel", event: InputEvent::CancelShopSetup },
                    ),
                },
            ),
        }
    }
}
