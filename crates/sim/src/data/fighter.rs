use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{Color, Pt, Rect, is_default};
use super::hitbox::Hitbox;
use super::moves::{Fx, Moves, Throws};
use super::unit;
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
    #[schemars(transform = unit::none)]
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
    #[schemars(transform = unit::px_pt)]
    pub combat_center: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interactions: Option<Interactions>,
    pub ai: Ai,
    pub moves: Moves,
    pub throws: Throws,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub projectiles: BTreeMap<String, Projectile>,
    /// Holdable items, granted by a move event's `item`. Holding one turns attacks into throws (`docs/FORMAT.md`,
    /// Items).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub items: BTreeMap<String, Item>,
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
    #[schemars(transform = unit::ratio)]
    pub weight: f64,
    /// Override the shared grab pummel damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::damage)]
    pub pummel_damage: Option<f64>,
    #[schemars(transform = unit::v_x)]
    pub run_speed: f64,
    #[schemars(transform = unit::a_x)]
    pub run_accel: f64,
    /// Initial dash speed, held for dashFrames, then easing into the run (default runSpeed × gait.dashSpeed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::v_x)]
    pub dash_speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::tick)]
    pub dash_frames: Option<u32>,
    /// Dash-back turn: a dash against facing flips facing and takes the dash speed at once, but its first frames are
    /// the `dashTurn` gait (turn pose) instead of `dash`. Counts toward dashFrames (default gait.dashTurnFrames).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::tick)]
    pub dash_turn_frames: Option<u32>,
    /// Run turnaround: facing flips at once, traction slides the old way, then the run accelerates the new way
    /// after this many frames (default gait.turnFrames; Ultimate's `TurnRun` stop frame).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::tick)]
    pub turn_frames: Option<u32>,
    /// Run brake (stick released while running): skids under traction for this many frames, during which holding
    /// back turns around and a flick dashes (default gait.brakeFrames; Ultimate's `RunBrake` stop frame).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::tick)]
    pub brake_frames: Option<u32>,
    /// Full-tilt walk speed; defaults to runSpeed × gait.walkSpeed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::v_x)]
    pub walk_speed: Option<f64>,
    /// Legacy (no `movement.ultimate`): idle and crouch velocity multiplier per frame.
    #[schemars(transform = unit::ratio)]
    pub ground_friction: f64,
    /// Grounded slowdown in px/frame² with `movement.ultimate` (default its `traction`; Ultimate traction × 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::a_x)]
    pub traction: Option<f64>,
    /// Full hop launch speed (upward). With `jumpInitialHeight` it is the speed after the initial rise.
    #[schemars(transform = unit::v_y)]
    pub jump_speed: f64,
    /// Ultimate's jump initial height: a full hop rises this far in equal steps over its first 4 airborne frames
    /// (no gravity), then continues at `jumpSpeed` (derived from the remaining height). Short hops and air jumps
    /// are unaffected. Omit for a plain jump.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_y)]
    pub jump_initial_height: Option<f64>,
    #[schemars(transform = unit::v_y)]
    pub short_hop_speed: f64,
    #[schemars(transform = unit::v_y)]
    pub air_jump_speed: f64,
    #[schemars(transform = unit::v_x)]
    pub air_jump_drift: f64,
    /// Mid-air jumps.
    #[schemars(transform = unit::count)]
    pub air_jumps: u32,
    #[schemars(transform = unit::v_x)]
    pub air_speed: f64,
    /// Air drift acceleration (px/frame²). With `movement.ultimate` it also drives attacks, up specials and helpless
    /// fall, times that movement's per-kind multiplier and the phase `airAccelMult`.
    #[schemars(transform = unit::a_x)]
    pub air_accel: f64,
    /// Airborne slowdown in px/frame² with `movement.ultimate`: toward zero with the stick neutral, toward airSpeed
    /// when above it (default its `airFriction`; Ultimate air friction × 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::a_x)]
    pub air_friction: Option<f64>,
    #[schemars(transform = unit::a_y)]
    pub gravity: f64,
    #[schemars(transform = unit::v_y)]
    pub fall_speed: f64,
    #[schemars(transform = unit::v_y)]
    pub fast_fall_speed: f64,
    /// Peach-style float: once per airtime, holding jump while descending (or jump pressed with down held) holds
    /// the vertical speed at `speed` for up to `frames` ticks, through aerials. Omit to disable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub float: Option<Float>,
}

