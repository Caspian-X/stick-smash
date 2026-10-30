//! Moves as phase timelines. See `docs/FORMAT.md` for the exact per-tick order.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{Color, Num, is_default, is_one, one};
use super::hitbox::Hitbox;
use super::visual::{AnimKey, AttachAnim};

/// Move slots. Specials are chosen by input direction; without `sspec`, side + special does `nspec`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Moves {
    pub jab: Move,
    pub ftilt: Move,
    pub utilt: Move,
    pub dtilt: Move,
    pub dashatk: Move,
    pub fsmash: Move,
    pub usmash: Move,
    pub dsmash: Move,
    pub nair: Move,
    pub fair: Move,
    pub bair: Move,
    pub uair: Move,
    pub dair: Move,
    pub grab: Move,
    pub dashgrab: Move,
    pub nspec: Move,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sspec: Option<Move>,
    pub uspec: Move,
    pub dspec: Move,
}

impl Moves {
    /// All present moves with their slot names, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Move)> {
        [
            ("jab", Some(&self.jab)),
            ("ftilt", Some(&self.ftilt)),
            ("utilt", Some(&self.utilt)),
            ("dtilt", Some(&self.dtilt)),
            ("dashatk", Some(&self.dashatk)),
            ("fsmash", Some(&self.fsmash)),
            ("usmash", Some(&self.usmash)),
            ("dsmash", Some(&self.dsmash)),
            ("nair", Some(&self.nair)),
            ("fair", Some(&self.fair)),
            ("bair", Some(&self.bair)),
            ("uair", Some(&self.uair)),
            ("dair", Some(&self.dair)),
            ("grab", Some(&self.grab)),
            ("dashgrab", Some(&self.dashgrab)),
            ("nspec", Some(&self.nspec)),
            ("sspec", self.sspec.as_ref()),
            ("uspec", Some(&self.uspec)),
            ("dspec", Some(&self.dspec)),
        ]
        .into_iter()
        .filter_map(|(k, m)| m.map(|m| (k, m)))
    }
}

/// Which global air acceleration a move uses while airborne (`global.movement`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AirControl {
    /// `attackAirAccel`.
    #[default]
    Attack,
    /// `aerialAccel`, and the move can fast-fall.
    Aerial,
    /// `upSpecialAccel`.
    UpSpecial,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Move {
    /// Name shown in the debug overlay and training panel.
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub air: AirControl,
    /// Pressing the other direction within `global.movement.bReverseWindow` frames turns the fighter around
    /// (and flips air momentum).
    #[serde(default, skip_serializing_if = "is_default")]
    pub b_reverse: bool,
    /// Runs from the first phase. A move must have at least one phase.
    pub phases: Vec<Phase>,
}

/// One step of a move.
///
/// Frame convention: a phase's frame 0 is the tick it's entered (the move's first phase is entered on the tick
/// the move starts). Each later tick advances the phase frame by 1, runs that frame's `events`, then checks
/// `transitions`. With `frames: N`, the phase ends on its frame N: the next phase's frame 0 is that same tick.
/// A single-phase move with `frames: N` therefore matches the old "last active frame + endLag" total N.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Phase {
    /// Unique within the move. Transitions refer to it. `end`, `helpless` and `landing` are reserved.
    pub name: String,
    /// Phase length (see the frame convention). Omit for a phase that loops until a transition fires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hitboxes: Vec<Hitbox>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    /// Checked in order; the first that fires wins. A phase with `frames` and no `timeout` transition ends the
    /// move when it times out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<Transition>,
    /// Hold to charge (smash attacks, chargeable specials).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charge: Option<Charge>,
    /// Counter stance: hits received during the window are countered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<Counter>,
    /// Airborne: vertical speed is held at 0 (hovering startup).
    #[serde(default, skip_serializing_if = "is_default")]
    pub hover: bool,
    /// Airborne: max fall speed during this phase (default: the fighter's fall speed or fast-fall speed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_cap: Option<f64>,
    /// Multiplies the move's air acceleration (0 = no drift control).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub air_accel_mult: f64,
    /// Grounded: phase frames below this use `attackSlideFriction` instead of `attackFriction` (dash attacks).
    #[serde(default, skip_serializing_if = "is_default")]
    pub slide: u32,
    /// Can grab a ledge (even while rising) from this phase frame on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledge_grab: Option<u32>,
    /// Body pose keyframes, in phase frames.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anim: Vec<AnimKey>,
    /// Attachment animation by attachment id (weapon swings, held props).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attach: BTreeMap<String, AttachAnim>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Button {
    #[default]
    Attack,
    Special,
}

/// While the button is held on `frame`, the phase stays on that frame and charge builds.
/// Charge multiplies the move's damage: × (1 + charge / `global.knockback.chargeDamageDivisor`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Charge {
    /// Phase frame that freezes while held.
    pub frame: u32,
    /// Max frames of charge.
    #[serde(default = "sixty", skip_serializing_if = "is_sixty")]
    pub max: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub button: Button,
    /// Jet-engine charge effects (flames behind, jet sound, glowing attachments).
    #[serde(default, skip_serializing_if = "is_default")]
    pub jet: bool,
}
fn sixty() -> u32 {
    60
}
fn is_sixty(v: &u32) -> bool {
    *v == 60
}

