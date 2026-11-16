use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::Path;

use crate::ItemSource;
use crate::input::InputEvent;
use crate::interface::components::inventory_controls::{SortLabel, cycle_sort};
use crate::interface::components::item_grid::{ItemCountLabel, ItemGrid};
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::inventory::{Inventory, InventoryPathExt};
use crate::state::inventory_view::InventoryViewPathExt;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Kafra or guild storage. Rows scroll; the cap comes from
/// `ZC_NOTIFY_STOREITEM_COUNTINFO`.
pub struct StorageWindow<P> {
    storage_path: P,
}

impl<P> StorageWindow<P> {
    pub fn new(storage_path: P) -> Self {
        Self { storage_path }
    }
}

impl<P> CustomWindow<ClientState> for StorageWindow<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Storage)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let storage_path = self.storage_path;
        let no_action = |_: &rust_state::State<ClientState>, _: &mut EventQueue<ClientState>| {};

        struct StorageSearchBox;

        window! {
            title: client_state().localization().storage_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            // The title-bar close does not tell the server storage is done.
            // The Close button sends CZ_CLOSE_STORE.
            closable: false,
            resizable: true,
            minimum_height: 240.0,
            maximum_height: 720.0,
            elements: (
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text! {
                            text: ItemCountLabel::new(self.storage_path, None),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        button! {
                            text: client_state().localization().close_button_text(),
                            event: InputEvent::CloseStorage,
                        },
                    ),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text_box! {
                            ghost_text: "Search name, effect or card",
                            state: storage_path.view().search(),
                            input_handler: DefaultHandler::<_, _, 48>::new(storage_path.view().search(), no_action),
                            focus_id: StorageSearchBox,
                        },
                        button! {
                            text: SortLabel::new(storage_path),
                            event: cycle_sort(storage_path),
                        },
                    ),
                },
                scroll_view! {
                    children: ItemGrid::new(self.storage_path, ItemSource::Storage, None),
                },
            ),
        }
    }
}
