//! Item Reform, Laphine Synthesis, and Laphine Upgrade.
//!
//! The server packet only names the tool. Eligible gear comes from the client
//! tables, and a click picks an inventory row. There is no index field.

use korangar_interface::Interface;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{PathExt, State};

use super::WindowClass;
use super::picker_list::{PickerKind, PickerList};
use crate::input::InputEvent;
use crate::loaders::{GameFileLoader, OverflowBehavior};
use crate::state::inventory::InventoryPathExt;
use crate::state::item_workshop::{ItemReformStatePathExt, LaphineSynthesisStatePathExt, LaphineUpgradeStatePathExt};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

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

        window! {
            title: "Item Reform",
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
                    text! { text: "Reform tool" },
                    text! {
                        text: state_path.catalyst_name(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    PickerList::new(state_path.catalyst(), PickerKind::Display),
                    text! { text: "Items" },
                    PickerList::new(state_path.candidates(), PickerKind::ReformItem),
                    text! { text: "Result" },
                    text! {
                        text: state_path.result_preview(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! {
                        text: state_path.kept(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! {
                        text: state_path.details(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! { text: "Materials" },
                    PickerList::new(state_path.materials(), PickerKind::Display),
                    button! {
                        text: "Reform",
                        disabled: ComputedSelector::new_default(|state: &ClientState| {
                            !*client_state().item_reform().submit_enabled().follow_safe(state)
                        }),
                        event: InputEvent::ItemReformSubmit,
                    },
                    button! {
                        text: "Close",
                        event: InputEvent::ItemReformClose,
                    },
                ),
            },
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

        window! {
            title: "Laphine Synthesis",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            elements: scroll_view! {
                children: (
                    text! {
                        text: state_path.box_name(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! {
                        text: state_path.status(),
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
                    text! { text: "Available" },
                    PickerList::new(state_path.candidates(), PickerKind::LaphineSource),
                    text! { text: "Chosen" },
                    PickerList::new(state_path.chosen(), PickerKind::LaphineSource),
                    button! {
                        text: "Synthesize",
                        disabled: ComputedSelector::new_default(|state: &ClientState| {
                            !*client_state().laphine_synthesis().submit_enabled().follow_safe(state)
                        }),
                        event: InputEvent::LaphineSynthesisSubmit,
                    },
                    button! {
                        text: "Clear",
                        event: InputEvent::LaphineSynthesisClear,
                    },
                    button! {
                        text: "Close",
                        event: InputEvent::LaphineSynthesisClose,
                    },
                ),
            },
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

        window! {
            title: "Laphine Upgrade",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            elements: scroll_view! {
                children: (
                    text! {
                        text: state_path.box_name(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! {
                        text: state_path.status(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! { text: "Result" },
                    text! {
                        text: state_path.result_preview(),
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                    text! { text: "Items" },
                    PickerList::new(state_path.candidates(), PickerKind::LaphineUpgradeItem),
                    button! {
                        text: "Upgrade",
                        disabled: ComputedSelector::new_default(|state: &ClientState| {
                            !*client_state().laphine_upgrade().submit_enabled().follow_safe(state)
                        }),
                        event: InputEvent::LaphineUpgradeSubmit,
                    },
                    button! {
                        text: "Close",
                        event: InputEvent::LaphineUpgradeClose,
                    },
                ),
            },
        }
    }
}

fn workshop_snaps(
    state: &State<ClientState>,
    loader: &GameFileLoader,
) -> (
    Vec<crate::state::item_workshop::WorkshopSnap>,
    &'static crate::state::item_workshop::WorkshopCatalog,
) {
    let items = state.follow(client_state().inventory().items()).clone();
    let catalog = crate::state::item_workshop::shared_catalog(loader);
    let snaps = crate::state::item_workshop::snaps_from_inventory(&items, catalog);
    (snaps, catalog)
}

pub fn open_item_reform(
    interface: &mut Interface<'static, ClientState>,
    loader: &GameFileLoader,
    state: &mut State<ClientState>,
    item_id: u32,
) {
    let (snaps, catalog) = workshop_snaps(state, loader);
    state.follow_mut(client_state().item_reform()).open(item_id, &snaps, catalog);
    interface.open_window(ItemReformWindow::new());
}

pub fn open_laphine_synthesis(
    interface: &mut Interface<'static, ClientState>,
    loader: &GameFileLoader,
    state: &mut State<ClientState>,
    item_id: u32,
) {
    let (snaps, catalog) = workshop_snaps(state, loader);
    state.follow_mut(client_state().laphine_synthesis()).open(item_id, &snaps, catalog);
    interface.open_window(LaphineSynthesisWindow::new());
}

pub fn open_laphine_upgrade(
    interface: &mut Interface<'static, ClientState>,
    loader: &GameFileLoader,
    state: &mut State<ClientState>,
    item_id: u32,
) {
    let (snaps, catalog) = workshop_snaps(state, loader);
    state.follow_mut(client_state().laphine_upgrade()).open(item_id, &snaps, catalog);
    interface.open_window(LaphineUpgradeWindow::new());
}

/// Refresh owned counts, then the reform request the Reform button sends.
pub fn reform_request(state: &mut State<ClientState>) -> Result<(u32, u16), &'static str> {
    let items = state.follow(client_state().inventory().items()).clone();
    let snaps = crate::state::item_workshop::snaps_from_inventory(&items, crate::state::item_workshop::loaded_catalog());
    let reform = state.follow_mut(client_state().item_reform());
    reform.refresh_snaps(&snaps);
    reform.request()
}
