# Original roster vs imported references: experimental normalization

Status: experimental normalization implemented; human balance acceptance is pending. The first pass is implemented in the ten original fighter packages (King excluded, see below), measured with a
targeted CPU matrix. Imported packages (`mike`, `whiplash`, `wizard`, `throbbin`) and shared `base`/`global` content are
unchanged. This records what was measured, what was changed and why, and what remains unproven. It is not a claim
of competitive parity, especially not for human play.

## Roster classification and scope

| Package | Role in this pass |
| --- | --- |
| `archer`, `brawler`, `dancer`, `golem`, `lancer`, `ninja`, `swordfighter`, `tinker`, `wrangler`, `yoyo` | Original roster. Tuned here. |
| `mike` (Ike), `wizard` (Zelda), `whiplash` (Zero Suit Samus), `throbbin` (Robin) | SSBU-derived references. Read-only. Provenance is recorded in [MIKE.md](MIKE.md), [imports/wizard.md](imports/wizard.md) and [imports/whiplash.md](imports/whiplash.md), and [imports/throbbin.md](imports/throbbin.md). |
| `king` | **Excluded at the user's request.** Another agent is replacing the package with a King Dedede import while this pass runs. No King data was read for tuning or used as a reference. |
| `base` | Shared abstract template (stats, grabs, throws, hurtboxes, poses, state anims), including the inherited throws. Not tuned. |

No behavior is selected by fighter id in the simulation, no engine mechanics or AI thresholds were changed, and no
imported hitbox, frame, stat or AI value was edited.

## The problem the baseline exposed

`just balance`-style diagnostics already existed in `crates/sim/tests/stages.rs`. That harness runs every fighter
against every other fighter on every stage, which is more sampling than this question needs; the targeted harness in
`crates/sim/tests/balance_original.rs` reuses the same measurement logic for originals × the three references.

At the starting commit, the original roster won 94–99.7% of 384 two-stock CPU matches per fighter against the three
imports (16 seeds × both seats × 4 stages × 3 references). It was not noise: the original roster also took 70–85% of
all KO stocks.

The dominant cause was a content-scale mismatch, not one bad character:

