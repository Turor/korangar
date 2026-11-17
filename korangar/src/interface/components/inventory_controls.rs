//! Weight bar for the inventory (inventory review #12). The tabs and sort
//! buttons live in `unified_inventory`.

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use rust_state::{Path, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::state::inventory_view::{WEIGHT_MARKS, weight_fraction};
use crate::state::theme::{BasicInfoThemePathExt, InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::{ClientState, client_theme};
use crate::world::{WeightPressure, weight_pressure};

pub const WEIGHT_BAR_HEIGHT: f32 = 8.0;

/// Carried weight as a bar. Fill uses the weight status colours; tick marks
/// sit at 50% (no natural HP/SP regen) and 90% (no attacks or skills).
pub struct WeightBar<A, B> {
    current: A,
    maximum: B,
}

impl<A, B> WeightBar<A, B> {
    pub fn new(current: A, maximum: B) -> Self {
        Self { current, maximum }
    }
}

impl<A, B> Element<ClientState> for WeightBar<A, B>
where
    A: Path<ClientState, u32>,
    B: Path<ClientState, u32>,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| BaseLayoutInfo {
            area: resolver.with_height(WEIGHT_BAR_HEIGHT),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let current = *state.get(&self.current);
        let maximum = *state.get(&self.maximum);
        let area = layout_info.area;
        let tokens = client_theme().tokens();
        let weight = client_theme().basic_info();
        let fill = match weight_pressure(current, maximum) {
            WeightPressure::Normal => *state.get(&weight.weight_color()),
            WeightPressure::Caution => *state.get(&weight.weight_caution_color()),
            WeightPressure::Overweight => *state.get(&weight.weight_over_color()),
        };
        let none = Color::rgba_u8(0, 0, 0, 0);
        let flat = ShadowPadding::diagonal(0.0, 0.0);
        layout.add_rectangle(area, CornerDiameter::uniform(3.0), *state.get(&tokens.raised()), none, flat);
        let width = area.width * weight_fraction(current, maximum);
        if width > 0.0 {
            layout.add_rectangle(Area { width, ..area }, CornerDiameter::uniform(3.0), fill, none, flat);
        }
        let mark = *state.get(&tokens.muted());
        for fraction in WEIGHT_MARKS {
            layout.add_rectangle(
                Area {
                    left: area.left + area.width * fraction - 1.0,
                    top: area.top,
                    width: 2.0,
                    height: area.height,
                },
                CornerDiameter::uniform(0.0),
                mark,
                none,
                flat,
            );
        }
    }
}
