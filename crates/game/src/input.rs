//! Input devices: the keyboard (one device, or split into left/right halves), gamepads, and scripted bots. Each
//! device maps its bindings to a sim `Input` (with a latch so sub-tick taps count) and to menu navigation presses.

use std::collections::BTreeMap;

use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;
use sim::Input;

use crate::settings::Saved;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Dev {
    Keys,
    KeysL,
    KeysR,
    Pad(Entity),
    /// Scripted device (screenshot tour); driven through [`Devices::bot`].
    Bot(u8),
}

impl Dev {
    /// The keyboard, whole or either half.
    pub fn is_keyboard(self) -> bool {
        matches!(self, Dev::Keys | Dev::KeysL | Dev::KeysR)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    Left,
    Right,
    Up,
    Down,
    Jump,
    Attack,
    Special,
    Grab,
    Shield,
    Start,
}

pub const ACTS: [Act; 10] = [
    Act::Left,
    Act::Right,
    Act::Up,
    Act::Down,
    Act::Jump,
    Act::Attack,
    Act::Special,
    Act::Grab,
    Act::Shield,
    Act::Start,
];

impl Act {
    pub fn id(self) -> &'static str {
        ["left", "right", "up", "down", "jump", "attack", "special", "grab", "shield", "start"][self as usize]
    }
    pub fn label(self) -> &'static str {
        ["Left", "Right", "Up", "Down", "Jump", "Attack", "Special", "Grab", "Shield", "Pause"][self as usize]
    }
}

/// Menu presses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    Start,
}

#[derive(Clone, Default)]
pub struct DevState {
    pub held: [bool; 10],
    pub pressed: [bool; 10],
    /// Analog stick (gamepads), y up, used when no direction button is held.
    pub stick: Vec2,
    pub latch: Input,
    /// Menu presses this frame (directions repeat while held).
    pub nav: Vec<Nav>,
    /// A control pressed this frame, by binding name (for remapping).
    pub raw: Option<String>,
    rep: Option<(Nav, f32)>,
    blocked: [bool; 10],
}

impl DevState {
    pub fn input(&self) -> Input {
        let h: [bool; 10] = std::array::from_fn(|i| self.held[i] && !self.blocked[i]);
        let ax = |neg: bool, pos: bool, analog: f32| match (neg, pos) {
            (true, false) => -127,
            (false, true) => 127,
            _ => (analog * 127.).round().clamp(-127., 127.) as i8,
        };
        Input {
            stick: (ax(h[0], h[1], self.stick.x), ax(h[3], h[2], self.stick.y)),
            buttons: (0..5).filter(|b| h[4 + b]).fold(0, |a, b| a | 1 << b),
        }
    }
    fn sample(&mut self, held: [bool; 10]) -> [bool; 10] {
        for (i, h) in held.iter().enumerate() {
            self.blocked[i] &= h;
        }
        let pressed: [bool; 10] = std::array::from_fn(|i| held[i] && !self.held[i] && !self.blocked[i]);
        self.held = held;
        self.pressed = pressed;
        if pressed[0] {
            self.latch.stick.0 = -127;
        }
        if pressed[1] {
            self.latch.stick.0 = 127;
        }
        if pressed[2] {
            self.latch.stick.1 = 127;
        }
        if pressed[3] {
            self.latch.stick.1 = -127;
        }
        self.latch.buttons |= (0..5).filter(|b| pressed[4 + b]).fold(0, |a, b| a | 1 << b);
        pressed
    }
    pub fn any_press(&self) -> bool {
        self.pressed[4..].iter().any(|p| *p) || self.nav.iter().any(|n| matches!(n, Nav::Confirm | Nav::Start))
    }
}

#[derive(Resource, Default)]
pub struct Devices {
    pub map: BTreeMap<Dev, DevState>,
    pub names: BTreeMap<Dev, String>,
    pad_profiles: BTreeMap<Dev, String>,
    /// Actions each bot holds, set by the script.
    pub bots: BTreeMap<u8, [bool; 10]>,
    /// Each bot's analog stick (y up).
    pub bot_sticks: BTreeMap<u8, Vec2>,
}

