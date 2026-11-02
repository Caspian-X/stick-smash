//! Ground gait while idle: walk by stick tilt, flick to dash (flick back to dash the other way), keep holding to
//! run, hold the other way to turn around with a skid (facing flips at once, speed carries), let go to brake.
use crate::actions::{Action, accept};
use crate::input::{Intent, Strike};
use crate::world::{Fighter, Gait, State, sign};
use crate::{GameData, World};

pub fn walk_speed(d: &GameData, f: &Fighter) -> f64 {
    let s = &d.fighters[f.data].stats;
    s.walk_speed.unwrap_or(s.run_speed * d.global.gait.walk_speed)
}
/// Moving fast enough for dash attacks / dash grabs.
pub fn dashing(d: &GameData, f: &Fighter) -> bool {
    if d.global.gait.enabled { matches!(f.gait, Gait::Dash | Gait::Run) } else { false }
}
pub fn initial_dash(d: &GameData, f: &Fighter) -> bool {
    d.global.gait.enabled
        && d.global.gait.dash_lock
        && f.state == State::Idle
        && f.gait == Gait::Dash
        && f.gait_t < dash_frames(d, f)
}
fn dash_frames(d: &GameData, f: &Fighter) -> u32 {
    d.fighters[f.data].stats.dash_frames.unwrap_or(d.global.gait.dash_frames)
}
fn dash_speed(d: &GameData, f: &Fighter) -> f64 {
    let s = &d.fighters[f.data].stats;
    s.dash_speed.unwrap_or(s.run_speed * d.global.gait.dash_speed)
}
/// Standing commands wait for initial dash to end; explicit dash cancels remain available.
pub fn can_cancel(d: &GameData, f: &Fighter, inp: &Intent) -> bool {
    if !initial_dash(d, f) {
        return true;
    }
    if inp.grab {
        return true;
    }
    if inp.shield {
        return false;
    }
    if inp.sp || inp.jump || inp.jump_attack {
        return true;
    }
    inp.atk
        && match inp.strike {
            Strike::Button => true,
            Strike::Stick { y, smash, .. } => y > 0 && smash,
            Strike::Combo => inp.up,
        }
}
fn set(f: &mut Fighter, gait: Gait) {
    f.gait = gait;
    f.gait_t = 0;
}
fn dash(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, dir: f64) {
    if dir != f.face || f.vx * dir < 0. {
        w.dust(f.x, f.y, 3);
    }
    set(f, Gait::Dash);
    f.face = dir;
    f.vx = dir * dash_speed(d, f).max(f.vx * dir);
    accept(w, i, Action::Dash);
}
pub fn step(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent) {
    let g = &d.global.gait;
    let s = &d.fighters[f.data].stats;
    let held = inp.dir();
    let dir = sign(inp.x);
    let flick = if inp.flick_r {
        1.
    } else if inp.flick_l {
        -1.
    } else {
        0.
    };
    if inp.down
        && inp.x == 0.
        && matches!(f.gait, Gait::Stand | Gait::Walk | Gait::Crouch | Gait::Rise | Gait::Run | Gait::Brake)
    {
        if f.gait != Gait::Crouch {
            let t = if f.gait == Gait::Rise { g.crouch_frames.saturating_sub(f.gait_t) } else { 0 };
            set(f, Gait::Crouch);
            f.gait_t = t;
        }
        f.gait_t = f.gait_t.saturating_add(1);
        f.vx *= s.ground_friction;
        f.run_ph += f.vx.abs() * 0.07;
        return;
    }
    if f.gait == Gait::Crouch {
        let t = g.crouch_frames.saturating_sub(f.gait_t);
        set(f, Gait::Rise);
        f.gait_t = t;
    }
    f.gait_t = f.gait_t.saturating_add(1);
    let t = f.gait_t;
    match f.gait {
        Gait::Dash if flick == -f.face => dash(w, d, i, f, flick),
        Gait::Dash if t < dash_frames(d, f) => f.vx = f.face * dash_speed(d, f).max(f.vx * f.face),
        Gait::Dash => set(f, if held == f.face { Gait::Run } else { Gait::Brake }),
        Gait::Run if dir == f.face && g.run_threshold > 0. && inp.x.abs() < g.walk_threshold => set(f, Gait::Walk),
        Gait::Run if held == f.face => f.vx = (f.vx + f.face * s.run_accel).clamp(-s.run_speed, s.run_speed),
        Gait::Run | Gait::Brake if held == -f.face => {
            set(f, Gait::Turn);
            f.face = held;
            accept(w, i, Action::Turn);
        }
        Gait::Run => set(f, Gait::Brake),
        Gait::Turn if t >= g.turn_frames => set(f, if held == f.face { Gait::Run } else { Gait::Stand }),
        Gait::Turn => f.vx *= g.turn_friction,
        Gait::Brake if flick != 0. => dash(w, d, i, f, flick),
        Gait::Brake if held == f.face => set(f, Gait::Run),
        Gait::Brake if t >= g.brake_frames => set(f, Gait::Stand),
        Gait::Brake => f.vx *= g.brake_friction,
        Gait::Stand | Gait::Walk | Gait::Rise if flick != 0. => dash(w, d, i, f, flick),
        Gait::Stand | Gait::Rise if held != 0. && held * f.vx > walk_speed(d, f) => {
            // Landing or leaving a skid while moving faster than a walk keeps running.
            f.face = held;
            set(f, Gait::Run);
        }
        Gait::Stand | Gait::Walk | Gait::Rise if g.run_threshold > 0. && inp.x.abs() >= g.run_threshold => {
            set(f, Gait::Run);
            f.face = dir;
            f.vx = (f.vx + dir * s.run_accel).clamp(-s.run_speed, s.run_speed);
        }
        Gait::Stand | Gait::Walk | Gait::Rise if dir != 0. => {
            if f.gait != Gait::Walk {
                set(f, Gait::Walk);
            }
            f.face = dir;
            let target = inp.x * walk_speed(d, f);
            let acc = s.run_accel * g.walk_accel;
            f.vx = if f.vx < target { (f.vx + acc).min(target) } else { (f.vx - acc).max(target) };
        }
        Gait::Stand | Gait::Walk | Gait::Rise => {
            if f.gait != Gait::Rise || t >= g.crouch_frames {
                set(f, Gait::Stand);
            }
            f.vx *= s.ground_friction;
        }
        Gait::Crouch => unreachable!(),
    }
    f.run_ph += f.vx.abs() * 0.07;
}
