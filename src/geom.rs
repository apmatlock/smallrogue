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
}

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
