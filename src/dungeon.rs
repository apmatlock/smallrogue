//! Random floor generation: rooms, corridors, doors and stairs.
//!
//! The steps, in order:
//! 1. Scatter non-overlapping rectangular rooms.
//! 2. Connect them into a tree, always joining the closest unconnected
//!    room. This guarantees every room is reachable without long
//!    corridors slicing across the whole map.
//! 3. Add a few extra corridors between neighbors, creating loops so
//!    the player can circle around monsters instead of every floor
//!    being a dead-end maze.
//! 4. Put doors where corridors enter rooms.
//! 5. Put the player in one room and the stairs in the farthest room.

use crate::geom::{Point, Rect};
use crate::map::{Map, Tile};
use crate::rng::Rng;

/// Settings that shape a floor. Zones (milestone 10) will each supply
/// their own, which is how floor size will vary by zone.
pub struct FloorParams {
    pub width: i32,
    pub height: i32,
    pub max_rooms: usize,
    pub room_w: (i32, i32), // min, max
    pub room_h: (i32, i32),
    /// Extra corridors per room, in percent, used to create loops.
    pub loop_percent: i32,
    /// Chance that a room entrance gets a door.
    pub door_percent: i32,
}

pub const STANDARD: FloorParams = FloorParams {
    width: 80,
    height: 36,
    max_rooms: 12,
    room_w: (5, 14),
    room_h: (4, 8),
    loop_percent: 25,
    door_percent: 70,
};

pub struct Level {
    pub map: Map,
    pub start: Point,
}

/// Builds a random floor. Every call with an `Rng` in the same state
/// produces the same floor.
pub fn generate(rng: &mut Rng, params: &FloorParams) -> Level {
    // Very rarely the dice place fewer than two rooms. Just try again;
    // the loop always ends because the rng keeps changing.
    loop {
        if let Some(level) = try_generate(rng, params) {
            return level;
        }
    }
}

fn try_generate(rng: &mut Rng, params: &FloorParams) -> Option<Level> {
    let mut map = Map::new_filled(params.width, params.height);

    let rooms = place_rooms(rng, params);
    if rooms.len() < 2 {
        return None;
    }
    for room in &rooms {
        map.carve_room(room.x, room.y, room.w, room.h);
    }

    connect_rooms(rng, &mut map, &rooms, params.loop_percent);

    for room in &rooms {
        add_doors(rng, &mut map, room, params.door_percent);
    }

    // Start in a random room; stairs go in whichever room is farthest.
    let start_room = rooms[rng.index(rooms.len())];
    let start = start_room.center();
    let stairs_room = rooms
        .iter()
        .max_by_key(|r| r.center().dist_sq(start))
        .copied()?;
    map.set_tile(random_point_in(rng, stairs_room), Tile::StairsDown);

    Some(Level { map, start })
}

fn place_rooms(rng: &mut Rng, p: &FloorParams) -> Vec<Rect> {
    let mut rooms: Vec<Rect> = Vec::new();
    // Many attempts will collide with existing rooms, so allow several
    // tries per room we want.
    for _ in 0..p.max_rooms * 5 {
        if rooms.len() == p.max_rooms {
            break;
        }
        let w = rng.range(p.room_w.0, p.room_w.1 + 1);
        let h = rng.range(p.room_h.0, p.room_h.1 + 1);
        // Keep a one-tile solid border around the whole map.
        let x = rng.range(1, p.width - w - 1);
        let y = rng.range(1, p.height - h - 1);
        let room = Rect::new(x, y, w, h);
        // A margin of 2 leaves at least one wall tile between rooms,
        // plus room for a corridor.
        if rooms.iter().all(|other| !room.overlaps(*other, 2)) {
            rooms.push(room);
        }
    }
    rooms
}

fn connect_rooms(rng: &mut Rng, map: &mut Map, rooms: &[Rect], loop_percent: i32) {
    // Grow a tree: repeatedly link the closest pair where one room is
    // already connected and the other isn't. (This is Prim's algorithm
    // for a minimum spanning tree.)
    let mut connected = vec![false; rooms.len()];
    connected[0] = true;
    let mut links: Vec<(usize, usize)> = Vec::new();

    for _ in 1..rooms.len() {
        let mut best: Option<(i32, usize, usize)> = None;
        for a in (0..rooms.len()).filter(|&i| connected[i]) {
            for b in (0..rooms.len()).filter(|&i| !connected[i]) {
                let d = rooms[a].center().dist_sq(rooms[b].center());
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, a, b));
                }
            }
        }
        let (_, a, b) = best.expect("an unconnected room always remains");
        connected[b] = true;
        links.push((a, b));
        carve_corridor(rng, map, rooms[a].center(), rooms[b].center());
    }

    // Extra links for loops: join a random room to its nearest
    // neighbor that it isn't already linked to.
    let extra = (rooms.len() as i32 * loop_percent / 100).max(1);
    for _ in 0..extra {
        let a = rng.index(rooms.len());
        let nearest_unlinked = (0..rooms.len())
            .filter(|&b| b != a && !links.contains(&(a, b)) && !links.contains(&(b, a)))
            .min_by_key(|&b| rooms[a].center().dist_sq(rooms[b].center()));
        if let Some(b) = nearest_unlinked {
            links.push((a, b));
            carve_corridor(rng, map, rooms[a].center(), rooms[b].center());
        }
    }
}

