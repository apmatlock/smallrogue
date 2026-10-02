//! Monster types, their data, and placing them on a new floor.
//!
//! Every kind of monster is described by one `Species` entry in a
//! table. Adding a monster means adding a `Kind` variant and one entry,
//! with no other code changes.

use crate::combat::{Attack, Defense};
use crate::dungeon::{self, Level};
use crate::frame::Rgb;
use crate::geom::Point;
use crate::item::Item;
use crate::rng::Rng;
use crate::zone::Place;

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
/// How much more common a monster is in its home zone.
pub const HOME_WEIGHT: i32 = 4;

/// Each pass through the zones after the first scales monsters as if
/// they were this many floors deeper again, on top of their depth. The
/// main lever against characters who outgrow the dungeon.
pub const LOOP_EXTRA_FLOORS: u32 = 3;
/// How much more each loop after the second adds than the one before,
/// in floors. Without it a character strong enough to finish loop 2
/// outgrew the dungeon: a fifth of bot runs never died.
pub const LOOP_GROWTH: u32 = 4;

/// The extra floors of strength monsters get after `loops_done`
/// passes through the zones: 0, then 3, then growing by
/// `LOOP_GROWTH` more each loop.
pub fn loop_extra_floors(loops_done: u32) -> u32 {
    LOOP_EXTRA_FLOORS * loops_done + LOOP_GROWTH * loops_done * loops_done.saturating_sub(1) / 2
}

/// Past this depth, each floor deeper adds `DEEP_EXTRA_PER_FLOOR` more
/// floors of strength. Without it, runs that got through loop 2's
/// crypts (19-24) rarely died before loop 3: the rest of loop 2 was a
/// plateau.
pub const DEEP_RAMP_FROM: u32 = 24;
pub const DEEP_EXTRA_PER_FLOOR: u32 = 1;

/// The extra floors of strength from the deep ramp at this depth.
pub fn deep_extra_floors(depth: u32) -> u32 {
    depth.saturating_sub(DEEP_RAMP_FROM) * DEEP_EXTRA_PER_FLOOR
}

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
    Kobold,
    Newt,
    GiantBat,
    GiantAnt,
    Hobgoblin,
    Skeleton,
    GiantSpider,
    Ogre,
    StoneGolem,
    Vampire,
    Dragon,
    Demon,
    Redcap,
    Harpy,
    Draugr,
    Owlbear,
    Gargoyle,
    Bulette,
    Oni,
    FrostGiant,
    Monkey,
    AcidMound,
    PinkJelly,
}

/// Special powers that change how a fight plays out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ability {
    /// Heals a little every turn.
    Regenerates,
    /// Each hit permanently lowers the player's maximum health.
    DrainsMaxHealth,
    /// Each hit heals it by the damage it deals.
    DrinksBlood,
    /// Its hit steals an unequipped item instead of hurting; it then
    /// runs away with it.
    StealsAndFlees,
    /// Its hits may eat away the enchantment of the player's armor.
    CorrodesArmor,
    /// Splits in two when hit without dying.
    Splits,
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
    pub abilities: &'static [Ability],
    /// How many appear together: (fewest, most). (1, 1) for loners.
    pub pack: (u32, u32),
}

impl Species {
    pub fn has(&self, ability: Ability) -> bool {
        self.abilities.contains(&ability)
    }
}

