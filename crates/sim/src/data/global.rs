//! `global.json`: every number that isn't specific to one fighter, move or stage.
//! Units: frames (60 per second), stage pixels, pixels/frame for speeds.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::Color;
use super::hitbox::Hitbox;
use super::unit;

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Global {
    #[serde(rename = "$schema", default, skip_serializing)]
    pub schema: Option<String>,
    #[schemars(transform = unit::none)]
    pub schema_version: u32,
    pub movement: Movement,
    pub input: InputRules,
    /// Walk / dash / run on the ground. Off = the reference single accelerate-to-run gait.
    #[serde(default)]
    pub gait: Gait,
    pub sim: SimRules,
    pub ai: AiRules,
    pub knockback: Knockback,
    pub hitstun: Hitstun,
    pub hitlag: Hitlag,
    pub hit_types: HitTypes,
    pub shield: Shield,
    pub dodge: Dodge,
    pub landing: Landing,
    pub tech: Tech,
    pub knockdown: Knockdown,
    pub ledge: Ledge,
    #[serde(default)]
    pub parry: Parry,
    /// Directional influence: bend a launch toward the stick held when hitlag ends.
    #[serde(default)]
    pub di: Di,
    pub grab: Grab,
    pub respawn: Respawn,
    #[serde(rename = "match")]
    pub match_: Match,
    /// Player colors by slot (8 slots).
    pub palette: Vec<PlayerColors>,
    /// Renderer tuning. Cosmetic only.
    pub render: super::render::Render,
    /// Procedural sound bank and game-side mixing. The sim never reads it.
    pub audio: super::audio::Audio,
    #[serde(default)]
    pub replay: Replay,
    /// Held items: throw timing, pickup and catch reach, loose-item landing, CPU use.
    #[serde(default)]
    pub items: Items,
}

/// Shared movement rules. Per-fighter speeds and gravity live in each fighter's `stats`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Movement {
    /// Frames crouching before takeoff. Jump still held on the last one = full hop, released = short hop.
    #[schemars(transform = unit::tick)]
    pub jumpsquat: u32,
    /// Legacy: air drift when no direction is held (multiplier per frame).
    #[schemars(transform = unit::ratio)]
    pub air_drag: f64,
    /// Legacy: air drift multiplier when a direction is held but already at max speed.
    #[schemars(transform = unit::ratio)]
    pub air_drag_held: f64,
    /// Fast fall allowed once vertical speed is above this (negative = still slightly rising; y down).
    #[schemars(transform = unit::v_y)]
    pub fast_fall_min_vy: f64,
    /// Legacy: downward speed applied the frame you fast fall (`ultimate` sets the fighter's fastFallSpeed at once).
    #[schemars(transform = unit::v_y)]
    pub fast_fall_kick: f64,
    /// Legacy: air accel for moves with `air: "attack"`.
    #[schemars(transform = unit::a_x)]
    pub attack_air_accel: f64,
    /// Legacy: air accel for moves with `air: "aerial"`.
    #[schemars(transform = unit::a_x)]
    pub aerial_accel: f64,
    /// Legacy: air accel in helpless fall and air dodges.
    #[schemars(transform = unit::a_x)]
    pub helpless_accel: f64,
    /// Legacy: air accel for moves with `air: "upSpecial"`.
    #[schemars(transform = unit::a_x)]
    pub up_special_accel: f64,
    /// Legacy: ground friction multiplier during attacks; `slide` frames use attackSlideFriction.
    #[schemars(transform = unit::ratio)]
    pub attack_friction: f64,
    #[schemars(transform = unit::ratio)]
    pub attack_slide_friction: f64,
    /// Legacy: air drift multiplier during attacks when not accelerating.
    #[schemars(transform = unit::ratio)]
    pub attack_air_drag: f64,
    /// Frames platforms are ignored after dropping through.
    #[schemars(transform = unit::tick)]
    pub platform_drop: u32,
    /// Held down passes through platforms in ordinary fall; a platform drop consumes its down gesture.
    #[serde(default)]
    pub platform_pass: bool,
    /// Fighters overlapping closer than this push apart.
    #[schemars(transform = unit::px_x)]
    pub push_radius: f64,
    #[schemars(transform = unit::ratio)]
    pub push_strength: f64,
    /// Ground speed needed for attack → dash attack and grab → dash grab.
    #[schemars(transform = unit::v_x)]
    pub dash_attack_speed: f64,
    #[schemars(transform = unit::v_x)]
    pub dash_grab_speed: f64,
    /// Frames a direction press counts as a "tap" for smash attacks, rolls and spot dodges.
    #[schemars(transform = unit::tick)]
    pub tap_window: u32,
    /// Frames after a `bReverse` move starts in which pressing the other direction turns it around.
    #[schemars(transform = unit::tick)]
    pub b_reverse_window: u32,
    /// Subtractive momentum (traction, air friction, fighter-relative air control, instant fast fall). Absent = the
    /// legacy multipliers and shared accelerations marked Legacy (TS compatibility), which it replaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultimate: Option<UltimateMovement>,
}

