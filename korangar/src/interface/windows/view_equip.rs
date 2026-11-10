//! Read-only paperdoll for another player's equipment.
//!
//! Slot frames come from the Gear helpers. This window does not draw the
//! local portrait and does not unequip.

use std::cell::UnsafeCell;

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use ragnarok_packets::EquipPosition;
use rust_state::State;

use super::WindowClass;
use super::equipment::{
    COSTUME_SLOTS, GEAR_ICON_SIDE, GEAR_SLOT_GAP, PAPERDOLL_ROWS, SHADOW_SLOTS, equip_slot_label, equip_slot_tooltip,
    paperdoll_block_height, paperdoll_geometry, square_strip_frames, strip_block_height,
};
use crate::graphics::Color;
use crate::interface::components::item_box::{paint_item_slot, slot_border_and_fill};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::view_equip::{ViewEquipState, ViewEquipStatePathExt, ViewedEquipItem, occupies_slot};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

pub struct ViewEquipWindow;

impl ViewEquipWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for ViewEquipWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ViewEquip)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: client_state().viewed_equipment().character_name(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 280.0,
            minimum_height: 360.0,
            elements: (
                text! {
                    text: client_state().viewed_equipment().summary(),
                },
                ViewEquipPaperdoll { tip: UnsafeCell::new(String::new()) },
            ),
        }
    }
}

struct ViewEquipPaperdoll {
    tip: UnsafeCell<String>,
}

impl Element<ClientState> for ViewEquipPaperdoll {
    type LayoutInfo = Area;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let height = paperdoll_block_height()
                + GEAR_SLOT_GAP
                + strip_block_height(COSTUME_SLOTS.len())
                + GEAR_SLOT_GAP
                + strip_block_height(SHADOW_SLOTS.len());
            resolver.with_height(height)
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let equipment_path = client_state().viewed_equipment();
        let viewed = state.get(&equipment_path);
        let mut hover_text = None;
        let geometry = paperdoll_geometry(layout_info.left, layout_info.top);
        for (index, row) in PAPERDOLL_ROWS.into_iter().enumerate() {
            paint_view_slot(state, layout, viewed, geometry.left_slots[index], row.left, &mut hover_text);
            paint_view_slot(state, layout, viewed, geometry.right_slots[index], row.right, &mut hover_text);
        }
        paint_view_slot(state, layout, viewed, geometry.ammo, EquipPosition::AMMO, &mut hover_text);

        let costume_top = layout_info.top + paperdoll_block_height() + GEAR_SLOT_GAP;
        for (index, frame) in square_strip_frames(COSTUME_SLOTS.len(), layout_info.left, costume_top)
            .into_iter()
            .enumerate()
        {
            paint_view_slot(state, layout, viewed, frame, COSTUME_SLOTS[index], &mut hover_text);
        }
        let shadow_top = costume_top + strip_block_height(COSTUME_SLOTS.len()) + GEAR_SLOT_GAP;
        for (index, frame) in square_strip_frames(SHADOW_SLOTS.len(), layout_info.left, shadow_top)
            .into_iter()
            .enumerate()
        {
            paint_view_slot(state, layout, viewed, frame, SHADOW_SLOTS[index], &mut hover_text);
        }
        if let Some(text) = hover_text {
            let tip = unsafe { &mut *self.tip.get() };
            *tip = text;
            struct ViewEquipTip;
            layout.add_tooltip(tip, ViewEquipTip.tooltip_id());
        }
    }
}

fn paint_view_slot(
    state: &State<ClientState>,
    layout: &mut WindowLayout<ClientState>,
    viewed: &ViewEquipState,
    frame: super::equipment::GearFrame,
    slot: EquipPosition,
    hover_text: &mut Option<String>,
) {
    let area = Area {
        left: frame.left,
        top: frame.top,
        width: frame.width,
        height: frame.height,
    };
    let hovered = area.check().run(layout);
    let (border, fill) = slot_border_and_fill(state, hovered, false);
    paint_item_slot(layout, area, border, fill, 4.0, true);

    if let Some(item) = viewed.items.iter().find(|item| occupies_slot(item.location, slot)) {
        paint_item_icon(layout, area, item);
        if hovered {
            *hover_text = Some(item.label.clone());
        }
    } else if hovered {
        *hover_text = Some(equip_slot_tooltip(slot).to_owned());
    }

    layout.add_text(
        Area {
            left: frame.left,
            top: frame.top + frame.height - 12.0,
            width: frame.width,
            height: 12.0,
        },
        equip_slot_label(slot),
        FontSize(9.0),
        *state.get(&client_theme().tokens().muted()),
        *state.get(&client_theme().tokens().gold()),
        HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
        VerticalAlignment::Top { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
}

fn paint_item_icon(layout: &mut WindowLayout<ClientState>, area: Area, item: &ViewedEquipItem) {
    let Some(texture) = item.texture.as_ref() else {
        return;
    };
    let side = GEAR_ICON_SIDE.min(area.width).min(area.height);
    layout.add_texture(
        Area {
            left: area.left + (area.width - side) / 2.0,
            top: area.top + 4.0,
            width: side,
            height: side,
        },
        texture.clone(),
        Color::WHITE,
        false,
    );
}
