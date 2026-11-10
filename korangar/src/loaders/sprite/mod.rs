use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::{Arc, Mutex};

use image::RgbaImage;
#[cfg(feature = "debug")]
use korangar_container::CacheStatistics;
use korangar_container::{Cacheable, SimpleCache};
#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, Timer, print_debug};
use korangar_interface::element::StateElement;
use korangar_loaders::FileLoader;
use ragnarok_bytes::{ByteReader, FromBytes};
use ragnarok_formats::sprite::{PaletteColor, RgbaImageData, SpriteData};
use ragnarok_formats::version::GenericFormatMetadata;
use rust_state::RustState;

use super::{FALLBACK_SPRITE_FILE, TextureLoader};
use crate::graphics::Texture;
use crate::loaders::GameFileLoader;
use crate::loaders::color::premultiply_alpha;
use crate::loaders::error::LoadError;

const MAX_CACHE_COUNT: u32 = 4096;
const MAX_CACHE_SIZE: usize = 256 << 20;

#[derive(Clone, Debug, RustState, StateElement)]
pub struct Sprite {
    pub palette_size: usize,
    #[hidden_element]
    pub textures: Vec<Arc<Texture>>,
    #[cfg(feature = "debug")]
    sprite_data: SpriteData,
}

impl Cacheable for Sprite {
    fn size(&self) -> usize {
        self.textures.iter().map(|t| t.get_byte_size()).sum()
    }
}

pub struct SpriteLoader {
    game_file_loader: Arc<GameFileLoader>,
    texture_loader: Arc<TextureLoader>,
    cache: Mutex<SimpleCache<String, Arc<Sprite>>>,
}

impl SpriteLoader {
    pub fn new(game_file_loader: Arc<GameFileLoader>, texture_loader: Arc<TextureLoader>) -> Self {
        Self {
            game_file_loader,
            texture_loader,
            cache: Mutex::new(SimpleCache::new(
                NonZeroU32::new(MAX_CACHE_COUNT).unwrap(),
                NonZeroUsize::new(MAX_CACHE_SIZE).unwrap(),
            )),
        }
    }

    #[cfg(feature = "debug")]
    pub fn cache_statistics(&self) -> CacheStatistics {
        self.cache.lock().unwrap().statistics()
    }

    pub fn load(&self, path: &str) -> Result<Arc<Sprite>, LoadError> {
        #[cfg(feature = "debug")]
        let timer = Timer::new_dynamic(format!("load sprite from {}", path.magenta()));

        let bytes = match self.game_file_loader.get(&format!("data\\sprite\\{path}")) {
            Ok(bytes) => bytes,
            Err(_error) => {
                #[cfg(feature = "debug")]
                {
                    print_debug!("Failed to load sprite: {:?}", _error);
                    print_debug!("Replacing with fallback");
                }

                return self.get_or_load(FALLBACK_SPRITE_FILE);
            }
        };
        let mut byte_reader: ByteReader = ByteReader::with_default_metadata::<GenericFormatMetadata>(&bytes);

        let sprite_data = match SpriteData::from_bytes(&mut byte_reader) {
            Ok(sprite_data) => sprite_data,
            Err(_error) => {
                #[cfg(feature = "debug")]
                {
                    print_debug!("Failed to load sprite: {:?}", _error);
                    print_debug!("Replacing with fallback");
                }

                return self.get_or_load(FALLBACK_SPRITE_FILE);
            }
        };

        let sprite = self.rasterize(path, sprite_data);
        let _result = self.cache.lock().unwrap().insert(path.to_string(), sprite.clone());

        #[cfg(feature = "debug")]
        if let Err(error) = _result {
            print_debug!(
                "[{}] sprite could not be added to cache. Path: '{}': {:?}",
                "error".red(),
                &path,
                error
            );
        }

        #[cfg(feature = "debug")]
        timer.stop();

        Ok(sprite)
    }

    pub fn get(&self, path: &str) -> Option<Arc<Sprite>> {
        self.cache.lock().unwrap().get(path).cloned()
    }

