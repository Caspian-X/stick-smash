# Bongo Boy reference and conversion record

Playable SSBU **Diddy Kong** adaptation, packaged as **Bongo Boy**. It exercises the shared projectile
trip hit, chargeable projectile scaling, a command-grab special branch and a charge-then-travel recovery.

## Target and status

- Package/id: `assets/fighters/bongo/fighter.json`; display name **Bongo Boy** (`bongo`), inheriting only
  the abstract `base` package.
- Branch: `t3/import-bongo-boy-fighter`. Source game: Super Smash Bros. Ultimate, patch **13.0.1**.
- Intended identity: a small, quick ape with above-average air mobility, a tail, a banana peel that trips
  grounded opponents, a chargeable peanut popgun, a grabbing/kicking Monkey Flip and a rocketbarrel
  recovery that explodes on landing.
- State: research, authoring, focused tests, stage/CPU coverage and windowed visual inspection complete.
  No controller playtest was performed.
- Remaining work: overcharge self-damage (the engine has no self-hitting attack), wall-contact explosion
  during the boost, banana pickup/throw and its two-slip durability, the Monkey Flip grab attack / grab
  jump (the shared hold/pummel/throws replace them), the held-jab repeat loop, angled forward-tilt
  variants, pivot grab, and the smash-input banana toss.

## Sources

| Source | URL / local path | Patch / commit / hash | Access date | What it establishes |
| --- | --- | --- | --- | --- |
| Ultimate Frame Data: Diddy Kong | https://ultimateframedata.com/diddy_kong → `sources/bongo/ufd.json` | page sha256 `0dfdad7665f4664d674f31f71b63cd16909f4efb860e1fcf3d2d899b12e9f1aa` | 2026-11-04 | Startup/active/total, damage tiers, landing lag, autocancels, charge boundaries, movement stats |
| Rubendal Diddy Kong scripts | https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/Diddy%20Kong/data.json → `sources/bongo/scripts.json` | commit `2e8f0d245481cb0c936524b6ffe1cdea867c4f68`, patch 13.0.1, full-input sha256 `4f3d8405d462455b1ee8f5acfec96a8e3ceec914ccbb8c33d1f90b71459dc553` | 2026-11-04 | Attack ids, bones, geometry, masks, damage, launch fields, hitlag, FKB, rehit, gadget scripts |
| Rubendal item data | https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/items.json → `sources/bongo/items.json` | same commit, full-input sha256 `bf8806de2ce69030f19904ddda022e45f3a1968c835683761e677c076cc2684e` | 2026-11-04 | Banana and peanut item hit data (banana 0% trip/BKB 50; peanut 3% base hit) |
| UFD per-move hitbox visuals | `https://ultimateframedata.com/hitboxes/diddy_kong/<Name>.gif` (34 files) → `sources/bongo/visuals.json` | per-file sha256 and frame counts recorded; representative: `DiddyKongJab1` `e6adac8c…`, `DiddyKongMonkeyFlip` `376ffe4c…`, `DiddyKongBanana` `08f2ff50…`, `DiddyKongRocketBarrels` `5d69e1cf…` | 2026-11-04 | Per-frame coverage, pose/weapon path, active-window cross-checks |
| SmashWiki move pages | Peanut Popgun, Monkey Flip, Rocketbarrel Boost, Banana Peel (move) → `sources/bongo/smashwiki.json` | page sha256s in that file | 2026-11-04 | Overcharge behavior, Monkey Flip branches, boost surface explosion, banana cooldown/toss angle |

Regeneration:

```sh
python3 tools/import-bongo/fetch.py          # re-acquire scripts/items/UFD and record hashes
just import-bongo                            # tools/import-bongo/import.py, then just bless
just bongo-visuals /tmp/bongo-reference      # download + hash-check the hitbox GIFs, write montages
```

`import.py` rewrites the whole package and `sources/bongo/ledger.json` (raw parsed fighter attacks and item
rows). Hand edits to `fighter.json` are lost on regeneration; authoring rules belong in the importer.
`visuals.py` keeps moved frames as montages; GIF frame 0 is the move's first frame. It refuses to rewrite the pinned
`visuals.json` when an upstream hash changes unless `--refresh` is passed. Clip lengths that match
the UFD total (jab 19, forward smash 50, neutral air 48, down air 46, Monkey Flip 47, Monkey Flip kick 32)
independently confirm the duration convention. The rocketbarrel clip is 61 frames, exactly the source charge
window 8–68; the popgun misfire clip is 109 frames, the post-charge explosion animation.

