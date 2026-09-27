use std::io::{self, Write};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute, queue,
    style::Print,
    terminal::{self, ClearType},
};

fn main() -> io::Result<()> {
    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;

    let result = run(&mut out);

    execute!(out, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    result
}

fn run(out: &mut impl Write) -> io::Result<()> {
    let (mut x, mut y): (u16, u16) = (10, 5);
    loop {
        queue!(
            out,
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0),
            Print("smallrogue - arrows/hjkl to move, q to quit"),
            cursor::MoveTo(x, y),
            Print('@'),
        )?;
        out.flush()?;

        // Turn-based: block until a key arrives, so the game idles at 0% CPU.
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Left | KeyCode::Char('h') => x = x.saturating_sub(1),
                KeyCode::Right | KeyCode::Char('l') => x += 1,
                KeyCode::Up | KeyCode::Char('k') => y = y.saturating_sub(1).max(1),
                KeyCode::Down | KeyCode::Char('j') => y += 1,
                _ => {}
            }
        }
    }
}
