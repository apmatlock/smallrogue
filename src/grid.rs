//! A rectangular grid holding one value per tile.
//!
//! `Grid<T>` is generic: `Grid<Tile>` stores the map's layout,
//! `Grid<bool>` stores which tiles are visible or remembered. Writing
//! the indexing logic once, here, means it is tested once and every
//! grid gets bounds checking for free.

use crate::geom::Point;

#[derive(Clone, Debug)]
pub struct Grid<T> {
    width: i32,
    height: i32,
    /// Values stored row by row in one flat Vec. A single allocation is
    /// faster and more cache friendly than a Vec of Vecs.
    cells: Vec<T>,
}

// `T: Clone` is a trait bound: these methods only exist for grids whose
// values can be copied, which `vec![value; n]` and `fill` require.
impl<T: Clone> Grid<T> {
    pub fn new(width: i32, height: i32, value: T) -> Self {
        assert!(width > 0 && height > 0, "grid must not be empty");
        Self {
            width,
            height,
            cells: vec![value; (width * height) as usize],
        }
    }

    /// Sets every cell to `value`, reusing the existing allocation.
    pub fn fill(&mut self, value: T) {
        self.cells.fill(value);
    }
}

impl<T> Grid<T> {
    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn in_bounds(&self, p: Point) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    /// Converts a point to a position in `cells`, or `None` if the
    /// point is off the grid. The only place that does this math.
    fn index(&self, p: Point) -> Option<usize> {
        self.in_bounds(p).then(|| (p.y * self.width + p.x) as usize)
    }

    pub fn get(&self, p: Point) -> Option<&T> {
        self.index(p).map(|i| &self.cells[i])
    }

    /// Writes to `p`. Writes outside the grid are ignored.
    pub fn set(&mut self, p: Point, value: T) {
        if let Some(i) = self.index(p) {
            self.cells[i] = value;
        }
    }

    /// Every point on the grid, row by row.
    pub fn points(&self) -> impl Iterator<Item = Point> + use<T> {
        let (w, h) = (self.width, self.height);
        (0..h).flat_map(move |y| (0..w).map(move |x| Point::new(x, y)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_and_set_respect_bounds() {
        let mut g = Grid::new(3, 2, 0);
        g.set(Point::new(2, 1), 7);
        g.set(Point::new(3, 1), 9); // off the grid: ignored
        assert_eq!(g.get(Point::new(2, 1)), Some(&7));
        assert_eq!(g.get(Point::new(-1, 0)), None);
        assert_eq!(g.points().count(), 6);
    }
}
