//! Headless balance runs: the bot plays many seeded games without
//! drawing anything, and the results are summarized.
//!
//! `cargo run --release -- --simulate 200` plays seeds 1 to 200.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::Path;
use std::time::Instant;

use crate::bot;
use crate::game::Game;

/// A run stops here if the bot is still alive, so a run can't go on
/// forever.
pub const MAX_TURNS: u64 = 20_000;

/// Actions in a row that take no time before a run counts as stuck.
const STUCK_AFTER: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    Died,
    TurnLimit,
    /// The bot kept choosing actions that don't use a turn, which is a
    /// bot bug worth knowing about.
    Stuck,
    /// Turns kept passing but no new floor was reached for a long time,
    /// most likely the bot going around in circles.
    Stalled,
}

/// Turns without reaching a new floor before a run counts as stalled.
const STALLED_AFTER: u64 = 5_000;

#[derive(Clone, Debug)]
pub struct RunResult {
    pub seed: u64,
    pub depth: u32,
    pub turns: u64,
    /// The character level reached.
    pub level: u32,
    pub ending: Ending,
    /// What killed the bot, e.g. "a jackal". Empty unless it died.
    pub killer: String,
}

/// Plays one run with the bot, until it dies or reaches `max_turns`.
pub fn play(seed: u64, max_turns: u64) -> RunResult {
    let mut game = Game::new(seed);
    let mut memory = bot::BotMemory::default();
    let mut idle = 0;
    let (mut depth, mut depth_turn) = (game.depth, game.turn);
    let ending = loop {
        if game.death.is_some() {
            break Ending::Died;
        }
        if game.turn >= max_turns {
            break Ending::TurnLimit;
        }
        let turn = game.turn;
        game.apply(bot::next_action(&game, &mut memory));
        idle = if game.turn == turn { idle + 1 } else { 0 };
        if idle >= STUCK_AFTER {
            break Ending::Stuck;
        }
        if game.depth != depth {
            (depth, depth_turn) = (game.depth, game.turn);
        } else if game.turn - depth_turn >= STALLED_AFTER {
            break Ending::Stalled;
        }
    };
    RunResult {
        seed,
        depth: game.depth,
        turns: game.turn,
        level: game.player.level,
        ending,
        killer: game.death.clone().unwrap_or_default(),
    }
}

/// Plays `runs` games from `first_seed` on, prints a summary, and saves
/// one CSV row per run to `csv_path`.
pub fn simulate(runs: u64, first_seed: u64, csv_path: &Path) -> io::Result<()> {
    if first_seed.checked_add(runs - 1).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "that many runs from that seed would go past the largest seed",
        ));
    }
    let started = Instant::now();
    let mut results = Vec::new();
    for seed in (0..runs).map(|i| first_seed + i) {
        results.push(play(seed, MAX_TURNS));
        // A progress line that overwrites itself.
        eprint!("\rPlayed {}/{runs}", results.len());
    }
    eprintln!();
    std::fs::write(csv_path, to_csv(&results))?;
    print!("{}", summary(&results, started.elapsed().as_secs_f64()));
    println!("CSV with one row per run: {}", csv_path.display());
    Ok(())
}

pub fn to_csv(results: &[RunResult]) -> String {
    let mut csv = String::from("seed,depth,turns,level,ending,killer\n");
    for r in results {
        let ending = match r.ending {
            Ending::Died => "died",
            Ending::TurnLimit => "turn limit",
            Ending::Stuck => "stuck",
            Ending::Stalled => "stalled",
        };
        // Killer names never contain commas or quotes, but quote them
        // anyway so the file stays valid if one ever does.
        let killer = r.killer.replace('"', "\"\"");
        let _ = writeln!(
            csv,
            "{},{},{},{},{ending},\"{killer}\"",
            r.seed, r.depth, r.turns, r.level
        );
    }
    csv
}

pub fn summary(results: &[RunResult], seconds: f64) -> String {
    let mut out = String::new();
    let n = results.len().max(1);
    let mut depths: Vec<u32> = results.iter().map(|r| r.depth).collect();
    depths.sort_unstable();
    let avg_depth = depths.iter().sum::<u32>() as f64 / n as f64;
    let median = depths.get(depths.len() / 2).copied().unwrap_or(0);
    let best = depths.last().copied().unwrap_or(0);
    let avg_turns = results.iter().map(|r| r.turns).sum::<u64>() as f64 / n as f64;
    let count = |e: Ending| results.iter().filter(|r| r.ending == e).count();

    let _ = writeln!(out, "Simulated {} runs in {seconds:.1}s", results.len());
    let _ = writeln!(
        out,
        "Depth reached:  average {avg_depth:.1}, median {median}, best {best}"
    );
    let _ = writeln!(out, "Turns survived: average {avg_turns:.0}");
    let _ = writeln!(
        out,
        "Endings:        {} died, {} hit the {MAX_TURNS}-turn limit, {} stuck, {} stalled",
        count(Ending::Died),
        count(Ending::TurnLimit),
        count(Ending::Stuck),
        count(Ending::Stalled)
    );

    let _ = writeln!(out, "\nDepth  Runs");
    let mut by_depth: BTreeMap<u32, usize> = BTreeMap::new();
    for &d in &depths {
        *by_depth.entry(d).or_default() += 1;
    }
    let widest = by_depth.values().copied().max().unwrap_or(1);
    for (depth, runs) in &by_depth {
        let bar = "#".repeat((runs * 40).div_ceil(widest));
        let _ = writeln!(out, "{depth:>5}  {runs:>4}  {bar}");
    }

    let mut killers: BTreeMap<&str, usize> = BTreeMap::new();
    for r in results.iter().filter(|r| r.ending == Ending::Died) {
        *killers.entry(r.killer.as_str()).or_default() += 1;
    }
    let mut killers: Vec<_> = killers.into_iter().collect();
    killers.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    if !killers.is_empty() {
        let _ = writeln!(out, "\nTop causes of death");
        for (killer, runs) in killers.iter().take(8) {
            let _ = writeln!(out, "  {killer:<22} {runs:>4}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(seed: u64, depth: u32, ending: Ending, killer: &str) -> RunResult {
        RunResult {
            seed,
            depth,
            turns: 100 * depth as u64,
            level: depth,
            ending,
            killer: killer.to_string(),
        }
    }

    #[test]
    fn csv_has_a_header_and_a_row_per_run() {
        let csv = to_csv(&[result(7, 3, Ending::Died, "a goblin")]);
        assert_eq!(
            csv,
            "seed,depth,turns,level,ending,killer\n7,3,300,3,died,\"a goblin\"\n"
        );
    }

    #[test]
    fn summary_counts_depths_and_killers() {
        let results = [
            result(1, 2, Ending::Died, "a jackal"),
            result(2, 4, Ending::Died, "a jackal"),
            result(3, 3, Ending::TurnLimit, ""),
        ];
        let text = summary(&results, 1.0);
        assert!(text.contains("average 3.0, median 3, best 4"), "{text}");
        assert!(text.contains("2 died, 1 hit"), "{text}");
        assert!(text.contains("a jackal"), "{text}");
    }

    #[test]
    fn a_run_plays_to_an_end() {
        let r = play(11, 1_500);
        assert!(r.turns > 0);
        assert!(
            !matches!(r.ending, Ending::Stuck | Ending::Stalled),
            "seed 11: {:?} at turn {}",
            r.ending,
            r.turns
        );
    }
}
