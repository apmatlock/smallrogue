//! Zones: stretches of the dungeon with their own look and layout.
//!
//! Three zones of `FLOORS_PER_ZONE` floors each make one loop. After
//! the last zone the dungeon starts over at the first, one loop deeper:
//! depth scaling keeps making monsters stronger, and more of each
//! zone's deep monsters have unlocked by then.

use crate::dungeon::FloorParams;
use crate::frame::Rgb;
use crate::item::ItemWeights;
use crate::monster::Kind;

/// Floors in each zone.
pub const FLOORS_PER_ZONE: u32 = 6;

pub struct Zone {
    pub name: &'static str,
    pub floor: FloorParams,
    /// Monsters at home here, which are more common (see
    /// `monster::HOME_WEIGHT`). Those in `EVERYWHERE` are at home in
    /// every zone.
    pub home: &'static [Kind],
    pub items: ItemWeights,
    pub wall_fg: Rgb,
    pub wall_bg: Rgb,
    pub floor_fg: Rgb,
    pub door_fg: Rgb,
}

/// Vermin at home in every zone.
pub const EVERYWHERE: [Kind; 4] = [Kind::Rat, Kind::Newt, Kind::Jackal, Kind::GiantBat];

/// The zones, in the order they're met.
pub const ZONES: [Zone; 3] = [
    // Tight: small rooms, doors everywhere, few ways round.
    Zone {
        name: "Crypts",
        floor: FloorParams {
            width: 70,
            height: 30,
            max_rooms: 14,
            room_w: (4, 9),
            room_h: (3, 6),
            loop_percent: 10,
            door_percent: 90,
        },
        // The dead, and what guards them.
        home: &[
            Kind::Zombie,
            Kind::Skeleton,
            Kind::Draugr,
            Kind::Ghoul,
            Kind::Gargoyle,
            Kind::StoneGolem,
            Kind::Wraith,
            Kind::Vampire,
            Kind::Demon,
        ],
        // Old libraries: more scrolls.
        items: ItemWeights {
            potion: 33,
            scroll: 40,
            weapon: 10,
            armor: 10,
            ring: 7,
            ration_percent: 25,
        },
        wall_fg: Rgb(125, 115, 135),
        wall_bg: Rgb(30, 26, 34),
        floor_fg: Rgb(100, 95, 110),
        door_fg: Rgb(150, 110, 80),
    },
    // Open: big halls, few doors, many ways round.
    Zone {
        name: "Flooded Halls",
        floor: FloorParams {
            width: 80,
            height: 36,
            max_rooms: 9,
            room_w: (8, 16),
            room_h: (5, 10),
            loop_percent: 45,
            door_percent: 40,
        },
        // Things that crawl, swarm, ooze and swim.
        home: &[
            Kind::GiantAnt,
            Kind::Harpy,
            Kind::AcidMound,
            Kind::GiantSpider,
            Kind::PinkJelly,
            Kind::Troll,
            Kind::FrostGiant,
        ],
        // Flasks wash up here: more potions.
        items: ItemWeights {
            potion: 45,
            scroll: 28,
            weapon: 10,
            armor: 10,
            ring: 7,
            ration_percent: 25,
        },
        wall_fg: Rgb(90, 125, 130),
        wall_bg: Rgb(18, 32, 36),
        floor_fg: Rgb(80, 110, 115),
        door_fg: Rgb(130, 115, 75),
    },
    // Sprawling: a bigger map of small rooms and long corridors.
    Zone {
        name: "Deep Warrens",
        floor: FloorParams {
            width: 100,
            height: 40,
            max_rooms: 18,
            room_w: (4, 10),
            room_h: (3, 7),
            loop_percent: 35,
            door_percent: 60,
        },
        // Tribes, beasts and burrowers.
        home: &[
            Kind::Kobold,
            Kind::Goblin,
            Kind::Redcap,
            Kind::Monkey,
            Kind::Orc,
            Kind::Hobgoblin,
            Kind::Ogre,
            Kind::Owlbear,
            Kind::Bulette,
            Kind::Oni,
            Kind::Dragon,
        ],
        // Stolen gear and stores: more weapons, armor and food.
        items: ItemWeights {
            potion: 33,
            scroll: 26,
            weapon: 16,
            armor: 16,
            ring: 9,
            ration_percent: 40,
        },
        wall_fg: Rgb(150, 115, 70),
        wall_bg: Rgb(38, 28, 18),
        floor_fg: Rgb(120, 100, 70),
        door_fg: Rgb(165, 110, 55),
    },
];

