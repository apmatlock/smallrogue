//! The score and the high score list: the best runs, kept in a small
//! text file between games. Runs rank by points (see `points`); at
//! equal points, the run that got there in fewer turns ranks higher.
//!
//! The file is `highscores.tsv` in the data folder (see `data_dir`):
//! `~/.local/share/smallrogue`, or `%APPDATA%\smallrogue` on Windows.
//! One run per line, fields
//! separated by tabs, the cause of death last since it is free text.
//! The list from before points, ranked by depth alone, is left as it
//! was in `scores.tsv` next to it.

use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::game::Game;

/// How many runs the list keeps.
pub const KEEP: usize = 10;

/// Points for each floor reached. A deep floor yields about 650
/// experience to the bot, so a floor is worth a little more than a
/// floor's fighting: depth decides the ranking, and experience breaks
/// near-ties between runs that died at about the same depth.
pub const POINTS_PER_FLOOR: u64 = 1000;
/// Points for each artifact picked up: about one floor.
pub const POINTS_PER_ARTIFACT: u64 = 1000;

/// A run's score: its depth, experience earned and artifacts found.
pub fn points(depth: u32, xp: u32, artifacts: usize) -> u64 {
    POINTS_PER_FLOOR * u64::from(depth) + u64::from(xp) + POINTS_PER_ARTIFACT * artifacts as u64
}

const HEADER: &str =
    "# smallrogue high scores v3: points depth level xp turns kills seed date seconds killer";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Score {
    pub points: u64,
    pub depth: u32,
    pub level: u32,
    /// Experience earned over the run.
    pub xp: u32,
    pub turns: u64,
    pub kills: u32,
    pub seed: u64,
    /// The day the run ended, as YYYY-MM-DD.
    pub date: String,
    /// Real time played.
    pub seconds: u64,
    /// What ended the run, e.g. "a troll".
    pub killer: String,
}

impl Score {
    pub fn from_game(game: &Game, date: String) -> Self {
        Self {
            points: game.score(),
            depth: game.depth,
            level: game.player.level,
            xp: game.player.xp,
            turns: game.turn,
            kills: game.stats.total_kills(),
            seed: game.seed,
            date,
            seconds: game.stats.seconds_played,
            killer: game.death.clone().unwrap_or_default(),
        }
    }

    /// Better runs sort first.
    fn rank(&self, other: &Self) -> Ordering {
        other
            .points
            .cmp(&self.points)
            .then(self.turns.cmp(&other.turns))
    }

    fn to_line(&self) -> String {
        // Tabs or newlines in the free text would break the format.
        let killer: String = self
            .killer
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{killer}",
            self.points,
            self.depth,
            self.level,
            self.xp,
            self.turns,
            self.kills,
            self.seed,
            self.date,
            self.seconds
        )
    }

    fn from_line(line: &str) -> Option<Self> {
        let fields: Vec<&str> = line.split('\t').collect();
        let [
            points,
            depth,
            level,
            xp,
            turns,
            kills,
            seed,
            date,
            seconds,
            killer,
        ] = fields[..]
        else {
            return None;
        };
        Some(Self {
            points: points.parse().ok()?,
            depth: depth.parse().ok()?,
            level: level.parse().ok()?,
            xp: xp.parse().ok()?,
            turns: turns.parse().ok()?,
            kills: kills.parse().ok()?,
            seed: seed.parse().ok()?,
            date: date.to_string(),
            seconds: seconds.parse().ok()?,
            killer: killer.to_string(),
        })
    }
}

/// Where the list is kept, if a data folder can be found.
pub fn default_path() -> Option<PathBuf> {
    Some(data_dir()?.join("highscores.tsv"))
}

/// The folder for high scores, settings and recordings, if one can be
/// found. `$XDG_DATA_HOME` wins when set. Windows then uses `%APPDATA%`,
/// its usual place for app data; it usually has no `HOME`, and Git Bash
/// sets one, so checking `%APPDATA%` first keeps every shell saving to
/// the same place. Elsewhere it's `~/.local/share`.
pub fn data_dir() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).map(PathBuf::from);
    let appdata = var("APPDATA").filter(|_| cfg!(windows));
    data_dir_from(var("XDG_DATA_HOME"), appdata, var("HOME"))
}

/// `data_dir` from the variables it reads, so tests can try each case.
fn data_dir_from(
    xdg_data_home: Option<PathBuf>,
    appdata: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    let base = xdg_data_home
        .filter(|p| p.is_absolute())
        .or(appdata.filter(|p| p.is_absolute()))
        .or_else(|| home.map(|h| h.join(".local/share")))?;
    Some(base.join("smallrogue"))
}

/// Reads the list. A missing file is an empty list; lines that can't
/// be read are skipped rather than losing the rest.
pub fn load(path: &Path) -> io::Result<Vec<Score>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut scores: Vec<Score> = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(Score::from_line)
        .collect();
    scores.sort_by(Score::rank);
    scores.truncate(KEEP);
    Ok(scores)
}

