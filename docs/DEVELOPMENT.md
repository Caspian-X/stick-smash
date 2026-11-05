# Development

This is the long-form guide for human and LLM contributors. [AGENTS.md](../AGENTS.md) is the terse, always-loaded summary (`CLAUDE.md` is its symlink). Read it first. [DESIGN.md](DESIGN.md) records decisions; [FORMAT.md](FORMAT.md) defines content semantics. For playing, playtesting and authoring content, use [README.md](../README.md), [PLAYTESTING.md](PLAYTESTING.md) and [MODDING.md](MODDING.md).

## Architecture and data flow

The workspace has four crates:

| Crate | Responsibility | Starting points |
| --- | --- | --- |
| `sim` | Bevy-free deterministic simulation, content types, loading, validation and schemas | `crates/sim/src/{lib,world,step,input}.rs`, `crates/sim/src/data/` |
| `render` | Read-only drawing of sim worlds: stick figures, attachments, stages, effects, camera and debug overlay; portrait and stage-thumbnail sheets | `crates/render/src/{lib,draw,pose,stage,fx,sheet,portrait,thumb}.rs` |
| `editor` | Fighter and stage documents, undo/save, real-sim previews and egui editing | `crates/editor/src/{lib,doc,ui}.rs` |
| `game` | Executable: content roots, devices/settings, screens, match stepping, audio and replays | `crates/game/src/{main,input,settings,play}.rs` |
| `pads` | Native gamepad backends for controllers gilrs can't read: Switch controllers on macOS (GameController framework) and GameCube adapters (hidapi); the only crate allowed `unsafe` | `crates/pads/src/{lib,mac,gcn}.rs` |

```text
assets/ + user content root
          | sources -> JSON strings (Source)
          v
sim::data::load -> migrate / resolve / validate -> GameData
                                                    |
devices -> raw Input[] -> game::play::Match -> sim::step_world(&mut World, ...)
                                                    |
                           +------------------------+--------------------+
                           | World + pending Fx                          | Sfx
                           v                                             v
                  render::Scene::push                              game::audio
                           |
                  drawing / camera / effects

editor::Doc -> validated edited GameData -> preview World -> the same Scene
```

`crates/game/src/main.rs::load_data` collects built-in `assets/` first and the platform user content directory second. The optional `sim` feature `fs` supplies `data::sources`: sorted root-relative JSON sources, excluding schema directories and symlinks. The simulation core itself accepts strings and does no file I/O. Later sources replace the same path. Screenshot runs use built-in content only.

`crates/sim/src/data/load.rs::load` parses JSON, calls `migrate_kind`, resolves fighter `extends`, deserializes typed content, and calls `validate`. Invalid packages report file/JSON paths and are skipped; unusable globals or no playable fighter/stage fail loading. `GameData` holds globals, sorted playable fighters/stages and a content hash. `validate` also accepts already-resolved data, including replay content.

`crates/game/src/play.rs::Match` owns the live `sim::World`. `main.rs` schedules `play::tick` in `FixedUpdate` with `Time<Fixed>::from_hz(sim::TICK_HZ)` (60 Hz). Input devices supply raw per-slot `sim::Input`; CPU intents are generated inside the sim. Pause and presentation slow motion can defer simulation steps.

After stepping, the game tracks hit records, queues sound events and calls `Scene::push` **before** clearing `world.fx` and `world.hits`. The renderer consumes its cloned snapshots, advances effects/camera per pushed tick, and interpolates between ticks using `Scene::alpha`. Audio consumes the queued `Fx::Sfx` events. Replays record after the event drain. Changing this order changes what the renderer sees and what replay checksums mean.

The editor also feeds `Scene`: fighter previews keep every sandbox frame for scrubbing; stage previews run an edited stage with optional CPUs. `Scene::fixed_view`, `overlay` and `pose_override` provide editor pan/zoom, handles and pose editing. Replacing `scene.data` invalidates cached static stage drawing.

## Rules and their reasons

