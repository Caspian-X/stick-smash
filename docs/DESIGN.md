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
- **Deferred, but not designed out.** Online play.

### Content format
- **Everything moddable is data:** fighters (stats, frame data, special-move behavior, visuals, hurtboxes) and stages. Core content uses the same format as user content, with no fighter- or stage-specific code.
- **Strict JSON** (`.json`), one schema per content type, generated from the Rust types with `schemars` and saved to `assets/schema/`. Each file points to its schema via `"$schema"`, so VS Code etc. gives autocomplete, validation and hover docs (from the `///` comments on the Rust fields).
  - Why not RON/YAML/TOML/JSON5: RON is Rust-only, YAML has parsing footguns, and TOML handles deeply nested arrays of hitboxes badly. Comments wouldn't survive an editor save anyway, so docs live in the schema instead.
  - The writer uses a compact pretty-printer: small objects and number arrays (one hitbox, one keyframe) stay on one line, so files are readable and diffs are clean.
- Every file has `"schemaVersion"`, versioned per content kind. The loader dispatches migrations by kind; see [FORMAT.md](FORMAT.md).
- **Packages are folders:** `fighters/<id>/fighter.json`, `stages/<id>/stage.json`, so images and sounds can live next to them later. `"extends": "<id>"` inherits from another package. The merge copies objects key by key and replaces arrays whole. `fighters/base` holds shared stats, grabs, throws, state poses and hurtboxes.
- Load order: built-in `assets/`, then a user content dir (platform data dir). Errors name the file and JSON path and skip that package instead of crashing.
- A content hash of loaded data is kept so online play can later check that both sides use the same data.

### Moves: phase timelines (replaces `kind`)
Every move is a list of **phases**, starting at the first. A phase has:
- a frame timeline: hitboxes (including per-frame `frames`), frame events (`setVel`, `addVel`, `spawn`, `sfx`,
  `fx`, `invuln`), and phase properties such as `armorThreshold` and `fallCap`,
- an optional duration, `frames`; omitting it makes a looping phase that needs a transition to leave,
- optional `charge`, which freezes the phase at its charge frame while the button is held, up to its maximum,
- ordered **transitions** with `on` triggers (`land`, `counter`, `release`, `timeout`, `hit`, `shieldPress`,
  `grounded`, `airborne`, `falling`). A transition's `to` names a phase or an ending: `end`, `helpless`, or
  `landing` with `lag`.

The five TS kinds (`projectile`, `gordo`, `groundPound`, `counter`, `superJump`) are expressed through these
phases. Projectiles are data too: spawn velocity, gravity, optional `bounce`, lifetime, hitbox, optional `bat`,
and `reflectVy` for counter reflections. No scripting language is needed; an extra event type can extend the
model later. See [FORMAT.md](FORMAT.md#moves) for exact frame timing, events and transition semantics.

### Visuals
- **Skeleton:** the current stick figure (head, shoulders, hips, elbows/hands, knees/feet, plus body rotation/pivot). Proportions are per fighter.
- **Poses:** named keyframe poses. `base` provides every state pose (idle, run cycle, jump, tumble, ledge, shield, knockdown…); fighters override what they need. Move phases animate by keyframing poses (with easing).
- **Attachments:** vector parts (line, circle, polygon, rounded rect) attached to a joint, with angle keyframes and an optional glow on active frames. This replaces the hard-coded sword/hammer/crown. Image attachments can come later.
- **Hurtboxes:** per fighter, with default and knockdown groups. Crouch and ledge groups are future work.

### Stages
`stage.json` (version 2): multiple static solid blocks with tapered undersides and independent top-corner ledges, pass-through platforms with optional frame-based waypoint motion, shared art and ledge zones, blast/camera bounds, and spawn/respawn points for up to 8 players. Version 1 ground migrates to a block with both ledges enabled. See FORMAT.md for motion timing and geometry constraints.

### Input
- Sim input = raw per-player state: quantized stick `(i8, i8)` + button bitflags. The sim derives presses, taps (smash input) and short hops from its own per-fighter input history, which is part of `World` so rollback stays correct.
- Keyboard maps directions to ±127. At first, sticks behave digitally through thresholds, so the game feels the same as the keyboard. True analog features (DI, walk speed) can come later without a format change.
- Mixed devices in one match. A join screen where any device presses a button to claim a slot. The keyboard can act as two devices (left/right halves). Rebinding per device. Gamepads are first-class; keyboard is the main test device for now.
- **Devices** (`game::input`): `Keys` (whole keyboard), `KeysL`/`KeysR` (split, the default), each connected
  gamepad, and scripted `Bot`s (screenshot tour). Every device maps 10 actions (left, right, up, down, jump, attack,
  special, grab, shield, start) through a binding profile (`keyboard`, `keyboard-left`, `keyboard-right`,
  `gamepad:<name>#<session index>` overrides over shared `gamepad:<name>` defaults, falling back to the built-in defaults) to a sim `Input`, plus menu presses: confirm = attack or
  jump (and Enter), back = special (and Esc/Backspace), start, and repeating directions. Gamepad left stick is
  analog into the sim stick; the d-pad overrides it. Presses are latched until the next sim tick. Screen, pause/resume and ownership changes clear latches and
  edge history; held controls are suppressed until released. Split-keyboard rebinding refuses keys owned by
  the other active half; gamepad capture can be cancelled with Esc or B/Back.
- **Persistence** (`game::settings`): one JSON file in the platform config dir (settings, bindings, last versus and
  training picks). User content loads from `<data dir>/stick-smash/content` after the built-in `assets/`;
  content errors are logged and shown as a toast.

### UI
- **bevy_ui** with directional focus navigation for menus, character select, the HUD, and anything else players use with a gamepad.
- **egui** for the editor, debug overlays and training panels.
- **Focus navigation** is our own small layer over bevy_ui (`game::ui`), not `bevy_input_focus`: Bevy's
  `InputFocus` is a single global focus, and character select needs one cursor per player. Lineup changes are deferred until all devices finish navigation, then
  cursors are repaired. Lobbies allow eight slots and offer only stages with enough spawn and respawn points;
  teams require two opposing sides. Unready disconnected slots leave; ready slots can be removed or reclaimed. Each focusable node
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
  select, main menu). Pause from any player's Start; the pausing device drives the pause menu, with ownership transferred on disconnect. Training uses
  the same select screen (one player + a dummy card) and an egui panel; dummy behavior and damage are also in the
  pause menu for gamepad players.