impl Devices {
    pub fn navs(&self) -> impl Iterator<Item = (Dev, Nav)> + '_ {
        self.map.iter().flat_map(|(d, s)| s.nav.iter().map(move |n| (*d, *n)))
    }
    pub fn profile(&self, d: Dev) -> String {
        self.pad_profiles.get(&d).cloned().unwrap_or_else(|| profile(d, &self.names))
    }
    pub fn name(&self, d: Dev) -> String {
        self.names.get(&d).cloned().unwrap_or_else(|| label(d))
    }
    /// The device's input for one sim tick: held state plus presses latched since the last tick.
    pub fn take(&mut self, d: Dev) -> Input {
        let Some(s) = self.map.get_mut(&d) else {
            return Input::default();
        };
        let held = s.input();
        let l = std::mem::take(&mut s.latch);
        let pick = |h: i8, l: i8| if h != 0 { h } else { l };
        Input {
            stick: (pick(held.stick.0, l.stick.0), pick(held.stick.1, l.stick.1)),
            buttons: held.buttons | l.buttons,
        }
    }
    /// Discard menu presses and suppress held controls until release.
    pub fn clear(&mut self) {
        for s in self.map.values_mut() {
            s.latch = Input::default();
            s.pressed = [false; 10];
            s.blocked = s.held;
            s.nav.clear();
            s.raw = None;
            s.rep = None;
        }
    }
    pub fn bot(&mut self, n: u8) -> &mut [bool; 10] {
        self.bots.entry(n).or_default()
    }
}

pub fn label(d: Dev) -> String {
    match d {
        Dev::Keys => "Keyboard".into(),
        Dev::KeysL => "Keyboard left".into(),
        Dev::KeysR => "Keyboard right".into(),
        Dev::Pad(_) => "Gamepad".into(),
        Dev::Bot(n) => format!("Bot {}", n + 1),
    }
}

/// Binding profile name for a device.
pub fn profile(d: Dev, names: &BTreeMap<Dev, String>) -> String {
    match d {
        Dev::Keys => "keyboard".into(),
        Dev::KeysL => "keyboard-left".into(),
        Dev::KeysR => "keyboard-right".into(),
        Dev::Pad(_) => {
            let name = names.get(&d).map_or("", |s| s.as_str());
            let index = names
                .iter()
                .filter(|(dev, n)| matches!(dev, Dev::Pad(_)) && n.as_str() == name)
                .position(|(dev, _)| *dev == d)
                .unwrap_or(0);
            format!("gamepad:{name}#{index}")
        }
        Dev::Bot(_) => "bot".into(),
    }
}

pub fn default_binds(profile: &str) -> [Vec<&'static str>; 10] {
    let l = [
        vec!["KeyA"],
        vec!["KeyD"],
        vec!["KeyW"],
        vec!["KeyS"],
        vec!["Space"],
        vec!["KeyJ"],
        vec!["KeyK"],
        vec!["KeyI"],
        vec!["KeyL"],
        vec!["Escape"],
    ];
    let r = [
        vec!["ArrowLeft"],
        vec!["ArrowRight"],
        vec!["ArrowUp"],
        vec!["ArrowDown"],
        vec!["ShiftRight", "Numpad4"],
        vec!["Period", "Numpad1"],
        vec!["Slash", "Numpad2"],
        vec!["Semicolon", "Numpad3"],
        vec!["Quote", "Numpad0"],
        vec!["Enter", "NumpadEnter"],
    ];
    match profile {
        "keyboard-left" => l,
        "keyboard-right" => r,
        "keyboard" => std::array::from_fn(|i| {
            let mut v = l[i].clone();
            if i < 9 {
                v.extend(&r[i]);
            }
            v
        }),
        _ => [
            vec!["DPadLeft"],
            vec!["DPadRight"],
            vec!["DPadUp"],
            vec!["DPadDown"],
            vec!["North", "West"],
            vec!["South"],
            vec!["East"],
            vec!["RightTrigger"],
            vec!["LeftTrigger", "LeftTrigger2", "RightTrigger2"],
            vec!["Start"],
        ],
    }
}

