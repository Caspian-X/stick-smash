//! `global.render`: renderer tuning (camera, procedural fighter overlays, effect particles). Purely cosmetic: the
//! sim never reads it. Units are stage pixels (y down unless noted) and ticks; angles in degrees.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::Color;
use super::unit;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Render {
    /// Translucent colors' alpha is raised to this power. The GPU blends in linear light, which makes faint
    /// overlays much stronger than the sRGB (canvas) blending the art was tuned for. 2.2 matches sRGB blending
    /// over black, 1 is plain linear blending; ~1.6 suits this game's dark-but-not-black scenes.
    #[schemars(transform = unit::none)]
    pub alpha_gamma: f64,
    pub camera: Camera,
    pub overlays: Overlays,
    pub fx: FxLook,
    pub hud: Hud,
    pub select: SelectLook,
    pub interpolate: Interpolate,
    /// Visible KO boundaries during gameplay. Omit to hide them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blast_zone: Option<BlastZoneLook>,
}

/// Dashed KO boundary. Width, dash and gap are in screen pixels, independent of camera zoom.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlastZoneLook {
    pub color: Color,
    #[schemars(transform = unit::ratio)]
    pub alpha: f64,
    #[schemars(transform = unit::uipx)]
    pub width: f64,
    #[schemars(transform = unit::uipx)]
    pub dash: f64,
    #[schemars(transform = unit::uipx)]
    pub gap: f64,
}

/// Drawing between ticks: something that moved further than this (stage pixels) in one tick teleported and is
/// drawn where it is now instead of sliding there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Interpolate {
    #[schemars(transform = unit::px_xy)]
    pub fighter_jump: f64,
    #[schemars(transform = unit::px_xy)]
    pub projectile_jump: f64,
}

/// Names of the CPU levels (`PlayerConfig::cpu`), for menus.
pub const CPU_LEVELS: [&str; 3] = ["Easy", "Normal", "Hard"];

/// Character select presentation (the game crate reads this; the sim never does).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectLook {
    /// Real seconds holding Back before leaving a selection screen.
    #[serde(default = "back_hold_secs")]
    #[schemars(transform = unit::s)]
    pub back_hold_secs: f32,
    /// Move slot a fighter plays on the stage when its player locks in, before its `win` pose.
    pub flourish: String,
    /// Roster tile figures no player is pointing at.
    pub roster_color: super::global::PlayerColors,
    /// Seconds for a card's READY flip and the ready banner's slide-in.
    #[schemars(transform = unit::s)]
    pub flip_secs: f32,
    #[schemars(transform = unit::s)]
    pub banner_secs: f32,
    /// Overshoot of the flip, chip drop and banner ease (back-out; 0 is none).
    #[schemars(transform = unit::ratio)]
    pub overshoot: f32,
    /// A dropped chip falls in from `dropHeight` pixels above, `dropScale` bigger.
    #[schemars(transform = unit::ratio)]
    pub drop_scale: f32,
    #[schemars(transform = unit::uipx)]
    pub drop_height: f32,
    /// The ready banner slides in from this far left (percent of the window width).
    #[schemars(transform = unit::ratio)]
    pub banner_from: f32,
    pub roulette: Roulette,
    /// Stage size labels: play area relative to `camera.referencePlayWidth` / `referencePlayHeight` (the larger
    /// ratio); below `sizeSmall` reads "Small", above `sizeLarge` "Large", otherwise "Medium".
    #[schemars(transform = unit::ratio)]
    pub size_small: f64,
    #[schemars(transform = unit::ratio)]
    pub size_large: f64,
    /// Stage thumbnails show the play area scaled by `[width, height]` (then widened to the tile's aspect).
    #[schemars(transform = unit::ratio)]
    pub thumb_crop: [f64; 2],
    pub pointer: PointerLook,
    pub back_ring: BackRing,
    pub hints: HintLook,
}

