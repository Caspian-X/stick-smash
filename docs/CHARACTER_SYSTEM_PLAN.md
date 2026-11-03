# Character system and creator plan

Status: **implementation started; most milestones remain planned**. Recorded 2026-11-03 following an architectural review
and discussion of varied fighters, reference imports, and a mouse-driven character creator.
Exact schemas, APIs, and milestone boundaries remain proposals. This document does not change the
current content format or authorize an engine rewrite.

Related: [design decisions](DESIGN.md), [content format](FORMAT.md),
[character imports](CHARACTER_IMPORTS.md), [development guide](DEVELOPMENT.md).

## Implementation progress

The first foundation slice adds optional generic named-joint figures, deterministic shared pose evaluation,
humanoid/quadruped data presets, renderer integration, validation, and schema-based editor rig/pose forms.
Existing fighters keep their legacy drawing and gameplay geometry. This is partial milestone B with an
initial editor interface, not completion of A–D. The editor now adds rest-only rig preview, generic joint selection/dragging, connected joint placement,
a blank starter, guarded rename/delete, and named pose translation dragging. Shape editing remains numeric/schema
based; IK and inline pose mouse editing are not implemented. The first opt-in attached collision slice adds circle/capsule move attacks and named hurt regions,
shared integer-tick placement, projectile/article routing, forms, and exact overlays. This is partial
milestone C. Named hurt regions now support default activation, typed state/crouch overrides and inclusive phase-tick windows through the shared collision/overlay resolver. Stable root-local grab/throw and ledge interaction overrides now support unequal body sizes, shared placement, validation and schema forms (partial milestones C/D). Pushing, shield/camera fitting, contact groups, IK, shield
colliders and advanced ledge/grab choreography remain deferred. Import overrides and later
combat/resource/object work also remain outstanding. See [format](FORMAT.md#optional-named-joint-figures).

## Goals

- Make varied fighters easier for humans and LLMs to author, inspect, and tune.
- Preserve the stickman aesthetic while supporting different proportions and body structures,
  including four-legged animals, tails, and additional limbs.
- Keep character behavior and geometry data-driven, with reusable mechanics rather than fighter-ID branches.
- Make imported facts, fitted adaptations, and deliberate tuning distinguishable and reproducible.
- Extend the existing fighter editor with a Character Creator / Body workspace.
- Preserve deterministic simulation, rollback snapshots, and useful compatibility with existing content.

Accurate frame data is only part of character feel. Movement, collision geometry, hurtbox exposure,
input routing, hitlag, launch trajectories, and stage scale also matter. These changes should make
iteration explainable and repeatable; they will not eliminate playtesting or make imports lossless.

## Current foundation and limits

The phase/event/transition move system remains the backbone. Keep the Bevy-free sim, JSON schemas,
content validation, inheritance-aware editing, timeline previews, source records, and regression tests.
The review found no fighter-ID-specific gameplay dispatch in the sim.

The main extension points are:

- `sim::data::visual::Pose`, `render::pose::Skel`, drawing, and editor handles assume eleven humanoid
  joints. Positions are absolute fighter-local coordinates; proportions are repeated across poses.
- Appearance already supports per-fighter stroke widths, head size, and vector attachments, but the
  core limb connections and circular head are drawn procedurally.
- Hurtboxes are default/knockdown rectangles. Attack circles have root-relative geometry tracks;
  they do not follow the rendered skeleton.
- Pose sampling and attachment-driven arm positioning currently live in rendering code.
- `HitType` mixes contact behavior, launch rules, and status/effect behavior into one choice.
- Move charge is local to a move; persistent ability restrictions use dedicated runtime fields.
- Articles support construction/travel and a released attack timeline, but cannot combine with
  several existing projectile features, including bounce, explosion, batting, and charge scaling.
- Reference generators duplicate conversion logic and overwrite whole authored fighter packages.
- The editor previews moves directly, but does not yet provide a general reusable raw-input scenario
  format with explanations of transitions, collision decisions, and rejected commands.

Relevant code: [visual data](../crates/sim/src/data/visual.rs),
[fighter data](../crates/sim/src/data/fighter.rs), [hit data](../crates/sim/src/data/hitbox.rs),
[move data](../crates/sim/src/data/moves.rs), [runtime state](../crates/sim/src/world.rs),
[pose evaluation](../crates/render/src/pose.rs), [drawing](../crates/render/src/draw.rs),
[editor viewport](../crates/editor/src/viewport.rs), [preview](../crates/editor/src/preview.rs).

## 1. Rig, appearance, and collision architecture

### Generic structure, approachable templates

Define a rig using stable named joints, parent relationships, rest transforms, and named attachment
points. Resolve names into a validated deterministic evaluation order. Do not introduce a separate
hard-coded quadruped skeleton beside the humanoid one.

Provide humanoid, quadruped, and blank templates. Templates expose useful proportion controls such as
height, torso length, limb lengths, and head size. Extra chains can describe tails or additional limbs.
Semantic roles such as grip, foot, or ledge attachment should be optional mappings, not assumptions
about the number or names of limbs.

Animations should support local rotations or rig-relative targets, with explicit stretch where needed.
The exact representation needs a prototype. Existing absolute-coordinate poses require an adapter or
migration; they cannot simply be reinterpreted as local transforms without changing their motion.

A shared rig system does not imply universal animation reuse. Humanoids and quadrupeds need appropriate
gaits and poses. Retargeting should initially work between compatible templates with explicit mappings.

### Separate authoring layers

1. **Rig:** rest geometry, connections, proportions, attachment points, and optional posing constraints.
2. **Appearance:** strokes, circles, polygons, widths, colors, layering, and attached shapes.
3. **Hurt geometry:** named vulnerable regions and their state/phase windows.
4. **Attack geometry:** move-owned damaging shapes, timing, priority, and contact history.
5. **Stage body:** a simple collider for floors and walls, independent of decorative limbs.

The style should remain recognizable through simple strokes, readable silhouettes, and restrained
filled shapes. A large hat can remain cosmetic; a sword can add attack reach without adding a hurtbox.
Changing proportions must expose which layers will change rather than silently scaling all gameplay.

### Joint-bound shapes

Support circles and capsules bound to a joint, a bone segment, or the fighter root. Retain explicit
root-relative tracks for imported or deliberately fitted coverage. Capsules reduce the overlapping
circle chains currently used to approximate blades and limbs.

Hurt regions can follow limbs, with authored overrides for crouching, invulnerability, or deliberate
gameplay simplification. Suggested hurtboxes generated from a rig are editable starting points.
They must not regenerate over manual tuning without an explicit operation.

Multiple overlapping hurt regions still represent one victim. Preserve explicit hitbox priority,
attack/contact identity, and rehit rules so contacting a torso and arm does not multiply damage.
Keep existing phase hit-history behavior compatible before extending it with named attack groups.

### Deterministic pose evaluation

Extract gameplay-relevant pose and attachment transforms into a pure Bevy-free evaluator shared by
simulation, rendering, and editor previews. Use `f64`, `libm`, and deterministic traversal. Collision
samples tick poses; rendering may interpolate between tick results without feeding back into gameplay.

Attachment-driven holding that changes a collision anchor belongs in this shared evaluator.
Cosmetic jitter, glow, and decorative sway remain presentation-only. If an animation or rig field
affects collision, treat it as gameplay content for replay/versioning purposes.

Initially keep inverse kinematics in authoring: a two-segment limb tool can bend an elbow or knee when
the user drags a hand or foot, then save ordinary pose keys. Runtime procedural locomotion is not required.

### Size-dependent integration

Audit shared assumptions in fighter pushing, grab/throw positioning, ledge hanging, shield visuals,
effects, camera framing, portraits, and selection showcases. Use body dimensions or named anchors where
appropriate, with explicit gameplay overrides. A larger drawing alone does not create coherent size
behavior. Preserve a simple stable stage collider initially; full articulated stage collision is deferred.

## 2. Character Creator within the fighter editor

Add a Body / Character Creator workspace within Fighters, sharing the existing document, undo/redo,
saving, schema validation, viewport, timeline, and sandbox. A separate application is unnecessary.

### Intended workflow

1. Create a fighter from a humanoid, quadruped, or blank template.
2. Place joints with the mouse and connect them into chains.
3. Add and edit strokes, circles, and polygons; attach them to joints or segments and set draw order.
4. Adjust proportions, mirror paired limbs, and optionally constrain joint motion.
5. Generate suggested hurt regions, edit them, and set the independent stage body.
6. Pose the rig, save named poses, and author animation keys through the timeline.
7. Attach attack geometry, configure moves, and test in the existing sandbox/training flow.

Useful editor capabilities:

- Direct manipulation with snapping, numeric fields, and stable element IDs.
- Safe rename/delete operations that update references or explain affected content.
- Separate toggles for appearance, rig, hurt regions, attacks, and stage body.
- Binding controls such as “attach to hand,” “follow segment,” and “relative to root.”
- Explicit resize choices and a preview of affected geometry.
- An animation audit cycling through idle, run, crouch, jump, attacks, and ledge poses after body edits.
- Missing-role and missing-animation diagnostics for custom structures.
- The same readable, schema-backed data for mouse edits and external/LLM edits; no opaque editor-only format.

### Scope boundaries

The first creator is a body/rig builder integrated with existing move authoring. It does not automatically
invent a balanced moveset or animate arbitrary creatures. Advanced retargeting, arbitrary-chain IK,
procedural foot placement, ragdolls, cloth, and automatic locomotion are separate possible projects.

Initial acceptance target: build one humanoid and one four-legged stick creature through the same editor,
save/reload them, animate both, and verify attached offensive and defensive shapes in the same sim.
Neither should need character-specific renderer or gameplay branches.

## 3. Imported facts and authored tuning

Establish a common pipeline:

Pinned source facts → explicit conversion/fitting settings → generated baseline → persistent authored overrides.

- Share parsing and conversion code; retain character-specific fitting as explicit data or documented rules.
- Preserve source revisions, timelines, unit conversions, unknowns, and fitted geometry.
- Give editable phases, hitboxes, joints, and attachments stable IDs so reordering does not retarget overrides.
- Keep editor/LLM tuning durable across regeneration. Report conflicts when source elements disappear or change.
- Expose provenance: imported, inherited, fitted, or manually tuned, with the relevant source/conversion record.
- Keep expected reference behavior independent of generated output in tests.

Do not silently change today's inheritance semantics (object merge, array replacement, null deletion).
Choose and document how generated baselines and ID-based overrides compose before implementing them.
Commands belong in `justfile`; the existing import workflow remains authoritative until updated.

## 4. Composable combat and move mechanics

### Separate hit properties and outcomes

Replace the overloaded single hit-kind choice with small typed dimensions: contact behavior, launch rule,
and status/effect behavior. Validate supported combinations and define application order. This permits
combinations such as an electric effect with an automatic launch-angle rule without adding compound kinds.

Give contact resolution explicit outcomes: damage, shield, parry, counter, and successful grab. Allow
transitions to select outcomes while preserving the existing broad `Hit` behavior for migrated content.
Share resolution primitives across melee, projectiles, and independent objects where semantics agree.

### Persistent resources and abilities

Before implementing stored-charge, ammunition, cooldown, meter, or stance mechanics, add declared typed
resources with bounded operations: test, spend, add, set, reset. Define persistence and reset policies for
landing, damage, death, and respawn. Runtime values belong in `World`; declarations and limits live in data.

Extend conditions to refer to these resources. Preserve distinct existing reset policies when migrating
up-special use and once-per-airtime restrictions. Avoid unrestricted variables, arbitrary scripts, or a
general-purpose language embedded in JSON.

### Independent owned objects

Generalize articles toward owned objects with a timeline and explicit movement, collision, damageability,
release/control, and ownership policies. Reuse timeline and contact primitives without forcing every object
through fighter states. Define reflection, owner loss/death, interruption, and despawn behavior explicitly.

This should eventually allow a bouncing object with a staged attack or a destructible trap with an explosion.
Not every feature combination must be legal. Fully controlled partners require additional design and are
not automatically solved by a more general projectile model.

### Movement modes

Make the precedence of ordinary physics, directional travel, and prescribed velocity explicit, including
gravity, steering, friction, and collision behavior. Consolidate the current interactions among hover,
travel speed, fall caps, velocity events, and air-control multipliers as concrete use cases require it.
Avoid changing global movement to compensate for one imported character.

## 5. Scenarios, explanations, and CPU capabilities

Build a reusable scenario format with initial conditions, raw input sequences, and expected observations.
Run the same scenarios headlessly and in the editor; keep direct move-start previews for isolated inspection.

Provide diagnostic explanations for accepted/rejected commands, phase transitions, resource changes,
movement modes, hitbox priority, and rehit suppression. Diagnostic output must not alter RNG, snapshots,
or simulation results. Include tick order and phase-relative versus elapsed time in explanations.

Use explicit move capabilities for CPU recovery and special use instead of increasingly indirect inference
from movement fields. CPU matchup performance is evidence about both the fighter and the AI's ability to
use it; it is not sufficient balance or fidelity validation on its own.

## Delivery order and acceptance gates

Each milestone should leave existing fighters usable. Prefer small vertical slices over replacing all
content and subsystems simultaneously. Resource/object work is driven by actual upcoming character needs.

### A. Authoring ownership and diagnostic foundation

- Establish baseline/override semantics, stable IDs, and shared importer boundaries.
- Add initial reusable scenarios and transition/input explanations.
- Gate: regenerate an imported fighter without losing manual tuning; source facts and overrides remain
  reviewable; a scenario produces equivalent editor and headless results.

### B. Generic rig and deterministic evaluation

- Prototype a named-joint rig, compatibility adapter, and common transform evaluator.
- Support simple vector appearance and humanoid/quadruped templates.
- Gate: existing representative poses retain their intended geometry; both body structures animate through
  one evaluator; cosmetic changes cannot change simulation collision results.

### C. Attached collision and size integration

Partial: opt-in move circles/capsules, fixed named hurt regions, shared tick geometry, routing and
editor forms/overlays are implemented. Region windows and size integration are still planned; the
full acceptance gate below has not been met.

- Add bound circles/capsules and hurt-region windows; preserve root-relative geometry.
- Address overlap priority, contact history, and size-dependent anchors.
- Gate: test limb-following hits, crouching hurt regions, disjoint weapons, multi-region overlap, facing,
  stage traversal, ledges, and grabs for small/large and humanoid/quadruped examples.

### D. Character Creator workflow

- Expose rig construction, shape editing, bindings, proportions, and animation auditing in Fighters.
- Add basic authoring IK where it materially improves posing.
- Gate: create, save, reload, animate, and test a humanoid and a quadruped through the UI; undo/redo and
  inheritance-aware saving work; no manual Rust changes are needed for either body structure.

### E. Combat composition

- Separate contact/launch/effect properties and expose contact outcomes.
- Gate: migrate representative existing hits without unintended behavior changes; test combined properties
  and equivalent applicable outcomes across melee and projectiles/articles.

### F. Mechanics required by subsequent fighters

- Add persistent resources, broader owned objects, movement modes, and CPU capabilities in focused slices.
- Gate: each new primitive has a concrete fighter use case, schema/editor support, deterministic snapshot
  coverage, and tests for interaction with existing mechanics.

Milestone E can proceed independently of much of the rig/editor work. Do not delay useful fixes until
the whole plan is complete, or build speculative mechanics solely to cover every possible SSBU fighter.

## Compatibility and verification

- Version content changes and provide explicit migration/legacy handling. Keep old absolute poses and
  root-relative hitboxes valid until their replacements are demonstrated.
- Bump `SIM_VERSION` for simulation behavior or checksum-representation changes. Handle incompatible
  recordings explicitly; do not promise old replay compatibility after gameplay semantics change.
- Preserve deterministic ordering, clone/restore behavior, and the separation of cosmetic randomness.
- Validate rig cycles, missing bindings, duplicate IDs, invalid geometry, unsupported combinations,
  and override conflicts with useful file/field paths. Bound new evaluator work and content sizes.
- Verify tick boundaries, facing, phase transitions, priority, rehit timing, and resource reset events.
- Exercise raw input as well as direct move stepping; retain independent source-ledger expectations.
- Run focused tests and required workspace checks. Regenerate schemas with `just bless` when types change.
- Extend screenshot tours and inspect actual captures for creator interaction, body variety, animation,
  collision alignment, portraits, and gameplay framing. Playtest traversal/recovery on built-in stages.
- Do not bless unexplained hashes or treat screenshots/CPU matches as substitutes for interaction tests.

## Open implementation decisions

- Local rotations, rig-relative targets, or a constrained combination for animation keys.
- Migration strategy for inherited absolute poses and attachment-driven holding.
- Exact storage/composition rules for generated baselines and stable-ID tuning overrides.
- Which semantic anchors are required for a playable custom rig, and their fallback rules.
- How hurt-region windows compose with state and phase overrides, armor, and invulnerability.
- Contact-group identity and priority across multiple shapes, phases, and independent objects.
- Whether shared pose/geometry code stays in `sim` or a small pure crate used by `sim` and presentation.

Resolve these with the humanoid/quadruped vertical slice and representative imported moves before fixing
the public schema. The intended outcome is a small set of composable, inspectable systems—not a wholesale
rewrite or a general-purpose animation/gameplay scripting engine.

### Interaction slice verification and remaining work

The interaction slice preserves absent-field arithmetic and keeps attachments in the existing fighter tick
order. It does not implement animated attachment anchors, swept climb clearance, throw pose retargeting,
or an attachment pass during hitlag. Static hang clearance includes the owning wall for authored anchors.
Milestones C and D remain partial. Acceptance tests cover real grab contact below/at/above the strict
height tolerance, both holder/victim index orders and facings for all four throw paths (including a moving
holder before release), and a tether catch outside ordinary authored catch reach. A recorded-file fixture
embeds interaction overrides and unequal bodies, reaches hold/throw through raw inputs, reloads the file,
and verifies playback checksums, final world equality, and backward/forward seeking. This fixture is a
bounded training recording; it does not claim completed-match directory verification.

The targeted `interactions` screenshot section captures unequal humanoid/quadruped hold, static ledge
hang, and the first climb-segment endpoint. All three captures were rendered and visually inspected.
They show scripted snapshots, not swept-path clearance or hand/limb retargeting.
