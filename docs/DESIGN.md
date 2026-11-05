# Design decisions

Living record of decisions made before/while porting from the TS version (`../stick-smash`). Status: **decided**, **proposed**, or **open**. Contributor workflows live in [DEVELOPMENT.md](DEVELOPMENT.md).

Planned character architecture, generic rigs, and the editor's Character Creator workflow are tracked in
[Character system and creator plan](CHARACTER_SYSTEM_PLAN.md). These are future changes, not the current format.

## Status and next

The data pipeline, deterministic sim/TS baseline, renderer, local play and fighter/stage editors are implemented.
Local multiplayer polish comes first. Online play is planned later: the sim is already deterministic and
rollback-ready (`World: Clone` plus raw inputs), but nothing net-specific is built. **Proposed:** `ggrs` over
iroh datagrams; the networking stack is not a settled decision.

## Decided

### Scope
- **Source of truth.** The TS data is converted to JSON once (Node script that imports the TS modules). After that, this repo is the main version.
- **Players.** Support up to 8 fighters; stage tests exercise both four- and eight-player matches. No code may assume two fighters (the TS `const [a, b] = w.fighters` / `other()` patterns go away). Pairwise logic loops over all pairs; AI picks a target.
- **Teams.** Free-for-all or teams, chosen per match. Friendly fire is a match rule. Hit filtering in the sim checks team + rule.
- **Gameplay feel beats fidelity.** The TS version is the starting point, not the spec. The port first matches TS exactly (checked by replaying TS traces) as a verified baseline. After that, gameplay changes that make the game feel better are welcome. They're deliberate, configurable where sensible, and covered by their own tests (TS-trace tests run with those features off).
- **Input pipeline.** Raw input (sticks, button bits including a second-jump-control bit) → gestures (`input::History::derive`: thresholded directions, deadzoned analog `x`/`y`, flicks = reaching `flickThreshold` within `flickFrames` of the deadzone, taps = recent flicks, attack `Strike` source: button / attack stick / attack + special) → one buffered command → state handlers → accepted `Action` events (`world.acts`, drained each tick like `fx`). Movement, DI and holds always read live input; only presses, combination flags and press-time directions are buffered. Charges and jab chains are held by the control that started the move (`MoveState::stick`).
- **Input buffer.** Human presses (attack, special, jump, grab, shield press) pressed while unable to act become a single buffered command for `global.input.bufferFrames` (5 ticks, inclusive age; 0 = TS behavior). Presses within `comboFrames` combine (jump + attack = short-hop aerial, two jumps = short hop, attack + special = smash when that option is on); otherwise the newer press replaces the older. The command is re-delivered each actionable tick until a handler accepts it (records a command `Action`) or it expires; any accepted command clears the buffer, and a fresh press on an actionable tick supersedes (or combines with) it, so nothing replays twice. `Fighter::actionable` is the single rule for buffering, frame advantage and the editor: idle, air, shield without stun, ledge once the incremented timer reaches `minHang`, knockdown from `minFrames`, never in hitlag. During jumpsquat (with `jumpAerials`), attack picks a short-hop aerial and any jump press forces a short hop; both are consumed before buffering. CPU intents bypass the buffer and are digital (`Intent::digital`: full-tilt analog, a held direction counts as a flick). Sub-tick taps are latched by the game crate's input layer so they last at least one tick.
- **Feel features** (each off in the TS-trace harness): ground gait (`global.gait`, `world::Gait` on idle fighters: walk / dash / dash turn / run / turn / brake, with per-fighter lengths; a run turnaround flips facing at once and skids, so jumping out and attacking backward is a reverse aerial rush), up smash out of shield before shield-attack-grab (`shield.upSmash`), release-timed parry (`parry`: the first `window` frames of the shield-drop landing; checked after counters, before shields, for melee and projectiles), directional influence (`di`: a launch flags `Fighter::di`; the first update after the victim's hitlag rotates velocity toward the live stick by up to `maxAngle` × the perpendicular component, speed unchanged; not autolink, trips, windboxes or armored hits), a fast-fall buffer (`input.fastFallBuffer`), flick-only platform drops, ledge trump (`ledge.trump`: an occupant hanging at least `minHang` frames is pushed off, loses invulnerability and can't regrab for `regrabCooldown`; climbers can't be trumped) and fresh ledge invulnerability (`ledge.freshInvuln`: reset by landing, climbing up, being hit or respawning). SDI is not implemented.
- **Coordinates.** Keep TS conventions: stage pixels, y down, velocities in px/frame, 60 Hz, `f64` + `libm`. Convert to Bevy's y-up world only when rendering.
- **Units: 5 px per Ultimate unit, for everything.** One SSBU distance unit is 5 px on both axes, for geometry
  (hitboxes, figures, stages), speeds (units/frame → px/tick) and accelerations (units/frame² → px/tick²); launch
  speed is 0.15 px/tick per reference knockback unit (Ultimate's 0.03 × 5). Before this, hit geometry used 5,
  horizontal speeds 3.5 and gravity/fall speeds 6, so imports moved and fell at different scales from what they
  hit, and stage dimensions could not be taken from source data. Now:
  - Stages take source dimensions × 5: Arena is Ultimate Battlefield (main platform ±80 units → 800 px, blast ±240 /
    top 192 / bottom −140, camera ±170 / 130 / −80, all relative to the main floor and centre). Citadel, Isles and
    Foundry keep their geometry; their blast and camera bounds moved by the same ratios as Arena's (proportional,
    not source).
  - Imports multiply every source speed, acceleration and gravity by `tools/units.py` `SCALE` (5), and derive jump
    speeds from the source's full hop / short hop / air jump heights with `units.jumps`, exact for the sim's
    discrete integration, so a measured apex is the source height × 5.
  - Hand-authored content (originals, Mike, shared tuning) was migrated once by `just unify-5px`
    (`tools/rescale/unify-5px.json`): horizontal and `xy` speeds ×5/3.5, vertical ×5/6, and jump speeds (plus
    upward recovery impulses) ×√(5/6) so jump heights in px stay the same under the lower gravity. Fitted
    up-special rises inside the import generators follow the same rule (Mike's Aether too); a rise held every tick
    keeps its per-tick climb (v − g + g′).
  - New content: author in px at this scale. From a reference, multiply source units by 5; jumps come from heights,
    not from speeds. `xy` speeds (dodges, `travelSpeed`) are treated as horizontal-dominant.
  - Not migrated: legacy TS-trace fields (linear knockback, `hitstun.gravity`/`maxFall`, `perKnockback`,
    `tumbleThreshold`; TS traces load frozen content), visual particle speeds and render tuning.
- **Multiplayer.** Local multiplayer + CPUs first. Keep simulation state and inputs suitable for eventual rollback; see "Status and next" for the proposed networking stack.

**Launch model** (`hitstun.ultimate`, off in TS traces): a launch is a separate vector that loses a fixed
`launchDecay` per frame (Ultimate's linear decay, in reference knockback units) while the victim's own gravity,
fall speed and drift act on a second component, so horizontal travel grows with the square of knockback, spikes
are not fall-capped and floaty fighters stay up longer. It persists through hitstun and the air state after it
and is cleared by landing, ledges, air jumps and other states. Hitstun is 0.4 × reference knockback, tumble starts
above 80, hitlag is floor(damage × 0.65 + 6) capped at 30, and `tech.lockout` stops mashed techs. An optional
hitstun cancel exists but is off until verified. Launch speed (`launchSpeed`) was 0.14, raised under the old
multiplicative drag; the unit unification made it 0.15 (Ultimate's 0.03 × 5) and Arena's blast zones Battlefield's,
so KO percents now follow source launch speed and stage size rather than separate tuning.

**Ultimate calibration** (King, Duchess and Stratus are the calibration set): `tools/reference_ko.py` (`just
reference-ko`) computes Ultimate's launch for a hitbox on a Mario victim at Battlefield centre, and `just calibrate`
(`crates/sim/tests/calibration.rs`) measures the same moves and the movement stats in the engine and prints both
side by side. Gaps it exposed are closed in the engine, each data-selected and absent from the TS-trace content:
Ultimate's full hop **initial height** (`stats.jumpInitialHeight`, four no-gravity ticks; full hops were 5–7 frames
long), **upward-launch gravity** (`hitstun.ultimate.topLaunch`, the `damage_fly_top_*` params: 70–110° tumble
launches fall with a weight-based gravity capped at 1.8 units in hitstun), the **Sakurai ramp** (0° below 60 KB to
38° at 88, 38° airborne; was 0/40 by tumble and 45° airborne), **hitlag** multiplied before flooring and capping
(`hitlag.multBeforeCap`), and **weight-based fixed knockback** (`fkb`, WBKB; generators used to resolve it at weight
100). After them, every compared smash, tilt, aerial and throw matches the reference: hitstun and launch speed exactly,
KO percent within one point, full / short hop air times and heights, landing lag and autocancel windows exactly.
Not modelled (documented in FORMAT.md / the records): the first-10-frame Mario physics of weight-independent hits, the
vertical launch cap, balloon hitstun speed-up, grounded non-tumble sliding and the grounded spike bounce factor.

**Momentum model** (`movement.ultimate`, off in TS traces): speeds change by fixed amounts per frame rather than
by multipliers, as in Ultimate. On the ground every slowdown is the fighter's `traction` (default 0.5 = Mario's 0.1 × 5, doubled above walk
speed), so a released run skids to a stop in about 11–20 frames and running momentum carries into attacks,
jumpsquat, landing lag and shield. In the air, a neutral stick loses `airFriction` per frame (≈ 0.88 of air speed
kept after 15 frames instead of 0.54), speed above the held air speed decays toward it instead of clamping, and
attacks, specials, helpless fall and air dodges use the same rule with the fighter's own `airAccel` times a
per-kind multiplier, so starting an aerial no longer changes momentum and slow air fighters stay slow while
attacking. Fast fall reaches the fighter's fast-fall speed at once. Initial dashes are an authored burst above
run speed (imports use their reference values, originals 1.12 × run speed) that eases into the run by traction.
Legacy multipliers remain in the data for the TS traces; see FORMAT.md, Momentum.

The walk/run transition uses separate entry and release tilt thresholds (`gait.runThreshold` and
`walkThreshold`) so pushing farther works after either a slow tilt or a flick, and easing off restores
walking without oscillation. `gait.dashLock` applies command-specific initial-dash restrictions: the
buffer retains unavailable standing attacks/shield until legal or expired, while deliberate dash cancels
remain available. Normal attack after a dash has started selects dash attack, not a late smash inferred
from the dash's original flick.

**Gait lengths are per fighter** (`stats.turnFrames`, `brakeFrames`, `dashTurnFrames`; FORMAT.md, Gait frames).
Ultimate's ground statuses are shaped like ours already (a run turnaround flips facing at once, slides on traction,
then runs back; a run brake is a traction skid; a dash-back reverses at once), but their lengths come from each
fighter's scripts: `TurnRun` stops the slide on frame 11 for Peach, 17 for King Dedede, 15 for Cloud, against our
shared 4, and `RunBrakeR` stops on 11 against 5. Imports carry those frames (`tools/units.py` `gait`), originals keep
the shared 4 / 5 so their feel is unchanged. The turnaround frame matters most: once it passes, run acceleration
takes over from traction, and for most fighters it decelerates harder, so a short `turnFrames` reverses faster.
The brake length changes little physically (traction applies either way) but keeps the turn-back window open as
long as the source's `TURN_RUN` flag. A dash-back keeps its instant reversal and gains a `dashturn` window (2
frames, before `TurnDash` sets its turn flag) so the figure visibly turns instead of snapping.

