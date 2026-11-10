use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use rust_state::{Path, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::state::theme::{BasicInfoThemePathExt, InterfaceThemePathExt};
use crate::state::{ClientState, client_theme};
use crate::world::{exp_bar_ratio, format_exp_percent};

struct BaseExpTip;
struct JobExpTip;

/// Base or job experience bar. The fill uses [`BasicInfoTheme`] tokens.
pub struct ExpBar<C, N> {
    current: C,
    next: N,
    base_experience: bool,
    tip: String,
}

impl<C, N> ExpBar<C, N> {
    pub fn base(current: C, next: N) -> Self {
        Self {
            current,
            next,
            base_experience: true,
            tip: String::new(),
        }
    }

    pub fn job(current: C, next: N) -> Self {
        Self {
            current,
            next,
            base_experience: false,
            tip: String::new(),
        }
    }
}

impl<C, N> Element<ClientState> for ExpBar<C, N>
where
    C: Path<ClientState, u64>,
    N: Path<ClientState, u64>,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let current = state.get(&self.current);
        let next = state.get(&self.next);
        self.tip = format_exp_percent(*current, *next);
        with_single_resolver(resolvers, |resolver| {
            let area = resolver.with_height(8.0);
            BaseLayoutInfo { area }
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
        let next = *state.get(&self.next);
        let ratio = exp_bar_ratio(current, next);
        let track = *state.get(&client_theme().basic_info().exp_track_color());
        let fill = if self.base_experience {
            *state.get(&client_theme().basic_info().base_exp_color())
        } else {
            *state.get(&client_theme().basic_info().job_exp_color())
        };

        layout.add_rectangle(
            layout_info.area,
            CornerDiameter::uniform(4.0),
            track,
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );

        if ratio > 0.0 {
            let mut filled = layout_info.area;
            filled.width = (layout_info.area.width * ratio).clamp(0.0, layout_info.area.width);
            layout.add_rectangle(
                filled,
                CornerDiameter::uniform(4.0),
                fill,
                Color::TRANSPARENT,
                ShadowPadding::uniform(0.0),
            );
        }

        if layout_info.area.check().run(layout) {
            let id = if self.base_experience {
                BaseExpTip.tooltip_id()
            } else {
                JobExpTip.tooltip_id()
            };
            layout.add_tooltip(&self.tip, id);
        }
    }
}