- **Teams in the renderer.** Each match gets a copy of `GameData` with the palette reordered per player (team
  color in team matches). The renderer and HUD color player *i* with palette *i*, so no render changes were
  needed. The content hash is unaffected (computed at load).
- **Match stats** (results screen) are game-side: every damaging entry in the sim's per-victim `World::hits` records credits damage (shield hits are excluded); a stock lost within 8 s of a hit is a KO for `last_hit_by`, otherwise a self-destruct. Hit records drain each tick alongside effects.
  Placement: the winner (and teammates) first, then by elimination time.
- **HUD safe area.** The game measures the HUD panels after layout and passes the bottom reserved fraction to
  the renderer. The camera fits the remaining rectangle; tags share its world mapping and offscreen bubbles
  stop above the panels.
- **Presentation tuning** (damage color ramp, hit shake, end-of-match slow motion, results delay) lives in
  `global.render.hud`; the sim never reads it.
- **Editor** is a mode of the same executable, reached from the main menu (see "Editor" below).

### Audio
- **Procedural bank.** The game synthesizes swept sine/triangle/square/saw tones and low-pass noise at startup,
  encoding mono PCM into in-memory WAV `AudioSource` assets. Bevy's built-in audio handles playback; no direct
  rodio dependency, binary sound assets or sound licenses. The TS WebAudio sound vocabulary is preserved, with
  separate confirm/back/join/ready cues. Music is deferred; its volume control is reserved.
- **Data.** `global.audio` holds synth layers, gains, envelopes, priorities, pitch variation, voice/rate limits,
  panning and impact scaling. Typed fields generate schema hover docs; the loader rejects invalid or excessive
  allocations. Master/SFX/music settings persist alongside existing bindings; older settings use data defaults.
- **Impact.** Hit knockback (shield-hit damage) selects a prebuilt strength variant, changing duration and gain.
  Hits, shield hits/breaks and blast-zone KOs include an optional world x in `Fx::Sfx`; the game maps it through
  the renderer's visible rectangle into stereo panning. Other sounds and UI cues are centered. This adds event
  context without changing gameplay or consuming sim randomness.
- **Time and availability.** Playback stays at normal pitch/duration in slow motion; events follow simulation
  timing. Each rendered frame retains the strongest event per name, then prioritizes within wall-clock cooldowns
  and voice limits. Tour fast-forward discards audio; attract/showcase worlds stay silent. Players also expire
  by real time, so Bevy's graceful missing-device fallback cannot accumulate queued audio entities.

### Crates
`sim` (logic + data types/loading) · `render` (fighter/stage/fx drawing, shared by gameplay and editor) · `editor` (lib: fighter editor, used by `game`) · `game` (bin: app, input, menus, modes) · later `net`.

### Editor (milestone 5)
A fighter and stage editor reached from the title menu (**keyboard and mouse only**). Priorities: fast iteration, clarity,
feel. Its own design, not a port of the TS panels.

- **Layout** (egui, Barlow fonts, the ink-navy/gold palette): a top bar (fighter picker with dirty dots, undo/redo,
  save, test, validation status, back), a left **browser** (moves grouped ground/smash/air/special/grab, throws,
  poses, fighter stats), the center **viewport** (the real renderer through `render::Scene` with a fixed view), a
  right **inspector** for the selection, and a bottom **timeline**.
