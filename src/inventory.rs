//! Using items: picking up, dropping, equipping, drinking and reading.
//!
//! Another `impl Game` block, like ai.rs, so game.rs stays about
//! movement and turns.

use crate::game::{Game, MsgKind, Outcome};
use crate::geom::{DIRECTIONS_8, Point};
use crate::item::{FloorItem, Item, ItemKind, PotionKind, ScrollKind};
use crate::lore::Lore;
use crate::monster::Ai;
use crate::player::{FOOD_MAX, RING_SLOTS};

/// Health restored by a potion of healing.
pub(crate) const HEALING: i32 = 15;
/// Maximum health gained from a potion of life.
const LIFE: i32 = 5;
/// Damage range of a potion of decay.
const DECAY_DAMAGE: (i32, i32) = (3, 8);

impl Game {
    pub fn item_at(&self, p: Point) -> Option<&FloorItem> {
        self.items.iter().find(|i| i.pos == p)
    }

    /// Does reading this scroll need the player to choose an item?
    pub fn scroll_needs_target(&self, letter: char) -> bool {
        matches!(
            self.player.item(letter).map(|i| i.kind),
            Some(ItemKind::Scroll(
                ScrollKind::Enchanting | ScrollKind::Identify
            ))
        )
    }

    /// Could `item` be the target of the scroll with letter `scroll`?
    pub fn is_read_target(&self, scroll: char, item: &Item) -> bool {
        match self.player.item(scroll).map(|i| i.kind) {
            Some(ItemKind::Scroll(ScrollKind::Enchanting)) => item.kind.is_equipment(),
            Some(ItemKind::Scroll(ScrollKind::Identify)) => {
                item.letter != scroll && !self.lore.fully_known(item)
            }
            _ => false,
        }
    }

    /// Learns a kind, noting it in the log if it's news.
    fn learn(&mut self, kind: ItemKind) {
        if self.lore.learn(kind) {
            self.log(&format!("It's a {}!", Lore::true_name(kind)));
        }
    }

    pub(crate) fn pick_up(&mut self) -> Outcome {
        let pos = self.player.pos;
        let Some(index) = self.items.iter().position(|i| i.pos == pos) else {
            self.log("There is nothing here to pick up.");
            return Outcome::Free;
        };
        let item = self.items.remove(index).item;
        let name = self.lore.with_article(&item);
        match self.player.add_item(item) {
            Ok(letter) => {
                self.stats.items_picked_up += 1;
                self.log(&format!("You pick up {name} ({letter})."));
                Outcome::TookTurn
            }
            Err(item) => {
                // Put it back where it was.
                self.items.push(FloorItem { pos, item });
                self.log(&format!("You see {name} here, but your pack is full."));
                Outcome::Free
            }
        }
    }

    pub(crate) fn drop_item(&mut self, letter: char) -> Outcome {
        let pos = self.player.pos;
        if self.item_at(pos).is_some() {
            self.log("There is already something here.");
            return Outcome::Free;
        }
        if let Some(item) = self.player.item(letter)
            && item.is_stuck()
        {
            let name = self.lore.name(item);
            self.log(&format!("Your {name} is cursed. You can't let go of it."));
            return Outcome::Free;
        }
        let Some(item) = self.player.remove_item(letter) else {
            return Outcome::Free;
        };
        self.log(&format!("You drop {}.", self.lore.with_article(&item)));
        self.items.push(FloorItem { pos, item });
        Outcome::TookTurn
    }

