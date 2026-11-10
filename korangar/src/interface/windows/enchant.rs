use korangar_interface::window::{CustomWindow, Window};

use super::WindowClass;
use super::picker_list::{PickerKind, PickerList};
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::enchant::{ACTION_PERFECT, ACTION_RANDOM, ACTION_RESET, ACTION_UPGRADE, EnchantWindowStatePathExt};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

pub struct EnchantWindow;

impl EnchantWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for EnchantWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Enchant)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().enchant_window();

        window! {
            title: "Enchant",
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
                    text! {
                        text: state_path.caution(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    PickerList::new(state_path.candidates(), PickerKind::EnchantItem),
                    text! { text: "Slots" },
                    PickerList::new(state_path.slots(), PickerKind::Display),
                    button! {
                        text: "Normal",
                        event: InputEvent::EnchantSetAction { action: ACTION_RANDOM },
                    },
                    button! {
                        text: "Perfect",
                        event: InputEvent::EnchantSetAction { action: ACTION_PERFECT },
                    },
                    button! {
                        text: "Upgrade",
                        event: InputEvent::EnchantSetAction { action: ACTION_UPGRADE },
                    },
                    button! {
                        text: "Reset",
                        event: InputEvent::EnchantSetAction { action: ACTION_RESET },
                    },
                    text! {
                        text: state_path.action_label(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! {
                        text: state_path.chance(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! {
                        text: state_path.cost(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! {
                        text: state_path.requirement(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! {
                        text: state_path.hint(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    PickerList::new(state_path.choices(), PickerKind::EnchantChoice),
                    PickerList::new(state_path.materials(), PickerKind::Display),
                    button! {
                        text: "Enchant",
                        event: InputEvent::EnchantSubmit,
                    },
                    button! {
                        text: "Close",
                        event: InputEvent::CloseEnchant,
                    },
                ),
            },
        }
    }
}
