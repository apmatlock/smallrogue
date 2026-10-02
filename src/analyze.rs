//! Reports on recorded runs: pace, where runs end, and how a player's
//! choices compare with the bot's.
//!
//! `smallrogue --analyze` reads every recording in the recordings
//! folder; `--analyze PATH` reads one file or another folder.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::bot;
use crate::game::{Action, Game};
use crate::item::ItemKind;
use crate::monster::Ai;
use crate::player::RING_SLOTS;
use crate::record::{self, Recording, Source, Step};
use crate::stats::format_duration;
use crate::zone::Place;

/// What one recording says about its run.
struct Run {
    name: String,
    recording: Recording,
    /// The replay ended as the recording says, so the game hasn't
    /// changed since and the bot comparison is fair.
    replays: bool,
    agreement: Agreement,
    /// What the player still carried at death, unused. `None` unless
    /// the run ended in death and replays.
    unused: Option<Unused>,
}

/// Things that could have helped, still in the pack at death.
#[derive(Default, Clone, Copy)]
struct Unused {
    /// Potions known to heal.
    healing: u32,
    /// Armor heavier (more protective) than what was worn.
    heavier_armor: bool,
    /// Unknown rings, with a ring finger free to try one.
    unknown_rings: bool,
    /// Unknown potions and scrolls.
    unknown_magic: u32,
}

/// How often the player chose what the bot would have.
#[derive(Default)]
struct Agreement {
    /// Choices made with an awake monster in view, and without.
    fighting: Tally,
    quiet: Tally,
    /// Disagreements by kind: (what the bot would do, what you did).
    /// A run of the same disagreement counts once: an unworn ring the
    /// bot wants on would otherwise count at every step.
    differences: BTreeMap<(&'static str, &'static str), u32>,
}

#[derive(Default, Clone, Copy)]
struct Tally {
    same: u32,
    total: u32,
}

impl Tally {
    fn add(&mut self, other: Tally) {
        self.same += other.same;
        self.total += other.total;
    }

    fn percent(self) -> String {
        match self.total {
            0 => "-".to_string(),
            t => format!("{}%", self.same * 100 / t),
        }
    }
}

/// Runs the analysis and returns the report, or why it couldn't.
pub fn analyze(path: Option<&Path>) -> Result<String, String> {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => record::default_dir().ok_or("no home directory to find recordings in")?,
    };
    let files = recording_files(&path)?;
    if files.is_empty() {
        return Err(format!("no recordings in {}", path.display()));
    }
    let mut runs = Vec::new();
    let mut skipped = Vec::new();
    for file in files {
        match record::load(&file) {
            Ok(recording) => runs.push(study(&file, recording)),
            Err(e) => skipped.push(e),
        }
    }
    let mut out = report(&runs);
    for e in skipped {
        let _ = writeln!(out, "Skipped {e}");
    }
    Ok(out)
}

/// The `.rec` files at `path`, oldest name first, or `path` itself if
/// it's a file.
fn recording_files(path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let entries = std::fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rec"))
        .collect();
    files.sort();
    Ok(files)
}

/// Replays a recording alongside the bot, noting at every choice the
/// player made whether the bot would have chosen the same.
fn study(file: &Path, recording: Recording) -> Run {
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut game = Game::new(recording.seed);
    let mut memory = bot::BotMemory::default();
    let mut agreement = Agreement::default();
    let mut ongoing: Option<(&'static str, &'static str)> = None;
    for entry in &recording.entries {
        let Step::Act(action) = entry.step else {
            game.give_up();
            break;
        };
        // A setting, not a move: nothing for the bot to compare.
        if let Action::AutoPickup(_) = action {
            game.apply(action);
            continue;
        }
        // Asked every step, so the bot's memory follows the run.
        let bot_choice = bot::next_action(&game, &mut memory);
        if entry.source == Source::You {
            let fighting = game
                .monsters
                .iter()
                .any(|m| game.is_visible(m.pos) && m.ai != Ai::Asleep);
            let tally = if fighting {
                &mut agreement.fighting
            } else {
                &mut agreement.quiet
            };
            tally.total += 1;
            if bot_choice == action {
                tally.same += 1;
                ongoing = None;
            } else {
                let pair = (kind(bot_choice), kind(action));
                if ongoing != Some(pair) {
                    *agreement.differences.entry(pair).or_default() += 1;
                }
                ongoing = Some(pair);
            }
        }
        game.apply(action);
    }
    let replays = recording
        .end
        .as_ref()
        .is_some_and(|end| *end == record::End::of(&game));
    let died = game.death.is_some() && !game.gave_up;
    let unused = (replays && died).then(|| unused_at_end(&game));
    Run {
        name,
        recording,
        replays,
        agreement,
        unused,
    }
}

