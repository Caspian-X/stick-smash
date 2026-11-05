//! Game-side procedural audio. Seconds and Hz (not simulation ticks).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::unit;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Audio {
    /// Mono PCM sample rate, 8000..96000 Hz.
    #[schemars(transform = unit::hz)]
    pub sample_rate: u32,
    /// Starting slider values, each 0..1. Saved settings override these.
    pub defaults: Volumes,
    /// Slider increment per left/right press, 0..1.
    #[schemars(transform = unit::ratio)]
    pub volume_step: f64,
    /// Maximum simultaneous voices, including sounds still waiting for an audio device.
    #[schemars(transform = unit::count)]
    pub max_voices: usize,
    /// Maximum new voices per rendered frame.
    #[schemars(transform = unit::count)]
    pub max_per_frame: usize,
    /// Minimum wall-clock seconds between repeats of the same sound.
    #[schemars(transform = unit::s)]
    pub cooldown: f64,
    /// Screen-space emitter displacement relative to listener ears; 0 disables panning.
    #[schemars(transform = unit::none)]
    pub pan: f64,
    /// Listener-to-emitter depth in normalized screen units (keeps attenuation mild).
    #[schemars(transform = unit::none)]
    pub depth: f64,
    /// Stereo listener ear separation in normalized screen units.
    #[schemars(transform = unit::none)]
    pub ear_gap: f64,
    /// Local pitch variation as a fraction (0..0.5), independent of sim RNG and training speed.
    #[schemars(transform = unit::ratio)]
    pub pitch_variation: f64,
    /// Number of pre-synthesized strength levels per sound, 1..16.
    #[schemars(transform = unit::count)]
    pub strength_levels: usize,
    /// Knockback/damage at which a scaled sound reaches full strength.
    #[schemars(transform = unit::v_xy)]
    pub strength_max: f64,
    /// Sound names referenced by sim/content and UI. Unknown names are skipped with a warning.
    pub sounds: BTreeMap<String, Sound>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Volumes {
    #[schemars(transform = unit::ratio)]
    pub master: f64,
    #[schemars(transform = unit::ratio)]
    pub sfx: f64,
    /// Reserved for future music; currently silent.
    #[schemars(transform = unit::ratio)]
    pub music: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sound {
    /// Mixer gain, 0..1.
    #[schemars(transform = unit::ratio)]
    pub volume: f64,
    /// Higher priorities win frame and voice limits (KO > hit > movement).
    #[schemars(transform = unit::none)]
    pub priority: u32,
    /// Additional minimum real seconds between repeats.
    #[schemars(transform = unit::s)]
    pub cooldown: f64,
    /// Duration multiplier at maximum impact (interpolated from 1).
    #[schemars(transform = unit::ratio)]
    pub strength_duration: f64,
    /// Gain multiplier at minimum impact (interpolated to 1).
    #[schemars(transform = unit::ratio)]
    pub strength_gain: f64,
    pub layers: Vec<Layer>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layer {
    pub wave: Wave,
    /// Exponential frequency sweep in Hz. For noise, a swept one-pole low-pass cutoff.
    #[schemars(transform = unit::hz)]
    pub from: f64,
    #[schemars(transform = unit::hz)]
    pub to: f64,
    /// Layer length in seconds, 0..3.
    #[schemars(transform = unit::s)]
    pub duration: f64,
    /// Click-free attack length in seconds.
    #[schemars(transform = unit::s)]
    pub attack: f64,
    /// Exponential amplitude decay factor over the layer.
    #[schemars(transform = unit::ratio)]
    pub decay: f64,
    #[schemars(transform = unit::ratio)]
    pub gain: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Wave {
    Sine,
    Triangle,
    Square,
    Saw,
    Noise,
}
