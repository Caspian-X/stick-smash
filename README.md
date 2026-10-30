# Stick Smash (Rust)

A Smash-style platform fighter in Rust with Bevy and egui, ported from the Phaser/TypeScript version.
It's meant for local multiplayer on one machine and online play between installed copies of the game.

```bash
cargo run -p game                 # play
cargo run -p game --features dev  # faster incremental builds (dynamic linking)
cargo test -p sim                 # headless simulation tests
```

One-shot TS content import (Node 26, sibling `../stick-smash` checkout): `node tools/convert-ts/convert.mjs`, then `BLESS=1 cargo test -p sim --test content`.

## Layout

```
crates/sim/    deterministic game logic, no Bevy (rollback-ready)
crates/game/   Bevy app: rendering, input, audio, menus, egui UI
assets/        JSON content: global tuning, stages, fighters (moddable), generated schemas
docs/          DESIGN.md (decisions), FORMAT.md (content format and move phase semantics)
tools/         one-shot TS → JSON converter
```

Status: milestone 1 (content format) done. The format is described in `docs/FORMAT.md`, and content was converted from the TS version with `tools/convert-ts/`. The sim itself is still a stub.
