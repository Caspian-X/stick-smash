# Design decisions

Living record of decisions made before/while porting from the TS version (`../stick-smash`). Status: **decided**, **proposed**, or **open**.

## Decided

### Scope
- **Source of truth.** The TS data is converted to JSON once (Node script that imports the TS modules). After that, this repo is the main version.
- **Players.** Design for N fighters (target 8), build and test for 4. No code may assume two fighters (the TS `const [a, b] = w.fighters` / `other()` patterns go away). Pairwise logic loops over all pairs; AI picks a target.
- **Teams.** Free-for-all or teams, chosen per match. Friendly fire is a match rule. Hit filtering in the sim checks team + rule.
- **Gameplay feel beats fidelity.** The TS version is the starting point, not the spec. The port first matches TS exactly (checked by replaying TS traces) as a verified baseline. After that, gameplay changes that make the game feel better are welcome. They're deliberate, configurable where sensible, and covered by their own tests (TS-trace tests run with those features off).
- **Input buffer.** Human presses (attack, special, jump, grab, shield press) are buffered for `global.input.bufferFrames` (5 ticks, inclusive age; 0 = TS behavior). Each entry captures press-time directions and smash taps; held buttons always use current raw input. Fresh presses keep every state's existing handling and take priority over replay. On an actionable tick, all fresh entries are consumed immediately; otherwise at most one oldest live entry is replayed and consumed, even if that state cannot use that action. Simultaneous blocked presses queue in attack, special, jump, grab, shield order. Replay states: idle, air, shield without shield stun, ledge when the handler's incremented timer reaches `minHang`, and knockdown when it exceeds `minFrames`. Hitlag blocks replay. Landing and other recovery states wait until the next update starts actionable. Jumpsquat, attacks/charge/cancels, hold/pummel, grabbed/mashing, hitstun/tech, helpless, respawn/death and all other states retain fresh-only handling; their buffered presses can still replay later if unexpired. CPU intents bypass the buffer. Buffer entries live in the rollback snapshot and age during countdown/end freezes (which supply no fresh presses or replay). Sub-tick taps are latched by the game crate's input layer so they last at least one tick.
- **Units.** Keep TS units: stage pixels, y down, velocities in px/frame, 60 Hz, `f64` + `libm`. Convert to Bevy's y-up world only when rendering.
- **Multiplayer.** Local multiplayer + CPUs first. Online stays possible: the sim is deterministic, `World: Clone` is the rollback snapshot, and inputs are small plain data. Candidate transport is `ggrs` over iroh datagrams. Nothing net-specific gets built yet.
- **Deferred, but not designed out.** Audio, drawing library details, online play.

### Content format
- **Everything moddable is data:** fighters (stats, frame data, special-move behavior, visuals, hurtboxes) and stages. Core content uses the same format as user content, with no fighter- or stage-specific code.
- **Strict JSON** (`.json`), one schema per content type, generated from the Rust types with `schemars` and saved to `assets/schema/`. Each file points to its schema via `"$schema"`, so VS Code etc. gives autocomplete, validation and hover docs (from the `///` comments on the Rust fields).
  - Why not RON/YAML/TOML/JSON5: RON is Rust-only, YAML has parsing footguns, and TOML handles deeply nested arrays of hitboxes badly. Comments wouldn't survive an editor save anyway, so docs live in the schema instead.
  - The writer uses a compact pretty-printer: small objects and number arrays (one hitbox, one keyframe) stay on one line, so files are readable and diffs are clean.
- Every file has `"schemaVersion"`. The loader migrates old versions forward.
- **Packages are folders:** `fighters/<id>/fighter.json`, `stages/<id>/stage.json`, so images and sounds can live next to them later. `"extends": "<id>"` inherits from another package. The merge copies objects key by key and replaces arrays whole. `fighters/base` holds shared stats, grabs, throws, state poses and hurtboxes.
- Load order: built-in `assets/`, then a user content dir (platform data dir). Errors name the file and JSON path and skip that package instead of crashing.
- A content hash of loaded data is kept so online play can later check that both sides use the same data.

