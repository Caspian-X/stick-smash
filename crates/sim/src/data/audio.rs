//! Game-side procedural audio. Seconds and Hz (not simulation ticks).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Audio {
    /// Mono PCM sample rate, 8000..96000 Hz.
    pub sample_rate: u32,
    /// Starting slider values, each 0..1. Saved settings override these.
    pub defaults: Volumes,
    /// Slider increment per left/right press, 0..1.
    pub volume_step: f64,
    /// Maximum simultaneous voices, including sounds still waiting for an audio device.
    pub max_voices: usize,
    /// Maximum new voices per rendered frame.
    pub max_per_frame: usize,
    /// Minimum wall-clock seconds between repeats of the same sound.
    pub cooldown: f64,
    /// Screen-space emitter displacement relative to listener ears; 0 disables panning.
    pub pan: f64,
    /// Listener-to-emitter depth in normalized screen units (keeps attenuation mild).
    pub depth: f64,
    /// Stereo listener ear separation in normalized screen units.
    pub ear_gap: f64,
    /// Local pitch variation as a fraction (0..0.5), independent of sim RNG and training speed.
    pub pitch_variation: f64,
    /// Number of pre-synthesized strength levels per sound, 1..16.
    pub strength_levels: usize,
    /// Knockback/damage at which a scaled sound reaches full strength.
    pub strength_max: f64,
    /// Sound names referenced by sim/content and UI. Unknown names are skipped with a warning.
    pub sounds: BTreeMap<String, Sound>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Volumes {
    pub master: f64,
    pub sfx: f64,
    /// Reserved for future music; currently silent.
    pub music: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sound {
    /// Mixer gain, 0..1.
    pub volume: f64,
    /// Higher priorities win frame and voice limits (KO > hit > movement).
    pub priority: u32,
    /// Additional minimum real seconds between repeats.
    pub cooldown: f64,
    /// Duration multiplier at maximum impact (interpolated from 1).
    pub strength_duration: f64,
    /// Gain multiplier at minimum impact (interpolated to 1).
    pub strength_gain: f64,
    pub layers: Vec<Layer>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layer {
    pub wave: Wave,
    /// Exponential frequency sweep in Hz. For noise, a swept one-pole low-pass cutoff.
    pub from: f64,
    pub to: f64,
    /// Layer length in seconds, 0..3.
    pub duration: f64,
    /// Click-free attack length in seconds.
    pub attack: f64,
    /// Exponential amplitude decay factor over the layer.
    pub decay: f64,
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
