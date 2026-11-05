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

Every file has `"$schema"` (relative path to its schema) and `"schemaVersion"`. Versions are **per content
kind**: global and fighter formats are currently version 1; stages are version 2. Only stage version 1 migrates
`ground` to `blocks`. Existing global/fighter files declaring version 2 are accepted as legacy aliases of version
1 and normalized on load; no fighter or global format change accompanied that version. Unknown versions are
rejected. Future migrations are dispatched by content kind so a stage change cannot alter a fighter or global file.

**Packages.** A fighter with `"extends": "<id>"` is merged over its parent before it's read: objects key by key,
arrays replaced whole, `null` deletes a key. `"abstract": true` marks a parent-only package (not playable, not
inherited). Files from the user content dir replace built-in files with the same path. A broken package is
skipped with an error naming the file and JSON path.

**Formatting.** Files are written by `data::json::to_compact_string`: 2-space indent, and any object or array that
fits in 120 columns and contains no objects stays on one line (a hitbox, a keyframe).

## Coordinates and units

- World/stage: stage pixels, **y down**, 60 ticks per second, speeds in px/tick.
- Scale: **5 px per Smash Ultimate unit** on both axes, for lengths, speeds (units/frame × 5 = px/tick),
  accelerations and gravity (units/frame² × 5 = px/tick²); launch speed is 0.15 px/tick per reference knockback
  unit. Built-in content and stage sizes use it ([DESIGN.md](DESIGN.md#scope), Units). A jump speed for a source
  height `h` units comes from `tools/units.py` (`jumps`), which is exact for the sim's integration. A full hop with
  `stats.jumpInitialHeight` (Ultimate's jump initial height, 0.55 × the full hop for most fighters) climbs that height
  in four equal no-gravity ticks, then continues at `jumpSpeed` (derived from the remaining height), so full hops land
  on Ultimate's frame (Peach 57, not 63); short hops and air jumps are unaffected.
- Fighter-local (hitboxes, hurtboxes, poses, events, projectile spawn): **x forward, y up**, origin at the feet.
  The sim mirrors x by facing and flips y.
- Angles in degrees: 0 = forward, 90 = up. This holds for launch angles, attachment angles and pose `rot`
  (positive `rot` pitches the body forward).
- Event velocities (`setVel`, `addVel`) are fighter-local too: `{"y": 17.5}` launches upward.
- The schemas tag every numeric field with `x-unit` (`px`, `px/tick`, `px/tick2`, `px/kb`, `tick`, `s`, `ratio`,
  `deg`, `damage`, `kb`, `uipx`, …) and spatial ones with `x-axis` (`x`, `y`, `xy`, or one per tuple element, such as
  `["x", "y"]` for points). `crates/sim/src/data/unit.rs` defines them; `just rescale` uses them to rescale content.

## Knockback models

Every fighter, throw, projectile and shared attack uses `global.knockback.ultimate`:
`((postHitPercent × (percentScale + hitDamage × damageScale) × weightScale × 2w/(1+w) + base) × kbg + bkb) × launchSpeed`.
`bkb` is in source reference units; `kbg` is source growth divided by 100 (90 → 0.9).
`w` is the existing reciprocal weight multiplier (100 / source weight). The built-in coefficients reproduce
the no-staleness/no-rage Ultimate equation and convert launch speed to this world's pixels. Charge changes
hitDamage without separately inflating bkb. There is no per-character or per-hitbox formula selector.
Fixed-knockback (`setKb`) hits ignore damage/percent: `bkb × launchSpeed × w`; their BKB uses the same
reference units. A hit with `fkb` (imports: the source FKB) is Ultimate's weight-based knockback instead: the shared
equation with postHitPercent 10 and hitDamage `fkb`, keeping the hit's own `bkb` and `kbg`, so only the first term
scales with weight (`((1 + fkb/2) × 1.4 × 200/(W+100) + 18) × kbg + bkb`); it takes precedence over `setKb`, which
imports keep as a marker. Before, generators resolved FKB at weight 100 into a `setKb` BKB, which then scaled the
whole launch by `w`. Windboxes remain
pixel-space forces and bypass launch calculation; grabs do not launch. Autolinks use their separate pull
velocity, with the shared formula supplying hitstun strength.

Original roster estimates preserve uncharged launch speed against a middleweight. Given old base B,
old growth G, old percent coefficient P and launch conversion L, new growth is
`G × P / (L × (percentScale + damage × damageScale) × weightScale)`, and new BKB is
`B/L - base × newGrowth`. Fixed BKB becomes `B/L`. Dynamic counters are fitted at their minimum damage;
their launch now also grows with the damage countered. Mike's imported ordinary-hit values are retained.
The pre-migration baseline in `crates/sim/tests/fixtures/knockback-linear.json` stores
`[authoredDamage, oldBKB, oldKBG, fittingDamage]` by source file and JSON pointer. Regression tests compare
actual launches at multiple percentages; TS traces restore these values and omit `ultimate` to exercise
the original linear compatibility formula. This compatibility path is not a fighter option.

Hitstun, launch decay, stage dimensions and DI remain shared game rules, so matching launch magnitude
alone does not promise matching SSBU KO percentages. Weight interactions and charge strength deliberately
follow the new shared equation and still need human playtesting.

### Hitstun and launch (`global.hitstun.ultimate`)

When `hitstun.ultimate` is present, a hit's launch velocity (knockback × angle, after ground bounce; autolink pulls
too) is stored on the fighter as a separate launch vector. Reference knockback is launch px /
`knockback.ultimate.launchSpeed` (1 without it). Each airborne hitstun tick:

- the launch vector's magnitude shrinks by `launchDecay` × launchSpeed px (reference units per frame; Ultimate
  0.051 / 0.03 = 1.7, = 0.255 px at launchSpeed 0.15), never past zero;
- the fighter's own component gets the victim's `stats.gravity`, capped at its `stats.fallSpeed`, and the stick
  accelerates it by `hitstun.driftAccel` up to `stats.airSpeed` (otherwise air friction under
  `movement.ultimate`, or `movement.airDrag`);
- `vx`/`vy` stay the sum, so a spike keeps its full speed with no fall cap on the launch part.

On the ground (non-tumble landing) the launch is cleared and `groundDrag` / `driftAccel` apply to the whole
velocity as before. The launch survives hitstun ending in the air: the `air` state keeps decaying it, with drift,
drag, fast fall and the fall cap acting on the own component only. Landing, a wall or ceiling (that component),
an air jump, a ledge grab, a new hit and any state other than `hitstun`/`air` clear it; remaining velocity then
belongs to the new state's physics. DI rotates the launch vector with the velocity.

Hitstun = floor(floor(reference × `perReferenceKnockback`) + `base`) × hitstunMult + hitstunAdd (Ultimate 0.4 and
0). Tumble starts above `tumbleThreshold` reference knockback (Ultimate 80).
Hitlag uses `global.hitlag` (Ultimate: base 6, perDamage 0.65, perKnockback 0, max 30). With `multBeforeCap`
(bundled) it is min(max, floor((base + damage × perDamage) × hitlagMult)), Ultimate's order (ssbwiki Hitlag); without
it the legacy min(max, floor(…)) × hitlagMult, floored.

`topLaunch` (`{ minAngle, maxAngle, gravity, gravityPerWeight, fallSpeed }`, bundled 70 / 110 / 0.21 / 0.00168 / 9):
Ultimate's `damage_fly_top_*` params. A tumbling launch whose angle is within the range uses gravity
`gravity + gravityPerWeight × W` (W = 100 / `stats.weight`, the source weight; Mario 0.0749 units) capped at
`fallSpeed` (1.8 units for everyone) on its own component while in hitstun, instead of the victim's own. After
hitstun, a fall faster than the fighter's own fall speed slows by its gravity per frame back to it (the snapshot flag
`Fighter::top_launch`). Sideways and non-tumbling launches keep the victim's own gravity.

