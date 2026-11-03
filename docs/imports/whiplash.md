# Whiplash reference and conversion record

## Target and status

- Package/id: `assets/fighters/whiplash/fighter.json`; display name: **Whiplash**.
- Branch: `char-tune`; source: Zero Suit Samus, Super Smash Bros. Ultimate, final balance patch 13.0.1.
- Identity: a light, quick acrobat with high jumps, quick kicks, forward/backward aerial pressure,
  long plasma attacks, paralysis setups and a Flip Jump that preserves her normal double jump.
- Import ready for playtesting: content, shared mechanics, regressions and visual inspection are complete.
  Hardware controller playtesting has not been performed by the importer.

## Sources and repeatability

| Source | Revision | What it establishes |
| --- | --- | --- |
| [Ultimate Frame Data](https://ultimateframedata.com/zero_suit_samus) | Accessed 2026-11-02; page hash in `sources/whiplash/visuals.json` | Gameplay attack windows, lag, autocancels, move notes and movement stats |
| UFD linked original GIFs | 38 original animations, URLs and SHA-256 hashes retained in `sources/whiplash/visuals.json` | Per-frame pose, tip travel and hit coverage |
| [Rubendal numeric scripts](https://github.com/rubendal/ssbu/blob/2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/Zero%20Suit%20Samus/data.json) | Commit `2e8f0d245481cb0c936524b6ffe1cdea867c4f68`, patch 13.0.1; original file hash and selected verbatim scripts in `sources/whiplash/scripts.json` | Damage, launch rules, masks, hit priority, root offsets, radii, fixed knockback and hitlag |
| [SmashWiki Boost Kick](https://www.ssbwiki.com/Zero_Suit_Samus_(SSBU)/Up_special) | Accessed 2026-11-02 | Final patch attack windows and 77-frame animation |
| [SmashWiki Flip Jump](https://www.ssbwiki.com/Zero_Suit_Samus_(SSBU)/Down_special), [move overview](https://www.ssbwiki.com/Flip_Jump) | Accessed 2026-11-02 | Kick/bounce choices, 19–38 footstool detection, intangibility and the one-use resource |
| [SmashWiki paralysis](https://www.ssbwiki.com/Paralyze) | Accessed 2026-11-02 | Freeze before launch and Ultimate paralysis duration |

Acquisition: source JSON and HTML were downloaded from the recorded URLs with `curl -L --fail`;
original GIFs were downloaded with Python `urllib.request`. `just whiplash-visuals [dir]` repeats the
hash-checked GIF download and extracts contact sheets using Pillow. Original frames were inspected
at `/tmp/whiplash-reference/contact-research.png`, `/tmp/whiplash-reference/normals-research.png`,
and `/tmp/whiplash-reference/BoostKickG-trajectory.png`.
GIF index 0 represents gameplay frame 1; source scripts and package phase frames retain their own explicit
frame convention. Raw scripts and the independent normalized attack ledger are durable repository inputs.

`just import-whiplash` runs `tools/import-zss/import.py` and `just bless`. It replaces the entire
Whiplash package and `sources/whiplash/ledger.json`; it does not patch hand edits. Keep edits intended
for regeneration in the generator. This is a fitted 2D adaptation, not an original skeleton extraction.

### Disagreements and decisions

- Boost Kick animation scripts hit on 4/10–22/26/32–33; UFD and the final-patch wiki establish real gameplay
  frames **6/12–24/28/34–35**. All attack windows are shifted +2, with total 77 and landing lag 30.
- Forward tilt's UFD damage shorthand omits the inner hit. Scripts establish **7% outer / 6% inner** for
  the neutral angle; those values and source priority are retained. Angled 8% variants are omitted.
- Down-air landing clears its script hitboxes at animation frame 3; UFD lists inclusive 1–3. The adaptation
  uses **1–3**, with the source 5.5% damage and 24-frame landing sequence.
- Flip Jump's UFD total is 38; the wiki lists the first actionable frame as 39. The flip phase lasts 38
  ticks, ending on its tick 38 according to this engine's phase convention. Detection starts at 19 and
  cannot exceed the phase duration.
- The linked upward Plasma Whip GIF contains55 frames, matching the page's total; its longer article removal command57 does not lengthen the fighter move. Boost Kick's aerial GIF contains76 frames, one fewer than its77-frame grounded GIF; both branches use77 in this adaptation.
- Paralyzer scripts explicitly interpolate uncharged and charged boxes. The adaptation interpolates damage,
  fixed launch, radius and lifetime continuously across 22 charge ticks. Fully charged ground launch uses the source's 315 degrees (−45); airborne launch uses 60. Weak shot ground launch uses a fitted
  0-degree angle while its airborne angle is 45. The projectile uses the later 0.7 hitlag multiplier
  throughout; the source weak bullet's first-tick 1.0 multiplier is omitted.

## Conversion rules

| Property | Mapping | Classification / limits |
| --- | --- | --- |
| Time | 60 Hz, inclusive attack windows; first phase enters at 0. Branch phases subtract their elapsed prefix; Paralyzer phase 21 with hold 20 spawns on release 0 and recovers 27 ticks later. | Converted; specific source motion-rate correction above |
| Root geometry | Source Z → forward X; source Y → upward Y; 5 pixels per unit. | Converted; source X depth discarded |
| Bone geometry | Knee, foot and moving tether-tip positions fitted from GIFs, retaining source bone-local radii/offset spacing. Up-air follows a per-frame somersault arc. | Fitted; no original bone matrices |
| Capsules | Overlapping circles at spacing no greater than the source-scaled radius, retaining original hitbox ordering. | Adapted projection |
| Locomotion | Walk/run/dash/air speeds ×3.5; gravity/fall speeds ×6. Source gravity .120, fall1.7/fast2.72. | Fitted world scale, independent of hit geometry |
| Jumping | Full 18.1, short 12.4, double 18.0 px/tick; source full/short/double heights 44.5/21.35/44.2 used as feel targets. | Fitted to shared gravity and stage scale |
| Weight | Source80 → reciprocal100/80 =1.25. | Converted for this game's weight convention |
| Ordinary knockback | Source BKB unchanged, KBG/100; shared Ultimate-style calculation. | Converted; shared DI/launch physics still apply |
| Fixed knockback | Source fixed value resolved at reference weight100: `(1 + FKB/2) * 1.4 + 18`, then source growth/base. | Adapted to shared fixed-launch representation |
| Typed angles | 361 → Sakurai; 367 → shared autolink; negative downward launch → spike where appropriate. | Shared implementation; exact bone-dependent autolinks fitted |
| Body/hurtboxes | Slim upright 2D collision/hurtbox groups, independent of animation. | Fitted; no per-bone hurtbox changes |

## Move ledger

The numeric ledger is `sources/whiplash/ledger.json`; it retains each attack's original fields and windows,
including masks and first-box priority. Below are gameplay frames, damage and the destination mapping.

| Slot / variant | Startup / active / total | Damage and behavior | Landing / charge |
| --- | --- | --- | --- |
| Jab 1/2/3 | 1 / 1 /23; 2 /2 /25; 3 /3–4 /34 | 1.5 /1.5 /3; holding attack may chain at phase frame6, fresh hit registry each jab | Shared hold-to-chain input |
| Ftilt | 6–8 /28 | 7 outer,6 inner; neutral angle only | — |
| Utilt | 3–4,9–10 /45 | 5 then7; handstand split closes overhead | — |
| Dtilt | 8–9 /29 | 8 main,6 sour; low sliding kick | — |
| Dash attack | 7–9,10–19 /51 | 8 early,5 late; attack-slide drift | — |
| Fsmash | 13–14,26–28 /64 | 5 linking kick then11; source foot sweetspot priority | Charge frame9 |
| Usmash | 10–11;14,17,20,23,26;28–29 /48 | 4 /0.8 multi /3 finisher; plasma column | Charge frame4 |
| Dsmash | 20–24 /41 | 8 close,6 far; paralysis burst forward, rather than both sides | Charge frame15 |
| Nair | 10–11 front,15–16 back /51 | 8; either half hits a victim only once overall | Lag8, AC1–3 and42+ |
| Fair | 6–7,13–14 /46 | 5 then7, source ground/air angles retained | Lag10, AC1–3 and38+ |
| Bair | 8–9 /40 | 10 close,12 far; close box has source first priority | Lag10, AC1–3 and31+ |
| Uair | 6–12 /34 | 6.5; foot/knee/thigh per-frame arc | Lag5, AC25+ only |
| Dair | 14–49 /67; landing1–3 /24 | 6 air spike /5 grounded launcher,5.5 landing shockwave; committed dive | Lag24, AC1–3 and50+ |
| Zair | 9–19 /49 | 5 moving tip; aerial grab input, extended forward ledge reach | Lag8 during10–39, tether2+ |
| Grab / dash grab | 15–25 /58;17–26 /66 | Moving tip; near hand active first3 frames only | Pivot grab omitted |
| F/B/U/D throw | Release9/11/5/22;total25/27/40/51 | Pre-hit5/2/2/4 then launch4/6/8/4 with source angles and growth | Shared held-victim animation |
| Paralyzer | Shot21–43, projectile24–48f; total48–70 | Charge4→6%, fixed34.1→68.4; freezes then launches | Hold on tick20 for up to22 ticks; ready phase21/release27 (overlapping source release-entry frame) |
| Plasma Whip | 22–23,24–25,26–27,28–29,31–35 /55 | 2 near,1.2 travelling multi,8 tip finisher; source ground/air differences | Up input before finisher selects rising4% variant |
| Plasma Whip upward | 31–32,37–38,42–43,46–47 /55 | Tip retracts inward/upward; grounded same-part hit once; air rehit adaptation | Air tether from20 through initial lash |
| Boost Kick ground/air | 6;12–24 rehit4;28;34–35 /77 | 5 /1.3 multi /4 final; source ground/air KB differs | Lag30, helpless afterward |
| Flip Jump | Total38, intangible3–12, contact detect19–38 | One use until landing; preserves double jump/up-B. Contact8% buries ground or spikes air and bounces | Attack/special press14+ selects kick |
| Flip Kick | 9–12 /59; hit bounce49 | 14; separate foot spike/leg launcher and ground masks; bounce on contact | Lag35 during animation/bounce |
| Flip stomp | Hit0–1 /58 | 8 source bury/spike boxes below feet, rising bounce | Landing ends bounce early |

## Adaptations and explicit limits

- Full kit is authored from base; no playable fighter's moves are inherited. Pummel damage is the source 1.3%; its timing uses the shared grab system. Shared dodges, shield, ledge climb/getup attacks and controller buffering stay in effect.
- Angled ftilt/fsmash, pivot grab, Final Smash, wall jump variants and original multi-bone hurtbox animation
  are omitted. Grounded and airborne side/up specials retain separate numeric attacks.
- Flip Jump uses a fitted 2D arc and model-independent facing. Its source backward relative hit direction
  is mapped to travel-forward kick geometry; default flip turns facing at the end. Manual backward kick
  selection and exact wall interactions are not represented. Bounce occurs on hit/shield under shared hit transitions.
- Tether recovery snaps to the ledge using the normal ledge system from an extended forward rectangle;
  the original hanging rope/pull animation, exact occupancy restrictions and tether wall interactions are adapted.
- Down-air physics and Boost Kick/Flip Jump launch speeds are fitted rather than extracted status parameters.
  SideB's upward recovery uses the linked55-frame GIF total; source hit windows remain literal.
- Paralysis uses the shared implementation's weight-independent freeze calculation before normal launch;
  repeated paralysis cannot freeze again until actionability, landing or ledge grab. Crouch-cancel modifiers
  are omitted. [Bury](https://www.ssbwiki.com/Buried) uses the equal-stock source duration
  `ceil(55 + 0.5 * postHitPercent + 1.5 * referenceKB)` with 8-frame directional / 14.4-frame button mashing.
  The 600-frame cap is adapted; stock modifiers, mash rate limiting and deferred weak-hit launch are omitted.
  Buried targets cannot be grabbed and sub-90-KB follow-ups retain the trap.
- The blaster, hair ribbon, jet heels, plasma lines and body poses are original simple shapes styled around
  the projected coverage, not ripped models or animation assets.

## Validation and handoff

- `cargo test -p sim --test whiplash`: **20 passed**. Literal source windows, aerial landing/autocancel,
  jab chains, source knockback at multiple percentages/weights, actual charge/release timing, jump+A/zair,
  buffered Flip Kick consumption, tether windows/range, footstool/kick contacts, paralysis and clone/restore
  are covered. Contact tests ensure up-smash and Boost Kick finishers still connect after the final linking hit.
- `just bless`: **28 content tests passed**, including invalid tether, charge/status and grounded-angle paths.
- `just check`: **passed** (format, clippy with warnings as errors, all workspace tests). Includes TS traces,
  replay round-trip/seeking, every built-in stage's platforms/recovery and Whiplash CPU matchups in both seats.
- The recovery test accepts either catching the target ledge or landing safely above/inside its block;
  a high recovery that lands on a platform is a valid witness. No fighter-id exception was added.
- Screenshots passed: `just shots /tmp/whiplash-shots fighter-kits,training,select`, followed by refreshed
  kit captures in `/tmp/whiplash-reviewed-shots` and Whiplash training captures in `/tmp/whiplash-training-shots`.
  Opened and inspected all eight kit captures (nair, fair, up-smash, whip, boost, flip, zair, Paralyzer) and
  the three training captures (idle, charged projectile, recovery). Corrected fair/Boost Kick pose alignment,
  whip-tip visual length and editor framing; zair now defaults to an airborne editor preview.
- `just import-whiplash` reproduces the canonical fighter package and normalized ledger **byte for byte**.
- New semantics use data-selected behavior, schema validation and editor controls: optional zair, tether reach,
  one-use aerial specials, input-driven follow-ups, ground angles, charged projectiles, paralysis, bury and pummel damage.
  `SIM_VERSION` is `-sim-17`. Golden world hash is `0x576c9949bb891017`; stripping only the new empty
  resource/immunity fields from Debug output reproduces the previous `0x6e6f62d1001c1ba7`, confirming
  the baseline CPU match changed representation rather than behavior.
- Physical controller playtesting remains a human follow-up; no hardware calibration was measured.
  Optional detailed balance diagnostics (`just balance 16 whiplash`) were not run.

The preceding branch state was committed and pushed as `e370251`; this import is a new local change set.
