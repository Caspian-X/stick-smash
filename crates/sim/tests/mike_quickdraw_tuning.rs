mod common;
use sim::{
    GameData, Input, World,
    input::{Intent, SPECIAL},
    moves,
    world::State,
};

fn setup(air: bool) -> (GameData, World) {
    let mut d = common::data();
    for s in &mut d.stages {
        s.blocks[0].left = -10000.;
        s.blocks[0].right = 10000.;
        s.blast.top = -10000.;
        s.blast.left = -10000.;
        s.blast.right = 10000.;
    }
    let mut w = common::fighters(&d, "mike", &["brawler"]);
    w.count = 0;
    (w.fighters[0].x, w.fighters[1].x) = (0., 5000.);
    w.fighters[0].face = 1.;
    if air {
        let f = &mut w.fighters[0];
        (f.y, f.ground, f.surf) = (-2000., false, None);
    }
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "sspec");
    w.fighters[0] = f;
    (d, w)
}
fn tick(w: &mut World, d: &GameData, held: bool) {
    let mut f = w.fighters[0].clone();
    moves::st_attack(w, d, 0, &mut f, Intent { sp_held: held, ..Default::default() }, 0.);
    w.fighters[0] = f;
    w.frame += 1;
}
fn release(w: &mut World, d: &GameData, charge: u32) {
    for _ in 0..15 + charge {
        tick(w, d, true);
    }
    tick(w, d, false);
    assert_eq!(moves::phase(d, &w.fighters[0]).name, "rush");
}

#[test]
fn charge_strength_saturates_but_can_hold_two_seconds_longer() {
    let (d, mut w) = setup(true);
    for _ in 0..15 + 69 {
        tick(&mut w, &d, true);
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().charge, 69);
    for _ in 0..120 {
        tick(&mut w, &d, true);
    }
    let m = w.fighters[0].mov.as_ref().unwrap();
    assert_eq!((m.charge, m.charge_hold, m.t), (69, 120, 15));
    let mut restored = w.clone();
    tick(&mut w, &d, true);
    tick(&mut restored, &d, true);
    assert_eq!(w, restored);
    assert_eq!(moves::phase(&d, &w.fighters[0]).name, "rush");
    let full_speed = w.fighters[0].vx;
    let (d, mut early) = setup(true);
    release(&mut early, &d, 69);
    assert_eq!(early.fighters[0].vx, full_speed);
}

#[test]
fn shorter_rush_is_bursty_with_requested_ground_distance_extrema() {
    for charge in [0, 35, 69] {
        let (d, mut w) = setup(false);
        for _ in 0..15 + charge {
            tick(&mut w, &d, true);
        }
        let start = w.fighters[0].x;
        tick(&mut w, &d, false);
        let speed = w.fighters[0].vx;
        assert!(speed > 15., "uncharged launch must already be quick");
        for _ in 1..18 {
            tick(&mut w, &d, false);
        }
        let distance = w.fighters[0].x - start;
        // The former distance, in px at 5 px/unit (it was measured at 3.5); rescaled speeds are rounded to 6 decimals.
        let old_distance = 37. * 8. * (1. + f64::from(charge) * 0.012) * 0.95 * 5. / 3.5;
        if charge == 0 {
            assert!((distance / old_distance - 0.75).abs() < 1e-6);
        }
        if charge == 69 {
            assert!((distance / old_distance - 1.).abs() < 1e-6);
        }
        tick(&mut w, &d, false);
        assert_eq!(w.fighters[0].state, State::Idle);
    }
}

