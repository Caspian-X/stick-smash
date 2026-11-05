//! `stages/<id>/stage.json`. World space: stage pixels, y grows downward.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::Color;
use super::unit;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stage {
    /// Package id (the folder name), set by the loader.
    #[serde(skip)]
    pub id: String,
    #[serde(rename = "$schema", default, skip_serializing)]
    pub schema: Option<String>,
    #[schemars(transform = unit::none)]
    pub schema_version: u32,
    pub name: String,
    /// Default view size; the camera starts framing `[0, width] × [0, height]`.
    #[schemars(transform = unit::px_x)]
    pub width: f64,
    #[schemars(transform = unit::px_y)]
    pub height: f64,
    /// Static solids in collision tie-break order. At least one is required. Bounds including
    /// undersides must not overlap; touching blocks can form walls or a raised centre.
    pub blocks: Vec<Ground>,
    /// Pass-through platforms.
    #[serde(default)]
    pub platforms: Vec<Platform>,
    pub ledge: LedgeZone,
    /// Fighters past these lines are KO'd.
    pub blast: Bounds,
    /// The camera never shows past these.
    pub camera: Bounds,
    /// Presentation-only visible-height limits in stage pixels. Each absent bound uses the automatic
    /// geometry-derived range; camera bounds still cap the view. Values must be positive, minHeight ≤ maxHeight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zoom: Option<Zoom>,
    /// Start positions by player slot. Needs one per player (8 for a full match).
    pub spawns: Vec<Spawn>,
    /// Respawn points by player slot.
    pub respawns: Vec<RespawnPoint>,
    pub art: StageArt,
}

/// Optional follow-camera zoom overrides; omitted bounds are automatic.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Zoom {
    /// Closest visible height in stage pixels; absent uses the automatic stage-size scaling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_y)]
    pub min_height: Option<f64>,
    /// Widest visible height in stage pixels; absent fits the stage geometry automatically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_y)]
    pub max_height: Option<f64>,
}

/// Solid rectangle with a tapered underside. Only the two top corners can be ledges.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ground {
    #[schemars(transform = unit::px_x)]
    pub left: f64,
    #[schemars(transform = unit::px_x)]
    pub right: f64,
    #[schemars(transform = unit::px_y)]
    pub top: f64,
    /// Bottom of the vertical walls.
    #[schemars(transform = unit::px_y)]
    pub bottom: f64,
    /// Underside below `bottom`: slopes `depth` down while moving `inset` inward, so rising fighters slide out to
    /// the wall.
    #[schemars(transform = unit::px_y)]
    pub under_depth: f64,
    /// Positive taper width on each side; cannot exceed half the block width.
    #[schemars(transform = unit::px_x)]
    pub under_inset: f64,
    /// Whether the top-left corner can be grabbed.
    pub left_ledge: bool,
    /// Whether the top-right corner can be grabbed.
    pub right_ledge: bool,
}

impl Ground {
    pub fn body_bottom(&self, x: f64, half: f64) -> f64 {
        let x = ((self.left + self.right) / 2.).clamp(x - half, x + half).clamp(self.left, self.right);
        self.bottom + ((x - self.left).min(self.right - x) / self.under_inset).min(1.) * self.under_depth
    }

    pub fn intersects_body(&self, x: f64, y: f64, half: f64, height: f64) -> bool {
        x + half > self.left && x - half < self.right && y > self.top && y - height < self.body_bottom(x, half)
    }

