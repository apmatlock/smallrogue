# smallrogue

A terminal dungeon crawler written in Rust. Explore randomly generated rooms and corridors, open doors, and descend into the dark.

The project is an early playable prototype: dungeon generation, movement, scrolling, field of view, and monsters with turn-based AI are implemented. Combat is under development; items and character progression are planned.

## Build and run

You need a Rust toolchain with Cargo and support for the Rust 2024 edition, plus an interactive terminal. The only direct dependency is [crossterm](https://crates.io/crates/crossterm).

```sh
cargo run
```

For an optimized build:

```sh
cargo run --release
```

Use a terminal at least 50 columns wide and 18 rows tall so the controls and message log fit. A larger window gives you more room to see the dungeon. The view scrolls to follow the player when the map is larger than the viewport.

### Replay a dungeon

Each run has a seed, shown in the sidebar. Supply it on the command line to reproduce the dungeon layouts:

```sh
cargo run -- --seed 1234
cargo run --release -- --seed 1234
```

Each floor is generated from the run seed and its depth, so its layout does not depend on the route you took through earlier floors. Layout reproducibility applies to the current generator; future changes may produce different layouts for the same seed.

## Controls

| Key | Action |
| --- | --- |
| Arrow keys or `h`, `j`, `k`, `l` | Move left, down, up, right |
| `y`, `u`, `b`, `n` | Move diagonally: up-left, up-right, down-left, down-right |
| `.` | Wait one turn |
| `>` | Descend while standing on stairs |
| `c` | Close an adjacent open door; choose a direction if several are nearby |
| `q`, `Esc`, or `Ctrl+C` | Quit during normal play |

Bump into a closed door to open it, then move again to step through. Moving, waiting, opening or closing a door, and descending each take a turn. Failed actions, such as walking into a wall, do not.

## Reading the dungeon

| Symbol | Meaning |
| --- | --- |
| `@` | You |
| `#` | Wall |
| `.` | Floor |
| `+` | Closed door |
| `'` | Open door |
| `>` | Stairs down |

Walls and closed doors block sight. Visible tiles appear in full color; explored tiles outside your field of view remain dimmed. Unexplored areas stay blank. The sidebar shows depth, turn count, and seed, while the bottom log reports recent actions and discoveries.

Monsters wander, spot you, and chase you, with different speeds and behaviors. Combat and death are the next milestone. There is currently no persistent scoring or saving. Descending generates a new floor, and quitting ends the run.

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
| `main` | Startup, command-line seed, and game loop |
| `game` | Game state, player actions, turns, and exploration |
| `dungeon` | Seeded room, corridor, door, and stair generation |
| `monster`, `ai`, `path` | Monster definitions, spawning, behavior, and pathfinding |
| `map`, `grid`, `geom` | Tiles, grid storage, and geometry |
| `fov` | Symmetric shadowcasting for field of view |
| `rng` | Seedable random number generation |
| `frame`, `ui` | Screen cells, map viewport, sidebar, and log |
| `input`, `term` | Keyboard commands and terminal rendering |

The terminal renderer draws only changed cells, and the input loop blocks while waiting for an event.

## Roadmap

The intended game is a grim, fast, endless fantasy crawl with short runs and depth reached as the score. Next milestones include:

- Combat, player stats, and a death screen
- Items, equipment, inventory, and identification
- Experience, skills, hunger, and traps
- Distinct dungeon zones and increasing difficulty
- High scores, help, and balance

See [DESIGN.md](DESIGN.md) for the design decisions and full milestone plan.

## Credits

This README was written by [Codex](https://openai.com/codex/), OpenAI's coding agent.
