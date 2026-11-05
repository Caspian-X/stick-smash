# Duchess reference and conversion record

Playable SSBU **Peach** (patch 13.0.1) adaptation, packaged as **Duchess** (`duchess`). It inherits only the
abstract `base` package and uses two shared data hooks: `stats.float` for the float and the `Counter` phase window
for Toad. Vegetable is a pluck phase followed by a spawned projectile; its
per-pluck random face is replaced by deterministic throw tiers (below).

## Target and status

- Package/id: `assets/fighters/duchess/fighter.json`; display name **Duchess** (`duchess`), `extends: "base"`.
- Branch: `t3/import-duchess-fighter`. Source game: Super Smash Bros. Ultimate, patch **13.0.1**.
- Intended identity: floaty, low-gravity, strong aerial drift and a very low fall speed; a slow ground game
  (walk 0.924, run 1.595), five aerials that work out of a held-jump hover, a slow but strong item projectile
  (Vegetable), a counter (Toad), a hip-first lunge (Peach Bomber) and a parachute recovery (Peach Parasol).
- State: research, authoring, focused tests, CPU/stage coverage, balance diagnostics and window-capture visual
  inspection complete. No physical controller playtest was performed.
- Remaining work: physical feel pass; the tool/face variants of the fsmash, held-item turnip play (catch,
  regrab, Z-drop) (see "Known approximations").

## Sources

