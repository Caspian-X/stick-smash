# Planty (Piranha Plant) reference and conversion record

Planty, adapted from Piranha Plant, is the first built-in package authored around the optional **named rig**, its charged reach and
its held/controlled projectiles. It is an original-roster addition, not a replacement of an existing slot.

## Target and status

- Package/id: `assets/fighters/plant/fighter.json`; display name: Planty (`plant`).
- Source import branch: `char-tune` (SSBU source 13.0.1).
- Intended identity: a potted nonhumanoid with a short but fast close game, a chargeable long-stem snipe, a
  held spike-ball projectile, a lingering damage cloud, and a long spinning recovery. The pot, stem segments,
  head, front leaf (formerly jaw) and side leaves are separate named joints so reach, hurt exposure and jaw contact all follow the
  same tick pose.
- State: implementation and automated validation complete; hardware playtesting was not performed.
- Remaining work: controller feel pass; source-accurate proximity-triggered Long-Stem bite, cloud
  windbox/growth, Ptooie after-bounce, bounce physics and damage-to-ball interactions; physical pot topple and
  the source 0.8× damage-taken multiplier. See "Known approximations".

## Sources

| Source | URL / local path | Patch / commit / hash | Access date | What it establishes |
| --- | --- | --- | --- | --- |
| UFD character page | https://ultimateframedata.com/piranha_plant → `docs/imports/sources/plant/ufd.json` | page sha256 `f1f595683038f0f033d1a9c4b6c382f485a4e9996be2c310c5b0042604607e7d` | 2026-11-04 | Startup/active/total frames, damage variants, landing lag, notes, charge frames, armor notes |
| Per-frame hitbox GIFs | `docs/imports/sources/plant/visuals.json` (32 animations, pinned per-file sha256) → `just plant-visuals` | UFD `hitboxes/piranha_plant/*.gif` hashes as recorded | 2026-11-04 | Coarse coverage, weapon/pose silhouettes, cloud/ball appearance, spin and grab contact |
| Numeric scripts | https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/Piranha%20Plant/data.json → `docs/imports/sources/plant/scripts.json` | commit `2e8f0d245481cb0c936524b6ffe1cdea867c4f68`, sha256 `38c60513b2db6ebed5f950505285696fc40b91a71a1082fab32ffe0bc7f99ea6` | 2026-11-04 | Attack arguments, frame windows, rehit, masks, armor threshold, article timeline |
| SmashWiki | https://www.ssbwiki.com/Piranha_Plant_(SSBU), `/Long-Stem_Strike`, `/Ptooie` | live pages, not retained | 2026-11-04 | Behaviour prose used only to resolve intent (blow front/back, swallow, hold the ball afloat, aim during charge, shield/armor notes) |
| UFD Misc Info | https://ultimateframedata.com/piranha_plant (stats block; not in the retained `ufd.json`) | live page | 2026-11-05 | Weight 112, gravity 0.140, walk 0.76, run 1.72, initial dash 1.82, air speed 1.0, total air acceleration 0.06, fall / fast fall 1.95 / 2.73 |
| Jump heights | ssbwiki "Jump" page, SSBU "Jump height ranking" table, https://www.ssbwiki.com/Jump (unitless game units) | live page | 2026-11-05 | Full hop 37.4, short hop 17.5, midair jump 38.52 |

Retained inputs and regeneration:

```sh
just plant-visuals /tmp/plant-reference   # download + hash-check UFD GIFs, write frame sheets
just import-plant                         # regenerate fighter.json + ledger.json, then just bless
```

`just import-plant` rewrites the package from the retained UFD and Rubendal inputs and writes
`docs/imports/sources/plant/ledger.json`. The ledger contains two layers:

- `sourceFacts`: UFD rows and raw `attacks(...)` windows per slot/article, read back out of the pinned inputs
  before any conversion. This is the layer tests compare literal expectations against.
