# Sneak reference and conversion record

Playable SSBU **Snake** adaptation, packaged as **Sneak**. It is the worked example for the shared
**exploding-gadget** hooks: a bouncing fused grenade, a planted remote C4 (`explosion.manual` + `detonate`),
a steered `article` missile, and the new `releaseToward` / `releaseAway` transition triggers that select a
throw arc from the live stick.

## Target and status

- Package/id: `assets/fighters/sneak/fighter.json`; display name **Sneak** (`sneak`), inheriting only the
  abstract `base` package. Requested name: "Sneak".
- Branch: `t3/3972654f`. Source game: Super Smash Bros. Ultimate, patch **13.0.1**.
- Intended identity: a slow, heavy, mid-range tactician. Worst-in-class ground mobility and a floaty fall;
  almost every special is something he throws and then explodes. The kit cycles between trap setup (grenade,
  C4), remote pressure (Nikita) and a long, armoured recovery (Cypher).
- State: research, authoring, focused tests, stage/CPU coverage and a `just balance` pass are complete.
  Rendering was checked against the UFD frame sheets and a run of Bevy window captures (see the visual
  inspection section); later re-captures in that session came back black because the display was occluded.
  No controller playtest was performed.
- Remaining work: see [Known approximations](#known-approximations--remaining-work).

## Sources

| Source | URL / local path | Patch / commit / hash | Access date | Establishes |
| --- | --- | --- | --- | --- |
| Rubendal Snake scripts | https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/Snake/data.json → `sources/sneak/scripts.json` | commit `2e8f0d245481cb0c936524b6ffe1cdea867c4f68`, patch 13.0.1, full-input sha256 `5751539029aeeb6e9a28a0cfe3aa0688ece1f61d44c34af6d63959a06815cb7f` | 2026-11-04 | Attack ids, bones, geometry, damage, angles, BKB/KBG/FKB, hitlag, rehit, masks, gadget article hitboxes (C4, Cypher, mortar shell, Nikita), throw absolutes |
| Ultimate Frame Data: Snake | https://ultimateframedata.com/snake/ → `sources/sneak/ufd.json` | page sha256 `1c2ee1b62d16d462443cd150f718309b0b8651ef74cd589c27b90d64b34de071` | 2026-11-04 | Startup/active/total frames, damage variants, landing lag, autocancel windows, charge-hold frames, gadget timings, movement stats |
| SmashWiki technical pages (`/Snake_(SSBU)/Neutral_special`, `/Side_special`, `/Up_special`, `/Down_special`, `/Up_smash`, `/Forward_smash`) and the move pages (`Hand_Grenade`, `Remote_Missile`, `C4`, `Cypher`) | live pages, not retained verbatim; the extracted values are in this record | page revisions as of 2026-11-04 | 2026-11-04 | Gadget hitbox tables (damage/BKB/KBG/radius), throw arcs, cooking semantics, Cypher armour/cancel windows, mortar shell parameters |
| UFD hitbox visualisations | `https://ultimateframedata.com/hitboxes/snake/<File>` (39 files) → `sources/sneak/visuals.json` | per-file sha256, size and frame count recorded | 2026-11-04 | Per-frame coverage and silhouettes for every normal, grab, throw, the grenade burst and the gadget hitboxes |

Regeneration:

```sh
just sneak-sources                # re-download and hash-pin scripts.json + ufd.json
just import-sneak                 # tools/import-sneak/import.py, then just bless
just sneak-visuals /tmp/snake-ref # download + hash-check the 39 UFD files, write montages
```

The normal attacks, grabs, throws, the grenade burst and the getup attacks are animated `.gif` files whose
frame count equals the UFD total (e.g. forward smash 73, neutral air 59, forward air 69), which independently
confirms the duration convention below. The grenade, Cypher, C4, mortar and Nikita hitboxes are single-frame
`.png` files. Names contain spaces (e.g. `Nikita Explosion (Strong).png`) and are URL-encoded by `visuals.py`.

### Disagreements and source selection

| Property | Source A | Source B | Chosen value and reason |
| --- | --- | --- | --- |
| Grenade explosion damage | UFD: 8.9–9.9 (collide 2.0–3.1) | SmashWiki's technical hitbox table and the move page: 9.0% both active frames | 9.0. The script dump's `snake_missile` row is 15.0 with a 23-unit radius; that row does not match either gameplay source and is treated as stale/unused data. The collide hit is dropped (see approximations). |
| Grenade explosion size | SmashWiki: radius 12 on the first active frame, 20 on the second | one radius per `explosion` | 80 px (16 units), between the two frames. |
| Up smash charge effect | Script: one `AttackHi4` mortar shot | SmashWiki: charge raises the shell's speed (3.5 → 4.7) and therefore range, not its damage | `chargeScale.speed = 0.00571` for 60 charge ticks, `damage = 0`. |
| Forward smash total | UFD: 73 | SmashWiki: interruptible 74, animation 86; script removes the launcher article at 68 | 73 (UFD). The launcher is drawn for the whole move. |
| Down smash windows | UFD: 8–10 and 20–22 | Script: frames 12–13 and 23–24 with motion-rate 0.5 halves | UFD (gameplay frames). The script's motion-rate note is why the raw animation frames run late. |
| Up tilt damage windows | UFD: 14.5 on 6–8, 13.5 on 9–13 | Script: four successive attack rows at 6, 7, 8, 9-13 | Identical after conversion; the script's per-frame positions are kept for the three 14.5 windows. |
| Up throw damage | UFD: 7.0 / 4.0 | Script: a 7% catch-only box at frame 22 and a 4% absolute throw at frame 23 | Merged into one 11% release hit (the engine's throw model has one release hit); release frame 23 → engine 22. |
| Cypher rise | UFD/SmashWiki: armour 7–94, cancel from frame 47, ride timeout at frame 89 | script: `SpecialAirHiHang` enables cut at 41, timeout at 89 | Cancel from engine frame 46 (source 47), ride 86 ticks from source frame 5; the 3.4 px/tick climb is fitted to the recovery distance, not extracted. |

## Conversion rules

| Property | Source | Destination | Classification |
| --- | --- | --- | --- |
| Frames | 1-based gameplay frames | Move's first tick = source frame 1 = phase frame 0, so a window on source frames *a–b* is phase *a−1 … b−1*; `frames` equals the UFD total | converted |
| Geometry scale | root-space units | 5 px per unit; source Z → forward X, source Y → up | converted (root `top`/`kneel`-free boxes) |
| Limb-bone geometry | `legr`, `legl`, `kneel`, `neck` bone-local offsets | fitted 2D positions from the UFD frame sheets (knee, kicks, body slam, fair spike, dash shoulder) | **fitted** |
| Capsules | numeric `Y2`/`Z2` | chains of circles spaced no farther than their radius (jab 2, dash attack body box, grabs) | converted |
| Launch fields | source BKB/KBG/FKB, angle codes | `bkb` unscaled, `kbg = KBG/100`; 361 → `sakurai`, 366–368 → `autolink`, 269/270 → `spike` (−89/−90°); FKB → fixed-launch `setKb` with the shared `((1 + FKB/2) × 1.4 + 18) × KBG/100 + BKB` fit | converted / adapted (FKB) |
| Masks | `FacingRestrict`, `Ground_or_Air` | `dir` (away/back), `situation` (ground/air) | converted |
| Hitlag / shield | script `Hitlag`, `ShieldDamage` | `hitlagMult`, `shieldDmgMult` (grenade 9%, shield −4.5 → 0.5×) | converted |
| Movement | UFD: weight 106, walk 0.882, run 1.595, dash 1.76, air 0.987, air accel 0.03, gravity 0.080, fall 1.73, fast fall 2.768 | speeds ×3.5 (walk 3.087, run 5.5825, dash 6.16, air 3.455, accel 0.105); gravity/fall ×6 (0.48 / 10.38 / 16.608); jumps fitted to clear the built-in platforms (13.0 / 9.0 / air 12.6) | converted + adapted (jumps) |
| Weight | 106 | `100/106` reciprocal knockback multiplier | converted |
| Pummel | 1.3 | `stats.pummelDamage` | converted |
| Poses | None (no animation files) | Authored stick-figure poses and a small state-clip set; the rest inherit `base` | **fitted** |

## Gadgets

All four gadget identities are content data; no code dispatches on fighter or move id.

**Grenade (neutral special).** The pin pull is a 21-frame phase that cannot throw; it then holds with no frame
limit (source: Snake may cook and walk while holding; elapsed cooking is omitted here). While held the grenade is an attached prop.
The throw is selected on release:

| Input on release | Projectile | Throw animation | Arc (vel, gravity 0.30) |
| --- | --- | --- | --- |
| stick away from facing (`releaseAway`) | `grenadeLob` | 25 frames, spawn on source frame 9 | underhand lob `[1.8, 10.6]`, lands close |
| stick toward facing (`releaseToward`) | `grenadeStrong` | 31 frames, spawn on source frame 10 | flat strong throw `[10.5, 3.0]` |
| neutral (`release`) or a fresh attack press | `grenadeMid` | 27 frames, spawn on source frame 8 | mid arc `[4.6, 5.2]` |
| shield during the hold | `grenadeDrop` | 21-frame drop | zero velocity at the feet |

All four use a fixed 129-tick projectile fuse from spawning. The source fuse runs about 150 frames from
unpinning (2.5 s); held/throw-animation time is not subtracted here, so elapsed cooking is omitted. They bounce (restitution 0.45, friction 0.72, max 3 counted before the fizzle cap, which equals the full fuse, wall restitution 0.5) and detonate on
fuse expiry with a fuse-only `explosion` (`armTicks: 0`, no proximity, radius 80 px, 9% at angle 80, BKB 60,
KBG 0.69, 0.5× shield damage). The `max` field is per projectile id, so the source's two-grenade cap is
modelled as two mid-arc plus one of each other arc (four in play at most).

**C4 (down special).** A charge pool `c4` (one use, no recharge) drives a `variants` timeline: with the use
available the down special plants, spending it; with the pool empty the same slot presses the switch. The
plant is a zero-velocity projectile that falls 16 px and settles on the floor (gravity 0.4, zero-restitution
bounce). Its `explosion` is `manual` with `armTicks: 0` and radius 85 px; its blast hit is 17% at angle 80,
BKB 40, KBG 0.9. The detonate timeline's frame 24 event runs `detonate: ["c4"]` and `restore` in one action set, so
pressing the switch again is possible immediately. A C4 that leaves the stage is removed without a blast and
the next press performs the (empty) switch animation, which restores the pool.

**Nikita (side special).** An `article` on the `sspec` slot: the launcher startup is 41 frames, then the
`guide` phase spawns the missile and keeps the fighter locked in the special (source: defenceless while
guiding). While unreleased the missile travels forward at 2.4 px/tick and its vertical speed follows the live
stick (`steer: 2.6`). Releasing special (or reaching `autoRelease` age 150) freezes the released stage: 100
ticks at 4.4 px/tick with the source's two explosion boxes — 14% (BKB 75, KBG 0.7, radius 9 units → 45 px)
inside a 7% ring (BKB 50, KBG 0.9, radius 13 units → 65 px). Shield cancels control into the 31-frame `away`
animation.

**Cypher (up special).** Startup 3 frames, then an 86-tick rise at 3.4 px/tick with `armorThreshold: 7`
(source: 7% damage-based armour on frames 7–94) and ledge grab from source frame 14. Attack or shield from
source frame 47, or the timeout, enters the 12-frame cut; the cut spawns the detached Cypher as an upward
6% projectile (angle 42, BKB 63, KBG 0.17) and ends helpless. Landing during the rise is 20 frames of lag.

**Mortar (up smash).** The launcher hit is 4% fixed-knockback (angle 96, FKB 120, ground only) on source
frames 11–12. The shell spawns on source frame 35 as a ballistic projectile (spawn 34/66 px, velocity
`[2.4, 11.5]`, gravity 0.34) whose first floor contact burns a 3-tick fuse (`bounce.maxBounces: 0`,
`fizzleLife: 3`) and then blasts for 14% at angle 65, BKB 87, KBG 0.6, radius 60 px. Charge raises the shell's
speed only (`chargeScale.speed`), matching the source.

## Move ledger

`startup/active/total` are UFD gameplay frames (inclusive); engine phase frames are one less. Every row is
authored by `tools/import-sneak/import.py`; the parsed script rows it used are in `sources/sneak/ledger.json`.

| Slot / variant | Source | Startup / active / total | Damage / launch / mask | Phases / classification |
| --- | --- | --- | --- | --- |
| Jab 1 | Attack11 | 3 / 3–4 / 15 | 2.5, 361→Sakurai, BKB 25/20/20, three root circles (Z 5.5/7.5/11.5, Y 10.4–10.8) | one phase; continues from source frame 6 while attack is held |
| Jab 2 | Attack12 | 4 / 4–5 / 24 | 2.5, 361, BKB 28/25/25, three root circles (Z 4/6.5/10.5) | one phase; continues from source frame 7 |
| Jab 3 | Attack13 | 8 / 8–9 / 46 | 6.0, 361, KBG 58, BKB 70, circles Z 4/7.8/13 | one phase, knee pose |
| Forward tilt 1 | AttackS3 | 4 / 4–5 / 31 | 4.0, 78° ground / 65° air, KBG 10, BKB 42/20 | fitted knee, 22 px radius; continues from source frame 10 |
| Forward tilt 2 | AttackS3S2 | 8 / 8 / 39 | 9.0 close (BKB 85), 11.0 far ground, 10.0 far air, all 361 | converted root circles; **converted** |
| Up tilt | AttackHi3 | 6 / 6–8, 9–13 / 37 | 14.5 (95°, KBG 82, BKB 65) then 13.5 (95°, KBG 88, BKB 50) | one phase, three converted root circles per window plus fitted knee/leg leg boxes; rising axe-kick pose |
| Down tilt | AttackLw3 | 6 / 6–7 / 30 | 12.0, angles 80/75/70, KBG 62, BKB 60, `dir: away`, low attack height | two converted circle sets (the source slides the three circles forward on the second frame) |
| Dash attack | AttackDash | 5 / 5–8, 9–12 / 41 | 11.0 then 8.0 (neck, 45°/110°, KBG 45/75, BKB 95/40); 9.0 then 6.0 root box | one phase, forward impulse 4.6 and slide 12; fitted shoulder box. Arm intangibility 5–12 is not modelled (no hurt regions) |
| Forward smash | AttackS4 | 41 / 41–43 / 73, charge hold 38 | 22.0, 361, KBG 75, BKB 66; 60 px ground blast circle and 35 px air circle at Z 16 | one phase; RPG-7 prop; charge frame 37 |
| Up smash | AttackHi4 | 11 / 11–12 / 54, charge hold 24 | launcher 4.0, 96°, FKB 120, ground only; shell 14.0, 65°, BKB 87, KBG 0.6 | one phase; launcher circles at Z 1/7 and the shell spawned on frame 35 |
| Down smash | AttackLw4 | 8 / 8–10, 20–22 / 44, charge hold 5 | 12.0 then 14.0, angles 25°/20°, KBG 87/88, BKB 30, `dir: away` | one phase; fitted front then back low kicks (20 px / 16.5 px) |
| Neutral air | AttackAirN | 10 / 10–11, 18–19, 26–27, 36–38 / 59, land 16, AC 1–3 / 50+ | 3.0 autolink (FKV 30 airborne, 50 ground; radius 4, capsule Z 4→11) then 12.0 Sakurai (KBG 90, BKB 25) | three phases at the autocancel boundaries; every multi window keeps both masks |
| Forward air | AttackAirF | 23 / 23–26 / 69, land 19, AC 1–3 / 63+ | 15.0 spike (269°→−89°) fitted forward-low, 14.0 at 80° fitted close, both KBG 90, BKB 35 | three phases; spike box listed first so the meteor wins overlap |
| Back air | AttackAirB | 7 / 7–9, 10–26 / 42, land 19, AC 40+ | 16.0 far / 14.0 close (361, KBG 72, BKB 43/42), then 10.0 / 9.0 (KBG 85, BKB 25), `dir: back` | two phases; fitted body-slam heights with the source's Z offsets |
| Up air | AttackAirHi | 10 / 10–12, 13–23 / 47, land 15, AC 1–4 / 36+ | 14.0 (80°, KBG 72, BKB 65) then 10.0 (70°, KBG 95, BKB 10), three circles above the head | three phases; converted Y 15/18/21 |
| Down air | AttackAirLw | 3 / 3–4, 10–11, 17–18, 25–26 / 59, land 20, AC 54+ | 4.0 (ground spike 266°, air 80° FKV 35, both 70° FKV 25) then 3.0, then 10.0 Sakurai (KBG 80, BKB 50) | two phases; converted boxes below the feet, headstand pose |
| Grab / dash grab | Catch / CatchDash | 8–9 / 34; 11–12 / 42 | grab capsules at 35→57.5 px (ground 20 px) and 25→67.5 px (air 10 px) | one phase each; dash grab slides to source frame 20 |
| Pummel | CatchAttack | 1 | 1.3 | shared hold state |
| Forward throw | ThrowF | release 21 / 39 | 9.0, 40°, KBG 65, BKB 60 | shared throw motion |
| Back throw | ThrowB | release 20 / 30 | 9.0, 140°, KBG 65, BKB 65 | shared |
| Up throw | ThrowHi | release 23 / 47 | 11.0 (7 + 4 merged), 90°, KBG 130, BKB 55 | shared |
| Down throw | ThrowLw | release 35 / 41 | 9.0, source 270° with no knockback; here −90° with BKB 25, KBG 0.35 as a knockdown approximation | shared |
| Neutral special | SpecialNStart / ThrowHi/M/Lw | pull 21, throws 27/31/25 | see gadgets | three throw phases + hold + shield drop |
| Side special | SpecialSStart / Hit / Away | 41 / launch / 36, away 31 | 14.0 direct, 7.0 ring | startup, guide, fire, away |
| Up special | SpecialHiStart / AirHiHang / AirHiCut | spawn 4, ride 86, cut 12, armour 7–94, cancel 47 | 6.0 Cypher shot (42°, BKB 63, KBG 0.17) | startup, rise, cut |
| Down special | SpecialLwStart / Blast | plant 24; detonate 32 with the blast on 25–27 | 17.0, 80°, BKB 40, KBG 0.9, radius 85 | plant (pool variant) / detonate (default) |

Omissions: the pivot grab (no destination slot; the shared grab covers it), the ledge/floor attacks (shared
ledge options), Snake's item and Final Smash animations, and the smoke/ear-cover flourish.

## Validation and handoff

`crates/sim/tests/sneak.rs` (13 tests):

- Reference windows and damage: jab/forward-tilt phase frames, up-tilt 14.5 → 13.5 windows, down-tilt pairs,
  down-smash front/back windows and directions, forward-smash 22% on phase 40–42 with the charge hold on 37,
  up-smash fixed-knockback launcher and shell spawn, every aerial's active windows and damage tiers, the
  landing lag on the main aerial phase only, and every move's total against the pinned UFD rows.
- Gadgets: the 21-frame pin pull and hold; `releaseAway` / `releaseToward` / neutral selecting the three
  throw arcs and spawning the right projectile; shield-drop at the feet; bounce (a floor contact flips the
  grenade and it arcs above the floor) and the 9% fuse blast with no owner damage; C4 plant (pool spent,
  projectile settled on the floor), remote detonation for 17% with no self-damage, pool restore and a second
  plant; Nikita steering up and down while held, release into the 14%/7% flight, and a landed hit; the mortar
  shell arc, floor contact and 14% blast; Cypher climb (>230 px), armour preserving the ride against a 5%
  hit, and the detached 6% Cypher projectile.
- Raw input: neutral/side/down/up special routing, ground attack → jab, jump + attack → forward air.
- Identity: package fields, all 19 slots, the `c4` pool, the projectile definitions' gadget semantics, the
  throw release/damage ledger, and pummel damage.

Screenshot tour: `crates/game/src/shots.rs::fighter_kits` adds `32a`–`32o` editor captures (jab, knee, body
blow, rising kick, RPG, mortar, double kick, four aerials, grenade pull, Nikita, Cypher, C4 plant/detonate)
plus the live training reads `32p`–`32s` (grenade flight, Nikita flight, Cypher rise, C4 blast). The
expected-name list in `shots::tests::preserves_screenshot_names` is updated in the same file.

Stage and CPU coverage: `tests/content.rs` roster expectation, `tests/stages.rs` `new_fighter_cpu_matchups`
(all four stages, three opponents, four seeds: 48/48 finished, 23 wins, 139 KOs, 11 self-KOs).

`just balance 4 sneak` (4 seeds, every opponent and stage, 256 matches): **0 unfinished**, 100 wins,
66.1k damage dealt / 90.7k taken, 406 deaths, 49 self-KOs (12%), 121 recovery deaths. Per-move starts/hits show the
whole kit used: up air 687 hits / 172 KOs, up tilt 370 / 105, forward tilt 1264, dash attack 834, down air
2444 (multi), down tilt 161, neutral air 68, forward air 38, back air 246, down smash 28, forward smash 50 /
9 KOs, up smash 46 launcher hits, Nikita 189 / 6 KOs, C4 16 / 7 KOs. Grenade hits land while Snake is already
back in `idle`/`shield` (the balance tool attributes a delayed projectile to the attacker's current state),
which is why the neutral special itself reports 348 whiffs.

Review: delegation to the requested `codex` / `gpt-6.1-sol` (high reasoning) reviewer was attempted four times;
three calls failed with transport errors and the one run that started stopped at the provider's usage limit, so
**no independent review was obtained**. The author's own review pass re-read the engine diff (both new triggers
are gated on every step-transition evaluation; buffered follow-up commands cannot take them), confirmed the
generator is byte-for-byte reproducible, that no `World`/projectile/article/charge source changed, that the
package contains no fighter-id dispatch, and that every phase has an exit (the only unbounded phase, the
grenade hold, has release/toward/away/attack/shield transitions).

New engine semantics (shared, data-driven): `Trigger::ReleaseToward` and `Trigger::ReleaseAway` — a `release`
that additionally requires the live movement stick to point toward or away from the fighter's facing, so a
held special can pick a throw arc. They are checked in the same first-match transition list as `release`;
buffered follow-up commands ignore them. The schema is regenerated and no `World` field changed, so
`SIM_VERSION` is **not** bumped and the golden world hash is unchanged (Sneak sorts after `ninja`, so no
existing fighter's index moves).

Commands run in this session:

```sh
cargo test -p sim --test sneak          # 13 passed
just bless                              # schema (+releaseToward/releaseAway) and canonical assets
just check                              # fmt-check, clippy -D warnings, workspace tests: green
just shots /tmp/sneak-shots fighter-kits # 19 sneak captures, inspected individually
just balance 4 sneak                    # see above
cargo test -p sim --test stages new_fighter_cpu_matchups -- --nocapture
```

Visual inspection. The UFD frame sheets were opened for jab 1, both forward tilts, up tilt, down tilt, dash
attack, forward smash, up smash, down smash, all five aerials both grabs, all four throws, the grenade burst,
the Cypher, and the C4/Nikita/mortar hitbox images. Bevy window captures were used for the delivered shots
(`just shots /tmp/sneak-shots fighter-kits`, 19 sneak captures, each 2560×1440 with thousands of colours — not
black). Inspected one by one: `32a` jab (three spheres at chest height meeting the punching hand), `32b` knee
(fitted circle on the raised knee), `32c` body blow (9/11/10 spheres ahead of the extended arm; previewed by
trimming the knee phase), `32d` rising kick, `32e` RPG (60/35 px blast ahead of the kneeling figure, launcher
prop at the shoulder), `32f` mortar (crouched launcher and the shell visible above and ahead), `32g` double
kick (front pair active, rear pair dashed/inactive), `32h` air kick, `32i` knee drop, `32j` drill (headstand
above the spheres), `32k` grenade pull (grenade in hand, bandana and pack visible), `32n` C4 plant (packet in
the planting hand, variant picker shows the pool requirement), `32o` C4 detonate (switch-press pose), and the
live training reads `32p` (grenade in flight after a held throw), `32q` (Nikita in flight at `guide` frame 12),
`32r` (Cypher ride with the drone above the head) and `32s` (C4 blast particles around the switch press). The
same pass also re-checked a stick-figure/hitbox proxy before the window captures worked; no alignment errors
were found beyond the fitted values recorded above. The live gadget shots are driven by predicates on the
projectile/move state, so they do not depend on fixed frame counts. Two later re-runs of the same section came
back entirely black (the game window lost visibility in this session); the inspected set is the run recorded
above, taken from the same code.

Controller playtest: not performed.

## Known approximations / remaining work

- **Grenade.** The throw arcs are data-selected from the stick at release, but the arcs' exact magnitudes are
  fitted to this game's stage scale (source distances are expressed in Training-stage units). Grenades have a
  fuse only: the source's 2.0–3.1% no-flinch contact is dropped because `explosion` suppresses contact hits,
  they do not detonate on contact, and they cannot be picked up, thrown back or batted. The source caps two
  grenades in play; the engine's per-projectile-id `max` gives two mid-arc and one of each other arc (four at
  most). **Elapsed cooking is omitted:** the fuse is fixed at 129 projectile ticks from spawning, regardless
  of the time held or spent in the throw animation. Bounces preserve that remaining fuse.
- **C4.** Only the ground plant exists: no sticking to walls or opponents, no transfer, no auto-explosion after
  1600 frames (a `manual` explosion freezes the fuse by definition), and the blast does not damage Snake.
  A C4 that falls out of bounds is removed without a blast.
- **Nikita.** The missile is steerable only while it is held (construction), which is faithful in input terms
  (Snake is locked in the special while guiding) but its post-release flight is straight; shield-cancel makes
  it fly on rather than drop, and a direct hit and a glancing hit differ only by which of the two rings was
  touched. It is indestructible (no 25% damage body) and has no fuel-dependent slowdown.
- **Cypher.** The ride is a constant climb with the source's armour window collapsed to the whole rise; the
  Cypher body itself is not destructible (13 HP in Ultimate) and its 6% detach hit is a single upward
  projectile rather than a thrown article.
- **Mortar.** The shell's charge scaling uses `chargeScale.speed`; the source's 1° launch angle, turning
  acceleration and 300-frame detector are not modelled, and the shell explodes 3 ticks after its first floor
  contact rather than after a fixed detonation window.
- **Hurtboxes and body.** The base fighter's rectangles are used; Snake's per-bone volumes, the dash attack's
  arm intangibility (frames 5–12) and up-tilt/other intangibility notes are not authored as hurt regions.
- **Presentation.** Poses, the bandana/pack, the launcher/Cypher/grenade/C4 props and the state clips are
  authored stick-figure work, not extracted animation. Most state clips (roll, ledge, tech, getup, dazed…)
  inherit `base`.
- **Down throw** is a small downward launch rather than the source's "lay" knockdown, because the throw model
  has no lay state.

## Delivery

Local branch `t3/3972654f`, uncommitted working tree (this record, the importer/fetcher/visuals tools, the
package, the two transition triggers, tests, registries and screenshots). Not pushed. The registry files
(`tests/content.rs`, `tests/stages.rs`, `shots.rs`, `justfile`) are expected to conflict with sibling imports;
the fighter and its records conflict with none.