#[test]
fn air_contact_keeps_momentum_and_recovers_earlier_at_low_charge() {
    for (charge, duration) in [(0, 20), (35, 32), (69, 44)] {
        let (d, mut w) = setup(true);
        release(&mut w, &d, charge);
        let speed = w.fighters[0].vx;
        w.fighters[1].x = w.fighters[0].x + 65.;
        w.fighters[1].y = w.fighters[0].y;
        tick(&mut w, &d, false);
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, "airSwing");
        assert_eq!(w.fighters[0].vx, 0., "brake until the sword connects");
        for _ in 0..5 {
            tick(&mut w, &d, false);
        }
        assert!(w.fighters[0].vx > speed * 0.6, "resume carry after the active swing");
        for _ in 5..duration - 1 {
            tick(&mut w, &d, false);
        }
        assert_eq!(w.fighters[0].state, State::Attack);
        tick(&mut w, &d, false);
        assert_eq!(w.fighters[0].state, State::Air);
        assert!(w.fighters[0].vx > speed * 0.25);
    }
}

#[test]
fn aerial_whiff_keeps_rush_momentum_but_cannot_use_up_special() {
    let (d, mut w) = setup(true);
    release(&mut w, &d, 0);
    let speed = w.fighters[0].vx;
    for _ in 0..18 {
        tick(&mut w, &d, false);
    }
    assert_eq!(moves::phase(&d, &w.fighters[0]).name, "specialFall");
    assert!(w.fighters[0].vx > speed * 0.95);
    common::step_draining(
        &mut w,
        &d,
        &[Input { stick: (0, 127), buttons: SPECIAL, ..Default::default() }, Input::default()],
    );
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "sspec");
}

#[test]
fn buffered_up_special_starts_after_a_real_uncharged_aerial_hit() {
    let (d, mut w) = setup(true);
    release(&mut w, &d, 0);
    w.fighters[1].x = w.fighters[0].x + 65.;
    w.fighters[1].y = w.fighters[0].y;
    w.fighters[1].ground = false;
    w.fighters[1].surf = None;
    w.fighters[1].vy = 0.;
    let mut connected = false;
    for _ in 0..60 {
        sim::step_world(&mut w, &d, &[Input::default(), Input::default()]);
        connected |= w.hits.iter().any(|h| h.attacker == Some(0));
        if w.fighters[0].mov.as_ref().is_some_and(|m| m.phase == 3 && m.t == 18) {
            break;
        }
    }
    assert!(connected, "the air swing must really connect, including ordinary hitlag");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().t, 18);
    common::step_draining(
        &mut w,
        &d,
        &[Input { stick: (0, 127), buttons: SPECIAL, ..Default::default() }, Input::default()],
    );
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "sspec");
    for _ in 0..4 {
        common::step_draining(&mut w, &d, &[Input::default(), Input::default()]);
        if w.fighters[0].mov.as_ref().is_some_and(|m| m.slot == "uspec") {
            return;
        }
    }
    panic!("buffered up special was not accepted after the aerial swing");
}

#[test]
fn charge_duration_extensions_reject_invalid_content() {
    let d = common::data();
    for n in [-1., f64::NAN, f64::INFINITY] {
        let mut f = d.fighter("mike").unwrap().clone();
        f.moves.sspec.as_mut().unwrap().phases[2].charge_frames = n;
        let err = sim::data::load::check_fighter("mike", &f).unwrap_err();
        assert!(err.path.ends_with("chargeFrames"), "{err:?}");
    }
    let mut f = d.fighter("mike").unwrap().clone();
    f.moves.sspec.as_mut().unwrap().phases[2].frames = None;
    assert!(sim::data::load::check_fighter("mike", &f).unwrap_err().path.ends_with("chargeFrames"));
}

#[test]
fn air_swing_keeps_detected_targets_in_range_at_every_charge() {
    for charge in [0, 35, 69] {
        for offset in [35., 65., 90.] {
            let (d, mut w) = setup(true);
            release(&mut w, &d, charge);
            w.fighters[1].x = w.fighters[0].x + offset;
            w.fighters[1].y = w.fighters[0].y;
            w.fighters[1].ground = false;
            w.fighters[1].surf = None;
            let mut hit = false;
            for _ in 0..8 {
                sim::step_world(&mut w, &d, &[Input::default(), Input::default()]);
                hit |= w.hits.iter().any(|h| h.attacker == Some(0));
            }
            assert!(hit, "charge {charge}, target {offset}px ahead");
        }
    }
}
