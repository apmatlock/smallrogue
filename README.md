# smallrogue

A grim, fast, endless dungeon crawler for the terminal, written in Rust. Descend through randomly generated rooms and corridors, fight what lives there, and use what you find. There is no bottom: how deep you get is your score.

The dungeon passes through three themed zones, the Crypts, the Flooded Halls and the Deep Warrens, then starts over, deadlier each time. Thirty-one kinds of monster, many with special powers, items to identify, character growth, hunger, traps and a high score list are all in. A built-in bot can play for you, and every run you play is recorded so it can be replayed or analyzed.

## Build and run

You need a Rust toolchain with Cargo and support for the Rust 2024 edition, plus an interactive terminal. The only direct dependency is [crossterm](https://crates.io/crates/crossterm).

```sh
cargo run --release
```

This opens the title screen: `n` starts a new game, `s` shows the high scores, `?` shows help and `q` quits. After each run you come back to the title.

Use a terminal at least 50 columns wide and 16 rows tall. A larger window shows more of the dungeon, plus key hints in the sidebar. The view scrolls to follow you when the floor is larger than the screen.

### Replay a dungeon

Each run has a seed, shown in the sidebar and on the death screen. Supply it on the command line to get the same dungeon again (this skips the title screen):

```sh
cargo run --release -- --seed 1234
```

Each floor's layout, monsters and items come from the run seed and the floor's depth, so a floor is the same no matter how you got there. Seeds may produce different dungeons after the generator changes.

### Recordings

Every run you play is recorded to `~/.local/share/smallrogue/recordings` (or under `$XDG_DATA_HOME`): the seed, then each action with the time it happened and who chose it (you, auto-explore or the bot), plus a line for each new floor. A recording is a small text file, written as you play, so nothing is lost if the game is closed mid-run. `--no-record` turns it off.

A seed and its actions decide a run, so a recording plays back exactly:

```sh
cargo run --release -- --replay ~/.local/share/smallrogue/recordings/2026-09-30-140509-seed1234.rec
```

Space pauses, `+` and `-` change the speed, and any other key stops.

To see what your recordings say:

```sh
cargo run --release -- --analyze
```

This reports each run (depth, time, turns, how much you played by hand, how it ended); your pace, as time and turns per floor in each zone and seconds per 100 turns; and how often your moves match what the bot would have done in the same spot, with the most common differences. Give a file or folder after `--analyze` to read somewhere else.

A recording replays exactly only as long as the game's rules stay the same. Runs recorded before a rule change are marked in the report and left out of the comparison with the bot; their floor times still count.

### Let the bot play

A built-in bot can play for you. It dives: once it has seen the stairs it heads down, first going back for anything useful it has seen nearby. It fights what hunts it, runs for the stairs when hurt, swaps in better gear, and experiments with unknown potions and scrolls, using only what a player could see and know.

```sh
cargo run --release -- --bot
```

While it plays, `+` and `-` change its speed, space pauses it, and Escape or `B` hands control back to you. Press `B` during a normal game to let it take over from where you are. Runs the bot played any part of don't go on the high score list.

### Balance runs

The bot can also play many games without drawing anything and report how they went:

```sh
cargo run --release -- --simulate 200
```

This plays seeds 1 to 200, printing a line as each run finishes, then the average, median and best depth reached, turns survived, a chart of depths, and the most common causes of death. It also saves one row per run to `target/sim.csv`. Use `--seed N` to start from a different seed and `--csv FILE` to save elsewhere. Use the `--release` build: it runs many times faster.

## Controls

| Key | Action |
| --- | --- |
| Arrow keys or `h`, `j`, `k`, `l` | Move left, down, up, right |
| `y`, `u`, `b`, `n` | Move diagonally: up-left, up-right, down-left, down-right |
| Move into a monster | Attack it |
| Move into a closed door | Open it |
| `.` | Wait one turn (rest) |
| `>` | Descend while standing on stairs; otherwise walk to stairs you have seen |
| `x` | Explore: walk to unexplored areas and pick up items until something happens (with auto pickup off, it leaves items alone) |
| `R` | Rest until healed, unless something happens first |
| `c` | Close an adjacent open door; choose a direction if several are nearby |
| `g` or `,` | Pick up an item (walking over an item also picks it up) |
| `@` | Auto pickup on or off; the choice is saved for later runs |
| `i` | Open your pack; press an item's letter for details and actions |
| `C` | Character sheet: level, experience, attributes, combat numbers and skills |
| `e` | Equip or remove a weapon, armor or ring; weapons and armor show how they would change your attack or defense (a `?` means an unknown enchantment, counted as +0) |
| `d` | Drop an item |
| `q` | Drink a potion |
| `r` | Read a scroll |
| `E` | Eat |
| `L` or `;` | Look: every monster, item and known trap in view, nearest first |
| `m` | Message history, newest first |
| `?` | Help: keys, symbols and how to play |
| `B` | Let the bot play; `B` or Escape takes control back |
| `Q` | Give up, after confirming with `y`. The run ends and goes on the high scores. |

In lists that don't fit on screen, press space or `>` for the next page and `<` for the previous one. Escape closes any list.

Exploring, walking to the stairs and resting stop as soon as a monster comes into view, you get hurt, you pick something up or you get hungrier. Any key stops them early. Resting won't start with a monster awake in view, or when you're too hungry to heal.

Moving, attacking, waiting, opening or closing a door, descending, and using an item each take a turn. Failed actions, such as walking into a wall, do not.

## Reading the dungeon

| Symbol | Meaning |
| --- | --- |
| `@` | You |
| `#` | Wall |
| `.` | Floor |
| `~` | Shallow water (walk through it like floor) |
| `+` | Closed door |
| `'` | Open door |
| `>` | Stairs down |
| `r` `j` `k` `:` `B` `g` `z` `M` `R` `Z` `a` `d` `y` `o` | Early (depths 1-5): rat, jackal, kobold, newt, giant bat, goblin, zombie, monkey, redcap, skeleton, giant ant, draugr, harpy, orc |
| `A` `h` `s` `G` `O` `Y` `J` `q` `X` | Middle (6-12): acid mound, hobgoblin, giant spider, ghoul, ogre, owlbear, pink jelly, gargoyle, bulette |
| `T` `8` `U` `W` `V` `P` `D` `&` | Deep (13 and on): troll, stone golem, oni, wraith, vampire, frost giant, dragon, demon |
| `)` | Weapon |
| `[` | Armor |
| `!` | Potion |
| `?` | Scroll |
| `=` | Ring |
| `%` | Food |
| `^` | A trap you know about |

Walls and closed doors block sight. What you can see is drawn in full color. Places you have explored stay on screen in cold, dim colors, along with the items lying there. Monsters are only shown while in sight. Look (`L`) names everything in view and says what makes each monster dangerous.

The sidebar shows the zone you're in, your health, food, attributes, depth, turn, seed and equipment, then every monster in view with a health bar and whether it is asleep, wandering or hunting. The log at the bottom reports what happens, with damage you take in red and kills in green; `m` scrolls back through it.

## How it plays

**Zones.** Every six floors the dungeon changes. The **Crypts** (floors 1-6) are tight: small rooms, doors everywhere, few ways round, and the dead. The **Flooded Halls** (7-12) are big open halls with many ways round, where things crawl, swarm and ooze. The **Deep Warrens** (13-18) sprawl across a larger map of small rooms and long corridors, home to goblin tribes, beasts and burrowers. Each zone has its own colors, its own monsters (which are much more common there), and its own leanings in what you find: scrolls in the crypts, potions in the halls, gear and food in the warrens. After the Warrens the cycle starts over at the Crypts, and every monster in the second pass and beyond is stronger than its depth alone would make it.

**Themed rooms.** About half of all floors have one special room: a crypt tomb where undead sleep over treasure, a flooded cistern where oozes lurk in the water beside an item on dry ground, or a warrens den of a sleeping pack on its loot, or a larder of stolen food with a guard.

**Monsters** sleep, wander, or hunt you. A monster can see you exactly when you can see it. Hunters that lose sight of you go to where they last saw you. Jackals and bats are fast; zombies and golems are slow. Rats and jackals can't open doors, so closing one can save you.

**The dungeon gets harder the deeper you go.** Monsters arrive at set depths, from rats and newts on the first floor to dragons at 22 and demons at 25, and are most common for a few floors after they first appear. Every monster also grows stronger with depth: its health compounds by 6% per floor, and it gains damage every 2 floors, accuracy every 3 and dodge every 4. Past depth 24 each floor adds the strength of an extra floor on top. Deeper floors hold more monsters, and fewer of them are asleep.

**Some monsters have special powers.** Trolls regenerate, so finish them quickly. A wraith's touch permanently lowers your maximum health. Vampires regenerate and heal themselves by drinking your blood. Monkeys snatch an item from your pack and run; they are faster than you but can't open doors, and they drop the item when killed. Acid mounds can eat away at your armor's enchantment, though never below -1. Pink jellies split in two whenever you hit them without killing them. Jackals hunt in packs of two or three, and orcs sometimes travel in pairs. If several attacks in a row leave a monster's health bar no lower, the log says your blows barely scratch it: it heals as fast as you hurt it, or you can't hurt it at all, and it may be time to leave.

**Combat** rolls to hit by comparing the attacker's accuracy with the defender's dodge, then rolls damage and subtracts armor. A hit always does at least 1 damage. You slowly regain health over time.

**Your character** is a fighter with Strength, Agility and Intellect. Strength adds damage, and Agility sets accuracy and dodge. You start with a sword, leather armor and a potion of healing.

**Growth.** Kills give experience. Each new level adds 4 maximum health and raises an attribute, cycling strength, agility, strength, agility, intellect. Skills improve separately, simply by doing things: attacking trains **melee** (accuracy, then damage), being attacked trains **dodge**, being hit while armored trains **armor** (protection, and less penalty from heavy armor), and slipping past sleeping monsters trains **stealth** (they are less likely to wake). Early skill levels come quickly.

**Items** are scattered on every floor.

- Weapons: dagger, sword, mace and battle axe. Lighter weapons hit more often; heavier ones hit harder.
- Artifacts, drawn in gold, are rare: about one run in three holds each, at most once. **Sunsteel**, somewhere on floors 13-20, does double damage to the undead (zombies, skeletons, draugr, ghouls, wraiths, vampires) and trolls, and its burns stop them healing for 5 turns. **Hellbane**, somewhere on floors 22-28, does triple damage to demons and oni. Their names show at once; only the enchantment needs finding out. Carry one and swap it in when its prey turns up.
- Armor: leather, chain mail and plate. Heavier armor protects more but makes you easier to hit.
- Rings: regeneration, accuracy, protection and awareness, which widens your sight. You can wear two.
- Enchantments make items stronger or weaker: weapons and armor roll from -3 to +2, rings from -3 to +3.
- Potions: healing, strength, life, and decay, which hurts.
- Scrolls: teleportation, magic mapping, enchanting, identify, and aggravate monsters, which wakes the whole floor.

**Identification.** Potions, scrolls and rings look different in every run: a murky potion might heal you in one game and burn you in the next. Drinking or reading one teaches you that kind for the rest of the run, and putting on a ring tells you what kind it is. Enchantments stay hidden until you wear an item for about 300 turns (less with more Intellect) or read a scroll of identify on it.

**Hunger.** Every turn uses a little food. You start with 1,800 and a ration of food. Below 300 you are hungry, below 150 you are weak (no healing, less accurate), and at 0 you start starving and lose health until you eat. Rations and strips of jerky turn up on many floors; eat with `E`. The game won't let you eat when most of the food would go to waste. A food bar under your health bar shows how much you have left and names the stage once you're hungry.

**Traps** hide on the floor: darts that hurt, alarms that wake everything nearby, teleport traps, and trapdoors that drop you a floor. Standing next to a hidden trap gives you a chance each turn to notice it, better with a ring of awareness, and magic mapping shows every trap. Known traps are drawn as `^`; stepping onto one takes a second move in the same direction to confirm. Auto-explore and the bot walk around known traps.

**Curses.** Gear with a negative enchantment is cursed. Once equipped, it can't be taken off for 50 turns per point below zero, and you find out the moment you put it on. Reading enchanting on a cursed item breaks the curse.

Your pack holds 26 items. Potions and scrolls of the same kind stack, and each item keeps its letter while you carry it.

**The end of a run.** When you die or give up, the death screen shows how the run ended, your character level, the seed and the time played, then the run in numbers: monsters slain and which kind most often, the toughest foe you killed, accuracy, damage dealt and taken, stairs and trapdoors, and what you picked up and used. Below that is the high score list with your run marked.

**High scores** keep the ten best runs, ranked by depth reached, then fewer turns. They are saved in `~/.local/share/smallrogue/scores.tsv` (or under `$XDG_DATA_HOME`), with each run's level, turns, kills, time played, how it ended and the date. Giving up counts as an ending, so it can't keep a bad run off the list. Time played only counts while you're playing: any gap of more than a minute between moves counts as a minute.

## Development

Run the tests and code checks with:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The code uses plain structs and grids, with game rules kept separate from terminal input and output:

| Module | Responsibility |
| --- | --- |
| `main`, `cli` | Startup, command-line options, title screen, game loop, and menus |
| `bot`, `sim` | The bot player, auto-explore and travel; headless balance runs |
| `record`, `analyze` | Recording runs and playing them back; reports on recordings |
| `game` | Game state, player actions, turns, and the message log |
| `player` | The player's stats, equipment and pack |
| `combat` | Hit chance and damage |
| `skills` | Skills that improve by use, experience and levels |
| `stats`, `scores` | Counts kept over a run; the saved high score list |
| `trap` | Hidden traps: placing, noticing and springing them |
| `item`, `inventory` | Item definitions and spawning; picking up, equipping and using items |
| `lore` | Per-run item appearances, what the player has identified, and item names |
| `monster`, `ai`, `path` | Monster definitions, spawning, behavior, and pathfinding |
| `abilities` | Special monster powers: stealing, draining, corroding, splitting |
| `zone`, `themed` | The zones and the loop through them; themed rooms |
| `dungeon` | Seeded room, corridor, door, and stair generation |
| `map`, `grid`, `geom` | Tiles, grid storage, and geometry |
| `fov` | Symmetric shadowcasting for field of view |
| `rng` | Seedable random number generation |
| `frame`, `ui`, `menu` | Screen cells, map viewport, sidebar, log, pop-up boxes, and the title and death screens |
| `input`, `term` | Keyboard commands and terminal rendering |
| `text` | Small English helpers |

The terminal renderer draws only changed cells, and the game waits for input without using any CPU.

## Roadmap

The first version is nearly complete: what's left is settling run length from recorded play. After that:

- More monster abilities: ranged attackers, and status effects like poison and paralysis
- More zones, monsters, items and themed rooms
- Save on quit, and more character backgrounds

See [DESIGN.md](DESIGN.md) for the design decisions and full milestone plan.

## Credits

This README was originally written by [Codex](https://openai.com/codex/), OpenAI's coding agent, and updated by Claude, Anthropic's AI assistant.
