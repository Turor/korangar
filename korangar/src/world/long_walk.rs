//! Long click walks.
//!
//! Hercules (`unit_walktoxy_sub`) silently drops a `CZ_REQUEST_MOVE2` whose
//! path is longer than `max_walk_path` (17), or longer than 14 cells when an
//! obstacle sits on the straight line (`OFFICIAL_WALKPATH`). The client plans
//! the whole path on the GAT and asks the server for one segment of at most
//! [`SERVER_WALK_SEGMENT`] cells at a time. When a segment finishes, the next
//! one is requested from the cell the player stopped on. A new click replaces
//! the destination.

use ragnarok_packets::{AttackRange, TilePosition};

use super::pathing::{PathFinder, Traversable};

/// Longest segment that Hercules accepts with and without an obstacle.
pub const SERVER_WALK_SEGMENT: usize = 14;
/// A click on an NPC walks to within this many cells before the dialog opens.
/// Hercules (`npc_checknear`) accepts `CZ_CONTACTNPC` up to `AREA_SIZE + 1`
/// cells away, so the dialog request at this range is always in reach.
pub const NPC_TALK_RANGE: AttackRange = AttackRange(3);
/// A segment the server does not answer with `ZC_NOTIFY_PLAYERMOVE` within this
/// time was rejected, and the long walk is dropped.
pub const SEGMENT_ACCEPT_TIMEOUT_MS: u32 = 1500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveWalk {
    destination: TilePosition,
    range: AttackRange,
    requested_from: TilePosition,
    requested_at: u32,
    accepted: bool,
}

/// What to do after the local player stopped moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkStep {
    /// No long walk is running.
    Idle,
    /// Request this cell next.
    Continue(TilePosition),
    /// The long walk reached its destination (or its range).
    Arrived,
    /// The long walk made no progress and was dropped.
    Aborted,
}

#[derive(Debug, Default)]
pub struct LongWalk {
    active: Option<ActiveWalk>,
}

fn chebyshev(a: TilePosition, b: TilePosition) -> u16 {
    a.x.abs_diff(b.x).max(a.y.abs_diff(b.y))
}

/// Cell to request for a planned `path` (index 0 is the start cell).
pub fn segment_target(path: &[TilePosition]) -> Option<TilePosition> {
    let last = path.len().checked_sub(1)?;
    path.get(last.min(SERVER_WALK_SEGMENT)).copied()
}

impl LongWalk {
    #[cfg(test)]
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    #[cfg(test)]
    pub fn destination(&self) -> Option<TilePosition> {
        self.active.map(|walk| walk.destination)
    }

    pub fn cancel(&mut self) {
        self.active = None;
    }

    /// Plans a walk from `start` to within `range` of `destination` and returns
    /// the cell to send in `CZ_REQUEST_MOVE2`, or `None` when the player is
    /// already in range. Replaces any running long walk.
    pub fn begin(
        &mut self,
        path_finder: &mut PathFinder,
        map: &impl Traversable,
        start: TilePosition,
        destination: TilePosition,
        range: AttackRange,
        now: u32,
    ) -> Option<TilePosition> {
        self.active = None;

        if chebyshev(start, destination) <= range.0 {
            return None;
        }

        let Some(path) = path_finder.find_long_walkable_path(map, start, destination, range) else {
            // No client path (unwalkable or too far): let the server decide.
            return (range.0 == 0).then_some(destination);
        };
        let target = segment_target(path)?;
        let goal = *path.last()?;

        if target != goal {
            self.active = Some(ActiveWalk {
                destination,
                range,
                requested_from: start,
                requested_at: now,
                accepted: false,
            });
        }

        Some(target)
    }

    /// The server accepted a walk for the local player.
    pub fn on_server_move(&mut self) {
        if let Some(walk) = self.active.as_mut() {
            walk.accepted = true;
        }
    }

