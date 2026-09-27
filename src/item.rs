//! Item kinds, their data, and placing items on a new floor.
//!
//! Like monsters, every kind of item is described by a table entry, so
//! adding one means a new enum variant and one entry.

use crate::dungeon::{self, Level};
use crate::frame::Rgb;
use crate::geom::Point;
use crate::rng::Rng;
use crate::text::article;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponKind {
    Dagger,
    Sword,
    Mace,
    Axe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmorKind {
    Leather,
    Chain,
    Plate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PotionKind {
    Healing,
    Strength,
    Life,
    Decay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollKind {
    Teleportation,
    MagicMapping,
    Enchanting,
    Aggravate,
}

pub struct WeaponStats {
    pub name: &'static str,
    /// Damage range, inclusive: (min, max).
    pub damage: (i32, i32),
    /// Added to the wielder's accuracy. Heavy weapons are clumsier.
    pub accuracy: i32,
    pub about: &'static str,
    /// Relative chance of appearing, compared with other weapons.
    pub weight: i32,
}

pub struct ArmorStats {
    pub name: &'static str,
    pub armor: i32,
    /// Added to the wearer's dodge. Heavy armor is harder to move in.
    pub dodge: i32,
    pub about: &'static str,
    pub weight: i32,
}

/// Name, description and rarity for potions and scrolls.
pub struct MagicStats {
    pub name: &'static str,
    pub about: &'static str,
    pub weight: i32,
}

impl WeaponKind {
    pub const ALL: [WeaponKind; 4] = [Self::Dagger, Self::Sword, Self::Mace, Self::Axe];

    pub fn stats(self) -> &'static WeaponStats {
        match self {
            Self::Dagger => &WeaponStats {
                name: "dagger",
                damage: (1, 4),
                accuracy: 2,
                about: "Light and quick, it rarely misses.",
                weight: 3,
            },
            Self::Sword => &WeaponStats {
                name: "sword",
                damage: (1, 5),
                accuracy: 0,
                about: "A plain, dependable blade.",
                weight: 3,
            },
            Self::Mace => &WeaponStats {
                name: "mace",
                damage: (2, 7),
                accuracy: -1,
                about: "A heavy flanged head on an iron haft.",
                weight: 2,
            },
            Self::Axe => &WeaponStats {
                name: "battle axe",
                damage: (3, 9),
                accuracy: -3,
                about: "Brutal when it lands, but slow to swing.",
                weight: 1,
            },
        }
    }
}

impl ArmorKind {
    pub const ALL: [ArmorKind; 3] = [Self::Leather, Self::Chain, Self::Plate];

    pub fn stats(self) -> &'static ArmorStats {
        match self {
            Self::Leather => &ArmorStats {
                name: "leather armor",
                armor: 1,
                dodge: 0,
                about: "Supple hide that doesn't slow you down.",
                weight: 3,
            },
            Self::Chain => &ArmorStats {
                name: "chain mail",
                armor: 3,
                dodge: -1,
                about: "Iron rings that turn aside most blades.",
                weight: 2,
            },
            Self::Plate => &ArmorStats {
                name: "plate armor",
                armor: 5,
                dodge: -3,
                about: "Nearly impenetrable, and very hard to move in.",
                weight: 1,
            },
        }
    }
}

impl PotionKind {
    pub const ALL: [PotionKind; 4] = [Self::Healing, Self::Strength, Self::Life, Self::Decay];

    pub fn stats(self) -> &'static MagicStats {
        match self {
            Self::Healing => &MagicStats {
                name: "healing",
                about: "Heals 15 health.",
                weight: 30,
            },
            Self::Strength => &MagicStats {
                name: "strength",
                about: "Permanently raises strength by 1.",
                weight: 8,
            },
            Self::Life => &MagicStats {
                name: "life",
                about: "Raises maximum health by 5 and heals you fully.",
                weight: 3,
            },
            Self::Decay => &MagicStats {
                name: "decay",
                about: "A foul brew that eats at the flesh.",
                weight: 12,
            },
        }
    }
}

impl ScrollKind {
    pub const ALL: [ScrollKind; 4] = [
        Self::Teleportation,
        Self::MagicMapping,
        Self::Enchanting,
        Self::Aggravate,
    ];

