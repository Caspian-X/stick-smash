# Wizard: Zelda reference and conversion record

## Target and status

- Package: `assets/fighters/wizard/fighter.json`; id/name stay `wizard` / **Wizard**. This replaces the old orb,
  levitation and counter kit. Branch: `char-tune`.
- Reference: Zelda, Super Smash Bros. Ultimate, numeric attack scripts from patch **13.0.1**.
- Identity: light, slow running but a quick initial dash; floaty movement, precise lightning kicks, multi-hit
  spells, a guided fireball, directional teleport and an independent delayed Phantom.
- All nineteen move slots and four throws are authored. Body/hurtboxes, movement integration and universal
  defensive/input rules remain this game's adaptations. This is a playable 2D reconstruction, not an extraction
  of Zelda's original skeleton, status modules or animations.

## Sources and reproduction

| Source | Revision / retained record | Establishes |
| --- | --- | --- |
| [Ultimate Frame Data: Zelda](https://ultimateframedata.com/zelda) | Accessed 2026-11-02; page hash and original GIF URLs/hashes in [visuals.json](sources/wizard/visuals.json) | Gameplay timing, damage variants, total frames, landing lag, autocancels, charge notes, movement stats |
| [Rubendal Zelda scripts](https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/Zelda/data.json) | Commit `2e8f0d245481cb0c936524b6ffe1cdea867c4f68`, patch 13.0.1, full-input SHA-256 in [scripts.json](sources/wizard/scripts.json) | Source hit ids, geometry, masks, damage, angles, BKB/KBG/FKB, hitlag, rehit, throw attacks |
| UFD original attack GIFs | Eighteen originals downloaded and frame sheets inspected; [manifest](sources/wizard/visuals.json) | Arm arcs, extended kick locations, sweet/sour overlap, magic coverage, Phantom silhouette/construction |
| [Phantom Slash damage explanation](https://www.ssbwiki.com/Zelda_(SSBU)/Down_special) | Secondary cross-check, accessed 2026-11-02; agrees with UFD's damage range | The status-applied damage bonus missing from attack scripts: base damage + 1.3 × (construction level + 1) |

Acquired with `curl -L` from the character page, raw script URL and GitHub commit API; original animations
with Python `urllib.request.urlretrieve`. Selected original scripts are retained verbatim in `scripts.json`.
The complete original download's hash is retained, not just the selected-script hash. The script parser
expands finite loops, tracks `frame` / `wait`, closes replaced/cleared attack ids, and preserves hit ordering.

Rebuild without network access:

```sh
just import-wizard
```

This runs [the retained importer](../../tools/import-zelda/import.py), replacing the entire authored Wizard
package and the normalized [attack ledger](sources/wizard/ledger.json), then runs `just bless`. Hand edits
will be overwritten by this explicit command; edit its documented authoring rules when regenerating.
The ledger is an audit of parsed attack facts and phase mapping; UFD gameplay expectations are also recorded
independently in the table below and literal regression fixtures.

Reacquire original visuals and inspect individual frames (Python with Pillow):

```sh
just wizard-visuals /tmp/wizard-reference
```

This checks each downloaded GIF against its recorded hash and extracts the manifest's selected frames.
A changed upstream GIF requires review before updating the manifest. GIF indices are zero-based; the first
pictured frame of inspected normal-attack GIFs corresponds to source frame 1. Phantom GIFs include construction.
GIF playback durations are not used as gameplay frame timing.

### Source disagreements and missing status data

- **Forward air:** the script uses animation frame 9, with motion-rate commands; UFD/GIF gameplay startup is
  frame 6. Use UFD's 6 / 7–10 active windows, 49 total, and 43+ autocancel; keep numeric damage/launch fields.
- **Nayru's Love:** script animation hits at 13–24 and 28. UFD gameplay windows are 11–22 / 26, reflector 4–40
  and intangibility 4–13. Use the gameplay windows rather than applying script animation frame numbers directly.
- **Down throw:** script animation applies its release at 51 and preliminary hits at 25/31/37/43. Use UFD's
  gameplay damage ticks 25/30/35/39 and release 42, total 61; four 1.5% ticks plus the script's 2% launch.
- **Phantom:** raw scripts show only 2/3/4/5/6 damage. Actual status-adjusted outputs are
  5.9 / 8.2 / 10.5 / 11.8 / 14.1 / 15.4 / 17.7. Preserve those actual values. UFD construction thresholds
  and first-hit timings take precedence over raw attack-animation startup alone; the differences are release
  status delays, not additional damage or a global charge multiplier.
- The original bone transforms, teleport status kinetics, guided-fire travel/size formula and Phantom HP/
  locomotion status modules were not extracted. These are **fitted** below, not asserted to match numerically.

## Conversion rules

| Property | Mapping | Classification and limits |
| --- | --- | --- |
| Frames | 60 Hz; phase entry is frame 0, inclusive hit windows, a phase of N frames times out at N | Converted gameplay windows; separate phases for aerial autocancels, rapid jab and specials |
| Root geometry | Source Z → forward X, source Y → local up; 5 px per source unit | Converted projection; source X depth is discarded |
| Root capsules | Overlapping circles spaced no farther apart than their radius; keep original id priority | Converted 2D coverage; no original depth collision |
| Animated bones | Fitted arm/leg/hand tracks from individual UFD frames; up-tilt track updated every active frame | Fitted, not root-space treatment of bone offsets |
| Presentation | Low circlet/hood, mantle/robe, extended kicking poses, casting arm sweep; no old staff | Fitted; spell tells follow the attacking foot or casting hand, Phantom has construction and strike shapes |
| Scale | One scale on every axis: 5 px per source unit for geometry, units/frame → px/tick and units/frame² → px/tick² (`tools/units.py`) | Converted; replaces the earlier × 3.5 horizontal / × 6 vertical movement scales |
| Walk/run/dash/air speed | UFD × 5: walk 0.914 → 4.57, run 1.43 → 7.15, initial dash 1.958 → 9.79, air 1.092 → 5.46 (also `airJumpDrift`); dash lasts 16 | Converted targets; shared gait/acceleration/friction implementation remains |
| Air accel / run accel | Air 0.065 × 5 → 0.325; run accel 0.4 (fitted 0.28 at the old 3.5 scale, × 5 / 3.5) | Air converted; run accel fitted |
| Gravity / fall | UFD × 5: gravity 0.071 → 0.355, fall 1.35 → 6.75, fast fall 2.16 → 10.8 | Converted |
| Jump | Source heights full 31.55, short 15.24, air 31.55 units ([SmashWiki Jump](https://www.ssbwiki.com/Jump), SSBU jump height ranking, accessed 2026-11-05; unitless game units) → `units.jumps` at gravity 0.355: full 10.4058, short 7.1786, air 10.7608 px/tick | Derived for the sim's discrete integration; measured (`just feel wizard`) 157.75 / 76.2 / 157.75 px = heights × 5 exactly. Supersedes the fitted 11.75 / 8.6 / 12 |
| Weight | 100 / 85 | Converted reciprocal knockback multiplier |
| Ordinary knockback | Unscaled source BKB, KBG / 100; shared Ultimate-style formula | Converted fields; no rage, staleness or source 1v1 multiplier; shared hitstun/DI/decay still apply |
| Fixed knockback | Resolve source fixed equation at reference weight 100 into this engine's `setKb` reference units | Adapted; target-weight behavior is this engine's fixed-launch rule |
| Special angle codes | 361 → Sakurai; 366/367 → autolink; downward angles keep their downward direction | Typed behavior; autolink uses this engine's pull rule |
| Hitlag / masks / rehit | Script multipliers and ground/air restrictions; gameplay multi-hit spacing | Converted, with engine-wide base hitlag and shield rules |
| Hurtboxes / stage body | Shared base static hurt rectangles and 74 px stage body | Shared approximation; limb extension is not a per-bone hurtbox reconstruction |

All content shares the 5 px/unit scale. Fitted (non-source) velocities below were authored at the old
× 3.5 / × 6 scale and converted per axis (x and xy × 5 / 3.5, y × 5 / 6). All resolved tuning lives in the
fighter JSON.

## Move ledger

All frames below are move-relative gameplay frames, not raw animation frames. Damage is before this game's
charge or short-hop rules. Numeric geometry and launch fields for every original hit id are in the normalized
ledger and retained scripts. “Converted/fitted” means converted timing/launch values with fitted bone geometry.

| Slot / source script | Startup / active / total | Damage, direction and priority | Other rules |
| --- | --- | --- | --- |
| jab / Attack11 | 4–5, 7 / 24 | 2.5 each; root capsule/sphere coverage and original id order | Holding A transitions from 9 into rapid startup, first rapid hit 4 frames later |
| rapid / Attack100Sub | One hit every 2 frames after startup | 0.4, forward; +3 reaction frames; ×8 shield stun | Loops while A held; release → finisher |
| finisher / Attack100End | 6–7 / 42 from finisher entry | 3, script launch | Separate phase, not another input slot |
| ftilt / AttackS3 | 12–13 / 36 | 11.5 inner, 15 outer; Sakurai, BKB 72, growth .68/.69 | Arm/hand fitted; angled up/down variants omitted |
| utilt / AttackHi3 | 7–19 / 29 | 7.2, angle 100, BKB 45, growth 1.1; away direction | Moving arm circles follow the overhead sweep |
| dtilt / AttackLw3 | 5–11 / 21 | 5.5, angle 62, BKB 15, growth 1.25 | Root coverage; crouching kick pose |
| dashatk / AttackDash | 6–7, 8–12 / 35 | 12 inner early / 9 outer early / 6 late; angle 50 | Early inner hit wins overlap; shared dash entry velocity |
| fsmash / AttackS4 | 16,18,20,22,24 / 49 | Four 1% autolinks, 13% finisher; Sakurai BKB 37 growth 1.1 | Hold at 10, global 60-frame charge maximum |
| usmash / AttackHi4 | 9–23,25–32,34 / 63 | 2% then .8%, rehit 4; 5% finisher angle 88, BKB 24 growth 2.22 | Hold at 3; preserves source ground/air masks on weak hits |
| dsmash / AttackLw4 | 5–6 front,13–14 back / 37 | 12 front, 10 back; sweet angle 20, inner Sakurai; away direction | Hold at 2; separate front/back windows |
| nair / AttackAirN | 6–7,10–11,14–15,18–19,22–23 / 50 | 2.5 front / 1.5 back multi, 5 finisher; original id priority | Landing lag 12; autocancel 1–3 and 38+ |
| fair / AttackAirF | 6,7–10 / 49 | 20% one-frame toe sweet spot first, 4% leg/body sour; BKB 24 growth .95 on sweet | Lag 15; autocancel 1–2 and 43+ |
| bair / AttackAirB | 6,7–10 / 49 | 20% one-frame rear sweet first, 4 sour; BKB 30 growth .96, back launch | Lag 16; autocancel 1–2 and 45+ |
| uair / AttackAirHi | 14–16,17–19 / 54 | 17 early / 12 late; angle 90, BKB 55, growth .73/.70 | Lag 12; autocancel 1–3 and 54+ |
| dair / AttackAirLw | 14,15–24 / 44 | 16 spike, late 5 close / 4 far; source 270/275 angles | Lag 12; autocancel 1–3 and 40+ |
| grab / Catch | 10–11 / 39 | Script root capsule, grab type | Standing telekinetic pose; pivot grab has no destination slot |
| dashgrab / CatchDash | 13–14 / 47 | Script root capsule, grab type | Shared dash-grab movement |
| fthrow / ThrowF | release 30 / 49 | 10, original absolute throw angle/BKB/growth | Shared throw carrying pose |
| bthrow / ThrowB | release 27 / 49 | 12, backward launch | Shared throw carrying pose |
| uthrow / ThrowHi | release 30 / 49 | 11, original upward launch | Shared throw carrying pose |
| dthrow / ThrowLw | 25,30,35,39, release 42 / 61 | Four 1.5 damage ticks, then 2% angle 104/BKB 75/growth .85 | Damage ticks affect only the held victim; release knockback sees all 8 damage |
| nspec / SpecialN | 11–22 rehit 4, 26 / 57 | 2 inner / 1 ground-only outer multi, 5 inner / 4 outer finisher | Intangible 4–13, reflector 4–40; reflection preserves the move and scales projectile damage ×1.25, speed ×1.8 |
| sspec / guided Din's Fire | Earliest explosion 44; maximum 70; explosion lasts 6 | 7–14 inner, 3.5–7 outer, inner first; source BKB/growth/angles | Launch at 14, fitted 11.4286 px/tick forward (8 at the old scale), steer 2 (2.4); minimum travel 16, auto-release age 42; 14-frame fuse after release; fighter release lag 39 |
| uspec / Farore's Wind | First hit 6–7, disappearance 17–34, reappear hit 35–36 / 75 | 6 first; 10/7 ground or 12/8 air reappearance; inner first | Aim live stick once at 17 (neutral up); 17 travel ticks at a fitted 30 px/tick (21 at the old scale); intangible, ledge grab from disappearance; air ends helpless, landing lag 30 |
| dspec / Phantom | Manual construction thresholds 15,20,28,33,38,45,50; free from 66; auto-launch 121 | 5.9,8.2,10.5,11.8,14.1,15.4,17.7; source angles/BKB/growth; independent attack | Second B press launches after minimum; release lag 39; fully built Phantom waits while Wizard can act |

Phantom's earliest manual first hits are 26 / 32 / 38 / 46 / 59 for kick/punch/side/overhead/rising slash.
The two intermediate construction refinements alter damage without changing their attack type. Manual windup
includes the status delay beyond raw attack-animation startup. Automatic full release uses a 6-frame windup
and first hits at 127. The independent body is destructible by melee, direct projectiles and blasts; fitted HP
progresses 8/10/13/15/18/20/25. Its ground/air windboxes preserve zero damage/hitlag and use shared wind force.
Fitted forward travel increases by construction stage: 5 / 7.1429 / 10 / 10 / 12.8571 / 12.8571 / 17.1429
px/tick (3.5 / 5 / 7 / 7 / 9 / 9 / 12 at the old scale, × 5 / 3.5). Sword collision and the connected visible blade follow fitted 2D tracks: a horizontal side slash,
descending overhead slash and a low-to-high rising full-charge arc. Static Knight body art remains simplified.

Shared mechanics/omissions: pummel remains the base game's 1.5% pummel rather than Zelda's 1.3%; defensive
animation timings, jumpsquat, hitstun, hitlag, ledge actions and platform rules remain shared. Angled ftilts,
pivot grab, final smash, original per-bone hurtboxes, exact special kinetics, Phantom cooldown/shield-health
status behavior and the frame-66 simultaneous-action launch shortcut are not reconstructed. Din's stage-element
early detonation/rare release-animation bug is omitted. These do not hide inherited placeholder specials.

## Engine support

Reusable data semantics, with no Wizard/Zelda id branches in gameplay:

- Optional `stats.dashSpeed` / `dashFrames` for an initial dash distinct from run speed and global dash timing.
- `Phase.reflector`, `travelSpeed`, `hidden`; `articleRelease` transition. Reflectors do not trigger counters.
- `Projectile.article`: controlled construction/travel, frozen release stage, per-stage attack timelines,
  optional shorter automatic windup, independent hurtbox/HP, stage drawings and attack drawings.
- `Throw.ticks`: damage-only pre-release contacts. Attack start ticks and article state are rollback data.
- AI recognizes directional-travel recovery data; editor exposes new phase/throw/stats fields and retains
  preview frames until an independent article finishes. Article definitions use the existing schema forms.
- Replay `SIM_VERSION`: `-sim-16`. Legacy TS tests load the archived original Wizard only for compatibility;
  the obsolete Wizard knockback estimates are removed from the current-roster migration baseline.

## Validation and handoff

Reference/raw-input coverage lives in `crates/sim/tests/wizard.rs`: literal active windows/totals, sweet/sour
priority through actual hit resolution, aerial landing boundaries, two consecutive reflections during
intangibility, raw jump+A/right-stick routing, one-time warp aiming, delayed fire, all Phantom release levels,
independent lifetime/destruction/clone restore, and down-throw damage ticks. Existing all-roster platform,
every-ledge recovery and seat-swapped CPU matchups include Wizard on every built-in stage.

Commands/results and inspected final captures are recorded at completion below. No physical-controller
playtest was performed by the agent; controller feel and matchup balance still need human playtesting.

Completion checks (2026-11-02):

- `just bless`: passes; generated schema and canonical content formatting reviewed.
- `just check`: passes across the workspace, including compatibility traces, replay round-trip/seek,
  roster matchups and every-stage traversal/recovery. Fifteen dedicated Wizard regressions pass; new
  independent-body projectile/blast and invalid-content cases also pass focused tests and sim clippy.
- `just import-wizard`: package and source ledger regenerate **byte-for-byte** after canonical formatting.
- `just wizard-visuals /tmp/wizard-reference-review`: all eighteen original hashes match; sheets extracted.
- Screenshot sections `fighter-kits,training,select` pass. Final kit captures were regenerated after the
  camera/restyling pass in `/tmp/zelda-verified-shots`, with the final Phantom sweep in `/tmp/zelda-sweep-shots`. Actually opened and inspected lightning kick, storm,
  Nayru's Love, warp reappearance, Phantom construction/strike, and Din's explosion captures. The earlier
  empty warp viewport and clipped fireball were corrected by framing the full preview trajectory/articles.
- Detailed optional `just balance` diagnostics were not run; existing seat-swapped CPU coverage passed.
  No physical-controller playtest by the agent. Human matchup balance and exact special kinetics remain
  limitations rather than claimed measurements.
- Delivery: local, uncommitted changes on `char-tune`; nothing committed or pushed for this import.

Momentum migration (2026-11-04): `drift()` in the importer rewrites phase `airAccelMult` for `movement.ultimate` (airAccel × kind multiplier) keeping each phase's effective drift; regenerated with `just import-wizard`.

Unified scale (2026-11-05): the importer now emits every speed/acceleration at 5 px/unit (`tools/units.py`):
source movement × 5, jump speeds from source heights, fitted velocities converted per axis, phase drift
(`airAccelMult`) values unchanged (their legacy reference accels are converted with the air accel). Measured
jump heights match the sources exactly (above). `cargo test -p sim --test wizard` passes; its warp-speed and
dash/run expectations now use the fitted 30 px/tick and UFD × 5. Earlier tuning notes above that quote the
fitted jump speeds or × 3.5 / × 6 values are superseded. The 510 px warp (17 × 30, was 357 px) sent
the CPU's out-of-shield Farore's Wind, aimed horizontally from the ground, off-stage (52 self-KOs / 222 KOs in
`stages::ranged_roster_cpu_matchups`). The shared CPU now aims a directional-warp up special up when it starts
on the ground (`ai.rs`, SIM_VERSION -sim-32); self-KOs dropped to about 24 / 224.

## Ultimate calibration (2026-11-05)

- **Traction / air friction**: Zelda (`zelda`) `ground_brake` 0.116 / `air_brake_x` 0.006 (fighter_param dump
  github.com/j99thoms/smash-charts `data/raw/ultimate_fighter_params.csv`, matching ssbwiki Traction and Air
  friction; high confidence) → `stats.traction` 0.58, `stats.airFriction` 0.03.
- **Full hop initial height**: Ultimate climbs 0.55 × the full hop in its first four airborne frames (ssbwiki Jump, the ratio for most fighters);
  the generator emits `stats.jumpInitialHeight` and derives `jumpSpeed` from the remaining height (`units.jumps`),
  so the full hop keeps its height and lands on Ultimate's frame. Short hops and air jumps are unchanged.
- **Fixed knockback**: FKB hits emit `fkb` with the source BKB/KBG (Ultimate WBKB, weight-correct) and keep `setKb`
  as a marker; they were resolved at weight 100 into a `setKb` BKB.
