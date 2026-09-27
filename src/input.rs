//! Reads keyboard events and turns them into commands.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::game::Action;
use crate::geom::Point;

pub enum Command {
    /// Something for the game to do.
    Act(Action),
    /// Close a door. Needs a direction if several doors are adjacent,
    /// which the main loop sorts out.
    Close,
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

/// Waits for a direction key. Any other key cancels and returns `None`.
pub fn next_direction() -> io::Result<Option<Point>> {
    loop {
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            return Ok(direction(key.code));
        }
    }
}

fn map_key(key: KeyEvent) -> Option<Command> {
    if let Some(dir) = direction(key.code) {
        return Some(Command::Act(Action::Move(dir)));
    }
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Command::Quit),
        KeyCode::Char('q') | KeyCode::Esc => Some(Command::Quit),
        KeyCode::Char('.') => Some(Command::Act(Action::Wait)),
        KeyCode::Char('>') => Some(Command::Act(Action::Descend)),
        KeyCode::Char('c') => Some(Command::Close),
        _ => None,
    }
}

/// The movement keys: arrows, vi keys (hjkl) and vi diagonals (yubn).
fn direction(code: KeyCode) -> Option<Point> {
    let (x, y) = match code {
        KeyCode::Left | KeyCode::Char('h') => (-1, 0),
        KeyCode::Right | KeyCode::Char('l') => (1, 0),
        KeyCode::Up | KeyCode::Char('k') => (0, -1),
        KeyCode::Down | KeyCode::Char('j') => (0, 1),
        KeyCode::Char('y') => (-1, -1),
        KeyCode::Char('u') => (1, -1),
        KeyCode::Char('b') => (-1, 1),
        KeyCode::Char('n') => (1, 1),
        _ => return None,
    };
    Some(Point::new(x, y))
}
