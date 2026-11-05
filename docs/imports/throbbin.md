# Throbbin reference and conversion record

Playable SSBU **Robin** adaptation, packaged as **Throbbin**. It is the first import that uses the shared
**charge resources** (limited uses with timed recharge), **move variants** chosen from those resources, a
**charge-tier transition gate**, and a **healing unblockable hit**.

## Target and status

- Package/id: `assets/fighters/throbbin/fighter.json`; display name **Throbbin** (`throbbin`), inheriting only
  the abstract `base` package.
- Branch: `char-tune`. Source game: Super Smash Bros. Ultimate, patch **13.0.1**. Requested name: "Throbbin";
  identity requested "as close to the real thing as possible".
- Intended identity: a slow, mid-weight tactician. Ground mobility is among the worst in the source game, but
  air acceleration is above average. Sword normals are short and weak; smash attacks and all five aerials swap
  to the stronger, longer, electric **Levin Sword** while it has uses. Four spells (Thunder charge tiers,
  Arcfire, Elwind, Nosferatu) draw from per-tome use pools that must recharge after depletion, so the kit
  cycles between pressure and downtime.
- State: research, authoring, engine support, focused tests and CPU/stage coverage complete. Visual
  inspection used the live editor preview frame by frame (plus a headless CPU-canvas render in the earlier
  session); every move and article has been checked against the pinned references. No controller playtest was
  performed.
- Remaining work: physical controller feel pass; discarded-weapon items (tomes/sword can be thrown in the
  source), Levin "sword already out" neutral-aerial rule, Nosferatu's held drain and
  damage-comparison healing bonus, gradual in-match durability UI, and Arcthunder's vortex multi-hit.

## Sources

| Source | URL / local path | Patch / commit / hash | Access date | Establishes |
| --- | --- | --- | --- | --- |
| Ultimate Frame Data: Robin | https://ultimateframedata.com/robin/ → `sources/throbbin/ufd.json` | page sha256 `670c4b5b807f11e82c3552f1bc20ffb9ee2efdf65a5384620766fac2dfe6e6ae` | 2026-11-03 | Startup/active/total, damage variants, landing lag, autocancels, charge frames, movement stats, tome/Levin use counts |
| Rubendal Robin scripts | https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/Robin/data.json → `sources/throbbin/scripts.json` | commit `2e8f0d245481cb0c936524b6ffe1cdea867c4f68`, patch 13.0.1, full-input sha256 `df517d5060620334565a0fbf562e54e94991b1b1666107ae65913877706fc38b` | 2026-11-03 | Attack ids, bones, geometry, masks, damage, angles, BKB/KBG/FKB, hitlag, rehit, shield damage, Levin/Bronze conditional blocks, article hit data |
| SmashWiki Robin move pages (`/Neutral_attack`, all aerials/smashes, `/Side_special`, `/Up_special`, `/Down_special`, `/Neutral_special`, `Robin_(SSBU)` durability section) | live pages, not retained verbatim; extracted rows are in `ufd.json` and this record | page revisions as of 2026-11-03 | 2026-11-03 | Gameplay hit windows per variant, timing charts, projectiles, grab/drain data, durability costs/recharge times, heal behavior |
| UFD per-move hitbox GIFs | `https://ultimateframedata.com/hitboxes/robin/<Name>.gif` (37 files) → `sources/throbbin/visuals.json` | per-file sha256 and frame counts recorded; acquired 2026-11-04 | 2026-11-04 | Per-frame hitbox coverage and silhouettes for every normal, the four sword moves in both variants, the three smashes and all five aerials in both variants, grabs/throws and the spells |
| UFD character page | https://ultimateframedata.com/robin/ | page sha256 in `ufd.json` | 2026-11-03 | Timing, damage tiers, totals, autocancels, movement stats |

Regeneration:

```sh
just import-robin                   # tools/import-robin/import.py, then just bless
just robin-visuals /tmp/robin-ref   # download + hash-check the hitbox GIFs, write frame montages
```

