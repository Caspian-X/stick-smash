# Mike tuning reference

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
then a 3-damage catch on 51 and downward plunge on 61. Landing starts a separate 6-damage hit and
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
imported; base hitlag and knockback formulas remain shared. Normal-attack base knockback is scaled by
0.1 and growth by 0.001 from the script; this is a starting conversion, not Smash's knockback equation.
Special travel, charge-column thresholds, counter scaling and flame-zone damage remain adaptations.
All of these choices live in the fighter's editable data.

Horizontal speeds use 3.5 pixels per reference speed unit; gravity and terminal
speeds use 6, so run speed is 5.2745, air speed is 3.969, gravity is 0.552, and the fast-fall ratio stays
1.6. Jump speeds target roughly the reference's 37/48-frame short/full-hop rhythm while reaching the
built-in platforms. Weight is the reciprocal multiplier 100/107 because this sim represents heavier
fighters with smaller knockback multipliers.

The shared movement system has one run speed/acceleration and a global jumpsquat, rather than separate
walk/initial-dash states and per-fighter jumpsquat. Shared shield, dodge, hitlag, knockback, input buffering
and ledge rules remain this game's rules. Hurtboxes remain the base fighter's rectangles; the original
per-bone 3D hurt volumes are not imported. Autocancels remove the attack's landing penalty; the landing
state still follows the sim's normal next-tick exit. Throws combine the listed preliminary and release
damage into one hit at the release frame (forward 8/32, back 22/37, up 20/42, down 40/57). Pummels use the
shared grab rules. Counter reflects projectiles through the existing counter system.

Aether's tossed sword is a short-lived ballistic projectile: it disappears on contact, and the held
sword reappears on ascent. It does not simulate a separately caught weapon. Armor uses a high damage
threshold. Eruption uses the shared charge knockback bonus and does not model self-damage or terrain-following
flame pillars. Ground attacks, including Quickdraw's grounded rush, retain the sim's edge stop.

## Checking changes

`cargo test -p sim --test mike` checks move windows, damage, charge duration, jab chains, Quickdraw
detection and collision, Counter, aerial landing boundaries, Aether's phases, numeric geometry conversion,
ground-only up-smash hits, and down-air sour/spike collision priority. Stage tests include
Mike's platform reach and ledge recovery; his CPU matchup test runs both seats on each built-in stage.
`just shots /tmp/mike-shots fighter-kits` includes Mike's smashes, aerial sweep, Eruption, and Aether.
For optional extended matchup diagnostics, use `just balance 16 mike`.
