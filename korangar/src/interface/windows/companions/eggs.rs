//! Pet egg picker opened by `ZC_PETEGG_LIST`. Choosing a row sends the wire
//! index. Close sends nothing.

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use super::bars::paint_icon_line;
use super::state::{CompanionWindowStatePathExt, EggChoice};
use crate::graphics::{CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::interface::font_scale::from_theme_body;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

const ROW_HEIGHT: f32 = 32.0;

struct EggClick<P> {
    eggs: P,
    index: usize,
}

impl<P> ClickHandler<ClientState> for EggClick<P>
where
    P: Path<ClientState, Vec<EggChoice>>,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        let Some(egg) = state.get(&self.eggs).get(self.index) else {
            return;
        };
        queue.queue(InputEvent::SelectPetEgg {
            wire_index: egg.wire_index,
        });
    }
}

pub(crate) struct EggList<P> {
    eggs: P,
    clicks: Vec<EggClick<P>>,
}

impl<P> EggList<P>
where
    P: Clone,
{
    pub(crate) fn new(eggs: P) -> Self {
        Self { eggs, clicks: Vec::new() }
    }
}

impl<P> Element<ClientState> for EggList<P>
where
    P: Path<ClientState, Vec<EggChoice>> + Clone + 'static,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let len = state.get(&self.eggs).len().max(1);
        if self.clicks.len() != state.get(&self.eggs).len() {
            self.clicks = (0..state.get(&self.eggs).len())
                .map(|index| EggClick { eggs: self.eggs, index })
                .collect();
        }
        with_single_resolver(resolvers, |resolver| BaseLayoutInfo {
            area: resolver.with_height(len as f32 * ROW_HEIGHT),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let eggs = state.get(&self.eggs);
        let theme = client_theme().tokens();
        let tokens = state.get(&theme);
        if eggs.is_empty() {
            layout.add_text(
                layout_info.area,
                "No eggs to hatch.",
                crate::loaders::FontSize(tokens.font_size),
                tokens.text,
                tokens.text,
                korangar_interface::prelude::HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                korangar_interface::prelude::VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            return;
        }
        if self.clicks.len() != eggs.len() {
            return;
        }
        let corner = CornerDiameter::uniform(tokens.corner_radius.min(6.0));
        let row_height = layout_info.area.height / eggs.len() as f32;
        for (index, egg) in eggs.iter().enumerate() {
            let row = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + row_height * index as f32,
                width: layout_info.area.width,
                height: row_height,
            };
            let hovered = row.check().run(layout);
            let border = if hovered { tokens.hover } else { tokens.border };
            layout.add_rectangle(row, corner, border, tokens.raised, ShadowPadding::uniform(0.0));
            paint_icon_line(
                layout,
                row,
                egg.texture.as_ref(),
                &egg.label,
                tokens.text,
                from_theme_body(tokens.font_size, 14.0),
            );
            if hovered {
                layout.register_click_handler(MouseButton::Left, &self.clicks[index]);
            }
        }
    }
}

pub struct PetEggWindow;

impl PetEggWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for PetEggWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::PetEggs)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let eggs = client_state().companion_window().eggs();
        window! {
            title: "Pet Eggs",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            minimum_width: 280.0,
            minimum_height: 180.0,
            elements: (
                text! {
                    text: "Choose an egg to hatch.",
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                EggList::new(eggs),
                button! {
                    text: "Close",
                    event: InputEvent::ClosePetEggs,
                },
            ),
        }
    }
}