`hitTypes.sakurai` (361): airborne victims launch at `air`; grounded ones at `groundLow` / `groundHigh` by the tumble
threshold, or with `ramp: [from, to]` (reference knockback) rising linearly from `groundLow` at `from` to `groundHigh`
at `to`. Bundled: Ultimate's 0° up to 60, 38° from 88, 38° in the air (ssbwiki Sakurai angle).
Optional `cancel: { airDodgeAfter, aerialAfter }` lets a tumbling airborne fighter air dodge / start an aerial
after that many hitstun ticks; it is absent (off) pending verification against Ultimate.

Without `hitstun.ultimate` (TS traces) hitstun is floor(floor(launch px × `perKnockback`) + `base`) × mult + add,
tumble compares launch px with `knockback.tumbleThreshold`, and launches fly with `hitstun.gravity` capped at
`maxFall` and multiplicative `airDrag`.

`tech.lockout` (frames, 0 = off, bundled 40): a shield press in hitstun starts a lockout during which further
hitstun presses don't count toward teching; a successful tech clears it. A single press within `tech.window`
frames before a tumble landing still techs; mashing from far out does not.

### Defensive windows (`dodge`, `tech`, `knockdown`, `ledge`, `shield.dropLag`)

`dodge.roll`, `spot`, `air` (neutral air dodge) and `dirAir` are windows
`{ frames, invulnStart, invulnEnd, moveStart, moveEnd, speed, decay, decayFrames, landingLag }` (fields a dodge
doesn't use are omitted). Frame numbers use the state clock like move phases: 0 is the tick the state starts
(Ultimate's frame 1), and the state ends when its clock reaches `frames`. The fighter is intangible on frames
`invulnStart..=invulnEnd` and hittable before and after. Start 0 sets invulnerability on the entry tick, replacing
any current invulnerability; a later start adds it when the clock gets there, and leaving the state first (hit,
walking off a ledge) cancels it. An end past `frames` carries over into the next state. The roll moves at `speed`
on `moveStart..=moveEnd`; a directional air dodge launches at `speed`, multiplies its velocity by `decay` below
`decayFrames`, and lands with `landingLag` (neutral too). `tech` has `invulnStart` / `invulnEnd` for a tech in
place (tech rolls use `dodge.roll`), `knockdown.getup` / `getupAttack` are `{ frames, invulnStart, invulnEnd }`,
and each ledge option (`climb`, `attack`, `roll`, `jump`) has `invulnStart` / `invulnEnd` (start 0 keeps the longer
of this and the hang invulnerability). Bundled values (state clock; Ultimate frame = +1): roll 30 frames,
intangible 3–14; spot dodge 26, 2–16; neutral air dodge 40, 2–28; directional 44, 2–21; tech 24, 2–19; getup 24,
2–21; getup attack 32, 2–13; ledge options unchanged from 0. `shield.dropLag` is 11 (Ultimate).

`dodge.staling` (absent = off): a roll, spot dodge or air dodge started within `window` ticks (bundled 90) of the
previous one moves one step along `steps` (`{ invuln, frames }` multipliers, rounded; the first repeat uses
`steps[0]`, then capped at the last), shortening intangibility from its start frame and lengthening the state; a
longer gap starts fresh. Bundled: 0.93 / 0.87 / 0.8 intangibility and 1.1 / 1.2 / 1.3 frames. Getup and tech rolls
neither stale nor count. The counter is `Fighter::dodge_stale` / `dodge_at` in the snapshot.

Older global files with the flat `dodge.rollInvuln` … `dirAirDodgeLandingLag`, `tech.invuln`,
`knockdown.getupInvuln` / `attackInvuln` / `getupFrames` / `attackFrames` and ledge `inv` load as windows starting
at 0 (`invuln` N = frames 0..=N−1), which reproduces the old timing exactly.

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
   charge +1, and skip steps 2 to 4 this tick. Movement still runs. With `holdAfterMax`, it can remain on
   this frame for that many additional held ticks after strength reaches `max`; those ticks do not increase
   damage, travel scaling or charge thresholds. Release still resumes immediately.
2. **B-reverse.** For a `bReverse` move, before the window closes: holding the other direction turns the fighter
   (once). In the air, horizontal speed flips too.
3. **Advance** the phase frame.
4. **Events** matching this frame, in list order (see below).
5. **Step transitions**, in list order, first match wins: `timeout`, `grounded`, `airborne`, `falling`,
   `release`, `attackHeld`, `targetNear`, `shieldPress`. A phase with `frames` and no `timeout` transition ends
   the move on timeout.
6. **Movement** with the current phase (after any transition). Grounded: attack friction or traction (`slide`
   frames use the slide value; see Momentum), gravity, ground snap with edge stop. Airborne: drift with the move's
   `air` accel × `airAccelMult`, fast-fall if `air: "aerial"`, gravity capped by `fallCap` (else fast-fall or fall speed), `hover` zeroes vertical
   speed, integrate. Landing fires `land` transitions. Ledges can be grabbed from phase frame `ledgeGrab` on, even
   while rising.
7. **Hit pass** (after every fighter has stepped): hitboxes of the current phase active on its current frame.
   A connecting hit fires the attacker's `hit` transitions. A hit taken inside a `counter` window is countered
   instead and fires the defender's `counter` transition.

**Entering a phase** (any transition): phase frame = 0, victim/group contact history is cleared unless the transition
sets `preserveHits: true`, and `at: 0` events run immediately.

**Ending tick.** When a transition ends the move during the step (step 5), that tick's movement (step 6) still uses
the phase that just ended, then the fighter is in its new state. This matches the TS version.

**Endings** (transition `to`): `end` goes to idle on the ground or air in the air. `helpless` goes to idle on the
ground or helpless in the air. `landing` goes to landing lag for `lag` frames. Anything else names a phase.

### Events

`at: N` fires on frame N. `from`/`to` (inclusive) fire on a range. `from` alone fires from that frame on, and
neither fires on every frame from 1. Frame-0 events must use `at: 0`. `every: K` also requires the world frame to
be divisible by K. `if` restricts to `grounded`, `airborne` or `countered`. One event's actions run in this order:
`mulVel`, `setVel`, `addVel`, `capFall`, `leaveGround`, `invuln`, `spawn`, `item`, `restore`, `detonate`, `sfx`,
`fx`. `item: "id"` puts one of the fighter's `items` in its hand (see Items).
`restore: {"charge": "id", "uses": N}` restores uses capped at capacity while preserving a pending recharge
until full. `detonate: ["c4"]` immediately explodes live owned explosives with those projectile ids;
`detonate: []` selects every owned remote (`explosion.manual`) gadget, so a timed fuse such as a grenade is
never fired by accident. Omit the field to leave explosives alone. Detonation does not move or age
projectiles and uses their current owner (including reflection), teams, shields and counters. Random ranges (`[a, b]`)
in effects draw in field order (x, y, vx, vy, life, size). With `global.sim.separateCosmeticRng` (default true),
effect ranges and charge particles use a separate rollback-owned RNG seeded from the match seed, leaving the
gameplay RNG unchanged. Set it false to share the gameplay RNG for TS compatibility. Both streams are deterministic.

### Transitions

A transition may set `preserveHits: true` to retain victim contact history for an attack that continues
across phases (for example, the same falling sword hit). Rehit intervals use the move
elapsed clock, so resetting the phase frame does not reset those cooldowns. Other transitions clear contacts.

`{ "on": <trigger>, "if": <cond>, "from": <min phase frame>, "to": <phase|end|helpless|landing>, "lag": <frames> }`.
Triggers: `timeout`, `land`, `grounded`, `airborne`, `falling` (vertical speed ≥ 0, y down), `release` (charge
button, else attack, not held), `releaseToward` / `releaseAway` (the same release, additionally requiring the
live movement stick to point toward / away from the fighter's facing, so a held special can pick a throw arc),
`attackHeld` (attack held), `attackPress`, `specialPress`, `upHeld`, `targetNear`,
`shieldPress`, `articleRelease` (a held controlled projectile was launched), `articleEnd` (it ended without a
launch, e.g. the caster let go), `counter`, `hit`.

`Phase.aim` keeps the move's single `bReverse` turnaround open for that phase, after the shared opening
window has closed. A charged move can therefore accept a direction during its charge (Long-Stem Strike) while
the committed strike phases ignore it. The one-turn lock is unchanged.

`targetNear` needs a phase `detection` rectangle (`x1 < x2`, `y1 < y2`), in fighter-local space
(x forward, y up). It fires when a hittable opponent's hurtbox overlaps that rectangle; facing, teams,
friendly fire and invulnerability use normal combat rules. It does not hit or push the opponent.

A move may override the global charge damage bonus with `chargeDamage`: damage × (1 + held ticks × value).
Charge knockback derives from charged damage; `chargeKbRatio` applies only in legacy TS compatibility mode. Events may set `chargeVelocity` to scale their `setVel`
and `addVel` by (1 + held ticks × value); `mulVel` is unchanged. Events and move hitboxes may set
`minCharge` to require that many held ticks (default 0). Charge persists across phases.
A finite phase may set nonnegative `chargeFrames`: its duration is `frames + round(charge × chargeFrames)`.
This scales recovery independently from the saturated hold time and leaves hitbox/event times unchanged.
These fields default to existing behavior; projectile damage still uses its own `chargeScale`.
Hitboxes also accept `situation: "both" | "ground" | "air"` (default `both`), filtering victims by
whether they are grounded. This applies to melee, grabs, windboxes and projectile contact/explosion hits;
geometry and collision priority are otherwise unchanged.

Hitboxes may set `group` (u8, default 0): contact history is keyed by victim and group. Overlapping
coverage/tier rows share one group; separate hit windows use different groups. Only one box connects per
victim per hit pass (first listed wins). `rehit` cooldowns apply independently to each victim/group pair;
`preserveHits` retains all groups and their move-elapsed cooldowns across phase boundaries.

Importers cluster inclusive source creation/clear windows by transitive overlap. Rows with `rehit` form a
loop identity. A following cluster touching only the loop's final seam and declaring a different `rehit`
(including none) starts a new identity, so a multihit finisher connects. Equal rehit, ordinary coverage/damage
changes without rehit, and overlaps wider than the seam continue the existing identity. Engine active ends
exclude the source clear tick; that tick still participates in source identity clustering.

### Counter

A phase with `counter` counters hits received on frames `from..=to`. On a melee hit: the attacker freezes
`attackerFreeze` frames, the defender turns to face them, gains `invuln`, and stores counter damage =
max(`minDamage`, incoming × `damageMult`). A projectile is reflected instead (speed × `reflectSpeed`, life =
`reflectLife`, vertical speed = the projectile's `reflectVy` if set) and stores 0 damage. Either way the
`counter` transition fires. Hitboxes with `counterDamage` use the stored damage and are inactive when it's 0. The
`countered` condition is true when it's above 0.

### Armor

A phase may set `armorThreshold` to a nonnegative damage threshold. Incoming hits at or below it
still deal damage, credit the attacker and apply hitlag/flash, but preserve the move, velocity, grounded
state and recovery resources. Stronger hits react normally. Grabs bypass armor; counters take priority.
Armor also blocks windbox pushes. Armored hits emit a small ring and never chain into or out of training combos.
Use a separate startup phase to limit armor to wind-up (charge holds retain that phase).

## Durability resources and move variants

A fighter may declare limited-use resources and moves may pick a whole phase timeline from them. This models
weapons and tomes that break after a number of uses and come back later (Throbbin's Levin Sword and four
tomes). Nothing here is fighter-specific.

```json
"charges": [{ "id": "levin", "uses": 8, "recharge": 600, "emptySfx": "toss", "refillSfx": "ready" }]
```

- `uses`: capacity. `recharge`: ticks from empty back to full. Uses return one at a time every
  `ceil(recharge / uses)` ticks, so a pool is full after `uses × that step` (at most one tick per use longer than
  `recharge`). Zero stays spent until respawn. `discardDelay`: ticks added before the first restored use.
  `emptySfx` / `refillSfx` are sound names. Spending a use while the pool recharges never refunds it: the
  countdown to the next restored use keeps running.
- `moves.<slot>.variants[]`: `{ "needs": { "charge": "levin", "uses": 1 }, "phases": [...] }`. The first
  variant whose need is satisfied is chosen **once, when the move starts**, and runs instead of `phases`. A
  move without variants uses `phases`.
- `phases[].cost`: `{ "charge": "...", "uses": n }` is spent when the phase is entered. A pool with fewer
  uses left spends what it has and is emptied (the item is used up) but the phase still runs.
- `transitions[].minCharge`: the transition only fires with at least that many held charge ticks. Transitions
  are checked in list order, so a charged special lists its highest tier first and the uncharged fallback last.
- Respawning refills every pool. The runtime state (`Fighter::charges`, `charge_wait`) is part of the rollback
  snapshot; `moves.*.variants` phase lists are validated exactly like the default timeline.

Two combat fields support resource-limited attacks: `hitboxes[].heal` (fraction of the damage dealt that heals
**the attacker**, capped at their current damage) and `hitboxes[].unblockable` (skips shields, counters and
parries, so a command grab such as Nosferatu still lands on a guarding victim).

## Projectiles

Defined per fighter under `projectiles.<id>` and spawned by a `spawn` event. Each tick a projectile moves by its
velocity and loses 1 life. With `gravity`/`maxFall` it falls. With `bounce` it bounces on surfaces and the
stage's sides. With `bat`, attacks knock it back and the attacker becomes its owner. Projectiles of different
owners destroy each other on contact. `trail` events run every tick in projectile-local space (x along its travel
direction).

Optional `chargeScale: { "speed": 0.024, "damage": 0.035 }` multiplies spawn velocity and damage
by `1 + held charge ticks × coefficient`. It reads the spawning move's charge; it does not change gravity or
radius. `life` scales the fuse the same way and may be negative, which shortens it with charge (cooking a
grenade); the result is clamped to at least one tick. Omit it for unscaled projectiles. Optional `bkb`, `radius`, `life`, `rehit`, `angleTo` and
`groundAngleTo` scale those properties too; `rehit` adds whole frames per held tick (rounded, at least 1). A
projectile whose `hit.rehit` is set stays in play between contact windows and cannot hit the same victim again
until the interval elapses; a projectile without it is consumed by its first contact. `hit.type: "damageOnly"`
deals damage and shield damage but skips hitstun, hitlag, launch, state changes and combo linking, which is
useful for lingering no-flinch hazards such as Poison Breath.

Optional `explosion: { "armTicks": 30, "proximity": 42, "radius": 52, "sfx": "quake", "fx": [...] }`
suppresses contact hits. After `armTicks` elapsed ticks, an opposing hittable hurtbox within `proximity`
triggers a blast; omit proximity for a fuse-only bomb. Lifetime expiry always detonates. The blast uses `hit`
and `radius`, hits every eligible opponent once (shields still work), emits projectile-local FX/SFX and removes
the projectile. Blast knockback and shield pushback point away from the blast center, with travel direction
then owner facing as tie-breakers. Active counters trigger without reflecting the blast or taking its damage;
other victims remain eligible. Unexploded gadgets can clash with opposing projectiles. `max`
also limits planted gadgets; a second spawn at the limit fails without replacing the first. Bounce physics
can settle a mine with zero restitution, minimum speed and friction. Settled contacts do not count as bounces;
a resting projectile retains its supporting surface and rides its motion delta. A projectile's rollback `age`
counts elapsed ticks independently of life changes from batting, reflection and bounce fizzling. Proximity uses
this age; fuse expiry still detonates even before arming.

## Items

Held items, Smash-style (`crates/sim/src/items.rs`). An item is defined per fighter under `items.<id>` and put in
the fighter's hand by a move event's `item` (Duchess's Vegetable pluck). A fighter holds at most one; an `item`
event while one is held does nothing.

```json
"items": {
  "turnip": {
    "name": "Turnip", "projectile": "turnip", "smashProjectile": "turnip_strong", "hold": [24, 56],
    "throws": {
      "forward": { "tilt": [11.4286, 3.3333], "smash": [14.2857, 3.5833] },
      "back": { "tilt": [11.4286, 3.3333], "smash": [14.2857, 3.5833] },
      "up": { "tilt": [0, 11.9048], "smash": [0, 14.7281] },
      "down": { "tilt": [0, -11.9048], "smash": [0, -14.7281] },
      "drop": [0, 0]
    },
    "speedDamage": 0.6, "rebound": [2.5, 6], "life": 600, "catch": true
  }
}
```

- `projectile` / `smashProjectile` name plain projectiles of the same package, with `bounce` (no article or
  explosion). A tilt throw or a drop launches `projectile`, a smash throw `smashProjectile` (default `projectile`),
  so the two strengths can carry different hits. The projectile's `shapes` draw the item in hand and on the floor;
  its `r`, `gravity`, `maxFall` and `bounce` move the loose item.
- `throws`: velocity (x forward, y up) by direction and strength. A back throw turns the fighter around first,
  so its x points the new way. `drop` is letting go. The projectile's `maxFall` still caps downward speed, so a
  down throw starts at most that fast.
- `speedDamage`: damage added per px/tick of throw speed above the projectile's own `vel` speed (taken away
  below it, never under zero). Ultimate's thrown-item damage `base − 0.4 + speed × 3` is 0.6 here; measured
  against each projectile's own speed, the forward throws keep the projectile's authored damage.
- `hold`: where a throw starts (fighter-local), and where the held item is drawn on named-joint figures (legacy
  figures draw it at the front hand).
- `rebound`: velocity a thrown item bounces off with when its hit connects (x back against its travel, y up).
  Omit it and a hit uses the item up.
- `life`: ticks a loose item stays in the world. `catch`: can be caught out of the air.
- `toss`: optional `{ frames, release }` override of the shared throw timing.

**Holding.** While holding, every action stays available (jumps, float, specials, shield, dodges, dash, walk,
run, ledge) except attacks and aerials, which throw:

| Input | Throw |
| --- | --- |
| Attack on the ground or in the air, neutral | forward tilt |
| Attack with a held direction (button) | that direction, tilt (behind = back) |
| Attack with a recent flick, the attack stick, attack + special, or out of a dash | that direction, smash |
| Grab on the ground | as attack |
| Grab in the air | drop |
| Shield + attack / grab | drop (up smash out of shield: up smash throw) |
| Jump + attack (short-hop aerial), jumpsquat attack | air throw by the aerial's direction; jumpsquat attack is an up smash throw |

A throw is the `toss` state (`sd.kind` `f` / `b` / `u` / `d` / `drop`, `sd.n` 1 for smash): `global.items.toss`
(`frames: 22`, `release: 5`, Ultimate's item throw) unless the item overrides it. Grounded, it slides with
landing friction; airborne, it drifts like an attack and can fast-fall, and landing keeps the throw going. On
state frame `release` the projectile spawns at `hold` with the throw's velocity, tagged with the item
(`Projectile.item`), and the hand empties. The state animation is `toss.<kind>`, falling back to `toss`
(`fighters/base` has a generic overhand one; Duchess overrides it).

**Loose items.** A thrown item becomes a loose item (`World.items`) when it touches the floor (it loses its hitbox
there, as in Smash), when its hit connects (with `rebound`), or when its life runs out or it clashes. Loose items
fall with the projectile's gravity, bounce with its `bounce.restitution` / `friction` until the rebound would be
slower than `items.settle`, rest on (and ride) surfaces, bounce off block sides, and vanish after `life` ticks or
outside the blast zones. They never hit anyone.

**Pickup and catch.** With an empty hand, attack or grab on the ground picks up a landed item whose centre is in
`items.pickup` (`landing` state with kind `pickup` for `pickupLag` ticks, animation `pickup`). Attack, grab or (in
the air) an air-dodge press catches an item in flight (loose and falling, or still thrown) whose centre is in
`items.catch`, with no lag; a catch replaces the air dodge or attack. Anyone can take anyone's item, but a
thrower cannot catch its own throw for `items.regrab` ticks. Being KO'd loses the item.

**CPU.** A CPU holding an item walks toward its target and smash-throws it forward once the target is within
`items.cpuRange` (and `ai.thresholds.attackHeight`), or after holding it `cpuHold` ticks. CPUs do not seek items
out, but an attack or grab near a landed one picks it up.

Snapshot state: `Fighter.item` (`Held { data, id }`: package index and item id), `Projectile.item`,
`World.items` (`LooseItem`) and `AiState.item_t`; all empty without items, so content without them behaves as
before. SIM_VERSION -sim-36.

## Visuals

- **Poses** (`poses.<name>`): joints `head, shoulder, hip, elbowF, handF, elbowB, handB, kneeF, footF, kneeB,
  footB`, plus `rot`, `pivot` (rotation center height) and `offsetY`. Missing joints come from `from` or `stand`.
- **Animations**: a list of keys `{ "at": t, "pose": <name or inline pose>, "lerp": bool, "if": cond, "minCharge": n }`.
  A key holds until the next one. With `lerp` the figure blends from the previous key. A key with `if` (`grounded`,
  `airborne`, `countered`) is skipped while the condition is false; a key with `minCharge` is skipped below that
  many held charge ticks, which lets one phase change its posed reach as charge grows. Move phases animate in phase frames
  (`anim`). State animations (`anims.<name>`) pick a `clock`: `state` (frames in state), `world`, `run` (run
  cycle degrees), or `spin` (tumble spin degrees, signed), and may `loop`. Exceptions: `air.flip` counts frames
  since the flip started, `pummel` frames since the pummel started. `idle` above `render.overlays.runSpeed`
  shows `run`.
- **State animation names**: `idle`, `walk`, `crawl`, `run`, `turn`, `brake`, `crouch`, `jumpsquat`, `landing`,
  `air.rise`, `air.fall`, `air.flip`, `helpless`, `shield`, `shieldDrop`, `roll`, `spot`, `airdodge`, `hold`,
  `pummel`, `throw.f`, `throw.b`, `throw.u`,
  `throw.d`, `grabbed`, `hitstun`, `tumble`, `shieldbreak`, `knockdown`, `getup`, `getupAttack`, `tech`, `ledge`,
  `climb`, `ledgeAttack`, `ledgeRoll`, `ledgeJump`, `dizzy`, `win`, `respawn`, `toss` (optional `toss.f`, `toss.b`,
  `toss.u`, `toss.d`, `toss.drop`), `pickup` (optional; falls back to `landing`).
  `walk`, `crawl`, `turn` and `brake` are optional authored gait clips; a package without them keeps the
  legacy `idle`/`run`/`crouch` selection. `turn` and `brake` sample the gait clock (`gait_t`, reset by every
  gait change), not the idle state clock, so the one-shot transition starts at its first key after any idle age;
  `walk`/`crawl` keep their authored clock and `run` uses the run-cycle phase. Holding down while grounded selects `crouch` through the ground gait; horizontal movement
  retains the stance when `global.gait.crawlSpeed` is enabled. Its
  animation blends with `idle` over `global.gait.crouchFrames` on entry and release; reversing the input
  reverses the blend from its current depth. The base fighter supplies a breathing loop inherited by the roster.
  Override `anims.crouch` and `poses.crouch` to customize it. Attacks, jumps and platform drops interrupt it normally.
- **Attachments**: vector `shapes` (line, circle, poly, rect) hung on a joint at an angle. On a hand the angle is
  relative to the forearm. A phase's `attach.<id>` can key `angle`, `offset` and `scale`, `show` a hidden
  attachment and `hide` a visible one (a sheathed weapon while casting or grabbing). Angle keys interpolate
  linearly without wrapping: use 350 → 430 to continue forward through 360°
  to 70°; 350 → 70 rotates backward by 280°. Keep subsequent keys on the same turn for continuous trails and
  recovery. While a phase keys the angle, the angle is absolute, `hold` poses the front arm to grip it, and a
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

`global.input.stickThreshold` is the inclusive digital stick threshold (1..127, currently 64): tilts, aerial and
throw directions, rolls. `deadzone` (18) zeroes analog movement (walk speed, air drift, DI) below it.
`flickThreshold` (100) and `flickFrames` (3): reaching the flick threshold within that many ticks of leaving the
deadzone is a flick (dash, smash, platform drop; it stays a "tap" for `movement.tapWindow`). `comboFrames` (2)
combines buffered presses that far apart. `fastFallBuffer` (6) lets a down press that early still fast fall once
descending (`fastFallMinVy: 0` requires actual descent). With `movement.platformPass`, a platform
drop consumes its down gesture, so a fresh down press is needed to fast fall afterward. Holding down
during ordinary fall or helpless fall passes through platforms without a new press; attacks, airdodges,
hitstun and tumble still collide with them. Solid blocks always collide. `global.gait` (walk / dash / run / turnaround / brake fractions and frames; per-fighter
`stats.walkSpeed` overrides the walk fraction), `global.parry`, `global.di`, `global.shield.upSmash`,
`global.ledge.trump` and `global.ledge.freshInvuln` tune the Smash-style feel features; DESIGN.md describes each.
`global.gait.runThreshold` is the deadzoned horizontal tilt that promotes a walk to a run (0..1;
0 retains flick-only running and ignores `walkThreshold`). `walkThreshold` is the lower return-to-walk
threshold, no greater than a nonzero `runThreshold`. Between them the current gait is retained to avoid jitter. `dashLock` restricts standing
attacks and shield during initial dash when enabled (bundled tuning disables it): tilts and forward/down smashes queue within `bufferFrames`, while
jump, dash attack, up smash, grab and specials can cancel. Opposite flicks still dash back. Crouching can
cancel a run after initial dash. The settings affect all fighters; they are not per-character speed fixes.
`global.input.bufferFrames` is the human press buffer lifetime in ticks (u32, currently 5); age equal to
the lifetime is still live, and 0 disables buffering for TS fidelity. Press-time directions and taps are
replayed once at the next buffer-actionable update, while held buttons use current input (see DESIGN.md).
`global.input.jumpAerials` (default true) enables the jump-button + attack short-hop aerial shortcut.
The combination stays one buffered action during recovery. An attack pressed during jumpsquat is saved
until takeoff and also forces a short hop, retaining its press-time aerial direction (including the right
stick). Tap-up + attack still selects up smash. Empty jumps still use the jump hold at takeoff. Set false
alongside `bufferFrames: 0` for the original TS jump/attack routing.
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

`global.ai.thresholds.surfaceChaseTicks` is the no-progress timeout for chasing an opponent on another
surface outside attack reach. After that many blocked grounded ticks the CPU commits to the target's surface,
walks toward its departure edge and jumps, using the normal air-jump / recovery logic toward the destination.
Horizontal progress resets the timer. Reaching the target surface or attack range ends the commitment.
When enabled, movement targets and departure edges use actual support surfaces, including platforms,
rather than the source block bounds. Zero keeps reference routing; TS trace tests use zero. Pursuit state
is part of the rollback snapshot.

Each fighter's `ai` contains:

- `reach`: close-range attack distance.
- `counterChance`: chance of countering instead of shielding; 0 skips this random draw.
- `recoveryShieldCancel`: optional phase name in `uspec` where an offstage recovery can shield-cancel.
  Recovery steering is selected by the move's `air: "upSpecial"`, independent of its identity.
- `specials`: ordered special tactics. Each requires distance strictly above `minDistance`, strictly below
  optional `maxDistance`, vertical separation strictly below `height`, optionally `grounded`, optionally
  `noProjectile` owned by this CPU. Only then is `chance` sampled. Success presses special toward the target;
  `neutral: true` requires grounded idle without hitlag before rolling chance, faces the target and clears
  directional input to choose neutral special even when a separate side special exists;
  `down` then adds down, and optional `hold: [min,max]` samples/floors special-button hold duration. A failed chance
  continues to the next tactic, preserving random draw order. Input routing chooses the move slot.

A projectile's `name` is its training hit label. The first two arena respawn points both use the reference
center position; additional player slots have their own points.

### Momentum (`global.movement.ultimate`)

With `movement.ultimate` present, horizontal speed changes by fixed amounts per frame (px/frame², Ultimate units
× 5) instead of by multipliers. Without it (TS traces) the multipliers marked *Legacy* in the schema apply:
`stats.groundFriction`, `gait.turnFriction`/`brakeFriction`, `sim.jumpsquatFriction`/`landingFriction`/
`shieldFriction`, `movement.attackFriction`/`attackSlideFriction`, `airDrag`/`airDragHeld`/`attackAirDrag`, the
shared `aerialAccel`/`attackAirAccel`/`upSpecialAccel`/`helplessAccel` and `fastFallKick`. Those fields stay
required so legacy data loads; the new model ignores them.

- **Traction** (`stats.traction`, default `ultimate.traction` 0.5 = Mario's 0.1 × 5; imports and Mike carry their
  source `ground_brake` × 5, originals the default): |vx| drops toward zero by
  traction per frame in idle, crouch, brake, turn skid, jumpsquat, landing lag, dizzy, grab hold, throws, shield
  and grounded attacks, times `fastTraction` (2) while faster than the fighter's walk speed. A grounded attack
  phase's `slide` frames multiply it by `attackSlideTraction` (0.5). Dodge, knockdown, shield-break and hitstun
  ground drag stay multiplicative.
- **Air friction** (`stats.airFriction`, default `ultimate.airFriction` 0.05 = 0.01 × 5; imports and Mike carry their
  source `air_brake_x` × 5): with the stick neutral (or no air
  control in the phase) |vx| drops toward zero by it; with the stick held and |vx| at or below `airSpeed` × tilt
  the fighter accelerates by its air accel × tilt up to that cap (opposite stick accelerates against the motion);
  above the cap |vx| drops toward the cap by air friction and is never clamped. The same rule runs in the air
  state, helpless fall, air dodges, attacks, specials and the own component of hitstun drift, so starting an
  aerial does not change momentum.
- **Air control** is fighter-relative: `stats.airAccel` × `aerialAccelMult` (1) for `air: "aerial"` moves,
  `attackAirAccelMult` (0.55) for `air: "attack"`, `upSpecialAccelMult` (0.65) for `air: "upSpecial"` and
  `helplessAccelMult` (0.65) in helpless fall and air dodges, times the phase `airAccelMult`. Bundled phase values
  were migrated to keep their old effective acceleration: new = old × legacy accel / (airAccel × kind multiplier).
- **Fast fall** sets vertical speed to the fighter's `fastFallSpeed` at once (still capped by a phase `fallCap`).
- **Initial dash** holds `stats.dashSpeed` (default runSpeed × `gait.dashSpeed` 1.12) for its frames; a faster
  fighter entering the run then slows toward `runSpeed` by its traction instead of snapping to it.

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

`MatchConfig::time_limit` (seconds) runs a clock: `World::time_left` counts down once the countdown reaches GO
and stops during the ending animation or after `over`. `None` (and training) plays until stocks run out. With
`match.timerWarnSeconds` seconds left the sim emits a `tick` sfx on each whole second. At zero the match ends like a
stock ending, with `World::time_up = true`; `World::time_winner()` picks the side with the most stocks left, then
the lowest pooled damage (teams pool both, and a fighter in the death sequence no longer counts the damage of the
stock they just lost) and returns `None` on a tie or when nobody has a stock left. `World::time_leaders()` lists
the fighters on the sides tied for the lead; that list is fixed into `World::time_tied` when the clock hits zero,
so hits during the ending animation cannot change who plays sudden death.
`MatchConfig::sudden_death` starts every fighter at `match.suddenDeathDamage` with one stock and no timer; players
whose `PlayerConfig::eliminated` is set (the sides that lost the time-up) start with no stocks and out. It is the
same config with `sudden_death: true` (see `MatchConfig::sudden_death()`), so replays reproduce the start.

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
respawn `descendFrames` must be positive. Projectile lifetimes must fit a positive `i32`. Throw/projectile legacy static
hitbox geometry is ignored (explicit `geometry` is rejected); only projectile `r` and active melee hitbox radii must be positive.
Global validation errors abort loading; invalid fighter/stage packages are skipped with a file and JSON path.
Global rules are stricter than before: `hitstun.gravity` / `maxFall`, `hitstun.ultimate.launchDecay`, `shield.max`, `shield.breakGravity` /
`breakMaxFall`, `knockback.chargeDamageDivisor` and `hitTypes.windbox.maxSpeed` must be positive, every
`ai.thresholds` value is range-checked, and `render.select`, `render.interpolate` and `render.hud.look`
presentation knobs (durations, scales, jump distances) are rejected when out of range.
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

`blast` sets the KO boundaries independently of the visible `camera` bounds. To keep fighters offscreen
before a KO, place the camera bounds inside the blast rectangle with enough clearance for their bodies
(Arena uses Battlefield's: 350 px inside the blast sides, 310 below the top line, 300 above the bottom one). Increasing blast distances alone does not ensure offscreen
KOs: the follow camera can chase a fighter to the boundary if its own bounds extend beyond it. Stage coordinates
increase downward, so a larger `blast.bottom` gives more room to fall and a larger `blast.top` lowers the ceiling.

When a blast boundary is visible during gameplay, `global.render.blastZone` draws a dashed danger line there,
independently of debug overlays. Its `color`, `alpha`, `width`, `dash` and `gap` tune the indicator; sizes are
screen pixels and stay constant while zooming. Omit it or set `alpha` to zero to hide it. The editor keeps its
own bounds overlay.

The presentation-only `zoom` object optionally overrides either visible-height limit:
`"zoom": { "minHeight": 480, "maxHeight": 1600 }`. Omitted bounds are automatic; an empty `zoom` object is invalid. Both values must be
positive and `minHeight ≤ maxHeight`; stage `camera` bounds always cap the final view. This is an optional
version-2 extension and does not affect simulation snapshots or replay checksums.

The automatic play area unions block rectangles (including undersides) and platform motion-path extrema.
At the current aspect ratio, its padded fit height is compared with the padded reference dimensions in
`global.render.camera.referencePlayWidth/Height`. The automatic maximum is `maxHeight` times that ratio
(with a minimum multiplier of 1), enlarged if necessary to accommodate the follow camera's stage bias.
The reference dimensions (680 × 395) are the play area of the Arena before the 5 px/unit rebuild; today's
Arena is larger, so its automatic maximum exceeds `maxHeight` and its Battlefield camera bounds cap it (956 px
tall at 16:9). Portrait framing may widen to fit a padded play area up to the camera bounds. The automatic minimum is `minHeight` times
`clamp(max(width/referencePlayWidth, height/referencePlayHeight) / minScaleStart, minScaleFloor, 1)`.
Thus only clearly smaller stages zoom in tighter. The editor displays both automatic and effective ranges
at 16:9 and draws the effective maximum view with stage bias and camera-bound clamping when bounds are visible.

## Controlled articles and directional travel

`Projectile.article` adds a construction/travel phase followed by an independent release timeline. Its
`slot` identifies the controlling move, `minRelease` bounds manual release and `autoRelease` ends standby.
`releaseOnUp` uses release of special (guided shots); otherwise a fresh special press releases (summons).
`steer` selects vertical travel speed from live stick Y before release. The last `stages[].from` reached is
selected and frozen on release. The birth tick is age 0; later projectile steps advance age by one.
The controlling fighter's `articleRelease` transition starts its own release animation. A summon may remain
in standby after its caster becomes actionable. Losing control of a guided shot releases that shot without
accepting an action or consuming the interrupted fighter's buffered command.

Each stage supplies `startup`, `frames`, `vel`, frame-windowed `hitboxes`, `shapes` and optional
`attackShapes`; optional `stroke` connects active tracked damaging centers so a summoned blade follows its
collision sweep. Velocity applies throughout the released stage; geometry uses the projectile's local forward/up coordinates,
mirrored by its captured facing. `autoStartup` optionally shortens automatic standby release: attack windows
and ending shift earlier by the same amount. Overlapping hitboxes share contact history; first-listed damage
wins, `rehit` controls repeats, and windboxes retain separate non-damaging force semantics. Reflectors/counters
can change article ownership. Optional `hurtbox` plus stage `health` makes an article destructible; health 0
means indestructible. Melee, direct projectiles and explosions damage that body separately from the caster.
Owner stock loss removes an article. These timelines cannot combine with legacy `explosion`, `bounce`, `bat`
or `chargeScale`; ordinary projectiles keep their existing behavior.

Simulation recordings use `sim-25` after the held-article, damage-only hit and optional gait-clip additions;
the replay container stays version 2.

Optional `releaseOnDir` launches the article when a left/right stick direction is held while special is still
held; letting go of special instead fires `articleEnd` (the caster swallows the article). The launch captures
the stick direction as the article's facing, so a held ball can be blown in front of or behind the caster.
Optional `hold: { "offset": [x,y], "bob": 6, "period": 24, "hitboxes": [...] }` keeps an unreleased article at
a fighter-local point (x forward, y up) and bobs it vertically on its construction clock. The held hitboxes are
active while the article is unreleased, share the article's contact history, and use construction age for
`start`/`end`. The first stage still supplies the standby shapes, and the last stage reached at construction age
is frozen on release.

`Phase.reflector` is an inclusive frame window with radius/height, projectile speed and damage multipliers.
It operates during invulnerability and preserves the phase, unlike a counter stance. `Phase.travelSpeed`
captures a normalized movement-stick direction once on phase entry (neutral means up), then travels at that
speed without gravity or drift until the phase changes. Normal stage collision still applies; `ledgeGrab`
controls whether travel can catch a ledge. `hidden` affects only rendering; author invulnerability separately.
Optional `stats.dashSpeed` / `dashFrames` override the shared initial dash, independently of run speed.
`Throw.ticks` are damage-only contacts with the held victim before the release hit, each with `at` and `damage`.

## Aerial tethers, follow-ups and status hits

The optional `moves.zair` slot is selected by grab while airborne; omitting it preserves the existing air
controls. It has the same phase, landing and animation semantics as any other move. The editor can copy a
move into this optional slot. `Phase.tether` is a fighter-local forward/up rectangle extending ledge reach
while `ledgeGrab` is open. Only ledges outside the block and in front of the fighter qualify; ordinary
occupancy, down-input rejection and regrab rules still apply. A successful tether uses the shared ledge
snap/hang, rather than simulating a rope. Split phases to close a tether window. `passPlatforms` suppresses
semisolid contact throughout its phase, without disabling solid collision.

`Move.oncePerAirtime` records the slot when the move starts. That special cannot restart until landing,
grounded ledge climb or respawn; damage and ledge hanging do not restore it. Ordinary air jumps and the
up-special resource remain independent. The used-slot list is rollback state. `Event.turn` reverses facing
before that event's velocity operations without reversing momentum.

`attackPress` and `specialPress` transitions accept fresh presses, rather than held buttons; `from` bounds
their input window. A buffered matching press may be accepted on the first legal tick. Acceptance emits a
structured `Followup { slot, phase }` action and consumes the command so it cannot become a delayed aerial/special.
`upHeld` follows the live up direction. Charge phases must leave room after the hold frame: if charge holds
on 20, a phase ending on 21 can transition into a release attack at its frame 0; ending on 20 would leave
the phase before it could hold. Entry and transition frames retain the existing convention.

`Hitbox.groundAngle` optionally overrides the launch angle for grounded victims, before the shared ground
bounce. `Projectile.chargeScale` optionally scales `bkb`, `radius` and `life`, alongside speed/damage, by
`1 + chargeTicks * value`; lifetime rounds to whole ticks. `angleTo` and `groundAngleTo` interpolate toward
maximum-charge launch angles using the move's charge maximum. `stats.pummelDamage` optionally overrides
the shared pummel damage while retaining its shared timing.

`type: "paralyze"` freezes the victim before launch, using `hitTypes.paralyze`: duration is reference-weight
knockback in source units times hitlag multiplier, plus `extraFrames`, floored and capped by `maxFrames`.
The victim's pending velocity/DI stays frozen with the state clock. Repeated paralysis instead causes an
ordinary hit until the victim becomes actionable, lands or grabs a ledge. This immunity is rollback state.
Attackers retain ordinary melee hitlag; projectiles do not freeze their owner.

`type: "bury"` traps grounded victims in the `buried` state. Equal-stock duration is
`ceil(baseFrames + postHitPercent * perPercent + referenceKnockback * perKnockback)`, capped by `maxFrames`.
Fresh directional inputs subtract `mash`; button presses subtract `buttonMash`. Weak follow-up hits below
`releaseThreshold` source knockback preserve the trap, stronger hits launch normally, and grabs cannot
capture buried fighters. Airborne/knocked-down targets launch normally. Stock-advantage modifiers, mash
rate limiting and deferred launch from weak hits are not simulated. The built-in 600-tick ceiling is an
adapted bound, not an SSBU rule. These hit types require their corresponding optional shared tuning objects.


## Optional named-joint figures

`fighter.rig` replaces the legacy humanoid drawing with a named-joint figure. Omit it to preserve
existing rendering. Presets live in `assets/templates/rigs/{humanoid,quadruped}.json`; the editor's
**Rig** section can add either preset and edit its schema-backed fields.

A rig has `joints` and optional `bones`. Each joint has a unique `id`, optional `parent`, local
`translation: [x,y]`, optional `angle` in counterclockwise degrees, and optional vector `shapes`.
Parents may be declared after children. Multiple roots are allowed; cycles and missing references
are rejected. Bones connect `from` and `to` joint IDs with a positive `width` and `color`.
Bone order is drawing order, followed by joint shapes in joint declaration order.

`Pose.joints` maps joint IDs to optional local `translation` and `angle` overrides. Components inherit
from `stand`, then the `from` chain, then the authored pose, with rig rest transforms as defaults.
Translations replace rest values, rather than adding to them. Keyframe interpolation preserves authored
unwrapped rotations; between-tick presentation interpolation takes the short angular path.
Existing whole-figure rotation, pivot and vertical offset still apply. Pose evaluation is shared pure
Rust in `sim::pose` and `sim::rig`.

Rigs affect appearance by default. Optional attached combat geometry is described below; stage bodies
remain independently authored. Legacy attachments and humanoid dodge afterimages are suppressed for
rigged figures. Joint shapes can use `when: glow` while attacking, charging, or countering. Player palette
roles retain body status tinting; fixed RGB colors remain authored colors.

Named-joint poses and rigs support forms and the generic mouse handles described below. Legacy
humanoid handles and onion skins are disabled for these rigs. Removing a rig through the editor also
clears its named-joint pose keys, as one undoable edit. No automatic gait conversion or collision binding
is implied by choosing a preset.

### Mouse rig authoring

In Fighters → Rig, the preview explicitly shows rest geometry. Select a joint in the list or viewport,
then drag its handle to change its parent-local rest translation. Parent rotations, facing and body
transforms are inverted before storing coordinates. Shift snaps and Alt uses fine movement.
Enable **Add connected joint**, then click the viewport to add a child of the selected joint; the
new stroke copies a connected/template stroke's style. With no selection, placement adds a root.
A blank rig starts with one root. The selected joint's rest angle and vector shapes remain editable
in forms. Bones and parent links are shown as viewport guides.

**Rename joint & references** updates parents, bones, named poses and inline animation pose keys.
**Delete leaf & connected strokes** removes a leaf and its incident bones, rejecting joints with
children or pose bindings with an explanation. Remove those bindings/children first, or remove the
whole rig with **Remove rig & joint keys**. Snapshot undo and inheritance-aware saves apply normally.
In Poses, dragging named joint handles stores local translation overrides; rotations stay in the
inspector. This workflow does not provide shape mouse editing, inline pose mouse editing or IK.


### Opt-in attached combat geometry

Move-phase hitboxes may set `geometry` to replace static `x`, `y`, `r` (those required legacy fields
remain serialized but are ignored for collision). `frames` must be empty when geometry is supplied.
For example:

```json
"geometry": {
  "kind": "capsule",
  "a": { "joint": "hand", "offset": [0, 0] },
  "b": { "joint": "weaponTip", "offset": [0, 0] },
  "radius": 4
}
```

A circle has `kind: "circle"`, `center` (a bound point), and `radius`. A bound point has a required
finite `offset: [x,y]` and optional `joint`. Without `joint`, coordinates are feet-relative forward/up,
unaffected by pose body rotation or vertical offset. With `joint`, the point includes animated parent
rotation, joint rotation, body pivot/rotation/offset, facing, and world placement. Only named rig joints
are anchors; cosmetic bones are not. Radius must be finite and positive. Runtime evaluation assumes
validated content and fails explicitly on invalid programmatic bindings rather than treating them as misses.

Optional fighter `hurtRegions` is a **nonempty** array of `{id, geometry}` with unique nonblank stable
IDs. It replaces both default and knockdown rectangles in all states; regions follow animated joints,
with activation described below. It requires `combatCenter: [x,y]`, a
stable root-local forward/up autolink target unaffected by the pose. Without regions, legacy hurt
rectangles and the default-rectangle autolink target remain exact fallbacks. Stage `body` is independent.

Only fighter move attacks accept explicit geometry. Throws, ordinary projectile hit definitions,
article timeline attacks, and global ledge/getup attacks reject it. Projectiles and articles retain
authored root-relative attacks and correctly contact fighter regions. Reflectors remain independent;
shield/parry/counter ordering still follows hurt overlap, without a separate shield collider. Throws
and pummels remain scripted. Region overlap produces one victim-level contact, respecting attack-array
priority, rehit clocks and preserved contact histories. Capsule hit effects/autolink use its midpoint.

Collision samples integer tick poses, never rendering interpolation, override poses, jitter, shape
appearance or attachment positioning. `global.gait.poseRunSpeed` replaces the former render overlay
threshold for selecting run poses; its compatibility default is the built-in 0.8. The legacy
`render.overlays.runSpeed` field is retained for decoding and no longer selects poses. Old embedded content
without this field decodes with that default. The optional fields need no replay container bump;
`sim-20` rejects prior simulation recordings explicitly. Bundled resolved content hashes change due to
the new gameplay tuning; fighter source geometry is unchanged.

Editor forms author attack geometry and regions. Bound attacks have no legacy circle handles or
keyboard nudges; inverse geometry editing is deferred. Debug/editor outlines use exact tick shapes.
Renaming joints updates collision references; leaf deletion rejects bound joints. The explicit
**Remove rig, joint keys & bound collision** action clears the region set if any region binds a joint,
and removes joint-bound attack boxes (root-only attacks remain); undo restores the edit.

Deferred: broader size integration beyond the stable interaction points below,
contact groups, IK, shield colliders, and advanced grab/ledge choreography. This is a first slice of
milestone C, not completion of it.

### Named hurt-region activation

Each region has optional `enabled` (default true) and `states` (default empty).
For example `"states": [{"state":{"state":"idle"},"enabled":true},
{"state":"crouch","enabled":false}]`. State names use the existing lowercase
State enum. The special typed `crouch` selector applies only to grounded idle
fighters in Gait::Crouch and takes precedence over idle (Rise uses idle).
At least one region must be enabled by default.

Move phases may author `hurtRegions: [{"region":"head","enabled":false,"from":2,"to":5}]`.
Windows use inclusive current `MoveState.t`, reset with phase transitions, and
ignore elapsed move time and rendering clocks. Outside a window, state/default
activation returns. Intervals must be ordered, end before a finite phase's end,
and never overlap for the same region. Loop phases permit finite windows.
Unknown or deleted IDs are errors with phase/reference paths. Other contexts
(projectiles, articles, throws) do not accept region windows.

All contact queries and exact paused tick overlays filter through the same resolver.
An intentionally empty active set has no damage, grab or proximity contact and
never restores the legacy rectangle. Global hittability/invulnerability rules still
apply. Fighters without regions retain legacy behavior. Editor forms expose default,
state and phase activation; region rename updates phase references, and deletion is guarded while referenced. External edits report missing IDs through validation. Stable grab/ledge anchors are described below; broader size integration, advanced choreography and IK remain pending.

### Stable grab and ledge interactions

Optional `fighter.interactions` owns stable feet-relative points (`x` forward, `y` up).
They do not follow animated joints, hurt regions, or `combatCenter`. Each field is optional:

- `grab.catchPoint`: holder reference for vertical grab tolerance, default `[0,0]`.
- `grab.holdPoint`: holder target for the captive anchor, default `[global.sim.grabOffset,0]`.
- `grab.heldAnchor`: victim attachment point, default `[0,0]`. The victim faces opposite the holder.
- `ledge.catchPoint`: ledge proximity reference, default `[0,stage.ledge.handY]`.
- `ledge.hangAnchor`: attachment to the block corner while facing inward,
  default `[stage.ledge.hangX,stage.ledge.hangY]`.
- `ledge.pullIn`: first climb segment's inward endpoint, default `global.sim.ledgePullIn`.
- `ledge.climbDistance`, `rollDistance`, `attackDistance`: second segment inward endpoints,
  default the corresponding `global.ledge` option's `dist`. Jump retains its existing behavior.

Points must be finite; distances must be finite and nonnegative. Objects inherit key by key;
`null` removes an inherited key and restores its fallback. Omitting all overrides retains legacy
arithmetic. The fighter inspector exposes these fields through schema forms.

Throws move the holder's target along the existing forward/back/up/down paths and subtract the
victim's mirrored anchor. Release uses that tick's placement. Captives have no scripted stage collision.
A grab object on either participant clears captive surface support; absent objects retain legacy support
behavior. Holder physics runs before attachment. Platform carry still precedes fighter updates and hitlag
still freezes each fighter: a captive can lag behind a moving holder during pummel hitlag until attachment
runs again. There is no extra attachment pass or added platform velocity.

Tether reach rectangles remain root-local. Ledges belong only to static blocks. Validation checks each
fighter's mirrored hanging body against neighbouring blocks, plus the owning block when `hangAnchor`
is overridden. Authored pull-in and climb/roll/attack endpoints must stay over their supporting block and clear all solids. Interaction coordinates and distances are limited to magnitude `1e100` to leave arithmetic headroom. Spawn, respawn and moving-platform clearance remain independent. Hang and climb endpoint
checks do not guarantee swept climb safety; authors must inspect paths around overhangs.
This slice does not resize pushing, shields, cameras or effects, retarget throw poses, or add IK.
Simulation recordings now use `sim-21`; the replay container version is unchanged.

`global.render.select.backHoldSecs` is the positive real-time hold duration for leaving selection screens (default 1.5 seconds for older embedded presentation data). UI updates cap each contribution at 50 ms. Contextual Back actions consume the hold until release.

`global.render.select.backRing` draws that hold as a ring of `ticks` dashes around the player's pointer arrow (`offset` from the arrow's tip, `radius`, `thickness`, `dash` length, the unfilled `track` opacity, and each dash's dark `rim`), the `hot` dashes behind the filled end whitening like a comet head around the `spark`. It pops in from `pop` scale on the `grow` spring as a hold starts, follows the hold on the `follow` spring, and swings `wobble` radians every `wobbleSecs`. A released hold drains the ring, which then shrinks away; no text is drawn.

`moves.*.phases[].groundSpeed` (default 0) enables grounded stick-controlled movement during
an attack phase, in pixels per tick at full tilt. It preserves facing and stored charge; zero
uses ordinary attack friction. Airborne phases retain their existing drift control.

`global.gait.crawlSpeed` enables movement while crouching, as a fraction of walk speed
(default zero for older content; bundled tuning uses 0.35). Down plus horizontal stick keeps
the crouched pose and proportional low movement speed. Release down to resume normal gait.
Jump, attacks and platform-drop inputs retain their normal priority. This is a shared game
rule, rather than a claim that every reference fighter can crawl.

`global.render.overlays.grabbed` optionally draws persistent binding bands on a captured fighter.
Each `bands` entry is [height above feet, horizontal radius, vertical radius] in fractions of body height; `width`, `color`
and `alpha` control the stroke. It appears whenever the fighter is grabbed and disappears on release.

`projectiles.*.article.reflectCooldown` defaults to zero and sets how many ticks any reflector is ignored after article reflection or a counter steals a held article. The old caster receives `articleEnd` when its held article becomes an independently owned shot.

### Resource restore, remote gadgets and float

Move events may use `restore: {"charge": "<pool id>", "uses": 1}` to add uses, capped at the pool's
capacity. The pool must exist and uses must be at least 1. Restore preserves an active recharge countdown,
including when an empty pool becomes nonempty; reaching capacity clears the countdown and plays
`refillSfx` once. A pool with no active countdown stays inactive. This also permits explicit refill of
`recharge: 0` pools. Within an event, spawn runs before restore, then detonate, then sfx and fx.
Phase entry costs still run before frame-zero events.

`detonate: true` immediately explodes every live projectile currently owned by the fighter that has a
**manual** `explosion`, in projectile order, even before its arm time. Timed fuses are not remotely fired and
keep running. No remote gadget is a no-op. Each uses the normal
blast radius, hit, target masks, opponent/team eligibility, shield/counter handling, article contacts,
damage/healing credit, hit records, fx and sfx, then is removed. It does not advance movement or age.
A reflected gadget belongs to its current owner.

`explosion.manual: true` (default false) disables proximity and fuse detonation and freezes the life
countdown. It still moves, collides, can be reflected/batted, clashes with opposing projectiles and is removed
outside the stage bounds as usual. External removal is still possible. With manual false, existing proximity
and fuse behavior is unchanged. For a planted remote bomb, declare a normal projectile with `vel: [0, 0]`,
an `explosion` containing `manual: true`, `armTicks` and `radius`, plus its ordinary `hit`, positive
`life`, radius and presentation fields. Use existing gravity/bounce data to settle it on terrain if needed;
trigger it from another move's detonate event. This is a remote blast gadget, not a pickup or trip-item system.

Optional `stats.float: { speed, frames }` is a Peach-style float. In the air (normal `Air` control), holding jump
while not rising, or a jump press with down held (instead of the air jump), starts it once per airtime: for up to
`frames` ticks the vertical speed is held at `speed` (0 hovers; positive sinks) while drift works as usual. An aerial
started during the float keeps it (the float continues through the aerial and after it); fast fall is ignored while
floating. Releasing jump, the frame limit, an air jump, a hit, a ledge, any non-aerial move or other state, and
landing end it; only landing makes it available again (`Fighter::floating` / `float_used` in the snapshot). The air
jump stays available. Optional `groundHeight` (px, default 0 = off) adds a ground float: leaving jumpsquat with down
held and the float unused lifts the fighter `groundHeight` above the ground with no vertical speed and starts the
float at once, instead of a short or full hop (the jump button with down, or a tap jump pulled down before
jumpsquat ends; the float then runs as above, so with tap jump alone it ends as soon as the stick leaves up and
jump is not held). SIM_VERSION -sim-34. This replaced `stats.floatSpeed` (a descent cap while jump was held, with no duration or latch);
SIM_VERSION -sim-33.

`restore`, `detonate` and `explosion.manual` add no rollback snapshot state; content that omits them keeps its
behavior.
