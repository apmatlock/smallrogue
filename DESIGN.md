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
- **Levels.** Kills give XP. Each level raises max HP and grants a small attribute increase.
- **Skills (learn by doing).** A small set of broad skills trained by use: Melee, Armor, Dodge, Stealth, Throwing. Training is fast, so a skill improves noticeably within one run.
- **Hunger.** A food counter ticks down each turn. Hungry gives a warning, Weak gives penalties, Starving deals damage. Food is common enough that careful play never starves.

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
| 8 | Growth | XP levels with health and attributes. Skills that rise through use |
| 9 | Hunger and traps | Food clock and hidden traps |
| 10 | Zones | Three themed zones, themed rooms, varying floor size, cycling with scaling |
| 11 | Polish and balance | High scores, help screen, tuning so runs last 10–20 minutes |

### Added along the way: bot and auto modes

Built between milestones 7 and 8, at the user's request.

- **Auto-explore (`x`)** walks to the nearest unexplored area or item, stopping when a monster appears, the player is hurt, or an item is picked up.
- **Travel (`>`)** walks to seen stairs when not standing on them.
- **Watchable bot (`--bot` or `B`)** plays on screen with adjustable speed, pause, and a key to take over.
- **Headless balance runs (`--simulate N`)** play N seeded games and print depth, turns and causes of death, plus a CSV.
- The bot plays like a careful beginner and only uses information a player could have. It prioritizes descending: once the stairs are known and reachable it heads down, and explores only while it hasn't found them.
- The bot chases only monsters that are hunting it and leaves sleeping ones alone. Reacting to monsters that pop in and out of view made it step back and forth forever.
- Balance runs report runs that stall (no new floor for 5,000 turns) separately, to catch bot bugs.

## Later, after v1

- Save on quit
- More backgrounds: rogue, mage (brings magic, spells and wands), ranger
- Species
- More zones, monsters, items and themed rooms
- A signature mechanic, once play shows what is fun
- A tile renderer
