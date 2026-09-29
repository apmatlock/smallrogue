# Session log

Where the project stands, so work can pick up where it left off. The design decisions and milestone plan live in [DESIGN.md](DESIGN.md); this file is the running status.

## Where we left off — 2026-09-29

Last commit: `f309b1c` "Bot memory with an Escape plan". Everything is committed and pushed to `main`. 155 tests pass; clippy and rustfmt are clean.

### Next step, to choose from

1. **Bot memory: Hold.** Fight from corridors instead of charging into groups. Codex rated this above Escape for impact. Reuses the new `BotMemory`: add a `Purpose::Hold`, pick a nearby corridor tile when a group is hunting, walk there, then wait and fight. Measure on seeds 1-100, confirm on fresh seeds 1001-1200.
2. **Bot memory: Fetch.** Short, budgeted detours for food, healing or clear gear upgrades before diving.
3. **Milestone 11: themed zones.** Crypts, flooded halls and warrens with their own floor sizes, colors, themed rooms and monster mixes, cycling with scaling. Zones also decide which monster abilities appear where.
4. **Milestone 12: polish and balance.** High scores, help, final tuning.

Smaller open items:
- Monster abilities step 4 (ranged attackers; status effects like poison and paralysis) needs new systems.
- Monster ideas from the bestiary survey that need abilities: hydra, minotaur, basilisk, cockatrice, mimic, gelatinous cube, will-o'-the-wisp, wendigo, banshee.
- Design question left open: should magic mapping reveal items as well as terrain and traps? It currently does, both on screen and for the bot.

## What's built

| Milestone | Status |
|---|---|
| 0-9 | Setup, map, dungeon generation, field of view, monsters and turns, combat, items, identification, growth (levels and skills), hunger and traps |
| 10 | Monster abilities: regeneration, life drain, blood drinking, theft, acid, splitting, packs |
| Extras | Bot, auto-explore (`x`), stair travel (`>`), watchable bot (`--bot`, `B`), headless balance runs (`--simulate N`); monsters scale with depth; 31 monster kinds; food bar; bot memory with Escape |

## Balance, as of the last measurement

Measured with the bot, which plays like a careful beginner. Target set by the user: the bot should die around depth 15-20.

| Measure (200 fresh seeds, 1001-1200) | Value |
|---|---|
| Median final depth | 14-15 |
| Average final depth | 23.3 |
| Deaths | 173 of 200 |
| Top killers | trolls, bulettes, draugr, orcs, giant spiders, pink jellies, wraiths |

The tuning knobs are constants at the top of `src/monster.rs` (scaling, alertness, monster count) and `src/abilities.rs` (acid, jelly cap, drain floor). Food supply is in `src/item.rs`.

## How we work

- Claude builds each milestone; the user plays and reviews.
- After each milestone: run a Codex review (`codex review --commit <sha>`), verify each finding, fix real ones with tests, then `codex review --uncommitted` on the fixes.
- Balance changes: measure on seeds 1-100, then confirm on fresh seeds 1001-1200 before keeping them. Results on tuning seeds alone have been misleading more than once.
- Stage files by name (no `git add -A`), check commit emails use the GitHub noreply address, scan the diff for anything sensitive, then push.
- Build release for simulations: `cargo build --release`, then `target/release/smallrogue --simulate 200`.