impl Kind {
    pub const ALL: [Kind; 31] = [
        Kind::Rat,
        Kind::Jackal,
        Kind::Goblin,
        Kind::Zombie,
        Kind::Orc,
        Kind::Ghoul,
        Kind::Troll,
        Kind::Wraith,
        Kind::Kobold,
        Kind::Newt,
        Kind::GiantBat,
        Kind::GiantAnt,
        Kind::Hobgoblin,
        Kind::Skeleton,
        Kind::GiantSpider,
        Kind::Ogre,
        Kind::StoneGolem,
        Kind::Vampire,
        Kind::Dragon,
        Kind::Demon,
        Kind::Redcap,
        Kind::Harpy,
        Kind::Draugr,
        Kind::Owlbear,
        Kind::Gargoyle,
        Kind::Bulette,
        Kind::Oni,
        Kind::FrostGiant,
        Kind::Monkey,
        Kind::AcidMound,
        Kind::PinkJelly,
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
                abilities: &[],
                pack: (1, 1),
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
                abilities: &[],
                pack: (2, 3),
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
                abilities: &[],
                pack: (1, 1),
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
                abilities: &[],
                pack: (1, 1),
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
                abilities: &[],
                pack: (1, 2),
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
                abilities: &[],
                pack: (1, 1),
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
                abilities: &[Ability::Regenerates],
                pack: (1, 1),
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
                abilities: &[Ability::DrainsMaxHealth],
                pack: (1, 1),
            },
            Kind::Kobold => &Species {
                name: "kobold",
                glyph: 'k',
                color: Rgb(150, 110, 165),
                speed: 100,
                sight: 7,
                opens_doors: true,
                min_depth: 1,
                max_hp: 5,
                accuracy: 2,
                dodge: 2,
                damage: (1, 4),
                armor: 0,
                verb: "stabs",
                xp: 2,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Newt => &Species {
                name: "newt",
                glyph: ':',
                color: Rgb(200, 170, 60),
                speed: 50,
                sight: 5,
                opens_doors: false,
                min_depth: 1,
                max_hp: 3,
                accuracy: 1,
                dodge: 1,
                damage: (1, 2),
                armor: 0,
                verb: "bites",
                xp: 1,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::GiantBat => &Species {
                name: "giant bat",
                glyph: 'B',
                color: Rgb(145, 115, 95),
                speed: 200,
                sight: 8,
                opens_doors: false,
                min_depth: 2,
                max_hp: 5,
                accuracy: 4,
                dodge: 5,
                damage: (1, 3),
                armor: 0,
                verb: "bites",
                xp: 3,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::GiantAnt => &Species {
                name: "giant ant",
                glyph: 'a',
                color: Rgb(175, 60, 50),
                speed: 150,
                sight: 6,
                opens_doors: false,
                min_depth: 4,
                max_hp: 9,
                accuracy: 4,
                dodge: 3,
                damage: (2, 5),
                armor: 1,
                verb: "bites",
                xp: 6,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Hobgoblin => &Species {
                name: "hobgoblin",
                glyph: 'h',
                color: Rgb(205, 125, 60),
                speed: 100,
                sight: 8,
                opens_doors: true,
                min_depth: 6,
                max_hp: 12,
                accuracy: 4,
                dodge: 2,
                damage: (2, 6),
                armor: 2,
                verb: "slashes",
                xp: 8,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Skeleton => &Species {
                name: "skeleton",
                glyph: 'Z',
                color: Rgb(225, 225, 205),
                speed: 100,
                sight: 6,
                opens_doors: true,
                min_depth: 3,
                max_hp: 10,
                accuracy: 3,
                dodge: 1,
                damage: (2, 5),
                armor: 3,
                verb: "hits",
                xp: 8,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::GiantSpider => &Species {
                name: "giant spider",
                glyph: 's',
                color: Rgb(125, 100, 165),
                speed: 150,
                sight: 7,
                opens_doors: false,
                min_depth: 8,
                max_hp: 12,
                accuracy: 5,
                dodge: 4,
                damage: (2, 6),
                armor: 1,
                verb: "bites",
                xp: 12,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Ogre => &Species {
                name: "ogre",
                glyph: 'O',
                color: Rgb(175, 145, 95),
                speed: 75,
                sight: 6,
                opens_doors: true,
                min_depth: 10,
                max_hp: 30,
                accuracy: 2,
                dodge: 0,
                damage: (5, 11),
                armor: 1,
                verb: "smashes",
                xp: 20,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::StoneGolem => &Species {
                name: "stone golem",
                glyph: '8',
                color: Rgb(165, 165, 165),
                speed: 50,
                sight: 6,
                opens_doors: true,
                min_depth: 14,
                max_hp: 50,
                accuracy: 3,
                dodge: 0,
                damage: (4, 9),
                armor: 5,
                verb: "crushes",
                xp: 25,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Vampire => &Species {
                name: "vampire",
                glyph: 'V',
                color: Rgb(195, 40, 65),
                speed: 100,
                sight: 8,
                opens_doors: true,
                min_depth: 18,
                max_hp: 32,
                accuracy: 7,
                dodge: 5,
                damage: (4, 10),
                armor: 2,
                verb: "bites",
                xp: 30,
                abilities: &[Ability::Regenerates, Ability::DrinksBlood],
                pack: (1, 1),
            },
            Kind::Dragon => &Species {
                name: "dragon",
                glyph: 'D',
                color: Rgb(215, 85, 40),
                speed: 100,
                sight: 8,
                opens_doors: false,
                min_depth: 22,
                max_hp: 60,
                accuracy: 6,
                dodge: 2,
                damage: (6, 14),
                armor: 5,
                verb: "rends",
                xp: 45,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Demon => &Species {
                name: "demon",
                glyph: '&',
                color: Rgb(230, 60, 40),
                speed: 125,
                sight: 8,
                opens_doors: true,
                min_depth: 25,
                max_hp: 45,
                accuracy: 8,
                dodge: 6,
                damage: (6, 13),
                armor: 3,
                verb: "claws",
                xp: 50,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Redcap => &Species {
                name: "redcap",
                glyph: 'R',
                color: Rgb(190, 40, 40),
                speed: 100,
                sight: 7,
                opens_doors: true,
                min_depth: 3,
                max_hp: 7,
                accuracy: 4,
                dodge: 3,
                damage: (2, 5),
                armor: 1,
                verb: "slashes",
                xp: 4,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Harpy => &Species {
                name: "harpy",
                glyph: 'y',
                color: Rgb(200, 160, 110),
                speed: 150,
                sight: 8,
                opens_doors: false,
                min_depth: 5,
                max_hp: 8,
                accuracy: 5,
                dodge: 5,
                damage: (1, 5),
                armor: 0,
                verb: "rakes",
                xp: 6,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Draugr => &Species {
                name: "draugr",
                glyph: 'd',
                color: Rgb(120, 150, 170),
                speed: 100,
                sight: 6,
                opens_doors: true,
                min_depth: 5,
                max_hp: 14,
                accuracy: 4,
                dodge: 1,
                damage: (3, 7),
                armor: 3,
                verb: "strikes",
                xp: 11,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Owlbear => &Species {
                name: "owlbear",
                glyph: 'Y',
                color: Rgb(150, 110, 70),
                speed: 100,
                sight: 7,
                opens_doors: false,
                min_depth: 10,
                max_hp: 22,
                accuracy: 4,
                dodge: 2,
                damage: (3, 8),
                armor: 2,
                verb: "mauls",
                xp: 16,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Gargoyle => &Species {
                name: "gargoyle",
                glyph: 'q',
                color: Rgb(130, 135, 145),
                speed: 75,
                sight: 7,
                opens_doors: false,
                min_depth: 11,
                max_hp: 20,
                accuracy: 4,
                dodge: 2,
                damage: (3, 8),
                armor: 6,
                verb: "claws",
                xp: 18,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Bulette => &Species {
                name: "bulette",
                glyph: 'X',
                color: Rgb(140, 120, 90),
                speed: 100,
                sight: 6,
                opens_doors: false,
                min_depth: 12,
                max_hp: 30,
                accuracy: 5,
                dodge: 1,
                damage: (4, 9),
                armor: 4,
                verb: "bites",
                xp: 24,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Oni => &Species {
                name: "oni",
                glyph: 'U',
                color: Rgb(200, 70, 60),
                speed: 100,
                sight: 7,
                opens_doors: true,
                min_depth: 15,
                max_hp: 36,
                accuracy: 5,
                dodge: 2,
                damage: (5, 12),
                armor: 3,
                verb: "clubs",
                xp: 30,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::FrostGiant => &Species {
                name: "frost giant",
                glyph: 'P',
                color: Rgb(160, 200, 230),
                speed: 100,
                sight: 7,
                opens_doors: true,
                min_depth: 19,
                max_hp: 55,
                accuracy: 6,
                dodge: 1,
                damage: (6, 14),
                armor: 4,
                verb: "smashes",
                xp: 40,
                abilities: &[],
                pack: (1, 1),
            },
            Kind::Monkey => &Species {
                name: "monkey",
                glyph: 'M',
                color: Rgb(170, 125, 75),
                speed: 125,
                sight: 8,
                opens_doors: false,
                min_depth: 3,
                max_hp: 6,
                accuracy: 5,
                dodge: 5,
                damage: (1, 3),
                armor: 0,
                verb: "grabs at",
                xp: 4,
                abilities: &[Ability::StealsAndFlees],
                pack: (1, 1),
            },
            Kind::AcidMound => &Species {
                name: "acid mound",
                glyph: 'A',
                color: Rgb(150, 200, 60),
                speed: 75,
                sight: 5,
                opens_doors: false,
                min_depth: 6,
                max_hp: 14,
                accuracy: 4,
                dodge: 1,
                damage: (1, 4),
                armor: 0,
                verb: "burns",
                xp: 9,
                abilities: &[Ability::CorrodesArmor],
                pack: (1, 1),
            },
            Kind::PinkJelly => &Species {
                name: "pink jelly",
                glyph: 'J',
                color: Rgb(230, 130, 170),
                speed: 100,
                sight: 5,
                opens_doors: false,
                min_depth: 10,
                max_hp: 30,
                accuracy: 3,
                dodge: 0,
                damage: (1, 4),
                armor: 0,
                verb: "smothers",
                xp: 3,
                abilities: &[Ability::Splits],
                pack: (1, 1),
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
    /// Running from the player, like a thief with its loot.
    Fleeing,
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
    /// An item it stole, dropped when it dies.
    pub carrying: Option<Item>,
}

impl Monster {
    /// A monster with its base stats, as on depth 1. Tests use this to
    /// set up fights; the game always spawns with `at_depth`.
    #[cfg(test)]
    pub fn new(kind: Kind, pos: Point, ai: Ai) -> Self {
        Self::at_depth(kind, pos, ai, 1)
    }

    /// A monster scaled for the given depth, and for the loop that
    /// depth is in.
    pub fn at_depth(kind: Kind, pos: Point, ai: Ai, depth: u32) -> Self {
        let loops_done = Place::at_depth(depth).loop_number - 1;
        let mut monster = Self {
            kind,
            pos,
            energy: 0,
            ai,
            hp: 0,
            boost: (depth.saturating_sub(1)
                + loop_extra_floors(loops_done)
                + deep_extra_floors(depth)) as i32,
            carrying: None,
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
pub fn spawn_for_floor(rng: &mut Rng, level: &Level, depth: u32, home: &[Kind]) -> Vec<Monster> {
    // Kinds that arrived recently are three times as common as old ones,
    // and kinds at home in the zone `HOME_WEIGHT` times, so each stretch
    // of the dungeon has its own feel.
    let pool: Vec<(Kind, i32)> = Kind::ALL
        .into_iter()
        .filter(|k| k.species().min_depth <= depth)
        .map(|k| {
            let recent = depth - k.species().min_depth < 6;
            let at_home = if home.contains(&k) { HOME_WEIGHT } else { 1 };
            (k, if recent { 3 } else { 1 } * at_home)
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
    let is_free = |monsters: &[Monster], pos: Point| {
        level.map.tile(pos).is_walkable() && monsters.iter().all(|m| m.pos != pos)
    };
    // A few spare attempts, in case a chosen spot is taken.
    for _ in 0..count * 3 {
        if monsters.len() >= count || rooms.is_empty() {
            break;
        }
        // Pick the room first: `rng` can't be borrowed twice in one
        // expression.
        let room = *rooms[rng.index(rooms.len())];
        let pos = dungeon::random_point_in(rng, room);
        if !is_free(&monsters, pos) {
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
        // Pack animals bring friends to the same room, all in the same
        // mood: a sleeping pack sleeps together.
        let (fewest, most) = kind.species().pack;
        let size = rng.range(fewest as i32, most as i32 + 1) as usize;
        let mut spots = vec![pos];
        for _ in 0..size * 4 {
            if spots.len() == size {
                break;
            }
            let p = dungeon::random_point_in(rng, room);
            if is_free(&monsters, p) && !spots.contains(&p) {
                spots.push(p);
            }
        }
        for p in spots {
            let ai = match ai {
                Ai::Wandering { .. } => Ai::Wandering { goal: p },
                other => other,
            };
            let mut monster = Monster::at_depth(kind, p, ai, depth);
            // A random head start stops every monster acting in lockstep.
            monster.energy = rng.range(0, ACTION_COST);
            monsters.push(monster);
        }
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
            let monsters = spawn_for_floor(&mut rng, &level, 3, &[]);
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
                .flat_map(|_| spawn_for_floor(rng, &level, depth, &[]))
                .map(|m| m.kind)
                .collect()
        };
        assert!(!kinds_at(3, &mut rng).contains(&Kind::Orc));
        let deep = kinds_at(16, &mut rng);
        for kind in [Kind::Orc, Kind::Ghoul, Kind::Troll, Kind::Wraith] {
            assert!(deep.contains(&kind), "{kind:?} missing at depth 16");
        }
        assert!(!deep.contains(&Kind::Dragon), "dragons wait until depth 22");
        let deepest = kinds_at(26, &mut rng);
        for kind in [Kind::Vampire, Kind::Dragon, Kind::Demon] {
            assert!(deepest.contains(&kind), "{kind:?} missing at depth 26");
        }
    }

    #[test]
    fn later_loops_scale_monsters_further() {
        let orc = |depth| Monster::at_depth(Kind::Orc, Point::default(), Ai::Asleep, depth);
        let last_floor = crate::zone::FLOORS_PER_ZONE * crate::zone::ZONES.len() as u32;
        // Within a loop, one floor deeper is one step of scaling.
        assert_eq!(orc(last_floor).boost, last_floor as i32 - 1);
        // Into the next loop, the extra floors are added on top.
        assert_eq!(
            orc(last_floor + 1).boost,
            (last_floor + LOOP_EXTRA_FLOORS) as i32
        );
    }

    #[test]
    fn the_deep_ramp_starts_after_the_second_crypts() {
        let boost = |depth| Monster::at_depth(Kind::Orc, Point::default(), Ai::Asleep, depth).boost;
        let loop_2 = loop_extra_floors(1) as i32;
        assert_eq!(boost(DEEP_RAMP_FROM), DEEP_RAMP_FROM as i32 - 1 + loop_2);
        // Each floor past the start is a step of depth plus a step of ramp.
        let past = DEEP_RAMP_FROM + 5;
        assert_eq!(
            boost(past),
            past as i32 - 1 + loop_2 + 5 * DEEP_EXTRA_PER_FLOOR as i32
        );
    }

    #[test]
    fn each_loop_adds_more_than_the_last() {
        assert_eq!(loop_extra_floors(0), 0);
        assert_eq!(loop_extra_floors(1), LOOP_EXTRA_FLOORS);
        let steps: Vec<u32> = (1..5)
            .map(|k| loop_extra_floors(k) - loop_extra_floors(k - 1))
            .collect();
        assert!(
            steps.windows(2).all(|w| w[1] == w[0] + LOOP_GROWTH),
            "{steps:?}"
        );
    }

    #[test]
    fn home_monsters_are_more_common() {
        let mut rng = Rng::new(21);
        let level = generate(&mut rng, &STANDARD);
        let skeletons = |home: &[Kind], rng: &mut Rng| {
            (0..60)
                .flat_map(|_| spawn_for_floor(rng, &level, 8, home))
                .filter(|m| m.kind == Kind::Skeleton)
                .count()
        };
        let away = skeletons(&[], &mut rng);
        let at_home = skeletons(&[Kind::Skeleton], &mut rng);
        assert!(at_home > away * 2, "{at_home} at home vs {away} away");
    }

    #[test]
    fn packs_arrive_together() {
        let mut rng = Rng::new(12);
        let level = generate(&mut rng, &STANDARD);
        let packs = (0..40)
            .map(|_| spawn_for_floor(&mut rng, &level, 6, &[]))
            .filter(|ms| ms.iter().filter(|m| m.kind == Kind::Orc).count() >= 2)
            .count();
        assert!(packs > 0, "orcs never came in a group");
    }

    /// Every kind needs its own letter, or the map becomes ambiguous.
    #[test]
    fn every_monster_has_a_unique_glyph() {
        for (i, a) in Kind::ALL.iter().enumerate() {
            for b in &Kind::ALL[i + 1..] {
                assert_ne!(a.species().glyph, b.species().glyph, "{a:?} and {b:?}");
            }
        }
    }

    #[test]
    fn shallow_floors_only_have_shallow_monsters() {
        let mut rng = Rng::new(5);
        let level = generate(&mut rng, &STANDARD);
        for m in spawn_for_floor(&mut rng, &level, 1, &[]) {
            assert_eq!(m.species().min_depth, 1);
        }
    }
}