- **Knockback growth.** The originals' hitboxes were migrated to the shared Ultimate-style equation by fitting old
  launch values at zero/move damage (see [FORMAT.md](FORMAT.md#knockback-models)). That preserved their old game's
  growth (roughly 0.7–2.9 in the new units), while the imported references carry source Ultimate growth divided by
  100 (~0.5–1.3). With `launchSpeed = 0.105`, the fitted high-percent growth is the difference: in matches, originals
  reliably KO'd around 126–148% while imports needed 210–256% for the same kind of stock.
- **Inherited throws.** Seven originals used `base` throws, whose growth was also fitted to the old scale. Back and
  forward throws became 60–77% of their KOs (e.g. Tinker 374 of ~488 KOs), so any normal-move tuning that ignored
  throws left the strongest kill option untouched. Editing `base` is out of scope, so those fighters now carry
  character-owned throw data (see below).
- **Frame data (not fixed here).** Original normals are much faster than the imported SSBU timelines: e.g. Brawler
  forward smash is 46 total frames / hit 14–17, Mike's is 83 / 31–35; Brawler nair is 28, Mike nair 59. This is part
  of the originals' identity and is a plausible residual source of their higher damage output. It is reported, not
  flattened.
- **Recovery attribution confound.** After the pass, 60% of Mike's deaths and 48% of Wizard's deaths follow
  recently observed recovery states, versus 4–34% for originals. This is a fraction of deaths, not of recovery
  attempts. It cannot establish recovery success rates or separate AI mistakes from forced offstage situations.

## Measurement method

Harness: `crates/sim/tests/balance_original.rs` (new; opt-in, `#[ignore]`d, deterministic, no engine changes). It
duplicates the event accounting of `stages.rs` so it can report both seats symmetrically instead of all-to-all.

```sh
# tuning sample: 16 seeds × both seats × 4 stages × 3 references = 384 matches per original
BALANCE_SEEDS=16 cargo test --release -p sim --test balance_original -- --ignored --nocapture original_vs_reference_balance

# holdout: 8 different seeds, never used while choosing factors
BALANCE_SEEDS=8 BALANCE_FIRST_SEED=17 cargo test --release -p sim --test balance_original -- \
  --ignored --nocapture original_vs_reference_balance

# one deterministic smoke match per seat
cargo test --release -p sim --test balance_original -- --ignored --nocapture original_reference_matchup_is_reproducible
```

`BALANCE_STAGES`, `BALANCE_ORIGINALS`, `BALANCE_REFERENCES` and `BALANCE_VERBOSE=1` narrow the matrix. The harness
prints one `BALANCE {json}` line per fighter/matchup/stage plus reference totals; raw artifacts are stored gzipped in
[`balance/`](balance/).

Protocol: 2 stocks, CPU level 2, every built-in stage (`arena`, `citadel`, `foundry`, `isles`), both seats for every
seed. Metrics per fighter:

- **W%** — match wins. Confounded by AI recovery, so not the only target.
- **KO%** — mean victim percent just before a KO credited within 240 ticks. Kill power in actual play.
- **KO-stock share** — `KOs dealt / (KOs dealt + stocks lost with a hit attributed within 240 ticks)`.
  This excludes deaths without a recent attributed hit, not necessarily unforced self-destructs. A failed AI recovery
  after a hit can count as an opponent KO; a longer forced recovery can count as self-KO. It is a diagnostic proxy,
  not an isolation of data balance. Recovery timestamps also refresh during offstage helplessness, so
  recovery-death% is deaths with a recent recovery observation divided by all deaths, not deaths per recovery attempt.
- **dmg/min**, **deaths/match**, **self-KO%**, **recovery-death%**, and per-slot starts/hits/tips/blocks/whiffs,
  punish starts/hits, KO counts and KO percent.

Uncertainty: ~2.6 percentage points (1 SE) for a per-fighter win rate over 384 matches, before seed/stage
correlation; a per-stage figure (96 matches) is about ±5 points. Differences smaller than ~5 points are not treated
as real. The holdout is used to check that tuning seeds did not simply overfit.

## Before (unchanged content)

| Fighter | W% vs imports | KO% | dmg/min | deaths/match | self-KO% | recovery-death% | KO-stock share |
| --- | --- | --- | --- | --- | --- | --- | --- |
| archer | 98.2 | 129 | 128 | 0.46 | 0.0 | 5.6 | 79.0 |
| brawler | 97.4 | 138 | 134 | 0.56 | 2.3 | 13.4 | 76.1 |
| dancer | 96.9 | 126 | 134 | 0.57 | 0.5 | 8.2 | 75.9 |
| golem | 97.7 | 139 | 150 | 0.47 | 3.9 | 36.1 | 78.5 |
| lancer | 99.7 | 137 | 159 | 0.31 | 0.0 | 6.7 | 84.9 |
| ninja | 97.7 | 129 | 126 | 0.53 | 0.0 | 3.4 | 76.4 |
| swordfighter | 98.7 | 132 | 145 | 0.38 | 1.4 | 6.9 | 82.1 |
| tinker | 95.8 | 145 | 126 | 0.70 | 0.7 | 11.9 | 70.4 |
| wrangler | 94.3 | 139 | 135 | 0.63 | 2.5 | 12.0 | 73.9 |
| yoyo | 96.6 | 148 | 138 | 0.63 | 0.8 | 9.5 | 74.1 |

Imports at baseline: Mike 2.2% W, 1.98 deaths/match, 48% of deaths after recovery, KO% 210; Whiplash 1.7%, 1.98,
14%, KO% 256; Wizard 4.2%, 1.95, 30%, KO% 212.

## Changes

All edits are character-owned JSON. Normals and specials keep their damage, timing, hitbox geometry, angles, BKB,
weight, movement, jumps, poses, attachments and projectiles. Only **knockback growth (`kbg`)** changes, plus explicit
throw data where a fighter previously inherited `base` throws. Growth multiplies both the percent-dependent term
and the equation's constant `18`, so these changes reduce launch and hitstun even at zero pre-hit damage despite
unchanged BKB. For example, Brawler jab against a middleweight drops from 4.28 to 2.7384 launch and 11 to 8 default
hitstun frames at zero pre-hit damage (17 to 11 frames at 50%). Combo routes, multihit connections and kill confirms
remain unvalidated. Preserving authored timing and geometry does not prove preserved gameplay identity.

```text
new kbg = old kbg × factor        (every hitbox and projectile hit in the package)
new throw kbg = base throw kbg × factor   (for packages that now own their throws)
```

| Fighter | kbg factor | non-throw kbg range before → after | throws |
| --- | --- | --- | --- |
| archer | 0.52 | 1.43–2.62 → 0.74–1.36 | owns base timing with scaled kbg |
| brawler | 0.53 | 1.19–2.10 → 0.63–1.11 | owns base timing with scaled kbg |
| dancer | 0.53 | 0.71–2.67 → 0.38–1.41 | owns base timing with scaled kbg |
| golem | 0.48 | 1.09–2.38 → 0.52–1.14 | owns base timing with scaled kbg |
| lancer | 0.42 | 0.71–2.54 → 0.30–1.07 | owns base timing with scaled kbg |
| ninja | 0.55 | 1.33–2.86 → 0.73–1.57 | already owned; scaled |
| swordfighter | 0.42 | 1.36–2.67 → 0.57–1.12 | owns base timing with scaled kbg |
| tinker | 0.62 | 1.47–2.34 → 0.91–1.45 | owns base timing with scaled kbg |
| wrangler | 0.55 | 1.24–2.31 → 0.68–1.27 | already owned; scaled |
| yoyo | 0.58 | 0.95–2.31 → 0.55–1.34 | already owned; scaled |

Throws for archer, brawler, dancer, golem, lancer, swordfighter and tinker now live in their own packages with the
unchanged base `release`, `total`, damage, BKB and angle. This keeps `base` untouched. It does mean those packages no
longer inherit future base-throw edits; that duplication is a deliberate review tradeoff.

Per-fighter rationale (all factors started from the same measured scale problem, then one tuning iteration moved each
fighter toward the same evidence band):

- **Archer** (0.52): zoner whose strongest pre-pass kill option was the inherited back throw (226 of its KOs). After
  scaling, KOs come from arrows, up air and side-special; arrow damage and spacing are unchanged. Kept slightly more
  kill power than the sword fighters because its close-range kit is the weakest.
- **Brawler** (0.53): inherited throws supplied 382 KOs at baseline. The tuned kit kills mainly with up air, up tilt,
  energy ball and forward air, matching its brawler identity. Weight, speed and jab/throw timings are untouched.
- **Dancer** (0.53): up-special and up air remain the finishers; inherited throw kills (334 at baseline) are no
  longer the primary plan. Fast movement and dance-special identity unchanged.
- **Golem** (0.48): heaviest original; high damage per hit (10.5) and up tilt/down-special pressure still work.
  Needed a larger growth reduction to offset that raw damage and survivability (weight 0.78).
- **Lancer** (0.42): longest reach and highest damage output in the matrix (504/match after tuning). Lunge, up air
  and up tilt remain its kills. The largest reduction keeps its ranged/disjoint identity while removing the
  old-scale kill confirms. Still a raw-win-rate outlier (see below).
- **Ninja** (0.55): already owned its throws and never relied on inherited throw kills, so it needed only the
  normal-move scale correction. Up air and down-special stay its finishers.
- **Swordfighter** (0.42): tipper/counter identity retained; counter, up air and Lunge now do the killing. Counter
  effectiveness against CPU opponents is an AI-reading confound (see below), so no counter data was nerfed.
- **Tinker** (0.62): smallest factor reduction because its normals were already the weakest of the ten and its
  baseline KOs were dominated by inherited throws. Gadgets, up air and up tilt still kill; no projectile or
  gadget timing changed.
- **Wrangler** (0.55): own throws, up air/up tilt identity; only normal growth scaled. Raw win rate is the lowest in
  the tuning sample (59.4%), not an overtuned outlier.
- **Yoyo** (0.58): own throws; fast up tilt/up air pressure remains. The smallest effective change, because its
  baseline original-vs-original placement was already weak and its kill confirms are technical.

## After

Tuning sample (seeds 1–16; the same matrix as the baseline):

| Fighter | W% | KO% | dmg/min | deaths/match | self-KO% | recovery-death% | KO-stock share |
| --- | --- | --- | --- | --- | --- | --- | --- |
| archer | 66.1 | 219 | 135 | 1.18 | 0.2 | 5.5 | 53.2 |
| brawler | 65.6 | 238 | 140 | 1.26 | 1.0 | 9.5 | 52.6 |
| dancer | 60.7 | 218 | 137 | 1.29 | 0.4 | 3.8 | 51.9 |
| golem | 68.2 | 264 | 157 | 1.18 | 2.9 | 33.8 | 52.4 |
| lancer | 62.2 | 280 | 167 | 1.26 | 0.4 | 7.7 | 51.8 |
| ninja | 67.2 | 212 | 131 | 1.17 | 0.2 | 4.2 | 53.4 |
| swordfighter | 69.5 | 255 | 158 | 1.16 | 1.6 | 7.4 | 53.8 |
| tinker | 64.3 | 219 | 130 | 1.27 | 0.6 | 12.3 | 51.0 |
| wrangler | 59.4 | 232 | 137 | 1.27 | 2.0 | 12.3 | 51.8 |
| yoyo | 68.0 | 234 | 143 | 1.20 | 0.2 | 6.9 | 54.4 |

Holdout seeds 17–24 (never used for factor selection):

| Fighter | W% | KO% | KO-stock share |
| --- | --- | --- | --- |
| archer | 62.0 | 224 | 51.5 |
| brawler | 65.6 | 237 | 53.2 |
| dancer | 62.5 | 214 | 53.1 |
| golem | 65.6 | 263 | 49.9 |
| lancer | 65.6 | 291 | 53.7 |
| ninja | 66.1 | 213 | 51.0 |
| swordfighter | 61.5 | 276 | 50.5 |
| tinker | 60.4 | 220 | 49.9 |
| wrangler | 53.6 | 237 | 48.2 |
| yoyo | 63.5 | 241 | 52.2 |

Per-reference win rate (tuning sample; all three columns are the original's wins):

| Fighter | vs Mike | vs Whiplash | vs Wizard |
| --- | --- | --- | --- |
| archer | 78.9 | 66.4 | 53.1 |
| brawler | 72.7 | 60.9 | 63.3 |
| dancer | 68.0 | 57.0 | 57.0 |
| golem | 64.1 | 69.5 | 71.1 |
| lancer | 76.6 | 60.2 | 50.0 |
| ninja | 74.2 | 58.6 | 68.8 |
| swordfighter | 89.8 | 55.5 | 63.3 |
| tinker | 79.7 | 58.6 | 54.7 |
| wrangler | 56.2 | 66.4 | 55.5 |
| yoyo | 59.4 | 78.1 | 66.4 |

Imports after: Mike 28.0% W (1.71 deaths/match, 11% self-KO, 60% recovery deaths, KO% 219); Whiplash 36.9%
(1.62, 13%, 15%, KO% 265); Wizard 39.7% (1.57, 29%, 48%, KO% 223).

Stage split (original wins, all references, tuning sample):

| Fighter | arena | citadel | foundry | isles |
| --- | --- | --- | --- | --- |
| archer | 73 | 64 | 80 | 48 |
| brawler | 66 | 57 | 81 | 58 |
| dancer | 67 | 41 | 83 | 52 |
| golem | 66 | 69 | 68 | 71 |
| lancer | 69 | 47 | 79 | 54 |
| ninja | 75 | 53 | 83 | 57 |
| swordfighter | 78 | 56 | 86 | 57 |
| tinker | 60 | 60 | 75 | 61 |
| wrangler | 64 | 57 | 57 | 59 |
| yoyo | 64 | 65 | 73 | 71 |

The small stage (`foundry`) favors the faster originals; the wider/raised stages (`citadel`, `isles`) are closer to
even. This is stage-specific evidence, not a claim that every stage is equally balanced.

### Collateral: original-vs-original pacing

Because `base` and the imports are fixed, matching the imported knockback scale necessarily changes original mirror
matches too. A 4-seed, both-seat, all-stage original-vs-original matrix (10 × 10 matchups, 3,200 matches) measured:

| Fighter | intra W% before → after | intra KO% before → after | match duration before → after |
| --- | --- | --- | --- |
| archer | 50.0 → 56.2 | 141 → 265 | 115 → 206 s |
| brawler | 50.6 → 60.6 | 154 → 290 | 116 → 204 s |
| dancer | 54.1 → 65.0 | 136 → 249 | 107 → 186 s |
| golem | 48.1 → 35.0 | 148 → 308 | 103 → 190 s |
| lancer | 63.1 → 47.5 | 155 → 345 | 107 → 210 s |
| ninja | 50.6 → 46.9 | 146 → 264 | 116 → 208 s |
| swordfighter | 65.0 → 40.3 | 146 → 326 | 109 → 213 s |
| tinker | 36.9 → 54.7 | 163 → 266 | 120 → 207 s |
| wrangler | 38.8 → 39.7 | 155 → 273 | 110 → 193 s |
| yoyo | 42.8 → 54.1 | 162 → 280 | 115 → 200 s |

The intra-roster spread stays wide (35–65% after, versus 37–65% before), so nobody was flattened to 50%. But two
things are worth a design review before committing:

1. **Pacing.** Two-stock original matches go from ~1.8 minutes to ~3.3 minutes, and KOs from ~150% to ~270%. This is
   the unavoidable cost of matching the imported scale while imported/global numbers are frozen. If the project wants
   to keep the original roster's quicker kill percents, the right lever is the shared `launchSpeed`/formula or the
   imports, not more per-fighter nerfs — both were out of scope for this pass.
2. **Intra-roster ordering changed.** Lancer/Swordfighter/Golem became weaker against their own roster while
   Tinker/Yoyo improved, because their factors were chosen against the import trio. The authored kit
   shapes are unchanged, but combo behavior and gameplay identity still require testing; this is not a validated original-roster rebalance.

If that pacing trade is rejected, this pass should not be committed as-is; the alternative is to revisit the shared
knockback scale with the parent, which is explicitly outside this task.

## Interpretation, confounds and confidence

- **Attributed KO-stock shares converge.** Originals take 51.0–54.4% in the tuning sample and 48.2–53.7%
  on the holdout. Mean credited KO% is roughly 212–291 versus 219–265 for imports, and damage per minute changes
  by roughly 2–10%. These measurements support investigating the normalization; attribution and CPU biases prevent
  treating them as proof of parity or human kill power.
- **Raw win rate still favors originals; recovery is a possible confound.** 60% of Mike's deaths and 48% of
  Wizard's deaths have a recent recovery observation. These figures do not measure the chance of dying per recovery
  attempt and cannot establish how much of the win-rate gap is caused by AI. Wizard's no-recent-hit death share
  rose from 19% to 29% as matches lengthened. Whiplash's recovery-observed death share is 15% and its weight is 1.25.
- **Residual strong outliers:** Lancer, Golem, Swordfighter and Yoyo still win 62–70% raw and 51.8–54.4% of KO
  stocks. Their remaining advantages are reach/frame data, weight survivability and, for Swordfighter, CPU opponents
  attacking into counters. Fixing those would need frame-data, damage or stat changes that alter identity; this pass
  deliberately stops at the knockback scale instead of nerfing them into the ground.
- **Residual weak outlier:** Wrangler (53.6% on holdout) is the least comfortable result, though its KO share is still
  51.8%. Its own-throw kit and up-air confirms may warrant a separate look in human play.
- **AI move selection is biased.** CPUs use up tilt, up air and down air far more than a human, and imported
  characters have source-faithful long recovery windows that the AI does not exploit well. Per-move KO attribution is
  therefore diagnostic, not a tier list. No AI threshold was changed to improve these scores.
- **What was deliberately not changed:** imported packages, `base`, `global.json`, damage, frame data, hitbox size,
  BKB, weight, movement, projectiles, cost/charge mechanics and engine behavior. `SIM_VERSION` is not bumped: this is
  content-only tuning, replay files embed their resolved content, and old recordings remain playable with their own
  values. The golden world hash does change because `brawler` and `ninja` are part of its scenario; that constant is
  updated deliberately in `crates/sim/tests/runtime.rs`, not blessed blindly.
- **TS traces.** The new throw overrides would otherwise change the linear/TS compatibility path for Brawler and
  Swordfighter. `tests/common/mod.rs::load_data(linear)` now drops those character throw overrides in linear mode so
  the original shared base throws (restored from `knockback-linear.json`) still drive the traces. `just traces` passes.
- **Confidence:** high that the old knockback scale was the dominant, reproducible mismatch; medium that the chosen
  factors are close to the right stopping point; low that any specific per-reference number generalizes to human
  play. Human playtesting is required before treating this as final balance.

## Verification and artifacts

| Artifact | Contents |
| --- | --- |
| [`balance/before-16-seeds.jsonl.gz`](balance/before-16-seeds.jsonl.gz) | Baseline: 16 seeds, both seats, all stages, originals unchanged. |
| [`balance/before-holdout-17-24.jsonl.gz`](balance/before-holdout-17-24.jsonl.gz) | Baseline on holdout seeds. |
| [`balance/after-16-seeds.jsonl.gz`](balance/after-16-seeds.jsonl.gz) | Tuned content, same tuning matrix. |
| [`balance/after-holdout-17-24.jsonl.gz`](balance/after-holdout-17-24.jsonl.gz) | Tuned content, holdout seeds. |
| [`balance/intra-before-originals-4-seeds.jsonl.gz`](balance/intra-before-originals-4-seeds.jsonl.gz) | Original-vs-original, baseline, 4 seeds. |
| [`balance/intra-after-originals-4-seeds.jsonl.gz`](balance/intra-after-originals-4-seeds.jsonl.gz) | Original-vs-original, tuned, 4 seeds. |

Each line is a `BALANCE {json}` record (see the harness). The historical records omit content hashes and seed-start metadata; their provenance and holdout independence rely on this documented protocol rather than self-contained artifact verification. Gzip keeps the import runs around 200 KB and the larger
intra-roster runs around 500 KB.

Checks run for this pass (in addition to the balance matrix):

```sh
cargo test -p sim --test content        # canonical assets, all packages load
cargo test -p sim --test ts_trace       # TS linear compatibility
cargo test -p sim --test knockback      # linear fallback fixtures
cargo test -p sim --test stages         # built-in stage/recovery/CPU tests
cargo test -p sim --test runtime cpu_match_golden_world_hash
just check
```

Results (the concurrent King import and a separate game/layout change were in the tree):

- `content`: passes 28/28, including canonical formatting for all tuned packages. No `just bless` was needed, and
  none was run (schemas and key ordering are untouched).
- `ts_trace`: passes after `tests/common/mod.rs` drops the new character-owned throw overrides in linear (TS) mode so
  the frozen base throws still drive the traces.
- `knockback`: the old `roster_estimates_preserve_uncharged_middleweight_launches` assertion deliberately encoded the
  pre-balance migration values, so it now checks `linear_compatibility_restores_recorded_launches` against the same
  frozen fixture instead. The test now reads resolved compatibility data, asserts restored damage/BKB/growth without overwriting them,
  and checks all seven inherited throw sets. Focused knockback and TS trace tests and clippy pass after review fixes.
  The frozen TS launch values are untouched.
- `runtime`: passes after updating the golden world hash to `0xe85ae82a53b74406` with a comment; this is an
  intentional content change (`brawler`/`ninja` are in the scenario), not a representation change. `SIM_VERSION` is
  not bumped.
- `stages`: passes 13/13 (plus the ignored balance diagnostic). An earlier in-flight version of the King import made
  `ranged_roster_cpu_matchups` fail its Wizard self-destruct ratio; the finished King data makes it pass, which also
  demonstrates how sensitive that assertion is to concurrent content.
- `cargo test -p sim`: all targets pass.
- Existing all-to-all diagnostics were cross-checked with `BALANCE_SEEDS=4 just balance 4 brawler` and `... tinker`
  (both pass, no unfinished matches); the targeted harness is used for the matrix so imports are sampled evenly
  instead of every opponent.
- `just check`: cannot complete in the shared tree because of concurrently edited files that are not part of this
  change: `cargo fmt --all -- --check` reports diffs in `crates/game/src/{layout_shots,main,select,settings,shots}.rs`
  and `crates/sim/tests/king.rs`, and clippy fails in `crates/sim/tests/king.rs`. Our touched files are rustfmt-clean
  and pass `cargo clippy -p sim --test balance_original --test knockback --test runtime -- -D warnings`. Re-run
  `just check` once the concurrent game/King work lands.

## Balance command

The targeted harness is available through `just balance-original [seeds] [first]`:

```make
# Targeted original-vs-import balance matrix (opt-in; release build)
balance-original seeds="16" first="1":
    BALANCE_SEEDS={{seeds}} BALANCE_FIRST_SEED={{first}} cargo test --release -p sim --test balance_original -- --ignored --nocapture original_vs_reference_balance
```

## Remaining work

1. Decide whether the pacing trade above is acceptable. If not, this pass is the wrong lever and the shared
   launch/knockback scale should be revisited with the parent; imported and global content were frozen for this task.
2. Human playtest each original against Mike, Whiplash and Wizard, especially offstage/recovery and low-percent
   combo routes; the CPU cannot validate those.
3. Decide whether to reduce the original roster's frame-data/reach advantage in a later pass, or accept fast normals
   as the original roster's identity. Its contribution to the remaining raw-win-rate asymmetry is unquantified.
4. Review the duplicated throw data: seven packages now carry base throw timings. A future shared/parameterised
   throw-scaling mechanism would avoid drift.
5. Revisit the intra-roster ordering change with the original roster's design intent; import-matchup tuning moved
   some fighters relative to their peers.
6. Re-run the matrix including King once the Dedede import is final, and re-check the golden hash and
   `ranged_roster_cpu_matchups` in the committed combined tree.

**Superseded scale (2026-11-05).** Every number above was measured before the 5 px/unit unification (DESIGN.md,
Units): horizontal speeds ×5/3.5, gravity ×5/6, original jump heights kept, imports' jumps from source heights,
launchSpeed 0.14 → 0.15 and Arena rebuilt as Battlefield. The matrix has not been re-run; rerun
`just balance-original` before relying on these results.
