//! Glow, junk badge, and lock glyph drawn on top of an item slot.
//!
//! [`paint`] is the only call `item_box` makes. The first slot of a frame
//! clears the hover; a hovered slot records the item the next J or L uses.

use korangar_interface::layout::WindowLayout;
use korangar_interface::layout::area::Area;
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use rust_state::State;

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::item_flags::HoverNote;
use crate::state::theme::{InterfaceThemePathExt, ThemeTokensPathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

pub fn paint(state: &State<ClientState>, layout: &mut WindowLayout<ClientState>, area: Area, note: Option<HoverNote>, is_hovered: bool) {
    let hovered = note.clone().filter(|_| is_hovered);
    state.update_value_with(client_state().item_flags(), move |flags| {
        flags.prepare_layout();
        if let Some(hovered) = hovered.clone() {
            flags.note_hover(hovered.key, hovered.index, hovered.name);
        }
    });

    let Some(note) = note else {
        return;
    };
    let appearance = state.get(&client_state().item_flags()).appearance_now(note.key);
    if !appearance.visible() {
        return;
    }

    if appearance.glow > 0 {
        let color = state.get(&client_theme().tokens().item_new_glow()).with_alpha_u8(appearance.glow);
        paint_glow(layout, area, color);
    }
    if appearance.junk {
        let color = *state.get(&client_theme().tokens().item_junk());
        let text = *state.get(&client_theme().tokens().text());
        paint_junk(layout, area, color, text);
    }
    if appearance.locked {
        let color = *state.get(&client_theme().tokens().item_lock());
        paint_lock(layout, area, color);
    }
}

fn paint_glow(layout: &mut WindowLayout<ClientState>, area: Area, color: Color) {
    let thickness = 2.0;
    let bars = [
        Area {
            left: area.left,
            top: area.top,
            width: area.width,
            height: thickness,
        },
        Area {
            left: area.left,
            top: area.top + area.height - thickness,
            width: area.width,
            height: thickness,
        },
        Area {
            left: area.left,
            top: area.top,
            width: thickness,
            height: area.height,
        },
        Area {
            left: area.left + area.width - thickness,
            top: area.top,
            width: thickness,
            height: area.height,
        },
    ];
    for bar in bars {
        layout.add_rectangle(
            bar,
            CornerDiameter::uniform(0.0),
            color,
            Color::TRANSPARENT,
            ShadowPadding::diagonal(0.0, 0.0),
        );
    }
}

fn paint_junk(layout: &mut WindowLayout<ClientState>, area: Area, color: Color, text: Color) {
    let badge = Area {
        left: area.left + 2.0,
        top: area.top + 2.0,
        width: 12.0,
        height: 12.0,
    };
    layout.add_rectangle(
        badge,
        CornerDiameter::uniform(3.0),
        color,
        Color::TRANSPARENT,
        ShadowPadding::diagonal(0.0, 0.0),
    );
    layout.add_text(
        badge,
        "J",
        FontSize(10.0),
        text,
        color,
        HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
}

fn paint_lock(layout: &mut WindowLayout<ClientState>, area: Area, color: Color) {
    let left = area.left + area.width - 12.0;
    let top = area.top + 2.0;
    let parts = [
        Area {
            left: left + 1.0,
            top,
            width: 6.0,
            height: 1.5,
        },
        Area {
            left: left + 1.0,
            top: top + 1.0,
            width: 1.5,
            height: 4.0,
        },
        Area {
            left: left + 5.5,
            top: top + 1.0,
            width: 1.5,
            height: 4.0,
        },
        Area {
            left,
            top: top + 4.0,
            width: 8.0,
            height: 6.0,
        },
    ];
    for part in parts {
        layout.add_rectangle(
            part,
            CornerDiameter::uniform(1.0),
            color,
            Color::TRANSPARENT,
            ShadowPadding::diagonal(0.0, 0.0),
        );
    }
}
