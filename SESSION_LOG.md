# Session log

Where the project stands, so work can pick up where it left off. The design decisions and milestone plan live in [DESIGN.md](DESIGN.md); this file is the running status.

## Where we left off — 2026-10-02

Last commit: gear comparison (4170c72). Everything is pushed to GitHub. 224 tests pass (2 more ignored); clippy and rustfmt are clean; Codex reviews were clean on every commit.

**First post-v1 session.** Built:
- Auto pickup toggle: `@` turns it on or off, saved in `settings.txt` next to the high scores (`src/settings.rs`). It's an action (`autopickup on|off` in recordings), so replays stay exact. With it off, walking onto an item names it and `g` picks it up; auto-explore leaves items alone; the bot always collects.
- A whole-codebase Codex review of v1 (read-only `codex exec`), with every bug claim checked against the code. Fixed: a resize in a menu counted as cancel and wasted an unknown scroll at its target prompt (menus now get `MenuKey::{Char, Cancel, Resize}`); auto-explore and travel steps are recorded as they happen (`RunLog` in `src/main.rs`); `--analyze` leaves runs with bot play out of the pace; recording names never collide (`-2`, `-3` suffixes).
- Gear comparison: unequipped weapons and armor show what equipping them would change, in item menus (`e`, `i`) and the details box ("armor +2, dodge -1"). Worked out as the player knows things (`ui::gear_change`, `as_known`): unknown enchantments count as 0 and get a "?".
- Recorded runs: the user's 9 newest (2026-10-01/02) reached depths 3-37, median 9, with best runs of 37 and 32. Compared with the first 10: healing left unused at death 9 of 9 down to 4 of 7, untried potions and scrolls 8 of 9 down to 1 of 7, same choice as the bot in fights 52% up to 64%. Pace 19.2 s per 100 turns (was 13.5), so the bot's median run would take about 19 minutes. One death by starvation at depth 8 (seed 488798273).

**Next:**
1. The first post-v1 milestone, recommended: one ranged enemy that warns a turn before it shoots, with bot support and a balance check (part of monster abilities step 4).
2. The user is gathering more recorded runs; run `--analyze` on the new batch.
3. Left from the Codex v1 review, not urgent: a stronger replay check (a rules id or state checkpoints; today only depth, turn, level and killer are compared); a cleanup guard so a failed start can't leave the terminal in raw mode; wrapping menu text on narrow screens. Other ideas it offered: poison with a cure, suspend and resume, a lure item, a treasure room that wakes guardians, and a shorter low-health warning naming the potion's letter.

## Earlier — 2026-10-01

Last commit: the v1 wrap-up. Everything is pushed to GitHub. 214 tests pass (2 more ignored); clippy (with `-D warnings`) and rustfmt are clean; Codex reviews were clean on every commit.

**Milestone 12 (polish and balance) is done, and with it v1.** Built this stretch:
- Readability: help pages, look (`L`/`;`), message history (`m`), title screen, time played, quitting goes on the high scores.
- Vampires at normal speed, healing half the blood they draw.
- Run recording, on by default: `--replay FILE`, `--analyze [PATH]` (pace by zone, deaths, items left unused, how the player's moves compare with the bot's). Recordings are in `~/.local/share/smallrogue/recordings`.
- From the user's first 10 recordings: hints for low health (naming known healing), more protective armor and unknown rings, and a death-screen line counting unused healing potions. The 10-20 minute target holds at the user's pace (13.5 s per 100 turns).
- README brought up to date.
- Growing loop scaling (`LOOP_GROWTH = 4` in `src/monster.rs`), from a 1,000-run batch on seeds 2001-3000: turn-limit runs 200 to 89, median 22.
- The user's two newest recordings, with the hints: depth 11 (healing used up before dying) and depth 32 (quit at full health, carrying 15 healing potions by depth 11, more than any bot run). Bot runs that reached depth 20 went on to the 30s, so the deep ramp (`DEEP_RAMP_FROM = 24`, `DEEP_EXTRA_PER_FLOOR = 1` in `src/monster.rs`) now makes each floor past 24 one floor stronger: on fresh seeds, deaths spread over 25-36 and runs past 36 fell from 78 to 11 of 200.

**Next:**
1. Pick what comes after v1 with the user: the list is at the end of DESIGN.md (save on quit, more backgrounds and magic, species, more zones and monsters, a signature mechanic, tiles), plus monster abilities step 4 (ranged attacks, poison, paralysis).
2. More recorded runs, then `smallrogue --analyze`. The deep ramp changes the rules past depth 24, so the depth-32 recording no longer replays exactly; its pace lines still count.
3. Open balance notes, not urgent: wraiths, vampires and demons cause about 40% of deaths; the bot's median (21) is a little above the 15-20 target, and getting it lower would mean a harder first loop.

