//! Hidden traps: placing them, spotting them, and setting them off.
//!
//! Traps start hidden. Standing near one gives a small chance each turn
//! to notice it (better with a ring of awareness), magic mapping reveals
//! them all, and setting one off reveals it the hard way. Only the
//! player sets traps off: monsters know their own dungeon.

use crate::dungeon::{self, Level};
use crate::frame::Rgb;
use crate::game::{Game, MsgKind, Outcome};
use crate::geom::Point;
use crate::item::FloorItem;
use crate::item::RingKind;
use crate::map::Tile;
use crate::monster::Ai;
use crate::rng::Rng;

/// Chance per turn of noticing a hidden trap in range, in percent, and
/// how much each point of a ring of awareness adds.
pub const NOTICE_PERCENT: i32 = 15;
pub const NOTICE_PER_AWARENESS: i32 = 10;
/// Monsters within this distance hear an alarm trap.
pub const ALARM_RADIUS: i32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapKind {
    Dart,
    Alarm,
    Teleport,
    Trapdoor,
}

impl TrapKind {
    pub const ALL: [TrapKind; 4] = [
        TrapKind::Dart,
        TrapKind::Alarm,
        TrapKind::Teleport,
        TrapKind::Trapdoor,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TrapKind::Dart => "dart trap",
            TrapKind::Alarm => "alarm trap",
            TrapKind::Teleport => "teleport trap",
            TrapKind::Trapdoor => "trapdoor",
        }
    }

    pub fn color(self) -> Rgb {
        match self {
            TrapKind::Dart => Rgb(210, 90, 80),
            TrapKind::Alarm => Rgb(225, 200, 80),
            TrapKind::Teleport => Rgb(190, 110, 220),
            TrapKind::Trapdoor => Rgb(170, 130, 90),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trap {
    pub pos: Point,
    pub kind: TrapKind,
    /// Whether the player knows it's there.
    pub known: bool,
}

/// Places hidden traps on a new floor: a few more the deeper it is.
/// They avoid the start room, the stairs and items.
pub fn spawn_for_floor(rng: &mut Rng, level: &Level, items: &[FloorItem], depth: u32) -> Vec<Trap> {
    let count = (1 + depth as usize / 3).min(6);
    let rooms: Vec<_> = level
        .rooms
        .iter()
        .filter(|&&r| r != level.start_room)
        .collect();
    let mut traps: Vec<Trap> = Vec::new();
    for _ in 0..count * 4 {
        if traps.len() == count || rooms.is_empty() {
            break;
        }
        let room = *rooms[rng.index(rooms.len())];
        let pos = dungeon::random_point_in(rng, room);
        let free = level.map.tile(pos) == Tile::Floor
            && items.iter().all(|i| i.pos != pos)
            && traps.iter().all(|t| t.pos != pos);
        if free {
            let kind = TrapKind::ALL[rng.index(TrapKind::ALL.len())];
            traps.push(Trap {
                pos,
                kind,
                known: false,
            });
        }
    }
    traps
}

impl Game {
    pub fn trap_at(&self, p: Point) -> Option<&Trap> {
        self.traps.iter().find(|t| t.pos == p)
    }

    /// Is there a trap here that the player knows about?
    pub fn known_trap_at(&self, p: Point) -> bool {
        self.trap_at(p).is_some_and(|t| t.known)
    }

    /// Each turn: a chance to notice hidden traps close by. The range is
    /// 1 tile, plus 1 per point of awareness.
    pub(crate) fn search_for_traps(&mut self) {
        let awareness = self.player.ring_bonus(RingKind::Awareness).max(0);
        let range = 1 + awareness;
        let chance = NOTICE_PERCENT + NOTICE_PER_AWARENESS * awareness;
        let pos = self.player.pos;
        for i in 0..self.traps.len() {
            let t = &self.traps[i];
            let d = t.pos - pos;
            let near = d.x.abs() <= range && d.y.abs() <= range;
            if !t.known && near && self.is_visible(t.pos) && self.rng.chance(chance) {
                self.traps[i].known = true;
                let name = self.traps[i].kind.name();
                self.log_as(&format!("You notice a {name}."), MsgKind::Good);
            }
        }
    }

    /// Sets off the trap under the player, if any. Returns an outcome
    /// only if the trap changed floors, which the caller must report.
    pub(crate) fn spring_trap(&mut self) -> Option<Outcome> {
        let pos = self.player.pos;
        let index = self.traps.iter().position(|t| t.pos == pos)?;
        self.traps[index].known = true;
        self.stats.traps_sprung += 1;
        let kind = self.traps[index].kind;
        match kind {
            TrapKind::Dart => {
                let damage = self.rng.range(2, 6) + self.depth as i32 / 3;
                self.hurt_player(damage);
                self.log_as(
                    &format!("A dart shoots out of the wall! You take {damage} damage."),
                    MsgKind::Bad,
                );
                if self.player.hp <= 0 {
                    self.kill_player("a dart trap");
                }
            }
            TrapKind::Alarm => {
                let last_seen = self.player.pos;
                for m in &mut self.monsters {
                    let d = m.pos - last_seen;
                    if d.x.abs().max(d.y.abs()) <= ALARM_RADIUS {
                        m.ai = Ai::Hunting { last_seen };
                    }
                }
                self.log_as(
                    "An alarm shrieks! Everything nearby heard that.",
                    MsgKind::Bad,
                );
            }
            TrapKind::Teleport => {
                self.log("You step on a teleport trap.");
                self.teleport_player();
            }
            TrapKind::Trapdoor => {
                let damage = self.rng.range(1, 4);
                self.hurt_player(damage);
                self.log_as(
                    &format!("A trapdoor opens beneath you! You fall and take {damage} damage."),
                    MsgKind::Bad,
                );
                if self.player.hp <= 0 {
                    self.kill_player("a fall through a trapdoor");
                    return None;
                }
                self.enter_floor(self.depth + 1);
                self.stats.trapdoor_falls += 1;
                self.log(&format!("You land on depth {}.", self.depth));
                return Some(Outcome::NewFloor);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dungeon::{STANDARD, generate};

    #[test]
    fn traps_avoid_the_start_room_items_and_stairs() {
        for seed in 0..40 {
            let mut rng = Rng::new(seed);
            let level = generate(&mut rng, &STANDARD);
            let items = crate::item::spawn_for_floor(&mut rng, &level);
            let traps = spawn_for_floor(&mut rng, &level, &items, 9);
            assert!(!traps.is_empty());
            let r = level.start_room;
            for t in &traps {
                assert_eq!(level.map.tile(t.pos), Tile::Floor);
                assert!(!t.known);
                assert!(items.iter().all(|i| i.pos != t.pos));
                let in_start =
                    t.pos.x >= r.x && t.pos.x < r.x + r.w && t.pos.y >= r.y && t.pos.y < r.y + r.h;
                assert!(!in_start, "seed {seed}");
            }
        }
    }

    // ---- Traps and hunger in play -----------------------------------

    use crate::game::Action;
    use crate::item::{FoodKind, Item, ItemKind, ScrollKind};
    use crate::map::Map;
    use crate::monster::{Kind, Monster};
    use crate::player::{FOOD_MAX, Hunger, WEAK_AT};

    /// An empty 30x9 room with the player at (2,5).
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

    fn put_trap(game: &mut Game, pos: Point, kind: TrapKind, known: bool) {
        game.traps.push(Trap { pos, kind, known });
    }

    const EAST: Point = Point::new(1, 0);

    #[test]
    fn a_hidden_dart_trap_hurts_and_reveals_itself() {
        let mut game = room_game();
        let spot = Point::new(3, 5);
        put_trap(&mut game, spot, TrapKind::Dart, false);
        let hp = game.player.hp;
        game.apply(Action::Move(EAST));
        assert_eq!(game.player.pos, spot);
        assert!(game.player.hp < hp);
        assert!(game.known_trap_at(spot));
        assert_eq!(game.stats.damage_taken, (hp - game.player.hp) as u32);
        assert_eq!(game.stats.traps_sprung, 1);
    }

    #[test]
    fn a_known_trap_needs_a_second_step() {
        let mut game = room_game();
        let spot = Point::new(3, 5);
        put_trap(&mut game, spot, TrapKind::Alarm, true);
        game.apply(Action::Move(EAST));
        assert_eq!(game.player.pos, Point::new(2, 5), "warned, not moved");
        assert_eq!(game.turn, 0);
        game.apply(Action::Move(EAST));
        assert_eq!(game.player.pos, spot, "the second step goes through");

        // Doing anything else in between cancels the warning.
        let mut game = room_game();
        put_trap(&mut game, spot, TrapKind::Alarm, true);
        game.apply(Action::Move(EAST));
        game.apply(Action::Wait);
        game.apply(Action::Move(EAST));
        assert_eq!(game.player.pos, Point::new(2, 5));
    }

    #[test]
    fn alarms_wake_monsters_nearby() {
        let mut game = room_game();
        put_trap(&mut game, Point::new(3, 5), TrapKind::Alarm, false);
        game.monsters
            .push(Monster::new(Kind::Rat, Point::new(20, 2), Ai::Asleep));
        game.apply(Action::Move(EAST));
        assert!(matches!(game.monsters[0].ai, Ai::Hunting { .. }));
    }

    #[test]
    fn a_trapdoor_drops_you_a_floor() {
        let mut game = room_game();
        put_trap(&mut game, Point::new(3, 5), TrapKind::Trapdoor, false);
        game.apply(Action::Move(EAST));
        assert_eq!(game.depth, 2);
        assert!(game.map.tile(game.player.pos).is_walkable());
        assert_eq!(game.stats.trapdoor_falls, 1);
        assert_eq!(game.stats.stairs_taken, 0);
    }

    #[test]
    fn standing_near_a_trap_eventually_reveals_it() {
        let mut game = room_game();
        let spot = Point::new(3, 5);
        put_trap(&mut game, spot, TrapKind::Dart, false);
        for _ in 0..60 {
            game.apply(Action::Wait);
        }
        assert!(game.known_trap_at(spot), "15% a turn for 60 turns");
    }

    /// A trap that just came into view can be spotted on the same turn,
    /// and one hidden behind a wall can't.
    #[test]
    fn trap_spotting_uses_the_current_view() {
        // Player at (1,1) in a corridor; a closed door at (2,1) hides the
        // trap at (3,1). Opening the door brings it into view.
        let mut game = Game::new(1);
        let mut map = Map::new_filled(7, 3);
        map.carve_h_corridor(1, 5, 1);
        map.set_tile(Point::new(2, 1), crate::map::Tile::DoorClosed);
        game.place_on_map(map, Point::new(1, 1));
        game.monsters.clear();
        game.items.clear();
        game.traps.clear();
        game.update_fov();
        let spot = Point::new(3, 1);
        put_trap(&mut game, spot, TrapKind::Dart, false);
        assert!(!game.is_visible(spot));
        // Standing next to the door, the trap is 2 tiles away: out of
        // spotting range, so it only matters that nothing leaks through
        // the closed door.
        for _ in 0..30 {
            game.apply(Action::Wait);
        }
        assert!(!game.known_trap_at(spot), "spotted through a closed door");
    }

    #[test]
    fn magic_mapping_reveals_traps() {
        let mut game = room_game();
        put_trap(&mut game, Point::new(25, 8), TrapKind::Teleport, false);
        let kind = ItemKind::Scroll(ScrollKind::MagicMapping);
        let scroll = game.player.add_item(Item::new(kind)).unwrap();
        game.apply(Action::Read {
            scroll,
            target: None,
        });
        assert!(game.known_trap_at(Point::new(25, 8)));
    }

    #[test]
    fn hunger_warns_then_starves() {
        let mut game = room_game();
        game.player.food = WEAK_AT + 1;
        game.apply(Action::Wait);
        assert_eq!(game.player.hunger(), Hunger::Weak);
        assert!(game.log.iter().any(|m| m.text.contains("weak with hunger")));

        // Weak: no healing.
        game.player.hp = 10;
        for _ in 0..20 {
            game.apply(Action::Wait);
        }
        assert_eq!(game.player.hp, 10);

        // Starving: health drains away until death.
        game.player.food = 1;
        for _ in 0..200 {
            game.apply(Action::Wait);
        }
        let cause = game.death_summary().expect("starved");
        assert!(cause.starts_with("Killed by starvation"), "{cause}");
    }

    #[test]
    fn eating_restores_food_up_to_the_maximum() {
        let mut game = room_game();
        let ration = 'd'; // part of the starting kit
        assert_eq!(
            game.player.item(ration).map(|i| i.kind),
            Some(ItemKind::Food(FoodKind::Ration))
        );
        game.apply(Action::Eat(ration));
        assert!(
            game.log.last().unwrap().text.contains("too full"),
            "not hungry yet"
        );
        assert!(
            game.player.item(ration).is_some(),
            "the refused ration is kept"
        );
        game.player.food = 200;
        game.apply(Action::Eat(ration));
        // The turn spent eating uses 1 food too.
        assert_eq!(game.player.food, 200 + FoodKind::Ration.nutrition() - 1);
        assert!(game.player.item(ration).is_none());
        game.player.food = FOOD_MAX - 200;
        let jerky = game
            .player
            .add_item(Item::new(ItemKind::Food(FoodKind::Jerky)))
            .unwrap();
        game.apply(Action::Eat(jerky));
        assert!(game.player.food <= FOOD_MAX);
    }
}
