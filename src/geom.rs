//! Small geometry helpers shared by every module.

use std::ops::{Add, Sub};

/// A position (or offset) on a grid.
///
/// We use `i32` rather than `u16`/`usize` so that offsets like (-1, 0)
/// and positions just off the edge of the map can be represented
/// without underflow. Converting to an array index happens in one
/// place only: `Map::index`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Squared straight-line distance. Squaring avoids a slow square
    /// root and still orders distances correctly.
    pub fn dist_sq(self, other: Point) -> i32 {
        let d = self - other;
        d.x * d.x + d.y * d.y
    }
}

/// The eight neighboring directions, clockwise from north.
pub const DIRECTIONS_8: [Point; 8] = [
    Point::new(0, -1),
    Point::new(1, -1),
    Point::new(1, 0),
    Point::new(1, 1),
    Point::new(0, 1),
    Point::new(-1, 1),
    Point::new(-1, 0),
    Point::new(-1, -1),
];

// Implementing `Add` lets us write `player + direction`.
impl Add for Point {
    type Output = Point;
    fn add(self, other: Point) -> Point {
        Point::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, other: Point) -> Point {
        Point::new(self.x - other.x, self.y - other.y)
    }
}

/// An axis-aligned rectangle: the floor area of a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn center(self) -> Point {
        Point::new(self.x + self.w / 2, self.y + self.h / 2)
    }

    /// True if the rectangles overlap or come within `margin` tiles of
    /// each other.
    pub fn overlaps(self, other: Rect, margin: i32) -> bool {
        self.x - margin < other.x + other.w
            && other.x - margin < self.x + self.w
            && self.y - margin < other.y + other.h
            && other.y - margin < self.y + self.h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_respects_margin() {
        let a = Rect::new(0, 0, 4, 4); // covers x 0..=3
        let b = Rect::new(6, 0, 4, 4); // covers x 6..=9, a gap of 2
        assert!(!a.overlaps(b, 2));
        assert!(a.overlaps(b, 3));
    }
}
