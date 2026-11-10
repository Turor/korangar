use korangar_interface::window::{CustomWindow, Window};
use rust_state::Path;

use crate::input::InputEvent;
use crate::interface::components::hotbar_grid::HotbarGrid;
use crate::interface::windows::WindowClass;
use crate::state::hotbar::Hotbar;
use crate::state::inventory::Inventory;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

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
            minimum_width: 420.0,
            maximum_width: 640.0,
            minimum_height: 72.0,
            maximum_height: 280.0,
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
                scroll_view! {
                    children: HotbarGrid::new(self.hotbar_path, self.inventory_path),
                },
            ),
        }
    }
}