/// The ring that fills around a pointer while its player holds Back to leave a select screen. Units are logical UI
/// pixels (the screen is 720 high) and seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackRing {
    /// Its centre from the arrow's tip, and its radius (both scaled by `pointer.scale`).
    #[schemars(transform = unit::uipx)]
    pub offset: [f32; 2],
    #[schemars(transform = unit::uipx)]
    pub radius: f32,
    /// Each dash's thickness, and how many dashes go round the ring.
    #[schemars(transform = unit::uipx)]
    pub thickness: f32,
    #[schemars(transform = unit::count)]
    pub ticks: u32,
    /// A dash's length as its share of the circle (below 1 leaves gaps), and an unfilled dash's opacity.
    #[schemars(transform = unit::ratio)]
    pub dash: f32,
    #[schemars(transform = unit::ratio)]
    pub track: f32,
    /// Each dash's dark rim, so the gauge reads over a portrait (0 leaves it out).
    #[schemars(transform = unit::uipx)]
    pub rim: f32,
    /// The filled end whitens the `hot` dashes behind it like a comet head.
    #[schemars(transform = unit::count)]
    pub hot: f32,
    /// The spark riding the filled end, as a multiple of the dash thickness.
    #[schemars(transform = unit::ratio)]
    pub spark: f32,
    /// Springs as (stiffness, damping ratio): the filled part chases the hold, and the ring pops in as it starts.
    #[schemars(transform = unit::none)]
    pub follow: [f32; 2],
    #[schemars(transform = unit::none)]
    pub grow: [f32; 2],
    /// Scale a starting hold grows from.
    #[schemars(transform = unit::ratio)]
    pub pop: f32,
    /// While it fills, the ring swings `wobble` radians one way and back every `wobble_secs`.
    #[schemars(transform = unit::rad)]
    pub wobble: f32,
    #[schemars(transform = unit::s)]
    pub wobble_secs: f32,
}

/// Select screens' player pointers. Units are logical UI pixels (the screen is 720 high) and seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PointerLook {
    /// Analog stick: top speed at full tilt, stick response exponent (above 1 is finer near the centre), and how
    /// fast the velocity follows the stick (per second; also how fast it brakes on release).
    #[schemars(transform = unit::uipx)]
    pub speed: f32,
    #[schemars(transform = unit::none)]
    pub curve: f32,
    #[schemars(transform = unit::hz)]
    pub accel: f32,
    /// Stick tilt below this is released.
    #[schemars(transform = unit::ratio)]
    pub deadzone: f32,
    /// The magnet pulls a slow pointer onto the nearest target within `magnetRadius` of its centre (any distance once
    /// the stick is released), at `magnetStrength` per second. It starts below `magnetSpeed` of the top speed and is
    /// full strength at rest.
    #[schemars(transform = unit::uipx)]
    pub magnet_radius: f32,
    #[schemars(transform = unit::hz)]
    pub magnet_strength: f32,
    #[schemars(transform = unit::ratio)]
    pub magnet_speed: f32,
    /// A gliding pointer hovers targets up to this far outside their edges.
    #[schemars(transform = unit::uipx)]
    pub reach: f32,
    /// Digital presses hop to the neighbouring target, sliding there in this many seconds with a back-out ease of
    /// this overshoot.
    #[schemars(transform = unit::s)]
    pub slide_secs: f32,
    #[schemars(transform = unit::ratio)]
    pub slide_overshoot: f32,
    /// Pointers resting on one target fan out by this much per player (at most a fifth of its width), from `rest` of
    /// its height above its centre (so the chip riding on the arrow stays clear of a tile's name).
    #[schemars(transform = unit::uipx)]
    pub fan: f32,
    #[schemars(transform = unit::ratio)]
    pub rest: f32,
    /// Squash and bounce when a pointer lands (`land`) or drops a chip (`drop`): scale amplitude, fading out over
    /// `bounceSecs` while wobbling `bounceWobble` radians per second.
    #[schemars(transform = unit::ratio)]
    pub land: f32,
    #[schemars(transform = unit::ratio)]
    pub drop: f32,
    #[schemars(transform = unit::s)]
    pub bounce_secs: f32,
    #[schemars(transform = unit::rad)]
    pub bounce_wobble: f32,
    /// Stroke thickness and size scale of the arrow.
    #[schemars(transform = unit::uipx)]
    pub stroke: f32,
    #[schemars(transform = unit::ratio)]
    pub scale: f32,
    pub motion: PointerMotion,
}

