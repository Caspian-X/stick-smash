//! `global.json`: every number that isn't specific to one fighter, move or stage.
//! Units: frames (60 per second), stage pixels, pixels/frame for speeds.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::Color;
use super::hitbox::Hitbox;

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Global {
    #[serde(rename = "$schema", default, skip_serializing)]
    pub schema: Option<String>,
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
}

/// Shared movement rules. Per-fighter speeds and gravity live in each fighter's `stats`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Movement {
    /// Frames crouching before takeoff. Jump still held on the last one = full hop, released = short hop.
    pub jumpsquat: u32,
    /// Air drift when no direction is held (multiplier per frame).
    pub air_drag: f64,
    /// Air drift multiplier when a direction is held but already at max speed.
    pub air_drag_held: f64,
    /// Fast fall allowed once vertical speed is above this (negative = still slightly rising; y down).
    pub fast_fall_min_vy: f64,
    /// Downward speed applied the frame you fast fall.
    pub fast_fall_kick: f64,
    /// Air accel for moves with `air: "attack"`.
    pub attack_air_accel: f64,
    /// Air accel for moves with `air: "aerial"`.
    pub aerial_accel: f64,
    pub helpless_accel: f64,
    /// Air accel for moves with `air: "upSpecial"`.
    pub up_special_accel: f64,
    /// Ground friction multiplier during attacks; `slide` frames use attackSlideFriction.
    pub attack_friction: f64,
    pub attack_slide_friction: f64,
    /// Air drift multiplier during attacks when not accelerating.
    pub attack_air_drag: f64,
    /// Frames platforms are ignored after dropping through.
    pub platform_drop: u32,
    /// Held down passes through platforms in ordinary fall; a platform drop consumes its down gesture.
    #[serde(default)]
    pub platform_pass: bool,
    /// Fighters overlapping closer than this push apart.
    pub push_radius: f64,
    pub push_strength: f64,
    /// Ground speed needed for attack → dash attack and grab → dash grab.
    pub dash_attack_speed: f64,
    pub dash_grab_speed: f64,
    /// Frames a direction press counts as a "tap" for smash attacks, rolls and spot dodges.
    pub tap_window: u32,
    /// Frames after a `bReverse` move starts in which pressing the other direction turns it around.
    pub b_reverse_window: u32,
}

/// Shared damage-dependent knockback. Omitting ultimate retains the original TS compatibility formula.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Knockback {
    /// Shared by every fighter, throw and projectile. None is the legacy TS compatibility mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultimate: Option<UltimateKnockback>,
    pub kb_percent_scale: f64,
    /// Launch speed (px/tick) above which the victim tumbles (can tech, lands in knockdown). Replaced by
    /// `hitstun.ultimate.tumbleThreshold` when that model is on.
    pub tumble_threshold: f64,
    /// Hitting a grounded victim downward bounces them up at this fraction.
    pub ground_bounce: f64,
    /// Charge: damage × (1 + charge / chargeDamageDivisor).
    pub charge_damage_divisor: f64,
    /// Fraction of the charge damage bonus that also applies to base knockback.
    pub charge_kb_ratio: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UltimateKnockback {
    pub percent_scale: f64,
    pub damage_scale: f64,
    pub weight_scale: f64,
    pub base: f64,
    /// Reference knockback to pixels/tick (reference launch speed × world scale).
    pub launch_speed: f64,
}

