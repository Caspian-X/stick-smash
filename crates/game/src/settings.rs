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

#[derive(Resource, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Saved {
    /// The keyboard is two devices (left and right halves) instead of one. Off by default: one keyboard player with
    /// WASD and the arrows both moving.
    #[serde(default)]
    pub layout_version: u32,
    pub split_keyboard: bool,
    pub fullscreen: bool,
    /// None uses the content defaults; old settings files remain compatible.
    pub volume: Option<sim::data::audio::Volumes>,
    /// Hitbox/debug overlay in versus matches (training always starts with it on).
    pub versus_hitboxes: bool,
    /// Step the sim once per rendered frame when the display runs at about 60 Hz (see `play::Pace`).
    pub lock_60: bool,
    /// Control profile (`keyboard`, `keyboard-left`, `keyboard-right`, `gamepad:<name>`) → action → controls.
    pub bindings: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// Control profile → control options (right stick, attack + special, stick range).
    pub controls: BTreeMap<String, ControlPrefs>,
    /// Named reusable control layouts, independent of devices and fighters. Any device of the same family may pick
    /// one; several devices may pick the same layout.
    pub layouts: BTreeMap<String, Layout>,
    /// Device profile → layout choice: empty is the device's own *Custom* layout, [`TILT`] / [`SMASH`] are the
    /// built-in profiles, and any other value names an entry in [`Self::layouts`]. A device with no entry uses
    /// [`SMASH`] (the default profile).
    pub layout_choices: BTreeMap<String, String>,
    pub versus: VersusPrefs,
    pub training: TrainingPrefs,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            layout_version: 1,
            split_keyboard: false,
            fullscreen: false,
            volume: None,
            versus_hitboxes: false,
            lock_60: true,
            bindings: default(),
            controls: default(),
            layouts: default(),
            layout_choices: default(),
            versus: default(),
            training: default(),
        }
    }
}

/// Editing a shared or built-in layout starts from its complete effective configuration.
pub fn customize_layout(saved: &mut Saved, profile: &str) {
    let options = control_prefs(saved, profile);
    let bindings = crate::input::ACTS
        .iter()
        .zip(crate::input::binds(saved, profile))
        .map(|(act, keys)| (act.id().to_string(), keys))
        .collect();
    saved.bindings.insert(profile.into(), bindings);
    saved.controls.insert(profile.into(), options);
    saved.layout_choices.insert(profile.into(), String::new());
}

/// Resolve join-order tap jump once for each old device; explicit profile choices always win.
pub fn migrate_layout(saved: &mut Saved, profile: &str, human: usize) {
    let model = profile.rsplit_once('#').map_or(profile, |(model, _)| model);
    if saved.layout_version != 0
        || saved.layout_choices.contains_key(profile)
        || saved.layout_choices.contains_key(model)
        || profile.starts_with("gamepad:") && saved.layout_choices.contains_key("gamepad")
    {
        return;
    }
    let Some(pref) = saved.versus.humans.get(human) else { return };
    let tap_jump = pref.tap_jump;
    customize_layout(saved, profile);
    saved.controls.get_mut(profile).unwrap().tap_jump = tap_jump;
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
    /// Up on the left stick jumps (a tap or a held direction), as well as the jump buttons.
    pub tap_jump: bool,
}
impl Default for ControlPrefs {
    fn default() -> Self {
        Self { stick_tilts: false, ab_smash: true, stick_range: 1., tap_jump: true }
    }
}
/// Smashstick, the default profile: a right-stick smash, attack + special smash, full sensitivity, up to jump.
pub const SMASH_PREFS: ControlPrefs =
    ControlPrefs { stick_tilts: false, ab_smash: true, stick_range: 1., tap_jump: true };
/// Tiltstick, the alternate: the same profile with the right stick doing tilts.
pub const TILT_PREFS: ControlPrefs =
    ControlPrefs { stick_tilts: true, ab_smash: true, stick_range: 1., tap_jump: true };
impl ControlPrefs {
    pub fn controls(self) -> sim::input::Controls {
        sim::input::Controls { stick_tilts: self.stick_tilts, ab_smash: self.ab_smash }
    }
}
/// Built-in profile keys. They keep the device's own action bindings and supply [`TILT_PREFS`] / [`SMASH_PREFS`].
pub const TILT: &str = "tilt";
pub const SMASH: &str = "smash";

/// Keyboard keys and gamepad buttons are not interchangeable, so a named layout only applies to its own family.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Family {
    #[default]
    Keyboard,
    Gamepad,
}

impl Family {
    pub fn of(profile: &str) -> Self {
        if profile.starts_with("keyboard") { Family::Keyboard } else { Family::Gamepad }
    }
    pub fn label(self) -> &'static str {
        match self {
            Family::Keyboard => "Keyboard",
            Family::Gamepad => "Gamepad",
        }
    }
}