**Defensive windows** (`dodge`, `tech`, `knockdown.getup` / `getupAttack`, ledge options; FORMAT.md): each dodge
has authored `invulnStart..=invulnEnd` intangibility with vulnerable startup and recovery, and repeated dodges
stale (`dodge.staling`). Before this, every dodge was invulnerable from its first frame through almost all of it,
which with the decaying launch model made defence too safe. TS traces pin the old timings.

### Deliberate divergences from Ultimate
Choices made on purpose; later tuning should not "fix" them back without a reason.
- **3-frame jumpsquat for everyone** (`movement.jumpsquat`): one jump timing to learn; weight and speed differ elsewhere.
- **5-frame input buffer** (`input.bufferFrames`, Ultimate 10): long enough for mashed inputs, short enough that stale presses don't fire.
- **12-frame tech window** (`tech.window`, Ultimate 11), with the Ultimate 40-frame lockout: a slightly kinder single press.
- **No SDI or LSI**: DI (`di`) is the only launch input; hitlag shakes are presentation.
- **Release-timed parry** (`parry`) with fixed defender / attacker freezes and a short invulnerability, not Ultimate's per-move parry frame data: one readable reward.
- **Flick-only platform drop**: holding down never drops through, so crouching and down tilts on platforms are safe.
- **Ledge trump** (`ledge.trump`) with our own push, cooldown and a minimum hang before an occupant can be trumped, so simultaneous grabs don't trade.
- **Float** (`stats.float`): holding jump at the top of a jump floats, as well as Ultimate's down + jump; no float
  cancel into fast fall (release jump first).