### Disagreements and source selection

| Property | Source A | Source B | Chosen value and reason |
| --- | --- | --- | --- |
| Forward-smash first hit | Script: four 5% rows with different FKB/angle and ground/air masks | UFD: 5% then 11% | Folded the four rows into one GA 5% box using the 35-FKB arm row; a single engine hitbox cannot carry both ground-only and air-only geometry. The second hit keeps 11% and 9%. |
| Up-smash multihit | Script: separate ground/air 2.5% rows plus repeated air rows | UFD: 2.5% multihit, 6% final | One GA fixed-KB box per multihit tier; the repeated air rows are the same hit re-issued on later frames. |
| Dash attack | Script: ground-only and air-only 2% rows | UFD: 2%/2%/3% | One GA box per tier using the 60/70-FKB rows, so the move hits both grounded and airborne victims. |
| Up throw timing | Script: 1% box at frame 17, 4% release at frame 19 | UFD: startup `17/19`, damage `1.0/4.0` | Sources agree: 1% tick at phase 16, 4% release at phase 18. |
| Monkey Flip grab window | Script `SpecialAirSJump`: catch frames 7–26 | UFD: active 18–36, total 47 | UFD/move frames 18–36. The pinned GIF shows the grab sphere appearing at GIF frame 17 and leaving after frame 36, confirming the move-frame reading. |
| Monkey Flip ground total | UFD: 47 air / 61 ground | Script: single leap/kick/recover timeline | Default path is 47 frames; landing uses 26 frames of lag. The extra grounded total is not separately authored (documented approximation). |
| Monkey Flip kick damage | UFD: 14% clean / 10% late | Script: 14/12 at frame 6 and 12/10 at frame 11 | Import keeps the higher box in each window: 14% then 12%. The 10% late box is folded away; recorded as an adaptation. |
| Rocketbarrel charge/hit mapping | Script `SpecialAirHiJump`: 10% at frame 1, 8% at frame 3, 6% at frame 10, clear at frame 29 | UFD: startup 8–68, active 8–9 / 10–16 / 17–35 | The 8-frame charge phase precedes the boost; boost phase frames 0–1, 2–8 and 9–27 map to UFD move frames 8–9, 10–16 and 17–35. |
| Overcharge total | UFD: misfire total 228, frame 124 | Hitbox GIF: 109 frames starting at source frame 120 | Charge phase 16 + max 105 + 3 held ticks = 124, then a 104-frame overcharge phase, totaling 228 from press. The GIF's earlier first frame is the pre-misfire animation. |
| Banana cooldown | SmashWiki: 12 frames (Smash 4) → 61 (8.0.0) → 85 (11.0.0) | UFD: one item generated at frame 20, no cooldown text | 85-tick `banana` charge pool models the Ultimate cooldown; `max: 1` models one peel at a time. |
| Banana toss | SmashWiki: 105° at 2.2 (tilt) / 3.036 (smash) | No scripted item toss velocity in the fighter data | Fitted `[-1.6, 2.4]` so the peel visibly arcs behind him and settles; the source's 105°/2.2 horizontal component is too small to read at this engine scale. Smash-input toss omitted. |
| Air mobility | Source ×3.5: air speed 3.234, air accel 0.175 | Requested identity: above-average air mobility | Fitted to `4.3` / `0.30`. This is an intentional gameplay adaptation, recorded here; walk/run/dash/gravity/fall remain source-scaled. |

## Conversion rules

