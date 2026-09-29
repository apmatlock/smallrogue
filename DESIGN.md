# smallrogue — Design and Build Plan

A grim, fast, endless fantasy dungeon crawl for the terminal, written in Rust.
Runs last 10–20 minutes. You die eventually; how deep you got is your score.

## Design pillars

1. **Short, sharp runs.** Every floor should take a minute or two. Difficulty ramps quickly.
2. **Readable.** Clean Brogue-style interface. The player should always understand what happened and why.
3. **Grim atmosphere.** Serious tone, real danger, NetHack's classic ASCII feel without its complexity.
4. **Light on the machine.** Runs well on a 2-core Celeron with little free RAM. Few dependencies, fast rebuilds.
5. **Built to grow.** Start small, but make adding zones, monsters, items and backgrounds cheap.

## Decisions so far

| Area | Decision |
|---|---|
| Setting | Classic fantasy dungeon, serious and grim, no story |
| Goal | Descend as deep as possible. Depth reached is the score |
| Depth | Endless |
| Dungeon | Classic rooms and corridors |
| Themes | Zones cover bands of floors, with special themed rooms inside them |
| Beyond last zone | Zones cycle in order with scaled-up monster variants each loop |
| Floor size | Varies by zone; the view scrolls when a floor is larger than the screen |
| Run length | 10–20 minutes for a decent player |
| Content scope | Start small, systems designed so content is easy to add |
| Saving | Not in v1. Keep all game state in one struct so save-on-quit is easy to add later |
| Meta progression | Pure permadeath. Only a high-score list carries over |
| Starting character | One background: the Fighter |
| Species | Not in v1 |
| Attributes | Strength, Agility, Intellect |
| Growth | Character levels from XP raise health and attributes. Skills rise separately through use |
| Combat | Roll to hit, then roll damage reduced by armor |
| Hunger | Yes, light pressure — enough to stop endless resting, rarely fatal for careful players |
| Magic | Later, with a mage background. Until then Intellect helps identify items |
| Identification | Potions, scrolls, rings and weapon/armor/ring enchantments start unknown. Wands wait for magic |
| Rings | Two worn at once. Regeneration, accuracy, protection, awareness |
| Curses | Soft: negative gear can't be removed for 50 turns per point below zero. Enchanting breaks a curse |
| Terrain | Floor, wall, stairs down, doors, traps |
| Vision | Line-of-sight field of view, explored areas remembered in dim colors |
| Visuals | Colored ASCII. Drawing is kept separate from game logic so tiles could be added |
| Input | Keyboard only |
| Signature mechanic | None yet. Let it emerge from play |

## Systems in detail

### Character
- **Attributes.** Strength adds melee damage and carry capacity. Agility adds accuracy and dodge. Intellect speeds identification of unknown items, and later powers magic.
- **Skills (learn by doing).** Melee (trained by attacking: +1 accuracy per level, +1 damage per 2), Dodge (trained by being attacked: +1 dodge per level), Armor (trained by being hit while armored: +1 armor per 2 levels and 1 less heavy-armor dodge penalty per level), Stealth (trained when a sleeping monster that can see you stays asleep: -2% wake chance per level, from 25% down to 5%). Levels cost 10, 20, 30... points, up to level 10. Throwing waits until there is something to throw.
- **Levels.** Experience per kill: rat 2, jackal 3, goblin 6, zombie 8. Level thresholds are 10, 30, 60, 100, 150... Each level gives +4 max health and one attribute point, cycling strength, agility, strength, agility, intellect.
- **Hunger.** Food starts at 1,800 (max 2,100) and drops 1 per turn. Hungry at 300 is a warning; Weak at 150 stops healing and costs 2 accuracy; Starving at 0 loses 1 health every 5 turns. Rations restore 1,800 and jerky 600. The fighter starts with one ration. Each floor has a 25% chance of a ration and 35% of jerky. Eating is refused if more than half the food would be wasted. In 200 bot runs, 3 starved: careful play basically never does.
- **Traps.** Dart (damage), alarm (wakes monsters within 20 tiles), teleport, and trapdoor (drops a floor with a little damage). 1 plus depth/3 per floor, up to 6, never in the start room. Hidden traps within 1 tile (plus awareness) are noticed at 15% per turn (plus 10% per awareness point). Magic mapping reveals traps. Stepping onto a known trap needs a second, identical move. Only the player sets traps off.
- **Balance with hunger:** median final depth 16, median death depth 14, and runs outlasting the turn cap fell from 36% to 22%. The remaining survivors dive fast enough that floor food covers them; monster abilities are the next source of pressure.

