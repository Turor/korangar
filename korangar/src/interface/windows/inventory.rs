use korangar_components::item_box;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::InventoryItem;
use rust_state::{ManuallyAssertExt, Path, VecIndexExt};

use crate::ItemSource;
use super::basic_info_view::{WeightColor, WeightLabel, ZenyLabel};
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::{BasicInfoThemePathExt, InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme, this_player};
use crate::world::{BasicInfoStatsPathExt, PlayerPathExt, ResourceMetadata};

/// Inventory grid. The inventory hotkey opens this window. Gear does not embed
/// it.
pub struct InventoryWindow<P> {
    items_path: P,
}

impl<P> InventoryWindow<P> {
    pub fn new(items_path: P) -> Self {
        Self { items_path }
    }
}

impl<P> CustomWindow<ClientState> for InventoryWindow<P>
where
    P: Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Inventory)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        // TODO: Probably this should be more dynamic
        const INVENTORY_ROWS: usize = 4;
        const INVENTORY_COLUMNS: usize = 10;

        let info = this_player().manually_asserted().basic_info();

        window! {
            title: client_state().localization().inventory_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                std::array::from_fn::<_, INVENTORY_ROWS, _>(|row| {
                split! {
                    gaps: theme().window().gaps(),
                    children: std::array::from_fn::<_, INVENTORY_COLUMNS, _>(|column| {
                        let path = self.items_path.index(row * INVENTORY_COLUMNS + column);

                        item_box! {
                            item_path: path,
                            source: ItemSource::Inventory,
                        }
                    }),
                }
            }),
                text! {
                    text: WeightLabel::new(info.weight(), info.maximum_weight()),
                    color: WeightColor::new(info.weight(), info.maximum_weight()),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: ZenyLabel::new(info.zeny()),
                    color: client_theme().basic_info().zeny_color(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
            ),
        }
    }
}