/// Counter window. A hit received in it fires the phase's `counter` transition: the attacker freezes, and
/// projectiles are reflected.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Counter {
    /// Inclusive phase frames.
    pub from: u32,
    pub to: u32,
    /// Counter damage (used by `counterDamage` hitboxes) = max(minDamage, incoming × damageMult).
    pub damage_mult: f64,
    pub min_damage: f64,
    /// Freeze frames for the countered attacker.
    pub attacker_freeze: u32,
    /// Invulnerability frames granted on trigger.
    pub invuln: u32,
    /// Reflected projectile: speed multiplier and new lifetime.
    pub reflect_speed: f64,
    pub reflect_life: u32,
}

/// Frame-scoped actions. Use `at` for one frame, or `from`/`to` (inclusive) for a range. `from` alone means
/// from there on; neither means every frame from 1. Actions in one event run in field order (velocity, then
/// `leaveGround`, `invuln`, `spawn`, `sfx`, `fx`); events on the same frame run in list order.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Event {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<u32>,
    /// Only on world frames divisible by this (alternate-frame particles).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "if")]
    pub cond: Option<Cond>,
    /// Multiply velocity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mul_vel: Option<Vel>,
    /// Set velocity (x forward, y up).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_vel: Option<Vel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add_vel: Option<Vel>,
    /// Cap downward speed at this (before gravity is applied this tick).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_fall: Option<f64>,
    /// Leave the ground (needed for upward launches from the ground).
    #[serde(default, skip_serializing_if = "is_default")]
    pub leave_ground: bool,
    /// Invulnerable for this many frames (keeps the longer of this and any current invulnerability).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invuln: Option<u32>,
    /// Spawn a projectile by id (from the fighter's `projectiles`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sfx: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx: Vec<Fx>,
}

impl Event {
    /// Does this event run on phase frame `t` (world frame `wf`)? Frame 0 only matches `at: 0`.
    pub fn fires(&self, t: u32, wf: u64) -> bool {
        let in_range = match (self.at, self.from, self.to) {
            (Some(a), _, _) => t == a,
            (None, f, to) => t >= f.unwrap_or(1) && to.is_none_or(|to| t <= to),
        };
        in_range && self.every.is_none_or(|n| n > 0 && wf.is_multiple_of(u64::from(n)))
    }
}

/// Velocity components; a missing component is left alone.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Vel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Cond {
    Grounded,
    Airborne,
    /// The phase was entered by a counter that caught a hit with damage (not a reflected projectile).
    Countered,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    /// The phase reached `frames` (checked after the frame's events).
    Timeout,
    /// The fighter landed this tick (fires during movement, after the step).
    Land,
    /// On the ground when checked in the step.
    Grounded,
    /// In the air when checked in the step.
    Airborne,
    /// Vertical speed is zero or downward.
    Falling,
    /// The charge button (or attack, without `charge`) isn't held.
    Release,
    /// Shield was pressed this tick.
    ShieldPress,
    /// A hit was countered in the phase's `counter` window.
    Counter,
    /// A hitbox of this phase connected (hit or shield).
    Hit,
}

/// Where a transition goes: another phase by name, or one of the move endings `end` (idle on the ground, air
/// in the air), `helpless` (idle on the ground, helpless in the air), `landing` (landing lag of `lag` frames).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transition {
    pub on: Trigger,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "if")]
    pub cond: Option<Cond>,
    /// Only from this phase frame on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<u32>,
    pub to: String,
    /// Landing lag frames for `to: "landing"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lag: Option<u32>,
}

pub const END: &str = "end";
pub const HELPLESS: &str = "helpless";
pub const LANDING: &str = "landing";

/// Visual effect, positioned in fighter-local space (or projectile-local for projectile trails).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Fx {
    Dust {
        #[serde(default)]
        x: Num,
        #[serde(default)]
        y: Num,
        n: u32,
    },
    Ring {
        #[serde(default)]
        x: Num,
        #[serde(default)]
        y: Num,
        r: f64,
        /// Radius growth per frame.
        grow: f64,
        life: u32,
        #[serde(default = "white", skip_serializing_if = "is_white")]
        color: Color,
    },
    /// One particle. `inheritVel` adds the emitter's velocity times this.
    Part {
        #[serde(default)]
        x: Num,
        #[serde(default)]
        y: Num,
        #[serde(default)]
        vx: Num,
        #[serde(default)]
        vy: Num,
        life: Num,
        size: Num,
        color: Color,
        #[serde(default, skip_serializing_if = "is_default")]
        gravity: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        #[serde(rename = "inheritVel")]
        inherit_vel: f64,
    },
    Burst {
        #[serde(default)]
        x: Num,
        #[serde(default)]
        y: Num,
        color: Color,
        n: u32,
        speed: f64,
        life: u32,
        size: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        gravity: f64,
    },
    Shake {
        k: f64,
    },
}

fn white() -> Color {
    Color::Rgb(0xffffff)
}
fn is_white(c: &Color) -> bool {
    *c == white()
}

/// Grab throws by direction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Throws {
    pub f: Throw,
    pub b: Throw,
    pub u: Throw,
    pub d: Throw,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Throw {
    /// Frame the victim is released and hit.
    pub release: u32,
    pub total: u32,
    /// Geometry and window fields are ignored.
    pub hit: Hitbox,
}