/// How a pointer reacts to its motion. Springs have a stiffness (per second squared) and a damping ratio (1 is
/// critical, lower swings).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PointerMotion {
    /// Leans into its horizontal travel by `lean` radians per pixel per second, at most `leanMax`.
    #[schemars(transform = unit::none)]
    pub lean: f32,
    #[schemars(transform = unit::rad)]
    pub lean_max: f32,
    #[schemars(transform = unit::none)]
    pub lean_spring: [f32; 2],
    /// Stretches along its travel by `stretch` per pixel per second, at most `stretchMax`; springing back past rest
    /// squashes it as it stops.
    #[schemars(transform = unit::none)]
    pub stretch: f32,
    #[schemars(transform = unit::ratio)]
    pub stretch_max: f32,
    #[schemars(transform = unit::none)]
    pub stretch_spring: [f32; 2],
    /// A carried chip trails behind on a spring, softly limited to `chipLag` pixels away, and swings `chipSwing`
    /// radians per pixel of sideways lag.
    #[schemars(transform = unit::none)]
    pub chip_spring: [f32; 2],
    #[schemars(transform = unit::uipx)]
    pub chip_lag: f32,
    #[schemars(transform = unit::none)]
    pub chip_swing: f32,
    /// Still (below `stillSpeed` pixels per second) for `breathAfter` seconds, it breathes: a bob of `breathBob`
    /// pixels and a `breathScale` swell every `breathSecs`, fading in over a second, each player out of phase.
    #[schemars(transform = unit::uipx)]
    pub still_speed: f32,
    #[schemars(transform = unit::s)]
    pub breath_after: f32,
    #[schemars(transform = unit::s)]
    pub breath_secs: f32,
    #[schemars(transform = unit::uipx)]
    pub breath_bob: f32,
    #[schemars(transform = unit::ratio)]
    pub breath_scale: f32,
    /// Springs step in slices of at most 1 / `substeps` seconds, so any frame rate moves alike.
    #[schemars(transform = unit::hz)]
    pub substeps: f32,
}

/// Per-card control hints: shown for `showSecs` after joining, after `idleSecs` without input, and while the
/// player points at their own card; fading in and out over `fadeSecs`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HintLook {
    #[schemars(transform = unit::s)]
    pub show_secs: f32,
    #[schemars(transform = unit::s)]
    pub idle_secs: f32,
    #[schemars(transform = unit::s)]
    pub fade_secs: f32,
}

/// Stage select vote roulette: a highlight hops over the voted stages, slowing down, and lands on the winner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Roulette {
    /// Seconds from the first hop to the landing.
    #[schemars(transform = unit::s)]
    pub secs: f32,
    /// Hops before the landing (more distinct votes only change where it starts).
    #[schemars(transform = unit::count)]
    pub hops: u32,
    /// Hop `k` of `hops` happens at `secs * (k / hops)^curve`: above 1 the hops slow down.
    #[schemars(transform = unit::none)]
    pub curve: f32,
    /// Seconds the landed stage shows before the match starts.
    #[schemars(transform = unit::s)]
    pub pause: f32,
    /// The landed tile's scale, plus a bounce of `landBounce` (fading out over the pause) bouncing `landWobble` radians over the pause.
    #[schemars(transform = unit::ratio)]
    pub land_scale: f32,
    #[schemars(transform = unit::ratio)]
    pub land_bounce: f32,
    #[schemars(transform = unit::rad)]
    pub land_wobble: f32,
}

/// In-match HUD and match-flow presentation (the game crate reads this; the sim never does).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hud {
    /// Percent number color by damage: `[percent, color]` stops, interpolated, sorted by percent.
    #[schemars(transform = unit::damage)]
    pub damage_ramp: Vec<(f64, Color)>,
    /// Percent number shake (logical px) while the sim's `hudShake` timer runs, scaled by the timer.
    #[schemars(transform = unit::uipx)]
    pub shake: f64,
    /// When the match ends ("GAME!"), the sim runs at `speed` for its first `ticks` end-freeze ticks. This
    /// stretches real time by `ticks / speed`.
    pub end_slowmo: Slowmo,
    /// The end-of-match banner ("GAME!" / "TIME!") and its handoff to the results screen, timed in real seconds.
    /// The results never wait for `match.endFrames`: only the winner's pose does.
    #[serde(default)]
    pub end_banner: EndBanner,
    /// Ticks the winner pose shows after the match is over, before the results screen (`0`: it opens as the pose
    /// starts). [`Hud::end_banner`] usually opens the screen first.
    #[schemars(transform = unit::tick)]
    pub results_delay: u32,
    /// Real seconds the focused results button must be held before it fires, so a press buffered from the match
    /// cannot pick one.
    #[serde(default = "results_hold_secs")]
    #[schemars(transform = unit::s)]
    pub results_hold_secs: f32,
    /// How the results screen enters (it arrives while the end freeze is still running).
    #[serde(default)]
    pub results_enter: ResultsEnter,
    /// Results stats: a fall counts as a KO for the last hitter if they hit within this many ticks, otherwise
    /// as a self-destruct.
    #[schemars(transform = unit::tick)]
    pub ko_credit_ticks: u64,
    pub look: HudLook,
}

