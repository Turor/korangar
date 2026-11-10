//! Styling shop. Hair, color, cloth, and headgear rows. The portrait draws the
//! loaded preview sprite, or the worn player sprite when the selection keeps
//! the current hair.

use std::sync::Arc;

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, Selector, State};

use super::WindowClass;
use super::equipment::{fit_sprite_part, part_is_drawable, sprite_part_top_left};
use crate::graphics::Color;
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::renderer::LayoutExt;
use crate::state::styling_shop::StylingShopStatePathExt;
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme, this_entity};
use crate::world::{AnimationData, Entity};

const PORTRAIT_HEIGHT: f32 = 220.0;

/// Selected tab uses the theme button hover colors. The idle tab keeps the
/// neutral button fill.
struct TabTone {
    headgear: bool,
    ink: bool,
}

impl TabTone {
    fn fill(headgear: bool) -> Self {
        Self { headgear, ink: false }
    }

    fn ink(headgear: bool) -> Self {
        Self { headgear, ink: true }
    }
}

impl Selector<ClientState, Color> for TabTone {
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a Color> {
        let active = client_state().styling_shop().headgear_tab().follow(state).copied().unwrap_or(false);
        let button = client_theme().button().follow(state)?;
        let selected = active == self.headgear;
        Some(match (self.ink, selected) {
            (false, true) => &button.hovered_background_color,
            (false, false) => &button.background_color,
            (true, true) => &button.hovered_foreground_color,
            (true, false) => &button.foreground_color,
        })
    }
}

struct StylingPortrait;

impl Element<ClientState> for StylingPortrait {
    type LayoutInfo = Area;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let origin = resolver.with_height(PORTRAIT_HEIGHT);
            Area {
                left: origin.left,
                top: origin.top,
                width: origin.width,
                height: PORTRAIT_HEIGHT,
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
        paint_styling_portrait(state, layout, *layout_info);
    }
}

fn paint_styling_portrait(state: &State<ClientState>, layout: &mut WindowLayout<ClientState>, area: Area) {
    let theme_path = client_theme().window();
    let window_theme = state.get(&theme_path);
    layout.add_rectangle(
        area,
        window_theme.corner_diameter,
        window_theme.background_color,
        window_theme.shadow_color,
        window_theme.shadow_padding,
    );

    let preview_path = client_state().styling_shop().preview();
    let preview = state.get(&preview_path).clone();
    let animation: Arc<AnimationData> = if let Some(animation) = preview {
        animation
    } else {
        let entity_path = this_entity();
        let Some(Entity::Player(player)) = state.try_get(&entity_path) else {
            return;
        };
        let Some(animation) = player.get_common().animation_data.clone() else {
            return;
        };
        animation
    };

    let Some(frame) = animation.animations.first().and_then(|animation| animation.frames.first()) else {
        return;
    };

    let inset = 4.0;
    let box_size = (area.width - inset * 2.0).min(area.height - inset * 2.0);
    if box_size <= 1.0 {
        return;
    }
    let box_left = area.left + (area.width - box_size) / 2.0;
    let box_top = area.top + (area.height - box_size) / 2.0;

    let mut union: Option<(i32, i32, i32, i32)> = None;
    for part in frame.frame_parts.iter().filter(|part| part_is_drawable(part, &animation)) {
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

    for part in frame.frame_parts.iter().filter(|part| part_is_drawable(part, &animation)) {
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
        let texture = animation.animation_pair[part.animation_index].sprites.textures[part.sprite_number].clone();
        layout.add_texture(Area { left, top, width, height }, texture, part.color, false);
    }
}

pub struct StylingShopWindow;

impl StylingShopWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for StylingShopWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::StylingShop)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().styling_shop();

        window! {
            title: "Styling Shop",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: state_path.tab_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Hair",
                            event: InputEvent::StylingShopTab { headgear: false },
                            background_color: TabTone::fill(false),
                            foreground_color: TabTone::ink(false),
                        },
                        button! {
                            text: "Headgear",
                            event: InputEvent::StylingShopTab { headgear: true },
                            background_color: TabTone::fill(true),
                            foreground_color: TabTone::ink(true),
                        },
                    ),
                },
                StylingPortrait,
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text! {
                            text: state_path.row_a(),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        button! {
                            text: "<",
                            event: InputEvent::StylingShopStep { row: 0, delta: -1 },
                        },
                        button! {
                            text: ">",
                            event: InputEvent::StylingShopStep { row: 0, delta: 1 },
                        },
                    ),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text! {
                            text: state_path.row_b(),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        button! {
                            text: "<",
                            event: InputEvent::StylingShopStep { row: 1, delta: -1 },
                        },
                        button! {
                            text: ">",
                            event: InputEvent::StylingShopStep { row: 1, delta: 1 },
                        },
                    ),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text! {
                            text: state_path.row_c(),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        button! {
                            text: "<",
                            event: InputEvent::StylingShopStep { row: 2, delta: -1 },
                        },
                        button! {
                            text: ">",
                            event: InputEvent::StylingShopStep { row: 2, delta: 1 },
                        },
                    ),
                },
                text! {
                    text: state_path.cost_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Buy",
                    event: InputEvent::StylingShopBuy,
                },
                button! {
                    text: "Reset",
                    event: InputEvent::StylingShopReset,
                },
                button! {
                    text: "Close",
                    event: InputEvent::StylingShopClose,
                },
            ),
        }
    }
}