- **Truthful preview.** A sandbox `World` (training rules, stage `arena` or the first stage) runs the real sim: the
  fighter starts the move on frame 0 (grounded, or in the air for aerials / when toggled) and the sandbox steps
  until the fighter is actionable again (plus a short tail, capped). Every frame is kept, so scrubbing is an index.
  Any data change re-validates (`sim::data::load::check_fighter`) and re-simulates from frame 0. Optional
  dummy (distance, percent) shows real knockback; optional held button shows charge. Throws grab the dummy and
  throw it. Invalid data keeps the last good preview and shows the error with its JSON path.
- **Timeline** segments come from the simulation (which phase the fighter was in on each frame), so loops,
  landings and hit transitions show as they really play. Rows below: the playhead phase's hitbox windows, event
  ticks and pose keys. Click/drag scrubs; space plays (0.25×–1×).
- **Direct manipulation.** In the viewport: drag a hitbox to move it, its rim to resize, the launch-arrow tip to set
  the angle (Shift snaps 15°). Hitboxes with per-frame `frames` edit the entry shown on the current frame.
  When the playhead sits on a pose key, the joints get handles and can be dragged (named poses are shared; the
  inspector says how many keys use it). Wheel zooms at the cursor, right/middle drag pans, F frames the fighter.
- **Shortcuts.** Space play · ←/→ (or , .) step · Home/End · [ ] previous/next phase · ⌘/Ctrl Z, Shift Z / Y
  redo · ⌘/Ctrl S save · ⌘/Ctrl D duplicate hitbox · N new hitbox · Delete removes it · Tab cycles hitboxes ·
  Esc deselects, then leaves.
- **Model** (`editor::doc`, plain Rust, unit-tested): per fighter a `Doc` with the typed merged `Fighter` being
  edited, the snapshot it was loaded/saved as, snapshot undo/redo (continuous drags and typing coalesce into one
  step), and the package's own raw JSON.
- **Save and `extends`.** The file written is the package's *own* document, not the merged fighter. On save the
  editor diffs the typed fighter (serialized) against its last-saved snapshot (serialized the same way) and patches
  only the changed paths into the raw document: objects recurse key by key, anything else (arrays included, as in
  the merge rule) is written whole, and removed keys become `null` when the parent still has them. Untouched values,
  `extends`, `$schema` and inherited data stay as they were, so a child stays a thin override; editing an inherited
  move writes that move's override into the child. Before writing, the real loader (`sim::data::load`) runs over all
  content with the new document in place; any error for the package refuses the save and lists file + JSON path.
  Files go to the user content dir (`<data dir>/stick-smash/content/fighters/<id>/fighter.json`), which overrides
  the built-in file; `--features dev` adds a toggle to write into `assets/` instead. Output uses
  `to_compact_string`. Abstract parents (`base`) aren't editable in v1.
- **Test** starts training with the edited (unsaved) data: P1 on the keyboard, a standing dummy. The pause menu's
  "Back to editor" returns with all editor state intact. After saves, leaving the editor reloads the game's content.
- **Stage editor** (the top bar's Fighters | Stages switch). Same frame, undo, saving, dialogs and shortcut sheet;
  `Doc<T: Content>` covers both kinds. The viewport is the real renderer on a preview `World` of the edited stage
  (art and backdrop included; render rebuilds its static layers when `Scene::data` changes), optionally with 1–4
  CPUs playing on it through the real sim (training rules; edits apply to the running sandbox). Direct manipulation
  works on generic shapes (`stage_view::Shape`: rect sides/corners, segment ends, points): blocks (edges, corners,
  per-corner ledge toggles above a selected block; add, duplicate, delete), platforms (move, resize ends; add,
  duplicate, delete) and their motion waypoints (point handles, the path and ghosts drawn; mode, phase, travel and
  wait in the inspector; a play / scrub clock so motion shows with the sandbox off), spawns (slot numbers in player colors, click the
  facing arrow to flip) and respawns (platform and start height), blast and camera bounds as dashed rects, the
  ledge zones (rim sets the radius). Default snapping is whole pixels, Shift (or the Grid toggle) snaps to the grid,
  Alt is 0.1 px. Symmetry mirrors edits around `width / 2`: a dragged part's mirror partner (found by geometry) gets
  the mirror image (blocks swap left/right ledges, platform paths mirror), a centred part stays centred and can't be
  dragged past the axis, and list elements without a reflection are left alone. Deleting a mirrored pair is checked
  as a whole. The sandbox World is rebuilt on any geometry or slot change (keeping its clock). Format-1 documents
  are migrated before they become the save baseline, so saves never mix `ground` with `blocks`. The inspector is hand-made for geometry and uses schema forms
  (with colour pickers) for art and the backdrop. Problems are the loader's `check_stage`; notes flag legal but
  likely-wrong setups (fewer than 8 spawns, spawns in the air, geometry past the blast zone, KOs off camera). New
  stages copy a template stage (unsaved edits included) as a whole file; only never-saved stages can change id.
  Test plays training or a 3-stock match vs a CPU on the stage; results and pause return to the editor.


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
