//! Subtractive momentum (`movement.ultimate`): air friction, traction, fighter-relative air control, instant fast
//! fall and the initial dash burst, plus the legacy multiplicative path it replaces.
mod common;
use sim::{
    GameData, Input, World,
    data::global::UltimateMovement,
    input::ATTACK,
    world::{Gait, State},
};

fn inp(x: i8, y: i8, buttons: u8) -> Input {
    Input { stick: (x, y), buttons, ..Default::default() }
}
fn tick(w: &mut World, d: &GameData, i: Input) {
    common::step_draining(w, d, &[i]);
}
fn ground(d: &GameData, id: &str) -> World {
    let mut w = common::fighters(d, id, &["brawler"]);
    w.count = 0;
    (w.fighters[0].x, w.fighters[1].x) = (400., 1100.);
    w
}
/// Player 0 airborne high above the stage, moving at `vx`, just past the apex.
fn air(d: &GameData, id: &str, vx: f64) -> World {
    let mut w = ground(d, id);
    let f = &mut w.fighters[0];
    f.set_state(State::Air);
    (f.ground, f.surf, f.y, f.vx, f.vy) = (false, None, 100., vx, 0.5);
    w
}
fn u(d: &GameData) -> &UltimateMovement {
    d.global.movement.ultimate.as_ref().unwrap()
}
fn stats<'a>(d: &'a GameData, id: &str) -> &'a sim::data::Stats {
    &d.fighter(id).unwrap().stats
}
fn air_friction(d: &GameData, id: &str) -> f64 {
    stats(d, id).air_friction.unwrap_or(u(d).air_friction)
}

#[test]
fn neutral_air_speed_decays_linearly() {
    let d = common::data();
    for id in ["brawler", "king", "duchess"] {
        let (a, k) = (stats(&d, id).air_speed, air_friction(&d, id));
        let mut w = air(&d, id, a);
        for _ in 0..15 {
            tick(&mut w, &d, Input::default());
        }
        let kept = w.fighters[0].vx / a;
        assert!((kept - (1. - 15. * k / a)).abs() < 1e-9, "{id}: kept {kept}");
    }
}

#[test]
fn overspeed_above_air_speed_decays_by_friction_instead_of_clamping() {
    let d = common::data();
    let (a, k) = (stats(&d, "brawler").air_speed, air_friction(&d, "brawler"));
    let mut w = air(&d, "brawler", 2. * a);
    tick(&mut w, &d, inp(127, 0, 0));
    assert!((w.fighters[0].vx - (2. * a - k)).abs() < 1e-9);
    // Held forward, overspeed slows to air speed and stays there.
    let mut w = air(&d, "brawler", a + 2.5 * k);
    for _ in 0..6 {
        tick(&mut w, &d, inp(127, 0, 0));
    }
    assert!(!w.fighters[0].ground);
    assert!((w.fighters[0].vx - a).abs() < 1e-9, "held drift settles at air speed");
    // Opposite stick accelerates against the motion at airAccel.
    let mut w = air(&d, "brawler", a);
    tick(&mut w, &d, inp(-127, 0, 0));
    assert!((w.fighters[0].vx - (a - stats(&d, "brawler").air_accel)).abs() < 1e-9);
}

#[test]
fn starting_an_aerial_keeps_the_drift_rule() {
    let d = common::data();
    let vx = 0.8 * stats(&d, "brawler").air_speed;
    let mut plain = air(&d, "brawler", vx);
    let mut nair = air(&d, "brawler", vx);
    for t in 0..12 {
        tick(&mut plain, &d, Input::default());
        tick(&mut nair, &d, inp(0, 0, if t == 0 { ATTACK } else { 0 }));
        assert_eq!(nair.fighters[0].state, State::Attack);
        assert_eq!(plain.fighters[0].vx, nair.fighters[0].vx, "tick {t}");
    }
}

#[test]
fn aerial_drift_follows_the_fighter_air_accel() {
    let d = common::data();
    let gain = |id: &str| {
        let mut w = air(&d, id, 0.);
        w.fighters[0].vy = 0.;
        tick(&mut w, &d, inp(0, 0, ATTACK));
        assert_eq!(w.fighters[0].state, State::Attack);
        let before = w.fighters[0].vx;
        tick(&mut w, &d, inp(127, 0, 0));
        w.fighters[0].vx - before
    };
    let (king, brawler) = (gain("king"), gain("brawler"));
    for (id, g) in [("king", king), ("brawler", brawler)] {
        assert!((g - stats(&d, id).air_accel * u(&d).aerial_accel_mult).abs() < 1e-9, "{id}: {g}");
    }
    assert!(king < brawler);
}