    /// Drops a long walk whose last segment the server never accepted.
    pub fn tick(&mut self, now: u32) -> bool {
        if let Some(walk) = self.active
            && !walk.accepted
            && now.saturating_sub(walk.requested_at) > SEGMENT_ACCEPT_TIMEOUT_MS
        {
            self.active = None;
            return true;
        }
        false
    }

    /// Called on the frame the local player stops on `current`.
    pub fn on_stopped(&mut self, path_finder: &mut PathFinder, map: &impl Traversable, current: TilePosition, now: u32) -> WalkStep {
        let Some(walk) = self.active else {
            return WalkStep::Idle;
        };

        if chebyshev(current, walk.destination) <= walk.range.0 {
            self.active = None;
            return WalkStep::Arrived;
        }

        if current == walk.requested_from {
            self.active = None;
            return WalkStep::Aborted;
        }

        match self.begin(path_finder, map, current, walk.destination, walk.range, now) {
            Some(next) if next != current => WalkStep::Continue(next),
            Some(_) => {
                self.active = None;
                WalkStep::Aborted
            }
            None => WalkStep::Arrived,
        }
    }
}

#[cfg(test)]
mod tests {
    use hashbrown::HashSet;

    use super::*;

    struct Grid {
        width: u16,
        height: u16,
        blocked: HashSet<TilePosition>,
    }

    impl Traversable for Grid {
        fn is_walkable(&self, position: TilePosition) -> bool {
            position.x < self.width && position.y < self.height && !self.blocked.contains(&position)
        }

        fn is_snipeable(&self, position: TilePosition) -> bool {
            self.is_walkable(position)
        }
    }

    fn open(width: u16, height: u16) -> Grid {
        Grid {
            width,
            height,
            blocked: HashSet::new(),
        }
    }

    fn tile(x: u16, y: u16) -> TilePosition {
        TilePosition { x, y }
    }

    #[test]
    fn short_walk_is_one_request() {
        let map = open(100, 100);
        let mut finder = PathFinder::default();
        let mut walk = LongWalk::default();

        let target = walk.begin(&mut finder, &map, tile(95, 86), tile(91, 86), AttackRange(0), 0);
        assert_eq!(target, Some(tile(91, 86)));
        assert!(!walk.is_active());
    }

    #[test]
    fn walk_longer_than_server_limit_is_split() {
        // VB2: prontera 95,86 -> 75,90 is 20 cells, more than Hercules accepts.
        let map = open(200, 200);
        let mut finder = PathFinder::default();
        let mut walk = LongWalk::default();

        let first = walk
            .begin(&mut finder, &map, tile(95, 86), tile(75, 90), AttackRange(0), 0)
            .unwrap();
        assert_eq!(chebyshev(tile(95, 86), first), SERVER_WALK_SEGMENT as u16);
        assert!(walk.is_active());
        walk.on_server_move();

        let WalkStep::Continue(second) = walk.on_stopped(&mut finder, &map, first, 100) else {
            panic!("expected a second segment");
        };
        assert_eq!(second, tile(75, 90));
        assert!(!walk.is_active(), "last segment fits in one request");
    }