/// The results screen's entrance, timed in real seconds so display refresh and the end freeze don't change it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultsEnter {
    /// Real seconds an element takes to settle once its entrance starts.
    #[schemars(transform = unit::s)]
    pub secs: f32,
    /// Logical pixels the content rises into place; the backdrop darkens over the same time.
    #[schemars(transform = unit::uipx)]
    pub slide: f32,
    /// Real seconds between one card's entrance and the next (the buttons follow the cards).
    #[schemars(transform = unit::s)]
    pub stagger: f32,
}

impl Default for ResultsEnter {
    fn default() -> Self {
        Self { secs: 0.3, slide: 30., stagger: 0.035 }
    }
}

/// The end banner's real-time clock: it slams in, holds, then fades out, and the results screen enters from the
/// moment it's gone. Keeping it in real seconds means the end slow motion can't stretch the banner or skip it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EndBanner {
    /// From the deciding KO (or TIME!) to the results screen.
    #[schemars(transform = unit::s)]
    pub secs: f32,
    /// The slam-in: the banner starts at `look.popScale`, fading in, and settles over this many seconds.
    #[schemars(transform = unit::s)]
    pub pop: f32,
    /// The fade-out, over the last of `secs`.
    #[schemars(transform = unit::s)]
    pub fade: f32,
}

impl Default for EndBanner {
    fn default() -> Self {
        Self { secs: 1., pop: 0.2, fade: 0.25 }
    }
}

