//! The player character: position, health, attributes and pack.

use crate::combat::{Attack, Defense};
use crate::geom::Point;
use crate::item::{ArmorKind, FoodKind, Item, ItemKind, PotionKind, RingKind, WeaponKind};
use crate::skills::{Skill, Skills};

/// One slot per letter, a to z.
pub const PACK_SIZE: usize = 26;

/// How many rings can be worn at once: one per hand.
pub const RING_SLOTS: usize = 2;

/// How far the player sees without a ring of awareness.
pub const BASE_SIGHT: i32 = 8;

/// Food: each turn uses 1. Eating adds the food's nutrition, up to the
/// maximum.
pub const FOOD_MAX: i32 = 2_100;
pub const FOOD_START: i32 = 1_800;
/// At or below these levels the player is hungry, then weak.
pub const HUNGRY_AT: i32 = 300;
pub const WEAK_AT: i32 = 150;
/// Accuracy lost while weak from hunger.
pub const WEAK_ACCURACY_PENALTY: i32 = 2;

/// How hungry the player is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Hunger {
    Fed,
    /// Just a warning.
    Hungry,
    /// No healing, and less accurate.
    Weak,
    /// Losing health until something is eaten.
    Starving,
}

impl Hunger {
    /// The word shown on screen, or "" when fed.
    pub fn label(self) -> &'static str {
        match self {
            Hunger::Fed => "",
            Hunger::Hungry => "Hungry",
            Hunger::Weak => "Weak",
            Hunger::Starving => "Starving",
        }
    }
}

/// Healing builds up by this much each turn; every 100 heals 1 health,
/// so about 1 health every 8 turns.
pub const BASE_REGEN: i32 = 12;

#[derive(Clone, Debug)]
pub struct Player {
    pub pos: Point,
    pub hp: i32,
    pub max_hp: i32,
    /// Adds melee damage. Will also set carrying capacity.
    pub strength: i32,
    /// Sets accuracy and dodge.
    pub agility: i32,
    /// Will speed up identifying items, and later power magic.
    pub intellect: i32,
    /// Carried items, each with its own letter. Equipped items stay in
    /// the pack, marked `equipped`.
    pub inventory: Vec<Item>,
    /// Healing built up toward the next point of health.
    pub regen_progress: i32,
    /// Character level, raised by experience from kills.
    pub level: u32,
    /// Total experience earned this run.
    pub xp: u32,
    pub skills: Skills,
    /// Nutrition left; see `FOOD_MAX`.
    pub food: i32,
}

impl Player {
    /// The one starting background for now: a sturdy melee fighter
    /// with a sword, leather armor and one healing potion.
    pub fn fighter(pos: Point) -> Self {
        let mut player = Self {
            pos,
            hp: 25,
            max_hp: 25,
            strength: 4,
            agility: 3,
            intellect: 2,
            inventory: Vec::new(),
            regen_progress: 0,
            level: 1,
            xp: 0,
            skills: Skills::default(),
            food: FOOD_START,
        };
        let kit = [
            ItemKind::Weapon(WeaponKind::Sword),
            ItemKind::Armor(ArmorKind::Leather),
            ItemKind::Potion(PotionKind::Healing),
            ItemKind::Food(FoodKind::Ration),
        ];
        for kind in kit {
            let mut item = Item::new(kind);
            item.equipped = kind.is_equipment();
            item.known = true; // you know your own gear
            player.add_item(item).expect("an empty pack has room");
        }
        player
    }

    /// Bonus melee damage from strength: +1 at 4, +2 at 6, and so on.
    pub fn strength_bonus(&self) -> i32 {
        (self.strength / 2 - 1).max(0)
    }

    /// The equipped weapon, if any.
    pub fn weapon(&self) -> Option<&Item> {
        self.inventory
            .iter()
            .find(|i| i.equipped && matches!(i.kind, ItemKind::Weapon(_)))
    }

    /// The equipped armor, if any.
    pub fn armor(&self) -> Option<&Item> {
        self.inventory
            .iter()
            .find(|i| i.equipped && matches!(i.kind, ItemKind::Armor(_)))
    }

    /// Equipped rings.
    pub fn rings(&self) -> impl Iterator<Item = &Item> {
        self.inventory
            .iter()
            .filter(|i| i.equipped && matches!(i.kind, ItemKind::Ring(_)))
    }

    /// The total enchantment of worn rings of one kind. Two +1 rings of
    /// accuracy add up to +2.
    pub fn ring_bonus(&self, kind: RingKind) -> i32 {
        self.rings()
            .filter(|i| i.kind == ItemKind::Ring(kind))
            .map(|i| i.enchant)
            .sum()
    }

    pub fn sight_radius(&self) -> i32 {
        (BASE_SIGHT + self.ring_bonus(RingKind::Awareness)).clamp(3, 14)
    }

    /// Healing gained per turn, out of 100 per point of health.
    pub fn regen_rate(&self) -> i32 {
        (BASE_REGEN + 6 * self.ring_bonus(RingKind::Regeneration)).max(2)
    }

