//! Paint item tooltip lines with the theme gold and accent.
//!
//! The second tooltip is the worn compare block. The shared tooltip renderer
//! stacks immediate tooltips vertically beside the cursor.

use korangar_networking::InventoryItem;
use rust_state::State;

use crate::graphics::Color;
use crate::interface::resource::ItemSource;
use crate::state::inventory::InventoryPathExt;
use crate::state::item_compare::{LineRole, TooltipLine, TooltipTone, item_tooltip_lines};
use crate::state::theme::InterfaceThemePathExt;
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};
use crate::world::ResourceMetadata;

pub struct ItemTooltipText {
    pub body: String,
    pub compare: String,
}

/// Hover and right-click text. Compare is filled only while Shift is held over
/// an inventory, storage, or cart item.
pub fn inventory_tooltips(state: &State<ClientState>, item: &InventoryItem<ResourceMetadata>, source: ItemSource) -> ItemTooltipText {
    let tokens_path = client_theme().tokens();
    let tokens = state.get(&tokens_path);
    let gold = tokens.gold;
    let accent = tokens.accent;
    let finder_path = client_state().item_finder();
    let catalog = state.get(&finder_path).tooltip_catalog();
    let compare_path = client_state().item_compare();
    let shift = state.get(&compare_path).shift_held;
    let comparable = matches!(source, ItemSource::Inventory | ItemSource::Storage | ItemSource::Cart);
    let items_path = client_state().inventory().items();
    let lines = if shift && comparable {
        let worn = state.get(&items_path);
        item_tooltip_lines(item, catalog, worn)
    } else {
        item_tooltip_lines(item, catalog, &[])
    };
    paint_tooltip(&lines, gold, accent)
}

pub fn paint_tooltip(lines: &[TooltipLine], gold: Color, accent: Color) -> ItemTooltipText {
    let gold = color_markup(gold);
    let accent = color_markup(accent);
    let mut body = Vec::new();
    let mut compare = Vec::new();
    for line in lines {
        let text = paint_line(line, &gold, &accent);
        if text.is_empty() {
            continue;
        }
        match line.role {
            LineRole::Item => body.push(text),
            LineRole::Compare => compare.push(text),
        }
    }
    ItemTooltipText {
        body: body.join("\n"),
        compare: compare.join("\n"),
    }
}

fn paint_line(line: &TooltipLine, gold: &str, accent: &str) -> String {
    let mut text = String::new();
    for part in &line.parts {
        if part.text.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        match part.tone {
            TooltipTone::Body => text.push_str(&part.text),
            TooltipTone::Gold => {
                text.push_str(gold);
                text.push_str(&part.text);
                text.push_str("^000000");
            }
            TooltipTone::Accent => {
                text.push_str(accent);
                text.push_str(&part.text);
                text.push_str("^000000");
            }
        }
    }
    text
}

fn color_markup(color: Color) -> String {
    format!(
        "^{:02X}{:02X}{:02X}",
        color.red_as_u8(),
        color.green_as_u8(),
        color.blue_as_u8()
    )
}