/// Where name tags and offscreen bubbles sit and how HUD text moves. Logical UI pixels unless noted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HudLook {
    /// Percent shake speed, radians per second.
    #[schemars(transform = unit::rad)]
    pub shake_speed: f32,
    /// Name tags float `tagHeight` stage pixels above the feet and show while the point `anchorHeight` above the
    /// feet is on screen; offscreen bubbles point at that anchor.
    #[schemars(transform = unit::px_y)]
    pub tag_height: f64,
    #[schemars(transform = unit::px_y)]
    pub anchor_height: f64,
    /// Offscreen bubbles shrink with distance, down to `bubbleMinScale` at `bubbleShrink` pixels away.
    #[schemars(transform = unit::uipx)]
    pub bubble_shrink: f32,
    #[schemars(transform = unit::ratio)]
    pub bubble_min_scale: f32,
    /// Countdown / GO! / GAME! text pops in from `popScale`, settling in the first 1 / `popSpeed` of its time.
    #[schemars(transform = unit::ratio)]
    pub pop_scale: f32,
    #[schemars(transform = unit::ratio)]
    pub pop_speed: f32,
    /// The match timer's scale bump on each second of its warning time, back to 1 by the next second.
    #[serde(default = "default_timer_pulse")]
    #[schemars(transform = unit::ratio)]
    pub timer_pulse: f32,
}
fn default_timer_pulse() -> f32 {
    0.16
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Slowmo {
    #[schemars(transform = unit::ratio)]
    pub speed: f64,
    #[schemars(transform = unit::tick)]
    pub ticks: u32,
}

/// Follow camera. Each tick it frames every fighter that's on screen-worthy (not dead/out), padded, keeps the
/// window's aspect ratio, and eases toward that framing. It never shows past the stage's `camera` bounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Camera {
    /// Padding beside the fighters' feet.
    #[schemars(transform = unit::px_x)]
    pub pad_x: f64,
    /// Padding above the fighters' feet (include the body height).
    #[schemars(transform = unit::px_y)]
    pub pad_top: f64,
    #[schemars(transform = unit::px_y)]
    pub pad_bottom: f64,
    /// Zoom limits as visible height in stage pixels: `minHeight` = closest zoom, `maxHeight` = widest.
    #[schemars(transform = unit::px_y)]
    pub min_height: f64,
    #[schemars(transform = unit::px_y)]
    pub max_height: f64,
    /// Reference play-area width before camera padding; the bundled value is Arena’s play area.
    #[schemars(transform = unit::px_x)]
    pub reference_play_width: f64,
    /// Reference play-area height (including block undersides), before camera padding; the bundled value is Arena’s play area.
    #[schemars(transform = unit::px_y)]
    pub reference_play_height: f64,
    /// Only stages smaller than this fraction of both reference dimensions zoom in tighter.
    #[schemars(transform = unit::ratio)]
    pub min_scale_start: f64,
    /// Smallest automatic minHeight multiplier; scaling below minScaleStart is linear.
    #[schemars(transform = unit::ratio)]
    pub min_scale_floor: f64,
    /// 0..1: how far the framing center is pulled toward the stage's center (keeps the stage in view).
    #[schemars(transform = unit::ratio)]
    pub stage_bias: f64,
    /// Fighters are framed no further than this outside the blast zone (keeps KO'd fighters from yanking it).
    #[schemars(transform = unit::px_xy)]
    pub blast_margin: f64,
    /// Fraction of the remaining distance covered per tick, for position and zoom. 1 = no smoothing.
    #[schemars(transform = unit::ratio)]
    pub follow: f64,
    #[schemars(transform = unit::ratio)]
    pub zoom_follow: f64,
    /// Screen shake (stage pixels): multiplied by `shakeDecay` each tick, cut off below `shakeMin`.
    #[schemars(transform = unit::ratio)]
    pub shake_decay: f64,
    #[schemars(transform = unit::px_xy)]
    pub shake_min: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Overlays {
    /// Grounded `idle` fighters moving faster than this (px/tick) use the `run` animation.
    #[schemars(transform = unit::v_x)]
    pub run_speed: f64,
    /// Opacity of the soft glow under the figure.
    #[schemars(transform = unit::ratio)]
    pub glow_alpha: f64,
    /// Body color while flashing from a hit.
    pub flash: Color,
    /// Body and back-limb colors while helpless.
    pub helpless: Color,
    pub helpless_back: Color,
    /// Random sideways jitter (px) while in hitlag after being hit.
    #[schemars(transform = unit::px_x)]
    pub hit_jitter: f64,
    pub charge: ChargeLook,
    /// Invincibility blinking after respawn: alpha `alpha` every other `period` ticks.
    #[schemars(transform = unit::tick)]
    pub blink_period: u32,
    #[schemars(transform = unit::ratio)]
    pub blink_alpha: f64,
    pub dodge: DodgeLook,
    pub shield: ShieldLook,
    pub dizzy: DizzyLook,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grabbed: Option<GrabLook>,
    pub respawn: RespawnLook,
    pub trail: TrailLook,
    /// Line width of the X eyes (knocked down, dizzy).
    #[schemars(transform = unit::px_xy)]
    pub x_eye_width: f64,
}

/// Smash charge look: body blinks to `color` every `blink` ticks, jitters, and glows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChargeLook {
    pub color: Color,
    #[schemars(transform = unit::tick)]
    pub blink: u32,
    #[schemars(transform = unit::px_xy)]
    pub jitter: f64,
    #[schemars(transform = unit::ratio)]
    pub glow_alpha: f64,
}

/// Dodging (invulnerable roll, spot dodge, air dodge, tech): pale body, ghosts behind along the velocity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DodgeLook {
    #[schemars(transform = unit::ratio)]
    pub alpha: f64,
    pub glow: Color,
    #[schemars(transform = unit::ratio)]
    pub glow_alpha: f64,
    #[schemars(transform = unit::count)]
    pub ghosts: u32,
    /// Ghost k sits `k × spacing` ticks of velocity behind.
    #[schemars(transform = unit::tick)]
    pub spacing: f64,
    /// Nearest ghost's opacity; each further ghost fades by the same step.
    #[schemars(transform = unit::ratio)]
    pub ghost_alpha: f64,
    #[schemars(transform = unit::px_xy)]
    pub ghost_width: f64,
}

/// Shield bubble: centered `y` above the feet, radius `minR + growR × shield HP / max`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShieldLook {
    #[schemars(transform = unit::px_y)]
    pub y: f64,
    #[schemars(transform = unit::px_xy)]
    pub min_r: f64,
    #[schemars(transform = unit::px_xy)]
    pub grow_r: f64,
    #[schemars(transform = unit::ratio)]
    pub alpha: f64,
    /// Fill opacity during shield stun (the outline turns white too).
    #[schemars(transform = unit::ratio)]
    pub stun_alpha: f64,
    #[schemars(transform = unit::px_xy)]
    pub width: f64,
    #[schemars(transform = unit::ratio)]
    pub stroke_alpha: f64,
}

