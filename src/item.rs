//! Item kinds, their data, and placing items on a new floor.
//!
//! Like monsters, every kind of item is described by a table entry, so
//! adding one means a new enum variant and one entry.

use crate::dungeon::{self, Level};
use crate::geom::Point;
use crate::rng::Rng;

/// A cursed item stays stuck for this many turns per point below zero.
pub const CURSE_TURNS_PER_POINT: u32 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeaponKind {
    Dagger,
    Sword,
    Mace,
    Axe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ArmorKind {
    Leather,
    Chain,
    Plate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PotionKind {
    Healing,
    Strength,
    Life,
    Decay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScrollKind {
    Teleportation,
    MagicMapping,
    Enchanting,
    Aggravate,
    Identify,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FoodKind {
    Ration,
    Jerky,
}

impl FoodKind {
    /// Name as a single item, e.g. "ration of food".
    pub fn name(self) -> &'static str {
        match self {
            FoodKind::Ration => "ration of food",
            FoodKind::Jerky => "strip of jerky",
        }
    }

    /// Name for several, e.g. "rations of food".
    pub fn plural(self) -> &'static str {
        match self {
            FoodKind::Ration => "rations of food",
            FoodKind::Jerky => "strips of jerky",
        }
    }

    /// How much hunger it satisfies.
    pub fn nutrition(self) -> i32 {
        match self {
            FoodKind::Ration => 1_800,
            FoodKind::Jerky => 600,
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            FoodKind::Ration => "A dense block of dried food. Fills you up.",
            FoodKind::Jerky => "Tough, salty meat. Takes the edge off hunger.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RingKind {
    Regeneration,
    Accuracy,
    Protection,
    Awareness,
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

/// Name, description and rarity for potions, scrolls and rings.
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
    pub const ALL: [ScrollKind; 5] = [
        Self::Teleportation,
        Self::MagicMapping,
        Self::Enchanting,
        Self::Aggravate,
        Self::Identify,
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
            Self::Identify => &MagicStats {
                name: "identify",
                about: "Reveals the true nature of one item.",
                weight: 18,
            },
        }
    }
}

impl RingKind {
    pub const ALL: [RingKind; 4] = [
        Self::Regeneration,
        Self::Accuracy,
        Self::Protection,
        Self::Awareness,
    ];

    pub fn stats(self) -> &'static MagicStats {
        match self {
            Self::Regeneration => &MagicStats {
                name: "regeneration",
                about: "Heals your wounds faster.",
                weight: 1,
            },
            Self::Accuracy => &MagicStats {
                name: "accuracy",
                about: "Guides your blows: +5% to hit per point.",
                weight: 1,
            },
            Self::Protection => &MagicStats {
                name: "protection",
                about: "Wards off harm: +1 armor per point.",
                weight: 1,
            },
            Self::Awareness => &MagicStats {
                name: "awareness",
                about: "Sharpens your senses: +1 sight radius per point.",
                weight: 1,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Weapon(WeaponKind),
    Armor(ArmorKind),
    Potion(PotionKind),
    Scroll(ScrollKind),
    Ring(RingKind),
    Food(FoodKind),
}

impl ItemKind {
    pub fn glyph(self) -> char {
        match self {
            Self::Weapon(_) => ')',
            Self::Armor(_) => '[',
            Self::Potion(_) => '!',
            Self::Scroll(_) => '?',
            Self::Ring(_) => '=',
            Self::Food(_) => '%',
        }
    }

    /// Potions, scrolls and food of the same kind share one slot.
    pub fn stacks(self) -> bool {
        matches!(self, Self::Potion(_) | Self::Scroll(_) | Self::Food(_))
    }

    /// Things you wear or wield: they carry an enchantment, can be
    /// cursed, and are identified by wearing them.
    pub fn is_equipment(self) -> bool {
        matches!(self, Self::Weapon(_) | Self::Armor(_) | Self::Ring(_))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub kind: ItemKind,
    /// Bonus (or penalty) on equipment, like the +1 in "+1 sword".
    /// Always 0 for potions and scrolls.
    pub enchant: i32,
    /// How many are in this stack. Always 1 for equipment.
    pub count: u32,
    /// Inventory letter. Assigned when picked up; stays the same until
    /// the item leaves the pack, so muscle memory works.
    pub letter: char,
    pub equipped: bool,
    /// Whether the player knows this piece of equipment's enchantment.
    /// (Potions and scrolls are known by kind instead; see `Lore`.)
    pub known: bool,
    /// Turns spent equipped while unknown, counting toward revealing it.
    pub worn_turns: u32,
    /// Turns left before a cursed item can be taken off. Only counts
    /// down while equipped. 0 means not (or no longer) cursed.
    pub curse_turns: u32,
}

impl Item {
    pub fn new(kind: ItemKind) -> Self {
        Self::enchanted(kind, 0)
    }

    /// A new item with the given enchantment. Negative ones are cursed
    /// for 50 turns per point below zero.
    pub fn enchanted(kind: ItemKind, enchant: i32) -> Self {
        Self {
            kind,
            enchant,
            count: 1,
            letter: ' ',
            equipped: false,
            known: false,
            worn_turns: 0,
            curse_turns: CURSE_TURNS_PER_POINT * (-enchant).max(0) as u32,
        }
    }

    /// Equipped and still cursed: it can't be taken off yet.
    pub fn is_stuck(&self) -> bool {
        self.equipped && self.curse_turns > 0
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
/// How often each category of item turns up, as relative weights,
/// plus the chance of a ration on a floor. Zones each supply their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemWeights {
    pub potion: i32,
    pub scroll: i32,
    pub weapon: i32,
    pub armor: i32,
    pub ring: i32,
    /// Chance per floor of a ration, in percent.
    pub ration_percent: i32,
}

impl ItemWeights {
    /// The weights from before zones; tests use them.
    #[cfg(test)]
    pub const STANDARD: Self = Self {
        potion: 38,
        scroll: 33,
        weapon: 11,
        armor: 11,
        ring: 7,
        ration_percent: 25,
    };
}

pub fn random_item(rng: &mut Rng, weights: &ItemWeights) -> Item {
    #[derive(Clone, Copy)]
    enum Category {
        Weapon,
        Armor,
        Potion,
        Scroll,
        Ring,
    }
    let category = weighted(
        rng,
        &[
            (Category::Potion, weights.potion),
            (Category::Scroll, weights.scroll),
            (Category::Weapon, weights.weapon),
            (Category::Armor, weights.armor),
            (Category::Ring, weights.ring),
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
        Category::Ring => {
            let table: Vec<_> = RingKind::ALL
                .iter()
                .map(|&k| (k, k.stats().weight))
                .collect();
            ItemKind::Ring(weighted(rng, &table))
        }
    };
    // About 6% of equipment is cursed: 3% at -1, 2% at -2, 1% at -3.
    // Rings are only useful when enchanted, so they roll higher.
    let enchant = match kind {
        ItemKind::Ring(_) => weighted(rng, &[(1, 50), (2, 30), (3, 14), (-1, 3), (-2, 2), (-3, 1)]),
        k if k.is_equipment() => {
            weighted(rng, &[(0, 70), (1, 20), (2, 4), (-1, 3), (-2, 2), (-3, 1)])
        }
        _ => 0,
    };
    Item::enchanted(kind, enchant)
}

/// Chance per floor of finding jerky. Rations come from the zone's
/// `ItemWeights`. Tuned so careful play never starves but resting
/// forever does.
pub const JERKY_PERCENT: i32 = 35;

/// Scatters a few random items across a new floor, never on the stairs
/// and never two on one tile. Food is rolled separately so its supply
/// stays steady.
pub fn spawn_for_floor(rng: &mut Rng, level: &Level, weights: &ItemWeights) -> Vec<FloorItem> {
    let mut wanted: Vec<Item> = (0..rng.range(3, 6))
        .map(|_| random_item(rng, weights))
        .collect();
    if rng.chance(weights.ration_percent) {
        wanted.push(Item::new(ItemKind::Food(FoodKind::Ration)));
    }
    if rng.chance(JERKY_PERCENT) {
        wanted.push(Item::new(ItemKind::Food(FoodKind::Jerky)));
    }
    let count = wanted.len();
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
            let item = wanted[items.len()].clone();
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
    fn negative_items_are_cursed_for_50_turns_per_point() {
        let sword = ItemKind::Weapon(WeaponKind::Sword);
        assert_eq!(Item::enchanted(sword, 1).curse_turns, 0);
        assert_eq!(Item::enchanted(sword, -1).curse_turns, 50);
        assert_eq!(Item::enchanted(sword, -3).curse_turns, 150);
    }

    #[test]
    fn curses_are_rare_but_happen() {
        let mut rng = Rng::new(9);
        let gear: Vec<Item> = (0..20_000)
            .map(|_| random_item(&mut rng, &ItemWeights::STANDARD))
            .filter(|i| matches!(i.kind, ItemKind::Weapon(_) | ItemKind::Armor(_)))
            .collect();
        let cursed = gear.iter().filter(|i| i.enchant < 0).count();
        let percent = cursed * 100 / gear.len();
        assert!((4..=8).contains(&percent), "{percent}% cursed");
        assert!(gear.iter().any(|i| i.enchant == -3));
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
            let item = random_item(&mut rng, &ItemWeights::STANDARD);
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
            let items = spawn_for_floor(&mut rng, &level, &ItemWeights::STANDARD);
            assert!(!items.is_empty());
            for (i, fi) in items.iter().enumerate() {
                assert_eq!(level.map.tile(fi.pos), crate::map::Tile::Floor);
                assert!(items[i + 1..].iter().all(|o| o.pos != fi.pos));
            }
        }
    }
}
