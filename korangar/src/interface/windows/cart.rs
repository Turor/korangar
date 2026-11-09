use korangar_components::item_box;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::InventoryItem;
use rust_state::{Path, VecIndexExt};

use crate::ItemSource;
use crate::interface::windows::WindowClass;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};
use crate::world::ResourceMetadata;

pub struct CartWindow<P> {
    items_path: P,
}

impl<P> CartWindow<P> {
    pub fn new(items_path: P) -> Self {
        Self { items_path }
    }
}

impl<P> CustomWindow<ClientState> for CartWindow<P>
where
    P: Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Cart)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        const CART_ROWS: usize = 4;
        const CART_COLUMNS: usize = 10;

        window! {
            title: client_state().localization().cart_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: std::array::from_fn::<_, CART_ROWS, _>(|row| {
                split! {
                    gaps: theme().window().gaps(),
                    children: std::array::from_fn::<_, CART_COLUMNS, _>(|column| {
                        let path = self.items_path.index(row * CART_COLUMNS + column);

                        item_box! {
                            item_path: path,
                            source: ItemSource::Cart,
                        }
                    }),
                }
            }),
        }
    }
}
