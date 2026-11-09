use std::cmp::Ordering;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, State, VecIndexExt};

use super::WindowClass;
use crate::input::InputEvent;
use crate::state::ClientState;
use crate::state::refine::{
    RefineCandidate, RefineCandidatePathExt, RefineMaterialRow, RefineMaterialRowPathExt, RefineState, RefineStatePathExt,
};
use crate::state::theme::InterfaceThemeType;

struct RefineCandidateList<A> {
    candidates_path: A,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> RefineCandidateList<A> {
    fn new(candidates_path: A) -> Self {
        Self {
            candidates_path,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for RefineCandidateList<A>
where
    A: Path<ClientState, Vec<RefineCandidate>> + Copy,
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

            let candidates = state.get(&self.candidates_path);
            match candidates.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(candidates.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..candidates.len() {
                        let candidate_path = self.candidates_path.index(index).manually_asserted();
                        self.elements.push(ErasedElement::new(button! {
                            text: candidate_path.label(),
                            event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                let RefineCandidate { index, .. } = state.get(&candidate_path);
                                queue.queue(InputEvent::RefineSelectItem { index: *index });
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

struct RefineMaterialList<A> {
    materials_path: A,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> RefineMaterialList<A> {
    fn new(materials_path: A) -> Self {
        Self {
            materials_path,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for RefineMaterialList<A>
where
    A: Path<ClientState, Vec<RefineMaterialRow>> + Copy,
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

            let materials = state.get(&self.materials_path);
            match materials.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(materials.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..materials.len() {
                        let material_path = self.materials_path.index(index).manually_asserted();
                        self.elements.push(ErasedElement::new(button! {
                            text: material_path.label(),
                            event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                let RefineMaterialRow { item_id, .. } = state.get(&material_path);
                                queue.queue(InputEvent::RefineWith { item_id: *item_id });
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

pub struct RefineWindow<A> {
    refine_path: A,
}

impl<A> RefineWindow<A> {
    pub fn new(refine_path: A) -> Self {
        Self { refine_path }
    }
}

impl<A> CustomWindow<ClientState> for RefineWindow<A>
where
    A: Path<ClientState, RefineState>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Refine)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let refine_path = self.refine_path;

        window! {
            title: "Refine",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            resizable: true,
            elements: (
                text! { text: refine_path.selected_label() },
                text! { text: refine_path.status() },
                text! { text: refine_path.blessing() },
                scroll_view! {
                    children: RefineCandidateList::new(refine_path.candidates()),
                },
                scroll_view! {
                    children: RefineMaterialList::new(refine_path.materials()),
                },
                button! {
                    text: "Close",
                    event: InputEvent::RefineClose,
                },
            ),
        }
    }
}
