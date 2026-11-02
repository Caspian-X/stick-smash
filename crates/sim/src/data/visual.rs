//! Stick-figure visuals: poses, animations and attachments. The sim never reads these; they live here so all
//! content shares one loader and schema.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{Color, Pt, is_default, is_one, one};
use super::moves::Cond;

/// Joint positions in fighter-local space (x forward, y up, feet at the origin). A pose may be partial: missing
/// joints come from `from` (another named pose), or from the `stand` pose.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pose {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shoulder: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hip: Option<Pt>,
    /// Front arm (drawn over the body).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elbow_f: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand_f: Option<Pt>,
    /// Back arm (drawn behind, darker).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elbow_b: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand_b: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knee_f: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foot_f: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knee_b: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foot_b: Option<Pt>,
    /// Whole-body rotation in degrees, positive = pitching forward, around `(0, pivot)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rot: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot: Option<f64>,
    /// Vertical offset of the whole figure (up).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_y: Option<f64>,
}

/// A named pose, or an inline (partial) pose.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PoseRef {
    Named(String),
    Inline(Box<Pose>),
}

/// Pose keyframe. Holds until the next key; with `lerp`, the figure blends from the previous key to this one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimKey {
    pub at: f64,
    pub pose: PoseRef,
    #[serde(default, skip_serializing_if = "is_default")]
    pub lerp: bool,
    /// Only used while this holds (`grounded` / `airborne` / `countered`); otherwise the key is skipped.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "if")]
    pub cond: Option<Cond>,
}

/// Time source for a state animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Clock {
    /// Frames since the fighter entered the state.
    #[default]
    State,
    /// World frame (idle breathing, wobbles).
    World,
    /// Run cycle phase in degrees, advanced by ground speed.
    Run,
    /// Tumble spin in degrees (signed: follows the launch direction).
    Spin,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Anim {
    #[serde(default, skip_serializing_if = "is_default")]
    pub clock: Clock,
    /// Repeat every this many clock units.
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    pub period: Option<f64>,
    pub keys: Vec<AnimKey>,
}

/// Drawing style of the stick figure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Style {
    pub head_radius: f64,
    pub torso_width: f64,
    pub front_limb_width: f64,
    pub back_limb_width: f64,
    /// Soft glow drawn under the limbs.
    pub glow_width: f64,
    pub torso_glow_width: f64,
    pub head_glow_radius: f64,
    /// Head outline width and fill.
    pub head_ring: f64,
    pub head_fill: Color,
    /// Eye rectangle `[x, y, w, h]` from the head center (x forward, y up, `(x, y)` = bottom-left corner).
    pub eye: [f64; 4],
}

/// Fighter joint (or `root` = the feet) an attachment hangs from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Joint {
    Root,
    Head,
    Shoulder,
    Hip,
    ElbowF,
    HandF,
    ElbowB,
    HandB,
    FootF,
    FootB,
}

/// When an attachment glows (shapes with `when: "glow"` replace `when: "noGlow"` ones).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum GlowWhen {
    /// During a counter window.
    Countering,
    /// While charging a `jet` charge.
    JetCharging,
    /// While a hitbox is active.
    Active,
}

/// A vector part attached to a joint: weapons, hats, held props.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Attachment {
    pub id: String,
    pub joint: Joint,
    /// Resting angle in degrees (0 forward, 90 up). On a hand, it's added to the forearm's direction; otherwise
    /// absolute. A phase's `attach` angle keys override it.
    #[serde(default, skip_serializing_if = "is_default")]
    pub angle: f64,
    /// Not drawn unless a phase's `attach.show` window says so.
    #[serde(default, skip_serializing_if = "is_default")]
    pub hidden: bool,
    /// Drawn behind the body.
    #[serde(default, skip_serializing_if = "is_default")]
    pub behind: bool,
    /// While a phase animates the angle, the front arm is posed to hold it: elbow at `elbow` px along the angle
    /// turned by `bend` degrees, hand at `hand` px, both from the shoulder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<Hold>,
    /// Swing trail (ring sector around the shoulder) while a hitbox is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trail: Option<Trail>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow: Option<GlowWhen>,
    /// Idle sway: the angle swings ± `deg` with a period of `period` ticks (world clock).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sway: Option<Sway>,
    /// Shapes in attachment space: x along the angle, y to its left (up when the angle is 0).
    pub shapes: Vec<Shape>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hold {
    pub elbow: f64,
    pub hand: f64,
    pub bend: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sway {
    pub deg: f64,
    pub period: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Trail {
    pub inner: f64,
    pub outer: f64,
}

/// Attachment animation within a move phase (phase frames). Angle, offset and scale keys interpolate linearly.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachAnim {
    /// `[frame, degrees]`, interpolated without wrapping. Use 350 → 430 to rotate forward through 360° to 70°;
    /// 350 → 70 rotates backward by 280°. Keep subsequent keys on the same turn for continuous trails.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub angle: Vec<[f64; 2]>,
    /// `[frame, x, y]` offset from the joint.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offset: Vec<[f64; 3]>,
    /// `[frame, scale]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scale: Vec<[f64; 2]>,
    /// Inclusive phase frames a `hidden` attachment is shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show: Option<[u32; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ShapeWhen {
    #[default]
    Always,
    Glow,
    NoGlow,
}

/// Vector shape. Coordinates are in the owner's local space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "shape", rename_all = "camelCase", deny_unknown_fields)]
pub enum Shape {
    Line {
        from: Pt,
        to: Pt,
        width: f64,
        color: Color,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        alpha: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        when: ShapeWhen,
    },
    Circle {
        at: Pt,
        r: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<Color>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Color>,
        #[serde(default, skip_serializing_if = "is_default")]
        width: f64,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        alpha: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        when: ShapeWhen,
    },
    Poly {
        points: Vec<Pt>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<Color>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Color>,
        #[serde(default, skip_serializing_if = "is_default")]
        width: f64,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        alpha: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        when: ShapeWhen,
    },
    /// Four-pointed sparkle. With `blink`, it shows only every other `blink` ticks (world clock).
    Star {
        at: Pt,
        r: f64,
        fill: Color,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        alpha: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        blink: u32,
        #[serde(default, skip_serializing_if = "is_default")]
        when: ShapeWhen,
    },
    /// Rectangle from corner `(x, y)` (the bottom-left in y-up space), with optional rounded corners.
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        radius: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<Color>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<Color>,
        #[serde(default, skip_serializing_if = "is_default")]
        width: f64,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        alpha: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        when: ShapeWhen,
    },
}