- **Held items** (`items`): one shared 22-frame throw (release on frame 6) for every direction and strength
  rather than per-item, per-direction animations; Z on the ground throws (by direction), Z in the air and shield +
  attack drop; dash throws are forward smash throws. Pickup is a 6-tick lag on the ground and instant in the air.
  No glide toss, item tossing out of a dodge roll, or attacking with a held item (battering items); a grounded
  down throw lands at once (the item loses its hitbox on the floor) instead of bouncing up. Down throws start at
  the projectile's fall cap. A thrown item that hits a shield rebounds like a hit.
- **Skids stay fully actionable**: run turnaround and run brake accept every idle action. Ultimate keeps jab, tilts,
  forward / down smash and turn dash locked during a run brake (King Dedede's `RunBrakeR` opens them on frame 15) and
  allows only jump, shield, grab, up smash and specials. A back flick during the brake turns the run around, never
  dashes back. A dash-back's turn window counts toward `dashFrames` (Ultimate's `TurnDash` runs 2 frames longer than
  `Dash`). Releasing the stick during a turnaround stands at its end rather than playing `TurnRunBrake`.

Calibration values set to Ultimate in the defensive-window pass (Ultimate frame = state clock + 1):
- Dodge windows: roll 30 frames, intangible 3–14 (Ultimate 4–15); spot dodge 26, 2–16 (3–17); neutral air dodge
  40, 2–28 (3–29); directional 44, 2–21 (20 frames from Ultimate frame 3). Tech and getups keep their lengths with
  2 startup frames.
