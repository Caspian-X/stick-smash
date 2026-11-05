//! Named joints for arbitrary stick-figure structures. Coordinates are local to the parent, +Y up.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::unit;

use super::{
    common::{Color, Pt, is_default},
    visual::Shape,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rig {
    /// Declaration order is stable; parents may appear before or after their children.
    pub joints: Vec<RigJoint>,
    /// Stick strokes connecting resolved joint positions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bones: Vec<RigBone>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RigBone {
    pub from: String,
    pub to: String,
    #[schemars(transform = unit::px_xy)]
    pub width: f64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RigJoint {
    /// Unique stable id, used by poses and bindings.
    pub id: String,
    /// Omit for a joint relative to the fighter root. Multiple root joints are allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Rest translation in the parent's rotated coordinate system.
    #[schemars(transform = unit::px_pt)]
    pub translation: Pt,
    /// Rest rotation in degrees, counterclockwise, relative to the parent.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(transform = unit::deg)]
    pub angle: f64,
    /// Vector appearance in joint-local space, after local rotation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shapes: Vec<Shape>,
}

/// Overrides keyed by joint id. Missing joints and fields retain their rest values.
pub type RigPose = BTreeMap<String, JointPose>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JointPose {
    /// Replaces the rest translation; it is not an additive offset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_pt)]
    pub translation: Option<Pt>,
    /// Replaces the rest angle. Keep angles unwrapped when animating through a full turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::deg)]
    pub angle: Option<f64>,
}
