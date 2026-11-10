use korangar_interface::window::{CustomWindow, Window};

use super::WindowClass;
use super::picker_list::{PickerKind, PickerList};
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::enchant_grade::EnchantGradeStatePathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

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

        window! {
            title: "Enchant grade",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            elements: scroll_view! {
                children: (
                    text! {
                        text: state_path.status(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    PickerList::new(state_path.candidates(), PickerKind::GradeItem),
                    text! {
                        text: state_path.chance(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! {
                        text: state_path.blessing(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    button! {
                        text: "Blessing +",
                        event: InputEvent::GradeBlessingAdd,
                    },
                    button! {
                        text: "Blessing -",
                        event: InputEvent::GradeBlessingRemove,
                    },
                    text! {
                        text: state_path.cost(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    PickerList::new(state_path.materials(), PickerKind::GradeMaterial),
                    button! {
                        text: "Grade",
                        event: InputEvent::GradeEnchantSubmit,
                    },
                    button! {
                        text: "Close",
                        event: InputEvent::GradeEnchantClose,
                    },
                ),
            },
        }
    }
}
