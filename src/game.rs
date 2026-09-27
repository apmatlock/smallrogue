//! The complete game state and the rules that change it.
//!
//! Nothing in this module knows about the terminal. It only answers
//! "what happens when the player does X?". Keeping it that way makes it
//! easy to test, to save later, and to draw with tiles someday.

use crate::geom::Point;
use crate::map::Map;

/// Something the player asked to do. The input module turns key
/// presses into these, so the game never sees raw keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Move(Point),
    Wait,
}

pub struct Game {
    pub map: Map,
    pub player: Point,
    /// Messages shown in the log, oldest first.
    pub log: Vec<String>,
    pub turn: u64,
}

impl Game {
    pub fn new() -> Self {
        let (map, start) = test_map();
        Self {
            map,
            player: start,
            log: vec!["You descend into the dark.".to_string()],
            turn: 0,
        }
    }

    /// Applies one player action. Only actions that take time
    /// advance the turn counter; bumping a wall does not.
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Move(delta) => {
                let target = self.player + delta;
                if self.map.is_walkable(target) {
                    self.player = target;
                    self.turn += 1;
                } else {
                    self.log("The stone wall does not yield.");
                }
            }
            Action::Wait => self.turn += 1,
        }
    }

    pub fn log(&mut self, message: &str) {
        // Skip exact repeats so bumping a wall ten times doesn't
        // flood the log.
        if self.log.last().map(String::as_str) != Some(message) {
            self.log.push(message.to_string());
        }
    }
}

/// A hand-made map, larger than most terminals so the scrolling view
/// can be tested. Milestone 2 replaces this with random generation.
fn test_map() -> (Map, Point) {
    let mut map = Map::new_filled(120, 50);

    // Rooms: (x, y, width, height)
    let rooms = [
        (3, 3, 12, 7),
        (25, 4, 16, 9),
        (52, 2, 10, 6),
        (75, 5, 20, 10),
        (100, 3, 15, 8),
        (6, 22, 14, 10),
        (35, 25, 22, 12),
        (70, 28, 12, 8),
        (95, 30, 18, 14),
        (20, 40, 10, 7),
    ];
    for &(x, y, w, h) in &rooms {
        map.carve_room(x, y, w, h);
    }

    // Join each room to the next with an L-shaped corridor between
    // their centers.
    for pair in rooms.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (ax, ay) = (a.0 + a.2 / 2, a.1 + a.3 / 2);
        let (bx, by) = (b.0 + b.2 / 2, b.1 + b.3 / 2);
        map.carve_h_corridor(ax, bx, ay);
        map.carve_v_corridor(ay, by, bx);
    }

    // A few pillars so the rooms aren't all empty boxes.
    for x in (38..55).step_by(4) {
        map.set_tile(Point::new(x, 28), crate::map::Tile::Wall);
        map.set_tile(Point::new(x, 33), crate::map::Tile::Wall);
    }

    let start = Point::new(3 + 6, 3 + 3);
    (map, start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walls_block_movement_and_cost_no_turn() {
        let mut game = Game::new();
        // Walk left until blocked by the room's west wall.
        for _ in 0..20 {
            game.apply(Action::Move(Point::new(-1, 0)));
        }
        assert_eq!(game.player.x, 3);
        let turns = game.turn;
        game.apply(Action::Move(Point::new(-1, 0)));
        assert_eq!(game.turn, turns);
    }

    #[test]
    fn repeated_messages_are_not_duplicated() {
        let mut game = Game::new();
        game.log("Hello");
        game.log("Hello");
        assert_eq!(game.log.iter().filter(|m| *m == "Hello").count(), 1);
    }
}

