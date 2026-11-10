//! Guild emblems drawn above other players.
//!
//! The chat overlay only covers speech. Emblems use the same clip-space
//! projection so every visible entity can show one, not only the local player.

use crate::graphics::{Color, ScreenPosition, ScreenSize};
use crate::renderer::GameInterfaceRenderer;
use crate::world::{Camera, Entity};

/// Lift above the entity origin. Speech bubbles use 14; emblems sit higher.
const EMBLEM_WORLD_LIFT: f32 = 22.0;
const PIXEL_SCALE: f32 = 2.0;

pub(crate) struct EmblemSprite {
    pub guild_id: u32,
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub(crate) struct EmblemOverlayFrame<'a> {
    pub renderer: &'a GameInterfaceRenderer,
    pub camera: &'a dyn Camera,
    pub entities: &'a [Entity],
    pub emblems: &'a [EmblemSprite],
    pub window_size: ScreenSize,
}

pub(crate) fn render(frame: &EmblemOverlayFrame<'_>) {
    if frame.emblems.is_empty() {
        return;
    }
    let view_projection = frame.camera.view_projection_matrix();

    for entity in frame.entities {
        let guild_id = entity.guild_id();
        let version = entity.emblem_version();
        if guild_id == 0 || version == 0 {
            continue;
        }
        let Some(emblem) = frame
            .emblems
            .iter()
            .find(|emblem| emblem.guild_id == guild_id && emblem.version == version)
        else {
            continue;
        };
        if emblem.width == 0 || emblem.height == 0 || emblem.pixels.is_empty() {
            continue;
        }

        let mut world = entity.get_position();
        world.y += EMBLEM_WORLD_LIFT;
        let clip = view_projection * world.to_homogeneous();
        if clip.w <= 0.1 {
            continue;
        }

        let screen = frame.camera.clip_to_screen_space(clip);
        let anchor_x = screen.x * frame.window_size.width;
        let anchor_y = screen.y * frame.window_size.height;
        let box_width = emblem.width as f32 * PIXEL_SCALE;
        let box_height = emblem.height as f32 * PIXEL_SCALE;
        let left = anchor_x - box_width / 2.0;
        let top = anchor_y - box_height;

        let width = emblem.width as usize;
        let height = emblem.height as usize;
        for y in 0..height {
            for x in 0..width {
                let index = (y * width + x) * 4;
                let Some(alpha) = emblem.pixels.get(index + 3).copied() else {
                    continue;
                };
                if alpha == 0 {
                    continue;
                }
                let red = emblem.pixels[index];
                let green = emblem.pixels[index + 1];
                let blue = emblem.pixels[index + 2];
                frame.renderer.render_rectangle(
                    ScreenPosition {
                        left: left + x as f32 * PIXEL_SCALE,
                        top: top + y as f32 * PIXEL_SCALE,
                    },
                    ScreenSize {
                        width: PIXEL_SCALE,
                        height: PIXEL_SCALE,
                    },
                    Color::rgba_u8(red, green, blue, alpha),
                );
            }
        }
    }
}
