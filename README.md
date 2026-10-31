# Stick Smash (Rust)

A Smash-style platform fighter in Rust with Bevy and egui, ported from the Phaser/TypeScript version.
It's meant for local multiplayer on one machine and online play between installed copies of the game.

```bash
cargo run -p game                 # play
cargo run -p game --features dev  # faster incremental builds (dynamic linking)
cargo run -p game -- --screenshot shots/  # scripted tour through menus, a 4-player match, results, training; saves PNGs
cargo test --workspace            # sim (incl. TS traces), render and game tests
```

## How to play

Pick **Versus** or **Training** on the title screen. On character select, any device presses a button to join
(up to 8 players). Left/right changes your fighter, attack locks in, special backs out. Move down to your card's
footer and sideways to reach **Add CPU** or a CPU's card (level, fighter, remove), or up to the rules (stocks,
stage, teams, friendly fire). Only stages that fit the lineup are offered; teams need at least two opposing sides. When everyone's ready, press start. Start/Esc pauses (the device that paused drives
the menu; ownership moves to a connected device if it disconnects). A disconnected unready player leaves select;
a ready slot stays available to remove or reclaim with another device. After the match: rematch, character select or main menu. F1 toggles the hitbox/debug overlay.

Training: you against a dummy that stands, shields, jumps, fights as a CPU or is controlled by a second device.
The panel (top right) shows your move and frame, the last hit's frame advantage and combo, and has dummy damage,
speed, freeze/step and reset. Dummy settings are also in the pause menu.

### Default controls

| | Keyboard left | Keyboard right | Gamepad |
|---|---|---|---|
| Move | W A S D | Arrow keys | Left stick / d-pad |
| Jump | Space (or up) | Right Shift, Num 4 (or up) | X / Y (or up) |
| Attack | J | . or Num 1 | A |
| Special | K | / or Num 2 | B |
| Grab | I | ; or Num 3 | RB |
| Shield | L | ' or Num 0 | LB, LT, RT |
| Pause | Esc | Enter | Start |

Settings → Keyboard switches between two keyboard players (above) and one player using both halves. Controls
rebinds any device (select an action, press the new control; Esc or the gamepad's B/Back cancels).
Split keyboard bindings refuse keys used by the other half. Gamepads have per-device session overrides with
shared model defaults. Gameplay presses clear across menus and pause; release held buttons after resuming. Tap a direction with attack for a smash
attack (hold to charge). Shield + left/right rolls, shield + down spot-dodges, shield in the air air-dodges.
"Up to jump" can be turned off per player on character select.

Settings, bindings and your last picks are saved to the platform config dir (`~/Library/Application Support/
stick-smash/settings.json` on macOS). Your own fighters and stages go in the platform data dir under
`stick-smash/content/`, laid out like `assets/`; they load after the built-in content and can override it.

One-shot TS content import (Node 26, sibling `../stick-smash` checkout): `node tools/convert-ts/convert.mjs`, then `BLESS=1 cargo test -p sim --test content`.

## Layout

```
crates/sim/    deterministic game logic, no Bevy (rollback-ready)
crates/render/ Bevy plugin drawing a sim World: figures, attachments, stage, effects, camera, debug overlay
crates/game/   Bevy app: input devices, menus (bevy_ui), match flow, HUD, training panel (egui)
assets/        JSON content: global tuning, stages, fighters (moddable), generated schemas; fonts (OFL)
docs/          DESIGN.md (decisions), FORMAT.md (content format and move phase semantics)
tools/         one-shot TS → JSON converter
```

Status: milestones 1–4 done (content format, sim verified against TS traces, rendering, playable game with
menus, join/character select, stock matches, training and HUD). Next: the editor.
