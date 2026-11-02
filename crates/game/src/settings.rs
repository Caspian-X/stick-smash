//! Settings, bindings and last menu selections, saved as JSON in the platform config dir. Also the content roots
//! (built-in assets, then the user content dir in the platform data dir).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

const APP: &str = "stick-smash";

pub(crate) fn replays_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|p| p.join(APP).join("replays"))
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Saved {
    /// The keyboard is two devices (left and right halves) instead of one. Off by default: one keyboard player with
    /// WASD and the arrows both moving.
    pub split_keyboard: bool,
    pub fullscreen: bool,
    /// None uses the content defaults; old settings files remain compatible.
    pub volume: Option<sim::data::audio::Volumes>,
    /// Hitbox/debug overlay in versus matches (training always starts with it on).
    pub versus_hitboxes: bool,
    /// Control profile (`keyboard`, `keyboard-left`, `keyboard-right`, `gamepad:<name>`) → action → controls.
    pub bindings: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// Control profile → control options (right stick, attack + special, stick range).
    pub controls: BTreeMap<String, ControlPrefs>,
    pub versus: VersusPrefs,
    pub training: TrainingPrefs,
}

/// Per-device control options. The gameplay ones are copied into the match config (and so into replays).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ControlPrefs {
    /// The right stick does tilts instead of smashes.
    pub stick_tilts: bool,
    /// Attack + special together is a smash attack.
    pub ab_smash: bool,
    /// Left-stick travel (0.5–1) that counts as a full tilt; lower is more sensitive.
    pub stick_range: f32,
}
impl Default for ControlPrefs {
    fn default() -> Self {
        Self { stick_tilts: false, ab_smash: true, stick_range: 1. }
    }
}
impl ControlPrefs {
    pub fn controls(self) -> sim::input::Controls {
        sim::input::Controls { stick_tilts: self.stick_tilts, ab_smash: self.ab_smash }
    }
}
/// A profile's control options (its own, else its model's, else defaults).
pub fn control_prefs(saved: &Saved, profile: &str) -> ControlPrefs {
    let model = profile.rsplit_once('#').map_or(profile, |(model, _)| model);
    saved.controls.get(profile).or_else(|| saved.controls.get(model)).copied().unwrap_or_default()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct VersusPrefs {
    pub stocks: i32,
    pub stage: String,
    pub teams: bool,
    pub friendly_fire: bool,
    /// Human picks by join order.
    pub humans: Vec<PlayerPref>,
    pub cpus: Vec<PlayerPref>,
}

impl Default for VersusPrefs {
    fn default() -> Self {
        Self { stocks: 3, stage: "arena".into(), teams: false, friendly_fire: false, humans: vec![], cpus: vec![] }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PlayerPref {
    pub fighter: String,
    pub level: u8,
    pub team: u8,
    pub tap_jump: bool,
}

impl Default for PlayerPref {
    fn default() -> Self {
        Self { fighter: String::new(), level: 1, team: 0, tap_jump: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum DummyMode {
    #[default]
    Stand,
    Shield,
    Jump,
    Cpu,
    Human,
}

impl DummyMode {
    pub const ALL: [DummyMode; 5] = [Self::Stand, Self::Shield, Self::Jump, Self::Cpu, Self::Human];
    /// `by` places along [`Self::ALL`], wrapping.
    pub fn cycle(self, by: i32) -> Self {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0) as i32;
        Self::ALL[(i + by).rem_euclid(Self::ALL.len() as i32) as usize]
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Stand => "Stand",
            Self::Shield => "Shield",
            Self::Jump => "Jump",
            Self::Cpu => "CPU",
            Self::Human => "Second player",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct TrainingPrefs {
    pub fighter: String,
    pub dummy_fighter: String,
    pub stage: String,
    pub mode: DummyMode,
    pub level: u8,
}

impl Default for TrainingPrefs {
    fn default() -> Self {
        Self {
            fighter: String::new(),
            dummy_fighter: String::new(),
            stage: "arena".into(),
            mode: DummyMode::Stand,
            level: 1,
        }
    }
}

/// Where settings are written; `None` (screenshot mode) keeps them in memory only.
#[derive(Resource)]
pub struct Persist(pub Option<PathBuf>);

pub fn config_file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(APP).join("settings.json"))
}

/// User content root (same layout as `assets/`), loaded after the built-in content.
pub fn user_content_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(APP).join("content"))
}

pub fn load(path: Option<&PathBuf>) -> Saved {
    let Some(text) = path.and_then(|p| fs::read_to_string(p).ok()) else {
        return Saved::default();
    };
    serde_json::from_str(&text).unwrap_or_else(|e| {
        warn!("settings file unreadable, using defaults: {e}");
        Saved::default()
    })
}

pub fn save(saved: Res<Saved>, persist: Res<Persist>) {
    let Some(path) = &persist.0 else { return };
    if !saved.is_changed() || saved.is_added() {
        return;
    }
    let write = || -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, serde_json::to_string_pretty(&*saved)?)
    };
    if let Err(e) = write() {
        warn!("couldn't save settings to {}: {e}", path.display());
    }
}
