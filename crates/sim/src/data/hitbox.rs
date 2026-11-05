use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{is_default, is_one, one};
use super::unit;

/// Hitbox behavior, modeled on Smash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HitType {
    #[default]
    Normal,
    /// Always launches downward (a positive `angle` is mirrored below the horizon).
    Spike,
    /// Grounded victims are knocked prone instead of launched. Airborne victims are hit normally.
    Trip,
    /// Angle chosen automatically: flat on weak grounded hits, higher on strong ones, 45° in the air (`global.hitTypes.sakurai`).
    Sakurai,
    /// Pulls the victim toward the hitbox center, plus the attacker's momentum. For multi-hit moves (`rehit`).
    Autolink,
    /// No damage, hitstun or hitlag. Pushes anything it touches every frame along `angle` (strength = bkb).
    Windbox,
    /// Longer victim hitlag (`global.hitTypes.electric`), with spark effects.
    Electric,
    /// Freeze in place before launching; cannot paralyze again until actionable or landed.
    Paralyze,
    /// Trap grounded targets in place; airborne targets launch normally.
    Bury,
    /// Grabs instead of hitting.
    Grab,
    /// Damage and shield damage only: no hitstun, hitlag, launch or state change. Used by lingering
    /// no-flinch hazards such as Poison Breath.
    DamageOnly,
}

/// Launch direction rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LaunchDir {
    /// The way the attacker faces.
    #[default]
    Forward,
    /// Away from the attacker's center (multi-direction hits).
    Away,
    /// Behind the attacker.
    Back,
}

/// Which victim positions a hitbox can hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Situation {
    #[default]
    Both,
    Ground,
    Air,
}

/// Per-frame geometry override: `[frame, x, y, r]`.
pub type HitboxFrame = [f64; 4];

/// A hit circle. In a move phase, `start`/`end` are inclusive phase frames. For throws, projectiles and other
/// hits that aren't frame-windowed they are ignored. When two boxes overlap a victim, the first listed wins.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hitbox {
    /// First active phase frame.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::tick)]
    pub start: u32,
    /// Last active phase frame (inclusive). Omit to stay active until the phase ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::tick)]
    pub end: Option<u32>,
    /// Offset forward from the feet. Negative = behind.
    #[schemars(transform = unit::px_x)]
    pub x: f64,
    /// Offset up from the feet.
    #[schemars(transform = unit::px_y)]
    pub y: f64,
    /// Radius.
    #[schemars(transform = unit::px_xy)]
    pub r: f64,
    /// Percent dealt.
    #[schemars(transform = unit::damage)]
    pub damage: f64,
    /// Base knockback in reference units (converted by global.knockback.ultimate.launchSpeed).
    /// Windboxes use this as pixel-space force instead.
    #[schemars(transform = unit::kb)]
    pub bkb: f64,
    /// Knockback growth as a fraction (100% reference growth = 1).
    #[schemars(transform = unit::ratio)]
    pub kbg: f64,
    /// Launch angle in degrees. 0 = forward, 90 = straight up, negative = down.
    #[schemars(transform = unit::deg)]
    pub angle: f64,
    /// Optional launch angle for grounded victims (before the shared ground-bounce rule).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::deg)]
    pub ground_angle: Option<f64>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub dir: LaunchDir,
    #[serde(default, skip_serializing_if = "is_default", rename = "type")]
    pub kind: HitType,
    /// Grounded, airborne, or either kind of victim. Also applies to projectile contact hits.
    #[serde(default, skip_serializing_if = "is_default")]
    pub situation: Situation,
    /// Fixed knockback: ignores damage/percent (launch speed = bkb × launchSpeed × victim weight multiplier).
    #[serde(default, skip_serializing_if = "is_default")]
    pub set_kb: bool,
    /// Ultimate's weight-based fixed knockback (FKB/WBKB): the shared formula with percent 10 and this value as the
    /// damage, ignoring the victim's percent and the hit's damage; `bkb` and `kbg` keep their source meaning. Needs
    /// `knockback.ultimate`; takes precedence over `setKb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::damage)]
    pub fkb: Option<f64>,
    /// Multi-hit: can hit the same victim/group again every this many move ticks (default: once until history clears).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::tick)]
    pub rehit: Option<u32>,
    /// Independent hit identity within one move. A victim can be hit once per group, so later rows with the
    /// same group are alternatives (early/late tiers, coverage changes) while a different group is a new hit
    /// (multi-hit sequences such as Diddy Kong's forward smash).
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::none)]
    pub group: u8,
    /// Multiplies the hitstun this box causes.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    #[schemars(transform = unit::ratio)]
    pub hitstun_mult: f64,
    /// Flat frames added to hitstun.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::tick)]
    pub hitstun_add: f64,
    /// Multiplies hitlag (freeze frames) on hit.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    #[schemars(transform = unit::ratio)]
    pub hitlag_mult: f64,
    /// Multiplies shield damage.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    #[schemars(transform = unit::ratio)]
    pub shield_dmg_mult: f64,
    /// Multiplies shield stun.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    #[schemars(transform = unit::ratio)]
    pub shield_stun_mult: f64,
    /// Counter strike: damage comes from the countered hit (see `Counter`). Inactive if nothing was countered.
    #[serde(default, skip_serializing_if = "is_default")]
    pub counter_damage: bool,
    /// Fraction of the damage dealt that heals the attacker (Nosferatu drains health). Capped at the
    /// attacker's current damage.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::ratio)]
    pub heal: f64,
    /// Ignores shields, counters and parries (a command grab such as Nosferatu still lands on a guarding victim).
    #[serde(default, skip_serializing_if = "is_default")]
    pub unblockable: bool,
    /// In a move, inactive below this many held charge ticks.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::tick)]
    pub min_charge: u32,
    /// Per-frame position/size `[frame, x, y, r]`. A frame without an entry uses the closest earlier entry, or
    /// x/y/r before the first entry. Omit for a box that stays put.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(transform = unit::hit_frame)]
    pub frames: Vec<HitboxFrame>,
    /// Explicit collision replaces static x/y/r; cannot combine with frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<CombatGeometry>,
}

