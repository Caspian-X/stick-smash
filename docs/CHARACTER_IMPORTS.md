# Importing a reference character

Use this workflow when adapting a character from another game. The result is an editable fighter package,
a source/conversion record, and tests of the intended behavior. A reference page starts the research; it
does not imply that every property can be imported automatically.

Start with [DEVELOPMENT.md](DEVELOPMENT.md) for repository rules and [FORMAT.md](FORMAT.md) for actual data
semantics. Copy the [reference-record template](templates/FIGHTER_REFERENCE.md) into `docs/imports/<id>.md`.
[Mike's record](MIKE.md), [package](../assets/fighters/mike/fighter.json), and
[tests](../crates/sim/tests/mike.rs) are a worked example, not universal conversion constants.
[Wizard's Zelda record](imports/wizard.md) covers a complete replacement, pinned-script regeneration,
animated bone fitting and independent controlled projectiles.
[Whiplash's Zero Suit Samus record](imports/whiplash.md) covers aerial grab/tether attacks, once-per-airtime
specials, follow-up input windows, paralysis and grounded bury effects.
[Planty's record](imports/plant.md) covers a nonhumanoid named rig, charge-tiered joint-bound reach,
a held/controlled projectile and a damage-only lingering hazard.
[Throbbin's Robin record](imports/throbbin.md) covers limited-use charge resources with timed recharge,
resource-selected move variants, charge-tiered specials, a healing unblockable hit and a Bronze fallback.
[Sneak's Snake record](imports/sneak.md) covers exploding gadgets: a bouncing fused grenade thrown along a
stick-selected arc (`releaseToward` / `releaseAway`), a planted remote C4 (`explosion.manual` + `detonate`
driven by a charge pool), a steered `article` missile and a ballistic mortar shell.

## 1. Establish the target

Record the requested name/id, branch, source game and patch, and the desired feel. Separate the character's
identity (movement, range, rhythm, recovery, combo tools) from exact numbers the user wants preserved.
Inspect the current checkout before editing; preserve other work. Use a requested branch without resetting
or discarding its changes. Ask only for missing information that changes the implementation.

Inventory the destination slots from `Moves::SLOTS` in `crates/sim/src/data/moves.rs`, plus `Throws`:

- jab, forward/up/down tilt, dash attack;
- forward/up/down smash;
- neutral/forward/back/up/down aerial;
- standing/dash grab, four throws, shared pummel;
- neutral/side/up/down special (`sspec` is optional).

Map source move names and variants to slots and phases before authoring. Record omissions and inherited
placeholders explicitly. Multi-part jabs, charges, grounded/air variants, landings and whiffs often need
multiple phases within one slot. Do not add an input slot just to name another phase.

## 2. Gather complementary sources

For an Ultimate Frame Data import, inspect all three layers before concluding information is unavailable:

| Evidence | Extract | Limits |
| --- | --- | --- |
| Character page text | Startup, active windows, total duration, damage variants, landing lag, autocancels, notes, movement stats where supplied | Does not establish numeric bone transforms or all launch rules |
| Linked hitbox GIFs / frame viewer | Per-frame coverage, weapon path, attack direction, pose, sweet/sour overlap | Image coordinates are a visual reference, not automatically world-space geometry |
| Versioned numeric attack scripts | Bone, sphere/capsule offsets, radius, damage, launch angle, knockback fields, hitlag, masks, rehit and conditional rules | Animation frames may be retimed; bone-local positions need animated transforms |

Download original animations where accessible and inspect representative individual frames/contact sheets.
Do not rely on a single composite screenshot. GIF duration metadata can differ from the gameplay timeline;
identify what each image frame represents. A release-only animation is not the entire charged move.

Mike cross-references the [Ike UFD page](https://ultimateframedata.com/ike) and
[Rubendal's patch 13.0.1 data](https://github.com/rubendal/ssbu/blob/master/public/data/patch/13.0.1/character/Ike/data.json).
Locate the corresponding character and verify the dataset structure for each new import. Pin the script
repository commit as well as its patch directory. Log URLs, access dates, filenames, hashes/revisions and
the command used to acquire/process material. Keep the normalized facts and authoring inputs needed to
reproduce the conversion in the repository when practical; a `/tmp` file is not a durable source record.

Resolve disagreements per property. State which source wins and retain the conflicting values with the
reason. Never silently mix patches or treat a mod's scripts as the unmodified game. If a generator is
used, retain it and its inputs, and expose its repeatable command through `justfile`; describe which
authored fields it replaces. Do not present a hand-fitted import as a lossless automated extraction.

## 3. Normalize facts before authoring

Maintain a move ledger independent of the resulting fighter JSON. For every move/variant, record:

- startup, inclusive active windows, total duration, damage and hit priority;
- landing lag, autocancel intervals, charge hold/release timing and maximum charge;
- launch angle/direction/type, knockback fields, hitlag, ground/air masks and rehit timing;
- movement events, armor/invulnerability/counter windows, detection, hit/whiff/landing outcomes;
- source citation and whether each property is converted, fitted, shared, adapted or unknown.

Keep source animation frames, real gameplay frames, move-relative frames and phase-relative frames
distinct. Account for motion-rate commands. An imported move's events and hits must agree on one timeline.

Choose and document units once per import:

- The sim runs at 60 Hz. World Y points down; fighter-local hitbox/pose/event Y points up. X points forward
  from the feet and is mirrored by facing. See [coordinates](FORMAT.md#coordinates-and-units).
- Root-space script offsets can be scaled directly only after verifying axis mapping. For Mike, source
  root Z became forward X, source Y remained up, and the offensive geometry scale was 5 px/unit.
- Bone-local offsets require the bone's animated transform. With no original skeleton animation, fit a
  2D hand/weapon track from visuals, transform the offsets, and label the result **fitted**. Do not silently
  apply root-space conversion to a sword-bone offset.
- Use native capsules for capsule geometry; when an approximation is necessary, record its spacing/coverage rule. Preserve first-hitbox
  priority where sour spots, spikes or masks overlap. Hurtboxes and the stage collision body are separate.
- Document movement scaling independently from geometry. Weight here multiplies knockback taken (lower
  means heavier). Source knockback formulas and numeric fields are not necessarily interchangeable.
- Resolve special angle codes into the engine's typed hit behavior. Record unsupported codes and approximations.

Use Mike's conversion ratios only as documented starting points. Current shared walking/dashing,
buffering, DI, parry, shield and ledge behavior belongs to this game; importing another character does not
justify changing those globals to compensate for an input calibration problem.

## 4. Author the package and presentation together

Create `assets/fighters/<id>/fighter.json` with the relative schema, fighter schema version, display name,
and usually `extends: "base"`. Extending another playable fighter is appropriate only when its inherited
behavior is intentional. Objects merge; arrays replace whole; `null` removes a key. Inspect resolved
behavior so inherited grabs, throws, animations and specials are not mistaken for imported data.

Implement representative normals/aerials first to establish scale, then the remaining kit and recovery.
Use phases, transitions, events, charge, detection and hitbox tracks before adding engine behavior. Keep
poses, attachment/weapon tracks and collision coverage aligned. Unwrap animated angles across 360° when
necessary so interpolation follows the intended sweep rather than reversing between adjacent keys.

The phase entry tick is frame 0; `start`/`end` are inclusive phase frames. A phase with `frames: N` times
out at N. Inspect phase entry and transition ticks rather than globally shifting every source frame by
one. See [frame convention and tick order](FORMAT.md#frame-convention).

For genuinely missing mechanics, follow [Add a move behavior](DEVELOPMENT.md#add-a-move-behavior-or-event-kind):
add reusable data semantics, validation, schema/editor support and tests. Never dispatch behavior by
fighter id. Keep tunables in content and deterministic state in `World`. Bump `SIM_VERSION` for behavioral
or snapshot changes; content-only tuning does not automatically require a bump.

## 5. Verify two separate contracts

**Reference behavior:** use isolated move stepping to check first/last active frames, phase boundaries,
damage variants, charge extrema, masks, overlap priority, landings, autocancels and recovery outcomes.
Expected values should come from the source ledger, not be copied back out of the generated JSON.
Test boundary frames just before/at/after each significant change. Mike's tests demonstrate this style.

**Playable behavior:** use raw `Input` through `step_world` as well. Directly starting a move bypasses
controller gestures, buffering, source-specific charging and jump selection. Check jump+A, right-stick
smash/tilt and aerials, jab hold/release, recovery from both sides, and buffered actions after landing or
hitlag. Keep controller range/deadzone issues separate from fighter speed. Clone/restore or replay-test
new persistent state. Drain `fx`, `hits` and `acts` for long-running match tests as the game does.

Validate the imported fighter on every built-in stage: platform reach, ledge recovery, CPU matches with
both seats, KOs, self-destructs and completion. Add it to appropriate hard-coded test rosters or give it a
focused matchup test. An id appearing in the select menu does not establish that these checks include it.

## 6. Integrate, inspect and hand off

1. Update the roster expectation in `tests/content.rs::all_assets_load` and appropriate CPU tests in
   `tests/stages.rs`. Discovery is automatic; there is no central playable-fighter registry.
2. Add representative screenshot setups in `crates/game/src/shots.rs` and update its expected names when
   needed. Existing kit tours use fixed fighter lists, so running a section alone does not capture a new id.
3. Run focused tests, then the required content and workspace checks:

   ```sh
   cargo test -p sim --test <id>
   just bless
   just check
   just shots /tmp/<id>-shots fighter-kits,training,select
   just balance 16 <id>
   ```

   The first command assumes a dedicated `<id>.rs` test target; otherwise name the actual target/filter.
   Balance diagnostics are optional. Inspect generated content diffs after `bless`. Do not bless an
   unexplained world hash; roster sorting, changed representation and gameplay can each affect it.
4. Open actual screenshots and inspect them. Verify attack poses, alignment, facing, sweep interpolation,
   charge/release, aerials and recovery; use the editor timeline and training Inputs history to diagnose
   timing and routing. A successful capture is not visual verification. If egui is missing, ensure its
   primary context belongs to the window camera, not an off-screen sheet camera.
5. Playtest movement and attacks with the relevant controller, preferably an optimized build (`just release`).
   Record observations separately from measurements; never claim hardware calibration was measured if it wasn't.
6. Finish the reference record: implemented mapping, source disagreements, conversions, fitted/unknown
   properties, engine adaptations, commands/results, inspected captures and remaining work. Report actual
   branch/commit/push status. Do not claim exact fidelity for approximated bones, formulas or hurtboxes.

An import is ready when the full kit is accounted for, source choices and approximations are reviewable,
focused and workspace checks pass, representative visuals have been inspected, and traversal/recovery
work on the built-in stages. Leave incomplete variants explicit rather than hiding them behind inherited data.

For SSBU-style damage-dependent knockback, use the shared formula described in
[FORMAT.md](FORMAT.md#knockback-models), with unscaled source BKB and KBG divided by 100. Do not use
Mike's superseded BKB × 0.1 / KBG × 0.001 linear conversion for a new import. Test weak and strong hits
at multiple post-hit percentages, charge scaling and target weights; fixed/autolink hits need separate treatment.

Animation polish rationale: [shared movement](ANIMATION_MOVEMENT.md), [original roster](ANIMATION_ROSTER.md), and [imported fighters](ANIMATION_IMPORTS.md). These passes preserve gameplay timing and distinguish authored presentation from source fitting.
