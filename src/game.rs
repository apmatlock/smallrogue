//! The complete game state and the rules that change it.
//!
//! Nothing in this module knows about the terminal. It only answers
//! "what happens when the player does X?". Keeping it that way makes it
//! easy to test, to save later, and to draw with tiles someday.

use crate::combat;
use crate::dungeon;
use crate::fov;
use crate::geom::{DIRECTIONS_8, Point};
use crate::grid::Grid;
use crate::item::{self, FloorItem, Item, ItemKind, PotionKind};
use crate::lore::Lore;
use crate::map::{Map, Tile};
use crate::monster::{self, Ability, Ai, Monster};
use crate::player::{Hunger, Player};
use crate::rng::{self, Rng};
use crate::skills::{self, Attribute, Skill};
use crate::stats::{Kill, Stats};
use crate::text::article;
use crate::themed;
use crate::trap::{self, Trap};
use crate::zone::{EVERYWHERE, Place};

/// A starving player loses 1 health every this many turns.
pub const STARVING_DAMAGE_EVERY: u64 = 5;

/// Turns an unknown item must be worn before revealing its enchantment,
/// at 2 Intellect. Each point above that cuts it by 15%.
pub const IDENTIFY_TURNS: u32 = 300;

/// Points of health percent above a warning's line that health must
/// climb back to before that warning can be given again.
const HEALTH_WARNING_MARGIN: i32 = 15;

/// What kind of news a log message is, so the screen can color it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsgKind {
    Info,
    /// Good for the player, like killing a monster.
    Good,
    /// Bad for the player, like taking damage.
    Bad,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub text: String,
    pub kind: MsgKind,
    /// How many times in a row this message happened. Shown as "(x3)"
    /// instead of repeating the line.
    pub count: u32,
}

/// Whether a player action used up time. Only actions that take time
/// let the monsters move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// Nothing happened, e.g. walking into a wall.
    Free,
    /// A normal action that takes one turn.
    TookTurn,
    /// Took the stairs. Monsters on the new floor get no move yet.
    NewFloor,
}

/// Something the player asked to do. The input module turns key
/// presses into these, so the game never sees raw keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Step in a direction. Stepping into a closed door opens it.
    Move(Point),
    Wait,
    Descend,
    /// Close the door in this direction.
    Close(Point),
    /// Pick up the item underfoot.
    PickUp,
    /// The `char`s below are inventory letters.
    Drop(char),
    /// Equip a weapon or armor, or take it off if already equipped.
    Equip(char),
    Drink(char),
    Eat(char),
    /// Read a scroll. `target` is the item a scroll of enchanting
    /// should improve.
    Read {
        scroll: char,
        target: Option<char>,
    },
    /// Turn picking up by walking over items on or off. Takes no time;
    /// it's an action so recordings keep the setting and replay exactly.
    AutoPickup(bool),
}