    pub fn hunger(&self) -> Hunger {
        match self.food {
            f if f <= 0 => Hunger::Starving,
            f if f <= WEAK_AT => Hunger::Weak,
            f if f <= HUNGRY_AT => Hunger::Hungry,
            _ => Hunger::Fed,
        }
    }

    pub fn skill(&self, skill: Skill) -> i32 {
        self.skills.level(skill) as i32
    }

    pub fn attack(&self) -> Attack {
        // Melee skill: +1 accuracy per level, +1 damage per 2 levels.
        let melee = self.skill(Skill::Melee);
        let bonus = self.strength_bonus() + melee / 2;
        let (damage, accuracy, enchant) = match self.weapon() {
            Some(Item {
                kind: ItemKind::Weapon(w),
                enchant,
                ..
            }) => (w.stats().damage, w.stats().accuracy, *enchant),
            _ => ((1, 2), 0, 0), // bare fists
        };
        // Enchantment adds to damage and accuracy alike. A cursed -1
        // blade can't drop damage below 1.
        let min = (damage.0 + bonus + enchant).max(1);
        let max = (damage.1 + bonus + enchant).max(min);
        Attack {
            accuracy: 1 - self.weak_penalty()
                + self.agility
                + accuracy
                + enchant
                + melee
                + self.ring_bonus(RingKind::Accuracy),
            damage: (min, max),
        }
    }

    fn weak_penalty(&self) -> i32 {
        if self.hunger() >= Hunger::Weak {
            WEAK_ACCURACY_PENALTY
        } else {
            0
        }
    }

    pub fn defense(&self) -> Defense {
        let protection = self.ring_bonus(RingKind::Protection);
        let dodge = self.agility + self.skill(Skill::Dodge);
        let armor_skill = self.skill(Skill::Armor);
        let base = match self.armor() {
            Some(Item {
                kind: ItemKind::Armor(a),
                enchant,
                ..
            }) => Defense {
                // Armor skill: each level takes 1 off heavy armor's
                // dodge penalty (never past zero), and every 2 levels
                // add 1 armor.
                dodge: dodge + (a.stats().dodge + armor_skill).min(0),
                armor: (a.stats().armor + enchant + armor_skill / 2).max(0),
            },
            _ => Defense { dodge, armor: 0 },
        };
        Defense {
            armor: (base.armor + protection).max(0),
            ..base
        }
    }

    // ---- Pack -------------------------------------------------------

    /// Adds an item to the pack, merging it into an existing stack when
    /// it can. Returns the letter it went under, or gives the item back
    /// if the pack is full.
    pub fn add_item(&mut self, mut item: Item) -> Result<char, Item> {
        if item.kind.stacks()
            && let Some(stack) = self.inventory.iter_mut().find(|i| i.kind == item.kind)
        {
            stack.count += item.count;
            return Ok(stack.letter);
        }
        let Some(letter) = ('a'..='z').find(|&c| self.item(c).is_none()) else {
            return Err(item);
        };
        item.letter = letter;
        self.inventory.push(item);
        // Keep the pack in letter order, so lists display neatly.
        self.inventory.sort_by_key(|i| i.letter);
        Ok(letter)
    }

    pub fn item(&self, letter: char) -> Option<&Item> {
        self.inventory.iter().find(|i| i.letter == letter)
    }

    pub fn item_mut(&mut self, letter: char) -> Option<&mut Item> {
        self.inventory.iter_mut().find(|i| i.letter == letter)
    }

    /// Takes a whole stack out of the pack, unequipping it.
    pub fn remove_item(&mut self, letter: char) -> Option<Item> {
        let index = self.inventory.iter().position(|i| i.letter == letter)?;
        let mut item = self.inventory.remove(index);
        item.equipped = false;
        Some(item)
    }