Tools for measuring: `target/big_report.py CSV...` breaks a batch down by zone, killer and estimated run length (untracked); `target/simwatch.py LOG BASELINE.csv TOTAL` (or `-` for no baseline) gives a live view; run sims as copied binaries in parallel and `wait` on their PIDs, never `pgrep -f`.

## Earlier — 2026-09-29 (third session)

Last commit: Milestone 11 wrap-up. **Milestone 11 (themed zones) is done**; the plan and results are in DESIGN.md. Next up: Milestone 12 (polish and balance).

### Milestone 11 progress

- **Step 1 done** (`75eda3f`): `src/zone.rs` with Crypts (1-6), Flooded Halls (7-12), Deep Warrens (13-18), cycling in loops of 18. Each zone sets floor size, rooms, loops, doors and colors; the sidebar names the zone. Balance neutral on fresh seeds (average 28.8 to 28.7), turn-limit runs 68 to 49.
- **Step 2 done** (`6b10f0d`): home monsters 4x as common in their zone, skeletons at depth 3 and draugr at 5, zone item weights (warrens: 40% rations). Fresh seeds against step 1: median 21 to 17, average 28.7 to 25.4, 85 deeper and 102 shallower. New top killers: giant spiders, vampires, oni. Turn-limit runs unchanged at 48 of 200.
- **Step 3 done** (`7b38c97`): shallow water and themed rooms (tomb, cistern, den, larder), kept as extra risk and reward by the user's choice even though they made runs much easier (fresh median 17 to 44). Controlled tests: extra monsters and extra items each strengthen the bot; water and reshuffled rolls alone don't.
- **Step 4 done**: `LOOP_EXTRA_FLOORS = 3` in `src/monster.rs`. Fresh seeds: median 20, average 23.7, turn-limit runs 84 to 26, 66 of 200 past the second crypts. 6 and above made loop 2 a wall.
- **Step 5 done**: Codex reviews clean on every step. Open balance item for milestone 12: vampires kill about a fifth of fresh runs, mostly in loop 2's crypts.
- Measuring: `target/run_zone2.sh` shows the pattern (a copied binary per version, both seed sets in parallel, `wait` on the PIDs). Latest results: `target/zone2_1.csv`, `target/zone2_fresh.csv`.

### Death screen stats and high scores

The death screen now shows the run in numbers (`src/stats.rs`: kills by monster, most slain, toughest foe, accuracy, damage dealt and taken, stairs and trapdoors, items, potions, scrolls, meals) and the high score list (`src/scores.rs`), with this run marked. Scores are ranked by depth, then fewer turns, top 10, saved in `~/.local/share/smallrogue/scores.tsv` (or under `$XDG_DATA_HOME`). Runs the bot played any part of aren't recorded. Blocks drop out in order (messages, then scores) on small terminals so the cause, stats and prompt always show.

### Fetch plan: kept

Once the stairs are known, before heading down, the bot goes back for the nearest item it has seen and wants (the same test as exploring) within 15 steps. The trip has a turn budget (twice the distance plus 10) and pauses for fights (hunting or adjacent monsters) and for resting below 80% health. A fetch that runs out of turns or loses its route is not retried for that item on the same floor.

| | Baseline (`5c7bcc1`) | Fetch |
|---|---|---|
| Seeds 1-100: median / average depth | 14 / 21.0 | 29 / 29.9 |
| Seeds 1-100: deaths | 86 | 66 |
| Seeds 1-100: same seed | | 68 deeper, 19 shallower |
| **Fresh 1001-1200: median / average depth** | **15 / 23.3** | **19 / 28.9** |
| Fresh: deaths | 173 | 131 |
| Fresh: same seed | | 120 deeper, 58 shallower |
| Fresh: hit the 20,000-turn limit | 27 | 69 |

A first version paused for any awake monster and stalled on 5 seeds: a fleeing monkey at the edge of view paused the fetch, the bot turned for the stairs, lost sight of it, and resumed, forever. Fixed, with 0 stalls since.

**Balance note:** results are now split in two. Most runs die by depth 20, but about a third go past depth 40 and never die before the turn limit. The median (19) is at the top of the 15-20 target, and the late game is probably too easy. A job for the balance pass.

Tip: `target/simwatch.py LOG BASELINE.csv TOTAL ...` (untracked) prints a live comparison while simulations run; `watch -n 10 python3 target/simwatch.py ...` in a tmux pane keeps it refreshed.

### Hold plan: tried and dropped

Hold had the bot fall back to a narrow spot (a corridor tile within 6 steps it could reach before any hunter) when two or more monsters were hunting, then wait and fight them one at a time. It looked good on the tuning seeds and was a wash on fresh ones, even after tuning (a 15-turn budget, and no holding when hungry with no food):

