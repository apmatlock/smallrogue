//! The dungeon map: a rectangular grid of tiles.

use crate::geom::Point;

/// What a single map square is made of.
///
/// `Copy` makes tiles as cheap to pass around as an integer, which they
/// effectively are: this enum is stored as a single byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Wall,
    Floor,
}

impl Tile {
    /// Can creatures walk onto this tile?
    pub fn is_walkable(self) -> bool {
        match self {
            Tile::Floor => true,
            Tile::Wall => false,
        }
    }
}

pub struct Map {
    pub width: i32,
    pub height: i32,
    /// Tiles stored row by row in one flat Vec. A single allocation is
    /// faster and more cache friendly than a Vec of Vecs.
    tiles: Vec<Tile>,
}

impl Map {
    /// Creates a map that is solid wall everywhere.
    pub fn new_filled(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            tiles: vec![Tile::Wall; (width * height) as usize],
        }
    }

    pub fn in_bounds(&self, p: Point) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    /// Converts a point to a position in the flat `tiles` Vec,
    /// or `None` if the point is off the map.
    fn index(&self, p: Point) -> Option<usize> {
        self.in_bounds(p)
            .then(|| (p.y * self.width + p.x) as usize)
    }

    /// Returns the tile at `p`. Anything off the map counts as wall,
    /// so callers never need to bounds-check first.
    pub fn tile(&self, p: Point) -> Tile {
        self.index(p).map_or(Tile::Wall, |i| self.tiles[i])
    }

    pub fn set_tile(&mut self, p: Point, tile: Tile) {
        if let Some(i) = self.index(p) {
            self.tiles[i] = tile;
        }
    }

    pub fn is_walkable(&self, p: Point) -> bool {
        self.tile(p).is_walkable()
    }

    // ---- Carving helpers --------------------------------------------
    // These build the hand-made test map now, and the random dungeon
    // generator will reuse them in milestone 2.

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
        assert!(map.is_walkable(Point::new(2, 2)));
        assert!(map.is_walkable(Point::new(4, 4)));
        assert!(!map.is_walkable(Point::new(5, 5)));
    }
}
