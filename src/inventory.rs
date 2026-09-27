//! Using items: picking up, dropping, equipping, drinking and reading.
//!
//! Another `impl Game` block, like ai.rs, so game.rs stays about
//! movement and turns.

use crate::game::{Game, MsgKind, Outcome};
use crate::geom::{DIRECTIONS_8, Point};
use crate::item::{FloorItem, ItemKind, PotionKind, ScrollKind};
use crate::monster::Ai;

/// Health restored by a potion of healing.
const HEALING: i32 = 15;
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
            Some(ItemKind::Scroll(ScrollKind::Enchanting))
        )
    }

    pub(crate) fn pick_up(&mut self) -> Outcome {
        let pos = self.player.pos;
        let Some(index) = self.items.iter().position(|i| i.pos == pos) else {
            self.log("There is nothing here to pick up.");
            return Outcome::Free;
        };
        let item = self.items.remove(index).item;
        let name = item.with_article();
        match self.player.add_item(item) {
            Ok(letter) => {
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
        let Some(item) = self.player.remove_item(letter) else {
            return Outcome::Free;
        };
        self.log(&format!("You drop {}.", item.with_article()));
        self.items.push(FloorItem { pos, item });
        Outcome::TookTurn
    }

    /// Equips a weapon or armor, swapping out whatever was in that
    /// slot. Equipping something already equipped takes it off.
    pub(crate) fn equip(&mut self, letter: char) -> Outcome {
        let Some(item) = self.player.item(letter) else {
            return Outcome::Free;
        };
        let kind = item.kind;
        let name = item.with_article();
        if !kind.is_equipment() {
            self.log("You can't equip that.");
            return Outcome::Free;
        }
        if item.equipped {
            self.player.item_mut(letter).unwrap().equipped = false;
            match kind {
                ItemKind::Weapon(_) => self.log(&format!("You put away {name}.")),
                _ => self.log(&format!("You take off {name}.")),
            }
            return Outcome::TookTurn;
        }
        // Only one weapon and one armor at a time.
        let same_slot = |k: ItemKind| std::mem::discriminant(&k) == std::mem::discriminant(&kind);
        for other in &mut self.player.inventory {
            if same_slot(other.kind) {
                other.equipped = false;
            }
        }
        self.player.item_mut(letter).unwrap().equipped = true;
        match kind {
            ItemKind::Weapon(_) => self.log(&format!("You are now wielding {name}.")),
            _ => self.log(&format!("You are now wearing {name}.")),
        }
        Outcome::TookTurn
    }

    pub(crate) fn drink(&mut self, letter: char) -> Outcome {
        let Some(ItemKind::Potion(kind)) = self.player.item(letter).map(|i| i.kind) else {
            self.log("You can't drink that.");
            return Outcome::Free;
        };
        self.player.take_one(letter);
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
                self.player.hp -= damage;
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

    pub(crate) fn read(&mut self, letter: char, target: Option<char>) -> Outcome {
        let Some(ItemKind::Scroll(kind)) = self.player.item(letter).map(|i| i.kind) else {
            self.log("You can't read that.");
            return Outcome::Free;
        };
        // Check the enchanting target before using up the scroll.
        let target = match kind {
            ScrollKind::Enchanting => {
                let valid = target
                    .and_then(|t| self.player.item(t))
                    .is_some_and(|i| i.kind.is_equipment());
                if !valid {
                    self.log("That scroll needs a weapon or armor to enchant.");
                    return Outcome::Free;
                }
                target
            }
            _ => None,
        };
        self.player.take_one(letter);

        match kind {
            ScrollKind::Teleportation => self.teleport_player(),
            ScrollKind::MagicMapping => {
                self.map_whole_floor();
                self.log_as("A map of this floor forms in your mind.", MsgKind::Good);
            }
            ScrollKind::Enchanting => {
                // `target` was checked above, before the scroll was used.
                let item = self.player.item_mut(target.unwrap()).unwrap();
                item.enchant += 1;
                let name = item.name();
                self.log_as(
                    &format!("Your {name} glows blue for a moment."),
                    MsgKind::Good,
                );
            }
            ScrollKind::Aggravate => {
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
    fn teleport_player(&mut self) {
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
    }
}

#[cfg(test)]
mod tests {
    use crate::game::{Action, Game};
    use crate::geom::Point;
    use crate::item::{ArmorKind, FloorItem, Item, ItemKind, PotionKind, ScrollKind, WeaponKind};
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
        assert_eq!(game.player.item('d').map(Item::name), Some("dagger".into()));
        assert_eq!(last_log(&game), "You pick up a dagger (d).");
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
        assert_eq!(game.player.item('a').map(Item::name), Some("sword".into()));
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
        let cause = game.death.expect("1 health can't survive decay");
        assert!(cause.starts_with("Killed by a potion of decay"), "{cause}");
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
        assert_eq!(game.player.item('b').unwrap().name(), "+1 leather armor");
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
}