- **Keep the sim deterministic and Bevy-free.** No core I/O, wall-clock time, OS/thread randomness or dependence on `HashMap` iteration order. Preserve fighter/pair order and RNG call order. Identical content, initial world and input history must reproduce the same world for traces, replays and eventual rollback networking.
- **Use `f64` and `libm` in simulation math.** This retains the TS numeric baseline and avoids platform-dependent transcendental implementations. Stage coordinates are pixels with y down and velocity in pixels/tick; raw input stick y points up. Rendering converts to Bevy's y-up coordinates.
- **Keep random state in the snapshot.** Gameplay draws use `world.rng`. `World::effect_rng` uses `world.cosmetic_rng` when `global.sim.separateCosmeticRng` is enabled, otherwise the gameplay stream. Both are deterministic; splitting them keeps cosmetic range edits from changing AI/gameplay. The renderer has its own non-sim `Rand` for particles, shake and jitter.
- **`World` is plain `Clone` rollback data.** Put evolving gameplay state, input history/buffers and RNG state there. Refer to fighters/projectiles by index; do not retain references, ECS entities or external mutable state. New snapshot fields always change the golden hash (it hashes `Debug` output) and replay checksums unless skipped in serialization, so they need a `SIM_VERSION` bump.
- **Emit presentation as `Fx`.** The game/render/audio layers own side effects; a headless simulation must still work. Per-victim `HitRecord`s carry game-side statistics without coupling simulation to the HUD.
- **Put tunables in `assets/` JSON.** Model them with typed fields and schema documentation, then validate their ranges. Gameplay tuning belongs with sim data; presentation tuning lives in `global.render`, `global.audio` and stage art. Editor UI constants are in `crates/editor/src/style.rs`.
- **Choose behavior through data, never fighter or move ids.** Use phase/event fields and typed discriminants such as hitbox `kind`. There is no legacy move `kind` dispatcher: phases express the original special moves. New behavior must be reusable by another package without a code change.
- **TS is a baseline, not the specification.** Keep deliberate improvements configurable where sensible and disable them in TS trace setup. Test the improved behavior separately; do not rewrite reference fixtures to conceal a Rust regression.
- **Match the compact style.** Follow nearby code; comment only non-obvious behavior. Add project scripts as `justfile` recipes, not shell files.

## Game screens and shared UI

`crates/game/src/main.rs` declares `Screen`, `Screen::ALL`, module registration and app wiring. Screen modules own plugins: `menu.rs` (title/settings/controls), `select.rs`, `stage_select.rs`, `hud.rs`, `results.rs`, `training.rs` and `replay/ui.rs`; shared `ui::UiPlugin`, `pointer::PointerPlugin` and `audio::SoundPlugin` are added alongside them. `editing.rs` installs the generic editor plugin for `Screen::Editor` and handles Test/Back/quit requests.

`crates/game/src/ui.rs` orders `UiSystems::{Input, Draw, Animate}` before `render::RenderSystems`. Read devices/update the plain model in Input, rebuild changed UI in Draw, and advance presentation in Animate. `ui::staying` skips work when a screen transition is pending. `ui::Rebuild<K>` and `ui::rebuild` compare a model/key and replace the screen root only when it changes; roots get `DespawnOnExit(screen)`. Focus survives rebuilds through stable `Focusable::key`s, optionally restricted to one owner.

Presentation that must not wait on the simulation counts real seconds in `Update` (`Time::delta_secs` / `elapsed_secs`), not sim ticks or frames: the end banner's clock pacing the results screen (`play::match_flow`), the results entrance (`results::animate`) and the pointer/hint/select animations. That keeps a 30, 60 or 144 Hz display and a long or short end freeze from changing how they feel. `Match` keeps ticking while the results are up, so a match that opens the results screen during its end freeze still finishes, poses and closes its replay behind it.

Give transient screen resources an explicit lifetime: insert them on enter and remove them on exit. Examples: select's `Moments`/`HintClock`, stage vote's `Vote`/`Clock`. Some state deliberately persists across screens: `Select` keeps the lobby for stage voting/return, `Match` can animate behind results, and the editor survives its Test round trip. Do not remove those as incidental cleanup. `UiPlugin` registers `input::clear` on entry to every `Screen::ALL` member so the button that changed screens cannot trigger the next screen. Input clearing also suppresses held controls until release.

Shared modules:

