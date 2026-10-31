# Content format

Content is strict JSON. The Rust types in `crates/sim/src/data/` define it; `assets/schema/*.schema.json` is
generated from them (`BLESS=1 cargo test -p sim --test content` rewrites schemas and reformats assets). Field
docs live on the Rust types and show up as editor hover docs through the schemas.

```
assets/
  global.json                  shared tuning (global.schema.json)
  stages/<id>/stage.json       stage geometry (stage.schema.json)
  fighters/<id>/fighter.json   fighters (fighter.schema.json)
  fighters/base/fighter.json   abstract parent: stats, grabs, throws, hurtboxes, poses, state anims
  schema/                      generated
```

Every file has `"$schema"` (relative path to its schema) and `"schemaVersion"`. The loader migrates older
versions forward and rejects newer ones.

**Packages.** A fighter with `"extends": "<id>"` is merged over its parent before it's read: objects key by key,
arrays replaced whole, `null` deletes a key. `"abstract": true` marks a parent-only package (not playable, not
inherited). Files from the user content dir replace built-in files with the same path. A broken package is
skipped with an error naming the file and JSON path.

**Formatting.** Files are written by `data::json::to_compact_string`: 2-space indent, and any object or array that
fits in 120 columns and contains no objects stays on one line (a hitbox, a keyframe).

## Coordinates and units

- World/stage: stage pixels, **y down**, 60 ticks per second, speeds in px/tick.
- Fighter-local (hitboxes, hurtboxes, poses, events, projectile spawn): **x forward, y up**, origin at the feet.
  The sim mirrors x by facing and flips y.
- Angles in degrees: 0 = forward, 90 = up. This holds for launch angles, attachment angles and pose `rot`
  (positive `rot` pitches the body forward).
- Event velocities (`setVel`, `addVel`) are fighter-local too: `{"y": 17.5}` launches upward.

## Moves

Moves sit in fixed slots (`jab` … `dspec`, optional `sspec`). A move is a list of **phases**, starting with the
first. Behavior comes only from phase data, never from slot or fighter ids. The exceptions are input routing (which
slot an input starts) and the once-per-airtime limit on `uspec`.

### Frame convention

A phase's **frame 0 is the tick it's entered**. The move's first phase is entered on the tick the move starts.
Every later tick advances the phase frame by one. With `frames: N`, the phase times out on its frame N, and the
next phase's frame 0 is that same tick. A one-phase move with `frames: N` lasts exactly like the TS
"last active frame + endLag = N" move: hitbox `start`/`end` values carry over unchanged.

### Per-tick order (fighter in a move)

1. **Charge.** If the phase has `charge`, its frame equals `charge.frame`, the button is held and charge < `max`:
   charge +1, and skip steps 2 to 4 this tick. Movement still runs.
2. **B-reverse.** For a `bReverse` move, before the window closes: holding the other direction turns the fighter
   (once). In the air, horizontal speed flips too.
3. **Advance** the phase frame.
4. **Events** matching this frame, in list order (see below).
5. **Step transitions**, in list order, first match wins: `timeout`, `grounded`, `airborne`, `falling`,
   `release`, `shieldPress`. A phase with `frames` and no `timeout` transition ends the move on timeout.
6. **Movement** with the current phase (after any transition). Grounded: attack friction (`slide` frames use slide
   friction), gravity, ground snap with edge stop. Airborne: drift with the move's `air` accel × `airAccelMult`,
   fast-fall if `air: "aerial"`, gravity capped by `fallCap` (else fast-fall or fall speed), `hover` zeroes vertical
   speed, integrate. Landing fires `land` transitions. Ledges can be grabbed from phase frame `ledgeGrab` on, even
   while rising.
7. **Hit pass** (after every fighter has stepped): hitboxes of the current phase active on its current frame.
   A connecting hit fires the attacker's `hit` transitions. A hit taken inside a `counter` window is countered
   instead and fires the defender's `counter` transition.

**Entering a phase** (any transition): phase frame = 0, the "already hit" list is cleared (the new phase can hit
everyone again), and `at: 0` events run immediately.

**Ending tick.** When a transition ends the move during the step (step 5), that tick's movement (step 6) still uses
the phase that just ended, then the fighter is in its new state. This matches the TS version.

**Endings** (transition `to`): `end` goes to idle on the ground or air in the air. `helpless` goes to idle on the
ground or helpless in the air. `landing` goes to landing lag for `lag` frames. Anything else names a phase.

### Events

