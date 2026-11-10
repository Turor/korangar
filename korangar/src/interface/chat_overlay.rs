//! Overhead speech bubbles and the top announce banner.
use korangar_networking::MessageColor;

use crate::graphics::{Color, ScreenPosition, ScreenSize};
use crate::loaders::FontSize;
use crate::renderer::{AlignHorizontal, GameInterfaceRenderer};
use crate::state::chat::{AnnounceBanner, ChatBubble, announce_scroll_left, fade_alpha};
use crate::state::theme::ChatTheme;
use crate::world::{Camera, Entity};

/// Lift above the entity origin. World Y is up; one ground tile is 5 units.
const BUBBLE_WORLD_LIFT: f32 = 14.0;
const BUBBLE_PAD_X: f32 = 8.0;
const BUBBLE_PAD_Y: f32 = 4.0;
const ANNOUNCE_TOP: f32 = 36.0;

pub(crate) struct ChatOverlayFrame<'a> {
    pub renderer: &'a GameInterfaceRenderer,
    pub camera: &'a dyn Camera,
    pub entities: &'a [Entity],
    pub bubbles: &'a [ChatBubble],
    pub announce: &'a Option<AnnounceBanner>,
    pub chat_theme: &'a ChatTheme,
    pub font_size: FontSize,
    pub window_size: ScreenSize,
    pub now: u32,
}

pub(crate) fn render(frame: &ChatOverlayFrame<'_>) {
    render_bubbles(frame);
    if let Some(banner) = frame.announce {
        render_announce(frame, banner);
    }
}

fn render_bubbles(frame: &ChatOverlayFrame<'_>) {
    let view_projection = frame.camera.view_projection_matrix();

    for bubble in frame.bubbles {
        let alpha = fade_alpha(bubble.started, bubble.until, frame.now);
        if alpha <= 0.0 || bubble.text.is_empty() {
            continue;
        }
        let Some(entity) = frame.entities.iter().find(|entity| entity.get_entity_id() == bubble.entity_id) else {
            continue;
        };

        let mut world = entity.get_position();
        world.y += BUBBLE_WORLD_LIFT;
        let clip = view_projection * world.to_homogeneous();
        if clip.w <= 0.1 {
            continue;
        }

        let screen = frame.camera.clip_to_screen_space(clip);
        let anchor_x = screen.x * frame.window_size.width;
        let anchor_y = screen.y * frame.window_size.height;
        let text_size = frame.renderer.measure_text(&bubble.text, frame.font_size);
        let box_size = ScreenSize {
            width: text_size.width + BUBBLE_PAD_X * 2.0,
            height: text_size.height + BUBBLE_PAD_Y * 2.0,
        };
        let top_left = ScreenPosition {
            left: anchor_x - box_size.width / 2.0,
            top: anchor_y - box_size.height,
        };

        frame.renderer.render_rectangle(
            top_left,
            box_size,
            frame.chat_theme.bubble_background_color().multiply_alpha(alpha),
        );
        frame.renderer.render_text(
            &bubble.text,
            ScreenPosition {
                left: anchor_x,
                top: top_left.top + BUBBLE_PAD_Y,
            },
            frame.chat_theme.bubble_color().multiply_alpha(alpha),
            frame.font_size,
            AlignHorizontal::Center,
        );
    }
}

fn render_announce(frame: &ChatOverlayFrame<'_>, banner: &AnnounceBanner) {
    let alpha = fade_alpha(banner.started, banner.until, frame.now);
    if alpha <= 0.0 || banner.text.is_empty() {
        return;
    }

    let text_size = frame.renderer.measure_text(&banner.text, frame.font_size);
    let left = announce_scroll_left(
        banner.started,
        banner.until,
        frame.now,
        frame.window_size.width,
        text_size.width,
    );
    let bar_height = text_size.height + BUBBLE_PAD_Y * 2.0;
    frame.renderer.render_rectangle(
        ScreenPosition {
            left: 0.0,
            top: ANNOUNCE_TOP,
        },
        ScreenSize {
            width: frame.window_size.width,
            height: bar_height,
        },
        frame.chat_theme.announce_background_color().multiply_alpha(alpha),
    );
    frame.renderer.render_text(
        &banner.text,
        ScreenPosition {
            left,
            top: ANNOUNCE_TOP + BUBBLE_PAD_Y,
        },
        announce_color(frame.chat_theme, banner.color).multiply_alpha(alpha),
        frame.font_size,
        AlignHorizontal::Left,
    );
}

fn announce_color(chat_theme: &ChatTheme, color: MessageColor) -> Color {
    match color {
        MessageColor::Broadcast => chat_theme.broadcast_color(),
        other => chat_theme.message_color(other),
    }
}