/// An L-shaped corridor. Randomly going horizontal-first or
/// vertical-first makes the layouts less uniform.
fn carve_corridor(rng: &mut Rng, map: &mut Map, from: Point, to: Point) {
    if rng.chance(50) {
        map.carve_h_corridor(from.x, to.x, from.y);
        map.carve_v_corridor(from.y, to.y, to.x);
    } else {
        map.carve_v_corridor(from.y, to.y, from.x);
        map.carve_h_corridor(from.x, to.x, to.y);
    }
}

/// Looks at the ring of tiles just outside a room. Wherever a corridor
/// passes straight through that ring, with wall on both sides, the
/// tile is an entrance and may become a door.
fn add_doors(rng: &mut Rng, map: &mut Map, room: &Rect, door_percent: i32) {
    let (left, right) = (room.x - 1, room.x + room.w);
    let (top, bottom) = (room.y - 1, room.y + room.h);

    let mut ring = Vec::new();
    for x in room.x..room.x + room.w {
        ring.push((Point::new(x, top), Point::new(0, 1)));
        ring.push((Point::new(x, bottom), Point::new(0, -1)));
    }
    for y in room.y..room.y + room.h {
        ring.push((Point::new(left, y), Point::new(1, 0)));
        ring.push((Point::new(right, y), Point::new(-1, 0)));
    }

    // `inward` points into the room. The wall runs across it.
    for (p, inward) in ring {
        let across = Point::new(inward.y, inward.x);
        let is_entrance = map.tile(p) == Tile::Floor
            && map.tile(p + inward).is_passable()
            && map.tile(p - inward).is_passable()
            && map.tile(p + across) == Tile::Wall
            && map.tile(p - across) == Tile::Wall;
        let beside_door = crate::geom::DIRECTIONS_8
            .iter()
            .any(|&d| matches!(map.tile(p + d), Tile::DoorClosed | Tile::DoorOpen));
        if is_entrance && !beside_door && rng.chance(door_percent) {
            map.set_tile(p, Tile::DoorClosed);
        }
    }
}

fn random_point_in(rng: &mut Rng, room: Rect) -> Point {
    Point::new(
        rng.range(room.x, room.x + room.w),
        rng.range(room.y, room.y + room.h),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::DIRECTIONS_8;
    use crate::grid::Grid;

    fn find_stairs(map: &Map) -> Point {
        map.points()
            .find(|&p| map.tile(p) == Tile::StairsDown)
            .expect("every floor has stairs")
    }

    /// Flood fill: marks every tile reachable from `start` through
    /// passable tiles (doors count as passable).
    fn reachable(map: &Map, start: Point) -> Grid<bool> {
        let mut seen = Grid::new(map.width(), map.height(), false);
        seen.set(start, true);
        let mut frontier = vec![start];
        while let Some(p) = frontier.pop() {
            for d in DIRECTIONS_8 {
                let n = p + d;
                if map.tile(n).is_passable() && seen.get(n) == Some(&false) {
                    seen.set(n, true);
                    frontier.push(n);
                }
            }
        }
        seen
    }

    #[test]
    fn every_floor_is_fully_connected() {
        for seed in 0..100 {
            let level = generate(&mut Rng::new(seed), &STANDARD);
            let map = &level.map;
            let reach = reachable(map, level.start);
            for p in map.points().filter(|&p| map.tile(p).is_passable()) {
                assert_eq!(
                    reach.get(p),
                    Some(&true),
                    "seed {seed}: {p:?} cannot be reached"
                );
            }
        }
    }

    #[test]
    fn border_is_solid_and_stairs_are_away_from_start() {
        for seed in 0..100 {
            let level = generate(&mut Rng::new(seed), &STANDARD);
            let m = &level.map;
            for x in 0..m.width() {
                assert_eq!(m.tile(Point::new(x, 0)), Tile::Wall);
                assert_eq!(m.tile(Point::new(x, m.height() - 1)), Tile::Wall);
            }
            for y in 0..m.height() {
                assert_eq!(m.tile(Point::new(0, y)), Tile::Wall);
                assert_eq!(m.tile(Point::new(m.width() - 1, y)), Tile::Wall);
            }
            assert_ne!(level.start, find_stairs(m), "seed {seed}");
        }
    }

    #[test]
    fn same_seed_same_floor() {
        let a = generate(&mut Rng::new(99), &STANDARD);
        let b = generate(&mut Rng::new(99), &STANDARD);
        assert_eq!(a.start, b.start);
        assert_eq!(find_stairs(&a.map), find_stairs(&b.map));
    }

    #[test]
    fn floors_have_doors() {
        let doors: usize = (0..20)
            .map(|seed| {
                let level = generate(&mut Rng::new(seed), &STANDARD);
                level
                    .map
                    .points()
                    .filter(|&p| level.map.tile(p) == Tile::DoorClosed)
                    .count()
            })
            .sum();
        assert!(doors > 20, "only {doors} doors across 20 floors");
    }
}