`at: N` fires on frame N. `from`/`to` (inclusive) fire on a range. `from` alone fires from that frame on, and
neither fires on every frame from 1. Frame-0 events must use `at: 0`. `every: K` also requires the world frame to
be divisible by K. `if` restricts to `grounded`, `airborne` or `countered`. One event's actions run in this order:
`mulVel`, `setVel`, `addVel`, `capFall`, `leaveGround`, `invuln`, `spawn`, `sfx`, `fx`. Random ranges (`[a, b]`)
in effects draw from the world RNG in field order (x, y, vx, vy, life, size), so effects affect determinism.

### Transitions

`{ "on": <trigger>, "if": <cond>, "from": <min phase frame>, "to": <phase|end|helpless|landing>, "lag": <frames> }`.
Triggers: `timeout`, `land`, `grounded`, `airborne`, `falling` (vertical speed ≥ 0, y down), `release` (charge
button, else attack, not held), `shieldPress`, `counter`, `hit`.

### Counter

A phase with `counter` counters hits received on frames `from..=to`. On a melee hit: the attacker freezes
`attackerFreeze` frames, the defender turns to face them, gains `invuln`, and stores counter damage =
max(`minDamage`, incoming × `damageMult`). A projectile is reflected instead (speed × `reflectSpeed`, life =
`reflectLife`, vertical speed = the projectile's `reflectVy` if set) and stores 0 damage. Either way the
`counter` transition fires. Hitboxes with `counterDamage` use the stored damage and are inactive when it's 0. The
`countered` condition is true when it's above 0.

## Projectiles

Defined per fighter under `projectiles.<id>` and spawned by a `spawn` event. Each tick a projectile moves by its
velocity and loses 1 life. With `gravity`/`maxFall` it falls. With `bounce` it bounces on surfaces and the
stage's sides. With `bat`, attacks knock it back and the attacker becomes its owner. Projectiles of different
owners destroy each other on contact. `trail` events run every tick in projectile-local space (x along its travel
direction).

## Visuals

- **Poses** (`poses.<name>`): joints `head, shoulder, hip, elbowF, handF, elbowB, handB, kneeF, footF, kneeB,
  footB`, plus `rot`, `pivot` (rotation center height) and `offsetY`. Missing joints come from `from` or `stand`.
- **Animations**: a list of keys `{ "at": t, "pose": <name or inline pose>, "lerp": bool, "if": cond }`. A key
  holds until the next one. With `lerp` the figure blends from the previous key. A key with `if` (`grounded`,
  `airborne`, `countered`) is skipped while the condition is false. Move phases animate in phase frames
  (`anim`). State animations (`anims.<name>`) pick a `clock`: `state` (frames in state), `world`, `run` (run
  cycle degrees), or `spin` (tumble spin degrees, signed), and may `loop`. Exceptions: `air.flip` counts frames
  since the flip started, `pummel` frames since the pummel started. `idle` above `render.overlays.runSpeed`
  shows `run`.
- **State animation names**: `idle`, `run`, `jumpsquat`, `landing`, `air.rise`, `air.fall`, `air.flip`, `helpless`,
  `shield`, `shieldDrop`, `roll`, `spot`, `airdodge`, `hold`, `pummel`, `throw.f`, `throw.b`, `throw.u`,
  `throw.d`, `grabbed`, `hitstun`, `tumble`, `shieldbreak`, `knockdown`, `getup`, `getupAttack`, `tech`, `ledge`,
  `climb`, `ledgeAttack`, `ledgeRoll`, `ledgeJump`, `dizzy`, `win`, `respawn`.
- **Attachments**: vector `shapes` (line, circle, poly, rect) hung on a joint at an angle. On a hand the angle is
  relative to the forearm. A phase's `attach.<id>` can key `angle`, `offset` and `scale`, and `show` a hidden
  attachment. While a phase keys the angle, the angle is absolute, `hold` poses the front arm to grip it, and a
  `trail` sweeps a ring sector around the shoulder over the last `render.overlays.trail.frames` ticks of each
  hitbox window. `glow` swaps shapes marked `when: "glow"` / `"noGlow"`. `sway` swings the angle ±`deg` over
  `period` world ticks. Shapes: `line`, `circle`, `poly` (any simple polygon), `rect`, `star` (four-pointed,
  optional `blink` period). Colors are `#rrggbb` or `player` / `playerDark` / `playerLight`.
- **Style**: limb/torso/glow widths, head radius, `headRing` width, `headFill`, `eye` rect from the head center.
  Procedural overlays (hit flash, charge blink/jitter, dodge ghosts, helpless grey, respawn blink, shield bubble,
  dizzy stars, X eyes, respawn platform) are drawn by the renderer for every fighter, tuned in
  `global.render.overlays`.
- **Stage art**: ground gradient, underside, edge strip (`edgeWidth`), `stripes`, platforms (`platformThickness`,
  `platformRadius`, `platformShadowBand: [offset, height, alpha]`) and an optional procedural `background` (sky
  gradient stops, moon with halo, ridges, twinkling starfield, `parallax`) filling the stage's `camera` bounds.

## Rendering tuning

`global.render`: `alphaGamma` (translucency correction, see DESIGN.md), `camera` (follow camera framing, zoom
limits, smoothing, shake decay; see DESIGN.md "Camera"), `overlays` (procedural fighter overlays), `fx` (particle
drag/shrink, ring width, dust and KO-beam looks). Cosmetic only: the sim never reads it.

## Simulation tuning and CPU data

`global.input.stickThreshold` is the inclusive digital stick threshold (1..127, currently 64).
`global.input.bufferFrames` is the human press buffer lifetime in ticks (u32, currently 5); age equal to
the lifetime is still live, and 0 disables buffering for TS fidelity. Press-time directions and taps are
replayed once at the next buffer-actionable update, while held buttons use current input (see DESIGN.md).
`global.sim` holds the reference simulation's shared state friction, collision padding/tolerance, push height,
tech landing speed, knockdown drag, flip duration, ledge offsets/speeds/cooldown, grab hold/release offsets and
speeds, throw motion dimensions, platform-drop offset, maximum percent, and projectile collision padding.
`hitstunParticleSpeed` controls the cosmetic trail threshold. Velocity magnitudes in this block are positive;
state logic supplies direction. `releaseVx/Vy` are an interrupted hold's victim velocity; `escape*` are the
hold-timer escape velocities. The remaining cosmetic event sizes/timings may be constants in the sim.

`global.ai` contains three-entry difficulty tables (levels 0..2): `react` is action cooldown, `aggression` is
attack attempt probability, `mash` and `tech` are per-tick chances, `shieldResponse` is out-of-shield response,
`defend`/`projectileDefend` are defensive response chances, and `charge` is the random smash-charge upper bound.
`initialCool` initializes the action cooldown. Its typed `thresholds` object exposes geometry, probabilities,
and timing for the shared decision tree. Probability cutoffs in a branch are cumulative (e.g. ledge jump,
attack, shield, then climb); wait/hold ranges sample a uniform value then floor it. Heights/distances are pixels,
velocities pixels/tick, durations ticks. These fields preserve the TS branch order and conditional RNG calls.

Each fighter's `ai` contains:

- `reach`: close-range attack distance.
- `counterChance`: chance of countering instead of shielding; 0 skips this random draw.
- `recoveryShieldCancel`: optional phase name in `uspec` where an offstage recovery can shield-cancel.
  Recovery steering is selected by the move's `air: "upSpecial"`, independent of its identity.
- `specials`: ordered special tactics. Each requires distance strictly above `minDistance`, strictly below
  optional `maxDistance`, vertical separation strictly below `height`, optionally `grounded`, optionally
  `noProjectile` owned by this CPU. Only then is `chance` sampled. Success presses special toward the target;
  `down` adds down, and optional `hold: [min,max]` samples/floors special-button hold duration. A failed chance
  continues to the next tactic, preserving random draw order. Input routing chooses the move slot.

A projectile's `name` is its training hit label. The first two arena respawn points both use the reference
center position; additional player slots have their own points.

## Match and input snapshots

`World::new(MatchConfig, &GameData) -> Result<World, String>` resolves stage/fighter ids into content indices.
It returns an error for unknown ids, zero players, nonpositive stocks, or too many players for the stage.
`Stage::max_players()` is `min(spawns.len(), respawns.len())`; lobbies use it to size player slots and filter stages.
`MatchConfig::opposing_sides()` counts distinct teams plus each unteamed player separately; lobbies should
require at least two sides. The constructor permits one side for training and tests. Player config contains
`fighter`, optional CPU level, optional team, and `tap_jump`; match config also contains seed, stocks, training,
and friendly fire. `None` teams are free-for-all. Friendly fire permits teammate hit/grab/projectile interaction;
AI still targets enemies. Victory is evaluated every tick when at most one side has stocks left, even without
a prior elimination. Winner is the last surviving team (or individual in free-for-all); all surviving
winning teammates enter the win state. Simultaneous elimination is a draw. Training replenishes stocks and
never ends. Once the result is committed and `end_t` starts, KO processing is disabled (also after `over`);
movement, combat, and effects continue, but stocks cannot change during the ending animation. Pair passes run in player-index order, and overlapping push pairs run in lexicographic order.

Raw `Input` is `(i8,i8)` stick (y up) and button bits jump=1, attack=2, special=4, grab=8, shield=16. Human
history advances during countdown, hitlag, respawn/death, and match ending. Rising direction edges update
last-press ticks before taps are computed, inclusively through `movement.tapWindow`. CPU intents bypass raw
history. AI selects the nearest living enemy by Euclidean distance, with lowest index breaking ties.

The ending-tick rule above is authoritative over the converter README's older approximation note. A step
ending retains that phase's movement for the tick; a phase-to-phase transition immediately uses the new phase.
`land`, `hit`, and `counter` are event-driven transitions; the other triggers are checked once in list order
during the step. Entering a phase runs only its frame-0 events that tick, without recursively checking its
step transitions. Charge freezes phase progression while movement continues. Multi-hit cooldowns and already-hit
sets reset on phase entry. Counter damage and accumulated charge persist across phase transitions.

For multiple victims in one hit pass, an immediate `hit` transition changes the boxes used for subsequent
victims in index order. A successful grab leaves the attack state, so it takes only the first eligible victim.


`World::hits: Vec<HitRecord>` records every damaging hit, shield hit, throw, pummel, and projectile hit in
simulation order. `HitRecord` contains `attacker: Option<usize>`, `victim: usize`, `damage: f64` (the attack's
nominal damage), and `shielded: bool`. The game drains it each tick alongside `World::fx`; both queues are
part of the rollback snapshot. `Fighter::last_hit_by: Option<usize>` remembers the latest damaging attacker
across state changes. Shield hits do not change it. The single training trace still describes only the latest hit.

Spawn `y` is the feet height (default: highest block top below `x`; required outside all blocks). A fighter starts grounded on the block or platform whose
height matches that spawn and whose horizontal span contains it; otherwise it starts airborne.
Hurtbox groups are `default` and `knockdown`. Autolink pulls toward the victim's `default` hurtbox center.

Content validation rejects nonpositive movement speeds, gravity, fall speeds, weight and collision radii,
negative magnitudes/timings, reversed hurtbox/stage bounds and frame windows, zero divisors/event periods,
and invalid content references. Integer frame counts are unsigned. Throws require `0 < release <= total`;
ledge options require `0 < a < b <= end` (jump allows `a == b`). Ground `underDepth` and `underInset` and
respawn `descendFrames` must be positive. Projectile lifetimes must fit a positive `i32`. Throw/projectile
hitbox geometry is ignored; only projectile `r` and active melee hitbox radii must be positive.
Global validation errors abort loading; invalid fighter/stage packages are skipped with a file and JSON path.
On blocks narrower than twice the AI target padding, that padding is reduced to half the block width.

## Procedural audio (`global.audio`)

The game synthesizes the `sounds` map at startup. Move events still use `"sfx": "slash"` (or any sound name
in that map); no file paths are required. The sim only emits events and never reads audio tuning.

Each sound has `volume` (0–1), `priority` (higher wins contention), `cooldown` (real seconds),
`strengthGain` (weakest impact gain relative to strongest), `strengthDuration` (strongest duration multiplier),
and `layers`. A layer specifies `wave` (`sine`, `triangle`, `square`, `saw`, `noise`), `from`/`to` frequency in Hz,
`duration`/`attack` in seconds, `decay` (exponential envelope factor) and `gain`. Noise uses the frequency sweep
as a low-pass cutoff. Layers mix through soft clipping with a click-free attack/release.

For example, lower `sounds.hit.layers[1].to` for a deeper thump, raise the noise layer's `gain` for more crack,
or increase `sounds.hit.strengthDuration` for heavier high-knockback hits. `strengthMax` is the knockback
(shield hits: damage) reaching full strength; `strengthLevels` controls how many variants are cached.
`pitchVariation` adds small game-side random pitch changes without touching sim RNG. UI names are `menu`,
`confirm`, `back`, `join`, `ready`; countdown uses `beep` and `go`. Unknown names warn once and remain silent.

`defaults` supplies master/SFX/music volumes until overridden by saved settings. Music is reserved and silent.
`volumeStep` sets slider increments. `maxVoices`, `maxPerFrame`, and global/per-sound `cooldown` bound polyphony
and fast-forward repetition. `pan` controls stereo width and `depth` the spatial emitter depth and `earGap` the listener separation; set `pan` to 0
for centered audio. `sampleRate` controls synthesis quality/memory. See the generated global schema for units
and supported ranges. Restart after tuning the bank, and run `just bless` when changing Rust data types.


## Stage geometry (schemaVersion 2)

`blocks` replaces the single `ground`. Version 1 stage files migrate automatically, with both corners enabled.
Each static solid has `left`, `right`, `top`, `bottom`, `underDepth`, `underInset`, `leftLedge`, and `rightLedge`.
The underside slopes inward below `bottom`, exactly as in version 1. Both underside dimensions must be positive;
`underInset` cannot exceed half the width. Blocks' bounding rectangles including their undersides must not
intersect (touching is allowed). Build a raised centre or wall with adjacent blocks of different heights.
Only exposed top corners can be grabbable; disable flags at seams or corners covered by another block.
The hanging body must also clear neighbouring solids for every loaded fighter.
Setting a corner flag false makes that edge a walk-off. Ledge grab-zone
settings remain shared in `ledge`; occupancy is independent for each block and side. All blocks share `art`.

Pass-through `platforms` keep `x1`, `x2`, and `y`, and optionally add `motion`:

```json
{
  "schemaVersion": 2,
  "blocks": [
    { "left": 300, "right": 980, "top": 520, "bottom": 580,
      "underDepth": 100, "underInset": 60, "leftLedge": true, "rightLedge": false }
  ],
  "platforms": [
    { "x1": 390, "x2": 570, "y": 400,
      "motion": { "mode": "pingPong", "phase": 0, "waypoints": [
        { "x": 0, "y": 0, "travelTicks": 120, "waitTicks": 30 },
        { "x": 240, "y": -80, "travelTicks": 120, "waitTicks": 30 }
      ] } }
  ]
}
```

Waypoint `x`/`y` are **offsets** added to the platform's base coordinates, not absolute stage positions.
At least two waypoints are required. A waypoint waits `waitTicks` (default 0), then travels linearly to the next waypoint.
Used `travelTicks` durations must be positive. `loop` travels from the last point back to the first;
`pingPong` reverses at endpoints. Leg j ↔ j+1 uses waypoint j's `travelTicks` in both directions;
the last waypoint's `travelTicks` is ignored in this mode and may be zero.
Endpoint waits occur once per visit. `phase` (default 0) advances the cycle in ticks at world frame zero.
Motion is a pure function of the world frame, including countdown frames. Standing fighters receive the
platform's position delta once per tick, including during hitlag. Jumping and dropping detach normally;
carry changes position without adding launch velocity. Moving platforms remain pass-through and have no ledges.
Rendering samples the same path at interpolated frames. Solid-block motion and per-piece art overrides are
not part of version 2.

Platforms require standing-body clearance from every block for every loaded fighter, across the full
continuous path (including between ticks, the closing loop segment, and `global.sim.platformPad`). Contact is allowed; penetration
is rejected. This rules out platforms under overhangs with less than the tallest fighter's body height of
clearance. Motion phase is included when resolving spawn support. Spawn bodies and respawn descent/wait
bodies must clear solids too. Version 1 off-ground spawns without `y` migrate to the old ground height.

Spawn validation checks the body of every loaded fighter against solid geometry, with paths such as
`spawns[2]`; motion errors identify paths such as `platforms[0].motion.waypoints[1].travelTicks`.
Internally, surface ids are block indices followed by platform indices offset by `blocks.len()`.
`StateData::ledge_block` identifies the block throughout hanging and climbing; it is part of the rollback snapshot.

### Stage follow-camera zoom

The presentation-only `zoom` object optionally overrides either visible-height limit:
`"zoom": { "minHeight": 480, "maxHeight": 1600 }`. Omitted bounds are automatic. Both values must be
positive and `minHeight ≤ maxHeight`; stage `camera` bounds always cap the final view. This is an optional
version-2 extension and does not affect simulation snapshots or replay checksums.

The automatic play area unions block rectangles (including undersides) and platform motion-path extrema.
At the current aspect ratio, its padded fit height is compared with the padded reference dimensions in
`global.render.camera.referencePlayWidth/Height`. The automatic maximum is `maxHeight` times that ratio
(with a minimum multiplier of 1), enlarged if necessary to accommodate the follow camera's stage bias.
Reference-sized and smaller fits retain the global maximum. The automatic minimum is `minHeight` times
`clamp(max(width/referencePlayWidth, height/referencePlayHeight) / minScaleStart, minScaleFloor, 1)`.
Thus only clearly smaller stages zoom in tighter. The editor displays both automatic and effective ranges
at 16:9 and draws the effective maximum view around the play-area centre when bounds are visible.
