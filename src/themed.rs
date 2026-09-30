//! Themed rooms: one special room on some floors, dressed to suit its
//! zone. A crypt tomb of sleeping dead guarding treasure, a flooded
//! cistern where things lurk in the water, a warrens den of a sleeping
//! pack on its loot, or a larder of stolen food.
//!
//! A themed room is added after a floor's usual monsters and items,
//! so it is extra risk and extra reward on top of an ordinary floor.

use crate::dungeon::{self, Level};
use crate::geom::{Point, Rect};
use crate::item::{self, FloorItem, FoodKind, Item, ItemKind, ItemWeights};
use crate::map::Tile;
use crate::monster::{Ai, Kind, Monster};
use crate::rng::Rng;

/// Chance a floor gets a themed room, in percent.
pub const THEMED_PERCENT: i32 = 50;

/// Share of a cistern's floor that is under water, in percent.
const CISTERN_WATER_PERCENT: i32 = 65;

/// Rooms smaller than this many tiles are too cramped to theme.
const MIN_AREA: i32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    /// Sleeping undead guarding 1-2 items.
    Tomb,
    /// Mostly shallow water, with oozes and an item on a dry spot.
    Cistern,
    /// A sleeping pack on 2-3 items.
    Den,
    /// Food, with a sleeping guard.
    Larder,
}

/// Monsters that rest in tombs.
const UNDEAD: [Kind; 6] = [
    Kind::Zombie,
    Kind::Skeleton,
    Kind::Draugr,
    Kind::Ghoul,
    Kind::Wraith,
    Kind::Vampire,
];
/// Monsters that lurk in cisterns.
const OOZES: [Kind; 2] = [Kind::AcidMound, Kind::PinkJelly];
/// Monsters that keep dens, weakest first.
const PACKS: [Kind; 5] = [
    Kind::Jackal,
    Kind::Kobold,
    Kind::Goblin,
    Kind::Hobgoblin,
    Kind::Orc,
];

/// What's on a floor so far, which a themed room adds to.
pub struct Contents<'a> {
    pub monsters: &'a mut Vec<Monster>,
    pub items: &'a mut Vec<FloorItem>,
}

/// Maybe dresses one room of the floor in one of `themes`. Returns the
/// theme and room used, if any.
pub fn add_themed_room(
    rng: &mut Rng,
    level: &mut Level,
    depth: u32,
    themes: &[Theme],
    item_weights: &ItemWeights,
    contents: Contents,
) -> Option<(Theme, Rect)> {
    if themes.is_empty() || !rng.chance(THEMED_PERCENT) {
        return None;
    }
    let rooms: Vec<Rect> = level
        .rooms
        .iter()
        .copied()
        .filter(|&r| r != level.start_room && r.w * r.h >= MIN_AREA)
        .collect();
    if rooms.is_empty() {
        return None;
    }
    let room = rooms[rng.index(rooms.len())];
    let theme = themes[rng.index(themes.len())];
    let unlocked = |kinds: &[Kind]| -> Vec<Kind> {
        kinds
            .iter()
            .copied()
            .filter(|k| k.species().min_depth <= depth)
            .collect()
    };
    let mut room_items: Vec<Item> = Vec::new();
    let mut sleepers: Vec<Kind> = Vec::new();
    match theme {
        Theme::Tomb => {
            let dead = unlocked(&UNDEAD);
            for _ in 0..rng.range(2, 4) {
                if !dead.is_empty() {
                    sleepers.push(dead[rng.index(dead.len())]);
                }
            }
            for _ in 0..rng.range(1, 3) {
                room_items.push(item::random_item(rng, item_weights));
            }
        }
        Theme::Cistern => {
            flood(rng, level, room);
            let oozes = unlocked(&OOZES);
            for _ in 0..rng.range(1, 3) {
                if !oozes.is_empty() {
                    sleepers.push(oozes[rng.index(oozes.len())]);
                }
            }
            room_items.push(item::random_item(rng, item_weights));
        }
        Theme::Den => {
            // One kind of pack animal, the same kind for the whole den.
            let kinds = unlocked(&PACKS);
            let kind = kinds[rng.index(kinds.len())];
            sleepers.extend(std::iter::repeat_n(kind, rng.range(3, 5) as usize));
            for _ in 0..rng.range(2, 4) {
                room_items.push(item::random_item(rng, item_weights));
            }
        }
        Theme::Larder => {
            for _ in 0..rng.range(2, 4) {
                let food = if rng.chance(50) {
                    FoodKind::Ration
                } else {
                    FoodKind::Jerky
                };
                room_items.push(Item::new(ItemKind::Food(food)));
            }
            let guards = unlocked(&PACKS[1..3]);
            if !guards.is_empty() {
                sleepers.push(guards[rng.index(guards.len())]);
            }
        }
    }

    let Contents { monsters, items } = contents;
    for kind in sleepers {
        if let Some(p) = free_spot(rng, level, room, monsters, items, |t| t.is_walkable()) {
            monsters.push(Monster::at_depth(kind, p, Ai::Asleep, depth));
        }
    }
    for item in room_items {
        // Items go on dry floor, like everywhere else.
        if let Some(pos) = free_spot(rng, level, room, monsters, items, |t| t == Tile::Floor) {
            items.push(FloorItem { pos, item });
        }
    }
    Some((theme, room))
}

