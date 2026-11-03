# Mike tuning reference

This is the worked character record for the [shared import workflow](CHARACTER_IMPORTS.md).

Mike is a playable starting point for an Ike-like heavy sword fighter. His package is
[`assets/fighters/mike/fighter.json`](../assets/fighters/mike/fighter.json), inheriting only the shared
abstract `base`. Timings and damage reference [Ultimate Frame Data: Ike](https://ultimateframedata.com/ike)
(accessed November 2, 2026), including its downloadable frame-by-frame hitbox GIFs. Numeric geometry,
launch rules and hitlag multipliers are cross-referenced against
[Rubendal's 13.0.1 attack-script data](https://github.com/rubendal/ssbu/blob/master/public/data/patch/13.0.1/character/Ike/data.json).
The frame numbers are authored directly at the sim's 60 Hz, using its frame-0 entry convention.
They are not sped up to match the original roster.

UFD is the authority for timing and headline damage when sources disagree. For example, the script dump
has a 7-damage jab finisher, a 13.5-damage forward tilt, and 9-damage late dash attack; Mike retains UFD's
5, 12.5, and 14 respectively. Script animation frames can also differ from displayed game frames because
of motion-rate commands. These numbers are cross-referenced, not treated as interchangeable timelines.

Normals preserve the listed startup, active windows, early/late damage, recovery, smash charge frames,
aerial landing lag, and autocancel boundaries. Hold attack to continue jab 1 into jab 2 at frame 9, and
jab 2 into jab 3 at its frame 9; releasing keeps that jab's independent recovery. Mike's sword has broad
coverage rather than Swordfighter's tipper reward (the late forward-smash tip retains its script's
19-damage sour spot). Neutral air is a low-damage sweeping starter;
back air is his quick aerial finisher. Smashes demand a committed read.

Eruption starts at frame 30 uncharged and 239 at full charge, with an 11-frame windup after releasing
from the hold point, and scales from 10 to 35 damage; full charge breaks a fresh shield. Additional
columns unlock at 100 and 209 held ticks, appearing four and eight frames after the initial eruption.
Quickdraw holds at frame 15, reaches full charge at frame 85, then rushes until an opponent enters its
forward detection box or the rush expires. The swing starts two frames after detection and deals 6–13
damage. A successful air swing restores normal air actions after 44 frames; a missed air rush forces
special fall and 30 frames of landing lag.

Aether has a 6-damage body hit on frame 15 and a separate 2-damage thrown sword, ascends on 35,
catches with repeated 1-damage hits from 44,
then a 3-damage catch on 51 and a continuing downward plunge on 61. Landing starts a separate 6-damage hit and
36-frame recovery. Grounded armor covers frames 5–35; airborne armor covers 15–35. Grabs bypass armor.
Counter activates on frames 9–33, has the listed frame-8 invulnerability and 73-frame whiff duration,
and retaliates on strike frames 4–5 with 43 total strike frames.

## Translation to this game

Root-space hitbox offsets and radii use 5 pixels per script unit: `top` bone Z becomes forward X,
and Y stays up. For example, jab 1's first sphere (Z 6.2, Y 9.4, radius 2) becomes (31, 47, radius 10).
Capsules are approximated with overlapping circles spaced no farther apart than their radius.
Sword-bone offsets use the same scale, transformed through hand/weapon tracks fitted to the GIFs,
then baked into per-frame hitbox positions. Those fitted 2D tracks replace the original 3D animation;
they are not exact projections of Nintendo's skeleton. Poses distinguish the jab punch and kick, up tilt's
horizontal blade lift, and the low-front → overhead → behind sweep of up smash.

Script launch angles are preserved where supported: 361 maps to `sakurai`, 270 to a downward `spike`,
and 367 to `autolink`. Ground/air collision masks distinguish the late up-smash ground hit, the falling
Aether ground launch versus air spike, and smaller aerial grab volumes. Normal down air keeps the
upper launching sphere ahead of the lower spike spheres in collision priority. Hitlag multipliers are
imported; base hitlag remains shared. Every fighter now uses the shared Ultimate-style knockback
formula: source base knockback is unscaled, and source growth is divided by 100 (90 → 0.9).
The no-staleness/no-rage Ultimate equation includes the hit's damage and post-hit victim percent;
reference launch speed is converted at 0.03 × 3.5 pixels per unit. This replaces the original rough
linear conversion, which ignored damage-dependent growth. Fixed-knockback hits retain their adapted
launch strengths in the shared reference units; autolinks retain their separate pull velocity. Launch
decay, hitstun, blast zones, rage, staleness and the 1v1 damage bonus are not a full Ultimate trajectory simulation.
The equation is cross-checked against the primary implementation in
[Rubendal's Ultimate calculator](https://github.com/rubendal/SSBU-Calculator/blob/master/js/formulas.js)
(`VSKB`, with staleness and rage disabled).
Special travel, charge-column thresholds, counter scaling and flame-zone damage remain adaptations.
All of these choices live in the fighter's editable data.

Horizontal speeds use 3.5 pixels per reference speed unit; gravity and terminal
speeds use 6, so run speed is 5.2745, air speed is 3.969, gravity is 0.552, and the fast-fall ratio stays
1.6. Jump speeds target roughly the reference's 37/48-frame short/full-hop rhythm while reaching the
built-in platforms. Weight is the reciprocal multiplier 100/107 because this sim represents heavier
fighters with smaller knockback multipliers.

The shared movement system now supplies analog walking, initial dash, running and turnarounds, with an
optional per-fighter `walkSpeed` and a global jumpsquat. The movement ratios above describe the original
import; later tuning should be recorded separately. A gradual push now promotes walking to running,
and easing off returns to walking. These gait rules address input-dependent speed differences without
changing Mike's imported movement stats. Initial dash uses the shared command restrictions documented in
[FORMAT.md](FORMAT.md#simulation-tuning-and-cpu-data). Shared shield, dodge, hitlag, knockback, input buffering
and ledge rules remain this game's rules. Hurtboxes remain the base fighter's rectangles; the original
per-bone 3D hurt volumes are not imported. Autocancels remove the attack's landing penalty; the landing
state still follows the sim's normal next-tick exit. Throws combine the listed preliminary and release
damage into one hit at the release frame (forward 8/32, back 22/37, up 20/42, down 40/57). Pummels use the
shared grab rules. Counter reflects projectiles through the existing counter system.

Aether's tossed sword is a short-lived ballistic projectile: it disappears on contact, and the held
sword reappears on ascent. It does not simulate a separately caught weapon. Armor uses a high damage
threshold. Eruption derives charge knockback from charged damage and does not model self-damage or terrain-following
flame pillars. Ground attacks, including Quickdraw's grounded rush, retain the sim's edge stop.
Quickdraw charging now uses normal gravity instead of hovering; its horizontal rush still hovers.
Gravity during startup is documented for [Quick Draw](https://www.ssbwiki.com/Quick_Draw), but exact
special-specific gravity coefficients have not been extracted, so ordinary fall speed here remains an adaptation.

## Checking changes

`cargo test -p sim --test mike` checks move windows, damage, charge duration, jab chains, Quickdraw
detection and collision, Counter, aerial landing boundaries, Aether's phases, numeric geometry conversion,
ground-only up-smash hits, and down-air sour/spike collision priority. Stage tests include
Mike's platform reach and ledge recovery; his CPU matchup test runs both seats on each built-in stage.
`just shots /tmp/mike-shots fighter-kits` includes Mike's smashes, aerial sweep, Eruption, and Aether.
For optional extended matchup diagnostics, use `just balance 16 mike`.


## Follow-up visual and coverage tuning

Friend playtesting prompted a second fit of the blade tracks and Eruption effects. The original UFD
GIFs were downloaded and inspected as individual frames on November 2, 2026:

- [Down tilt](https://ultimateframedata.com/hitboxes/ike/IkeDTilt.gif), frames 6–7: the blade slopes
  slightly downward as it extends. The old numeric fitting incorrectly sloped the collision centers
  upward. The centers now follow the visible −10° blade at hand height; damage, radius and frames 7–8
  remain unchanged. This is a corrected 2D fit, not a direct bone transform.
- [Up air](https://ultimateframedata.com/hitboxes/ike/IkeUAir.gif), frames 12–17: the sword travels from
  front to overhead to behind. Mike's rear hand/blade track now finishes farther back, at 190° instead
  of 160°, with the collision centers transformed alongside it. The 13–18 active window is unchanged.
  This extra rear coverage is a playtest adaptation of the source arc.
- [Forward smash](https://ultimateframedata.com/hitboxes/ike/IkeFSmash.gif), full windup and frames
  30–34: [UFD](https://ultimateframedata.com/ike) confirms startup 31, strong hit 33–35, charge hold 24,
  total 83. Mike already had those timings, so they remain intact. The windup now shows separate
  preparation stages before the overhead swing, making the commitment easier to read.

Eruption now emits upward-moving orange flames and bright cores throughout the height of each active
column, rather than relying on a radial burst near its base. The columns appear at local X 43, 140,
and 220 with the same active windows and held-charge thresholds (0, 100, 209) as their damaging volumes.
These are cosmetic events; they do not extend collision duration. The particles cover the tall zone
immediately rather than waiting for a ground emitter to rise after the damage has ended.

Original GIF SHA-256 values for this inspection:

- `IkeDTilt.gif`: `40515193418df46e65130c0a043b7336c7d996e6cbc9fee9d28934888c387968`
- `IkeUAir.gif`: `09226dfa0dadf3aac9f839f266962c7774de0d9ce8ef894e26270e242e107cea`
- `IkeFSmash.gif`: `6db58aff70f49d39cfa05a543a43629c97c73d23c6aa2a3c6596fa087dd0793d`

`cargo test -p sim --test mike_visual_tuning` passes four regressions: both-facing low-sweep direction,
rear-shoulder up-air coverage, verified forward-smash startup, and charge-gated tall flame emissions.

### Short hop and Quickdraw feel

The requested short-hop reduction is a deliberate playtest adjustment: launch speed is now
7.0623076923 instead of 10.1, producing half the previous peak height under the existing discrete
gravity integration. Full hop, double jump and gravity stay unchanged. This halves height rather
than speed. It is not a claim that Ultimate uses this height in our world units. The raw-input
regression in `mike_hop_tuning.rs` measures both short and full hops through the normal jump controls.

Quickdraw now separates charge strength from how long it may remain held. Strength saturates after
69 held ticks, followed by up to 120 extra held ticks before automatic release. The extra hold
reflects Ultimate's roughly two-second fully charged hold documented by
[Quick Draw](https://www.ssbwiki.com/Quick_Draw); it grants no additional damage or travel.

The following are fitted playtest changes rather than exact Ultimate kinetics: the rush is compressed
from 37 to 18 ticks, with a faster launch and 75% of the former uncharged ground distance, growing to
the same former fully charged distance. An aerial swing briefly brakes for its strike, then resumes
65% of rush momentum during recovery; a whiff carries its momentum into the existing special fall.
Ground swing recovery is 22–34 ticks and aerial swing
recovery is 20–44 ticks as charge increases from 0 to 69. The full-charge endpoints preserve the
previous source timing; lighter charges unlock follow-ups, including up special after aerial contact,
sooner. This charge-dependent recovery is a requested adaptation, not an extracted Ultimate rule.
`mike_quickdraw_tuning.rs` covers saturated holds, travel extrema, momentum, recovery and buffered up B.

### Aether capture correction

The rising hit previously used one small sphere above Mike's head. The numeric `SpecialHi2` script
instead describes a forward capsule, translated here to X 70, Y 40–80, radius 45, rehit 7 and hitlag
multiplier 0.5. Three overlapping spheres now cover that volume. The initial 6-damage launch remains
at frame 15, the rising hit starts at 44, and the 3-damage catch starts at 51. These use the existing
[13.0.1 script reference](https://github.com/rubendal/ssbu/blob/master/public/data/patch/13.0.1/character/Ike/data.json)
and [UFD's visual timeline](https://ultimateframedata.com/ike).

Two fitted kinetics address the observed disconnect: the thrown sword now starts forward/above the
hand and travels visibly upward before contact, and Mike starts descending when the catch sends the
victim down. Previously the sword could immediately replace the initial launch velocity, while Mike
continued rising for ten frames after spiking the victim. Sword travel and descent speed are adapted
to this game's gravity and autolink implementation, not extracted animation velocities.

The catch-to-plunge transition retains contact history using the generic `preserveHits` field, so
one continuing falling hit does not deal its damage twice just because the phase changes. Landing
still clears contacts for the separate impact. Rehit intervals use the move's elapsed clock across
such transitions. `mike_aether_tuning.rs` checks natural contacts, with ordinary hitlag and no target
repositioning, in both facings at three distances and three starting percentages. On a flat floor the
neutral target takes 6 → 2 → 1 → 3 → 6. DI, intervening platforms and other escape situations can still
change the sequence; this is not a forced cinematic grab.

### Follow-up verification

The five Mike test targets (`mike`, `mike_aether_tuning`, `mike_hop_tuning`,
`mike_quickdraw_tuning`, `mike_visual_tuning`) pass, as does `just check` across the workspace.
The generic runtime contact-history regression covers both retained and cleared phase contacts,
including a rehit cooldown spanning a phase-frame reset. The existing golden world hash is unchanged.
Simulation compatibility is `-sim-18` for the new charge/transition semantics and hit-history clock.

`just shots /tmp/mike-tuning-shots fighter-kits,training` passed. Inspected the low sweep, rear up-air
coverage, smash anticipation, tossed sword and Eruption captures. Repeated the training section in
`/tmp/mike-tuning-reviewed` with the full-charge capture synchronized to the third column, and inspected
that image too. Physical controller playtesting remains for the user; these changes are local on
`char-tune`, uncommitted and unpushed alongside the Whiplash import.
