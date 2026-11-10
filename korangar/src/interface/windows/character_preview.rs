//! Live creation preview. Hair thumbnails come from the GRF. The head sprite is
//! recolored with the `.pal` file into textures that are not inserted into the
//! sprite cache.

use std::sync::Arc;

use image::RgbaImage;
use korangar_loaders::FileLoader;
use ragnarok_bytes::{ByteReader, FromBytes};
use ragnarok_formats::sprite::{Palette, PaletteColor, SpriteData};
use ragnarok_formats::version::GenericFormatMetadata;
use ragnarok_packets::EntityId;

use crate::graphics::Texture;
use crate::loaders::{AsyncLoader, GameFileLoader, ImageType, TextureLoader, premultiply_alpha};
use crate::state::character_screen::{self, CharacterScreen};
use crate::world::{
    AnimationData, EntityType, creation_part_files, hair_color_thumb_grf_path, hair_palette_grf_path, hair_palette_id, hair_thumb_grf_path,
};

pub fn sync_preview(
    screen: &mut CharacterScreen,
    async_loader: &AsyncLoader,
    game_files: &GameFileLoader,
    textures: &TextureLoader,
    window_open: bool,
) {
    if !window_open {
        if screen.preview.is_some() || !screen.hair_thumbs.is_empty() {
            screen.clear_preview();
        }
        return;
    }

    if screen.thumbs_need_load() {
        let hair = (1..=screen.style_count())
            .map(|style| load_thumb(game_files, textures, &hair_thumb_grf_path(screen.doram, screen.sex == 0, style)))
            .collect();
        let color = (0..screen.color_count())
            .map(|color| load_thumb(game_files, textures, &hair_color_thumb_grf_path(color)))
            .collect();
        screen.store_thumbs(hair, color);
    }

    if let Some(_key) = screen.take_preview_request() {
        let female = screen.sex == 0;
        let files = creation_part_files(screen.doram, female, screen.hair_style);
        let head = files.get(1).cloned().unwrap_or_default();
        let entity_id = character_screen::preview_entity_id(screen.preview_generation);
        screen.note_pending_entity(entity_id, head);
        if let Some(animation) = async_loader.request_animation_data_load(EntityId(entity_id), EntityType::Player, files) {
            screen.install_preview(entity_id, animation);
        }
    }

    if screen.needs_head_recolor() {
        let female = screen.sex == 0;
        let palette_id = hair_palette_id(sex_of(screen.sex), screen.doram, Some(screen.hair_style as usize));
        let palette = hair_palette_grf_path(screen.doram, female, palette_id, screen.hair_color);
        let textures = recolor_sprite(game_files, textures, &screen.head_sprite, &palette);
        screen.store_head_textures(textures);
    }
}

/// Returns the animation unless this creation preview accepts it.
/// Styling-shop ids and stale creation ids stay with the caller.
pub fn take_preview_animation(
    screen: &mut CharacterScreen,
    entity_id: EntityId,
    animation: Arc<AnimationData>,
) -> Option<Arc<AnimationData>> {
    if character_screen::is_preview_entity(entity_id.0) && screen.install_preview(entity_id.0, Arc::clone(&animation)) {
        return None;
    }
    Some(animation)
}

fn sex_of(sex: u8) -> ragnarok_packets::Sex {
    match sex {
        0 => ragnarok_packets::Sex::Female,
        _ => ragnarok_packets::Sex::Male,
    }
}

fn load_thumb(game_files: &GameFileLoader, textures: &TextureLoader, path: &str) -> Option<Arc<Texture>> {
    let grf_path = format!("data\\texture\\{path}");
    if !game_files.file_exists(&grf_path) {
        return None;
    }
    textures.get_or_load(path, ImageType::Color).ok()
}

fn palette_color(palette: &PaletteColor, index: u8) -> [u8; 4] {
    let alpha = match index {
        0 => 0,
        _ => 255,
    };
    [palette.red, palette.green, palette.blue, alpha]
}

/// Decode a sprite with an optional replacement palette. Palette images are not
/// row-flipped. The result is not stored in [`crate::loaders::SpriteLoader`].
pub fn recolor_sprite(
    game_files: &GameFileLoader,
    textures: &TextureLoader,
    sprite_path: &str,
    palette_path: &str,
) -> Option<Vec<Arc<Texture>>> {
    if sprite_path.is_empty() {
        return None;
    }
    let bytes = game_files.get(&format!("data\\sprite\\{sprite_path}.spr")).ok()?;
    let mut reader = ByteReader::with_default_metadata::<GenericFormatMetadata>(&bytes);
    let mut sprite = SpriteData::from_bytes(&mut reader).ok()?;
    if let Ok(palette_bytes) = game_files.get(palette_path)
        && palette_bytes.len() >= 1024
        && let Ok(palette) = Palette::from_bytes(&mut ByteReader::without_metadata(&palette_bytes[..1024]))
    {
        sprite.palette = Some(palette);
    }
    let palette = sprite.palette.as_ref()?;
    let mut images = Vec::with_capacity(sprite.palette_image_data.len() + sprite.rgba_image_data.len());
    for image in &sprite.palette_image_data {
        let mut data = Vec::with_capacity(image.data.0.len() * 4);
        for index in &image.data.0 {
            data.extend(palette_color(&palette.colors[*index as usize], *index));
        }
        premultiply_alpha(&mut data);
        let texture = textures.create_color(
            palette_path,
            RgbaImage::from_raw(image.width as u32, image.height as u32, data)?,
            true,
        );
        images.push(texture);
    }
    for image in &sprite.rgba_image_data {
        let mut data = image
            .data
            .chunks_exact(4 * image.width as usize)
            .rev()
            .flat_map(|pixels| pixels.chunks_exact(4).flat_map(|pixel| [pixel[3], pixel[2], pixel[1], pixel[0]]))
            .collect::<Vec<_>>();
        premultiply_alpha(&mut data);
        let texture = textures.create_color(
            sprite_path,
            RgbaImage::from_raw(image.width as u32, image.height as u32, data)?,
            true,
        );
        images.push(texture);
    }
    Some(images)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::EntityType;

    fn animation() -> Arc<AnimationData> {
        Arc::new(AnimationData {
            animation_pair: Vec::new(),
            animations: Vec::new(),
            delays: Vec::new(),
            entity_type: EntityType::Player,
        })
    }

    #[test]
    fn styling_sentinel_is_not_a_creation_preview() {
        let styling = 0xF000_0000 | 1;
        assert!(!character_screen::is_preview_entity(styling));
        assert!(character_screen::is_preview_entity(character_screen::preview_entity_id(1)));
        let mut screen = CharacterScreen::default();
        assert_eq!(screen.pending_entity, 0);
        let loaded = animation();
        let returned = take_preview_animation(&mut screen, EntityId(styling), Arc::clone(&loaded));
        let returned = returned.expect("styling load is returned");
        assert!(Arc::ptr_eq(&returned, &loaded));
        assert!(screen.preview.is_none());
    }
}
