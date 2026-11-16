//! Gat types from `ZC_UPDATE_MAPINFO` (`0x0192`).
//!
//! Hercules `map_gat2cell` decides walkability. The GAT file loader uses a
//! different byte map and is left alone. Packet type 5 is an ice wall: snipable
//! and not walkable.

use ragnarok_formats::map::TileFlags;
use ragnarok_packets::TilePosition;

use crate::world::pathing::Traversable;

/// Hercules gat type to tile flags. `None` leaves the cell unchanged.
pub fn flags_for_gat(cell_type: u16) -> Option<TileFlags> {
    Some(match cell_type {
        0 | 2 | 4 | 6 => TileFlags::WALKABLE | TileFlags::SNIPABLE,
        1 => TileFlags::empty(),
        3 => TileFlags::WALKABLE | TileFlags::SNIPABLE | TileFlags::WATER,
        5 => TileFlags::SNIPABLE,
        _ => return None,
    })
}

/// Flat walk grid used by the pathing test and the logic-mode session.
///
/// Coordinates outside the grid, including `u16` wrap to 65535, are not
/// walkable. [`PathFinder`] builds neighbors with wrapping subtraction.
#[derive(Clone, Debug)]
pub struct WalkGrid {
    width: u16,
    height: u16,
    walkable: Vec<bool>,
    snipeable: Vec<bool>,
}

impl WalkGrid {
    /// One-cell-tall corridor. A blocked middle cell has no way around it.
    pub fn corridor(length: u16) -> Self {
        let cells = usize::from(length);
        Self {
            width: length,
            height: 1,
            walkable: vec![true; cells],
            snipeable: vec![true; cells],
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    /// Applies a gat type. Unknown types and out-of-range tiles return false
    /// and leave the cell as it was.
    pub fn apply_gat(&mut self, position: TilePosition, cell_type: u16) -> bool {
        let Some(flags) = flags_for_gat(cell_type) else {
            return false;
        };
        let Some(index) = self.index(position) else {
            return false;
        };
        self.walkable[index] = flags.contains(TileFlags::WALKABLE);
        self.snipeable[index] = flags.contains(TileFlags::SNIPABLE);
        true
    }

    fn index(&self, position: TilePosition) -> Option<usize> {
        if position.x >= self.width || position.y >= self.height {
            return None;
        }
        Some(usize::from(position.x) + usize::from(position.y) * usize::from(self.width))
    }
}

impl Traversable for WalkGrid {
    fn is_walkable(&self, position: TilePosition) -> bool {
        self.index(position).is_some_and(|index| self.walkable[index])
    }

    fn is_snipeable(&self, position: TilePosition) -> bool {
        self.index(position).is_some_and(|index| self.snipeable[index])
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_formats::map::TileFlags;
    use ragnarok_packets::TilePosition;

    use super::{WalkGrid, flags_for_gat};
    use crate::world::pathing::{PathFinder, Traversable};

    #[test]
    fn gat_types_match_hercules_walkability() {
        assert!(flags_for_gat(0).unwrap().contains(TileFlags::WALKABLE));
        assert!(flags_for_gat(2).unwrap().contains(TileFlags::WALKABLE));
        assert!(flags_for_gat(3).unwrap().contains(TileFlags::WALKABLE | TileFlags::WATER));
        assert!(flags_for_gat(4).unwrap().contains(TileFlags::WALKABLE));
        assert!(flags_for_gat(6).unwrap().contains(TileFlags::WALKABLE));
        assert!(!flags_for_gat(1).unwrap().contains(TileFlags::WALKABLE));
        let ice = flags_for_gat(5).unwrap();
        assert!(!ice.contains(TileFlags::WALKABLE));
        assert!(ice.contains(TileFlags::SNIPABLE));
        assert!(flags_for_gat(7).is_none());
        assert!(flags_for_gat(99).is_none());
    }

    #[test]
    fn ice_wall_blocks_the_only_path() {
        let mut grid = WalkGrid::corridor(8);
        let mut finder = PathFinder::default();
        let start = TilePosition { x: 0, y: 0 };
        let goal = TilePosition { x: 7, y: 0 };
        let blocked = TilePosition { x: 3, y: 0 };

        assert!(finder.find_walkable_path(&grid, start, goal).is_some());
        assert!(grid.apply_gat(blocked, 5));
        assert!(!grid.is_walkable(blocked));
        assert!(grid.is_snipeable(blocked));
        assert!(finder.find_walkable_path(&grid, start, goal).is_none());

        assert!(grid.apply_gat(blocked, 0));
        assert!(grid.is_walkable(blocked));
        assert!(finder.find_walkable_path(&grid, start, goal).is_some());

        assert!(grid.apply_gat(blocked, 6));
        assert!(grid.is_walkable(blocked));
        assert!(grid.apply_gat(blocked, 1));
        assert!(!grid.is_walkable(blocked));
        assert!(finder.find_walkable_path(&grid, start, goal).is_none());

        assert!(!grid.apply_gat(blocked, 99));
        assert!(!grid.is_walkable(blocked));
        assert!(!grid.apply_gat(TilePosition { x: 0, y: 1 }, 5));
        assert!(!grid.is_walkable(TilePosition { x: 0, y: 65535 }));
        assert!(!grid.is_walkable(TilePosition { x: 65535, y: 0 }));
    }
}
