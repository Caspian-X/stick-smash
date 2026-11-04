# Attack animation pass: brawler through wrangler

Presentation-only pass over the attack animations of the ten original fighters. Nothing here changes
frame data: no `frames`, hitbox, event, transition or physics field was edited, and no engine behavior
was added. Only move-phase `anim` tracks (and `windup`-phase tracks) changed. The built-in packages have no named rigs or joint-bound hurt regions, so this pass does not move their gameplay geometry. Pose-driven rig combat geometry requires separate review for future packages.

## What the data means

Per [FORMAT.md](FORMAT.md#visuals), a phase's `anim` is a key list on phase ticks: a key holds until the
next one, and `lerp` on the *destination* key blends from the previous key across `[previous.at,
key.at)`. Sampling never overshoots, so a key at the active start reaches its pose exactly on that tick.
There is no easing curve and no per-joint IK from anim keys; whole-body `rot`/`pivot`/`offsetY` plus the
authored joints are the vocabulary.

Two consequences shaped this pass:

- A key placed at the last active tick is the destination of a blend that began at the *first* active
  tick. That is why many kits previously drifted toward `stand`/`air` while the box was still active,
  and why a single key at frame 0 made heavy kits look frozen through their whole windup.
- Attachment angle tracks are the readable part of the weapon kits. They were left untouched, so the
  prop keeps its authored contact angle, trail sweep and tip position on every active tick. Whole-body
  pose changes are limited to the ticks between windows and the recovery, where they cannot separate a
  weapon from its hit geometry.

## The shared beat

Every phase with a hitbox now runs the same authored beat, scaled by that phase's damage and length:

1. **Entry** (frame 0): the kit's authored windup/neutral pose. When the authored entry *was* the
   extended contact pose (`entry == pose at first active frame`), it is replaced by the fighter's windup
   pose (`back`, `crouch`, `air`, `spearCoil`, `spearAir`, ...) so the first tick already reads as intent
   instead of showing the finished strike.
2. **Coil** (frame 1, or 2 for longer startups): a 2–10° whole-body lean plus a small sink.
3. **Committed coil** (first active frame − 3, startups of 8+ ticks): a deeper version of the same lean,
   still blending, so long startups keep moving instead of holding a frozen pose.
4. **Contact** (first active frame): the authored contact pose, verbatim, reached over the last 2–3 ticks
   of startup (the coil key makes the strike accelerate into contact rather than slide for the whole
   startup). Aerials only do the lerp/timing part (no sink), avoiding mid-air flailing.
5. **Hold** (last active frame): the same contact pose repeated, so every active tick shows the contact
   pose that matches the box. Authored pose changes *inside* a window are kept (for example Lancer's
   vault rolls `spearOverhead → spearAir` mid-window); only the drift back to neutral is suppressed.
6. **Followthrough** (last active frame + 2): the contact pose pitched 3–8° forward (with a small sink),
   i.e. the body passes through the strike instead of stopping dead.
7. **Recovery**: a lightly counter-pitched neutral key around the middle of the recovery, then the
   authored neutral (`stand`/`air`/`guard`) on the last tick. Every recovery now moves; previously most
   phases held their final pose for 5–30 ticks and then snapped to `idle`.
8. **Spin phases** (whole-body `rot` deltas of 240°+, e.g. Dancer's air pirouette, Spinning Top and
   Cartwheel, Brawler's nair): authored rotation keys are preserved and only the lead-in plus the
   recovery tail are added, so the spin keeps its authored timing. Weapon rotations live on the attach
   tracks and were not treated as pose spins.
9. **Joined phases** (a phase reached from a sibling, e.g. Dancer's double fair, Golem's smash
   `strike`, Lancer's vault) keep their authored entry pose, because the previous phase already ended
   in it.

Style inputs per fighter are the coil angle, sink and followthrough angle. Golem leans hardest (9°/3 px
before scaling), Swordfighter and Ninja next (7°), then Dancer, Lancer, Yoyo, Tinker and Wrangler (6°),
and Archer (5°, since the bow carries the read). Aerials halve the coil and drop the sink; damage ≥14
phases scale up 30% and ≤5 damage phases down 30%, so the deepest added sink is about 4 px on Golem's
tilts. Weak moves therefore show a 1-tick coil and a 2–4 tick strike instead of a long dead hold.

## Per-fighter findings and changes

| Fighter | Slots inspected | Main issues found | This pass |
| --- | --- | --- | --- |
| Brawler | jab…dspec (16) | Startup held frame 0 through `active−1` (up to 13 frozen ticks on fsmash); no recovery at all (last key sat on the follow pose, then `idle` snapped). | Coil + accelerating strike, contact held exactly through each window, followthrough and a real recovery. Spin nair keeps its authored 40→320° rotation. |
| Swordfighter | grab, dashgrab, jab…dspec (19) | Body pose was one static key for the entire move on every attack; only the sword angle moved. Recovery held a single pose for up to 35 ticks. | Windup→contact→follow body beats timed to the existing sword keys (unchanged), so the torso now leans with the blade; longer recoveries settle. |
| Golem | jab…nspec (17) | Same static single-pose problem, worst offender for "designed" feel: jab/ftilt/aerials showed the extended strike pose from frame 0 for the whole startup (up to 9 ticks). | Grounded `back` windup into the authored `stonePunch`/`stoneLift`/`stoneDrop`/`curl` contact, held through the window, followed by a heavy overshoot. Smash `windup` phases breathe into their brace instead of freezing. |
| Dancer | jab…dspec (17) | Contact poses were already timed to windows, but finish/`dspec` phases were single frozen poses (up to 18 recovery ticks) and `sspec` held `handstand` from frame 0 across the whole active window. | Coil/settle added; `sspec` now crouches into a handstand on the active start and cartwheels through the window; nair/nspec/uspec `finish` phases hold the contact 0–3 then recoil and settle. |
| Lancer | jab…dspec (17) | Prop track was strong; body lerped into `spearLunge` across the whole startup and then eases back to guard with no recoil. | Coil held then a 3-tick snap into the authored lunge, contact held through the active window, a 5–6.5° followthrough and a guard settle. Spear angle/offset keys, trail and tip alignment are untouched. |
| Ninja | jab…dspec (17) | Entry pose held for the whole startup, then a hard 1-tick snap into the inline contact pose; the only return key sat at `frames−4`, so the contact was already blending home throughout the active window. | Coil variants of `back`/`air`, 2-tick lerped snap into each authored inline contact, hold through the window, followthrough and settle. |
| Yoyo | jab…dspec (18) | Body was `back → lean` at contact with no blend, and the authored `stand` key sat on the last active tick, so the body drifted home during the actives. | Coil, contact lean held through each window, followthrough twist; the yoyo's angle/scale/show keys are untouched. |
| Archer | jab…dspec (17) | Inline contact poses were authored, but the return key was placed at the last active tick, drifting the body home while the box was live. | Coil, exact contact held, followthrough, settle. Bow/kite keys untouched. |
| Tinker | jab…dspec (17) | Same drift, plus the longest frozen startups (sspec: 21 ticks). | Coil + accelerating strike, contact hold, followthrough; wrench/nose-flame keys untouched. |
| Wrangler | jab, grab, dashgrab…dspec (19) | Same drift; bodies stayed neutral through whip contacts and held for up to 15 startup ticks. | Coil, contact lean held, followthrough twist. Whip angle/scale/show keys untouched. |

## Review corrections and verification

Sol review caught floor penetration caused by whole-body coil rotation, inherited Whiplash attack
bookends, and changed spin timing. The final original-roster coils use baked joint positions with
compensated support feet and knees, rather than rotating the entire skeleton during interpolation.
Feet remain at the floor through the Golem thrust startup, including ticks between keys and both
facings. Torso lean and compression remain visible. Aerial coils keep their airborne rotation.

Dancer's neutral-air and up-special spin tracks retain their original continuous authored motion.
Cartwheel's crouched lead-in meets the original trajectory at first contact (90 degrees at tick 5),
then follows the original rotation and pivot. Tests check every active rotation tick.

The shared integration captures include startup coils, first spin contacts, Whiplash's later tether
contacts, movement boundaries and charge/recovery poses. The gameplay tuning on Ninja and Lancer was
independently compared with KIT_POLISH.md after an agent accidentally reset and reconstructed those
two packages; the non-animation deltas match the intended tuning exactly.

## Limits and follow-ups

- Rendered integration captures supplement structural checks; controller playtesting remains the judge of feel.
- **Linear only.** The sampler has no curve or hold-then-snap primitive, so acceleration comes from key
  spacing. Coil/followthrough use whole-body `rot` (a few degrees, small sink) because anim keys cannot
  retract a single joint generically. Where a kit owns a joint-specific windup pose it is used instead
  (Golem's `back`, Lancer's `spearCoil`, Dancer's `crouch`).
- Grounded support coils are baked with foot compensation. Deliberate stance changes still move feet between contact points; there is no procedural ground IK.
- **Weapon kits deliberately keep a neutral body at contact** (Yoyo, Archer, Tinker, Wrangler, Lancer):
  the prop is the hit geometry cue, and rotating the body during a window would shift the hand/attachment
  origin away from the authored contact position. Their improvement is anticipation, hold and recovery.
- **Multi-hit windows keep a held body** where the box re-hits (Yoyo dsmash, Wrangler dspec); the prop
  sweep remains the motion. This is intended, not a missed beat.
- Not in scope: throws, pummels, state animations (`idle`, `run`, `landing`, …) and named-rig poses.

Final validation: `just bless` and `just check` passed. The `animations`, `fighter-kits`, `training`, `crouch` and `interactions` captures were inspected, followed by fresh `animations,fighter-kits` captures after review fixes. New floor/contact/spin regression checks pass. This does not replace human controller playtesting.
