//! The bot: decides what to do next, playing like a careful beginner.
//!
//! It looks at the game the way a player does, using only what a player
//! could know: monsters in sight, tiles already explored, items as the
//! lore names them. It never peeks at hidden stats, unexplored map or
//! the true kind of an unidentified item. The pieces here serve three
//! features: auto-explore, travel to the stairs, and the full bot that
//! plays on its own (on screen or headless for balance testing).

use crate::game::{Action, Game};
use crate::geom::Point;
use crate::item::{Item, ItemKind, PotionKind, ScrollKind};
use crate::map::Tile;
use crate::monster::{Ai, Monster};
use crate::path;
use crate::player::{PACK_SIZE, RING_SLOTS};

/// Rest before exploring when below this percent of health.
const REST_BELOW: i32 = 80;
/// With a sleeping monster in view, rest up to this before moving on.
const REST_NEAR_SLEEPERS: i32 = 90;
/// Drink known healing at or below this percent when in danger.
const HEAL_BELOW: i32 = 40;
/// Only try unknown potions when at least this healthy.
const EXPERIMENT_ABOVE: i32 = 70;

/// The bot's next move. Always returns something; waiting is the
/// fallback when nothing better comes to mind.
pub fn next_action(game: &Game) -> Action {
    let pos = game.player.pos;
    let hp = health_percent(game);
    let visible = visible_monsters(game);
    // Only monsters actively after us are chased. A wandering one that
    // is close enough notices us next turn anyway (sight is symmetric)
    // and then comes to us; chasing wanderers can make the bot and the
    // monster step back and forth around a doorway forever.
    let hunting: Vec<&Monster> = visible
        .iter()
        .copied()
        .filter(|m| matches!(m.ai, Ai::Hunting { .. }))
        .collect();
    let asleep: Vec<&Monster> = visible
        .iter()
        .copied()
        .filter(|m| m.ai == Ai::Asleep)
        .collect();
    let adjacent: Vec<&Monster> = visible
        .iter()
        .copied()
        .filter(|m| m.pos.is_adjacent(pos))
        .collect();

    // Emergencies first.
    if hp <= HEAL_BELOW
        && (!hunting.is_empty() || hp <= 25)
        && let Some(letter) = known_item(game, ItemKind::Potion(PotionKind::Healing))
    {
        return Action::Drink(letter);
    }
    if hp <= 30
        && !adjacent.is_empty()
        && let Some(letter) = known_item(game, ItemKind::Scroll(ScrollKind::Teleportation))
    {
        return Action::Read {
            scroll: letter,
            target: None,
        };
    }

    // Fight whatever is next to us, weakest-looking first.
    if let Some(m) = adjacent.iter().min_by_key(|m| health_bar(m)) {
        return Action::Move(m.pos - pos);
    }
    if let Some(action) = step_toward_monster(game, &hunting) {
        return action;
    }

    // Nothing awake in sight: tidy up, heal, then carry on.
    if let Some(action) = improve_gear(game) {
        return action;
    }
    if let Some(action) = use_items(game, hp) {
        return action;
    }
    if hp < REST_BELOW {
        return Action::Wait;
    }
    // Sleeping monsters are left alone: walking up to one can hide it
    // behind a corner, and a bot that only reacts to what it sees would
    // then step back and forth forever. Rest up in case it wakes.
    if !asleep.is_empty() && hp < REST_NEAR_SLEEPERS {
        return Action::Wait;
    }
    // Descending comes before exploring: once the stairs are known and
    // reachable, head down. Short runs reward depth, not thoroughness.
    // Items along the way still get picked up by walking over them.
    if game.map.tile(pos) == Tile::StairsDown {
        return Action::Descend;
    }
    if let Some(action) = stairs_step(game) {
        return action;
    }
    if let Some(action) = explore_step(game) {
        return action;
    }
    // Nothing left to explore and no way to the stairs: a sleeping
    // monster must be blocking the way on. Only now go and fight it.
    step_toward_monster(game, &asleep).unwrap_or(Action::Wait)
}

/// One step of auto-explore: toward the nearest item worth picking up
/// or the nearest edge of the explored area. `None` once the floor is
/// fully explored (or the rest is out of reach).
pub fn explore_step(game: &Game) -> Option<Action> {
    // The path search never counts the starting tile as a goal, so an
    // item underfoot (say, left there when the pack was full) is
    // handled here first.
    if game
        .item_at(game.player.pos)
        .is_some_and(|fi| worth_picking_up(game, &fi.item))
    {
        return Some(Action::PickUp);
    }
    let is_goal = |p: Point| {
        let wanted = game.map.is_revealed(p)
            && game
                .item_at(p)
                .is_some_and(|fi| worth_picking_up(game, &fi.item));
        wanted || is_frontier(game, p)
    };
    step_to(game, is_goal)
}