/// Ultimate-style momentum: speeds change by fixed amounts per frame instead of by multipliers. Rates are px/frame²
/// (Ultimate units × 5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UltimateMovement {
    /// Default `stats.traction`: grounded slowdown toward zero in idle, crouch, brake, turn skid, jumpsquat, landing
    /// lag, shield, grabs, throws and grounded attacks (Ultimate Mario 0.1 → 0.5).
    #[schemars(transform = unit::a_x)]
    pub traction: f64,
    /// Traction multiplier while faster than the fighter's walk speed, so a run skids to a stop in a readable time.
    #[schemars(transform = unit::ratio)]
    pub fast_traction: f64,
    /// Traction multiplier during a grounded attack phase's `slide` frames (dash attacks).
    #[schemars(transform = unit::ratio)]
    pub attack_slide_traction: f64,
    /// Default `stats.airFriction`: airborne slowdown with the stick neutral (toward zero) or above the held air
    /// speed (toward it; overspeed is never clamped). Ultimate ≈ 0.01 → 0.05.
    #[schemars(transform = unit::a_x)]
    pub air_friction: f64,
    /// `stats.airAccel` multiplier for moves with `air: "aerial"`; phase `airAccelMult` multiplies on top.
    #[schemars(transform = unit::ratio)]
    pub aerial_accel_mult: f64,
    /// `stats.airAccel` multiplier for moves with `air: "attack"`.
    #[schemars(transform = unit::ratio)]
    pub attack_air_accel_mult: f64,
    /// `stats.airAccel` multiplier for moves with `air: "upSpecial"`.
    #[schemars(transform = unit::ratio)]
    pub up_special_accel_mult: f64,
    /// `stats.airAccel` multiplier in helpless fall and air dodges.
    #[schemars(transform = unit::ratio)]
    pub helpless_accel_mult: f64,
}

/// Shared damage-dependent knockback. Omitting ultimate retains the original TS compatibility formula.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Knockback {
    /// Shared by every fighter, throw and projectile. None is the legacy TS compatibility mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultimate: Option<UltimateKnockback>,
    #[schemars(transform = unit::ratio)]
    pub kb_percent_scale: f64,
    /// Launch speed (px/tick) above which the victim tumbles (can tech, lands in knockdown). Replaced by
    /// `hitstun.ultimate.tumbleThreshold` when that model is on.
    #[schemars(transform = unit::v_xy)]
    pub tumble_threshold: f64,
    /// Hitting a grounded victim downward bounces them up at this fraction.
    #[schemars(transform = unit::ratio)]
    pub ground_bounce: f64,
    /// Charge: damage × (1 + charge / chargeDamageDivisor).
    #[schemars(transform = unit::tick)]
    pub charge_damage_divisor: f64,
    /// Fraction of the charge damage bonus that also applies to base knockback.
    #[schemars(transform = unit::ratio)]
    pub charge_kb_ratio: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UltimateKnockback {
    #[schemars(transform = unit::ratio)]
    pub percent_scale: f64,
    #[schemars(transform = unit::ratio)]
    pub damage_scale: f64,
    #[schemars(transform = unit::ratio)]
    pub weight_scale: f64,
    #[schemars(transform = unit::kb)]
    pub base: f64,
    /// Reference knockback to pixels/tick (reference launch speed × world scale).
    #[schemars(transform = unit::px_kb)]
    pub launch_speed: f64,
}

/// Legacy model: hitstun = floor(launch px × perKnockback) + base, then per-hitbox hitstunMult/hitstunAdd; launches
/// fly under the shared gravity/maxFall with multiplicative airDrag. `ultimate` replaces all of that.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hitstun {
    #[schemars(transform = unit::tick2_px)]
    pub per_knockback: f64,
    #[schemars(transform = unit::tick)]
    pub base: f64,
    #[schemars(transform = unit::a_y)]
    pub gravity: f64,
    #[schemars(transform = unit::v_y)]
    pub max_fall: f64,
    #[schemars(transform = unit::ratio)]
    pub air_drag: f64,
    /// Velocity multiplier per frame while in hitstun on the ground (both models).
    #[schemars(transform = unit::ratio)]
    pub ground_drag: f64,
    /// Horizontal drift you can add while in hitstun (per frame, both models).
    #[schemars(transform = unit::a_x)]
    pub drift_accel: f64,
    /// Ultimate-style decaying launch. Absent = the legacy model above (TS compatibility).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultimate: Option<UltimateHitstun>,
}

/// The launch is a separate vector that shrinks linearly to zero; the victim's own gravity, fall speed and drift
/// act on a second component. Knockback values here are reference units (launch px / `knockback.ultimate.launchSpeed`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UltimateHitstun {
    /// Hitstun = floor(reference knockback × perReferenceKnockback) + base, then hitstunMult/hitstunAdd (Ultimate 0.4).
    #[schemars(transform = unit::tick)]
    pub per_reference_knockback: f64,
    #[schemars(transform = unit::tick)]
    pub base: f64,
    /// Launch speed lost per frame, in reference knockback units (Ultimate: 0.051 decay / 0.03 speed per unit = 1.7).
    #[schemars(transform = unit::kb_tick)]
    pub launch_decay: f64,
    /// Reference knockback above which the victim tumbles (Ultimate 80).
    #[schemars(transform = unit::kb)]
    pub tumble_threshold: f64,
    /// Out of tumble, allow an air dodge / aerial after this many frames of hitstun. Absent = never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel: Option<HitstunCancel>,
    /// Ultimate's upward launch physics (`damage_fly_top_*`). Absent = the victim's own gravity and fall speed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_launch: Option<TopLaunch>,
}

/// A tumbling launch whose angle is within `minAngle..=maxAngle` (70–110 in Ultimate) falls with gravity
/// `gravity + gravityPerWeight × source weight` capped at `fallSpeed` while in hitstun, instead of the victim's own;
/// after hitstun a fall faster than the fighter's own slows back to it by its gravity per frame.
/// Ultimate: 0.042 + 0.000336 × weight units/frame², fall speed 1.8 (× 5 here).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopLaunch {
    #[schemars(transform = unit::deg)]
    pub min_angle: f64,
    #[schemars(transform = unit::deg)]
    pub max_angle: f64,
    #[schemars(transform = unit::a_y)]
    pub gravity: f64,
    /// Per unit of source weight (100 / `stats.weight`).
    #[schemars(transform = unit::a_y)]
    pub gravity_per_weight: f64,
    #[schemars(transform = unit::v_y)]
    pub fall_speed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HitstunCancel {
    #[schemars(transform = unit::tick)]
    pub air_dodge_after: u32,
    #[schemars(transform = unit::tick)]
    pub aerial_after: u32,
}

