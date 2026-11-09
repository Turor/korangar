use std::cmp::Ordering;

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, State, VecIndexExt};

use super::WindowClass;
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::item_workshop::{
    EnchantGradeStatePathExt, GradeMaterialRow, GradeMaterialRowPathExt, ItemReformStatePathExt, LaphineSynthesisStatePathExt,
    LaphineUpgradeStatePathExt,
};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

struct GradeOptionList<A> {
    materials_path: A,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> GradeOptionList<A> {
    fn new(materials_path: A) -> Self {
        Self {
            materials_path,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for GradeOptionList<A>
where
    A: Path<ClientState, Vec<GradeMaterialRow>> + Copy,
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
                                let row = state.get(&material_path);
                                queue.queue(InputEvent::GradeEnchantPick {
                                    material_index: row.material_index,
                                });
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

struct ReformIndex;
struct GradeIndex;
struct GradeBlessing;
struct SynthesisIndex;
struct SynthesisCount;
struct UpgradeIndex;

pub struct ItemReformWindow;

impl ItemReformWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for ItemReformWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ItemReform)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().item_reform();
        let index_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Item reform",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Inventory index" },
                text_box! {
                    ghost_text: state_path.index_ghost(),
                    state: state_path.index_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.index_text(), index_action),
                    focus_id: ReformIndex,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Reform",
                    event: InputEvent::ItemReformSubmit,
                },
                button! {
                    text: "Close",
                    event: InputEvent::ItemReformClose,
                },
            ),
        }
    }
}

pub struct EnchantGradeWindow;

impl EnchantGradeWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for EnchantGradeWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::EnchantGrade)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().enchant_grade();
        let index_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};
        let blessing_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Enchant grade",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Inventory index" },
                text_box! {
                    ghost_text: state_path.index_ghost(),
                    state: state_path.index_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.index_text(), index_action),
                    focus_id: GradeIndex,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Select equipment",
                    event: InputEvent::GradeEnchantSelect,
                },
                text! { text: "Blessing amount" },
                text_box! {
                    ghost_text: state_path.blessing_ghost(),
                    state: state_path.blessing_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.blessing_text(), blessing_action),
                    focus_id: GradeBlessing,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                GradeOptionList::new(state_path.materials()),
                button! {
                    text: "Grade",
                    event: InputEvent::GradeEnchantSubmit,
                },
                button! {
                    text: "Close",
                    event: InputEvent::GradeEnchantClose,
                },
            ),
        }
    }
}

pub struct LaphineSynthesisWindow;

impl LaphineSynthesisWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for LaphineSynthesisWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::LaphineSynthesis)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().laphine_synthesis();
        let index_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};
        let count_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Laphine synthesis",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Inventory index" },
                text_box! {
                    ghost_text: state_path.index_ghost(),
                    state: state_path.index_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.index_text(), index_action),
                    focus_id: SynthesisIndex,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Count" },
                text_box! {
                    ghost_text: state_path.count_ghost(),
                    state: state_path.count_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.count_text(), count_action),
                    focus_id: SynthesisCount,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Add stack",
                    event: InputEvent::LaphineSynthesisAdd,
                },
                button! {
                    text: "Clear stacks",
                    event: InputEvent::LaphineSynthesisClear,
                },
                button! {
                    text: "Synthesize",
                    event: InputEvent::LaphineSynthesisSubmit,
                },
                button! {
                    text: "Close",
                    event: InputEvent::LaphineSynthesisClose,
                },
            ),
        }
    }
}

pub struct LaphineUpgradeWindow;

impl LaphineUpgradeWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for LaphineUpgradeWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::LaphineUpgrade)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().laphine_upgrade();
        let index_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Laphine upgrade",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Inventory index" },
                text_box! {
                    ghost_text: state_path.index_ghost(),
                    state: state_path.index_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.index_text(), index_action),
                    focus_id: UpgradeIndex,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Upgrade",
                    event: InputEvent::LaphineUpgradeSubmit,
                },
                button! {
                    text: "Close",
                    event: InputEvent::LaphineUpgradeClose,
                },
            ),
        }
    }
}
