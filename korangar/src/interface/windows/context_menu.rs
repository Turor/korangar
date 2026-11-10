//! Small closable menu opened by right-clicking a player.

use std::cmp::Ordering;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, State, VecIndexExt};

use super::WindowClass;
use crate::input::InputEvent;
use crate::state::context_menu::{ContextMenuEntryPathExt, ContextMenuStatePathExt};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

pub struct ContextMenuWindow;

impl ContextMenuWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for ContextMenuWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ContextMenu)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: client_state().context_menu().title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 180.0,
            elements: (
                ContextMenuList { buttons: Vec::new() },
            ),
        }
    }
}

struct ContextMenuList {
    buttons: Vec<ElementBox<ClientState>>,
}

impl Element<ClientState> for ContextMenuList {
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            use korangar_interface::prelude::*;

            let count = state.get(&client_state().context_menu().entries()).len();
            match count.cmp(&self.buttons.len()) {
                Ordering::Equal => {}
                Ordering::Less | Ordering::Greater => {
                    self.buttons.clear();
                    for index in 0..count {
                        let label = client_state().context_menu().entries().index(index).manually_asserted().label();
                        self.buttons.push(ErasedElement::new(button! {
                            text: label,
                            event: move |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                queue.queue(InputEvent::ContextMenuPick { index });
                            },
                        }));
                    }
                }
            }

            self.buttons.iter_mut().enumerate().for_each(|(index, element)| {
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
        self.buttons.iter().enumerate().for_each(|(index, element)| {
            element.lay_out(state, store.child_store(index as u64), &(), layout);
        });
    }
}