- `input.rs`: whole/split keyboard, gamepads and scripted bots, bindings, menu repeats, sub-tick press latches; the sim derives action edges from its own history.
- `pointer.rs`: each human's analog/mouse pointer, magnet, chip carrying and animation. `grid.rs`: fitting tile grids and digital navigation shared by fighter/stage selection. Logical target boxes come from `ui::Spot`.
- `hints.rs`: device-specific control captions and hint fade clocks.
- `showcase.rs`: live selected lineup behind the UI, swaps picks in place, lock-in flourish and ready win poses. `ui::Safe` markers reserve the roster/card bands for camera framing.
- `replay/model.rs`: recording, RLE inputs, reset events, checksums and version constants; `player.rs`: deterministic playback, checkpoints and seeking; `storage.rs`: bounded headers, files, retention and headless verification; `content.rs`: embedded-data validation/presentation adaptation; `ui.rs`: library, playback controls and results return; `tests.rs`: replay regression coverage.

## Editor structure

`crates/editor/src/doc.rs` is the plain-Rust workspace/document model. `Doc<T: Content>` shares fighter/stage snapshot undo, dirty paths, validation and save behavior. Saves diff the typed saved baseline against the edit and patch only changed paths into the package's own JSON, preserving `extends` and untouched inherited values; arrays replace whole. The loader validates the proposed save before writing. Normal saves target user content; the `dev` feature exposes saving to built-in assets.

`lib.rs` owns editor state, commands and the generic state-bound plugin. `ui.rs` owns the shared frame, mode switch, shortcuts, toasts and shortcut sheet; `ui/dialogs.rs` owns dialogs. `form.rs` builds schema-driven forms for fields without a custom inspector (stage definitions use the `stage.<Name>` namespace). `style.rs` holds editor-only theme/constants.

| Shared / fighter | Stage |
| --- | --- |
| `pane.rs`: `Pane<H, S>` viewport state and common zoom/pan/drag interaction | `stage_port.rs`: stage viewport/toolbars using that interaction |
| `view.rs`: camera, coordinate transforms and fighter hit testing/overlay | `stage_view.rs`: generic `Shape`s, `Part`s, handles, snapping, mirror partners, deletion rules, notes and overlay |
| `section.rs`: JSON path mapping for dirty markers/problem navigation | `stage_ed.rs`: stage sections/actions, motion clock and live CPU sandbox |
| `preview.rs`: fighter subjects and real-sim frame history | `stage_panels.rs`: stage browser and inspector |
| `browser.rs`, `viewport.rs`, `timeline.rs`, `inspect.rs`: fighter panels | Geometry changes rebuild the preview world because surface/slot indices may change |
| `overlay.rs`: shared screen-sized knobs and dashed outlines | Used by both viewports |

## Checks and regression tools

Run commands from the repository root. `just` lists recipes.

| Command | What it checks |
| --- | --- |
| `just run` / `just release` / `just dev` | Play: debug, optimized (judge feel here), or debug with dynamic linking for fast rebuilds |
| `just fmt` | `cargo fmt --all` |
| `just check` | `fmt-check` (`cargo fmt --all -- --check`), `lint` (`cargo clippy --workspace --all-targets -- -D warnings`), then `test` (`cargo test --workspace`) |
| `just test` | All workspace tests; accepts cargo test arguments |
| `just traces [filter]` | `crates/sim/tests/ts_trace.rs`, optionally selecting fixture names by substring |
| `just bless` | `BLESS=1` content tests: regenerate `assets/schema/*.schema.json` and canonically format assets; still runs content assertions |
| `just shots [dir] [sections]` | Rendered screenshot tour; optional comma-separated section names, default directory `/tmp/stick-smash-shots` |
| `just verify-replays <dir>` | Headless re-simulation of `.replay.json` files; fails for desync or unfinished matches |
| `just readme-shots` | Regenerates `docs/images/*.webp` from named shots (`17a-match-citadel`, `05-select-ready`, `05c-stage-roulette`, `14-training`, `12-results`, `03b-editor-move`, `03q-stage-moving-sandbox`); needs ImageMagick 7 with WebP |
| `just balance [seeds] [fighter]` | Optional ignored release diagnostic in `crates/sim/tests/stages.rs`; recipe defaults to 16 seeds and every loaded fighter, or one fighter id |

