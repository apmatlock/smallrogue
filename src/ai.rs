//! Monster behavior: what each monster does on its turn.
//!
//! This is a second `impl Game` block. Rust lets one type's methods be
//! split across files, which keeps game.rs focused on the player.

use crate::game::Game;
use crate::geom::Point;
use crate::map::Tile;
use crate::monster::{ACTION_COST, Ai};
use crate::path;

/// Chance per turn that a sleeping monster that can see the player
/// wakes up. The stealth skill will lower this in milestone 8.
const WAKE_PERCENT: i32 = 25;

impl Game {
    /// Gives every monster its energy for this turn and lets it act as
    /// many times as that energy allows.
    pub(crate) fn monsters_act(&mut self) {
        // Loop by index, not by iterator: each monster's turn needs to
        // read the others (to avoid walking into them), which a
        // mutable iterator over the Vec would not allow.
        for i in 0..self.monsters.len() {
            self.monsters[i].energy += self.monsters[i].species().speed;
            while self.monsters[i].energy >= ACTION_COST {
                self.monsters[i].energy -= ACTION_COST;
                self.monster_turn(i);
            }
        }
    }

    /// Can monster `i` see the player right now?
    ///
    /// Field of view is symmetric, so the monster sees the player
    /// exactly when the player sees the monster's tile, limited by the
    /// monster's own sight range.
    fn monster_sees_player(&self, i: usize) -> bool {
        let m = &self.monsters[i];
        let range = m.species().sight;
        self.is_visible(m.pos) && m.pos.dist_sq(self.player) <= range * range + range
    }

    fn monster_turn(&mut self, i: usize) {
        let sees = self.monster_sees_player(i);
        let name = self.monsters[i].name();
        let visible = self.is_visible(self.monsters[i].pos);

        // First update what the monster is doing.
        let ai = match self.monsters[i].ai {
            Ai::Asleep if sees && self.rng.chance(WAKE_PERCENT) => {
                self.log(&format!("The {name} wakes up!"));
                Ai::Hunting {
                    last_seen: self.player,
                }
            }
            Ai::Asleep => return,
            Ai::Wandering { .. } if sees => {
                self.log(&format!("The {name} notices you!"));
                Ai::Hunting {
                    last_seen: self.player,
                }
            }
            Ai::Hunting { .. } if sees => Ai::Hunting {
                last_seen: self.player,
            },
            // Reached the last place it saw the player, and the player
            // is gone: give up and wander.
            Ai::Hunting { last_seen } if last_seen == self.monsters[i].pos => {
                if visible {
                    self.log(&format!("The {name} loses track of you."));
                }
                Ai::Wandering {
                    goal: self.random_floor_tile(),
                }
            }
            // Reached its wandering goal: pick a new one.
            Ai::Wandering { goal } if goal == self.monsters[i].pos => Ai::Wandering {
                goal: self.random_floor_tile(),
            },
            other => other,
        };
        self.monsters[i].ai = ai;

        // Then act on it.
        let pos = self.monsters[i].pos;
        let goal = match ai {
            Ai::Hunting { .. } if sees && pos.is_adjacent(self.player) => {
                // Combat arrives in milestone 5.
                self.log(&format!("The {name} lunges at you!"));
                return;
            }
            Ai::Hunting { last_seen } => last_seen,
            Ai::Wandering { goal } => goal,
            Ai::Asleep => return,
        };
        if let Some(step) = self.monster_path_step(i, goal) {
            self.monster_step(i, step);
        }
    }

    /// The next tile on the way to `goal`, routing around other
    /// monsters and through doors the monster can open.
    fn monster_path_step(&self, i: usize, goal: Point) -> Option<Point> {
        let me = &self.monsters[i];
        let opens_doors = me.species().opens_doors;
        let can_enter = |p: Point| {
            let tile = self.map.tile(p);
            let passable = tile.is_walkable() || (opens_doors && tile == Tile::DoorClosed);
            passable && p != self.player && self.monster_at(p).is_none()
        };
        path::first_step(me.pos, goal, self.map.width(), self.map.height(), can_enter)
    }

    fn monster_step(&mut self, i: usize, step: Point) {
        let name = self.monsters[i].name();
        if self.map.tile(step) == Tile::DoorClosed {
            // Opening a door uses the monster's action.
            self.map.set_tile(step, Tile::DoorOpen);
            if self.is_visible(step) {
                self.log(&format!("The {name} opens a door."));
            }
        } else if step != self.player && self.monster_at(step).is_none() {
            self.monsters[i].pos = step;
        }
    }

    /// A random walkable tile, used as a wandering goal.
    fn random_floor_tile(&mut self) -> Point {
        for _ in 0..200 {
            let p = Point::new(
                self.rng.range(0, self.map.width()),
                self.rng.range(0, self.map.height()),
            );
            if self.map.tile(p).is_walkable() {
                return p;
            }
        }
        self.player // practically never reached
    }
}
