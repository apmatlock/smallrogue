//! Recordings: every action of a run, with when it happened and who
//! chose it, saved as a small text file while the run is played.
//!
//! A run is fully decided by its seed and its actions, so a recording
//! replays exactly. Recordings serve replays (watching a run again)
//! and analysis (how long floors take, where runs end, how a player's
//! choices compare with the bot's).
//!
//! The format, one entry per line after a short header:
//!
//! ```text
//! smallrogue recording v1
//! seed 1234
//! started 2026-09-30 14:05:09
//! 1520 you move 1 0
//! 1633 auto move 0 1
//! floor 2 at 61200 turn 312
//! 90210 you quit
//! end depth 7 turn 2150 level 9 by a troll
//! ```
//!
//! Times are milliseconds of play, with long idle gaps capped, so they
//! add up to the time played. Sources are `you`, `auto` (auto-explore
//! and travel) and `bot`. `floor` lines mark reaching a new depth, so
//! floor times can be read even when a later version of the game
//! would no longer replay the run the same way.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::game::{Action, Game};
use crate::geom::Point;

const HEADER: &str = "smallrogue recording v1";

/// Who chose an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    You,
    /// Auto-explore or travel to the stairs.
    Auto,
    Bot,
}

impl Source {
    fn word(self) -> &'static str {
        match self {
            Source::You => "you",
            Source::Auto => "auto",
            Source::Bot => "bot",
        }
    }

    fn from_word(word: &str) -> Option<Self> {
        Some(match word {
            "you" => Source::You,
            "auto" => Source::Auto,
            "bot" => Source::Bot,
            _ => return None,
        })
    }
}

/// One recorded step: an action, or quitting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Act(Action),
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Milliseconds of play when it happened.
    pub ms: u64,
    pub source: Source,
    pub step: Step,
}

/// How the run ended, as recorded. Replays check against it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct End {
    pub depth: u32,
    pub turn: u64,
    pub level: u32,
    pub killer: String,
}

impl End {
    pub fn of(game: &Game) -> Self {
        Self {
            depth: game.depth,
            turn: game.turn,
            level: game.player.level,
            killer: game.death.clone().unwrap_or_default(),
        }
    }
}

/// Reaching a new depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Floor {
    pub depth: u32,
    pub ms: u64,
    pub turn: u64,
}

/// A recording read back from a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recording {
    pub seed: u64,
    /// When the run started, as "YYYY-MM-DD HH:MM:SS" (UTC).
    pub started: String,
    pub entries: Vec<Entry>,
    /// New depths reached, in order. Depth 1 is not listed.
    pub floors: Vec<Floor>,
    /// Missing if the game was closed or crashed mid-run.
    pub end: Option<End>,
}

/// Writes a recording as the run is played.
pub struct Recorder {
    out: BufWriter<File>,
}

impl Recorder {
    /// Starts a new recording file for `seed` in `dir`.
    pub fn start(dir: &Path, seed: u64) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let now = now_utc();
        // "2026-09-30 14:05:09" becomes "2026-09-30-140509".
        let stamp = now.replace(' ', "-").replace(':', "");
        let path = dir.join(format!("{stamp}-seed{seed}.rec"));
        let mut out = BufWriter::new(File::create(&path)?);
        writeln!(out, "{HEADER}\nseed {seed}\nstarted {now}")?;
        out.flush()?;
        Ok(Self { out })
    }

    /// Adds steps, all at the same moment and from the same source.
    /// Flushed at once, so a crash loses nothing already played.
    pub fn write(&mut self, ms: u64, source: Source, steps: &[Step]) -> io::Result<()> {
        for &step in steps {
            let entry = Entry { ms, source, step };
            writeln!(self.out, "{}", entry_line(&entry))?;
        }
        self.out.flush()
    }

    /// Notes reaching a new depth.
    pub fn floor(&mut self, floor: Floor) -> io::Result<()> {
        writeln!(
            self.out,
            "floor {} at {} turn {}",
            floor.depth, floor.ms, floor.turn
        )?;
        self.out.flush()
    }

    pub fn finish(&mut self, game: &Game) -> io::Result<()> {
        let e = End::of(game);
        writeln!(
            self.out,
            "end depth {} turn {} level {} by {}",
            e.depth, e.turn, e.level, e.killer
        )?;
        self.out.flush()
    }
}

/// Where recordings are kept: next to the high scores.
pub fn default_dir() -> Option<PathBuf> {
    let scores = crate::scores::default_path()?;
    Some(scores.parent()?.join("recordings"))
}

