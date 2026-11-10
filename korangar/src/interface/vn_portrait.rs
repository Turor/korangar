//! Framed portraits on both ends of the bottom dialog band.
//!
//! The left figure is the player's body and head, the same composed animation
//! the world draws, plus headgear and a palette swap when those files exist.
//! The right figure is the server cutin, or the NPC idle sprite facing left.

use std::sync::Arc;

use cgmath::Vector2;
use ragnarok_formats::action::SpriteClip;
use ragnarok_packets::Sex;
use rust_state::State;

use crate::graphics::{Color, ScreenPosition, ScreenSize, Texture};
use crate::interface::windows::equipment::{part_is_drawable, sprite_part_top_left};
use crate::loaders::{ActionLoader, GameFileLoader, ImageType, Sprite, SpriteLoader, TextureLoader};
use crate::renderer::GameInterfaceRenderer;
use crate::state::theme::{InterfaceThemePathExt, ThemeTokens};
use crate::state::vn_dialog::{
    self, FACING_LEFT, FRAME_BORDER, PortraitFrame, PortraitSubject, RightPortraitSource, band_layout, headgear_sprite_candidates,
    idle_animation_index, npc_is_speaking, place_portrait_part, portrait_scale, right_portrait_source, side_alpha,
};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme, this_entity};
use crate::world::{Actions, AnimationData, AnimationFrame, Entity, EntityType, Library};

struct Stamp {
    texture: Arc<Texture>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    flip: bool,
    color: Color,
}

#[allow(clippy::too_many_arguments)]
pub fn render_vn_portraits(
    state: &State<ClientState>,
    game_file_loader: &GameFileLoader,
    sprite_loader: &SpriteLoader,
    action_loader: &ActionLoader,
    texture_loader: &TextureLoader,
    library: &Library,
    renderer: &GameInterfaceRenderer,
    screen: ScreenSize,
    dialog_open: bool,
) {
    if !dialog_open {
        return;
    }

    let layout = band_layout(screen.width, screen.height);
    let (player_frame, npc_frame) = (layout.player_frame, layout.npc_frame);
    let dim = state.follow(client_state().game_settings()).dialog_portrait_dim;
    let (npc_speaks, npc_id) = {
        let dialog = state.follow(client_state().dialog_window());
        (
            npc_is_speaking(dialog.is_awaiting_choice(), dialog.input_is_open()),
            dialog.npc_id(),
        )
    };
    let player_alpha = side_alpha(!npc_speaks, dim);
    let npc_alpha = side_alpha(npc_speaks, dim);

    {
        let tokens_path = client_theme().tokens();
        let tokens = state.get(&tokens_path);
        paint_frame(renderer, player_frame, tokens, !npc_speaks);
        paint_frame(renderer, npc_frame, tokens, npc_speaks);
    }
    let (player_left, player_top, player_size) = player_frame.inner();
    let (npc_left, npc_top, npc_size) = npc_frame.inner();

    if let Some(Entity::Player(player)) = state.try_follow(this_entity())
        && let Some(animation) = player.get_common().animation_data.as_ref()
    {
        let portrait = vn_dialog::player_portrait_parts(
            player.head_top,
            player.head_mid,
            player.head_bottom,
            player.hair_color,
            player.cloth_color,
        );
        debug_assert!(portrait.includes_body && portrait.includes_head);
        if let Some(frame) = idle_frame(animation, usize::from(portrait.facing)) {
            let parts = player.get_entity_part_files(library);
            let body_dye = parts.first().and_then(|path| {
                vn_dialog::body_palette_file(path, portrait.body_palette)
                    .filter(|palette| game_file_loader.file_exists(palette))
                    .and_then(|palette| sprite_loader.get_with_palette(&format!("{path}.spr"), &palette).ok())
            });
            let head_dye = parts.get(1).and_then(|path| {
                vn_dialog::head_palette_file(path, portrait.hair_palette)
                    .filter(|palette| game_file_loader.file_exists(palette))
                    .and_then(|palette| sprite_loader.get_with_palette(&format!("{path}.spr"), &palette).ok())
            });
            let sex_folder = match player.get_common().sex {
                Sex::Female => "여",
                _ => "남",
            };
            let mut stamps = frame_stamps(animation, frame, body_dye.as_ref(), head_dye.as_ref());
            for view_id in [portrait.head_bottom, portrait.head_mid, portrait.head_top] {
                stamps.extend(headgear_stamps(
                    game_file_loader,
                    sprite_loader,
                    action_loader,
                    library,
                    view_id,
                    sex_folder,
                    usize::from(portrait.facing),
                ));
            }
            paint_stamps(renderer, &stamps, player_left, player_top, player_size, player_alpha);
        }
    }

    let npc = state
        .follow(client_state().entities())
        .iter()
        .find(|entity| entity.get_entity_id() == npc_id);
    let subject = match npc.map(Entity::get_entity_type) {
        Some(EntityType::Npc) => PortraitSubject::Npc,
        Some(EntityType::Monster) => PortraitSubject::Monster,
        Some(EntityType::Warp) => PortraitSubject::Warp,
        Some(EntityType::Hidden) => PortraitSubject::Hidden,
        Some(EntityType::Player) => PortraitSubject::Player,
        None => PortraitSubject::Missing,
    };
    let cutin = state
        .follow(client_state().npc_presentation())
        .cutin()
        .map(|(path, _)| path.to_owned());
    let source = right_portrait_source(cutin.is_some(), subject);

    match source {
        RightPortraitSource::Cutin => {
            if let Some(path) = cutin
                && let Ok(texture) = texture_loader.get_or_load(&path, ImageType::Color)
            {
                paint_texture(renderer, &texture, npc_left, npc_top, npc_size, npc_alpha);
            }
        }
        RightPortraitSource::Sprite => {
            if let Some(animation) = npc.and_then(|entity| match entity {
                Entity::Npc(npc) => npc.get_common().animation_data.as_ref(),
                Entity::Player(_) => None,
            }) && let Some(frame) = idle_frame(animation, FACING_LEFT)
            {
                let stamps = frame_stamps(animation, frame, None, None);
                paint_stamps(renderer, &stamps, npc_left, npc_top, npc_size, npc_alpha);
            }
        }
        RightPortraitSource::None => {}
    }
}

