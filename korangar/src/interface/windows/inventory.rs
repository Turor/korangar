use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path};

use super::basic_info_view::{WeightColor, WeightLabel, ZenyLabel};
use crate::ItemSource;
use crate::interface::components::inventory_controls::{SortLabel, TabButton, WeightBar, cycle_sort};
use crate::interface::components::item_grid::{CursorReveal, INVENTORY_FALLBACK_CAPACITY, ItemCountLabel, ItemGrid};
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::inventory::{Inventory, InventoryPathExt};
use crate::state::inventory_view::{InventoryTab, InventoryViewPathExt};
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::{BasicInfoThemePathExt, InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme, this_player};
use crate::world::{BasicInfoStatsPathExt, PlayerPathExt};

/// Inventory grid. The inventory hotkey opens this window. Gear does not embed
/// it. Rows scroll up to the server's slot cap.
pub struct InventoryWindow<P> {
    inventory_path: P,
}

impl<P> InventoryWindow<P> {
    pub fn new(inventory_path: P) -> Self {
        Self { inventory_path }
    }
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
        let no_action = |_: &rust_state::State<ClientState>, _: &mut EventQueue<ClientState>| {};

        struct InventorySearchBox;

        window! {
            title: client_state().localization().inventory_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            minimum_height: 240.0,
            maximum_height: 720.0,
            elements: (
                split! {
                    gaps: theme().window().gaps(),
                    children: std::array::from_fn::<_, 5, _>(|tab| TabButton::new(inventory_path, InventoryTab::ALL[tab])),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text_box! {
                            ghost_text: "Search name, effect or card",
                            state: inventory_path.view().search(),
                            input_handler: DefaultHandler::<_, _, 48>::new(inventory_path.view().search(), no_action),
                            focus_id: InventorySearchBox,
                        },
                        button! {
                            text: SortLabel::new(inventory_path),
                            event: cycle_sort(inventory_path),
                        },
                    ),
                },
                text! {
                    text: ItemCountLabel::new(self.inventory_path, Some(INVENTORY_FALLBACK_CAPACITY)),
                    overflow_behavior: OverflowBehavior::Shrink,
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
                scroll_view! {
                    reveal: CursorReveal::new(self.inventory_path, Some(INVENTORY_FALLBACK_CAPACITY)),
                    children: ItemGrid::new(self.inventory_path, ItemSource::Inventory, Some(INVENTORY_FALLBACK_CAPACITY)),
                },
            ),
        }
    }
}