pub fn save(path: &Path, scores: &[Score]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut text = format!("{HEADER}\n");
    for s in scores {
        text.push_str(&s.to_line());
        text.push('\n');
    }
    // Write a new file, then swap it in, so a crash midway can't leave
    // half a list behind.
    let tmp = path.with_extension("tsv.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Adds a run to the list, keeping the best `KEEP`. Returns its place
/// (0 for first), or `None` if it didn't make the list. A run that ties
/// an older one ranks below it.
pub fn insert(scores: &mut Vec<Score>, score: Score) -> Option<usize> {
    let place = scores
        .iter()
        .position(|s| score.rank(s) == Ordering::Less)
        .unwrap_or(scores.len());
    if place >= KEEP {
        return None;
    }
    scores.insert(place, score);
    scores.truncate(KEEP);
    Some(place)
}

/// Today's date in UTC as YYYY-MM-DD.
pub fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    date_from_days((secs / 86_400) as i64)
}

/// A calendar date from days since 1970-01-01, using Howard Hinnant's
/// `civil_from_days` algorithm.
pub(crate) fn date_from_days(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run that reached `depth` with `xp` experience, after `turns`.
    fn score(depth: u32, xp: u32, turns: u64) -> Score {
        Score {
            points: points(depth, xp, 0),
            depth,
            level: 1,
            xp,
            turns,
            kills: 0,
            seed: 7,
            date: "2026-09-29".to_string(),
            seconds: 600,
            killer: "a rat".to_string(),
        }
    }

    #[test]
    fn points_add_depth_experience_and_artifacts() {
        assert_eq!(points(1, 0, 0), 1000);
        assert_eq!(points(20, 4762, 1), 20_000 + 4762 + 1000);
        // A floor deeper outranks a floor's worth of fighting.
        assert!(points(21, 4000, 0) > points(20, 4650, 0));
    }

    #[test]
    fn more_points_rank_higher_then_fewer_turns() {
        let mut list = Vec::new();
        assert_eq!(insert(&mut list, score(5, 100, 900)), Some(0));
        assert_eq!(insert(&mut list, score(9, 400, 2000)), Some(0));
        assert_eq!(insert(&mut list, score(5, 100, 500)), Some(1));
        assert_eq!(
            insert(&mut list, score(5, 100, 500)),
            Some(2),
            "ties go below"
        );
        // At the same depth, more experience wins whatever the turns.
        assert_eq!(insert(&mut list, score(5, 300, 3000)), Some(1));
        let rows: Vec<_> = list.iter().map(|s| (s.depth, s.xp, s.turns)).collect();
        assert_eq!(
            rows,
            [
                (9, 400, 2000),
                (5, 300, 3000),
                (5, 100, 500),
                (5, 100, 500),
                (5, 100, 900)
            ]
        );
    }

    #[test]
    fn only_the_best_are_kept() {
        let mut list = Vec::new();
        for depth in 1..=KEEP as u32 {
            insert(&mut list, score(depth + 10, 0, 100));
        }
        assert_eq!(insert(&mut list, score(1, 0, 100)), None);
        assert_eq!(insert(&mut list, score(30, 0, 100)), Some(0));
        assert_eq!(list.len(), KEEP);
        assert_eq!(list.last().unwrap().depth, 12, "the worst fell off");
    }

    #[test]
    fn scores_survive_a_save_and_load() {
        let dir = std::env::temp_dir().join(format!("smallrogue-test-{}", std::process::id()));
        let path = dir.join("highscores.tsv");
        let mut list = vec![score(3, 20, 300), score(8, 300, 800)];
        list[0].killer = "a\ttricky\nname".to_string();
        save(&path, &list).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded[0], list[1], "sorted on load, every field kept");
        assert_eq!(loaded[1].killer, "a tricky name");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_file_is_an_empty_list_and_bad_lines_are_skipped() {
        let path = std::env::temp_dir().join("smallrogue-no-such-scores.tsv");
        assert_eq!(load(&path).unwrap(), Vec::new());
        assert_eq!(Score::from_line("not\ta\tscore"), None);
        // A line from the old depth-only list isn't mistaken for one.
        let old = "9\t12\t4000\t80\t7\t2026-09-29\t600\ta troll";
        assert_eq!(Score::from_line(old), None);
    }

    #[test]
    fn the_data_folder_follows_xdg_then_appdata_then_home() {
        let abs = |name: &str| std::env::temp_dir().join(name);
        let some = |name: &str| Some(abs(name));
        let dir = |xdg, appdata, home| data_dir_from(xdg, appdata, home);
        assert_eq!(
            dir(some("xdg"), some("appdata"), some("home")),
            some("xdg").map(|p| p.join("smallrogue")),
            "XDG_DATA_HOME wins"
        );
        assert_eq!(
            dir(None, some("appdata"), some("home")),
            some("appdata").map(|p| p.join("smallrogue")),
            "APPDATA before HOME, so Git Bash and PowerShell agree"
        );
        assert_eq!(
            dir(None, None, some("home")),
            some("home").map(|p| p.join(".local/share").join("smallrogue"))
        );
        // Relative paths are ignored rather than trusted.
        assert_eq!(
            dir(Some("rel".into()), Some("rel".into()), some("home")),
            some("home").map(|p| p.join(".local/share").join("smallrogue"))
        );
        assert_eq!(dir(None, None, None), None);
    }

    #[test]
    fn the_new_list_has_its_own_file() {
        // The old depth-ranked list in scores.tsv is left alone.
        if let Some(path) = default_path() {
            assert_eq!(path.file_name().unwrap(), "highscores.tsv");
        }
    }

    #[test]
    fn dates_come_out_right() {
        assert_eq!(date_from_days(0), "1970-01-01");
        assert_eq!(date_from_days(11_016), "2000-02-29");
        assert_eq!(date_from_days(20_725), "2026-09-29");
    }
}