/// Hitlag (freeze frames for both fighters) = min(max, floor(base + damage × perDamage + knockback × perKnockback)),
/// then × the hitbox's hitlagMult (floored). With `multBeforeCap` (Ultimate) it is
/// min(max, floor((base + damage × perDamage + knockback × perKnockback) × hitlagMult)) instead.
/// Ultimate: base 6, perDamage 0.65, perKnockback 0, max 30.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hitlag {
    #[schemars(transform = unit::tick)]
    pub base: f64,
    #[schemars(transform = unit::tick)]
    pub per_damage: f64,
    #[schemars(transform = unit::tick2_px)]
    pub per_knockback: f64,
    #[schemars(transform = unit::tick)]
    pub max: f64,
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub mult_before_cap: bool,
}

/// Rules for each hitbox `type`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HitTypes {
    pub trip: TripRule,
    pub sakurai: SakuraiRule,
    pub autolink: AutolinkRule,
    pub windbox: WindboxRule,
    pub electric: ElectricRule,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paralyze: Option<ParalyzeRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bury: Option<BuryRule>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParalyzeRule {
    #[schemars(transform = unit::tick)]
    pub max_frames: u32,
    #[schemars(transform = unit::tick)]
    pub extra_frames: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuryRule {
    #[schemars(transform = unit::tick)]
    pub base_frames: f64,
    #[schemars(transform = unit::tick)]
    pub per_percent: f64,
    #[schemars(transform = unit::tick)]
    pub per_knockback: f64,
    #[schemars(transform = unit::tick)]
    pub max_frames: f64,
    #[schemars(transform = unit::tick)]
    pub mash: f64,
    #[schemars(transform = unit::tick)]
    pub button_mash: f64,
    #[schemars(transform = unit::kb)]
    pub release_threshold: f64,
}

/// Grounded victim knocked prone: slide speed away from the attacker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TripRule {
    #[schemars(transform = unit::v_x)]
    pub slide: f64,
}

/// Auto angle: grounded hits below tumble knockback use groundLow, stronger grounded hits groundHigh, airborne hits air.
/// With `ramp: [from, to]` (reference knockback) the grounded angle instead rises linearly from groundLow at `from` to
/// groundHigh at `to` (Ultimate: 0° below 60, 38° from 88, 38° in the air).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SakuraiRule {
    #[schemars(transform = unit::deg)]
    pub ground_low: f64,
    #[schemars(transform = unit::deg)]
    pub ground_high: f64,
    #[schemars(transform = unit::deg)]
    pub air: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::kb)]
    pub ramp: Option<[f64; 2]>,
}

/// Victim velocity = (hitbox center − victim center) × pull + attacker velocity × momentum.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutolinkRule {
    #[schemars(transform = unit::ratio)]
    pub pull: f64,
    #[schemars(transform = unit::ratio)]
    pub momentum: f64,
}

/// Push per frame = bkb × push × victim weight, capped at maxSpeed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindboxRule {
    #[schemars(transform = unit::a_xy)]
    pub push: f64,
    #[schemars(transform = unit::v_xy)]
    pub max_speed: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElectricRule {
    /// Victim hitlag multiplier.
    #[schemars(transform = unit::ratio)]
    pub victim_hitlag_mult: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Shield {
    #[schemars(transform = unit::damage)]
    pub max: f64,
    /// Shield HP lost per frame while held.
    #[schemars(transform = unit::damage)]
    pub decay: f64,
    /// Shield HP regained per frame while not shielding.
    #[schemars(transform = unit::damage)]
    pub regen: f64,
    /// Shield HP lost = damage × damageMult × hitbox.shieldDmgMult.
    #[schemars(transform = unit::ratio)]
    pub damage_mult: f64,
    /// Shield stun = floor(damage × stunPerDamage + stunBase).
    #[schemars(transform = unit::tick)]
    pub stun_per_damage: f64,
    #[schemars(transform = unit::tick)]
    pub stun_base: f64,
    /// Shield hitlag = floor(hitlagBase + damage × hitlagPerDamage).
    #[schemars(transform = unit::tick)]
    pub hitlag_base: f64,
    #[schemars(transform = unit::tick)]
    pub hitlag_per_damage: f64,
    /// Defender slide = damage × pushPerDamage + pushBase.
    #[schemars(transform = unit::v_x)]
    pub push_per_damage: f64,
    #[schemars(transform = unit::v_x)]
    pub push_base: f64,
    /// Attacker's own pushback on a shielded grounded hit.
    #[schemars(transform = unit::v_x)]
    pub attacker_pushback: f64,
    /// Frames of lag after releasing shield (Ultimate 11).
    #[schemars(transform = unit::tick)]
    pub drop_lag: u32,
    /// Shield + up attack (or attack with up held) is an up smash rather than a grab. Off for TS fidelity.
    #[serde(default)]
    pub up_smash: bool,
    #[schemars(transform = unit::v_y)]
    pub break_launch: f64,
    #[schemars(transform = unit::a_y)]
    pub break_gravity: f64,
    #[schemars(transform = unit::v_y)]
    pub break_max_fall: f64,
    /// Dizzy duration = max(dizzyMin, dizzyBase − percent × dizzyPerPercent).
    #[schemars(transform = unit::tick)]
    pub dizzy_base: f64,
    #[schemars(transform = unit::tick)]
    pub dizzy_min: f64,
    #[schemars(transform = unit::tick)]
    pub dizzy_per_percent: f64,
    /// Frames removed from dizzy per button press.
    #[schemars(transform = unit::tick)]
    pub dizzy_mash: f64,
    /// Shield HP after recovering from dizzy.
    #[serde(rename = "dizzyRecoverHP")]
    #[schemars(transform = unit::damage)]
    pub dizzy_recover_hp: f64,
}

/// Dodge timelines. Frame numbers are the state clock: 0 is the tick the dodge starts (Ultimate's frame 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Dodge {
    pub roll: Window,
    pub spot: Window,
    /// Neutral air dodge.
    pub air: Window,
    /// Directional air dodge.
    pub dir_air: Window,
    /// Repeated rolls, spot dodges and air dodges get less intangibility and more frames. Absent = off.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub staling: DodgeStaling,
}

