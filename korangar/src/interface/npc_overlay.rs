//! Cutin illustration and the NPC progress bar drawn over the player.

use std::sync::Arc;

use cgmath::Point3;
use korangar_interface::application::Clip;

use crate::graphics::{Color, ScreenClip, ScreenPosition, ScreenSize};
use crate::loaders::{GameFileLoader, ImageType, TextureLoader};
use crate::renderer::{GameInterfaceRenderer, SpriteRenderer};
use crate::state::npc_dialog::{NpcPresentation, place_cutin, progress_anchor, progress_bar_size, progress_fill_color};
use crate::state::theme::WorldTheme;
use crate::world::Camera;

pub fn render_npc_cutin(
    presentation: &NpcPresentation,
    game_file_loader: &GameFileLoader,
    texture_loader: &TextureLoader,
    renderer: &GameInterfaceRenderer,
    window_size: ScreenSize,
) {
    if let Some((path, anchor)) = presentation.cutin()
        && game_file_loader.file_exists(&format!("data\\texture\\{path}"))
        && let Ok(texture) = texture_loader.get_or_load(path, ImageType::Color)
    {
        let size = texture.get_size();
        let rect = place_cutin(
            anchor,
            size.width as f32,
            size.height as f32,
            window_size.width,
            window_size.height,
        );
        renderer.render_sprite(
            Arc::clone(&texture),
            ScreenPosition {
                left: rect.left,
                top: rect.top,
            },
            ScreenSize {
                width: rect.width,
                height: rect.height,
            },
            ScreenClip::unbound(),
            Color::WHITE,
            false,
        );
    }
}

pub fn render_npc_progress(
    presentation: &NpcPresentation,
    renderer: &GameInterfaceRenderer,
    camera: &dyn Camera,
    theme: &WorldTheme,
    window_size: ScreenSize,
    player_position: Option<Point3<f32>>,
    now_ms: u32,
) {
    if !presentation.progress_active() {
        return;
    }
    let Some(player_position) = player_position else {
        return;
    };

    let clip = camera.view_projection_matrix() * player_position.to_homogeneous();
    let screen = camera.clip_to_screen_space(clip);
    let Some(anchor) = progress_anchor(clip.w, screen.x * window_size.width, screen.y * window_size.height) else {
        return;
    };

    let status = &theme.status_bar;
    let (full, fill) = progress_bar_size(
        status.player_bar_width,
        status.health_height,
        presentation.progress_fraction(now_ms),
    );
    let background = anchor - status.border_size - ScreenSize::only_width(full.width / 2.0);
    let background_size = ScreenSize {
        width: full.width,
        height: full.height,
    } + status.border_size * 2.0;
    renderer.render_rectangle(background, background_size, status.background_color);

    let color = progress_fill_color(presentation.progress_color(), status.spell_point_color);
    renderer.render_rectangle(anchor - ScreenSize::only_width(full.width / 2.0), fill, color);
}
