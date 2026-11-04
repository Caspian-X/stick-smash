# Bleth reference and conversion record

Playable SSBU **Byleth** adaptation, packaged as **Bleth**. It is the first import that draws four separate
weapons from one kit: the **Sword of the Creator** (whip-sword), **Areadbhar** (lance), **Aymr** (axe) and
**Failnaught** (bow). It uses one hidden attachment per weapon, shown per phase, so each move visibly wields the
weapon the source gives it.

## Target and status

- Package/id: `assets/fighters/verdant/fighter.json`; display name **Bleth** (`verdant`), inheriting only the
  abstract `base` package.
- Branch: `t3/db74d87e` (this worktree). Source game: Super Smash Bros. Ultimate, patch **13.0.1**.
- Intended identity: very long range, slow, high-damage reads. Areadbhar stabs, Aymr slams, Failnaught charges a
  piercing arrow and the Sword of the Creator stretches into a whip and tethers ledges. Ground mobility is below
  the roster average and the smashes are near-helpless on shield.
- State: research, authoring, focused tests, stage/CPU coverage and visual inspection complete. No controller
  playtest was performed.

## Sources

| Source | URL / local path | Patch / commit / hash | Access date | Establishes |
| --- | --- | --- | --- | --- |
| Rubendal Byleth scripts | https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/Byleth/data.json → `sources/verdant/scripts.json` | commit `2e8f0d245481cb0c936524b6ffe1cdea867c4f68`, patch 13.0.1, full-input sha256 `77626794b2fcf703c44fdb81f70242c66e966fcefd70fd5c5bee901a17352ae7` | 2026-11-04 | Attack ids, article/bone per move, geometry, masks, damage, angles, BKB/KBG/FKB, hitlag, rehit, shield damage, armour flags and the four weapon articles |
| Ultimate Frame Data: Byleth | https://ultimateframedata.com/byleth/ → `sources/verdant/ufd.json` | page sha256 `5738f310fff258fedc773923e582e496b8d31dc4801e444281cfcdc5a543a62f` | 2026-11-04 | Startup/active/total, damage variants, landing lag, autocancels, charge frames, grab/throw timing, tether and armour notes, movement stats |
| UFD per-move hitbox GIFs | `https://ultimateframedata.com/hitboxes/byleth/<Name>.gif` (36 files) → `sources/verdant/visuals.json` | per-file sha256 and frame counts recorded; acquired 2026-11-04 | 2026-11-04 | Per-frame hitbox coverage, weapon silhouette, swing direction and reach for every normal, aerial, grab, throw and special |

Regeneration:

```sh
just import-verdant                   # tools/import-verdant/import.py, then just bless
just verdant-visuals /tmp/verdant-reference   # download + hash-check the hitbox GIFs, write frame montages
```

