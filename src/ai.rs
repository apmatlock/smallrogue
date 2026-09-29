//! Monster behavior: what each monster does on its turn.
//!
//! This is a second `impl Game` block. Rust lets one type's methods be
//! split across files, which keeps game.rs focused on the player.

use crate::game::Game;
use crate::geom::Point;
use crate::grid::Grid;
use crate::map::Tile;
use crate::monster::{ACTION_COST, Ai};
use crate::path;
use crate::skills::Skill;

/// Chance per turn that a sleeping monster that can see the player
/// wakes up, before the player's stealth is counted.
const WAKE_PERCENT: i32 = 25;

/// Each level of stealth takes this much off the chance to wake.
const STEALTH_PER_LEVEL: i32 = 2;

impl Game {
    /// Gives every monster its energy for this turn and lets it act as
    /// many times as that energy allows.
    pub(crate) fn monsters_act(&mut self) {
        // Reuse the grid's memory unless the floor size changed.
        if self.monster_grid.width() == self.map.width()
            && self.monster_grid.height() == self.map.height()
        {
            self.monster_grid.fill(false);
        } else {
            self.monster_grid = Grid::new(self.map.width(), self.map.height(), false);
        }
        for m in &self.monsters {
            self.monster_grid.set(m.pos, true);
        }
        // Loop by index, not by iterator: each monster's turn needs to
        // read the others (to avoid walking into them), which a
        // mutable iterator over the Vec would not allow.
        for i in 0..self.monsters.len() {
            self.monsters[i].energy += self.monsters[i].species().speed;
            while self.monsters[i].energy >= ACTION_COST {
                if self.death.is_some() {
                    return;
                }
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
        self.is_visible(m.pos) && m.pos.dist_sq(self.player.pos) <= range * range + range
    }

    fn monster_turn(&mut self, i: usize) {
        let sees = self.monster_sees_player(i);
        let name = self.monsters[i].name();
        let visible = self.is_visible(self.monsters[i].pos);

        // First update what the monster is doing.
        let wake = (WAKE_PERCENT - STEALTH_PER_LEVEL * self.player.skill(Skill::Stealth)).max(5);
        let ai = match self.monsters[i].ai {
            Ai::Asleep if sees && self.rng.chance(wake) => {
                self.log(&format!("The {name} wakes up!"));
                Ai::Hunting {
                    last_seen: self.player.pos,
                }
            }
            Ai::Asleep => {
                // It could see the player and didn't wake: sneaking
                // past trains stealth.
                if sees {
                    self.train(Skill::Stealth, 1);
                }
                return;
            }
            Ai::Wandering { .. } if sees => {
                self.log(&format!("The {name} notices you!"));
                Ai::Hunting {
                    last_seen: self.player.pos,
                }
            }
            Ai::Hunting { .. } if sees => Ai::Hunting {
                last_seen: self.player.pos,
            },
            // Reached the last place it saw the player, and the player
            // is gone: give up and wander.
            Ai::Hunting { last_seen } if last_seen == self.monsters[i].pos => {
                if visible {
                    self.log(&format!("The {name} loses track of you."));
                }
                Ai::Wandering {
                    goal: self.new_wander_goal(i),
                }
            }
            // Reached its wandering goal: pick a new one.
            Ai::Wandering { goal } if goal == self.monsters[i].pos => Ai::Wandering {
                goal: self.new_wander_goal(i),
            },
            other => other,
        };
        self.monsters[i].ai = ai;

        // Then act on it.
        let pos = self.monsters[i].pos;
        let goal = match ai {
            Ai::Hunting { .. } if sees && pos.is_adjacent(self.player.pos) => {
                self.monster_attack(i);
                return;
            }
            Ai::Hunting { last_seen } => last_seen,
            Ai::Wandering { goal } => goal,
            Ai::Asleep => return,
        };
        match self.monster_path_step(i, goal) {
            Some(step) => self.monster_step(i, step),
            // No route, e.g. the goal is behind a door this monster
            // can't open. Rather than stand there forever, head
            // somewhere else next turn.
            None if !sees => {
                self.monsters[i].ai = Ai::Wandering {
                    goal: self.new_wander_goal(i),
                };
            }
            // Blocked while it can still see the player (usually by
            // other monsters in the way): wait for a gap.
            None => {}
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
            // Other creatures block the way, except that the player's
            // tile is allowed as the goal so hunters can path to them.
            let has_monster = self.monster_grid.get(p) == Some(&true);
            let occupied = (p != goal && p == self.player.pos) || has_monster;
            passable && !occupied
        };
        path::first_step(me.pos, goal, self.map.width(), self.map.height(), can_enter)
    }

    fn monster_step(&mut self, i: usize, step: Point) {
        let name = self.monsters[i].name();
        if self.map.tile(step) == Tile::DoorClosed {
            if !self.monsters[i].species().opens_doors {
                return;
            }
            // Opening a door uses the monster's action.
            self.map.set_tile(step, Tile::DoorOpen);
            self.fov_dirty = true;
            if self.is_visible(step) {
                self.log(&format!("The {name} opens a door."));
            }
        } else if step != self.player.pos && self.monster_grid.get(step) == Some(&false) {
            self.monster_grid.set(self.monsters[i].pos, false);
            self.monster_grid.set(step, true);
            self.monsters[i].pos = step;
        }
    }

    /// Picks a random tile that monster `i` can actually walk to, so
    /// it never sets off for somewhere it can't reach. Other monsters
    /// are ignored here, since they move out of the way over time.
    fn new_wander_goal(&mut self, i: usize) -> Point {
        let me = &self.monsters[i];
        let opens_doors = me.species().opens_doors;
        let map = &self.map;
        let tiles = path::reachable(me.pos, map.width(), map.height(), |p| {
            let tile = map.tile(p);
            tile.is_walkable() || (opens_doors && tile == Tile::DoorClosed)
        });
        if tiles.is_empty() {
            return me.pos; // boxed in: stay put
        }
        tiles[self.rng.index(tiles.len())]
    }
}