- Dodge staling: a 90-tick window and three steps to 80% intangibility / 130% duration.
- Shield drop lag 11.

### Content format
- **Everything moddable is data:** fighters (stats, frame data, special-move behavior, visuals, hurtboxes) and stages. Core content uses the same format as user content, with no fighter- or stage-specific code.
- **Strict JSON** (`.json`), one schema per content type, generated from the Rust types with `schemars` and saved to `assets/schema/`. Each file points to its schema via `"$schema"`, so VS Code etc. gives autocomplete, validation and hover docs (from the `///` comments on the Rust fields).
  - Why not RON/YAML/TOML/JSON5: RON is Rust-only, YAML has parsing footguns, and TOML handles deeply nested arrays of hitboxes badly. Comments wouldn't survive an editor save anyway, so docs live in the schema instead.
  - The writer uses a compact pretty-printer: small objects and number arrays (one hitbox, one keyframe) stay on one line, so files are readable and diffs are clean.
- Every file has `"schemaVersion"`, versioned per content kind. The loader dispatches migrations by kind; see [FORMAT.md](FORMAT.md).
- **Packages are folders:** `fighters/<id>/fighter.json`, `stages/<id>/stage.json`, so images and sounds can live next to them later. Fighters support `"extends": "<id>"`; stages are complete documents. The fighter merge copies objects key by key and replaces arrays whole. `fighters/base` holds shared stats, grabs, throws, state poses and hurtboxes.
- Load order: built-in `assets/`, then a user content dir (platform data dir). Errors name the file and JSON path and skip that package; unusable globals or no playable fighter/stage fail loading.
- A content hash of loaded data is kept so online play can later check that both sides use the same data.