- `moves`: generated phase/hit rows, written for review convenience. It is not used as the source of truth.

### Disagreements and source selection

| Property / move | Source A | Source B | Chosen value and reason |
| --- | --- | --- | --- |
| Ptooie thrown damage | UFD: 14 (initial) / 18 (blown) / 15.3 (after bounce) | SmashWiki: 14 / 18 / 13 late; the retained `packun_spikeball/Shoot` block contains conditional 18.0 (non-reactive) and 14.0 (reactive) rows, and `Fall` is 14.0 | Held 14, freshly blown 18, late blown 13. There is no bounce stage in this engine, so the UFD 15.3 after-bounce tier is omitted rather than guessed. |
| Poison Breath hit data | Script: inner 1.2/2.8, outer 0.6/1.6, rehit 4/7 | UFD: damage shown as `!!!`; SmashWiki: 1.2 center / 0.6 edge uncharged and 2.7 / 1.6 full | One merged 50 px box using inner values 1.2 → 2.8% and rehit 4 → 7. The noise of a per-frame rehit cloud does not justify two overlapping boxes; documented as an approximation. |
| Long-Stem bite damage | UFD: 8.4 close/12 far (uncharged), 18.2 close/26 far (full) | Script: one 26.0 full-charge box with a zero/lerp branch | Four distance tiers with base damage 8.4/9.3/10.2/12.0 and `chargeDamage = (26/12-1)/60`. At full charge the nearest tier is 18.2 and the longest is 26.0. The source close/far labels are treated as short/long reach. |
| Long-Stem endlag | UFD: 39 hit / 49 whiff, +10 tipped over | Scripts do not carry the status timings | Base total 39 on hit and 49 on whiff, with +10 at full charge via `chargeFrames` (treated as the source tipped-over case). The un-tipped full-charge case is not separately modeled. |
| Piranhacopter loop damage | UFD: 1.2 hits 1-8 / 4.0 hit 9 | Script: 1.2/0.8 in the late loop, 4.0 final boxes | Script values converted directly; the 0.8 late-loop autolink rows are retained rather than replaced by the UFD summary. |

## Conversion rules

- **Frames.** Source frame `f` becomes engine phase frame `f - entry`. Most moves use entry 1 (the move starts
  on source frame 1). Multi-phase/animation-timed moves use the phase's own entry: `fair` entry 4,
  `uspec` entry 13 (source SpecialHi starts at 13), `jab rapid` entry 0 (`Attack100Sub` is a loop body), and
  Long-Stem's bite script is phase-relative already. UFD "active 2—3" for jab 1 is therefore engine 1–2.
- **Geometry scale.** Source gameplay units → 5 px for attack radii/offsets (`SCALE = 5.0`). This is independent
  of the rig's visual proportions; the two are aligned by authoring poses, not by scaling the rig.
- **Bone spaces.** Script offsets are bone-local. `top` is Plant's root bone (Z forward / Y up in source), so its
  offsets are already feet-relative: `bound()` emits a joint-less (fighter-root) point. Binding them to the
  animated head added the head translation/rotation a second time (jab 1's Z=5.3 Y=6.3 landed 31.5 px above the
  head instead of 26.5/31.5 forward/up from the feet). `mouth` and `potc` are animated X-forward joints, and
  `arml`/`armr` are X-out leaf joints, so those stay joint-bound. The mapping is explicit in
  `tools/import-plant/import.py::BONE_JOINT`; an unknown bone falls back to the root, never silently to a joint.
- **Capsules.** Source boxes with numeric `X2`/`Y2`/`Z2` are capsules; both endpoints are converted in the same
  bone frame. Rapid jab (source Z 8→13), rapid finisher (9→15), forward tilt (Y 7.5→10.5 at Z 12→13) and dash
  attack's `top` box (Y 11→5) therefore cover the source line, not just its first point. Degenerate source pairs
  collapse back to a circle.