/// A reusable named layout: action bindings and control options, with the device family they fit.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    pub family: Family,
    pub bindings: BTreeMap<String, Vec<String>>,
    pub options: ControlPrefs,
}
impl Default for Layout {
    fn default() -> Self {
        Self { family: Family::Keyboard, bindings: BTreeMap::new(), options: ControlPrefs::default() }
    }
}

/// A device's selected layout. Stored as a string for compatibility (see [`Saved::layout_choices`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    /// The device's own saved bindings and options.
    Device,
    Tilt,
    Smash,
    Named(String),
}
impl Choice {
    pub fn key(&self) -> String {
        match self {
            Choice::Device => String::new(),
            Choice::Tilt => TILT.into(),
            Choice::Smash => SMASH.into(),
            Choice::Named(n) => n.clone(),
        }
    }
    pub fn label(&self) -> String {
        match self {
            Choice::Device => "Custom".into(),
            Choice::Tilt => "Tiltstick".into(),
            Choice::Smash => "Smashstick".into(),
            Choice::Named(n) => n.clone(),
        }
    }
}

/// A profile's layout choice, with missing or mismatched named layouts falling back to the device's own layout.
pub fn layout_choice(saved: &Saved, profile: &str) -> Choice {
    let named =
        |n: &str| saved.layouts.get(n).filter(|l| l.family == Family::of(profile)).map(|_| Choice::Named(n.into()));
    let model = profile.rsplit_once('#').map_or(profile, |(model, _)| model);
    let choice = saved.layout_choices.get(profile).or_else(|| {
        // Saved settings predate choices: same-model pads share a model profile, and old pads shared one.
        (model != profile)
            .then(|| saved.layout_choices.get(model))
            .flatten()
            .or_else(|| profile.starts_with("gamepad:").then(|| saved.layout_choices.get("gamepad")).flatten())
    });
    match choice.map(String::as_str) {
        Some(TILT) => Choice::Tilt,
        Some(SMASH) => Choice::Smash,
        Some("") => Choice::Device,
        Some(n) => named(n).unwrap_or(Choice::Device),
        // Settings from before profiles keep their own bindings/options; untouched devices use the default.
        None if has_device_settings(saved, profile) => Choice::Device,
        None => Choice::Smash,
    }
}

/// Whether a profile (or its old model / shared gamepad profile) has saved bindings or options.
fn has_device_settings(saved: &Saved, profile: &str) -> bool {
    let has = |p: &str| saved.bindings.contains_key(p) || saved.controls.contains_key(p);
    let model = profile.rsplit_once('#').map_or(profile, |(model, _)| model);
    has(profile) || has(model) || profile.starts_with("gamepad:") && has("gamepad")
}

/// Names the built-in profiles use in the UI; user layouts may not take them.
pub fn reserved_name(name: &str) -> bool {
    [TILT, SMASH, "tiltstick", "smashstick", "custom"].iter().any(|r| name.eq_ignore_ascii_case(r))
}

/// The named layout a profile uses, if it exists and fits the profile's device family.
pub fn active_layout<'a>(saved: &'a Saved, profile: &str) -> Option<(&'a str, &'a Layout)> {
    match layout_choice(saved, profile) {
        Choice::Named(n) => saved
            .layouts
            .get_key_value(&n)
            .filter(|(_, l)| l.family == Family::of(profile))
            .map(|(n, l)| (n.as_str(), l)),
        _ => None,
    }
}

/// The layouts a device of this profile may pick, in selection order.
pub fn layout_choices(saved: &Saved, profile: &str) -> Vec<Choice> {
    let family = Family::of(profile);
    let mut v = vec![Choice::Smash, Choice::Tilt, Choice::Device];
    v.extend(
        saved
            .layouts
            .iter()
            .filter(|(n, l)| l.family == family && !reserved_name(n))
            .map(|(n, _)| Choice::Named(n.clone())),
    );
    v
}

/// Pick the next / previous layout for a device; later layouts are the user's, in name order.
pub fn cycle_layout(saved: &mut Saved, profile: &str, by: i32) {
    let choices = layout_choices(saved, profile);
    let cur = choices.iter().position(|c| *c == layout_choice(saved, profile)).unwrap_or(0) as i32;
    let next = choices[(cur + by).rem_euclid(choices.len() as i32) as usize].clone();
    saved.layout_choices.insert(profile.into(), next.key());
}

/// Delete a named layout and return every device that picked it to its own Custom layout.
pub fn delete_layout(saved: &mut Saved, name: &str) -> bool {
    let removed = saved.layouts.remove(name).is_some();
    for c in saved.layout_choices.values_mut().filter(|c| *c == name) {
        c.clear();
    }
    removed
}

