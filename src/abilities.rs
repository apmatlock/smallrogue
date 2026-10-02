//! What special monster abilities do when they come into play.
//!
//! Another `impl Game` block. The attack code in game.rs decides *when*
//! an ability triggers; the functions here carry it out.

use crate::game::{Game, MsgKind};
use crate::geom::{DIRECTIONS_8, Point};
use crate::item::FloorItem;
use crate::monster::{Ai, Monster};

/// Jellies stop splitting once a floor holds this many.
pub const MAX_JELLIES: usize = 10;
/// Life drain never takes maximum health below this.
pub const MIN_DRAINED_HEALTH: i32 = 10;
/// Acid never corrodes armor below this enchantment.
pub const MIN_CORRODED_ENCHANT: i32 = -1;
/// Chance that an acid hit corrodes the armor. Every hit doing so made
/// armor loss snowball: balance runs dropped from depth 16 to 12.
pub const CORRODE_PERCENT: i32 = 33;

impl Game {
    /// Monster `i` snatches one unequipped item and starts to flee.
    /// Returns false if there was nothing to steal.
    pub(crate) fn steal_item(&mut self, i: usize) -> bool {
        let loose: Vec<char> = self
            .player
            .inventory
            .iter()
            .filter(|item| !item.equipped)
            .map(|item| item.letter)
            .collect();
        if loose.is_empty() {
            return false;
        }
        let letter = loose[self.rng.index(loose.len())];
        let Some(item) = self.player.take_one(letter) else {
            return false;
        };
        let name = self.monsters[i].name();
        let what = self.lore.with_article(&item);
        self.log_as(
            &format!("The {name} snatches {what} and runs!"),
            MsgKind::Bad,
        );
        let m = &mut self.monsters[i];
        m.carrying = Some(item);
        m.ai = Ai::Fleeing;
        true
    }

    /// A draining hit: maximum health drops by 1, down to a floor.
    pub(crate) fn drain_max_health(&mut self, i: usize) {
        let p = &mut self.player;
        if p.max_hp <= MIN_DRAINED_HEALTH {
            return;
        }
        p.max_hp -= 1;
        p.hp = p.hp.min(p.max_hp);
        // Copy the number out: logging needs the whole game, so the
        // borrow of the player has to end first.
        let max = p.max_hp;
        let name = self.monsters[i].name();
        self.log_as(
            &format!("The {name}'s touch withers you. Your maximum health falls to {max}."),
            MsgKind::Bad,
        );
    }

    /// A blood-drinking hit heals the attacker by half the damage it
    /// dealt. (All of it made vampires the top killer by far: 41 of 200
    /// fresh runs, against 19 with half and a normal speed.)
    pub(crate) fn drink_blood(&mut self, i: usize, damage: i32) {
        let m = &mut self.monsters[i];
        let max = m.max_hp();
        if m.hp < max {
            m.hp = (m.hp + damage / 2).min(max);
            let name = m.name();
            self.log(&format!("The {name} drinks your blood and looks stronger."));
        }
    }

    /// An acid hit may eat 1 point off the worn armor's enchantment.
    pub(crate) fn acid_hit(&mut self) {
        if self.rng.chance(CORRODE_PERCENT) {
            self.corrode_armor();
        }
    }

    /// Eats 1 point off the worn armor's enchantment.
    pub(crate) fn corrode_armor(&mut self) {
        let Some(letter) = self.player.armor().map(|a| a.letter) else {
            return;
        };
        let armor = self
            .player
            .item_mut(letter)
            .expect("worn armor is in the pack");
        if armor.enchant <= MIN_CORRODED_ENCHANT {
            return;
        }
        armor.enchant -= 1;
        // Unidentified armor loses the point just the same; the player
        // only sees the new number once they know it.
        let armor = self.player.item(letter).expect("still there");
        let name = self.lore.name(armor);
        let text = if armor.known {
            format!("Acid eats into your armor! It is now {name}.")
        } else {
            format!("Acid eats into your {name}!")
        };
        self.log_as(&text, MsgKind::Bad);
    }