    /// Exact convex sweep of a rectangle along a straight segment. Contact without penetration is allowed.
    pub fn intersects_sweep(&self, rect: [f64; 4], delta: [f64; 2]) -> bool {
        let [left, top, right, bottom] = rect;
        let [dx, dy] = delta;
        let polygon = [
            [self.left, self.top],
            [self.right, self.top],
            [self.right, self.bottom],
            [self.right - self.under_inset, self.bottom + self.under_depth],
            [self.left + self.under_inset, self.bottom + self.under_depth],
            [self.left, self.bottom],
        ];
        let rectangle = [[left, top], [right, top], [right, bottom], [left, bottom]];
        for [ax, ay] in
            [[1., 0.], [0., 1.], [self.under_depth, self.under_inset], [self.under_depth, -self.under_inset], [dy, -dx]]
        {
            if ax == 0. && ay == 0. {
                continue;
            }
            let project = |points: &[[f64; 2]]| {
                points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                    let v = p[0] * ax + p[1] * ay;
                    (lo.min(v), hi.max(v))
                })
            };
            let (lo, hi) = project(&polygon);
            let (rlo, rhi) = project(&rectangle);
            let shift = dx * ax + dy * ay;
            if hi <= rlo + shift.min(0.) || rhi + shift.max(0.) <= lo {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Platform {
    #[schemars(transform = unit::px_x)]
    pub x1: f64,
    #[schemars(transform = unit::px_x)]
    pub x2: f64,
    #[schemars(transform = unit::px_y)]
    pub y: f64,
    /// Translation relative to x1/x2/y; absent means stationary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<Motion>,
}

/// Deterministic translation sampled at the world frame. Travel is linear; no velocity state is stored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Motion {
    /// At least two offsets. Leg j to j+1 uses waypoint j's travelTicks in both directions.
    /// In loop mode the last duration closes the loop; in pingPong it is ignored. Waits occur before departure.
    pub waypoints: Vec<Waypoint>,
    pub mode: PathMode,
    /// Advance this many ticks into the repeating cycle at world frame zero.
    #[serde(default)]
    #[schemars(transform = unit::tick)]
    pub phase: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PathMode {
    /// Last waypoint travels back to the first.
    Loop,
    /// Reverse at each endpoint, without duplicating endpoint waits.
    PingPong,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Waypoint {
    /// Horizontal translation from the platform's base coordinates.
    #[schemars(transform = unit::px_x)]
    pub x: f64,
    /// Vertical translation from the platform's base height (positive is down).
    #[schemars(transform = unit::px_y)]
    pub y: f64,
    /// Travel ticks to the next waypoint, also used on the return leg. Ignored at the last pingPong waypoint.
    #[schemars(transform = unit::tick)]
    pub travel_ticks: u32,
    #[serde(default)]
    /// Ticks spent at this waypoint before departing; defaults to zero.
    #[schemars(transform = unit::tick)]
    pub wait_ticks: u32,
}

impl Motion {
    /// Fractional frames are supported for rendering; simulation uses integer world frames.
    pub fn offset(&self, frame: f64) -> (f64, f64) {
        let n = self.waypoints.len();
        if n < 2 {
            return (0., 0.);
        }
        let count = if self.mode == PathMode::Loop { n } else { 2 * (n - 1) };
        let index = |k: usize| if k < n { k } else { 2 * (n - 1) - k };
        let travel = |k: usize| {
            let a = index(k);
            let b = index((k + 1) % count);
            self.waypoints[if self.mode == PathMode::PingPong { a.min(b) } else { a }].travel_ticks
        };
        let duration: u64 = (0..count)
            .map(|k| {
                let p = &self.waypoints[index(k)];
                u64::from(p.wait_ticks) + u64::from(travel(k))
            })
            .sum();
        if duration == 0 {
            return (0., 0.);
        }
        let mut t = (frame % duration as f64 + (self.phase % duration) as f64) % duration as f64;
        for k in 0..count {
            let a = &self.waypoints[index(k)];
            let b = &self.waypoints[index((k + 1) % count)];
            if t < f64::from(a.wait_ticks) {
                return (a.x, a.y);
            }
            t -= f64::from(a.wait_ticks);
            if t < f64::from(travel(k)) {
                let u = t / f64::from(travel(k));
                return (a.x + (b.x - a.x) * u, a.y + (b.y - a.y) * u);
            }
            t -= f64::from(travel(k));
        }
        (self.waypoints[0].x, self.waypoints[0].y)
    }
}

impl Platform {
    pub fn position(&self, frame: f64) -> (f64, f64, f64) {
        let (dx, dy) = self.motion.as_ref().map_or((0., 0.), |m| m.offset(frame));
        (self.x1 + dx, self.x2 + dx, self.y + dy)
    }
}

/// Ledge grab zone: a half circle on the outside of each enabled block corner, centered `cy` below it. A falling
/// fighter grabs when their hand point (`handY` above the feet) is inside. `reachIn` lets the flat side extend a
/// few pixels over the stage. `hangX`/`hangY` = where the feet sit while hanging.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgeZone {
    #[schemars(transform = unit::px_xy)]
    pub r: f64,
    #[schemars(transform = unit::px_y)]
    pub cy: f64,
    #[schemars(transform = unit::px_y)]
    pub hand_y: f64,
    #[schemars(transform = unit::px_x)]
    pub reach_in: f64,
    #[schemars(transform = unit::px_x)]
    pub hang_x: f64,
    #[schemars(transform = unit::px_y)]
    pub hang_y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bounds {
    #[schemars(transform = unit::px_x)]
    pub left: f64,
    #[schemars(transform = unit::px_x)]
    pub right: f64,
    #[schemars(transform = unit::px_y)]
    pub top: f64,
    #[schemars(transform = unit::px_y)]
    pub bottom: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Spawn {
    #[schemars(transform = unit::px_x)]
    pub x: f64,
    /// Feet height; omit to stand on the highest block below x.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(transform = unit::px_y)]
    pub y: Option<f64>,
    /// 1 = facing right, -1 = left.
    #[schemars(transform = unit::none)]
    pub face: i8,
}

/// Respawn: descend from `startY` to `endY` above `x`, then wait on the respawn platform.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RespawnPoint {
    #[schemars(transform = unit::px_x)]
    pub x: f64,
    #[schemars(transform = unit::px_y)]
    pub start_y: f64,
    #[schemars(transform = unit::px_y)]
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
    #[schemars(transform = unit::px_y)]
    pub edge_width: f64,
    #[schemars(transform = unit::px_y)]
    pub platform_thickness: f64,
    #[schemars(transform = unit::px_xy)]
    pub platform_radius: f64,
    /// Shadow band: starts `[offset]` below the platform top, `[height]` tall, at `[alpha]` opacity.
    #[schemars(transform = unit::shadow_band)]
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
    #[schemars(transform = unit::px_x)]
    pub x: f64,
    #[schemars(transform = unit::px_y)]
    pub y: f64,
    #[schemars(transform = unit::px_x)]
    pub w: f64,
    #[schemars(transform = unit::px_y)]
    pub h: f64,
    #[schemars(transform = unit::px_x)]
    pub every: f64,
    pub color: Color,
    #[schemars(transform = unit::ratio)]
    pub alpha: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Background {
    /// Vertical gradient `[y, color]` stops, top to bottom. Solid above the first and below the last.
    #[schemars(transform = unit::px_y)]
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
    #[schemars(transform = unit::ratio)]
    pub parallax: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Moon {
    #[schemars(transform = unit::px_x)]
    pub x: f64,
    #[schemars(transform = unit::px_y)]
    pub y: f64,
    #[schemars(transform = unit::px_xy)]
    pub r: f64,
    pub color: Color,
    /// Halo: `rings` circles growing by `step` px, each at `alpha`.
    pub glow: Color,
    #[schemars(transform = unit::count)]
    pub rings: u32,
    #[schemars(transform = unit::px_xy)]
    pub step: f64,
    #[schemars(transform = unit::ratio)]
    pub alpha: f64,
}

/// `y = base − amp × (0.5 + 0.3 sin(x × 0.006 + seed) + 0.2 sin(x × 0.017 + 3 seed))`, filled down to the bottom.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ridge {
    #[schemars(transform = unit::px_y)]
    pub base: f64,
    #[schemars(transform = unit::px_y)]
    pub amp: f64,
    #[schemars(transform = unit::none)]
    pub seed: f64,
    pub color: Color,
}

/// Twinkling stars scattered (fixed pseudo-random layout) over the rectangle `x, y, w, h`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Starfield {
    #[schemars(transform = unit::count)]
    pub n: u32,
    #[schemars(transform = unit::px_x)]
    pub x: f64,
    #[schemars(transform = unit::px_y)]
    pub y: f64,
    #[schemars(transform = unit::px_x)]
    pub w: f64,
    #[schemars(transform = unit::px_y)]
    pub h: f64,
    #[schemars(transform = unit::px_xy)]
    pub size: [f64; 2],
    pub color: Color,
    /// Twinkle speed (degrees per tick).
    #[schemars(transform = unit::deg)]
    pub twinkle: f64,
}

impl Stage {
    pub fn spawn_y(&self, x: f64) -> Option<f64> {
        self.blocks.iter().filter(|b| x >= b.left && x <= b.right).map(|b| b.top).min_by(f64::total_cmp)
    }

    pub fn max_players(&self) -> usize {
        self.spawns.len().min(self.respawns.len())
    }
}
