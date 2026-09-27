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
use crate::monster::{self, Monster};
use crate::rng::{self, Rng};

/// How far the player can see, in tiles.
pub const VIEW_RADIUS: i32 = 8;

/// Whether a player action used up time. Only actions that take time
/// let the monsters move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    /// Nothing happened, e.g. walking into a wall.
    Free,
    /// A normal action that takes one turn.
    TookTurn,
    /// Took the stairs. Monsters on the new floor get no move yet.
    NewFloor,
}

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
    pub monsters: Vec<Monster>,
    /// Randomness for events during play, such as monsters waking.
    /// Kept separate from floor generation so what happens on one
    /// floor never changes the layout of the next.
    pub(crate) rng: Rng,
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
            monsters: Vec::new(),
            rng: Rng::new(rng::mix(seed, u64::MAX)),
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
        self.monsters = monster::spawn_for_floor(&mut floor_rng, &level, depth);
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

    /// Applies one player action, then lets the monsters respond if
    /// the action took time.
    pub fn apply(&mut self, action: Action) {
        let outcome = match action {
            Action::Move(delta) => self.move_player(delta),
            Action::Wait => Outcome::TookTurn,
            Action::Descend => self.descend(),
            Action::Close(dir) => self.close_door(dir),
        };
        if outcome == Outcome::Free {
            return;
        }
        self.turn += 1;
        // Monsters need to know what the player can see (and so what
        // can see the player) after the player's move.
        self.update_fov();
        if outcome == Outcome::TookTurn {
            self.monsters_act();
            // Monsters may have opened doors, changing the view.
            self.update_fov();
        }
    }

    fn move_player(&mut self, delta: Point) -> Outcome {
        let target = self.player + delta;
        if let Some(m) = self.monster_at(target) {
            // Attacking arrives in milestone 5.
            let name = m.name();
            self.log(&format!("The {name} is in your way."));
            return Outcome::Free;
        }
        match self.map.tile(target) {
            Tile::DoorClosed => {
                // Opening takes a turn; you step through on the next.
                self.map.set_tile(target, Tile::DoorOpen);
                self.log("You open the door.");
                Outcome::TookTurn
            }
            tile if tile.is_walkable() => {
                self.player = target;
                if tile == Tile::StairsDown {
                    self.log("There is a staircase down here. Press > to descend.");
                }
                Outcome::TookTurn
            }
            _ => {
                self.log("The stone wall does not yield.");
                Outcome::Free
            }
        }
    }

    fn descend(&mut self) -> Outcome {
        if self.map.tile(self.player) != Tile::StairsDown {
            self.log("There are no stairs here.");
            return Outcome::Free;
        }
        self.enter_floor(self.depth + 1);
        self.log(&format!("You descend to depth {}.", self.depth));
        Outcome::NewFloor
    }

    fn close_door(&mut self, dir: Point) -> Outcome {
        let target = self.player + dir;
        match self.map.tile(target) {
            Tile::DoorOpen if self.monster_at(target).is_some() => {
                self.log("Something is standing in the doorway.");
                Outcome::Free
            }
            Tile::DoorOpen => {
                self.map.set_tile(target, Tile::DoorClosed);
                self.log("You close the door.");
                Outcome::TookTurn
            }
            Tile::DoorClosed => {
                self.log("That door is already closed.");
                Outcome::Free
            }
            _ => {
                self.log("There is no door there.");
                Outcome::Free
            }
        }
    }

    pub fn monster_at(&self, p: Point) -> Option<&Monster> {
        self.monsters.iter().find(|m| m.pos == p)
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
        game.monsters.clear();
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

    use crate::monster::{Ai, Kind};

    /// An empty 20x9 room with the player on the left side.
    fn room_game() -> Game {
        let mut game = Game::new(1);
        let mut map = Map::new_filled(22, 11);
        map.carve_room(1, 1, 20, 9);
        game.place_on_map(map, Point::new(2, 5));
        game.monsters.clear();
        game.log.clear();
        game.update_fov();
        game
    }

    fn add_monster(game: &mut Game, kind: Kind, pos: Point, ai: Ai) -> usize {
        game.monsters.push(Monster::new(kind, pos, ai));
        game.monsters.len() - 1
    }

    #[test]
    fn hunting_monster_closes_in_and_stops_adjacent() {
        let mut game = room_game();
        let rat = add_monster(&mut game, Kind::Rat, Point::new(7, 5), Ai::Asleep);
        game.monsters[rat].ai = Ai::Hunting {
            last_seen: game.player,
        };
        for _ in 0..10 {
            game.apply(Action::Wait);
        }
        assert!(game.monsters[rat].pos.is_adjacent(game.player));
        assert!(game.log.iter().any(|m| m.contains("lunges")));
    }

    #[test]
    fn speed_controls_how_often_monsters_move() {
        let mut game = room_game();
        let far = |x| Point::new(x, 1);
        // Out of the player's sight radius? No: the room is small, so
        // give them a far-off wander goal instead of hunting.
        let jackal = add_monster(
            &mut game,
            Kind::Jackal,
            far(10),
            Ai::Wandering { goal: far(20) },
        );
        let rat = add_monster(
            &mut game,
            Kind::Rat,
            Point::new(10, 9),
            Ai::Wandering {
                goal: Point::new(20, 9),
            },
        );
        let zombie = add_monster(
            &mut game,
            Kind::Zombie,
            Point::new(10, 5),
            Ai::Wandering {
                goal: Point::new(20, 5),
            },
        );
        game.player = Point::new(1, 9); // tuck the player in a corner
        game.update_fov();
        for m in &mut game.monsters {
            m.energy = 0;
        }
        for _ in 0..4 {
            game.apply(Action::Wait);
        }
        let moved = |i: usize, start: i32| game.monsters[i].pos.x - start;
        assert_eq!(moved(jackal, 10), 6); // speed 150
        assert_eq!(moved(rat, 10), 4); // speed 100
        assert_eq!(moved(zombie, 10), 2); // speed 50
    }

    #[test]
    fn sleeping_monster_out_of_sight_stays_put() {
        let mut game = corridor_game();
        let pos = Point::new(5, 1); // behind the closed door
        let rat = add_monster(&mut game, Kind::Rat, pos, Ai::Asleep);
        for _ in 0..20 {
            game.apply(Action::Wait);
        }
        assert_eq!(game.monsters[rat].pos, pos);
        assert_eq!(game.monsters[rat].ai, Ai::Asleep);
    }

    #[test]
    fn only_some_monsters_open_doors() {
        // The player waits at (1,1) behind the closed door at (3,1).
        // Each monster starts beyond the door, hunting toward (2,1).
        let door = Point::new(3, 1);
        let hunt = Ai::Hunting {
            last_seen: Point::new(2, 1),
        };

        let mut game = corridor_game();
        add_monster(&mut game, Kind::Rat, Point::new(5, 1), hunt);
        for _ in 0..5 {
            game.apply(Action::Wait);
        }
        assert_eq!(
            game.map.tile(door),
            Tile::DoorClosed,
            "rats can't open doors"
        );

        let mut game = corridor_game();
        add_monster(&mut game, Kind::Goblin, Point::new(5, 1), hunt);
        for _ in 0..5 {
            game.apply(Action::Wait);
        }
        assert_eq!(game.map.tile(door), Tile::DoorOpen, "goblins can");
    }

    #[test]
    fn player_cannot_walk_into_a_monster() {
        let mut game = room_game();
        add_monster(&mut game, Kind::Rat, Point::new(3, 5), Ai::Asleep);
        game.apply(Action::Move(EAST));
        assert_eq!(game.player, Point::new(2, 5));
        assert_eq!(game.turn, 0);
    }

    /// Plays random moves on real floors and checks the rules that must
    /// always hold, whatever happens.
    #[test]
    fn random_play_keeps_invariants() {
        for seed in 0..4 {
            let mut game = Game::new(seed);
            let mut dice = Rng::new(seed + 1000);
            for _ in 0..400 {
                let action = match dice.range(0, 12) {
                    0 => Action::Wait,
                    1 => Action::Descend,
                    // Stand on the stairs now and then to go deeper.
                    2 if dice.chance(5) => {
                        let stairs = game
                            .map
                            .points()
                            .find(|&p| game.map.tile(p) == Tile::StairsDown);
                        game.player = stairs.unwrap();
                        Action::Wait
                    }
                    _ => Action::Move(DIRECTIONS_8[dice.index(8)]),
                };
                game.apply(action);

                assert!(game.map.tile(game.player).is_walkable(), "seed {seed}");
                for (i, m) in game.monsters.iter().enumerate() {
                    assert!(
                        game.map.tile(m.pos).is_walkable(),
                        "seed {seed}: monster in wall"
                    );
                    assert_ne!(m.pos, game.player, "seed {seed}: monster on player");
                    let overlaps = game.monsters[i + 1..].iter().any(|o| o.pos == m.pos);
                    assert!(!overlaps, "seed {seed}: monsters overlap");
                }
            }
        }
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
