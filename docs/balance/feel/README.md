# Feel report

A repeatable set of movement, landing, launch and KO numbers per playable fighter, for comparing physics and
content changes before and after. It only measures: raw `Input`s go through `step_world` (plus
`combat::apply_hit` for the synthetic launches), and nothing in the sim changes.

```sh
just feel            # every fighter: one `FEEL <fighter> {json}` line each, then markdown tables
just feel mike       # one fighter
just feel-baseline   # rewrite baseline.md and baseline.jsonl here
```

The harness is `crates/sim/tests/feel_report.rs` (an ignored test, run in release; the full roster takes about
two seconds). It is deterministic: the same content gives byte-identical output. To compare a change, run
`just feel-baseline` on it and diff against the committed files, or keep the old `baseline.jsonl` aside and
compare the JSON.

## Setup

- Stage `arena`, main block only: its pass-through platforms are removed from the loaded data so tall jumps and
  launches are not caught by them. The fighter starts at centre stage (x 640), standing, facing right.
- Distances are pixels, speeds pixels per tick, times ticks (60 per second). Vertical heights are positive up.
- A value that can't be measured is `null`, or `{ "value": null, "reason": "…" }`. Tables show these as `–`.

## Metrics

**Jumps** (`jumps`). Frame 1 is the first airborne tick (after jumpsquat).
- `full`: jump held through jumpsquat. `height` from takeoff to apex, `apex` frame of the highest point, `air`
  frame it lands on the block.
- `short`: jump pressed for one tick and released inside jumpsquat.
- `air`: a jump press at the full hop's apex; height is measured from where the air jump starts.
- `shortRatio`: short hop height / full hop height.

**Ground** (`ground`).
- `dash20`: distance covered in the first 20 ticks after a one-tick flick from standstill (stick then neutral),
  i.e. the initial dash plus its brake.
- `dash.tap`: the same one-tick flick, then neutral until the fighter stands still (`vx` 0, gait `Stand`):
  `distance` (px) and `frames` (ticks, counting the flick). With `gait.dashRelease` the release skids at once, so
  this is a few ticks at dash speed plus a traction skid (Ultimate's stutter step).
- `dash.hold`: distance (px) over the initial dash with the stick held: `dashFrames` ticks from the flick.
- `run95`: ticks with the stick held from standstill until `vx` reaches 95% of `runSpeed`. A dash at or above run
  speed makes this 1.
- `stop`: from a full run (stick held 40 ticks), stick released: ticks until `|vx| < 0.1` and distance covered
  (the run-stop: brake plus the standing slide).
- `turn`: from a full run, stick held the other way: ticks until facing flips (`face`), until `vx ≤ 0`
  (`cross`) and until `vx` reaches 95% of `runSpeed` the other way (`full`), and the distance slid the old way
  before reversing (`slide`, px).
- `dashBack`: a flick held for 3 ticks, then a one-tick flick the other way and the stick neutral for 19 more:
  `distance` covered from the flick back (px) and `turn`, the ticks spent in the `dashturn` window.

**Air** (`air`).
- `drift`: full hop with the stick held forward until the apex, then neutral. `apexVx` is the speed at the apex
  and `retained15` the speed 15 ticks after letting go, both as a fraction of `airSpeed`.
- `nair`: the same, but attack (neutral) is pressed on the apex tick.
- `fastFall`: ticks from holding down at the full hop apex until `vy` reaches 95% of `fastFallSpeed`.

**Landing** (`landing`): ticks from the tick the fighter touches the block until it is `actionable` (0 means
actionable on the landing tick).
- `soft`: dropped from 1 px above the ground with no speed. `hard`: a neutral full hop landing.
- `helpless`: dropped in the helpless state from 100 px.
- `nair` … `dair`: aerial started on the full hop apex tick (attack button for nair, attack stick for the rest),
  then neutral. An aerial that ends before touchdown gets the ordinary landing.

**Launch** (`launch`): a synthetic hit (12 damage, BKB 40, KBG 1.0, normal type) applied to the fighter standing
at centre by a `brawler` far to the left, at 0, 50, 100 and 150% before the hit, no DI.
- `angle45` and `spike270` (angle 270, which bounces off the ground) each list per percent: `hitlag`, `hitstun`
  frames, launch `speed`, `tumble`, `dx` (horizontal distance when the fighter leaves hitstun), `apex` (highest
  rise above the starting feet), `died`, and `landed` (ticks after the hit until it is back on the main block,
  `null` if it died).
- `death45`: lowest percent (before the hit) at which the 45° hit KOs, or `null` below 300%.

**KO percents** (`ko`): the fighter attacks a standing, neutral `brawler` dummy at centre stage, attacker on the
left facing it (backing it for the back air). Moves: uncharged forward smash (one tick of attack stick), up tilt
(attack stick with stick tilts on), short hop back air, and forward throw (grab, then forward once the hold
allows a throw). The spacing scan moves the attacker in 2 px steps from 300 px away to 0. For the back air it
also tries every press timing from inside jumpsquat to 40 ticks after the jump press, as long as the move starts
in the air. Each move reports two setups:
- `first`: the outermost spacing that connects (the earliest timing at that spacing).
- `best`: the spacing and timing whose first hit launches the dummy hardest at 100%.

For each setup a binary search finds the lowest dummy percent (before the move) for two outcomes, with `range`
(attacker distance, px) and, for the back air, `delay` (ticks after the jump press):
- `percent` is the **launch KO**: the dummy crosses the left, right or top blast line before touching the ground
  or a ledge since the hit, or crosses the bottom line while still in hitstun (a spike that kills). Flying past
  the edge and falling out the bottom after hitstun, or landing and then dropping off the block (the dummy has no
  inputs), is a ring-out instead. `null` with reason `no launch KO below 300%` means it never
  happens below the cap.
- `ringOut` is the lowest percent at which the dummy leaves the main block at all (hangs on a ledge, or dies),
  reported only when that first exit is not already a launch KO. It shows where a move pushes a helpless dummy
  off the stage before it can kill. `null` means the move launch-KOs first or never moves the dummy off.

The table prints `first / best` when they differ, `none` for no launch KO and `no hit` when no setup connected.
The ring-out column lists forward smash / up tilt / back air / forward throw, each as `first/best` when they
differ.

The synthetic launch's `died` and `death45` use the same launch-KO rule.

A move with several hitboxes reports whatever it hits at those spacings, so `first` often finds a sourspot or
late hit. Both numbers are for this one dummy, weight and stage.