fn idle_frame(animation: &AnimationData, direction: usize) -> Option<&AnimationFrame> {
    let index = idle_animation_index(direction, animation.animations.len())?;
    animation.animations.get(index)?.frames.first()
}

fn frame_stamps(
    animation: &AnimationData,
    frame: &AnimationFrame,
    body_dye: Option<&Arc<Sprite>>,
    head_dye: Option<&Arc<Sprite>>,
) -> Vec<Stamp> {
    frame
        .frame_parts
        .iter()
        .filter(|part| part_is_drawable(part, animation))
        .filter_map(|part| {
            let dyed = match part.animation_index {
                0 => body_dye,
                1 => head_dye,
                _ => None,
            };
            let texture = dyed
                .and_then(|sprite| sprite.textures.get(part.sprite_number))
                .or_else(|| {
                    animation.animation_pair[part.animation_index]
                        .sprites
                        .textures
                        .get(part.sprite_number)
                })?
                .clone();
            let (x, y) = sprite_part_top_left(part.offset.x, part.offset.y, part.size.x, part.size.y);
            Some(Stamp {
                texture,
                x,
                y,
                w: part.size.x,
                h: part.size.y,
                flip: part.mirror,
                color: part.color,
            })
        })
        .collect()
}

fn headgear_stamps(
    game_file_loader: &GameFileLoader,
    sprite_loader: &SpriteLoader,
    action_loader: &ActionLoader,
    library: &Library,
    view_id: u16,
    sex_folder: &str,
    direction: usize,
) -> Vec<Stamp> {
    let suffix = library.accessory_sprite_suffix(view_id);
    for path in headgear_sprite_candidates(view_id, sex_folder, suffix) {
        let sprite_file = format!("{path}.spr");
        let action_file = format!("{path}.act");
        if !game_file_loader.file_exists(&format!("data\\sprite\\{sprite_file}"))
            || !game_file_loader.file_exists(&format!("data\\sprite\\{action_file}"))
        {
            continue;
        }
        let Ok(sprite) = sprite_loader.get_or_load(&sprite_file) else {
            continue;
        };
        let Ok(actions) = action_loader.get_or_load(&action_file) else {
            continue;
        };
        return action_stamps(&actions, &sprite, direction);
    }
    Vec::new()
}

fn action_stamps(actions: &Actions, sprite: &Sprite, direction: usize) -> Vec<Stamp> {
    let Some(action_index) = idle_animation_index(direction, actions.actions.len()) else {
        return Vec::new();
    };
    let Some(action) = actions.actions.get(action_index) else {
        return Vec::new();
    };
    let Some(motion) = action.motions.first() else {
        return Vec::new();
    };

    motion.sprite_clips.iter().filter_map(|clip| clip_stamp(clip, sprite)).collect()
}