/// One timed defensive state. Frames count from 0 on the entry tick; the state ends when its clock reaches `frames`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Window {
    #[schemars(transform = unit::tick)]
    pub frames: u32,
    /// First and last intangible frame (inclusive). Start 0 replaces any current invulnerability on the entry
    /// tick; a later start adds it on that frame (leaving the state first cancels it). End below start = none; an
    /// end past `frames` carries over into the next state.
    #[schemars(transform = unit::tick)]
    pub invuln_start: u32,
    #[schemars(transform = unit::tick)]
    pub invuln_end: u32,
    /// Roll: frames moving at `speed` (inclusive).
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    #[schemars(transform = unit::tick)]
    pub move_start: u32,
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    #[schemars(transform = unit::tick)]
    pub move_end: u32,
    /// Roll ground speed / directional air dodge launch speed.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    #[schemars(transform = unit::v_xy)]
    pub speed: f64,
    /// Directional air dodge: velocity × decay each frame below `decayFrames`, then helpless drift.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    #[schemars(transform = unit::ratio)]
    pub decay: f64,
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    #[schemars(transform = unit::tick)]
    pub decay_frames: u32,
    /// Air dodges: landing lag when touching down during the dodge.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    #[schemars(transform = unit::tick)]
    pub landing_lag: u32,
}

impl Window {
    /// Intangible frame count before staling.
    pub fn invuln_len(&self) -> u32 {
        span(self.invuln_start, self.invuln_end)
    }
}

/// Frames in the inclusive window `start..=end` (0 when end < start).
pub fn span(start: u32, end: u32) -> u32 {
    end.saturating_add(1).saturating_sub(start)
}

