//! Screen layout: turns the game state into a `Frame`.
//!
//! Layout:
//! ```text
//! +--------------------------------+-----------+
//! |                                |  sidebar  |
//! |          map viewport          |           |
//! |                                |           |
//! +--------------------------------+-----------+
//! |              message log                   |
//! +--------------------------------------------+
//! ```

use crate::frame::{BLACK, Cell, Frame, Rgb};
use crate::game::{Game, MsgKind};
use crate::geom::Point;
use crate::item::{Item, ItemKind};
use crate::lore::Lore;
use crate::map::Tile;
use crate::menu::{self, Line};
use crate::monster::{Ability, Ai, Monster};
use crate::player::{FOOD_MAX, Hunger};
use crate::scores::Score;
use crate::skills::{self, Skill};
use crate::text::article;
use crate::zone::Zone;

const SIDEBAR_WIDTH: i32 = 22;
const LOG_HEIGHT: i32 = 4;
pub const MIN_WIDTH: u16 = 50;
pub const MIN_HEIGHT: u16 = 16;

// A muted palette to suit the grim tone.
const STAIRS_FG: Rgb = Rgb(230, 200, 90);
const PLAYER_FG: Rgb = Rgb(240, 230, 200);
const WATER_FG: Rgb = Rgb(80, 140, 190);
const WATER_BG: Rgb = Rgb(10, 24, 40);
const TEXT: Rgb = Rgb(190, 190, 190);
const TEXT_DIM: Rgb = Rgb(110, 110, 110);
const TITLE: Rgb = Rgb(200, 60, 50);
const BORDER: Rgb = Rgb(60, 60, 60);
const GOOD: Rgb = Rgb(120, 200, 120);
const BAD: Rgb = Rgb(225, 95, 80);
const HEALTH_FULL: Rgb = Rgb(110, 25, 25);
const HEALTH_EMPTY: Rgb = Rgb(35, 18, 18);
/// Bar colors as (filled, empty).
const HEALTH_BAR: (Rgb, Rgb) = (HEALTH_FULL, HEALTH_EMPTY);
const FOOD_BAR: (Rgb, Rgb) = (Rgb(105, 75, 20), Rgb(34, 27, 14));

pub fn draw(game: &Game, width: u16, height: u16) -> Frame {
    let mut frame = Frame::new(width, height);

    if width < MIN_WIDTH || height < MIN_HEIGHT {
        frame.print(0, 0, "Terminal too small.", TEXT);
        frame.print(
            0,
            1,
            &format!("Need at least {MIN_WIDTH}x{MIN_HEIGHT}."),
            TEXT_DIM,
        );
        return frame;
    }

    let (w, _) = (width as i32, height as i32);
    let (view_w, view_h) = map_area(width, height);

    draw_map(&mut frame, game, view_w, view_h);
    draw_dividers(&mut frame, view_w, view_h, w);
    draw_sidebar(&mut frame, game, view_w + 2, view_h);
    draw_log(&mut frame, game, view_h + 1);
    frame
}

/// The size of the map viewport for a given screen size.
fn map_area(width: u16, height: u16) -> (i32, i32) {
    let view_w = width as i32 - SIDEBAR_WIDTH - 1; // -1 for the divider column
    let view_h = height as i32 - LOG_HEIGHT - 1; // -1 for the divider row
    (view_w, view_h)
}

/// Writes a short status, like "bot playing", in the sidebar's empty
/// second row.
pub fn draw_status(frame: &mut Frame, text: &str) {
    if frame.width >= MIN_WIDTH && frame.height >= MIN_HEIGHT {
        let (view_w, _) = map_area(frame.width, frame.height);
        frame.print(view_w + 2, 1, text, GOOD);
    }
}

/// The normal game screen with a pop-up box over the map, showing
/// `page` of the box's lines. Also returns how many pages there are.
pub fn draw_with_box(
    game: &Game,
    size: (u16, u16),
    title: &str,
    lines: &[Line],
    page: usize,
) -> (Frame, usize) {
    let (width, height) = size;
    let mut frame = draw(game, width, height);
    let mut pages = 1;
    if width >= MIN_WIDTH && height >= MIN_HEIGHT {
        pages = menu::draw_box(&mut frame, map_area(width, height), title, lines, page);
    }
    (frame, pages)
}

/// A line per item, like "a) sword (wielded)". Only items for which
/// `show` is true are listed.
pub fn item_list(game: &Game, show: impl Fn(&Item) -> bool) -> Vec<Line> {
    game.player
        .inventory
        .iter()
        .filter(|i| show(i))
        .map(|i| {
            let worn = match (i.equipped, i.kind) {
                (true, ItemKind::Weapon(_)) => " (wielded)",
                (true, _) => " (worn)",
                _ => "",
            };
            let cursed = if i.is_stuck() && i.known {
                " (cursed)"
            } else {
                ""
            };
            let name = game.lore.name(i);
            Line::new(format!("{}) {name}{worn}{cursed}", i.letter), TEXT)
        })
        .collect()
}

/// The inside of the box describing one item, with the keys that act
/// on it.
pub fn item_details(lore: &Lore, item: &Item) -> Vec<Line> {
    let mut lines: Vec<Line> = lore
        .describe(item)
        .into_iter()
        .map(|text| Line::new(text, TEXT))
        .collect();
    let actions = match item.kind {
        ItemKind::Ring(_) if item.equipped => "e) take off   d) drop",
        ItemKind::Ring(_) => "e) put on   d) drop",
        ItemKind::Weapon(_) | ItemKind::Armor(_) if item.equipped => "e) remove   d) drop",
        ItemKind::Weapon(_) | ItemKind::Armor(_) => "e) equip   d) drop",
        ItemKind::Potion(_) => "q) drink   d) drop",
        ItemKind::Scroll(_) => "r) read   d) drop",
        ItemKind::Food(_) => "E) eat   d) drop",
    };
    lines.push(Line::new("", TEXT));
    lines.push(Line::new(actions, TEXT_DIM));
    lines
}