/// Turns most of a room's floor into shallow water. Doors, stairs and
/// anything already lying on the floor are left dry.
fn flood(rng: &mut Rng, level: &mut Level, room: Rect) {
    for y in room.y..room.y + room.h {
        for x in room.x..room.x + room.w {
            let p = Point::new(x, y);
            if level.map.tile(p) == Tile::Floor && rng.chance(CISTERN_WATER_PERCENT) {
                level.map.set_tile(p, Tile::Water);
            }
        }
    }
}

/// A random tile in `room` that suits `fits` and holds no monster or
/// item, after a fair number of tries.
fn free_spot(
    rng: &mut Rng,
    level: &Level,
    room: Rect,
    monsters: &[Monster],
    items: &[FloorItem],
    fits: impl Fn(Tile) -> bool,
) -> Option<Point> {
    (0..40).find_map(|_| {
        let p = dungeon::random_point_in(rng, room);
        let tile = level.map.tile(p);
        let free = fits(tile)
            && tile != Tile::StairsDown
            && monsters.iter().all(|m| m.pos != p)
            && items.iter().all(|i| i.pos != p);
        free.then_some(p)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dungeon::{STANDARD, generate};

    /// Tries seeds until the theme is placed, and returns what it did.
    fn themed(theme: Theme, depth: u32) -> (Level, Rect, Vec<Monster>, Vec<FloorItem>) {
        for seed in 0..200 {
            let mut rng = Rng::new(seed);
            let mut level = generate(&mut rng, &STANDARD);
            let (mut monsters, mut items) = (Vec::new(), Vec::new());
            let contents = Contents {
                monsters: &mut monsters,
                items: &mut items,
            };
            if let Some((t, room)) = add_themed_room(
                &mut rng,
                &mut level,
                depth,
                &[theme],
                &ItemWeights::STANDARD,
                contents,
            ) {
                assert_eq!(t, theme);
                return (level, room, monsters, items);
            }
        }
        panic!("{theme:?} never placed");
    }

    fn inside(room: Rect, p: Point) -> bool {
        p.x >= room.x && p.x < room.x + room.w && p.y >= room.y && p.y < room.y + room.h
    }

    #[test]
    fn a_tomb_holds_sleeping_undead_and_treasure() {
        let (level, room, monsters, items) = themed(Theme::Tomb, 10);
        assert_ne!(room, level.start_room);
        assert!((2..=3).contains(&monsters.len()));
        for m in &monsters {
            assert!(UNDEAD.contains(&m.kind) && m.ai == Ai::Asleep);
            assert!(inside(room, m.pos));
            assert!(m.kind.species().min_depth <= 10, "{:?} too early", m.kind);
        }
        assert!((1..=2).contains(&items.len()));
    }

    #[test]
    fn a_cistern_is_mostly_water_with_its_item_on_dry_land() {
        let (level, room, _, items) = themed(Theme::Cistern, 10);
        let water: Vec<Point> = level
            .map
            .points()
            .filter(|&p| level.map.tile(p) == Tile::Water)
            .collect();
        assert!(
            water.iter().all(|&p| inside(room, p)),
            "water only in the room"
        );
        assert!(water.len() as i32 >= room.w * room.h / 3);
        assert_eq!(items.len(), 1);
        assert_eq!(level.map.tile(items[0].pos), Tile::Floor);
    }

    #[test]
    fn a_den_is_one_kind_of_pack() {
        let (_, _, monsters, items) = themed(Theme::Den, 8);
        assert!((3..=4).contains(&monsters.len()));
        assert!(monsters.iter().all(|m| m.kind == monsters[0].kind));
        assert!((2..=3).contains(&items.len()));
    }

    #[test]
    fn a_larder_is_full_of_food() {
        let (_, _, _, items) = themed(Theme::Larder, 14);
        assert!(items.len() >= 2);
        assert!(
            items
                .iter()
                .all(|i| matches!(i.item.kind, ItemKind::Food(_)))
        );
    }

    #[test]
    fn early_tombs_only_hold_what_has_unlocked() {
        let (_, _, monsters, _) = themed(Theme::Tomb, 2);
        assert!(monsters.iter().all(|m| m.kind == Kind::Zombie));
    }
}