| Source | URL / local path | Patch / commit / hash | Access date | Establishes |
| --- | --- | --- | --- | --- |
| Rubendal Peach scripts | https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/Peach/data.json → `sources/duchess/scripts.json` | commit `2e8f0d245481cb0c936524b6ffe1cdea867c4f68`, patch 13.0.1, sha256 `c8e30d3644b0b2d17e49a3ea13c935897b6546572c4c5ad2097e5f641cbdb137` | 2026-11-04 | Attack ids, bones, geometry, damage, angles, BKB/KBG/FKB, hitlag, shield damage, rehit, ground/air masks, specials/motions, turnip/parasol/Toad timings |
| Rubendal item row (Vegetable) | same repository, `public/data/patch/13.0.1/items.json` → `sources/duchess/items.json` (only `peachdaikon`/`daisydaikon` retained) | same commit, full-file sha256 `bf8806de2ce69030f19904ddda022e45f3a1968c835683761e677c076cc2684e` | 2026-11-04 | The thrown turnip's hit: angle 30, KBG 45, BKB 25, size 2.1, hitlag 0.8, `ShieldDamage=-2.5`, `Rehit=100` |
| Ultimate Frame Data: Peach | https://ultimateframedata.com/peach/ → `sources/duchess/ufd.html` (verbatim) and `sources/duchess/ufd.json` (parsed rows) | page sha256 `6833bd893ce5ba4cfab04f0a795379be8f910dab0249243261bf067bece65df3` | 2026-11-04 | Startup/active/total, landing lag, autocancels, charge frames, damage variants, throw rows, movement stats, float note |
| UFD per-move hitbox clips | page links under `hitboxes/peach/<Name>.gif` (lowercase after redirect) → `sources/duchess/visuals.json` | per-file sha256, byte size and frame count in `visuals.json` | 2026-11-04 | Per-frame hitbox coverage and silhouettes for every normal, aerial, smash, grab/throw, the three fsmash weapons, Peach Bomber (hit), Parasol (rise/fall) and Turnip |
| SmashWiki pages: [Peach (SSBU)](https://www.ssbwiki.com/Peach_(SSBU)), [Floating](https://www.ssbwiki.com/Float), [Vegetable](https://www.ssbwiki.com/Vegetable), [Peach (SSBU)/Side special](https://www.ssbwiki.com/Peach_(SSBU)/Side_special), [/Up special](https://www.ssbwiki.com/Peach_(SSBU)/Up_special), [/Neutral special](https://www.ssbwiki.com/Peach_(SSBU)/Neutral_special), [/Forward smash](https://www.ssbwiki.com/Peach_(SSBU)/Forward_smash), [Item throw](https://www.ssbwiki.com/Item_throw) | live pages, not retained verbatim; extracted facts are in this record | revisions as of 2026-11-04 | 2026-11-04 | Float duration/limits/techniques, turnip face chances and base damage, Peach Bomber/Parasol parameters, fsmash weapon rules, item-throw strength table |

Regeneration:

```sh
just import-duchess            # tools/import-duchess/import.py, then just bless
just duchess-visuals /tmp/duchess-reference   # download + hash-check the hitbox clips, write montages
python3 tools/import-duchess/fetch.py         # refresh the pinned sources (network) and re-hash them
```

`import.py` rewrites the whole package and `sources/duchess/ledger.json` (every parsed attack row and script
window per slot, including the fsmash pan/racket rows, the four specials and the turnip/spore article rows).
Hand edits to `fighter.json` are lost on regeneration; authoring rules belong in the importer.

The hashes in the table above are for the **upstream payloads** (stored inside each wrapper). The retained
files' own sha256 values are: `scripts.json`
`ee08cad23e2e35633b17d24ac30896eaef450b714a27c0d33770d0d23bf67327`, `items.json`
`6919b61b1d6e3e93011887a843f8683aae7fecbbaa01e45d9c17ce6591f254d2`, `ufd.json`
`45240999bb128beb90d14e96057c3a2f145bf843198f9e3f5ba0f36a5911bd20`, `visuals.json`
`788a1f4bdcf0af3e1cb9440f9710ac3a36cacb29adb7ab8bb4084bccea2f5d99`, and `ufd.html`
`6833bd893ce5ba4cfab04f0a795379be8f910dab0249243261bf067bece65df3` (the page itself).

The GIF frame counts independently match the UFD totals where a clip covers exactly one move: jab 1 27, jab 2 29,
ftilt 36, utilt 37, dtilt 23, dash attack 37, fsmash 45 (all three weapons), usmash 44, dsmash 54, nair 48,
fair 57, bair 53, uair 45, dair 38, grab 36, dash grab 44, forward/back/up throw 39/49/49, down throw 64,
Peach Parasol 39, Peach Bomber hit 25. That is why the engine totals use those numbers.

### Disagreements and source selection

| Property | Source A | Source B | Chosen value and reason |
| --- | --- | --- | --- |
| Forward throw | UFD "2.0/6.0", total 39 | script `ATTACK_ABS` 8.0 at 45° (KBG 83, BKB 48) plus a 2.0 catch-only hit on frame 14 | Script. The UFD row is stale (it matches neither the absolute throw nor the 1v1 multiplier); the catch hit becomes a `ticks` entry. Same treatment for back 9.0/2.0, up 6.0/2.0, down 7.0/1.0. |
| Forward smash total | UFD 45 | SmashWiki "interruptible 46, animation 59" | UFD total (45); the wiki's 46 is its own interruptibility convention. |
| Parasol landing lag | UFD 26 open / 40 closed | SmashWiki "interruptible 27 / animation 40" (open), 41/64 (closed) | 26 and 40, matching UFD and the in-game FAF convention used by every other imported aerial. |
| Parasol open hit angle | script `SpecialHiOpen` `Angle=80`, `Rehit=18` | SmashWiki lists 65 as a "standard" angle | Script (80). |
| Toad counter window | UFD "Counter window 9-34 from 10-35" | SmashWiki 9-34 | 9-34 (source frames) → phase frames 8-33. |
| Peach Bomber totals | UFD 62 grounded miss / 87 airborne miss; "hit is 25 total frames" | script `SpecialSJump` enables a 25-frame timeout, `SpecialSHitEnd` is a 4-frame script | Grounded whiff 62 (11 windup + 25 thrust + 26 recovery); on contact a 25-frame hit phase; the airborne whiff uses an open `fall` phase (ledge grab from the source's frame 16) instead of a fixed 87 so the engine can keep steering to a ledge. The windup is one tick shorter than the source's 12 so the search box is live on source frames 13-36 (phase entry does not run transition checks). |
| Turnip damage | UFD "8.3—27.6" for the whole move | UFD/SmashWiki base faces 5/10/16/23 and the item-throw formula `d − 0.4 + speed×3` | UFD's low end 8.3 is the common smiley forward throw; a smash throw adds `(3.64 − 2.6) × 3 = 3.1` → 11.4. Special faces, Mr. Saturn and Bob-omb are omitted. |
| Float | SmashWiki: 2.5 s, once per airtime, no vertical movement, drift at air speed, persists through aerials | param `uniq_float_float_frame` 150 | `stats.float { speed: 0, frames: 150 }` (2026-11-05; replaced the fitted 1.0 px/tick `floatSpeed` sink). See "Float". |
| Pivot grab | UFD lists it (startup 10, total 39) | `Moves` has no pivot-grab slot | Omitted; standing/dash grabs and the four throws are imported. |

## Conversion rules

| Property | Source | Destination | Classification |
| --- | --- | --- | --- |
| Frames | 1-based gameplay frames, inclusive windows | Move tick 1 = source frame 1 = phase frame 0, so **phase frame = source frame − 1**; durations use the UFD totals, including the splits for aerial autocancel phases | converted |
| Root geometry | script `top` offsets (`Z` forward, `Y` up) | 5 px per source unit; `X` depth discarded; numeric `Y2`/`Z2` pairs become overlapping circle chains | converted |
| Limb geometry | `havel`, `arml`, `armr`, `legr`, `kneer`, `handr`, `hip` offsets | the hitbox rides the authored pose joint for that bone (`BONE_JOINT` in the importer); the script's small bone offsets are added at the same 5 px/unit | **fitted** (no original skeleton animation) |
| Launch fields | `Angle`, `BKB`, `KBG`, `FKB` | `bkb` unscaled, `kbg = KBG/100`; 361 → `sakurai`, 366-368 → `autolink`, >180 → negative angle (`spike` past −60°); FKB → fixed-launch `setKb` with the shared reference conversion | converted / adapted fixed launch |
| Masks | `FacingRestrict`, `Ground_or_Air` | `dir: away/back`, `situation: ground/air` | converted |
| Hitlag / shield | `Hitlag`, `ShieldDamage` | `hitlagMult`, `shieldDmgMult` | converted |
| Poses | script bones plus the UFD hitbox frames | 22 authored stick-figure poses (slaps, kicks, club swing, rise, spin, crown strike, hip check, counter, bomber lunge, parasol, pluck, toss) and matching attachment angles | **fitted** |
| Movement | UFD walk 0.924, run 1.595, dash 1.826, air 1.029, air accel 0.12, gravity 0.070, fall 1.19 / 1.904 | 5 px/unit on every axis: 4.62 / 7.975 / 9.13 / 5.145 / 0.6; gravity 0.35, fall 5.95 / 9.52; air-jump drift = air speed 5.145 (was a fitted 3.6 ≈ 1.029 × 3.5); fitted run accel 0.6 → 0.8571 (×5/3.5) | converted |
| Jumps | Heights (units) full 30.03, short 14.5, air 30.03: ssbwiki "Jump", SSBU "Jump height ranking" table, https://www.ssbwiki.com/Jump, accessed 2026-11-05 | `units.jumps` at gravity 0.35: jump 10.0776, short hop 6.95, air jump 10.4276 px/tick. Feel harness: full 150.15 / short 72.5 / air 150.15 px (target 150.15 / 72.5 / 150.15); airtimes full 64, short 42, air jump 89 frames vs UFD full/short hop 57 / 41 | converted; the earlier fit to the UFD airtimes (12.0 / 8.6 / 11.6 at gravity 0.42) is superseded |
| Weight | 89 | `100/89` reciprocal knockback multiplier | converted |
| Pummel | 1.3 | `stats.pummelDamage` | converted |
| Specials | script rows/windows | counter window, spore projectiles, search→burst lunge, rising hits + fall-capped open parasol, pluck→toss | converted with the documented engine adaptations |

## Move ledger

`startup/active/total` are source gameplay frames; engine phase frames are one less. Slot totals are the sum of
the authored phases. The parsed rows are in `sources/duchess/ledger.json`.

| Slot | Source | Startup / active / total | Damage / launch / mask | Phases / notes |
| --- | --- | --- | --- | --- |
| Jab 1 | `Attack11` | 2 / 2 / 27 | 2.0, 361 (Sakurai) or 180° (inward), five overlapping root boxes | Own phase; chains to jab 2 from source frame 13 while attack is held |
| Jab 2 | `Attack12` | 2 / 2-3 / 29 | 3.0, 35°, KBG 70, BKB 60, hitlag 1.8, `arml`/`shoulderl` | Own phase (so it can hit a victim jab 1 already hit); fitted hand/shoulder coverage |
| Forward tilt | `AttackS3` | 7 / 7-9, 10-15 / 36 | 7.0 close (leg/knee), 8.0 mid (knee), 6.0 late; 88°/75°/92° | Low kick; the late window is its own phase so it can re-hit |
| Up tilt | `AttackHi3` | 9 / 9-13 / 37 | 8.0 (BKB 65, `dir: away`) and 10.0 (BKB 40) above the head | Root capsule + high box; fitted arms-up pose |
| Down tilt | `AttackLw3` | 8 / 8-9 / 23 | 7.0: 275° ground spike box and 80° airborne box | Both `situation` masks kept |
| Dash attack | `AttackDash` | 6 / 6-9, 17-19 / 37 | 4.0 (fixed KB, ground and air rows) then 6.0 (65°) | Slide phase + push phase (separate contacts) with a forward impulse |
| Forward smash (golf club) | `AttackS4` | 15 / 15-17 / 45, charge 5 | 15.0 ×3, 45°, KBG 65/68, BKB 60/65 | Club track follows the rising diagonal seen in the frames. Frying pan (`AttackS4Hi`, 18.0) and tennis racket (`AttackS4Lw`, 13.5) rows are recorded but not reachable. |
| Up smash | `AttackHi4` | 14 / 14-16, 17-20 / 44, charge 9 | 17.0/15.0 arm tips, 12.0 both sides, then 10.0/12.0 late | Windup/early/late/cooldown phases; head/arm intangibility 14-20 is not modelled (no per-bone hurtboxes) |
| Down smash | `AttackLw4` | 6,10,14,18,22,26 / 30 / 54, charge 2 | 2.0 ×6 fixed-KB low hits (0°/180°), 3.0 final (361°, KBG 180, BKB 40) | Windup + six 4-frame spin phases + final, so every multi-hit can re-hit at ground height |
| Neutral air | `AttackAirN` | 5 / 5-8, 9-19 / 48, land 7, AC 1-4 / 36+ | 13.0 then 6.0 hands + 5.0 hips | Startup/hit1/hit2/late phases; fitted hands/hip coverage, both sides |
| Forward air | `AttackAirF` | 16 / 16-20 / 57, land 13, AC 1-9 / 41+ | 14.0 arm (sour) and 15.0 crown (sweet) | Crown leaves the head and is struck with the hand; its track follows the vertical arc in the frames |
| Back air | `AttackAirB` | 6 / 6-7, 8-13 / 53, land 11, AC 1-4 / 30+ | 12.0 then 7.0 behind (`dir: back`) | Hip box behind the feet |
| Up air | `AttackAirHi` | 10 / 10-13, 15-19 / 45, land 7, AC 1-4 / 41+ | 4.0 autolink first hit (FKB 30/55), 6.0 finisher (76°/70°) | Startup/windup/hit1/hit2/late phases; root capsules above the head, per-row windows |
| Down air | `AttackAirLw` | 12 / 12-13, 18-19, 24-25, 30-31 / 38, land 8, AC 1-11 / 38+ | 2.0 + 1.0, then 5.0 finisher (60°) | Startup + three stomp phases (with gaps) + finish + late, so all four windows re-hit below the feet |
| Grab / dash grab | `Catch` / `CatchDash` | 6-7 / 36; 9-10 / 44 | capsules at 20→43.5 px and 20→52 px | Slide on the dash grab only |
| Pummel | `CatchAttack` | 1 | 1.3 (`stats.pummelDamage`) | Shared hold state |
| Forward throw | `ThrowF` | release 16 / 39 | 8.0, 45°, KBG 0.83, BKB 48; 2.0 pre-hit at frame 14 | Shared throw motion |
| Back throw | `ThrowB` | release 22 / 49 | 9.0, 140°, KBG 0.88, BKB 35; 2.0 pre-hit at frame 20 | |
| Up throw | `ThrowHi` | release 26 / 49 | 6.0, 88°, KBG 0.80, BKB 72; 2.0 pre-hit at frame 19 | |
| Down throw | `ThrowLw` | release 43 / 64 | 7.0, 106°, KBG 0.60, BKB 60; 1.0 pre-hit at frame 34 | |
| Neutral special (Toad) | `SpecialN`/`SpecialNHit`, `peach_kinopiospore/Shot` | counter 9-34; 62 / 60 on hit | spores 3.5 each, 45°, KBG 210, BKB 40, shield +2 | `counter` phase 8-33; six spore births at 3,6,9,12,15,18. The source's 8-10 intangibility is not authored: an intangible victim is skipped before the counter check, so the window itself is the guard. |
| Side special (Peach Bomber) | `SpecialSJump`/`SpecialSHitEnd` | searchbox 13-36; hit 2-3 after contact; 62 ground whiff | 12.0, 361 (Sakurai), KBG 73, BKB 60, ShieldDamage 6 | Windup 12, thrust 24 (ledge grab from source 16), 25-frame burst on contact, 26 recovery; air whiff falls with ledge grabs |
| Up special (Peach Parasol) | `SpecialHiStart`/`Open`/`Fall` | hits 7-9, 11-15, 16-21, 22-27 (rehit 5), 31-32; attack 39 | 3.0 (FKB 160/130), 1.0 rehit 5 (93°, 87° and the frame-22 368 autolink replacements), 4.0 final (hitlag 3); open parasol 2.0 rehit 18 | Five rise phases (one per script window), then open (caps descent at a fitted 1.6667 px/tick, 2.0 at the old ×6 scale, closes on a fresh special press or after 480 frames) and a closed special-fall phase that runs until landing with 40 frames of lag (it still grabs ledges, as in the source); every rise phase lands into 26 frames of lag rather than staying in the move; open landing lag 26 |
| Down special (Vegetable) | `SpecialLw`, `peachdaikon` | pluck 39; toss 22 | Turnip 8.3 (weak tier) or 11.4 (smash tier), angle 30, KBG 0.45, BKB 25, hitlag 0.8 | Pluck (39) → `ready` charge phase → `toss`/`tossStrong` spawns the projectile at frame 6 |

## Float

The source float (`Floating`, SmashWiki <https://www.ssbwiki.com/Floating> and <https://www.ssbwiki.com/Peach_(SSBU)>,
accessed 2026-11-05): holding jump in the air hovers for up to **2.5 s**; the param `uniq_float_float_frame` is **150**
(clone_pack_workbench `owner_params.tsv`, high confidence). It has no vertical movement and full air-speed drift, can
be started once per airtime (touching solid ground restores it; a ledge does not), survives any aerial (the float
continues afterwards; landing inside an aerial takes its landing lag unless autocanceled), is not restored by a hit,
and pressing down and jump together while falling starts it at once (mistimed, it spends the air jump). Neither page
says the air jump is consumed by a normal float; the engine keeps it.

Engine (2026-11-05): `stats.float: { speed: 0, frames: 150 }` (FORMAT.md, "Resource restore, remote gadgets and
float"). Holding jump while not rising starts it (so a full hop with jump held floats at its apex), and so does jump
with down held; it holds the vertical speed at 0 for up to 150 ticks through aerials; releasing jump, the limit, an
air jump, a hit, a ledge or any non-aerial move ends it; only landing restores it. Differences from the source: fast
fall does nothing while floating (release jump first), there is no "hold up and release jump" variant, and the
float aerial short-hop multiplier is not modelled (the engine has none). Tests: `float_holds_height_through_a_nair_
and_ends_on_the_frame_limit` (height held through a nair, exactly 150 ticks, air jump kept) and
`float_is_once_per_airtime_and_ends_on_release` (latch, release, landing restore, down + jump, a hit ends it).

## Vegetable determinism

The source plucks one of eight faces (smiley ≈ 88.7%, winking 6.89%, dot-eye 1.7%, stitch-face 1.7%, Bob-omb 0.4%,
Mr. Saturn 0.6%) and the damage also depends on the item-throw type. This engine has no random projectile
selection hook and replays are checksummed, so **no RNG is consumed**: the pluck always ends in the common smiley
turnip and the throw strength is chosen by how long the special button is held after the pluck (a tap throws the
normal tier; ≥8 held ticks throws the smash tier, which adds the item-throw strength difference). Both tiers are
fixed projectiles (`turnip`, `turnip_strong`), so a pluck/throw in a replay is identical every time. The rare faces,
Mr. Saturn, Bob-omb, holding/regrabbing the turnip and throwing it in other directions are omitted.

## Validation and handoff

- Reference tests (`crates/sim/tests/duchess.rs`, 15): package identity/slots/stats (no zair, no charge pools,
  100/89 weight, float 1.0); every slot's duration against the pinned UFD totals; hit windows after the entry
  shift (jab 1-2, ftilt 6-8/9-14, dtilt 7-8, dsmash 5-6…25-26/29-30, nair 4-7/8-18, fair 15-19, bair 5-6/7-12,
  uair 9-12/14-18, dair 11-12…29-30, usmash 13-15/16-19, fsmash 14-16) and their damage tiers; smash charge
  frames 4/8/1; aerial landing-lag and autocancel boundaries through real phase progression (nair 7/AC 1-4,36+;
  fair 13/1-9,41+; bair 11/1-4,30+; uair 7/1-4,41+; dair 8/1-11,38+); float caps a held-jump descent at 1.0 and
  releases immediately, and does not apply during an attack; Vegetable pluck length, no projectile during the
  pluck, weak/strong tiers (including the strong tier's re-derived shield multiplier) and that neither tier
  consumes the gameplay RNG; Toad's counter window boundaries, the attacker freeze, six spore births across three
  directions, the 62-frame whiff, and projectile stealing/reflection with `reflectLife` 24; Peach Bomber travel,
  the search→12% burst transition, the rebound, and the search box's source 13-36 window (a victim arriving on the
  last thrust frame still detonates, one after it does not); Parasol's five rise windows, the open-descent cap,
  closing on a fresh press, the closed special fall that persists past its 15-frame animation and lands with 40
  frames (26 open); throw damages/angles/pre-hits/releases and pummel 1.3; raw-input routing for
  jab/tilts/specials/grab and up-B in the air; that every independently authored window (dsmash, dash attack,
  ftilt, nair, dair, uair) can hit the same victim twice; fitted contact coverage (jab 40/55, ftilt 55/70, dtilt
  40/55, dsmash ±35/50, bair −55/−70) and nothing at 240 px; source launch typing (Sakurai 361 on bair, autolink
  up air, 275° down-tilt spike with an 80° air box, ground/air dash-attack masks, fixed down-smash KB, spore
  constants).
- Shared-behaviour coverage: `tests/content.rs` roster expectation (17 fighters) and `tests/stages.rs`
  `new_fighter_cpu_matchups` (all four stages, three opponents, four seeds: 48/48 finished).
- `just balance 4 duchess` (4 seeds, every opponent and stage, 256 matches): **0 unfinished, 155 wins, 51
  self-KOs, 65 recovery deaths, 84 161 damage dealt / 76 246 taken**. Multi-hit moves now register their full
  source windows (up air 2516 hits / 209 KOs, down air 3148, up tilt 2361 / 96, dash attack 1570, Parasol 1025,
  ftilt 795, down smash 231, bair 389, jab 233, nair 199, turnip 232 projectile contacts, Peach Bomber 208).
  Forward air reports 104 hits from 1150 starts: its window starts at frame 16 and the shared CPU starts aerials
  too low, the same limitation Throbbin's neutral air records. Self-KOs stay well under the shared 3:1 guard.
- `just check` (fmt, clippy `-D warnings`, whole workspace tests) passes; `just bless` rewrote only the new
  package (no schema or other asset changes). The CPU golden world hash was re-blessed because a new package id
  shifts the resolved `Fighter::data` indices in the debug snapshot (`0xa990a75d4fa27ad4` → `0x7b6281b152cd794f`).
  `SIM_VERSION` is unchanged: no snapshot field or behaviour of existing content changed.
- Multi-part attacks retain presentation phases, with groups distinguishing independent windows and
  `preserveHits` keeping continuous swings and loop cooldowns intact. The independent-window and
  continuous-swing contact tests cover both contracts.
- Visual inspection (window captures, `just shots /tmp/duchess-shots fighter-kits,training`): **35a golf club,
  35b crown knuckle, 35g Toad guard, 35i parasol rise, 35j pluck, 35k turnip toss, 35o float, 35p turnip flight,
  35q parasol descent** were opened and compared with the UFD montages. That pass fixed three presentation bugs
  found only on screen: the crown/hand-crown/parasol were rotated 90° (their shapes are authored upright, so
  their attachment angle is 0), the crown was hidden in the wrong aerial phase (the fair split starts with a
  9-frame autocancel phase), and Toad stood beside her head instead of at her waist. The fsmash club, crown,
  dress and parasol silhouettes now sit where the source frames put them. After the review fixes (phase splits,
  two editor shot frames moved to the new hit phases) three re-capture attempts returned flat black frames
  (window occluded in this session), so those two frame values were verified only through the data/tests.
- The review pass (independent reviewer, then a self-check of its must-fix list) also corrected: multi-part
  attacks split into presentation phases (contact identity is now assigned across those splits),
  Toad's source intangibility removed because it would skip the counter check, the closed parasol kept as an
  endless special fall with its own 40-frame landing lag, jab/up-air windows derived from the ledger rows,
  the parasol rise split to carry the real script rows (including the frame-22 autolink replacement), the search
  box timed to source frames 13-36, and the strong turnip's shield multiplier re-derived at 11.4 damage.
- Delivery: local branch `t3/import-duchess-fighter`, uncommitted working tree (this record, sources, importer,
  visuals tool, package, tests and registry/shots entries).

## Known approximations

- Fitted presentations: 22 stick-figure poses and the dress/crown/parasol/golf-club/Toad attachments are authored
  to match the reference frames; limb-bone hitboxes ride those joints rather than a projected Nintendo skeleton.
- Forward smash: only the un-angled golf club is imported. The source picks the frying pan (up), golf club
  (neutral) or tennis racket (down) by the smash's stick angle; the engine's smash charge cannot branch on the
  stick, so the pan/racket rows stay in the ledger only.
- Vegetable: the move is a single pluck-then-throw timeline (no held item, catch, regrab or Z-drop); faces are
  deterministic tiers and the special faces/items are omitted. Damage tiers follow the UFD range, not the
  per-face base table.
- Float: no cancel into fast fall and no hold-up variant (see "Float").
- Toad: six spores are three fixed-direction projectiles fired twice rather than six randomly deviated sprays; the
  source's back-hit "switch places" is the engine's shared counter turn. The spores have `Rehit=0` in the source;
  the *turnip item* row's `Rehit=100` is dropped because a projectile contact consumes it here.
- Peach Bomber: one lunge speed (the source's tilt/smash inputs differ); the airborne 87-frame miss is an open
  fall phase instead of a fixed total; the 0.95 backward/1.4 upward hit rebound is converted at the movement scale.
- Parasol: reopening the parasol from helplessness is not modelled; the open canopy's 480-frame lifetime becomes a
  phase timeout; closing drops into a closed special-fall phase that runs until she lands (40-frame landing lag),
  so the 15-frame closing animation is not a separate timing.
- Up smash head/arm intangibility (source frames 14-20) is not modelled; hurtboxes are the shared base rectangles.
- Toad's source intangibility (frames 8-10) is not authored because the engine skips intangible victims before the
  counter check; the counter window is the protection instead.
- Pivot grab has no destination slot and is omitted. Autolinks use the engine's shared pull rule.
- No physical controller playtest; movement follows the source stats and jump heights at 5 px/unit, not measured
  hardware input.

## Contact identity correction (2026-11-04)

Inclusive source creation/clear windows that overlap transitively are one hit identity. Regeneration assigns
`group` across the whole timeline before splitting presentation/landing phases, then preserves history on
timeout transitions. Coverage and damage-tier changes cannot hit the same victim again; separated windows
retain independent groups. Full-move contact probes cover the continuous swings and genuine multi-hits.

Parasol's rising phases also use whole-timeline grouping. The rehit-5 loop continues through `rise2` →
`rise3`; a real gap before `rise4` starts a new identity. The coverage-following rising probe retains
`3 + 1 + 1 + 1 + 1 + 1 + 4`, but its loop contacts move from elapsed ticks `10,15,16,22,27` to
`10,15,20,22,27`, preserving the five-tick cooldown through the presentation seam. The final 4% hit still lands.

Momentum migration (2026-11-04): `drift()` in the importer rewrites phase `airAccelMult` for `movement.ultimate` (airAccel × kind multiplier) keeping each phase's effective drift; regenerated with `just import-duchess`.

## Unified scale (2026-11-05)

The importer emits one 5 px/unit scale (`tools/units.py`) for geometry, speeds and accelerations: source
movement, the Peach Bomber lunge (2.9 → 14.5) and its hit rebound (−0.95 / 1.4 → −4.75 / 7.0) are source × 5
(were ×3.5 on both axes). Fitted values convert by factor (x ×5/3.5, y ×5/6): dash attack impulse 5.1429, Bomber
lift 0.4167, Parasol rise 11.4109 (12.5 × √(5/6), same height) and open fall cap 1.6667, float 1.0, run accel 0.8571, turnip `[11.4286,
3.3333]` / strong `[14.2857, 3.5833]` with gravity 0.2667, max fall 7.5, bounce min speed 1.3333, spores
`[8.5714, 5.8333]`, `[14.2857, 1.25]`, `[10.7143, -2.5]`. `drift()` scales the old shared accels by 5/3.5, so
every `airAccelMult` is unchanged. Jump speeds now come from the source jump heights (Jumps row), not the UFD
airtimes: with height and gravity both source-exact the full hop lasts 64 frames (UFD 57) and the short hop 42
(UFD 41). Expected: rising 30.03 units at gravity 0.070 and falling back capped at 1.19 already takes about
63 frames, so UFD's 57 does not follow from the listed height, gravity and fall speed alone. Balance and CPU figures under "Validation" predate
this change.

## Ultimate calibration (2026-11-05)

`just calibrate duchess` compares the engine with Ultimate (`tools/reference_ko.py`) on a Mario dummy at Battlefield
centre; see DESIGN.md "Ultimate calibration". Changes for Duchess:

- **Airtime**: the full hop took 64 frames against UFD's 57 because Ultimate climbs a "jump initial height"
  (`jump_initial_y` 16.5165 = 0.55 × 30.03, ssbwiki Jump) in the first four frames without gravity and only then
  integrates the remaining height. `stats.jumpInitialHeight` 82.5825 px, `jumpSpeed` 6.7034 (from 13.5135 units);
  the full hop now lasts 57 airborne frames, the short hop 41 (UFD 41 / 57), heights unchanged (150.15 / 72.5 px).
- **Traction / air friction**: `ground_brake` 0.111 / `air_brake_x` 0.01 (fighter_param dump
  github.com/j99thoms/smash-charts `data/raw/ultimate_fighter_params.csv`, matching ssbwiki Traction / Air friction;
  high confidence) → `stats.traction` 0.555, `stats.airFriction` 0.05.
- **Fixed knockback**: FKB hits emit `fkb` with the source BKB/KBG (Ultimate WBKB) instead of a BKB resolved at
  weight 100.
- **Float**: `stats.float` (above).

Result: every compared smash, tilt, aerial and throw matches the reference (hitstun and launch speed exact, KO
percent exact); movement, airtimes, landing lag and autocancels match UFD. Multi-hit down air from a short hop picks
a different hit at 150% (harness limitation, not a physics gap).