/// The character sheet: level, attributes, combat numbers and skills.
pub fn character_lines(game: &Game) -> Vec<Line> {
    let p = &game.player;
    // Work out combat numbers as the player knows them: unidentified
    // enchantments count as 0, and the numbers get a "?" so the sheet
    // can't be used to identify gear by comparing before and after.
    let mut known = p.clone();
    let mut unsure = false;
    for item in known
        .inventory
        .iter_mut()
        .filter(|i| i.equipped && !i.known)
    {
        // Any unknown piece gets the "?", even a +0 one: leaving it
        // off would itself reveal the +0.
        unsure = true;
        item.enchant = 0;
    }
    let (attack, defense) = (known.attack(), known.defense());
    let q = if unsure { "?" } else { "" };
    let next = skills::xp_for_level(p.level + 1);
    let mut lines = vec![
        Line::new(
            format!("Level {}   Experience {}/{next}", p.level, p.xp),
            TEXT,
        ),
        Line::new(format!("Health {}/{}", p.hp, p.max_hp), TEXT),
        Line::new(
            format!("Food {}/{FOOD_MAX} {}", p.food, p.hunger().label()),
            TEXT,
        ),
        Line::new(
            format!(
                "Strength {}   Agility {}   Intellect {}",
                p.strength, p.agility, p.intellect
            ),
            TEXT,
        ),
        Line::new(
            format!(
                "Attack: accuracy {}{q}, damage {}-{}{q}",
                attack.accuracy, attack.damage.0, attack.damage.1
            ),
            TEXT,
        ),
        Line::new(
            format!(
                "Defense: dodge {}{q}, armor {}{q}   Sight {}",
                defense.dodge,
                defense.armor,
                p.sight_radius()
            ),
            TEXT,
        ),
        Line::new("", TEXT),
        Line::new("Skills improve as you use them:", TITLE),
    ];
    for skill in Skill::ALL {
        let level = p.skills.level(skill);
        let progress = if level >= skills::MAX_SKILL {
            "mastered".to_string()
        } else {
            let (have, need) = p.skills.progress(skill);
            format!("{have}/{need} to next")
        };
        lines.push(Line::new(
            format!("{:<8} {level:>2}   {progress}", skill.name()),
            TEXT,
        ));
        lines.push(Line::new(
            format!("            {}", skill.about()),
            TEXT_DIM,
        ));
    }
    lines
}

/// Every key, for the help box.
pub fn help_lines() -> Vec<Line> {
    let heading = |t: &str| Line::new(t, GOOD);
    let text = |t: &str| Line::new(t, TEXT);
    let mut lines = vec![heading("Keys")];
    lines.extend(
        [
            "arrows or hjkl   move, or attack by moving into",
            "yubn             move diagonally",
            ".                wait a turn (rest)",
            ">                descend, or walk to seen stairs",
            "x                explore until something happens",
            "c                close a door",
            "g                pick up (walking over also works)",
            "i                inventory",
            "C                character: level, attributes, skills",
            "e                equip or remove",
            "d                drop",
            "q                drink a potion",
            "r                read a scroll",
            "E                eat",
            "L or ;           look at what's in view",
            "m                message history",
            "?                this help",
            "B                let the bot play (B again stops it)",
            "Q                quit (ends the run)",
        ]
        .map(text),
    );
    lines.push(text(""));
    lines.push(heading("Symbols"));
    lines.extend(
        [
            "@  you               #  wall",
            ".  floor             ~  shallow water",
            "+  closed door       '  open door",
            ">  stairs down       ^  a trap you've found",
            ")  weapon            [  armor",
            "!  potion            ?  scroll",
            "=  ring              %  food",
            "Letters are monsters. Look (L) says which.",
        ]
        .map(text),
    );
    lines.push(text(""));
    lines.push(heading("How to play"));
    lines.extend(
        [
            "Go as deep as you can: depth is your score.",
            "Every six floors the dungeon changes: the Crypts,",
            "the Flooded Halls, then the Deep Warrens. After",
            "that it starts over, and is deadlier each time.",
            "Kill monsters to gain levels. Skills grow by use.",
            "Resting heals, but you grow hungry. Eat before you",
            "weaken, or you starve.",
            "Potions and scrolls are unknown until used. Gear",
            "hides its enchantment until worn for a while.",
            "Cursed gear sticks for a while; enchanting it",
            "breaks the curse.",
        ]
        .map(text),
    );
    lines
}

/// The message log, newest first, for looking back at what happened.
pub fn history_lines(game: &Game) -> Vec<Line> {
    game.log
        .iter()
        .rev()
        .take(HISTORY_LENGTH)
        .map(|m| {
            let text = if m.count > 1 {
                format!("{} (x{})", m.text, m.count)
            } else {
                m.text.clone()
            };
            Line::new(text, message_color(m.kind))
        })
        .collect()
}

/// How far back the message history goes.
const HISTORY_LENGTH: usize = 200;

fn message_color(kind: MsgKind) -> Rgb {
    match kind {
        MsgKind::Info => TEXT,
        MsgKind::Good => GOOD,
        MsgKind::Bad => BAD,
    }
}

