//! Field of view using symmetric shadowcasting.
//!
//! Based on Albert Ford's algorithm:
//! <https://www.albertford.com/shadowcasting/>
//!
//! The idea: split the view into four quadrants (north, east, south,
//! west). Scan each quadrant row by row moving away from the viewer.
//! Each row is visible between a start slope and an end slope. When a
//! wall is found, the rows behind it are split into narrower slopes so
//! the wall casts a shadow.
//!
//! "Symmetric" means that if A can see B, then B can see A. That keeps
//! monster sight fair: nothing can see you that you can't see.
//!
//! Slopes are stored as exact fractions, not floats, so there are no
//! rounding surprises at the edges of walls.

use crate::geom::Point;

/// Computes every tile visible from `origin` within `radius`.
///
/// This function knows nothing about maps. It asks `blocks(p)` whether
/// a tile stops sight and calls `mark(p)` for each visible tile. That
/// makes it reusable for monsters later and easy to test.
pub fn compute(
    origin: Point,
    radius: i32,
    blocks: impl Fn(Point) -> bool,
    mut mark: impl FnMut(Point),
) {
    mark(origin);
    let mut fov = Fov {
        origin,
        radius,
        blocks: &blocks,
        mark: &mut mark,
    };
    for quadrant in [
        Quadrant::North,
        Quadrant::East,
        Quadrant::South,
        Quadrant::West,
    ] {
        fov.scan(
            quadrant,
            Row {
                depth: 1,
                start: Slope::new(-1, 1),
                end: Slope::new(1, 1),
            },
        );
    }
}

/// Bundles the fixed inputs so the recursive `scan` has fewer
/// parameters. `dyn` means "any closure of this shape", chosen at run
/// time, which lets this struct avoid generic parameters.
struct Fov<'a> {
    origin: Point,
    radius: i32,
    blocks: &'a dyn Fn(Point) -> bool,
    mark: &'a mut dyn FnMut(Point),
}

#[derive(Clone, Copy)]
enum Quadrant {
    North,
    East,
    South,
    West,
}

/// A slope as the exact fraction `num / den`. `den` is always positive.
#[derive(Clone, Copy)]
struct Slope {
    num: i32,
    den: i32,
}

impl Slope {
    const fn new(num: i32, den: i32) -> Self {
        Self { num, den }
    }
}

/// One row of a quadrant: `depth` tiles away from the viewer, visible
/// between the `start` and `end` slopes.
#[derive(Clone, Copy)]
struct Row {
    depth: i32,
    start: Slope,
    end: Slope,
}

impl Row {
    /// The range of columns this row covers.
    fn columns(&self) -> std::ops::RangeInclusive<i32> {
        let min = round_ties_up(self.depth, self.start);
        let max = round_ties_down(self.depth, self.end);
        min..=max
    }

    fn next(&self) -> Row {
        Row {
            depth: self.depth + 1,
            ..*self
        }
    }

    /// True if the tile's center lies within this row's slopes. Only
    /// such tiles count as visible, which is what makes the result
    /// symmetric. (Walls are shown even if only partly in view.)
    fn is_symmetric(&self, col: i32) -> bool {
        col * self.start.den >= self.depth * self.start.num
            && col * self.end.den <= self.depth * self.end.num
    }
}

/// The slope through the left edge of the tile at (depth, col).
fn slope(depth: i32, col: i32) -> Slope {
    Slope::new(2 * col - 1, 2 * depth)
}

/// `depth * slope`, rounded to the nearest integer, halves rounding up.
fn round_ties_up(depth: i32, s: Slope) -> i32 {
    // floor(d*n/den + 1/2) == floor((2*d*n + den) / (2*den))
    (2 * depth * s.num + s.den).div_euclid(2 * s.den)
}

/// `depth * slope`, rounded to the nearest integer, halves rounding
/// down.
fn round_ties_down(depth: i32, s: Slope) -> i32 {
    // ceil(x - 1/2) == -floor(1/2 - x)
    -(s.den - 2 * depth * s.num).div_euclid(2 * s.den)
}

