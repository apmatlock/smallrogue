//! smallrogue: a grim, fast, endless dungeon crawl.
//!
//! Module overview:
//! - `geom`  — points and offsets
//! - `map`   — the tile grid
//! - `game`  — game state and rules (no terminal code)
//! - `frame` — a backend-independent screen picture
//! - `ui`    — lays out the game into a frame
//! - `input` — keys to commands
//! - `term`  — draws frames to the terminal

mod frame;
mod game;
mod geom;
mod input;
mod map;
mod term;
mod ui;

use std::io;

use game::Game;
use input::Command;

fn main() -> io::Result<()> {
    let mut terminal = term::Terminal::new()?;
    let mut game = Game::new();

    // The whole game loop: draw, wait for a key, apply it, repeat.
    loop {
        let (w, h) = terminal.size()?;
        terminal.present(ui::draw(&game, w, h))?;

        match input::next_command()? {
            Command::Act(action) => game.apply(action),
            Command::Redraw => {}
            Command::Quit => break,
        }
    }
    Ok(())
}
