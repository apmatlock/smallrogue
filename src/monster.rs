//! Monster types, their data, and placing them on a new floor.
//!
//! Every kind of monster is described by one `Species` entry in a
//! table. Adding a monster means adding a `Kind` variant and one entry,
//! with no other code changes.

use crate::combat::{Attack, Defense};
use crate::dungeon::{self, Level};
use crate::frame::Rgb;
use crate::geom::Point;
use crate::rng::Rng;

/// Energy a creature spends to take one action. Each turn a monster
/// gains energy equal to its speed, so speed 100 acts once per turn,
/// 150 acts three times every two turns, and 50 every other turn.
pub const ACTION_COST: i32 = 100;

// ---- Scaling with depth ------------------------------------------------
// A monster on depth d is (d - 1) floors "deeper" than its base stats.
// These are the knobs balance runs tune.

/// Extra health per floor, in percent, compounding: each floor's
/// monsters have this much more than the floor above. Compounding keeps
/// the endless dungeon ahead of a character who keeps growing.
pub const HP_PERCENT_PER_FLOOR: f64 = 6.0;
/// Floors per +1 accuracy.
pub const FLOORS_PER_ACCURACY_POINT: i32 = 3;
/// Floors per +1 damage (to both ends of the damage range).
pub const FLOORS_PER_DAMAGE_POINT: i32 = 2;
/// Floors per +1 dodge.
pub const FLOORS_PER_DODGE_POINT: i32 = 4;
/// Chance a monster starts asleep on depth 1, in percent. Deeper floors
/// are more alert: this drops by `ASLEEP_DROP_PER_FLOOR` per floor,
/// down to `ASLEEP_MIN_PERCENT`.
pub const ASLEEP_PERCENT: i32 = 50;
pub const ASLEEP_DROP_PER_FLOOR: i32 = 3;
pub const ASLEEP_MIN_PERCENT: i32 = 5;
/// Monsters per floor: 2 plus the depth, up to this many.
pub const MAX_MONSTERS: usize = 16;

/// Extra experience per floor, in percent of the base. Not compounding,
/// so the character can't simply outgrow the dungeon.
pub const XP_PERCENT_PER_FLOOR: i32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Rat,
    Jackal,
    Goblin,
    Zombie,
    Orc,
    Ghoul,
    Troll,
    Wraith,
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
    pub max_hp: i32,
    pub accuracy: i32,
    pub dodge: i32,
    /// Damage range, inclusive: (min, max).
    pub damage: (i32, i32),
    pub armor: i32,
    /// How its attack reads in the log: "The rat bites you."
    pub verb: &'static str,
    /// Experience for killing one.
    pub xp: u32,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Rat,
        Kind::Jackal,
        Kind::Goblin,
        Kind::Zombie,
        Kind::Orc,
        Kind::Ghoul,
        Kind::Troll,
        Kind::Wraith,
    ];

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
                max_hp: 4,
                accuracy: 2,
                dodge: 3,
                damage: (1, 3),
                armor: 0,
                verb: "bites",
                xp: 2,
            },
            Kind::Jackal => &Species {
                name: "jackal",
                glyph: 'j',
                color: Rgb(190, 150, 70),
                speed: 150,
                sight: 8,
                opens_doors: false,
                min_depth: 1,
                max_hp: 6,
                accuracy: 3,
                dodge: 4,
                damage: (1, 3),
                armor: 0,
                verb: "bites",
                xp: 3,
            },
            Kind::Goblin => &Species {
                name: "goblin",
                glyph: 'g',
                color: Rgb(90, 160, 70),
                speed: 100,
                sight: 8,
                opens_doors: true,
                min_depth: 2,
                max_hp: 10,
                accuracy: 3,
                dodge: 2,
                damage: (2, 5),
                armor: 1,
                verb: "hits",
                xp: 6,
            },
            Kind::Zombie => &Species {
                name: "zombie",
                glyph: 'z',
                color: Rgb(130, 150, 120),
                speed: 50,
                sight: 5,
                opens_doors: true,
                min_depth: 2,
                max_hp: 16,
                accuracy: 1,
                dodge: 0,
                damage: (3, 6),
                armor: 1,
                verb: "claws",
                xp: 8,
            },
            Kind::Orc => &Species {
                name: "orc",
                glyph: 'o',
                color: Rgb(170, 95, 60),
                speed: 100,
                sight: 7,
                opens_doors: true,
                min_depth: 5,
                max_hp: 16,
                accuracy: 4,
                dodge: 2,
                damage: (3, 7),
                armor: 2,
                verb: "hits",
                xp: 10,
            },
            Kind::Ghoul => &Species {
                name: "ghoul",
                glyph: 'G',
                color: Rgb(150, 170, 130),
                speed: 100,
                sight: 7,
                opens_doors: true,
                min_depth: 9,
                max_hp: 22,
                accuracy: 5,
                dodge: 3,
                damage: (3, 8),
                armor: 1,
                verb: "claws",
                xp: 16,
            },
            Kind::Troll => &Species {
                name: "troll",
                glyph: 'T',
                color: Rgb(95, 140, 75),
                speed: 100,
                sight: 6,
                opens_doors: true,
                min_depth: 13,
                max_hp: 34,
                accuracy: 5,
                dodge: 1,
                damage: (4, 11),
                armor: 3,
                verb: "pounds",
                xp: 30,
            },
            Kind::Wraith => &Species {
                name: "wraith",
                glyph: 'W',
                color: Rgb(185, 185, 225),
                speed: 150,
                sight: 8,
                opens_doors: true,
                min_depth: 16,
                max_hp: 30,
                accuracy: 7,
                dodge: 6,
                damage: (4, 10),
                armor: 0,
                verb: "touches",
                xp: 40,
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
    pub hp: i32,
    /// Floors of scaling on top of its base stats: depth minus 1.
    pub boost: i32,
}

