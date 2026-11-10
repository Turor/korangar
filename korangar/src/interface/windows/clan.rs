//! Clan window: name, master, map, online count, alliances, and antagonists.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::graphics::Color;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::ClientState;
use crate::state::clan::{ClanState, ClanStatePathExt};
use crate::state::theme::InterfaceThemeType;

struct ClanBody<A> {
    clan_path: A,
}

impl<A> ClanBody<A> {
    fn new(clan_path: A) -> Self {
        Self { clan_path }
    }
}

struct ClanBodyLayout {
    area: Area,
    row_height: f32,
}

impl<A> Element<ClientState> for ClanBody<A>
where
    A: Path<ClientState, ClanState>,
{
    type LayoutInfo = ClanBodyLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let rows = state.get(&self.clan_path).rows.len().max(1);
            let row_height = 18.0;
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
        let lines = &state.get(&self.clan_path).rows;

        for (index, line) in lines.iter().enumerate() {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + layout_info.row_height * index as f32,
                width: layout_info.area.width,
                height: layout_info.row_height,
            };
            let color = if line == "Alliances" || line == "Antagonists" {
                Color::rgb_u8(235, 200, 120)
            } else {
                Color::rgb_u8(220, 224, 232)
            };
            layout.add_text(
                area,
                line.as_str(),
                FontSize(12.0),
                color,
                Color::rgb_u8(255, 160, 60),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        }
    }
}

pub struct ClanWindow<A> {
    clan_path: A,
}

impl<A> ClanWindow<A> {
    pub fn new(clan_path: A) -> Self {
        Self { clan_path }
    }
}

impl<A> CustomWindow<ClientState> for ClanWindow<A>
where
    A: Path<ClientState, ClanState>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Clan)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Clan",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            minimum_width: 360.0,
            maximum_width: 560.0,
            closable: true,
            elements: (
                either! {
                    selector: self.clan_path.joined(),
                    on_true: ClanBody::new(self.clan_path),
                    on_false: text! { text: "You are not in a clan.", },
                },
            ),
        }
    }
}