/// Each roll, spot dodge or air dodge started within `window` ticks of the previous one goes one step further down
/// `steps` (the first repeat uses `steps[0]`, capped at the last); a longer gap starts fresh. Getup and tech rolls
/// neither stale nor count. Empty `steps` = off.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct DodgeStaling {
    #[schemars(transform = unit::tick)]
    pub window: u32,
    pub steps: Vec<StaleStep>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaleStep {
    /// Intangible frames × this (rounded; the start frame stays).
    #[schemars(transform = unit::ratio)]
    pub invuln: f64,
    /// Total frames × this (rounded).
    #[schemars(transform = unit::ratio)]
    pub frames: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Landing {
    /// Landing lag from a normal fall faster than softLandSpeed.
    #[schemars(transform = unit::tick)]
    pub hard: u32,
    #[schemars(transform = unit::v_y)]
    pub soft_land_speed: f64,
    #[schemars(transform = unit::tick)]
    pub helpless: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tech {
    /// Press shield within this many frames before landing in tumble to tech.
    #[schemars(transform = unit::tick)]
    pub window: u32,
    /// First and last intangible frame of a tech in place (state clock as in `dodge.roll`; 0 = the landing tick).
    #[schemars(transform = unit::tick)]
    pub invuln_start: u32,
    #[schemars(transform = unit::tick)]
    pub invuln_end: u32,
    #[schemars(transform = unit::tick)]
    pub frames: u32,
    /// A shield press in hitstun that doesn't tech blocks further tech presses for this many frames (Ultimate 40),
    /// so mashing cannot tech. 0 = off.
    #[serde(default)]
    #[schemars(transform = unit::tick)]
    pub lockout: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Knockdown {
    /// Frames lying down before you can act.
    #[schemars(transform = unit::tick)]
    pub min_frames: u32,
    /// Automatically stand up at this frame.
    #[schemars(transform = unit::tick)]
    pub auto_getup: u32,
    /// Neutral getup: `frames` and intangibility as in `dodge.roll`.
    pub getup: Window,
    pub getup_attack: Window,
    /// Getup attack active frames (inclusive).
    #[schemars(transform = unit::tick)]
    pub attack_active: [u32; 2],
    pub attack: Vec<Hitbox>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ledge {
    #[schemars(transform = unit::tick)]
    pub invuln: u32,
    /// Frames hanging before you can act.
    #[schemars(transform = unit::tick)]
    pub min_hang: u32,
    /// Auto-drop after hanging this long.
    #[schemars(transform = unit::tick)]
    pub max_hang: u32,
    #[schemars(transform = unit::tick)]
    pub regrab_cooldown: u32,
    #[schemars(transform = unit::v_y)]
    pub jump_speed: f64,
    pub climb: LedgeOption,
    pub attack: LedgeOption,
    pub roll: LedgeOption,
    pub jump: LedgeOption,
    /// Ledge attack active frames (inclusive).
    #[schemars(transform = unit::tick)]
    pub attack_active: [u32; 2],
    pub attack_hit: Vec<Hitbox>,
    /// Grabbing an occupied hanging ledge knocks its holder off. Absent = occupied ledges can't be grabbed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trump: Option<Trump>,
    /// Ledge grab invulnerability only after landing, being hit or respawning since the last grab.
    #[serde(default)]
    pub fresh_invuln: bool,
}

/// The fighter knocked off a ledge: pushed away from the stage (and up), unable to regrab for a moment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Trump {
    #[schemars(transform = unit::v_x)]
    pub push_x: f64,
    #[schemars(transform = unit::v_y)]
    pub push_y: f64,
    #[schemars(transform = unit::tick)]
    pub regrab_cooldown: u32,
    /// The holder must have hung at least this many frames, so simultaneous arrivals don't trade.
    #[schemars(transform = unit::tick)]
    pub min_hang: u32,
}

/// Releasing shield opens a parry window: a hit landing in it does nothing, freezes the attacker and frees the
/// defender. `window` 0 disables it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Parry {
    /// Frames after releasing shield (the release frame is the first).
    #[schemars(transform = unit::tick)]
    pub window: u32,
    /// Freeze for the defender (attacker gets this plus attackerFreeze).
    #[schemars(transform = unit::tick)]
    pub freeze: u32,
    #[schemars(transform = unit::tick)]
    pub attacker_freeze: u32,
    /// Invulnerability after a parry, so the rest of a multi-hit move whiffs.
    #[schemars(transform = unit::tick)]
    pub invuln: u32,
}

/// The launch is rotated toward the stick by up to maxAngle degrees, scaled by how perpendicular the stick is;
/// speed is unchanged. 0 disables it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Di {
    #[schemars(transform = unit::deg)]
    pub max_angle: f64,
    /// Minimum stick magnitude (0–1) that counts.
    #[schemars(transform = unit::ratio)]
    pub min_stick: f64,
}

/// Ground gait. Speeds are fractions of the fighter's runSpeed (walkSpeed may be overridden per fighter).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Gait {
    /// Speed selecting the run animation, including combat joint sampling.
    #[serde(default = "default_pose_run_speed")]
    #[schemars(transform = unit::v_x)]
    pub pose_run_speed: f64,
    pub enabled: bool,
    /// Full-tilt walk speed; a partial tilt walks proportionally slower.
    #[schemars(transform = unit::ratio)]
    pub walk_speed: f64,
    /// Walk acceleration as a fraction of runAccel.
    #[schemars(transform = unit::ratio)]
    pub walk_accel: f64,
    /// Walking promotes to running at this deadzoned horizontal tilt (0..1); zero requires a flick.
    #[schemars(transform = unit::ratio)]
    pub run_threshold: f64,
    /// Running returns to walking below this tilt (hysteresis; <= runThreshold).
    #[schemars(transform = unit::ratio)]
    pub walk_threshold: f64,
    /// Initial dash restricts standing attacks and shield until it ends; jump, dash attack, up smash,
    /// grab and specials can cancel it.
    pub dash_lock: bool,
    /// Releasing the stick during the initial dash skids by traction at once (Ultimate's dash: a tapped flick is a
    /// short stutter step) instead of holding dash speed for dashFrames; holding again re-accelerates by runAccel.
    pub dash_release: bool,
    /// A flick starts a dash at this speed, held for dashFrames; flick back to dash the other way.
    #[schemars(transform = unit::ratio)]
    pub dash_speed: f64,
    #[schemars(transform = unit::tick)]
    pub dash_frames: u32,
    /// Dash-back turn window (frames of the dash shown as the `dashTurn` gait; `stats.dashTurnFrames` overrides).
    #[schemars(transform = unit::tick)]
    pub dash_turn_frames: u32,
    /// Backward body lean at the start of a dash-back turn, easing out over the window (figures without a `turn`
    /// clip).
    #[schemars(transform = unit::deg)]
    pub dash_turn_lean: f64,
    /// Holding the other way while running turns around: facing flips at once and speed skids with turnFriction
    /// (the fighter's traction with `movement.ultimate`) for turnFrames (`stats.turnFrames` overrides), then the
    /// run accelerates the new way.
    #[schemars(transform = unit::tick)]
    pub turn_frames: u32,
    #[schemars(transform = unit::ratio)]
    pub turn_friction: f64,
    /// Letting go while running skids for brakeFrames (`stats.brakeFrames` overrides) with brakeFriction (traction
    /// with `movement.ultimate`).
    #[schemars(transform = unit::tick)]
    pub brake_frames: u32,
    #[schemars(transform = unit::ratio)]
    pub brake_friction: f64,
    /// Frames to lower into a crouch or rise back to idle. Zero switches immediately.
    #[schemars(transform = unit::tick)]
    pub crouch_frames: u32,
    /// Crouched movement as a fraction of walk speed; zero disables crawling.
    #[schemars(transform = unit::ratio)]
    pub crawl_speed: f64,
}

impl Default for Gait {
    fn default() -> Self {
        Self {
            pose_run_speed: default_pose_run_speed(),
            enabled: false,
            walk_speed: 0.,
            walk_accel: 0.,
            run_threshold: 0.,
            walk_threshold: 0.,
            dash_lock: false,
            dash_release: false,
            dash_speed: 0.,
            dash_frames: 0,
            dash_turn_frames: 0,
            dash_turn_lean: 0.,
            turn_frames: 0,
            turn_friction: 0.,
            brake_frames: 0,
            brake_friction: 0.,
            crouch_frames: 0,
            crawl_speed: 0.,
        }
    }
}

