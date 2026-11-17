use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, PathExt};

use super::basic_info_view::{WeightColor, WeightLabel, ZenyLabel};
use crate::ItemSource;
use crate::input::InputEvent;
use crate::interface::components::inventory_controls::WeightBar;
use crate::interface::components::item_grid::{
    CART_FALLBACK_CAPACITY, CursorReveal, INVENTORY_FALLBACK_CAPACITY, ItemCountLabel, ItemGrid,
};
use crate::interface::components::storage_controls::SavedSearchStrip;
use crate::interface::components::unified_inventory::{AccentTabs, DualPane, PaneLabel, SortButtons};
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::inventory::{Inventory, InventoryPathExt};
use crate::state::inventory_view::InventoryViewPathExt;
use crate::state::localization::LocalizationPathExt;
use crate::state::storage_tools::StorageToolsPathExt;
use crate::state::theme::{BasicInfoThemePathExt, InterfaceThemePathExt, InterfaceThemeType};
use crate::state::unified_inventory::UnifiedInventory;
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme, this_player};
use crate::world::{BasicInfoStatsPathExt, PlayerPathExt};

/// The unified inventory (Matthew t275u). One window holds the bag, the cart
/// and Kafra or guild storage:
///
/// - Category tabs (All, Usable, Equip, Ammo, Etc, Cards, Favorites), the
///   search and the A-Z / Recent / Zeny sort apply to every pane.
/// - With a cart equipped, Inventory / Cart tabs swap to a split view with the
///   cart beside the bag.
/// - Opening storage splits the window into bag | storage until storage closes.
///
/// Each half of the split has its own scroll pane and draws half the
/// columns less one; an accent bar runs down the split.
pub struct InventoryWindow<P> {
    inventory_path: P,
}

impl<P> InventoryWindow<P> {
    pub fn new(inventory_path: P) -> Self {
        Self { inventory_path }
    }
}

fn unified(state: &ClientState) -> &UnifiedInventory {
    client_state().unified_inventory().follow_safe(state)
}

impl<P> CustomWindow<ClientState> for InventoryWindow<P>
where
    P: Path<ClientState, Inventory> + Copy + 'static,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Inventory)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let info = this_player().manually_asserted().basic_info();
        let inventory_path = self.inventory_path;
        let storage_path = client_state().storage();
        let cart_path = client_state().cart();
        let no_action = |_: &rust_state::State<ClientState>, _: &mut EventQueue<ClientState>| {};

        struct InventorySearchBox;

        window! {
            title: client_state().localization().inventory_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            // Closing the window while storage is open also closes storage
            // on the server (see `sync_unified_inventory`).
            closable: true,
            resizable: true,
            minimum_width: crate::interface::windows::resizable::GRID_WINDOW_MINIMUM_WIDTH,
            maximum_width: crate::interface::windows::resizable::GRID_WINDOW_MAXIMUM_WIDTH,
            minimum_height: 240.0,
            maximum_height: 720.0,
            elements: (
                either! {
                    selector: ComputedSelector::new_default(|state: &ClientState| unified(state).shows_cart_tabs()),
                    on_true: AccentTabs::panes(inventory_path),
                    on_false: (),
                },
                AccentTabs::categories(inventory_path),
                text_box! {
                    ghost_text: "Search name, effect or card",
                    state: inventory_path.view().search(),
                    input_handler: DefaultHandler::<_, _, 48>::new(inventory_path.view().search(), no_action),
                    focus_id: InventorySearchBox,
                },
                SortButtons::new(inventory_path),
                either! {
                    selector: ComputedSelector::new_default(|state: &ClientState| unified(state).storage_open()),
                    on_true: (
                        SavedSearchStrip::new(client_state().storage_tools(), inventory_path),
                        split! {
                            gaps: theme().window().gaps(),
                            children: (
                                button! {
                                    text: "Save search",
                                    event: InputEvent::SaveStorageSearch,
                                },
                                button! {
                                    text: "Deposit matching",
                                    event: InputEvent::DepositMatchingToStorage,
                                },
                                button! {
                                    text: "Close storage",
                                    event: InputEvent::CloseStorage,
                                },
                            ),
                        },
                        text! {
                            text: client_state().storage_tools().status(),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                    ),
                    on_false: (),
                },
                text! {
                    text: WeightLabel::new(info.weight(), info.maximum_weight()),
                    color: WeightColor::new(info.weight(), info.maximum_weight()),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                WeightBar::new(info.weight(), info.maximum_weight()),
                text! {
                    text: ZenyLabel::new(info.zeny()),
                    color: client_theme().basic_info().zeny_color(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                either! {
                    selector: ComputedSelector::new_default(|state: &ClientState| unified(state).is_split()),
                    on_true: DualPane::new(
                        (
                            text! {
                                text: PaneLabel::new("Inventory", inventory_path, Some(INVENTORY_FALLBACK_CAPACITY)),
                                overflow_behavior: OverflowBehavior::Shrink,
                            },
                            scroll_view! {
                                reveal: CursorReveal::new(inventory_path, Some(INVENTORY_FALLBACK_CAPACITY)),
                                children: ItemGrid::new(inventory_path, ItemSource::Inventory, Some(INVENTORY_FALLBACK_CAPACITY)).split(),
                            },
                        ),
                        either! {
                            selector: ComputedSelector::new_default(|state: &ClientState| unified(state).storage_open()),
                            on_true: (
                                text! {
                                    text: PaneLabel::new("Storage", storage_path, None),
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                scroll_view! {
                                    children: ItemGrid::new(storage_path, ItemSource::Storage, None).split(),
                                },
                            ),
                            on_false: (
                                text! {
                                    text: PaneLabel::new("Cart", cart_path, Some(CART_FALLBACK_CAPACITY)),
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                scroll_view! {
                                    children: ItemGrid::new(cart_path, ItemSource::Cart, Some(CART_FALLBACK_CAPACITY)).split(),
                                },
                            ),
                        },
                    ),
                    on_false: (
                        text! {
                            text: ItemCountLabel::new(inventory_path, Some(INVENTORY_FALLBACK_CAPACITY)),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        scroll_view! {
                            reveal: CursorReveal::new(inventory_path, Some(INVENTORY_FALLBACK_CAPACITY)),
                            children: ItemGrid::new(inventory_path, ItemSource::Inventory, Some(INVENTORY_FALLBACK_CAPACITY)),
                        },
                    ),
                },
            ),
        }
    }
}
