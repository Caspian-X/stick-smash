# Shared responsiveness and launch tuning

The roster uses one Ultimate knockback equation, including original fighters whose hitboxes were migrated and subsequently balanced. There is no per-fighter formula switch. Imports and originals still have different authored growth and damage; this pass preserves those differences.

The shared launch conversion was 0.105 pixels/tick per reference unit. At 200% the median authored non-fixed hitbox launch ranged from roughly 10–24 pixels/tick across packages (a content audit, not weighted by move usage). Horizontal hitstun drag of 0.97 further attenuated travel at the time (since replaced by the linear launch decay of `hitstun.ultimate`, see FORMAT.md, which lets the same launches travel further). Raising launchSpeed to 0.14 increases launch speeds by one third without changing damage, angles, relative growth or fighter weight. Fixed hits also use that conversion. This needs human testing for multi-hit retention and recovery difficulty; launch measurements alone do not establish KO percentages or competitive balance.

Bundled movement now uses three jumpsquat frames (was four), four turnaround frames (was nine), five braking frames (was eight), and walking acceleration equal to run acceleration (was 0.7 times). Initial dash no longer blocks shield or standing commands. The optional dashLock mechanic remains available and covered by explicit opt-in tests. Top speeds, gravity, air acceleration, fighter stats and move startup/recovery remain authored per character.

`feel::roster_raw_inputs_move_jump_and_shield_without_dash_delay` drives actual raw stick/button inputs for every playable fighter, requiring first-tick displacement, next-tick shield and takeoff after three startup ticks. `knockback::roster_finishers_launch_at_multiple_percents_through_real_hit_application` applies each fighter's strongest smash hit through combat at 0/100/200/400%, checking increasing launch, a 200% speed floor, and proportional percent scaling. Measured 200% launch speeds were 35–56 pixels/tick in the shared checkout. Concurrent fighter edits may change those values.

This pass changes bundled content only, so it does not require a SIM_VERSION change: old replay content retains its original settings. TS traces explicitly restore four-frame jumpsquat and disable gait/Ultimate knockback. The linear-content fixture restores damage as well as BKB/growth, keeping later damage edits out of the historical launch contract.

Momentum then moved to the subtractive `movement.ultimate` model (FORMAT.md, Momentum): traction 0.35 px/frame²
(Ultimate Mario 0.1 × 3.5, doubled above walk speed; 0.5 = 0.1 × 5 since the unit unification), air friction 0.035
(now 0.05), fighter-relative air control with
per-kind multipliers (aerial 1, other attacks 0.55, up special and helpless 0.65), instant fast fall and dash
bursts easing into the run. No fighter sets its own `traction` or `airFriction` yet: per-character values from
references are still to be gathered. Phase `airAccelMult` values were migrated to keep each phase's previous
effective acceleration (Mike's 0.389 aerial correction became the default 1); phases without one now use the
fighter's own air accel, which slows heavy fighters' aerial drift (King 0.175 instead of the shared 0.45). Mike's
Quickdraw rush speed was refitted so its requested ground distances survive slide traction. This is a SIM_VERSION
change (-sim-30); TS traces disable the block.

The 5 px/unit unification (DESIGN.md, Units) then put all content on one scale: shared horizontal speeds and
accelerations ×5/3.5 (dodge roll 7 → 10, directional air dodge 10 → 14.29, dash attack/grab slides, shield push,
trip slide, drift in hitstun), shared vertical speeds ×5/6, the ledge jump and shield-break launch ×√(5/6) (15 → 13.69,
same height under the lower gravity) and `launchSpeed` 0.14 → 0.15 (Ultimate's 0.03 × 5). The hitstun particle and audio
strength thresholds follow launch speed (×15/14). Legacy TS-trace fields were left alone. Content-only:
no SIM_VERSION change.