- **Contacts and poses.** Runnable attacks use the converted source geometry; the static `x`/`y`/`r` fields remain
  serialized but are ignored for collision. `virtualhit1..3` are animated virtual bones with no numeric offset in
  the scripts; the fitted rig has no equivalent, so they follow the head and are labeled fitted presentation.
  The 2D rig is fitted, not extracted: stem/head/leaf motion, the jaw opening and the Long-Stem reach tiers are
  **fitted presentation/geometry**. Poses were refit until the visible bite met the converted contact: `lowStrike`
  for the low jab chain, `lowSweep`/`lowSweepBack` for down tilt, down smash and back air, `upStrike` for up tilt
  and up smash. Runnable attacks and hurt regions use the same named joints and the same tick pose.
- **Knockback and weight.** Shared Ultimate formula; source BKB is used directly and KBG/100. Plant's source
  weight 112 becomes the reciprocal 100/112 multiplier (lower = heavier). Hitlag and shield multipliers come
  from the scripts. No per-fighter formula override.
- **Masks and launch rules.** `FacingRestrict` CHECK_POS → `dir: away`, CHECK_B → `dir: back`,
  `Ground_or_Air` G/A → situation filters. Angle 361 → Sakurai, 366/367/368 → autolink, negative angles are
  kept as signed launch angles. Fixed-knockback (`FKB`) rows become `setKb`.
- **Movement and recovery.** One scale, source × 5 (`units.SCALE`, px/tick and px/tick²), from UFD Misc Info:
  walk 0.76 → 3.8, run 1.72 → 8.6, dash 1.82 → 9.1, air 1.0 → 5.0, air accel 0.06 → 0.3, gravity 0.14 → 0.7,
  fall 1.95 → 9.75, fast fall 2.73 → 13.65. These replace the earlier hard-coded ×3.5 / ×6 numbers
  (2.66 / 6.02 / 6.37 / 3.5 / 0.21 / 0.84 / 11.7 / 16.38). Fitted, converted ×5/3.5: `runAccel` 1.1429 (was 0.8),
  `airJumpDrift` 5.0 (was 3.5).
- **Jumps.** `units.jumps(37.4, 17.5, 38.52, 0.7)` gives `jumpSpeed` 15.8304, `shortHopSpeed` 10.7188,
  `airJumpSpeed` 16.7739, superseding the fitted 15.5 / 9.8 / 14.5. The feel harness measures 187.0 / 87.5 /
  192.6 px rises (targets 187.0 / 87.5 / 192.6). No shared movement changes were made.
- **Fitted velocities.** Authored, non-source speeds converted per axis (x ×5/3.5, y ×5/6): dash-attack lunge
  7.8571 (was 5.5), Piranhacopter launch 7.86 / sustained rise 6.36 (was 8 / 6.5; a held rise keeps its per-tick climb v − g under the new gravity), Poison Breath drift 1.2857
  (was 0.9), blown Ptooie 11.4286 (was 8). The lunge and launch `setVel` are now written as `{x, y}` objects
  instead of `[x, y]` arrays so the unit tags apply to them. The copter climb test's floor moved 280 → 233 px (×5/6; measured 270). Piranhacopter uses data events for a sustained
  rise and the shared `upSpecialAccel` drift. Ledge catch points and grab anchors are authored for the
  nonhumanoid shape (`interactions`); the shared grab/ledge logic is unchanged.
- **Charged reach.** Long-Stem's extension is a fan of four named poses (`stretch0/15/30/45` and their bite
  variants) selected by `anim[].minCharge`. The same tiers select mouth-anchored hitboxes via `minCharge`, so
  the visible head, the box and the stem/head hurt regions agree on every tick. This is a generic animation
  selector, not a fighter-id branch.

## Move ledger

All rows are source → engine conversions. `startup/active/total` are inclusive UFD/source frames unless the
script was more specific. "same" means the source-script/phase windows map with the stated entry.

