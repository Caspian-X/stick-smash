# Stratus reference and conversion record

Playable SSBU **Cloud** adaptation, packaged as **Stratus** (`stratus`). It is the first import whose whole
resource is a **single-use Limit gauge** filled by a charge special (`Event.restore` on release) and spent by
**move variants** that select Limit Break timelines for all four specials.

## Target and status

- Package/id: `assets/fighters/stratus/fighter.json`; display name **Stratus** (`stratus`), inheriting only the
  abstract `base` package.
- Branch: `t3/import-stratus-fighter` (task worktree `t3-c2ec2b93`). Source game: Super Smash Bros. Ultimate, patch **13.0.1**. Requested
  name: "Stratus"; requested identity: SSBU Cloud.
- Intended identity: a fast-moving, fast-falling swordsman whose attack animations are slow for their range. The
  Buster Sword gives a long disjointed hitbox on most normals, with a slow 19-frame forward smash, a 12-frame up
  smash and an 18-frame forward air. Limit Charge fills a one-shot gauge that upgrades every special: a faster,
  multi-hit Blade Beam, an immediate five-hit Cross Slash, a 1.5× Climhazzard and Finishing Touch.
- State: research, authoring, focused tests, Limit semantics, CPU/stage coverage and screenshot section complete.
  A display-unlocked `fighter-kits` run produced real captures; `31a`–`31l` were inspected at full resolution and
  the CPU-canvas pass covers every active window. Remaining work is listed under **Known approximations**.
- Deliverable: package, two-reference-source pin (`docs/imports/sources/stratus/`), importer and montage tool
  (`tools/import-stratus/`), focused tests (`crates/sim/tests/stratus.rs`) and record.

## Sources

| Source | URL / local path | Patch / commit / hash | Access date | What it establishes |
| --- | --- | --- | --- | --- |
| Rubendal Cloud scripts | <https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/Cloud/data.json> → `sources/stratus/scripts.json` | commit `2e8f0d245481cb0c936524b6ffe1cdea867c4f68`, full-input sha256 `f7812dc2b4849fcdd77e0b84872b2d3e45555bede204c423e060bec5e1a0c3f4` | 2026-11-04 | Attack ids, bones, geometry, masks, damage, angles, BKB/KBG/FKB, hitlag, rehit, shield damage, Windbox rows, the `_LB` Limit Break variants, the `cloud_wave` article hits |
| Ultimate Frame Data: Cloud | <https://ultimateframedata.com/cloud/> → `sources/stratus/ufd.json` | page sha256 `c77ee4c1b4c2bd4b13f499c482ced75ceb96ed992523a29f9f60f9c50081bbca` | 2026-11-04 | Startup/active/total, damage tiers, landing lag, autocancels, charge-hold frames, grab/throw rows, movement stats, Limit Charge rate, Finishing Touch windows |
| UFD per-move hitbox GIFs | `https://ultimateframedata.com/hitboxes/cloud/<Name>.gif` (41 files) → `sources/stratus/visuals.json` | per-file sha256, size and frame count recorded; acquired 2026-11-04 | 2026-11-04 | Per-frame coverage, weapon path and pose for every normal, aerial, smash, grab, throw and special (including both facing variants of Cross Slash and the Limit waves) |
| SmashWiki Cloud (SSBU), Limit (status), Cross Slash, Climhazzard, Blade Beam, Limit Charge / Finishing Touch | live pages, not retained verbatim | page revisions as of 2026-11-04 | 2026-11-04 | Limit gauge units/rate, Limit Break move changes and stat boosts, natural-combo rules, move descriptions and totals |

Regeneration:

```sh
just stratus-sources              # explicitly refresh source pins
just import-stratus               # rebuild from local pins, then just bless
just stratus-visuals /tmp/stratus-reference   # download + hash-check the 41 GIFs, write frame montages
```

The character page only links the generic getup/ledge/trip clips, so the move GIFs were probed under
`hitboxes/cloud/`; `CloudPivotGrab` and the generic `cloudGetupAttack{U,D}` / `cloudLedgeAttack` /
`cloudTripAttack` clips are rolled into the grab and ignored respectively. GIF frame 0 is the move's first
pictured frame. The frame count matches the UFD total for many clips (neutral air 38, back air 42, dash attack
45, up/down smash 45/49, grabs 36/44, all four throws, Cross Slash 40/31/55, Limit Cross Slash 60) and disagrees
for the clips that skip or compress animation (forward smash 45 against a 66-frame move; Climhazzard 65 with no
UFD total; Finishing Touch 83/87 against 84/87). It is a cross-check, never the duration source.