/// Should the bot bother walking to this item? It skips things it knows
/// are harmful and gear that looks worse than what it has on, and, with
/// a full pack, anything that won't stack onto something carried.
fn worth_picking_up(game: &Game, item: &Item) -> bool {
    let p = &game.player;
    let stacks_onto_carried = item.kind.stacks() && p.inventory.iter().any(|i| i.kind == item.kind);
    if p.inventory.len() >= PACK_SIZE && !stacks_onto_carried {
        return false;
    }
    let known = game.lore.knows(item.kind);
    match item.kind {
        ItemKind::Potion(PotionKind::Decay) | ItemKind::Scroll(ScrollKind::Aggravate) => !known,
        ItemKind::Weapon(_) => weapon_score(game, Some(item)) > weapon_score(game, p.weapon()),
        ItemKind::Armor(_) => armor_score(Some(item)) > armor_score(p.armor()),
        _ => true,
    }
}

/// One step toward the stairs, if they've been seen and can be reached.
/// `None` when standing on them already.
pub fn stairs_step(game: &Game) -> Option<Action> {
    step_to(game, |p| {
        game.map.is_revealed(p) && game.map.tile(p) == Tile::StairsDown
    })
}

/// Why auto-explore or travel shouldn't start, if there is a reason.
pub fn auto_blocked(game: &Game) -> Option<String> {
    let m = visible_monsters(game)
        .into_iter()
        .find(|m| m.ai != Ai::Asleep)?;
    Some(format!("Not with the {} in view.", m.name()))
}

/// A snapshot taken before each automatic step, to notice anything
/// that should hand control back to the player.
pub struct Watch {
    hp: i32,
    pack: u32,
    /// Where visible monsters stood. Positions rather than a count, so a
    /// new monster appearing as another disappears is still noticed.
    monsters_seen: Vec<Point>,
}

impl Watch {
    pub fn new(game: &Game) -> Self {
        Self {
            hp: game.player.hp,
            pack: game.player.inventory.iter().map(|i| i.count).sum(),
            monsters_seen: visible_monsters(game).iter().map(|m| m.pos).collect(),
        }
    }

    /// Returns why to stop, or `None` to keep going. An empty reason
    /// means stop quietly: the game already logged what happened.
    pub fn reason_to_stop(&self, game: &Game) -> Option<String> {
        if game.death.is_some() {
            return Some(String::new());
        }
        if game.player.hp < self.hp {
            return Some("You are hurt!".to_string());
        }
        let visible = visible_monsters(game);
        if let Some(new) = visible
            .iter()
            .find(|m| !self.monsters_seen.contains(&m.pos))
        {
            return Some(format!("You see a {}.", new.name()));
        }
        if let Some(m) = visible.iter().find(|m| m.ai != Ai::Asleep) {
            return Some(format!("The {} is awake nearby.", m.name()));
        }
        let pack: u32 = game.player.inventory.iter().map(|i| i.count).sum();
        if pack != self.pack {
            return Some(String::new());
        }
        None
    }
}

// ---- Seeing the world as a player would ------------------------------

/// A monster's health as the sidebar shows it: a bar 18 cells wide,
/// not the exact number, which the player never sees.
fn health_bar(m: &Monster) -> i32 {
    let max = m.max_hp().max(1);
    (m.hp.max(0) * 18 + max - 1) / max
}

fn health_percent(game: &Game) -> i32 {
    game.player.hp * 100 / game.player.max_hp.max(1)
}

fn visible_monsters(game: &Game) -> Vec<&Monster> {
    game.monsters
        .iter()
        .filter(|m| game.is_visible(m.pos))
        .collect()
}

/// An explored tile you could walk onto with an unexplored tile
/// straight up, down, left or right of it.
///
/// Diagonal neighbors don't count. The outer corners of a room are
/// never in view from inside it, so counting them would make every room
/// corner look unexplored forever. Worse, a monster standing in such a
/// corner blocks the goal while in sight and unblocks it when out of
/// sight, and the bot would step back and forth between the two views.
/// Rooms and corridors join straight through walls and doors, never
/// only at corners, so nothing is missed.
fn is_frontier(game: &Game, p: Point) -> bool {
    const STRAIGHT: [Point; 4] = [
        Point::new(0, -1),
        Point::new(1, 0),
        Point::new(0, 1),
        Point::new(-1, 0),
    ];
    let map = &game.map;
    map.is_revealed(p)
        && map.tile(p).is_passable()
        && STRAIGHT.iter().any(|&d| {
            let n = p + d;
            map.in_bounds(n) && !map.is_revealed(n)
        })
}