### TS baseline

`crates/sim/tests/fixtures/ts/` contains recorded TS inputs and per-frame snapshots. `ts_trace.rs::matches_typescript_traces` explicitly sets:

```rust
d.global.input.buffer_frames = 0;
d.global.sim.separate_cosmetic_rng = false;
d.global.ai.thresholds.surface_chase_ticks = 0;
d.global.hitstun.ultimate = None; // plus the old hitlag values and tech.lockout = 0
```

Keep that setup current when adding an optional improvement. The comparison uses a `1e-6` tolerance for floating state and exact comparisons for discrete fields.

`just record-traces` runs `tools/ts-trace/record.mjs` against the read-only sibling `../stick-smash` with Node 26, without npm dependencies. It rebuilds the suite twice and requires identical bytes before writing fixtures and coverage/report files. `node tools/ts-trace/record.mjs --verify` checks without writing. See [the recorder guide](../tools/ts-trace/README.md). Recording is only for an intentional reference change; normal Rust checks use committed fixtures and need no TS checkout.

### Golden world hash

`crates/sim/tests/runtime.rs::cpu_match_golden_world_hash` runs a fixed-seed Brawler/Ninja CPU scenario for 900 ticks, injecting cosmetic range events with RNG separation off and draining FX/hits each tick. It hashes `format!("{w:?}")` with FNV-1a. This catches changes in world representation as well as gameplay/RNG order; it is distinct from replay JSON checksums. It loads the built-in assets and `World` stores content by index, so it also changes when gameplay values in `base`, `brawler`, `ninja`, `arena` or `global.json` change, or when a fighter sorting before `ninja` or a stage sorting before `arena` is added.

For an intentional change:

1. Run `cargo test -p sim --test runtime cpu_match_golden_world_hash -- --nocapture` and inspect the actual/expected hash difference alongside the code/content change.
2. Explain the intended behavior or snapshot change and add/run the relevant scenario tests and TS traces. Investigate an unexplained difference before updating anything.
3. Update the assertion's expected constant in `runtime.rs` deliberately; `just bless` does **not** update it. Check whether `SIM_VERSION` also needs a bump, then rerun the focused test and `just check`.

### Content, screens and editor

`crates/sim/tests/content.rs` checks loading inherited packages, exact built-in id lists, capacity, migrations, validation paths, hashing, schema freshness and canonical formatting. Change explicit roster/stage expectations when intentionally adding packages. Add invalid-input tests for new content constraints. `stage_geometry.rs`, `stages.rs`, `fighter_kits.rs`, `behavior.rs`, `gadgets.rs`, `armor.rs`, `buffer.rs` and `input.rs` cover focused simulation behavior; `stages.rs` also checks full-jump platform reach, ledge recovery and CPU matches.

`crates/game/src/main.rs::screens::select_and_stage_screens_own_their_resources` (`cargo test -p game select_and_stage_screens_own_their_resources`) is a headless screen smoke test: `MinimalPlugins` without rendering, audio or windows, exercising select/stage enter/exit ownership. It is not a full graphical app test. `select.rs` (lobby, pointers, carrying) and `stage_select.rs` (its own headless App lobby) have tests too, as do `ui.rs`, input, grids/pointers, hints, audio, stats, shots and replays.

`cargo test -p editor` runs document patch/undo/inheritance tests, preview simulation of every move/throw, coordinate and geometry manipulation tests, and `lib.rs::tests::every_screen_draws` (headless egui subjects/dialogs in both modes). Use these when changing editor behavior; use screenshots for actual rendered composition.

The screenshot tour lives in `crates/game/src/shots.rs`. Sections: `menus`, `fighter-editor`, `stage-editor`, `select`, `roster`, `match`, `results-replay`, `training`, `crouch`, `blast-zones`, `fighter-kits`, `stages`. The `crouch` section captures Mike standing, holding down, moving while crouched and standing again after release; `blast-zones` exposes all four KO boundaries with a wide camera and debug overlays off. Each section resets through title/settings/devices/editor/match state before its setup; the seed is fixed and settings are not persisted. Navigation uses scripted bot presses where appropriate. `roster` expands fighters/stages to 30; `results-replay` fast-forwards normal recorded ticks in bounded batches, then exercises playback/seek/return. Failures/timeouts name the section, remaining sections continue, and the process exits nonzero. Example: `just shots /tmp/stick-smash-shots select,results-replay`. Inspect the resulting PNGs; a successful capture alone does not establish visual quality.