fn unused_at_end(game: &Game) -> Unused {
    let p = &game.player;
    let worn_armor = match p.armor().map(|i| i.kind) {
        Some(ItemKind::Armor(a)) => a.stats().armor,
        _ => 0,
    };
    let unworn = || p.inventory.iter().filter(|i| !i.equipped);
    Unused {
        healing: game.healing_carried(),
        heavier_armor: unworn()
            .any(|i| matches!(i.kind, ItemKind::Armor(a) if a.stats().armor > worn_armor)),
        unknown_rings: p.rings().count() < RING_SLOTS
            && unworn().any(|i| matches!(i.kind, ItemKind::Ring(_)) && !game.lore.knows(i.kind)),
        unknown_magic: unworn()
            .filter(|i| matches!(i.kind, ItemKind::Potion(_) | ItemKind::Scroll(_)))
            .filter(|i| !game.lore.knows(i.kind))
            .map(|i| i.count)
            .sum(),
    }
}

/// A word for the kind of action, to group disagreements.
fn kind(action: Action) -> &'static str {
    match action {
        Action::Move(_) => "move",
        Action::Wait => "wait",
        Action::Descend => "descend",
        Action::Close(_) => "close a door",
        Action::PickUp => "pick up",
        Action::Drop(_) => "drop",
        Action::Equip(_) => "equip",
        Action::Drink(_) => "drink",
        Action::Eat(_) => "eat",
        Action::Read { .. } => "read",
        Action::AutoPickup(_) => "auto pickup",
    }
}

/// Milliseconds of play the recording covers.
fn length_ms(r: &Recording) -> u64 {
    r.entries.last().map_or(0, |e| e.ms)
}

fn report(runs: &[Run]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{} recorded runs\n", runs.len());
    let _ = writeln!(
        out,
        "{:<30} {:>5} {:>8} {:>6}  {:>4}  ended",
        "recording", "depth", "time", "turns", "hand"
    );
    for run in runs {
        let r = &run.recording;
        let ended = match &r.end {
            Some(e) if e.killer.is_empty() => "(alive)".to_string(),
            Some(e) => e.killer.clone(),
            None => "(unfinished)".to_string(),
        };
        let depth = r
            .end
            .as_ref()
            .map_or_else(|| r.floors.last().map_or(1, |f| f.depth), |e| e.depth);
        let turns = r.end.as_ref().map_or(0, |e| e.turn);
        let by_hand = r.entries.iter().filter(|e| e.source == Source::You).count();
        let hand = (by_hand * 100)
            .checked_div(r.entries.len())
            .map_or("-".to_string(), |p| format!("{p}%"));
        let _ = writeln!(
            out,
            "{:<30} {:>5} {:>8} {:>6}  {:>4}  {ended}{}",
            run.name,
            depth,
            format_duration(length_ms(r) / 1000),
            turns,
            hand,
            if run.replays { "" } else { " *" }
        );
    }
    if runs.iter().any(|r| !r.replays) {
        let _ = writeln!(
            out,
            "* doesn't replay the same any more (or unfinished): left out of the bot comparison"
        );
    }

    pace(&mut out, runs);
    left_unused(&mut out, runs);
    compare_with_bot(&mut out, runs);
    out
}

/// What dying players still carried that might have saved them.
fn left_unused(out: &mut String, runs: &[Run]) {
    let deaths: Vec<Unused> = runs.iter().filter_map(|r| r.unused).collect();
    if deaths.is_empty() {
        return;
    }
    let n = deaths.len();
    let count = |f: fn(&Unused) -> bool| deaths.iter().filter(|u| f(u)).count();
    let _ = writeln!(out, "\nLeft unused at death ({n} deaths)");
    let healing: u32 = deaths.iter().map(|u| u.healing).sum();
    let _ = writeln!(
        out,
        "  Healing potions:         {} of {n} deaths ({healing} potions)",
        count(|u| u.healing > 0)
    );
    let _ = writeln!(
        out,
        "  Heavier armor than worn: {} of {n}",
        count(|u| u.heavier_armor)
    );
    let _ = writeln!(
        out,
        "  Unknown rings, unworn:   {} of {n}",
        count(|u| u.unknown_rings)
    );
    let magic: u32 = deaths.iter().map(|u| u.unknown_magic).sum();
    let _ = writeln!(
        out,
        "  Untried potions/scrolls: {} of {n} ({magic} items)",
        count(|u| u.unknown_magic > 0)
    );
}