### Moves: phase timelines (replaces `kind`)
Every move is a list of **phases**. A phase has:
- a frame timeline: hitboxes (same fields as the TS version, including per-frame `frames`), and frame events (`setVel`, `addVel`, `spawn` a projectile, `sfx`, `fx`, `invuln`, `armor`, `fallCap`…),
- an end: `after` N frames, or `loop` (charge, hover),
- **transitions**: `onLand`, `onHitReceived` (counter), `onRelease` / `whileHeld` (charge), `onTimeout`, `onHit`, each going to another phase or ending the move (`end`, `endHelpless`).

The 5 TS kinds (`projectile`, `gordo`, `groundPound`, `counter`, `superJump`) are re-expressed in this format. Projectiles are data too: physics (gravity, bounce, friction, maxBounces), lifetime, hitbox, and flags like `batable`/`reflectable`. No scripting language for now. It can be added later as an extra event type if the data model hits a wall.

### Visuals
- **Skeleton:** the current stick figure (head, shoulders, hips, elbows/hands, knees/feet, plus body rotation/pivot). Proportions are per fighter.
- **Poses:** named keyframe poses. `base` provides every state pose (idle, run cycle, jump, tumble, ledge, shield, knockdown…); fighters override what they need. Move phases animate by keyframing poses (with easing).
- **Attachments:** vector parts (line, circle, polygon, rounded rect) attached to a joint, with angle keyframes and an optional glow on active frames. This replaces the hard-coded sword/hammer/crown. Image attachments can come later.
- **Hurtboxes:** per fighter, per state group (default, crouch, knockdown, ledge).

### Stages
`stage.json`: solid ground + pass-through platforms, underside shape, ledges (grab zones), blast zones, camera bounds, and spawn/respawn points for up to 8 players. The current stage converts as-is. Bigger stages come later.

### Input
- Sim input = raw per-player state: quantized stick `(i8, i8)` + button bitflags. The sim derives presses, taps (smash input) and short hops from its own per-fighter input history, which is part of `World` so rollback stays correct.
- Keyboard maps directions to ±127. At first, sticks behave digitally through thresholds, so the game feels the same as the keyboard. True analog features (DI, walk speed) can come later without a format change.
- Mixed devices in one match. A join screen where any device presses a button to claim a slot. The keyboard can act as two devices (left/right halves). Rebinding per device. Gamepads are first-class; keyboard is the main test device for now.
- **Devices** (`game::input`): `Keys` (whole keyboard), `KeysL`/`KeysR` (split, the default), each connected
  gamepad, and scripted `Bot`s (screenshot tour). Every device maps 10 actions (left, right, up, down, jump, attack,
  special, grab, shield, start) through a binding profile (`keyboard`, `keyboard-left`, `keyboard-right`,
  `gamepad:<name>`, falling back to the built-in defaults) to a sim `Input`, plus menu presses: confirm = attack or
  jump (and Enter), back = special (and Esc/Backspace), start, and repeating directions. Gamepad left stick is
  analog into the sim stick; the d-pad overrides it. Presses are latched until the next sim tick.
- **Persistence** (`game::settings`): one JSON file in the platform config dir (settings, bindings, last versus and
  training picks). User content loads from `<data dir>/stick-smash/content` after the built-in `assets/`;
  content errors are logged and shown as a toast.

### UI
- **bevy_ui** with directional focus navigation for menus, character select, the HUD, and anything else players use with a gamepad.
- **egui** for the editor, debug overlays and training panels.
- **Focus navigation** is our own small layer over bevy_ui (`game::ui`), not `bevy_input_focus`: Bevy's
  `InputFocus` is a single global focus, and character select needs one cursor per player. Each focusable node
  has a stable `key` and an optional owner; screens are rebuilt from a plain model when it changes and focus
  survives by key. Directions move geometrically between nodes (last frame's layout, closest by distance +
  3 × sideways offset; sideways moves stay within 45°; vertical lists wrap). "Spinner" rows take left/right as
  value changes, so every card has a non-spinner footer (Lock in / Remove CPU / Add CPU) to travel between cards.
- **Look.** Barlow Condensed Black Italic for the wordmark, percents and big moments, Barlow Condensed for menu
  rows, Barlow for help text (OFL, `assets/fonts/`, with a system sans fallback for missing glyphs). Ink-navy
  translucent panels over the live stage, the 8-color palette as the accent strip, gold for focus. The title
  menus play a CPU attract match behind them; character select shows the picked fighters on the stage (ready ones
  strike their win pose). UI is laid out for 720 logical pixels high and scaled with the window.