The GIFs live under `hitboxes/robin/` (lowercase); the character page only links the generic getup/ledge/trip
visuals, and resolving its relative links returns HTML. `visuals.py` keeps the moved frames as montages and
records each file's sha256, size and frame count. GIF frame 0 is the move's first frame. Where a clip covers
exactly one move its frame count equals the UFD total (forward smash 57, neutral air 47, Elwind 85, Nosferatu
49), which independently confirms the duration convention below; several clips do not (a spell clip can run
past the move into the projectile, e.g. Arcfire 77 frames against a 63-frame move), so the count is only used
as a duration cross-check for the clips that line up.

The importer rewrites the whole package and `sources/throbbin/ledger.json` (raw parsed attacks per slot with
their Levin/Bronze block tag). Hand edits to `fighter.json` are lost on regeneration; authoring rules belong in
the importer.

### Disagreements and source selection

| Property | Source A | Source B | Chosen value and reason |
| --- | --- | --- | --- |
| Tome pool sizes | UFD panel: Fire 12, Arcfire 2, jab finisher 1, Wind 10, Elwind 1–2, rapid jab 2 minimum | SmashWiki durability table: Thunder 20 (1/3/5/8), Fire 6 (Arcfire 1, jab finisher 0.5), Wind 10 (1 per blade), Dark 3 | SmashWiki. It is the per-game table with costs; UFD's panel is a summary that mixes Smash 4 values. Fractional costs are rounded up to whole uses (jab finisher 1). |
| Levin hit type | scripts: `Angle=361` (Sakurai) **and** `Effect=collision_attr_elec` | engine has one hit type per hitbox | Angle behavior wins where the source angle is 361/367 (forward smash, back air, autolinks) because it drives launches and combos; other Levin hits (30–92°, spikes) use `electric` for its hitlag/spark behavior. Recorded, not silent. |
| Forward smash windows | UFD "16–18 (19–27)" | script sets the Bronze box over a `frame(0,38)` range with a sword-flag conditional | UFD/wiki gameplay windows: Levin clean 16–18, late 19–27, Bronze 16–18. Script ranges are animation-frame bookkeeping, not hit windows. |
| Neutral aerial | UFD "7–10/20–23 (24–28)" | scripts list Levin 11.5 on 9–13 and 22–26 plus a 5.0 late box | Levin 11.5 on 7–10 and 20–23, late 5.0 on 24–28, Bronze 6.9 on 7–10 and 20–23 (SmashWiki timing; the script start frames run 2 late because of a 0.9 motion rate). The hitbox *positions* come from the pinned GIF frames, which put the first swing in front and the second behind. |
| Arcthunder | script: 6.0 initial, 2.4 rehit-4 vortex, 8.0 final | engine projectiles have one contact hit plus an optional blast | The projectile keeps the 8.0 final hit as its blast (proximity-triggered) and drops the 2.4 vortex ticks and the 6.0 initial hit. The vortex needs per-tick rehit growth the projectile model does not express. |
| Empty tome behavior | UFD: "empty tome Thunder has 39 frames", "empty tome Elwind has 51" | SmashWiki: "performs the animation but produces no hitboxes" | Empty variants are animation-only with those durations. No weak fallback spell. |
| Elwind damage tiers | script: early 7.0 / late 5.0 per blade | projectile model has one hit value | 7.0 imported; the 5.0 late tier is omitted (documented limitation of projectile contact hits). |

## Conversion rules

