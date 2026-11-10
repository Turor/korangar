//! Sprite-box picking for NPCs.
//!
//! The GPU picker only writes fully opaque sprite pixels, so a click on the
//! gaps of an NPC sprite (between arms, around a thin body, on a translucent
//! cape) lands on the tile behind it and walks the player. roBrowser picks an
//! entity by its sprite box instead. This module does the same as a fallback
//! when the GPU picker reports a tile.

use ragnarok_packets::EntityId;

use super::{Camera, Entity, EntityType};
use crate::graphics::{ScreenPosition, ScreenSize};

/// Extra pixels around the sprite box that still count as a hit.
pub const PICK_PADDING: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenBounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    /// Distance from the camera, smaller is in front.
    pub distance: f32,
}

impl ScreenBounds {
    pub fn from_points(points: &[(f32, f32)], distance: f32) -> Option<Self> {
        let first = points.first()?;
        let mut bounds = Self {
            left: first.0,
            top: first.1,
            right: first.0,
            bottom: first.1,
            distance,
        };
        for &(x, y) in &points[1..] {
            bounds.left = bounds.left.min(x);
            bounds.right = bounds.right.max(x);
            bounds.top = bounds.top.min(y);
            bounds.bottom = bounds.bottom.max(y);
        }
        let finite = [bounds.left, bounds.top, bounds.right, bounds.bottom]
            .iter()
            .all(|value| value.is_finite());
        (finite && bounds.right > bounds.left && bounds.bottom > bounds.top).then_some(bounds)
    }

    pub fn contains(&self, x: f32, y: f32, padding: f32) -> bool {
        x >= self.left - padding && x <= self.right + padding && y >= self.top - padding && y <= self.bottom + padding
    }
}

/// The front-most candidate whose box contains the point.
pub fn pick_front_most(candidates: impl IntoIterator<Item = (EntityId, ScreenBounds)>, x: f32, y: f32) -> Option<EntityId> {
    candidates
        .into_iter()
        .filter(|(_, bounds)| bounds.contains(x, y, PICK_PADDING))
        .min_by(|(_, a), (_, b)| a.distance.total_cmp(&b.distance))
        .map(|(entity_id, _)| entity_id)
}

/// NPC whose sprite box is under the mouse, if any.
pub fn npc_under_cursor(camera: &dyn Camera, entities: &[Entity], screen_size: ScreenSize, mouse: ScreenPosition) -> Option<EntityId> {
    pick_front_most(
        entities
            .iter()
            .filter(|entity| entity.get_entity_type() == EntityType::Npc)
            .filter_map(|entity| Some((entity.get_entity_id(), entity.screen_bounds(camera, screen_size)?))),
        mouse.left,
        mouse.top,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(left: f32, top: f32, right: f32, bottom: f32, distance: f32) -> ScreenBounds {
        ScreenBounds {
            left,
            top,
            right,
            bottom,
            distance,
        }
    }

    #[test]
    fn bounds_from_projected_corners() {
        let points = [(110.0, 300.0), (110.0, 200.0), (150.0, 300.0), (150.0, 200.0)];
        assert_eq!(
            ScreenBounds::from_points(&points, 5.0),
            Some(bounds(110.0, 200.0, 150.0, 300.0, 5.0))
        );
        assert_eq!(ScreenBounds::from_points(&[(1.0, 1.0), (1.0, 1.0)], 1.0), None);
        assert_eq!(ScreenBounds::from_points(&[(f32::NAN, 1.0), (3.0, 4.0)], 1.0), None);
        assert_eq!(ScreenBounds::from_points(&[], 1.0), None);
    }

    #[test]
    fn click_in_the_sprite_box_but_off_the_opaque_pixels_hits() {
        let npc = bounds(100.0, 100.0, 140.0, 190.0, 50.0);
        assert_eq!(pick_front_most([(EntityId(7), npc)], 101.0, 102.0), Some(EntityId(7)));
        assert_eq!(pick_front_most([(EntityId(7), npc)], 97.0, 192.0), Some(EntityId(7)), "padding");
        assert_eq!(pick_front_most([(EntityId(7), npc)], 90.0, 150.0), None);
        assert_eq!(pick_front_most([(EntityId(7), npc)], 120.0, 200.0), None);
    }

    #[test]
    fn overlapping_npcs_pick_the_one_in_front() {
        let back = bounds(100.0, 100.0, 140.0, 190.0, 80.0);
        let front = bounds(120.0, 110.0, 160.0, 200.0, 40.0);
        assert_eq!(
            pick_front_most([(EntityId(1), back), (EntityId(2), front)], 130.0, 150.0),
            Some(EntityId(2))
        );
        assert_eq!(
            pick_front_most([(EntityId(1), back), (EntityId(2), front)], 105.0, 150.0),
            Some(EntityId(1))
        );
    }
}
