use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{Color, Pt, Rect, is_default};
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
    /// One-line pitch on the character select card.
    #[serde(default)]
    pub tagline: String,
    pub stats: Stats,
    /// Collision body for stage geometry.
    pub body: Body,
    /// Limited-use resources (weapons, tomes). Moves reference them by id for variants, costs and gates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub charges: Vec<super::moves::ChargePool>,
    pub hurtboxes: Hurtboxes,
    /// Nonempty regions replace both legacy rectangle groups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt_regions: Option<Vec<super::hitbox::HurtRegion>>,
    /// Stable feet-relative autolink target; required with hurtRegions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat_center: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interactions: Option<Interactions>,
    pub ai: Ai,
    pub moves: Moves,
    pub throws: Throws,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub projectiles: BTreeMap<String, Projectile>,
    /// Effects for fighter-wide moments.
    #[serde(default, skip_serializing_if = "FighterFx::is_empty")]
    pub fx: FighterFx,
    pub style: Style,
    /// Optional named-joint figure. Omit to draw the legacy humanoid skeleton.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rig: Option<super::rig::Rig>,
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
    /// Override the shared grab pummel damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pummel_damage: Option<f64>,
    pub run_speed: f64,
    pub run_accel: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash_speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash_frames: Option<u32>,
    /// Full-tilt walk speed; defaults to runSpeed × gait.walkSpeed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk_speed: Option<f64>,
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

/// Hurtboxes for the default and knockdown state groups.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hurtboxes {
    pub default: Rect,
    pub knockdown: Rect,
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
    /// Per held charge tick, add these fractions to launch speed and damage (1 + ticks × value).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charge_scale: Option<ProjectileCharge>,
    /// Suppress contact hits until armed, then explode on proximity or lifetime expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explosion: Option<Explosion>,
    /// Independently controlled construction / travel followed by a staged attack timeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article: Option<Article>,
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
pub struct Article {
    pub slot: String,
    pub min_release: u32,
    pub auto_release: u32,
    /// Release the special button to launch; otherwise a second special press launches.
    #[serde(default)]
    pub release_on_up: bool,
    /// A fresh left/right stick input while still holding special launches the article in that direction;
    /// letting go of special ends it instead (Ptooie's blow-away and swallow).
    #[serde(default)]
    pub release_on_dir: bool,
    /// Ticks during which any reflector is ignored after reflection.
    #[serde(default)]
    pub reflect_cooldown: u32,
    /// While unreleased, keep the article at this fighter-local point and let it breathe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<ArticleHold>,
    /// Vertical speed selected by the live stick during travel, zero for stationary construction.
    #[serde(default)]
    pub steer: f64,
    /// Optional destructible body, relative to the projectile origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurtbox: Option<Rect>,
    pub stages: Vec<ArticleStage>,
}

/// Standby placement of a held article. The point follows the caster, so a held ball stays at the mouth
/// while the fighter moves or turns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArticleHold {
    /// Fighter-local point (x forward, y up) for the article's origin.
    pub offset: Pt,
    /// Vertical breathing amplitude in pixels.
    #[serde(default, skip_serializing_if = "is_default")]
    pub bob: f64,
    /// Breathing period in ticks. Required when `bob` is non-zero.
    #[serde(default, skip_serializing_if = "is_default")]
    pub period: u32,
    /// Active while the article is held (inclusive `start`/`end` are construction-age ticks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hitboxes: Vec<Hitbox>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArticleStage {
    /// Select the last stage whose construction age has been reached, frozen at release.
    pub from: u32,
    pub startup: u32,
    /// Optional shorter windup for automatic release from standby. Hit windows shift by the same amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_startup: Option<u32>,
    pub frames: u32,
    pub vel: Pt,
    /// Zero means indestructible.
    #[serde(default)]
    pub health: f64,
    pub hitboxes: Vec<Hitbox>,
    #[serde(default)]
    pub shapes: Vec<Shape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attack_shapes: Vec<Shape>,
    /// Connect active tracked damaging hitbox centers, following their per-frame tracks (e.g. a summoned blade).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<ArticleStroke>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArticleStroke {
    pub width: f64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectileCharge {
    pub speed: f64,
    pub damage: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub bkb: f64,
    /// Frames added to `hit.rehit` per held charge tick (rounds at each hit check).
    #[serde(default, skip_serializing_if = "is_default")]
    pub rehit: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub radius: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub life: f64,
    /// Launch angle at maximum charge; interpolated from the base hit's angle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle_to: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_angle_to: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Explosion {
    /// Ticks before proximity can trigger. The fuse still expires at projectile life zero.
    pub arm_ticks: u32,
    /// Distance to an opponent's hurtbox that triggers; omit for fuse only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proximity: Option<f64>,
    pub radius: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sfx: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx: Vec<Fx>,
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
    /// Grounded idle without hitlag only: face the target and clear stick directions to select neutral
    /// instead of side special (or down special with `down`).
    #[serde(default, skip_serializing_if = "is_default")]
    pub neutral: bool,
    /// Add down to the toward-target special input.
    pub down: bool,
    /// Optional special-button hold duration range; sampled then floored.
    pub hold: Option<[f64; 2]>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Interactions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grab: Option<GrabInteraction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledge: Option<LedgeInteraction>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrabInteraction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catch_point: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_point: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_anchor: Option<Pt>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgeInteraction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catch_point: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hang_anchor: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_in: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub climb_distance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roll_distance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_distance: Option<f64>,
}