### Combat
- Bump into a monster to attack.
- Hit chance compares the attacker's accuracy with the defender's dodge.
- Damage is a weapon roll plus strength bonus, minus the defender's armor, with a minimum of 1 on a hit.
- Every attack produces a clear log message such as "The ghoul hits you for 4."

### Dungeon
- **Generation.** Place non-overlapping rectangular rooms, connect them with corridors, add doors at room entrances, place stairs, monsters, items and traps.
- **Zones (v1 has three).** For example: Crypts (floors 1–4), Flooded Halls (5–8), Deep Warrens (9–12). Each zone sets floor size, wall and floor colors, monster pool, item weights and themed rooms.
- **Cycling.** After the last zone, the cycle restarts with a loop number. Each loop increases monster health, damage and accuracy.
- **Themed rooms.** Special rooms with their own layout and contents, such as an ossuary, a treasure vault or a guard post.
- **Seeds.** Every floor comes from a seeded random generator, so a run can be replayed exactly for debugging.

### Items
- **Weapons and armor.** Always visibly a sword or chain mail, but enchantments are hidden until worn long enough or identified. Enchantments roll from -3 to +2; negative rolls are rare (about 6% of gear).
- **Consumables.** Potions and scrolls get random appearances each run, such as "a murky potion". Using one identifies its kind.
- **Rings.** Up to two worn at once. Regeneration heals faster, accuracy adds to hit chance, protection adds armor, and awareness widens sight (and will reveal traps once they exist). A ring's enchantment sets its strength. Rings start unknown.
- **Wands.** Deferred until magic arrives with the mage background.
- **Soft curses.** Equipping negative gear locks it in place for 50 turns per point below zero (-1: 50, -2: 100, -3: 150). Reading enchanting on it breaks the curse at once.
- **Identification.** Drinking or reading an unknown kind identifies it for the rest of the run. A new scroll of identify reveals one chosen item. Equipped weapons, armor and rings reveal their enchantment after about 300 turns worn, faster with higher Intellect. Carrying alone does nothing.

### Monsters
- Defined in data tables: symbol, color, health, attack, defense, speed, zone, behavior.
- Simple behaviors first: wander until the player is seen, then chase and attack. Later: fleeing, ranged attackers, packs.
- A speed or energy system allows fast and slow monsters.

### Interface
Terminal screen split into: the map, a sidebar with health, hunger, attributes and depth, and a message log. Screens for inventory, character sheet, help, death summary and high scores.

## Architecture

- **One game state struct** holds the map, entities, player, RNG and message log. This makes save-on-quit a matter of serializing one value later.
- **Plain structs and vectors, no ECS library.** Easier to learn and fast to compile. Revisit only if entity handling gets painful.
- **Renderer boundary.** Game logic never calls crossterm. It produces what to draw, and a terminal module draws it. A tile renderer could replace that module later.
- **Content as Rust data tables.** Monsters, items and zones live in const arrays. This avoids a serialization library and keeps builds fast.
- **Dependencies.** crossterm for the terminal. Possibly one tiny RNG crate, or our own small generator. Nothing else without a clear reason.
- **Modules (planned):** `map`, `dungeon` (generation), `fov`, `entity`, `combat`, `ai`, `item`, `player`, `zone`, `ui`, `render`, `rng`.

## Milestones

Each milestone ends with something playable. Claude builds each one with clear, explained code. You play and review before the next begins.

| # | Milestone | Playable result |
|---|---|---|
| 0 | Setup | Rust installed, `@` moves around the screen ✅ |
| 1 | Map and rendering | Walk around a hand-made map with walls and colors, with a scrolling view ✅ |
| 2 | Dungeon generation | Every floor is a new random set of rooms, corridors and doors. Stairs take you deeper ✅ |
| 3 | Field of view | You only see what is in sight. Explored areas remain dim ✅ |
| 4 | Monsters and turns | Monsters wander, spot you and chase you. Speed differences work ✅ |
| 5 | Combat and UI | Fight and die. Sidebar, message log, death screen ✅ |
| 6 | Items and inventory | Pick up, drop, equip weapons and armor, drink potions, read scrolls ✅ |
| 7 | Identification | Unknown potions, scrolls and enchantments, rings, soft curses, scroll of identify ✅ |
| 8 | Growth | XP levels with health and attributes. Skills that rise through use ✅ |
| 9 | Hunger and traps | Food clock and hidden traps ✅ |
| 10 | Monster abilities | Monsters that change how you play: regenerating trolls, draining wraiths, a thief, a gear-wrecker, a splitter, packs ✅ |
| 11 | Zones | Three themed zones, themed rooms, varying floor size, cycling with scaling. Zones choose which monster abilities appear where |
| 12 | Polish and balance | High scores, help screen, tuning so runs last 10–20 minutes |