| Property | Source | Destination | Classification |
| --- | --- | --- | --- |
| Frames | 1-based gameplay frames; script motion-rate commands | Phase frame = source frame − 1; `frames` equals the UFD total. Dsmash's motion rate 0.6 and dash attack's 0.8 are resolved to the UFD gameplay frames | converted |
| Geometry | root `top` boxes: source Z forward, Y up, size; bone-local boxes use hand/knee/foot/waist transforms | Root boxes at 5 px/unit; bone-local geometry fitted to the pinned GIFs and the stick poses. Overlapping root rows are folded to one/two circles per tier | converted (root) / **fitted** (bone) |
| Capsules | `X2/Y2/Z2` segments | Dash attack's 2% rows use a short root capsule; folded to one circle per tier. Bone-local geometry is fitted from the GIFs; no bone capsule is projected | fitted |
| Launch fields | source BKB/KBG/FKB, angle codes | `bkb` unscaled, `kbg = KBG/100`; 361 → `sakurai`, 366–368 → `autolink`, >180 → negative angle (`spike` at 270); FKB → fixed-launch `setKb` with the shared `((1+FKB/2)×1.4+18)×KBG/100+BKB` adaptation | converted |
| Masks | `FacingRestrict`, `Ground_or_Air` | `dir` (`away`/`back`), `situation` (`ground`/`air`); folded ground/air pairs use GA | converted |
| Hitlag / shield | script `Hitlag`, `ShieldDamage` | `hitlagMult`, `shieldDmgMult` | converted |
| Poses / attachments | UFD GIF silhouettes; no original skeleton animation | Authored stick poses with a hip tail, head cap, hidden popgun and hidden rocketbarrel pack; per-phase `show` and angle keys. Tail is presentation and a fitted bair tail box (see approximations) | **fitted** |
| Movement | UFD: walk 1.313, run 2.006, dash 2.09, air 0.924, air accel 0.05, gravity 0.125, fall 1.75, fast fall 2.8 | ×3.5 speeds, ×6 gravity/fall; jump speeds fitted to clear the built-in platforms (14.2 / 9.9 / air 14.0); air speed/accel fitted upward (4.3 / 0.30) | converted + adapted |
| Weight | 90 | `100/90` reciprocal knockback multiplier | converted |
| Pummel | 1.0 | `stats.pummelDamage` | converted |

Diddy's tail is animated as a thick brown curl from the hip. The source scripts contain no tail attack
hitbox. The back air's third, upper box is a **fitted identity addition** along the tail's spin path so the
move has the requested tail disjoint; the source's leg boxes remain first in priority.

## Move ledger

`startup/active/total` are source gameplay frames (inclusive). Engine phases use one less.

