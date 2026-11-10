//! Hair, color, sex, and Doram controls for the creation window.
//!
//! Cells use theme tokens. A missing GRF thumbnail falls back to the style
//! number. The live sprite is painted from the preview stored on
//! [`crate::state::character_screen::CharacterScreen`].

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::alignment::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use rust_state::State;

use super::equipment::{fit_sprite_part, sprite_part_top_left};
use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::character_screen::CharacterScreen;
use crate::state::theme::InterfaceThemePathExt;
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};
use crate::world::{AnimationData, AnimationFramePart};

/// Twelve columns keep the 23 human styles in two rows and every color in one,
/// so the creation window (name, picker, Create) fits a 720px screen.
const COLUMNS: usize = 12;
const CELL: f32 = 40.0;
const GAP: f32 = 4.0;
const ROW: f32 = 30.0;
const PREVIEW: f32 = 116.0;
const LABEL: f32 = 16.0;

const NUMBERS: [&str; 24] = [
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20", "21", "22", "23",
];

const HAIR: u8 = 0;
const COLOR: u8 = 1;
const RACE: u8 = 2;
const SEX: u8 = 3;

struct AppearancePick {
    kind: u8,
    index: u16,
}

impl ClickHandler<ClientState> for AppearancePick {
    fn handle_click(&self, state: &State<ClientState>, _: &mut EventQueue<ClientState>) {
        let kind = self.kind;
        let index = self.index;
        state.update_value_with(client_state().character_screen(), move |screen| match kind {
            HAIR => screen.set_hair_style(index),
            COLOR => screen.set_hair_color(index),
            RACE => screen.set_doram(index == 1),
            SEX => screen.set_sex(index as u8),
            _ => {}
        });
    }
}

pub struct CharacterAppearance {
    picks: Vec<AppearancePick>,
}

impl CharacterAppearance {
    pub fn new() -> Self {
        let mut picks = Vec::with_capacity(23 + 9 + 4);
        for style in 1..=23 {
            picks.push(AppearancePick { kind: HAIR, index: style });
        }
        for color in 0..=8 {
            picks.push(AppearancePick { kind: COLOR, index: color });
        }
        picks.push(AppearancePick { kind: RACE, index: 0 });
        picks.push(AppearancePick { kind: RACE, index: 1 });
        picks.push(AppearancePick { kind: SEX, index: 0 });
        picks.push(AppearancePick { kind: SEX, index: 1 });
        Self { picks }
    }

    fn hair(&self, style: u16) -> &AppearancePick {
        &self.picks[(style as usize) - 1]
    }

    fn color(&self, color: u16) -> &AppearancePick {
        &self.picks[23 + color as usize]
    }

    fn race(&self, doram: bool) -> &AppearancePick {
        &self.picks[32 + usize::from(doram)]
    }

    fn sex(&self, female: bool) -> &AppearancePick {
        // Pick 34 is sex 0 (female). Pick 35 is sex 1 (male).
        &self.picks[if female { 34 } else { 35 }]
    }
}

impl Default for CharacterAppearance {
    fn default() -> Self {
        Self::new()
    }
}

fn block_height(styles: u16) -> f32 {
    let rows = (styles as usize).div_ceil(COLUMNS) as f32;
    ROW + GAP + ROW + GAP + PREVIEW + GAP + LABEL + rows * (CELL + GAP) + LABEL + CELL
}

