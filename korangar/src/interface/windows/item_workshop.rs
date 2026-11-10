use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::State;

use super::WindowClass;
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::item_workshop::{ItemReformStatePathExt, LaphineSynthesisStatePathExt, LaphineUpgradeStatePathExt};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

struct ReformIndex;
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