/// Everything of note in view, nearest first: monsters with what they
/// are doing, how hurt they look and what makes them dangerous, then
/// items and known traps. Uses only what the player can see.
pub fn look_lines(game: &Game) -> Vec<Line> {
    let pos = game.player.pos;
    let steps = |p: Point| {
        let d = p - pos;
        d.x.abs().max(d.y.abs())
    };
    let mut monsters: Vec<&Monster> = game
        .monsters
        .iter()
        .filter(|m| game.is_visible(m.pos))
        .collect();
    monsters.sort_by_key(|m| steps(m.pos));
    let mut lines = Vec::new();
    for m in monsters {
        let s = m.species();
        let doing = match m.ai {
            Ai::Asleep => "asleep",
            Ai::Wandering { .. } => "wandering",
            Ai::Hunting { .. } => "hunting you",
            Ai::Fleeing => "fleeing",
        };
        lines.push(Line::new(
            format!("{} {}, {doing}, {}", s.glyph, s.name, how_hurt(m)),
            s.color,
        ));
        let traits = monster_traits(m);
        if !traits.is_empty() {
            lines.push(Line::new(format!("  {}", traits.join(", ")), TEXT_DIM));
        }
    }
    let mut items: Vec<_> = game
        .items
        .iter()
        .filter(|fi| game.is_visible(fi.pos))
        .collect();
    items.sort_by_key(|fi| steps(fi.pos));
    for fi in items {
        let name = game.lore.with_article(&fi.item);
        lines.push(Line::new(format!("{} {name}", fi.item.kind.glyph()), TEXT));
    }
    for t in game
        .traps
        .iter()
        .filter(|t| t.known && game.is_visible(t.pos))
    {
        lines.push(Line::new(
            format!("^ {} {}", article(t.kind.name()), t.kind.name()),
            t.kind.color(),
        ));
    }
    if lines.is_empty() {
        lines.push(Line::new("Nothing of note in view.", TEXT_DIM));
    }
    lines
}

/// How hurt a monster looks, in words, from its health bar.
fn how_hurt(m: &Monster) -> &'static str {
    let percent = m.hp.max(0) * 100 / m.max_hp().max(1);
    match percent {
        100.. => "unhurt",
        67.. => "lightly hurt",
        34.. => "hurt",
        _ => "badly hurt",
    }
}

/// What a player would soon learn about a monster: its speed and
/// special powers.
fn monster_traits(m: &Monster) -> Vec<&'static str> {
    let s = m.species();
    let mut traits = Vec::new();
    match s.speed {
        200.. => traits.push("very fast"),
        101.. => traits.push("fast"),
        ..=74 => traits.push("very slow"),
        75..=99 => traits.push("slow"),
        _ => {}
    }
    if s.pack.1 > 1 {
        traits.push("hunts in packs");
    }
    for ability in s.abilities {
        traits.push(match ability {
            Ability::Regenerates => "heals over time",
            Ability::DrainsMaxHealth => "drains maximum health",
            Ability::DrinksBlood => "heals by biting",
            Ability::StealsAndFlees => "steals and runs",
            Ability::CorrodesArmor => "corrodes armor",
            Ability::Splits => "splits when hit",
        });
    }
    traits
}

/// Finds the map coordinate shown at the viewport's left or top edge,
/// along one axis.
///
/// - If the map fits in the view, it is centered (the result is
///   negative, meaning blank space before the map starts).
/// - Otherwise the camera follows the player, but stops at the map's
///   edges so we never scroll into empty space.
fn camera_origin(player: i32, map_len: i32, view_len: i32) -> i32 {
    if map_len <= view_len {
        -(view_len - map_len) / 2
    } else {
        (player - view_len / 2).clamp(0, map_len - view_len)
    }
}

fn draw_map(frame: &mut Frame, game: &Game, view_w: i32, view_h: i32) {
    let zone = game.place().zone();
    let origin = Point::new(
        camera_origin(game.player.pos.x, game.map.width(), view_w),
        camera_origin(game.player.pos.y, game.map.height(), view_h),
    );

    for sy in 0..view_h {
        for sx in 0..view_w {
            let p = origin + Point::new(sx, sy);
            // Three cases: in sight (full color), remembered (cold and
            // dim), or never seen (left blank).
            if game.is_visible(p) {
                frame.set(sx, sy, tile_cell(game.map.tile(p), zone));
            } else if game.map.is_revealed(p) {
                let cell = tile_cell(game.map.tile(p), zone);
                frame.set(
                    sx,
                    sy,
                    Cell {
                        fg: cell.fg.remembered(),
                        bg: cell.bg.remembered(),
                        ..cell
                    },
                );
            }
        }
    }

    // Items don't move on their own, so remembered ones stay drawn,
    // dimmed, like the tiles under them.
    for fi in &game.items {
        let s = fi.pos - origin;
        if s.x < 0 || s.y < 0 || s.x >= view_w || s.y >= view_h {
            continue;
        }
        let fg = game.lore.color(fi.item.kind);
        let fg = if game.is_visible(fi.pos) {
            fg
        } else if game.map.is_revealed(fi.pos) {
            fg.remembered()
        } else {
            continue;
        };
        frame.set(
            s.x,
            s.y,
            Cell {
                ch: fi.item.kind.glyph(),
                fg,
                bg: BLACK,
            },
        );
    }

    // Known traps, like items, stay drawn once seen.
    for t in game.traps.iter().filter(|t| t.known) {
        let s = t.pos - origin;
        if s.x < 0 || s.y < 0 || s.x >= view_w || s.y >= view_h {
            continue;
        }
        let fg = if game.is_visible(t.pos) {
            t.kind.color()
        } else if game.map.is_revealed(t.pos) {
            t.kind.color().remembered()
        } else {
            continue;
        };
        frame.set(
            s.x,
            s.y,
            Cell {
                ch: '^',
                fg,
                bg: BLACK,
            },
        );
    }

    // Monsters are only drawn while in sight. There is no memory of
    // where a monster was: it may have moved.
    for m in game.monsters.iter().filter(|m| game.is_visible(m.pos)) {
        let s = m.pos - origin;
        // On a small terminal a visible monster can be outside the map
        // area; drawing it anyway would scribble over the sidebar.
        if s.x < 0 || s.y < 0 || s.x >= view_w || s.y >= view_h {
            continue;
        }
        let species = m.species();
        frame.set(
            s.x,
            s.y,
            Cell {
                ch: species.glyph,
                fg: species.color,
                bg: BLACK,
            },
        );
    }

    let s = game.player.pos - origin;
    frame.set(
        s.x,
        s.y,
        Cell {
            ch: '@',
            fg: PLAYER_FG,
            bg: BLACK,
        },
    );
}