| Slot / variant | Source | Startup / active / total | Damage / launch / mask | Phases / notes |
| --- | --- | --- | --- | --- |
| Jab 1 | Attack11 | 3 / 3 / 19 | 2.0, 361→Sakurai, BKB 25, KBG 0.25, three hand/top circles | phase 2 only; attack held from phase 5 chains to jab 2 |
| Jab 2 | Attack12 | 5 / 5 / 20 | 1.5, 361, BKB 20, KBG 0.25 | phase 4; chains from phase 5 |
| Jab 3 | Attack13 | 5 / 5 / 34 | 4.0, 361, BKB 60, KBG 0.80, sidekick | phase 4; no rapid loop (approximation) |
| Forward tilt | AttackS3 | 10 / 10–11 (12–16) / 36 | 10.0 clean, 7.0 late, 35°, BKB 60, KBG 0.60 | clean phases 9–10, late 11–15; angled hi/lo variants folded |
| Up tilt | AttackHi3 | 6 / 6–11 / 29 | 6.0, 92°, BKB 28, KBG 1.50, arm/shoulder | 3 fitted swept boxes above/front |
| Down tilt | AttackLw3 | 4 / 4–5 / 18 | 5.5, 60°/50°, BKB 15, KBG 0.75, low clap | phases 3–4; near and far boxes |
| Dash attack | AttackDash | 8 / 8–9, 16–17, 22–23 / 40 | 2.0/2.0/3.0, FKB 60/70 then 83°/BKB 80, KBG 0.90 | three phases (`hit1` 0–8, `hit2` 0–7, `hit3` 0–22) so each hit can reconnect; forward impulse and slide |
| Forward smash | AttackS4 | 12 / 12–13 (21–23) / 50 | 5.0 first (FKB 35), 11.0 + 9.0 second (46°/60°, BKB 40, KBG 1.16) | `hit1` 0–13 (charge hold 3, first hit 11–12), `hit2` 0–35 (second hit 6–8); separate phases clear contact history; forward impulses at 11 and 20 |
| Up smash | AttackHi4 | 5 / 5–8, 12–15, 19–23 / 53 | 2.5 fixed-KB multihit, 6.0 final (85°, BKB 55, KBG 1.31) | `hit1` 0–7 (charge hold 0, leap), `hit2` 0–6, `hit3` 0–37; each tier is a separate phase; landing lag 30 from move frame 22 |
| Down smash | AttackLw4 | 7 / 7–8, 12–13 / 50 | 12.0 front (33°, BKB 46, KBG 0.77), 15.0 back (35°, BKB 38, KBG 0.76) | charge hold phase 1; front phases 6–7, back 11–12 |
| Neutral air | AttackAirN | 8 / 8–20 / 48, landing 8, AC 1–3 / 27+ | 6.0, 80°, BKB 60, KBG 0.75, four body circles | cartwheel track; main phase lands for 8 |
| Forward air | AttackAirF | 6 / 6–9 (10–16) / 46, landing 16, AC 1–2 / 26+ | 10.0 clean (361, BKB 30, KBG 0.86), 8.0 late | dropkick; clean phases 5–8, late 9–15 |
| Back air | AttackAirB | 5 / 5–8 / 30, landing 7, AC 1–2 / 20+ | 9.0, 361, BKB 30, KBG 1.02, `dir: back` | reverse spin kick; fitted tail box above/behind |
| Up air | AttackAirHi | 4 / 4–7 / 36, landing 9, AC 27+ | 7.0, 76°, BKB 57, KBG 0.89, `dir: away` | overhead kick behind/above |
| Down air | AttackAirLw | 15 / 15–16 / 46, landing 17, AC 1–14 / 35+ | 13.0 fists (270° spike, BKB 25, KBG 1.10), 10.0 arms (BKB 20, KBG 1.00) | two boxes below the feet |
| Grab | Catch | 7 / 7–8 / 36 | grab, reach 40 px | shared hold/pummel/throws |
| Dash grab | CatchDash | 10 / 10–11 / 44 | grab, reach 45 px, slide 18 | pivot grab omitted |
| Pummel | CatchAttack | 1 | 1.0 | `stats.pummelDamage` |
| Forward throw | ThrowF | release 21 / 31 | 9.0, 46°, BKB 69, KBG 0.65 | shared throw motion |
| Back throw | ThrowB | release 18 / 51 | 12.0, 42°, BKB 60, KBG 0.55 | strongest throw |
| Up throw | ThrowHi | 17 / 19 / 26 | 1% tick then 4.0, 87°, BKB 70, KBG 0.72 | tick at phase 16, release 18 |
| Down throw | ThrowLw | 22 / 31 | 7.0, 106°, BKB 85, KBG 0.59 | combo throw |
| Neutral special | SpecialNStart/Shoot/Blow | shot at 16, max 121, misfire 124, shot total 48, misfire total 228 | peanut 3.3–15.0, 70°→45°, BKB 40, KBG 0.70; melee 2.0; overcharge 23.0 | charge holds at phase 15, max 105 + 3 held ticks; release from phase 15; shot phase 33 (entry tick + 33 = 48-tick tap); overcharge 104 |
| Side special | SpecialAirSJump / AirSKick | leap 17, grab 18–36, kick 6–10 / 11–25, total 47 / 61 | grab; kick 14.0 clean, 12.0 late (361, BKB 80, KBG 0.50); UFD's late 10% is folded away | attack press during leap/flip → kick; grab hitbox → shared hold; otherwise recover 11 frames; landing lag 26 (leap/flip) or 18 (kick) |
| Up special | SpecialAirHiStart/Jump + barreljet | charge 8–68, boost hits 8–9 / 10–16 / 17–35, landing explosion total 57–59 | boost 10.0 / 8.0 / 6.0 (361/80°/70°, BKB 40, KBG 0.85–0.90), explosion 18.0 (70°, BKB 50, KBG 0.70); `chargeDamage: 0` keeps all hit damage fixed | charge 8 frames, max 60, jet; boost 34 frames with charge-scaled launch and drift; land explosion 59 frames and 55-frame endlag; otherwise fall 30 → helpless |
| Down special | SpecialLw / SpecialAirLw | item at 20, total 39 | banana 0%, trip, BKB 50; one peel | `banana` charge pool (1 use, 85-tick recharge) gates the throw; `max: 1` blocks a second peel |

Special notes:
- Banana is a normal projectile with gravity/bounce and a `trip` contact, not an explosion gadget. It is
  consumed on its one contact and expires after 600 ticks. Bongo cannot pick it up or throw it.
- The banana pool is spent when the throw phase is entered; the recharge schedule is the source's 85-tick
  generation cooldown. Using the move while the pool is empty runs a 39-frame shrug with no spawn.
- Peanut charge scales projectile damage from 3.3 to 15.0, speed from 13 to 21.5 and angle from 70° to 45°.
  The close melee hit stays 2.0 (`chargeDamage: 0`).
- Overcharge is a 23% local Sakurai hitbox plus a 104-frame stun animation. The source also damages Diddy;
  this engine has no self-hitting attack, so the self-damage is not applied.
- Monkey Flip's grab maps to the shared hold, so pummel and the four throws are available; the source's
  scratch attack and grab jump are not separately modeled.
