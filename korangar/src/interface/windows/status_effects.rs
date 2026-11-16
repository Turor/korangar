//! Detail list of local-player statuses: icon, live countdown, and the
//! client tooltip. The always-on icon strip is in the status panel.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::graphics::Color;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::status_effects::{StatusEffects, format_remaining, remaining_ms};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

#[allow(dead_code)]
struct StatusRowTip(u16);

/// Live `Name 12s` label for one detail row.
fn detail_label(name: &str, remaining_milliseconds: u32) -> String {
    match format_remaining(remaining_milliseconds) {
        time if time.is_empty() => name.to_owned(),
        time => format!("{name} {time}"),
    }
}

struct StatusEffectsElement<A> {
    status_effects_path: A,
    /// Rebuilt every layout so the countdown stays live.
    labels: Vec<String>,
}

impl<A> StatusEffectsElement<A> {
    fn new(status_effects_path: A) -> Self {
        Self {
            status_effects_path,
            labels: Vec::new(),
        }
    }
}

struct StatusEffectsLayoutInfo {
    area: Area,
    row_height: f32,
}

const ROW_HEIGHT: f32 = 22.0;
const ROW_ICON_SIDE: f32 = 20.0;

impl<A> Element<ClientState> for StatusEffectsElement<A>
where
    A: Path<ClientState, StatusEffects>,
{
    type LayoutInfo = StatusEffectsLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let now = *state.get(&client_state().client_tick());
        let effects = state.get(&self.status_effects_path).effects();
        self.labels = effects
            .iter()
            .map(|effect| {
                let name = effect.tooltip.lines().next().unwrap_or_default();
                detail_label(name, remaining_ms(effect, now))
            })
            .collect();
        with_single_resolver(resolvers, |resolver| {
            let rows = effects.len().max(1);
            let area = resolver.with_height(ROW_HEIGHT * rows as f32);
            Self::LayoutInfo {
                area,
                row_height: ROW_HEIGHT,
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let effects = state.get(&self.status_effects_path).effects();
        if effects.is_empty() {
            layout.add_text(
                layout_info.area,
                "No active statuses",
                FontSize(12.0),
                *state.get(&client_theme().tokens().muted()),
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            return;
        }

        for (index, effect) in effects.iter().enumerate() {
            let row = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + layout_info.row_height * index as f32,
                width: layout_info.area.width,
                height: layout_info.row_height,
            };
            let icon_area = Area {
                left: row.left + 2.0,
                top: row.top + (row.height - ROW_ICON_SIDE) / 2.0,
                width: ROW_ICON_SIDE,
                height: ROW_ICON_SIDE,
            };
            if let Some(icon) = effect.icon.as_ref() {
                layout.add_texture(icon_area, icon.clone(), Color::WHITE, true);
            }
            let text_area = Area {
                left: icon_area.left + ROW_ICON_SIDE + 4.0,
                top: row.top,
                width: (row.width - ROW_ICON_SIDE - 6.0).max(0.0),
                height: row.height,
            };
            if let Some(label) = self.labels.get(index) {
                layout.add_text(
                    text_area,
                    label.as_str(),
                    FontSize(12.0),
                    *state.get(&client_theme().tokens().text()),
                    *state.get(&client_theme().tokens().accent()),
                    HorizontalAlignment::Left { offset: 0.0, border: 2.0 },
                    VerticalAlignment::Center { offset: 0.0 },
                    OverflowBehavior::Shrink,
                );
            }
            if row.check().run(layout) {
                layout.add_tooltip(effect.tooltip.as_str(), StatusRowTip(effect.status_index).tooltip_id());
            }
        }
    }
}

pub struct StatusEffectsWindow<A> {
    status_effects_path: A,
}

impl<A> StatusEffectsWindow<A> {
    pub fn new(status_effects_path: A) -> Self {
        Self { status_effects_path }
    }
}

impl<A> CustomWindow<ClientState> for StatusEffectsWindow<A>
where
    A: Path<ClientState, StatusEffects>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::StatusEffects)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Status",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 160.0,
            maximum_width: 280.0,
            elements: (
                crate::interface::windows::WindowGroupTabs::new(WindowClass::StatusEffects),
                scroll_view! {
                    children: StatusEffectsElement::new(self.status_effects_path),
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::detail_label;

    #[test]
    fn detail_label_appends_the_live_time() {
        assert_eq!(detail_label("Blessing", 4200), "Blessing 5s");
        assert_eq!(detail_label("Riding", 0), "Riding");
    }
}
