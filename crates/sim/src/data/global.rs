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

/// Knockback = (bkb + victimPercent × kbg × kbPercentScale) × victimWeight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Knockback {
    pub kb_percent_scale: f64,
    /// Knockback above this sends the victim tumbling (can tech, lands in knockdown).
    pub tumble_threshold: f64,
    /// Hitting a grounded victim downward bounces them up at this fraction.
    pub ground_bounce: f64,
    /// Charge: damage × (1 + charge / chargeDamageDivisor).
    pub charge_damage_divisor: f64,
    /// Fraction of the charge damage bonus that also applies to base knockback.
    pub charge_kb_ratio: f64,
}

/// Hitstun = floor(knockback × perKnockback) + base, then per-hitbox hitstunMult/hitstunAdd.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hitstun {
    pub per_knockback: f64,
    pub base: f64,
    pub gravity: f64,
    pub max_fall: f64,
    pub air_drag: f64,
    pub ground_drag: f64,
    /// Horizontal drift you can add while in hitstun (per frame).
    pub drift_accel: f64,
}

/// Hitlag (freeze frames for both fighters) = min(max, floor(base + damage × perDamage + knockback × perKnockback)).
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
    /// Frames of lag after releasing shield.
    pub drop_lag: u32,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Dodge {
    pub roll_invuln: u32,
    pub roll_frames: u32,
    pub roll_speed: f64,
    pub roll_move_start: u32,
    pub roll_move_end: u32,
    pub spot_invuln: u32,
    pub spot_frames: u32,
    pub air_dodge_invuln: u32,
    pub air_dodge_frames: u32,
    pub air_dodge_landing_lag: u32,
    pub dir_air_dodge_invuln: u32,
    pub dir_air_dodge_frames: u32,
    pub dir_air_dodge_speed: f64,
    pub dir_air_dodge_decay: f64,
    pub dir_air_dodge_decay_frames: u32,
    pub dir_air_dodge_landing_lag: u32,
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
    pub invuln: u32,
    pub frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Knockdown {
    /// Frames lying down before you can act.
    pub min_frames: u32,
    /// Automatically stand up at this frame.
    pub auto_getup: u32,
    pub getup_frames: u32,
    pub getup_invuln: u32,
    pub attack_frames: u32,
    pub attack_invuln: u32,
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
    /// Invulnerability frames.
    pub inv: u32,
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
    pub end_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayerColors {
    pub main: Color,
    pub dark: Color,
    pub light: Color,
}

/// Digital stick interpretation; history is part of the rollback snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputRules {
    pub stick_threshold: i8,
    /// Human press buffer lifetime in ticks (inclusive); 0 disables re-delivery.
    pub buffer_frames: u32,
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
