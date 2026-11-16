//! Themed item menu opened by right-clicking a slot.
//!
//! The focused row uses the button hover token. Escape closes it because the
//! window is closable and has [`WindowClass::ItemMenu`].

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, State, VecIndexExt};

use super::WindowClass;
use crate::input::InputEvent;
use crate::state::item_menu::{ItemMenuRowPathExt, ItemMenuStatePathExt};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

pub struct ItemMenuWindow;

impl ItemMenuWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for ItemMenuWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ItemMenu)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: client_state().item_menu().title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 180.0,
            elements: (
                ItemMenuList { buttons: Vec::new(), built_focus: 0, built_count: 0, built: false },
            ),
        }
    }
}

struct ItemMenuList {
    buttons: Vec<ElementBox<ClientState>>,
    built_focus: usize,
    built_count: usize,
    built: bool,
}

impl Element<ClientState> for ItemMenuList {
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            use korangar_interface::prelude::*;

            let count = state.get(&client_state().item_menu().entries()).len();
            let focus = *state.get(&client_state().item_menu().focus());
            let changed = !self.built || count != self.built_count || focus != self.built_focus;
            if changed {
                self.buttons.clear();
                self.built = true;
                self.built_count = count;
                self.built_focus = focus;
                for index in 0..count {
                    let label = client_state().item_menu().entries().index(index).manually_asserted().label();
                    if index == focus {
                        self.buttons.push(ErasedElement::new(button! {
                            text: label,
                            background_color: theme().button().hovered_background_color(),
                            event: move |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                queue.queue(InputEvent::ItemMenuPick { index });
                            },
                        }));
                    } else {
                        self.buttons.push(ErasedElement::new(button! {
                            text: label,
                            event: move |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                queue.queue(InputEvent::ItemMenuPick { index });
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
