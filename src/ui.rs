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
use crate::game::Game;
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
        camera_origin(game.player.x, game.map.width(), view_w),
        camera_origin(game.player.y, game.map.height(), view_h),
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

    let s = game.player - origin;
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
    frame.print(x, 0, "SMALLROGUE", TITLE);
    frame.print(x, 2, &format!("Depth: {}", game.depth), TEXT);
    frame.print(x, 3, &format!("Turn:  {}", game.turn), TEXT);
    frame.print(x, 4, &format!("Seed:  {}", game.seed), TEXT_DIM);

    // Key help sits at the bottom of the sidebar.
    let keys = [
        "hjkl/arrows  move",
        "yubn    diagonals",
        ".        wait",
        ">        descend",
        "c        close door",
        "q        quit",
    ];
    let keys_top = height - keys.len() as i32;
    for (i, line) in keys.iter().enumerate() {
        frame.print(x, keys_top + i as i32, line, TEXT_DIM);
    }

    // Monsters in view, nearest first, in the space between.
    let mut in_view: Vec<&Monster> = game
        .monsters
        .iter()
        .filter(|m| game.is_visible(m.pos))
        .collect();
    in_view.sort_by_key(|m| m.pos.dist_sq(game.player));
    let list_top = 6;
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
        frame.print(x + 2, y, species.name, TEXT);
        frame.print(x + 10, y, state, TEXT_DIM);
    }
}

fn draw_log(frame: &mut Frame, game: &Game, top: i32) {
    // Show the newest messages, newest at the bottom, older ones dimmer.
    let shown = game.log.iter().rev().take(LOG_HEIGHT as usize);
    for (i, msg) in shown.enumerate() {
        let color = if i == 0 { TEXT } else { TEXT_DIM };
        frame.print(1, top + LOG_HEIGHT - 1 - i as i32, msg, color);
    }
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