/// Tiles the player knows they can walk through: explored, and open
/// or a door.
///
/// Monsters are deliberately ignored. Walking into one attacks it, so a
/// monster in the way just becomes a fight. Treating monster tiles as
/// blocked only while they are in view made routes flip each time a
/// monster slipped in or out of sight, and the bot would pace between
/// two tiles forever.
fn known_walkable(game: &Game, p: Point) -> bool {
    let tile = game.map.tile(p);
    game.map.is_revealed(p) && (tile.is_walkable() || tile == Tile::DoorClosed)
}

/// A move toward the nearest tile where `is_goal` holds.
fn step_to(game: &Game, is_goal: impl Fn(Point) -> bool) -> Option<Action> {
    let pos = game.player.pos;
    let map = &game.map;
    let step = path::first_step_to_any(
        pos,
        map.width(),
        map.height(),
        |p| known_walkable(game, p),
        is_goal,
    )?;
    Some(Action::Move(step - pos))
}

/// A move toward the nearest of `targets`, pathing around walls.
fn step_toward_monster(game: &Game, targets: &[&Monster]) -> Option<Action> {
    // Without this check, an empty list would search the whole map.
    if targets.is_empty() {
        return None;
    }
    let pos = game.player.pos;
    let map = &game.map;
    let is_target = |p: Point| targets.iter().any(|m| m.pos == p);
    let step = path::first_step_to_any(
        pos,
        map.width(),
        map.height(),
        |p| known_walkable(game, p) || is_target(p),
        is_target,
    )?;
    Some(Action::Move(step - pos))
}

/// The letter of a carried item of a kind the player has identified.
fn known_item(game: &Game, kind: ItemKind) -> Option<char> {
    if !game.lore.knows(kind) {
        return None;
    }
    game.player
        .inventory
        .iter()
        .find(|i| i.kind == kind)
        .map(|i| i.letter)
}

// ---- Gear ------------------------------------------------------------

/// The enchantment as far as the player knows: unknown counts as 0.
fn known_enchant(item: &Item) -> i32 {
    if item.known { item.enchant } else { 0 }
}

/// Rough expected damage per swing, scaled up to stay an integer.
fn weapon_score(game: &Game, item: Option<&Item>) -> i32 {
    let bonus = game.player.strength_bonus();
    let (low, high, accuracy) = match item.map(|i| (i.kind, known_enchant(i))) {
        Some((ItemKind::Weapon(w), e)) => {
            let s = w.stats();
            (s.damage.0 + e, s.damage.1 + e, s.accuracy + e)
        }
        _ => (1, 2, 0), // fists
    };
    (low + high + 2 * bonus).max(2) * (80 + 5 * accuracy)
}

/// Protection per hit counts double against the dodge it costs.
fn armor_score(item: Option<&Item>) -> i32 {
    match item.map(|i| (i.kind, known_enchant(i))) {
        Some((ItemKind::Armor(a), e)) => 6 * (a.stats().armor + e) + 3 * a.stats().dodge,
        _ => 0,
    }
}

/// Swaps in better weapons and armor, puts on rings, and takes off
/// rings known to be bad.
fn improve_gear(game: &Game) -> Option<Action> {
    let p = &game.player;

    if let Some(bad) = p
        .rings()
        .find(|r| r.known && r.enchant < 0 && !r.is_stuck())
    {
        return Some(Action::Equip(bad.letter));
    }
    if p.rings().count() < RING_SLOTS
        && let Some(ring) = p.inventory.iter().find(|i| {
            matches!(i.kind, ItemKind::Ring(_)) && !i.equipped && !(i.known && i.enchant <= 0)
        })
    {
        return Some(Action::Equip(ring.letter));
    }

    let weapon = p.weapon();
    if !weapon.is_some_and(Item::is_stuck) {
        let current = weapon_score(game, weapon);
        let better = p
            .inventory
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::Weapon(_)) && !i.equipped)
            .max_by_key(|i| weapon_score(game, Some(i)))
            .filter(|i| weapon_score(game, Some(i)) > current);
        if let Some(item) = better {
            return Some(Action::Equip(item.letter));
        }
    }

    let armor = p.armor();
    if !armor.is_some_and(Item::is_stuck) {
        let current = armor_score(armor);
        let better = p
            .inventory
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::Armor(_)) && !i.equipped)
            .max_by_key(|i| armor_score(Some(i)))
            .filter(|i| armor_score(Some(i)) > current);
        if let Some(item) = better {
            return Some(Action::Equip(item.letter));
        }
    }
    None
}