/// Time and turns per floor, overall and by zone. Floor times come
/// from the recordings' floor lines, so they hold even for runs that
/// no longer replay.
fn pace(out: &mut String, runs: &[Run]) {
    // Per zone, in the order they're met: (floors finished,
    // milliseconds, turns).
    let mut by_zone: BTreeMap<usize, (u64, u64, u64)> = BTreeMap::new();
    let (mut total_ms, mut total_turns) = (0, 0);
    for run in runs {
        let r = &run.recording;
        let mut from = (1, 0, 0); // depth, ms, turn
        for f in &r.floors {
            let z = by_zone.entry(Place::at_depth(from.0).zone).or_default();
            z.0 += 1;
            z.1 += f.ms - from.1;
            z.2 += f.turn - from.2;
            from = (f.depth, f.ms, f.turn);
        }
        // Time and turns must cover the same stretch. An unfinished
        // recording has no final turn count, so it only counts up to
        // the last floor it reached.
        match &r.end {
            Some(end) => {
                total_ms += length_ms(r);
                total_turns += end.turn;
            }
            None => {
                total_ms += from.1;
                total_turns += from.2;
            }
        }
    }
    let _ = writeln!(out, "\nPace");
    if total_turns > 0 {
        let per_100 = total_ms as f64 / total_turns as f64 / 10.0;
        let _ = writeln!(
            out,
            "  {:.1} seconds per 100 turns. The bot's median run to depth 20",
            per_100
        );
        let _ = writeln!(
            out,
            "  (about 5,800 turns) would take you about {} at this pace.",
            format_duration((per_100 * 58.0) as u64)
        );
    }
    let _ = writeln!(
        out,
        "  {:<14} {:>6} {:>10} {:>10}",
        "zone", "floors", "time/floor", "turns/floor"
    );
    for (&zone, (floors, ms, turns)) in &by_zone {
        let _ = writeln!(
            out,
            "  {:<14} {:>6} {:>10} {:>10}",
            crate::zone::ZONES[zone].name,
            floors,
            format_duration(ms / floors / 1000),
            turns / floors
        );
    }
    if by_zone.is_empty() {
        let _ = writeln!(out, "  No floors finished yet.");
    }
}

