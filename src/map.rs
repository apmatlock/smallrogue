//! The dungeon map: the tile layout of one floor, plus what the player
//! remembers of it.

use crate::geom::Point;
use crate::grid::Grid;

/// What a single map square is made of.
///
/// `Copy` makes tiles as cheap to pass around as an integer, which they
/// effectively are: this enum is stored as a single byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Wall,
    Floor,
    DoorClosed,
    DoorOpen,
    StairsDown,
    /// Shallow water: walked through like floor, only looks different.
    Water,
}

impl Tile {
    /// Can creatures step onto this tile right now? A closed door is
    /// not walkable: it has to be opened first.
    pub fn is_walkable(self) -> bool {
        // `matches!` is shorthand for a `match` that returns a bool.
        matches!(
            self,
            Tile::Floor | Tile::DoorOpen | Tile::StairsDown | Tile::Water
        )
    }

    /// Is this tile part of the dungeon's layout that creatures can
    /// ever pass through, counting doors whether open or closed? Used
    /// to check that every floor is fully connected.
    pub fn is_passable(self) -> bool {
        self.is_walkable() || self == Tile::DoorClosed
    }

    /// Does this tile stop line of sight?
    pub fn blocks_sight(self) -> bool {
        matches!(self, Tile::Wall | Tile::DoorClosed)
    }
}

pub struct Map {
    tiles: Grid<Tile>,
    /// Tiles the player has seen at some point on this floor. They
    /// stay drawn, dimmed, after going out of view.
    revealed: Grid<bool>,
}

impl Map {
    /// Creates a map that is solid wall everywhere.
    pub fn new_filled(width: i32, height: i32) -> Self {
        Self {
            tiles: Grid::new(width, height, Tile::Wall),
            revealed: Grid::new(width, height, false),
        }
    }

    pub fn in_bounds(&self, p: Point) -> bool {
        self.tiles.in_bounds(p)
    }

    pub fn width(&self) -> i32 {
        self.tiles.width()
    }

    pub fn height(&self) -> i32 {
        self.tiles.height()
    }

    /// Returns the tile at `p`. Anything off the map counts as wall,
    /// so callers never need to bounds-check first.
    pub fn tile(&self, p: Point) -> Tile {
        self.tiles.get(p).copied().unwrap_or(Tile::Wall)
    }

    pub fn set_tile(&mut self, p: Point, tile: Tile) {
        self.tiles.set(p, tile);
    }

    pub fn is_revealed(&self, p: Point) -> bool {
        self.revealed.get(p).copied().unwrap_or(false)
    }

    pub fn reveal(&mut self, p: Point) {
        self.revealed.set(p, true);
    }

    /// Every point on the map, row by row.
    pub fn points(&self) -> impl Iterator<Item = Point> + use<> {
        self.tiles.points()
    }

    // ---- Carving helpers --------------------------------------------
    // Used by the dungeon generator and by tests that build small maps
    // by hand.

    /// Turns a rectangle of tiles into floor. `x, y` is the top-left
    /// corner of the floor area.
    pub fn carve_room(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set_tile(Point::new(xx, yy), Tile::Floor);
            }
        }
    }

    /// Carves a horizontal corridor between x1 and x2 (either order).
    pub fn carve_h_corridor(&mut self, x1: i32, x2: i32, y: i32) {
        for x in x1.min(x2)..=x1.max(x2) {
            self.set_tile(Point::new(x, y), Tile::Floor);
        }
    }

    /// Carves a vertical corridor between y1 and y2 (either order).
    pub fn carve_v_corridor(&mut self, y1: i32, y2: i32, x: i32) {
        for y in y1.min(y2)..=y1.max(y2) {
            self.set_tile(Point::new(x, y), Tile::Floor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_map_is_wall() {
        let map = Map::new_filled(5, 5);
        assert_eq!(map.tile(Point::new(-1, 0)), Tile::Wall);
        assert_eq!(map.tile(Point::new(5, 5)), Tile::Wall);
    }

    #[test]
    fn carving_makes_floor() {
        let mut map = Map::new_filled(10, 10);
        map.carve_room(2, 2, 3, 3);
        assert!(map.tile(Point::new(2, 2)).is_walkable());
        assert!(map.tile(Point::new(4, 4)).is_walkable());
        assert!(!map.tile(Point::new(5, 5)).is_walkable());
    }
}