| Fresh seeds 1001-1200 | Baseline | First Hold | Tuned Hold |
|---|---|---|---|
| Median depth | 15 | 15 | 15 |
| Average depth | 23.3 | 22.6 | 23.6 |
| Deaths | 173 | 170 | 169 |
| Starvation deaths | 5 | 12 | 9 |
| Same seed vs baseline | | 77 deeper, 74 shallower | 78 deeper, 69 shallower |

On seeds 1-100 both versions showed 47 runs deeper and 22 shallower, another case of tuning-seed gains that don't carry over. The code was not committed; the patch is in `target/hold.patch` (untracked).

`--simulate` now prints one line per finished run (seed, depth, turn, level, how it ended) to stderr, so `2>run.log` plus `tail -f run.log` shows progress.

### Next step

1. **Milestone 12: polish and balance, in progress.** Part A (readability) is done and pushed; vampires are tuned (speed 100, half blood healing). Runs are now recorded (`src/record.rs`, `src/analyze.rs`): every terminal run saves to `~/.local/share/smallrogue/recordings`; `--replay FILE` watches one and `--analyze` reports pace per zone, deaths, and how the player's moves compare with the bot's. The user's first 10 recorded runs (2026-09-30/10-01) ended at depths 2-9 in 1-5 minutes, at a pace of 13.5 seconds per 100 turns (so the bot's median run would take about 13 minutes: the length target holds). They died with healing potions in 9 of 9 deaths and rarely wore better armor or tried rings and unknown items, so hints were added (low health with a healing reminder, gear hints on pick up, a death-screen note) and `--analyze` now lists what was left unused at death. No balance changes from this: the bot uses its items and reaches the target depth. README done. A 1,000-run batch (seeds 2001-3000) then led to growing loop scaling (`LOOP_GROWTH = 4`): turn-limit runs 200 to 89, median unchanged at 22. Left: v1 wrap-up, and more recorded runs to see whether the hints help. Earlier notes: Help screen and final tuning (high scores are done). Balance items: vampires kill about a fifth of fresh runs, mostly in loop 2's crypts; themed rooms made the game easier and loop scaling pulled it back, so floors 1-18 are gentler than before milestone 11.

Smaller open items:
- Monster abilities step 4 (ranged attackers; status effects like poison and paralysis) needs new systems.
- Monster ideas from the bestiary survey that need abilities: hydra, minotaur, basilisk, cockatrice, mimic, gelatinous cube, will-o'-the-wisp, wendigo, banshee.
- Design question left open: should magic mapping reveal items as well as terrain and traps? It currently does, both on screen and for the bot.

## What's built

| Milestone | Status |
|---|---|
| 0-9 | Setup, map, dungeon generation, field of view, monsters and turns, combat, items, identification, growth (levels and skills), hunger and traps |
| 10 | Monster abilities: regeneration, life drain, blood drinking, theft, acid, splitting, packs |
| 11 | Themed zones: Crypts, Flooded Halls, Deep Warrens; themed rooms and shallow water; loops with extra scaling |
| 12 | Polish and balance: help pages, look, message history, title screen, time played, recorded runs (`--replay`, `--analyze`), item hints, vampire tuning, growing loop scaling and the deep ramp |
| Post-v1 | Auto pickup toggle (`@`), gear comparison, fixes from the Codex v1 review |
| Extras | Bot, auto-explore (`x`), stair travel (`>`), watchable bot (`--bot`, `B`), headless balance runs (`--simulate N`); monsters scale with depth; 31 monster kinds; food bar; bot memory with Escape and Fetch |

## Balance, as of the last measurement

Measured with the bot, which plays like a careful beginner. Target set by the user: the bot should die around depth 15-20.

| Measure | Value |
|---|---|
| Median final depth | 21 (200 fresh seeds, 1001-1200) |
| Average final depth | 21.7 |
| Deaths | 196 of 200 (4 hit the turn limit); 11 runs past depth 36, best 38 |
| Top killers | wraiths (14%), vampires (13%), demons (11%) |

The tuning knobs are constants at the top of `src/monster.rs` (scaling, loop scaling, home weight, alertness, monster count), `src/zone.rs` (zone floors, homes, item weights, themes), `src/themed.rs` and `src/abilities.rs` (acid, jelly cap, drain floor). Food supply is in `src/item.rs`.

## How we work

- Claude builds each milestone; the user plays and reviews.
- After each milestone: run a Codex review (`codex review --commit <sha>`), verify each finding, fix real ones with tests, then `codex review --uncommitted` on the fixes.
- Balance changes: measure on seeds 1-100, then confirm on fresh seeds 1001-1200 before keeping them. Results on tuning seeds alone have been misleading more than once.
- Stage files by name (no `git add -A`), check commit emails use the GitHub noreply address, scan the diff for anything sensitive, then push.
- Build release for simulations: `cargo build --release`, then `target/release/smallrogue --simulate 200`.
