use korangar_interface::window::{CustomWindow, Window};
use rust_state::Path;

use crate::input::InputEvent;
use crate::interface::components::hotbar_grid::HotbarGrid;
use crate::interface::components::hotbar_layout::{
    HOTBAR_WINDOW_MAXIMUM_HEIGHT, HOTBAR_WINDOW_MAXIMUM_WIDTH, HOTBAR_WINDOW_MINIMUM_HEIGHT, HOTBAR_WINDOW_MINIMUM_WIDTH,
};
use crate::interface::windows::WindowClass;
use crate::state::hotbar::Hotbar;
use crate::state::inventory::Inventory;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::{HotbarThemePathExt, InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

pub struct HotbarWindow<H, I> {
    hotbar_path: H,
    inventory_path: I,
}

impl<H, I> HotbarWindow<H, I> {
    pub fn new(hotbar_path: H, inventory_path: I) -> Self {
        Self {
            hotbar_path,
            inventory_path,
        }
    }
}

impl<H, I> CustomWindow<ClientState> for HotbarWindow<H, I>
where
    H: Path<ClientState, Hotbar> + Copy,
    I: Path<ClientState, Inventory> + Copy,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Hotbar)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: client_state().localization().hotbar_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            // Matthew t274u: resize from every edge and corner; the grid
            // reflows and the slots shrink or grow with the window.
            resizable: true,
            background_color: client_theme().hotbar().background_color(),
            minimum_width: HOTBAR_WINDOW_MINIMUM_WIDTH,
            maximum_width: HOTBAR_WINDOW_MAXIMUM_WIDTH,
            minimum_height: HOTBAR_WINDOW_MINIMUM_HEIGHT,
            maximum_height: HOTBAR_WINDOW_MAXIMUM_HEIGHT,
            elements: (
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Tab",
                            event: InputEvent::SwitchHotbarTab,
                        },
                        button! {
                            text: "Rows",
                            event: InputEvent::CycleHotbarRows,
                        },
                        button! {
                            text: "Rotate",
                            event: InputEvent::RotateHotbar,
                        },
                        button! {
                            text: "Keys",
                            event: InputEvent::ToggleKeybindsWindow,
                        },
                    ),
                },
                HotbarGrid::new(self.hotbar_path, self.inventory_path),
            ),
        }
    }
}