/// Legacy model: hitstun = floor(launch px × perKnockback) + base, then per-hitbox hitstunMult/hitstunAdd; launches
/// fly under the shared gravity/maxFall with multiplicative airDrag. `ultimate` replaces all of that.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hitstun {
    pub per_knockback: f64,
    pub base: f64,
    pub gravity: f64,
    pub max_fall: f64,
    pub air_drag: f64,
    /// Velocity multiplier per frame while in hitstun on the ground (both models).
    pub ground_drag: f64,
    /// Horizontal drift you can add while in hitstun (per frame, both models).
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
    pub per_reference_knockback: f64,
    pub base: f64,
    /// Launch speed lost per frame, in reference knockback units (Ultimate: 0.051 decay / 0.03 speed per unit = 1.7).
    pub launch_decay: f64,
    /// Reference knockback above which the victim tumbles (Ultimate 80).
    pub tumble_threshold: f64,
    /// Out of tumble, allow an air dodge / aerial after this many frames of hitstun. Absent = never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel: Option<HitstunCancel>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HitstunCancel {
    pub air_dodge_after: u32,
    pub aerial_after: u32,
}

/// Hitlag (freeze frames for both fighters) = min(max, floor(base + damage × perDamage + knockback × perKnockback)),
/// then × the hitbox's hitlagMult (floored). Ultimate: base 6, perDamage 0.65, perKnockback 0, max 30.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hitlag {
    pub base: f64,
    pub per_damage: f64,
    pub per_knockback: f64,
    pub max: f64,
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
    pub max_frames: u32,
    pub extra_frames: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuryRule {
    pub base_frames: f64,
    pub per_percent: f64,
    pub per_knockback: f64,
    pub max_frames: f64,
    pub mash: f64,
    pub button_mash: f64,
    pub release_threshold: f64,
}

/// Grounded victim knocked prone: slide speed away from the attacker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TripRule {
    pub slide: f64,
}

/// Auto angle: grounded hits below tumble knockback use groundLow, stronger grounded hits groundHigh, airborne hits air.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SakuraiRule {
    pub ground_low: f64,
    pub ground_high: f64,
    pub air: f64,
}

/// Victim velocity = (hitbox center − victim center) × pull + attacker velocity × momentum.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutolinkRule {
    pub pull: f64,
    pub momentum: f64,
}