fn entry_line(e: &Entry) -> String {
    format!("{} {} {}", e.ms, e.source.word(), step_text(e.step))
}

pub(crate) fn step_text(step: Step) -> String {
    let Step::Act(action) = step else {
        return "quit".to_string();
    };
    match action {
        Action::Move(d) => format!("move {} {}", d.x, d.y),
        Action::Wait => "wait".to_string(),
        Action::Descend => "descend".to_string(),
        Action::Close(d) => format!("close {} {}", d.x, d.y),
        Action::PickUp => "pickup".to_string(),
        Action::Drop(c) => format!("drop {c}"),
        Action::Equip(c) => format!("equip {c}"),
        Action::Drink(c) => format!("drink {c}"),
        Action::Eat(c) => format!("eat {c}"),
        Action::Read {
            scroll,
            target: None,
        } => format!("read {scroll}"),
        Action::Read {
            scroll,
            target: Some(t),
        } => format!("read {scroll} {t}"),
    }
}

fn parse_step(words: &[&str]) -> Option<Step> {
    let letter = |i: usize| -> Option<char> {
        let mut chars = words.get(i)?.chars();
        let c = chars.next()?;
        chars.next().is_none().then_some(c)
    };
    let dir = || -> Option<Point> {
        Some(Point::new(
            words.get(1)?.parse().ok()?,
            words.get(2)?.parse().ok()?,
        ))
    };
    let action = match *words.first()? {
        "quit" => return Some(Step::Quit),
        "move" => Action::Move(dir()?),
        "wait" => Action::Wait,
        "descend" => Action::Descend,
        "close" => Action::Close(dir()?),
        "pickup" => Action::PickUp,
        "drop" => Action::Drop(letter(1)?),
        "equip" => Action::Equip(letter(1)?),
        "drink" => Action::Drink(letter(1)?),
        "eat" => Action::Eat(letter(1)?),
        "read" => Action::Read {
            scroll: letter(1)?,
            target: if words.len() > 2 {
                Some(letter(2)?)
            } else {
                None
            },
        },
        _ => return None,
    };
    Some(Step::Act(action))
}

/// Reads a recording back. Unreadable lines are errors, since a replay
/// that skipped one would go wrong from there on.
pub fn load(path: &Path) -> Result<Recording, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn parse(text: &str) -> Result<Recording, String> {
    let mut lines = text.lines().enumerate();
    if lines.next().map(|(_, l)| l) != Some(HEADER) {
        return Err("not a smallrogue recording".to_string());
    }
    let mut seed = None;
    let mut started = String::new();
    let mut entries = Vec::new();
    let mut floors = Vec::new();
    let mut end = None;
    for (i, line) in lines {
        let bad = || format!("line {} can't be read: {line}", i + 1);
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.first().copied() {
            None => {}
            Some("seed") => seed = Some(words.get(1).and_then(|s| s.parse().ok()).ok_or_else(bad)?),
            Some("started") => started = words[1..].join(" "),
            Some("end") => end = Some(parse_end(&words).ok_or_else(bad)?),
            Some("floor") => floors.push(parse_floor(&words).ok_or_else(bad)?),
            Some(ms) => {
                let ms = ms.parse().map_err(|_| bad())?;
                let source = words
                    .get(1)
                    .and_then(|w| Source::from_word(w))
                    .ok_or_else(bad)?;
                let step = parse_step(&words[2..]).ok_or_else(bad)?;
                entries.push(Entry { ms, source, step });
            }
        }
    }
    Ok(Recording {
        seed: seed.ok_or("no seed line")?,
        started,
        entries,
        floors,
        end,
    })
}

/// Reads "floor 2 at 61200 turn 312".
fn parse_floor(words: &[&str]) -> Option<Floor> {
    match words {
        ["floor", depth, "at", ms, "turn", turn] => Some(Floor {
            depth: depth.parse().ok()?,
            ms: ms.parse().ok()?,
            turn: turn.parse().ok()?,
        }),
        _ => None,
    }
}

/// Reads "end depth 7 turn 2150 level 9 by a troll".
fn parse_end(words: &[&str]) -> Option<End> {
    let field = |name: &str| -> Option<&str> {
        let i = words.iter().position(|w| *w == name)?;
        words.get(i + 1).copied()
    };
    let by = words.iter().position(|w| *w == "by")?;
    Some(End {
        depth: field("depth")?.parse().ok()?,
        turn: field("turn")?.parse().ok()?,
        level: field("level")?.parse().ok()?,
        killer: words[by + 1..].join(" "),
    })
}

