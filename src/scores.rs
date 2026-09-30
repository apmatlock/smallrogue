//! The high score list: the best runs, kept in a small text file
//! between games. Depth is the score; at equal depth, the run that got
//! there in fewer turns ranks higher.
//!
//! The file lives at `$XDG_DATA_HOME/smallrogue/scores.tsv`, or
//! `~/.local/share/smallrogue/scores.tsv`. One run per line, fields
//! separated by tabs, the cause of death last since it is free text.

use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::game::Game;

/// How many runs the list keeps.
pub const KEEP: usize = 10;

const HEADER: &str =
    "# smallrogue high scores v2: depth level turns kills seed date seconds killer";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Score {
    pub depth: u32,
    pub level: u32,
    pub turns: u64,
    pub kills: u32,
    pub seed: u64,
    /// The day the run ended, as YYYY-MM-DD.
    pub date: String,
    /// Real time played (0 for runs saved before this was kept).
    pub seconds: u64,
    /// What ended the run, e.g. "a troll".
    pub killer: String,
}

impl Score {
    pub fn from_game(game: &Game, date: String) -> Self {
        Self {
            depth: game.depth,
            level: game.player.level,
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
            .depth
            .cmp(&self.depth)
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
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{killer}",
            self.depth, self.level, self.turns, self.kills, self.seed, self.date, self.seconds
        )
    }

    /// Reads a line in either format: version 1 had no time played.
    /// The killer never holds a tab, so the field count tells them
    /// apart.
    fn from_line(line: &str) -> Option<Self> {
        let fields: Vec<&str> = line.split('\t').collect();
        let (seconds, killer) = match fields.len() {
            7 => (0, fields[6]),
            8 => (fields[6].parse().ok()?, fields[7]),
            _ => return None,
        };
        Some(Self {
            depth: fields[0].parse().ok()?,
            level: fields[1].parse().ok()?,
            turns: fields[2].parse().ok()?,
            kills: fields[3].parse().ok()?,
            seed: fields[4].parse().ok()?,
            date: fields[5].to_string(),
            seconds,
            killer: killer.to_string(),
        })
    }
}

/// Where the list is kept, if a home directory can be found.
pub fn default_path() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(data.join("smallrogue").join("scores.tsv"))
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
fn date_from_days(days: i64) -> String {
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

    fn score(depth: u32, turns: u64) -> Score {
        Score {
            depth,
            level: 1,
            turns,
            kills: 0,
            seed: 7,
            date: "2026-09-29".to_string(),
            seconds: 600,
            killer: "a rat".to_string(),
        }
    }

    #[test]
    fn old_lines_without_a_time_still_load() {
        let old = Score::from_line("9\t12\t4000\t80\t7\t2026-09-29\ta troll").unwrap();
        assert_eq!(
            (old.depth, old.seconds, old.killer.as_str()),
            (9, 0, "a troll")
        );
        let new = Score::from_line(&score(9, 4000).to_line()).unwrap();
        assert_eq!(new.seconds, 600);
    }

    #[test]
    fn deeper_ranks_higher_then_fewer_turns() {
        let mut list = Vec::new();
        assert_eq!(insert(&mut list, score(5, 900)), Some(0));
        assert_eq!(insert(&mut list, score(9, 2000)), Some(0));
        assert_eq!(insert(&mut list, score(5, 500)), Some(1));
        assert_eq!(insert(&mut list, score(5, 500)), Some(2), "ties go below");
        let depths: Vec<_> = list.iter().map(|s| (s.depth, s.turns)).collect();
        assert_eq!(depths, [(9, 2000), (5, 500), (5, 500), (5, 900)]);
    }

    #[test]
    fn only_the_best_are_kept() {
        let mut list = Vec::new();
        for depth in 1..=KEEP as u32 {
            insert(&mut list, score(depth + 10, 100));
        }
        assert_eq!(insert(&mut list, score(1, 100)), None);
        assert_eq!(insert(&mut list, score(30, 100)), Some(0));
        assert_eq!(list.len(), KEEP);
        assert_eq!(list.last().unwrap().depth, 12, "the worst fell off");
    }

    #[test]
    fn scores_survive_a_save_and_load() {
        let dir = std::env::temp_dir().join(format!("smallrogue-test-{}", std::process::id()));
        let path = dir.join("scores.tsv");
        let mut list = vec![score(3, 300), score(8, 800)];
        list[0].killer = "a\ttricky\nname".to_string();
        save(&path, &list).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded[0].depth, 8, "sorted on load");
        assert_eq!(loaded[1].killer, "a tricky name");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_file_is_an_empty_list_and_bad_lines_are_skipped() {
        let path = std::env::temp_dir().join("smallrogue-no-such-scores.tsv");
        assert_eq!(load(&path).unwrap(), Vec::new());
        assert_eq!(Score::from_line("not\ta\tscore"), None);
    }

    #[test]
    fn dates_come_out_right() {
        assert_eq!(date_from_days(0), "1970-01-01");
        assert_eq!(date_from_days(11_016), "2000-02-29");
        assert_eq!(date_from_days(20_725), "2026-09-29");
    }
}