/// Push per frame = bkb × push × victim weight, capped at maxSpeed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindboxRule {
    pub push: f64,
    pub max_speed: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElectricRule {
    /// Victim hitlag multiplier.
    pub victim_hitlag_mult: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Shield {
    pub max: f64,
    /// Shield HP lost per frame while held.
    pub decay: f64,
    /// Shield HP regained per frame while not shielding.
    pub regen: f64,
    /// Shield HP lost = damage × damageMult × hitbox.shieldDmgMult.
    pub damage_mult: f64,
    /// Shield stun = floor(damage × stunPerDamage + stunBase).
    pub stun_per_damage: f64,
    pub stun_base: f64,
    /// Shield hitlag = floor(hitlagBase + damage × hitlagPerDamage).
    pub hitlag_base: f64,
    pub hitlag_per_damage: f64,
    /// Defender slide = damage × pushPerDamage + pushBase.
    pub push_per_damage: f64,
    pub push_base: f64,
    /// Attacker's own pushback on a shielded grounded hit.
    pub attacker_pushback: f64,
    /// Frames of lag after releasing shield (Ultimate 11).
    pub drop_lag: u32,
    /// Shield + up attack (or attack with up held) is an up smash rather than a grab. Off for TS fidelity.
    #[serde(default)]
    pub up_smash: bool,
    pub break_launch: f64,
    pub break_gravity: f64,
    pub break_max_fall: f64,
    /// Dizzy duration = max(dizzyMin, dizzyBase − percent × dizzyPerPercent).
    pub dizzy_base: f64,
    pub dizzy_min: f64,
    pub dizzy_per_percent: f64,
    /// Frames removed from dizzy per button press.
    pub dizzy_mash: f64,
    /// Shield HP after recovering from dizzy.
    #[serde(rename = "dizzyRecoverHP")]
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
    pub frames: u32,
    /// First and last intangible frame (inclusive). Start 0 replaces any current invulnerability on the entry
    /// tick; a later start adds it on that frame (leaving the state first cancels it). End below start = none; an
    /// end past `frames` carries over into the next state.
    pub invuln_start: u32,
    pub invuln_end: u32,
    /// Roll: frames moving at `speed` (inclusive).
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub move_start: u32,
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub move_end: u32,
    /// Roll ground speed / directional air dodge launch speed.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub speed: f64,
    /// Directional air dodge: velocity × decay each frame below `decayFrames`, then helpless drift.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub decay: f64,
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub decay_frames: u32,
    /// Air dodges: landing lag when touching down during the dodge.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
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
    pub window: u32,
    pub steps: Vec<StaleStep>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaleStep {
    /// Intangible frames × this (rounded; the start frame stays).
    pub invuln: f64,
    /// Total frames × this (rounded).
    pub frames: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Landing {
    /// Landing lag from a normal fall faster than softLandSpeed.
    pub hard: u32,
    pub soft_land_speed: f64,
    pub helpless: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tech {
    /// Press shield within this many frames before landing in tumble to tech.
    pub window: u32,
    /// First and last intangible frame of a tech in place (state clock as in `dodge.roll`; 0 = the landing tick).
    pub invuln_start: u32,
    pub invuln_end: u32,
    pub frames: u32,
    /// A shield press in hitstun that doesn't tech blocks further tech presses for this many frames (Ultimate 40),
    /// so mashing cannot tech. 0 = off.
    #[serde(default)]
    pub lockout: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Knockdown {
    /// Frames lying down before you can act.
    pub min_frames: u32,
    /// Automatically stand up at this frame.
    pub auto_getup: u32,
    /// Neutral getup: `frames` and intangibility as in `dodge.roll`.
    pub getup: Window,
    pub getup_attack: Window,
    /// Getup attack active frames (inclusive).
    pub attack_active: [u32; 2],
    pub attack: Vec<Hitbox>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ledge {
    pub invuln: u32,
    /// Frames hanging before you can act.
    pub min_hang: u32,
    /// Auto-drop after hanging this long.
    pub max_hang: u32,
    pub regrab_cooldown: u32,
    pub jump_speed: f64,
    pub climb: LedgeOption,
    pub attack: LedgeOption,
    pub roll: LedgeOption,
    pub jump: LedgeOption,
    /// Ledge attack active frames (inclusive).
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
    pub push_x: f64,
    pub push_y: f64,
    pub regrab_cooldown: u32,
    /// The holder must have hung at least this many frames, so simultaneous arrivals don't trade.
    pub min_hang: u32,
}

/// Releasing shield opens a parry window: a hit landing in it does nothing, freezes the attacker and frees the
/// defender. `window` 0 disables it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Parry {
    /// Frames after releasing shield (the release frame is the first).
    pub window: u32,
    /// Freeze for the defender (attacker gets this plus attackerFreeze).
    pub freeze: u32,
    pub attacker_freeze: u32,
    /// Invulnerability after a parry, so the rest of a multi-hit move whiffs.
    pub invuln: u32,
}

/// The launch is rotated toward the stick by up to maxAngle degrees, scaled by how perpendicular the stick is;
/// speed is unchanged. 0 disables it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Di {
    pub max_angle: f64,
    /// Minimum stick magnitude (0–1) that counts.
    pub min_stick: f64,
}

/// Ground gait. Speeds are fractions of the fighter's runSpeed (walkSpeed may be overridden per fighter).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Gait {
    /// Speed selecting the run animation, including combat joint sampling.
    #[serde(default = "default_pose_run_speed")]
    pub pose_run_speed: f64,
    pub enabled: bool,
    /// Full-tilt walk speed; a partial tilt walks proportionally slower.
    pub walk_speed: f64,
    /// Walk acceleration as a fraction of runAccel.
    pub walk_accel: f64,
    /// Walking promotes to running at this deadzoned horizontal tilt (0..1); zero requires a flick.
    pub run_threshold: f64,
    /// Running returns to walking below this tilt (hysteresis; <= runThreshold).
    pub walk_threshold: f64,
    /// Initial dash restricts standing attacks and shield until it ends; jump, dash attack, up smash,
    /// grab and specials can cancel it.
    pub dash_lock: bool,
    /// A flick starts a dash at this speed, held for dashFrames; flick back to dash the other way.
    pub dash_speed: f64,
    pub dash_frames: u32,
    /// Holding the other way while running turns around: facing flips at once and speed skids with turnFriction.
    pub turn_frames: u32,
    pub turn_friction: f64,
    /// Letting go while running skids for brakeFrames with brakeFriction.
    pub brake_frames: u32,
    pub brake_friction: f64,
    /// Frames to lower into a crouch or rise back to idle. Zero switches immediately.
    pub crouch_frames: u32,
    /// Crouched movement as a fraction of walk speed; zero disables crawling.
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
            dash_speed: 0.,
            dash_frames: 0,
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
    pub a: u32,
    /// Frame movement ends.
    pub b: u32,
    /// Actionable frame.
    pub end: u32,
    /// Slide distance onto the stage.
    pub dist: f64,
    /// First and last intangible frame (state clock, 0 = the tick the option starts; start 0 keeps the longer of
    /// this and any current invulnerability).
    pub invuln_start: u32,
    pub invuln_end: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Grab {
    /// Hold time = holdBase + victimPercent × holdPerPercent.
    pub hold_base: f64,
    pub hold_per_percent: f64,
    /// Hold frames removed per victim button press.
    pub mash_per_press: f64,
    /// Frames before the grabber can pummel or throw.
    pub min_hold: u32,
    pub pummel_frames: u32,
    pub pummel_hit_frame: u32,
    pub pummel_damage: f64,
    pub pummel_hitlag: u32,
    /// Vertical tolerance for grab boxes.
    pub height_tolerance: f64,
    pub release_lag: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Respawn {
    pub dead_frames: u32,
    pub descend_frames: u32,
    pub max_wait: u32,
    pub invuln: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Match {
    pub countdown: u32,
    pub go_frames: u32,
    /// Ticks the match stays frozen after its last KO ("GAME!") before the winner poses. Keep it short: nothing
    /// happens in it but the KO effects, and the end slow motion stretches it in real time.
    pub end_frames: u32,
    /// The timer shows its warning style and beeps once a second for its last this many seconds.
    #[serde(default = "default_timer_warn_seconds")]
    pub timer_warn_seconds: u32,
    /// Damage each fighter starts sudden death with.
    #[serde(default = "default_sudden_death_damage")]
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
    pub stick_threshold: i8,
    /// Analog movement (walking, air drift, DI) ignores the stick below this.
    #[serde(default)]
    pub deadzone: i8,
    /// A flick (dash, smash, platform drop) reaches this within flickFrames of leaving the deadzone.
    #[serde(default = "default_flick")]
    pub flick_threshold: i8,
    #[serde(default = "default_flick_frames")]
    pub flick_frames: u32,
    /// Human press buffer lifetime in ticks (inclusive); 0 disables re-delivery.
    pub buffer_frames: u32,
    /// Buffered presses this many ticks apart combine (jump + attack, two jumps, attack + special).
    #[serde(default)]
    pub combo_frames: u32,
    /// Down pressed up to this many ticks before falling still fast-falls once descending (0 = same tick only).
    #[serde(default)]
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
    pub jumpsquat_friction: f64,
    pub landing_friction: f64,
    pub shield_friction: f64,
    pub dodge_friction: f64,
    pub shieldbreak_friction: f64,
    pub knockdown_friction: f64,
    pub push_height: f64,
    pub main_pad: f64,
    pub platform_pad: f64,
    pub surface_tolerance: f64,
    /// Minimum downward landing speed to tech or enter knockdown from tumble.
    pub tech_min_speed: f64,
    /// Speed above which hitstun emits alternate-frame trail particles.
    pub hitstun_particle_speed: f64,
    pub knockdown_drag: f64,
    pub flip_frames: u32,
    pub ledge_drop_speed: f64,
    pub ledge_drop_offset: f64,
    pub ledge_pull_in: f64,
    pub ledge_jump_offset: f64,
    pub ledge_jump_drift: f64,
    pub ledge_jump_cooldown: u32,
    pub grab_offset: f64,
    pub release_vx: f64,
    pub release_vy: f64,
    pub escape_attacker_vx: f64,
    pub escape_vx: f64,
    pub escape_vy: f64,
    pub throw_forward: f64,
    pub throw_back_height: f64,
    pub throw_up_height: f64,
    pub throw_down_offset: f64,
    pub platform_drop_offset: f64,
    pub max_damage: f64,
    pub projectile_pad: f64,
    pub projectile_wall_top: f64,
}
/// CPU decision thresholds in reference branch order. Arrays are indexed by difficulty (0..2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiRules {
    pub initial_cool: f64,
    pub react: [f64; 3],
    pub aggression: [f64; 3],
    pub mash: [f64; 3],
    pub tech: [f64; 3],
    pub shield_response: [f64; 3],
    pub defend: [f64; 3],
    pub projectile_defend: [f64; 3],
    pub charge: [f64; 3],
    /// Named thresholds for the fixed, shared decision tree (not fighter/move identities).
    pub thresholds: AiThresholds,
}

/// Geometry, timing, and probability thresholds for the shared CPU decision tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiThresholds {
    pub recovery_height: f64,
    pub recovery_edge: f64,
    pub center_chance: f64,
    pub knockdown_wait: f64,
    pub knockdown_random: f64,
    pub knockdown_attack: f64,
    pub knockdown_center: f64,
    pub ledge_wait_min: f64,
    pub ledge_wait_max: f64,
    pub ledge_jump: f64,
    pub ledge_attack: f64,
    pub ledge_shield: f64,
    pub hold_min: f64,
    pub hold_max: f64,
    pub pummel_max: f64,
    pub pummel_chance: f64,
    pub throw_kill_percent: f64,
    pub throw_down: f64,
    pub throw_up: f64,
    pub throw_forward: f64,
    pub shield_close: f64,
    pub shield_grab: f64,
    pub shield_special: f64,
    pub jump_hold: f64,
    pub shield_min: f64,
    pub under_pad: f64,
    pub under_bottom: f64,
    pub offstage_pad: f64,
    pub recover_jump_vy: f64,
    pub recover_jump_height: f64,
    pub recover_special_vy: f64,
    pub recover_special_height: f64,
    pub recover_special_distance: f64,
    pub recover_special_depth: f64,
    pub center_deadzone: f64,
    pub target_pad: f64,
    pub target_deadzone: f64,
    /// No-progress ticks before committing to a jump across surfaces. Zero disables it for reference traces.
    #[serde(default)]
    pub surface_chase_ticks: u32,
    pub defend_distance: f64,
    pub defend_height: f64,
    pub shield_hold_min: f64,
    pub shield_hold_max: f64,
    pub projectile_distance: f64,
    pub projectile_height: f64,
    pub projectile_shield_chance: f64,
    pub projectile_shield_frames: f64,
    pub jump_target_height: f64,
    pub jump_target_distance: f64,
    pub jump_chance: f64,
    pub drop_height: f64,
    pub drop_distance: f64,
    pub drop_chance: f64,
    pub attack_height: f64,
    pub punish_dizzy_charge: f64,
    pub punish_charge: f64,
    pub smash_chance: f64,
    pub smash_forward: f64,
    pub smash_down: f64,
    pub grab_chance: f64,
    pub tilt_chance: f64,
    pub down_tilt_chance: f64,
    pub jab_chance: f64,
    pub up_attack_height: f64,
    pub up_attack_distance: f64,
    pub down_attack_height: f64,
    pub down_attack_distance: f64,
    pub projectile_target_height: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Replay {
    pub auto_save_count: usize,
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
