# One-shot TypeScript content conversion

From the Rust repository root, with Node 26 and the read-only sibling `../stick-smash` checkout:

```sh
node tools/convert-ts/convert.mjs
BLESS=1 cargo test -p sim --test content
cargo test -p sim
cargo clippy -p sim
```

The converter resolves paths relative to itself, imports the TS modules directly, and registers an extensionless
`.ts` / directory `index.ts` resolver using `module.register`. Node 26 currently prints a deprecation warning for
that API; it remains the requested hook mechanism. There are no npm dependencies. Rerunning replaces the six
content assets; BLESS regenerates the three schemas and applies the Rust canonical JSON formatter.

Written content: `assets/global.json`, `assets/stages/arena/stage.json`, and
`assets/fighters/{base,brawler,swordfighter,king}/fighter.json`. Base is abstract; each playable fighter extends it
with a recursive object diff (arrays replace whole). The original two spawn points and first respawn are preserved;
six additional palette sets and six additional starts / seven respawns are deliberately authored additions.

## Rust compatibility changes

- `data/global.rs`: explicitly serialize/deserialize `dizzy_recover_hp` as `dizzyRecoverHP`. The requirement to
  preserve TS global keys conflicts with serde's automatic `dizzyRecoverHp` spelling.
- `data/moves.rs`: explicitly name the `Fx::Part` field `inheritVel`. Serde's enum `rename_all` renames variants,
  not fields inside variants, so the existing definition otherwise expected `inherit_vel`.
- `Event::fires` uses `is_multiple_of` in place of `% == 0`, retaining its nonzero guard, to clear the existing
  `manual_is_multiple_of` clippy warning. This does not change event behavior or the format.

No tests or game-crate files are changed.

## Timing audit against TS

References are to the sibling checkout's `src/sim/` sources at conversion time.

| Conversion | TS lines checked | Decision |
| --- | --- | --- |
| Standard moves | `moves.ts:27-50,95-105`, `data/index.ts` `moveTotal` | One phase times out at last hitbox end + endLag. Inclusive hitbox windows and geometry-frame entries are copied unchanged. Charge/smash/weapon/step/quake/dash events preserve ordering. |
| Ordinary rise | `moves.ts:97-105,117-127`, `physics.ts:78` | Startup lasts 4; x damping occurs on 1..3. Rise frame 0 is move frame 4 and launches immediately. Shift both hitbox endpoints and geometry keys by 4; timeout is total minus 4 (32 for Brawler, 34 for Swordfighter). Particles occur on rise 1..13, gated by world-frame parity. Ledge grabs begin on rise 3 (move 7). Landing lag begins at rise 3 because TS requires move t > rise.frame+2. |
| Ground pound | `moves.ts:18-23,85-94,117-125`, `combat.ts:31`, `physics.ts:75-78` | Startup enters fall/wave at move 8. Last startup damping still runs on 8. Fall frame 0 already has its hitbox and fall cap, but velocity/particles begin on frame 1. Fall timeout is 50-8+1 = 43, so cancellation is move 51. Grounded is checked before timeout; there is deliberately no land transition, so a landing enters wave on the next step. Wave is active on 0..5 and ends on recovery 22. |
| Super jump | `moves.ts:62-77,117-127`, `combat.ts:33-34`, `physics.ts:76` | Squat x damping runs on 1..8 before launch at 8. Squat ledge threshold is 7; rise/plunge/wave thresholds are 0. Rise hits on phase 0..9 (move 8..17; TS t < 18). Rise particles start at phase 1, including the tick whose falling transition enters plunge. Plunge entry damps x and plays smash; forced plunge velocity, plunge particles and shield cancel begin on frame 1. Both rising and plunging land into wave immediately. Wave hits on 0..5 and ends on recovery 28. |
| Counter | `core.ts` `countering`, `moves.ts:78-84`, `combat.ts:32,193-203`, `projectiles.ts` reflection | Stance window is inclusive 4..24; stance timeout 44. Counter entry sets strike frame 0, emits the trigger effects, then strikes on 3..7 and ends on 26. Counter damage is disabled for reflected projectiles, including the strike slash sound. Reflect life is 75; Gordo reflected y-up velocity is 4. |

An additional temporary Node check executed actual TS `stAttack`, `activeBoxes`, and `counter`, and compared
standard totals/windows, rise launch/windows/particle ticks, airborne and grounded pound timing, super-jump
rise/plunge timing, and counter strike timing against the generated assets. This is a source trace check, not a
Rust-vs-TS full simulation replay: the Rust gameplay sim is still a stub.

## Approximations and format limits

- All floats are rounded to three decimals, including animation periods, radians-to-degrees conversion, and
  vector shapes. Sinusoidal world/run animations use eight interpolated intervals. Short nonlinear throw and
  climb arcs retain more keys; constant holds and linear interior samples are compressed.
- State and move samples use breathing at world frame 0 except world-clock animations. Thus run, move bodies,
  and state-clock poses freeze the secondary breathing motion. World loops retain sampled breathing, but its
  separate frequency need not close at the primary shake/sway/dizzy/win loop boundary.
- `air.flip` uses the rising-air pose throughout its 20-frame rotation. Tumble uses positive-vx spin at the TS
  rate (0.3 radians/tick), with the shake frozen at world frame 0; velocity-dependent spin direction and initial
  spin phase are not expressible in the state animation format.
- Ground-pound startup samples the airborne spinning pose; the same phase is also used when grounded. The
  format has no grounded/airborne conditional pose key, so the grounded version also spins. Hitboxes and
  gameplay events still use the proper grounded conditions.
- Climb keys bake a representative left-ledge pull-up, using the default stage's hang geometry and TS easeOut
  position for hand IK. Geometry on custom stages may need new keys. Ledge roll follows the same representative
  climb path; ledge-jump samples stop at the configured end (7).
- Weapon body presets freeze breathing. Hair tie is a static line (no world-clock sway). Sword geometry includes
  counter glow colors/widths but omits the blinking star at the tip. Weapon trails retain the format's inner/outer
  radii; TS's exact procedural trailing wedge and four-frame fade are renderer behavior, not keyed shapes.
- The held Gordo follows an eight-key approximation of the throw arc, with static spikes and eyes; its TS
  0.3-radian-per-frame prethrow spin is omitted. Flying Gordo spin converts the signed y-up rate, but the format's
  distance-based rotation does not encode TS's absolute-world-x initial orientation. The energy charge orb uses
  separate glow/core attachments so their differing radius growth remains correct (subject to rounding).
- Palette additions, extra starts/respawns, and the stage name are authored choices. Camera bounds use the exact
  background extents. StageArt holds the six requested colors; TS platform-shadow opacity, ground decoration,
  backgrounds and stars are outside that format.
- TS procedural fighter overlays (hit flashes, charge jitter, dodge ghosts, shield bubble, dizzy orbiting stars,
  respawn ellipse, state-dependent face/eye drawing) have no corresponding content fields and are not converted.
  Skeleton poses, style, crown, hair tie, weapons, held props, projectile drawing and requested event FX are data.
- Particle ranges preserve their distributions and draw counts. Negated sorted ranges (e.g. King's downward
  air-jump vy) invert the individual random sample within the range relative to TS; this is a cosmetic difference.
- The prescribed new per-tick order intentionally uses the current phase after transitions. TS `stAttack` keeps
  the old move's metadata for its final physics call even after `setState` ends the move. Consequently ending
  ticks can differ in friction, drift or fall cap (notably pound timeout); preserving that legacy tail together
  with immediate endings needs engine support rather than different asset frame counts. Phase boundaries,
  windows and event ticks above preserve the requested convention.
