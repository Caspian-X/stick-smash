//! `global.render`: renderer tuning (camera, procedural fighter overlays, effect particles). Purely cosmetic: the
//! sim never reads it. Units are stage pixels (y down unless noted) and ticks; angles in degrees.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::Color;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Render {
    /// Translucent colors' alpha is raised to this power. The GPU blends in linear light, which makes faint
    /// overlays much stronger than the sRGB (canvas) blending the art was tuned for. 2.2 matches sRGB blending
    /// over black, 1 is plain linear blending; ~1.6 suits this game's dark-but-not-black scenes.
    pub alpha_gamma: f64,
    pub camera: Camera,
    pub overlays: Overlays,
    pub fx: FxLook,
    pub hud: Hud,
    pub select: SelectLook,
}

/// Character select presentation (the game crate reads this; the sim never does).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectLook {
    /// Move slot a fighter plays on the stage when its player locks in, before its `win` pose.
    pub flourish: String,
    /// Roster tile figures no player is pointing at.
    pub roster_color: super::global::PlayerColors,
    /// Seconds for a card's READY flip and the ready banner's slide-in.
    pub flip_secs: f32,
    pub banner_secs: f32,
    pub roulette: Roulette,
    /// Stage size labels: play area relative to `camera.referencePlayWidth` / `referencePlayHeight` (the larger
    /// ratio); below `sizeSmall` reads "Small", above `sizeLarge` "Large", otherwise "Medium".
    pub size_small: f64,
    pub size_large: f64,
    /// Stage thumbnails show the play area scaled by `[width, height]` (then widened to the tile's aspect).
    pub thumb_crop: [f64; 2],
    pub pointer: PointerLook,
    pub hints: HintLook,
}

/// Select screens' player pointers. Units are logical UI pixels (the screen is 720 high) and seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PointerLook {
    /// Analog stick: top speed at full tilt, stick response exponent (above 1 is finer near the centre), and how
    /// fast the velocity follows the stick (per second; also how fast it brakes on release).
    pub speed: f32,
    pub curve: f32,
    pub accel: f32,
    /// Stick tilt below this is released.
    pub deadzone: f32,
    /// The magnet pulls a slow pointer onto the nearest target within `magnetRadius` of its centre (any distance once
    /// the stick is released), at `magnetStrength` per second. It starts below `magnetSpeed` of the top speed and is
    /// full strength at rest.
    pub magnet_radius: f32,
    pub magnet_strength: f32,
    pub magnet_speed: f32,
    /// A gliding pointer hovers targets up to this far outside their edges.
    pub reach: f32,
    /// Digital presses hop to the neighbouring target, sliding there in this many seconds.
    pub slide_secs: f32,
    /// Pointers resting on one target fan out by this much per player.
    pub fan: f32,
    /// Squash and bounce when a pointer lands (`land`) or drops a chip (`drop`): scale amplitude, fading out over
    /// `bounceSecs` while wobbling `bounceWobble` radians per second.
    pub land: f32,
    pub drop: f32,
    pub bounce_secs: f32,
    pub bounce_wobble: f32,
    /// Stroke thickness and size scale of the arrow.
    pub stroke: f32,
    pub scale: f32,
}

/// Per-card control hints: shown for `showSecs` after joining, after `idleSecs` without input, and while the
/// player points at their own card; fading in and out over `fadeSecs`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HintLook {
    pub show_secs: f32,
    pub idle_secs: f32,
    pub fade_secs: f32,
}

/// Stage select vote roulette: a highlight hops over the voted stages, slowing down, and lands on the winner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Roulette {
    /// Seconds from the first hop to the landing.
    pub secs: f32,
    /// Hops before the landing (more distinct votes only change where it starts).
    pub hops: u32,
    /// Hop `k` of `hops` happens at `secs * (k / hops)^curve`: above 1 the hops slow down.
    pub curve: f32,
    /// Seconds the landed stage shows before the match starts.
    pub pause: f32,
    /// The landed tile's scale, plus a bounce of `landBounce` (fading out over the pause) bouncing `landWobble` radians over the pause.
    pub land_scale: f32,
    pub land_bounce: f32,
    pub land_wobble: f32,
}