    /// Monster `i` was hit and lived: if it splits, half its health
    /// becomes a new monster next to it.
    pub(crate) fn split_monster(&mut self, i: usize) {
        let kind = self.monsters[i].kind;
        let jellies = self.monsters.iter().filter(|m| m.kind == kind).count();
        if self.monsters[i].hp < 2 || jellies >= MAX_JELLIES {
            return;
        }
        let pos = self.monsters[i].pos;
        let Some(spot) = self.free_spot_near(pos) else {
            return;
        };
        let half = self.monsters[i].hp / 2;
        self.monsters[i].hp -= half;
        let mut twin = Monster::at_depth(
            kind,
            spot,
            Ai::Hunting {
                last_seen: self.player.pos,
            },
            1,
        );
        // Same depth scaling as its parent, and exactly half its health.
        twin.boost = self.monsters[i].boost;
        twin.hp = half;
        self.monsters.push(twin);
        let name = kind.species().name;
        self.log(&format!("The {name} splits in two!"));
    }

    /// A free, walkable tile next to `pos` with no monster or player.
    fn free_spot_near(&self, pos: Point) -> Option<Point> {
        DIRECTIONS_8.iter().map(|&d| pos + d).find(|&p| {
            self.map.tile(p).is_walkable() && p != self.player.pos && self.monster_at(p).is_none()
        })
    }