| Slot / variant | Source citation | Startup / active / total | Damage / launch / mask / priority | Phases / classification |
| --- | --- | --- | --- | --- |
| Jab 1/2/3 | UFD Jab 1–3; Attack11/12/13 | 2 / 2–3 / 19, 20, 25; jab3 7 / 7–8 | 2.4, 2.2, 4.4; angle 361→Sakurai; 180° reverse rows become forward-facing normal | Three phases with full source recovery when not chaining. The engine has one held-chain trigger, so it uses jab 2's earlier continuation (source frame 7) into jab 3, then rapid after jab 3's active window (source frame 9). UFD's separate jab-2 jab-3/rapid split at 8/7 is not modeled as two triggers. **converted/adapted** |
| Rapid jab / finisher | UFD; Attack100Sub, Attack100End | loop 4 ticks per sub-hit; finisher 7 / 6–7 / 40 | 0.6 every 4 ticks; finisher 3.4 50° | `rapid` loops and releases into `rapidEnd`; `Attack100Sub` is phase-entry 0. **converted** |
| Forward tilt 1/2 | AttackS3/S32 | 7 / 7–8 / 27; second 5 / 5–6 / 31 | 5.5 autolink; 6.0 Sakurai | Fresh attack press from source frame 12 (engine 11). **converted** |
| Up tilt | AttackHi3 | 6 / 6–13 / 26 | 7.0 body / 9.0 head, angle 85/90; UFD head intangibility 6–13 authored as the engine 5–12 head hurt window | one phase, `upStrike` contact pose. **converted** |
| Down tilt | AttackLw3 | 9 / 9–11 / 26 | 6.0 close / 7.0 far, 73/78° | one phase; low source boxes (Y 4→2) match the fitted `lowSweep` jaw. **converted** |
| Dash attack | AttackDash | 7 / 7–9 and 10–14 / 47 | 10.0 early, 7.0 late; setVel + slide 10 | one phase. **converted** |
| Forward smash | AttackS4 | 16 / 16–17 / 51, charge 5 | 15.0 close / 19.0 far, 45°; UFD head intangibility 16–17 authored as the engine 15–16 head hurt window | one phase. **converted** |
| Up smash | AttackHi4 | 12/16 / 11–14, 15–17 / 51, charge 6 | 3.0 rehit then 12.0, 75/105/90° | one phase. **converted** |
| Down smash | AttackLw4 | 10/14 / 13–14, 17–18 / 43, charge 5 | 12.0 then 14.0, 30/32° | one phase; front then back low sweep in the fitted pose. **converted** |
| Neutral air | AttackAirN | 8/11/14/17/18 / 7–16, 17–18 / 47, land 15 | 2.0 rehit 3 autolink then 3.0 Sakurai | one air phase. **converted** |
| Forward air | AttackAirF (entry 4) | 10–12 → engine 6–8 / 44, land 13 | 9.0 then 11.0, 45° | one air phase. **converted** |
| Back air | AttackAirB | 14 / 13–14 / 61, land 16 | 14.5, 45°, back-facing | one air phase; root box behind the feet met by the fitted back arc. **converted** |
| Up air | AttackAirHi | 7 / 6–8 / 38, land 10 | 10.0, 85°, away | one air phase. **converted** |
| Down air | AttackAirLw | 9/12 / 8–10, 11–13 / 44, land 18 | 11.0 then 9.0; 45° and -90° spike row | one air phase. **converted** |
| Grab / dash grab | Catch/CatchDash | 6 / 5–7; dash 9 / 8–10 | grab, mouth-bound circle 17/15 | ground grab uses 34 frames; dash 42 + slide 8 | **converted** |
| Pummel | CatchAttack | 1 frame hit | 1.4 (source script); shared 11-tick pummel timing | shared `pummel` state. **converted + shared** |
| Four throws | ThrowF/B/Hi/Lw | release 13/18/19/29, totals 31/34/49/47 | F 8.0 45°, B 11.0 45°, U 12.0 90° + 5.0 tick, D 3.0 66° + two 2.5 ticks | shared hold placement, plant mouth anchors. **converted** |
| Neutral special: Ptooie | SpecialNStart/Shoot; spikeball Start/Loop/Wait/Shoot/Fall | held hit from source frame 9; earliest blow source 22; shoot 41; swallow 30 (close) | held 14%, blown 18% for 4 ticks then 13%; angle 55, BKB 50/45, KBG 0.8, size 5 | article, see specials. **converted/adapted** |
| Side special: Poison Breath | SpecialSStart/Shoot; poisonbreath article | 10 entry + 11 charge startup; cloud 82 (122 full) ticks; rehit 4 (7) | 1.2→2.8% damage-only, no flinch, 0.1 shield mult | ordinary drifting projectile. **adapted**, see below |
| Up special: Piranhacopter | SpecialHi | hit 1 source 15–16; loop 18–57 rehit 6; final 58–69 | 1.2 loop / 4.0 final, 60–145° | 58-tick spin + 20-tick fall; sustained rise data events, shared drift. **converted/adapted** |
| Down special: Long-Stem Strike | SpecialLwBite_Attack; UFD Down B | 18 entry to charge; 2-frame bite; 39 hit / 49 whiff (+10 tipped modeled by charge) | 8.4–12 tier base → 18.2–26 at full; 55°, BKB 75, KBG 55, size 7.9 | 6 phases, four reach tiers, direction aim. **fitted reach**, source damage/timing **converted** |

