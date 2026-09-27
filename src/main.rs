//! smallrogue: a grim, fast, endless dungeon crawl.
//!
//! Module overview:
//! - `geom`    — points, directions and rectangles
//! - `rng`     — seedable random numbers
//! - `grid`    — a generic one-value-per-tile container
//! - `map`     — the tile layout and the player's memory of it
//! - `fov`     — field of view (what can be seen from where)
//! - `dungeon` — random floor generation
//! - `game`    — game state and rules (no terminal code)
//! - `frame`   — a backend-independent screen picture
//! - `ui`      — lays out the game into a frame
//! - `input`   — keys to commands
//! - `term`    — draws frames to the terminal
//!
//! Run with `cargo run -- --seed 1234` to replay a specific dungeon.

mod dungeon;
mod fov;
mod frame;
mod game;
mod geom;
mod grid;
mod input;
mod map;
mod rng;
mod term;
mod ui;

use std::io;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use game::{Action, Game};
use input::Command;
use term::Terminal;

fn main() -> ExitCode {
    // Handle arguments before touching the terminal, so errors print
    // normally.
    let seed = match seed_from_args() {
        Ok(seed) => seed,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };

    match run(seed) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(seed: u64) -> io::Result<()> {
    let mut terminal = Terminal::new()?;
    let mut game = Game::new(seed);

    // The whole game loop: draw, wait for a key, apply it, repeat.
    loop {
        draw(&mut terminal, &game)?;

        match input::next_command()? {
            Command::Act(action) => game.apply(action),
            Command::Close => close_door(&mut terminal, &mut game)?,
            Command::Redraw => {}
            Command::Quit => break,
        }
    }
    Ok(())
}

fn draw(terminal: &mut Terminal, game: &Game) -> io::Result<()> {
    let (w, h) = terminal.size()?;
    terminal.present(ui::draw(game, w, h))
}

/// Closes an adjacent door, asking for a direction only when there is
/// more than one to choose from.
fn close_door(terminal: &mut Terminal, game: &mut Game) -> io::Result<()> {
    match game.adjacent_open_doors().as_slice() {
        [] => game.log("There is no open door next to you."),
        [dir] => game.apply(Action::Close(*dir)),
        _ => {
            game.log("Close which door? (direction)");
            draw(terminal, game)?;
            match input::next_direction()? {
                Some(dir) => game.apply(Action::Close(dir)),
                None => game.log("Never mind."),
            }
        }
    }
    Ok(())
}

/// Reads `--seed N` from the command line, or picks a seed from the
/// clock. Seeds from the clock stay under a billion so they are short
/// enough to read off the screen and type back in.
fn seed_from_args() -> Result<u64, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            Ok((nanos % 1_000_000_000) as u64)
        }
        [flag, value] if flag == "--seed" => value
            .parse()
            .map_err(|_| format!("invalid seed '{value}': expected a whole number")),
        _ => Err("usage: smallrogue [--seed N]".to_string()),
    }
}
