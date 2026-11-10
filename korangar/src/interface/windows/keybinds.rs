//! Shortcut list modeled on roBrowser `ShortCutOption`. Click a row, then press
//! a key. Escape cancels the capture and is never stored.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, State};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::settings::Keybinds;
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, client_theme};

const MAX_BINDS: usize = 96;
const ROW_HEIGHT: f32 = 22.0;

struct RowClick {
    index: u16,
}

impl ClickHandler<ClientState> for RowClick {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(InputEvent::BeginKeybindCapture { index: self.index });
    }
}

struct KeybindList<A> {
    keybinds_path: A,
    clicks: [RowClick; MAX_BINDS],
}

impl<A> KeybindList<A> {
    fn new(keybinds_path: A) -> Self {
        Self {
            keybinds_path,
            clicks: std::array::from_fn(|index| RowClick { index: index as u16 }),
        }
    }
}

struct KeybindListLayout {
    area: Area,
    rows: usize,
}

impl<A> Element<ClientState> for KeybindList<A>
where
    A: Path<ClientState, Keybinds>,
{
    type LayoutInfo = KeybindListLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let rows = state.get(&self.keybinds_path).len().clamp(1, MAX_BINDS);
            Self::LayoutInfo {
                area: resolver.with_height(ROW_HEIGHT * rows as f32),
                rows,
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
        use korangar_interface::prelude::*;

        let keybinds = state.get(&self.keybinds_path);
        let shown = keybinds.len().min(layout_info.rows);
        for index in 0..shown {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + ROW_HEIGHT * index as f32,
                width: layout_info.area.width,
                height: ROW_HEIGHT,
            };
            let hovered = area.check().run(layout);
            let background = match hovered {
                true => *state.get(&client_theme().button().hovered_background_color()),
                false => *state.get(&client_theme().button().background_color()),
            };
            layout.add_rectangle(
                area,
                *state.get(&client_theme().button().corner_diameter()),
                background,
                *state.get(&client_theme().button().shadow_color()),
                *state.get(&client_theme().button().shadow_padding()),
            );
            if hovered {
                layout.register_click_handler(MouseButton::Left, &self.clicks[index]);
            }
            layout.add_text(
                area,
                keybinds.row_label(index),
                *state.get(&client_theme().button().font_size()),
                *state.get(&client_theme().text().color()),
                *state.get(&client_theme().text().highlight_color()),
                HorizontalAlignment::Left { offset: 6.0, border: 4.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        }
    }
}

pub struct KeybindsWindow<A> {
    keybinds_path: A,
}

impl<A> KeybindsWindow<A> {
    pub fn new(keybinds_path: A) -> Self {
        Self { keybinds_path }
    }
}

impl<A> CustomWindow<ClientState> for KeybindsWindow<A>
where
    A: Path<ClientState, Keybinds> + Copy,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Keybinds)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Keybinds",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 280.0,
            maximum_width: 480.0,
            minimum_height: 160.0,
            maximum_height: 520.0,
            elements: (
                text! {
                    text: "Click a row, then press a key. Escape cancels.",
                },
                button! {
                    text: "Reset defaults",
                    event: InputEvent::ResetKeybinds,
                },
                scroll_view! {
                    children: KeybindList::new(self.keybinds_path),
                },
            ),
        }
    }
}
