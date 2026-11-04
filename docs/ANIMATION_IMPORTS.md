# Animation and pose passes for imported fighters

This note records how visual motion is added to a fighter package without touching its gameplay. It
complements [CHARACTER_IMPORTS.md](CHARACTER_IMPORTS.md), which covers reference research and numeric
conversion, and [FORMAT.md](FORMAT.md#visuals), which defines poses, animations and attachments. The
companion passes in this change set cover the shared `base` movement states
([ANIMATION_MOVEMENT.md](ANIMATION_MOVEMENT.md)) and the original fighters' attack tracks
([ANIMATION_ROSTER.md](ANIMATION_ROSTER.md)); this note covers imported packages, whose own `stand`/`crouch`/
`air` poses anchor their state animations.

## What an animation pass may change

The point of this work is that a fighter should *read* as designed and responsive while its frame data
stays exactly as authored. The user-facing complaint ("she feels stiff") is not a request for a faster
move. Change only presentation:

- allowed: `poses.*`, `anims.*`, a move phase's `anim` keys, attachment `angle`/`show` keys outside active
  hitbox windows, attachment shape geometry;
- not allowed here: phase `frames`, `start`/`end` hitbox windows, damage/launch/hitlag, `setVel`/`move`
  events, phase transitions, charge frames, physics stats, input routing, facing rules.

The engine samples integer tick poses for collision only through an optional named `rig` and joint-bound
hurt regions. Neither imported fighter in this pass has one, so `poses` and `anims` are cosmetic by
construction; a package that does have a `rig` must keep its bone transforms aligned with the reference
and must not gain pose-driven gameplay by accident.

## Rules of thumb

- **Keep contact frames exact.** A phase `anim` key that holds during an active hitbox window fixes the
  limb the hitbox was fitted to. Add anticipation before it and follow-through after it instead of
  retiming the hold. The King hammer and Whiplash flip kick keep their fitted contact joints bit-for-bit.
- **Weapon anchors.** A keyed attachment angle hangs the weapon off the named joint, and a `hold` entry
  pose the front arm from that angle. Changing the shoulder height of a pose used while the angle is keyed
  would slide the drawn weapon away from its fitted hitbox path. New carry/brace poses therefore keep the
  shoulders of the pose they replace.
- **State animations are the loudest part of the game.** Idle, run, crouch, rise, fall and helpless are
  visible far more often than any single move; a live fall loop or a landing absorb sells weight with no
  frame data at all.
- **Respect the real durations.** Bundled tuning uses a 3-tick jumpsquat and a 3-tick hard landing. A
  squash or absorb that only reaches its extreme after 10 ticks is invisible on a jump; author the
  readable compression inside the first ticks and let move landing lags (9–18 ticks) use the later
  recovery keys. A state anim is cut when its state ends, so the last visible key before that cut should
  be close to the pose the next state starts with.
- **Clocks.** `state` is frames in the state, `world` is the tick counter (good for breathing, fall
  flutter and flail loops), `run` is the run-cycle angle and `spin` the tumble spin. `loop` repeats the
  clock, so a `world` loop gives a fully self-contained idle.
- **Hold semantics.** A key holds until the next key; `lerp` blends from the previous key to this one;
  `if` conditions (`grounded`, `airborne`, `countered`) skip a key. A key after the last visible tick is
  just a target the sampler never reaches.
- **Label authorship.** Reference GIFs usually prove attack silhouettes only. Jumps, crouches, landings
  and idle loops authored without a matching reference frame are *fitted presentation*, not imported
  source animation. Say so in the fighter's import record.
- **Do not re-pose fitted attack beats.** Whiplash's flip entry/stretch/tuck/recover and King's Jet
  Hammer swing were fitted against per-frame captures. Locomotion passes reuse existing poses as
  bookends where possible instead of re-deriving them.

## Verification

Fast checks that catch the usual mistakes:

1. `cargo test -p sim --test animation_response` samples resolved poses and asserts the intended ordering
   (load deeper than idle, launch extended past standing, landing absorb deeper than the load, hold keys
   returning to their anchor angle, live `world` loops).
2. The fighter's own behaviour tests (`--test king`, `--test whiplash`) confirm contact geometry, charge
   and flip windows are unchanged.
3. `cargo test -p sim --test content` confirms the package still validates and is canonically formatted
   (`just bless` regenerates schemas/formats assets when content types change).
4. `just shots <dir> fighter-kits,training` captures the result. A successful capture is not visual
   verification: open the PNGs and check takeoff, apex, landing, the charge carry and any weapon that is
   visible outside its hit window.

`just import-king` / `just import-whiplash` regenerate the packages, so any pose or animation intended to
survive must live in `tools/import-*/import.py`, not only in the JSON. Each generator writes its own
package and normalized ledger; both are reproducible byte for byte after `just bless` formatting.

## This pass

King (`tools/import-dedede/import.py`):

- takeoff reads as half crouch → deep load (arms swept back) → full-body launch extension → rise tuck;
- falling braces the legs and pumps the arms on a `world` loop, and helpless flails instead of freezing;
- landing absorbs well below any stance, settles, then stands; idle and run gained a weight bob;
- Jet Hammer carry is a braced stance with a strain wobble on the held hammer that returns to the entry
  angle, so the release starts from the grip the player saw. The charge phase still has no hitboxes and
  the swing/recovery hitbox tracks are untouched.
- Super Dedede Jump's squat phase leans into the retained capture's settle pose before the leap extends.

Whiplash (`tools/import-zss/import.py`):

- her own crouch, air, rise, fall and landing poses replace the generic inherited states; the landing
  absorbs and recovers, the rise reaches with both hands, the fall keeps its legs drawn up on a `world`
  loop and helpless flails loosely;
- idle/run use her own stride with her weight over the planted foot;
- the double-jump `air.flip` is her tight tuck somersault instead of a rotated generic pose;
- the flip kick keeps the just-fitted stretch/tuck/recover track and the fitted contact foot; only the
  rebound got its own recoil pose. The curved lash and its retraction are unchanged.

Mike and Wizard were reviewed and left alone in this pass: their weapon/bone tracks are reference-fitted,
they show no locomotion gap that justifies re-posing them, and Mike's existing idle/rise/fall already
override the inherited states.

## Known limits

- State animations cannot know *how long* the state will last, so one landing track serves a 3-tick hard
  landing and an 18-tick aerial landing lag. The short case intentionally ends near the idle pose.
- A midair jump does not reset the air-state clock (`flip_t` only covers the flip animation), so repeat
  rise poses show the settled rise key rather than replaying the takeoff extension.
- Whiplash locomotion uses dedicated `locomotionAir` and `locomotionCrouch` poses. The original inherited attack bookends remain intact, including their active-window interpolation.
- No new shared clock, per-fighter pose blending or engine behavior was added. These passes are content
  only, so they need no `SIM_VERSION` change and do not affect replays, traces or the golden world hash.

Integration review moved jump-load endpoints to tick 2 and added near-standing landing keys at tick 2, so the short hard landing reaches recovery before Idle. These fixes live in the generators too.