/// In-match HUD and match-flow presentation (the game crate reads this; the sim never does).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hud {
    /// Percent number color by damage: `[percent, color]` stops, interpolated, sorted by percent.
    pub damage_ramp: Vec<(f64, Color)>,
    /// Percent number shake (logical px) while the sim's `hudShake` timer runs, scaled by the timer.
    pub shake: f64,
    /// When the match ends ("GAME!"), the sim runs at `speed` for its first `ticks` end-freeze ticks.
    pub end_slowmo: Slowmo,
    /// Ticks the winner pose shows after the match is over, before the results screen.
    pub results_delay: u32,
    /// Results stats: a fall counts as a KO for the last hitter if they hit within this many ticks, otherwise
    /// as a self-destruct.
    pub ko_credit_ticks: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Slowmo {
    pub speed: f64,
    pub ticks: u32,
}

/// Follow camera. Each tick it frames every fighter that's on screen-worthy (not dead/out), padded, keeps the
/// window's aspect ratio, and eases toward that framing. It never shows past the stage's `camera` bounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Camera {
    /// Padding beside the fighters' feet.
    pub pad_x: f64,
    /// Padding above the fighters' feet (include the body height).
    pub pad_top: f64,
    pub pad_bottom: f64,
    /// Zoom limits as visible height in stage pixels: `minHeight` = closest zoom, `maxHeight` = widest.
    pub min_height: f64,
    pub max_height: f64,
    /// Reference play-area width before camera padding; the bundled value is Arena’s play area.
    #[serde(default = "reference_play_width")]
    pub reference_play_width: f64,
    /// Reference play-area height (including block undersides), before camera padding; the bundled value is Arena’s play area.
    #[serde(default = "reference_play_height")]
    pub reference_play_height: f64,
    /// Only stages smaller than this fraction of both reference dimensions zoom in tighter.
    #[serde(default = "min_scale_start")]
    pub min_scale_start: f64,
    /// Smallest automatic minHeight multiplier; scaling below minScaleStart is linear.
    #[serde(default = "min_scale_floor")]
    pub min_scale_floor: f64,
    /// 0..1: how far the framing center is pulled toward the stage's center (keeps the stage in view).
    pub stage_bias: f64,
    /// Fighters are framed no further than this outside the blast zone (keeps KO'd fighters from yanking it).
    pub blast_margin: f64,
    /// Fraction of the remaining distance covered per tick, for position and zoom. 1 = no smoothing.
    pub follow: f64,
    pub zoom_follow: f64,
    /// Screen shake (stage pixels): multiplied by `shakeDecay` each tick, cut off below `shakeMin`.
    pub shake_decay: f64,
    pub shake_min: f64,
}

// Older replay payloads omit these presentation fields. Keep fallback tuning in the bundled JSON too.
fn camera_default(key: &str) -> f64 {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../../../../assets/global.json")).expect("bundled global JSON");
    v["render"]["camera"][key].as_f64().expect("bundled camera tuning")
}
fn reference_play_width() -> f64 {
    camera_default("referencePlayWidth")
}
fn reference_play_height() -> f64 {
    camera_default("referencePlayHeight")
}
fn min_scale_start() -> f64 {
    camera_default("minScaleStart")
}
fn min_scale_floor() -> f64 {
    camera_default("minScaleFloor")
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Overlays {
    /// Grounded `idle` fighters moving faster than this (px/tick) use the `run` animation.
    pub run_speed: f64,
    /// Opacity of the soft glow under the figure.
    pub glow_alpha: f64,
    /// Body color while flashing from a hit.
    pub flash: Color,
    /// Body and back-limb colors while helpless.
    pub helpless: Color,
    pub helpless_back: Color,
    /// Random sideways jitter (px) while in hitlag after being hit.
    pub hit_jitter: f64,
    pub charge: ChargeLook,
    /// Invincibility blinking after respawn: alpha `alpha` every other `period` ticks.
    pub blink_period: u32,
    pub blink_alpha: f64,
    pub dodge: DodgeLook,
    pub shield: ShieldLook,
    pub dizzy: DizzyLook,
    pub respawn: RespawnLook,
    pub trail: TrailLook,
    /// Line width of the X eyes (knocked down, dizzy).
    pub x_eye_width: f64,
}

/// Smash charge look: body blinks to `color` every `blink` ticks, jitters, and glows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChargeLook {
    pub color: Color,
    pub blink: u32,
    pub jitter: f64,
    pub glow_alpha: f64,
}