/// `stats.float`. Starts in the air (normal `Air` control) once per airtime when jump is held while not rising,
/// or on a jump press with down held (from the ground too, with `groundHeight`). While it lasts the fighter's vertical speed is `speed` (0 = hover; positive
/// sinks) and drift, aerials and the air jump work as usual; an aerial started during the float keeps it.
/// Releasing jump, the frame limit, an air jump, fast fall, being hit, a ledge, any non-aerial state and landing end
/// it; landing (not a ledge) makes it available again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Float {
    #[schemars(transform = unit::v_y)]
    pub speed: f64,
    #[schemars(transform = unit::tick)]
    pub frames: u32,
    /// Ground float: leaving jumpsquat with down held (float unused) starts the float at once, lifted this far
    /// above the ground with no vertical speed, instead of a hop. 0 (default) disables it.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::px_y)]
    pub ground_height: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Body {
    #[schemars(transform = unit::px_x)]
    pub half_width: f64,
    #[schemars(transform = unit::px_y)]
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
    #[schemars(transform = unit::px_x)]
    pub reach: f64,
    /// Chance to counter instead of shielding; zero skips the RNG call.
    #[schemars(transform = unit::ratio)]
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
    #[schemars(transform = unit::px_pt)]
    pub spawn: Pt,
    /// Launch velocity (x forward, y up).
    #[schemars(transform = unit::v_pt)]
    pub vel: Pt,
    /// Collision radius.
    #[schemars(transform = unit::px_xy)]
    pub r: f64,
    /// Frames alive.
    #[schemars(transform = unit::tick)]
    pub life: u32,
    /// Max alive per owner. Spawning past it does nothing (plays `fullSfx` if set).
    #[schemars(transform = unit::count)]
    pub max: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::a_y)]
    pub gravity: f64,
    /// Max fall speed (with gravity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::v_y)]
    pub max_fall: Option<f64>,
    /// Collides with the stage and bounces. Without it the projectile flies through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounce: Option<Bounce>,
    /// Any attack can knock it back, making the attacker its owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bat: Option<Bat>,
    /// Vertical speed (up) given when reflected by a counter. Omit to keep it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::v_y)]
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
    #[schemars(transform = unit::deg_px)]
    pub spin: f64,
}

/// A holdable item (`items.<id>`), Smash-style. While it is held, attacks and aerials throw it (the `toss` state);
/// a thrown item is its `projectile` (smash throws: `smashProjectile`) launched at the throw's velocity. It turns
/// into a loose world item when it lands, connects or runs out of life, and a loose item can be picked up on the
/// ground or caught in the air by anyone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Item {
    /// Training label.
    pub name: String,
    /// Projectile (from `projectiles`, with `bounce`) a tilt throw or a drop launches. Its shapes draw the item;
    /// its radius, gravity, fall cap and bounce move the loose item.
    pub projectile: String,
    /// Projectile a smash throw launches (default `projectile`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smash_projectile: Option<String>,
    /// Fighter-local point (x forward, y up) a throw starts from; also where the held item is drawn on figures
    /// without a front hand.
    #[schemars(transform = unit::px_pt)]
    pub hold: Pt,
    pub throws: ItemThrows,
    /// Damage added per px/tick of throw speed above the projectile's own `vel` speed (taken away below it),
    /// never below zero. Ultimate's thrown-item damage adds 3 per unit/frame: 0.6 at 5 px per unit.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::damage)]
    pub speed_damage: f64,
    /// Velocity a thrown item bounces off with when its hit connects (x back against its travel, y up). Omit to use
    /// the item up on a hit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::v_pt)]
    pub rebound: Option<Pt>,
    /// Ticks a loose item stays in the world.
    #[schemars(transform = unit::tick)]
    pub life: u32,
    /// Can be caught out of the air (thrown or falling). A landed item can always be picked up.
    #[serde(default, skip_serializing_if = "is_default")]
    pub catch: bool,
    /// Throw timing for this item (default `global.items.toss`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toss: Option<super::global::Toss>,
}

