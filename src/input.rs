//! Reads keyboard events and turns them into commands.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::game::Action;
use crate::geom::Point;

/// Commands that need an item chosen from the pack first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Drop,
    Equip,
    Drink,
    Read,
    Eat,
}

pub enum Command {
    /// Something for the game to do.
    Act(Action),
    /// Choose an item, then do this with it.
    Use(Verb),
    Inventory,
    Character,
    Help,
    /// What's in view: monsters, items and traps.
    Look,
    /// Scroll back through the message log.
    History,
    /// Descend, or walk to the stairs if they're elsewhere.
    Descend,
    /// Auto-explore until something needs attention.
    Explore,
    /// Hand the game to the bot, or take it back.
    ToggleBot,
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
            if has_ctrl_or_alt(key) {
                return Ok(None);
            }
            return Ok(direction(key.code));
        }
    }
}

/// Waits for a key while a menu is open. Returns the character typed,
/// or `None` for Escape and other non-character keys, which close the
/// menu.
pub fn next_menu_key() -> io::Result<Option<char>> {
    loop {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                return Ok(match key.code {
                    // Ctrl+D is not "d": combinations with Ctrl or Alt
                    // close the menu instead of choosing something.
                    KeyCode::Char(c) if !has_ctrl_or_alt(key) => Some(c),
                    _ => None,
                });
            }
            Event::Resize(..) => return Ok(None),
            _ => {}
        }
    }
}

/// What `poll_key` saw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polled {
    /// No key within the time allowed.
    Nothing,
    /// A key: its character, or `None` for Escape and other keys.
    Key(Option<char>),
}

/// Waits up to `timeout` for a key, without blocking longer. Used while
/// the bot plays or auto-explore runs, so a key press can interrupt.
pub fn poll_key(timeout: std::time::Duration) -> io::Result<Polled> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if !event::poll(left)? {
            return Ok(Polled::Nothing);
        }
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            return Ok(Polled::Key(match key.code {
                KeyCode::Char(c) if !has_ctrl_or_alt(key) => Some(c),
                _ => None,
            }));
        }
    }
}

/// Waits for any key, after a short pause that swallows keys already
/// pressed. Without the pause, a player hammering a direction key in a
/// fight would skip the death screen before reading it.
pub fn wait_for_any_key() -> io::Result<()> {
    std::thread::sleep(std::time::Duration::from_millis(600));
    while event::poll(std::time::Duration::ZERO)? {
        event::read()?;
    }
    loop {
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            return Ok(());
        }
    }
}

fn has_ctrl_or_alt(key: KeyEvent) -> bool {
    key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

fn map_key(key: KeyEvent) -> Option<Command> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Command::Quit);
    }
    // Ctrl or Alt with a letter is never a game command, so Ctrl+D
    // can't drop something by accident.
    if has_ctrl_or_alt(key) {
        return None;
    }
    if let Some(dir) = direction(key.code) {
        return Some(Command::Act(Action::Move(dir)));
    }
    match key.code {
        // Capital Q, so a slip of the finger can't end a run.
        KeyCode::Char('Q') => Some(Command::Quit),
        KeyCode::Char('.') => Some(Command::Act(Action::Wait)),
        KeyCode::Char('>') => Some(Command::Descend),
        KeyCode::Char('x') => Some(Command::Explore),
        KeyCode::Char('B') => Some(Command::ToggleBot),
        KeyCode::Char('c') => Some(Command::Close),
        KeyCode::Char('g' | ',') => Some(Command::Act(Action::PickUp)),
        KeyCode::Char('i') => Some(Command::Inventory),
        KeyCode::Char('C') => Some(Command::Character),
        KeyCode::Char('d') => Some(Command::Use(Verb::Drop)),
        KeyCode::Char('e') => Some(Command::Use(Verb::Equip)),
        KeyCode::Char('q') => Some(Command::Use(Verb::Drink)),
        KeyCode::Char('r') => Some(Command::Use(Verb::Read)),
        KeyCode::Char('E') => Some(Command::Use(Verb::Eat)),
        KeyCode::Char('?') => Some(Command::Help),
        KeyCode::Char('L' | ';') => Some(Command::Look),
        KeyCode::Char('m') => Some(Command::History),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn ctrl_letters_are_not_commands() {
        let ctrl_d = key(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert!(map_key(ctrl_d).is_none());
        let d = key(KeyCode::Char('d'), KeyModifiers::NONE);
        assert!(matches!(map_key(d), Some(Command::Use(Verb::Drop))));
    }

    #[test]
    fn look_and_history_have_keys() {
        let plain = |c| map_key(key(KeyCode::Char(c), KeyModifiers::NONE));
        assert!(matches!(plain('L'), Some(Command::Look)));
        assert!(matches!(plain(';'), Some(Command::Look)));
        assert!(matches!(plain('m'), Some(Command::History)));
        assert!(matches!(plain('l'), Some(Command::Act(_))), "l still moves");
    }

    #[test]
    fn ctrl_c_and_capital_q_quit_but_lowercase_q_drinks() {
        let ctrl_c = key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches!(map_key(ctrl_c), Some(Command::Quit)));
        let big_q = key(KeyCode::Char('Q'), KeyModifiers::SHIFT);
        assert!(matches!(map_key(big_q), Some(Command::Quit)));
        let q = key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(matches!(map_key(q), Some(Command::Use(Verb::Drink))));
        let esc = key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(map_key(esc).is_none(), "Escape no longer quits");
    }
}
