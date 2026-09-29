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
use crate::monster::{Ai, Monster};
use crate::player::{FOOD_MAX, Hunger};
use crate::skills::{self, Skill};

const SIDEBAR_WIDTH: i32 = 22;
const LOG_HEIGHT: i32 = 4;
pub const MIN_WIDTH: u16 = 50;
pub const MIN_HEIGHT: u16 = 16;

// A muted palette to suit the grim tone.
const WALL_FG: Rgb = Rgb(120, 110, 100);
const WALL_BG: Rgb = Rgb(28, 26, 24);
const FLOOR_FG: Rgb = Rgb(105, 98, 88);
const DOOR_FG: Rgb = Rgb(160, 110, 60);
const STAIRS_FG: Rgb = Rgb(230, 200, 90);
const PLAYER_FG: Rgb = Rgb(240, 230, 200);
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
    [
        "arrows or hjkl   move, or attack by moving into",
        "yubn             move diagonally",
        ".                wait a turn",
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
        "?                this help",
        "B                let the bot play (B again stops it)",
        "Q                quit (ends the run)",
    ]
    .into_iter()
    .map(|t| Line::new(t, TEXT))
    .collect()
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
                frame.set(sx, sy, tile_cell(game.map.tile(p)));
            } else if game.map.is_revealed(p) {
                let cell = tile_cell(game.map.tile(p));
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

/// How each kind of tile looks.
fn tile_cell(tile: Tile) -> Cell {
    let (ch, fg, bg) = match tile {
        Tile::Wall => ('#', WALL_FG, WALL_BG),
        Tile::Floor => ('.', FLOOR_FG, BLACK),
        Tile::DoorClosed => ('+', DOOR_FG, BLACK),
        Tile::DoorOpen => ('\'', DOOR_FG, BLACK),
        Tile::StairsDown => ('>', STAIRS_FG, BLACK),
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
        let color = match msg.kind {
            MsgKind::Info => TEXT,
            MsgKind::Good => GOOD,
            MsgKind::Bad => BAD,
        };
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

/// The screen shown after death: how it happened and the last few
/// messages, so the player can see what went wrong.
pub fn draw_death(game: &Game, width: u16, height: u16) -> Frame {
    let mut frame = Frame::new(width, height);
    let (w, h) = (width as i32, height as i32);
    let center = |text: &str| (w - text.chars().count() as i32).max(0) / 2;

    let cause = game
        .death_summary()
        .unwrap_or_else(|| "You died.".to_string());
    let lines: Vec<(String, Rgb)> = vec![
        ("You have died.".to_string(), TITLE),
        (String::new(), TEXT),
        (cause, TEXT),
        (format!("Seed {}", game.seed), TEXT_DIM),
        (String::new(), TEXT),
    ];
    let recent: Vec<(String, Rgb)> = game
        .log
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|m| (m.text.clone(), TEXT_DIM))
        .collect();
    let footer = ("Press any key to leave the dungeon.".to_string(), TEXT);

    let total = lines.len() + recent.len() + 2;
    let mut y = ((h - total as i32) / 2).max(0);
    for (text, color) in lines.iter().chain(&recent) {
        frame.print(center(text), y, text, *color);
        y += 1;
    }
    y += 1;
    frame.print(center(&footer.0), y, &footer.0, footer.1);
    frame
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
        assert!(row_text(&frame, 22).contains("You descend"));
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
    fn death_screen_shows_the_cause() {
        let mut game = Game::new(1);
        game.death = Some("a rat".to_string());
        let frame = draw_death(&game, 80, 24);
        let text: String = (0..24).map(|y| row_text(&frame, y)).collect();
        assert!(text.contains("You have died."));
        assert!(text.contains("Killed by a rat"));
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