/// Ledge getup timing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgeOption {
    /// Frames to pull up.
    #[schemars(transform = unit::tick)]
    pub a: u32,
    /// Frame movement ends.
    #[schemars(transform = unit::tick)]
    pub b: u32,
    /// Actionable frame.
    #[schemars(transform = unit::tick)]
    pub end: u32,
    /// Slide distance onto the stage.
    #[schemars(transform = unit::px_x)]
    pub dist: f64,
    /// First and last intangible frame (state clock, 0 = the tick the option starts; start 0 keeps the longer of
    /// this and any current invulnerability).
    #[schemars(transform = unit::tick)]
    pub invuln_start: u32,
    #[schemars(transform = unit::tick)]
    pub invuln_end: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Grab {
    /// Hold time = holdBase + victimPercent × holdPerPercent.
    #[schemars(transform = unit::tick)]
    pub hold_base: f64,
    #[schemars(transform = unit::tick)]
    pub hold_per_percent: f64,
    /// Hold frames removed per victim button press.
    #[schemars(transform = unit::tick)]
    pub mash_per_press: f64,
    /// Frames before the grabber can pummel or throw.
    #[schemars(transform = unit::tick)]
    pub min_hold: u32,
    #[schemars(transform = unit::tick)]
    pub pummel_frames: u32,
    #[schemars(transform = unit::tick)]
    pub pummel_hit_frame: u32,
    #[schemars(transform = unit::damage)]
    pub pummel_damage: f64,
    #[schemars(transform = unit::tick)]
    pub pummel_hitlag: u32,
    /// Vertical tolerance for grab boxes.
    #[schemars(transform = unit::px_y)]
    pub height_tolerance: f64,
    #[schemars(transform = unit::tick)]
    pub release_lag: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Respawn {
    #[schemars(transform = unit::tick)]
    pub dead_frames: u32,
    #[schemars(transform = unit::tick)]
    pub descend_frames: u32,
    #[schemars(transform = unit::tick)]
    pub max_wait: u32,
    #[schemars(transform = unit::tick)]
    pub invuln: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Match {
    #[schemars(transform = unit::tick)]
    pub countdown: u32,
    #[schemars(transform = unit::tick)]
    pub go_frames: u32,
    /// Ticks the match stays frozen after its last KO ("GAME!") before the winner poses. Keep it short: nothing
    /// happens in it but the KO effects, and the end slow motion stretches it in real time.
    #[schemars(transform = unit::tick)]
    pub end_frames: u32,
    /// The timer shows its warning style and beeps once a second for its last this many seconds.
    #[serde(default = "default_timer_warn_seconds")]
    #[schemars(transform = unit::s)]
    pub timer_warn_seconds: u32,
    /// Damage each fighter starts sudden death with.
    #[serde(default = "default_sudden_death_damage")]
    #[schemars(transform = unit::damage)]
    pub sudden_death_damage: f64,
}
fn default_timer_warn_seconds() -> u32 {
    10
}
fn default_sudden_death_damage() -> f64 {
    300.
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerColors {
    pub main: Color,
    pub dark: Color,
    pub light: Color,
}

/// Stick interpretation (stick units are -127..127); history is part of the rollback snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputRules {
    /// A direction counts as held (tilts, aerial directions, rolls) from here.
    #[schemars(transform = unit::none)]
    pub stick_threshold: i8,
    /// Analog movement (walking, air drift, DI) ignores the stick below this.
    #[serde(default)]
    #[schemars(transform = unit::none)]
    pub deadzone: i8,
    /// A flick (dash, smash, platform drop) reaches this within flickFrames of leaving the deadzone.
    #[serde(default = "default_flick")]
    #[schemars(transform = unit::none)]
    pub flick_threshold: i8,
    #[serde(default = "default_flick_frames")]
    #[schemars(transform = unit::tick)]
    pub flick_frames: u32,
    /// Human press buffer lifetime in ticks (inclusive); 0 disables re-delivery.
    #[schemars(transform = unit::tick)]
    pub buffer_frames: u32,
    /// A buffered press stays alive while its button is held (Ultimate's hold buffer), expiring `bufferFrames`
    /// after release. Off for TS fidelity.
    #[serde(default)]
    pub hold_buffer: bool,
    /// Buffered presses this many ticks apart combine (jump + attack, two jumps, attack + special).
    #[serde(default)]
    #[schemars(transform = unit::tick)]
    pub combo_frames: u32,
    /// Down pressed up to this many ticks before falling still fast-falls once descending (0 = same tick only).
    #[serde(default)]
    #[schemars(transform = unit::tick)]
    pub fast_fall_buffer: u32,
    /// Jump + attack and attacks during jumpsquat queue a short-hop aerial; a second jump press during
    /// jumpsquat short hops. Off for TS fidelity.
    #[serde(default = "default_true")]
    pub jump_aerials: bool,
}
fn default_flick() -> i8 {
    64
}
fn default_flick_frames() -> u32 {
    99
}
/// Shared simulation constants retained from the reference implementation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimRules {
    /// Keep effect ranges and charge particles from advancing gameplay randomness. Defaults to true for
    /// older user `global.json` overrides written before this field existed (the bundled value).
    #[serde(default = "default_true")]
    pub separate_cosmetic_rng: bool,
    /// Legacy velocity multipliers in jumpsquat, landing lag (also dizzy, grab hold, throw) and shield;
    /// `movement.ultimate` uses the fighter's traction instead.
    #[schemars(transform = unit::ratio)]
    pub jumpsquat_friction: f64,
    #[schemars(transform = unit::ratio)]
    pub landing_friction: f64,
    #[schemars(transform = unit::ratio)]
    pub shield_friction: f64,
    #[schemars(transform = unit::ratio)]
    pub dodge_friction: f64,
    #[schemars(transform = unit::ratio)]
    pub shieldbreak_friction: f64,
    #[schemars(transform = unit::ratio)]
    pub knockdown_friction: f64,
    #[schemars(transform = unit::px_y)]
    pub push_height: f64,
    #[schemars(transform = unit::px_x)]
    pub main_pad: f64,
    #[schemars(transform = unit::px_x)]
    pub platform_pad: f64,
    #[schemars(transform = unit::px_y)]
    pub surface_tolerance: f64,
    /// Minimum downward landing speed to tech or enter knockdown from tumble.
    #[schemars(transform = unit::v_y)]
    pub tech_min_speed: f64,
    /// Speed above which hitstun emits alternate-frame trail particles.
    #[schemars(transform = unit::v_xy)]
    pub hitstun_particle_speed: f64,
    #[schemars(transform = unit::ratio)]
    pub knockdown_drag: f64,
    #[schemars(transform = unit::tick)]
    pub flip_frames: u32,
    #[schemars(transform = unit::v_y)]
    pub ledge_drop_speed: f64,
    #[schemars(transform = unit::px_x)]
    pub ledge_drop_offset: f64,
    #[schemars(transform = unit::px_x)]
    pub ledge_pull_in: f64,
    #[schemars(transform = unit::px_y)]
    pub ledge_jump_offset: f64,
    #[schemars(transform = unit::v_x)]
    pub ledge_jump_drift: f64,
    #[schemars(transform = unit::tick)]
    pub ledge_jump_cooldown: u32,
    #[schemars(transform = unit::px_x)]
    pub grab_offset: f64,
    #[schemars(transform = unit::v_x)]
    pub release_vx: f64,
    #[schemars(transform = unit::v_y)]
    pub release_vy: f64,
    #[schemars(transform = unit::v_x)]
    pub escape_attacker_vx: f64,
    #[schemars(transform = unit::v_x)]
    pub escape_vx: f64,
    #[schemars(transform = unit::v_y)]
    pub escape_vy: f64,
    #[schemars(transform = unit::px_x)]
    pub throw_forward: f64,
    #[schemars(transform = unit::px_y)]
    pub throw_back_height: f64,
    #[schemars(transform = unit::px_y)]
    pub throw_up_height: f64,
    #[schemars(transform = unit::px_x)]
    pub throw_down_offset: f64,
    #[schemars(transform = unit::px_y)]
    pub platform_drop_offset: f64,
    #[schemars(transform = unit::damage)]
    pub max_damage: f64,
    #[schemars(transform = unit::px_x)]
    pub projectile_pad: f64,
    #[schemars(transform = unit::px_y)]
    pub projectile_wall_top: f64,
}
/// CPU decision thresholds in reference branch order. Arrays are indexed by difficulty (0..2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiRules {
    #[schemars(transform = unit::tick)]
    pub initial_cool: f64,
    #[schemars(transform = unit::tick)]
    pub react: [f64; 3],
    #[schemars(transform = unit::ratio)]
    pub aggression: [f64; 3],
    #[schemars(transform = unit::ratio)]
    pub mash: [f64; 3],
    #[schemars(transform = unit::ratio)]
    pub tech: [f64; 3],
    #[schemars(transform = unit::ratio)]
    pub shield_response: [f64; 3],
    #[schemars(transform = unit::ratio)]
    pub defend: [f64; 3],
    #[schemars(transform = unit::ratio)]
    pub projectile_defend: [f64; 3],
    #[schemars(transform = unit::tick)]
    pub charge: [f64; 3],
    /// Named thresholds for the fixed, shared decision tree (not fighter/move identities).
    pub thresholds: AiThresholds,
}