/// Item throw velocities (x forward, y up) by direction and strength.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemThrows {
    pub forward: ThrowSpeeds,
    /// The fighter turns around first, so x points the new way.
    pub back: ThrowSpeeds,
    pub up: ThrowSpeeds,
    pub down: ThrowSpeeds,
    /// Letting go: grab in the air, or attack / grab from shield.
    #[schemars(transform = unit::v_pt)]
    pub drop: Pt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThrowSpeeds {
    #[schemars(transform = unit::v_pt)]
    pub tilt: Pt,
    #[schemars(transform = unit::v_pt)]
    pub smash: Pt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Article {
    pub slot: String,
    #[schemars(transform = unit::tick)]
    pub min_release: u32,
    #[schemars(transform = unit::tick)]
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
    #[schemars(transform = unit::tick)]
    pub reflect_cooldown: u32,
    /// While unreleased, keep the article at this fighter-local point and let it breathe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<ArticleHold>,
    /// Vertical speed selected by the live stick during travel, zero for stationary construction.
    #[serde(default)]
    #[schemars(transform = unit::v_y)]
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
    #[schemars(transform = unit::px_pt)]
    pub offset: Pt,
    /// Vertical breathing amplitude in pixels.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::px_y)]
    pub bob: f64,
    /// Breathing period in ticks. Required when `bob` is non-zero.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::tick)]
    pub period: u32,
    /// Active while the article is held (inclusive `start`/`end` are construction-age ticks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hitboxes: Vec<Hitbox>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArticleStage {
    /// Select the last stage whose construction age has been reached, frozen at release.
    #[schemars(transform = unit::tick)]
    pub from: u32,
    #[schemars(transform = unit::tick)]
    pub startup: u32,
    /// Optional shorter windup for automatic release from standby. Hit windows shift by the same amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::tick)]
    pub auto_startup: Option<u32>,
    #[schemars(transform = unit::tick)]
    pub frames: u32,
    #[schemars(transform = unit::v_pt)]
    pub vel: Pt,
    /// Zero means indestructible.
    #[serde(default)]
    #[schemars(transform = unit::damage)]
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
    #[schemars(transform = unit::px_xy)]
    pub width: f64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectileCharge {
    #[schemars(transform = unit::ratio)]
    pub speed: f64,
    #[schemars(transform = unit::ratio)]
    pub damage: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::ratio)]
    pub bkb: f64,
    /// Frames added to `hit.rehit` per held charge tick (rounds at each hit check).
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::tick)]
    pub rehit: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::ratio)]
    pub radius: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::ratio)]
    pub life: f64,
    /// Launch angle at maximum charge; interpolated from the base hit's angle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::deg)]
    pub angle_to: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::deg)]
    pub ground_angle_to: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Explosion {
    /// Ignore proximity and freeze the fuse; only a detonate event explodes this gadget.
    #[serde(default, skip_serializing_if = "is_default")]
    pub manual: bool,
    /// Ticks before proximity can trigger. Unless manual, the fuse expires at projectile life zero.
    #[schemars(transform = unit::tick)]
    pub arm_ticks: u32,
    /// Distance to an opponent's hurtbox that triggers; omit for fuse only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_xy)]
    pub proximity: Option<f64>,
    #[schemars(transform = unit::px_xy)]
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
    #[schemars(transform = unit::ratio)]
    pub restitution: f64,
    /// Minimum bounce speed (up).
    #[schemars(transform = unit::v_y)]
    pub min_speed: f64,
    /// Horizontal speed multiplier per floor bounce.
    #[schemars(transform = unit::ratio)]
    pub friction: f64,
    /// After this many bounces, life drops to `fizzleLife`.
    #[schemars(transform = unit::count)]
    pub max_bounces: u32,
    #[schemars(transform = unit::tick)]
    pub fizzle_life: u32,
    /// Horizontal speed kept when hitting the stage's side walls.
    #[schemars(transform = unit::ratio)]
    pub wall_restitution: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sfx: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bat {
    /// New horizontal speed (in the hit's launch direction) and vertical speed (up).
    #[schemars(transform = unit::v_x)]
    pub speed: f64,
    #[schemars(transform = unit::v_y)]
    pub vy: f64,
    #[schemars(transform = unit::tick)]
    pub life: u32,
    /// Frames before it can be batted again.
    #[schemars(transform = unit::tick)]
    pub cooldown: u32,
    /// Hitlag for the batter.
    #[schemars(transform = unit::tick)]
    pub hitlag: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialTactic {
    /// Strict lower bound on horizontal target distance.
    #[schemars(transform = unit::px_x)]
    pub min_distance: f64,
    /// Optional strict upper bound on horizontal target distance.
    #[schemars(transform = unit::px_x)]
    pub max_distance: Option<f64>,
    /// Strict upper bound on absolute vertical target separation.
    #[schemars(transform = unit::px_y)]
    pub height: f64,
    /// Require the CPU to be grounded.
    pub grounded: bool,
    /// Require no currently owned projectile.
    pub no_projectile: bool,
    /// Success probability, sampled after geometry and ownership checks pass.
    #[schemars(transform = unit::ratio)]
    pub chance: f64,
    /// Grounded idle without hitlag only: face the target and clear stick directions to select neutral
    /// instead of side special (or down special with `down`).
    #[serde(default, skip_serializing_if = "is_default")]
    pub neutral: bool,
    /// Add down to the toward-target special input.
    pub down: bool,
    /// Optional special-button hold duration range; sampled then floored.
    #[schemars(transform = unit::tick)]
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
    #[schemars(transform = unit::px_pt)]
    pub catch_point: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_pt)]
    pub hold_point: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_pt)]
    pub held_anchor: Option<Pt>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgeInteraction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_pt)]
    pub catch_point: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_pt)]
    pub hang_anchor: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_x)]
    pub pull_in: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_x)]
    pub climb_distance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_x)]
    pub roll_distance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_x)]
    pub attack_distance: Option<f64>,
}