/// The controls bound to each action for a profile (saved overrides, else defaults).
pub fn binds(saved: &Saved, profile: &str) -> [Vec<String>; 10] {
    let model = profile.rsplit_once('#').map_or(profile, |(model, _)| model);
    let custom = saved
        .bindings
        .get(profile)
        .or_else(|| saved.bindings.get(model))
        .or_else(|| profile.starts_with("gamepad:").then(|| saved.bindings.get("gamepad")).flatten());
    let def = default_binds(profile);
    std::array::from_fn(|i| {
        custom
            .and_then(|c| c.get(ACTS[i].id()))
            .cloned()
            .unwrap_or_else(|| def[i].iter().map(|s| s.to_string()).collect())
    })
}

macro_rules! table {
    ($name:ident, $ty:ty, [$($v:ident),* $(,)?]) => {
        pub const $name: &[($ty, &str)] = &[$((<$ty>::$v, stringify!($v))),*];
    };
}
table!(
    KEYS,
    KeyCode,
    [
        KeyA,
        KeyB,
        KeyC,
        KeyD,
        KeyE,
        KeyF,
        KeyG,
        KeyH,
        KeyI,
        KeyJ,
        KeyK,
        KeyL,
        KeyM,
        KeyN,
        KeyO,
        KeyP,
        KeyQ,
        KeyR,
        KeyS,
        KeyT,
        KeyU,
        KeyV,
        KeyW,
        KeyX,
        KeyY,
        KeyZ,
        Digit0,
        Digit1,
        Digit2,
        Digit3,
        Digit4,
        Digit5,
        Digit6,
        Digit7,
        Digit8,
        Digit9,
        ArrowLeft,
        ArrowRight,
        ArrowUp,
        ArrowDown,
        Space,
        Enter,
        Escape,
        Backspace,
        Tab,
        ShiftLeft,
        ShiftRight,
        ControlLeft,
        ControlRight,
        AltLeft,
        AltRight,
        Comma,
        Period,
        Slash,
        Semicolon,
        Quote,
        BracketLeft,
        BracketRight,
        Backslash,
        Minus,
        Equal,
        Backquote,
        Numpad0,
        Numpad1,
        Numpad2,
        Numpad3,
        Numpad4,
        Numpad5,
        Numpad6,
        Numpad7,
        Numpad8,
        Numpad9,
        NumpadEnter,
        NumpadAdd,
        NumpadSubtract,
        NumpadMultiply,
        NumpadDivide,
        NumpadDecimal,
        Home,
        End,
        PageUp,
        PageDown,
        Insert,
        Delete,
    ]
);
table!(
    BUTTONS,
    GamepadButton,
    [
        South,
        East,
        North,
        West,
        LeftTrigger,
        LeftTrigger2,
        RightTrigger,
        RightTrigger2,
        Select,
        Start,
        Mode,
        LeftThumb,
        RightThumb,
        DPadUp,
        DPadDown,
        DPadLeft,
        DPadRight,
        C,
        Z,
    ]
);