/// Dodging (invulnerable roll, spot dodge, air dodge, tech): pale body, ghosts behind along the velocity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DodgeLook {
    pub alpha: f64,
    pub glow: Color,
    pub glow_alpha: f64,
    pub ghosts: u32,
    /// Ghost k sits `k × spacing` ticks of velocity behind.
    pub spacing: f64,
    /// Nearest ghost's opacity; each further ghost fades by the same step.
    pub ghost_alpha: f64,
    pub ghost_width: f64,
}

/// Shield bubble: centered `y` above the feet, radius `minR + growR × shield HP / max`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShieldLook {
    pub y: f64,
    pub min_r: f64,
    pub grow_r: f64,
    pub alpha: f64,
    /// Fill opacity during shield stun (the outline turns white too).
    pub stun_alpha: f64,
    pub width: f64,
    pub stroke_alpha: f64,
}

/// Stars circling the head while dizzy: an ellipse `rx`×`ry` centered `y` above the feet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DizzyLook {
    pub stars: u32,
    pub y: f64,
    pub rx: f64,
    pub ry: f64,
    pub r: f64,
    /// Degrees per tick.
    pub speed: f64,
    pub color: Color,
}

/// Glowing platform under a respawning fighter: an ellipse `w`×`h` centered `y` below the feet, pulsing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RespawnLook {
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub alpha: f64,
    /// Opacity swing and its speed (degrees per tick).
    pub pulse: f64,
    pub speed: f64,
}

/// Attachment swing trail: covers the last `frames` ticks of the swing, fades out over `fade` ticks after the
/// hitbox ends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrailLook {
    pub frames: u32,
    pub fade: u32,
    pub alpha: f64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FxLook {
    /// Particle horizontal speed multiplier per tick.
    pub drag: f64,
    /// Particles shrink to this fraction of their size by the end of their life.
    pub shrink: f64,
    pub ring_width: f64,
    pub dust: DustLook,
    pub beam: BeamLook,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DustLook {
    pub color: Color,
    /// Random horizontal offset, ±.
    pub spread: f64,
    /// Random horizontal speed, ±.
    pub vx: f64,
    /// Random vertical speed range (y down).
    pub vy: [f64; 2],
    pub life: f64,
    pub size: [f64; 2],
}

/// KO blast: a beam from the KO point toward the stage center, sparks and a ring. The point is clamped inside
/// the view by `inset`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BeamLook {
    pub life: u32,
    pub length: f64,
    /// Half-width at the far end, growing by `grow` over the beam's life.
    pub width: f64,
    pub grow: f64,
    /// Bands from the KO point outward, each fading more. The first band is white.
    pub bands: u32,
    pub alpha: f64,
    pub inset: f64,
    pub bursts: Vec<BurstLook>,
    pub ring: RingLook,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BurstLook {
    pub color: Color,
    pub n: u32,
    pub speed: f64,
    pub life: f64,
    pub size: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RingLook {
    pub r: f64,
    pub grow: f64,
    pub life: u32,
    pub color: Color,
}
