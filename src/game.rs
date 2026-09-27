//! The complete game state and the rules that change it.
//!
//! Nothing in this module knows about the terminal. It only answers
//! "what happens when the player does X?". Keeping it that way makes it
//! easy to test, to save later, and to draw with tiles someday.

use crate::dungeon;
use crate::fov;
use crate::geom::{DIRECTIONS_8, Point};
use crate::grid::Grid;
use crate::map::{Map, Tile};
use crate::rng::{self, Rng};

/// How far the player can see, in tiles.
pub const VIEW_RADIUS: i32 = 8;

/// Something the player asked to do. The input module turns key
/// presses into these, so the game never sees raw keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Step in a direction. Stepping into a closed door opens it.
    Move(Point),
    Wait,
    Descend,
    /// Close the door in this direction.
    Close(Point),
}

pub struct Game {
    pub map: Map,
    pub player: Point,
    /// Messages shown in the log, oldest first.
    pub log: Vec<String>,
    pub turn: u64,
    pub depth: u32,
    /// The run's seed. Each floor's layout is derived from it, so the
    /// same seed always produces the same dungeon.
    pub seed: u64,
    /// Tiles the player can see right now. Recomputed after every
    /// action, because moving or opening a door changes it.
    visible: Grid<bool>,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut game = Self {
            map: Map::new_filled(1, 1), // replaced by enter_floor below
            player: Point::default(),
            log: Vec::new(),
            turn: 0,
            depth: 0,
            seed,
            visible: Grid::new(1, 1, false),
        };
        game.enter_floor(1);
        game.log("You descend into the dark.");
        game.update_fov();
        game
    }

    /// Generates and moves the player onto a new floor.
    fn enter_floor(&mut self, depth: u32) {
        // Each floor gets its own generator seeded from (run seed,
        // depth). Floor 5 of a seed is then always the same, no matter
        // what random events happened on floors 1 to 4.
        let mut floor_rng = Rng::new(rng::mix(self.seed, depth as u64));
        let level = dungeon::generate(&mut floor_rng, &dungeon::STANDARD);
        self.place_on_map(level.map, level.start);
        self.depth = depth;
    }

    /// Swaps in a new map with the player at `at`, resetting what is
    /// visible to match the new map's size.
    fn place_on_map(&mut self, map: Map, at: Point) {
        self.visible = Grid::new(map.width(), map.height(), false);
        self.map = map;
        self.player = at;
    }

    /// Recalculates what the player sees and adds it to the map's
    /// memory.
    fn update_fov(&mut self) {
        self.visible.fill(false);

        // Borrow the two fields separately: the closures read the map
        // and write `visible` at the same time, which Rust allows
        // because they are different fields.
        let map = &self.map;
        let visible = &mut self.visible;
        fov::compute(
            self.player,
            VIEW_RADIUS,
            |p| map.tile(p).blocks_sight(),
            |p| visible.set(p, true),
        );

        let mut spotted_stairs = false;
        for p in self.map.points() {
            if self.is_visible(p) && !self.map.is_revealed(p) {
                self.map.reveal(p);
                spotted_stairs |= self.map.tile(p) == Tile::StairsDown;
            }
        }
        if spotted_stairs {
            self.log("You see a staircase leading down.");
        }
    }

    pub fn is_visible(&self, p: Point) -> bool {
        self.visible.get(p).copied().unwrap_or(false)
    }

    /// Applies one player action. Only actions that take time advance
    /// the turn counter; bumping a wall or a failed command does not.
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Move(delta) => self.move_player(delta),
            Action::Wait => self.turn += 1,
            Action::Descend => {
                if self.map.tile(self.player) == Tile::StairsDown {
                    self.enter_floor(self.depth + 1);
                    self.turn += 1;
                    self.log(&format!("You descend to depth {}.", self.depth));
                } else {
                    self.log("There are no stairs here.");
                }
            }
            Action::Close(dir) => {
                let target = self.player + dir;
                match self.map.tile(target) {
                    Tile::DoorOpen => {
                        self.map.set_tile(target, Tile::DoorClosed);
                        self.turn += 1;
                        self.log("You close the door.");
                    }
                    Tile::DoorClosed => self.log("That door is already closed."),
                    _ => self.log("There is no door there."),
                }
            }
        }
        self.update_fov();
    }

    fn move_player(&mut self, delta: Point) {
        let target = self.player + delta;
        match self.map.tile(target) {
            Tile::DoorClosed => {
                // Opening takes a turn; you step through on the next.
                self.map.set_tile(target, Tile::DoorOpen);
                self.turn += 1;
                self.log("You open the door.");
            }
            tile if tile.is_walkable() => {
                self.player = target;
                self.turn += 1;
                if tile == Tile::StairsDown {
                    self.log("There is a staircase down here. Press > to descend.");
                }
            }
            _ => self.log("The stone wall does not yield."),
        }
    }

    /// Directions from the player to each adjacent open door.
    pub fn adjacent_open_doors(&self) -> Vec<Point> {
        DIRECTIONS_8
            .into_iter()
            .filter(|&d| self.map.tile(self.player + d) == Tile::DoorOpen)
            .collect()
    }

    pub fn log(&mut self, message: &str) {
        // Skip exact repeats so bumping a wall ten times doesn't
        // flood the log.
        if self.log.last().map(String::as_str) != Some(message) {
            self.log.push(message.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A game on a tiny hand-made map, so tests control the layout.
    ///
    /// ```text
    /// #######
    /// #..+.>#     @ starts at (1,1)
    /// #######     + is a closed door at (3,1), > at (5,1)
    /// ```
    fn corridor_game() -> Game {
        let mut game = Game::new(1);
        let mut map = Map::new_filled(7, 3);
        map.carve_h_corridor(1, 5, 1);
        map.set_tile(Point::new(3, 1), Tile::DoorClosed);
        map.set_tile(Point::new(5, 1), Tile::StairsDown);
        game.place_on_map(map, Point::new(1, 1));
        game.log.clear();
        game.update_fov();
        game
    }

    const EAST: Point = Point::new(1, 0);
    const WEST: Point = Point::new(-1, 0);

    #[test]
    fn walls_block_movement_and_cost_no_turn() {
        let mut game = corridor_game();
        game.apply(Action::Move(WEST));
        assert_eq!(game.player, Point::new(1, 1));
        assert_eq!(game.turn, 0);
    }

    #[test]
    fn bumping_a_door_opens_it_then_you_walk_through() {
        let mut game = corridor_game();
        game.apply(Action::Move(EAST)); // to (2,1)
        game.apply(Action::Move(EAST)); // opens the door, stays put
        assert_eq!(game.player, Point::new(2, 1));
        assert_eq!(game.map.tile(Point::new(3, 1)), Tile::DoorOpen);
        game.apply(Action::Move(EAST)); // steps into the doorway
        assert_eq!(game.player, Point::new(3, 1));
        assert_eq!(game.turn, 3);
    }

    #[test]
    fn closing_a_door() {
        let mut game = corridor_game();
        game.map.set_tile(Point::new(3, 1), Tile::DoorOpen);
        game.player = Point::new(2, 1);
        assert_eq!(game.adjacent_open_doors(), vec![EAST]);
        game.apply(Action::Close(EAST));
        assert_eq!(game.map.tile(Point::new(3, 1)), Tile::DoorClosed);
    }

    #[test]
    fn descending_needs_stairs_and_builds_a_new_floor() {
        let mut game = corridor_game();
        game.apply(Action::Descend);
        assert_eq!(game.depth, 1);

        game.player = Point::new(5, 1);
        game.apply(Action::Descend);
        assert_eq!(game.depth, 2);
        assert_eq!(game.map.width(), dungeon::STANDARD.width);
        assert!(game.map.tile(game.player).is_walkable());
    }

    #[test]
    fn closed_doors_block_sight_until_opened() {
        let mut game = corridor_game();
        let beyond = Point::new(4, 1);
        assert!(!game.is_visible(beyond));
        assert!(!game.map.is_revealed(beyond));

        game.apply(Action::Move(EAST)); // step next to the door
        game.apply(Action::Move(EAST)); // open it
        assert!(game.is_visible(beyond));
        assert!(game.map.is_revealed(beyond));
        assert!(game.log.iter().any(|m| m.contains("staircase")));
    }

    #[test]
    fn remembered_tiles_stay_revealed_after_losing_sight() {
        let mut game = corridor_game();
        game.apply(Action::Move(EAST));
        game.apply(Action::Move(EAST)); // open the door
        game.player = Point::new(2, 1);
        game.apply(Action::Close(EAST));
        let beyond = Point::new(4, 1);
        assert!(!game.is_visible(beyond));
        assert!(game.map.is_revealed(beyond));
    }

    #[test]
    fn same_seed_same_run() {
        let a = Game::new(1234);
        let b = Game::new(1234);
        assert_eq!(a.player, b.player);
    }

    #[test]
    fn repeated_messages_are_not_duplicated() {
        let mut game = corridor_game();
        game.log("Hello");
        game.log("Hello");
        assert_eq!(game.log.iter().filter(|m| *m == "Hello").count(), 1);
    }
}
