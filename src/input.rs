//! Reads keyboard events and turns them into commands.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::game::Action;
use crate::geom::Point;

pub enum Command {
    /// Something for the game to do.
    Act(Action),
    Quit,
    /// Nothing to do, but redraw (e.g. the window was resized).
    Redraw,
}

/// Blocks until a meaningful event arrives. Because this waits instead
/// of polling, the game uses no CPU while you think.
pub fn next_command() -> io::Result<Command> {
    loop {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if let Some(cmd) = map_key(key) {
                    return Ok(cmd);
                }
            }
            Event::Resize(..) => return Ok(Command::Redraw),
            _ => {}
        }
    }
}

fn map_key(key: KeyEvent) -> Option<Command> {
    let step = |x, y| Some(Command::Act(Action::Move(Point::new(x, y))));
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Command::Quit),
        KeyCode::Char('q') | KeyCode::Esc => Some(Command::Quit),
        KeyCode::Left | KeyCode::Char('h') => step(-1, 0),
        KeyCode::Right | KeyCode::Char('l') => step(1, 0),
        KeyCode::Up | KeyCode::Char('k') => step(0, -1),
        KeyCode::Down | KeyCode::Char('j') => step(0, 1),
        KeyCode::Char('y') => step(-1, -1),
        KeyCode::Char('u') => step(1, -1),
        KeyCode::Char('b') => step(-1, 1),
        KeyCode::Char('n') => step(1, 1),
        KeyCode::Char('.') => Some(Command::Act(Action::Wait)),
        _ => None,
    }
}