/// Stars circling the head while dizzy: an ellipse `rx`×`ry` centered `y` above the feet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DizzyLook {
    #[schemars(transform = unit::count)]
    pub stars: u32,
    #[schemars(transform = unit::px_y)]
    pub y: f64,
    #[schemars(transform = unit::px_x)]
    pub rx: f64,
    #[schemars(transform = unit::px_y)]
    pub ry: f64,
    #[schemars(transform = unit::px_xy)]
    pub r: f64,
    /// Degrees per tick.
    #[schemars(transform = unit::deg)]
    pub speed: f64,
    pub color: Color,
}

/// Glowing platform under a respawning fighter: an ellipse `w`×`h` centered `y` below the feet, pulsing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RespawnLook {
    #[schemars(transform = unit::px_y)]
    pub y: f64,
    #[schemars(transform = unit::px_x)]
    pub w: f64,
    #[schemars(transform = unit::px_y)]
    pub h: f64,
    #[schemars(transform = unit::ratio)]
    pub alpha: f64,
    /// Opacity swing and its speed (degrees per tick).
    #[schemars(transform = unit::ratio)]
    pub pulse: f64,
    #[schemars(transform = unit::deg)]
    pub speed: f64,
}

/// Attachment swing trail: covers the last `frames` ticks of the swing, fades out over `fade` ticks after the
/// hitbox ends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrailLook {
    #[schemars(transform = unit::tick)]
    pub frames: u32,
    #[schemars(transform = unit::tick)]
    pub fade: u32,
    #[schemars(transform = unit::ratio)]
    pub alpha: f64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FxLook {
    /// Particle horizontal speed multiplier per tick.
    #[schemars(transform = unit::ratio)]
    pub drag: f64,
    /// Particles shrink to this fraction of their size by the end of their life.
    #[schemars(transform = unit::ratio)]
    pub shrink: f64,
    #[schemars(transform = unit::px_xy)]
    pub ring_width: f64,
    pub dust: DustLook,
    pub beam: BeamLook,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DustLook {
    pub color: Color,
    /// Random horizontal offset, ±.
    #[schemars(transform = unit::px_x)]
    pub spread: f64,
    /// Random horizontal speed, ±.
    #[schemars(transform = unit::v_x)]
    pub vx: f64,
    /// Random vertical speed range (y down).
    #[schemars(transform = unit::v_y)]
    pub vy: [f64; 2],
    #[schemars(transform = unit::tick)]
    pub life: f64,
    #[schemars(transform = unit::px_xy)]
    pub size: [f64; 2],
}

/// KO blast: a beam from the KO point toward the stage center, sparks and a ring. The point is clamped inside
/// the view by `inset`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BeamLook {
    #[schemars(transform = unit::tick)]
    pub life: u32,
    #[schemars(transform = unit::px_xy)]
    pub length: f64,
    /// Half-width at the far end, growing by `grow` over the beam's life.
    #[schemars(transform = unit::px_xy)]
    pub width: f64,
    #[schemars(transform = unit::px_xy)]
    pub grow: f64,
    /// Bands from the KO point outward, each fading more. The first band is white.
    #[schemars(transform = unit::count)]
    pub bands: u32,
    #[schemars(transform = unit::ratio)]
    pub alpha: f64,
    #[schemars(transform = unit::px_xy)]
    pub inset: f64,
    pub bursts: Vec<BurstLook>,
    pub ring: RingLook,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BurstLook {
    pub color: Color,
    #[schemars(transform = unit::count)]
    pub n: u32,
    #[schemars(transform = unit::v_xy)]
    pub speed: f64,
    #[schemars(transform = unit::tick)]
    pub life: f64,
    #[schemars(transform = unit::px_xy)]
    pub size: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RingLook {
    #[schemars(transform = unit::px_xy)]
    pub r: f64,
    #[schemars(transform = unit::v_xy)]
    pub grow: f64,
    #[schemars(transform = unit::tick)]
    pub life: u32,
    pub color: Color,
}

fn back_hold_secs() -> f32 {
    1.5
}

fn results_hold_secs() -> f32 {
    0.25
}

/// Binding bands [height, horizontal radius, vertical radius], as fractions of fighter body height.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrabLook {
    #[schemars(transform = unit::ratio)]
    pub bands: Vec<[f64; 3]>,
    #[schemars(transform = unit::px_xy)]
    pub width: f64,
    pub color: Color,
    #[schemars(transform = unit::ratio)]
    pub alpha: f64,
}