- **Flow.** Title → Versus/Training → join + character select (one screen) → match → results (rematch, character
  select, main menu). Pause from any player's Start; only the pausing device drives the pause menu. Training uses
  the same select screen (one player + a dummy card) and an egui panel; dummy behavior and damage are also in the
  pause menu for gamepad players.
- **Teams in the renderer.** Each match gets a copy of `GameData` with the palette reordered per player (team
  color in team matches). The renderer and HUD color player *i* with palette *i*, so no render changes were
  needed. The content hash is unaffected (computed at load).
- **Match stats** (results screen) are game-side: damage increases are credited to the sim's hit trace attacker for
  that tick (else `sd.by`); a stock lost within 8 s of a hit is a KO for the last hitter, otherwise a self-destruct.
  Placement: the winner (and teammates) first, then by elimination time.
- **Presentation tuning** (damage color ramp, hit shake, end-of-match slow motion, results delay) lives in
  `global.render.hud`; the sim never reads it.
- **Editor** is a mode of the same executable, reached from the main menu. It's a fast-follow after the first playable build and gets its own UI design (not a port of the TS panels).

### Crates
`sim` (logic + data types/loading) · `render` (fighter/stage/fx drawing, shared by gameplay and editor) · `game` (bin: app, input, menus, modes) · later `editor` (lib, used by `game`) and `net`.

### Rendering (milestone 3)
- **Vector drawing** is our own small tessellator (`render::canvas`): polylines, circles, sectors, rounded rects
  and ear-clipped polygons into one vertex-colored triangle mesh per layer (backdrop, stars, stage, world,
  debug), rebuilt each frame, MSAA ×4. No gizmos (they can't fill), no lyon dependency.
- **Interpolation.** The game pushes a `World` snapshot to `render::Scene` every tick and sets `alpha` (fraction
  of a tick since then) every frame. Fighter positions and skeletons, projectiles, particles and the camera
  interpolate between the last two ticks. Fighters snap when their state, facing or position jumps.
- **Effects** are render-side: `Fx` events become particles/rings/beams with a non-sim RNG, advanced per tick.
- **Blending.** The GPU blends in linear light; the art was tuned for sRGB canvas blending, so translucent alpha
  is raised to `render.alphaGamma` (1.6) on output. Faint glows/shadows then look like the TS version.
- **Cosmetic tuning** lives in `global.render` (camera, overlays, effect looks) and stage `art`. It is part of
  the content hash; harmless, since it's loaded content either way.

### Camera (decided)
Tuning is in `global.json` → `render.camera` (one rule set for all stages; per-stage overrides can come with
bigger stages). Each tick: frame every fighter that isn't dead/out (feet positions, clamped to the blast zone ±
`blastMargin`), pad by `padX`/`padTop`/`padBottom`, pull the center `stageBias` toward the stage's center, fit the
window's aspect ratio, clamp the visible height to `minHeight..maxHeight` (zoom limits), and clamp the view inside
the stage's `camera` bounds. The camera eases toward that target by `follow` (position) and `zoomFollow` (zoom)
per tick, so it's deterministic for a given world history (screenshots reproduce). Shake comes from `Fx::Shake`,
decays by `shakeDecay` per tick and offsets the view randomly. The backdrop gets a little parallax
(`art.background.parallax`).

## Milestones

1. **Data:** Rust content types + schemas; Node script converts TS fighters/stage into the new format (including the 5 kinds as phases); tests load and validate all assets.
2. **Sim port:** physics, states, combat, moves, projectiles, AI. N players, teams, raw input. A check against the TS sim: record inputs and per-frame state from TS, replay in Rust, compare with a tolerance.
3. **Render:** stick figures from pose data, attachments, stage, fx, debug overlay, camera that follows the fighters.
4. **Playable:** join screen with mixed devices, character select, CPU level, stock matches, training mode, HUD.
5. **Editor** (fast-follow).

## Open
- Audio: port the synthesizer (`fundsp`) or use sample files.
