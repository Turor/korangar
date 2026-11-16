use std::cmp::Ordering;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{EquipSwitchSlot, EquipSwitchSlotPathExt};
use rust_state::{ManuallyAssertExt, Path, State, VecIndexExt};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::state::ClientState;
use crate::state::theme::InterfaceThemeType;

struct SwitchSet<A> {
    slots_path: A,
    elements: Vec<ElementBox<ClientState>>,
    showing_empty: bool,
}

impl<A> SwitchSet<A> {
    fn new(slots_path: A) -> Self {
        Self {
            slots_path,
            elements: Vec::new(),
            showing_empty: false,
        }
    }
}

impl<A> Element<ClientState> for SwitchSet<A>
where
    A: Path<ClientState, Vec<EquipSwitchSlot>>,
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

            let slots = state.get(&self.slots_path);
            if slots.is_empty() {
                if !self.showing_empty {
                    self.elements.clear();
                    self.elements.push(ErasedElement::new(text! {
                        text: "No items in the switch set.",
                    }));
                    self.showing_empty = true;
                }
            } else {
                if self.showing_empty {
                    self.elements.clear();
                    self.showing_empty = false;
                }
                match slots.len().cmp(&self.elements.len()) {
                    Ordering::Less => self.elements.truncate(slots.len()),
                    Ordering::Equal => {}
                    Ordering::Greater => {
                        for index in self.elements.len()..slots.len() {
                            let slot_path = self.slots_path.index(index).manually_asserted();
                            self.elements.push(ErasedElement::new(split! {
                                gaps: theme().window().gaps(),
                                children: (
                                    text! {
                                        text: slot_path.label(),
                                        overflow_behavior: crate::loaders::OverflowBehavior::Shrink,
                                    },
                                    button! {
                                        text: "Remove",
                                        event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                            let index = state.get(&slot_path).index;
                                            queue.queue(InputEvent::RemoveEquipSwitchItem { index });
                                        },
                                    },
                                ),
                            }));
                        }
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

pub struct SwitchEquipWindow<A> {
    slots_path: A,
}

impl<A> SwitchEquipWindow<A> {
    pub fn new(slots_path: A) -> Self {
        Self { slots_path }
    }
}

impl<A> CustomWindow<ClientState> for SwitchEquipWindow<A>
where
    A: Path<ClientState, Vec<EquipSwitchSlot>>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::SwitchEquip)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Switch equipment",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                crate::interface::windows::WindowGroupTabs::new(WindowClass::SwitchEquip),
                button! {
                    text: "Activate set",
                    event: InputEvent::ActivateEquipSwitch,
                },
                SwitchSet::new(self.slots_path),
            ),
        }
    }
}