/// Short display name for a control.
pub fn control_label(name: &str) -> String {
    let s = match name {
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "Period" => ".",
        "Comma" => ",",
        "Slash" => "/",
        "Semicolon" => ";",
        "Quote" => "'",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Backslash" => "\\",
        "Minus" => "-",
        "Equal" => "=",
        "Backquote" => "`",
        "ShiftLeft" => "Left Shift",
        "ShiftRight" => "Right Shift",
        "ControlLeft" => "Left Ctrl",
        "ControlRight" => "Right Ctrl",
        "AltLeft" => "Left Alt",
        "AltRight" => "Right Alt",
        "Escape" => "Esc",
        "South" => "A / Cross",
        "East" => "B / Circle",
        "North" => "Y / Triangle",
        "West" => "X / Square",
        "LeftTrigger" => "LB",
        "RightTrigger" => "RB",
        "LeftTrigger2" => "LT",
        "RightTrigger2" => "RT",
        "DPadUp" => "D-pad ↑",
        "DPadDown" => "D-pad ↓",
        "DPadLeft" => "D-pad ←",
        "DPadRight" => "D-pad →",
        "LeftThumb" => "L3",
        "RightThumb" => "R3",
        _ => "",
    };
    if !s.is_empty() {
        return s.into();
    }
    let s = name.strip_prefix("Key").or(name.strip_prefix("Digit")).unwrap_or(name);
    s.strip_prefix("Numpad").map_or(s.to_string(), |n| format!("Num {n}"))
}

const REPEAT_DELAY: f32 = 0.38;
const REPEAT_EVERY: f32 = 0.085;
const MENU_STICK: f32 = 0.55;

/// Reads every device into [`Devices`]. Runs in `PreUpdate` after Bevy's input systems.
pub fn poll(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<(Entity, &Gamepad, Option<&Name>)>,
    saved: Res<Saved>,
    time: Res<Time>,
    mut devs: ResMut<Devices>,
) {
    let devs = &mut *devs;
    let mut live: Vec<Dev> = if saved.split_keyboard { vec![Dev::KeysL, Dev::KeysR] } else { vec![Dev::Keys] };
    for (n, (e, _, name)) in pads.iter().enumerate() {
        let label = name.map_or_else(|| format!("Gamepad {}", n + 1), |nm| nm.as_str().to_string());
        if !devs.pad_profiles.contains_key(&Dev::Pad(e)) {
            let index = devs.names.iter().filter(|(dev, name)| matches!(dev, Dev::Pad(_)) && **name == label).count();
            devs.pad_profiles.insert(Dev::Pad(e), format!("gamepad:{label}#{index}"));
        }
        devs.names.insert(Dev::Pad(e), label);
        live.push(Dev::Pad(e));
    }
    live.extend(devs.bots.keys().map(|b| Dev::Bot(*b)));
    devs.map.retain(|d, _| live.contains(d));
    let raw_key = KEYS.iter().find(|(k, _)| keys.just_pressed(*k)).map(|(_, n)| n.to_string());
    for d in live {
        let mut held = [false; 10];
        let mut stick = Vec2::ZERO;
        let mut raw = None;
        let mut extra: Vec<Nav> = vec![];
        let prof = devs.profile(d);
        match d {
            Dev::Keys | Dev::KeysL | Dev::KeysR => {
                let b = binds(&saved, &prof);
                for (i, ctl) in b.iter().enumerate() {
                    held[i] = ctl.iter().any(|c| KEYS.iter().any(|(k, n)| n == c && keys.pressed(*k)));
                }
                raw.clone_from(&raw_key);
                let jp = |ks: &[KeyCode]| ks.iter().any(|k| keys.just_pressed(*k));
                let enter = jp(&[KeyCode::Enter, KeyCode::NumpadEnter]);
                if enter && d != Dev::KeysL {
                    extra.push(Nav::Confirm);
                }
                if jp(&[KeyCode::Escape]) && d != Dev::KeysR || jp(&[KeyCode::Backspace]) {
                    extra.push(Nav::Back);
                }
            }
            Dev::Pad(e) => {
                let Ok((_, pad, _)) = pads.get(e) else {
                    continue;
                };
                let b = binds(&saved, &prof);
                for (i, ctl) in b.iter().enumerate() {
                    held[i] = ctl.iter().any(|c| BUTTONS.iter().any(|(k, n)| n == c && pad.pressed(*k)));
                }
                stick = pad.left_stick();
                raw = BUTTONS.iter().find(|(k, _)| pad.just_pressed(*k)).map(|(_, n)| n.to_string());
            }
            Dev::Bot(n) => {
                held = devs.bots[&n];
                stick = devs.bot_sticks.get(&n).copied().unwrap_or_default();
            }
        }
        let s = devs.map.entry(d).or_default();
        let pressed = s.sample(held);
        s.stick = stick;
        s.raw = raw;
        let now = s.input();
        s.nav = extra;
        if pressed[4] || pressed[5] {
            s.nav.push(Nav::Confirm);
        }
        if pressed[6] {
            s.nav.push(Nav::Back);
        }
        // Esc is both "back" and the left keyboard's pause; in menus it means back.
        if pressed[9] && !s.nav.contains(&Nav::Back) {
            s.nav.push(Nav::Start);
        }
        // Menu directions: digital or a firmly tilted stick, repeating while held.
        let (x, y) = (f32::from(now.stick.0) / 127., f32::from(now.stick.1) / 127.);
        let dir = if x.abs() >= y.abs() && x.abs() >= MENU_STICK {
            Some(if x < 0. { Nav::Left } else { Nav::Right })
        } else if y.abs() >= MENU_STICK {
            Some(if y > 0. { Nav::Up } else { Nav::Down })
        } else {
            None
        };
        match (dir, s.rep) {
            (Some(n), Some((m, t))) if n == m => {
                let t2 = t + time.delta_secs();
                if t2 >= REPEAT_DELAY {
                    s.nav.push(n);
                    s.rep = Some((n, REPEAT_DELAY - REPEAT_EVERY));
                } else {
                    s.rep = Some((n, t2));
                }
            }
            (Some(n), _) => {
                s.nav.push(n);
                s.rep = Some((n, 0.));
            }
            (None, _) => s.rep = None,
        }
    }
}

