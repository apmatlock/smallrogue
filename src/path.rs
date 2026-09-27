//! Pathfinding: how monsters find their way around walls.
//!
//! Breadth-first search (BFS) explores outward from the start one step
//! at a time, like ripples in a pond, so the first time it reaches the
//! goal it has found a shortest route. With every step costing the same
//! and floors this small, BFS is as good as fancier algorithms like A*
//! and much simpler.

use std::collections::VecDeque;

use crate::geom::Point;
use crate::grid::Grid;

/// Straight steps are tried before diagonals, which makes paths look
/// natural instead of zig-zagging.
const STEPS: [Point; 8] = [
    Point::new(0, -1),
    Point::new(1, 0),
    Point::new(0, 1),
    Point::new(-1, 0),
    Point::new(1, -1),
    Point::new(1, 1),
    Point::new(-1, 1),
    Point::new(-1, -1),
];

/// Finds the first step of a shortest path from `from` to `to`.
///
/// `can_enter(p)` says whether a tile may be walked through. The goal
/// itself is always allowed, so a monster can path to the player's
/// tile even though it is occupied. Returns `None` if there is no path
/// or `from == to`.
pub fn first_step(
    from: Point,
    to: Point,
    width: i32,
    height: i32,
    can_enter: impl Fn(Point) -> bool,
) -> Option<Point> {
    if from == to {
        return None;
    }
    // For each reached tile, the tile we came from. `None` = unvisited.
    let mut came_from: Grid<Option<Point>> = Grid::new(width, height, None);
    came_from.set(from, Some(from));
    let mut queue = VecDeque::from([from]);

    while let Some(p) = queue.pop_front() {
        if p == to {
            // Walk the trail backwards until the step right after
            // `from`.
            let mut step = p;
            loop {
                let prev = came_from.get(step).copied().flatten()?;
                if prev == from {
                    return Some(step);
                }
                step = prev;
            }
        }
        for d in STEPS {
            let n = p + d;
            let unvisited = came_from.get(n) == Some(&None);
            if unvisited && (n == to || can_enter(n)) {
                came_from.set(n, Some(p));
                queue.push_back(n);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goes_around_a_wall() {
        // A wall at x = 2 from y = 0 to 3; the only gap is at y = 4.
        let wall = |p: Point| p.x == 2 && p.y < 4;
        let step = first_step(Point::new(0, 0), Point::new(4, 0), 5, 5, |p| !wall(p));
        // The first step must head down toward the gap.
        assert!(matches!(step, Some(p) if p.y == 1));
    }

    #[test]
    fn no_path_returns_none() {
        let step = first_step(Point::new(0, 0), Point::new(4, 0), 5, 5, |p| p.x != 2);
        assert_eq!(step, None);
    }

    #[test]
    fn prefers_straight_steps() {
        let step = first_step(Point::new(0, 0), Point::new(3, 0), 5, 5, |_| true);
        assert_eq!(step, Some(Point::new(1, 0)));
    }
}