/// How each kind of tile looks in a zone.
fn tile_cell(tile: Tile, zone: &Zone) -> Cell {
    let (ch, fg, bg) = match tile {
        Tile::Wall => ('#', zone.wall_fg, zone.wall_bg),
        Tile::Floor => ('.', zone.floor_fg, BLACK),
        Tile::DoorClosed => ('+', zone.door_fg, BLACK),
        Tile::DoorOpen => ('\'', zone.door_fg, BLACK),
        Tile::StairsDown => ('>', STAIRS_FG, BLACK),
        Tile::Water => ('~', WATER_FG, WATER_BG),
    };
    Cell { ch, fg, bg }
}

fn draw_dividers(frame: &mut Frame, view_w: i32, view_h: i32, w: i32) {
    let line = |ch| Cell {
        ch,
        fg: BORDER,
        bg: BLACK,
    };
    for y in 0..view_h {
        frame.set(view_w, y, line('│'));
    }
    for x in 0..w {
        frame.set(x, view_h, line('─'));
    }
    frame.set(view_w, view_h, line('┴'));
}

fn draw_sidebar(frame: &mut Frame, game: &Game, x: i32, height: i32) {
    let width = SIDEBAR_WIDTH - 2;
    let p = &game.player;
    frame.print(x, 0, "SMALLROGUE", TITLE);
    let place = game.place();
    frame.print(x, 1, &place.title(), place.zone().wall_fg);
    let health = format!("Health {}/{}", p.hp, p.max_hp);
    draw_bar(
        frame,
        (x, 2, width),
        &health,
        (p.hp, p.max_hp),
        TEXT,
        HEALTH_BAR,
    );
    // The food bar names the hunger stage once there is one, in red when
    // it starts to hurt.
    let hunger = p.hunger();
    let food = format!("Food {} {}", p.food, hunger.label());
    let food_color = if hunger >= Hunger::Weak { BAD } else { TEXT };
    draw_bar(
        frame,
        (x, 3, width),
        &food,
        (p.food, FOOD_MAX),
        food_color,
        FOOD_BAR,
    );
    frame.print(
        x,
        4,
        &format!("Str {}  Agi {}  Int {}", p.strength, p.agility, p.intellect),
        TEXT,
    );
    frame.print(
        x,
        5,
        &format!("Depth {}  Turn {}", game.depth, game.turn),
        TEXT,
    );
    let next = skills::xp_for_level(p.level + 1);
    frame.print(
        x,
        6,
        &format!("Level {}  XP {}/{next}", p.level, p.xp),
        TEXT,
    );
    frame.print(x, 7, &format!("Seed {}", game.seed), TEXT_DIM);
    // Equipment: weapon, armor, then rings, one line each.
    let gear: Vec<&Item> = p
        .weapon()
        .into_iter()
        .chain(p.armor())
        .chain(p.rings())
        .collect();
    for (i, item) in gear.iter().enumerate() {
        let y = 8 + i as i32;
        frame.set(
            x,
            y,
            Cell {
                ch: item.kind.glyph(),
                fg: game.lore.color(item.kind),
                bg: BLACK,
            },
        );
        frame.print(x + 2, y, &game.lore.name(item), TEXT);
    }

    // Key hints sit at the bottom of the sidebar.
    let keys = ["?  all keys", "i  inventory"];
    // On short terminals the hints would cover the status above, which
    // matters more, so they are left out.
    let list_top = 9 + gear.len() as i32;
    let keys_top = height - keys.len() as i32;
    let show_keys = keys_top >= list_top;
    if show_keys {
        for (i, line) in keys.iter().enumerate() {
            frame.print(x, keys_top + i as i32, line, TEXT_DIM);
        }
    }

    // Monsters in view, nearest first, in the space between. Each gets
    // a health bar behind its name.
    let mut in_view: Vec<&Monster> = game
        .monsters
        .iter()
        .filter(|m| game.is_visible(m.pos))
        .collect();
    in_view.sort_by_key(|m| m.pos.dist_sq(game.player.pos));
    let list_bottom = if show_keys { keys_top - 1 } else { height };
    let room = (list_bottom - list_top).max(0) as usize;
    for (i, m) in in_view.iter().take(room).enumerate() {
        let y = list_top + i as i32;
        let species = m.species();
        frame.set(
            x,
            y,
            Cell {
                ch: species.glyph,
                fg: species.color,
                bg: BLACK,
            },
        );
        let state = match m.ai {
            Ai::Asleep => "asleep",
            Ai::Wandering { .. } => "wandering",
            Ai::Hunting { .. } => "hunting",
            Ai::Fleeing => "fleeing",
        };
        draw_bar(
            frame,
            (x + 2, y, width - 2),
            &monster_label(species.name, state, width - 2),
            (m.hp, m.max_hp()),
            TEXT,
            HEALTH_BAR,
        );
    }
}

/// "name  state" for a monster's sidebar bar. Long names like "giant
/// spider" get a shortened state so both fit.
fn monster_label(name: &str, state: &str, width: i32) -> String {
    let name_width = name.chars().count() as i32;
    if name_width + 1 + state.len() as i32 <= width {
        let pad = (width - state.len() as i32).max(name_width + 1) as usize;
        format!("{name:<pad$}{state}")
    } else {
        let room = (width - name_width - 1).max(0) as usize;
        format!("{name} {}", &state[..room.min(state.len())])
    }
}