/// Geometry, timing, and probability thresholds for the shared CPU decision tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiThresholds {
    #[schemars(transform = unit::px_y)]
    pub recovery_height: f64,
    #[schemars(transform = unit::px_x)]
    pub recovery_edge: f64,
    #[schemars(transform = unit::ratio)]
    pub center_chance: f64,
    #[schemars(transform = unit::tick)]
    pub knockdown_wait: f64,
    #[schemars(transform = unit::tick)]
    pub knockdown_random: f64,
    #[schemars(transform = unit::ratio)]
    pub knockdown_attack: f64,
    #[schemars(transform = unit::ratio)]
    pub knockdown_center: f64,
    #[schemars(transform = unit::tick)]
    pub ledge_wait_min: f64,
    #[schemars(transform = unit::tick)]
    pub ledge_wait_max: f64,
    #[schemars(transform = unit::ratio)]
    pub ledge_jump: f64,
    #[schemars(transform = unit::ratio)]
    pub ledge_attack: f64,
    #[schemars(transform = unit::ratio)]
    pub ledge_shield: f64,
    #[schemars(transform = unit::tick)]
    pub hold_min: f64,
    #[schemars(transform = unit::tick)]
    pub hold_max: f64,
    #[schemars(transform = unit::count)]
    pub pummel_max: f64,
    #[schemars(transform = unit::ratio)]
    pub pummel_chance: f64,
    #[schemars(transform = unit::damage)]
    pub throw_kill_percent: f64,
    #[schemars(transform = unit::ratio)]
    pub throw_down: f64,
    #[schemars(transform = unit::ratio)]
    pub throw_up: f64,
    #[schemars(transform = unit::ratio)]
    pub throw_forward: f64,
    #[schemars(transform = unit::px_x)]
    pub shield_close: f64,
    #[schemars(transform = unit::ratio)]
    pub shield_grab: f64,
    #[schemars(transform = unit::ratio)]
    pub shield_special: f64,
    #[schemars(transform = unit::tick)]
    pub jump_hold: f64,
    #[schemars(transform = unit::damage)]
    pub shield_min: f64,
    #[schemars(transform = unit::px_x)]
    pub under_pad: f64,
    #[schemars(transform = unit::px_y)]
    pub under_bottom: f64,
    #[schemars(transform = unit::px_y)]
    pub offstage_pad: f64,
    #[schemars(transform = unit::v_y)]
    pub recover_jump_vy: f64,
    #[schemars(transform = unit::px_y)]
    pub recover_jump_height: f64,
    #[schemars(transform = unit::v_y)]
    pub recover_special_vy: f64,
    #[schemars(transform = unit::px_y)]
    pub recover_special_height: f64,
    #[schemars(transform = unit::px_x)]
    pub recover_special_distance: f64,
    #[schemars(transform = unit::px_y)]
    pub recover_special_depth: f64,
    #[schemars(transform = unit::px_x)]
    pub center_deadzone: f64,
    #[schemars(transform = unit::px_x)]
    pub target_pad: f64,
    #[schemars(transform = unit::px_x)]
    pub target_deadzone: f64,
    /// No-progress ticks before committing to a jump across surfaces. Zero disables it for reference traces.
    #[serde(default)]
    #[schemars(transform = unit::tick)]
    pub surface_chase_ticks: u32,
    #[schemars(transform = unit::px_x)]
    pub defend_distance: f64,
    #[schemars(transform = unit::px_y)]
    pub defend_height: f64,
    #[schemars(transform = unit::tick)]
    pub shield_hold_min: f64,
    #[schemars(transform = unit::tick)]
    pub shield_hold_max: f64,
    #[schemars(transform = unit::px_x)]
    pub projectile_distance: f64,
    #[schemars(transform = unit::px_y)]
    pub projectile_height: f64,
    #[schemars(transform = unit::ratio)]
    pub projectile_shield_chance: f64,
    #[schemars(transform = unit::tick)]
    pub projectile_shield_frames: f64,
    #[schemars(transform = unit::px_y)]
    pub jump_target_height: f64,
    #[schemars(transform = unit::px_x)]
    pub jump_target_distance: f64,
    #[schemars(transform = unit::ratio)]
    pub jump_chance: f64,
    #[schemars(transform = unit::px_y)]
    pub drop_height: f64,
    #[schemars(transform = unit::px_x)]
    pub drop_distance: f64,
    #[schemars(transform = unit::ratio)]
    pub drop_chance: f64,
    #[schemars(transform = unit::px_y)]
    pub attack_height: f64,
    #[schemars(transform = unit::tick)]
    pub punish_dizzy_charge: f64,
    #[schemars(transform = unit::tick)]
    pub punish_charge: f64,
    #[schemars(transform = unit::ratio)]
    pub smash_chance: f64,
    #[schemars(transform = unit::ratio)]
    pub smash_forward: f64,
    #[schemars(transform = unit::ratio)]
    pub smash_down: f64,
    #[schemars(transform = unit::ratio)]
    pub grab_chance: f64,
    #[schemars(transform = unit::ratio)]
    pub tilt_chance: f64,
    #[schemars(transform = unit::ratio)]
    pub down_tilt_chance: f64,
    #[schemars(transform = unit::ratio)]
    pub jab_chance: f64,
    #[schemars(transform = unit::px_y)]
    pub up_attack_height: f64,
    #[schemars(transform = unit::px_x)]
    pub up_attack_distance: f64,
    #[schemars(transform = unit::px_y)]
    pub down_attack_height: f64,
    #[schemars(transform = unit::px_x)]
    pub down_attack_distance: f64,
    #[schemars(transform = unit::px_y)]
    pub projectile_target_height: f64,
}

