//! `stages/<id>/stage.json`. World space: stage pixels, y grows downward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::Color;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stage {
    /// Package id (the folder name), set by the loader.
    #[serde(skip)]
    pub id: String,
    #[serde(rename = "$schema", default, skip_serializing)]
    pub schema: Option<String>,
    pub schema_version: u32,
    pub name: String,
    /// Default view size; the camera starts framing `[0, width] × [0, height]`.
    pub width: f64,
    pub height: f64,
    pub ground: Ground,
    /// Pass-through platforms.
    #[serde(default)]
    pub platforms: Vec<Platform>,
    pub ledge: LedgeZone,
    /// Fighters past these lines are KO'd.
    pub blast: Bounds,
    /// The camera never shows past these.
    pub camera: Bounds,
    /// Start positions by player slot. Needs one per player (8 for a full match).
    pub spawns: Vec<Spawn>,
    /// Respawn points by player slot.
    pub respawns: Vec<RespawnPoint>,
    pub art: StageArt,
}

/// The main solid block. Its top corners are the grabbable ledges.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ground {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    /// Bottom of the vertical walls.
    pub bottom: f64,
    /// Underside below `bottom`: slopes `depth` down while moving `inset` inward, so rising fighters slide out to
    /// the wall.
    pub under_depth: f64,
    pub under_inset: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Platform {
    pub x1: f64,
    pub x2: f64,
    pub y: f64,
}

/// Ledge grab zone: a half circle on the outside of each ground corner, centered `cy` below it. A falling
/// fighter grabs when their hand point (`handY` above the feet) is inside. `reachIn` lets the flat side extend a
/// few pixels over the stage. `hangX`/`hangY` = where the feet sit while hanging.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgeZone {
    pub r: f64,
    pub cy: f64,
    pub hand_y: f64,
    pub reach_in: f64,
    pub hang_x: f64,
    pub hang_y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bounds {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Spawn {
    pub x: f64,
    /// Feet height; omit to stand on the ground.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    /// 1 = facing right, -1 = left.
    pub face: i8,
}

/// Respawn: descend from `startY` to `endY` above `x`, then wait on the respawn platform.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RespawnPoint {
    pub x: f64,
    pub start_y: f64,
    pub end_y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageArt {
    /// Ground face gradient, top to bottom.
    pub ground_top: Color,
    pub ground_bottom: Color,
    pub underside: Color,
    /// Bright strip along the ground's top edge.
    pub edge: Color,
    pub platform: Color,
    /// Soft band under each platform.
    pub platform_shadow: Color,
    /// Height of the `edge` strip.
    pub edge_width: f64,
    pub platform_thickness: f64,
    pub platform_radius: f64,
    /// Shadow band: starts `[offset]` below the platform top, `[height]` tall, at `[alpha]` opacity.
    pub platform_shadow_band: [f64; 3],
    /// Dashes along the ground face.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stripes: Option<Stripes>,
    /// Procedural backdrop, filling the `camera` bounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Background>,
}

/// Dashes `w`×`h` every `every` px, `y` below the ground top, starting `x` from the left edge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stripes {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub every: f64,
    pub color: Color,
    pub alpha: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Background {
    /// Vertical gradient `[y, color]` stops, top to bottom. Solid above the first and below the last.
    pub sky: Vec<(f64, Color)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moon: Option<Moon>,
    /// Hill silhouettes, back to front.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ridges: Vec<Ridge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stars: Option<Starfield>,
    /// 0..1: how much the backdrop stays put on screen while the camera moves (0 = moves with the stage).
    #[serde(default)]
    pub parallax: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Moon {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub color: Color,
    /// Halo: `rings` circles growing by `step` px, each at `alpha`.
    pub glow: Color,
    pub rings: u32,
    pub step: f64,
    pub alpha: f64,
}

/// `y = base − amp × (0.5 + 0.3 sin(x × 0.006 + seed) + 0.2 sin(x × 0.017 + 3 seed))`, filled down to the bottom.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ridge {
    pub base: f64,
    pub amp: f64,
    pub seed: f64,
    pub color: Color,
}

/// Twinkling stars scattered (fixed pseudo-random layout) over the rectangle `x, y, w, h`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Starfield {
    pub n: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub size: [f64; 2],
    pub color: Color,
    /// Twinkle speed (degrees per tick).
    pub twinkle: f64,
}

impl Stage {
    pub fn max_players(&self) -> usize {
        self.spawns.len().min(self.respawns.len())
    }
}
