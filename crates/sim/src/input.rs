/// Raw input; positive stick y points up. Bits: jump, attack, special, grab, shield.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Input {
    pub stick: (i8, i8),
    pub buttons: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Intent {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub jump: bool,
    pub jump_held: bool,
    pub atk: bool,
    pub atk_held: bool,
    pub sp: bool,
    pub sp_held: bool,
    pub grab: bool,
    pub shield: bool,
    pub shield_p: bool,
    pub down_p: bool,
    pub tap_l: bool,
    pub tap_r: bool,
    pub tap_u: bool,
    pub tap_d: bool,
    pub any_p: bool,
}
impl Intent {
    pub fn dir(self) -> f64 {
        f64::from(self.right as i8 - self.left as i8)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    pub held: [bool; 9],
    pub last_press: [i64; 4],
}
impl Default for History {
    fn default() -> Self {
        Self { held: [false; 9], last_press: [-99; 4] }
    }
}
impl History {
    pub fn derive(&mut self, input: Input, tick: u64, threshold: i8, window: u32, tap_jump: bool) -> Intent {
        let (x, y) = input.stick;
        let mut held = [false; 9];
        held[..4].copy_from_slice(&[x <= -threshold, x >= threshold, y >= threshold, y <= -threshold]);
        for b in 0..5 {
            held[b + 4] = input.buttons & (1 << b) != 0;
        }
        let pressed: [bool; 9] = std::array::from_fn(|i| held[i] && !self.held[i]);
        for (i, p) in pressed[..4].iter().enumerate() {
            if *p {
                self.last_press[i] = tick as i64;
            }
        }
        let tap: [bool; 4] = std::array::from_fn(|i| held[i] && tick as i64 - self.last_press[i] <= i64::from(window));
        self.held = held;
        Intent {
            left: held[0],
            right: held[1],
            up: held[2],
            down: held[3],
            jump: pressed[4] || tap_jump && pressed[2],
            jump_held: held[4] || tap_jump && held[2],
            atk: pressed[5],
            atk_held: held[5],
            sp: pressed[6],
            sp_held: held[6],
            grab: pressed[7],
            shield: held[8],
            shield_p: pressed[8],
            down_p: pressed[3],
            tap_l: tap[0],
            tap_r: tap[1],
            tap_u: tap[2],
            tap_d: tap[3],
            any_p: pressed.iter().any(|x| *x),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferedPress {
    pub tick: u64,
    pub intent: Intent,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Buffer {
    pub entries: Vec<BufferedPress>,
}
impl Buffer {
    pub fn deliver(&mut self, inp: &mut Intent, tick: u64, frames: u32, actionable: bool) {
        self.entries.retain(|e| tick - e.tick <= u64::from(frames));
        if frames == 0 {
            self.entries.clear();
            return;
        }
        let presses = [inp.atk, inp.sp, inp.jump, inp.grab, inp.shield_p];
        if presses.iter().any(|p| *p) {
            if !actionable {
                for (kind, pressed) in presses.into_iter().enumerate() {
                    if !pressed {
                        continue;
                    }
                    let mut intent = Intent {
                        left: inp.left,
                        right: inp.right,
                        up: inp.up,
                        down: inp.down,
                        tap_l: inp.tap_l,
                        tap_r: inp.tap_r,
                        tap_u: inp.tap_u,
                        tap_d: inp.tap_d,
                        ..Default::default()
                    };
                    match kind {
                        0 => intent.atk = true,
                        1 => intent.sp = true,
                        2 => intent.jump = true,
                        3 => intent.grab = true,
                        _ => intent.shield_p = true,
                    }
                    self.entries.push(BufferedPress { tick, intent });
                }
            }
        } else if actionable && !self.entries.is_empty() {
            let saved = self.entries.remove(0).intent;
            *inp = Intent {
                jump_held: inp.jump_held,
                atk_held: inp.atk_held,
                sp_held: inp.sp_held,
                shield: inp.shield,
                down_p: inp.down_p,
                any_p: true,
                ..saved
            };
        }
    }
}