#[test]
fn fast_fall_reaches_fast_fall_speed_on_the_first_tick() {
    let d = common::data();
    for id in ["king", "duchess", "brawler"] {
        let mut w = air(&d, id, 0.);
        tick(&mut w, &d, inp(0, -127, 0));
        assert!(w.fighters[0].ff);
        assert_eq!(w.fighters[0].vy, stats(&d, id).fast_fall_speed, "{id}");
    }
}

/// Ticks and distance to stop from a full run with the stick released.
fn stop(d: &GameData) -> (u32, f64) {
    let mut w = ground(d, "brawler");
    for _ in 0..40 {
        tick(&mut w, d, inp(127, 0, 0));
    }
    assert_eq!(w.fighters[0].gait, Gait::Run);
    let x = w.fighters[0].x;
    for n in 1..120 {
        tick(&mut w, d, Input::default());
        if w.fighters[0].vx == 0. {
            return (n, w.fighters[0].x - x);
        }
    }
    panic!("never stopped");
}

#[test]
fn stopping_from_a_run_is_finite_and_shrinks_with_traction() {
    let mut d = common::data();
    let i = d.fighters.iter().position(|f| f.id == "brawler").unwrap();
    let mut last = (u32::MAX, f64::MAX);
    for k in [0.2, 0.35, 0.6] {
        d.fighters[i].stats.traction = Some(k);
        let (n, dist) = stop(&d);
        assert!(n < 40 && dist > 0., "traction {k}: {n} ticks, {dist} px");
        assert!(n < last.0 && dist < last.1, "traction {k}: {n} ticks, {dist} px after {last:?}");
        last = (n, dist);
    }
}

#[test]
fn dash_burst_exceeds_run_speed_and_eases_into_the_run() {
    let d = common::data();
    let s = stats(&d, "brawler");
    let dash = s.dash_speed.unwrap();
    assert!(dash > s.run_speed);
    let mut w = ground(&d, "brawler");
    tick(&mut w, &d, inp(127, 0, 0));
    assert_eq!(w.fighters[0].vx, dash);
    let k = s.traction.unwrap_or(u(&d).traction);
    let mut prev = dash;
    let mut eased = false;
    for _ in 0..40 {
        tick(&mut w, &d, inp(127, 0, 0));
        let vx = w.fighters[0].vx;
        assert!(vx >= s.run_speed && vx <= prev && prev - vx <= k + 1e-9, "{prev} → {vx}");
        eased |= w.fighters[0].gait == Gait::Run && vx > s.run_speed;
        prev = vx;
    }
    assert!(eased, "speed above the run carries into it");
    assert_eq!(prev, s.run_speed);
}

#[test]
fn legacy_model_keeps_multiplicative_drag_and_shared_accels() {
    let mut d = common::data();
    d.global.movement.ultimate = None;
    let m = d.global.movement.clone();
    let a = stats(&d, "king").air_speed;
    let mut w = air(&d, "king", a);
    tick(&mut w, &d, Input::default());
    assert_eq!(w.fighters[0].vx, a * m.air_drag);
    tick(&mut w, &d, inp(127, 0, 0));
    assert_eq!(w.fighters[0].vx, (a * m.air_drag + stats(&d, "king").air_accel).min(a));
    // Aerials use the shared aerialAccel and attackAirDrag, whatever the fighter's own air accel.
    let mut w = air(&d, "king", 0.);
    tick(&mut w, &d, inp(0, 0, ATTACK));
    let before = w.fighters[0].vx;
    tick(&mut w, &d, inp(127, 0, 0));
    assert_eq!(w.fighters[0].vx, before + m.aerial_accel);
    let v = w.fighters[0].vx;
    tick(&mut w, &d, Input::default());
    assert_eq!(w.fighters[0].vx, v * m.attack_air_drag);
    // Fast fall kicks to the shared speed and accelerates from there.
    let mut w = air(&d, "king", 0.);
    tick(&mut w, &d, inp(0, -127, 0));
    assert_eq!(w.fighters[0].vy, m.fast_fall_kick + stats(&d, "king").gravity);
    // A dash end snaps to run speed; ground friction multiplies.
    let mut w = ground(&d, "king");
    let s = stats(&d, "king");
    for _ in 0..40 {
        tick(&mut w, &d, inp(127, 0, 0));
    }
    assert_eq!(w.fighters[0].vx, s.run_speed);
    tick(&mut w, &d, Input::default());
    tick(&mut w, &d, Input::default());
    assert_eq!(w.fighters[0].vx, s.run_speed * d.global.gait.brake_friction);
}
