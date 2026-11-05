use crate::actions::{Action, accept};
use crate::physics::*;
use crate::{
    GameData, World,
    data::{self, Color, Move, Num, Phase, Transition, Trigger},
    input::{Intent, Strike},
    world::{Fighter, MoveState, State},
};
use data::moves::{AirControl, Button, Cond};

pub fn definition<'a>(d: &'a GameData, f: &Fighter) -> &'a Move {
    let slot = &f.mov.as_ref().expect("attack move").slot;
    d.fighters[f.data].moves.get(slot).expect("known, present move slot")
}
pub fn phase<'a>(d: &'a GameData, f: &Fighter) -> &'a Phase {
    let m = f.mov.as_ref().unwrap();
    &definition(d, f).timeline(m.variant)[m.phase]
}
/// The timeline the current move run is using (variant selected at move start).
pub fn timeline<'a>(d: &'a GameData, f: &Fighter) -> &'a [Phase] {
    definition(d, f).timeline(f.mov.as_ref().map_or(0, |m| m.variant))
}
/// Charge resources: the first variant whose `needs` is satisfied, else the default timeline.
fn choose_variant(d: &GameData, f: &Fighter, m: &Move) -> u8 {
    m.variants.iter().position(|v| crate::charges::has(f, d, &v.needs)).map_or(0, |i| i as u8 + 1)
}
/// Spend a phase's cost, if any, from `f` (the move's own fighter copy), playing the depletion sound.
pub fn pay_cost(w: &mut World, d: &GameData, f: &mut Fighter, p: &Phase) {
    if let Some(cost) = &p.cost
        && let Some(sfx) = crate::charges::spend(d, f, cost)
    {
        w.sfx(sfx);
    }
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
        Num::Range([a, b]) => w.effect_rng().range(a, b),
    }
}
pub fn effects(
    w: &mut World,
    fx: &[data::moves::Fx],
    (x, y): (f64, f64),
    face: f64,
    (vx, vy): (f64, f64),
    player: usize,
) {
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
                w.fx.push(crate::Fx::Burst {
                    x,
                    y,
                    color,
                    player,
                    n: f64::from(n),
                    speed,
                    life: f64::from(life),
                    size,
                    gravity,
                });
            }
            F::Shake { k } => w.fx.push(crate::Fx::Shake { k }),
        }
    }
}
fn events(w: &mut World, d: &GameData, i: usize, f: &mut Fighter) {
    let p = phase(d, f);
    let t = f.mov.as_ref().unwrap().t;
    for e in &p.events {
        let charge = f.mov.as_ref().unwrap().charge;
        if !e.fires(t, w.frame) || !cond(f, e.cond) || charge < e.min_charge {
            continue;
        }
        if e.turn {
            f.face = -f.face;
        }
        let speed = 1. + f64::from(charge) * e.charge_velocity;
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
                f.vx = x * f.face * speed;
            }
            if let Some(y) = v.y {
                f.vy = -y * speed;
            }
        }
        if let Some(v) = e.add_vel {
            if let Some(x) = v.x {
                f.vx += x * f.face * speed;
            }
            if let Some(y) = v.y {
                f.vy -= y * speed;
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
        if let Some(id) = &e.item {
            crate::items::give(f, id);
        }
        if let Some(need) = &e.restore
            && let Some(s) = crate::charges::restore(d, f, need)
        {
            w.sfx(s);
        }
        if let Some(ids) = &e.detonate {
            w.fighters[i] = f.clone();
            crate::projectiles::detonate(w, d, i, ids);
            *f = w.fighters[i].clone();
        }
        if let Some(s) = &e.sfx {
            w.sfx(s.clone());
        }
        effects(w, &e.fx, (f.x, f.y), f.face, (f.vx, f.vy), i);
    }
}
pub fn start_move(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, slot: &str) {
    start_move_as(w, d, i, f, slot, None);
}
/// `start_move` with an optional forced phase timeline (editor previews edit a specific variant). A forced
/// variant is selected before the first phase's entry events and cost run, exactly like a resource-chosen one.
pub fn start_move_as(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, slot: &str, forced: Option<u8>) {
    if d.fighters[f.data].moves.get(slot).is_some_and(|m| m.once_per_airtime)
        && !f.used_air_moves.iter().any(|s| s == slot)
    {
        f.used_air_moves.push(slot.into());
    }
    let variant = match forced {
        Some(v) => d.fighters[f.data].moves.get(slot).map_or(0, |m| v.min(m.variants.len() as u8)),
        None => d.fighters[f.data].moves.get(slot).map_or(0, |m| choose_variant(d, f, m)),
    };
    f.set_state(State::Attack);
    if d.fighters[f.data].moves.get(slot).is_none_or(|m| m.air != AirControl::Aerial) {
        // Only aerials keep a full hop's initial rise and a float.
        (f.rise, f.floating) = (0, 0);
    }
    f.mov = Some(MoveState {
        slot: slot.into(),
        started: w.frame,
        phase: 0,
        variant,
        t: 0,
        elapsed: 0,
        charge: 0,
        charge_hold: 0,
        counter_damage: 0.,
        reversed: false,
        stick: false,
    });
    let p = phase(d, f).clone();
    pay_cost(w, d, f, &p);
    events(w, d, i, f);
}
/// A move chosen by a player's press: recorded as accepted, held by the control that started it.
pub fn start_attack(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, slot: &str, inp: Intent) {
    accept(w, i, Action::Move(slot.into()));
    start_move(w, d, i, f, slot);
    if let (Some(m), Strike::Stick { .. }) = (f.mov.as_mut(), inp.strike) {
        m.stick = true;
    }
}
/// The attack control holding the current move: the attack stick or the attack button.
pub fn attack_held(f: &Fighter, inp: &Intent) -> bool {
    if f.mov.as_ref().is_some_and(|m| m.stick) { inp.stick_held } else { inp.atk_held }
}
/// Everything a transition must satisfy apart from its trigger: `from`, the held-charge tier, the condition
/// and the charge-resource gate. Every path that selects a transition filters on this, so a gated branch cannot
/// be taken by one caller and refused by another.
fn eligible(d: &GameData, f: &Fighter, tr: &Transition, t: u32) -> bool {
    tr.from.is_none_or(|n| t >= n)
        && f.mov.as_ref().is_none_or(|m| m.charge >= tr.min_charge)
        && cond(f, tr.cond)
        && tr.needs.as_ref().is_none_or(|need| crate::charges::has(f, d, need))
}
/// A phase input window can accept a buffered follow-up while the containing move remains locked.
pub fn accepts_command(d: &GameData, f: &Fighter, inp: &Intent) -> bool {
    f.state == State::Attack
        && f.mov.is_some()
        && phase(d, f).transitions.iter().any(|tr| {
            // Commands are delivered on the next tick, so `from` is checked against that frame.
            eligible(d, f, tr, f.mov.as_ref().unwrap().t.saturating_add(1))
                && match tr.on {
                    Trigger::AttackPress => inp.atk,
                    Trigger::SpecialPress => inp.sp,
                    _ => false,
                }
        })
}
fn transition(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, tr: &Transition) {
    match tr.to.as_str() {
        "end" => f.set_state(if f.ground { State::Idle } else { State::Air }),
        "helpless" => f.set_state(if f.ground { State::Idle } else { State::Helpless }),
        "landing" => landing(f, tr.lag.unwrap_or(0)),
        name => {
            let n = timeline(d, f).iter().position(|p| p.name == name).expect("validated phase");
            {
                let m = f.mov.as_mut().unwrap();
                m.phase = n;
                m.t = 0;
            }
            if !tr.preserve_hits {
                f.sd.clear_hits();
            }
            let p = phase(d, f).clone();
            pay_cost(w, d, f, &p);
            events(w, d, i, f);
        }
    }
}
pub fn trigger(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, on: Trigger) -> bool {
    if f.mov.is_none() {
        return false;
    }
    let t = f.mov.as_ref().unwrap().t;
    if let Some(tr) = phase(d, f).transitions.iter().find(|tr| tr.on == on && eligible(d, f, tr, t)) {
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
    accept(w, i, Action::Move("uspec".into()));
    start_move(w, d, i, f, "uspec");
}
pub fn special(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, dir: f64) {
    if inp.up && !f.up_b {
        up_special(w, d, i, f, dir);
        return;
    }
    let slot = if inp.down {
        "dspec"
    } else if dir != 0. && d.fighters[f.data].moves.sspec.is_some() {
        "sspec"
    } else {
        "nspec"
    };
    if d.fighters[f.data].moves.get(slot).is_some_and(|m| m.once_per_airtime)
        && f.used_air_moves.iter().any(|s| s == slot)
    {
        return;
    }
    reverse(f, dir);
    accept(w, i, Action::Move(slot.into()));
    start_move(w, d, i, f, slot);
}
pub fn aerial_slot(f: &Fighter, inp: Intent) -> &'static str {
    let (ax, ay) = if let Strike::Stick { x, y, .. } = inp.strike { (x, y) } else { (0, 0) };
    let stick = (ax, ay) != (0, 0);
    let dir = if stick { f64::from(ax) } else { inp.dir() };
    if ay > 0 || !stick && inp.up {
        "uair"
    } else if ay < 0 || !stick && inp.down {
        "dair"
    } else if dir != 0. && dir == -f.face {
        "bair"
    } else if dir != 0. {
        "fair"
    } else {
        "nair"
    }
}
pub fn start_jump(w: &mut World, i: usize, f: &mut Fighter, inp: Intent, aerials: bool) {
    accept(w, i, Action::Jump);
    let aerial = (aerials && inp.jump_attack).then(|| aerial_slot(f, inp).to_string());
    f.set_state(State::Jumpsquat);
    f.sd.jump_aerial = aerial;
    f.sd.short = aerials && inp.short_hop;
}
/// Ground attack for a press. The attack stick and attack + special pick a fixed smash / tilt; the button picks
/// by the movement stick: a recent flick smashes, a held direction tilts, neutral jabs, and dashing / running
/// dash attacks (an up flick still up smashes; a forward flick early in a dash still forward smashes).
fn ground_attack(d: &GameData, f: &mut Fighter, inp: Intent, dir: f64) -> &'static str {
    let mv = &d.global.movement;
    let tap = if inp.tap_r {
        1.
    } else if inp.tap_l {
        -1.
    } else {
        0.
    };
    match inp.strike {
        Strike::Stick { x, y, smash } => {
            if x != 0 {
                f.face = f64::from(x);
            }
            match (x != 0, y > 0, smash) {
                (true, _, true) => "fsmash",
                (true, _, false) => "ftilt",
                (_, true, true) => "usmash",
                (_, true, false) => "utilt",
                (_, _, true) => "dsmash",
                _ => "dtilt",
            }
        }
        Strike::Combo => {
            if inp.up {
                "usmash"
            } else if inp.down {
                "dsmash"
            } else {
                if dir != 0. {
                    f.face = dir;
                }
                "fsmash"
            }
        }
        Strike::Button if crate::gait::dashing(d, f) => {
            if inp.tap_u {
                "usmash"
            } else {
                "dashatk"
            }
        }
        Strike::Button => {
            if tap != 0. {
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
            } else if !d.global.gait.enabled && dir != 0. && dir == f.face && f.vx.abs() > mv.dash_attack_speed {
                "dashatk"
            } else if dir != 0. {
                f.face = dir;
                "ftilt"
            } else {
                "jab"
            }
        }
    }
}
pub fn ground_actions(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, dir: f64) -> bool {
    let mv = &d.global.movement;
    if (inp.atk || inp.grab) && crate::items::take(w, d, i, f) {
        return true;
    }
    let holding = f.item.is_some();
    if holding && (inp.grab || inp.atk && !(inp.jump_attack && d.global.input.jump_aerials)) {
        // Holding an item, attacks and grab throw it instead.
        let (kind, smash) = crate::items::aim(d, f, &inp);
        crate::items::toss(w, d, i, f, kind, smash);
    } else if inp.grab {
        let gait = d.global.gait.enabled;
        let dash = if gait { crate::gait::dashing(d, f) } else { f.vx.abs() > mv.dash_grab_speed };
        if dir != 0. && !(gait && dash) {
            f.face = dir;
        }
        start_attack(w, d, i, f, if dash { "dashgrab" } else { "grab" }, inp);
    } else if inp.shield || inp.shield_p {
        accept(w, i, Action::Shield);
        f.set_state(State::Shield);
    } else if inp.sp {
        special(w, d, i, f, inp, dir);
    } else if inp.jump_attack && d.global.input.jump_aerials {
        start_jump(w, i, f, inp, true);
    } else if inp.atk {
        let slot = ground_attack(d, f, inp, dir);
        start_attack(w, d, i, f, slot, inp);
    } else if inp.jump {
        start_jump(w, i, f, inp, d.global.input.jump_aerials);
    } else if inp.flick_d && f.surf.is_some_and(|n| n >= d.stages[w.stage].blocks.len()) {
        // A down flick drops through; a slow press crouches, so down + attack stays a down tilt.
        accept(w, i, Action::PlatformDrop);
        if mv.platform_pass {
            // The drop's down press must not become an airborne fast-fall command.
            f.history.last_press[3] = -99;
        }
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
        m.t == c.frame
            && (m.charge < c.max || m.charge_hold < c.hold_after_max)
            && if c.button == Button::Special { inp.sp_held } else { attack_held(f, &inp) }
    });
    if let Some(c) = charge {
        let m = f.mov.as_mut().unwrap();
        if m.charge < c.max {
            m.charge += 1;
        } else {
            m.charge_hold += 1;
        }
        let n = m.charge.saturating_add(m.charge_hold);
        if n.is_multiple_of(10) {
            w.sfx("charge");
        }
        if n.is_multiple_of(3) {
            let x = f.x + w.effect_rng().range(-20., 20.);
            let y = f.y - w.effect_rng().range(10., 80.);
            w.part((x, y), (0., -1.), 14., Color::Rgb(0xffe680), 3., 0.);
        }
        if c.jet {
            let x = f.x - f.face * w.effect_rng().range(56., 66.);
            let y = f.y - w.effect_rng().range(56., 70.);
            let vx = -f.face * w.effect_rng().range(2., 4.);
            let vy = w.effect_rng().range(-0.5, 0.5);
            let sz = w.effect_rng().range(3., 6.);
            w.part((x, y), (vx, vy), 12., Color::Rgb(if !n.is_multiple_of(2) { 0xff9a3c } else { 0xffe680 }), sz, 0.);
            if n.is_multiple_of(6) {
                w.sfx("jet");
            }
        }
        // A charging move can still aim once within its own window (Long-Stem Strike's direction input).
        let m = f.mov.as_ref().unwrap();
        if def.b_reverse && !m.reversed && (phase(d, f).aim || m.elapsed < mv.b_reverse_window) && dir == -f.face {
            f.mov.as_mut().unwrap().reversed = true;
            reverse(f, dir);
        }
    } else {
        let m = f.mov.as_mut().unwrap();
        m.t += 1;
        m.elapsed += 1;
        let m = f.mov.as_ref().unwrap();
        if def.b_reverse && !m.reversed && (phase(d, f).aim || m.elapsed < mv.b_reverse_window) && dir == -f.face {
            f.mov.as_mut().unwrap().reversed = true;
            reverse(f, dir);
        }
        events(w, d, i, f);
        let p = phase(d, f);
        let t = f.mov.as_ref().unwrap().t;
        let frames = p
            .frames
            .map(|n| n.saturating_add(libm::round(f64::from(f.mov.as_ref().unwrap().charge) * p.charge_frames) as u32));
        let held = if p.charge.as_ref().is_some_and(|c| c.button == Button::Special) {
            inp.sp_held
        } else {
            attack_held(f, &inp)
        };
        let tr = p.transitions.iter().find(|tr| {
            eligible(d, f, tr, t)
                && match tr.on {
                    Trigger::Timeout => frames.is_some_and(|n| t >= n),
                    Trigger::Grounded => f.ground,
                    Trigger::Airborne => !f.ground,
                    Trigger::Falling => f.vy >= 0.,
                    Trigger::Release => !held,
                    // A directional throw: the same release, gated on which way the stick points.
                    Trigger::ReleaseToward => !held && inp.x * f.face > 0.,
                    Trigger::ReleaseAway => !held && inp.x * f.face < 0.,
                    Trigger::AttackHeld => attack_held(f, &inp),
                    Trigger::AttackPress => inp.atk,
                    Trigger::SpecialPress => inp.sp,
                    Trigger::UpHeld => inp.up,
                    Trigger::TargetNear => p.detection.is_some_and(|r| {
                        let (x1, x2) = (f.x + r.x1 * f.face, f.x + r.x2 * f.face);
                        w.fighters.iter().enumerate().any(|(v, other)| {
                            w.opponents(i, v)
                                && other.hittable()
                                && crate::combat::rect_hurt(
                                    d,
                                    other,
                                    w.frame,
                                    crate::data::Rect {
                                        x1: x1.min(x2),
                                        x2: x1.max(x2),
                                        y1: f.y - r.y2,
                                        y2: f.y - r.y1,
                                    },
                                )
                        })
                    }),
                    Trigger::ShieldPress => inp.shield_p,
                    _ => false,
                }
        });
        // The ending tick keeps the phase that ended for movement.
        if let Some(tr) = tr {
            if matches!(tr.on, Trigger::AttackPress | Trigger::SpecialPress) {
                accept(w, i, Action::Followup { slot: f.mov.as_ref().unwrap().slot.clone(), phase: tr.to.clone() });
            }
            transition(w, d, i, f, tr);
        } else if frames.is_some_and(|n| t >= n) && !p.transitions.iter().any(|t| t.on == Trigger::Timeout) {
            f.set_state(if f.ground { State::Idle } else { State::Air });
        }
        if f.mov.is_none() {
            movement(w, d, i, f, inp, (def, p, t));
            return;
        }
    }
    let p = phase(d, f);
    let t = f.mov.as_ref().unwrap().t;
    movement(w, d, i, f, inp, (def, p, t));
}
fn movement(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, (def, p, t): (&Move, &Phase, u32)) {
    let mv = &d.global.movement;
    if p.pass_platforms {
        f.drop = f.drop.max(2);
    }
    if let Some(speed) = p.travel_speed {
        if t == 0 {
            let (x, y) = if inp.x == 0. && inp.y == 0. { (0., 1.) } else { (inp.x, inp.y) };
            let len = libm::hypot(x, y);
            f.sd.dx = x / len;
            f.sd.dy = -y / len;
        }
        f.vx = f.sd.dx * speed;
        f.vy = f.sd.dy * speed;
        integrate(w, d, i, f, false);
        if f.state == State::Attack && p.ledge_grab.is_some_and(|n| t >= n) {
            try_ledge(w, d, i, f, inp, true);
        }
        return;
    }
    if f.ground {
        if p.ground_speed > 0. {
            f.vx = inp.x * p.ground_speed;
        } else if t < p.slide {
            skid(d, f, mv.attack_slide_friction, mv.ultimate.as_ref().map_or(1., |u| u.attack_slide_traction));
        } else {
            skid(d, f, mv.attack_friction, 1.);
        }
        fall(w, d, i, f, true, None);
        return;
    }
    let acc = match def.air {
        AirControl::Attack => air_accel(d, f, mv.attack_air_accel, |u| u.attack_air_accel_mult),
        AirControl::Aerial => air_accel(d, f, mv.aerial_accel, |u| u.aerial_accel_mult),
        AirControl::UpSpecial => air_accel(d, f, mv.up_special_accel, |u| u.up_special_accel_mult),
    } * p.air_accel_mult;
    air_drift(d, f, inp.x, acc, mv.attack_air_drag);
    if def.air == AirControl::Aerial && f.floating == 0 && fast_fall(d, f, inp) {
        accept(w, i, Action::FastFall);
    }
    let s = &d.fighters[f.data].stats;
    let cap = p.fall_cap.unwrap_or(if f.ff { s.fast_fall_speed } else { s.fall_speed });
    if def.air != AirControl::Aerial || !crate::physics::scripted_rise(d, f) {
        f.vy = (f.vy + s.gravity).min(cap);
    }
    if p.hover {
        f.vy = 0.;
    }
    integrate(w, d, i, f, false);
    if f.state == State::Attack && p.ledge_grab.is_some_and(|n| t >= n) {
        try_ledge(w, d, i, f, inp, true);
    }
}