fn clip_stamp(clip: &SpriteClip, sprite: &Sprite) -> Option<Stamp> {
    if clip.sprite_number < 0 {
        return None;
    }
    let mut index = clip.sprite_number as usize;
    if clip.sprite_type == Some(1) {
        index += sprite.palette_size;
    }
    let texture = sprite.textures.get(index)?.clone();
    let image = texture.get_size();
    let zoom = clip.zoom.unwrap_or(1.0);
    let zoom2 = clip.zoom2.unwrap_or_else(|| Vector2::new(1.0, 1.0));
    let w = ((image.width as f32) * zoom2.x * zoom).ceil().max(1.0) as i32;
    let h = ((image.height as f32) * zoom2.y * zoom).ceil().max(1.0) as i32;
    let (x, y) = sprite_part_top_left(clip.position.x, clip.position.y, w, h);
    Some(Stamp {
        texture,
        x,
        y,
        w,
        h,
        flip: clip.mirror_on != 0,
        color: clip_color(clip.color),
    })
}

fn clip_color(color: Option<u32>) -> Color {
    let Some(color) = color else {
        return Color::WHITE;
    };
    let alpha = ((color >> 24) & 0xFF) as f32 / 255.0;
    if alpha <= 0.0 {
        return Color::WHITE;
    }
    Color {
        red: (color & 0xFF) as f32 / 255.0,
        green: ((color >> 8) & 0xFF) as f32 / 255.0,
        blue: ((color >> 16) & 0xFF) as f32 / 255.0,
        alpha,
    }
}

/// A themed panel behind a portrait: the border token around a panel fill.
/// The speaker's border uses the accent token.
fn paint_frame(renderer: &GameInterfaceRenderer, frame: PortraitFrame, tokens: &ThemeTokens, speaker: bool) {
    if frame.size <= FRAME_BORDER * 2.0 {
        return;
    }
    let border = if speaker { tokens.accent } else { tokens.border };
    renderer.render_rectangle(
        ScreenPosition {
            left: frame.left,
            top: frame.top,
        },
        ScreenSize {
            width: frame.size,
            height: frame.size,
        },
        border,
    );
    renderer.render_rectangle(
        ScreenPosition {
            left: frame.left + FRAME_BORDER,
            top: frame.top + FRAME_BORDER,
        },
        ScreenSize {
            width: frame.size - FRAME_BORDER * 2.0,
            height: frame.size - FRAME_BORDER * 2.0,
        },
        // Opaque, like the dialog band behind the text.
        tokens.panel.with_alpha_u8(255),
    );
}

fn paint_stamps(renderer: &GameInterfaceRenderer, stamps: &[Stamp], box_left: f32, box_top: f32, box_size: f32, alpha: f32) {
    let mut union: Option<(i32, i32, i32, i32)> = None;
    for stamp in stamps {
        union = Some(match union {
            None => (stamp.x, stamp.y, stamp.x + stamp.w, stamp.y + stamp.h),
            Some((min_x, min_y, max_x, max_y)) => (
                min_x.min(stamp.x),
                min_y.min(stamp.y),
                max_x.max(stamp.x + stamp.w),
                max_y.max(stamp.y + stamp.h),
            ),
        });
    }
    let Some((min_x, min_y, max_x, max_y)) = union else {
        return;
    };
    let (art_w, art_h) = ((max_x - min_x) as f32, (max_y - min_y) as f32);
    let scale = portrait_scale(art_w, art_h, box_size);
    if scale <= 0.0 {
        return;
    }

    for stamp in stamps {
        let part = (
            (stamp.x - min_x) as f32,
            (stamp.y - min_y) as f32,
            stamp.w as f32,
            stamp.h as f32,
        );
        let (left, top, width, height) = place_portrait_part(art_w, art_h, part, scale, box_left, box_top, box_size);
        let tint = if stamp.color.alpha <= 0.001 { Color::WHITE } else { stamp.color };
        renderer.render_sprite_flipped(
            stamp.texture.clone(),
            ScreenPosition { left, top },
            ScreenSize { width, height },
            tint.multiply_alpha(alpha),
            stamp.flip,
        );
    }
}

fn paint_texture(renderer: &GameInterfaceRenderer, texture: &Arc<Texture>, box_left: f32, box_top: f32, box_size: f32, alpha: f32) {
    let size = texture.get_size();
    let (art_w, art_h) = (size.width as f32, size.height as f32);
    let scale = portrait_scale(art_w, art_h, box_size);
    if scale <= 0.0 {
        return;
    }
    let (left, top, width, height) = place_portrait_part(art_w, art_h, (0.0, 0.0, art_w, art_h), scale, box_left, box_top, box_size);
    renderer.render_sprite_flipped(
        Arc::clone(texture),
        ScreenPosition { left, top },
        ScreenSize { width, height },
        Color::WHITE.multiply_alpha(alpha),
        false,
    );
}