The importer rewrites the whole package and `sources/stratus/ledger.json` (raw parsed attack rows per slot, with
each row's source frame and argument map). Hand edits to `fighter.json` are lost on regeneration; authoring rules
belong in `tools/import-stratus/import.py`.

### Disagreements and source selection

| Property | Source A | Source B | Chosen value and reason |
| --- | --- | --- | --- |
| Limit Charge duration | Task brief: "7 s in the source" | UFD: "100 Unit cap and fills by 0.3 Units per frame, so 18 Units a second"; SmashWiki: 0.3/frame since 7.0.0, 334 frames (5.567 s) | **334 held frames**. The 7-second figure is the pre-7.0.0 rate (0.25/frame, 400 frames); patch 13.0.1 is 0.3/frame and both pinned sources agree. Recorded rather than silently following the brief. |
| Finishing Touch windows | UFD: startup 12, active `12(13—14)/22—26`, total 84 | Script: 1.0 hits on frames 16, 17, 18, 19; windbox on 27; `FT_MOTION_RATE(0.688)` from frame 1 | **UFD gameplay windows** (phase 12–13 and 21–25). The script frames are animation-frame bookkeeping under a 0.688 motion rate, the same pattern as Throbbin's neutral air. Damage/launch still come from the script. |
| Up-smash windows | UFD: `12(13—14/15—16)` | Script: attacks on frames 14, 15, 17 | **UFD windows** (phase 11, 12–13, 14–15) with the script's three damage tiers (13.0, 12.0/11.0, 8.0). The script runs at 0.83/0.738 motion rates before frame 14. |
| Forward-smash windows | UFD: 19/24/28 | Script: 19/24/28 | Agree; three windows (phase 18/23/27) are authored separately. |
| Forward air clean tier | UFD: 13.0 early / 14.0 tipper / 11.0 late | Script: 14.0 spike (air), 14.0 (ground), 13.0 large box, 13.0 arm box, then 11.0 at frame 21 | **Both**: the 14.0 air/ground tippers plus the 13.0 large arc box and the 13.0 arm box; 11.0 is the late hit at 20–24. UFD's "13.0 early" is the arc box, not a fourth damage value. |
| Blade Beam late tier | UFD/script: 8.0 then 6.0 after 18 frames (air 6.4 then 4.8) | engine projectile carries one contact hit | **Early tier only** (8.0 ground / 6.4 air). The late tier would need a time-since-spawn damage switch the projectile model does not express. |
| Limit Blade Beam | Script: 2.0 autolink rehit 6 (up to eight multi-hits) then a 3.0 launcher; SmashWiki: 19% total | engine projectile carries one contact hit | **19.0 (ground) / 15.0 (air) in one contact hit** with a *fitted* KBG so the launch at 100% matches the source's final 3.0 hit; see the conversion table. The multi-hit animation is not modeled. |
| Limit Break status | SmashWiki: mobility boosts, 15-second expiry, gauge gain from dealing/taking damage | engine has no per-resource stat modifiers | **Not modeled**: the gauge only comes from Limit Charge and has no expiry. Recorded. |
| Cross Slash continuation | SmashWiki: natural combo, only continues if each hit connects | engine cannot gate a phase chain on "it hit" | **Auto-chained at the earliest cancel** (12 + 14 + 55 frames). A whiff still runs the whole sequence. Recorded as an adaptation. |
| Jab 1 inner box | Script: a 2.5 box at Z=14 with a 180° angle, followed by a duplicate at the same point with 361° | first-listed wins in this engine | The 180° row is dropped as shadowed; the far box keeps its 361° Sakurai launch. |
| Throw damage split | UFD: forward throw 4.0/3.0, back 3.0/3.0, up 2.0/2.5/4.0 | script `ATTACK_ABS` launch row plus `ATTACK` catch-only rows | The catch-only rows become `Throw.ticks`; the `ATTACK_ABS` row is the release hit. Totals match UFD (7 / 6 / 8.5 / 7). |

## Conversion rules

| Property | Source units / convention | Destination mapping | Classification |
| --- | --- | --- | --- |
| Frames | 1-based gameplay frames; scripts carry motion rates | Move's first tick = source frame 1 = phase frame 0, so **hit windows shift by one (phase frame = source frame − 1)**. Durations are the source totals; `frames` equals the number of frames in the move | converted |
| Root geometry | `top`-bone `X/Y/Z`, `X2/Y2/Z2` | source Z → forward X, source Y → up, **5 px per unit** (the same scale Mike and Throbbin use for root boxes). Capsules become overlapping circles no farther apart than their radius | converted |
| Weapon/limb geometry | `haver`/`kneer`/`footl`/`footr` bone-local offsets | fitted 2-D positions from the GIF frames and the authored poses, never a projection of Nintendo's skeleton | **fitted** |
| Masks | `FacingRestrict`, `Ground_or_Air` | `dir` (`away`/`back`), `situation` (`ground`/`air`) | converted |
| Launch fields | source BKB/KBG/FKB, angle codes | `bkb` unscaled, `kbg = KBG/100`; 361 → `sakurai`, 366–368 → `autolink`, >180 → negative angle (`spike` past −60°); FKB → fixed-launch `setKb` with the shared conversion | converted |
| Hitlag / shield | `Hitlag`, `ShieldDamage` | `hitlagMult`, `shieldDmgMult` | converted |
| Poses | GIF silhouettes and swing descriptions | authored stick-figure poses (`thrust`, `chop`, `sweepLow`, `sweepUp`, `overhead`, `cross`, `cast`, `spin`, `rise`, `plunge`, air poses) with per-phase attachment angle keys and `hold` so the front arm grips the sword | **fitted** |
| Sword | Buster Sword, 84 px blade | poly blade + guard + grip on `handF`, pale-blue `glow` while attacking, trail sector around the shoulder | **fitted** |
| Movement | UFD: walk 1.155, run 2.167, dash 2.145, air 1.155, air accel 0.07, gravity 0.098, fall 1.68, fast fall 2.688 (units/frame, units/frame²) | × 5 px/unit (`tools/units.py`): walk / air speed / air-jump drift 5.775, run 10.835, dash 10.725, air accel 0.35, gravity 0.49, fall 8.4, fast fall 13.44. `runAccel` 1.4286 is fitted (1.0 at the old ×3.5) | converted (runAccel fitted) |
| Gait frames | Cloud scripts (1-indexed `frame(Frame=N)` markers): `Dash` DASH_TO_RUN, `TurnDash` TURN_DASH, `TurnRun` / `RunBrakeR` RUN_BRAKE_FLAG_STOP at 11, 3, 15, 11; SmashWiki Ultimate Dash table: 10 dash frames | the frames before each marker (`units.gait`): `dashFrames`, `dashTurnFrames`, `turnFrames`, `brakeFrames` 10, 2, 14, 10 (2026-11-05: all but `dashTurnFrames` were one frame too long; `dashFrames` was hand-entered). A released dash skids by traction (`gait.dashRelease`) | converted |
| Jumps | ssbwiki [Cloud (SSBU)](https://www.ssbwiki.com/Cloud_(SSBU)) attributes, accessed 2026-11-05 (unitless game units; the [Jump](https://www.ssbwiki.com/Jump) page's SSBU table row is garbled by rowspans): full hop 32.5, short hop 17, double jump 32.5. Limit Break's jump boost is not modeled | `units.jumps(…, gravity 0.49)`: jump 12.375, short hop 8.8837, air jump 12.865 px/tick. Feel harness measures 162.5 / 85.0 / 162.5 px = source × 5 exactly. Supersedes the earlier platform-fitted 14.8 / 10.4 / 13.8 (old scale) | converted |
| Weight | 100 | `100/100 = 1.0` reciprocal knockback multiplier | converted |
| Pummel | 1.3 | `stats.pummelDamage` | converted |
| Limit wave launch | final hit 3.0, 48°, KBG 226, BKB 50 | 19.0 damage with `kbg = 0.726`, `bkb = 50` (air 15.0 / `kbg = 0.803`), fitted so the launch at 100% matches the source final hit (23.8 px/tick) while carrying the full damage total | **fitted** |
| Body | — | `base` collision body (halfWidth 14, height 74) and hurtboxes | shared |

## Limit resource

`charges: [{ "id": "limit", "uses": 1, "recharge": 0 }]` — one use, never recharges; respawning refills it.

| Slot | Normal timeline | Limit variant (`needs: {limit, 1}`) | Source change |
| --- | --- | --- | --- |
| `dspec` (default) | 13-frame Limit Charge: charge frame 6, `max: 334`, `holdAfterMax: 600`; a `restore` on phase frame 12 with `minCharge: 334` fills the pool on release | 84-frame **Finishing Touch**, cost 1 limit: 1.0% at phase 12–13 with KBG 4.31/BKB 60, and a windbox at 21–25 | SmashWiki/UFD: Limit Charge → Finishing Touch when the gauge is full |
| `nspec` | Blade Beam, 53 frames grounded / 60 airborne, spawn phase frame 17 (UFD frame 18) | Limit Blade Beam, 60 frames, invulnerable phase 9–16, spawn frame 15, faster and 19% total | Script `SpecialN_LB`, `SpecialAirN_LB`; SmashWiki multi-hit 19% and transcendent priority |
| `sspec` | Cross Slash: 12 + 14 + 55 frames, 4.0 / 3.0 / 3.0+3.0+6.0 hits | Limit Cross Slash: one 60-frame sequence, five hits 5.0/5.0/3.0/3.0/10.0, invulnerable phase 5–10 | Script `SpecialS{1,2,3}_LB`; UFD `10—11/15—16/21—22/28/38—39` |
| `uspec` | Climhazzard: 9-frame thrust, 24-frame rise, optional 26-frame plunge + 26-frame landing phase | Limit Climhazzard: 9 + 36, 6.0 thrust and 7.0 rise, 19.1703 px/tick rise against the normal 15.5188 (fitted to the source's 1.5× height), invulnerable move frames 5–12 | Script `SpecialHi_LB`, `SpecialAirHi_LB`; SmashWiki 1.5× height and ledge grab throughout |

The gauge has no HUD meter; the pool is exposed in the snapshot like every other `charges` pool. The Limit
Break movement buff, the 15-second expiry and gauge gain from dealing/taking damage are not modeled
(see Known approximations).

## Move ledger

`startup/active/total` are source gameplay frames (inclusive) as pinned in `ufd.json`; engine phases are one
less. Every row is authored by `tools/import-stratus/import.py`; the raw parsed attack rows it used are in
`sources/stratus/ledger.json`. Geometry marked *fitted* comes from the GIF frames.

| Slot / variant | Source | Startup / active / total | Damage / launch / mask | Startup / recovery notes | Phases / classification |
| --- | --- | --- | --- | --- | --- |
| Jab 1 | Attack11 | 4 / 4–5 / 29 | 2.5, 361→Sakurai, three root boxes at Z 7/10.5/14 | continues into jab 2 from source 8 | `jab1` 29; root boxes converted |
| Jab 2 | Attack12 | 5 / 5–6 / 32 | 2.0, 361, three boxes Z 3/7/11 | continues from source 9 | `jab2` 32 |
| Jab 3 | Attack13 | 6 / 6–7 / 37 | 3.5, 30°, BKB 75/KBG 0.5, root capsule Z 16→8 (r 22.5) | — | `jab3` 37 |
| Forward tilt | AttackS3 | 9 / 9–10 / 35 | 11.0, 361, root capsule Z 14.5→7 (r 30) | — | `main` 35 |
| Up tilt | AttackHi3 | 6 / 6–9 / 32 | 8.0, 95°, `dir: away`, BKB 46/KBG 1.0, four boxes + a vertical root capsule | — | `main` 32; arm boxes fitted overhead |
| Down tilt | AttackLw3 | 7 / 7–8 (9–17) / 40 | 7.0 then 6.0, 80°, BKB 80/KBG 0.75; low front capsule + lingering back-leg box | back-leg intangibility not modeled | `main` 40 |
| Dash attack | AttackDash | 9 / 9–13 (14–18) / 45 | 11.0 then 8.0, 60°/65°, BKB 100/KBG 0.7/0.5, hitlag 1.0 | slide to source 20, forward impulse 6.0 px/tick | `windup` 8, `early` 5, `late` 5, `recovery` 27; body boxes fitted |
| Forward smash | AttackS4 | 19 / 24 / 28 / 66, charge hold 5 (phase 4) | drag hits 3.0 (FKB 25/10, ground/air pair), 3.0/2.0/4.0 at 24, clean 13.0/12.0 at 28 (361, BKB 42/KBG 1.15, hitlag 1.8) | three windows, each its own phase so the drag hits can combo into the clean hit | `windup` 18, `drag1` 1, `link1` 4, `drag2` 1, `link2` 3, `clean` 1, `recovery` 38 |
| Up smash | AttackHi4 | 12 / 13–14 / 15–16 / 45, charge hold 5 (phase 4) | 13.0 (rows persist across the whole window) then 12.0/11.0 then 8.0, 83°, BKB 40/KBG 1.05/0.96/0.9, hitlag 1.2 | the 13.0 rows stay up while the 12.0/11.0 are added, then every row becomes the 8.0 tier | `windup` 11, `hits` 3, `late` 2, `recovery` 29 |
| Down smash | AttackLw4 | 8 / 8–9 and 21–23 / 49, charge hold 2 (phase 1) | front 3.0 ×4 (air/ground pairs, FKB 90/85), back 11.0 at 35°, BKB 35/KBG 0.92, `dir: back` | front then back are separate phases (both can hit) | `windup` 7, `front` 3, `link` 10, `back` 4, `recovery` 25 |
| Neutral air | AttackAirN | 5 / 5–15 / 38, land 9, autocancel 31+ | 8.0, 40°, BKB 25/KBG 1.0, three `haver` boxes | 38 = main 30 + autocancel 8 | four sweeping boxes fitted to the 360° arc |
| Forward air | AttackAirF | 18 / 18–19 (20–25) / 53, land 11, autocancel 1–2 and 43+ | 14.0 spike (air) / 14.0 (ground), 13.0 large arc (45°, BKB 30/KBG 0.83, r 39), 13.0 arm, 11.0 late | 53 = 2 + 15 + 2 + 6 + 17 + 11; the clean and late hits are separate phases | fitted overhead chop |
| Back air | AttackAirB | 11 / 11–12 / 42, land 8, autocancel 1–4 and 37+ | 13.0, 361, BKB 20/KBG 0.95, `dir: back`, two fitted boxes rising behind | 42 = 4 + 6 + 1 + 1 + 24 + 6; each one-frame row is its own phase | fitted rising back slash |
| Up air | AttackAirHi | 8 / 8–9 (10–23) / 44, land 9, autocancel 32+ | 11.0 then 8.0, 80°, BKB 42/KBG 0.8, three `top` capsules above plus three `haver` boxes | 44 = 7 + 2 + 15 + 7 + 13; the 11.0 and 8.0 tiers are separate phases | fitted overhead arc |
| Down air | AttackAirLw | 11 / 11–13 (14–39) / 64, land 16, autocancel 1–4 and 46+ | 15.0 spike (air), 13.0 (60°, BKB 30/KBG 1.05), 8.0 late (65°, BKB 80/KBG 0.55) | 64 = 4 + 6 + 3 + 27 + 5 + 19; early and late are separate phases | mostly vertical box below the feet |
| Grab / dash grab | Catch / CatchDash | 9–10 / 36; 12–13 / 44 | root capsules at 20→43.5 px and 20→52 px | dash grab slides to source 18 | `main` |
| Pummel | CatchAttack | 1 | 1.3 (`stats.pummelDamage`) | shared hold timing | inherited |
| Forward throw | ThrowF | release 9 / 35 | release 3.0 at 49°, BKB 48/KBG 1.25; 4.0 tick at source 7 | shared motion | `ticks` |
| Back throw | ThrowB | release 15 / 31 | release 3.0 at 38°, BKB 48/KBG 1.10; 3.0 tick at source 13 | shared | `ticks` |
| Up throw | ThrowHi | release 15 / 50 | release 4.0 at 80°, BKB 78/KBG 0.97; 2.0 and 2.5 ticks at source 8/13 | shared | `ticks` |
| Down throw | ThrowLw | release 18 / 43 | 7.0 at 122°, BKB 40/KBG 1.10 | shared | — |
| Neutral special | SpecialN / SpecialAirN | 18 / 53 ground, 60 air | wave 8.0 ground / 6.4 air at 55°, BKB 38/KBG 0.60, shield damage 0.5×; ground windbox on 15–16 | spawn at source 18 | `startup` 18 → `groundTail` 35 / `airTail` 42 |
| Side special | SpecialS1–3 | 10 / 40; 2 / 31; 2,11,25 / 55 | 4.0 (75°/112°/autolink), 3.0 (95°/60°), 3.0+3.0 fixed-KB then 6.0 at 50° | auto-chained at the earliest cancel | `slash1` 12 → `slash2` 14 → third slash: `windup` 1, `hit1` 2, `link` 7, `hit2` 2, `link2` 12, `hit3` 2, `recovery` 29 |
| Up special | SpecialHi / Hi2 / Hi2Landing | 7 / 10 / 20+ | 3.0 (FKB 80) thrust, 4.0 rise (FKB 121 then BKB 98/50), 4.5 plunge (270° air spike / 45°), 3.5 landing hit at 64° | rise 15.5188 px/tick from move frame 8 (fitted so the rise clears a full hop; Limit 17.5), ledge grab from source 7; special press from move frame 19 (Limit 32) starts the plunge; otherwise helpless | `thrust` 9, rise split into 1 + 1 + 5 + 17 (Limit 1 + 1 + 4 + 30), `plunge` 26 (hitbox persists to landing), `landHit` 26 |
| Down special | SpecialLw | — | no hitboxes | charge 6, max 334, restore at phase 12 when the hold reached the cap; shield cancels | `charge` 13 |
| Limit: Finishing Touch | SpecialLw with `LB_SCENE` | 12 / 13–14, 22–26 / 84 | front 1.0 at 80° (BKB 60/KBG 4.31) on 11–14, rear 1.0 (KBG 4.06) on 21–25, plus a centre windbox (`dir: away`) that pushes both sides | the front and rear sweeps are separate phases | `windup` 11, `front` 4, `link` 6, `rear` 5, `recovery` 58 |
| Limit: Blade Beam | SpecialN_LB / AirN_LB | 16 / — / 60 | 19.0 ground / 15.0 air total (fitted single contact), invulnerable 10–17 | spawn phase 15, invuln phase 9 | `main` 60 |
| Limit: Cross Slash | SpecialS1–3_LB | 10 / 15 / 21 / 28 / 38 | 5.0, 5.0, 3.0, 3.0, 10.0 (50°, BKB 30/KBG 1.47), invulnerable 6–11 | one sequence, no inputs; each window is its own phase | 11 phases from `windup` 9 to `recovery` 21 (total 60) |
| Limit: Climhazzard | SpecialHi_LB / AirHi_LB | 7 / 10 / — | 6.0 thrust, 7.0 rise (88°, BKB 50/KBG 1.42), invulnerable 5–12 | 19.1703 px/tick, follows up from move frame 32 (source 33) | `thrust` 9, rise split 1 + 1 + 4 + 30 (36) |

Special notes:
- The Limit pool is spent by the variant's first phase `cost`, at move start. An interrupted Limit special still
  spends the gauge (the pool is charged on phase entry), matching the source's one-use rule.
- Limit Charge's `restore` event sits on the final animation frame with `minCharge: 334`, so only a hold that
  reached the source's 100-unit cap fills the gauge; a shorter tap plays the animation and restores nothing.
- `recharge: 0` means the spent gauge never returns during a stock; respawning refills it.
- Aerial landing lag and autocancels come from UFD (forward air 11, back air 8, neutral/up air 9, down air 16);
  landing inside an ending autocancel window ends the move with no lag.

## Validation and handoff

- Reference tests (`crates/sim/tests/stratus.rs`, 20): package identity and all 19 slots + four throws; the
  one-use `limit` pool with `recharge: 0`; absolute hit windows (forward smash 18/23/27 with 3.0/2.0/13.0 tiers;
  forward tilt 8–9; down tilt's 7.0 box through 17 and 6.0 box through 7; back air's two one-frame windows;
  forward air's 14.0 spike/ground, 13.0 arc and 11.0 late on 19–24; down air's 15.0/13.0 on 10–12 and 8.0 on
  13–39; up air's 11.0 and 8.0 tiers; up smash's persisted 13.0 rows and 8.0 late tier; down smash front/back);
  every move and throw duration against the pinned UFD totals; Limit Charge fills at exactly 334 held ticks and
  not at 333 or a short tap; all four Limit variants selected and the pool spent, then falling back; empty-meter
  timelines; Finishing Touch's 1.0%/KBG 4.31 front and rear hits and the centre windbox; Limit Blade Beam damage
  and spawn; Cross Slash auto-chaining `slash1 → slash2 → third slash`; **multi-hit contact** (forward smash
  drag → drag → clean lands three times; Limit Cross Slash lands five windows for 26%); Climhazzard's first
  rising hit on move frame 9, follow-up from move frame 19 (Limit 32), the lingering plunge hit and the 26-frame
  follow-up landing phase; aerial landing lag and ending autocancel; Blade Beam spawning on phase 17 and not
  before; contact reach at close range with nothing at 220 px; raw-input routing for jab/tilts/specials/grab;
  respawn refill.
- Multi-hit structure: groups distinguish separated source windows. Continuous swings preserve contact
  history through coverage/tier phase splits, so re-created boxes do not grant extra hits. Durations are
  preserved; the ledger lists the presentation phase splits.
- Contact coverage: `move_reach_matches_fitted_geometry` pins both fighters and asserts jab 45/60, forward tilt
  60/80, forward smash 70/85, down tilt 45/60, neutral air 40/55, forward/back air 60/80, up air and down air 35,
  and nothing at 220 px.
- Shared-behaviour coverage: `tests/content.rs` roster expectation; `tests/stages.rs`
  `new_fighter_cpu_matchups` (4 stages, 3 opponents, 4 seeds) and the automatic
  `platforms_are_in_jump_reach` / `every_ledge_is_recoverable` / `cpu_matches_finish_on_every_stage` sweeps.
  Observed: arena 8/12 wins, 35 KOs, 4 self-KOs; citadel 11/12, 32, 0; foundry 11/12, 35, 0; isles 12/12, 34,
  0 (42/48 wins, 136 KOs, 4 self-KOs). Every match finished.
- `just balance 4 stratus` (4 seeds, every opponent and stage, 256 matches): **0 unfinished**, 211 wins,
  86.3k damage dealt / 63.8k taken, 243 deaths, 22 self-KOs, 81 recovery deaths. Per-move contacts are spread
  across the kit (up air 1559, down air 1895, up tilt 1581, Cross Slash 1723, forward smash 158, Blade Beam 147,
  Finishing Touch 47 with 10 KOs, Climhazzard 426); grabs show zero damaging hits because a grab deals no
  damage, while the pummel (411) and throws (1774) prove the catch works.
- No new engine semantics were needed: Limit uses the existing `charges`, `restore`, `cost` and `variants`
  fields from the Throbbin import. `SIM_VERSION` is unchanged.
- Visual inspection, two passes.
  1. **UFD per-move hitbox GIFs** (41 files, hashes in `visuals.json`): frames around every active window were
     opened for forward/up/down smash, forward tilt, up/down tilt, dash attack, jab 3, neutral/forward/back/up/down
     air, both grabs, Cross Slash 1–3 and Limit Cross Slash, Climhazzard (+Limit), Finishing Touch and both Blade
     Beam waves. That pass set the pose silhouettes and the fitted hitbox directions: neutral air is a 360° body
     swing (front, over the head, behind, then low), forward air is an overhead chop whose 13.0 arc circle sits in
     front at head height while the 11.0 late hit is low, back air rises behind, up air arcs above the head, down
     air is a vertical box under the feet, down tilt is a flat low sweep, up tilt/up smash cover above and in front,
     and Finishing Touch's windbox covers both sides.
  2. **Window screenshots** (`31a`–`31v` in the `fighter-kits` section; a display-unlocked run produced real
     captures and `31a`–`31l` were opened at full resolution): forward smash (blade level through the dummy), up
     smash (blade overhead with the hitbox over the sword), down smash (low front sweep then the back sweep),
     neutral air (blade up on the frame-8 arc), forward air (blade forward with the large arc hitboxes), back air
     (blade rising behind), up air, down air (blade straight down under the feet), jab, forward/up/down tilt. The
     blade, grip and gold guard track the front hand, the trail follows the swing, and facing matches the hitbox
     side. The specials, grab, dash attack and throw poses were checked through the CPU-canvas pass below rather
     than the window captures. Later capture attempts (after the multi-hit phase split, which re-bases the same
     attachment keys) came back black again, so the window evidence is the `31a`–`31l` set.
  A CPU-canvas rasterisation (`render::draw::draw_fighter`, the portrait-sheet path) was also used while fitting
  the per-phase attachment angle keys, since it isolates the pose from the editor timeline.
- Controller playtest: not performed.
- Known approximations / remaining work:
  - Limit Break's movement buff, 15-second expiry and gauge gain from damage dealt/taken are not modeled; the
    gauge only comes from Limit Charge.
  - Blade Beam's late (distance-based) damage tier is omitted; the wave flies straight and does not follow stage
    contours. Its Limit version is a single 19%/15% contact rather than the 2.0 multi-hit sequence, with a fitted
    KBG so the launch matches the source final hit.
  - Cross Slash auto-chains instead of requiring a press and a connecting hit; the first two slashes' remaining
    animation time is skipped, so the combined move is 81 frames.
  - Weapon and limb geometry, poses and hurtbox coverage are fitted; hurtboxes and the collision body are
    inherited from `base`.
  - Finishing Touch's air variant is 3 frames longer in the source (87 vs 84); one timeline is authored.
  - Limit Charge's cancel-on-special-press is not modeled (shield cancels instead).
  - The Buster Sword is drawn on `handF` with a pose-driven grip; it is not a two-hand IK rig.
- Delivery: worktree branch `t3/import-stratus-fighter`, uncommitted working tree (package, sources, tools, tests, shots,
  docs).

## Contact identity correction (2026-11-04)

Inclusive source creation/clear windows that overlap transitively are one hit identity. Regeneration assigns
`group` across the whole timeline before splitting presentation/landing phases, then preserves history on
timeout transitions. Coverage and damage-tier changes cannot hit the same victim again; separated windows
retain independent groups. Full-move contact probes cover the continuous swings and genuine multi-hits.

The late-window regression reads `sources/stratus/ufd.json` directly: up smash's 8% tier is source 15–16,
up air ends on 23, and down air ends on 39. Engine endpoints are each one less; no phase split extends them.

Momentum migration (2026-11-04): `drift()` in the importer rewrites phase `airAccelMult` for `movement.ultimate` (airAccel × kind multiplier) keeping each phase's effective drift; regenerated with `just import-stratus`.

Unified 5 px/unit scale (2026-11-05): movement stats are UFD source × 5 and jump speeds come from the source
jump heights (see Conversion rules); the earlier ×3.5 / ×6 stats and fitted jump speeds are superseded. Fitted
velocities in the generator were converted per axis (x ×5/3.5, y ×5/6): dash attack impulse 4.2 → 6.0,
Blade Beam 11 / 9 → 15.7143 / 12.8571, Limit Blade Beam 14 / 11 → 20.0 / 15.7143, Climhazzard rise 17 / 21 →
15.5188 / 19.1703 (× √(5/6), same height), plunge −14 → −11.6667 (capped by the 8.4 fall speed). `airAccelMult` ratios are unchanged
(scale-free). Feel harness (`FEEL_FIGHTER=stratus … feel_report`): full 162.5 px (apex tick 26), short 85.0 px
(19), double jump 162.5 px (26). Test change: the lingering-plunge check starts at x 1000; from the centre the
plunge now lands on the rebuilt Arena's top platform (y 248) before its hitbox opens (this failed with the
old content too).

## Ultimate calibration (2026-11-05)

- **Traction / air friction**: Cloud (`cloud`) `ground_brake` 0.106 / `air_brake_x` 0.009 (fighter_param dump
  github.com/j99thoms/smash-charts `data/raw/ultimate_fighter_params.csv`, matching ssbwiki Traction and Air
  friction; high confidence) → `stats.traction` 0.53, `stats.airFriction` 0.045. Limit Break traction 0.1219 is not modelled.
- **Full hop initial height**: Ultimate climbs 0.55 × the full hop in its first four airborne frames (jump_initial_y 17.875 = 0.55 × 32.5);
  the generator emits `stats.jumpInitialHeight` and derives `jumpSpeed` from the remaining height (`units.jumps`),
  so the full hop keeps its height and lands on Ultimate's frame. Short hops and air jumps are unchanged.
- **Fixed knockback**: FKB hits emit `fkb` with the source BKB/KBG (Ultimate WBKB, weight-correct) and keep `setKb`
  as a marker; they were resolved at weight 100 into a `setKb` BKB.
- **Aerial autocancels**: forward, back and down air now autocancel on their opening frames (UFD 1–2, 1–4, 1–4);
  the generator gave those startup phases landing lag.
- `just calibrate stratus`: every compared move matches the reference within one KO point (down smash 133 vs
  132), hitstun and launch speed exact; air times 49 / 37 (UFD 49 / 37); run 10.835 px/tick.