    /// Drops what a dead monster carried, on its tile or next to it.
    pub(crate) fn drop_loot(&mut self, monster: &Monster) {
        let Some(item) = monster.carrying.clone() else {
            return;
        };
        let spot = std::iter::once(monster.pos)
            .chain(DIRECTIONS_8.iter().map(|&d| monster.pos + d))
            .find(|&p| self.map.tile(p).is_walkable() && self.item_at(p).is_none());
        let what = self.lore.with_article(&item);
        let name = monster.name();
        match spot {
            Some(pos) => {
                self.items.push(FloorItem { pos, item });
                self.log_as(&format!("The {name} drops {what}."), MsgKind::Good);
            }
            // Nowhere to put it: rare enough to just say so.
            None => self.log(&format!("{what} is lost in the scuffle.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::{Action, Game};
    use crate::geom::Point;
    use crate::item::{ArmorKind, Item, ItemKind};
    use crate::map::Map;
    use crate::monster::{Ai, HEALTH_BAR_CELLS, Kind, Monster};

    fn room_game() -> Game {
        let mut game = Game::new(1);
        let mut map = Map::new_filled(32, 11);
        map.carve_room(1, 1, 30, 9);
        game.place_on_map(map, Point::new(2, 5));
        game.monsters.clear();
        game.items.clear();
        game.traps.clear();
        game.log.clear();
        game.update_fov();
        game
    }

    fn hunter(game: &Game, kind: Kind, pos: Point) -> Monster {
        let mut m = Monster::new(
            kind,
            pos,
            Ai::Hunting {
                last_seen: game.player.pos,
            },
        );
        m.energy = 0;
        m
    }

    /// Waits until `done` holds, up to a limit, so tests don't depend
    /// on a lucky roll.
    fn wait_until(game: &mut Game, done: impl Fn(&Game) -> bool) {
        for _ in 0..300 {
            if done(game) || game.death.is_some() {
                return;
            }
            game.apply(Action::Wait);
        }
    }

    #[test]
    fn trolls_regenerate() {
        let mut game = room_game();
        let mut troll = hunter(&game, Kind::Troll, Point::new(25, 2));
        troll.ai = Ai::Asleep;
        troll.hp = 5;
        game.monsters.push(troll);
        game.apply(Action::Wait);
        assert!(game.monsters[0].hp > 5);
    }

    #[test]
    fn wraiths_drain_maximum_health() {
        let mut game = room_game();
        game.player.hp = 500;
        game.player.max_hp = 500;
        let wraith = hunter(&game, Kind::Wraith, Point::new(3, 5));
        game.monsters.push(wraith);
        wait_until(&mut game, |g| g.player.max_hp < 500);
        assert!(game.player.max_hp < 500);
    }

    #[test]
    fn vampires_heal_by_half_the_blood_they_draw() {
        let mut game = room_game();
        let mut vampire = hunter(&game, Kind::Vampire, Point::new(3, 5));
        vampire.hp = 1;
        game.monsters.push(vampire);
        game.drink_blood(0, 9);
        assert_eq!(game.monsters[0].hp, 1 + 4);
        // Never past full health.
        game.drink_blood(0, 1000);
        assert_eq!(game.monsters[0].hp, game.monsters[0].max_hp());
    }

    #[test]
    fn monkeys_steal_and_drop_the_loot_when_killed() {
        let mut game = room_game();
        game.player.hp = 500;
        let monkey = hunter(&game, Kind::Monkey, Point::new(3, 5));
        game.monsters.push(monkey);
        let loose = |g: &Game| g.player.inventory.iter().filter(|i| !i.equipped).count();
        let before = loose(&game);
        wait_until(&mut game, |g| g.monsters[0].carrying.is_some());
        assert_eq!(game.monsters[0].ai, Ai::Fleeing);
        assert_eq!(loose(&game), before - 1, "one item taken");

        // Kill the thief where it stands: the item comes back.
        let spot = game.monsters[0].pos;
        game.monsters[0].hp = 0;
        let thief = game.monsters.remove(0);
        game.drop_loot(&thief);
        assert!(game.item_at(spot).is_some() || !game.items.is_empty());
    }

    #[test]
    fn a_fleeing_thief_moves_away() {
        let mut game = room_game();
        let mut monkey = hunter(&game, Kind::Monkey, Point::new(4, 5));
        monkey.ai = Ai::Fleeing;
        monkey.carrying = Some(Item::new(ItemKind::Armor(ArmorKind::Plate)));
        game.monsters.push(monkey);
        let before = game.monsters[0].pos.dist_sq(game.player.pos);
        game.apply(Action::Wait);
        assert!(game.monsters[0].pos.dist_sq(game.player.pos) > before);
    }

    #[test]
    fn acid_corrodes_armor_down_to_a_floor() {
        let mut game = room_game();
        let before = game.player.armor().unwrap().enchant;
        let mound = hunter(&game, Kind::AcidMound, Point::new(3, 5));
        game.monsters.push(mound);
        game.player.hp = 500;
        wait_until(&mut game, |g| g.player.armor().unwrap().enchant < before);
        assert_eq!(game.player.armor().unwrap().enchant, before - 1);
        for _ in 0..10 {
            game.corrode_armor();
        }
        assert_eq!(
            game.player.armor().unwrap().enchant,
            super::MIN_CORRODED_ENCHANT
        );
    }

    #[test]
    fn jellies_split_when_hit_and_stop_at_the_cap() {
        let mut game = room_game();
        let mut jelly = hunter(&game, Kind::PinkJelly, Point::new(3, 5));
        jelly.hp = 1_000; // never dies from a hit here
        game.monsters.push(jelly);
        let total = |g: &Game| g.monsters.iter().map(|m| m.hp).sum::<i32>();
        for _ in 0..60 {
            let Some(target) = game
                .monsters
                .iter()
                .find(|m| m.pos.is_adjacent(game.player.pos))
            else {
                break;
            };
            let dir = target.pos - game.player.pos;
            game.apply(Action::Move(dir));
        }
        assert!(game.monsters.len() > 1, "never split");
        assert!(game.monsters.len() <= super::MAX_JELLIES);
        assert!(total(&game) <= 1_000, "splitting never creates health");
    }

    fn scratch_warnings(game: &Game) -> usize {
        game.log
            .iter()
            .filter(|m| m.text.contains("barely scratch"))
            .count()
    }

    #[test]
    fn attacks_that_never_wear_a_monster_down_get_one_warning() {
        let mut game = room_game();
        game.player.hp = 100_000;
        game.player.max_hp = 100_000;
        // Far too tough to dent, and healing from every bite.
        let mut vampire = Monster::at_depth(
            Kind::Vampire,
            Point::new(3, 5),
            Ai::Hunting {
                last_seen: game.player.pos,
            },
            60,
        );
        vampire.energy = 0;
        game.monsters.push(vampire);
        for _ in 0..crate::game::SCRATCH_AFTER - 1 {
            game.apply(Action::Move(Point::new(1, 0)));
        }
        assert_eq!(scratch_warnings(&game), 0, "too soon to tell");
        for _ in 0..30 {
            game.apply(Action::Move(Point::new(1, 0)));
        }
        assert_eq!(game.monsters[0].health_bar(), HEALTH_BAR_CELLS);
        assert_eq!(scratch_warnings(&game), 1, "once per monster");
    }

    #[test]
    fn a_fight_that_goes_somewhere_gets_no_warning() {
        let mut game = room_game();
        game.player.hp = 100_000;
        game.player.max_hp = 100_000;
        let mut ogre = hunter(&game, Kind::Ogre, Point::new(3, 5));
        ogre.ai = Ai::Asleep;
        game.monsters.push(ogre);
        while !game.monsters.is_empty() {
            game.apply(Action::Move(Point::new(1, 0)));
        }
        assert_eq!(scratch_warnings(&game), 0);
    }
}
