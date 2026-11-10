//! Item Finder shell. Search runs on the local catalog. See
//! `state::item_finder`.

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::graphics::Color;
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::client_theme;
use crate::state::ClientState;
use crate::state::item_finder::{ItemFinderState, ItemFinderStatePathExt};
use crate::state::theme::InterfaceThemeType;

const QUERY_LIMIT: usize = 64;

struct FinderBody<A> {
    finder_path: A,
}

impl<A> FinderBody<A> {
    fn new(finder_path: A) -> Self {
        Self { finder_path }
    }
}

struct FinderBodyLayout {
    area: Area,
    row_height: f32,
}

impl<A> Element<ClientState> for FinderBody<A>
where
    A: Path<ClientState, ItemFinderState>,
{
    type LayoutInfo = FinderBodyLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let rows = state.get(&self.finder_path).rows.len().max(1);
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
        let rows = &state.get(&self.finder_path).rows;
        if rows.is_empty() {
            layout.add_text(
                layout_info.area,
                "No matches.",
                FontSize(12.0),
                *state.get(&client_theme().tokens().text()),
                *state.get(&client_theme().tokens().accent()),
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            return;
        }
        for (index, line) in rows.iter().enumerate() {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + layout_info.row_height * index as f32,
                width: layout_info.area.width,
                height: layout_info.row_height,
            };
            layout.add_text(
                area,
                line.as_str(),
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

pub struct ItemFinderWindow<A> {
    finder_path: A,
}

impl<A> ItemFinderWindow<A> {
    pub fn new(finder_path: A) -> Self {
        Self { finder_path }
    }
}

impl<A> CustomWindow<ClientState> for ItemFinderWindow<A>
where
    A: Path<ClientState, ItemFinderState> + 'static,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ItemFinder)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        struct QueryBox;

        let search = |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::SearchItemFinder);
        };
        let category = |name: &'static str| {
            move |_state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                queue.queue(InputEvent::SetItemFinderCategory { category: name });
            }
        };

        window! {
            title: "Item Finder",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            minimum_width: 420.0,
            maximum_width: 720.0,
            closable: true,
            elements: (
                text! { text: self.finder_path.status(), },
                text_box! {
                    ghost_text: "Name or effect",
                    state: self.finder_path.query(),
                    input_handler: DefaultHandler::<_, _, QUERY_LIMIT>::new(self.finder_path.query(), search),
                    focus_id: QueryBox,
                },
                split! {
                    children: (
                        button! { text: "Weapons", event: category("weapon"), },
                        button! { text: "Armor", event: category("armor"), },
                        button! { text: "Cards", event: category("card"), },
                        button! { text: "Enchants", event: category("enchant"), },
                        button! { text: "Search", event: InputEvent::SearchItemFinder, },
                    ),
                },
                FinderBody::new(self.finder_path),
            ),
        }
    }
}