    /// Takes one item off a stack (a single potion, say), removing the
    /// stack once it runs out.
    pub fn take_one(&mut self, letter: char) -> Option<Item> {
        let stack = self.item_mut(letter)?;
        if stack.count > 1 {
            stack.count -= 1;
            let mut one = stack.clone();
            one.count = 1;
            return Some(one);
        }
        self.remove_item(letter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn potion() -> Item {
        Item::new(ItemKind::Potion(PotionKind::Healing))
    }

    #[test]
    fn fighter_starts_equipped() {
        let p = Player::fighter(Point::default());
        let kind = |i: Option<&Item>| i.map(|i| (i.kind, i.enchant, i.known));
        let sword = ItemKind::Weapon(WeaponKind::Sword);
        let leather = ItemKind::Armor(ArmorKind::Leather);
        assert_eq!(kind(p.weapon()), Some((sword, 0, true)));
        assert_eq!(kind(p.armor()), Some((leather, 0, true)));
        // Same numbers the built-in sword and armor had before items.
        assert_eq!(p.attack().damage, (2, 6));
        assert_eq!(p.defense().armor, 1);
    }

    #[test]
    fn potions_stack_under_one_letter() {
        let mut p = Player::fighter(Point::default());
        let letter = p.add_item(potion()).unwrap();
        assert_eq!(letter, 'c'); // merged into the starting potion
        assert_eq!(p.item('c').unwrap().count, 2);
        assert_eq!(p.take_one('c').unwrap().count, 1);
        assert_eq!(p.item('c').unwrap().count, 1);
        p.take_one('c');
        assert!(p.item('c').is_none());
    }

    #[test]
    fn letters_stay_put_when_items_leave() {
        let mut p = Player::fighter(Point::default());
        let dagger = Item::new(ItemKind::Weapon(WeaponKind::Dagger));
        // a-d hold the starting kit.
        assert_eq!(p.add_item(dagger.clone()), Ok('e'));
        p.remove_item('b');
        assert!(p.item('e').is_some(), "e keeps its letter");
        assert_eq!(p.add_item(dagger), Ok('b'), "the gap is reused");
    }

    #[test]
    fn a_full_pack_refuses_items() {
        let mut p = Player::fighter(Point::default());
        let axe = Item::new(ItemKind::Weapon(WeaponKind::Axe));
        while p.inventory.len() < PACK_SIZE {
            p.add_item(axe.clone()).unwrap();
        }
        assert_eq!(p.add_item(axe.clone()), Err(axe));
    }

    #[test]
    fn enchantment_and_heavy_gear_change_stats() {
        let mut p = Player::fighter(Point::default());
        p.item_mut('a').unwrap().enchant = 2;
        assert_eq!(p.attack().damage, (4, 8));
        assert_eq!(p.attack().accuracy, 1 + 3 + 2);

        let plate = Item::new(ItemKind::Armor(ArmorKind::Plate));
        let letter = p.add_item(plate).unwrap();
        p.item_mut('b').unwrap().equipped = false;
        p.item_mut(letter).unwrap().equipped = true;
        assert_eq!(p.defense().armor, 5);
        assert_eq!(p.defense().dodge, 0);
    }

    #[test]
    fn rings_add_up() {
        let mut p = Player::fighter(Point::default());
        for enchant in [2, 1] {
            let mut ring = Item::enchanted(ItemKind::Ring(RingKind::Accuracy), enchant);
            ring.equipped = true;
            p.add_item(ring).unwrap();
        }
        assert_eq!(p.ring_bonus(RingKind::Accuracy), 3);
        assert_eq!(p.attack().accuracy, 1 + 3 + 3);

        let mut ring = Item::enchanted(ItemKind::Ring(RingKind::Awareness), 2);
        ring.equipped = true;
        p.add_item(ring).unwrap();
        assert_eq!(p.sight_radius(), BASE_SIGHT + 2);
    }

    #[test]
    fn protection_and_regeneration_rings() {
        let mut p = Player::fighter(Point::default());
        let base_armor = p.defense().armor;
        let base_regen = p.regen_rate();
        for kind in [RingKind::Protection, RingKind::Regeneration] {
            let mut ring = Item::enchanted(ItemKind::Ring(kind), 2);
            ring.equipped = true;
            p.add_item(ring).unwrap();
        }
        assert_eq!(p.defense().armor, base_armor + 2);
        assert_eq!(p.regen_rate(), base_regen + 12);
    }

    #[test]
    fn skills_improve_combat() {
        let mut p = Player::fighter(Point::default());
        let (attack, defense) = (p.attack(), p.defense());
        p.skills.train(Skill::Melee, 10 + 20); // level 2
        p.skills.train(Skill::Dodge, 10); // level 1
        assert_eq!(p.attack().accuracy, attack.accuracy + 2);
        assert_eq!(
            p.attack().damage,
            (attack.damage.0 + 1, attack.damage.1 + 1)
        );
        assert_eq!(p.defense().dodge, defense.dodge + 1);
    }

    #[test]
    fn armor_skill_softens_heavy_armor() {
        let mut p = Player::fighter(Point::default());
        let plate = Item::new(ItemKind::Armor(ArmorKind::Plate));
        let letter = p.add_item(plate).unwrap();
        p.item_mut('b').unwrap().equipped = false;
        p.item_mut(letter).unwrap().equipped = true;
        assert_eq!(p.defense().dodge, 3 - 3);
        p.skills.train(Skill::Armor, 10 + 20); // level 2
        assert_eq!(p.defense().dodge, 3 - 1);
        assert_eq!(p.defense().armor, 5 + 1);
        p.skills.train(Skill::Armor, 30 + 40 + 50); // level 5
        assert_eq!(p.defense().dodge, 3, "the penalty never turns into a bonus");
    }

    #[test]
    fn hunger_stages() {
        let mut p = Player::fighter(Point::default());
        assert_eq!(p.hunger(), Hunger::Fed);
        let accuracy = p.attack().accuracy;
        p.food = HUNGRY_AT;
        assert_eq!(p.hunger(), Hunger::Hungry);
        p.food = WEAK_AT;
        assert_eq!(p.hunger(), Hunger::Weak);
        assert_eq!(p.attack().accuracy, accuracy - WEAK_ACCURACY_PENALTY);
        p.food = 0;
        assert_eq!(p.hunger(), Hunger::Starving);
    }

    #[test]
    fn bare_hands_still_hurt() {
        let mut p = Player::fighter(Point::default());
        p.remove_item('a');
        assert_eq!(p.attack().damage, (2, 3));
    }
}
