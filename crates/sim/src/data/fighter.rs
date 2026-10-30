use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{Pt, Rect, is_default};
use super::hitbox::Hitbox;
use super::moves::{Fx, Moves, Throws};
use super::visual::{Anim, Attachment, Pose, Shape, Style};

/// `fighters/<id>/fighter.json`. With `extends`, the parent's data is merged in first (objects key by key,
/// arrays replaced whole, `null` removes a key), so every field may come from the parent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fighter {
    /// Package id (the folder name), set by the loader.
    #[serde(skip)]
    pub id: String,
    #[serde(rename = "$schema", default, skip_serializing)]
    pub schema: Option<String>,
    pub schema_version: u32,
    /// Id of the package this one inherits from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
    /// Only for inheriting from: not playable, and not checked as a complete fighter. Not inherited.
    #[serde(default, skip_serializing_if = "is_default", rename = "abstract")]
    pub is_abstract: bool,
    pub name: String,
    /// Menu help line.
    #[serde(default)]
    pub blurb: String,
    pub stats: Stats,
    /// Collision body for stage geometry.
    pub body: Body,
    pub hurtboxes: Hurtboxes,
    pub ai: Ai,
    pub moves: Moves,
    pub throws: Throws,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub projectiles: BTreeMap<String, Projectile>,
    /// Effects for fighter-wide moments.
    #[serde(default, skip_serializing_if = "FighterFx::is_empty")]
    pub fx: FighterFx,
    pub style: Style,
    /// Named poses, used by `anims`, move phases and other poses (`from`).
    pub poses: BTreeMap<String, Pose>,
    /// State animations by name (`idle`, `run`, `air.rise`, `hitstun`…). See `docs/FORMAT.md` for the list.
    pub anims: BTreeMap<String, Anim>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stats {
    /// Knockback taken multiplier. Lower = heavier.
    pub weight: f64,
    pub run_speed: f64,
    pub run_accel: f64,
    pub ground_friction: f64,
    /// Full hop launch speed (upward).
    pub jump_speed: f64,
    pub short_hop_speed: f64,
    pub air_jump_speed: f64,
    pub air_jump_drift: f64,
    /// Mid-air jumps.
    pub air_jumps: u32,
    pub air_speed: f64,
    pub air_accel: f64,
    pub gravity: f64,
    pub fall_speed: f64,
    pub fast_fall_speed: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Body {
    pub half_width: f64,
    pub height: f64,
}

/// Hurtboxes by state group. Missing optional groups use `default`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hurtboxes {
    pub default: Rect,
    pub knockdown: Rect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crouch: Option<Rect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledge: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ai {
    /// How close the CPU wants to be to attack.
    pub reach: f64,
    /// Chance to counter instead of shielding; zero skips the RNG call.
    pub counter_chance: f64,
    /// Recovery behavior is enabled by move air control, with this optional shield-cancel phase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_shield_cancel: Option<String>,
    /// Ordered special tactics; chance is sampled only after all geometric conditions pass.
    #[serde(default)]
    pub specials: Vec<SpecialTactic>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FighterFx {
    /// On every mid-air jump.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub air_jump: Vec<Fx>,
}

impl FighterFx {
    fn is_empty(&self) -> bool {
        self.air_jump.is_empty()
    }
}

/// A projectile spawned by a move's `spawn` event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Projectile {
    /// Training hit label.
    pub name: String,
    /// Spawn offset from the feet (x forward, y up).
    pub spawn: Pt,
    /// Launch velocity (x forward, y up).
    pub vel: Pt,
    /// Collision radius.
    pub r: f64,
    /// Frames alive.
    pub life: u32,
    /// Max alive per owner. Spawning past it does nothing (plays `fullSfx` if set).
    pub max: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub gravity: f64,
    /// Max fall speed (with gravity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fall: Option<f64>,
    /// Collides with the stage and bounces. Without it the projectile flies through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounce: Option<Bounce>,
    /// Any attack can knock it back, making the attacker its owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bat: Option<Bat>,
    /// Vertical speed (up) given when reflected by a counter. Omit to keep it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflect_vy: Option<f64>,
    /// Geometry and window fields are ignored; the hit comes from the projectile's position.
    pub hit: Hitbox,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn_sfx: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_sfx: Option<String>,
    /// Effects emitted while flying (in projectile-local space, x along its travel direction).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trail: Vec<super::moves::Event>,
    /// Drawing: `shapes` rotate by `spin` degrees per pixel traveled, `overlay` doesn't rotate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shapes: Vec<Shape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlay: Vec<Shape>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub spin: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bounce {
    /// Fraction of fall speed kept on a floor bounce.
    pub restitution: f64,
    /// Minimum bounce speed (up).
    pub min_speed: f64,
    /// Horizontal speed multiplier per floor bounce.
    pub friction: f64,
    /// After this many bounces, life drops to `fizzleLife`.
    pub max_bounces: u32,
    pub fizzle_life: u32,
    /// Horizontal speed kept when hitting the stage's side walls.
    pub wall_restitution: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sfx: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bat {
    /// New horizontal speed (in the hit's launch direction) and vertical speed (up).
    pub speed: f64,
    pub vy: f64,
    pub life: u32,
    /// Frames before it can be batted again.
    pub cooldown: u32,
    /// Hitlag for the batter.
    pub hitlag: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialTactic {
    /// Strict lower bound on horizontal target distance.
    pub min_distance: f64,
    /// Optional strict upper bound on horizontal target distance.
    pub max_distance: Option<f64>,
    /// Strict upper bound on absolute vertical target separation.
    pub height: f64,
    /// Require the CPU to be grounded.
    pub grounded: bool,
    /// Require no currently owned projectile.
    pub no_projectile: bool,
    /// Success probability, sampled after geometry and ownership checks pass.
    pub chance: f64,
    /// Add down to the toward-target special input.
    pub down: bool,
    /// Optional special-button hold duration range; sampled then floored.
    pub hold: Option<[f64; 2]>,
}