fn compare_with_bot(out: &mut String, runs: &[Run]) {
    let mut fighting = Tally::default();
    let mut quiet = Tally::default();
    let mut differences: BTreeMap<(&str, &str), u32> = BTreeMap::new();
    for run in runs.iter().filter(|r| r.replays) {
        fighting.add(run.agreement.fighting);
        quiet.add(run.agreement.quiet);
        for (&k, &n) in &run.agreement.differences {
            *differences.entry(k).or_default() += n;
        }
    }
    let _ = writeln!(out, "\nYou and the bot (your own moves only)");
    let _ = writeln!(
        out,
        "  Same choice as the bot: {} with monsters about ({} moves), {} otherwise ({} moves)",
        fighting.percent(),
        fighting.total,
        quiet.percent(),
        quiet.total
    );
    let mut differences: Vec<_> = differences.into_iter().collect();
    differences.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    if !differences.is_empty() {
        let _ = writeln!(
            out,
            "  Most common differences, each stretch counted once (bot would / you did):"
        );
        for ((bot, you), n) in differences.iter().take(8) {
            let _ = writeln!(out, "    {n:>5}  {bot} / {you}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recording of the bot playing seed 3 for a while, as text.
    fn bot_recording(steps: usize) -> String {
        let mut game = Game::new(3);
        game.journal = Some(Vec::new());
        let mut memory = bot::BotMemory::default();
        let mut text = "smallrogue recording v1\nseed 3\nstarted 2026-09-30 12:00:00\n".to_string();
        let mut depth = 1;
        for i in 0..steps as u64 {
            if game.death.is_some() {
                break;
            }
            game.apply(bot::next_action(&game, &mut memory));
            for action in game.journal.as_mut().unwrap().drain(..) {
                // Marked as the player's, so the comparison has moves
                // to compare: they should all match the bot.
                let step = record::step_text(Step::Act(action));
                text.push_str(&format!("{} you {step}\n", i * 500));
            }
            if game.depth != depth {
                depth = game.depth;
                text.push_str(&format!(
                    "floor {depth} at {} turn {}\n",
                    i * 500,
                    game.turn
                ));
            }
        }
        let e = record::End::of(&game);
        text.push_str(&format!(
            "end depth {} turn {} level {} by {}\n",
            e.depth, e.turn, e.level, e.killer
        ));
        text
    }

    #[test]
    fn the_bot_agrees_with_itself_and_floors_are_timed() {
        let dir = std::env::temp_dir().join(format!("smallrogue-analyze-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rec"), bot_recording(2500)).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a recording").unwrap();
        let text = analyze(Some(&dir)).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();

        assert!(text.contains("1 recorded runs"), "{text}");
        assert!(text.contains("Crypts"), "floor times by zone\n{text}");
        assert!(text.contains("seconds per 100 turns"));
        // Replaying the bot's own moves, the bot always agrees.
        let line = text
            .lines()
            .find(|l| l.contains("Same choice as the bot"))
            .unwrap();
        // Both kinds of moment are 100%, or "-" if there were none.
        for part in line.split(", ") {
            assert!(part.contains("100%") || part.contains(" - "), "{line}");
        }
        assert!(!text.contains("Most common differences"), "{text}");
        assert!(
            !text.contains(" *"),
            "the recording replays exactly\n{text}"
        );
    }

    /// Found by the Codex review: an unfinished run's time on its last
    /// floor was counted without its turns, inflating the pace.
    #[test]
    fn an_unfinished_run_counts_only_finished_floors() {
        let text = "smallrogue recording v1\nseed 3\n\
                    100 you wait\n\
                    floor 2 at 60000 turn 200\n\
                    900000 you wait\n";
        let dir =
            std::env::temp_dir().join(format!("smallrogue-unfinished-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("u.rec"), text).unwrap();
        let report = analyze(Some(&dir)).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        // 60 seconds over 200 turns: 30 seconds per 100 turns, not the
        // 900 seconds of the whole recording.
        assert!(report.contains("30.0 seconds per 100 turns"), "{report}");
        assert!(report.contains("(unfinished)"));
    }

    /// Writes a sample recording where `--analyze` can be tried on it.
    #[test]
    #[ignore = "writes target/sample.rec; run with --ignored"]
    fn write_a_sample_recording() {
        std::fs::write("target/sample.rec", bot_recording(4000)).unwrap();
    }

    #[test]
    fn a_stretch_of_the_same_difference_counts_once() {
        // Waiting 50 times where the bot would do something else the
        // whole time: one difference, not 50.
        let mut text = "smallrogue recording v1\nseed 3\n".to_string();
        for i in 0..50 {
            text.push_str(&format!("{} you wait\n", i * 100));
        }
        // Finish it with how the replay really ends, so it counts.
        let e = record::End::of(&record::replay(&record::parse(&text).unwrap()));
        text.push_str(&format!(
            "end depth {} turn {} level {} by {}\n",
            e.depth, e.turn, e.level, e.killer
        ));
        let dir = std::env::temp_dir().join(format!("smallrogue-stretch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("w.rec"), text).unwrap();
        let report = analyze(Some(&dir)).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let header = "each stretch counted once";
        let differences: Vec<&str> = report
            .lines()
            .skip_while(|l| !l.contains(header))
            .skip(1)
            .collect();
        assert_eq!(differences.len(), 1, "{report}");
        assert!(differences[0].trim_start().starts_with("1  "), "{report}");
    }

    #[test]
    fn unused_items_are_found_in_the_pack() {
        use crate::item::{ArmorKind, Item, PotionKind, RingKind, ScrollKind};
        let mut game = Game::new(1);
        let p = &mut game.player;
        for kind in [
            ItemKind::Armor(ArmorKind::Plate),
            ItemKind::Ring(RingKind::Accuracy),
            ItemKind::Potion(PotionKind::Strength),
            ItemKind::Scroll(ScrollKind::Identify),
        ] {
            p.add_item(Item::new(kind)).unwrap();
        }
        let u = unused_at_end(&game);
        // The fighter's starting potion of healing is known.
        assert_eq!(u.healing, 1);
        assert!(u.heavier_armor && u.unknown_rings);
        assert_eq!(u.unknown_magic, 2);
    }

    #[test]
    fn an_empty_folder_says_so() {
        let dir = std::env::temp_dir().join(format!("smallrogue-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = analyze(Some(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(result.unwrap_err().contains("no recordings"));
    }
}
