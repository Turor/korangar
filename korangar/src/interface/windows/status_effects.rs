//! Compact local-player status effect strip (buffs/debuffs by EFST index).

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::status_effects::StatusEffects;
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ClientState, client_theme};

struct StatusEffectsElement<A> {
    status_effects_path: A,
}

impl<A> StatusEffectsElement<A> {
    fn new(status_effects_path: A) -> Self {
        Self { status_effects_path }
    }
}

struct StatusEffectsLayoutInfo {
    area: Area,
    row_height: f32,
}

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
        with_single_resolver(resolvers, |resolver| {
            let effects = state.get(&self.status_effects_path).effects();
            let row_height = 18.0;
            let rows = effects.len().max(1);
            let area = resolver.with_height(row_height * rows as f32);
            Self::LayoutInfo { area, row_height }
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
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + layout_info.row_height * index as f32,
                width: layout_info.area.width,
                height: layout_info.row_height,
            };
            layout.add_text(
                area,
                effect.label.as_str(),
                FontSize(12.0),
                *state.get(&client_theme().tokens().text()),
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
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