// ---- Potions and scrolls ---------------------------------------------

/// Uses helpful known items and experiments with unknown ones, but only
/// while nothing awake is in sight.
fn use_items(game: &Game, hp: i32) -> Option<Action> {
    let lore = &game.lore;
    // Scanning the map is slow-ish, so only do it when it matters.
    let still_exploring = || game.map.points().any(|p| is_frontier(game, p));
    for item in &game.player.inventory {
        let letter = item.letter;
        let known = lore.knows(item.kind);
        match item.kind {
            ItemKind::Potion(PotionKind::Strength | PotionKind::Life) if known => {
                return Some(Action::Drink(letter));
            }
            ItemKind::Potion(_) if !known && hp >= EXPERIMENT_ABOVE => {
                return Some(Action::Drink(letter));
            }
            ItemKind::Scroll(ScrollKind::MagicMapping) if known && still_exploring() => {
                return Some(read(letter, None));
            }
            ItemKind::Scroll(ScrollKind::Enchanting) if known => {
                if let Some(target) = enchant_target(game, letter) {
                    return Some(read(letter, Some(target)));
                }
            }
            ItemKind::Scroll(ScrollKind::Identify) if known => {
                if let Some(target) = first_read_target(game, letter) {
                    return Some(read(letter, Some(target)));
                }
            }
            ItemKind::Scroll(_) if !known => {
                // The target list is what the game would show on screen
                // after starting to read, so choosing from it is fair.
                let target = if game.scroll_needs_target(letter) {
                    first_read_target(game, letter)
                } else {
                    None
                };
                return Some(read(letter, target));
            }
            _ => {}
        }
    }
    None
}

fn read(scroll: char, target: Option<char>) -> Action {
    Action::Read { scroll, target }
}

fn first_read_target(game: &Game, scroll: char) -> Option<char> {
    game.player
        .inventory
        .iter()
        .find(|i| game.is_read_target(scroll, i))
        .map(|i| i.letter)
}

