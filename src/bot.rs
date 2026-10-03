//! The bot: decides what to do next, playing like a careful beginner.
//!
//! It looks at the game the way a player does, using only what a player
//! could know: monsters in sight, tiles already explored, items as the
//! lore names them. It never peeks at hidden stats, unexplored map or
//! the true kind of an unidentified item. The pieces here serve three
//! features: auto-explore, travel to the stairs, and the full bot that
//! plays on its own (on screen or headless for balance testing).

use crate::game::{Action, Game};
use crate::geom::{DIRECTIONS_8, Point};
use crate::inventory::HEALING;
use crate::item::{FoodKind, Item, ItemKind, PotionKind, ScrollKind};
use crate::map::Tile;
use crate::monster::{Ability, Ai, Monster};
use crate::path;
use crate::player::{Hunger, PACK_SIZE, RING_SLOTS};

/// Rest before exploring when below this percent of health.
const REST_BELOW: i32 = 80;
/// With a sleeping monster in view, rest up to this before moving on.
const REST_NEAR_SLEEPERS: i32 = 90;
/// Drink known healing at or below this percent when in danger.
const HEAL_BELOW: i32 = 40;

/// From this depth on, monsters hit hard enough that the bot rests to
/// full before moving on, as a player with `R` does. On 1,000 seeds
/// this took the median depth from 22 to 28. Drinking healing earlier
/// too (at 55%) made no difference.
const DEEP_FROM: u32 = 13;

/// Health percent to rest up to before moving on, deeper floors asking
/// for more.
fn rest_below(game: &Game) -> i32 {
    if game.depth >= DEEP_FROM {
        100
    } else {
        REST_BELOW
    }
}
/// Only try unknown potions when at least this healthy.
const EXPERIMENT_ABOVE: i32 = 70;

/// Turns an escape plan may run before the bot gives up on it.
const ESCAPE_BUDGET: u16 = 100;
/// Hurt at or below this percent, the bot runs for the stairs if
/// nothing hunting it is faster than the player.
const ESCAPE_BELOW: i32 = 50;

/// How far, in steps, the bot will go out of its way for an item it
/// has seen before taking the stairs.
const FETCH_REACH: usize = 15;

/// How far the bot will back off to a corridor to fight a splitter.
const HOLD_REACH: usize = 8;
/// Turns a hold may last, so it can't wait forever on a jelly that
/// never comes.
const HOLD_BUDGET: u16 = 60;

