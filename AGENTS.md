# AGENTS.md

Stick Smash in Rust: a Smash-style platform fighter, ported from the Phaser/TypeScript version (`../stick-smash`). Bevy 0.20 + bevy_egui 0.43.

## Commands
Tasks live in the `justfile` (`just` lists them). Add new scripts as `just` recipes, not shell files.
- `just run` (`just dev` for dynamic linking / faster rebuilds), `just test`, `just lint`, `just check`.
- `just bless` after changing content data types (regenerates schemas, reformats assets). `just shots [dir]` for screenshots.

## Layout
- `crates/sim/`: deterministic game logic. A pure function of `(World, GameData, inputs)`.
- `crates/render/`: Bevy plugin that draws a sim `World` read-only (figures, attachments, stage, fx, camera, debug overlay). Fed via `render::Scene::push` each tick.
- `crates/editor/`: fighter editor (lib used by `game`, `Screen::Editor`). Plain-Rust model in `doc.rs` (workspace, per-fighter docs, snapshot undo, saving that patches only changed paths into the package's own JSON so `extends` stays intact), `preview.rs` (sandbox `World` running the real sim from frame 0, every frame kept), `view.rs` (camera, transforms, hit-testing, overlay); egui layer in `ui.rs`/`inspect.rs`. Draws through `render::Scene` (`fixed_view`, `overlay`, `pose_override`). The game hooks it up in `game/src/editing.rs` (menu entry, Test → training → "Back to editor").
- `crates/game/`: Bevy app (input devices, bevy_ui menus/HUD, match flow, egui training panel). `cargo run -p game -- --screenshot <dir>` runs a scripted tour (bot devices pressing buttons through title, the editor (via `editor::Inbox` commands), select, a 4-player match, pause, results, training) and saves PNGs for visual checks; the steps live in `shots.rs`. Owns the sim `World` as a resource and steps it in `FixedUpdate` at `sim::TICK_HZ`.
- `assets/`: JSON data (global tuning, stage, fighters). Fighters are meant to be user-moddable.

## Rules
- **`crates/sim` must stay deterministic and Bevy-free.** No I/O, no wall-clock time, no OS/thread randomness (use `world.rng`), no `HashMap` iteration order in logic. Use `libm` for sin/cos/atan2/etc. Use `f64` like the TS version.
- `sim::World` is the rollback snapshot: keep it plain `Clone` data. Refer to other fighters/projectiles by index, never by reference.
- Visuals/audio leave the sim as `Fx` events; the game crate drains `world.fx` each tick.
- All tunable numbers live in `assets/` JSON, not in code. The sim parses JSON strings; the game crate reads files.
- Behavior differences between moves are selected by data (`kind`), never by fighter or move id.
- The TS version is a starting point, not the spec. The initial port matches it exactly (TS trace tests); deliberate gameplay improvements come after, configurable where sensible, with TS-trace tests running with them off.
- Match the compact code style; comments only where behavior isn't obvious.
