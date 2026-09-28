# smallrogue

A grim, fast, endless dungeon crawler for the terminal, written in Rust. Descend through randomly generated rooms and corridors, fight what lives there, and use what you find. There is no bottom: how deep you get is your score.

The game is an early playable prototype. Dungeon generation, field of view, monsters, combat, death, and items with an inventory all work. Item identification, character growth, hunger, traps, themed zones and high scores are still to come.

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

## Controls

| Key | Action |
| --- | --- |
| Arrow keys or `h`, `j`, `k`, `l` | Move left, down, up, right |
| `y`, `u`, `b`, `n` | Move diagonally: up-left, up-right, down-left, down-right |
| Move into a monster | Attack it |
| Move into a closed door | Open it |
| `.` | Wait one turn |
| `>` | Descend while standing on stairs |
| `c` | Close an adjacent open door; choose a direction if several are nearby |
| `g` or `,` | Pick up an item (walking over an item also picks it up) |
| `i` | Open your pack; press an item's letter for details and actions |
| `e` | Equip or remove a weapon or armor |
| `d` | Drop an item |
| `q` | Drink a potion |
| `r` | Read a scroll |
| `?` | Show all keys |
| `Q` | Quit, after confirming with `y`. Quitting ends the run. |

In lists that don't fit on screen, press space or `>` for the next page and `<` for the previous one. Escape closes any list.

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
| `r` `j` `g` `z` | Rat, jackal, goblin, zombie |
| `)` | Weapon |
| `[` | Armor |
| `!` | Potion |
| `?` | Scroll |

Walls and closed doors block sight. What you can see is drawn in full color. Places you have explored stay on screen in cold, dim colors, along with the items lying there. Monsters are only shown while in sight.

The sidebar shows your health, attributes, depth, turn, seed and equipment, then every monster in view with a health bar and whether it is asleep, wandering or hunting. The log at the bottom reports what happens, with damage you take in red and kills in green.

## How it plays

**Monsters** sleep, wander, or hunt you. A monster can see you exactly when you can see it. Hunters that lose sight of you go to where they last saw you. Jackals are fast and zombies are slow. Rats and jackals can't open doors, so closing one can save you.

**Combat** rolls to hit by comparing the attacker's accuracy with the defender's dodge, then rolls damage and subtracts armor. A hit always does at least 1 damage. You slowly regain health over time. When you die, a death screen shows what killed you, how deep you got, and the seed.

**Your character** is a fighter with Strength, Agility and Intellect. Strength adds damage, and Agility sets accuracy and dodge. You start with a sword, leather armor and a potion of healing.

**Items** are scattered on every floor.

- Weapons: dagger, sword, mace and battle axe. Lighter weapons hit more often; heavier ones hit harder.
- Armor: leather, chain mail and plate. Heavier armor protects more but makes you easier to hit.
- Weapons and armor can be enchanted, from -1 to +2.
- Potions: healing, strength, life, and decay, which hurts.
- Scrolls: teleportation, magic mapping, enchanting, and aggravate monsters, which wakes the whole floor.

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
| `main` | Startup, command-line seed, game loop, and menus |
| `game` | Game state, player actions, turns, and the message log |
| `player` | The player's stats, equipment and pack |
| `combat` | Hit chance and damage |
| `item`, `inventory` | Item definitions and spawning; picking up, equipping and using items |
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

- Item identification: random potion and scroll appearances each run, and hidden enchantments
- Experience levels and skills that improve through use
- Hunger and traps
- Distinct dungeon zones and increasing difficulty
- High scores, help, and balance

See [DESIGN.md](DESIGN.md) for the design decisions and full milestone plan.

## Credits

This README was originally written by [Codex](https://openai.com/codex/), OpenAI's coding agent, and updated by Claude, Anthropic's AI assistant.
