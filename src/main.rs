//! smallrogue: a grim, fast, endless dungeon crawl.
//!
//! Module overview:
//! - `geom`    — points, directions and rectangles
//! - `rng`     — seedable random numbers
//! - `grid`    — a generic one-value-per-tile container
//! - `map`     — the tile layout and the player's memory of it
//! - `fov`     — field of view (what can be seen from where)
//! - `dungeon` — random floor generation
//! - `zone`    — themed stretches of floors, and the loop through them
//! - `player`  — the player character's stats
//! - `combat`  — rolling attacks and damage
//! - `skills`  — skills that improve by use; levels from experience
//! - `stats`   — counts kept over a run for the death screen
//! - `scores`  — the high score list, saved between runs
//! - `record`  — recordings of runs, for replays and analysis
//! - `analyze` — reports on recorded runs
//! - `item`    — item kinds, their data, and spawning
//! - `inventory` — picking up, equipping and using items
//! - `lore`    — what the player knows about items; item names
//! - `text`    — small English helpers
//! - `themed`  — themed rooms: tombs, cisterns, dens and larders
//! - `trap`    — hidden traps: placing, noticing and springing them
//! - `monster` — monster kinds, their data, and spawning
//! - `path`    — pathfinding around walls
//! - `ai`      — what monsters do on their turn
//! - `abilities` — special monster powers: stealing, draining, splitting
//! - `bot`     — a bot player, auto-explore and travel
//! - `sim`     — headless bot runs for balance testing
//! - `cli`     — command-line options
//! - `game`    — game state and rules (no terminal code)
//! - `frame`   — a backend-independent screen picture
//! - `ui`      — lays out the game into a frame
//! - `input`   — keys to commands
//! - `term`    — draws frames to the terminal
//!
//! Run with `cargo run -- --seed 1234` to replay a specific dungeon,
//! `--bot` to watch the bot play, or `--simulate 200` for balance runs.

mod abilities;
mod ai;
mod analyze;
mod bot;
mod cli;
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
mod lore;
mod map;
mod menu;
mod monster;
mod path;
mod player;
mod record;
mod rng;
mod scores;
mod sim;
mod skills;
mod stats;
mod term;
mod text;
mod themed;
mod trap;
mod ui;
mod zone;

use std::io;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use game::{Action, Game};
use input::{Command, Polled, Verb};
use item::{Item, ItemKind};
use menu::Line;
use record::{Recorder, Source, Step};
use term::Terminal;