/// Draws `label` on top of a bar whose filled part shows
/// `current / max`. `area` is (x, y, width); `colors` are the bar's
/// (filled, empty) backgrounds.
fn draw_bar(
    frame: &mut Frame,
    area: (i32, i32, i32),
    label: &str,
    amount: (i32, i32),
    fg: Rgb,
    colors: (Rgb, Rgb),
) {
    let (x, y, width) = area;
    let (current, max) = amount;
    // Round up, so anything alive shows at least one filled cell.
    let filled = if max > 0 {
        (current.max(0) * width + max - 1) / max
    } else {
        0
    };
    let mut label = label.chars();
    for i in 0..width {
        let ch = label.next().unwrap_or(' ');
        let bg = if i < filled { colors.0 } else { colors.1 };
        frame.set(x + i, y, Cell { ch, fg, bg });
    }
}

fn draw_log(frame: &mut Frame, game: &Game, top: i32) {
    // Show the newest messages, newest at the bottom, older ones dimmer.
    let shown = game.log.iter().rev().take(LOG_HEIGHT as usize);
    for (i, msg) in shown.enumerate() {
        let color = message_color(msg.kind);
        let color = if i == 0 { color } else { faded(color) };
        let text = if msg.count > 1 {
            format!("{} (x{})", msg.text, msg.count)
        } else {
            msg.text.clone()
        };
        frame.print(1, top + LOG_HEIGHT - 1 - i as i32, &text, color);
    }
}

/// A darker version of a color, for older log lines.
fn faded(c: Rgb) -> Rgb {
    let f = |v: u8| (v as u32 * 60 / 100) as u8;
    Rgb(f(c.0), f(c.1), f(c.2))
}

/// The high score list as it stands after this run, for the death
/// screen.
pub struct Board<'a> {
    pub scores: &'a [Score],
    /// This run's place in `scores`, if it made the list.
    pub this_run: Option<usize>,
    /// Said above the list, e.g. why this run isn't on it.
    pub note: Option<&'a str>,
}

/// One block of lines on the death screen, drawn left-aligned and
/// centered as a whole so columns line up.
type Block = Vec<(String, Rgb)>;

/// The screen shown after death: how it happened, what the run
/// achieved, the high scores, and the last few messages.
///
/// Blocks are added in order of importance while they fit, so a small
/// terminal still shows the cause of death and the key stats.
pub fn draw_death(game: &Game, board: &Board, width: u16, height: u16) -> Frame {
    let mut frame = Frame::new(width, height);
    let (w, h) = (width as i32, height as i32);

    let cause = game
        .death_summary()
        .unwrap_or_else(|| "You died.".to_string());
    let heading: Block = vec![
        ("You have died.".to_string(), TITLE),
        (String::new(), TEXT),
        (cause, TEXT),
        (
            format!("Character level {}, seed {}.", game.player.level, game.seed),
            TEXT_DIM,
        ),
    ];
    let footer: Block = vec![("Press any key to leave the dungeon.".to_string(), TEXT)];

    // Heading and footer always show. The other blocks follow in
    // order of importance while they fit, each after a blank line, and
    // each needing at least its title and one line.
    let mut room = h - heading.len() as i32 - 2;
    let mut blocks = vec![(heading, true)];
    let optional: [&dyn Fn(usize) -> Block; 3] = [
        &|lines| death_stats(game, lines),
        &|lines| death_scores(board, w, lines),
        &|lines| recent_messages(game, lines),
    ];
    for block in optional {
        let lines = room - 1;
        if lines < 2 {
            break;
        }
        let mut block = block(lines as usize);
        // Builders aim to fit, but the budget is enforced here.
        block.truncate(lines as usize);
        room -= block.len() as i32 + 1;
        blocks.push((block, false));
    }
    blocks.push((footer, true));

    let total: i32 = blocks.iter().map(|(b, _)| b.len() as i32 + 1).sum::<i32>() - 1;
    let mut y = ((h - total) / 2).max(0);
    let text_w = |t: &str| t.chars().count() as i32;
    for (block, centered) in &blocks {
        // Tables share a left edge so their columns line up.
        let block_w = block.iter().map(|(t, _)| text_w(t)).max().unwrap_or(0);
        for (text, color) in block {
            let line_w = if *centered { text_w(text) } else { block_w };
            frame.print(((w - line_w) / 2).max(0), y, text, *color);
            y += 1;
        }
        y += 1;
    }
    frame
}

/// The run in numbers, most interesting first, in at most `lines`.
fn death_stats(game: &Game, lines: usize) -> Block {
    let s = &game.stats;
    let pair = |a: (&str, String), b: (&str, String)| {
        (
            format!("{:<15}{:>7}   {:<15}{:>7}", a.0, a.1, b.0, b.1),
            TEXT,
        )
    };
    let n = |v: u32| v.to_string();
    let accuracy = s.accuracy().map_or("-".to_string(), |a| format!("{a}%"));
    let mut block = vec![("The run".to_string(), GOOD)];
    if let Some((name, count)) = s.most_killed() {
        block.push((format!("Most slain: {name} ({count})"), TEXT));
    }
    if let Some(k) = s.toughest_kill {
        block.push((
            format!(
                "Toughest foe slain: {} {}, on depth {}",
                article(k.name),
                k.name,
                k.depth
            ),
            TEXT,
        ));
    }
    block.extend([
        pair(
            ("Monsters slain", n(s.total_kills())),
            ("Accuracy", accuracy),
        ),
        pair(
            ("Damage dealt", n(s.damage_dealt)),
            ("Damage taken", n(s.damage_taken)),
        ),
        pair(
            ("Stairs taken", n(s.stairs_taken)),
            ("Trapdoor falls", n(s.trapdoor_falls)),
        ),
        pair(
            ("Items found", n(s.items_picked_up)),
            ("Traps sprung", n(s.traps_sprung)),
        ),
        pair(
            ("Potions drunk", n(s.potions_drunk)),
            ("Scrolls read", n(s.scrolls_read)),
        ),
        pair(
            ("Meals eaten", n(s.meals_eaten)),
            ("Experience", n(game.player.xp)),
        ),
    ]);
    block.truncate(lines);
    block
}

