//! Icon rows shared by the enchant and grade windows.
//!
//! Borders come from the theme. Gold and the hover token are only used when a
//! row is selected or under the pointer.

use std::cmp::Ordering;
use std::sync::Arc;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element, ElementBox, ErasedElement};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use rust_state::{ManuallyAssertExt, Path, State, VecIndexExt};

use crate::graphics::{Color, CornerDiameter, ShadowPadding, Texture};
use crate::input::InputEvent;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::inventory::InventoryPathExt;
use crate::state::picker::PickerRow;
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

/// What a click on a row means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKind {
    EnchantItem,
    EnchantChoice,
    GradeItem,
    GradeMaterial,
    /// Slots and cost lines. They show an icon and do not send a packet.
    Display,
}

struct RowClick<A> {
    row: A,
    kind: PickerKind,
}

impl<A> ClickHandler<ClientState> for RowClick<A>
where
    A: Path<ClientState, PickerRow> + Copy,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        let row = state.get(&self.row);
        match self.kind {
            PickerKind::EnchantItem => queue.queue(InputEvent::EnchantSelectItem { index: row.index }),
            PickerKind::EnchantChoice => queue.queue(InputEvent::EnchantPickChoice {
                index: row.index,
                item_id: row.item_id,
            }),
            PickerKind::GradeItem => queue.queue(InputEvent::GradeEnchantSelect { index: row.index }),
            PickerKind::GradeMaterial => queue.queue(InputEvent::GradeEnchantPick {
                material_index: i32::from(row.index),
            }),
            PickerKind::Display => {}
        }
    }
}

struct PickerRowElement<A> {
    row: A,
    kind: PickerKind,
    click: RowClick<A>,
}

impl<A> PickerRowElement<A>
where
    A: Copy,
{
    fn new(row: A, kind: PickerKind) -> Self {
        Self {
            row,
            kind,
            click: RowClick { row, kind },
        }
    }
}

fn row_texture(state: &State<ClientState>, index: u16, item_id: u32, by_index: bool) -> Option<Arc<Texture>> {
    let items_path = client_state().inventory().items();
    let items = state.get(&items_path);
    let item = if by_index {
        items.iter().find(|item| item.index.0 == index)
    } else if item_id != 0 {
        items.iter().find(|item| item.item_id.0 == item_id)
    } else {
        None
    };
    item.and_then(|item| item.metadata.texture.clone())
}

impl<A> Element<ClientState> for PickerRowElement<A>
where
    A: Path<ClientState, PickerRow> + Copy,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| Self::LayoutInfo {
            area: resolver.with_height(36.0),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let hovered = layout_info.area.check().run(layout);
        let (index, item_id, selected) = {
            let row = state.get(&self.row);
            (row.index, row.item_id, row.selected)
        };
        let border = if selected {
            *state.get(&client_theme().tokens().gold())
        } else if hovered {
            *state.get(&client_theme().tokens().hover())
        } else {
            *state.get(&client_theme().tokens().border())
        };
        let fill = *state.get(&client_theme().tokens().raised());
        let radius = *state.get(&client_theme().tokens().corner_radius());
        layout.add_rectangle(
            layout_info.area,
            CornerDiameter::uniform(radius.min(8.0)),
            border,
            *state.get(&client_theme().tokens().bg()),
            ShadowPadding::diagonal(0.0, 0.0),
        );
        let inset = 2.0;
        layout.add_rectangle(
            Area {
                left: layout_info.area.left + inset,
                top: layout_info.area.top + inset,
                width: (layout_info.area.width - inset * 2.0).max(1.0),
                height: (layout_info.area.height - inset * 2.0).max(1.0),
            },
            CornerDiameter::uniform((radius * 0.6).min(6.0)),
            fill,
            fill,
            ShadowPadding::diagonal(0.0, 0.0),
        );

        let by_index = matches!(self.kind, PickerKind::EnchantItem | PickerKind::GradeItem);
        if let Some(texture) = row_texture(state, index, item_id, by_index) {
            let icon = 24.0;
            layout.add_texture(
                Area {
                    left: layout_info.area.left + 6.0,
                    top: layout_info.area.top + (layout_info.area.height - icon) / 2.0,
                    width: icon,
                    height: icon,
                },
                texture,
                Color::WHITE,
                false,
            );
        }

        let row = state.get(&self.row);
        layout.add_text(
            Area {
                left: layout_info.area.left + 36.0,
                top: layout_info.area.top,
                width: (layout_info.area.width - 42.0).max(1.0),
                height: layout_info.area.height,
            },
            &row.label,
            FontSize(*state.get(&client_theme().tokens().font_size())),
            *state.get(&client_theme().tokens().text()),
            *state.get(&client_theme().tokens().gold()),
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );

        if hovered && self.kind != PickerKind::Display {
            layout.register_click_handler(MouseButton::Left, &self.click);
        }
    }
}

pub struct PickerList<A> {
    rows: A,
    kind: PickerKind,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> PickerList<A> {
    pub fn new(rows: A, kind: PickerKind) -> Self {
        Self {
            rows,
            kind,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for PickerList<A>
where
    A: Path<ClientState, Vec<PickerRow>> + Copy,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let rows = state.get(&self.rows);
            match rows.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(rows.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..rows.len() {
                        let row = self.rows.index(index).manually_asserted();
                        self.elements.push(ErasedElement::new(PickerRowElement::new(row, self.kind)));
                    }
                }
            }

            self.elements.iter_mut().enumerate().for_each(|(index, element)| {
                element.create_layout_info(state, store.child_store(index as u64), resolver);
            });
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        self.elements.iter().enumerate().for_each(|(index, element)| {
            element.lay_out(state, store.child_store(index as u64), &(), layout);
        });
    }
}