impl Fov<'_> {
    /// Converts (depth, column) within a quadrant to a map position.
    fn to_map(&self, q: Quadrant, depth: i32, col: i32) -> Point {
        let o = self.origin;
        match q {
            Quadrant::North => Point::new(o.x + col, o.y - depth),
            Quadrant::South => Point::new(o.x + col, o.y + depth),
            Quadrant::East => Point::new(o.x + depth, o.y + col),
            Quadrant::West => Point::new(o.x - depth, o.y + col),
        }
    }

    fn in_radius(&self, p: Point) -> bool {
        // `+ radius` rounds the circle out a little so it looks less
        // pointy at the four compass directions.
        p.dist_sq(self.origin) <= self.radius * self.radius + self.radius
    }

    fn scan(&mut self, q: Quadrant, mut row: Row) {
        if row.depth > self.radius {
            return;
        }
        // Whether the previous tile in this row was a wall. `None` at
        // the start of the row.
        let mut prev_wall: Option<bool> = None;

        for col in row.columns() {
            let p = self.to_map(q, row.depth, col);
            let is_wall = (self.blocks)(p);

            if (is_wall || row.is_symmetric(col)) && self.in_radius(p) {
                (self.mark)(p);
            }
            // Wall then floor: the visible area starts again here.
            if prev_wall == Some(true) && !is_wall {
                row.start = slope(row.depth, col);
            }
            // Floor then wall: everything seen so far in this row
            // continues into the next row, up to this wall's edge.
            if prev_wall == Some(false) && is_wall {
                let mut next = row.next();
                next.end = slope(row.depth, col);
                self.scan(q, next);
            }
            prev_wall = Some(is_wall);
        }
        // If the row ended on floor, keep scanning outward.
        if prev_wall == Some(false) {
            self.scan(q, row.next());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    /// Parses a picture into walls; returns the grid and the '@'.
    fn parse(rows: &[&str]) -> (Grid<bool>, Point) {
        let mut walls = Grid::new(rows[0].len() as i32, rows.len() as i32, false);
        let mut origin = Point::default();
        for (y, line) in rows.iter().enumerate() {
            for (x, ch) in line.chars().enumerate() {
                let p = Point::new(x as i32, y as i32);
                walls.set(p, ch == '#');
                if ch == '@' {
                    origin = p;
                }
            }
        }
        (walls, origin)
    }

    fn visible_from(walls: &Grid<bool>, origin: Point, radius: i32) -> Grid<bool> {
        let mut seen = Grid::new(walls.width(), walls.height(), false);
        compute(
            origin,
            radius,
            |p| walls.get(p).copied().unwrap_or(true),
            |p| seen.set(p, true),
        );
        seen
    }

    #[test]
    fn open_room_is_fully_visible() {
        let (walls, o) = parse(&[
            "#######", //
            "#.....#", "#..@..#", "#.....#", "#######",
        ]);
        let seen = visible_from(&walls, o, 10);
        assert!(walls.points().all(|p| seen.get(p) == Some(&true)));
    }

    #[test]
    fn walls_cast_shadows() {
        let (walls, o) = parse(&[
            "###########", //
            "#@..#.....#",
            "#...#.....#",
            "###########",
        ]);
        let seen = visible_from(&walls, o, 20);
        assert_eq!(seen.get(Point::new(4, 1)), Some(&true)); // the wall
        assert_eq!(seen.get(Point::new(7, 1)), Some(&false)); // behind it
    }

    #[test]
    fn radius_limits_sight() {
        let (walls, o) = parse(&["@.........."]);
        let seen = visible_from(&walls, o, 4);
        assert_eq!(seen.get(Point::new(4, 0)), Some(&true));
        assert_eq!(seen.get(Point::new(5, 0)), Some(&false));
    }

    /// If A sees B, B must see A, for every pair of floor tiles.
    #[test]
    fn sight_is_symmetric() {
        let (walls, _) = parse(&[
            "############",
            "#....#.....#",
            "#.#.....#..#",
            "#...##.....#",
            "#.#....#.#.#",
            "#....#.....#",
            "############",
        ]);
        let floors: Vec<Point> = walls
            .points()
            .filter(|&p| walls.get(p) == Some(&false))
            .collect();
        for &a in &floors {
            let seen_from_a = visible_from(&walls, a, 20);
            for &b in &floors {
                if seen_from_a.get(b) == Some(&true) {
                    let seen_from_b = visible_from(&walls, b, 20);
                    assert_eq!(
                        seen_from_b.get(a),
                        Some(&true),
                        "{a:?} sees {b:?} but not back"
                    );
                }
            }
        }
    }
}
