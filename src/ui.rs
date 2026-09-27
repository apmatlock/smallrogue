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
use crate::map::Tile;
use crate::monster::{Ai, Monster};

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

    let (w, h) = (width as i32, height as i32);
    let view_w = w - SIDEBAR_WIDTH - 1; // -1 for the divider column
    let view_h = h - LOG_HEIGHT - 1; // -1 for the divider row

    draw_map(&mut frame, game, view_w, view_h);
    draw_dividers(&mut frame, view_w, view_h, w);
    draw_sidebar(&mut frame, game, view_w + 2, view_h);
    draw_log(&mut frame, game, view_h + 1);
    frame
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
    draw_bar(
        frame,
        (x, 2, width),
        &format!("Health {}/{}", p.hp, p.max_hp),
        (p.hp, p.max_hp),
        TEXT,
    );
    frame.print(
        x,
        3,
        &format!("Str {}  Agi {}  Int {}", p.strength, p.agility, p.intellect),
        TEXT,
    );
    frame.print(
        x,
        4,
        &format!("Depth {}  Turn {}", game.depth, game.turn),
        TEXT,
    );
    frame.print(x, 5, &format!("Seed {}", game.seed), TEXT_DIM);

    // Key help sits at the bottom of the sidebar.
    let keys = [
        "hjkl/arrows  move",
        "yubn    diagonals",
        "walk into   attack",
        ".        wait",
        ">        descend",
        "c        close door",
        "q        quit",
    ];
    let keys_top = height - keys.len() as i32;
    for (i, line) in keys.iter().enumerate() {
        frame.print(x, keys_top + i as i32, line, TEXT_DIM);
    }

    // Monsters in view, nearest first, in the space between. Each gets
    // a health bar behind its name.
    let mut in_view: Vec<&Monster> = game
        .monsters
        .iter()
        .filter(|m| game.is_visible(m.pos))
        .collect();
    in_view.sort_by_key(|m| m.pos.dist_sq(game.player.pos));
    let list_top = 7;
    let room = (keys_top - 1 - list_top).max(0) as usize;
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
        };
        draw_bar(
            frame,
            (x + 2, y, width - 2),
            &format!("{:<8}{state}", species.name),
            (m.hp, species.max_hp),
            TEXT,
        );
    }
}

/// Draws `label` on top of a bar whose filled part shows
/// `current / max`. `area` is (x, y, width).
fn draw_bar(frame: &mut Frame, area: (i32, i32, i32), label: &str, amount: (i32, i32), fg: Rgb) {
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
        let bg = if i < filled {
            HEALTH_FULL
        } else {
            HEALTH_EMPTY
        };
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

    let cause = game.death.as_deref().unwrap_or("You died.");
    let lines: Vec<(String, Rgb)> = vec![
        ("You have died.".to_string(), TITLE),
        (String::new(), TEXT),
        (cause.to_string(), TEXT),
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
    fn death_screen_shows_the_cause() {
        let mut game = Game::new(1);
        game.death = Some("Killed by a rat on depth 1 after 5 turns.".to_string());
        let frame = draw_death(&game, 80, 24);
        let text: String = (0..24).map(|y| row_text(&frame, y)).collect();
        assert!(text.contains("You have died."));
        assert!(text.contains("Killed by a rat"));
    }

    #[test]
    fn health_bar_fills_in_proportion() {
        let mut frame = Frame::new(10, 1);
        draw_bar(&mut frame, (0, 0, 10), "", (5, 10), TEXT);
        assert_eq!(frame.get(4, 0).bg, HEALTH_FULL);
        assert_eq!(frame.get(5, 0).bg, HEALTH_EMPTY);
        // One health left still shows a sliver.
        draw_bar(&mut frame, (0, 0, 10), "", (1, 100), TEXT);
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
