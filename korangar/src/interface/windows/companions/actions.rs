//! Buttons, rename boxes, and section order for the companion window.
//!
//! roBrowser's homunculus panel offers Feed and Delete. Vaporize is an
//! alchemist skill, not a panel command, so this window does not send one.
//! Standby is client-side: Hercules has no standby opcode for homunculus or
//! mercenary AI.

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use rust_state::{Path, State};

use super::bars::{MeterKind, boxed_accessory, boxed_meter};
use super::skills::boxed_skills;
use super::state::{CompanionWindowState, CompanionWindowStatePathExt};
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::ClientState;

struct PetRenameFocus;
struct HomunRenameFocus;

pub(crate) struct CompanionActions<P> {
    path: P,
    elements: Vec<ElementBox<ClientState>>,
    signature: Signature,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Signature {
    pet: bool,
    pet_rename: bool,
    homunculus: bool,
    homunculus_rename: bool,
    mercenary: bool,
    elemental: bool,
    homunculus_skills: u64,
    mercenary_skills: u64,
}

impl Signature {
    const EMPTY: Self = Self {
        pet: false,
        pet_rename: false,
        homunculus: false,
        homunculus_rename: false,
        mercenary: false,
        elemental: false,
        homunculus_skills: 0,
        mercenary_skills: 0,
    };
}

impl<P> CompanionActions<P>
where
    P: Path<ClientState, CompanionWindowState> + Clone + 'static,
{
    pub(crate) fn new(path: P) -> Self {
        Self {
            path,
            elements: Vec::new(),
            signature: Signature::EMPTY,
        }
    }

    fn sync(&mut self, state: &State<ClientState>) {
        use korangar_interface::prelude::*;

        let next = Signature {
            pet: *state.get(&self.path.pet_known()),
            pet_rename: *state.get(&self.path.pet_rename_allowed()),
            homunculus: *state.get(&self.path.homunculus_known()),
            homunculus_rename: *state.get(&self.path.homunculus_rename_allowed()),
            mercenary: *state.get(&self.path.mercenary_known()),
            elemental: *state.get(&self.path.elemental_known()),
            homunculus_skills: skill_signature(state.get(&self.path.homunculus_skills())),
            mercenary_skills: skill_signature(state.get(&self.path.mercenary_skills())),
        };
        if next == self.signature && !self.elements.is_empty() {
            return;
        }
        self.signature = next;
        self.elements.clear();

        let path = self.path;
        self.elements.push(ErasedElement::new(text! { text: "Pet" }));
        self.elements.push(ErasedElement::new(text! {
            text: path.pet_text(),
            overflow_behavior: OverflowBehavior::Shrink,
        }));
        if next.pet {
            self.elements.push(boxed_meter(path, path.pet_hunger_line(), MeterKind::PetHunger));
            self.elements
                .push(boxed_meter(path, path.pet_intimacy_line(), MeterKind::PetIntimacy));
            self.elements
                .push(boxed_accessory(path.pet_accessory_label(), path.pet_accessory_texture()));
            self.elements
                .push(ErasedElement::new(button! { text: "Feed", event: InputEvent::PetFeed }));
            self.elements.push(ErasedElement::new(
                button! { text: "Performance", event: InputEvent::PetPerformance },
            ));
            self.elements.push(ErasedElement::new(
                button! { text: "Return to egg", event: InputEvent::PetReturn },
            ));
            self.elements.push(ErasedElement::new(
                button! { text: "Unequip accessory", event: InputEvent::PetUnequip },
            ));
            if next.pet_rename {
                let draft = path.pet_rename_draft();
                let confirm = move |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    queue.queue(InputEvent::ConfirmPetRename);
                };
                self.elements.push(ErasedElement::new(text_box! {
                    ghost_text: "New pet name",
                    state: draft,
                    input_handler: DefaultHandler::<_, _, 24>::new(draft, confirm),
                    focus_id: PetRenameFocus,
                    overflow_behavior: OverflowBehavior::Shrink,
                }));
                self.elements.push(ErasedElement::new(button! {
                    text: "Rename pet",
                    event: InputEvent::ConfirmPetRename,
                }));
            }
        }

        self.elements.push(ErasedElement::new(text! { text: "Homunculus" }));
        self.elements.push(ErasedElement::new(text! {
            text: path.homunculus_text(),
            overflow_behavior: OverflowBehavior::Shrink,
        }));
        if next.homunculus {
            self.elements.push(boxed_meter(path, path.homunculus_hp_line(), MeterKind::HomHp));
            self.elements.push(boxed_meter(path, path.homunculus_sp_line(), MeterKind::HomSp));
            self.elements.push(boxed_meter(path, path.homunculus_exp_line(), MeterKind::HomExp));
            self.elements
                .push(boxed_meter(path, path.homunculus_hunger_line(), MeterKind::HomHunger));
            self.elements
                .push(boxed_meter(path, path.homunculus_intimacy_line(), MeterKind::HomIntimacy));
            self.elements
                .push(ErasedElement::new(button! { text: "Feed", event: InputEvent::HomunculusFeed }));
            self.elements.push(ErasedElement::new(
                button! { text: "Release", event: InputEvent::HomunculusRelease },
            ));
            self.elements.push(ErasedElement::new(
                button! { text: "Follow", event: InputEvent::HomunculusFollow },
            ));
            self.elements.push(ErasedElement::new(
                button! { text: "Attack", event: InputEvent::HomunculusAttack },
            ));
            self.elements.push(ErasedElement::new(
                button! { text: "Standby", event: InputEvent::HomunculusStandby },
            ));
            if next.homunculus_rename {
                let draft = path.homunculus_rename_draft();
                let confirm = move |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                    queue.queue(InputEvent::ConfirmHomunculusRename);
                };
                self.elements.push(ErasedElement::new(text_box! {
                    ghost_text: "New homunculus name",
                    state: draft,
                    input_handler: DefaultHandler::<_, _, 24>::new(draft, confirm),
                    focus_id: HomunRenameFocus,
                    overflow_behavior: OverflowBehavior::Shrink,
                }));
                self.elements.push(ErasedElement::new(button! {
                    text: "Rename homunculus",
                    event: InputEvent::ConfirmHomunculusRename,
                }));
            }
            self.elements.push(ErasedElement::new(text! { text: "Homunculus skills" }));
            self.elements.push(boxed_skills(path.homunculus_skills(), true));
        }