impl Hitbox {
    pub fn targets(&self, grounded: bool) -> bool {
        match self.situation {
            Situation::Both => true,
            Situation::Ground => grounded,
            Situation::Air => !grounded,
        }
    }
    /// Active on phase frame `t`?
    pub fn active(&self, t: u32) -> bool {
        t >= self.start && self.end.is_none_or(|e| t <= e)
    }

    /// Geometry `(x, y, r)` on phase frame `t`, applying `frames` (closest entry at or before `t`).
    pub fn geom_at(&self, t: u32) -> (f64, f64, f64) {
        let t = f64::from(t);
        let mut best: Option<&HitboxFrame> = None;
        for e in &self.frames {
            if e[0] <= t && best.is_none_or(|b| e[0] >= b[0]) {
                best = Some(e);
            }
        }
        best.map_or((self.x, self.y, self.r), |e| (e[1], e[2], e[3]))
    }
}

/// Feet-relative forward/up, or a named joint's rotated local space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundPoint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint: Option<String>,
    #[schemars(transform = unit::px_pt)]
    pub offset: super::Pt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum CombatGeometry {
    Circle {
        center: BoundPoint,
        #[schemars(transform = unit::px_xy)]
        radius: f64,
    },
    Capsule {
        a: BoundPoint,
        b: BoundPoint,
        #[schemars(transform = unit::px_xy)]
        radius: f64,
    },
}
impl CombatGeometry {
    pub fn points_mut(&mut self) -> Vec<&mut BoundPoint> {
        match self {
            Self::Circle { center, .. } => vec![center],
            Self::Capsule { a, b, .. } => vec![a, b],
        }
    }
    pub fn points(&self) -> Vec<&BoundPoint> {
        match self {
            Self::Circle { center, .. } => vec![center],
            Self::Capsule { a, b, .. } => vec![a, b],
        }
    }
    pub fn radius(&self) -> f64 {
        match self {
            Self::Circle { radius, .. } | Self::Capsule { radius, .. } => *radius,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HurtRegion {
    /// Default activation. State selectors override this; phase windows override selectors.
    #[serde(default = "region_enabled", skip_serializing_if = "region_is_enabled")]
    pub enabled: bool,
    /// Typed state overrides. Crouch overrides idle while grounded in the crouch gait.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<HurtStateOverride>,
    pub id: String,
    pub geometry: CombatGeometry,
}

fn region_enabled() -> bool {
    true
}
fn region_is_enabled(value: &bool) -> bool {
    *value
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HurtState {
    State(crate::world::State),
    Crouch,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HurtStateOverride {
    pub state: HurtState,
    pub enabled: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HurtRegionWindow {
    /// Stable hurtRegions ID; deleting a referenced region is a validation error.
    pub region: String,
    pub enabled: bool,
    /// Inclusive current phase MoveState.t, independent of elapsed move time.
    #[schemars(transform = unit::tick)]
    pub from: u32,
    #[schemars(transform = unit::tick)]
    pub to: u32,
}