    #[test]
    fn very_long_walk_beyond_client_path_limit_is_split() {
        let map = open(300, 50);
        let mut finder = PathFinder::default();
        let mut walk = LongWalk::default();

        let mut current = tile(10, 10);
        let destination = tile(250, 10);
        let mut next = walk.begin(&mut finder, &map, current, destination, AttackRange(0), 0).unwrap();
        let mut requests = 1;
        loop {
            assert!(chebyshev(current, next) as usize <= SERVER_WALK_SEGMENT);
            current = next;
            match walk.on_stopped(&mut finder, &map, current, 0) {
                WalkStep::Continue(cell) => {
                    next = cell;
                    requests += 1;
                }
                WalkStep::Idle => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(current, destination);
        assert_eq!(requests, 18);
    }

    #[test]
    fn segments_follow_the_path_around_walls() {
        let mut map = open(40, 40);
        for y in 0..30 {
            map.blocked.insert(tile(20, y));
        }
        let mut finder = PathFinder::default();
        let mut walk = LongWalk::default();

        let first = walk.begin(&mut finder, &map, tile(15, 5), tile(25, 5), AttackRange(0), 0).unwrap();
        assert!(map.is_walkable(first));
        assert!(walk.is_active(), "detour around the wall is longer than one segment");
    }

    #[test]
    fn new_click_replaces_the_destination() {
        let map = open(200, 200);
        let mut finder = PathFinder::default();
        let mut walk = LongWalk::default();

        walk.begin(&mut finder, &map, tile(100, 100), tile(150, 100), AttackRange(0), 0);
        assert_eq!(walk.destination(), Some(tile(150, 100)));
        let target = walk.begin(&mut finder, &map, tile(102, 100), tile(100, 130), AttackRange(0), 10);
        assert_eq!(walk.destination(), Some(tile(100, 130)));
        assert_eq!(chebyshev(tile(102, 100), target.unwrap()), SERVER_WALK_SEGMENT as u16);

        let target = walk.begin(&mut finder, &map, tile(102, 103), tile(104, 104), AttackRange(0), 20);
        assert_eq!(target, Some(tile(104, 104)));
        assert!(!walk.is_active());
    }

    #[test]
    fn rejected_segment_times_out() {
        let map = open(200, 200);
        let mut finder = PathFinder::default();
        let mut walk = LongWalk::default();

        walk.begin(&mut finder, &map, tile(10, 10), tile(60, 10), AttackRange(0), 1000);
        assert!(!walk.tick(1000 + SEGMENT_ACCEPT_TIMEOUT_MS));
        assert!(walk.tick(1001 + SEGMENT_ACCEPT_TIMEOUT_MS));
        assert!(!walk.is_active());

        walk.begin(&mut finder, &map, tile(10, 10), tile(60, 10), AttackRange(0), 1000);
        walk.on_server_move();
        assert!(!walk.tick(10_000));
        assert!(walk.is_active());
    }

    #[test]
    fn no_progress_aborts() {
        let map = open(200, 200);
        let mut finder = PathFinder::default();
        let mut walk = LongWalk::default();

        walk.begin(&mut finder, &map, tile(10, 10), tile(60, 10), AttackRange(0), 0);
        assert_eq!(walk.on_stopped(&mut finder, &map, tile(10, 10), 0), WalkStep::Aborted);
        assert_eq!(walk.on_stopped(&mut finder, &map, tile(10, 10), 0), WalkStep::Idle);
    }

    #[test]
    fn approach_in_range_stops_short_of_the_target() {
        let map = open(200, 200);
        let mut finder = PathFinder::default();
        let mut walk = LongWalk::default();

        // Already in range: nothing to request.
        assert_eq!(
            walk.begin(&mut finder, &map, tile(10, 10), tile(12, 12), AttackRange(3), 0),
            None
        );

        let first = walk
            .begin(&mut finder, &map, tile(10, 10), tile(40, 10), AttackRange(3), 0)
            .unwrap();
        assert_eq!(first, tile(24, 10));
        assert_eq!(
            walk.on_stopped(&mut finder, &map, first, 0),
            WalkStep::Continue(tile(37, 10)),
            "second segment ends three cells from the NPC"
        );
        assert_eq!(walk.on_stopped(&mut finder, &map, tile(37, 10), 0), WalkStep::Idle);
    }

    #[test]
    fn unreachable_destination_is_sent_as_is() {
        let mut map = open(50, 50);
        map.blocked.insert(tile(30, 30));
        let mut finder = PathFinder::default();
        let mut walk = LongWalk::default();

        assert_eq!(
            walk.begin(&mut finder, &map, tile(10, 10), tile(30, 30), AttackRange(0), 0),
            Some(tile(30, 30))
        );
        assert!(!walk.is_active());
    }
}
