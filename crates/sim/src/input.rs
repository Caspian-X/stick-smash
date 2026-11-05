//! Raw controls → per-tick gestures (`Intent`) → one buffered command, accepted only when an action happens.
use crate::data::global::InputRules;
use serde::{Deserialize, Serialize};

pub const JUMP: u8 = 1;
pub const ATTACK: u8 = 2;
pub const SPECIAL: u8 = 4;
pub const GRAB: u8 = 8;
pub const SHIELD: u8 = 16;
/// A second jump control held alongside the first (two-button short hop).
pub const JUMP2: u8 = 32;

/// Raw input; positive stick y points up. Bits: [`JUMP`], [`ATTACK`], [`SPECIAL`], [`GRAB`], [`SHIELD`], [`JUMP2`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Input {
    pub stick: (i8, i8),
    pub buttons: u8,
    /// Independent directional attack stick; never contributes to movement or tap jump.
    #[serde(default)]
    pub attack_stick: (i8, i8),
}

/// Per-player control options, part of the match config (so replays keep them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Controls {
    /// The attack stick does tilts instead of smashes.
    pub stick_tilts: bool,
    /// Attack + special together is a smash attack in the held direction.
    pub ab_smash: bool,
}

/// How an attack press picks its move.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Strike {
    /// The attack button: smash, tilt, jab or dash attack from the movement stick.
    #[default]
    Button,
    /// The attack stick: a fixed direction (one axis is 0), smash or tilt per [`Controls::stick_tilts`].
    Stick { x: i8, y: i8, smash: bool },
    /// Attack + special: a smash toward the held direction (forward when neutral).
    Combo,
}
impl Strike {
    /// Points up (attack stick or combo with up held).
    pub fn up(self, inp: &Intent) -> bool {
        match self {
            Strike::Stick { y, .. } => y > 0,
            _ => inp.up,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Intent {
    /// Directions held past `stickThreshold`.
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    /// Analog stick past the deadzone, -1..1, y up.
    pub x: f64,
    pub y: f64,
    pub jump: bool,
    pub jump_held: bool,
    /// A second jump control pressed (two-button short hop).
    pub short_hop: bool,
    /// Jump-button and attack pressed together (short-hop aerial shortcut).
    pub jump_attack: bool,
    pub atk: bool,
    /// The attack button (not the stick) held.
    pub atk_held: bool,
    /// The attack stick held past the threshold.
    pub stick_held: bool,
    pub strike: Strike,
    pub sp: bool,
    pub sp_held: bool,
    pub grab: bool,
    #[serde(default)]
    pub grab_held: bool,
    pub shield: bool,
    pub shield_p: bool,
    pub down_p: bool,
    /// Down pressed within `fastFallBuffer` ticks and still held.
    pub down_recent: bool,
    /// Recent flicks (within `tapWindow`), for smashes, rolls and dodges.
    pub tap_l: bool,
    pub tap_r: bool,
    pub tap_u: bool,
    pub tap_d: bool,
    /// Flicks this tick (dash, platform drop).
    pub flick_l: bool,
    pub flick_r: bool,
    pub flick_d: bool,
    pub any_p: bool,
}
impl Intent {
    pub fn dir(self) -> f64 {
        f64::from(self.right as i8 - self.left as i8)
    }
    /// CPU intents are digital: full-tilt analog, and a held direction counts as a flick.
    pub fn digital(mut self) -> Self {
        self.x = self.dir();
        self.y = f64::from(self.up as i8 - self.down as i8);
        self.flick_l = self.left;
        self.flick_r = self.right;
        self.flick_d = self.down_p;
        self.down_recent = self.down_p;
        self
    }
    fn presses(&self) -> bool {
        self.atk || self.sp || self.jump || self.grab || self.shield_p
    }
    /// Retain steering and holds while an initial dash rejects this command.
    pub(crate) fn movement_only(self) -> Self {
        Self {
            atk: false,
            sp: false,
            jump: false,
            jump_attack: false,
            short_hop: false,
            grab: false,
            shield: false,
            shield_p: false,
            flick_d: false,
            ..self
        }
    }
    /// The press part of an intent (what a buffered command remembers): presses, combination flags and the
    /// directions at press time.
    fn command(&self) -> Self {
        Self {
            left: self.left,
            right: self.right,
            up: self.up,
            down: self.down,
            jump: self.jump,
            short_hop: self.short_hop,
            jump_attack: self.jump_attack,
            atk: self.atk,
            strike: self.strike,
            sp: self.sp,
            grab: self.grab,
            shield_p: self.shield_p,
            tap_l: self.tap_l,
            tap_r: self.tap_r,
            tap_u: self.tap_u,
            tap_d: self.tap_d,
            ..Default::default()
        }
    }
    /// `self` (earlier) and `next` as one command, if they combine.
    fn combine(self, next: Self, ab_smash: bool) -> Option<Self> {
        let kinds = |i: &Self| (i.atk, i.sp, i.jump, i.grab, i.shield_p);
        let (a, b) = (kinds(&self), kinds(&next));
        let attack = if next.atk { next } else { self };
        match (a, b) {
            // Jump then attack (or the reverse): a short-hop aerial.
            ((false, false, true, false, false), (true, false, _, false, false))
            | ((true, false, false, false, false), (false, false, true, false, false)) => {
                Some(Self { jump: true, atk: true, jump_attack: true, ..attack })
            }
            // Two jump presses: a short hop.
            ((false, false, true, false, false), (false, false, true, false, false)) => {
                Some(Self { short_hop: true, ..self })
            }
            ((true, false, false, false, false), (false, true, false, false, false))
            | ((false, true, false, false, false), (true, false, false, false, false))
                if ab_smash && attack.strike == Strike::Button =>
            {
                Some(Self { sp: false, strike: Strike::Combo, ..attack })
            }
            _ => None,
        }
    }
    /// `now` still holds a button that made this command.
    fn held_by(&self, now: &Self) -> bool {
        let atk = match self.strike {
            Strike::Stick { .. } => now.stick_held,
            _ => now.atk_held,
        };
        self.jump && now.jump_held
            || self.atk && atk
            || self.sp && now.sp_held
            || self.grab && now.grab_held
            || self.shield_p && now.shield
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    /// Left, right, up, down past the threshold, then the six buttons.
    pub held: [bool; 10],
    pub last_press: [i64; 4],
    /// Last tick each direction was inside the deadzone.
    pub last_low: [i64; 4],
    pub last_flick: [i64; 4],
    pub stick: (i8, i8),
    pub attack_held: bool,
}
impl Default for History {
    fn default() -> Self {
        Self {
            held: [false; 10],
            last_press: [-99; 4],
            // Neutral at the world's first tick; after a mid-match reset a held stick can't flick until neutral.
            last_low: [0; 4],
            last_flick: [-99; 4],
            stick: (0, 0),
            attack_held: false,
        }
    }
}
/// Deadzoned analog value, -1..1.
pub fn analog(v: i8, deadzone: i8) -> f64 {
    let (a, dz) = (f64::from(v).abs(), f64::from(deadzone.max(0)));
    if a <= dz || a == 0. { 0. } else { f64::from(v).signum() * ((a - dz) / (127. - dz)).min(1.) }
}
impl History {
    pub fn derive(
        &mut self,
        input: Input,
        tick: u64,
        rules: &InputRules,
        window: u32,
        tap_jump: bool,
        controls: Controls,
    ) -> Intent {
        let threshold = i16::from(rules.stick_threshold);
        let axes = |(x, y): (i8, i8)| [-i16::from(x), i16::from(x), i16::from(y), -i16::from(y)];
        let (now, before) = (axes(input.stick), axes(self.stick));
        let tick = tick as i64;
        let mut held = [false; 10];
        let mut flick = [false; 4];
        for d in 0..4 {
            held[d] = now[d] >= threshold;
            if now[d] >= i16::from(rules.flick_threshold)
                && before[d] < i16::from(rules.flick_threshold)
                && tick - self.last_low[d] <= i64::from(rules.flick_frames)
            {
                self.last_flick[d] = tick;
                flick[d] = true;
            }
            if now[d] < i16::from(rules.deadzone).max(1) {
                self.last_low[d] = tick;
            }
        }
        for b in 0..6 {
            held[b + 4] = input.buttons & (1 << b) != 0;
        }
        let pressed: [bool; 10] = std::array::from_fn(|i| held[i] && !self.held[i]);
        for (i, p) in pressed[..4].iter().enumerate() {
            if *p {
                self.last_press[i] = tick;
            }
        }
        let tap: [bool; 4] = std::array::from_fn(|i| held[i] && tick - self.last_flick[i] <= i64::from(window));
        let (ax, ay) = input.attack_stick;
        let (ux, uy) = (ax.unsigned_abs(), ay.unsigned_abs());
        let stick = if ux >= uy && ux >= threshold as u8 {
            (ax.signum(), 0)
        } else if uy >= threshold as u8 {
            (0, ay.signum())
        } else {
            (0, 0)
        };
        let stick_held = stick != (0, 0);
        let stick_pressed = stick_held && !self.attack_held;
        self.attack_held = stick_held;
        self.held = held;
        self.stick = input.stick;
        let jump = pressed[4] || pressed[9] || tap_jump && pressed[2];
        let combo = controls.ab_smash && pressed[5] && pressed[6];
        let strike = if stick_pressed {
            Strike::Stick { x: stick.0, y: stick.1, smash: !controls.stick_tilts }
        } else if combo {
            Strike::Combo
        } else {
            Strike::Button
        };
        let atk = pressed[5] || stick_pressed;
        Intent {
            left: held[0],
            right: held[1],
            up: held[2],
            down: held[3],
            x: analog(input.stick.0, rules.deadzone),
            y: analog(input.stick.1, rules.deadzone),
            jump,
            jump_held: held[4] || held[9] || tap_jump && held[2],
            short_hop: pressed[9],
            jump_attack: (pressed[4] || pressed[9]) && atk,
            atk,
            atk_held: held[5],
            stick_held,
            strike,
            sp: pressed[6] && !combo,
            sp_held: held[6],
            grab: pressed[7],
            grab_held: held[7],
            shield: held[8],
            shield_p: pressed[8],
            down_p: pressed[3],
            down_recent: held[3] && tick - self.last_press[3] <= i64::from(rules.fast_fall_buffer),
            tap_l: tap[0],
            tap_r: tap[1],
            tap_u: tap[2],
            tap_d: tap[3],
            flick_l: flick[0],
            flick_r: flick[1],
            flick_d: flick[3],
            any_p: stick_pressed || pressed.iter().any(|x| *x),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BufferedPress {
    /// Press tick (combos, the input display).
    pub tick: u64,
    pub intent: Intent,
    /// The lifetime runs from here: the press tick, or with `holdBuffer` the last tick its button was held.
    #[serde(default)]
    pub held: u64,
}
/// The latest command pressed while unable to act. It's re-delivered each actionable tick until an action
/// accepts it or it expires; a fresh press supersedes it (combining when close enough). With `holdBuffer` it
/// doesn't expire while its button stays held. Accepted, it's gone: a held button presses again only when
/// released and pressed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Buffer {
    pub cmd: Option<BufferedPress>,
}
impl Buffer {
    /// Returns the press tick when this tick's intent came from the buffer.
    pub fn deliver(
        &mut self,
        inp: &mut Intent,
        tick: u64,
        rules: &InputRules,
        actionable: bool,
        ab_smash: bool,
    ) -> Option<u64> {
        if let Some(c) = self.cmd.as_mut().filter(|c| rules.hold_buffer && c.intent.held_by(inp)) {
            c.held = tick;
        }
        self.cmd = self.cmd.filter(|c| rules.buffer_frames > 0 && tick - c.held <= u64::from(rules.buffer_frames));
        let recent = self.cmd.filter(|c| tick - c.tick <= u64::from(rules.combo_frames));
        if inp.presses() {
            let fresh = inp.command();
            let merged = recent.and_then(|c| c.intent.combine(fresh, ab_smash));
            if actionable {
                self.cmd = None;
                if let Some(m) = merged {
                    *inp = live(inp, m);
                }
            } else if rules.buffer_frames > 0 {
                self.cmd = Some(BufferedPress { tick, intent: merged.unwrap_or(fresh), held: tick });
            }
            return None;
        }
        if !actionable {
            return None;
        }
        let c = self.cmd?;
        *inp = live(inp, c.intent);
        Some(c.tick)
    }
    /// An action accepted the delivered command.
    pub fn accept(&mut self) {
        self.cmd = None;
    }
}
/// A command with this tick's held buttons, analog stick and flicks.
fn live(now: &Intent, cmd: Intent) -> Intent {
    Intent {
        x: now.x,
        y: now.y,
        jump_held: now.jump_held,
        atk_held: now.atk_held,
        stick_held: now.stick_held,
        sp_held: now.sp_held,
        shield: now.shield,
        down_p: now.down_p,
        down_recent: now.down_recent,
        flick_l: now.flick_l,
        flick_r: now.flick_r,
        flick_d: now.flick_d,
        any_p: true,
        ..cmd
    }
}
