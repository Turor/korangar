//! Closable skill description. Text comes from the skill-tree window state.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use super::WindowClass;
use super::skill_tree::{SkillTreeWindowState, SkillTreeWindowStatePathExt};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, client_theme};

struct SkillDescriptionElement<A> {
    state_path: A,
    body: String,
}

impl<A> SkillDescriptionElement<A> {
    fn new(state_path: A) -> Self {
        Self {
            state_path,
            body: String::new(),
        }
    }
}

struct SkillDescriptionLayout {
    area: Area,
}

impl<A> Element<ClientState> for SkillDescriptionElement<A>
where
    A: Path<ClientState, SkillTreeWindowState>,
{
    type LayoutInfo = SkillDescriptionLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let body_path = self.state_path.description_body();
        self.body = state.get(&body_path).clone();
        let token_path = client_theme().tokens();
        let font_size = state.get(&token_path).font_size;
        let lines = self.body.lines().filter(|line| !line.is_empty()).count().max(1) as f32;
        with_single_resolver(resolvers, |resolver| Self::LayoutInfo {
            area: resolver.with_height((lines * (font_size + 6.0)).max(48.0)),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let token_path = client_theme().tokens();
        let tokens = state.get(&token_path);
        let text: &str = if self.body.is_empty() {
            "No description"
        } else {
            self.body.as_str()
        };
        layout.add_text(
            layout_info.area,
            text,
            FontSize(tokens.font_size),
            tokens.text,
            tokens.hover,
            HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
            VerticalAlignment::Top { offset: 2.0 },
            OverflowBehavior::LineBreak,
        );
    }
}

pub struct SkillDescriptionWindow<A> {
    state_path: A,
}

impl<A> SkillDescriptionWindow<A> {
    pub fn new(state_path: A) -> Self {
        Self { state_path }
    }
}

impl<A> CustomWindow<ClientState> for SkillDescriptionWindow<A>
where
    A: Path<ClientState, SkillTreeWindowState> + Copy,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::SkillDescription)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: self.state_path.description_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 240.0,
            maximum_width: 420.0,
            minimum_height: 80.0,
            maximum_height: 480.0,
            elements: (
                scroll_view! {
                    children: SkillDescriptionElement::new(self.state_path),
                },
            ),
        }
    }
}