/// The high score table in at most `lines`. Narrow terminals drop the
/// kills and date columns. This run's row is marked, and stays in view
/// when the table is cut short.
fn death_scores(board: &Board, width: i32, lines: usize) -> Block {
    let mut block = vec![("High scores".to_string(), GOOD)];
    if let Some(note) = board.note {
        block.push((note.to_string(), TEXT_DIM));
    }
    if board.scores.is_empty() {
        block.push(("No runs recorded yet.".to_string(), TEXT_DIM));
        return block;
    }
    let wide = width >= 70;
    let killer_w = if wide {
        20
    } else {
        (width as usize).saturating_sub(34).clamp(8, 20)
    };
    let row = |mark: &str,
               place: &str,
               depth: &str,
               level: &str,
               turns: &str,
               kills: &str,
               killer: &str,
               date: &str| {
        let killer: String = killer.chars().take(killer_w).collect();
        if wide {
            format!(
                "{mark}{place:>2}  {depth:>5}  {level:>5}  {turns:>6}  {kills:>5}  {killer:<killer_w$}  {date}"
            )
        } else {
            format!("{mark}{place:>2}  {depth:>5}  {level:>5}  {turns:>6}  {killer}")
        }
    };
    block.push((
        row(
            "  ",
            "#",
            "Depth",
            "Level",
            "Turns",
            "Kills",
            "Killed by",
            "Date",
        ),
        TEXT_DIM,
    ));
    let rows: Vec<(String, Rgb)> = board
        .scores
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let this = board.this_run == Some(i);
            let line = row(
                if this { "> " } else { "  " },
                &(i + 1).to_string(),
                &s.depth.to_string(),
                &s.level.to_string(),
                &s.turns.to_string(),
                &s.kills.to_string(),
                &s.killer,
                &s.date,
            );
            (line, if this { STAIRS_FG } else { TEXT })
        })
        .collect();
    let fit = lines.saturating_sub(block.len());
    match board.this_run.filter(|&i| i >= fit && fit > 0) {
        // Out of view: show the top of the table, then this run.
        Some(i) => {
            block.extend(rows[..fit - 1].iter().cloned());
            block.push(rows[i].clone());
        }
        None => block.extend(rows.into_iter().take(fit)),
    }
    block.truncate(lines);
    block
}