        self.elements.push(ErasedElement::new(text! { text: "Mercenary" }));
        self.elements.push(ErasedElement::new(text! {
            text: path.mercenary_text(),
            overflow_behavior: OverflowBehavior::Shrink,
        }));
        if next.mercenary {
            self.elements.push(boxed_meter(path, path.mercenary_hp_line(), MeterKind::MercHp));
            self.elements.push(boxed_meter(path, path.mercenary_sp_line(), MeterKind::MercSp));
            self.elements.push(ErasedElement::new(text! {
                text: path.mercenary_remaining(),
                overflow_behavior: OverflowBehavior::Shrink,
            }));
            self.elements.push(ErasedElement::new(text! { text: "Mercenary skills" }));
            self.elements.push(boxed_skills(path.mercenary_skills(), false));
            self.elements.push(ErasedElement::new(
                button! { text: "Dismiss", event: InputEvent::MercenaryDismiss },
            ));
        }

        self.elements.push(ErasedElement::new(text! { text: "Elemental" }));
        self.elements.push(ErasedElement::new(text! {
            text: path.elemental_text(),
            overflow_behavior: OverflowBehavior::Shrink,
        }));
        if next.elemental {
            self.elements.push(boxed_meter(path, path.elemental_hp_line(), MeterKind::ElHp));
            self.elements.push(boxed_meter(path, path.elemental_sp_line(), MeterKind::ElSp));
        }
    }
}

impl<P> Element<ClientState> for CompanionActions<P>
where
    P: Path<ClientState, CompanionWindowState> + Clone + 'static,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            self.sync(state);
            for (index, element) in self.elements.iter_mut().enumerate() {
                element.create_layout_info(state, store.child_store(index as u64), resolver);
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        for (index, element) in self.elements.iter().enumerate() {
            element.lay_out(state, store.child_store(index as u64), &(), layout);
        }
    }
}

fn skill_signature(skills: &[super::state::CompanionSkill]) -> u64 {
    skills.iter().fold(skills.len() as u64, |acc, skill| {
        acc.wrapping_mul(31).wrapping_add(u64::from(skill.skill_id))
    })
}
