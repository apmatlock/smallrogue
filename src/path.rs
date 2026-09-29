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
/// `can_enter(p)` says whether a tile may be walked through, and is
/// checked for the goal too: a goal the walker can't enter has no
/// path. Returns `None` if there is no path or `from == to`.
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
            if unvisited && can_enter(n) {
                came_from.set(n, Some(p));
                queue.push_back(n);
            }
        }
    }
    None
}

/// Like `first_step`, but heads for whichever tile satisfying
/// `is_goal` is nearest. Used by auto-explore, which wants the closest
/// unexplored spot rather than one fixed destination.
pub fn first_step_to_any(
    from: Point,
    width: i32,
    height: i32,
    can_enter: impl Fn(Point) -> bool,
    is_goal: impl Fn(Point) -> bool,
) -> Option<Point> {
    let mut came_from: Grid<Option<Point>> = Grid::new(width, height, None);
    came_from.set(from, Some(from));
    let mut queue = VecDeque::from([from]);

    while let Some(p) = queue.pop_front() {
        if p != from && is_goal(p) {
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
            if came_from.get(n) == Some(&None) && can_enter(n) {
                came_from.set(n, Some(p));
                queue.push_back(n);
            }
        }
    }
    None
}

/// Every tile reachable from `from` (not counting `from` itself),
/// walking only through tiles where `can_enter` is true.
pub fn reachable(
    from: Point,
    width: i32,
    height: i32,
    can_enter: impl Fn(Point) -> bool,
) -> Vec<Point> {
    let mut seen = Grid::new(width, height, false);
    seen.set(from, true);
    let mut queue = VecDeque::from([from]);
    let mut found = Vec::new();
    while let Some(p) = queue.pop_front() {
        for d in STEPS {
            let n = p + d;
            if seen.get(n) == Some(&false) && can_enter(n) {
                seen.set(n, true);
                found.push(n);
                queue.push_back(n);
            }
        }
    }
    found
}

/// Every tile within `max` steps of `from` (not counting `from`
/// itself), with how many steps it takes to get there, nearest first.
pub fn distances(
    from: Point,
    width: i32,
    height: i32,
    max: usize,
    can_enter: impl Fn(Point) -> bool,
) -> Vec<(Point, usize)> {
    let mut seen = Grid::new(width, height, false);
    seen.set(from, true);
    let mut queue = VecDeque::from([(from, 0)]);
    let mut found = Vec::new();
    while let Some((p, dist)) = queue.pop_front() {
        if dist == max {
            continue;
        }
        for d in STEPS {
            let n = p + d;
            if seen.get(n) == Some(&false) && can_enter(n) {
                seen.set(n, true);
                found.push((n, dist + 1));
                queue.push_back((n, dist + 1));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_stop_at_the_limit() {
        let found = distances(Point::new(0, 0), 10, 1, 3, |_| true);
        assert_eq!(
            found,
            vec![
                (Point::new(1, 0), 1),
                (Point::new(2, 0), 2),
                (Point::new(3, 0), 3)
            ]
        );
    }

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
    fn reachable_stops_at_walls() {
        let tiles = reachable(Point::new(0, 0), 5, 5, |p| p.x < 2);
        assert_eq!(tiles.len(), 9); // a 2x5 strip, minus the start
        assert!(tiles.iter().all(|p| p.x < 2));
    }

    #[test]
    fn heads_for_the_nearest_goal() {
        // Goals at x = 4 and x = 1 on the same row; x = 1 is nearer.
        let goal = |p: Point| p.y == 0 && (p.x == 4 || p.x == 1);
        let step = first_step_to_any(Point::new(2, 0), 5, 5, |_| true, goal);
        assert_eq!(step, Some(Point::new(1, 0)));
        let none = first_step_to_any(Point::new(2, 0), 5, 5, |_| true, |_| false);
        assert_eq!(none, None);
    }

    #[test]
    fn prefers_straight_steps() {
        let step = first_step(Point::new(0, 0), Point::new(3, 0), 5, 5, |_| true);
        assert_eq!(step, Some(Point::new(1, 0)));
    }
}
