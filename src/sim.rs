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
use crate::item::{self, ItemKind, WeaponKind};

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
    /// The artifacts lying on floors the bot reached.
    pub offered: Vec<WeaponKind>,
    /// The artifacts it picked up.
    pub found: Vec<WeaponKind>,
    /// Monsters killed with an artifact made to slay them.
    pub slayer_kills: u32,
}

/// The artifact the player is wielding, if any.
fn wielded_artifact(game: &Game) -> Option<WeaponKind> {
    match game.player.weapon()?.kind {
        ItemKind::Weapon(w) if w.is_artifact() => Some(w),
        _ => None,
    }
}

/// How many monsters of an artifact's prey the player has killed.
fn prey_killed(game: &Game, artifact: WeaponKind) -> u32 {
    let prey = artifact.slaying().map_or(&[][..], |s| s.prey);
    prey.iter()
        .map(|kind| {
            game.stats
                .kills
                .get(kind.species().name)
                .copied()
                .unwrap_or(0)
        })
        .sum()
}

/// Adds any artifacts in `kinds` not already in `list`.
fn note(list: &mut Vec<WeaponKind>, kinds: impl IntoIterator<Item = WeaponKind>) {
    for kind in kinds {
        if !list.contains(&kind) {
            list.push(kind);
        }
    }
}

/// Plays one run with the bot, until it dies or reaches `max_turns`.
pub fn play(seed: u64, max_turns: u64) -> RunResult {
    let mut game = Game::new(seed);
    let mut memory = bot::BotMemory::default();
    let mut idle = 0;
    let (mut depth, mut depth_turn) = (game.depth, game.turn);
    let (mut offered, mut found, mut slayer_kills) = (Vec::new(), Vec::new(), 0);
    let ending = loop {
        if game.death.is_some() {
            break Ending::Died;
        }
        if game.turn >= max_turns {
            break Ending::TurnLimit;
        }
        let turn = game.turn;
        let wielded = wielded_artifact(&game);
        let before = wielded.map(|w| prey_killed(&game, w));
        game.apply(bot::next_action(&game, &mut memory));
        if let (Some(w), Some(before)) = (wielded, before) {
            slayer_kills += prey_killed(&game, w) - before;
        }
        note(
            &mut found,
            game.player.inventory.iter().filter_map(|i| match i.kind {
                ItemKind::Weapon(w) if w.is_artifact() => Some(w),
                _ => None,
            }),
        );
        idle = if game.turn == turn { idle + 1 } else { 0 };
        if idle >= STUCK_AFTER {
            break Ending::Stuck;
        }
        if game.depth != depth {
            (depth, depth_turn) = (game.depth, game.turn);
            note(&mut offered, item::artifacts_on_floor(seed, depth));
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
        offered,
        found,
        slayer_kills,
    }
}

/// Artifact names for the CSV, joined with "+".
fn names(kinds: &[WeaponKind]) -> String {
    kinds
        .iter()
        .map(|k| format!("{k:?}"))
        .collect::<Vec<_>>()
        .join("+")
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
        let r = play(seed, MAX_TURNS);
        // One line per finished run, so progress can be followed with
        // `tail -f` when stderr goes to a file.
        eprintln!("{}/{runs}  {}", results.len() + 1, run_line(&r));
        results.push(r);
    }
    std::fs::write(csv_path, to_csv(&results))?;
    print!("{}", summary(&results, started.elapsed().as_secs_f64()));
    println!("CSV with one row per run: {}", csv_path.display());
    Ok(())
}

/// A short account of one run, e.g. "seed 7: depth 3, turn 300, killed by a goblin".
pub fn run_line(r: &RunResult) -> String {
    let end = match r.ending {
        Ending::Died => format!("killed by {}", r.killer),
        Ending::TurnLimit => "hit the turn limit".to_string(),
        Ending::Stuck => "stuck".to_string(),
        Ending::Stalled => "stalled".to_string(),
    };
    format!(
        "seed {}: depth {}, turn {}, level {}, {end}",
        r.seed, r.depth, r.turns, r.level
    )
}

pub fn to_csv(results: &[RunResult]) -> String {
    let mut csv = String::from("seed,depth,turns,level,ending,killer,offered,found,slayer_kills\n");
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
            "{},{},{},{},{ending},\"{killer}\",{},{},{}",
            r.seed,
            r.depth,
            r.turns,
            r.level,
            names(&r.offered),
            names(&r.found),
            r.slayer_kills
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
            offered: vec![WeaponKind::Sunsteel],
            found: vec![],
            slayer_kills: 0,
        }
    }

    #[test]
    fn a_run_line_says_how_it_ended() {
        assert_eq!(
            run_line(&result(7, 3, Ending::Died, "a goblin")),
            "seed 7: depth 3, turn 300, level 3, killed by a goblin"
        );
        assert_eq!(
            run_line(&result(8, 2, Ending::Stalled, "")),
            "seed 8: depth 2, turn 200, level 2, stalled"
        );
    }

    #[test]
    fn csv_has_a_header_and_a_row_per_run() {
        let csv = to_csv(&[result(7, 3, Ending::Died, "a goblin")]);
        assert_eq!(
            csv,
            "seed,depth,turns,level,ending,killer,offered,found,slayer_kills\n\
             7,3,300,3,died,\"a goblin\",Sunsteel,,0\n"
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
