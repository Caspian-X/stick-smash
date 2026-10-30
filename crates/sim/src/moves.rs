use crate::physics::*;
use crate::{
    GameData, World,
    data::{self, Color, Move, Num, Phase, Transition, Trigger},
    input::Intent,
    world::{Fighter, MoveState, State},
};
use data::moves::{AirControl, Button, Cond};

pub fn definition<'a>(d: &'a GameData, f: &Fighter) -> &'a Move {
    let slot = &f.mov.as_ref().expect("attack move").slot;
    d.fighters[f.data].moves.iter().find(|(s, _)| s == slot).map(|(_, m)| m).unwrap_or(&d.fighters[f.data].moves.nspec)
}
pub fn phase<'a>(d: &'a GameData, f: &Fighter) -> &'a Phase {
    &definition(d, f).phases[f.mov.as_ref().unwrap().phase]
}
pub fn cond(f: &Fighter, c: Option<Cond>) -> bool {
    match c {
        None => true,
        Some(Cond::Grounded) => f.ground,
        Some(Cond::Airborne) => !f.ground,
        Some(Cond::Countered) => f.mov.as_ref().is_some_and(|m| m.counter_damage > 0.),
    }
}
fn num(w: &mut World, n: Num) -> f64 {
    match n {
        Num::Fixed(n) => n,
        Num::Range([a, b]) => w.rng.range(a, b),
    }
}
pub fn effects(w: &mut World, fx: &[data::moves::Fx], (x, y): (f64, f64), face: f64, (vx, vy): (f64, f64), player: usize) {
    use data::moves::Fx as F;
    for e in fx {
        match *e {
            F::Dust { x: dx, y: dy, n } => {
                let x = x + num(w, dx) * face;
                let y = y - num(w, dy);
                w.dust(x, y, n);
            }
            F::Ring { x: dx, y: dy, r, grow, life, color } => {
                let x = x + num(w, dx) * face;
                let y = y - num(w, dy);
                w.fx.push(crate::Fx::Ring { x, y, r, grow, life, color, player });
            }
            F::Part { x: dx, y: dy, vx: dvx, vy: dvy, life, size, color, gravity, inherit_vel } => {
                let x = x + num(w, dx) * face;
                let y = y - num(w, dy);
                let vx = num(w, dvx) * face + vx * inherit_vel;
                let vy = -num(w, dvy) + vy * inherit_vel;
                let life = num(w, life);
                let size = num(w, size);
                w.fx.push(crate::Fx::Part { x, y, vx, vy, life, size, color, gravity, player });
            }
            F::Burst { x: dx, y: dy, color, n, speed, life, size, gravity } => {
                let x = x + num(w, dx) * face;
                let y = y - num(w, dy);
                w.fx.push(crate::Fx::Burst { x, y, color, player, n: f64::from(n), speed, life: f64::from(life), size, gravity });
            }
            F::Shake { k } => w.fx.push(crate::Fx::Shake { k }),
        }
    }
}
fn events(w: &mut World, d: &GameData, i: usize, f: &mut Fighter) {
    let p = phase(d, f);
    let t = f.mov.as_ref().unwrap().t;
    for e in &p.events {
        if !e.fires(t, w.frame) || !cond(f, e.cond) {
            continue;
        }
        if let Some(v) = e.mul_vel {
            if let Some(x) = v.x {
                f.vx *= x;
            }
            if let Some(y) = v.y {
                f.vy *= y;
            }
        }
        if let Some(v) = e.set_vel {
            if let Some(x) = v.x {
                f.vx = x * f.face;
            }
            if let Some(y) = v.y {
                f.vy = -y;
            }
        }
        if let Some(v) = e.add_vel {
            if let Some(x) = v.x {
                f.vx += x * f.face;
            }
            if let Some(y) = v.y {
                f.vy -= y;
            }
        }
        if let Some(cap) = e.cap_fall {
            f.vy = f.vy.min(cap);
        }
        if e.leave_ground {
            f.ground = false;
            f.surf = None;
        }
        if let Some(n) = e.invuln {
            f.inv = f.inv.max(n);
        }
        if let Some(id) = &e.spawn {
            crate::projectiles::spawn(w, d, i, f, id);
        }
        if let Some(s) = &e.sfx {
            w.sfx(s.clone());
        }
        effects(w, &e.fx, (f.x, f.y), f.face, (f.vx, f.vy), i);
    }
}
pub fn start_move(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, slot: &str) {
    f.set_state(State::Attack);
    f.mov = Some(MoveState { slot: slot.into(), phase: 0, t: 0, elapsed: 0, charge: 0, counter_damage: 0., reversed: false });
    events(w, d, i, f);
}
fn transition(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, tr: &Transition) {
    match tr.to.as_str() {
        "end" => f.set_state(if f.ground { State::Idle } else { State::Air }),
        "helpless" => f.set_state(if f.ground { State::Idle } else { State::Helpless }),
        "landing" => landing(f, tr.lag.unwrap_or(0)),
        name => {
            let n = definition(d, f).phases.iter().position(|p| p.name == name).expect("validated phase");
            let m = f.mov.as_mut().unwrap();
            m.phase = n;
            m.t = 0;
            f.sd.hit.clear();
            f.sd.hit_at.clear();
            events(w, d, i, f);
        }
    }
}
pub fn trigger(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, on: Trigger) -> bool {
    if f.mov.is_none() {
        return false;
    }
    let t = f.mov.as_ref().unwrap().t;
    if let Some(tr) = phase(d, f).transitions.iter().find(|tr| tr.on == on && tr.from.is_none_or(|n| t >= n) && cond(f, tr.cond)) {
        transition(w, d, i, f, tr);
        true
    } else {
        false
    }
}
fn reverse(f: &mut Fighter, dir: f64) {
    if dir != 0. && dir != f.face {
        f.face = dir;
        if !f.ground {
            f.vx = -f.vx;
        }
    }
}
pub fn up_special(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, dir: f64) {
    f.up_b = true;
    reverse(f, dir);
    start_move(w, d, i, f, "uspec");
}
pub fn special(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, dir: f64) {
    if inp.up && !f.up_b {
        up_special(w, d, i, f, dir);
        return;
    }
    reverse(f, dir);
    start_move(
        w,
        d,
        i,
        f,
        if inp.down {
            "dspec"
        } else if dir != 0. && d.fighters[f.data].moves.sspec.is_some() {
            "sspec"
        } else {
            "nspec"
        },
    );
}
pub fn ground_actions(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, dir: f64) -> bool {
    let mv = &d.global.movement;
    if inp.grab {
        if dir != 0. {
            f.face = dir;
        }
        start_move(w, d, i, f, if f.vx.abs() > mv.dash_grab_speed { "dashgrab" } else { "grab" });
    } else if inp.shield {
        f.set_state(State::Shield);
    } else if inp.sp {
        special(w, d, i, f, inp, dir);
    } else if inp.atk {
        let tap = if inp.tap_r {
            1.
        } else if inp.tap_l {
            -1.
        } else {
            0.
        };
        let slot = if tap != 0. {
            f.face = tap;
            "fsmash"
        } else if inp.tap_u {
            "usmash"
        } else if inp.tap_d {
            "dsmash"
        } else if inp.up {
            "utilt"
        } else if inp.down {
            "dtilt"
        } else if dir != 0. && dir == f.face && f.vx.abs() > mv.dash_attack_speed {
            "dashatk"
        } else if dir != 0. {
            f.face = dir;
            "ftilt"
        } else {
            "jab"
        };
        start_move(w, d, i, f, slot);
    } else if inp.jump {
        f.set_state(State::Jumpsquat);
    } else if inp.down_p && f.surf.is_some_and(|n| n != 0) {
        f.drop = mv.platform_drop;
        f.ground = false;
        f.surf = None;
        f.y += d.global.sim.platform_drop_offset;
        f.set_state(State::Air);
    } else {
        return false;
    }
    true
}
pub fn st_attack(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, dir: f64) {
    let def = definition(d, f);
    let mv = &d.global.movement;
    let charge = phase(d, f).charge.as_ref().filter(|c| {
        let m = f.mov.as_ref().unwrap();
        m.t == c.frame && m.charge < c.max && if c.button == Button::Special { inp.sp_held } else { inp.atk_held }
    });
    if let Some(c) = charge {
        let m = f.mov.as_mut().unwrap();
        m.charge += 1;
        let n = m.charge;
        if n.is_multiple_of(10) {
            w.sfx("charge");
        }
        if n.is_multiple_of(3) {
            let x = f.x + w.rng.range(-20., 20.);
            let y = f.y - w.rng.range(10., 80.);
            w.part((x, y), (0., -1.), 14., Color::Rgb(0xffe680), 3., 0.);
        }
        if c.jet {
            let x = f.x - f.face * w.rng.range(56., 66.);
            let y = f.y - w.rng.range(56., 70.);
            let vx = -f.face * w.rng.range(2., 4.);
            let vy = w.rng.range(-0.5, 0.5);
            let sz = w.rng.range(3., 6.);
            w.part((x, y), (vx, vy), 12., Color::Rgb(if !n.is_multiple_of(2) { 0xff9a3c } else { 0xffe680 }), sz, 0.);
            if n.is_multiple_of(6) {
                w.sfx("jet");
            }
        }
    } else {
        let m = f.mov.as_ref().unwrap();
        if def.b_reverse && !m.reversed && m.elapsed < mv.b_reverse_window && dir == -f.face {
            f.mov.as_mut().unwrap().reversed = true;
            reverse(f, dir);
        }
        let m = f.mov.as_mut().unwrap();
        m.t += 1;
        m.elapsed += 1;
        events(w, d, i, f);
        let p = phase(d, f);
        let t = f.mov.as_ref().unwrap().t;
        let held = if p.charge.as_ref().is_some_and(|c| c.button == Button::Special) { inp.sp_held } else { inp.atk_held };
        let tr = p.transitions.iter().find(|tr| {
            tr.from.is_none_or(|n| t >= n)
                && cond(f, tr.cond)
                && match tr.on {
                    Trigger::Timeout => p.frames.is_some_and(|n| t >= n),
                    Trigger::Grounded => f.ground,
                    Trigger::Airborne => !f.ground,
                    Trigger::Falling => f.vy >= 0.,
                    Trigger::Release => !held,
                    Trigger::ShieldPress => inp.shield_p,
                    _ => false,
                }
        });
        // The ending tick keeps the phase that ended for movement.
        let old = p.clone();
        if let Some(tr) = tr {
            transition(w, d, i, f, tr);
        } else if p.frames.is_some_and(|n| t >= n) && !p.transitions.iter().any(|t| t.on == Trigger::Timeout) {
            f.set_state(if f.ground { State::Idle } else { State::Air });
        }
        if f.mov.is_none() {
            movement(w, d, i, f, inp, (def, &old, t));
            return;
        }
    }
    let p = phase(d, f);
    let t = f.mov.as_ref().unwrap().t;
    movement(w, d, i, f, inp, (def, p, t));
}
fn movement(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, (def, p, t): (&Move, &Phase, u32)) {
    let dir = inp.dir();
    let mv = &d.global.movement;
    if f.ground {
        f.vx *= if t < p.slide { mv.attack_slide_friction } else { mv.attack_friction };
        fall(w, d, i, f, true, None);
        return;
    }
    let acc = match def.air {
        AirControl::Attack => mv.attack_air_accel,
        AirControl::Aerial => mv.aerial_accel,
        AirControl::UpSpecial => mv.up_special_accel,
    } * p.air_accel_mult;
    air_drift(d, f, dir, acc, mv.attack_air_drag);
    if def.air == AirControl::Aerial {
        fast_fall(d, f, inp);
    }
    let s = &d.fighters[f.data].stats;
    let cap = p.fall_cap.unwrap_or(if f.ff { s.fast_fall_speed } else { s.fall_speed });
    f.vy = (f.vy + s.gravity).min(cap);
    if p.hover {
        f.vy = 0.;
    }
    integrate(w, d, i, f, false);
    if f.state == State::Attack && p.ledge_grab.is_some_and(|n| t >= n) {
        try_ledge(w, d, i, f, inp, true);
    }
}