    /// Equips a weapon, armor or ring. Equipping something already
    /// equipped takes it off, unless a curse holds it in place.
    pub(crate) fn equip(&mut self, letter: char) -> Outcome {
        let Some(item) = self.player.item(letter) else {
            return Outcome::Free;
        };
        let kind = item.kind;
        let name = self.lore.with_article(item);
        if !kind.is_equipment() {
            self.log("You can't equip that.");
            return Outcome::Free;
        }

        if item.equipped {
            if item.is_stuck() {
                let turns = item.curse_turns;
                let name = self.lore.name(item);
                self.log_as(
                    &format!("Your {name} is cursed! It won't come off for {turns} more turns."),
                    MsgKind::Bad,
                );
                return Outcome::Free;
            }
            self.player.item_mut(letter).unwrap().equipped = false;
            match kind {
                ItemKind::Weapon(_) => self.log(&format!("You put away {name}.")),
                _ => self.log(&format!("You take off {name}.")),
            }
            return Outcome::TookTurn;
        }

        // Make room: rings need a free hand; a weapon or armor replaces
        // the one in its slot, unless that one is cursed.
        if let ItemKind::Ring(_) = kind {
            if self.player.rings().count() >= RING_SLOTS {
                self.log("You are already wearing two rings. Take one off first.");
                return Outcome::Free;
            }
        } else {
            let same_slot =
                |k: ItemKind| std::mem::discriminant(&k) == std::mem::discriminant(&kind);
            let current = self
                .player
                .inventory
                .iter()
                .find(|i| i.equipped && same_slot(i.kind));
            if let Some(current) = current {
                if current.is_stuck() {
                    let name = self.lore.name(current);
                    self.log_as(
                        &format!("Your cursed {name} won't let you change it."),
                        MsgKind::Bad,
                    );
                    return Outcome::Free;
                }
                let current = current.letter;
                self.player.item_mut(current).unwrap().equipped = false;
            }
        }

        let item = self.player.item_mut(letter).unwrap();
        item.equipped = true;
        // Whether a curse is still active: a broken or expired one no
        // longer counts, even if the enchantment is still negative.
        let cursed = item.curse_turns > 0;
        if cursed {
            // You find out the hard way.
            item.known = true;
        }
        match kind {
            ItemKind::Weapon(_) => self.log(&format!("You are now wielding {name}.")),
            ItemKind::Ring(_) => {
                self.log(&format!("You put on {name}."));
                // You feel what a ring does as soon as it's on.
                self.learn(kind);
            }
            _ => self.log(&format!("You are now wearing {name}.")),
        }
        if cursed {
            let item = self.player.item(letter).unwrap();
            let (name, turns) = (self.lore.name(item), item.curse_turns);
            self.log_as(
                &format!("It's a cursed {name}! It won't come off for {turns} turns."),
                MsgKind::Bad,
            );
        }
        Outcome::TookTurn
    }

    pub(crate) fn eat(&mut self, letter: char) -> Outcome {
        let Some(ItemKind::Food(kind)) = self.player.item(letter).map(|i| i.kind) else {
            self.log("You can't eat that.");
            return Outcome::Free;
        };
        // Refuse if more than half of it would go to waste.
        let wasted = self.player.food + kind.nutrition() - FOOD_MAX;
        if wasted > kind.nutrition() / 2 {
            self.log("You're too full to eat that now.");
            return Outcome::Free;
        }
        self.player.take_one(letter);
        self.stats.meals_eaten += 1;
        self.player.food = (self.player.food + kind.nutrition()).min(FOOD_MAX);
        match kind {
            crate::item::FoodKind::Ration => {
                self.log_as("That food really hits the spot.", MsgKind::Good)
            }
            crate::item::FoodKind::Jerky => self.log("You chew the tough, salty jerky."),
        }
        Outcome::TookTurn
    }

    pub(crate) fn drink(&mut self, letter: char) -> Outcome {
        let Some(ItemKind::Potion(kind)) = self.player.item(letter).map(|i| i.kind) else {
            self.log("You can't drink that.");
            return Outcome::Free;
        };
        self.player.take_one(letter);
        self.stats.potions_drunk += 1;
        self.learn(ItemKind::Potion(kind));
        let p = &mut self.player;
        match kind {
            PotionKind::Healing => {
                p.hp = (p.hp + HEALING).min(p.max_hp);
                self.log_as("You feel much better.", MsgKind::Good);
            }
            PotionKind::Strength => {
                p.strength += 1;
                self.log_as("You feel stronger.", MsgKind::Good);
            }
            PotionKind::Life => {
                p.max_hp += LIFE;
                p.hp = p.max_hp;
                self.log_as(
                    "Warmth floods your body. You feel more alive.",
                    MsgKind::Good,
                );
            }
            PotionKind::Decay => {
                let damage = self.rng.range(DECAY_DAMAGE.0, DECAY_DAMAGE.1 + 1);
                self.hurt_player(damage);
                self.log_as(
                    &format!("The potion burns like acid! You take {damage} damage."),
                    MsgKind::Bad,
                );
                if self.player.hp <= 0 {
                    self.kill_player("a potion of decay");
                }
            }
        }
        Outcome::TookTurn
    }

