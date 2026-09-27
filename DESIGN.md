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
| Identification | Potions, scrolls, wands, rings and weapon/armor enchantments start unknown |
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
- **Weapons and armor.** Always visibly a sword or chain mail, but enchantments are hidden until worn or identified.
- **Consumables.** Potions and scrolls get random appearances each run, such as "a murky potion". Using one identifies its kind.
- **Magic gear.** Wands and rings start unknown.
- **Identification.** By use, by scroll of identify, or gradually by carrying an item, which is faster with high Intellect.

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
| 3 | Field of view | You only see what is in sight. Explored areas remain dim |
| 4 | Monsters and turns | Monsters wander, spot you and chase you. Speed differences work |
| 5 | Combat and UI | Fight and die. Sidebar, message log, death screen |
| 6 | Items and inventory | Pick up, drop, equip weapons and armor, drink potions, read scrolls |
| 7 | Identification | Unknown potions, scrolls, wands, rings and enchantments |
| 8 | Growth | XP levels with health and attributes. Skills that rise through use |
| 9 | Hunger and traps | Food clock and hidden traps |
| 10 | Zones | Three themed zones, themed rooms, varying floor size, cycling with scaling |
| 11 | Polish and balance | High scores, help screen, tuning so runs last 10–20 minutes |

## Later, after v1

- Save on quit
- More backgrounds: rogue, mage (brings magic and spells), ranger
- Species
- More zones, monsters, items and themed rooms
- A signature mechanic, once play shows what is fun
- A tile renderer
