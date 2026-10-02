//! The terminal backend: the only module that writes to the screen.
//!
//! It keeps the previously drawn frame and, on each update, only sends
//! the cells that changed. On a slow machine or over SSH this makes
//! redraws nearly free.

use std::io::{self, BufWriter, Stdout, Write};

use crossterm::{
    cursor, execute, queue,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType},
};

use crate::frame::{Frame, Rgb};

pub struct Terminal {
    out: BufWriter<Stdout>,
    previous: Option<Frame>,
}

impl Terminal {
    /// Switches the terminal into game mode: raw input (no line
    /// buffering or echo), a separate screen, and a hidden cursor.
    pub fn new() -> io::Result<Self> {
        install_panic_hook();
        terminal::enable_raw_mode()?;
        let mut out = BufWriter::new(io::stdout());
        if let Err(e) = execute!(out, terminal::EnterAlternateScreen, cursor::Hide) {
            // There's no `Terminal` yet whose drop would undo raw mode,
            // so undo it here; otherwise the shell is left unusable.
            restore_terminal();
            return Err(e);
        }
        Ok(Self {
            out,
            previous: None,
        })
    }

    pub fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    /// Draws `frame`, sending only what changed since the last call.
    pub fn present(&mut self, frame: Frame) -> io::Result<()> {
        // If the size changed (or this is the first draw), redraw all.
        let previous = self
            .previous
            .take()
            .filter(|p| p.width == frame.width && p.height == frame.height);
        if previous.is_none() {
            queue!(self.out, terminal::Clear(ClearType::All))?;
        }

        // Track what the terminal's cursor and colors currently are,
        // so we only send escape codes when something differs.
        let mut cursor_at: Option<(u16, u16)> = None;
        let mut fg: Option<Rgb> = None;
        let mut bg: Option<Rgb> = None;

        for y in 0..frame.height {
            for x in 0..frame.width {
                let cell = frame.get(x, y);
                if previous.as_ref().is_some_and(|p| p.get(x, y) == cell) {
                    continue;
                }
                if cursor_at != Some((x, y)) {
                    queue!(self.out, cursor::MoveTo(x, y))?;
                }
                if fg != Some(cell.fg) {
                    queue!(self.out, SetForegroundColor(to_color(cell.fg)))?;
                    fg = Some(cell.fg);
                }
                if bg != Some(cell.bg) {
                    queue!(self.out, SetBackgroundColor(to_color(cell.bg)))?;
                    bg = Some(cell.bg);
                }
                queue!(self.out, Print(cell.ch))?;
                cursor_at = Some((x + 1, y));
            }
        }

        self.out.flush()?;
        self.previous = Some(frame);
        Ok(())
    }
}

/// `Drop` runs when the `Terminal` goes out of scope, including on an
/// early `?` return, so the user's shell is always restored.
impl Drop for Terminal {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    let mut out = io::stdout();
    let _ = execute!(
        out,
        crossterm::style::ResetColor,
        cursor::Show,
        terminal::LeaveAlternateScreen
    );
    let _ = terminal::disable_raw_mode();
}

/// Release builds use `panic = "abort"`, which skips `Drop`. This hook
/// restores the terminal before the panic message prints, so a crash
/// never leaves the shell in raw mode.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));
}

fn to_color(c: Rgb) -> Color {
    Color::Rgb {
        r: c.0,
        g: c.1,
        b: c.2,
    }
}
