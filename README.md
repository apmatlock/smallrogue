# smallrogue

A grim, fast, endless dungeon crawler for the terminal, written in Rust. Descend through randomly generated rooms and corridors, fight what lives there, and use what you find. There is no bottom: how deep you get is your score.

The game is an early playable prototype. Dungeon generation, field of view, monsters, combat, death, items with an inventory, item identification, character growth, monsters that scale with depth, hunger and traps all work. Monster abilities, themed zones and high scores are still to come.

## Build and run

You need a Rust toolchain with Cargo and support for the Rust 2024 edition, plus an interactive terminal. The only direct dependency is [crossterm](https://crates.io/crates/crossterm).

```sh
cargo run
```

For an optimized build:

```sh
cargo run --release
```

Use a terminal at least 50 columns wide and 16 rows tall. A larger window shows more of the dungeon, plus key hints in the sidebar. The view scrolls to follow you when the floor is larger than the screen.

### Replay a dungeon

Each run has a seed, shown in the sidebar and on the death screen. Supply it on the command line to get the same dungeon again:

```sh
cargo run -- --seed 1234
cargo run --release -- --seed 1234
```

Each floor's layout, monsters and items come from the run seed and the floor's depth, so a floor is the same no matter how you got there. Seeds may produce different dungeons after the generator changes.

### Let the bot play

A built-in bot can play for you. It dives: once it has seen the stairs it heads down, exploring only until it finds them. Along the way it fights what hunts it, swaps in better gear, and experiments with unknown potions and scrolls, using only what a player could see and know.

```sh
cargo run -- --bot
```

While it plays, `+` and `-` change its speed, space pauses it, and Escape or `B` hands control back to you. Press `B` during a normal game to let it take over from where you are.

### Balance runs

The bot can also play many games without drawing anything and report how they went:

```sh
cargo run --release -- --simulate 200
```

This plays seeds 1 to 200 and prints the average, median and best depth reached, turns survived, a chart of depths, and the most common causes of death. It also saves one row per run to `target/sim.csv`. Use `--seed N` to start from a different seed and `--csv FILE` to save elsewhere. Use the `--release` build: it runs many times faster.

## Controls

| Key | Action |
| --- | --- |
| Arrow keys or `h`, `j`, `k`, `l` | Move left, down, up, right |
| `y`, `u`, `b`, `n` | Move diagonally: up-left, up-right, down-left, down-right |
| Move into a monster | Attack it |
| Move into a closed door | Open it |
| `.` | Wait one turn |
| `>` | Descend while standing on stairs; otherwise walk to stairs you have seen |
| `x` | Explore: walk to unexplored areas and pick up items until something happens |
| `c` | Close an adjacent open door; choose a direction if several are nearby |
| `g` or `,` | Pick up an item (walking over an item also picks it up) |
| `i` | Open your pack; press an item's letter for details and actions |
| `C` | Character sheet: level, experience, attributes, combat numbers and skills |
| `e` | Equip or remove a weapon, armor or ring |
| `d` | Drop an item |
| `q` | Drink a potion |
| `r` | Read a scroll |
| `E` | Eat |
| `?` | Show all keys |
| `B` | Let the bot play; `B` or Escape takes control back |
| `Q` | Quit, after confirming with `y`. Quitting ends the run. |

In lists that don't fit on screen, press space or `>` for the next page and `<` for the previous one. Escape closes any list.

Exploring and walking to the stairs stop as soon as a monster comes into view, you get hurt, or you pick something up. Any key stops them early.

Moving, attacking, waiting, opening or closing a door, descending, and using an item each take a turn. Failed actions, such as walking into a wall, do not.

## Reading the dungeon

| Symbol | Meaning |
| --- | --- |
| `@` | You |
| `#` | Wall |
| `.` | Floor |
| `+` | Closed door |
| `'` | Open door |
| `>` | Stairs down |
| `r` `j` `g` `z` `k` `:` `B` `a` | Early: rat, jackal, goblin, zombie, kobold, newt, giant bat, giant ant |
| `o` `h` `Z` `G` `s` `O` | Middle: orc, hobgoblin, skeleton, ghoul, giant spider, ogre |
| `T` `8` `W` `V` `D` `&` | Deep: troll, stone golem, wraith, vampire, dragon, demon |
| `)` | Weapon |
| `[` | Armor |
| `!` | Potion |
| `?` | Scroll |
| `=` | Ring |
| `%` | Food |
| `^` | A trap you know about |

Walls and closed doors block sight. What you can see is drawn in full color. Places you have explored stay on screen in cold, dim colors, along with the items lying there. Monsters are only shown while in sight.

The sidebar shows your health, attributes, depth, turn, seed and equipment, then every monster in view with a health bar and whether it is asleep, wandering or hunting. The log at the bottom reports what happens, with damage you take in red and kills in green.

## How it plays

**Monsters** sleep, wander, or hunt you. A monster can see you exactly when you can see it. Hunters that lose sight of you go to where they last saw you. Jackals are fast and zombies are slow. Rats and jackals can't open doors, so closing one can save you.

**The dungeon gets harder the deeper you go.** Twenty kinds of monster arrive at set depths, from rats and newts on the first floor to dragons at 22 and demons at 25, and are most common for a few floors after they first appear. Every monster also grows stronger with depth: its health compounds by 6% per floor, and it gains damage every 2 floors, accuracy every 3 and dodge every 4. Deeper floors hold more monsters, and fewer of them are asleep.

**Combat** rolls to hit by comparing the attacker's accuracy with the defender's dodge, then rolls damage and subtracts armor. A hit always does at least 1 damage. You slowly regain health over time. When you die, a death screen shows what killed you, how deep you got, and the seed.

**Your character** is a fighter with Strength, Agility and Intellect. Strength adds damage, and Agility sets accuracy and dodge. You start with a sword, leather armor and a potion of healing.

**Growth.** Kills give experience. Each new level adds 4 maximum health and raises an attribute, cycling strength, agility, strength, agility, intellect. Skills improve separately, simply by doing things: attacking trains **melee** (accuracy, then damage), being attacked trains **dodge**, being hit while armored trains **armor** (protection, and less penalty from heavy armor), and slipping past sleeping monsters trains **stealth** (they are less likely to wake). Early skill levels come quickly.

**Items** are scattered on every floor.

- Weapons: dagger, sword, mace and battle axe. Lighter weapons hit more often; heavier ones hit harder.
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
| `main`, `cli` | Startup, command-line options, game loop, and menus |
| `bot`, `sim` | The bot player, auto-explore and travel; headless balance runs |
| `game` | Game state, player actions, turns, and the message log |
| `player` | The player's stats, equipment and pack |
| `combat` | Hit chance and damage |
| `skills` | Skills that improve by use, experience and levels |
| `trap` | Hidden traps: placing, noticing and springing them |
| `item`, `inventory` | Item definitions and spawning; picking up, equipping and using items |
| `lore` | Per-run item appearances, what the player has identified, and item names |
| `monster`, `ai`, `path` | Monster definitions, spawning, behavior, and pathfinding |
| `dungeon` | Seeded room, corridor, door, and stair generation |
| `map`, `grid`, `geom` | Tiles, grid storage, and geometry |
| `fov` | Symmetric shadowcasting for field of view |
| `rng` | Seedable random number generation |
| `frame`, `ui`, `menu` | Screen cells, map viewport, sidebar, log, pop-up boxes, and death screen |
| `input`, `term` | Keyboard commands and terminal rendering |
| `text` | Small English helpers |

The terminal renderer draws only changed cells, and the game waits for input without using any CPU.

## Roadmap

The intended game is a grim, fast, endless fantasy crawl with short runs and depth reached as the score. Next milestones include:

- Monster abilities: regenerating trolls, draining wraiths, thieves, gear-wreckers, splitting jellies and packs
- Distinct themed dungeon zones
- High scores, help, and balance

See [DESIGN.md](DESIGN.md) for the design decisions and full milestone plan.

## Credits

This README was originally written by [Codex](https://openai.com/codex/), OpenAI's coding agent, and updated by Claude, Anthropic's AI assistant.