/// Shared held-item rules (`docs/FORMAT.md`, Items). Boxes are fighter-local (x forward, y up from the feet).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Items {
    /// Item throw timing (Ultimate: the item leaves the hand on frame 6 of about 22).
    pub toss: Toss,
    /// Attack or grab on the ground picks up a landed item whose centre is in this box.
    pub pickup: super::common::Rect,
    /// Attack, grab or (in the air) air dodge catches an item in flight whose centre is in this box.
    pub catch: super::common::Rect,
    /// Lag after a ground pickup (`landing` state, kind `pickup`).
    #[schemars(transform = unit::tick)]
    pub pickup_lag: u32,
    /// Ticks before a fighter can catch an item it threw.
    #[schemars(transform = unit::tick)]
    pub regrab: u32,
    /// A landing loose item comes to rest when it would bounce back up slower than this.
    #[schemars(transform = unit::v_y)]
    pub settle: f64,
    /// CPU: smash-throw a held item forward at an opponent closer than this (and within `ai.thresholds.attackHeight`).
    #[schemars(transform = unit::px_x)]
    pub cpu_range: f64,
    /// CPU: throw a held item toward the target after holding it this long.
    #[schemars(transform = unit::tick)]
    pub cpu_hold: u32,
}
impl Default for Items {
    fn default() -> Self {
        let rect = |x1, x2, y1, y2| super::common::Rect { x1, x2, y1, y2 };
        Self {
            toss: Toss { frames: 22, release: 5 },
            pickup: rect(-30., 45., -10., 40.),
            catch: rect(-35., 50., -10., 110.),
            pickup_lag: 6,
            regrab: 15,
            settle: 1.5,
            cpu_range: 260.,
            cpu_hold: 180,
        }
    }
}

/// Item throw timing, on the `toss` state clock (0 = the tick the throw starts).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Toss {
    /// The throw ends (actionable again).
    #[schemars(transform = unit::tick)]
    pub frames: u32,
    /// The item leaves the hand.
    #[schemars(transform = unit::tick)]
    pub release: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Replay {
    #[schemars(transform = unit::count)]
    pub auto_save_count: usize,
    #[schemars(transform = unit::tick)]
    pub checkpoint_ticks: u64,
}
impl Default for Replay {
    fn default() -> Self {
        Self { auto_save_count: 10, checkpoint_ticks: 300 }
    }
}

fn default_pose_run_speed() -> f64 {
    0.8
}