    /// Reads a scroll. Enchanting and identify act on `target`.
    ///
    /// If the player already knows the scroll, a missing or unsuitable
    /// target cancels the reading. If they don't, the scroll is used up
    /// anyway: otherwise the target prompt would reveal what an unknown
    /// scroll is for free.
    pub(crate) fn read(&mut self, letter: char, target: Option<char>) -> Outcome {
        let Some(ItemKind::Scroll(kind)) = self.player.item(letter).map(|i| i.kind) else {
            self.log("You can't read that.");
            return Outcome::Free;
        };
        let known = self.lore.knows(ItemKind::Scroll(kind));
        let target = target.filter(|&t| {
            self.player
                .item(t)
                .is_some_and(|item| self.is_read_target(letter, item))
        });
        let needs_target = self.scroll_needs_target(letter);
        if needs_target && target.is_none() && known {
            self.log("You need to choose something for that scroll to work on.");
            return Outcome::Free;
        }
        self.player.take_one(letter);
        self.stats.scrolls_read += 1;
        self.learn(ItemKind::Scroll(kind));

        match (kind, target) {
            (ScrollKind::Teleportation, _) => self.teleport_player(),
            (ScrollKind::MagicMapping, _) => {
                self.map_whole_floor();
                self.log_as("A map of this floor forms in your mind.", MsgKind::Good);
            }
            (ScrollKind::Enchanting, Some(t)) => {
                let item = self.player.item_mut(t).unwrap();
                item.enchant += 1;
                let broke_curse = item.curse_turns > 0;
                item.curse_turns = 0;
                let name = self.lore.name(self.player.item(t).unwrap());
                self.log_as(
                    &format!("Your {name} glows blue for a moment."),
                    MsgKind::Good,
                );
                if broke_curse {
                    self.log_as("The curse on it is broken.", MsgKind::Good);
                }
            }
            (ScrollKind::Identify, Some(t)) => {
                let item = self.player.item_mut(t).unwrap();
                item.known = true;
                let kind = item.kind;
                self.lore.learn(kind);
                let name = self.lore.with_article(self.player.item(t).unwrap());
                self.log_as(&format!("It is {name}."), MsgKind::Good);
            }
            (ScrollKind::Enchanting | ScrollKind::Identify, None) => {
                self.log("Its magic fades with nothing to work on.");
            }
            (ScrollKind::Aggravate, _) => {
                let last_seen = self.player.pos;
                for m in &mut self.monsters {
                    m.ai = Ai::Hunting { last_seen };
                }
                self.log_as(
                    "A piercing shriek echoes through the halls! Everything is coming.",
                    MsgKind::Bad,
                );
            }
        }
        Outcome::TookTurn
    }

    /// Moves the player to a random free tile, preferring somewhere
    /// far from where they stand.
    pub(crate) fn teleport_player(&mut self) {
        let from = self.player.pos;
        let free: Vec<Point> = self
            .map
            .points()
            .filter(|&p| self.map.tile(p).is_walkable() && self.monster_at(p).is_none())
            .collect();
        let far: Vec<Point> = free
            .iter()
            .copied()
            .filter(|p| p.dist_sq(from) > 15 * 15)
            .collect();
        let choices = if far.is_empty() { &free } else { &far };
        if let Some(&to) = choices.get(self.rng.index(choices.len().max(1))) {
            self.player.pos = to;
        }
        self.log("The world twists around you. You are somewhere else.");
    }