/// Everything the bot remembers between turns: at most one plan.
///
/// A plan is a decision the bot sticks to even as monsters move in and
/// out of view. Deciding afresh every turn from what is in sight is what
/// made earlier versions pace back and forth forever, so plans stick,
/// but always end: on arrival, on running out of turns, when no route
/// is left, or on a new floor.
#[derive(Clone, Debug, Default)]
pub struct BotMemory {
    plan: Option<Plan>,
    /// The floor the memory belongs to.
    depth: u32,
    /// Items this floor the bot set out to fetch and didn't get to, so
    /// it doesn't keep setting out for them.
    gave_up_on: Vec<Point>,
    /// A hold has been tried on this floor. Only one is allowed: a
    /// jelly that drops in and out of view would otherwise start and
    /// end holds forever, the bot stepping back and forth between them.
    held: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Plan {
    purpose: Purpose,
    goal: Point,
    turns_left: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    /// Get to the stairs and take them, leaving a fight behind.
    Escape,
    /// Go and pick up an item already seen, before heading down.
    Fetch,
    /// Back into a corridor and fight a splitter there, where its
    /// halves can only come at the player one or two at a time.
    Hold,
}

/// The bot's next move. Always returns something; waiting is the
/// fallback when nothing better comes to mind.
pub fn next_action(game: &Game, memory: &mut BotMemory) -> Action {
    if memory.depth != game.depth {
        *memory = BotMemory {
            depth: game.depth,
            ..BotMemory::default()
        };
    }
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

    // Emergencies first. How bad could the next turn be? The worst
    // hits of everything adjacent, plus fast hunters that can close in
    // and strike this turn. A player can judge this from what the
    // monsters are and how deep they are.
    let danger: i32 = visible
        .iter()
        .map(|m| {
            let d = m.pos - pos;
            let reach = d.x.abs().max(d.y.abs());
            // Actions it may get this turn: 2 for anything faster than
            // the player. One is spent closing in from two tiles away.
            let actions = (m.species().speed + 99) / 100;
            let attacks = match reach {
                1 => actions,
                2 if matches!(m.ai, Ai::Hunting { .. }) => actions - 1,
                _ => 0,
            };
            attacks * m.attack().damage.1
        })
        .sum();
    let hp_now = game.player.hp;
    let could_die = danger > 0 && hp_now <= danger;
    let threatened = !hunting.is_empty() || !adjacent.is_empty();
    let hurt = hp <= HEAL_BELOW && (threatened || hp <= 25);
    if could_die || hurt {
        let potion = |kind| known_item(game, ItemKind::Potion(kind));
        let teleport = known_item(game, ItemKind::Scroll(ScrollKind::Teleportation));
        // Surrounded, or a healing potion won't cover the worst case:
        // leave instead of prolonging a fight that can't be won.
        // Healing can't go past full health.
        let healed = (hp_now + HEALING).min(game.player.max_hp);
        let healing_falls_short = healed <= danger;
        if (adjacent.len() >= 2 || (could_die && healing_falls_short))
            && let Some(letter) = teleport
        {
            return read(letter, None);
        }
        if let Some(letter) = potion(PotionKind::Healing) {
            return Action::Drink(letter);
        }
        // A potion of life heals fully; it's saved for moments like this.
        if let Some(letter) = potion(PotionKind::Life) {
            return Action::Drink(letter);
        }
        if could_die && let Some(letter) = teleport {
            return read(letter, None);
        }
    }

    // Last resort: badly hurt, cornered, and out of known healing and
    // teleports. An unknown scroll might be teleportation.
    if hp <= 20
        && !adjacent.is_empty()
        && let Some(scroll) = game
            .player
            .inventory
            .iter()
            .find(|i| matches!(i.kind, ItemKind::Scroll(_)) && !game.lore.knows(i.kind))
    {
        return read_unknown(game, scroll.letter);
    }

    // On the stairs with trouble near: leave. Taking the stairs ends the
    // turn on a new floor, so the monsters here get no parting blow.
    let on_stairs = game.map.tile(pos) == Tile::StairsDown;
    if on_stairs && (!adjacent.is_empty() || !hunting.is_empty()) {
        return Action::Descend;
    }

    // Follow through on an escape, or decide to start one. A fetch or a
    // hold is only a detour, so an escape can cut it short.
    if memory.plan.is_none_or(|p| p.purpose != Purpose::Escape)
        && let Some(escape) = consider_escape(game, &hunting, hp)
    {
        memory.plan = Some(escape);
    }
    if memory.plan.is_none_or(|p| p.purpose == Purpose::Fetch)
        && !memory.held
        && let Some(hold) = consider_hold(game, &hunting)
    {
        memory.plan = Some(hold);
        memory.held = true;
    }
    if let Some(action) = follow_plan(game, memory) {
        return action;
    }

    // Facing what a carried artifact was made for: take it up first.
    if let Some(action) = wield_slayer(game, &hunting, &adjacent) {
        return action;
    }

    // Fight whatever is next to us, weakest-looking first.
    if let Some(m) = adjacent.iter().min_by_key(|m| m.health_bar()) {
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
    // Resting burns food, so only rest with food to spare.
    let can_rest = game.player.hunger() == Hunger::Fed || has_food(game);
    if hp < rest_below(game) && can_rest {
        return Action::Wait;
    }
    // Sleeping monsters are left alone: walking up to one can hide it
    // behind a corner, and a bot that only reacts to what it sees would
    // then step back and forth forever. Rest up in case it wakes.
    // (Moving on instead, to avoid giving sleepers more chances to wake,
    // measured worse: median depth 15 to 14 on the same 100 seeds.)
    if !asleep.is_empty() && hp < REST_NEAR_SLEEPERS.max(rest_below(game)) && can_rest {
        return Action::Wait;
    }
    // Descending comes before exploring: once the stairs are known and
    // reachable, head down. Short runs reward depth, not thoroughness.
    // Items along the way still get picked up by walking over them, and
    // ones seen nearby are fetched first.
    // With auto pickup off, walking over an item leaves it there, so
    // the bot picks it up itself.
    if !game.auto_pickup && item_underfoot_wanted(game) {
        return Action::PickUp;
    }
    if known_stairs(game).is_some() && memory.plan.is_none() {
        memory.plan = consider_fetch(game, &memory.gave_up_on);
        if let Some(action) = follow_plan(game, memory) {
            return action;
        }
    }
    if game.map.tile(pos) == Tile::StairsDown {
        return Action::Descend;
    }
    if let Some(action) = stairs_step(game) {
        return action;
    }
    if let Some(action) = explore(game, true) {
        return action;
    }
    // Nothing left to explore and no way to the stairs: a sleeping
    // monster must be blocking the way on. Only now go and fight it.
    step_toward_monster(game, &asleep).unwrap_or(Action::Wait)
}

/// Where the stairs are, if the player has seen them.
fn known_stairs(game: &Game) -> Option<Point> {
    game.map
        .points()
        .find(|&p| game.map.is_revealed(p) && game.map.tile(p) == Tile::StairsDown)
}

/// Decides whether to run for the stairs: when hurt, and nothing
/// hunting is faster than the player. A monster at the player's speed
/// that follows spends its actions moving, not attacking, so reaching
/// the stairs costs nothing.
///
/// Also walking away from slow monsters (zombies, golems, ogres) looked
/// good on 100 tuning seeds but lost on 200 fresh ones (47 runs deeper,
/// 78 shallower): skipping those fights costs experience needed later.
fn consider_escape(game: &Game, hunting: &[&Monster], hp: i32) -> Option<Plan> {
    let none_faster = hunting.iter().all(|m| m.species().speed <= 100);
    if hunting.is_empty() || hp > ESCAPE_BELOW || !none_faster {
        return None;
    }
    let goal = known_stairs(game)?;
    step_to(game, |p| p == goal)?;
    Some(Plan {
        purpose: Purpose::Escape,
        goal,
        turns_left: ESCAPE_BUDGET,
    })
}

/// Whether a splitter is hunting the player: a pink jelly, whose every
/// hit that doesn't kill makes another.
fn splitter_hunting(hunting: &[&Monster]) -> bool {
    hunting.iter().any(|m| m.species().has(Ability::Splits))
}

/// A corridor tile: at most two of its neighbors are open, so at most
/// two monsters can reach whoever stands there. Unexplored neighbors
/// count as open, since they might be.
fn is_chokepoint(game: &Game, p: Point) -> bool {
    let open = DIRECTIONS_8
        .iter()
        .map(|&d| p + d)
        .filter(|&n| !game.map.is_revealed(n) || game.map.tile(n).is_walkable())
        .count();
    open <= 2
}

/// Decides whether to back into a corridor: a splitter is hunting, the
/// player is out in the open, and a corridor is within `HOLD_REACH`.
/// Measured on its own, a hold for groups of any kind was a wash; a
/// jelly is different, since in a room it can surround the player with
/// its own halves.
fn consider_hold(game: &Game, hunting: &[&Monster]) -> Option<Plan> {
    let pos = game.player.pos;
    if !splitter_hunting(hunting) || is_chokepoint(game, pos) {
        return None;
    }
    let map = &game.map;
    let (goal, _) = path::distances(pos, map.width(), map.height(), HOLD_REACH, |p| {
        known_walkable(game, p) && !game.known_trap_at(p)
    })
    .into_iter()
    .find(|&(p, _)| map.tile(p).is_walkable() && is_chokepoint(game, p))?;
    Some(Plan {
        purpose: Purpose::Hold,
        goal,
        turns_left: HOLD_BUDGET,
    })
}

/// Decides whether to go back for an item: the nearest one worth
/// picking up that has been seen within `FETCH_REACH` steps.
fn consider_fetch(game: &Game, gave_up_on: &[Point]) -> Option<Plan> {
    let map = &game.map;
    let (goal, steps) = path::distances(
        game.player.pos,
        map.width(),
        map.height(),
        FETCH_REACH,
        |p| known_walkable(game, p),
    )
    .into_iter()
    .find(|&(p, _)| worth_fetching(game, p) && !gave_up_on.contains(&p))?;
    Some(Plan {
        purpose: Purpose::Fetch,
        goal,
        // Room to fight something met on the way.
        turns_left: steps as u16 * 2 + 10,
    })
}

fn worth_fetching(game: &Game, p: Point) -> bool {
    game.map.is_revealed(p)
        && game
            .item_at(p)
            .is_some_and(|fi| worth_picking_up(game, &fi.item))
}

/// One step of the current plan, clearing it once it's over.
fn follow_plan(game: &Game, memory: &mut BotMemory) -> Option<Action> {
    let plan = memory.plan.as_mut()?;
    if plan.turns_left == 0 {
        give_up(memory);
        return None;
    }
    plan.turns_left -= 1;
    match plan.purpose {
        Purpose::Escape => {
            if game.player.pos == plan.goal {
                memory.plan = None;
                return Some(Action::Descend);
            }
            // Routes ignore monsters, so one in the way gets attacked.
            let goal = plan.goal;
            let step = step_to(game, |p| p == goal);
            if step.is_none() {
                memory.plan = None;
            }
            step
        }
        Purpose::Fetch => {
            // Fights and resting come first; the plan waits (while its
            // turns still run down). Only monsters the bot would fight
            // count: waiting on a wanderer or a fleeing thief at the
            // edge of view would have the bot step in and out of sight
            // of it forever.
            let pos = game.player.pos;
            let fight = visible_monsters(game)
                .iter()
                .any(|m| matches!(m.ai, Ai::Hunting { .. }) || m.pos.is_adjacent(pos));
            if fight || health_percent(game) < rest_below(game) {
                return None;
            }
            let goal = plan.goal;
            if !worth_fetching(game, goal) {
                memory.plan = None;
                return None;
            }
            if game.player.pos == goal {
                memory.plan = None;
                return Some(Action::PickUp);
            }
            let step = step_to(game, |p| p == goal);
            if step.is_none() {
                give_up(memory);
            }
            step
        }
        Purpose::Hold => {
            let pos = game.player.pos;
            let visible = visible_monsters(game);
            let hunting: Vec<&Monster> = visible
                .iter()
                .copied()
                .filter(|m| matches!(m.ai, Ai::Hunting { .. }))
                .collect();
            if !splitter_hunting(&hunting) {
                memory.plan = None;
                return None;
            }
            if pos != plan.goal {
                let goal = plan.goal;
                let step = step_to(game, |p| p == goal);
                if step.is_none() {
                    memory.plan = None;
                }
                return step;
            }
            // In place: fight what comes, and let the rest come to us.
            if visible.iter().any(|m| m.pos.is_adjacent(pos)) {
                None
            } else {
                Some(Action::Wait)
            }
        }
    }
}

/// Clears a plan that didn't reach its goal. An item that couldn't be
/// fetched is not tried again on this floor.
fn give_up(memory: &mut BotMemory) {
    if let Some(plan) = memory.plan.take()
        && plan.purpose == Purpose::Fetch
    {
        memory.gave_up_on.push(plan.goal);
    }
}

/// One step of auto-explore: toward the nearest item worth picking up
/// or the nearest edge of the explored area. `None` once the floor is
/// fully explored (or the rest is out of reach). With auto pickup off,
/// items are left to the player.
pub fn explore_step(game: &Game) -> Option<Action> {
    explore(game, game.auto_pickup)
}

/// Exploring, collecting items on the way if `collect` is set. The bot
/// always collects, whatever the player's auto pickup setting.
fn explore(game: &Game, collect: bool) -> Option<Action> {
    // The path search never counts the starting tile as a goal, so an
    // item underfoot (say, left there when the pack was full) is
    // handled here first.
    if collect && item_underfoot_wanted(game) {
        return Some(Action::PickUp);
    }
    let is_goal = |p: Point| {
        let wanted = collect
            && game.map.is_revealed(p)
            && game
                .item_at(p)
                .is_some_and(|fi| worth_picking_up(game, &fi.item));
        wanted || is_frontier(game, p)
    };
    step_to(game, is_goal)
}

fn item_underfoot_wanted(game: &Game) -> bool {
    game.item_at(game.player.pos)
        .is_some_and(|fi| worth_picking_up(game, &fi.item))
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
        // Made for the monsters that are hardest to kill: always kept.
        ItemKind::Weapon(w) if w.is_artifact() => true,
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

/// One turn of resting until healed: wait, or `None` at full health.
pub fn rest_step(game: &Game) -> Option<Action> {
    (game.player.hp < game.player.max_hp).then_some(Action::Wait)
}

/// Why resting shouldn't start, if there is a reason: the same as for
/// exploring, plus already being healed or too hungry to heal.
pub fn rest_blocked(game: &Game) -> Option<String> {
    if game.player.hp >= game.player.max_hp {
        return Some("You are already at full health.".to_string());
    }
    if game.player.hunger() >= Hunger::Weak {
        return Some("You are too hungry to heal by resting. Eat first.".to_string());
    }
    auto_blocked(game)
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
    hunger: Hunger,
    pack: u32,
    /// Where visible monsters stood. Positions rather than a count, so a
    /// new monster appearing as another disappears is still noticed.
    monsters_seen: Vec<Point>,
}

impl Watch {
    pub fn new(game: &Game) -> Self {
        Self {
            hp: game.player.hp,
            hunger: game.player.hunger(),
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
        // The only way on crosses a known trap. The game has just asked
        // for confirmation; that choice belongs to the player.
        if game.trap_warning.is_some() {
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
        // Getting hungrier: the game has logged it, and the player may
        // want to eat.
        if game.player.hunger() != self.hunger {
            return Some(String::new());
        }
        let pack: u32 = game.player.inventory.iter().map(|i| i.count).sum();
        if pack != self.pack {
            return Some(String::new());
        }
        None
    }
}

// ---- Seeing the world as a player would ------------------------------

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

fn has_food(game: &Game) -> bool {
    game.player
        .inventory
        .iter()
        .any(|i| matches!(i.kind, ItemKind::Food(_)))
}

/// A move toward the nearest tile where `is_goal` holds.
///
/// Known traps are avoided when there is another way; if a trap is the
/// only way through, the route crosses it (the game asks for the step
/// twice, and the bot simply repeats it).
fn step_to(game: &Game, is_goal: impl Fn(Point) -> bool) -> Option<Action> {
    let pos = game.player.pos;
    let map = &game.map;
    let search = |avoid_traps: bool| {
        path::first_step_to_any(
            pos,
            map.width(),
            map.height(),
            |p| known_walkable(game, p) && !(avoid_traps && game.known_trap_at(p)),
            &is_goal,
        )
    };
    let step = search(true).or_else(|| search(false))?;
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

/// Equips a carried artifact that slays a monster hunting us or next to
/// us, unless the weapon in hand already does. Once the fight is over,
/// `improve_gear` goes back to the best everyday weapon.
fn wield_slayer(game: &Game, hunting: &[&Monster], adjacent: &[&Monster]) -> Option<Action> {
    let p = &game.player;
    let slays =
        |item: &Item, m: &Monster| matches!(item.kind, ItemKind::Weapon(w) if w.slays(m.kind));
    let foes = || hunting.iter().chain(adjacent);
    let wielded = p.weapon();
    if wielded.is_some_and(Item::is_stuck) || wielded.is_some_and(|w| foes().any(|m| slays(w, m))) {
        return None;
    }
    p.inventory
        .iter()
        .find(|i| !i.equipped && foes().any(|m| slays(i, m)))
        .map(|i| Action::Equip(i.letter))
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
    // Eat once hungry. Jerky first: save the big rations for later.
    if game.player.hunger() >= Hunger::Hungry {
        let food = |kind| {
            game.player
                .inventory
                .iter()
                .find(|i| i.kind == ItemKind::Food(kind))
        };
        if let Some(item) = food(FoodKind::Jerky).or_else(|| food(FoodKind::Ration)) {
            return Some(Action::Eat(item.letter));
        }
    }
    let lore = &game.lore;
    // Scanning the map is slow-ish, so only do it when it matters.
    let still_exploring = || game.map.points().any(|p| is_frontier(game, p));
    for item in &game.player.inventory {
        let letter = item.letter;
        let known = lore.knows(item.kind);
        match item.kind {
            ItemKind::Potion(PotionKind::Strength) if known => {
                return Some(Action::Drink(letter));
            }
            // Life is kept for emergencies, or used in a quiet moment
            // when badly hurt.
            ItemKind::Potion(PotionKind::Life) if known && hp <= 50 => {
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
            // Unknown scrolls only with nothing at all in sight, sleeping
            // monsters included: one might be aggravate monsters.
            ItemKind::Scroll(_)
                if !known && hp >= EXPERIMENT_ABOVE && visible_monsters(game).is_empty() =>
            {
                return Some(read_unknown(game, letter));
            }
            _ => {}
        }
    }
    None
}

/// Reads an unknown scroll. The target list is what the game would show
/// on screen after starting to read, so choosing from it is fair.
fn read_unknown(game: &Game, letter: char) -> Action {
    let target = if game.scroll_needs_target(letter) {
        first_read_target(game, letter)
    } else {
        None
    };
    read(letter, target)
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
    fn with_auto_pickup_off_explore_leaves_items_but_the_bot_does_not() {
        use crate::item::{FloorItem, Item, PotionKind};
        let mut game = Game::new(1);
        game.monsters.clear();
        game.apply(Action::AutoPickup(false));
        let pos = game.player.pos;
        let potion = Item::new(ItemKind::Potion(PotionKind::Healing));
        game.items.push(FloorItem { pos, item: potion });
        assert_ne!(explore_step(&game), Some(Action::PickUp));
        let mut memory = BotMemory::default();
        assert_eq!(next_action(&game, &mut memory), Action::PickUp);
    }

    #[test]
    fn auto_explore_with_pickup_off_still_explores_everything() {
        let mut game = Game::new(2);
        game.monsters.clear();
        game.apply(Action::AutoPickup(false));
        let items = game.items.len();
        let mut steps = 0;
        while let Some(action) = explore_step(&game) {
            game.apply(action);
            steps += 1;
            assert!(steps < 5000);
        }
        assert_eq!(game.items.len(), items, "nothing was picked up");
        let unseen = game
            .map
            .points()
            .filter(|&p| game.map.tile(p).is_passable() && !game.map.is_revealed(p))
            .count();
        assert_eq!(unseen, 0);
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
        let with_wanderer = next_action(&game, &mut BotMemory::default());
        let wanderer = game.monsters.pop().unwrap();
        assert_eq!(
            with_wanderer,
            next_action(&game, &mut BotMemory::default()),
            "chased a wanderer"
        );
        game.monsters.push(wanderer);

        // No weapon, armor or other gear: enchanting has no target.
        game.monsters.clear();
        game.player.remove_item('a');
        game.player.remove_item('b');
        let kind = ItemKind::Scroll(ScrollKind::Enchanting);
        game.lore.learn(kind);
        game.player.add_item(Item::new(kind)).unwrap();
        for _ in 0..5 {
            let action = next_action(&game, &mut BotMemory::default());
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
        // Items nearby would be fetched first, which is fine but not
        // what this test is about.
        game.items.clear();
        let stairs = game
            .map
            .points()
            .find(|&p| game.map.tile(p) == Tile::StairsDown)
            .unwrap();
        // Show the bot the stairs, and nothing else of the floor.
        game.map.reveal(stairs);
        let mut steps = 0;
        while game.depth == 1 {
            let action = next_action(&game, &mut BotMemory::default());
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
    fn the_bot_eats_when_hungry_and_routes_around_known_traps() {
        use crate::trap::{Trap, TrapKind};
        let mut game = Game::new(4);
        game.monsters.clear();
        game.player.food = crate::player::HUNGRY_AT;
        assert!(matches!(
            next_action(&game, &mut BotMemory::default()),
            Action::Eat(_)
        ));
        game.player.food = crate::player::FOOD_START;

        // Put a known trap on the bot's next step: it should go round.
        let Some(Action::Move(step)) = explore_step(&game) else {
            panic!("expected a step");
        };
        let spot = game.player.pos + step;
        game.traps.push(Trap {
            pos: spot,
            kind: TrapKind::Dart,
            known: true,
        });
        if let Some(Action::Move(new_step)) = explore_step(&game) {
            assert_ne!(game.player.pos + new_step, spot);
        }
    }

    #[test]
    fn auto_moves_stop_at_a_trap_warning() {
        let mut game = Game::new(1);
        game.monsters.clear();
        let watch = Watch::new(&game);
        game.trap_warning = Some(game.player.pos + Point::new(1, 0));
        assert!(watch.reason_to_stop(&game).is_some());
    }

    /// A bot in an empty 20x9 room at (2,5), with nothing else around.
    fn quiet_room() -> Game {
        let mut game = Game::new(1);
        let mut map = crate::map::Map::new_filled(22, 11);
        map.carve_room(1, 1, 20, 9);
        game.place_on_map(map, Point::new(2, 5));
        game.monsters.clear();
        game.items.clear();
        game.traps.clear();
        game.update_fov();
        game
    }

    fn hunter_at(game: &Game, kind: crate::monster::Kind, pos: Point) -> Monster {
        Monster::new(
            kind,
            pos,
            Ai::Hunting {
                last_seen: game.player.pos,
            },
        )
    }

    #[test]
    fn on_the_stairs_with_trouble_near_it_goes_down() {
        let mut game = quiet_room();
        game.map.set_tile(game.player.pos, Tile::StairsDown);
        let orc = hunter_at(&game, crate::monster::Kind::Orc, Point::new(3, 5));
        game.monsters.push(orc);
        game.update_fov();
        assert_eq!(
            next_action(&game, &mut BotMemory::default()),
            Action::Descend
        );
    }

    /// A quiet room with seen stairs at the far end, (19,5).
    fn room_with_stairs() -> (Game, Point) {
        let mut game = quiet_room();
        let stairs = Point::new(19, 5);
        game.map.set_tile(stairs, Tile::StairsDown);
        game.map.reveal(stairs);
        for p in game.map.points().collect::<Vec<_>>() {
            game.map.reveal(p);
        }
        (game, stairs)
    }

    fn drop_at(game: &mut Game, kind: ItemKind, pos: Point) {
        game.items.push(crate::item::FloorItem {
            pos,
            item: Item::new(kind),
        });
    }

    #[test]
    fn it_fetches_a_nearby_item_before_heading_down() {
        let (mut game, _) = room_with_stairs();
        let ration = Point::new(4, 8);
        drop_at(&mut game, ItemKind::Food(FoodKind::Ration), ration);
        let mut memory = BotMemory::default();
        for _ in 0..10 {
            let action = next_action(&game, &mut memory);
            if game.player.pos == ration {
                assert_eq!(action, Action::PickUp);
                game.apply(action);
                assert!(game.item_at(ration).is_none());
                // Then straight on down.
                assert_eq!(next_action(&game, &mut memory), stairs_step(&game).unwrap());
                return;
            }
            let Action::Move(d) = action else {
                panic!("expected a step, got {action:?}");
            };
            game.player.pos = game.player.pos + d;
            game.update_fov();
        }
        panic!("never reached the ration");
    }

    #[test]
    fn items_out_of_reach_are_left_behind() {
        let (mut game, _) = room_with_stairs();
        game.player.pos = Point::new(18, 5);
        game.update_fov();
        drop_at(
            &mut game,
            ItemKind::Food(FoodKind::Ration),
            Point::new(2, 5),
        );
        let mut memory = BotMemory::default();
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(1, 0))
        );
        assert!(memory.plan.is_none());
    }

    #[test]
    fn a_fetch_waits_while_a_monster_is_about() {
        use crate::monster::Kind;
        let (mut game, _) = room_with_stairs();
        drop_at(
            &mut game,
            ItemKind::Food(FoodKind::Ration),
            Point::new(2, 8),
        );
        let mut memory = BotMemory::default();
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(0, 1))
        );
        let orc = hunter_at(&game, Kind::Orc, Point::new(5, 5));
        game.monsters.push(orc);
        game.update_fov();
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(1, 0)),
            "goes for the orc"
        );
        assert!(memory.plan.is_some(), "the fetch is kept for later");
    }

    /// Seed 1076 once stalled on a floor: a fleeing monkey at the edge
    /// of view paused the fetch, the bot turned for the stairs, lost
    /// sight of the monkey, resumed the fetch, and so on forever.
    #[test]
    fn a_fleeing_monster_does_not_pause_a_fetch() {
        use crate::monster::Kind;
        let (mut game, _) = room_with_stairs();
        drop_at(
            &mut game,
            ItemKind::Food(FoodKind::Ration),
            Point::new(2, 8),
        );
        game.monsters
            .push(Monster::new(Kind::Monkey, Point::new(15, 2), Ai::Fleeing));
        game.update_fov();
        let mut memory = BotMemory::default();
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(0, 1))
        );
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(0, 1)),
            "kept going for the ration"
        );
    }

