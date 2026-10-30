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
- **Animations**: a list of keys `{ "at": t, "pose": <name or inline pose>, "lerp": bool }`. A key holds until
  the next one. With `lerp` the figure blends from the previous key. Move phases animate in phase frames
  (`anim`). State animations (`anims.<name>`) pick a `clock`: `state` (frames in state), `world`, or `run` (run
  cycle degrees), and may `loop`.
- **State animation names**: `idle`, `run`, `jumpsquat`, `landing`, `air.rise`, `air.fall`, `air.flip`, `helpless`,
  `shield`, `shieldDrop`, `roll`, `spot`, `airdodge`, `hold`, `pummel`, `throw.f`, `throw.b`, `throw.u`,
  `throw.d`, `grabbed`, `hitstun`, `tumble`, `shieldbreak`, `knockdown`, `getup`, `getupAttack`, `tech`, `ledge`,
  `climb`, `ledgeAttack`, `ledgeRoll`, `ledgeJump`, `dizzy`, `win`, `respawn`.
- **Attachments**: vector `shapes` (line, circle, poly, rect) hung on a joint at an angle. On a hand the angle is
  relative to the forearm. A phase's `attach.<id>` can key `angle`, `offset` and `scale`, and `show` a hidden
  attachment. While a phase keys the angle, `hold` poses the front arm to grip it. `glow` swaps shapes marked
  `when: "glow"` / `"noGlow"`. Colors are `#rrggbb` or `player` / `playerDark` / `playerLight`.

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

`World::new(MatchConfig, &GameData)` resolves stage/fighter ids into content indices. Player config contains
`fighter`, optional CPU level, optional team, and `tap_jump`; match config also contains seed, stocks, training,
and friendly fire. `None` teams are free-for-all. Friendly fire permits teammate hit/grab/projectile interaction;
AI still targets enemies. Winner is the last surviving team (or individual in free-for-all); all surviving
winning teammates enter the win state. Simultaneous elimination is a draw. Training replenishes stocks and
never ends. Pair passes run in player-index order, and overlapping push pairs run in lexicographic order.

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
