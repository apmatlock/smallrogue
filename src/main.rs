//! smallrogue: a grim, fast, endless dungeon crawl.
//!
//! Module overview:
//! - `geom`    — points, directions and rectangles
//! - `rng`     — seedable random numbers
//! - `grid`    — a generic one-value-per-tile container
//! - `map`     — the tile layout and the player's memory of it
//! - `fov`     — field of view (what can be seen from where)
//! - `dungeon` — random floor generation
//! - `player`  — the player character's stats
//! - `combat`  — rolling attacks and damage
//! - `item`    — item kinds, their data, and spawning
//! - `inventory` — picking up, equipping and using items
//! - `text`    — small English helpers
//! - `monster` — monster kinds, their data, and spawning
//! - `path`    — pathfinding around walls
//! - `ai`      — what monsters do on their turn
//! - `game`    — game state and rules (no terminal code)
//! - `frame`   — a backend-independent screen picture
//! - `ui`      — lays out the game into a frame
//! - `input`   — keys to commands
//! - `term`    — draws frames to the terminal
//!
//! Run with `cargo run -- --seed 1234` to replay a specific dungeon.

mod ai;
mod combat;
mod dungeon;
mod fov;
mod frame;
mod game;
mod geom;
mod grid;
mod input;
mod inventory;
mod item;
mod map;
mod menu;
mod monster;
mod path;
mod player;
mod rng;
mod term;
mod text;
mod ui;

use std::io;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use game::{Action, Game};
use input::{Command, Verb};
use item::{Item, ItemKind};
use menu::Line;
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

        if game.death.is_some() {
            let (w, h) = terminal.size()?;
            terminal.present(ui::draw_death(&game, w, h))?;
            input::wait_for_any_key()?;
            break;
        }

        match input::next_command()? {
            Command::Act(action) => game.apply(action),
            Command::Close => close_door(&mut terminal, &mut game)?,
            Command::Use(verb) => use_item(&mut terminal, &mut game, verb, None)?,
            Command::Inventory => show_inventory(&mut terminal, &mut game)?,
            Command::Help => {
                show_box(&mut terminal, &game, "Keys", &ui::help_lines())?;
                input::next_menu_key()?;
            }
            Command::Redraw => {}
            Command::Quit => {
                if confirm_quit(&mut terminal, &mut game)? {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn draw(terminal: &mut Terminal, game: &Game) -> io::Result<()> {
    let (w, h) = terminal.size()?;
    terminal.present(ui::draw(game, w, h))
}

fn show_box(terminal: &mut Terminal, game: &Game, title: &str, lines: &[Line]) -> io::Result<()> {
    let (w, h) = terminal.size()?;
    terminal.present(ui::draw_with_box(game, w, h, title, lines))
}

/// Shows a list of items and waits for a letter. Returns the letter if
/// it names one of the listed items, or `None` if the player cancels.
fn choose_item(
    terminal: &mut Terminal,
    game: &Game,
    title: &str,
    show: impl Fn(&Item) -> bool,
) -> io::Result<Option<char>> {
    show_box(terminal, game, title, &ui::item_list(game, &show))?;
    Ok(input::next_menu_key()?.filter(|&c| game.player.item(c).is_some_and(&show)))
}

/// A test for which items a menu should list.
type ItemFilter<'a> = &'a dyn Fn(&Item) -> bool;

/// Carries out a verb like "drink": asks which item (unless `letter`
/// already says), and for a scroll of enchanting, which item to
/// enchant.
fn use_item(
    terminal: &mut Terminal,
    game: &mut Game,
    verb: Verb,
    letter: Option<char>,
) -> io::Result<()> {
    let is_potion = |i: &Item| matches!(i.kind, ItemKind::Potion(_));
    let is_scroll = |i: &Item| matches!(i.kind, ItemKind::Scroll(_));
    let is_gear = |i: &Item| i.kind.is_equipment();
    let (title, show, none): (&str, ItemFilter, &str) = match verb {
        Verb::Drop => ("Drop what?", &|_| true, "You have nothing to drop."),
        Verb::Equip => (
            "Equip or remove what?",
            &is_gear,
            "You have nothing to equip.",
        ),
        Verb::Drink => ("Drink what?", &is_potion, "You have no potions."),
        Verb::Read => ("Read what?", &is_scroll, "You have no scrolls."),
    };
    if !game.player.inventory.iter().any(show) {
        game.log(none);
        return Ok(());
    }
    let letter = match letter {
        Some(letter) => letter,
        None => match choose_item(terminal, game, title, show)? {
            Some(letter) => letter,
            None => return Ok(()),
        },
    };
    let action = match verb {
        Verb::Drop => Action::Drop(letter),
        Verb::Equip => Action::Equip(letter),
        Verb::Drink => Action::Drink(letter),
        Verb::Read => {
            let mut target = None;
            if game.scroll_needs_target(letter) {
                target = choose_item(terminal, game, "Enchant what?", is_gear)?;
                if target.is_none() {
                    game.log("Never mind.");
                    return Ok(());
                }
            }
            Action::Read {
                scroll: letter,
                target,
            }
        }
    };
    game.apply(action);
    Ok(())
}

/// The pack screen: pick an item to see its details, then optionally
/// act on it with that item's keys.
fn show_inventory(terminal: &mut Terminal, game: &mut Game) -> io::Result<()> {
    if game.player.inventory.is_empty() {
        game.log("Your pack is empty.");
        return Ok(());
    }
    let title = format!(
        "Pack ({}/{})",
        game.player.inventory.len(),
        player::PACK_SIZE
    );
    let Some(letter) = choose_item(terminal, game, &title, |_| true)? else {
        return Ok(());
    };
    let item = game
        .player
        .item(letter)
        .expect("chosen from the pack")
        .clone();
    let title = format!("{}) {}", item.letter, item.name());
    show_box(terminal, game, &title, &ui::item_details(&item))?;

    let verb = match (input::next_menu_key()?, item.kind) {
        (Some('d'), _) => Verb::Drop,
        (Some('e'), ItemKind::Weapon(_) | ItemKind::Armor(_)) => Verb::Equip,
        (Some('q'), ItemKind::Potion(_)) => Verb::Drink,
        (Some('r'), ItemKind::Scroll(_)) => Verb::Read,
        _ => return Ok(()),
    };
    use_item(terminal, game, verb, Some(letter))
}

/// Asks before quitting, since quitting ends the run for good.
fn confirm_quit(terminal: &mut Terminal, game: &mut Game) -> io::Result<bool> {
    let lines = [Line::new(
        "This ends the run. Press y to quit.",
        frame::Rgb(190, 190, 190),
    )];
    show_box(terminal, game, "Quit?", &lines)?;
    Ok(input::next_menu_key()? == Some('y'))
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