fn main() -> ExitCode {
    // Handle arguments before touching the terminal, so errors print
    // normally.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match cli::parse(&args) {
        Ok(options) => options,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };

    if let Some(path) = &options.analyze {
        return match analyze::analyze(path.as_deref()) {
            Ok(report) => {
                print!("{report}");
                ExitCode::SUCCESS
            }
            Err(msg) => {
                eprintln!("{msg}");
                ExitCode::FAILURE
            }
        };
    }
    if let Some(path) = &options.replay {
        // Read the file before touching the terminal, so a bad file
        // gets a plain error message.
        let recording = match record::load(path) {
            Ok(r) => r,
            Err(msg) => {
                eprintln!("{msg}");
                return ExitCode::FAILURE;
            }
        };
        return match Terminal::new().and_then(|mut t| watch_replay(&mut t, &recording)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        };
    }

    let result = match options.simulate {
        Some(runs) => {
            if let Some(dir) = options.csv.parent().filter(|d| !d.as_os_str().is_empty()) {
                let _ = std::fs::create_dir_all(dir);
            }
            sim::simulate(runs, options.seed.unwrap_or(1), &options.csv)
        }
        None => run(options.seed, options.bot, options.record),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Adds a finished run to the saved high scores, unless the bot
/// played it. Returns the list, this run's place on it, and a note
/// for the death screen. Trouble with the file is reported there
/// rather than ending the game with an error.
fn record_score(
    game: &Game,
    bot_played: bool,
) -> (Vec<scores::Score>, Option<usize>, Option<String>) {
    let Some(path) = scores::default_path() else {
        return (
            Vec::new(),
            None,
            Some("No home directory to keep scores in.".to_string()),
        );
    };
    let mut list = match scores::load(&path) {
        Ok(list) => list,
        Err(e) => {
            return (
                Vec::new(),
                None,
                Some(format!("Couldn't read the scores: {e}")),
            );
        }
    };
    if bot_played {
        let note = "The bot played this run, so it isn't recorded.";
        return (list, None, Some(note.to_string()));
    }
    let score = scores::Score::from_game(game, scores::today());
    let place = scores::insert(&mut list, score);
    let mut note = match place {
        Some(0) => Some("A new best run!".to_string()),
        Some(_) => None,
        None => Some(format!("This run didn't make the top {}.", scores::KEEP)),
    };
    if place.is_some()
        && let Err(e) = scores::save(&path, &list)
    {
        note = Some(format!("Couldn't save the scores: {e}"));
    }
    (list, place, note)
}

/// Real time spent playing a run. A long gap between two moves counts
/// as `IDLE_CAP` at most, so walking away doesn't pad the time.
struct PlayClock {
    played: Duration,
    last: Instant,
}

const IDLE_CAP: Duration = Duration::from_secs(60);

impl PlayClock {
    fn new() -> Self {
        Self {
            played: Duration::ZERO,
            last: Instant::now(),
        }
    }

    /// Adds the time since the last tick and returns the total.
    fn tick(&mut self) -> Duration {
        let now = Instant::now();
        self.played += (now - self.last).min(IDLE_CAP);
        self.last = now;
        self.played
    }
}

/// Plays a recording back on screen, at a chosen speed. Ends on the
/// death screen, or with a note if the recording stops mid-run.
fn watch_replay(terminal: &mut Terminal, recording: &record::Recording) -> io::Result<()> {
    let mut game = Game::new(recording.seed);
    let (mut speed, mut paused) = (2, false);
    let steps = &recording.entries;
    let mut next = 0;
    while next < steps.len() && game.death.is_none() {
        let status = if paused {
            format!("REPLAY paused {next}/{}", steps.len())
        } else {
            format!(
                "REPLAY {}/{} {next}/{}",
                speed + 1,
                BOT_SPEEDS_MS.len(),
                steps.len()
            )
        };
        draw_with_status(terminal, &game, Some(&status))?;
        let wait = if paused {
            Duration::from_secs(3600)
        } else {
            Duration::from_millis(BOT_SPEEDS_MS[speed])
        };
        match input::poll_key(wait)? {
            Polled::Nothing if !paused => {
                match steps[next].step {
                    Step::Act(action) => game.apply(action),
                    Step::Quit => game.give_up(),
                }
                next += 1;
            }
            Polled::Nothing => {}
            Polled::Key(Some('+' | '=')) => speed = (speed + 1).min(BOT_SPEEDS_MS.len() - 1),
            Polled::Key(Some('-')) => speed = speed.saturating_sub(1),
            Polled::Key(Some(' ')) => paused = !paused,
            Polled::Key(_) => return Ok(()),
        }
    }
    if game.death.is_none() {
        game.log("The recording ends here: the run wasn't finished.");
        draw_with_status(terminal, &game, Some("REPLAY ended"))?;
        input::wait_for_any_key()?;
        return Ok(());
    }
    // A replay only matches while the game's rules are the same as when
    // it was recorded.
    let note = match &recording.end {
        Some(end) if *end != record::End::of(&game) => {
            "This replay went differently: the game has changed since."
        }
        _ => "A replay: nothing is added to the high scores.",
    };
    let scores = scores::default_path()
        .and_then(|p| scores::load(&p).ok())
        .unwrap_or_default();
    let board = ui::Board {
        scores: &scores,
        this_run: None,
        note: Some(note),
    };
    let (w, h) = terminal.size()?;
    terminal.present(ui::draw_death(&game, &board, w, h))?;
    input::wait_for_any_key()
}

/// Starts recording a run, turning on the game's journal. If the file
/// can't be made, the run goes on unrecorded and the log says why.
fn start_recording(game: &mut Game) -> Option<Recorder> {
    let started = match record::default_dir() {
        Some(dir) => Recorder::start(&dir, game.seed),
        None => Err(io::Error::other("no home directory")),
    };
    match started {
        Ok(recorder) => {
            game.journal = Some(Vec::new());
            Some(recorder)
        }
        Err(e) => {
            game.log(&format!("Not recording this run: {e}"));
            None
        }
    }
}

/// Moves the actions the game has applied since last time into the
/// recording. If writing fails, recording stops and the log says so.
fn save_steps(recorder: &mut Option<Recorder>, game: &mut Game, played: Duration, source: Source) {
    let Some(journal) = game.journal.as_mut() else {
        return;
    };
    let steps: Vec<Step> = journal.drain(..).map(Step::Act).collect();
    let Some(rec) = recorder.as_mut() else {
        return;
    };
    if steps.is_empty() {
        return;
    }
    if let Err(e) = rec.write(played.as_millis() as u64, source, &steps) {
        *recorder = None;
        game.journal = None;
        game.log(&format!("Recording stopped: {e}"));
    }
}

/// Delay between bot moves at each speed setting, slowest first.
const BOT_SPEEDS_MS: [u64; 7] = [500, 250, 120, 60, 25, 8, 0];

/// The bot's settings while it is playing.
struct Autoplay {
    speed: usize,
    paused: bool,
    /// What the bot remembers between turns, like a plan to escape.
    memory: bot::BotMemory,
    /// Bot actions in a row that used no time. A safety net: if the bot
    /// ever keeps choosing something the game refuses, it stops rather
    /// than spinning forever.
    idle: u32,
}

impl Autoplay {
    fn new() -> Self {
        Self {
            speed: 2,
            paused: false,
            memory: bot::BotMemory::default(),
            idle: 0,
        }
    }

    fn status(&self) -> String {
        if self.paused {
            "BOT paused".to_string()
        } else {
            format!("BOT speed {}/{}", self.speed + 1, BOT_SPEEDS_MS.len())
        }
    }
}

/// Plays in the terminal. With a seed or the bot asked for, plays that
/// one run; otherwise the title screen offers run after run.
fn run(seed: Option<u64>, start_with_bot: bool, record: bool) -> io::Result<()> {
    let mut terminal = Terminal::new()?;
    if seed.is_some() || start_with_bot {
        return play(
            &mut terminal,
            seed.unwrap_or_else(clock_seed),
            start_with_bot,
            record,
        );
    }
    loop {
        let best = scores::default_path()
            .and_then(|p| scores::load(&p).ok())
            .and_then(|list| list.into_iter().next());
        let (w, h) = terminal.size()?;
        terminal.present(ui::draw_title(best.as_ref(), w, h))?;
        match input::next_menu_key()? {
            Some('n') => play(&mut terminal, clock_seed(), false, record)?,
            Some('s') => show_scores(&mut terminal)?,
            Some('?') => {
                show_box_over(&mut terminal, &ui::draw_blank, "Help", &ui::help_lines())?;
            }
            Some('q' | 'Q') => return Ok(()),
            // Anything else, including a resize, just redraws.
            _ => {}
        }
    }
}

/// The saved high scores on a screen of their own.
fn show_scores(terminal: &mut Terminal) -> io::Result<()> {
    let (list, note) = match scores::default_path().map(|p| scores::load(&p)) {
        Some(Ok(list)) => (list, None),
        Some(Err(e)) => (Vec::new(), Some(format!("Couldn't read the scores: {e}"))),
        None => (
            Vec::new(),
            Some("No home directory to keep scores in.".to_string()),
        ),
    };
    let board = ui::Board {
        scores: &list,
        this_run: None,
        note: note.as_deref(),
    };
    let (w, h) = terminal.size()?;
    terminal.present(ui::draw_scores(&board, w, h))?;
    input::next_menu_key()?;
    Ok(())
}

/// Plays one run from `seed` until death or giving up, ending on the
/// death screen.
fn play(terminal: &mut Terminal, seed: u64, start_with_bot: bool, record: bool) -> io::Result<()> {
    let mut game = Game::new(seed);
    let mut autoplay = None;
    if start_with_bot {
        start_bot(&mut game, &mut autoplay);
    }
    let mut recorder = if record {
        start_recording(&mut game)
    } else {
        None
    };
    let mut depth_recorded = game.depth;
    // Who chose the actions the game is about to report.
    let mut source = Source::You;

    // Whether the bot played any part of this run. Such runs don't
    // go on the high score list.
    let mut bot_played = false;
    let mut clock = PlayClock::new();

    // The whole game loop: draw, wait for a key, apply it, repeat.
    loop {
        bot_played |= autoplay.is_some();
        let played = clock.tick();
        game.stats.seconds_played = played.as_secs();
        save_steps(&mut recorder, &mut game, played, source);
        if game.depth != depth_recorded {
            depth_recorded = game.depth;
            if let Some(rec) = recorder.as_mut() {
                let _ = rec.floor(record::Floor {
                    depth: game.depth,
                    ms: played.as_millis() as u64,
                    turn: game.turn,
                });
            }
        }
        let status = autoplay.as_ref().map(Autoplay::status);
        draw_with_status(terminal, &game, status.as_deref())?;

        if game.death.is_some() {
            if let Some(rec) = recorder.as_mut() {
                // A failure here only costs the end line; replays still
                // work without it.
                let _ = rec.finish(&game);
            }
            let (scores, this_run, note) = record_score(&game, bot_played);
            let board = ui::Board {
                scores: &scores,
                this_run,
                note: note.as_deref(),
            };
            let (w, h) = terminal.size()?;
            terminal.present(ui::draw_death(&game, &board, w, h))?;
            input::wait_for_any_key()?;
            return Ok(());
        }

        if let Some(bot) = autoplay.as_mut() {
            // Paused: wait as long as it takes for a key.
            let wait = if bot.paused {
                Duration::from_secs(3600)
            } else {
                Duration::from_millis(BOT_SPEEDS_MS[bot.speed])
            };
            match input::poll_key(wait)? {
                Polled::Nothing if !bot.paused => {
                    source = Source::Bot;
                    let turn = game.turn;
                    game.apply(bot::next_action(&game, &mut bot.memory));
                    bot.idle = if game.turn == turn { bot.idle + 1 } else { 0 };
                    if bot.idle >= 20 {
                        autoplay = None;
                        game.log("The bot seems stuck, so you take back control.");
                    }
                }
                Polled::Nothing => {}
                Polled::Key(Some('+' | '=')) => {
                    bot.speed = (bot.speed + 1).min(BOT_SPEEDS_MS.len() - 1)
                }
                Polled::Key(Some('-')) => bot.speed = bot.speed.saturating_sub(1),
                Polled::Key(Some(' ')) => bot.paused = !bot.paused,
                Polled::Key(_) => {
                    autoplay = None;
                    game.log("You take back control.");
                }
            }
            continue;
        }

        let command = input::next_command()?;
        source = match command {
            Command::Explore | Command::Descend => Source::Auto,
            _ => Source::You,
        };
        match command {
            Command::Act(action) => game.apply(action),
            Command::Close => close_door(terminal, &mut game)?,
            Command::Use(verb) => use_item(terminal, &mut game, verb, None)?,
            Command::Inventory => show_inventory(terminal, &mut game)?,
            Command::Help => {
                show_box(terminal, &game, "Help", &ui::help_lines())?;
            }
            Command::Look => {
                show_box(terminal, &game, "In view", &ui::look_lines(&game))?;
            }
            Command::History => {
                let title = "Messages, newest first";
                show_box(terminal, &game, title, &ui::history_lines(&game))?;
            }
            Command::Character => {
                show_box(terminal, &game, "Character", &ui::character_lines(&game))?;
            }
            Command::Descend => descend_or_travel(terminal, &mut game)?,
            Command::Explore => {
                if auto_move(terminal, &mut game, bot::explore_step)? {
                    game.log("There is nothing left to explore here.");
                }
            }
            Command::ToggleBot => start_bot(&mut game, &mut autoplay),
            Command::Redraw => {}
            Command::Quit => {
                // The run ends like a death, with the same screen and
                // a place on the high scores.
                if confirm_quit(terminal, &mut game)? {
                    game.give_up();
                    if let Some(rec) = recorder.as_mut() {
                        let _ = rec.write(clock.tick().as_millis() as u64, source, &[Step::Quit]);
                    }
                }
            }
        }
    }
}

fn start_bot(game: &mut Game, autoplay: &mut Option<Autoplay>) {
    *autoplay = Some(Autoplay::new());
    game.log("The bot takes over. Space pauses, + and - set speed, Esc or B stops it.");
}

/// Repeats automatic steps (exploring, or walking to the stairs) until
/// there are none left, something happens that needs the player, or a
/// key is pressed. Returns true if it ran out of steps normally.
fn auto_move(
    terminal: &mut Terminal,
    game: &mut Game,
    step: fn(&Game) -> Option<Action>,
) -> io::Result<bool> {
    if let Some(reason) = bot::auto_blocked(game) {
        game.log(&reason);
        return Ok(false);
    }
    // A generous cap, in case something unforeseen keeps it going.
    for _ in 0..2_000 {
        let Some(action) = step(game) else {
            return Ok(true);
        };
        let watch = bot::Watch::new(game);
        game.apply(action);
        if let Some(reason) = watch.reason_to_stop(game) {
            if !reason.is_empty() {
                game.log(&reason);
            }
            return Ok(false);
        }
        draw(terminal, game)?;
        // A short pause makes the walk visible, and any key stops it.
        if input::poll_key(Duration::from_millis(12))? != Polled::Nothing {
            return Ok(false);
        }
    }
    Ok(false)
}

/// `>`: descend when on the stairs; otherwise walk to them if seen.
fn descend_or_travel(terminal: &mut Terminal, game: &mut Game) -> io::Result<()> {
    let on_stairs = game.map.tile(game.player.pos) == map::Tile::StairsDown;
    let stairs_seen = game
        .map
        .points()
        .any(|p| game.map.is_revealed(p) && game.map.tile(p) == map::Tile::StairsDown);
    if on_stairs || !stairs_seen {
        game.apply(Action::Descend);
        return Ok(());
    }
    if auto_move(terminal, game, bot::stairs_step)? {
        if game.map.tile(game.player.pos) == map::Tile::StairsDown {
            game.log("You reach the stairs. Press > again to descend.");
        } else {
            game.log("You can't find a way to the stairs.");
        }
    }
    Ok(())
}

fn draw_with_status(terminal: &mut Terminal, game: &Game, status: Option<&str>) -> io::Result<()> {
    let (w, h) = terminal.size()?;
    let mut frame = ui::draw(game, w, h);
    if let Some(text) = status {
        ui::draw_status(&mut frame, text);
    }
    terminal.present(frame)
}

fn draw(terminal: &mut Terminal, game: &Game) -> io::Result<()> {
    let (w, h) = terminal.size()?;
    terminal.present(ui::draw(game, w, h))
}

/// Shows a box over the map and waits for a key, turning pages when
/// the lines don't fit: space or > goes forward, < goes back. Returns
/// the first other key (`None` for Escape and the like).
fn show_box(
    terminal: &mut Terminal,
    game: &Game,
    title: &str,
    lines: &[Line],
) -> io::Result<Option<char>> {
    show_box_over(terminal, &|w, h| ui::draw(game, w, h), title, lines)
}

/// Like `show_box`, over any background: the map in play, or a blank
/// screen at the title.
fn show_box_over(
    terminal: &mut Terminal,
    background: &dyn Fn(u16, u16) -> frame::Frame,
    title: &str,
    lines: &[Line],
) -> io::Result<Option<char>> {
    let mut page = 0;
    loop {
        let (w, h) = terminal.size()?;
        let (frame, pages) = ui::box_over(background(w, h), (w, h), title, lines, page);
        terminal.present(frame)?;
        match input::next_menu_key()? {
            Some(' ' | '>') if pages > 1 => page = (page + 1) % pages,
            Some('<') if pages > 1 => page = (page + pages - 1) % pages,
            key => return Ok(key),
        }
    }
}

/// Shows a list of items and waits for a letter. Returns the letter if
/// it names one of the listed items, or `None` if the player cancels.
fn choose_item(
    terminal: &mut Terminal,
    game: &Game,
    title: &str,
    show: impl Fn(&Item) -> bool,
) -> io::Result<Option<char>> {
    let key = show_box(terminal, game, title, &ui::item_list(game, &show))?;
    Ok(key.filter(|&c| game.player.item(c).is_some_and(&show)))
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
    let is_food = |i: &Item| matches!(i.kind, ItemKind::Food(_));
    let (title, show, none): (&str, ItemFilter, &str) = match verb {
        Verb::Drop => ("Drop what?", &|_| true, "You have nothing to drop."),
        Verb::Equip => (
            "Equip or remove what?",
            &is_gear,
            "You have nothing to equip.",
        ),
        Verb::Drink => ("Drink what?", &is_potion, "You have no potions."),
        Verb::Read => ("Read what?", &is_scroll, "You have no scrolls."),
        Verb::Eat => ("Eat what?", &is_food, "You have nothing to eat."),
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
        Verb::Eat => Action::Eat(letter),
        Verb::Read => {
            let target = if game.scroll_needs_target(letter) {
                match choose_read_target(terminal, game, letter)? {
                    Some(target) => target,
                    None => return Ok(()),
                }
            } else {
                None
            };
            Action::Read {
                scroll: letter,
                target,
            }
        }
    };
    game.apply(action);
    Ok(())
}

/// Asks what a scroll of enchanting or identify should work on.
///
/// Returns `Some(target)` to go ahead with the reading (the target may
/// be `None`), or `None` to cancel it. Only a scroll the player already
/// knows can be cancelled: an unknown one is read the moment it's
/// chosen, and its magic is wasted if nothing is picked.
fn choose_read_target(
    terminal: &mut Terminal,
    game: &mut Game,
    scroll: char,
) -> io::Result<Option<Option<char>>> {
    let kind = game.player.item(scroll).expect("chosen from the pack").kind;
    let known = game.lore.knows(kind);
    let any = game
        .player
        .inventory
        .iter()
        .any(|i| game.is_read_target(scroll, i));
    if !any {
        if known {
            game.log("There is nothing for that scroll to work on.");
            return Ok(None);
        }
        return Ok(Some(None));
    }
    let title = match kind {
        ItemKind::Scroll(item::ScrollKind::Enchanting) => "Enchant what?",
        _ => "Identify what?",
    };
    let target = choose_item(terminal, game, title, |i| game.is_read_target(scroll, i))?;
    match (target, known) {
        (Some(t), _) => Ok(Some(Some(t))),
        (None, true) => {
            game.log("Never mind.");
            Ok(None)
        }
        (None, false) => Ok(Some(None)),
    }
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
    let title = format!("{}) {}", item.letter, game.lore.name(&item));
    let key = show_box(terminal, game, &title, &ui::item_details(&game.lore, &item))?;
    let verb = match (key, item.kind) {
        (Some('d'), _) => Verb::Drop,
        (Some('e'), k) if k.is_equipment() => Verb::Equip,
        (Some('q'), ItemKind::Potion(_)) => Verb::Drink,
        (Some('r'), ItemKind::Scroll(_)) => Verb::Read,
        (Some('E'), ItemKind::Food(_)) => Verb::Eat,
        _ => return Ok(()),
    };
    use_item(terminal, game, verb, Some(letter))
}

/// Asks before quitting, since quitting ends the run for good.
fn confirm_quit(terminal: &mut Terminal, game: &mut Game) -> io::Result<bool> {
    let lines = [Line::new(
        "This ends the run. Press y to give up.",
        frame::Rgb(190, 190, 190),
    )];
    Ok(show_box(terminal, game, "Quit?", &lines)? == Some('y'))
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

/// A seed from the clock, kept under a billion so it's short enough to
/// read off the screen and type back in.
fn clock_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    (nanos % 1_000_000_000) as u64
}
