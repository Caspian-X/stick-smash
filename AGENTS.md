# AGENTS.md

Stick Smash in Rust: a Smash-style platform fighter, ported from the Phaser/TypeScript version (`../stick-smash`). Bevy 0.20 + bevy_egui 0.43.

## Commands
- `cargo run -p game` (add `--features dev` for dynamic linking / faster rebuilds).
- `cargo test -p sim`: headless sim tests. `cargo clippy --workspace`.

## Layout
- `crates/sim/`: deterministic game logic. A pure function of `(World, GameData, inputs)`.
- `crates/game/`: Bevy app (rendering, input, audio, menus, egui UI). Owns the sim `World` as a resource and steps it in `FixedUpdate` at `sim::TICK_HZ`.
- `assets/`: JSON data (global tuning, stage, fighters). Fighters are meant to be user-moddable.

## Rules
- **`crates/sim` must stay deterministic and Bevy-free.** No I/O, no wall-clock time, no OS/thread randomness (use `world.rng`), no `HashMap` iteration order in logic. Use `libm` for sin/cos/atan2/etc. Use `f64` like the TS version.
- `sim::World` is the rollback snapshot: keep it plain `Clone` data. Refer to other fighters/projectiles by index, never by reference.
- Visuals/audio leave the sim as `Fx` events; the game crate drains `world.fx` each tick.
- All tunable numbers live in `assets/` JSON, not in code. The sim parses JSON strings; the game crate reads files.
- Behavior differences between moves are selected by data (`kind`), never by fighter or move id.
- The TS version is a starting point, not the spec. The initial port matches it exactly (TS trace tests); deliberate gameplay improvements come after, configurable where sensible, with TS-trace tests running with them off.
- Match the compact code style; comments only where behavior isn't obvious.