impl Monster {
    /// A monster with its base stats, as on depth 1. Tests use this to
    /// set up fights; the game always spawns with `at_depth`.
    #[cfg(test)]
    pub fn new(kind: Kind, pos: Point, ai: Ai) -> Self {
        Self::at_depth(kind, pos, ai, 1)
    }

    /// A monster scaled for the given depth.
    pub fn at_depth(kind: Kind, pos: Point, ai: Ai, depth: u32) -> Self {
        let mut monster = Self {
            kind,
            pos,
            energy: 0,
            ai,
            hp: 0,
            boost: depth.saturating_sub(1) as i32,
        };
        monster.hp = monster.max_hp();
        monster
    }

    /// Full health for this monster, scaled by depth.
    pub fn max_hp(&self) -> i32 {
        let growth = (1.0 + HP_PERCENT_PER_FLOOR / 100.0).powi(self.boost);
        (self.species().max_hp as f64 * growth).round() as i32
    }

    /// Experience for killing it, scaled by depth.
    pub fn xp(&self) -> u32 {
        (self.species().xp as i32 * (100 + XP_PERCENT_PER_FLOOR * self.boost) / 100) as u32
    }

    pub fn species(&self) -> &'static Species {
        self.kind.species()
    }

    pub fn name(&self) -> &'static str {
        self.species().name
    }

    pub fn attack(&self) -> Attack {
        let s = self.species();
        let damage = self.boost / FLOORS_PER_DAMAGE_POINT;
        Attack {
            accuracy: s.accuracy + self.boost / FLOORS_PER_ACCURACY_POINT,
            damage: (s.damage.0 + damage, s.damage.1 + damage),
        }
    }

    pub fn defense(&self) -> Defense {
        let s = self.species();
        Defense {
            dodge: s.dodge + self.boost / FLOORS_PER_DODGE_POINT,
            armor: s.armor,
        }
    }
}

/// Places monsters for a new floor. Deeper floors get more monsters and
/// a wider choice of kinds. The start room is always left empty, so the
/// player never begins a floor next to something awake.
pub fn spawn_for_floor(rng: &mut Rng, level: &Level, depth: u32) -> Vec<Monster> {
    // Kinds that arrived recently are three times as common as old ones,
    // so each stretch of the dungeon has its own feel.
    let pool: Vec<(Kind, i32)> = Kind::ALL
        .into_iter()
        .filter(|k| k.species().min_depth <= depth)
        .map(|k| {
            let recent = depth - k.species().min_depth < 6;
            (k, if recent { 3 } else { 1 })
        })
        .collect();
    let total_weight: i32 = pool.iter().map(|(_, w)| w).sum();
    let rooms: Vec<_> = level
        .rooms
        .iter()
        .filter(|&&r| r != level.start_room)
        .collect();

    let count = (2 + depth as usize).min(MAX_MONSTERS);
    let asleep_percent =
        (ASLEEP_PERCENT - ASLEEP_DROP_PER_FLOOR * (depth as i32 - 1)).max(ASLEEP_MIN_PERCENT);
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
        let mut roll = rng.range(0, total_weight);
        let kind = pool
            .iter()
            .find(|(_, w)| {
                roll -= w;
                roll < 0
            })
            .map(|(k, _)| *k)
            .expect("the rolls add up to the total weight");
        let ai = if rng.chance(asleep_percent) {
            Ai::Asleep
        } else {
            Ai::Wandering { goal: pos }
        };
        let mut monster = Monster::at_depth(kind, pos, ai, depth);
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
    fn monsters_scale_with_depth() {
        let shallow = Monster::at_depth(Kind::Orc, Point::default(), Ai::Asleep, 4);
        let deep = Monster::at_depth(Kind::Orc, Point::default(), Ai::Asleep, 16);
        assert!(deep.max_hp() > shallow.max_hp());
        assert_eq!(deep.hp, deep.max_hp(), "starts at full health");
        assert!(deep.attack().accuracy > shallow.attack().accuracy);
        assert!(deep.attack().damage.1 > shallow.attack().damage.1);
        assert!(deep.defense().dodge > shallow.defense().dodge);
        assert!(deep.xp() > shallow.xp());
        let base = Monster::new(Kind::Rat, Point::default(), Ai::Asleep);
        assert_eq!(base.max_hp(), Kind::Rat.species().max_hp);
    }

    #[test]
    fn new_monsters_appear_deeper() {
        let mut rng = Rng::new(8);
        let level = generate(&mut rng, &STANDARD);
        let kinds_at = |depth, rng: &mut Rng| -> Vec<Kind> {
            (0..30)
                .flat_map(|_| spawn_for_floor(rng, &level, depth))
                .map(|m| m.kind)
                .collect()
        };
        assert!(!kinds_at(3, &mut rng).contains(&Kind::Orc));
        let deep = kinds_at(16, &mut rng);
        for kind in [Kind::Orc, Kind::Ghoul, Kind::Troll, Kind::Wraith] {
            assert!(deep.contains(&kind), "{kind:?} missing at depth 16");
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