/// Where a depth falls in the cycle of zones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    /// Index into `ZONES`.
    pub zone: usize,
    /// Which pass through the zones, from 1.
    pub loop_number: u32,
    /// The floor within the zone, from 1.
    pub floor: u32,
}

impl Place {
    pub fn at_depth(depth: u32) -> Self {
        let i = depth.max(1) - 1;
        let zones = ZONES.len() as u32;
        Self {
            zone: (i / FLOORS_PER_ZONE % zones) as usize,
            loop_number: i / (FLOORS_PER_ZONE * zones) + 1,
            floor: i % FLOORS_PER_ZONE + 1,
        }
    }

    pub fn zone(self) -> &'static Zone {
        &ZONES[self.zone]
    }

    /// The zone's name, with the loop in Roman numerals after the
    /// first: "Crypts", then "Crypts II".
    pub fn title(self) -> String {
        let name = self.zone().name;
        match self.loop_number {
            1 => name.to_string(),
            n => format!("{name} {}", roman(n)),
        }
    }

    /// Said on arriving at a zone's first floor, `None` elsewhere.
    pub fn welcome(self) -> Option<String> {
        if self.floor != 1 {
            return None;
        }
        let name = self.zone().name;
        Some(match self.loop_number {
            1 => format!("You enter the {name}."),
            _ => format!("You enter the {name} again. Older, deadlier things stir here now."),
        })
    }
}

/// A number in Roman numerals.
fn roman(mut n: u32) -> String {
    const DIGITS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (value, digits) in DIGITS {
        while n >= value {
            out.push_str(digits);
            n -= value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depths_map_to_zones_and_loops() {
        let at = |d| {
            let p = Place::at_depth(d);
            (p.zone().name, p.loop_number, p.floor)
        };
        assert_eq!(at(1), ("Crypts", 1, 1));
        assert_eq!(at(6), ("Crypts", 1, 6));
        assert_eq!(at(7), ("Flooded Halls", 1, 1));
        assert_eq!(at(18), ("Deep Warrens", 1, 6));
        assert_eq!(at(19), ("Crypts", 2, 1));
        assert_eq!(at(37), ("Crypts", 3, 1));
    }

    #[test]
    fn only_a_zones_first_floor_welcomes() {
        assert_eq!(
            Place::at_depth(7).welcome().as_deref(),
            Some("You enter the Flooded Halls.")
        );
        assert_eq!(Place::at_depth(8).welcome(), None);
        assert!(Place::at_depth(19).welcome().unwrap().contains("again"));
    }

    #[test]
    fn later_loops_are_numbered() {
        assert_eq!(Place::at_depth(5).title(), "Crypts");
        assert_eq!(Place::at_depth(20).title(), "Crypts II");
        assert_eq!(Place::at_depth(36 + 13).title(), "Deep Warrens III");
        assert_eq!(roman(1994), "MCMXCIV");
    }

    /// Every monster lives somewhere, and only in one place.
    #[test]
    fn every_monster_has_one_home() {
        for kind in Kind::ALL {
            let homes = ZONES.iter().filter(|z| z.home.contains(&kind)).count()
                + usize::from(EVERYWHERE.contains(&kind));
            assert_eq!(homes, 1, "{kind:?}");
        }
    }

    #[test]
    fn every_zone_builds_floors() {
        use crate::dungeon::generate;
        use crate::rng::Rng;
        for zone in &ZONES {
            for seed in 0..30 {
                let level = generate(&mut Rng::new(seed), &zone.floor);
                assert_eq!(level.map.width(), zone.floor.width);
                assert!(level.rooms.len() >= 2, "{} seed {seed}", zone.name);
            }
        }
    }
}