Footstool attack and the source's unused pummel variants are omitted; no destination slot exists and the
source footstool attack is a crouch-shaped interaction rather than a normal move.

## Named rig, poses and animation

Rig (`rig.joints`): `pot → stem0 → stem1 → stem2 → neck → head → jaw`, plus `mouth`, `top`, `leafBase`, `leafL`,
`leafR`. The pot is the root and stays at the floor (`pot` world y = fighter feet y in every state tested).
Stem segments are drawn as bones; leaves are separate bones/shapes. The head carries the upright and rear
leaves, and the jaw carries the front leaf; the mouth joint remains the grab/throw/bite anchor.
`hurtRegions` follow `pot`, `stem0..neck`, `head/top` and `jaw/mouth`; `body` remains the stage collision body.

Named poses cover stand/crouch/coil/strike/bite/air/charge/stretch tier/grab/throw/ledge/guard/win. All 41 state
clips listed below are authored (not inherited): `idle, walk, run, crawl, turn, brake, crouch, jumpsquat,
landing, air.rise, air.fall, air.flip, helpless, hitstun, tumble, shield, shieldDrop, roll, spot, airdodge,
knockdown, getup, getupAttack, tech, ledge, climb, ledgeAttack, ledgeRoll, ledgeJump, dizzy, win, respawn,
hold, pummel, throw.f/b/u/d, grabbed, buried, shieldbreak`. Movement/defense/hit/grab/throw/ledge presentation is
therefore plant-specific. Every move phase has an authored beat: coil → contact hold through the active window
→ followthrough → reachable exit key. The extension tiers are named poses, so every attack key resolves to a
valid pose (checked by `plant::every_authored_clip_and_move_pose_resolves`). The low jab/tilt/smash/back-air
contacts and the raised up-tilt/up-smash use the named `lowStrike`, `lowSweep`, `lowSweepBack` and `upStrike`
poses so the visible head/jaw does not sit a body-height away from the converted box.

The source UFD GIFs prove attack silhouettes, ball/cloud appearance and pot anchoring; they do not provide bone
transforms. Their per-frame overlays also confirm the bone-space conversion: jab 1's three boxes sit at the pot
rim in front of the feet (not above the head), down tilt's boxes run just above the floor, forward-tilt's capsule
slants up-forward, and fsmash/up-tilt place their head/mouth boxes on the animated mouth. Locomotion,
jump/landing, hitstun and the exact stem bends are **fitted presentation**. The Long-Stem reach tiers and the
copter leaf spin are **fitted** from the source silhouette/promo, not measured bone animation. No source
animation files were available or claimed.

