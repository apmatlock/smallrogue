//! The player character: position, health, attributes and pack.

use crate::combat::{Attack, Defense};
use crate::geom::Point;
use crate::item::{ArmorKind, Item, ItemKind, PotionKind, WeaponKind};

/// One slot per letter, a to z.
pub const PACK_SIZE: usize = 26;

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
        };
        let kit = [
            ItemKind::Weapon(WeaponKind::Sword),
            ItemKind::Armor(ArmorKind::Leather),
            ItemKind::Potion(PotionKind::Healing),
        ];
        for kind in kit {
            let mut item = Item::new(kind);
            item.equipped = kind.is_equipment();
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

    pub fn attack(&self) -> Attack {
        let bonus = self.strength_bonus();
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
            accuracy: 1 + self.agility + accuracy + enchant,
            damage: (min, max),
        }
    }

    pub fn defense(&self) -> Defense {
        match self.armor() {
            Some(Item {
                kind: ItemKind::Armor(a),
                enchant,
                ..
            }) => Defense {
                dodge: self.agility + a.stats().dodge,
                armor: (a.stats().armor + enchant).max(0),
            },
            _ => Defense {
                dodge: self.agility,
                armor: 0,
            },
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
        assert_eq!(p.weapon().map(Item::name), Some("sword".to_string()));
        assert_eq!(p.armor().map(Item::name), Some("leather armor".to_string()));
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
        assert_eq!(p.add_item(dagger.clone()), Ok('d'));
        p.remove_item('b');
        assert!(p.item('d').is_some(), "d keeps its letter");
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
    fn bare_hands_still_hurt() {
        let mut p = Player::fighter(Point::default());
        p.remove_item('a');
        assert_eq!(p.attack().damage, (2, 3));
    }
}
