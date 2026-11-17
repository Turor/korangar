use korangar_interface::window::{CustomWindow, Window};
use rust_state::Path;

use crate::interface::components::hotbar_grid::HotbarGrid;
use crate::interface::components::hotbar_layout::{
    HOTBAR_GRAB_HEIGHT, HOTBAR_WINDOW_BORDER, HOTBAR_WINDOW_MAXIMUM_HEIGHT, HOTBAR_WINDOW_MAXIMUM_WIDTH, HOTBAR_WINDOW_MINIMUM_HEIGHT,
    HOTBAR_WINDOW_MINIMUM_WIDTH,
};
use crate::interface::windows::WindowClass;
use crate::state::hotbar::Hotbar;
use crate::state::inventory::Inventory;
use crate::state::theme::{HotbarThemePathExt, InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, client_theme};

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
            // Matthew t280u: no title text and no buttons; the slim strip at
            // the top still moves the window.
            title: "",
            title_height: HOTBAR_GRAB_HEIGHT,
            title_gap: 0.0,
            border: HOTBAR_WINDOW_BORDER,
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            // Matthew t274u: resize from every edge and corner; the slots
            // shrink or grow with the window.
            resizable: true,
            background_color: client_theme().hotbar().background_color(),
            minimum_width: HOTBAR_WINDOW_MINIMUM_WIDTH,
            maximum_width: HOTBAR_WINDOW_MAXIMUM_WIDTH,
            minimum_height: HOTBAR_WINDOW_MINIMUM_HEIGHT,
            maximum_height: HOTBAR_WINDOW_MAXIMUM_HEIGHT,
            elements: (HotbarGrid::new(self.hotbar_path, self.inventory_path),),
        }
    }
}