/// Enchant whatever is cursed first, then the weapon, then the armor,
/// then anything else the scroll accepts. `None` if nothing qualifies,
/// so the bot never tries a reading the game would refuse.
fn enchant_target(game: &Game, scroll: char) -> Option<char> {
    let p = &game.player;
    p.inventory
        .iter()
        .find(|i| i.is_stuck())
        .or(p.weapon())
        .or(p.armor())
        .or_else(|| p.inventory.iter().find(|i| i.kind.is_equipment()))
        .filter(|i| game.is_read_target(scroll, i))
        .map(|i| i.letter)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Auto-explore alone, with no monsters, must uncover every
    /// reachable tile and collect every item.
    #[test]
    fn exploring_uncovers_the_whole_floor() {
        for seed in 0..10 {
            let mut game = Game::new(seed);
            game.monsters.clear();
            let mut steps = 0;
            while let Some(action) = explore_step(&game) {
                game.apply(action);
                steps += 1;
                assert!(steps < 5_000, "seed {seed}: explore never finishes");
            }
            let unseen = game
                .map
                .points()
                .filter(|&p| game.map.tile(p).is_passable() && !game.map.is_revealed(p))
                .count();
            assert_eq!(unseen, 0, "seed {seed}");
            // Anything left must be something the bot chose to skip.
            for fi in &game.items {
                assert!(
                    !worth_picking_up(&game, &fi.item),
                    "seed {seed}: left behind {}",
                    game.lore.name(&fi.item)
                );
            }
        }
    }

    #[test]
    fn exploring_picks_up_an_item_underfoot() {
        use crate::item::{FloorItem, Item, PotionKind};
        let mut game = Game::new(1);
        game.monsters.clear();
        let pos = game.player.pos;
        let potion = Item::new(ItemKind::Potion(PotionKind::Healing));
        game.items.push(FloorItem { pos, item: potion });
        assert_eq!(explore_step(&game), Some(Action::PickUp));
        game.apply(Action::PickUp);
        assert!(game.item_at(pos).is_none());
    }

    #[test]
    fn travel_reaches_the_stairs() {
        let mut game = Game::new(3);
        game.monsters.clear();
        while let Some(action) = explore_step(&game) {
            game.apply(action);
        }
        let mut steps = 0;
        while let Some(action) = stairs_step(&game) {
            game.apply(action);
            steps += 1;
            assert!(steps < 500);
        }
        assert_eq!(game.map.tile(game.player.pos), Tile::StairsDown);
    }

    #[test]
    fn watch_notices_a_new_monster_even_when_another_leaves() {
        use crate::monster::{Kind, Monster};
        let mut game = Game::new(1);
        game.monsters.clear();
        let spot = |dx| game.player.pos + Point::new(dx, 0);
        let (a, b) = (spot(1), spot(-1));
        game.monsters.push(Monster::new(Kind::Rat, a, Ai::Asleep));
        let watch = Watch::new(&game);
        game.monsters[0].pos = b; // one seen monster gone, another here
        assert!(watch.reason_to_stop(&game).is_some());
    }

    #[test]
    fn the_bot_does_not_chase_wanderers_or_read_enchanting_without_a_target() {
        use crate::item::Item;
        use crate::monster::{Kind, Monster};
        let mut game = Game::new(1);
        game.monsters.clear();
        let near = game.player.pos + Point::new(2, 0);
        game.monsters
            .push(Monster::new(Kind::Rat, near, Ai::Wandering { goal: near }));
        game.update_fov();
        // Wanderers are ignored: the choice is the same without it.
        let with_wanderer = next_action(&game);
        let wanderer = game.monsters.pop().unwrap();
        assert_eq!(with_wanderer, next_action(&game), "chased a wanderer");
        game.monsters.push(wanderer);

        // No weapon, armor or other gear: enchanting has no target.
        game.monsters.clear();
        game.player.remove_item('a');
        game.player.remove_item('b');
        let kind = ItemKind::Scroll(ScrollKind::Enchanting);
        game.lore.learn(kind);
        game.player.add_item(Item::new(kind)).unwrap();
        for _ in 0..5 {
            let action = next_action(&game);
            assert!(!matches!(action, Action::Read { .. }), "{action:?}");
            game.apply(action);
        }
    }

    /// Once the stairs are in view, the bot goes for them rather than
    /// finishing the floor.
    #[test]
    fn the_bot_heads_down_as_soon_as_it_knows_the_way() {
        let mut game = Game::new(3);
        game.monsters.clear();
        let stairs = game
            .map
            .points()
            .find(|&p| game.map.tile(p) == Tile::StairsDown)
            .unwrap();
        // Show the bot the stairs, and nothing else of the floor.
        game.map.reveal(stairs);
        let mut steps = 0;
        while game.depth == 1 {
            let action = next_action(&game);
            if stairs_step(&game).is_some() {
                assert_eq!(Some(action), stairs_step(&game), "wandered off exploring");
            }
            game.apply(action);
            steps += 1;
            assert!(steps < 2_000, "never went down");
        }
    }

    /// A monster on the route, seen or not, must not change the route:
    /// that flip-flopping is what made the bot pace forever.
    #[test]
    fn routes_ignore_monsters_in_the_way() {
        use crate::monster::{Kind, Monster};
        let mut game = Game::new(2);
        game.monsters.clear();
        let before = explore_step(&game);
        let Some(Action::Move(step)) = before else {
            panic!("expected a step, got {before:?}");
        };
        let blocker = game.player.pos + step;
        game.monsters
            .push(Monster::new(Kind::Rat, blocker, Ai::Asleep));
        game.update_fov();
        assert!(game.is_visible(blocker));
        assert_eq!(explore_step(&game), before, "the route changed");
    }

    #[test]
    fn watch_stops_for_damage_and_new_monsters() {
        let mut game = Game::new(1);
        game.monsters.clear();
        let watch = Watch::new(&game);
        assert_eq!(watch.reason_to_stop(&game), None);
        game.player.hp -= 1;
        assert_eq!(
            watch.reason_to_stop(&game).as_deref(),
            Some("You are hurt!")
        );
    }

    /// The bot plays real runs and never gets stuck repeating an action
    /// that takes no time (which would freeze the game).
    #[test]
    fn the_bot_always_makes_progress() {
        for seed in 0..4 {
            let mut game = Game::new(seed);
            let mut idle = 0;
            while game.death.is_none() && game.turn < 1_500 {
                let turn = game.turn;
                game.apply(next_action(&game));
                idle = if game.turn == turn { idle + 1 } else { 0 };
                assert!(idle < 20, "seed {seed}: stuck at turn {turn}");
            }
            assert!(
                game.depth > 1 || game.death.is_some(),
                "seed {seed}: never left floor 1"
            );
        }
    }
}
