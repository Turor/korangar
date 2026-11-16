use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::Path;

use crate::ItemSource;
use crate::interface::components::inventory_controls::{SortLabel, cycle_sort};
use crate::interface::components::item_grid::{CART_FALLBACK_CAPACITY, ItemCountLabel, ItemGrid};
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::inventory::{Inventory, InventoryPathExt};
use crate::state::inventory_view::InventoryViewPathExt;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Pushcart. Rows scroll up to `MAX_CART` (100 on Turoran).
pub struct CartWindow<P> {
    cart_path: P,
}

impl<P> CartWindow<P> {
    pub fn new(cart_path: P) -> Self {
        Self { cart_path }
    }
}

impl<P> CustomWindow<ClientState> for CartWindow<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Cart)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let cart_path = self.cart_path;
        let no_action = |_: &rust_state::State<ClientState>, _: &mut EventQueue<ClientState>| {};

        struct CartSearchBox;

        window! {
            title: client_state().localization().cart_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            minimum_height: 240.0,
            maximum_height: 720.0,
            elements: (
                text! {
                    text: ItemCountLabel::new(self.cart_path, Some(CART_FALLBACK_CAPACITY)),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text_box! {
                            ghost_text: "Search name, effect or card",
                            state: cart_path.view().search(),
                            input_handler: DefaultHandler::<_, _, 48>::new(cart_path.view().search(), no_action),
                            focus_id: CartSearchBox,
                        },
                        button! {
                            text: SortLabel::new(cart_path),
                            event: cycle_sort(cart_path),
                        },
                    ),
                },
                scroll_view! {
                    children: ItemGrid::new(self.cart_path, ItemSource::Cart, Some(CART_FALLBACK_CAPACITY)),
                },
            ),
        }
    }
}
