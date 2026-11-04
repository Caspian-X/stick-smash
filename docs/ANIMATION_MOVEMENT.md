# Movement animation readability

This pass makes the shared `base` movement poses read as designed input responses. It changes only `anims` in `assets/fighters/base/fighter.json`: no move phases, frame data, physics, collision, damage, hitboxes, input derivation or facing were touched. No fighter id is branched on in the engine.

## Why base animations

`base` is the common visual parent for most of the roster. Editing its named poses would also shift every move pose that inherits them; several fighters use `stand`, `crouch`, `air`, `lean` and `back` as attack pose parents. This pass therefore leaves all named poses alone and only edits state-animation keys (plus references to existing named poses). Move animations and their contact joints are unchanged. King supplies its own movement animations and Mike overrides `air.rise`, so their existing designed variants remain.

Grabs were just given dedicated hold/grabbed bands. Those poses are deliberately untouched here.

## Changes

- **Jumpsquat**: the three-frame crouch hold is now load → deepest compression → extension, with both feet at `y = 0` on every frame. The last frame raises the arms toward the takeoff pose, so the jump is legible before the fighter leaves the ground. The grounded window and takeoff timing are unchanged.
- **Jump rise**: the first three air frames now start from a stretched launch pose and settle into the previous rise pose. This gives the takeoff a direction instead of snapping to a static airborne pose.
- **Landing**: impact compression is deeper than the old crouch pose, then recovers through a ready stance over the first two visible recovery frames and eases to `stand`. Short hard landings now end near the neutral pose instead of popping out of a deep crouch; longer helpless/airdog landings keep a readable recovery stance.
- **Shield drop**: release now travels from the shield pose through a lowered guard to `stand` during the existing drop lag. The feet stay planted and no action is made faster.
- **Spot dodge**: an immediate duck/sidestep pose holds through the dodge, then returns to the stance before the state ends. Previously the fighter froze in a leaning pose for the whole dodge.
- **Airdodge**: folds from the airborne pose into `curl` over three frames and unfolds over the last three. Directional airdodges keep the unfolded pose until their longer state ends.
- **Run**: added a two-pixel torso bob (highest over the passing leg, lowest at each contact) and a matching shoulder drop. Stride timing and foot contacts are unchanged; both feet still land at each contact key.
- **Crouch**: the breathing loop now includes a slight hip/forward-hand shift in addition to the head and shoulder bob. Feet remain planted.
- **Helpless**: a slow one-second limp sway replaces the static fall pose. It is cosmetic and does not imply control.
- **Hitstun**: switched from a world-clock flail loop to a state-clock recoil. The first frame is the largest lean-back, the body settles over the next two frames, and it recovers toward `stand` by frame 24. State timing means hitlag now holds the recoil instead of wobbling during freeze, and every new hit restarts the visible reaction. Tumble still uses its spin animation for launches above the tumble threshold.

## Deliberate engine gaps and later gait clips

These states are not visually distinguishable from data for packages without dedicated clips:

- **Walk** reuses `idle` unless the package authors a `walk` state animation. `state_anim` selects `run` above
  `gait.poseRunSpeed`.
- **Crouch crawl** reuses the crouch breathing clip unless the package authors `crawl`; moving while crouched
  then selects `crawl`.
- **Turnaround and braking** keep showing `run` until speed falls below the run-pose threshold, then snap to
  `idle`, unless the package authors `turn` / `brake`. Those two clips sample the gait clock (`gait_t`), which
  resets on the gait change, so they enter at their first key instead of inheriting an arbitrary idle state time.

The optional `walk`, `crawl`, `turn` and `brake` clips added for Piranha Plant use this selector; packages that
do not define them (the original roster) keep the previous fallback exactly.
- **State animation changes do not cross-fade**. Rise → fall at the apex, landing → idle and shield-drop → idle are direct clip swaps. The data was brought closer at those boundaries, but a presentation-only blend over a few ticks would remove the remaining pop.
- **Tumble spin is not reset when a new tumble starts**. `Fighter::spin` accumulates and is reset only on respawn, so a fresh launch can begin the tumble clip part-way around. Resetting it on entering tumble would make the spin start consistently; it changes snapshot state, so it needs the normal `SIM_VERSION`/replay review.

## Checks

`crates/sim/tests/animation_movement.rs` drives a real jump press and samples the three jumpsquat frames, the landing recovery, run contacts, crouch breathing, spot dodge, hitstun and airdodge. It asserts grounded feet stay at `y = 0`, jumpsquat compresses then extends, the run body rises over the passing leg with both feet down at contact, and hitstun is independent of world-frame phase. Focused `pose`, `crouch`, `feel` and `content::all_assets_load` tests also pass. The asset still needs the normal `just bless` canonical formatting and the usual rendered game check.

## Review corrections

The landing near-neutral key is at tick 2 (the final visible tick of a three-tick hard landing), spot dodge ducks on acceptance and finishes at tick 25, and neutral airdodge finishes unfolding at tick 29. The movement tests drive actual exits, both airdodge variants, and early landing rather than sampling poses after the state has ended.