    pub fn stats(self) -> &'static MagicStats {
        match self {
            Self::Teleportation => &MagicStats {
                name: "teleportation",
                about: "Carries you to a distant part of the floor.",
                weight: 15,
            },
            Self::MagicMapping => &MagicStats {
                name: "magic mapping",
                about: "Reveals the layout of the whole floor.",
                weight: 12,
            },
            Self::Enchanting => &MagicStats {
                name: "enchanting",
                about: "Adds +1 to a weapon or armor of your choice.",
                weight: 15,
            },
            Self::Aggravate => &MagicStats {
                name: "aggravate monsters",
                about: "Lets out a shriek that alerts the whole floor.",
                weight: 8,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Weapon(WeaponKind),
    Armor(ArmorKind),
    Potion(PotionKind),
    Scroll(ScrollKind),
}

impl ItemKind {
    pub fn glyph(self) -> char {
        match self {
            Self::Weapon(_) => ')',
            Self::Armor(_) => '[',
            Self::Potion(_) => '!',
            Self::Scroll(_) => '?',
        }
    }

    pub fn color(self) -> Rgb {
        match self {
            Self::Weapon(_) => Rgb(170, 175, 195),
            Self::Armor(_) => Rgb(160, 135, 100),
            Self::Potion(PotionKind::Healing) => Rgb(215, 80, 90),
            Self::Potion(PotionKind::Strength) => Rgb(215, 145, 60),
            Self::Potion(PotionKind::Life) => Rgb(235, 215, 120),
            Self::Potion(PotionKind::Decay) => Rgb(115, 170, 80),
            Self::Scroll(_) => Rgb(225, 215, 185),
        }
    }

    /// Potions and scrolls of the same kind share one inventory slot.
    pub fn stacks(self) -> bool {
        matches!(self, Self::Potion(_) | Self::Scroll(_))
    }

    pub fn is_equipment(self) -> bool {
        matches!(self, Self::Weapon(_) | Self::Armor(_))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub kind: ItemKind,
    /// Bonus (or penalty) on weapons and armor, like the +1 in "+1
    /// sword". Always 0 for potions and scrolls.
    pub enchant: i32,
    /// How many are in this stack. Always 1 for equipment.
    pub count: u32,
    /// Inventory letter. Assigned when picked up; stays the same until
    /// the item leaves the pack, so muscle memory works.
    pub letter: char,
    pub equipped: bool,
}

impl Item {
    pub fn new(kind: ItemKind) -> Self {
        Self {
            kind,
            enchant: 0,
            count: 1,
            letter: ' ',
            equipped: false,
        }
    }

    /// The item's name without an article, e.g. "+1 sword" or
    /// "3 potions of healing".
    pub fn name(&self) -> String {
        let enchant = match self.enchant {
            0 => String::new(),
            e => format!("{e:+} "),
        };
        let (noun, what) = match self.kind {
            ItemKind::Weapon(w) => return format!("{enchant}{}", w.stats().name),
            ItemKind::Armor(a) => return format!("{enchant}{}", a.stats().name),
            ItemKind::Potion(p) => ("potion", p.stats().name),
            ItemKind::Scroll(s) => ("scroll", s.stats().name),
        };
        if self.count > 1 {
            format!("{} {noun}s of {what}", self.count)
        } else {
            format!("{noun} of {what}")
        }
    }

    /// The name with "a"/"an" in front when there is just one. Armor
    /// names like "chain mail" read better without one.
    pub fn with_article(&self) -> String {
        let name = self.name();
        if self.count > 1 || matches!(self.kind, ItemKind::Armor(_)) {
            name
        } else {
            format!("{} {name}", article(&name))
        }
    }

    /// Lines describing the item, for the inventory screen.
    pub fn describe(&self) -> Vec<String> {
        let e = self.enchant;
        match self.kind {
            ItemKind::Weapon(w) => {
                let s = w.stats();
                vec![
                    s.about.to_string(),
                    format!(
                        "Damage {}-{}, accuracy {:+}.",
                        (s.damage.0 + e).max(1),
                        (s.damage.1 + e).max(1),
                        s.accuracy + e
                    ),
                ]
            }
            ItemKind::Armor(a) => {
                let s = a.stats();
                vec![
                    s.about.to_string(),
                    format!("Armor {}, dodge {:+}.", (s.armor + e).max(0), s.dodge),
                ]
            }
            ItemKind::Potion(p) => vec![p.stats().about.to_string()],
            ItemKind::Scroll(s) => vec![s.stats().about.to_string()],
        }
    }
}

/// An item lying on the dungeon floor.
#[derive(Clone, Debug)]
pub struct FloorItem {
    pub pos: Point,
    pub item: Item,
}

/// Picks one entry from `(thing, weight)` pairs, where a higher weight
/// makes a thing more likely.
fn weighted<T: Copy>(rng: &mut Rng, choices: &[(T, i32)]) -> T {
    let total: i32 = choices.iter().map(|(_, w)| w).sum();
    let mut roll = rng.range(0, total);
    for &(thing, weight) in choices {
        if roll < weight {
            return thing;
        }
        roll -= weight;
    }
    choices[choices.len() - 1].0
}

/// Rolls a random item: first its category, then its kind, then an
/// enchantment for weapons and armor.
pub fn random_item(rng: &mut Rng) -> Item {
    #[derive(Clone, Copy)]
    enum Category {
        Weapon,
        Armor,
        Potion,
        Scroll,
    }
    let category = weighted(
        rng,
        &[
            (Category::Potion, 40),
            (Category::Scroll, 35),
            (Category::Weapon, 12),
            (Category::Armor, 13),
        ],
    );
    let kind = match category {
        Category::Weapon => {
            let table: Vec<_> = WeaponKind::ALL
                .iter()
                .map(|&k| (k, k.stats().weight))
                .collect();
            ItemKind::Weapon(weighted(rng, &table))
        }
        Category::Armor => {
            let table: Vec<_> = ArmorKind::ALL
                .iter()
                .map(|&k| (k, k.stats().weight))
                .collect();
            ItemKind::Armor(weighted(rng, &table))
        }
        Category::Potion => {
            let table: Vec<_> = PotionKind::ALL
                .iter()
                .map(|&k| (k, k.stats().weight))
                .collect();
            ItemKind::Potion(weighted(rng, &table))
        }
        Category::Scroll => {
            let table: Vec<_> = ScrollKind::ALL
                .iter()
                .map(|&k| (k, k.stats().weight))
                .collect();
            ItemKind::Scroll(weighted(rng, &table))
        }
    };
    let mut item = Item::new(kind);
    if kind.is_equipment() {
        item.enchant = weighted(rng, &[(0, 70), (1, 20), (2, 7), (-1, 3)]);
    }
    item
}

/// Scatters a few random items across a new floor, never on the stairs
/// and never two on one tile.
pub fn spawn_for_floor(rng: &mut Rng, level: &Level) -> Vec<FloorItem> {
    let count = rng.range(3, 6) as usize;
    let mut items: Vec<FloorItem> = Vec::new();
    for _ in 0..count * 3 {
        if items.len() == count {
            break;
        }
        let room = level.rooms[rng.index(level.rooms.len())];
        let pos = dungeon::random_point_in(rng, room);
        let free =
            level.map.tile(pos) == crate::map::Tile::Floor && items.iter().all(|i| i.pos != pos);
        if free {
            let item = random_item(rng);
            items.push(FloorItem { pos, item });
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dungeon::{STANDARD, generate};

    #[test]
    fn names_read_naturally() {
        let mut sword = Item::new(ItemKind::Weapon(WeaponKind::Sword));
        assert_eq!(sword.with_article(), "a sword");
        sword.enchant = 1;
        assert_eq!(sword.with_article(), "a +1 sword");
        sword.enchant = -1;
        assert_eq!(sword.name(), "-1 sword");

        let mut potion = Item::new(ItemKind::Potion(PotionKind::Healing));
        assert_eq!(potion.with_article(), "a potion of healing");
        potion.count = 3;
        assert_eq!(potion.with_article(), "3 potions of healing");

        let armor = Item::new(ItemKind::Armor(ArmorKind::Leather));
        assert_eq!(armor.with_article(), "leather armor");
    }

    #[test]
    fn weighted_choice_follows_weights() {
        let mut rng = Rng::new(3);
        let picks = (0..10_000)
            .filter(|_| weighted(&mut rng, &[('a', 3), ('b', 1)]) == 'a')
            .count();
        assert!((7_200..7_800).contains(&picks), "{picks}");
    }

    #[test]
    fn only_equipment_is_enchanted() {
        let mut rng = Rng::new(4);
        for _ in 0..500 {
            let item = random_item(&mut rng);
            if !item.kind.is_equipment() {
                assert_eq!(item.enchant, 0);
            }
        }
    }

    #[test]
    fn floor_items_sit_on_free_floor() {
        for seed in 0..50 {
            let mut rng = Rng::new(seed);
            let level = generate(&mut rng, &STANDARD);
            let items = spawn_for_floor(&mut rng, &level);
            assert!(!items.is_empty());
            for (i, fi) in items.iter().enumerate() {
                assert_eq!(level.map.tile(fi.pos), crate::map::Tile::Floor);
                assert!(items[i + 1..].iter().all(|o| o.pos != fi.pos));
            }
        }
    }
}