/// A profile's own control options (its own, else its model's, else defaults).
pub fn device_control_prefs(saved: &Saved, profile: &str) -> ControlPrefs {
    let model = profile.rsplit_once('#').map_or(profile, |(model, _)| model);
    saved.controls.get(profile).or_else(|| saved.controls.get(model)).copied().unwrap_or_default()
}

/// A profile's effective control options after its selected layout.
pub fn control_prefs(saved: &Saved, profile: &str) -> ControlPrefs {
    let base = device_control_prefs(saved, profile);
    match layout_choice(saved, profile) {
        Choice::Tilt => TILT_PREFS,
        Choice::Smash => SMASH_PREFS,
        Choice::Named(_) => active_layout(saved, profile).map_or(base, |(_, l)| l.options),
        Choice::Device => base,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input;

    fn layout(attack: &str) -> Layout {
        Layout {
            family: Family::Gamepad,
            bindings: [("attack".into(), vec![attack.into()])].into(),
            options: ControlPrefs { stick_tilts: true, ab_smash: false, stick_range: 0.7, tap_jump: false },
        }
    }

    #[test]
    fn old_settings_without_layouts_keep_their_bindings_and_options() {
        let old = r#"{
            "splitKeyboard": true,
            "fullscreen": true,
            "bindings": {"keyboard": {"attack": ["KeyU"]}},
            "controls": {"keyboard": {"stickTilts": true, "abSmash": false, "stickRange": 0.8}}
        }"#;
        let s: Saved = serde_json::from_str(old).expect("old settings load");
        assert!(s.split_keyboard && s.fullscreen);
        assert!(s.layouts.is_empty() && s.layout_choices.is_empty());
        assert_eq!(
            s.controls["keyboard"],
            ControlPrefs { stick_tilts: true, ab_smash: false, stick_range: 0.8, tap_jump: true }
        );
        assert_eq!(control_prefs(&s, "keyboard"), s.controls["keyboard"]);
        assert_eq!(layout_choice(&s, "keyboard"), Choice::Device, "saved device settings stay as Custom");
        assert_eq!(input::binds(&s, "keyboard")[5], ["KeyU"]);
        assert_eq!(layout_choice(&s, "gamepad:New#0"), Choice::Smash, "untouched devices use the default profile");
        let old: Saved = serde_json::from_str(
            r#"{"controls": {"gamepad:Same": {"stickTilts": true, "stickRange": 0.65}},
                "bindings": {"gamepad:Same": {"attack": ["North"]}}}"#,
        )
        .expect("old model settings load");
        assert_eq!(layout_choice(&old, "gamepad:Same#2"), Choice::Device, "old model defaults still apply");
        assert!(control_prefs(&old, "gamepad:Same#2").stick_tilts);
        assert_eq!(input::binds(&old, "gamepad:Same#2")[5], ["North"]);
    }

    #[test]
    fn built_in_profiles_keep_device_bindings_and_supply_their_options() {
        let mut s = Saved::default();
        s.controls.insert("gamepad:Pad#0".into(), ControlPrefs { ab_smash: false, stick_range: 0.8, ..default() });
        s.bindings.insert("gamepad:Pad#0".into(), [("attack".into(), vec!["North".into()])].into());
        s.layout_choices.insert("gamepad:Pad#0".into(), TILT.into());
        assert_eq!(control_prefs(&s, "gamepad:Pad#0"), TILT_PREFS);
        assert_eq!(input::binds(&s, "gamepad:Pad#0")[5], ["North"]);
        s.layout_choices.insert("gamepad:Pad#0".into(), SMASH.into());
        assert_eq!(control_prefs(&s, "gamepad:Pad#0"), SMASH_PREFS);
        assert_eq!(input::binds(&s, "gamepad:Pad#0")[5], ["North"]);
        assert_eq!(layout_choice(&s, "gamepad:Pad#0").label(), "Smashstick");
        assert_eq!(Choice::Tilt.label(), "Tiltstick");
        assert_eq!(Choice::Device.label(), "Custom");
    }

    #[test]
    fn legacy_up_to_jump_is_migrated_once_without_overwriting_explicit_choices() {
        let mut old: Saved = serde_json::from_str(r#"{"versus": {"humans": [{"tapJump": false}]}}"#).unwrap();
        assert_eq!(old.layout_version, 0);
        migrate_layout(&mut old, "keyboard", 0);
        assert_eq!(layout_choice(&old, "keyboard"), Choice::Device);
        assert!(!control_prefs(&old, "keyboard").tap_jump);
        old.layout_choices.insert("keyboard".into(), TILT.into());
        migrate_layout(&mut old, "keyboard", 0);
        assert_eq!(control_prefs(&old, "keyboard"), TILT_PREFS);
        let mut restored: Saved = serde_json::from_str(&serde_json::to_string(&old).unwrap()).unwrap();
        migrate_layout(&mut restored, "keyboard", 0);
        assert_eq!(layout_choice(&restored, "keyboard"), Choice::Tilt);
        assert_eq!(Saved::default().layout_version, 1);
    }

    #[test]
    fn a_named_layout_applies_to_its_family_only() {
        let mut s = Saved::default();
        s.layouts.insert("Pad A".into(), layout("West"));
        for p in ["gamepad:Pad#0", "gamepad:Pad#1"] {
            s.layout_choices.insert(p.into(), "Pad A".into());
        }
        assert_eq!(impact(&s, "gamepad:Pad#0"), ("West".into(), true, false, 0.7, false));
        assert_eq!(impact(&s, "gamepad:Pad#1"), ("West".into(), true, false, 0.7, false));
        // A keyboard profile cannot use gamepad button names; it stays on its own layout.
        s.layout_choices.insert("keyboard".into(), "Pad A".into());
        assert_eq!(layout_choice(&s, "keyboard"), Choice::Device);
        assert_eq!(input::binds(&s, "keyboard")[5], input::default_binds("keyboard")[5]);
        assert_eq!(control_prefs(&s, "keyboard"), ControlPrefs::default());
        // Adding a model-level override cannot leak into the pads that picked a layout.
        s.bindings.insert("gamepad:Pad".into(), [("attack".into(), vec!["East".into()])].into());
        assert_eq!(input::binds(&s, "gamepad:Pad#0")[5], ["West"]);
    }

    fn impact(s: &Saved, profile: &str) -> (String, bool, bool, f32, bool) {
        let c = control_prefs(s, profile);
        (input::binds(s, profile)[5].join("+"), c.stick_tilts, c.ab_smash, c.stick_range, c.tap_jump)
    }

    #[test]
    fn deleting_a_layout_returns_all_its_devices_to_their_own_layout() {
        let mut s = Saved::default();
        s.layouts.insert("Pad A".into(), layout("West"));
        s.controls.insert("gamepad:Pad#0".into(), ControlPrefs { stick_range: 0.9, ..default() });
        s.bindings.insert("gamepad:Pad#0".into(), [("attack".into(), vec!["East".into()])].into());
        s.layout_choices.insert("gamepad:Pad#0".into(), "Pad A".into());
        s.layout_choices.insert("gamepad:Pad#1".into(), "Pad A".into());
        assert!(delete_layout(&mut s, "Pad A"));
        assert!(!delete_layout(&mut s, "Pad A"));
        assert_eq!(layout_choice(&s, "gamepad:Pad#0"), Choice::Device);
        assert_eq!(layout_choice(&s, "gamepad:Pad#1"), Choice::Device, "deletion falls back to Custom, not Smashstick");
        assert_eq!(input::binds(&s, "gamepad:Pad#0")[5], ["East"]);
        assert_eq!(input::binds(&s, "gamepad:Pad#1")[5], input::default_binds("gamepad:Pad#1")[5]);
        assert_eq!(control_prefs(&s, "gamepad:Pad#0").stick_range, 0.9);
    }

    #[test]
    fn same_model_devices_pick_layouts_independently() {
        let mut s = Saved::default();
        s.layouts.insert("Pad A".into(), layout("West"));
        s.bindings.insert("gamepad:Same#1".into(), [("attack".into(), vec!["North".into()])].into());
        s.layout_choices.insert("gamepad:Same#0".into(), "Pad A".into());
        s.layout_choices.insert("gamepad:Same#1".into(), TILT.into());
        assert_eq!(input::binds(&s, "gamepad:Same#0")[5], ["West"]);
        assert_eq!(input::binds(&s, "gamepad:Same#1")[5], ["North"]);
        assert!(control_prefs(&s, "gamepad:Same#0").stick_tilts);
        assert!(control_prefs(&s, "gamepad:Same#1").stick_tilts);
        assert_eq!(s.layout_choices["gamepad:Same#0"], "Pad A");
    }

    #[test]
    fn default_profile_is_smashstick_and_tiltstick_is_the_alternate() {
        let mut s = Saved::default();
        assert_eq!(layout_choice(&s, "keyboard"), Choice::Smash);
        assert_eq!(control_prefs(&s, "keyboard"), SMASH_PREFS);
        assert_eq!(layout_choices(&s, "keyboard")[..3], [Choice::Smash, Choice::Tilt, Choice::Device]);
        cycle_layout(&mut s, "keyboard", 1);
        assert_eq!(layout_choice(&s, "keyboard"), Choice::Tilt);
        assert!(control_prefs(&s, "keyboard").stick_tilts);
        assert!(reserved_name("Smashstick"));
        assert!(reserved_name("Tiltstick"));
    }
}
