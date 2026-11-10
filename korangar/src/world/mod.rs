mod action;
mod animation;
mod basic_info;
mod cameras;
mod effect;
pub mod effects;
mod entity;
pub mod entity_pick;
mod fade_state;
mod ground_item;
mod library;
mod light;
pub mod long_walk;
mod map;
pub(crate) mod map_load_failure;
mod model;
mod object;
mod particles;
mod pathing;
mod sound;
mod sprite_paths;
pub(crate) use self::sprite_paths::{
    JOB_NOVICE, JOB_SUMMONER, creation_part_files, hair_color_thumb_grf_path, hair_palette_grf_path, hair_palette_id, hair_thumb_grf_path,
};
mod video;

use std::sync::Arc;

pub use self::action::*;
pub use self::animation::*;
pub use self::basic_info::*;
pub use self::cameras::*;
pub use self::effect::*;
pub use self::entity::*;
pub use self::fade_state::*;
pub use self::ground_item::*;
pub(crate) use self::library::execute_lua_chunk;
pub use self::library::*;
pub use self::light::*;
pub use self::map::*;
pub use self::model::*;
pub use self::object::*;
pub use self::particles::*;
pub use self::pathing::*;
pub use self::sound::*;
pub use self::video::*;
use crate::graphics::Texture;

pub struct ResourceSetBuffer<K> {
    visible: Vec<K>,
}

impl<K> Default for ResourceSetBuffer<K> {
    fn default() -> Self {
        Self { visible: Vec::new() }
    }
}

impl<K> ResourceSetBuffer<K> {
    pub(super) fn create_set(&mut self, initializer: impl FnOnce(&mut Vec<K>)) -> ResourceSet<'_, K> {
        self.visible.clear();

        initializer(&mut self.visible);

        ResourceSet { visible: &self.visible }
    }
}

#[derive(Default)]
pub struct ResourceSet<'a, K> {
    visible: &'a [K],
}

impl<K> ResourceSet<'_, K> {
    pub(super) fn iterate_visible(&self) -> std::slice::Iter<'_, K> {
        self.visible.iter()
    }
}

#[derive(Debug, Clone)]
pub struct ResourceMetadata {
    pub texture: Option<Arc<Texture>>,
    pub name: String,
    pub description: String,
}