Balance diagnostics print JSON damage, tip contacts, whiffs/punishes, KO moves, recovery deaths and unfinished matches, with seats swapped. They are opt-in and are not part of `just check`.

## Version boundaries

- **Simulation compatibility:** `crates/game/src/replay/model.rs::SIM_VERSION` is currently the package version (`CARGO_PKG_VERSION`, so a workspace version bump also invalidates replays) plus `-sim-31` (authored dodge / tech / getup windows and dodge staling, after `-sim-29`'s `hitstun.ultimate` launch model). Bump when simulation behavior or checksum representation changes: state transitions, physics, AI, RNG order, input derivation/buffering, snapshot fields or serialization. Replays embed their content and only check the format and `SIM_VERSION`, so a content-only balance edit leaves old recordings playable. Assess new content semantics that old embedded data will execute differently.
- **Content versions:** each content kind has its own `schemaVersion` history. `crates/sim/src/data/mod.rs::SCHEMA_VERSION` currently refers to stage version 2; fighter/global are version 1, with legacy version 2 accepted and normalized to 1. `data/load.rs::migrate_kind` dispatches by `ContentKind`, migrating stage v1 `ground` to v2 `blocks`; global files with the flat pre-window `dodge.rollInvuln` … / `tech.invuln` / `knockdown.getupInvuln` / ledge `inv` timings are rewritten in place to the window shape (no version bump; frame-exact). Do not migrate unrelated kinds when changing one format. Extend migration tests in `tests/content.rs`, regenerate schemas and update FORMAT.md when changing compatibility.
- **Replay container:** `replay/model.rs::FORMAT_VERSION` is 2, separate from `SIM_VERSION` and content versions. `storage.rs` writes a small JSON metadata header on the first line, then the replay payload: resolved content/package ids, match config, RLE raw inputs, input reset events and periodic/final FNV-1a checksums of drained serialized worlds. Change the format version when the recording contract changes and update storage/model validation and replay tests. Unsupported format/sim versions are reported explicitly.

## Common changes

### Add or replace a move on a fighter

1. Edit `assets/fighters/<id>/fighter.json`, under `moves.<slot>`. Reuse an existing slot from `crates/sim/src/data/moves.rs::Moves::SLOTS`; `sspec` is optional. Inherited moves can be thin object overrides, but phase arrays replace whole.
2. Author phases, hitboxes, events, transitions, charge and pose/attachment keys using [FORMAT.md](FORMAT.md#moves). Add referenced poses, attachments or projectiles in that fighter package as needed. This normally needs no Rust behavior change.
3. Add focused coverage in `crates/sim/tests/fighter_kits.rs` or the relevant behavior test. Preview the move in the editor with held-button/dummy options; use training for live interaction. Run `just bless`, relevant tests and `just check`; capture affected kits if presentation changed. TS traces run the built-in `brawler`, `king` and `swordfighter` on `arena`, so editing those (or `base`) can break `just traces`; `brawler` / `ninja` edits change the golden hash (see [Golden world hash](#golden-world-hash)).
4. If adding a genuinely new input slot: extend `Moves` fields, `get`/`get_mut` and `SLOTS` in `data/moves.rs` (`base` must supply it unless it's optional like `sspec`; `runtime.rs` asserts `Moves::iter()` order matches `SLOTS`); add selection in `crates/sim/src/moves.rs` (`special`, `ground_actions`) or `states.rs` (aerials, jumpsquat); `ai.rs` intents if CPUs should use it; editor `lib.rs::SLOTS` (browser order, `every_screen_draws`, the copy-move dialog), `browser.rs` groups and `preview.rs::AERIALS`; and FORMAT.md. Audit slot consumers with e.g. `rg '"dspec"'` rather than adding an id exception.

### Add a fighter

For imports from another game, follow [Character imports](CHARACTER_IMPORTS.md), the canonical research, conversion, authoring and validation workflow. Keep a [reference record](templates/FIGHTER_REFERENCE.md); [Mike](MIKE.md) is the worked example. The steps below cover package integration for any new fighter.

1. Add `assets/fighters/<id>/fighter.json` with `$schema: "../../schema/fighter.schema.json"`, `schemaVersion: 1`, a display name and `extends` pointing to `base` or a playable template. `abstract` is not inherited; omit it or use `false` for a playable package. The merged result must be complete: extending `base` supplies the shared fields, otherwise supply every required move/stat/body/visual field. The editor's New Fighter dialog can create the thin package in user content instead.
2. Customize stats, phases, poses, attachments, projectiles and AI data there. No central roster registration is needed: `load` discovers packages and sorts ids.
3. Update the expected fighter ids in `crates/sim/tests/content.rs::all_assets_load`. CPU match coverage uses hard-coded lists in `crates/sim/tests/stages.rs`: add the fighter to `roster_cpu_matchups` (and `new_fighter_cpu_matchups` if wanted). A new id shifts fighter indices and may change the golden hash (see [Golden world hash](#golden-world-hash)). Add relevant kit/behavior tests and, if useful, a kit capture in `crates/game/src/shots.rs`.
4. Run `just bless`, `just check`, editor previews and select/training screenshots. Optionally run `just balance 16 <id>` and review match completion/recovery as well as wins.

### Add a stage

1. Add a complete `assets/stages/<id>/stage.json` based on an existing stage: `$schema: "../../schema/stage.schema.json"`, `schemaVersion: 2`, blocks/platforms, bounds, ledges, art and spawn/respawn slots. Stages do not use fighter inheritance. The editor can copy a template, including unsaved edits.
2. Give it enough spawn **and** respawn points: built-in stages need eight (`content.rs` asserts `max_players() >= 8`); keep geometry legal under `crates/sim/src/data/load.rs::check_stage`. Optional waypoint motion is frame-based; optional `zoom` overrides presentation limits.
3. Update expected stage ids in `crates/sim/tests/content.rs::all_assets_load`. Every stage automatically runs `stages.rs`'s `platforms_are_in_full_jump_reach`, `every_ledge_is_recoverable`, `cpu_matches_finish_on_every_stage` (4×4-CPU and one 8-CPU match) and the matchup tests; add any stage-specific regression there or in `stage_geometry.rs`. A stage id sorting before `arena` changes the golden hash (see [Golden world hash](#golden-world-hash)). Stage screenshots use the hard-coded `STAGES` and parallel shot-name arrays in `shots.rs::stages` (plus `preserves_screenshot_names`).
4. Run `just bless`, `just check`, stage-editor preview/CPU sandbox and stage-select/gameplay captures. New geometry types also need sim collision/data/validation, surface-index ordering (blocks first, then platforms), `sim::data::camera::play_area`, drawing in `crates/render/src/stage.rs`, `crates/editor/src/stage_view.rs` `Part`, `shape`/`set_shape` and handle support, and the `stage_panels.rs` inspector.

### Add a move behavior or event kind

1. First check whether phase transitions and existing event fields can express it. For a reusable extension, edit `crates/sim/src/data/moves.rs` (`Phase`, `Event`, `Trigger`, or content `Fx`), `data/hitbox.rs` (hit `kind`) or `data/fighter.rs` (projectile data), with serde/schema docs and defaults where compatible. `Event` is a timed struct, not a single tagged kind enum.
2. Implement execution in `crates/sim/src/moves.rs`; route collision/state/projectile semantics through `combat.rs`, `states.rs`, `physics.rs` or `projectiles.rs` as applicable. Dispatch on typed data; never compare a fighter/move id to pick behavior. Update `world.rs` if the behavior needs persistent snapshot state.
3. Add constraints/references in `crates/sim/src/data/load.rs` (`check_phase`, `check_event`, `check_hitbox`, `check_fighter` as applicable) and negative content tests. Add focused simulation tests. For a new emitted presentation event, update `sim::world::Fx` and its consumers in `crates/render/src/fx.rs` / `crates/game/src/audio.rs` as needed.
4. Run `just bless` to regenerate schemas. Update `docs/FORMAT.md` with timing, ordering, defaults and interactions; extend custom editor controls (`inspect.rs`, `timeline.rs`, `viewport.rs`) only where schema forms are insufficient. Author an asset exercising the behavior.
5. Review content migration and `SIM_VERSION`, update the golden constant only for an understood change, and run `just check` plus relevant screenshots/optional diagnostics.

### Add a global tuning value

1. Add a typed, documented field in `crates/sim/src/data/global.rs`, or `data/render.rs` / `data/audio.rs` for presentation. Supply the value in `assets/global.json`; choose a serde default when old embedded/user data should remain readable. `global.render`/`global.audio` are embedded in replays: a new required presentation field breaks decoding old replays unless serde-defaulted or backfilled in `game/src/replay/content.rs::presentation`. Gameplay fields need a `SIM_VERSION` bump and change the golden hash (see [Golden world hash](#golden-world-hash)).
2. Run `just bless` to generate schema/format changes. Add range, finiteness and cross-field validation in `crates/sim/src/data/load.rs` (`check_global` or the relevant helper), with valid/invalid cases in `crates/sim/tests/content.rs`.
3. Read the field in the appropriate sim/render/game consumer, replacing the old tuning literal. Document units, defaults and semantics in `docs/FORMAT.md`.
4. Test the behavior, review trace compatibility and versioning, rerun `just bless` if types changed again, then `just check`.

### Add a screen

1. Add its module/plugin in `crates/game/src/<screen>.rs`; declare/register it in `main.rs`.
2. Add the `Screen` variant **and** a `Screen::ALL` entry (update the array length). The latter gives the screen entry input clearing in `ui.rs`.
3. Register enter/exit systems, state-gated updates and `UiSystems` ordering. Insert transient resources on enter, remove on exit; document any cross-screen lifetime, and remove resources that outlive the screen in the shots `Step::Reset` arm too. Build a `ui::rebuild` root keyed by visible state with stable focus keys, or explicitly use `DespawnOnExit` for another root pattern.
4. Connect navigation/transitions from existing screens and return paths. Extend headless ownership/transition tests in `main.rs::screens` as applicable.
5. Add a screenshot section/setup in `crates/game/src/shots.rs`, including `screen_is` support if needed (unlisted screens fall back to `Screen::Title`). Run `just check` and the new section; inspect the captures and test held-input transitions.

### Add a screenshot section

1. In `crates/game/src/shots.rs`, write a function returning `Vec<Step>` and register its name/function in `TOUR` (update the array length). `Section::new` supplies Home/Reset isolation; avoid relying on another section's state.
2. Use existing setup steps, `press` and bot input to exercise real navigation. Use `shot` for settled captures or `Step::Shot` for a deliberately timed frame. Give output stems unique, stable names. Use bounded `Until`/`Fast` steps for simulation conditions.
3. Extend `Step` and the `run` dispatcher only when existing steps cannot express the setup. Restore/reset any new mutable state through section isolation. Update `tests::preserves_screenshot_names` in the same file for added filenames, and update the section list here and in AGENTS.md when it changes. `just readme-shots` depends on specific shot names (see the commands table); rename those with care.
4. Each section has a 120 s timeout and fails if any `Step::Shot` PNG is missing. Run `just shots /tmp/stick-smash-shots <section>`, inspect every PNG, then run it alongside adjacent sections or the full tour to check isolation. Run relevant shots tests and `just check` after Rust changes.

## One-shot TS import

`just convert-ts` runs `tools/convert-ts/convert.mjs` and then `just bless`. It needs Node 26 and the sibling `../stick-smash`, imports actual TS modules without npm dependencies, and overwrites `assets/global.json`, `assets/stages/arena/stage.json` and `assets/fighters/{base,brawler,swordfighter,king}/fighter.json`. This is the original bootstrap tool, not the regular authoring workflow; rerunning can overwrite later Rust-side content work. See [the converter guide](../tools/convert-ts/README.md). The separate `tools/ts-trace` recorder reads TS to generate comparison fixtures, not game assets.

The `animations` screenshot section freezes Brawler, King and Whiplash at visible jumpsquat, takeoff, landing and dodge boundary ticks. Use it with `fighter-kits` for animation review: `just shots /tmp/animation-shots animations,fighter-kits`.