The importer rewrites the whole package and `sources/verdant/ledger.json` (the parsed attack rows per script
with their motion-rate commands, plus each authored phase's weapon and hit windows). Hand edits to
`fighter.json` are lost on regeneration; authoring rules belong in the importer.

The GIFs live under `hitboxes/byleth/`. Two of the names the character page links redirect
(`BylethJapRapidFinisher` → `BylethJabRapidFinisher`, `BylethAymrSuperArmor` → `BylethAymrSuperArmour`); the
download tool follows them. GIF frame 0 is source gameplay frame 1 for the clips whose frame count equals the
UFD total (jab 22/24/34, ftilt 34, utilt 34, dtilt 35, dash attack 44, fsmash 63, usmash 56, nair 51, fair 39,
bair 46, uair 48, dair 59, grab 39, dash grab 48, fthrow 35, bthrow 43, uthrow 37, dthrow 31, Failnaught 80/160,
Areadbhar 65/61, Sword of the Creator 46). Down smash (73) and Aymr (145) do not line up, so their frame counts
are not used as durations.

### Weapon mapping (from the pinned scripts, not guessed)

| Weapon | Article / bone in the scripts | Moves |
| --- | --- | --- |
| Sword of the Creator | default sword (`sword1`/`armr`/`top`), `master_sword` article | jab 1-3 and the rapid flurry, forward/up tilt, **down tilt** (article), dash attack, **up smash** (article), up air, up special (article) |
| Areadbhar (lance) | `master_spear` article, `haver` bone | **forward smash**, forward air, back air, side special |
| Aymr (axe) | `master_axe` article, `haver` bone | **down smash**, down air, down special |
| Failnaught (bow) | `master_bow` article + `master_arrow1/2` | **neutral air**, neutral special |

The source grab uses `CATCH` on the top bone with no article, so grab and dash grab draw no weapon. Throws use
the `master_sword` article but run the shared hold/throw animations, which carry no attachment keys; no weapon is
drawn during them (recorded below).

### Disagreements and source selection

| Property | Source A | Source B | Chosen value and reason |
| --- | --- | --- | --- |
| Forward smash damage tiers | UFD: Normal 12.0/18.0, Down 11.5/17.25, Up 13.0/19.5 | scripts: three separate move scripts (`AttackS4`, `AttackS4Lw`, `AttackS4Hi`) with those values | The destination `fsmash` slot has no stick-angle variant. Only the normal swing (12.0 body / 18.0 tip) is imported; the two angled variants are recorded as an explicit omission. |
| Hit windows vs script frames | UFD gameplay frames (e.g. fsmash 23-25) | scripts are animation frames, retimed by `FT_MOTION_RATE(0.8)` around the smash hold | UFD gameplay frames win for every phase window and total. Script frames are only used to read the hit data order. |
| Down special | UFD: grounded 30.0 hit / 8.0 shockwave, active 62-67 / 2-5, armour 34-63 | `master_axe` `SpecialLw` (30.0 at animation frame 61), `SpecialAirLw` (25.0) and `SpecialLwHit` (four 8.0 boxes on frames 2-5, grounded only) | The grounded and aerial impacts are separate phases (`impact` 30.0, `impactAir` 25.0). The shockwave follows the impact as its own 4-frame phase with the source's 8.0/88° grounded-only hit. |
| Up special damage | UFD: 1.0 hit / 3.0 grab / 6.5 final | scripts: 1.0 autolink at frame 16-17, 3.0 multihits through the whip, no plain 6.5 row (the 6.5 is the `Overtake` hitgrab throw) | The whip's 1.0 autolink and 3.0 multihit are imported. The hitgrab throw is not modeled (see approximations). |
| Neutral special charge | UFD: uncharged shot at 45, fully charged at 114, totals 80/160 | `SpecialN` / `SpecialNMax`: the second only exists once the fully charged arrow article is generated | Two tiers, matching the source. The engine's hold frame is the last draw frame (43) with a 69-tick hold; releasing earlier still looses at the minimum draw. |
| Rapid jab cadence | UFD: hits on 11, 14, 17, 20, 23… | `Attack100Sub`: 0.4 on the first frame of a restarting 1-frame window | A looping phase with a `rehit: 3` box, which reproduces the three-frame cadence (and the same damage/knockback per hit). |

## Conversion rules

| Property | Source units / convention | Destination mapping | Evidence / limitation |
| --- | --- | --- | --- |
| Frames | 1-based gameplay frames; scripts carry motion rates | Move's first tick = source frame 1 = phase frame 0, so **phase frame = source frame − 1**; `frames` equals the source total directly | A 63-frame forward smash runs 63 ticks. Multi-hit windows and landing lag follow the UFD rows. |
| Root geometry | `top`-bone `Z`/`Y`/`Size` | 5 px per source unit, source Z → forward X, source Y → up; capsules become overlapping circles | Calibrated against the UFD hitbox circles: a 4.5-unit box is drawn at ~22 px radius next to a 74 px figure. |
| Weapon geometry | `haver`/`sword1`/`armr` bone offsets | Fitted weapon tracks per move from the UFD frames and the authored attachment angles; never a projection of Nintendo's skeleton | **fitted**. Reach tests bound every slot's furthest hitbox to the authored weapon length. |
| Weapon length | — | Sword of the Creator 62 px (whip scale up to 2.4 for down tilt/up smash/up air/up special), Areadbhar 126+12 px, Aymr 62+30 px, Failnaught 40 px bow | **fitted** from the per-move GIFs. |
| Launch fields | source BKB/KBG/FKB, angle codes | `bkb` unscaled, `kbg = KBG/100`; 361 → `sakurai`, 366-368 → `autolink`, >180 → negative angle (`spike` past −60°), FKB → fixed-launch `setKb` | converted |
| Masks | `FacingRestrict`, `Ground_or_Air` | `dir` (away/back), `situation` (ground/air) | converted. The down air's air/ground pair becomes two boxes with situations, keeping the 270° spike only in the air. |
| Hitlag / shield | script `Hitlag`, `ShieldDamage` | `hitlagMult`, `shieldDmgMult` | converted (down smash: 8 shield damage on 23 → ×1.3478). |
| Armour | Aymr "damage-based armour frames 34-63, 36% in 1v1" | `slam` phase `armorThreshold: 36` over source frames 35-61 | converted window; the per-hit threshold is the engine's armour rule. |
| Poses | UFD swing frames | Authored stick-figure poses (`thrust`, `slashDown`, `sweepUp`, `sweepLow`, `overhead`, `slam`, `draw`, `whipLow`, air poses) with per-phase attachment angles and `hold` so the front arm grips the weapon | **fitted** |
| Movement | UFD: walk 0.97, run 1.43, dash 1.8, air 0.89, air accel 0.053, gravity 0.089, fall 1.6, fast fall 2.56 | ×3.5 for speeds (walk 3.395, run 5.005, dash 6.3, air 3.115, accel 0.1855); gravity/fall ×6 (0.534 / 9.6 / 15.36); jump speeds fitted so a full hop clears the built-in platforms (13.4 / 9.4 / air jump 12.9) | converted + adapted (jumps) |
| Weight | 97 | `100/97` reciprocal knockback multiplier | converted |
| Pummel | 1.5 | `stats.pummelDamage` | converted |
| Hurtboxes | none in the pinned scripts | inherited `base` body (half width 14, height 74) and hurtboxes | **shared** |

## Move ledger

`startup/active/total` are source gameplay frames (inclusive) as pinned in `ufd.json`; engine phases are one
less. Every row is authored by `tools/import-verdant/import.py`; the parsed attack rows are in
`sources/verdant/ledger.json`.

| Slot / variant | Source | Startup / active / total | Damage / launch / mask | Weapon | Phases / notes |
| --- | --- | --- | --- | --- | --- |
| Jab 1 | Attack11 | 4 / 4-5 / 22 | 1.5, three 361/180 rows → Sakurai | Sword | Continues into jab 2 from phase 10 while attack is held |
| Jab 2 | Attack12 | 4 / 4-5 / 24 | 2.0, Sakurai | Sword | Continues into jab 3 from phase 8 |
| Jab 3 | Attack13 | 5 / 5-7 / 34 | 4.5, Sakurai | Sword | Continues into the flurry from phase 10 |
| Rapid jab | Attack100Sub | 11, 14, 17… | 0.4 ×2 boxes, Sakurai, rehit 3 | Sword (whip scale 1.8) | Loops until the button is released; reach 120 px |
| Rapid finisher | Attack100End | 6 / 6-7 / 50 | 3.5, 54°, KBG 1.62 | Sword (whip) | Capsule converted from the script |
| Forward tilt | AttackS3 | 8 / 8-10 / 34 | 11.0, 35°, BKB 48, KBG 0.76 | Sword | Overhead-to-forward slash |
| Up tilt | AttackHi3 | 9 / 9-15 / 34 | 10.0, 92-101°, BKB 66, KBG 0.83 | Sword | Rising arc; five script boxes folded into a fitted sweep plus one 18 px root circle |
| Down tilt | AttackLw3 | 13 / 13-15 / 35 | 8.0, 93-96°, BKB 67, KBG 0.55 | Sword (whip scale 2.1) | Two root capsules reaching 141 px |
| Dash attack | AttackDash | 9 / 9-11 / 44 | 9.0 sour / 13.0 sweet, Sakurai, BKB 61, KBG 0.68 | Sword | Forward impulse (x 4.2) and slide frames |
| Forward smash | AttackS4 | 23 / 23-25 / 63, charge 14 | 18.0 tip (36°, BKB 60, KBG 0.90, hitlag 1.45) and 12.0 body (Sakurai, BKB 55, KBG 0.90) | Areadbhar | Tip boxes fitted at 132/150 px; body box converted as a root capsule |
| Up smash | AttackHi4 | 13 / 13-16 (rehit 7), 17-27 (rehit 4), 28-29 / 56, charge 7 | 2.0 autolink multihits, 10.0 finisher (79°, BKB 68, KBG 1.04) | Sword (whip scale 2.2) | `windup` 27 (whip column to 170 px), then `final` 2 and `recover` 27; the finisher is its own phase so it can hit a victim the multihit already touched |
| Down smash | AttackLw4 | 19, 29 / 19-21, 29-31 / 74, charge 3 | 23.0 both sides (63°, BKB 74, KBG 0.66), 8 shield damage, hitlag 1.4 | Aymr | `front` 28 then `back` 46; the phase split lets both swings hit the same victim (axe-head boxes fitted at 78/-74 px) |
| Neutral air | AttackAirN | 6 / 6-26 (rehit 7), 28 / 51, land 13, AC 1-5 / 39+ | 2.5 multihit, 3.0 finisher (51°, KBG 1.5), 1.0 landing hit (70°, BKB 60, KBG 0.50) | Failnaught | Swings are `swing1` (multihit) and `swing2` (finisher, own phase) so the finisher lands after the multihit; the landing hit is a two-frame phase that then runs 11 frames of shared landing lag |
| Forward air | AttackAirF | 12 / 12-13 / 39, land 11, AC 1-2 / 36+ | 12.75 sweet (Sakurai, BKB 52, KBG 0.84), 8.5 sour (48°) | Areadbhar | Lance fitted to 122 px |
| Back air | AttackAirB | 13 / 13-14 (15-17) / 46, land 13, AC 1-5 / 40+ | 15.0 sweet / 10.0 sour at 28°/46°, then 7.5 / 5.0 late | Areadbhar | Lance swung behind |
| Up air | AttackAirHi | 10 / 10-19 (rehit 5), 20-23 / 48, land 14, AC 1-3 / 48+ | 3.0 autolink multihits, 5.5 finisher (81°, BKB 50, KBG 1.4) | Sword (whip scale 2.3) | Whip arc above the head, split into multihit and finisher phases |
| Down air | AttackAirLw | 22 / 22-24 / 59, land 28, AC 1 / 52+ | 19.0: 270° spike in the air and 58-72° otherwise, 6 shield damage | Aymr | Six source rows kept as air/ground pairs; all boxes below the waist, matching the UFD frames |
| Grab / dash grab | Catch / CatchDash | 6-7 / 39; 10-11 / 48 | grabboxes at 25-45 px (r 16.5) and 22-48 px (r 15) | none | Dash grab slides (18 frames) |
| Pummel | CatchAttack | 1 / 22 | 1.5 (`stats.pummelDamage`) | none | shared hold state |
| Forward throw | ThrowF | release 13 / 35 | 6.0, 38°, BKB 60, KBG 1.0 | shared | shared throw motion |
| Back throw | ThrowB | release 30 / 43 | 7.0, 40°, BKB 50, KBG 0.8 | shared | shared |
| Up throw | ThrowHi | release 15 / 37 | 9.0, 76°, BKB 60, KBG 1.1 | shared | shared |
| Down throw | ThrowLw | release 13 / 31 | 5.0, 74°, BKB 50, KBG 1.5 | shared | shared |
| Neutral special | SpecialN / SpecialNMax | draw hold at 43 (44-frame draw), loose 36 / max loose 47; totals 80 / 160 | arrow 12.0 (35°, BKB 39, KBG 0.78, shield 5) or fully charged 29.0 (Sakurai, BKB 53, KBG 0.56, shield 9) | Failnaught | 69 held ticks reach the 114-frame full draw; releasing earlier looses at the minimum draw. Shield cancels from phase 20. `bReverse` |
| Side special | SpecialSStart/SF/SFDash + SpecialAirSF/SpecialAirSFDash | ground 20 / 20-24 / 65, air 21 / 21-24 / 61 | ground 17.25 sweet (59°, BKB 115, KBG 0.40) / 11.5 sour (45°, BKB 85, KBG 0.35); air 14.25 (55°, BKB 86, KBG 0.59) / 9.5 (45°, BKB 85, KBG 0.35) | Areadbhar | Ground: 19-frame wind-up, 5-frame lunge (forward impulse x 6.0), 41-frame recovery. Air branch: 5-frame down-forward thrust and 37-frame air recovery (landing lag 16). The ground sweetspot starts one frame after its sour boxes, as the source does |
| Up special | SpecialHi / SpecialAirHi + `master_sword` | 9 ground / 10 air / 46, tether from F7 | 3.0 multihit (75°, BKB 40) through the whip; the 1.0 autolink is folded in | Sword (whip scale 2.4) | `startup` 8 already opens the tether from its frame 6 (source frame 7) with a 30-210 px rectangle; `whip` 9 carries the 3.0 rehit-3 hit; then `rise` (29 frames, y 10.5) → landing lag 24 or helpless |
| Down special | SpecialLw / SpecialAirLw / SpecialLwHit | 62 / 62-67 / 133, armour 34-63 | grounded 30.0 (51°, BKB 60, KBG 0.83, shield 25), aerial 25.0 (BKB 85, KBG 0.78), shockwave 8.0 (88°, BKB 100, KBG 0.30, grounded only) | Aymr | `raise` 33, `slam` 28 with the 36% armour from source frame 34, grounded/aerial `impact` 6 (62-67, armour kept through it), `wave` 4 and `recover` 62 = 133 |

Special notes:
- The four weapons are **hidden attachments**; every phase of a move keys its weapon's `show` window and its
  swing angle, so a move never draws more than one weapon. Nothing is drawn at rest.
- The neutral-special charge is binary, matching the source's two arrow articles. Partial charge uses the
  uncharged arrow.
- The down special's shockwave is a phase, not a projectile: the engine's projectile model would have needed a
  fixed spawn and the shockwave is anchored to the axe impact.
- Groups distinguish separate hits and loop-to-finisher seams, while `preserveHits` retains contact history
  across presentation phase splits. A finisher can therefore hit after its multihit loop without granting
  extra contacts to ordinary damage/coverage changes.
- Neutral special's charge hold ends on the source's 45th frame even if the button is released earlier. A held
  full charge fires automatically on source frame 114 and completes after 160 ticks, with no extra saturated hold.
- The up special's tether is the shared `Phase.tether`/`ledgeGrab` system; no rope simulation.

## Validation and handoff

- Reference tests (`crates/sim/tests/verdant.rs`, 16): package identity and the full slot/throw ledger; the
  source per-move weapon mapping with one hidden attachment each; active windows and totals for every normal,
  tilt, smash, aerial and grab; damage/launch/armour spot checks (fsmash 18/12, dsmash 23 + 8 shield, dair
  spike/ground split, usmash autolink/finisher, arrow 12/29, dspec 30/25 + 36 armour, throws 6/7/9/5, movement);
  the duration ledger for all 19 slots; arrow charge tiers (0 and 130 held ticks spawn the 12% and 29% arrows,
  early release waits for source frame 45, and a saturated hold still looses the charged arrow); tether recovery
  at 100 px from source frame 7 on (and not at 215 px or after the whip window); fitted reach bounds for every
  moving slot plus contact checks (fsmash hits at 120 px and misses at 200, dtilt at 100/160, grab at 40-55 px
  and not 90); multi-hit-to-finisher and both down-smash swings landing on one pinned victim; the side
  special's ground/air branch totals and its one-frame sweetspot delay; the down special's phase partition and
  impact frame; every move completing; and raw-input routing for jab/tilts/specials/grab/dash grab plus the
  jump+A short-hop aerial.
- Stage coverage: `tests/content.rs` roster and `tests/stages.rs` `new_fighter_cpu_matchups` include `verdant`
  (every stage, three opponents, four seeds). All matches finish.
- `just balance 4 verdant` (4 seeds, four stages, every opponent, seat-swapped, 256 matches): 0 unfinished,
  108 wins, 63 614 damage dealt / 80 016 taken, 384 deaths, 50 self-KOs, 150 recovery deaths. Per-move hits are
  spread sensibly: up air 2943 (its multihits and finisher now both land; 186 KOs), up tilt 898, down air 550,
  dash attack 530, pummel 483, forward tilt 469, neutral air 237, neutral special 219 (31 KOs), forward smash
  192, side special 176, forward air 93, up smash 83, down special 52, back air 53, down tilt 42, down smash 16.
  Grabs report 0 hits from about 2200 starts: the shared CPU presses grab outside its 61 px range (the same
  pattern shows for other fighters), while the focused grab test connects at 40-55 px. All matches finish.
- Visual inspection: the 36 UFD GIFs were downloaded (hashes in `visuals.json`) and the frames around each
  active window were opened for the sword normals, all smashes, all aerials, grabs/throws, Failnaught (both
  charge levels), Areadbhar (ground and air), Aymr (impact, armour) and the Sword of the Creator (ground and
  air). A CPU-canvas pass (`render::draw::draw_fighter`) then rasterized every attack phase with the live
  hitbox circles overlaid. It corrected the down air (the whole swing is below the fighter), widened the bow
  silhouette, aligned the forward-air hitbox height with the lance, retimed the back/up-air weapon angles to
  their phase-relative hit windows, and showed that the sword crossed the head during the grab, which is why
  grabs now draw no weapon (the source grab has no article).
  Finally the real window captures of `just shots /tmp/verdant-shots3 fighter-kits` were opened: `34a`
  (Areadbhar fsmash), `34b` (stretched Sword up smash), `34c` (Aymr down smash), `34d` (whip down tilt), `34e`
  (Sword forward tilt), `34f` (Areadbhar forward air), `34g` (Areadbhar back air), `34h` (Aymr down air),
  `34i` (Failnaught neutral air), `34j`/`34k` (Failnaught draw and loose), `34l` (Areadbhar side special), `34m`
  (Sword tether), `34n` (Aymr down special) and `34o` (Sword jab). Each phase drew exactly one weapon at the
  angle of its swing, with the sources' lance/axe tips and whip length overlying their hitboxes; the captures
  also confirmed that the aerial preview lands before the late active frames, so the down-air shot uses the
  wind-up frame rather than its frame-22 hit.
- Commands run: `python3 tools/import-verdant/import.py`; `just bless` (with a private target dir, so the
  formatter used this checkout's assets); `just verdant-visuals /tmp/verdant-reference`;
  `cargo test -p sim --test verdant` (16 passed); `cargo test --workspace` (all green); `cargo fmt --all -- --check`;
  `cargo clippy --workspace --all-targets -- -D warnings`; `just balance 4 verdant`; and
  `just shots /tmp/verdant-shots4 fighter-kits`.
- Controller playtest: not performed.
- Screenshot tour: `34a`-`34o` cover the Areadbhar forward smash, the stretched Sword up smash, the Aymr down
  smash, the whip down tilt, the forward tilt, forward/back/down air, the Failnaught draw and loose, the
  Areadbhar rush, the tether and the Aymr slam.
- Known approximations and omissions:
  - Fitted weapon geometry and poses; not a projection of Nintendo's skeleton.
  - Forward smash's up/down angled variants are not imported (the slot does not select by stick angle).
  - Failnaught arrow speed, life and collider size are adapted; the pinned scripts carry no article velocity.
    Charge is two tiers, not a continuous scale.
  - Up special's hitgrab and 6.5 throw are not modeled; the whip's 3.0 multihit stands in for both the 1.0
    autolink and the grab. The grounded version rises (adapted) instead of being a stationary whip swing, so it
    works as recovery.
  - Down special's aerial branch has the 25% hit but no shockwave, and the early-landing armour exception and
    the F12-41 turnaround window are not modeled. Armour runs through the impact phase (to source frame 67)
    rather than stopping at 63. Its landing lag (24), the side special's air landing lag (16) and the up
    special's landing lag are adapted.
  - The side special's aerial branch shares the ground wind-up, so its startup is source frame 20 rather than
    21. The air thrust angles and its 14.25/9.5 damage are the pinned `SpecialAirSF` values.
  - Throws use the shared hold/throw animations; no weapon is drawn during them.
  - The neutral air's two multihit boxes are folded into three body circles (front, back, top) rather than the
    script's ±Z bone rows.
  - No wall-jump from the tether, no rapid-jab shield-stun multiplier (9× in the script), and the FSmash's
    angled variants above.
  - `visuals.py` refuses to overwrite a pinned GIF whose hash changed unless rerun with `--refresh`.
- Delivery: local worktree branch `t3/db74d87e`, uncommitted working tree (this record, the importer and visuals
  tool, the package, the source captures and tests).

Contact identity review (2026-11-04): up air's rehit-5 loop and non-rehit finisher touch at source frame 20.
The shared loop-seam rule gives the finisher its own identity: the coverage-following probe changes from
`3 + 3` (suppressed finisher under naive overlap clustering) to `3 + 3 + 5.5`. Other base timelines with
rehit followed by a tie-off were probed: up smash stays `2 + 2 + 2 + 2 + 10`, neutral air stays
`2.5 + 2.5 + 2.5 + 3`. The rule adds an identity at up smash's rehit-7 → rehit-4 seam without changing its
complete damage sequence. No other import's base non-jab/non-recovery timeline declares rehit.

Naming decision (2026-11-04): the display name changed from Verdant to **Bleth**. Package id `verdant`,
directory, tools, pinned sources and `34a`–`34o` screenshot names deliberately remain unchanged.