/// The current time in UTC as "YYYY-MM-DD HH:MM:SS".
fn now_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let day = crate::scores::date_from_days((secs / 86_400) as i64);
    let (h, m, s) = (secs / 3600 % 24, secs / 60 % 60, secs % 60);
    format!("{day} {h:02}:{m:02}:{s:02}")
}

/// Plays a recording's actions into a fresh game, stopping at a quit.
/// Returns the game as the recording left it.
#[cfg(test)]
pub fn replay(recording: &Recording) -> Game {
    let mut game = Game::new(recording.seed);
    for entry in &recording.entries {
        match entry.step {
            Step::Act(action) => game.apply(action),
            Step::Quit => game.give_up(),
        }
    }
    game
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot;

    #[test]
    fn every_action_survives_a_round_trip() {
        let actions = [
            Action::Move(Point::new(-1, 1)),
            Action::Wait,
            Action::Descend,
            Action::Close(Point::new(0, -1)),
            Action::PickUp,
            Action::Drop('a'),
            Action::Equip('b'),
            Action::Drink('c'),
            Action::Eat('d'),
            Action::Read {
                scroll: 'e',
                target: None,
            },
            Action::Read {
                scroll: 'e',
                target: Some('f'),
            },
        ];
        for action in actions {
            let text = step_text(Step::Act(action));
            let words: Vec<&str> = text.split(' ').collect();
            assert_eq!(parse_step(&words), Some(Step::Act(action)));
        }
        assert_eq!(parse_step(&["quit"]), Some(Step::Quit));
        assert_eq!(parse_step(&["drink", "ab"]), None);
        assert_eq!(parse_step(&["fly"]), None);
    }

    /// The whole point: the bot plays a while, its journal becomes a
    /// recording, and replaying that gives the very same game.
    #[test]
    fn a_recording_replays_exactly() {
        let mut game = Game::new(77);
        game.journal = Some(Vec::new());
        let mut memory = bot::BotMemory::default();
        let mut text = format!("{HEADER}\nseed 77\nstarted 2026-09-30 12:00:00\n");
        for i in 0..3000 {
            if game.death.is_some() {
                break;
            }
            game.apply(bot::next_action(&game, &mut memory));
            for action in game.journal.as_mut().unwrap().drain(..) {
                let entry = Entry {
                    ms: i * 100,
                    source: Source::Bot,
                    step: Step::Act(action),
                };
                text.push_str(&entry_line(&entry));
                text.push('\n');
            }
        }
        let e = End::of(&game);
        text.push_str(&format!(
            "end depth {} turn {} level {} by {}\n",
            e.depth, e.turn, e.level, e.killer
        ));

        let recording = parse(&text).unwrap();
        assert_eq!(recording.end.as_ref(), Some(&e));
        let again = replay(&recording);
        assert_eq!(End::of(&again), e);
        assert_eq!(again.player.pos, game.player.pos);
        assert_eq!(again.player.hp, game.player.hp);
        assert_eq!(again.stats.total_kills(), game.stats.total_kills());
    }

    #[test]
    fn a_quit_replays_as_giving_up() {
        let text = format!("{HEADER}\nseed 5\n10 you wait\n20 you quit\n");
        let game = replay(&parse(&text).unwrap());
        assert!(game.gave_up && game.death.is_some());
        assert_eq!(game.turn, 1);
    }

    #[test]
    fn bad_files_are_refused() {
        assert!(parse("hello").is_err());
        assert!(
            parse(&format!("{HEADER}\n10 you wait\n")).is_err(),
            "no seed"
        );
        assert!(parse(&format!("{HEADER}\nseed 1\n10 you dance\n")).is_err());
    }

    #[test]
    fn recorder_files_read_back() {
        let dir = std::env::temp_dir().join(format!("smallrogue-rec-{}", std::process::id()));
        let mut game = Game::new(9);
        let mut rec = Recorder::start(&dir, 9).unwrap();
        rec.write(5, Source::You, &[Step::Act(Action::Wait)])
            .unwrap();
        game.apply(Action::Wait);
        let floor = Floor {
            depth: 2,
            ms: 7,
            turn: 1,
        };
        rec.floor(floor).unwrap();
        rec.finish(&game).unwrap();
        let file = std::fs::read_dir(&dir).unwrap().next().unwrap().unwrap();
        assert!(file.file_name().to_string_lossy().ends_with("-seed9.rec"));
        let back = load(&file.path()).unwrap();
        assert_eq!(back.seed, 9);
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.floors, vec![floor]);
        assert_eq!(back.end, Some(End::of(&game)));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
