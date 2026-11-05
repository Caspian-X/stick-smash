# Gordo Ramsay (SSBU King Dedede, 13.0.1) reference and conversion record

## Target and status

- Package/id: `assets/fighters/king/fighter.json`; package id stays **`king`** so existing selections,
  settings, replay references and CPU rosters keep resolving. In-game display name: **Gordo Ramsay**.
- Reference identity: **King Dedede** from *Super Smash Bros. Ultimate* patch **13.0.1**. Source facts are
  recorded under the reference's own name, `King Dedede`; only the player-facing destination name changes.
- Branch: current local branch; this work is uncommitted in a shared checkout.
- Intended identity: a super-heavy penguin with four midair jumps, the slowest air speed, a huge hammer,
  a bouncing/battable Gordo, armored Super Dedede Jump and a chargeable Jet Hammer. Normal attacks are
  deliberate and committal; recovery is long but predictable.
- State: **authoring and validation complete for the implemented scope**. Remaining work is listed under
  [Known approximations and gaps](#known-approximations-and-gaps); swallow/copy, Gordo wall-stick and true
  source bone transforms are the largest missing pieces.

## Sources

| Source | URL / local path | Patch / commit / hash | Access date | What it establishes |
| --- | --- | --- | --- | --- |
| Frame-data page | https://ultimateframedata.com/king_dedede | page SHA-256 `9a23d46496b779d101240b667758455dc12af13dc0ff017d75d66b6512b84e29`; parsed copy `docs/imports/sources/king/ufd.json` | 2026-11-03 | Startup/active/total, damage variants, landing lag, autocancel windows, charge frames, armor/invulnerability notes, movement stats |
| Numeric attack scripts | https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/King%20Dedede/data.json | repo commit `2e8f0d245481cb0c936524b6ffe1cdea867c4f68` (2021-12-04); source file SHA-256 `a081a4dbc20a4432c670229ce737fca482b217e2417c305bd342e0a6d93fc24a`; retained wrapper `docs/imports/sources/king/scripts.json` SHA-256 `433b0ae9c24d7c69000f9cf8e815ba915acf80677658b1c3bece0c8eb5c6dc5a` | 2026-11-03 | Hitbox IDs, bone names, damage/angle/BKB/KBG/FKB, hitlag, rehit, ground/air masks, facing restrictions, catch volumes, special flags |
| Per-frame hitbox visuals | UFD linked GIFs, e.g. `hitboxes/king_dedede/KingDededeFSmash.gif` | 28 animations pinned by SHA-256 and inspected frame indices in `docs/imports/sources/king/visuals.json` SHA-256 `443d32691e3d33811b9f88292999074ae28daaaa91ee855fa376095f57cf06e4` | 2026-11-03 | Hammer path, weapon/head alignment, active-frame references, grab/special silhouettes |
| Numeric movement stats | UFD Misc Info plus ssbwiki attributes (weight 127, four midair jumps, fall speeds 1.95 / 3.12) | same page/date; wiki https://www.ssbwiki.com/King_Dedede_(SSBU) used only for cross-checking jump count | 2026-11-03 | Mobility inputs |
| Jump heights | ssbwiki "Jump" page, SSBU "Jump height ranking" table, https://www.ssbwiki.com/Jump (unitless game units) | full 32.85, short 16.02, first midair jump 28.50284665 | 2026-11-05 | Jump speeds |

Retained commands and inputs:

```sh
just import-king          # python3 tools/import-dedede/import.py, then just bless
just king-visuals DIR     # downloads hash-checked GIFs and writes per-frame sheets (Pillow)
cargo test -p sim --test king
just shots /tmp/king-shots fighter-kits,training
```

`tools/import-dedede/import.py` writes the fighter package and the normalized ledger
(`docs/imports/sources/king/ledger.json`). It replaces the whole King package; it never edits another
fighter, global tuning, stage geometry or balance reports. `just bless` still runs the repository-wide
formatter/schema check.

### Disagreements

| Property / move | Source A | Source B | Chosen value and reason |
| --- | --- | --- | --- |
| Jab 1 active window | UFD: 10–11 | Script `Attack11`: `frame(10)` + `wait(1)` clears one frame later | UFD 10–11 (engine 9–10) for the playable window; damage/launch from the script |
| Rapid jab timing | UFD: 5/8/11…, loop | Script `Attack100Sub` has no frame header, only a one-frame body loop | Three-tick self-loop phase; damage/launch/rehit/shieldstun from the script |
| FSmash aerial-only 0% boxes | Script frames 40–43 contain 0% aerial-only boxes | UFD lists 16/18.5/25/11 | 0% boxes omitted; the real damage variants remain |
| Down-air sour timing | UFD calls 8% “Late” | Pinned `AttackAirLw` creates meteor, ground and sour boxes together at frame 22, clears at 25 | Script takes precedence: simultaneous coverage on source frames 22–24; meteor boxes have overlap priority |
| Jet Hammer active window | UFD partial: 27–29; full: 10–11 | Script `SpecialLw`/`SpecialLwMax`: attack at animation frames 10 / 9 | 17-frame entry + release frame 10–12; full tier uses the same release animation with a `minCharge` box |
| Super Dedede Jump falling hit | UFD startup 69 | Script `SpecialHiJump` sets the hitbox at animation frame 52 | Fitted leap keeps the source armor windows and falling-hit semantics; exact 69-frame spacing is not forced |
| Inhale capture start | UFD startup 14 / active 17–** | Script `SpecialNStart` CATCH at frame 17 | Capture/windbox start at source frame 17; the 14-frame note is recorded but not claimed as capture timing |
| Gordo damage decay | Script article: 14.0 → 12.5 → 11.0 by article age | Engine cannot combine article stages with `bounce`/`bat` | One 14% Gordo with bounce/bat; age decay documented as a gap |
| Pummel timing | UFD total 21 including 14 hitlag | Engine shares pummel timing globally | Imported damage only (`1.6`); shared timing retained |
| Pivot grab | UFD: 12 / 42, active 12–14 | No pivot-grab destination slot | Standing grab is reused on turnaround; pivot variant omitted |
| Stage body size | Dedede's visual height is about 88–92 px | Arena's lowest platform leaves 76 px of body clearance | Stage body 18×74 (the shared clearance limit); visual/hurtboxes stay larger |

## Conversion rules

| Property | Source units / convention | Destination mapping | Evidence / limitation |
| --- | --- | --- | --- |
| Frames | Source gameplay frames are 1-based; GIF index = source frame − 1 | Phase frame 0 = source frame 1; every parsed script window is shifted by −1 | Boundary tests read the resulting inclusive phase windows |
| Hammer geometry | Bone-local `hammer1`/`hammer2`/`top` offsets, source units | Hand is fitted at 30 px from the shoulder; hammer head centre is 50 px beyond the grip (80 px from the shoulder). Source hammer spheres collapse onto that fitted head path; source radius = `Size × 5` px | **Fitted**; source bone transforms are unavailable. Hitbox tracks use the same attachment angle keys as the drawn hammer |
| Body geometry | `top`/`head`/`hip` offsets | Fixed fitted body points (`utilt` head, `dtilt` low body, dash shoulder/body, nair belly, grabs/inhale front) with source radii × 5 | **Fitted**; not a bone-transform extraction |
| Capsules / priority | Source sphere IDs, sometimes X2/Y2/Z2 capsules | One circle at the fitted head/body per damage/launch group; source ID order preserved for overlap priority | Capsules are approximated by one circle at the head or body point |
| Angles | Source angle codes | 361 → `sakurai` (45°), 366/367/368 → `autolink` (90°), 270 → `spike` (or `bury` for Super Jump), 180°–359° mirrored to negative angles; other angles retained | No fighter/move-ID dispatch |
| Knockback | Source BKB/KBG/FKB | `bkb` = source BKB; `kbg` = source KBG / 100. Fixed-FKB hits use the shared bounded `((1+FKB/2)×1.4+18)×KBG/100+BKB` adaptation and `setKb` | Current Ultimate formula, not Mike's old BKB × 0.1 / KBG × 0.001 |
| Hitlag / shield | Source Hitlag and ShieldDamage | `hitlagMult`; negative ShieldDamage → `shieldDmgMult = max(0,(damage+ShieldDamage)/damage)` | Where source values are non-default |
| Movement | Weight 127; walk 1.029, run 1.496, dash 1.815, air 0.735, air accel 0.05, gravity 0.097, fall 1.95, fast fall 3.12 (units/frame, units/frame²) | One scale, source × 5 (`units.SCALE`): walk 5.145, run 7.48, dash 9.075, air 3.675, air accel 0.25, gravity 0.485, fall 9.75, fast fall 15.6; weight = 100/127. Fitted, converted ×5/3.5 from the old 3.5 px/unit horizontal scale: `runAccel` 1.0286 (was 0.72), `airJumpDrift` 5.4286 (was 3.8) | Traversal tests verify every built-in stage |
| Gait frames | Dedede scripts (1-indexed `frame(Frame=N)` markers): `Dash` DASH_TO_RUN, `TurnDash` TURN_DASH, `TurnRun` / `RunBrakeR` RUN_BRAKE_FLAG_STOP at 11, 3, 17, 11; SmashWiki Ultimate Dash table: 10 dash frames | the frames before each marker (`units.gait`): `dashFrames`, `dashTurnFrames`, `turnFrames`, `brakeFrames` 10, 2, 16, 10 (2026-11-05: all but `dashTurnFrames` were one frame too long; `dashFrames` was hand-entered). A released dash skids by traction (`gait.dashRelease`) | converted |
| Jumps | Heights full 32.85, short 16.02, first midair 28.50284665 units; four midair jumps | `units.jumps` at gravity 0.485: `jumpSpeed` 12.3798, `shortHopSpeed` 8.5725, `airJumpSpeed` 12.0006; `airJumps: 4`. Feel harness measures 164.25 / 80.1 / 142.51 px (targets 164.25 / 80.1 / 142.51) | The sim has one `airJumpSpeed`, so all four midair jumps use the first jump's height; ssbwiki's later jumps decay, so the combined midair height exceeds the source 107.25 |
| Fitted velocities | No source values | Authored speeds in move events and projectiles converted per axis (x ×5/3.5, y ×5/6) to the 5 px/unit scale: Super Jump launch 20.0832 (22 × √(5/6): same height) / fall cap 10, Jet Hammer carry 2.2857, lunges, Gordo/star velocity, gravity, bounce and bat speeds | **Fitted** |
| Hurtboxes | No numeric hurt volumes supplied by the sources | Authored 46 × 92 default and 72 × 20 knockdown rectangles; interaction points separate | **Fitted** from the silhouette; body collision is 18 × 74 |
| Poses / weapon | No native SSBU skeleton | Fitted 2D poses, hammer/coat/crown/beak/belly attachments, and unwrapped hammer angle tracks | **Fitted**; frame captures inspected |
| Grab / ledge size | Source body is much larger than the shared base | `interactions.grab.catchPoint [0,20]`, `holdPoint [58,40]`; ledge uses shared anchors | Shared grab vertical tolerance and all ledge tests pass |

## Move ledger

Source frame numbers are 1-based; engine phase frames are source − 1. “Main” rows list the principal
damage/launch variants; every emitted hitbox is in `fighter.json`. Active windows below are the source
windows; the tests assert the engine-relative equivalents.

| Slot / variant | Source citation | Startup / active / total | Damage / launch / mask / priority | Landing / autocancel / charge | Phases / classification |
| --- | --- | --- | --- | --- | --- |
| Jab 1 / 2 / rapid / finisher | `Attack11`, `Attack12`, `Attack100Sub`, `Attack100End`; UFD Jab rows | 10 / 10–11 / 32; 11 / 11–12 / 27; 5/8/11… loop; 4 / ** / 49 | 2.5° 361 BKB35/25 KBG15, +5 reaction frames; 2.2° 361 BKB35/30/25 KBG15/12; 0.5 rush; 3.0 angle70 BKB80 KBG162 | —; rapid loops every 3 ticks; finisher recovery 49 | `jab1`, `jab2`, `rapidStart`, `rapid`, `finish`; converted launch, fitted body/head points |
| Forward tilt | `AttackS3`; UFD Forward Tilt | 12/16/20/23 / 12–21 + 23 / 48 | 2.0 angle22/100/250 FKB60/50/30/26; final 3.0 angle50 BKB30 KBG190 | — | `main`; multi-hit fitted hammer sweep, source rehit 3–4 |
| Up tilt | `AttackHi3`; UFD Up Tilt | 7 / 7–13 / 38 | 11.5 close / 9.5 far, angle96, BKB65, KBG94 | Head/shoulder/arm intangible frames 7–13 (not modeled as hurt-region windows) | `main`; head-body fitted points |
| Down tilt | `AttackLw3`; UFD Down Tilt | 6 / 6–7, 8–11 / 37 | 10. early / 6. late, angle35, BKB60, KBG75; source Trip 0.4 | — | `main`; fitted low body points |
| Dash attack | `AttackDash`; UFD Dash Attack | 26 / 26–27, 28–41 / 66 | 16. angle361 BKB65 KBG92; 13. angle361 BKB55 KBG92 | — | `main`; fitted forward body points, `setVel` lunge |
| Forward smash | `AttackS4`; UFD Forward Smash | 40 / 40–41, 42–43, 44 / 77 | 16. close; 18.5 handle; 25. head; 11. shockwave; KBG55/85/74/30, BKB85/50/70/100, angles 45/361/60 | Charge hold source frame 33; phase charge frame 32 | `main`; charge, aerial-only 0% boxes omitted |
| Up smash | `AttackHi4`; UFD Up Smash | 17 / 17–24 / 67 | 16. hammer (BKB40, KBG95/96), 10. body | Charge hold frame 6; phase frame 5 | `main`; overhead fitted hammer track |
| Down smash | `AttackLw4`; UFD Down Smash | 14 / 14–22 / 52 | 13. angle30/361 BKB60 KBG80 | Charge hold frame 3; phase frame 2 | `main`; fitted front/rear head circles |
| Neutral air | `AttackAirN`; UFD Neutral Air | 7 / 7–8, 9–29 / 39 | 12. angle60 BKB40 KBG95, away; 7. angle80 BKB40 KBG95, away | Lag 9; autocancel 1–2, 35–39 | `initial`, `active`, `late`; body-centred fitted belly circles |
| Forward air | `AttackAirF`; UFD Forward Air | 13 / 13–15 / 41 | 12. angle361 BKB15 KBG98 | Lag 18; autocancel 1–4, 40–41 | `initial`, `active`, `late`; fitted overhead hammer |
| Back air | `AttackAirB`; UFD Back Air | 17 / 17–19 / 37 | 16. angle361 BKB30 KBG88, back | Lag 13; autocancel 1–4, 33–37 | `initial`, `active`, `late`; fitted rear hammer |
| Up air | `AttackAirHi`; UFD Up Air | 10 / 10–22 multi, 24–25 final / 44 | 1. autolink 367 FKB20/40, rehit 2; 5. angle60 BKB55 KBG167 | Lag 13; autocancel 1–4, 42–44 | `initial`, `active`, `late`; fitted copter path |
| Down air | `AttackAirLw`; UFD Down Air | 22 / 22–24 / 47 | Simultaneous 15. spike/air, 15. ground, 8. sour BKB20, angle 270/361 | Lag 18; autocancel 1–6, 44–47 | `initial`, `active`, `late`; fitted meteor position |
| Grab / dash grab | `Catch`, `CatchDash`; UFD grabs | 8 / 8–10 / 39; 11 / 11–13 / 47 | Grab, no damage; source capsule near/far | — | `main`; two fitted circles per grab |
| Pivot grab | `CatchTurn`; UFD Pivot Grab | 12 / 12–14 / 42 | Grab | — | Omitted: no destination slot |
| Pummel | `CatchAttack`; UFD Pummel | 2 / ** / 21 | 1.6, shared timing | 14 hitlag frames in source; shared engine timing | Stats override only |
| Forward throw | `ThrowF`; UFD | 14 release / 37 | 6. angle45 BKB70 KBG80; 4. hammer tick at 12 | — | `throws.f` |
| Back throw | `ThrowB`; UFD | 19 release / 41 | 9. angle60 BKB60 KBG79; 4. hammer tick at 16; source reverses facing at frame 6 | — | `throws.b` (facing reversal not modeled; throw launch direction handles it) |
| Up throw | `ThrowHi`; UFD | 19 release / 39 | 5. angle90 BKB70 KBG72; 4. hammer tick at 16 | — | `throws.u` |
| Down throw | `ThrowLw`; UFD | 26 release / 41 | 6. angle80 BKB70 KBG82 | Source `FT_CATCH_STOP(8,1)` at 24 not separately modeled | `throws.d` |
| Neutral special — Inhale | `SpecialNStart`, `SpecialNLoop`, `SpecialNSwallowWait`, `SpecialNSpit`; UFD Neutral B | 14/17 start; min total 74; extended endlag 19 | CATCH size 9, SEARCH pull; swallow throw 8., spit 12. | Loop while special held; 19-frame finish | `start`, `inhale`, `finish`; shared grab hold adaptation |
| Side special — Gordo Throw | `SpecialSStart`, `dedede_gordo`; UFD Side B | 29 throw / 58 | Hammer 10. angle361 BKB30 KBG100; Gordo 14.→11., angle60 BKB70 KBG66, shield ×0.5 | Gordo life 5–146 in source; one 140-tick projectile here | `main`; Gordo projectile with gravity, bounce, bat/reflect; wall stick omitted |
| Up special — Super Dedede Jump | `SpecialHiStartL`, `SpecialHiJump`, `SpecialHiLandingL`; UFD Up B | Falling hitbox 69; landing hit 4–5; stars 6–20; total landing 60 (30 when canceled) | Falling 15. bury/air 15. spike; landing 12. angle90/60 BKB105/100; star 5. angle70 BKB8 KBG65 | Invuln 18–21; armor 22–34 and 69–76; landing lag 60/30 | `squat`, `launch`, `rise`, `plungeArmor`, `plunge`, `wave`; fitted leap, cancel → helpless |
| Down special — Jet Hammer | `SpecialLwStart`, `SpecialLw`, `SpecialLwMax`; UFD Down B | Entry 17; partial active 27–29; full active 10–11; total 59/69 | Partial 11.→29.8 angle361 BKB30 KBG100; full 40. angle361 BKB60 KBG46 | 136 charge ticks; armor ≤14% frames 1–14; max tier adds 10 recovery frames | `start`, `charge`, `swing`, `recovery`; `chargeDamage`, `minCharge`, `chargeFrames`, armor |

### Special behavior and shared mechanics

- **Inhale**: the `inhale` phase has a grab circle and a windbox pull (angle 180°, BKB 1.5). Capture enters
  the shared Hold state, so pummel/four throws work. Swallow, copy, swallow-walk/jump and the spit-star
  article are not modeled. Inhale releasing special enters the 19-frame finish.
- **Gordo**: `sspec` starts a hammer hitbox and spawns one `gordo` projectile. It has gravity, three bounces,
  a bat response (any attack can take ownership and reset its cooldown) and wall bounces. Source wall-stick,
  Gordo get and age-scaled damage are not modeled.
- **Super Dedede Jump**: the fitted leap launches at 20.0832 px/tick (22 before the 5 px/unit rescale, × √(5/6) so it rises as high), has the source invulnerability event and
  armor on `launch`/`plungeArmor`, and waits for the natural fall. `shieldPress` during the plunge enters the
  shared helpless state. The separate 30-frame cancel landing lag is not modeled.
- **Jet Hammer**: 17-frame entry, a held `charge` phase capped at 136 ticks, then a 14-frame armored swing and 45–55-frame unarmored recovery. Per-move
  `chargeDamage = (29.8/11−1)/136`; a second hitbox with `minCharge: 136` supplies the 40% full tier.
  Armor threshold 14 applies only on release frames 1–14; the remaining 45–55 frames are unarmored recovery. Ground values are used in the air (32/22 air variants are a gap).
- **Smash charge** uses the shared Ultimate charge formula; source charge-hold frames are mapped to phase
  frames (33/6/3 → 32/5/2). Fixed-FKB multi-hits use the bounded adaptation above.
- Shared input buffering, DI, shield, parry, ledge and movement belong to this game. Pummel timing, pivot
  grab and Gordo wall behavior are shared/omitted as noted, not silently claimed as source-exact.

## Validation and handoff

- **Reference tests**: `cargo test -p sim --test king` (17 tests) checks package identity/display name
  “Gordo Ramsay”, all slots/throws/projectiles, ground and aerial active windows, autocancel landing lag,
  smash charge scaling and launch growth, Jet Hammer partial/full damage and armor, Gordo spawn/bat/bounce,
  Inhale pull/capture, the jab chain/rapid loop/finisher, a capture-to-throw path, Super Dedede Jump
  armor/falling hits/landing stars/cancel, throw facts, and CPU completion with King in either seat on every
  built-in stage.
- **Raw input tests**: jump + attack short-hop aerial, neutral/side/up/down special routing, held special
  and shield-cancel paths; the focused test drives real `Input` through `step_world`, not only direct
  `start_move`.
- **Stage and CPU coverage**: `every_ledge_is_recoverable` and `platforms_are_in_jump_reach` include
  `king` automatically; the focused CPU test runs King/Brawler in both seats on `arena`, `citadel`, `isles`
  and `foundry`. Before the four review fixes, `just balance 4 king` completed 208 matches with 0 unfinished, 47 wins, 365 KOs, 13
  self-KOs and 33 recovery deaths. These diagnostics are evidence about the fighter plus CPU behavior, not
  a balance claim.
- **Content constraints**: `just bless` and `just check` pass; the package remains schema v1 and no shared
  schema was changed. All workspace tests pass, including the preserved TS traces.
- **TS baseline**: production `king` is now an import, so `crates/sim/tests/common/mod.rs` substitutes the
  frozen pre-replacement package `crates/sim/tests/fixtures/king-ts.json` for `linear_data()` only. This
  keeps every King TS trace and the original-roster throw patching intact without pretending the imported
  package is the TS King.
- **Visual inspection**: captured and opened `25a–25p` under `/tmp/king-final-shots` (smash hitbox/head
  alignment, Gordo throw and flight, inhale cone, Super Jump rise/landing stars, meteor pose, Jet Hammer charge,
  grab reach, and training idle/recovery). The fitted hammer track and hitbox overlays agree on the FSmash
  and Gordo captures; the body uses a 74 px stage collider while the visual/hurtboxes are larger.
- **Controller playtest**: **not performed**. No hardware, deadzone or rumble claim is made.
- **Known approximations and gaps**:
  - No SSBU skeleton animation: poses, hammer geometry and body coverage are fitted. Source bone-local
    capsules collapse to one fitted head/body circle per group; hitboxes do not follow an imported skeleton.
  - Inhale swallow/copy/swallow movement/spit-star and projectile swallowing are not implemented.
  - Gordo age-scaled damage, wall stick/get, and the exact Gordo article timeline are not implemented;
    its launch velocity, bounce and bat parameters are fitted/retained rather than extracted from the article motion data.
  - Super Dedede Jump's exact 69-frame falling window, star article behavior and 30-frame cancel lag are
    adapted; cancel enters helpless and the leap speed is fitted. The air Jet Hammer variants use grounded
    values; armor also applies in the air.
  - Pivot grab is omitted (no slot); pummel timing is shared; the 0%-aerial-only FSmash boxes are omitted.
  - Up-tilt head/arm intangibility and down-tilt trip probability are not modeled as hurt-state windows.
  - Dedede's stage body is 18×74 for built-in platform clearance; visual and hurtbox proportions are larger.
- **Final checks after review**: `just check` passes (452 tests, 3 optional diagnostics ignored), as do
  the `fighter-kits,training` tour and a refreshed active-release kit capture. Inspected
  `/tmp/king-review-fixed-shots/25o-king-landing-stars.png` and
  `/tmp/king-review-fixed-kits/25f-king-jet.png`; generator regeneration is byte-for-byte stable.
- **Review fixes**: release armor ends after frame 14 regardless of charge; charge-added duration belongs to
  unarmored recovery. Jet Hammer attachment keys reach the collision position throughout active frames 10–12.
  Down-air meteor/ground/sour coverage is simultaneous on source frames 22–24, following the pinned script.
  Landing stars have mirrored spawn offsets as well as velocities. Dedicated runtime and geometry tests cover
  these boundaries; the refreshed kit capture samples Jet Hammer's active release rather than entry startup.
- **Delivery**: committed locally with the source record, generator, tests and captures workflow; not pushed.

## Dedicated hammer polish (2026-11-03)

Jet Hammer now permits slow grounded movement while carrying partial or full charge (1.6 pixels/tick then; 2.2857 at 5 px/unit),
without changing facing or discarding charge. Airborne drift also retains charge. Releasing special
swings with accumulated power. This uses reusable opt-in `Phase.groundSpeed`; other phases keep
ordinary attack movement. The generator retains the authored speed.

Kit audit used the retained UFD rows, pinned 13.0.1 scripts and move ledger across jab phases,
five grounded attacks, smashes, all aerials, grabs/throws and four specials. Existing boundary tests
check their authored timing, damage and landing semantics. The release GIF frame sheet was inspected
again: its frame 9 contact and subsequent recovery support the fitted hammer swing, but its release
animation must not be interpreted as the entry-plus-charge timeline. Remaining differences listed
above are still adaptations, not newly claimed source-exact behavior. In particular UFD up tilt lists
10/8 damage while the pinned script supplies 11.5/9.5 (script retained); full Jet Hammer shares the
partial release coverage/timeline rather than modeling a separate max animation, and the source air
Jet Hammer damage/ground-only armor remain gaps. No shared movement or knockback tuning changed.

Raw-input regressions cover carrying partial and capped charge in both horizontal directions,
airborne drift, and release retaining charge through the swing.

### Jump ascent tuning

**Superseded (2026-11-05)** by the source-derived speeds in [Conversion rules](#conversion-rules): with every axis
at 5 px/unit the jump speeds come from the ssbwiki heights through `units.jumps` (12.3798 / 8.5725 / 12.0006 at
gravity 0.485). The observation below still stands as feedback against the old ×6 vertical scale.

Tester feedback identified an overly abrupt ascent. Full-hop speed is now 13.5 (was 15.1468),
short-hop 9.4 (was 10.5775), and midair jump 13 (was 15.1468) pixels/tick. These are fitted gameplay
values replacing the height-derived import approximation, not source-exact speeds. Gravity and fall
caps stay unchanged, so apex height is lower too; four midair jumps remain. The original conversion
and source heights above record the import baseline. Raw-input tests cover takeoff and midair reset;
stage reach/recovery tests check the reduced heights. (`raw_jump_heights_match_the_source_and_midair_jumps_are_gentler`
now measures the full / short / first midair rises against source × 5 instead.)

### Jump, landing and carry presentation

The retained material contains attack GIFs plus the Super Dedede Jump and Jet Hammer captures; it has no
ordinary jump, crouch, landing or idle animation, so the locomotion states in this section are **fitted
presentation**, not imported source animation. See [animation passes](../ANIMATION_IMPORTS.md) for the
general rules.

- Takeoff now runs through a half crouch into a deeper load with the arms swept back (`jumpHalf`,
  `jumpLoad`), then a full-body launch extension (`launchReach`) that settles into the existing airborne
  pose. The timing is unchanged: jumpsquat is still the shared 3 ticks and the pose only shows inside it.
- The fall braces the legs and pumps the arms on a 48-tick world loop, and helpless flails on a 40-tick
  loop instead of holding one frozen pose. The retimed jump ascent (above) makes this loop visible for
  longer, which was the main reason to add it.
- Landing absorbs well below any stance (`landAbsorb`), settles through `landSettle` and stands by the
  tenth tick. Hard landings are 3 ticks, so ordinary jump landings show the absorb and start of the
  settle; move landing lags (9–18) play the full recovery. The first key is the readable compression.
- Idle gained a wider weight shift (hips and hammer arm, not just the head) and the run cycle lowers at
  foot contact and lifts through the pass, so the super-heavy read comes from the body, not from speed.
- Jet Hammer carry is a braced stance (`jetBrace` → `jetStrain`) over the ten ticks after entry, plus a
  small strain wobble on the keyed hammer angle (105 → 102 → 108 → 103.5 → 105 through the charge) that
  ends on the entry angle so the release starts from the grip the player saw. The braced pose keeps the
  crouch shoulder height, which is what the keyed-angle `hold` anchor hangs the hammer from, so the drawn
  grip does not slide relative to the fitted swing path. The charge phase still has no hitboxes and the
  swing/recovery attachment keys and fit are untouched.
- Super Dedede Jump's 18-tick `squat` now uses the retained GIF's settle pose: the crouch plants the
  hammer, leans back into `uspecWind`/`uspecWindHold`, and `launch` starts from that same wind-up before
  extending overhead. Neither phase carries hitboxes (the fitted falling hits live in `plungeArmor`/
  `plunge`), so this is presentation only.
- `crates/sim/tests/animation_response.rs` checks the height ordering (load below idle, launch above
  standing, absorb below the load, settle to standing), that the fall loop is live, that the carry keeps
  a fixed grip anchor, that the charge wobble returns to 105°, that the Super Jump wind-up is non-offensive and hands its
  pose to the leap, and that neither package has a named rig
  that could turn these poses into collision. `--test king` still passes; `--test content` loads and
  validates every package and leaves formatting to `just bless`. No gameplay value, timing, hitbox or
  attachment contact changed, and the golden world hash is untouched.

Naming decision (2026-11-04): the display name is **Gordo Ramsay**, formerly Penguin King. Package id
`king`, directory, tools, pinned sources and `25*`/`27*` screenshot names remain unchanged.

Momentum migration (2026-11-04): no phase sets `airAccelMult`, so drift now uses the fighter's airAccel × the `movement.ultimate` kind multipliers; regeneration is unchanged.

## Ultimate calibration (2026-11-05)

- **Traction / air friction**: King Dedede (`dedede`) `ground_brake` 0.085 / `air_brake_x` 0.006 (fighter_param dump
  github.com/j99thoms/smash-charts `data/raw/ultimate_fighter_params.csv`, matching ssbwiki Traction and Air
  friction; high confidence) → `stats.traction` 0.425, `stats.airFriction` 0.03.
- **Full hop initial height**: Ultimate climbs 0.55 × the full hop in its first four airborne frames (jump_initial_y 18.0675 = 0.55 × 32.85);
  the generator emits `stats.jumpInitialHeight` and derives `jumpSpeed` from the remaining height (`units.jumps`),
  so the full hop keeps its height and lands on Ultimate's frame. Short hops and air jumps are unchanged.
- **Fixed knockback**: FKB hits emit `fkb` with the source BKB/KBG (Ultimate WBKB, weight-correct) and keep `setKb`
  as a marker; they were resolved at weight 100 into a `setKb` BKB.
- `just calibrate king`: hitstun and launch speed exact and KO percent exact for every compared smash, tilt,
  aerial (short hop and full hop apex) and throw against a Mario dummy; full / short hop air time 48 / 36 frames
  (UFD 48 / 36, were 53 / 37 counting the landing tick); landing lag and autocancels match UFD. The up smash only
  hits above a standing dummy and the multi-hit up air from the full hop apex has no identified finishing hit.