    pub fn get_or_load(&self, path: &str) -> Result<Arc<Sprite>, LoadError> {
        let Some(sprite) = self.get(path) else {
            return self.load(path);
        };

        Ok(sprite)
    }

    /// Same sprite as [`Self::get_or_load`], with the indexed frames dyed by an
    /// external `.pal`. The plain sprite cache entry is left alone.
    pub fn get_with_palette(&self, sprite_path: &str, palette_grf_path: &str) -> Result<Arc<Sprite>, LoadError> {
        let cache_key = format!("{sprite_path}\0{palette_grf_path}");
        if let Some(sprite) = self.get(&cache_key) {
            return Ok(sprite);
        }

        let bytes = self
            .game_file_loader
            .get(&format!("data\\sprite\\{sprite_path}"))
            .map_err(LoadError::File)?;
        let mut byte_reader: ByteReader = ByteReader::with_default_metadata::<GenericFormatMetadata>(&bytes);
        let mut sprite_data = SpriteData::from_bytes(&mut byte_reader).map_err(LoadError::Conversion)?;

        let palette_bytes = self.game_file_loader.get(palette_grf_path).map_err(LoadError::File)?;
        let mut palette_reader: ByteReader = ByteReader::with_default_metadata::<GenericFormatMetadata>(&palette_bytes);
        let palette = ragnarok_formats::sprite::Palette::from_bytes(&mut palette_reader).map_err(LoadError::Conversion)?;
        sprite_data.palette = Some(palette);

        let sprite = self.rasterize(&cache_key, sprite_data);
        let _result = self.cache.lock().unwrap().insert(cache_key, sprite.clone());
        Ok(sprite)
    }

    fn rasterize(&self, path: &str, sprite_data: SpriteData) -> Arc<Sprite> {
        #[cfg(feature = "debug")]
        let cloned_sprite_data = sprite_data.clone();

        // No palette is required for an RGBA-only sprite. Indexed data has already
        // required and validated its palette in SpriteData::from_bytes.
        let palette = sprite_data.palette;

        let rgba_images: Vec<RgbaImageData> = sprite_data
            .rgba_image_data
            .iter()
            .map(|image_data| {
                // Revert the rows, the image is flipped upside down
                // Convert the pixel from ABGR format to RGBA format
                let width = image_data.width;
                let data = image_data
                    .data
                    .chunks_exact(4 * width as usize)
                    .rev()
                    .flat_map(|pixels| {
                        pixels
                            .chunks_exact(4)
                            .flat_map(|pixel| [pixel[3], pixel[2], pixel[1], pixel[0]])
                            .collect::<Vec<u8>>()
                    })
                    .collect();

                RgbaImageData {
                    width: image_data.width,
                    height: image_data.height,
                    data,
                }
            })
            .collect();

        // TODO: Move this to an extension trait in `korangar_loaders`.
        pub fn color_bytes(palette: &PaletteColor, index: u8) -> [u8; 4] {
            let alpha = match index {
                0 => 0,
                _ => 255,
            };

            [palette.red, palette.green, palette.blue, alpha]
        }

        let palette_images = sprite_data.palette_image_data.iter().map(|image_data| {
            let data: Vec<u8> = image_data
                .data
                .0
                .iter()
                .flat_map(|palette_index| {
                    color_bytes(
                        &palette.as_ref().expect("indexed sprite palette validated by parser").colors[*palette_index as usize],
                        *palette_index,
                    )
                })
                .collect();

            RgbaImageData {
                width: image_data.width,
                height: image_data.height,
                data,
            }
        });
        let palette_size = palette_images.len();

        let textures = palette_images
            .chain(rgba_images)
            .map(|mut image_data| {
                premultiply_alpha(&mut image_data.data);

                self.texture_loader.create_color(
                    path,
                    RgbaImage::from_raw(image_data.width as u32, image_data.height as u32, image_data.data).unwrap(),
                    false,
                )
            })
            .collect();

        Arc::new(Sprite {
            palette_size,
            textures,
            #[cfg(feature = "debug")]
            sprite_data: cloned_sprite_data,
        })
    }
}