## Visual refinement

The display name is **Planty**; the `plant` package id, source filenames and import paths remain unchanged to
preserve references and regeneration. The head is now a three-leaf crown, with the former jaw carrying its
forward blade. A wider terracotta pot has a rim, dark soil and a highlight. Stem and side leaves are green.
The joint hierarchy and translations, poses, hitboxes, hurtboxes, timing and damage are unchanged; the jaw's
existing animation fans the front leaf as the stem lunges.

Bevy captures `29a`–`29z` (the filenames present in the Plant tour) were inspected, including select, idle, fsmash, extension, Ptooie, poison, copter,
grab and ledge. The crown and pot read clearly in editor close-ups; gameplay-scale veins are subtle and the
training hitbox overlay obscures the silhouette. The walk/run captures show idle after reaching the dummy,
so they do not establish gait quality. The tour does not capture neutral or down aerial poses. No controller
playtest was performed.

## Validation and handoff

Focused `crates/sim/tests/plant.rs` (26 tests):

- literal source expectations from the independent ledger: jab 1 window/damage/angle code, fsmash 15/19,
  rapid jab 0.6 every 4th frame, LSS 8.4–26 tiers, UFD/Ptooie/spikeball values;
- named rig grounding: pot stays on the floor through jumpsquat/landing; every state clip and move pose resolves;
- animation/collision agreement: fsmash and every LSS tier keep the mouth box on the animated head in both
  facings, with the head/jaw regions disabled during the bite;
- raw input: fsmash charge/release, jump+A short-hop aerial, special facing, Long-Stem charge aim (one turn then
  lock), Ptooie hold/hit/front-back launch/swallow, poison charge/rehit/no-flinch, copter steady climb and
  steering both ways;
- source/engine boundaries: Long-Stem armor starts with the charge phase, head intangibility starts at travel
  frame 3, damage tiers at charge 0/20/35/50/60; up tilt / fsmash / usmash / uair head (and uair stem) windows;
- launch at 0/80/150 percent and heavier-victim comparison; clone/restore through charging specials;
  grab/pummel/throw anchor; nair/copter rehit windows; transition `articleEnd` spelling;
- regression tests added for this review pass (all six fail against the pre-fix generator/sim code):
  raw held jab reaches jab 3, the rapid loop and the finisher, while a tap or early release keeps jab 1's 19 /
  jab 2's 20 / finisher's 40 tick recovery; `top` contacts resolve from the fighter root (jab 1 26.5/31.5 px)
  and stay put across animated frames, down tilt's active boxes stay low in both facings, source capsules keep
  both endpoints, and representative contacts overlap the visible joints; hurt regions follow the rig, mirror,
  and follow the refitted low/up poses; a held Ptooie reflected by Nayru's Love immediately ends the caster's
  hold, becomes an independently owned shot with no hold control or immediate re-reflection, damages the
  reflector's opponent, expires, and stays clone/restore deterministic; turn and brake clips sample `gait_t`
  (start pose and clock identical after 3 vs 12 idle ticks, then visibly animate and recover) on both the legacy
  skeleton and named-rig paths.

Commands run in this session:

```sh
cargo test -p sim --test plant                 # 26 passed
cargo test -p sim                              # all sim targets passed (incl. content, stages, runtime golden)
just bless                                     # schemas + canonical assets
just check                                     # exit 0: fmt-check, clippy -D warnings, workspace tests
just shots /tmp/plant-import-shots plant,select,training  # exit 0, 67 captures written (all black locally)
python3 tools/import-plant/import.py && just bless        # byte-identical rerun (reproducible generator)
```

