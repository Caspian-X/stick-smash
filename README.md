# Stick Smash (Rust)

A Smash-style platform fighter in Rust with Bevy and egui, ported from the Phaser/TypeScript version.
It's meant for local multiplayer on one machine and online play between installed copies of the game.

```bash
cargo run -p game                 # play (P1 keyboard vs a CPU)
cargo run -p game --features dev  # faster incremental builds (dynamic linking)
cargo run -p game -- --screenshot shots/  # CPU-vs-CPU match, saves PNGs at interesting ticks, exits
cargo test --workspace            # sim (incl. TS traces) + render tests
```

Controls (temporary, until the milestone 4 menus): A/D/W/S move, Space jump, J attack, K special, I grab,
L shield. R restarts, 1–3 picks P1's fighter, F1 toggles the debug overlay (hitboxes, hurtboxes, ledge and
blast zones, state/move/frame text).

One-shot TS content import (Node 26, sibling `../stick-smash` checkout): `node tools/convert-ts/convert.mjs`, then `BLESS=1 cargo test -p sim --test content`.

## Layout

```
crates/sim/    deterministic game logic, no Bevy (rollback-ready)
crates/render/ Bevy plugin drawing a sim World: figures, attachments, stage, effects, camera, debug overlay
crates/game/   Bevy app: input, match flow, audio, menus, egui UI
assets/        JSON content: global tuning, stages, fighters (moddable), generated schemas
docs/          DESIGN.md (decisions), FORMAT.md (content format and move phase semantics)
tools/         one-shot TS → JSON converter
```

Status: milestones 1–3 done (content format, sim verified against TS traces, rendering). The game is
temporarily playable (one keyboard player vs a CPU); menus, join screen and HUD are milestone 4.