fn recent_messages(game: &Game, lines: usize) -> Block {
    let mut block: Block = game
        .log
        .iter()
        .rev()
        .take(lines.saturating_sub(1).min(5))
        .rev()
        .map(|m| (m.text.clone(), TEXT_DIM))
        .collect();
    block.insert(0, ("Last messages".to_string(), GOOD));
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads one screen row back out of a frame as a string.
    fn row_text(frame: &Frame, y: u16) -> String {
        (0..frame.width).map(|x| frame.get(x, y).ch).collect()
    }

    #[test]
    fn newest_message_is_on_bottom_row() {
        let mut game = Game::new(1);
        game.log("newest");
        let frame = draw(&game, 80, 24);
        assert!(row_text(&frame, 23).contains("newest"));
        assert!(row_text(&frame, 22).contains("You enter the Crypts."));
        assert!(row_text(&frame, 21).contains("You descend"));
    }

    /// Finds a tile the player can see that lies outside the map area
    /// of a small terminal, puts a rat there, and checks the rat is not
    /// drawn over the sidebar or log.
    #[test]
    fn monsters_outside_the_map_view_are_not_drawn() {
        use crate::monster::{Ai, Kind, Monster};
        let (w, h) = (MIN_WIDTH, MIN_HEIGHT);
        let view_w = w as i32 - SIDEBAR_WIDTH - 1;
        let view_h = h as i32 - LOG_HEIGHT - 1;

        for seed in 0..50 {
            let mut game = Game::new(seed);
            game.monsters.clear();
            let origin = Point::new(
                camera_origin(game.player.pos.x, game.map.width(), view_w),
                camera_origin(game.player.pos.y, game.map.height(), view_h),
            );
            let outside = game.map.points().find(|&p| {
                let s = p - origin;
                let off_view = s.x >= view_w || s.y >= view_h;
                let on_screen = s.x >= 0 && s.y >= 0 && s.x < w as i32 && s.y < h as i32;
                game.is_visible(p) && off_view && on_screen && p != game.player.pos
            });
            let Some(p) = outside else { continue };
            game.monsters.push(Monster::new(Kind::Rat, p, Ai::Asleep));
            let frame = draw(&game, w, h);
            let s = p - origin;
            assert_ne!(frame.get(s.x as u16, s.y as u16).ch, 'r', "seed {seed}");
            return;
        }
        panic!("no seed produced a visible tile outside the view");
    }

    #[test]
    fn status_stays_readable_on_the_smallest_terminal() {
        let game = Game::new(1);
        for height in MIN_HEIGHT..MIN_HEIGHT + 12 {
            let frame = draw(&game, MIN_WIDTH, height);
            assert!(row_text(&frame, 3).contains("Food 1800"), "height {height}");
            assert!(row_text(&frame, 5).contains("Depth 1"), "height {height}");
            assert!(row_text(&frame, 6).contains("Level 1"), "height {height}");
            assert!(row_text(&frame, 7).contains("Seed 1"), "height {height}");
            assert!(row_text(&frame, 8).contains("sword"), "height {height}");
        }
    }

    /// Collects every screen row from every page of a box.
    fn all_pages(game: &Game, size: (u16, u16), lines: &[Line]) -> String {
        let (_, pages) = draw_with_box(game, size, "T", lines, 0);
        (0..pages)
            .flat_map(|page| {
                let (frame, _) = draw_with_box(game, size, "T", lines, page);
                (0..size.1).map(move |y| row_text(&frame, y))
            })
            .collect()
    }

    #[test]
    fn every_menu_line_is_reachable_by_paging() {
        use crate::item::{Item, ItemKind, WeaponKind};
        let mut game = Game::new(1);
        while game.player.inventory.len() < crate::player::PACK_SIZE {
            let dagger = Item::new(ItemKind::Weapon(WeaponKind::Dagger));
            game.player.add_item(dagger).unwrap();
        }
        let pack = all_pages(&game, (80, 24), &item_list(&game, |_| true));
        for letter in 'a'..='z' {
            assert!(
                pack.contains(&format!("{letter}) ")),
                "item {letter} never shown"
            );
        }

        let help = all_pages(&game, (MIN_WIDTH, MIN_HEIGHT), &help_lines());
        for line in help_lines() {
            // Lines may be clipped on the right at this width; the key
            // column at the start must still show.
            let start: String = line.text.chars().take(10).collect();
            assert!(help.contains(&start), "help line {start:?} never shown");
        }
    }

    fn attack_line(game: &Game) -> String {
        character_lines(game)
            .into_iter()
            .find(|l| l.text.starts_with("Attack:"))
            .expect("the sheet has an attack line")
            .text
    }

    /// Equipping unidentified gear must not change the sheet's numbers,
    /// or comparing before and after would identify it for free.
    #[test]
    fn character_sheet_does_not_leak_enchantments() {
        use crate::item::{Item, WeaponKind};
        let mut game = Game::new(1);
        let before = attack_line(&game);
        let sword = Item::enchanted(ItemKind::Weapon(WeaponKind::Sword), 2);
        let letter = game.player.add_item(sword).unwrap();
        game.player.item_mut('a').unwrap().equipped = false;
        game.player.item_mut(letter).unwrap().equipped = true;
        let after = attack_line(&game);
        assert_eq!(after.replace('?', ""), before, "the +2 shows through");
        assert!(after.contains('?'));
        game.player.item_mut(letter).unwrap().known = true;
        assert!(!attack_line(&game).contains('?'));

        // An unknown +0 item still gets the "?".
        let plain = Item::new(ItemKind::Weapon(WeaponKind::Mace));
        let mace = game.player.add_item(plain).unwrap();
        game.player.item_mut(letter).unwrap().equipped = false;
        game.player.item_mut(mace).unwrap().equipped = true;
        assert!(attack_line(&game).contains('?'));
    }

    #[test]
    fn character_sheet_lists_every_skill() {
        let mut game = Game::new(1);
        game.player.skills.train(Skill::Melee, 12);
        let text: String = character_lines(&game)
            .iter()
            .map(|l| l.text.clone() + "\n")
            .collect();
        assert!(text.contains("Level 1   Experience 0/10"), "{text}");
        assert!(text.contains("melee     1   2/20 to next"), "{text}");
        for skill in Skill::ALL {
            assert!(text.contains(skill.name()));
        }
    }

    #[test]
    fn help_fits_an_80_column_screen_and_covers_the_basics() {
        let lines = help_lines();
        let (view_w, _) = map_area(80, 24);
        // The box keeps two columns of border and padding each side.
        let inner = (view_w - 4) as usize;
        for l in &lines {
            assert!(l.text.chars().count() <= inner, "too wide: {}", l.text);
        }
        let all: String = lines.iter().map(|l| l.text.as_str()).collect();
        for needle in [
            "Symbols",
            "How to play",
            "~  shallow water",
            "L or ;",
            "m   ",
        ] {
            assert!(all.contains(needle), "missing {needle}");
        }
    }

    #[test]
    fn history_is_newest_first_with_repeats_counted() {
        let mut game = Game::new(1);
        game.log("first");
        game.log("again");
        game.log("again");
        let lines = history_lines(&game);
        assert_eq!(lines[0].text, "again (x2)");
        assert_eq!(lines[1].text, "first");
    }

    #[test]
    fn look_describes_monsters_items_and_traps_in_view() {
        use crate::item::{FloorItem, FoodKind, Item};
        use crate::monster::Kind;
        use crate::trap::{Trap, TrapKind};
        let mut game = Game::new(1);
        let mut map = crate::map::Map::new_filled(20, 9);
        map.carve_room(1, 1, 18, 7);
        game.place_on_map(map, Point::new(5, 4));
        game.monsters.clear();
        game.items.clear();
        game.traps.clear();
        game.update_fov();
        assert_eq!(look_lines(&game)[0].text, "Nothing of note in view.");

        let (a, b, c) = (Point::new(6, 4), Point::new(8, 4), Point::new(3, 4));
        let mut vampire = Monster::new(Kind::Vampire, b, Ai::Asleep);
        vampire.hp /= 2;
        game.monsters.push(vampire);
        game.items.push(FloorItem {
            pos: a,
            item: Item::new(ItemKind::Food(FoodKind::Ration)),
        });
        game.traps.push(Trap {
            pos: c,
            kind: TrapKind::Dart,
            known: true,
        });
        game.update_fov();
        let text: Vec<String> = look_lines(&game).into_iter().map(|l| l.text).collect();
        assert_eq!(text[0], "V vampire, asleep, hurt");
        assert_eq!(text[1], "  fast, heals over time, heals by biting");
        assert!(text[2].starts_with("% "), "{text:?}");
        assert_eq!(text[3], "^ a dart trap");
    }

    /// A dead player with some history, and a full score list with
    /// this run in eighth place.
    fn death_fixture() -> (Game, Vec<Score>) {
        use crate::stats::Kill;
        let mut game = Game::new(1);
        game.death = Some("a troll".to_string());
        for (name, xp) in [("orc", 10), ("orc", 10), ("troll", 40)] {
            game.stats.record_kill(Kill { name, xp, depth: 7 });
        }
        game.stats.hits = 3;
        game.stats.misses = 1;
        let scores = (0..10)
            .map(|i| Score {
                depth: 20 - i,
                level: 10,
                turns: 1000,
                kills: 50,
                seed: i as u64,
                date: "2026-09-29".to_string(),
                killer: if i == 7 { "a troll" } else { "an orc" }.to_string(),
            })
            .collect();
        (game, scores)
    }

    fn screen_text(frame: &Frame) -> Vec<String> {
        (0..frame.height).map(|y| row_text(frame, y)).collect()
    }

    #[test]
    fn death_screen_shows_the_cause_stats_and_scores() {
        let (game, scores) = death_fixture();
        let board = Board {
            scores: &scores,
            this_run: Some(7),
            note: None,
        };
        let text = screen_text(&draw_death(&game, &board, 80, 30)).join("\n");
        assert!(text.contains("You have died."));
        assert!(text.contains("Killed by a troll"));
        assert!(text.contains("Most slain: orc (2)"), "{text}");
        assert!(text.contains("Toughest foe slain: a troll, on depth 7"));
        assert!(text.contains("Accuracy") && text.contains("75%"));
        assert!(text.contains("High scores"));
        assert!(text.contains(">  8"), "this run is marked\n{text}");
        assert!(text.contains("Press any key"));
    }

    #[test]
    fn a_small_death_screen_keeps_this_run_in_view() {
        let (game, scores) = death_fixture();
        let board = Board {
            scores: &scores,
            this_run: Some(7),
            note: None,
        };
        let (w, h) = (MIN_WIDTH, 24);
        let lines = screen_text(&draw_death(&game, &board, w, h));
        let text = lines.join("\n");
        assert!(text.contains(">  8"), "this run is still shown\n{text}");
        assert!(text.contains("Press any key"));
        // Nothing runs off the right edge: the last column only ever
        // holds the end of a line, never the middle of a word.
        for line in &lines {
            assert!(line.trim_end().chars().count() <= w as usize, "{line}");
        }
    }

    #[test]
    fn the_smallest_death_screen_still_shows_the_essentials() {
        let (game, scores) = death_fixture();
        let board = Board {
            scores: &scores,
            this_run: None,
            note: Some("The bot played this run, so it isn't recorded."),
        };
        let text = screen_text(&draw_death(&game, &board, MIN_WIDTH, MIN_HEIGHT)).join("\n");
        assert!(text.contains("Killed by a troll"));
        assert!(text.contains("Monsters slain"), "{text}");
        assert!(text.contains("Press any key"));
    }

    /// Found by the Codex review: with no scores yet and a note, the
    /// score block once ran over its space and hid the prompt.
    #[test]
    fn the_prompt_always_shows_on_every_supported_size() {
        let (game, scores) = death_fixture();
        let note = Some("The bot played this run, so it isn't recorded.");
        let boards = [
            Board {
                scores: &[],
                this_run: None,
                note,
            },
            Board {
                scores: &scores,
                this_run: Some(9),
                note,
            },
        ];
        for board in &boards {
            for h in MIN_HEIGHT..=45 {
                for w in [MIN_WIDTH, 64, 80, 120] {
                    let lines = screen_text(&draw_death(&game, board, w, h));
                    assert!(
                        lines.iter().any(|l| l.contains("Press any key")),
                        "{w}x{h}\n{}",
                        lines.join("\n")
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "prints sample death screens; run with --ignored --nocapture"]
    fn show_death_screens() {
        let (game, scores) = death_fixture();
        let board = Board {
            scores: &scores,
            this_run: Some(7),
            note: None,
        };
        for (w, h) in [(80, 24), (100, 40), (MIN_WIDTH, MIN_HEIGHT)] {
            println!("--- {w}x{h}");
            for line in screen_text(&draw_death(&game, &board, w, h)) {
                println!("|{line}|");
            }
        }
    }

    #[test]
    fn food_bar_shows_hunger() {
        let mut game = Game::new(1);
        game.player.food = FOOD_MAX / 2;
        let frame = draw(&game, 80, 24);
        let (view_w, _) = map_area(80, 24);
        let x = view_w + 2;
        let row = row_text(&frame, 3);
        assert!(row.contains("Food 1050"), "{row}");
        assert!(!row.contains("Hungry"));
        // Half full: the first half of the bar is filled.
        let width = SIDEBAR_WIDTH - 2;
        assert_eq!(frame.get((x + width / 2 - 1) as u16, 3).bg, FOOD_BAR.0);
        assert_eq!(frame.get((x + width / 2 + 1) as u16, 3).bg, FOOD_BAR.1);

        game.player.food = 100;
        let frame = draw(&game, 80, 24);
        assert!(row_text(&frame, 3).contains("Food 100 Weak"));
        assert_eq!(frame.get(x as u16, 3).fg, BAD);
    }

    #[test]
    fn monster_labels_fit_their_bar() {
        assert_eq!(monster_label("rat", "hunting", 16), "rat      hunting");
        let long = monster_label("giant spider", "hunting", 16);
        assert_eq!(long, "giant spider hun");
        assert_eq!(long.chars().count(), 16);
    }

    #[test]
    fn health_bar_fills_in_proportion() {
        let mut frame = Frame::new(10, 1);
        draw_bar(&mut frame, (0, 0, 10), "", (5, 10), TEXT, HEALTH_BAR);
        assert_eq!(frame.get(4, 0).bg, HEALTH_FULL);
        assert_eq!(frame.get(5, 0).bg, HEALTH_EMPTY);
        // One health left still shows a sliver.
        draw_bar(&mut frame, (0, 0, 10), "", (1, 100), TEXT, HEALTH_BAR);
        assert_eq!(frame.get(0, 0).bg, HEALTH_FULL);
    }

    #[test]
    fn small_map_is_centered() {
        assert_eq!(camera_origin(5, 10, 20), -5);
    }

    #[test]
    fn camera_follows_player_in_middle() {
        assert_eq!(camera_origin(50, 100, 20), 40);
    }

    #[test]
    fn camera_stops_at_edges() {
        assert_eq!(camera_origin(2, 100, 20), 0);
        assert_eq!(camera_origin(98, 100, 20), 80);
    }
}