### Milestone 10 plan: monster abilities

Added to the plan after comparing our roster with the recurring cast of classic roguelikes (Rogue, NetHack, Angband, DCSS, Brogue and others). Our eight monsters are all classics, but they only differ in numbers and speed. The most memorable roguelike monsters change how you play instead of just hitting harder. In order:

1. **Upgrade existing monsters (small).** Trolls regenerate health each turn, so finishing a fight matters. Wraiths drain maximum health or experience on a hit.
2. **Three new behaviors (medium).**
   - A **thief**, such as a monkey or nymph, steals an item and flees. Killing it drops the item.
   - A **gear-wrecker**, such as a rust monster or acid creature, lowers armor enchantment on a hit. This plays off enchantments and curses.
   - A **splitter**, a jelly, spawns copies when hit, with a cap so floors can't overflow. Jellies conventionally use `j`, so either they or our jackal need a different letter.
3. **Packs (medium).** Jackals and orcs spawn in groups, which makes doorways and corridors matter.
4. **Later, each needing a new system.** Ranged attackers that keep their distance, such as centaurs or goblin archers, need ranged attacks. Paralysis, poison and petrification need a status-effect system.

**Built (steps 1 to 3):** trolls regenerate 3% of full health a turn; wraiths drain 1 maximum health per hit (never below 10); vampires regenerate and heal by the damage they deal; the monkey (`M`, depth 3, faster than the player, can't open doors) steals one unequipped item and flees, dropping it when killed; the acid mound (`A`, depth 6, slow) has a one-in-three chance per hit to take 1 point off worn armor's enchantment, never below -1, hidden on unidentified armor; the pink jelly (`J`, depth 10) splits when hit without dying, sharing its health, up to 10 per floor; jackals come in packs of 2-3 and orcs in groups of 1-2. The owlbear moved to depth 10 with slightly lower damage.
- **Tuning with abilities (200 bot runs each).** First pass: median final depth 12, with orc packs the top killer. Smaller packs and spreading out the depth 8-9 arrivals didn't move the median. A controlled test on the same 100 seeds found the systemic cause: switching acid off raised the median from 12 to 16, while switching theft off only reached 13. Permanent armor loss on every acid hit made every later fight deadlier. Final: median final depth 15, median death depth 13, 12 starvations. Step 4 (ranged attacks, status effects) is still to come.

Milestone 11's zones will then decide which abilities appear where: for example, drainers and ghouls in crypts, jellies and acid in flooded halls, packs and thieves in warrens.

### Moved ahead of milestones 9 and 10: monster scaling

Built right after milestone 8, at the user's request, so balance runs had a real difficulty curve to measure. Themed zones stay in milestone 11.

- **New monsters by depth:** orc (5), ghoul (9), troll (13), wraith (16, fast and evasive). Kinds that arrived in the last 6 floors are three times as common as older ones.
- **Stat scaling:** health compounds 6% per floor; +1 damage per 2 floors, +1 accuracy per 3, +1 dodge per 4; experience grows 8% of base per floor (not compounding, so the character can't simply outgrow the dungeon).
- **Alertness:** 50% of monsters start asleep on depth 1, 3% fewer per floor, down to 5%. Monsters per floor: 2 plus depth, up to 16.
- **Target:** the bot (a careful beginner) should die around depth 15-20. Tuned with 200-game runs: median final depth 15, median death depth 13, deaths spread from 3 to 19. About a third of runs still snowball past the turn cap because resting is free; hunger in milestone 9 is meant to close that gap, after which the numbers should be re-checked.
- All knobs are constants at the top of `src/monster.rs`.
- **Twelve more monsters** were added at the user's request, as stats and speed only (abilities come in milestone 10). Early: kobold (1), newt (1, slow), giant bat (2, double speed), giant ant (4, fast). Middle: hobgoblin (6), skeleton (6, well armored), giant spider (8, fast and accurate), ogre (10, slow, huge hits). Deep: stone golem (14, half speed, enormous health), vampire (18), dragon (22, can't open doors), demon (25). Base stats account for the compounding depth scaling, so each arrives a step above its neighbors rather than towering over them. Balance held: median final depth 16, median death depth 14, 9 starvations in 200 runs.
- **Eight folklore monsters** from the bestiary survey, also stats and speed only: redcap (3), harpy (5, fast flyer), draugr (7, armored), owlbear (9), gargoyle (11, slow and stone-hard), bulette (12), oni (15) and frost giant (19). That makes 28 kinds. The bulette was softened after it became the top killer. Balance with 28 kinds (200 bot runs): median final depth 19, median death depth 14, deaths spread across many monsters.

### Added along the way: bot and auto modes

Built between milestones 7 and 8, at the user's request.

- **Auto-explore (`x`)** walks to the nearest unexplored area or item, stopping when a monster appears, the player is hurt, or an item is picked up.
- **Travel (`>`)** walks to seen stairs when not standing on them.
- **Watchable bot (`--bot` or `B`)** plays on screen with adjustable speed, pause, and a key to take over.
- **Headless balance runs (`--simulate N`)** play N seeded games and print depth, turns and causes of death, plus a CSV.
- The bot plays like a careful beginner and only uses information a player could have. It prioritizes descending: once the stairs are known and reachable it heads down, and explores only while it hasn't found them.
- The bot chases only monsters that are hunting it and leaves sleeping ones alone. Reacting to monsters that pop in and out of view made it step back and forth forever.
- Balance runs report runs that stall (no new floor for 5,000 turns) separately, to catch bot bugs.

### Bot improvements, first batch

After asking Codex how to make the bot more successful, four small changes that need no memory between turns were built and measured one at a time on the same 100 seeds, then confirmed on 200 fresh seeds (1001-1200).

- **Kept: descend when threatened on the stairs.** Taking the stairs ends the turn on a new floor, so nothing gets a parting blow. Small gain in average depth.
- **Rejected: stop resting near sleeping monsters.** Codex expected fewer wake-ups; the median dropped from 15 to 14, so the bot still rests.
- **Kept: unknown scrolls only with nothing in sight,** so an aggravate scroll wakes the floor at a safer moment, and as a last-ditch gamble when cornered at 20% health or less. Median 15 to 16.
- **Kept: danger-aware emergencies.** The bot adds up the worst hits of adjacent monsters and fast hunters two tiles away (known from each monster's kind and the depth). It teleports when surrounded or when a healing potion wouldn't cover a worst-case turn, drinks healing otherwise, and saves potions of life for emergencies or a quiet moment at half health.
- **Fresh-seed result** (after a Codex fix so fast monsters count for two attacks and healing is capped at full health): 85 runs deeper, 62 shallower, 53 unchanged; 178 deaths down to 172; average depth 22.8 to 23.1; median unchanged at 14. The larger gains Codex suggested (fighting from corridors, committed escapes, closing doors on pursuers, supply detours) need the bot to remember a plan between turns.

### Bot memory: Escape

The bot now keeps a tiny memory between turns: at most one plan (a purpose, a goal tile and a turn budget), wiped on every new floor. Plans stick even as monsters move in and out of view, which is what caused earlier pacing loops, and always end on arrival, when the budget runs out, or when no route is left. Emergencies still come first.

The first purpose is **Escape**: when at half health or less and nothing hunting it is faster than the player, the bot heads for known stairs and takes them. A same-speed pursuer spends its actions following, not attacking.

- On 200 fresh seeds, escaping when hurt was roughly neutral (31 runs deeper, 29 shallower; median 14 to 15; average 23.1 to 23.3).
- A second trigger, walking away from slow monsters, looked good on the 100 tuning seeds but lost clearly on fresh seeds (47 deeper, 78 shallower, 11 more deaths), so it was removed. Skipping those fights costs experience the bot needs later.
- No stalls in any run. The same memory is ready for the Hold (fight from corridors) and Fetch (short supply detours) plans.

## Later, after v1

- Save on quit
- More backgrounds: rogue, mage (brings magic, spells and wands), ranger
- Species
- More zones, monsters, items and themed rooms
- A signature mechanic, once play shows what is fun
- A tile renderer