This review pass re-ran the generator, `cargo test -p sim --test plant`, the affected
`content/attached_geometry/feel/wizard/fighter_kits/rigs/animation_*/runtime/stages` targets, and `just bless`
(no schema or golden changes: only the Plant package and ledger changed). The temporary pre-fix package + sim
revert used to prove the six regressions fail was restored; the fixed tree is what the commands above ran.

`just balance 16 plant` was not required for this task and was not run. The CPU matchup test in
`crates/sim/tests/stages.rs` includes Plant on every built-in stage in both seat orders (12 matches per stage),
and the ordinary stage CPU tests still finish.


## Known approximations / remaining work

- **Ptooie:** the held ball follows and bobs at the mouth as a 14% hitbox, is blown front/back by the stick
  (18% for 4 ticks, then 13%) and is swallowed on special release. Not modeled: the ball's rise/fall
  height changing throw distance, the after-bounce 15.3% tier and bounce physics, damage decay after hits,
  the source 10%-damage "stop blowing" armor interaction, melee hits destroying/knocking the ball, and reduced
  contact endlag. The ball is one article with no gravity; it flies level and expires. It auto-releases after
  180 ticks (anti-stall adaptation; the source can hold indefinitely). A reflector that catches the *held* ball
  knocks it out of the grip: it becomes an ordinary shot owned by the reflector (who does not gain hold control),
  the caster takes its data-authored `articleEnd` transition out of the hold immediately, and an 8-tick
  re-reflect cooldown temporarily suppresses every reflector, not just the original one. The 300-tick hold timeout
  remains only as a failsafe if an article disappears without a transition.
- **Poison Breath:** no hitstun/hitlag/state change (damage-only), source damage/life/rehit scaling, one merged
  50 px circle. Not modeled: the 25-frame radius growth, the 12–25 windbox, the drift stopping at frame 45/75,
  separate inner/outer boxes, and exact source shieldstun.
- **Piranhacopter:** sustained rise and shared up-special drift are data-driven; the source's exact ascent
  curve, max horizontal tune and frame-by-frame hitbox placement are fitted. The multi-hit loop is grouped by
  the script's windows.
- **Long-Stem Strike:** reach tiers (about 200/255/310/404 px) and their damage split are fitted. The source
  damage-based armor *is* authored through the shared engine armor: 15% during the charge phase and 12% once
  released, matching the implemented `armorThreshold` values. Not modeled: the proximity-triggered early bite,
  the attack-button early strike/cancel, pot physical topple/terrain interaction, the 0.8× damage-taken
  multiplier, and the source's 18% 1v1 armor variant. The +10 tipped-over endlag is approximated by charge-scaled
  recovery; the un-tipped full-charge case is not distinct.
- Head/stem intangibility windows from UFD are authored as hurt-region windows for up tilt, forward smash, up
  smash and up air (see the ledger rows); the LSS head/jaw windows come from its script. Other UFD notes are not
  separately investigated. These are region windows on the shared hit pipeline, not per-fighter code.
- The rig silhouette, crown and leaf shapes, state loops and stem bends are fitted presentation. No source
  skeleton animation was available. Hardware calibration (controller deadzones, DI feel) was not measured.

## Delivery

The original import was developed on branch `char-tune` from `bd5363f`. `SIM_VERSION` was bumped once from `sim-24` to `sim-25` for this package; the
review fixes (jab windows, root-space contacts, reflected-hold release, gait clip clock) stay within the same
unreleased batch, so it was not bumped again. The generator remains the source of the package and ledger:
`just import-plant` rewrites both from the pinned UFD/Rubendal inputs.

Counter contacts also release a held ball into an independent shot and end the former caster's hold. `article.reflectCooldown` is authored as 8 ticks for Ptooie, defaults to zero for older content, and suppresses all reflectors during that window.

Momentum migration (2026-11-04): no phase sets `airAccelMult`, so drift now uses the fighter's airAccel × the `movement.ultimate` kind multipliers; regeneration is unchanged.