| Property | Source | Destination | Classification |
| --- | --- | --- | --- |
| Frames | 1-based gameplay frames; scripts carry motion rates | Move's first tick = source frame 1 = phase frame 0, so **hit windows shift by one: phase frame = source frame − 1**. Durations are the number of frames in the move, so `frames` equals the source total directly (a 57-frame forward smash runs 57 ticks, interruptible one tick after it ends) | converted |
| Geometry scale | root-space units | 5 px per source unit, source Z → forward X, source Y → up | converted (root `top` boxes) |
| Weapon geometry | `sword`/`armr` bone offsets | Fitted 2-D blade tracks per slot, authored with the attachment angles that pose the arm; not a projection of Nintendo's skeleton | **fitted** |
| Capsules | numeric `X2`/`Y2`/`Z2` | chains of circles spaced no farther than their radius (jab, tilts, up smash, down smash sparks, grab) | converted |
| Launch fields | source BKB/KBG/FKB, angle codes | `bkb` unscaled, `kbg = KBG/100`; 361 → `sakurai`, 366–368 → `autolink`, >180 → negative angle (`spike` past −60°); FKB → fixed-launch `setKb` | converted |
| Masks | `FacingRestrict`, `Ground_or_Air` | `dir` (away/back), `situation` (ground/air) | converted |
| Hitlag / shield | script `Hitlag`, `ShieldDamage` | `hitlagMult`, `shieldDmgMult` | converted |
| Poses | SmashWiki/script swing descriptions | Authored stick-figure poses (`thrust`, `sweepLow`, `sweepUp`, `overhead`, `cast`, `castHigh`, `drain`, air poses) with per-phase attachment angle keys and `hold` so the front arm grips the blade | **fitted** |
| Movement | UFD: walk 0.936, run 1.265, dash 1.815, air 1.05, air accel 0.08, gravity 0.089, fall 1.5, fast fall 2.4 | ×3.5 for speeds (walk 3.276, run 4.4275, dash 6.3525, air 3.675, accel 0.28); gravity/fall ×6 (0.534 / 9.0 / 14.4); jump speeds fitted so a full hop clears the built-in platforms (13.2 / 9.4 / air jump 13.0) | converted + adapted (jumps) |
| Weight | 95 | `100/95` reciprocal knockback multiplier | converted |
| Pummel | 1.5 | `stats.pummelDamage` | converted |

## Durability resources (new shared semantics)

Content now declares pools on a fighter (`charges`) and moves refer to them. Nothing dispatches on fighter id.

| Pool | Uses | Recharge | Spent by |
| --- | --- | --- | --- |
| `levin` | 8 | 600 ticks (10 s) | every smash and aerial (Levin `variants[].phases[0].cost`) |
| `thunder` | 20 | 720 ticks (12 s) | Thunder 1, Elthunder 3, Arcthunder 5, Thoron 8 |
| `fire` | 6 | 720 ticks (12 s) | Arcfire 1, jab 3 (Fire tome) 1 |
| `wind` | 10 | 360 ticks (6 s) | Elwind 1 per blade, rapid jab 1 per activation |
| `dark` | 3 | 1800 ticks (30 s) | Nosferatu 1 |

- A pool with fewer uses than a move asks for still casts and is emptied ("the tome is used up"), matching
  SmashWiki. An emptied pool starts its recharge and restores **one use every `ceil(recharge / uses)` ticks**
  (integer schedule, so no curve drift), with `discardDelay` before the first one; `recharge: 0` stays spent
  until respawn. Spending during a recharge never refunds a use — the countdown keeps running.
- `Transition.needs` gates a transition on a pool (jab 2 only chains into the Fire burst while the Fire tome has
  a use; the fallback transition follows it in the list). `Move.variants` covers whole-timeline choices: one
  Elwind variant needs two Wind uses, the next needs one (single blade), and the default timeline is the
  empty-tome animation.
- Respawning refills every pool (source behavior).
- Depletion and refill play a sound (`emptySfx` / `refillSfx`) but do not spawn the discarded tome/sword item,
  which this engine cannot pick up or throw.
- The HUD has no durability meter yet; the engine exposes the pools in the snapshot for one.

## Move ledger

`startup/active/total` are source gameplay frames (inclusive) as pinned in `ufd.json`; engine phases are one
less. Every row is authored by `tools/import-robin/import.py`; the raw parsed attack rows it used are in
`sources/throbbin/ledger.json`.