pub struct Game {
    pub map: Map,
    pub player: Player,
    /// Messages shown in the log, oldest first.
    pub log: Vec<Message>,
    /// Set when the player dies, naming what did it, e.g. "a jackal".
    /// Once set, the game ignores further actions.
    pub death: Option<String>,
    /// The run ended by quitting rather than dying.
    pub gave_up: bool,
    pub turn: u64,
    pub depth: u32,
    /// The run's seed. Each floor's layout is derived from it, so the
    /// same seed always produces the same dungeon.
    pub seed: u64,
    /// Tiles the player can see right now. Recomputed after every
    /// action, because moving or opening a door changes it.
    visible: Grid<bool>,
    pub monsters: Vec<Monster>,
    /// Which tiles hold a monster, rebuilt at the start of each monster
    /// phase. Pathfinding asks this for every tile it considers, so a
    /// grid lookup beats searching the monster list each time.
    pub(crate) monster_grid: Grid<bool>,
    /// Set when a monster opens a door, which can change what the
    /// player sees, so the view is only recomputed when needed.
    pub(crate) fov_dirty: bool,
    pub items: Vec<FloorItem>,
    pub traps: Vec<Trap>,
    /// A known trap the player just tried to step onto. Moving the same
    /// way again (and only that) confirms the step.
    pub(crate) trap_warning: Option<Point>,
    /// What the player knows about items, and how unknown ones look.
    pub lore: Lore,
    /// Randomness for events during play, such as monsters waking.
    /// Kept separate from floor generation so what happens on one
    /// floor never changes the layout of the next.
    pub(crate) rng: Rng,
    /// Counts kept over the run, for the death screen.
    pub stats: Stats,
    /// When recording, every action applied, oldest first, for the
    /// recorder to collect. `None` (the default) keeps nothing.
    pub journal: Option<Vec<Action>>,
    /// Whether walking onto an item picks it up. On at the start of
    /// every run; the player's saved choice is applied as an action.
    pub auto_pickup: bool,
    /// How low health has been warned about: 0 none, 1 below half,
    /// 2 below a quarter.
    health_warned: u8,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut game = Self {
            map: Map::new_filled(1, 1), // replaced by enter_floor below
            player: Player::fighter(Point::default()),
            log: Vec::new(),
            death: None,
            gave_up: false,
            turn: 0,
            depth: 0,
            seed,
            visible: Grid::new(1, 1, false),
            monsters: Vec::new(),
            monster_grid: Grid::new(1, 1, false),
            fov_dirty: false,
            items: Vec::new(),
            traps: Vec::new(),
            trap_warning: None,
            lore: Lore::new(&mut Rng::new(rng::mix(seed, 0x4C4F_5245))),
            rng: Rng::new(rng::mix(seed, u64::MAX)),
            stats: Stats::default(),
            journal: None,
            auto_pickup: true,
            health_warned: 0,
        };
        // The fighter knows the healing potion they start with.
        game.lore.learn(ItemKind::Potion(PotionKind::Healing));
        game.enter_floor(1);
        game.log("You descend into the dark.");
        game.welcome_to_zone();
        game.update_fov();
        game
    }

    /// Generates and moves the player onto a new floor.
    pub(crate) fn enter_floor(&mut self, depth: u32) {
        // Each floor gets its own generator seeded from (run seed,
        // depth). Floor 5 of a seed is then always the same, no matter
        // what random events happened on floors 1 to 4.
        let mut floor_rng = Rng::new(rng::mix(self.seed, depth as u64));
        let zone = Place::at_depth(depth).zone();
        let mut level = dungeon::generate(&mut floor_rng, &zone.floor);
        let home: Vec<_> = zone.home.iter().chain(&EVERYWHERE).copied().collect();
        self.monsters = monster::spawn_for_floor(&mut floor_rng, &level, depth, &home);
        self.items = item::spawn_for_floor(&mut floor_rng, &level, &zone.items);
        themed::add_themed_room(
            &mut floor_rng,
            &mut level,
            depth,
            zone.themes,
            &zone.items,
            themed::Contents {
                monsters: &mut self.monsters,
                items: &mut self.items,
            },
        );
        self.traps = trap::spawn_for_floor(&mut floor_rng, &level, &self.items, depth);
        self.place_on_map(level.map, level.start);
        self.depth = depth;
    }

    /// Where the current floor falls in the cycle of zones.
    pub fn place(&self) -> Place {
        Place::at_depth(self.depth)
    }

    /// Announces a zone on its first floor.
    pub(crate) fn welcome_to_zone(&mut self) {
        if let Some(text) = self.place().welcome() {
            self.log_as(&text, MsgKind::Good);
        }
    }

    /// Swaps in a new map with the player at `at`, resetting what is
    /// visible to match the new map's size.
    pub(crate) fn place_on_map(&mut self, map: Map, at: Point) {
        self.visible = Grid::new(map.width(), map.height(), false);
        self.map = map;
        self.player.pos = at;
    }

    /// Recalculates what the player sees and adds it to the map's
    /// memory.
    pub(crate) fn update_fov(&mut self) {
        self.visible.fill(false);

        // Borrow the two fields separately: the closures read the map
        // and write `visible` at the same time, which Rust allows
        // because they are different fields.
        let map = &self.map;
        let visible = &mut self.visible;
        // Note which tiles are newly seen while computing, so only those
        // need revealing, rather than scanning the whole map afterwards.
        let mut newly_seen = Vec::new();
        fov::compute(
            self.player.pos,
            self.player.sight_radius(),
            |p| map.tile(p).blocks_sight(),
            |p| {
                visible.set(p, true);
                if !map.is_revealed(p) {
                    newly_seen.push(p);
                }
            },
        );

        let mut spotted_stairs = false;
        for p in newly_seen {
            self.map.reveal(p);
            spotted_stairs |= self.map.tile(p) == Tile::StairsDown;
        }
        if spotted_stairs {
            self.log("You see a staircase leading down.");
        }
    }

    pub fn is_visible(&self, p: Point) -> bool {
        self.visible.get(p).copied().unwrap_or(false)
    }

    /// Applies one player action, then lets the monsters respond if
    /// the action took time.
    pub fn apply(&mut self, action: Action) {
        if self.death.is_some() {
            return;
        }
        if let Some(journal) = &mut self.journal {
            journal.push(action);
        }
        // A trap warning only carries over to the very next action.
        let warned = self.trap_warning.take();
        let outcome = match action {
            Action::Move(delta) => self.move_player(delta, warned),
            Action::Wait => Outcome::TookTurn,
            Action::Descend => self.descend(),
            Action::Close(dir) => self.close_door(dir),
            Action::PickUp => self.pick_up(),
            Action::Drop(letter) => self.drop_item(letter),
            Action::Equip(letter) => self.equip(letter),
            Action::Drink(letter) => self.drink(letter),
            Action::Eat(letter) => self.eat(letter),
            Action::Read { scroll, target } => self.read(scroll, target),
            Action::AutoPickup(on) => self.set_auto_pickup(on),
        };
        if outcome == Outcome::Free {
            return;
        }
        self.turn += 1;
        // A fatal action (drinking decay, say) still used its turn, but
        // nothing happens after it: no healing, no monster moves.
        if self.death.is_some() {
            return;
        }
        self.player_upkeep();
        if self.death.is_some() {
            return;
        }
        // Monsters need to know what the player can see (and so what
        // can see the player) after the player's move.
        self.update_fov();
        // Spot traps with this turn's view, not last turn's.
        self.search_for_traps();
        if outcome == Outcome::TookTurn {
            self.monsters_act();
            // Monsters may have opened doors, changing the view.
            if std::mem::take(&mut self.fov_dirty) {
                self.update_fov();
            }
        }
        if self.death.is_none() {
            self.warn_about_health();
        }
    }

    /// Says so when health falls below half, then below a quarter,
    /// pointing at known healing if the player carries any. Recorded
    /// runs showed players dying with healing potions in the pack.
    ///
    /// A warning isn't repeated until health has recovered well above
    /// its line, so a fight hovering around it doesn't fill the log.
    fn warn_about_health(&mut self) {
        let percent = self.player.hp * 100 / self.player.max_hp.max(1);
        let band = match percent {
            ..=25 => 2,
            26..=50 => 1,
            _ => 0,
        };
        if band > self.health_warned {
            self.health_warned = band;
            let text = if band == 2 {
                "You are badly hurt!"
            } else {
                "You are below half health."
            };
            let hint = match self.known_healing() {
                Some(name) => format!(" You have {name}: q to drink."),
                None => String::new(),
            };
            self.log_as(&format!("{text}{hint}"), MsgKind::Bad);
        } else if band < self.health_warned {
            let line = if self.health_warned == 2 { 25 } else { 50 };
            if percent > line + HEALTH_WARNING_MARGIN {
                self.health_warned = band;
            }
        }
    }

    /// How many potions the player carries that they know would heal.
    pub fn healing_carried(&self) -> u32 {
        self.player
            .inventory
            .iter()
            .filter(|i| {
                matches!(
                    i.kind,
                    ItemKind::Potion(PotionKind::Healing | PotionKind::Life)
                ) && self.lore.knows(i.kind)
            })
            .map(|i| i.count)
            .sum()
    }

    /// A potion the player carries and knows will heal, as "a potion of
    /// healing", if any.
    pub fn known_healing(&self) -> Option<String> {
        [PotionKind::Healing, PotionKind::Life]
            .into_iter()
            .map(ItemKind::Potion)
            .filter(|&k| self.lore.knows(k))
            .find_map(|k| self.player.inventory.iter().find(|i| i.kind == k))
            .map(|item| {
                let one = Item {
                    count: 1,
                    ..item.clone()
                };
                self.lore.with_article(&one)
            })
    }

    fn move_player(&mut self, delta: Point, warned: Option<Point>) -> Outcome {
        let target = self.player.pos + delta;
        if let Some(i) = self.monsters.iter().position(|m| m.pos == target) {
            self.player_attack(i);
            return Outcome::TookTurn;
        }
        match self.map.tile(target) {
            Tile::DoorClosed => {
                // Opening takes a turn; you step through on the next.
                self.map.set_tile(target, Tile::DoorOpen);
                self.log("You open the door.");
                Outcome::TookTurn
            }
            tile if tile.is_walkable() => {
                if self.known_trap_at(target) && warned != Some(target) {
                    let name = self.trap_at(target).expect("known trap").kind.name();
                    self.log(&format!(
                        "There is a {name} there. Move that way again to step on it."
                    ));
                    self.trap_warning = Some(target);
                    return Outcome::Free;
                }
                self.player.pos = target;
                if let Some(outcome) = self.spring_trap() {
                    return outcome;
                }
                if tile == Tile::StairsDown {
                    self.log("There is a staircase down here. Press > to descend.");
                }
                // Walking onto an item picks it up as part of the move,
                // unless the player turned that off.
                if self.auto_pickup {
                    if self.item_at(target).is_some() {
                        self.pick_up();
                    }
                } else if let Some(fi) = self.item_at(target) {
                    let name = self.lore.with_article(&fi.item);
                    self.log(&format!("You see {name} here."));
                }
                Outcome::TookTurn
            }
            _ => {
                self.log("The stone wall does not yield.");
                Outcome::Free
            }
        }
    }

    fn set_auto_pickup(&mut self, on: bool) -> Outcome {
        self.auto_pickup = on;
        self.log(if on {
            "Auto pickup is on: walking over items picks them up."
        } else {
            "Auto pickup is off: press g to pick things up."
        });
        Outcome::Free
    }

    fn descend(&mut self) -> Outcome {
        if self.map.tile(self.player.pos) != Tile::StairsDown {
            self.log("There are no stairs here.");
            return Outcome::Free;
        }
        self.enter_floor(self.depth + 1);
        self.stats.stairs_taken += 1;
        self.log(&format!("You descend to depth {}.", self.depth));
        self.welcome_to_zone();
        Outcome::NewFloor
    }

    fn close_door(&mut self, dir: Point) -> Outcome {
        let target = self.player.pos + dir;
        match self.map.tile(target) {
            Tile::DoorOpen if self.monster_at(target).is_some() => {
                self.log("Something is standing in the doorway.");
                Outcome::Free
            }
            Tile::DoorOpen => {
                self.map.set_tile(target, Tile::DoorClosed);
                self.log("You close the door.");
                Outcome::TookTurn
            }
            Tile::DoorClosed => {
                self.log("That door is already closed.");
                Outcome::Free
            }
            _ => {
                self.log("There is no door there.");
                Outcome::Free
            }
        }
    }

    /// Things that happen to the player every turn: healing, curses
    /// wearing off, and worn gear slowly revealing itself.
    fn player_upkeep(&mut self) {
        self.tick_hunger();
        if self.death.is_some() {
            return;
        }
        let p = &mut self.player;
        // A body weak from hunger doesn't heal.
        if p.hunger() < Hunger::Weak {
            p.regen_progress += p.regen_rate();
            while p.regen_progress >= 100 {
                p.regen_progress -= 100;
                p.hp = (p.hp + 1).min(p.max_hp);
            }
        }

        let threshold = self.identify_threshold();
        let mut faded = Vec::new();
        let mut learned = Vec::new();
        for item in self.player.inventory.iter_mut().filter(|i| i.equipped) {
            if item.curse_turns > 0 {
                item.curse_turns -= 1;
                if item.curse_turns == 0 {
                    faded.push(item.letter);
                }
            }
            if !item.known {
                item.worn_turns += 1;
                if item.worn_turns >= threshold {
                    item.known = true;
                    learned.push(item.letter);
                }
            }
        }
        // Log afterwards: naming needs `self.lore` while the loop above
        // was borrowing the pack.
        for letter in faded {
            let name = self
                .lore
                .name(self.player.item(letter).expect("still worn"));
            self.log_as(&format!("The curse on your {name} fades."), MsgKind::Good);
        }
        for letter in learned {
            let name = self
                .lore
                .name(self.player.item(letter).expect("still worn"));
            self.log(&format!("You've worn it long enough to know it: {name}."));
        }
    }

    /// Uses up a turn's food, warns as hunger gets worse, and hurts a
    /// starving player.
    fn tick_hunger(&mut self) {
        let before = self.player.hunger();
        self.player.food = (self.player.food - 1).max(0);
        let now = self.player.hunger();
        if now > before {
            match now {
                Hunger::Hungry => self.log("You are getting hungry."),
                Hunger::Weak => self.log_as("You feel weak with hunger.", MsgKind::Bad),
                Hunger::Starving => {
                    self.log_as("You are starving! Eat something now.", MsgKind::Bad)
                }
                Hunger::Fed => {}
            }
        }
        if now == Hunger::Starving && self.turn.is_multiple_of(STARVING_DAMAGE_EVERY) {
            self.hurt_player(1);
            if self.player.hp <= 0 {
                self.kill_player("starvation");
            }
        }
    }

    /// Turns of wearing needed to reveal an item, shorter for clever
    /// characters.
    pub fn identify_threshold(&self) -> u32 {
        let percent = (100 - 15 * (self.player.intellect - 2)).clamp(20, 100);
        IDENTIFY_TURNS * percent as u32 / 100
    }

    /// The player attacks monster `i`. Any attack, hit or miss, alerts
    /// the monster.
    fn player_attack(&mut self, i: usize) {
        self.train(Skill::Melee, 1);
        let name = self.monsters[i].name();
        let defense = self.monsters[i].defense();
        match combat::resolve(&mut self.rng, self.player.attack(), defense) {
            None => {
                self.stats.misses += 1;
                self.log(&format!("You miss the {name}."));
            }
            Some(damage) => {
                self.stats.hits += 1;
                // Only the health it had counts, not overkill.
                self.stats.damage_dealt += damage.min(self.monsters[i].hp).max(0) as u32;
                self.monsters[i].hp -= damage;
                if self.monsters[i].hp <= 0 {
                    let dead = self.monsters.remove(i);
                    self.stats.record_kill(Kill {
                        name,
                        xp: dead.xp(),
                        depth: self.depth,
                    });
                    self.log_as(&format!("You kill the {name}!"), MsgKind::Good);
                    self.drop_loot(&dead);
                    self.gain_xp(dead.xp());
                    return;
                }
                self.log(&format!("You hit the {name} for {damage}."));
                if self.monsters[i].species().has(Ability::Splits) {
                    self.split_monster(i);
                }
            }
        }
        // Being attacked alerts a monster, but a thief with loot keeps
        // running.
        if self.monsters[i].carrying.is_none() {
            self.monsters[i].ai = Ai::Hunting {
                last_seen: self.player.pos,
            };
        }
    }

    /// Monster `i` attacks the player.
    pub(crate) fn monster_attack(&mut self, i: usize) {
        // Being attacked trains dodging, hit or miss.
        self.train(Skill::Dodge, 1);
        let m = &self.monsters[i];
        let (name, verb) = (m.name(), m.species().verb);
        match combat::resolve(&mut self.rng, m.attack(), self.player.defense()) {
            None => self.log(&format!("The {name} misses you.")),
            Some(damage) => {
                let species = self.monsters[i].species();
                // A thief's hit steals instead of hurting, if there is
                // anything loose to take.
                if species.has(Ability::StealsAndFlees)
                    && self.monsters[i].carrying.is_none()
                    && self.steal_item(i)
                {
                    return;
                }
                self.hurt_player(damage);
                self.log_as(
                    &format!("The {name} {verb} you for {damage}."),
                    MsgKind::Bad,
                );
                if self.player.armor().is_some() {
                    self.train(Skill::Armor, 1);
                }
                if self.player.hp <= 0 {
                    self.kill_player(&format!("{} {name}", article(name)));
                    return;
                }
                if species.has(Ability::DrainsMaxHealth) {
                    self.drain_max_health(i);
                }
                if species.has(Ability::DrinksBlood) {
                    self.drink_blood(i, damage);
                }
                if species.has(Ability::CorrodesArmor) {
                    self.acid_hit();
                }
            }
        }
    }

    /// Adds skill training, announcing any new skill level.
    pub(crate) fn train(&mut self, skill: Skill, amount: u32) {
        if let Some(level) = self.player.skills.train(skill, amount) {
            self.log_as(
                &format!("Your {} skill improves to {level}.", skill.name()),
                MsgKind::Good,
            );
        }
    }

    /// Adds experience, raising the character's level as often as it
    /// reaches the next threshold.
    pub(crate) fn gain_xp(&mut self, amount: u32) {
        self.player.xp += amount;
        while self.player.xp >= skills::xp_for_level(self.player.level + 1) {
            let p = &mut self.player;
            p.level += 1;
            p.max_hp += skills::HEALTH_PER_LEVEL;
            p.hp += skills::HEALTH_PER_LEVEL;
            let (name, value) = match skills::attribute_for_level(p.level) {
                Attribute::Strength => {
                    p.strength += 1;
                    ("strength", p.strength)
                }
                Attribute::Agility => {
                    p.agility += 1;
                    ("agility", p.agility)
                }
                Attribute::Intellect => {
                    p.intellect += 1;
                    ("intellect", p.intellect)
                }
            };
            let level = p.level;
            self.log_as(
                &format!("You reach level {level}! Your {name} rises to {value}."),
                MsgKind::Good,
            );
        }
    }

    /// Takes health from the player, counting it for the death screen.
    /// The caller checks for death, since only it knows the cause.
    pub(crate) fn hurt_player(&mut self, damage: i32) {
        self.stats.damage_taken += damage.max(0) as u32;
        self.player.hp -= damage;
    }

    /// Ends the run. `killer` finishes the sentence "Killed by ...",
    /// e.g. "a jackal" or "a potion of decay".
    pub(crate) fn kill_player(&mut self, killer: &str) {
        self.player.hp = 0;
        self.log_as("You die...", MsgKind::Bad);
        self.death = Some(killer.to_string());
    }

    /// One sentence summing up the death, or `None` while alive. Built
    /// when asked rather than at the moment of death, so the turn
    /// count includes the fatal turn.
    pub fn death_summary(&self) -> Option<String> {
        let killer = self.death.as_ref()?;
        let how = if self.gave_up {
            "Gave up".to_string()
        } else {
            format!("Killed by {killer}")
        };
        Some(format!(
            "{how} on depth {} after {} turns.",
            self.depth, self.turn
        ))
    }

    /// Ends the run by quitting. It still counts as a finished run, so
    /// quitting before a bad death can't keep a score off the list.
    pub fn give_up(&mut self) {
        self.log("You give up.");
        self.death = Some("gave up".to_string());
        self.gave_up = true;
    }

    pub fn monster_at(&self, p: Point) -> Option<&Monster> {
        self.monsters.iter().find(|m| m.pos == p)
    }

    /// Directions from the player to each adjacent open door.
    pub fn adjacent_open_doors(&self) -> Vec<Point> {
        DIRECTIONS_8
            .into_iter()
            .filter(|&d| self.map.tile(self.player.pos + d) == Tile::DoorOpen)
            .collect()
    }

    pub fn log(&mut self, message: &str) {
        self.log_as(message, MsgKind::Info);
    }

    pub fn log_as(&mut self, text: &str, kind: MsgKind) {
        // A repeat of the last message bumps its count instead of
        // adding a line, so ten misses in a row take one line.
        if let Some(last) = self.log.last_mut()
            && last.text == text
        {
            last.count += 1;
            return;
        }
        self.log.push(Message {
            text: text.to_string(),
            kind,
            count: 1,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A game on a tiny hand-made map, so tests control the layout.
    ///
    /// ```text
    /// #######
    /// #..+.>#     @ starts at (1,1)
    /// #######     + is a closed door at (3,1), > at (5,1)
    /// ```
    fn corridor_game() -> Game {
        let mut game = Game::new(1);
        let mut map = Map::new_filled(7, 3);
        map.carve_h_corridor(1, 5, 1);
        map.set_tile(Point::new(3, 1), Tile::DoorClosed);
        map.set_tile(Point::new(5, 1), Tile::StairsDown);
        game.place_on_map(map, Point::new(1, 1));
        game.monsters.clear();
        game.items.clear();
        game.log.clear();
        game.update_fov();
        game
    }

    const EAST: Point = Point::new(1, 0);
    const WEST: Point = Point::new(-1, 0);

    #[test]
    fn walls_block_movement_and_cost_no_turn() {
        let mut game = corridor_game();
        game.apply(Action::Move(WEST));
        assert_eq!(game.player.pos, Point::new(1, 1));
        assert_eq!(game.turn, 0);
    }

    #[test]
    fn bumping_a_door_opens_it_then_you_walk_through() {
        let mut game = corridor_game();
        game.apply(Action::Move(EAST)); // to (2,1)
        game.apply(Action::Move(EAST)); // opens the door, stays put
        assert_eq!(game.player.pos, Point::new(2, 1));
        assert_eq!(game.map.tile(Point::new(3, 1)), Tile::DoorOpen);
        game.apply(Action::Move(EAST)); // steps into the doorway
        assert_eq!(game.player.pos, Point::new(3, 1));
        assert_eq!(game.turn, 3);
    }

    #[test]
    fn closing_a_door() {
        let mut game = corridor_game();
        game.map.set_tile(Point::new(3, 1), Tile::DoorOpen);
        game.player.pos = Point::new(2, 1);
        assert_eq!(game.adjacent_open_doors(), vec![EAST]);
        game.apply(Action::Close(EAST));
        assert_eq!(game.map.tile(Point::new(3, 1)), Tile::DoorClosed);
    }

    #[test]
    fn descending_needs_stairs_and_builds_a_new_floor() {
        let mut game = corridor_game();
        game.apply(Action::Descend);
        assert_eq!(game.depth, 1);

        game.player.pos = Point::new(5, 1);
        game.apply(Action::Descend);
        assert_eq!(game.depth, 2);
        assert_eq!(game.stats.stairs_taken, 1, "only the real descent counts");
        assert_eq!(game.map.width(), Place::at_depth(2).zone().floor.width);
        assert!(game.map.tile(game.player.pos).is_walkable());
    }

    #[test]
    fn closed_doors_block_sight_until_opened() {
        let mut game = corridor_game();
        let beyond = Point::new(4, 1);
        assert!(!game.is_visible(beyond));
        assert!(!game.map.is_revealed(beyond));

        game.apply(Action::Move(EAST)); // step next to the door
        game.apply(Action::Move(EAST)); // open it
        assert!(game.is_visible(beyond));
        assert!(game.map.is_revealed(beyond));
        assert!(game.log.iter().any(|m| m.text.contains("staircase")));
    }

    #[test]
    fn remembered_tiles_stay_revealed_after_losing_sight() {
        let mut game = corridor_game();
        game.apply(Action::Move(EAST));
        game.apply(Action::Move(EAST)); // open the door
        game.player.pos = Point::new(2, 1);
        game.apply(Action::Close(EAST));
        let beyond = Point::new(4, 1);
        assert!(!game.is_visible(beyond));
        assert!(game.map.is_revealed(beyond));
    }

    use crate::monster::{Ai, Kind};

    /// An empty 20x9 room with the player on the left side.
    fn room_game() -> Game {
        let mut game = Game::new(1);
        let mut map = Map::new_filled(22, 11);
        map.carve_room(1, 1, 20, 9);
        game.place_on_map(map, Point::new(2, 5));
        game.monsters.clear();
        game.items.clear();
        game.log.clear();
        game.update_fov();
        game
    }

    #[test]
    fn low_health_is_called_out_once_with_a_healing_hint() {
        let mut game = room_game();
        let max = game.player.max_hp;
        let last = |g: &Game| g.log.last().unwrap().text.clone();
        let at = |g: &mut Game, percent: i32| {
            g.player.hp = max * percent / 100;
            g.apply(Action::Wait);
        };
        at(&mut game, 45);
        assert_eq!(
            last(&game),
            "You are below half health. You have a potion of healing: q to drink."
        );
        let logged = game.log.len();
        at(&mut game, 45);
        assert_eq!(game.log.len(), logged, "not repeated");
        at(&mut game, 20);
        assert!(last(&game).starts_with("You are badly hurt!"));
        // A small recovery doesn't re-arm the warning...
        let logged = game.log.len();
        at(&mut game, 30);
        at(&mut game, 20);
        assert_eq!(game.log.len(), logged, "{}", last(&game));
        // ...but healing well clear of it does.
        at(&mut game, 100);
        game.player.inventory.retain(|i| i.equipped);
        at(&mut game, 45);
        assert_eq!(
            last(&game),
            "You are below half health.",
            "no healing to mention"
        );
    }

    fn add_monster(game: &mut Game, kind: Kind, pos: Point, ai: Ai) -> usize {
        game.monsters.push(Monster::new(kind, pos, ai));
        game.monsters.len() - 1
    }

    #[test]
    fn hunting_monster_closes_in_and_stops_adjacent() {
        let mut game = room_game();
        let rat = add_monster(&mut game, Kind::Rat, Point::new(7, 5), Ai::Asleep);
        game.monsters[rat].ai = Ai::Hunting {
            last_seen: game.player.pos,
        };
        for _ in 0..10 {
            game.apply(Action::Wait);
        }
        assert!(game.monsters[rat].pos.is_adjacent(game.player.pos));
        assert!(game.log.iter().any(|m| m.text.starts_with("The rat")));
    }

    #[test]
    fn speed_controls_how_often_monsters_move() {
        let mut game = room_game();
        let far = |x| Point::new(x, 1);
        // Out of the player's sight radius? No: the room is small, so
        // give them a far-off wander goal instead of hunting.
        let jackal = add_monster(
            &mut game,
            Kind::Jackal,
            far(10),
            Ai::Wandering { goal: far(20) },
        );
        let rat = add_monster(
            &mut game,
            Kind::Rat,
            Point::new(10, 9),
            Ai::Wandering {
                goal: Point::new(20, 9),
            },
        );
        let zombie = add_monster(
            &mut game,
            Kind::Zombie,
            Point::new(10, 5),
            Ai::Wandering {
                goal: Point::new(20, 5),
            },
        );
        game.player.pos = Point::new(1, 9); // tuck the player in a corner
        game.update_fov();
        for m in &mut game.monsters {
            m.energy = 0;
        }
        for _ in 0..4 {
            game.apply(Action::Wait);
        }
        let moved = |i: usize, start: i32| game.monsters[i].pos.x - start;
        assert_eq!(moved(jackal, 10), 6); // speed 150
        assert_eq!(moved(rat, 10), 4); // speed 100
        assert_eq!(moved(zombie, 10), 2); // speed 50
    }

    #[test]
    fn sleeping_monster_out_of_sight_stays_put() {
        let mut game = corridor_game();
        let pos = Point::new(5, 1); // behind the closed door
        let rat = add_monster(&mut game, Kind::Rat, pos, Ai::Asleep);
        for _ in 0..20 {
            game.apply(Action::Wait);
        }
        assert_eq!(game.monsters[rat].pos, pos);
        assert_eq!(game.monsters[rat].ai, Ai::Asleep);
    }

    #[test]
    fn only_some_monsters_open_doors() {
        // The player waits at (1,1) behind the closed door at (3,1).
        // Each monster starts beyond the door, hunting toward (2,1).
        let door = Point::new(3, 1);
        let hunt = Ai::Hunting {
            last_seen: Point::new(2, 1),
        };

        let mut game = corridor_game();
        add_monster(&mut game, Kind::Rat, Point::new(5, 1), hunt);
        for _ in 0..5 {
            game.apply(Action::Wait);
        }
        assert_eq!(
            game.map.tile(door),
            Tile::DoorClosed,
            "rats can't open doors"
        );

        let mut game = corridor_game();
        add_monster(&mut game, Kind::Goblin, Point::new(5, 1), hunt);
        for _ in 0..5 {
            game.apply(Action::Wait);
        }
        assert_eq!(game.map.tile(door), Tile::DoorOpen, "goblins can");
    }

    #[test]
    fn unreachable_wander_goal_is_replaced() {
        // A rat on the far side of the closed door, wanting to wander
        // to the player's side, which it can never reach.
        let mut game = corridor_game();
        let goal = Point::new(2, 1);
        let rat = add_monster(
            &mut game,
            Kind::Rat,
            Point::new(5, 1),
            Ai::Wandering { goal },
        );
        game.monsters[rat].energy = 0;
        game.apply(Action::Wait);
        // (4,1) is the only other tile the rat can reach.
        let new_goal = Point::new(4, 1);
        assert_eq!(game.monsters[rat].ai, Ai::Wandering { goal: new_goal });
    }

    #[test]
    fn rats_do_not_open_a_door_they_are_hunting_toward() {
        // The rat last saw the player in the doorway, then the player
        // stepped back and closed the door.
        let mut game = corridor_game();
        let door = Point::new(3, 1);
        add_monster(
            &mut game,
            Kind::Rat,
            Point::new(4, 1),
            Ai::Hunting { last_seen: door },
        );
        for _ in 0..5 {
            game.apply(Action::Wait);
        }
        assert_eq!(game.map.tile(door), Tile::DoorClosed);
    }

    #[test]
    fn walking_into_a_monster_attacks_it_and_wakes_it() {
        let mut game = room_game();
        let rat = add_monster(&mut game, Kind::Rat, Point::new(3, 5), Ai::Asleep);
        game.monsters[rat].hp = 100; // survive the hit
        game.apply(Action::Move(EAST));
        assert_eq!(
            game.player.pos,
            Point::new(2, 5),
            "attacking doesn't move you"
        );
        assert_eq!(game.turn, 1);
        assert!(matches!(game.monsters[rat].ai, Ai::Hunting { .. }));
        assert!(game.log.iter().any(|m| m.text.contains("the rat")));
    }

    #[test]
    fn killing_a_monster_removes_it() {
        let mut game = room_game();
        add_monster(&mut game, Kind::Rat, Point::new(3, 5), Ai::Asleep);
        for _ in 0..30 {
            if game.monsters.is_empty() {
                break;
            }
            game.apply(Action::Move(EAST));
        }
        assert!(game.monsters.is_empty());
        let kill = game.log.iter().find(|m| m.text == "You kill the rat!");
        assert_eq!(kill.map(|m| m.kind), Some(MsgKind::Good));

        // The kill is counted, and the damage matches the rat's health
        // (overkill doesn't count).
        let s = &game.stats;
        assert_eq!(s.kills.get("rat"), Some(&1));
        assert_eq!(s.toughest_kill.map(|k| k.name), Some("rat"));
        let rat_hp = Monster::new(Kind::Rat, Point::default(), Ai::Asleep).hp;
        assert_eq!(s.damage_dealt, rat_hp as u32);
        let swings = game
            .log
            .iter()
            .filter(|m| m.text.starts_with("You "))
            .count();
        assert!(s.hits >= 1 && (s.hits + s.misses) as usize <= swings);
    }

    #[test]
    fn monsters_can_kill_the_player_and_the_game_stops() {
        let mut game = room_game();
        game.player.hp = 1;
        let hunt = Ai::Hunting {
            last_seen: game.player.pos,
        };
        add_monster(&mut game, Kind::Jackal, Point::new(3, 5), hunt);
        for _ in 0..100 {
            game.apply(Action::Wait);
            if game.death.is_some() {
                break;
            }
        }
        let cause = game
            .death_summary()
            .expect("the jackal should win eventually");
        assert!(
            cause.starts_with("Killed by a jackal on depth 1"),
            "{cause}"
        );
        assert_eq!(game.player.hp, 0);

        let turn = game.turn;
        game.apply(Action::Wait);
        assert_eq!(game.turn, turn, "no actions after death");
    }

    #[test]
    fn the_player_slowly_heals() {
        let mut game = corridor_game();
        game.player.hp = 10;
        // 12 healing per turn, 100 per health: 17 turns heal 2.
        for _ in 0..17 {
            game.apply(Action::Wait);
        }
        assert_eq!(game.player.hp, 12);
        game.player.hp = game.player.max_hp;
        for _ in 0..20 {
            game.apply(Action::Wait);
        }
        assert_eq!(game.player.hp, game.player.max_hp, "never above max");
    }

    /// Plays random moves on real floors and checks the rules that must
    /// always hold, whatever happens.
    #[test]
    fn random_play_keeps_invariants() {
        for seed in 0..4 {
            let mut game = Game::new(seed);
            let mut dice = Rng::new(seed + 1000);
            for _ in 0..400 {
                let letter = (b'a' + dice.range(0, 8) as u8) as char;
                let action = match dice.range(0, 18) {
                    0 => Action::Wait,
                    1 => Action::Descend,
                    12 => Action::PickUp,
                    13 => Action::Drop(letter),
                    14 => Action::Equip(letter),
                    15 => Action::Drink(letter),
                    16 => Action::Read {
                        scroll: letter,
                        target: Some((b'a' + dice.range(0, 8) as u8) as char),
                    },
                    // Stand on the stairs now and then to go deeper.
                    2 if dice.chance(5) => {
                        let stairs = game
                            .map
                            .points()
                            .find(|&p| game.map.tile(p) == Tile::StairsDown);
                        game.player.pos = stairs.unwrap();
                        Action::Wait
                    }
                    _ => Action::Move(DIRECTIONS_8[dice.index(8)]),
                };
                game.apply(action);
                if game.death.is_some() {
                    break;
                }

                assert!(game.map.tile(game.player.pos).is_walkable(), "seed {seed}");
                assert!(
                    game.monsters.iter().all(|m| m.hp > 0),
                    "seed {seed}: dead monster"
                );

                let pack = &game.player.inventory;
                for (i, item) in pack.iter().enumerate() {
                    assert!(item.count >= 1, "seed {seed}: empty stack");
                    assert!(pack[i + 1..].iter().all(|o| o.letter != item.letter));
                    assert!(!item.equipped || item.kind.is_equipment());
                }
                let equipped = |f: fn(&crate::item::ItemKind) -> bool| {
                    pack.iter().filter(|i| i.equipped && f(&i.kind)).count()
                };
                use crate::item::ItemKind as K;
                assert!(equipped(|k| matches!(k, K::Weapon(_))) <= 1, "seed {seed}");
                assert!(equipped(|k| matches!(k, K::Armor(_))) <= 1, "seed {seed}");
                assert!(equipped(|k| matches!(k, K::Ring(_))) <= 2, "seed {seed}");
                for (i, fi) in game.items.iter().enumerate() {
                    assert!(game.map.tile(fi.pos).is_walkable(), "seed {seed}");
                    assert!(game.items[i + 1..].iter().all(|o| o.pos != fi.pos));
                }
                for (i, m) in game.monsters.iter().enumerate() {
                    assert!(
                        game.map.tile(m.pos).is_walkable(),
                        "seed {seed}: monster in wall"
                    );
                    assert_ne!(m.pos, game.player.pos, "seed {seed}: monster on player");
                    let overlaps = game.monsters[i + 1..].iter().any(|o| o.pos == m.pos);
                    assert!(!overlaps, "seed {seed}: monsters overlap");
                }
            }
        }
    }

    #[test]
    fn same_seed_same_run() {
        let a = Game::new(1234);
        let b = Game::new(1234);
        assert_eq!(a.player.pos, b.player.pos);
    }

    #[test]
    fn repeated_messages_are_counted_not_duplicated() {
        let mut game = corridor_game();
        game.log("Hello");
        game.log("Hello");
        game.log("Hello");
        assert_eq!(game.log.len(), 1);
        assert_eq!(game.log[0].count, 3);
        game.log("Bye");
        game.log("Hello");
        assert_eq!(game.log.len(), 3, "only consecutive repeats merge");
    }
}