/// Clear input when screens or ownership change.
pub fn clear(mut devs: ResMut<Devices>) {
    devs.clear();
}

pub fn keyboard_conflict(saved: &Saved, profile: &str, control: &str) -> bool {
    let other = match profile {
        "keyboard-left" => "keyboard-right",
        "keyboard-right" => "keyboard-left",
        _ => return false,
    };
    saved.split_keyboard && binds(saved, other).iter().flatten().any(|c| c == control)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clearing_drops_taps_and_blocks_held_buttons_until_release() {
        let mut d = Devices::default();
        let s = d.map.entry(Dev::Keys).or_default();
        s.held[5] = true;
        s.latch.buttons = 3;
        s.pressed[5] = true;
        d.clear();
        assert_eq!(d.take(Dev::Keys).buttons, 0);
        assert!(!d.map[&Dev::Keys].pressed[5]);
        let s = d.map.get_mut(&Dev::Keys).unwrap();
        let mut held = [false; 10];
        held[5] = true;
        s.sample(held);
        assert_eq!(s.input().buttons, 0);
        s.sample([false; 10]);
        s.sample(held);
        assert_eq!(d.take(Dev::Keys).buttons, 2);
    }
    #[test]
    fn identical_gamepads_can_override_model_defaults_independently() {
        let mut saved = Saved::default();
        saved.bindings.insert("gamepad:Same".into(), [("attack".into(), vec!["North".into()])].into());
        saved.bindings.insert("gamepad:Same#1".into(), [("attack".into(), vec!["West".into()])].into());
        assert_eq!(binds(&saved, "gamepad:Same#0")[5], ["North"]);
        assert_eq!(binds(&saved, "gamepad:Same#1")[5], ["West"]);
    }
    #[test]
    fn split_keyboard_conflicts_include_defaults() {
        let saved = Saved { split_keyboard: true, ..Saved::default() };
        assert!(keyboard_conflict(&saved, "keyboard-left", "Period"));
        assert!(!keyboard_conflict(&saved, "keyboard-left", "KeyJ"));
    }
}