- Rocketbarrel Boost does not leave helpless after the boost until the fall phase ends. Touching a wall or
  ceiling during the boost is not detected; only a landing starts the explosion phase.

## Validation and handoff

- Reference tests (`crates/sim/tests/bongo.rs`, 20): package identity/all 19 slots/4 throws/pool; active
  windows for jab, fsmash, usmash, dsmash, nair, fair, bair, dair; durations and damage tiers; banana trip
  on contact, settlement on the ground, consumption, one-at-a-time and 600-tick expiry; 85-tick banana cooldown checked at tick 84 and 85 and the empty animation;
  peanut damage/speed/angle at 0/52/105 held ticks; tapped neutral-special duration (48 ticks); overcharge
  at max+3 and its 23% hitbox; Monkey Flip kick/grab/default branches; Rocketbarrel charge, boost, landing
  explosion, full-charge auto-launch and unscaled full-charge hit damage; successive hits of f-smash, dash
  attack and up smash; up-smash grounded contact; raw-input routing, jump+A, attack stick, charge hold/release
  and landing-lag buffering; fitted contact reach.
- Shared-behaviour coverage: `tests/content.rs` roster expectation and duplicate-id path, `tests/stages.rs`
  `new_fighter_cpu_matchups` (all four stages, three opponents, four seeds: 17/48 wins, 139 KOs, 12
  self-KOs) and the existing ledge/jump/CPU sweeps now include the package.
- `just balance 4 bongo` (4 seeds, every opponent and stage, 256 matches): **0 unfinished**, 75 wins,
  76.0k damage dealt / 87.4k taken, 32 self-KOs, 162 recovery deaths. Per-move hits are spread across the
  kit (utilt 1345, uair 1175, dair 1025, dash attack 1759, ftilt 535, fsmash 161, usmash 141, banana 361,
  popgun 407, Monkey Flip 169, rocketbarrel 540). The recovery deaths and self-KOs reflect the shared
  CPU's offstage behavior with a light, fast-falling fighter; all matches finish.
- `cargo test -p sim --test bongo`, `just bless`, `just check` and the content/runtime suites were run.
  The golden world hash was re-blessed because `bongo` sorts before `brawler` and shifts the fighter data
  indices; `SIM_VERSION` was not bumped (no snapshot field or per-tick rule changed).
- Visual inspection: windowed `fighter-kits` captures (2560×1440, real colour variance, `/tmp/bongo-shots8`)
  were opened for `33a` jab, `33c` fsmash, `33d` usmash, `33g` fair, `33h` bair, `33i` uair, `33j` dair,
  `33k` peanut charge, `33l` peanut shot, `33m` monkey flip, `33n` rocketbarrel, `33o` banana throw,
  `33p` grab, `33q` monkey kick and `33r` settled banana. UFD montages for jab, fsmash, usmash, dsmash,
  nair, fair, bair, uair, dair, Monkey Flip, the kick and the banana were compared alongside them. The
  popgun now reads as a brown barrel with a pale muzzle, the settled peel rests on the stage behind Bongo,
  and the tail/cap are visible; the rocketbarrel pack overlaps the tail at the default editor zoom but the
  two barrel bands are visible. A black first attempt was discarded; the second run produced >4k colours
  per capture.
- Controller playtest: not performed.
- Independent review: an attempted Codex `gpt-6.1-sol` high-reasoning review failed because the provider
  reported a usage limit; no external review findings were received. The checklist above and the focused
  tests were self-reviewed against `docs/CHARACTER_IMPORTS.md`.
- Known approximations: fitted bone geometry and poses; folded ground/air hitbox rows; folded forward-tilt
  angles; fitted air speed/accel and jump speeds; fitted banana toss; overcharge self-damage; wall-contact
  rocketbarrel explosion; banana pickup/throw and two-slip durability; Monkey Flip scratch/grab jump;
  held-jab repeat; pivot grab; no controller calibration. Monkey Flip's kick branch does not spend the
  up-special resource in this engine, so Rocketbarrel remains available before landing (the source forbids it).
  Projectile transcendent priority, item catching
  and item clashing rules are not modeled: the banana and peanut use the ordinary projectile clash,
  reflection and batting rules. The engine's once-per-airtime list is not restored by taking a hit, so
  Monkey Flip stays spent for that airtime.
- Delivery: local branch `t3/import-bongo-boy-fighter`; commit `d0f88f6` is the initial import and `c56ee17`
  contains the review fixes (multi-hit phases, unscaled rocketbarrel damage, popgun total, tests and this record).