| Slot / variant | Source | Startup / active / total | Damage / launch / mask | Phases / notes |
| --- | --- | --- | --- | --- |
| Jab 1 | Attack11 | 4 / 4–5 / 31 | 2.0, 361→Sakurai, 9 px root + sword circles | Bronze; continues into jab 2 from frame 13 while attack is held |
| Jab 2 | Attack12 | 5 / 5–6 / 34 | 1.5, 361 (the script's first capsule row; its extra 180° and duplicate rows are folded into one capsule) | Bronze; continues into jab 3 only while the Fire tome has a use, else skips to `jabEmpty` |
| Jab 3 (Fire) | Attack13 | 6 / 6–7 / 35 | 5.0, 361, angle 180 rows→forward | Costs 1 `fire`; continues into the rapid loop from frame 9 only while the Wind tome has a use |
| Rapid jab | Attack100Sub | 4/6/8… | 0.9, 361, rehit 2 | A 60-tick loop that pays 1 `wind` per loop (about the source's one-per-second drain) and exits to the finisher when the tome is empty |
| Rapid finisher | Attack100End | 5 / 5–6 / 47 | 2.0, 74°, hitlag 3, two body circles | Wind blast upward |
| Forward tilt | AttackS3 | 9 / 9–10 / 32 | 7.5, 361, capsule | Bronze slash |
| Up tilt | AttackHi3 | 6 / 6–14 / 32 | 6.0 (three overlapping boxes, BKB 50/60/70), 96°, `dir: away` | Fitted arm-sweep track |
| Down tilt | AttackLw3 | 7 / 7–8 / 21 | 6.0, 30°, capsule | Bronze low poke |
| Dash attack | AttackDash | 8 / 8–10 (11–18) / 41 | 10.0 then 6.0, 45°/65° | Slide frames + forward impulse |
| Forward smash (Levin) | AttackS4 | 16 / 16–18 (19–27) / 57, charge 6 | 16.0 clean (Sakurai, capsule 85→35 px), 10.0 late | Variant needs 1 `levin`; costs 1 |
| Forward smash (Bronze) | AttackS4 else-branch | 16 / 16–18 / 57, charge 6 | 9.6, 361, capsule 70→40 px | Default timeline |
| Up smash (Levin) | AttackHi4 | 12 / 12, 13–16 (17–32) / 52, charge 5 | 15.0 clean (electric, overhead track), 10.0 front box, 10.0 late | Variant needs 1 `levin` |
| Up smash (Bronze) | AttackHi4 else | 12 / 12–16 / 52, charge 5 | 9.0, 55° | Default timeline |
| Down smash (Levin) | AttackLw4 | 16 / 16–17, sparks 19–28 / 56, charge 3 | 15.0 clean both sides, 12.0/10.0 spark rows (direction away) | Front then back low sweep tracks |
| Down smash (Bronze) | AttackLw4 else | 16 / 16–17 / 56, charge 3 | 8.0, 28° both sides | Default timeline |
| Neutral air (Levin) | AttackAirN | 7 / 7–10, 20–23 (24–28) / 47, land 11, AC 1–3 / 40+ | 11.5 in front, then 11.5 and a 5.0 late hit **behind** (the UFD frames show the blade crossing the body), 30° | Variant needs 1 `levin`; each swing is its own phase so both can hit the same victim |
| Neutral air (Bronze) | AttackAirN else | 7 / 7–10, 20–23 / 47, land 11, AC 1–3 / 32+ | 6.9 ×2 | Default timeline |
| Forward air (Levin) | AttackAirF | 12 / 12–15 (16–19) / 33, land 11, AC 1–3 / 28+ | 12.5 clean (55°), 5.0 late | Variant |
| Forward air (Bronze) | AttackAirF else | 12 / 12–15 / 33, land 11, AC 1–3 / 27+ | 7.5, 361 | Default |
| Back air (Levin) | AttackAirB | 9 / 9–10 (11–16) / 39, land 11, AC 32+ | 15.0 (361, behind), 5.0 late | Variant |
| Back air (Bronze) | AttackAirB else | 9 / 9–10 / 39, land 11, AC 32+ | 9.0, 361, behind | Default |
| Up air (Levin) | AttackAirHi | 10 / 10–13 (14–23) / 45, land 11, AC 1–4 / 28+ | 13.0 (electric), 5.0 late | Variant |
| Up air (Bronze) | AttackAirHi else | 10 / 10–13 / 45, land 11, AC 1–4 / 27+ | 7.8, 95° (the script's plain angle, not a Sakurai one) | Default |
| Down air (Levin) | AttackAirLw | 13 / 13–15 (16–17, 18–24) / 59, land 16, AC 48+ | 11.0 spike (air) and 11.0 at 70° (ground) early, 12.0 clean, 8.0 late; a mostly vertical box below the feet, as the UFD frames show | Variant. The script's 12.0 clean tier sits between the UFD early/late tiers and is kept; UFD's four-value list folds it away. |
| Down air (Bronze) | AttackAirLw else | 13 / 13–15 (16–24) / 59, land 16, AC 48+ | 7.2 then 6.0 | Default |
| Grab / dash grab | Catch / CatchDash | 7–8 / 39; 10–11 / 47 | capsules at 20→46 px and 20→55 px | Slide on the dash grab only |
| Pummel | CatchAttack | 1 | 1.5 (`stats.pummelDamage`) | shared hold state |
| Forward throw | ThrowF | release 16 / 31 | 8.0, 45°, BKB 95, KBG 0.25 | shared throw motion |
| Back throw | ThrowB | release 25 / 44 | 11.0, 52°, BKB 85, KBG 0.60 | shared |
| Up throw | ThrowHi | release 15 / 47 | 9.0, 93°, BKB 100, KBG 0.30 | shared |
| Down throw | ThrowLw | release 16 / 37 | 6.0, 85°, BKB 50, KBG 1.08 | shared |
| Neutral special | SpecialN/Shoot + `reflet_thunder` | charge frame 8; tiers at 33 / 73 / 133; 32/38/38 / 74 (Thoron) | Thunder 5.5 (50°, KBG 0.20), Elthunder 11.0 (50°, 1.0), Arcthunder 8.0 blast (45°, 0.85), Thoron 2.6 rehit 4 (45°, 1.91) | Charge holds at frame 7 with `max: 126`; `release` transitions pick the tier by `minCharge` (126/66/26/0). Costs 1/3/5/8. Empty tome: 39-frame animation. |
| Side special | SpecialS + `reflet_gigafire` | 17 / 63 | Arcfire bolt 2.0 (115°, fixed KB), flame pillar 1.3 rehit 8, burst 4.0 at 70° (the script's angle; UFD lists 110°) | Bolt + lingering flame + late burst projectiles at fixed offsets; the pillar spawns at a fixed point rather than where the bolt lands. Costs 1 `fire`. |
| Up special | SpecialHi + `reflet_elwind` | blades at 8 and 28; total 85 | First blade 7.0 at the source's 255° point-blank spike, second 7.0 at 55° | Startup, rise1 (wind −1, ledge catch from frame 14), rise2 (wind −1), `land` → 25-frame landing lag, else helpless. A projectile carries one contact hit, so Elwind 1's paired 5.0/80° body box is dropped (adaptation). One Wind use gives the single-blade variant; empty tome: 51 frames, no lift. |
| Down special | SpecialLwStart/Capture/End | grab 15–18, intangible 15–18, interruptible 50 | 1.0 paralysing capture, then 2.0 rehit 8 drain with `heal: 1.5` | Unblockable capture and drain (ignores shields/counters, from melee or projectile contact); costs 1 `dark`. A whiff runs a no-hit recovery and totals the source's 49 frames; catching on the first window tick also totals 49, and a later catch runs up to three ticks longer because the drain length is fixed. |

Special notes:
- Levin attacks pay their use when the move's first phase is entered, so an interrupted smash still consumes it
  (matching the source's "being interrupted counts as a use").
- The Levin-era "sword already out" rule for neutral air is not modeled: neutral air uses its Levin variant
  whenever the pool has a use.
- Poses show the Levin-length blade outside Levin attacks; the Bronze variant scales the attachment to 0.62.
- The sword rides on his back while he casts or grabs (the source sheathes it): those phases hide the hand blade
  and show a `swordBack` copy, using the new per-phase `attach.hide` window (the inverse of `show`).

## Validation and handoff

- Reference tests (`crates/sim/tests/throbbin.rs`, 30): package identity and pool sizes; Levin variant/cost
  wiring and Bronze fallback data; active windows (forward smash 15–17 clean and 18–26 late after the entry
  shift; forward tilt 8–9; down tilt 6–7); 8 Levin uses then Bronze plus gradual recharge to full after 600
  ticks, including that a use spent while recharging is not refunded; Thunder tier selection at 0/26/66/126
  held ticks with costs 1/3/5/8 and each tier's projectile damage (Shoot0/1, the Arcthunder blast, Thoron
  2.6 + rehit 4); every move and variant duration against the pinned UFD totals; empty-pool routing (the Fire
  burst and the Wind rapid loop are skipped, one Wind use still gives the first Elwind blade, empty tomes
  animate without hitboxes); Nosferatu's four-frame capture window, whiff recovery, shield bypass and
  1.5x drain healing; both neutral-air swings landing on the same victim; every Levin move completing with
  empty pools; aerial landing lag; contact reach; raw-input routing for jab/tilts/specials/grab; every sword
  angle key turning at most half a turn (a wrapped key spins the blade the long way); the forward smash's blade
  lying within 25° of horizontal through frames 15–26; every spell/grab phase stowing the hand sword and showing
  the back copy while normal attacks keep it in hand; and every spell article carrying vector shapes.
- Contact coverage: `move_reach_matches_fitted_geometry` places a still dummy at 40/55/70/85/100 px in the
  direction each attack swings and asserts the intended close distances connect and nothing reaches 170 px.
- Shared-behaviour coverage: `tests/content.rs` roster expectation, `tests/stages.rs` `new_fighter_cpu_matchups`
  (all four stages, three opponents, four seeds: 48/48 finished, 11 wins, 134 KOs, 8 self-KOs) and the existing
  `every_ledge_is_recoverable` / `cpu_matches_finish_on_every_stage` sweeps now include the package.
- `just throbbin-matchups 8` (Arena, every other fighter, 8 seat-swapped seeds, 160 matches): 81 wins (51%),
  450 KOs, 38 self-KOs, 0 unfinished. Worst matchups: golem and stratus 0/8, lancer/duchess/ninja 2/8; best:
  king 8/8, bongo/plant/whiplash 7/8. The test asserts every match finishes and self-KOs stay under a third of
  KOs.
- `just balance 6 throbbin` (6 seeds, every opponent and stage, 360 matches): 0 unfinished, 121 wins, 89k
  damage dealt / 131k taken, 68 self-KOs (more than an earlier pass because a single Wind use now yields the
  one-blade Elwind, as in the source). Per-move hit counts are spread sensibly (up air 1215, down air 1111, up
  tilt 1879, Arcfire 1054, Nosferatu 3208 drain ticks). Neutral air reports 0 hits: the CPU rarely selects it
  (about 1.3 starts per match) and always starts it 45-50 px above the ground while descending, so it lands
  before its frame-7 window (and long before the second swing at frame 20). Contact tests, a raw short-hop
  input and the two-swing test all connect; the cause is the shared CPU's aerial timing, not the move data.
- Unblockable hits now bypass guards, counters and parries on projectile contact too (they already did for
  melee); `recharge: 0` keeps a pool spent until respawn; variant counts are validated against the `u8` snapshot
  field; the editor's preview marks record which timeline ran, the picker forces it through `preview::Setup`
  (so selecting Base/Bronze previews that timeline instead of snapping back), and the inspector, timeline, inline
  pose targets and direct manipulation all follow the selected/simulated variant.
- New engine semantics (shared, data-driven): `Fighter::charges`, `ChargePool`, `ChargeNeed`, `Move::variants`,
  `Phase::cost`, `Transition::{minCharge,needs}`, `Hitbox::{heal,unblockable}`, `sim::charges` (spend/tick/
  refill). Recharge restores only newly earned uses, so a use spent mid-recharge stays spent (the pool then
  runs another shorter cycle for the missing units). The editor's move inspector gained a timeline picker and
  its pose/animation and hurt-region passes now walk every variant timeline, and the names/box lists in the
  timeline panel follow the selected timeline. Validation rejects unknown pool ids, zero-use pools and zero-use variant needs; the phase lists of
  variants are validated like the default timeline. `SIM_VERSION` bumped to `-sim-27` and the CPU golden world
  hash re-blessed because the snapshot gained `Fighter::charges` / `charge_wait` and `MoveState::variant`.
- Commands run: `cargo test -p sim --test throbbin`, `just bless`, `just lint`, `cargo fmt --all`,
  `cargo test --release --workspace` (all green), `just shots /tmp/throbbin-shots fighter-kits` (sections
  finish; PNGs are black in this session), `cargo test --release -p sim --test stages new_fighter`,
  `just throbbin-matchups 8`.
- Visual inspection: four passes. (1) The UFD per-move hitbox GIFs were downloaded (37 files, hashes in
  `visuals.json`) and frames around each active window were opened for Levin forward/up/down smash, neutral,
  forward, back, up and down air (Levin and Bronze), jab 1/3, up/down tilt, Thunder, Thoron, Arcfire, Elwind
  and Nosferatu. That pass corrected fitted geometry: down air's hit is a mostly vertical box below the feet
  (not a forward sweep), forward air's late hit sits up-forward, neutral air's first swing covers the front while
  its second swing and late hit sweep **behind** (the blade crosses the body), forward smash's late hit is at
  clean-hit height, and Nosferatu's capture orb sits closer to the hand. Where a GIF exists its frame count
  matches the UFD total the duration uses (forward smash 57, neutral air 47, Elwind 85, Nosferatu 49); the Jab
  and a few spell GIFs only cover part of their move, so the frame count is not treated as the duration for
  those. (2) Since Bevy window captures are black in this session, poses, attachments and blade lengths were
  rendered through the CPU canvas (`render::draw::draw_fighter`) for the same move set and rasterized for
  review; that pass set the blade length/thickness, the gold electric glow, the tome size and the Bronze blade
  scale. (3) A frame-by-frame editor pass captured every move (and both timelines of the sword moves) through
  the real editor preview, cropped the viewport and compared it with the pinned UFD montages. It found and fixed
  four sword arcs whose last key was written on the far side of a wrap (forward smash 10 → 350, up tilt 150 → 340,
  neutral air 340 → 20, down air 60 → 250): with no angle wrapping the blade spun the long way while the hitbox
  stayed forward, so its tip left the hitbox during the active windows. The dash attack's cut was also pitched
  45° up instead of lying along its chest-height box, and the spell/grab phases used `sword(0)`, which both
  pointed the blade forward and overrode the `cast`/`castHigh` arm pose through the hold IK. The pass also
  stowed the sword for those phases and the Fire jab / Wind loop. (4) A body/pose pass read the same dense GIF
  strips for silhouette timing: air smashes and aerials now pitch the skeleton with the source (forward air
  dives ~50°, up air leans ~20°, neutral air ~40°, back air ~15°), with the lean taken back out of the keyed
  blade angle so the fitted geometry stays aligned; smash/tilt/dash tracks gained windup, impact and recovery
  keys instead of holding one pose for 40+ frames. The same pass found the articles had no drawing at all (the
  imported `hit` data was valid but invisible): every Thunder tier, Arcfire bolt/flame/burst, Elwind blade and
  the Thoron beam now has vector shapes, trails, spawn FX and a Thunder charge spark. The Thunder tiers were
  also using the overhead `castHigh` pose while the article left the chest; they now use a two-handed forward
  cast, and Arcfire raises the tome hand through its release, so each spell leaves the hand that throws it
  (the Arcfire bolt spawn moved from chest height to the raised hand). The renders were checked frame by frame
  in the editor and in the live game.
- Controller playtest: not performed.
- Screenshot tour: `30a`-`30m` cover the Levin smashes and aerials, the four specials, the jab, the charge pose
  and a Bronze forward smash; the Bronze shot forces the base timeline through the editor's preview variant
  instead of editing the charge pools (which validation rejects).
- Known approximations: fitted weapon geometry and poses; Arcthunder vortex (the 2.4 rehit stage is folded
  into the 8.0 blast, and the 6.0 initial contact is dropped because an exploding projectile carries one hit);
  Arcfire pillar placement; Elwind late damage tier (a projectile carries one contact hit); Nosferatu is a
  paralysing unblockable drain rather than a held command grab, and its damage-comparison healing bonus is not
  modeled; no discarded-item mechanic; no in-match durability meter; the resting blade is always drawn at Levin
  length outside Bronze attacks; the stowed sword sits on the back rather than in the hip sheath the source
  shows during the Wind loop; the spells' vector shapes are stylized (their sizes follow the hit radius and the
  source art's palette), not source models, and the source's per-frame article animation is not reproduced.
- Delivery: local branch `char-tune`, uncommitted working tree (this record, the importer, the package, engine
  changes and tests).

Momentum migration (2026-11-04): `drift()` in the importer rewrites phase `airAccelMult` for `movement.ultimate` (airAccel × kind multiplier) keeping each phase's effective drift; regenerated with `just import-robin`.