    /// Reveals every floor tile, and every wall that borders one.
    fn map_whole_floor(&mut self) {
        let points: Vec<Point> = self.map.points().collect();
        for p in points {
            let tile = self.map.tile(p);
            let borders_open = DIRECTIONS_8
                .iter()
                .any(|&d| self.map.tile(p + d).is_passable());
            if tile.is_passable() || borders_open {
                self.map.reveal(p);
            }
        }
        // The map shows the floor's traps too.
        for trap in &mut self.traps {
            trap.known = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::{Action, Game};
    use crate::geom::Point;
    use crate::item::{
        ArmorKind, FloorItem, Item, ItemKind, PotionKind, RingKind, ScrollKind, WeaponKind,
    };
    use crate::map::{Map, Tile};
    use crate::monster::{Ai, Kind, Monster};

    /// An empty 30x9 room, the player at (2,5), and nothing else.
    fn room_game() -> Game {
        let mut game = Game::new(1);
        let mut map = Map::new_filled(32, 11);
        map.carve_room(1, 1, 30, 9);
        game.place_on_map(map, Point::new(2, 5));
        game.monsters.clear();
        game.items.clear();
        game.log.clear();
        game.update_fov();
        game
    }

    fn give(game: &mut Game, kind: ItemKind) -> char {
        game.player.add_item(Item::new(kind)).unwrap()
    }

    fn last_log(game: &Game) -> &str {
        &game.log.last().unwrap().text
    }

    #[test]
    fn walking_onto_an_item_picks_it_up() {
        let mut game = room_game();
        let dagger = Item::new(ItemKind::Weapon(WeaponKind::Dagger));
        game.items.push(FloorItem {
            pos: Point::new(3, 5),
            item: dagger,
        });
        game.apply(Action::Move(Point::new(1, 0)));
        assert!(game.items.is_empty());
        // a-d hold the starting kit, so the dagger goes to e.
        let dagger = game.player.item('e').unwrap();
        assert_eq!(dagger.kind, ItemKind::Weapon(WeaponKind::Dagger));
        assert_eq!(last_log(&game), "You pick up a dagger (e).");
        assert_eq!(game.turn, 1, "picking up is part of the step");
    }

    #[test]
    fn dropping_and_picking_up_again() {
        let mut game = room_game();
        game.apply(Action::Drop('a'));
        assert!(game.player.weapon().is_none(), "dropping unequips");
        assert!(game.item_at(game.player.pos).is_some());
        game.apply(Action::Drop('b'));
        assert_eq!(last_log(&game), "There is already something here.");
        game.apply(Action::PickUp);
        assert_eq!(game.lore.name(game.player.item('a').unwrap()), "+0 sword");
    }

    #[test]
    fn equipping_swaps_and_toggles() {
        let mut game = room_game();
        let axe = give(&mut game, ItemKind::Weapon(WeaponKind::Axe));
        game.apply(Action::Equip(axe));
        assert_eq!(game.player.weapon().map(|i| i.letter), Some(axe));
        assert!(
            !game.player.item('a').unwrap().equipped,
            "the sword was put away"
        );
        game.apply(Action::Equip(axe));
        assert!(game.player.weapon().is_none());
        game.apply(Action::Equip('c')); // the healing potion
        assert_eq!(last_log(&game), "You can't equip that.");
    }

    #[test]
    fn potions_take_effect_and_are_used_up() {
        let mut game = room_game();
        game.player.hp = 5;
        game.apply(Action::Drink('c'));
        assert_eq!(game.player.hp, 20);
        assert!(game.player.item('c').is_none());

        let strength = give(&mut game, ItemKind::Potion(PotionKind::Strength));
        game.apply(Action::Drink(strength));
        assert_eq!(game.player.strength, 5);

        let life = give(&mut game, ItemKind::Potion(PotionKind::Life));
        game.apply(Action::Drink(life));
        assert_eq!(game.player.max_hp, 30);
        assert_eq!(game.player.hp, 30);
    }

    #[test]
    fn healing_never_exceeds_max() {
        let mut game = room_game();
        game.player.hp = game.player.max_hp - 1;
        game.apply(Action::Drink('c'));
        assert_eq!(game.player.hp, game.player.max_hp);
    }

    #[test]
    fn decay_can_kill() {
        let mut game = room_game();
        game.player.hp = 1;
        let decay = give(&mut game, ItemKind::Potion(PotionKind::Decay));
        game.apply(Action::Drink(decay));
        let cause = game.death_summary().expect("1 health can't survive decay");
        assert_eq!(
            cause,
            "Killed by a potion of decay on depth 1 after 1 turns."
        );
    }

    #[test]
    fn a_fatal_potion_on_a_healing_turn_stays_fatal() {
        let mut game = room_game();
        game.player.hp = 1;
        // The drink will land exactly on a turn when healing happens.
        game.player.regen_progress = 99;
        let decay = give(&mut game, ItemKind::Potion(PotionKind::Decay));
        game.apply(Action::Drink(decay));
        assert!(game.death.is_some());
        assert_eq!(game.player.hp, 0);
    }

    #[test]
    fn teleport_moves_somewhere_free_and_far() {
        let mut game = room_game();
        let start = game.player.pos;
        let scroll = give(&mut game, ItemKind::Scroll(ScrollKind::Teleportation));
        game.apply(Action::Read {
            scroll,
            target: None,
        });
        assert!(game.map.tile(game.player.pos).is_walkable());
        assert!(game.player.pos.dist_sq(start) > 15 * 15);
    }

    #[test]
    fn magic_mapping_reveals_the_floor() {
        let mut game = Game::new(3);
        let stairs = game
            .map
            .points()
            .find(|&p| game.map.tile(p) == Tile::StairsDown)
            .unwrap();
        assert!(!game.map.is_revealed(stairs));
        let scroll = give(&mut game, ItemKind::Scroll(ScrollKind::MagicMapping));
        game.apply(Action::Read {
            scroll,
            target: None,
        });
        assert!(game.map.is_revealed(stairs));
    }

    #[test]
    fn enchanting_needs_a_valid_target() {
        let mut game = room_game();
        // This test is about a scroll the player already knows.
        game.lore.learn(ItemKind::Scroll(ScrollKind::Enchanting));
        let scroll = give(&mut game, ItemKind::Scroll(ScrollKind::Enchanting));
        assert!(game.scroll_needs_target(scroll));

        game.apply(Action::Read {
            scroll,
            target: Some('c'),
        }); // a potion
        assert!(game.player.item(scroll).is_some(), "scroll not wasted");
        assert_eq!(game.turn, 0);

        game.apply(Action::Read {
            scroll,
            target: Some('b'),
        });
        assert_eq!(game.player.item('b').unwrap().enchant, 1);
        let armor = game.player.item('b').unwrap();
        assert_eq!(game.lore.name(armor), "+1 leather armor");
        assert!(game.player.item(scroll).is_none());
    }

    #[test]
    fn aggravate_wakes_everything() {
        let mut game = room_game();
        game.monsters
            .push(Monster::new(Kind::Rat, Point::new(25, 2), Ai::Asleep));
        let scroll = give(&mut game, ItemKind::Scroll(ScrollKind::Aggravate));
        game.apply(Action::Read {
            scroll,
            target: None,
        });
        assert!(matches!(game.monsters[0].ai, Ai::Hunting { .. }));
    }

    #[test]
    fn a_full_pack_leaves_the_item_on_the_floor() {
        let mut game = room_game();
        while game.player.inventory.len() < crate::player::PACK_SIZE {
            give(&mut game, ItemKind::Armor(ArmorKind::Plate));
        }
        game.items.push(FloorItem {
            pos: Point::new(3, 5),
            item: Item::new(ItemKind::Weapon(WeaponKind::Mace)),
        });
        game.apply(Action::Move(Point::new(1, 0)));
        assert_eq!(game.items.len(), 1);
        assert!(last_log(&game).contains("pack is full"));
    }

    // ---- Identification, rings and curses ---------------------------

    fn give_item(game: &mut Game, item: Item) -> char {
        game.player.add_item(item).unwrap()
    }

    #[test]
    fn drinking_an_unknown_potion_teaches_it() {
        let mut game = room_game();
        let kind = ItemKind::Potion(PotionKind::Strength);
        let letter = give(&mut game, kind);
        assert!(!game.lore.knows(kind));
        game.apply(Action::Drink(letter));
        assert!(game.lore.knows(kind));
        assert!(
            game.log
                .iter()
                .any(|m| m.text == "It's a potion of strength!")
        );
    }

    #[test]
    fn an_unknown_targeted_scroll_is_used_up_even_without_a_target() {
        let mut game = room_game();
        let kind = ItemKind::Scroll(ScrollKind::Enchanting);
        let scroll = give(&mut game, kind);
        game.apply(Action::Read {
            scroll,
            target: None,
        });
        assert!(game.player.item(scroll).is_none(), "used up");
        assert!(game.lore.knows(kind), "and learned");
        assert_eq!(game.turn, 1);

        // Once known, reading without a target is refused instead.
        let scroll = give(&mut game, kind);
        game.apply(Action::Read {
            scroll,
            target: None,
        });
        assert!(game.player.item(scroll).is_some());
        assert_eq!(game.turn, 1);
    }

    #[test]
    fn identify_reveals_gear_and_kinds() {
        let mut game = room_game();
        let identify = ItemKind::Scroll(ScrollKind::Identify);
        let axe = give_item(
            &mut game,
            Item::enchanted(ItemKind::Weapon(WeaponKind::Axe), 2),
        );
        let scroll = give(&mut game, identify);
        assert!(game.is_read_target(scroll, game.player.item(axe).unwrap()));
        assert!(
            !game.is_read_target(scroll, game.player.item('a').unwrap()),
            "sword is known"
        );
        game.apply(Action::Read {
            scroll,
            target: Some(axe),
        });
        assert!(game.player.item(axe).unwrap().known);
        assert_eq!(last_log(&game), "It is a +2 battle axe.");

        let potion = give(&mut game, ItemKind::Potion(PotionKind::Life));
        let scroll = give(&mut game, identify);
        game.apply(Action::Read {
            scroll,
            target: Some(potion),
        });
        assert!(game.lore.knows(ItemKind::Potion(PotionKind::Life)));
    }

    #[test]
    fn cursed_gear_sticks_until_the_curse_fades() {
        let mut game = room_game();
        let cursed = give_item(
            &mut game,
            Item::enchanted(ItemKind::Weapon(WeaponKind::Mace), -2),
        );
        game.apply(Action::Equip(cursed));
        let mace = game.player.item(cursed).unwrap();
        assert!(mace.known, "a curse reveals itself");
        // -2 means 100 turns, and equipping it used the first one.
        assert_eq!(mace.curse_turns, 99);
        assert!(last_log(&game).contains("cursed"));

        let turn = game.turn;
        game.apply(Action::Equip(cursed)); // try to take it off
        game.apply(Action::Equip('a')); // try to swap to the sword
        game.apply(Action::Drop(cursed));
        assert_eq!(game.turn, turn, "none of those work or take time");
        assert_eq!(game.player.weapon().map(|w| w.letter), Some(cursed));

        for _ in 0..100 {
            game.apply(Action::Wait);
        }
        assert!(
            game.log
                .iter()
                .any(|m| m.text == "The curse on your -2 mace fades.")
        );
        game.apply(Action::Equip('a'));
        assert_eq!(game.player.weapon().map(|w| w.letter), Some('a'));
    }

    #[test]
    fn a_broken_or_expired_curse_stays_gone() {
        let mut game = room_game();
        let scroll_kind = ItemKind::Scroll(ScrollKind::Enchanting);
        game.lore.learn(scroll_kind);
        let mace = give_item(
            &mut game,
            Item::enchanted(ItemKind::Weapon(WeaponKind::Mace), -3),
        );
        let scroll = give(&mut game, scroll_kind);
        game.apply(Action::Read {
            scroll,
            target: Some(mace),
        }); // unworn: now -2, no curse
        game.log.clear();
        game.apply(Action::Equip(mace));
        let item = game.player.item(mace).unwrap();
        assert!(!item.known, "no curse, so no early reveal");
        assert!(game.log.iter().all(|m| !m.text.contains("cursed")));
        game.apply(Action::Equip(mace));
        assert!(game.player.weapon().is_none(), "comes straight off");

        // A curse that ran out doesn't come back on re-equipping.
        let armor = give_item(
            &mut game,
            Item::enchanted(ItemKind::Armor(ArmorKind::Chain), -1),
        );
        game.apply(Action::Equip(armor));
        for _ in 0..50 {
            game.apply(Action::Wait);
        }
        game.apply(Action::Equip(armor)); // off
        game.log.clear();
        game.apply(Action::Equip(armor)); // on again
        assert!(game.log.iter().all(|m| !m.text.contains("cursed")));
        game.apply(Action::Equip(armor));
        assert!(game.player.armor().is_none());
    }

    #[test]
    fn enchanting_breaks_a_curse() {
        let mut game = room_game();
        let cursed = give_item(
            &mut game,
            Item::enchanted(ItemKind::Armor(ArmorKind::Chain), -1),
        );
        game.apply(Action::Equip(cursed));
        let scroll = give(&mut game, ItemKind::Scroll(ScrollKind::Enchanting));
        game.apply(Action::Read {
            scroll,
            target: Some(cursed),
        });
        let armor = game.player.item(cursed).unwrap();
        assert_eq!((armor.enchant, armor.curse_turns), (0, 0));
        game.apply(Action::Equip(cursed));
        assert!(game.player.armor().is_none(), "it comes off now");
    }

    #[test]
    fn wearing_gear_reveals_it_and_intellect_speeds_that_up() {
        for (intellect, turns) in [(2, 300), (4, 210)] {
            let mut game = room_game();
            game.player.intellect = intellect;
            assert_eq!(game.identify_threshold(), turns);
            let plate = give_item(
                &mut game,
                Item::enchanted(ItemKind::Armor(ArmorKind::Plate), 1),
            );
            game.apply(Action::Equip(plate)); // one turn worn
            for _ in 0..turns - 2 {
                game.apply(Action::Wait);
            }
            assert!(!game.player.item(plate).unwrap().known);
            game.apply(Action::Wait);
            assert!(game.player.item(plate).unwrap().known, "Int {intellect}");
        }
    }

    #[test]
    fn carrying_alone_reveals_nothing() {
        let mut game = room_game();
        let plate = give_item(
            &mut game,
            Item::enchanted(ItemKind::Armor(ArmorKind::Plate), 1),
        );
        for _ in 0..400 {
            game.apply(Action::Wait);
        }
        assert!(!game.player.item(plate).unwrap().known);
    }

    #[test]
    fn rings_teach_their_kind_and_only_two_fit() {
        let mut game = room_game();
        let kind = ItemKind::Ring(RingKind::Protection);
        let rings: Vec<char> = (0..3)
            .map(|_| give_item(&mut game, Item::enchanted(kind, 1)))
            .collect();
        let armor = game.player.defense().armor;
        game.apply(Action::Equip(rings[0]));
        assert!(game.lore.knows(kind));
        assert_eq!(game.player.defense().armor, armor + 1);
        game.apply(Action::Equip(rings[1]));
        game.apply(Action::Equip(rings[2]));
        assert!(!game.player.item(rings[2]).unwrap().equipped);
        assert!(last_log(&game).contains("two rings"));
    }

    #[test]
    fn awareness_widens_sight() {
        let mut game = room_game();
        let far = Point::new(2 + 10, 5);
        assert!(!game.is_visible(far));
        let ring = give_item(
            &mut game,
            Item::enchanted(ItemKind::Ring(RingKind::Awareness), 3),
        );
        game.apply(Action::Equip(ring));
        assert!(game.is_visible(far));
    }
}