impl Element<ClientState> for CharacterAppearance {
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let screen_path = client_state().character_screen();
        let styles = state.get(&screen_path).style_count();
        with_single_resolver(resolvers, |resolver| {
            let area = resolver.with_height(block_height(styles));
            BaseLayoutInfo { area }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _store: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let screen_path = client_state().character_screen();
        let token_path = client_theme().tokens();
        let screen = state.get(&screen_path);
        let tokens = state.get(&token_path);
        let area = layout_info.area;
        let mut top = area.top;

        paint_choice(
            layout,
            area.left,
            top,
            area.width * 0.5 - GAP,
            ROW,
            "Human",
            !screen.doram,
            self.race(false),
            tokens,
        );
        paint_choice(
            layout,
            area.left + area.width * 0.5,
            top,
            area.width * 0.5 - GAP,
            ROW,
            "Doram",
            screen.doram,
            self.race(true),
            tokens,
        );
        top += ROW + GAP;

        let sex_locked = screen.sex_locked;
        paint_choice(
            layout,
            area.left,
            top,
            area.width * 0.5 - GAP,
            ROW,
            "Female",
            screen.sex == 0,
            self.sex(true),
            tokens,
        );
        if !sex_locked && cell_hovered(layout, area.left, top, area.width * 0.5 - GAP, ROW) {
            layout.register_click_handler(MouseButton::Left, self.sex(true));
        }
        paint_choice(
            layout,
            area.left + area.width * 0.5,
            top,
            area.width * 0.5 - GAP,
            ROW,
            "Male",
            screen.sex == 1,
            self.sex(false),
            tokens,
        );
        if !sex_locked && cell_hovered(layout, area.left + area.width * 0.5, top, area.width * 0.5 - GAP, ROW) {
            layout.register_click_handler(MouseButton::Left, self.sex(false));
        }
        // paint_choice only draws. Sex clicks are registered above so a locked account
        // cannot change it; race clicks are registered here.
        if cell_hovered(layout, area.left, top - ROW - GAP, area.width * 0.5 - GAP, ROW) {
            layout.register_click_handler(MouseButton::Left, self.race(false));
        }
        if cell_hovered(
            layout,
            area.left + area.width * 0.5,
            top - ROW - GAP,
            area.width * 0.5 - GAP,
            ROW,
        ) {
            layout.register_click_handler(MouseButton::Left, self.race(true));
        }
        top += ROW + GAP;

        let preview = korangar_interface::layout::area::Area {
            left: area.left,
            top,
            width: area.width,
            height: PREVIEW,
        };
        layout.add_rectangle(
            preview,
            CornerDiameter::uniform(tokens.corner_radius),
            tokens.panel,
            tokens.border,
            ShadowPadding::diagonal(1.0, 2.0),
        );
        paint_sprite(layout, preview, screen);
        top += PREVIEW + GAP;

        layout.add_text(
            label_area(area.left, top, area.width),
            "Hair",
            FontSize(tokens.font_size),
            tokens.text,
            tokens.border,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        top += LABEL;

        let cell = ((area.width - GAP * (COLUMNS as f32 - 1.0)) / COLUMNS as f32).clamp(36.0, CELL);
        let styles = screen.style_count();
        for index in 0..styles {
            let column = (index as usize) % COLUMNS;
            let row = (index as usize) / COLUMNS;
            let left = area.left + column as f32 * (cell + GAP);
            let cell_top = top + row as f32 * (CELL + GAP);
            let style = index + 1;
            let selected = screen.hair_style == style;
            paint_cell(layout, left, cell_top, cell, CELL - 4.0, selected, tokens);
            if let Some(texture) = screen.hair_thumbs.get(index as usize).and_then(|texture| texture.clone()) {
                layout.add_texture(inset(left, cell_top, cell, CELL - 4.0, 3.0), texture, Color::WHITE, false);
            } else {
                layout.add_text(
                    inset(left, cell_top, cell, CELL - 4.0, 2.0),
                    NUMBERS[style as usize],
                    FontSize(tokens.font_size),
                    tokens.text,
                    tokens.border,
                    HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                    VerticalAlignment::Center { offset: 0.0 },
                    OverflowBehavior::Shrink,
                );
            }
            if cell_hovered(layout, left, cell_top, cell, CELL - 4.0) {
                layout.register_click_handler(MouseButton::Left, self.hair(style));
            }
        }
        let rows = (styles as usize).div_ceil(COLUMNS) as f32;
        top += rows * (CELL + GAP);

        layout.add_text(
            label_area(area.left, top, area.width),
            "Color",
            FontSize(tokens.font_size),
            tokens.text,
            tokens.border,
            HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        top += LABEL;

        let colors = screen.color_count();
        for index in 0..colors {
            let left = area.left + index as f32 * (cell + GAP);
            let selected = screen.hair_color == index;
            paint_cell(layout, left, top, cell, CELL - 8.0, selected, tokens);
            if let Some(texture) = screen.color_thumbs.get(index as usize).and_then(|texture| texture.clone()) {
                layout.add_texture(inset(left, top, cell, CELL - 8.0, 3.0), texture, Color::WHITE, false);
            } else {
                layout.add_text(
                    inset(left, top, cell, CELL - 8.0, 2.0),
                    NUMBERS[index as usize],
                    FontSize(tokens.font_size),
                    tokens.text,
                    tokens.border,
                    HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                    VerticalAlignment::Center { offset: 0.0 },
                    OverflowBehavior::Shrink,
                );
            }
            if cell_hovered(layout, left, top, cell, CELL - 8.0) {
                layout.register_click_handler(MouseButton::Left, self.color(index));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_choice(
    layout: &mut WindowLayout<ClientState>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    label: &'static str,
    selected: bool,
    _pick: &AppearancePick,
    tokens: &crate::state::theme::ThemeTokens,
) {
    paint_cell(layout, left, top, width, height, selected, tokens);
    layout.add_text(
        inset(left, top, width, height, 2.0),
        label,
        FontSize(tokens.font_size),
        match selected {
            true => tokens.accent,
            false => tokens.text,
        },
        tokens.border,
        HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
}

fn paint_cell(
    layout: &mut WindowLayout<ClientState>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    selected: bool,
    tokens: &crate::state::theme::ThemeTokens,
) {
    let hovered = cell_hovered(layout, left, top, width, height);
    let border = match (selected, hovered) {
        (true, _) => tokens.accent,
        (false, true) => tokens.hover,
        (false, false) => tokens.border,
    };
    layout.add_rectangle(
        korangar_interface::layout::area::Area { left, top, width, height },
        CornerDiameter::uniform(tokens.corner_radius),
        tokens.raised,
        border,
        ShadowPadding::diagonal(1.0, 2.0),
    );
}

fn cell_hovered(layout: &mut WindowLayout<ClientState>, left: f32, top: f32, width: f32, height: f32) -> bool {
    korangar_interface::layout::area::Area { left, top, width, height }
        .check()
        .run(layout)
}

fn inset(left: f32, top: f32, width: f32, height: f32, pad: f32) -> korangar_interface::layout::area::Area {
    korangar_interface::layout::area::Area {
        left: left + pad,
        top: top + pad,
        width: (width - pad * 2.0).max(1.0),
        height: (height - pad * 2.0).max(1.0),
    }
}

fn label_area(left: f32, top: f32, width: f32) -> korangar_interface::layout::area::Area {
    korangar_interface::layout::area::Area {
        left,
        top,
        width,
        height: LABEL,
    }
}

fn part_is_drawable(part: &AnimationFramePart, animation: &AnimationData) -> bool {
    part.size.x > 0
        && part.size.y > 0
        && part.animation_index < animation.animation_pair.len()
        && part.sprite_number < animation.animation_pair[part.animation_index].sprites.textures.len()
}

fn paint_sprite(layout: &mut WindowLayout<ClientState>, area: korangar_interface::layout::area::Area, screen: &CharacterScreen) {
    let Some(animation) = screen.preview.as_ref() else {
        return;
    };
    let Some(frame) = animation.animations.first().and_then(|animation| animation.frames.first()) else {
        return;
    };
    let inset_px = 6.0;
    let box_size = (area.width - inset_px * 2.0).min(area.height - inset_px * 2.0);
    if box_size <= 1.0 {
        return;
    }
    let box_left = area.left + (area.width - box_size) / 2.0;
    let box_top = area.top + (area.height - box_size) / 2.0;
    let mut union: Option<(i32, i32, i32, i32)> = None;
    for part in frame.frame_parts.iter().filter(|part| part_is_drawable(part, animation)) {
        let (x, y) = sprite_part_top_left(part.offset.x, part.offset.y, part.size.x, part.size.y);
        union = Some(match union {
            None => (x, y, x + part.size.x, y + part.size.y),
            Some((min_x, min_y, max_x, max_y)) => (
                min_x.min(x),
                min_y.min(y),
                max_x.max(x + part.size.x),
                max_y.max(y + part.size.y),
            ),
        });
    }
    let Some((min_x, min_y, max_x, max_y)) = union else {
        return;
    };
    for part in frame.frame_parts.iter().filter(|part| part_is_drawable(part, animation)) {
        let (x, y) = sprite_part_top_left(part.offset.x, part.offset.y, part.size.x, part.size.y);
        let (left, top, width, height) = fit_sprite_part(
            min_x,
            min_y,
            max_x - min_x,
            max_y - min_y,
            x,
            y,
            part.size.x,
            part.size.y,
            box_left,
            box_top,
            box_size,
        );
        let replacement = match part.animation_index {
            1 => screen
                .head_textures
                .as_ref()
                .and_then(|textures| textures.get(part.sprite_number))
                .cloned(),
            _ => None,
        };
        let texture =
            replacement.unwrap_or_else(|| animation.animation_pair[part.animation_index].sprites.textures[part.sprite_number].clone());
        layout.add_texture(
            korangar_interface::layout::area::Area { left, top, width, height },
            texture,
            part.color,
            false,
        );
    }
}

#[cfg(test)]
mod layout_tests {
    use super::block_height;

    #[test]
    fn picker_leaves_room_for_name_and_create_at_720p() {
        // The window adds a title, the name box, and the Create button (about
        // 110px) and opens about 150px from the top of a 720px screen.
        assert!(block_height(23) <= 420.0, "picker height {}", block_height(23));
        assert!(block_height(7) < block_height(23));
    }
}