### Moves: phase timelines
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

### Held items
Items are an engine capability, not a fighter feature: a fighter package declares `items` (a projectile per throw
strength, throw velocities by direction, a lifetime, catchability), a move event's `item` puts one in hand, and
everything after that is shared (`sim::items`, FORMAT.md "Items"). Holding an item keeps every movement and defensive
option and specials; attacks and aerials become throws (the `toss` state), as in Ultimate. A thrown item is an
ordinary projectile tagged with the item, so hits, shields, counters, reflection and clashes reuse the projectile
rules; when it lands, connects or expires it becomes a loose world item (`World::items`) that falls, rests on
surfaces and can be picked up or caught by anyone. Damage per throw strength comes from the projectiles, with an
optional speed term (`speedDamage`) for Ultimate's speed-scaled item damage. Items live in fighter packages for
now (Duchess's turnip is the first); a match item mode would add global item definitions and spawning on top of
the same held / thrown / loose model.

### Visuals
- **Skeleton:** the current stick figure (head, shoulders, hips, elbows/hands, knees/feet, plus body rotation/pivot). Proportions are per fighter.
- **Poses:** named keyframe poses. `base` provides every state pose (idle, run cycle, jump, tumble, ledge, shield, knockdown…); fighters override what they need. Move phases animate by keyframing poses (with easing).
- **Attachments:** vector parts (line, circle, polygon, rounded rect) attached to a joint, with angle keyframes and an optional glow on active frames. This replaces the hard-coded sword/hammer/crown. Image attachments can come later.
- **Hurtboxes:** per fighter, with default and knockdown groups. Crouch and ledge groups are future work.

### Stages
`stage.json` (version 2): multiple static solid blocks with tapered undersides and independent top-corner ledges, pass-through platforms with optional frame-based waypoint motion, shared art and ledge zones, blast/camera bounds, and spawn/respawn points for up to 8 players. Version 1 ground migrates to a block with both ledges enabled. See FORMAT.md for motion timing and geometry constraints.
Built-in stage sizes follow the Units decision: Arena is Battlefield at 5 px/unit.

### Input
- Sim input = raw per-player state: quantized stick `(i8, i8)` + button bitflags. The sim derives presses, taps (smash input) and short hops from its own per-fighter input history, which is part of `World` so rollback stays correct.
- Keyboard maps directions to ±127. Gameplay derives digital directions through thresholds; select pointers use analog motion. Analog gameplay features (DI, walk speed) remain future work without requiring an input format change.
- Mixed devices in one match. Joining and fighter selection share one screen; an action press claims a slot, while back leaves (a mouse click can join the keyboard player). The keyboard can act as two devices (left/right halves). Rebinding per device; gamepads are first-class.
- **Devices** (`game::input`): `Keys` (whole keyboard), `KeysL`/`KeysR` (split, the default), each connected
  gamepad, and scripted `Bot`s (screenshot tour). Every device maps 10 actions (left, right, up, down, jump, attack,
  special, grab, shield, start) through a binding profile (`keyboard`, `keyboard-left`, `keyboard-right`,
  `gamepad:<name>#<session index>` overrides over shared `gamepad:<name>` defaults, falling back to the built-in defaults) to a sim `Input`, plus menu presses: confirm = attack or
  jump (and Enter), back = special (and Esc/Backspace), start, and repeating directions. Gamepad left stick is
  analog into the sim stick; the d-pad overrides it. Presses are latched until the next sim tick. Screen, pause/resume and ownership changes clear latches and
  edge history; held controls are suppressed until released. Split-keyboard rebinding refuses keys owned by
  the other active half; gamepad capture can be cancelled with Esc or B/Back. A device's selected layout supplies
  bindings and options before its profile/model overrides; built-in Tiltstick / Smashstick profiles retain device bindings and supply
  right-stick mode, AB smash, stick range and up-jump options. The same resolution feeds gameplay polling, menu/hints, the match slot's `PlayerConfig.controls`
  and its `tapJump`, so replays embed the effective options. A device with no explicit choice uses the built-in
  Smashstick profile; settings from before profiles use Custom when the device already has saved bindings/options.
  Legacy join-order up-jump is materialized once into each joining device’s Custom layout unless it already chose
  a profile. Editing any layout copies all effective options and bindings into Custom before making the edit.
- **Persistence** (`game::settings`): one JSON file in the platform config dir (settings, per-device bindings and
  options, named control layouts and each device's layout choice, last versus and training picks). Named layouts and
  layout choices are device-family scoped; a missing or mismatched named layout falls back to the device's own
  Custom layout without rewriting it. Built-in Smashstick/Tiltstick profiles supply documented defaults and keep
  the device's bindings. Deleting a named layout moves its devices back to Custom. User content loads from `<data dir>/stick-smash/content` after the built-in `assets/`;
  content errors are logged and shown as a toast.

### UI
- **bevy_ui** for menus, fighter/stage selection and HUD; directional menu focus and independent select pointers support gamepads.
- **egui** for the editor, debug overlays and training panels.
- **Focus navigation** is our own small layer over bevy_ui (`game::ui`), not `bevy_input_focus`: Bevy's
  `InputFocus` is a single global focus, and character select needs one cursor per player. Lineup changes are deferred until all devices finish navigation, then
  cursors are repaired. Lobbies allow eight slots; stages need enough spawn and respawn points to be playable;
  teams require two opposing sides. Unready disconnected slots leave; ready slots can be removed or reclaimed. Each focusable node
  has a stable `key` and an optional owner; screens are rebuilt from a plain model when it changes and focus
  survives by key. Directions move geometrically between nodes (last frame's layout, closest by distance +
  3 × sideways offset; sideways moves stay within 45°; vertical lists wrap). "Spinner" rows take left/right as
  value changes. Fighter/stage grids use `grid.rs` for row-aware digital navigation; each human also has a free
  pointer (`pointer.rs`) with analog acceleration/magnetism or direct mouse control. Chips ride on the pointers
  until dropped on a tile. Small tiles show their name only while pointed at or holding a chip; the grid fits the available space and roster size.
- **Selection.** Hover previews a fighter; confirm drops the chip and locks in. Free pointers can add CPUs,
  carry CPU chips to other tiles, change CPU level/team and adjust match rules. Confirming one's own chip picks
  it up (or starts once everyone is ready); special also picks it up. Grab/shield opens per-card team/layout
  options. Slim player cards use device-specific contextual hints (`hints.rs`) with join/idle/ready fades.
  Versus stage selection is a thumbnail grid with a large preview: each human drops a vote chip, then roulette
  chooses among the votes with vote-count weighting. Stages too small for the lineup are dimmed. Back takes a
  vote back first; with no vote, it returns to fighter selection with picks locked in. A disconnect also returns
  there, so its reclaim flow handles disconnected devices.
- **Look.** Barlow Condensed Black Italic for the wordmark, percents and big moments, Barlow Condensed for menu
  rows, Barlow for help text (OFL, `assets/fonts/`, with a system sans fallback for missing glyphs). Ink-navy
  translucent panels over the live stage, the 8-color palette as the accent strip, gold for focus. The title
  menus play a CPU attract match behind them; character select's shared `showcase.rs` keeps fighters in card
  order, swaps changed picks in place, and plays a lock-in flourish before the ready win pose. UI is laid out for
  720 logical pixels high and scaled with the window.
- **Flow.** Title → Versus → join + character select → stage vote/roulette → match → results (rematch, character
  select, main menu, replay). The end banner ("GAME!" / "TIME!") and the results handoff run on a real-time clock
  (`render.hud.endBanner`): the banner slams in, holds and fades out over `secs`, then the results enter, so the end
  freeze (`match.endFrames`) only paces the winner's pose and can neither stretch nor skip the banner. The results
  enter with a slide/scale and a darkening backdrop (`render.hud.resultsEnter`), and their buttons need a short hold
  (`render.hud.resultsHoldSecs`), so a press buffered from the match picks nothing. Training goes from its select
  screen directly to the match. Pause from any player's Start; the pausing device drives the pause menu, with
  ownership transferred on disconnect. Training uses the same select screen (one player + a dummy card) and an egui
  panel; dummy behavior and damage are also in the pause menu for gamepad players.
- **Teams in the renderer.** Each match gets a copy of `GameData` with the palette reordered per player (team
  color in team matches). The renderer and HUD color player *i* with palette *i*, so no render changes were
  needed. The content hash is unaffected (computed at load).
- **Match stats** (results screen) are game-side: every damaging entry in the sim's per-victim `World::hits` records credits damage (shield hits are excluded); a stock lost within 8 s of a hit is a KO for `last_hit_by`, otherwise a self-destruct. Hit records drain each tick alongside effects.
  Placement: the winner (and teammates) first, then by elimination time.
- **HUD safe area.** The game measures the HUD panels after layout and passes the reserved top and bottom
  fractions to the renderer (`Scene::top_safe_area` / `bottom_safe_area`). The camera fits the remaining
  rectangle; tags share its world mapping and offscreen bubbles stop above the panels.
- **Presentation tuning** (damage color ramp, hit shake, end-of-match slow motion, the end banner's pace, the
  results entrance and button hold) lives in `global.render.hud`; the sim never reads it. Name-tag and offscreen-bubble placement plus HUD text pops are
  tuned separately in `render.hud.look`.
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
`sim` (logic + data types/loading) · `render` (fighter/stage/fx drawing, shared by gameplay and editor) · `editor` (lib: fighter and stage editors, used by `game`) · `game` (bin: app, input, menus, modes, audio and replays). No networking crate yet.

### Editor
A fighter and stage editor reached from the title menu (**keyboard and mouse only**). Priorities: fast iteration, clarity,
feel. Its own design, not a port of the TS panels.

- **Layout** (egui, Barlow fonts, the ink-navy/gold palette): a top bar (Fighters/Stages switch, picker with dirty dots, undo/redo,
  save, test, validation status, back), a left **browser** (moves grouped ground/smash/air/special/grab, throws,
  poses, fighter stats), the center **viewport** (the real renderer through `render::Scene` with a fixed view), a
  right **inspector** for the selection, and a bottom **timeline** in fighter mode. Stage mode shares the frame with its own browser, geometry/art inspector and motion controls.
- **Truthful preview.** A sandbox `World` (training rules, stage `arena` or the first stage with enough slots) runs the real sim: the
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
  When the playhead shows an editable pose key (not an interpolated blend), the joints get handles and can be dragged (named poses are shared; the
  inspector says how many keys use it). Wheel zooms at the cursor, right/middle drag pans, F frames the fighter.
- **Shortcuts.** Space play · ←/→ (or , .) step · Home/End · [ ] previous/next phase · ⌘/Ctrl Z, Shift Z / Y
  redo · ⌘/Ctrl S save · ⌘/Ctrl D duplicate hitbox · N new hitbox · Delete removes it · Tab cycles hitboxes ·
  Esc deselects, then leaves.
- **Model** (`editor::doc`, plain Rust, unit-tested): `Doc<T: Content>` for fighters and stages, with the typed content being
  edited, the snapshot it was loaded/saved as, snapshot undo/redo (continuous drags and typing coalesce into one
  step), and the package's own raw JSON.
- **Save and `extends`.** The file written is the package's *own* document, not the merged fighter. On save the
  editor diffs the typed fighter (serialized) against its last-saved snapshot (serialized the same way) and patches
  only the changed paths into the raw document: objects recurse key by key, anything else (arrays included, as in
  the merge rule) is written whole, and removed keys become `null` when the parent still has them. Untouched values,
  `extends`, `$schema` and inherited data stay as they were, so a child stays a thin override; editing an inherited
  move writes that move's override into the child. Before writing, the real loader (`sim::data::load`) runs over all
  content with the new document in place; any error for the package refuses the save and lists file + JSON path.
  Files go to the user content dir (`<data dir>/stick-smash/content/{fighters,stages}/<id>/{fighter,stage}.json`), which overrides
  the built-in file; `--features dev` adds a toggle to write into `assets/` instead. Output uses
  `to_compact_string`. Abstract parents (`base`) aren't listed for editing.
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
  as a whole. The sandbox World is rebuilt on any geometry or slot change: CPU play restarts at frame zero;
  without CPUs, the motion preview retains its clock. Format-1 documents
  are migrated before they become the save baseline, so saves never mix `ground` with `blocks`. The inspector is hand-made for geometry and uses schema forms
  (with colour pickers) for art and the backdrop. Problems are the loader's `check_stage`; notes flag legal but
  likely-wrong setups (fewer than 8 spawns, spawns in the air, geometry past the blast zone, KOs off camera). New
  stages copy a template stage (unsaved edits included) as a whole file; only never-saved stages can change id.
  Test plays training or a 3-stock match vs a CPU on the stage; results and pause return to the editor.
### Rendering
- **Vector drawing** is our own small tessellator (`render::canvas`): polylines, circles, sectors, rounded rects
  and ear-clipped polygons into one vertex-colored triangle mesh per layer (backdrop, stars, stage, world,
  debug, editor overlay), MSAA ×4. Dynamic layers rebuild each frame; backdrop/static stage geometry is cached
  until the stage or content changes (moving platforms rebuild the stage layer). No gizmos (they can't fill), no lyon dependency.
- **Interpolation.** The game pushes a `World` snapshot to `render::Scene` every tick and sets `alpha` (fraction
  of a tick since then) every frame. Fighter positions and skeletons, projectiles, particles and the camera
  interpolate between the last two ticks. Fighters snap when their state, facing or position jumps.
- **Frame pacing.** Rendering runs on the main thread (no pipelined rendering) with one frame in flight. When the
  display refreshes at about 60 Hz (median of recent frame intervals within 58–62 Hz) and the "Lock sim to 60 Hz
  display" setting is on, the sim steps once per rendered frame instead of through `Time<Fixed>`, and draws the
  newest tick (`alpha` 1 at normal speed), so vsync can't alias into 0- or 2-tick frames. Other refresh rates and
  the screenshot tours keep the fixed-step accumulator.
- **Interpolation gaps.** A fighter or projectile that moved further in one tick than
  `render.interpolate.fighterJump` / `projectileJump` stage pixels is treated as teleported and drawn at its
  new position instead of sliding across.
- **Effects** are render-side: `Fx` events become particles/rings/beams with a non-sim RNG, advanced per tick.
- **Blending.** The GPU blends in linear light; the art was tuned for sRGB canvas blending, so translucent alpha
  is raised to `render.alphaGamma` (1.6) on output. Faint glows/shadows then look like the TS version.
- **Cosmetic tuning** lives in `global.render` (camera, overlays, effect looks) and stage `art`. It is part of
  the content hash; harmless, since it's loaded content either way.

### Camera
Tuning is in `global.json` → `render.camera`. Geometry-derived zoom limits scale with stage size and platform
paths (`sim::data::camera`); optional stage `zoom.minHeight` / `maxHeight` override them, capped by camera bounds.
Each tick: frame every fighter that isn't dead/out (feet positions, clamped to the blast zone ±
`blastMargin`), pad by `padX`/`padTop`/`padBottom`, pull the center `stageBias` toward the stage's center, fit the
gameplay area's aspect ratio, clamp the visible height to the resolved zoom limits, and clamp the view inside
the stage's `camera` bounds. The camera eases toward that target by `follow` (position) and `zoomFollow` (zoom)
per pushed tick; its unshaken framing is a function of the world history and the gameplay area's aspect ratio. UI safe areas reserve space above/below gameplay.
Shake comes from `Fx::Shake`, decays by `shakeDecay` per tick and offsets the drawn view with render-side randomness each frame. The backdrop gets a little parallax
(`art.background.parallax`).

All fighters share the Ultimate-style damage/percent/weight launch formula (`FORMAT.md`, Knockback models).
Imported source BKB/growth and estimated original-roster values use the same reference units. Original kits
were fitted to their previous uncharged middleweight launches; charge and weight responses now follow the
shared equation. Fixed launches, autolink pulls and windbox forces remain distinct mechanics. The former
linear formula is retained only for TS compatibility fixtures.