    #[test]
    fn a_fetch_that_runs_out_of_turns_is_not_restarted() {
        let (mut game, _) = room_with_stairs();
        let ration = Point::new(2, 8);
        drop_at(&mut game, ItemKind::Food(FoodKind::Ration), ration);
        let mut memory = BotMemory::default();
        next_action(&game, &mut memory);
        memory.plan.as_mut().unwrap().turns_left = 0;
        assert_eq!(
            next_action(&game, &mut memory),
            stairs_step(&game).unwrap(),
            "gives up and heads down"
        );
        assert_eq!(memory.gave_up_on, vec![ration]);
        assert!(memory.plan.is_none());
    }

    #[test]
    fn an_escape_cuts_a_fetch_short() {
        use crate::monster::Kind;
        let (mut game, _) = room_with_stairs();
        drop_at(
            &mut game,
            ItemKind::Food(FoodKind::Ration),
            Point::new(2, 8),
        );
        let mut memory = BotMemory::default();
        next_action(&game, &mut memory);
        assert_eq!(memory.plan.unwrap().purpose, Purpose::Fetch);
        // A fight goes badly: time to leave, ration or not.
        let orc = hunter_at(&game, Kind::Orc, Point::new(1, 5)); // speed 100
        game.monsters.push(orc);
        game.update_fov();
        game.player.hp = game.player.max_hp / 3;
        game.player.inventory.retain(|i| i.equipped);
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(1, 0))
        );
        assert_eq!(memory.plan.unwrap().purpose, Purpose::Escape);
    }

    #[test]
    fn hurt_it_runs_for_the_stairs_and_keeps_running() {
        use crate::monster::Kind;
        let (mut game, stairs) = room_with_stairs();
        let orc = hunter_at(&game, Kind::Orc, Point::new(1, 5)); // speed 100
        game.monsters.push(orc);
        game.update_fov();
        game.player.hp = game.player.max_hp / 3;
        game.player.inventory.retain(|i| i.equipped); // no potions to lean on
        let mut memory = BotMemory::default();
        let first = next_action(&game, &mut memory);
        assert_eq!(
            first,
            Action::Move(Point::new(1, 0)),
            "heads for the stairs, not the orc"
        );
        assert!(memory.plan.is_some());

        // Even with the orc gone from view, the plan holds.
        game.monsters.clear();
        game.update_fov();
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(1, 0))
        );
        let _ = stairs;
    }

    #[test]
    fn it_does_not_run_from_something_faster() {
        use crate::monster::Kind;
        let (mut game, _) = room_with_stairs();
        let bat = hunter_at(&game, Kind::GiantBat, Point::new(3, 5)); // speed 200
        game.monsters.push(bat);
        game.update_fov();
        game.player.hp = game.player.max_hp / 3;
        game.player.inventory.retain(|i| i.equipped);
        let mut memory = BotMemory::default();
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(1, 0)),
            "fights the bat"
        );
        assert!(memory.plan.is_none());
    }

    #[test]
    fn a_new_floor_wipes_the_plan() {
        use crate::monster::Kind;
        let (mut game, _) = room_with_stairs();
        let orc = hunter_at(&game, Kind::Orc, Point::new(1, 5));
        game.monsters.push(orc);
        game.update_fov();
        game.player.hp = game.player.max_hp / 3;
        game.player.inventory.retain(|i| i.equipped);
        let mut memory = BotMemory::default();
        next_action(&game, &mut memory);
        assert!(memory.plan.is_some(), "hurt, it plans an escape");
        game.depth += 1;
        game.monsters.clear();
        next_action(&game, &mut memory);
        assert!(memory.plan.is_none());
    }

    #[test]
    fn surrounded_and_hurt_it_teleports_rather_than_drinks() {
        use crate::item::Item;
        use crate::monster::Kind;
        let mut game = quiet_room();
        for pos in [Point::new(3, 5), Point::new(2, 4)] {
            let orc = hunter_at(&game, Kind::Orc, pos);
            game.monsters.push(orc);
        }
        game.update_fov();
        let teleport = ItemKind::Scroll(ScrollKind::Teleportation);
        game.lore.learn(teleport);
        let letter = game.player.add_item(Item::new(teleport)).unwrap();
        game.player.hp = 8;
        assert_eq!(
            next_action(&game, &mut BotMemory::default()),
            Action::Read {
                scroll: letter,
                target: None
            }
        );
    }

    #[test]
    fn fast_monsters_count_twice_and_healing_is_capped() {
        use crate::item::Item;
        use crate::monster::Kind;
        let mut game = quiet_room();
        let teleport = ItemKind::Scroll(ScrollKind::Teleportation);
        game.lore.learn(teleport);
        let scroll = game.player.add_item(Item::new(teleport)).unwrap();
        game.lore.learn(ItemKind::Potion(PotionKind::Healing)); // 'c'

        // One giant ant next to us: speed 150, so two bites of up to 5.
        // At 9 health one bite can't kill but two can, and a healing
        // potion capped at full health (10 here) wouldn't cover it.
        let ant = hunter_at(&game, Kind::GiantAnt, Point::new(3, 5));
        let worst = ant.attack().damage.1;
        game.monsters.push(ant);
        game.update_fov();
        game.player.max_hp = 2 * worst;
        game.player.hp = 2 * worst - 1;
        assert!(game.player.hp > worst, "one bite alone can't kill");
        assert_eq!(
            next_action(&game, &mut BotMemory::default()),
            Action::Read {
                scroll,
                target: None
            }
        );
    }

    #[test]
    fn life_potions_are_saved_while_healthy() {
        use crate::item::Item;
        let mut game = quiet_room();
        let life = ItemKind::Potion(PotionKind::Life);
        game.lore.learn(life);
        let letter = game.player.add_item(Item::new(life)).unwrap();
        assert_ne!(
            next_action(&game, &mut BotMemory::default()),
            Action::Drink(letter)
        );
        game.player.hp = game.player.max_hp / 3;
        assert_eq!(
            next_action(&game, &mut BotMemory::default()),
            Action::Drink(letter),
            "used when badly hurt"
        );
    }

    #[test]
    fn unknown_scrolls_wait_until_nothing_is_in_sight() {
        use crate::item::Item;
        use crate::monster::Kind;
        let mut game = quiet_room();
        let kind = ItemKind::Scroll(ScrollKind::MagicMapping);
        let letter = game.player.add_item(Item::new(kind)).unwrap();
        // A sleeping rat within sight (6 tiles; sight reaches 8).
        let rat = Monster::new(Kind::Rat, Point::new(8, 5), Ai::Asleep);
        game.monsters.push(rat);
        game.update_fov();
        assert!(game.is_visible(Point::new(8, 5)));
        assert!(!matches!(
            next_action(&game, &mut BotMemory::default()),
            Action::Read { .. }
        ));
        game.monsters.clear();
        game.update_fov();
        assert_eq!(
            next_action(&game, &mut BotMemory::default()),
            Action::Read {
                scroll: letter,
                target: None
            }
        );
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
            let mut memory = BotMemory::default();
            while game.death.is_none() && game.turn < 1_500 {
                let turn = game.turn;
                game.apply(next_action(&game, &mut memory));
                idle = if game.turn == turn { idle + 1 } else { 0 };
                assert!(idle < 20, "seed {seed}: stuck at turn {turn}");
            }
            assert!(
                game.depth > 1 || game.death.is_some(),
                "seed {seed}: never left floor 1"
            );
        }
    }

    #[test]
    fn resting_waits_until_healed() {
        let mut game = quiet_room();
        game.player.hp = game.player.max_hp / 2;
        assert_eq!(rest_blocked(&game), None);
        let mut turns = 0;
        while let Some(action) = rest_step(&game) {
            assert_eq!(action, Action::Wait);
            let watch = Watch::new(&game);
            game.apply(action);
            assert_eq!(watch.reason_to_stop(&game), None);
            turns += 1;
            assert!(turns < 2_000, "never healed");
        }
        assert_eq!(game.player.hp, game.player.max_hp);
        assert!(rest_blocked(&game).is_some(), "already at full health");
    }

    #[test]
    fn resting_waits_for_no_hunter_or_hunger() {
        use crate::monster::Kind;
        let mut game = quiet_room();
        game.player.hp = game.player.max_hp / 2;
        game.player.food = crate::player::WEAK_AT;
        assert!(rest_blocked(&game).unwrap().contains("hungry"));
        game.player.food = crate::player::FOOD_START;
        let orc = hunter_at(&game, Kind::Orc, Point::new(1, 5));
        game.monsters.push(orc);
        game.update_fov();
        assert!(rest_blocked(&game).unwrap().contains("orc"));
    }

    #[test]
    fn getting_hungrier_stops_automatic_steps() {
        let mut game = quiet_room();
        game.player.hp = game.player.max_hp / 2;
        game.player.food = crate::player::HUNGRY_AT + 1;
        let watch = Watch::new(&game);
        game.apply(Action::Wait);
        assert_eq!(game.player.hunger(), Hunger::Hungry);
        assert_eq!(watch.reason_to_stop(&game), Some(String::new()));
    }

    #[test]
    fn it_takes_up_a_slayer_for_its_prey_and_puts_it_away_after() {
        use crate::item::WeaponKind;
        use crate::monster::Kind;
        let mut game = quiet_room();
        let sunsteel = Item::new(ItemKind::Weapon(WeaponKind::Sunsteel));
        let letter = game.player.add_item(sunsteel).unwrap();
        // Make the everyday weapon clearly better, so it's swapped back.
        let axe = Item::enchanted(ItemKind::Weapon(WeaponKind::Axe), 3);
        let axe_letter = game.player.add_item(axe).unwrap();
        game.player.item_mut(axe_letter).unwrap().known = true;
        let mut memory = BotMemory::default();
        assert_eq!(next_action(&game, &mut memory), Action::Equip(axe_letter));
        game.apply(Action::Equip(axe_letter));
        let vampire = hunter_at(&game, Kind::Vampire, Point::new(4, 5));
        game.monsters.push(vampire);
        game.update_fov();
        assert_eq!(next_action(&game, &mut memory), Action::Equip(letter));
        game.monsters.clear();
        game.apply(Action::Equip(letter));
        assert_eq!(next_action(&game, &mut memory), Action::Equip(axe_letter));
    }

    /// A 9x5 room joined at (10,3) to a corridor running east, with
    /// everything seen.
    fn room_and_corridor() -> Game {
        let mut game = Game::new(1);
        let mut map = crate::map::Map::new_filled(24, 7);
        map.carve_room(1, 1, 9, 5);
        for x in 10..22 {
            map.set_tile(Point::new(x, 3), Tile::Floor);
        }
        game.place_on_map(map, Point::new(7, 3));
        game.monsters.clear();
        game.items.clear();
        game.traps.clear();
        for p in game.map.points().collect::<Vec<_>>() {
            game.map.reveal(p);
        }
        game.update_fov();
        game
    }

    #[test]
    fn it_backs_into_a_corridor_to_fight_a_jelly() {
        use crate::monster::Kind;
        let mut game = room_and_corridor();
        let jelly = hunter_at(&game, Kind::PinkJelly, Point::new(3, 3));
        game.monsters.push(jelly);
        game.update_fov();
        let mut memory = BotMemory::default();
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(1, 0)),
            "away from the jelly, toward the corridor"
        );
        assert_eq!(memory.plan.map(|p| p.goal), Some(Point::new(11, 3)));

        // In the corridor, it waits for the jelly rather than going
        // back out to meet it, then fights it when it arrives.
        game.player.pos = Point::new(11, 3);
        game.monsters[0].pos = Point::new(6, 3);
        game.update_fov();
        assert_eq!(next_action(&game, &mut memory), Action::Wait);
        game.monsters[0].pos = Point::new(10, 3);
        game.update_fov();
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(-1, 0))
        );
    }

    #[test]
    fn only_one_hold_a_floor() {
        use crate::monster::Kind;
        let mut game = room_and_corridor();
        let jelly = hunter_at(&game, Kind::PinkJelly, Point::new(3, 3));
        game.monsters.push(jelly);
        game.update_fov();
        let mut memory = BotMemory::default();
        next_action(&game, &mut memory);
        assert!(memory.plan.is_some_and(|p| p.purpose == Purpose::Hold));

        // The jelly drops out of view and the hold ends; when it's back,
        // the bot meets it instead of starting another hold.
        let jelly = game.monsters.pop().unwrap();
        game.update_fov();
        next_action(&game, &mut memory);
        assert!(memory.plan.is_none());
        game.monsters.push(jelly);
        game.update_fov();
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(-1, 0))
        );
        assert!(memory.plan.is_none());
    }

    #[test]
    fn other_monsters_are_met_in_the_open() {
        use crate::monster::Kind;
        let mut game = room_and_corridor();
        let orc = hunter_at(&game, Kind::Orc, Point::new(3, 3));
        game.monsters.push(orc);
        game.update_fov();
        let mut memory = BotMemory::default();
        assert_eq!(
            next_action(&game, &mut memory),
            Action::Move(Point::new(-1, 0))
        );
        assert!(memory.plan.is_none());
    }

    #[test]
    fn auto_explore_always_wants_an_artifact() {
        use crate::item::WeaponKind;
        let mut game = quiet_room();
        let axe = Item::enchanted(ItemKind::Weapon(WeaponKind::Axe), 5);
        let letter = game.player.add_item(axe).unwrap();
        game.player.item_mut(letter).unwrap().known = true;
        game.apply(Action::Equip(letter));
        let hellbane = Item::new(ItemKind::Weapon(WeaponKind::Hellbane));
        assert!(worth_picking_up(&game, &hellbane));
    }
}
