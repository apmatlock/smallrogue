//! Monster types, their data, and placing them on a new floor.
//!
//! Every kind of monster is described by one `Species` entry in a
//! table. Adding a monster means adding a `Kind` variant and one entry,
//! with no other code changes.

use crate::dungeon::{self, Level};
use crate::frame::Rgb;
use crate::geom::Point;
use crate::rng::Rng;

/// Energy a creature spends to take one action. Each turn a monster
/// gains energy equal to its speed, so speed 100 acts once per turn,
/// 150 acts three times every two turns, and 50 every other turn.
pub const ACTION_COST: i32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Rat,
    Jackal,
    Goblin,
    Zombie,
}

pub struct Species {
    pub name: &'static str,
    pub glyph: char,
    pub color: Rgb,
    pub speed: i32,
    /// How far it can see you, at most the player's own sight radius.
    pub sight: i32,
    pub opens_doors: bool,
    /// The shallowest depth where it can appear.
    pub min_depth: u32,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Rat, Kind::Jackal, Kind::Goblin, Kind::Zombie];

    pub fn species(self) -> &'static Species {
        match self {
            Kind::Rat => &Species {
                name: "rat",
                glyph: 'r',
                color: Rgb(150, 120, 90),
                speed: 100,
                sight: 6,
                opens_doors: false,
                min_depth: 1,
            },
            Kind::Jackal => &Species {
                name: "jackal",
                glyph: 'j',
                color: Rgb(190, 150, 70),
                speed: 150,
                sight: 8,
                opens_doors: false,
                min_depth: 1,
            },
            Kind::Goblin => &Species {
                name: "goblin",
                glyph: 'g',
                color: Rgb(90, 160, 70),
                speed: 100,
                sight: 8,
                opens_doors: true,
                min_depth: 2,
            },
            Kind::Zombie => &Species {
                name: "zombie",
                glyph: 'z',
                color: Rgb(130, 150, 120),
                speed: 50,
                sight: 5,
                opens_doors: true,
                min_depth: 2,
            },
        }
    }
}

/// What a monster is currently doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ai {
    /// Does nothing until it notices the player.
    Asleep,
    /// Walking toward a random spot on the floor.
    Wandering { goal: Point },
    /// Chasing the player, heading for where it last saw them.
    Hunting { last_seen: Point },
}

#[derive(Clone, Debug)]
pub struct Monster {
    pub kind: Kind,
    pub pos: Point,
    pub energy: i32,
    pub ai: Ai,
}

impl Monster {
    pub fn new(kind: Kind, pos: Point, ai: Ai) -> Self {
        Self {
            kind,
            pos,
            energy: 0,
            ai,
        }
    }

    pub fn species(&self) -> &'static Species {
        self.kind.species()
    }

    pub fn name(&self) -> &'static str {
        self.species().name
    }
}

/// Places monsters for a new floor. Deeper floors get more monsters and
/// a wider choice of kinds. The start room is always left empty, so the
/// player never begins a floor next to something awake.
pub fn spawn_for_floor(rng: &mut Rng, level: &Level, depth: u32) -> Vec<Monster> {
    let pool: Vec<Kind> = Kind::ALL
        .into_iter()
        .filter(|k| k.species().min_depth <= depth)
        .collect();
    let rooms: Vec<_> = level
        .rooms
        .iter()
        .filter(|&&r| r != level.start_room)
        .collect();

    let count = (2 + depth as usize).min(12);
    let mut monsters: Vec<Monster> = Vec::new();
    // A few spare attempts, in case a chosen spot is taken.
    for _ in 0..count * 3 {
        if monsters.len() == count || rooms.is_empty() {
            break;
        }
        // Pick the room first: `rng` can't be borrowed twice in one
        // expression.
        let room = *rooms[rng.index(rooms.len())];
        let pos = dungeon::random_point_in(rng, room);
        let free = level.map.tile(pos).is_walkable() && monsters.iter().all(|m| m.pos != pos);
        if !free {
            continue;
        }
        let kind = pool[rng.index(pool.len())];
        let ai = if rng.chance(50) {
            Ai::Asleep
        } else {
            Ai::Wandering { goal: pos }
        };
        let mut monster = Monster::new(kind, pos, ai);
        // A random head start stops every monster acting in lockstep.
        monster.energy = rng.range(0, ACTION_COST);
        monsters.push(monster);
    }
    monsters
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dungeon::{STANDARD, generate};

    #[test]
    fn spawns_avoid_the_start_room_and_walls() {
        for seed in 0..50 {
            let mut rng = Rng::new(seed);
            let level = generate(&mut rng, &STANDARD);
            let monsters = spawn_for_floor(&mut rng, &level, 3);
            assert!(!monsters.is_empty());
            let r = level.start_room;
            for m in &monsters {
                assert!(level.map.tile(m.pos).is_walkable());
                let in_start =
                    m.pos.x >= r.x && m.pos.x < r.x + r.w && m.pos.y >= r.y && m.pos.y < r.y + r.h;
                assert!(!in_start, "seed {seed}: monster in start room");
            }
        }
    }

    #[test]
    fn shallow_floors_only_have_shallow_monsters() {
        let mut rng = Rng::new(5);
        let level = generate(&mut rng, &STANDARD);
        for m in spawn_for_floor(&mut rng, &level, 1) {
            assert_eq!(m.species().min_depth, 1);
        }
    }
}
