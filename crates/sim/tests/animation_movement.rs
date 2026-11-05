//! Movement-state animation timing: input -> readable pose, with planted feet where the state is grounded.

mod common;

use sim::{
    GameData, Input, World,
    input::{JUMP, SHIELD},
    pose::{Skel, fighter_pose, state_anim},
    world::{Gait, State},
};

fn world(d: &GameData) -> World {
    let mut w = common::fighters(d, "brawler", &["brawler"]);
    w.count = 0;
    w
}

fn tick(w: &mut World, d: &GameData, x: i8, y: i8, buttons: u8) {
    common::step_draining(w, d, &[Input { stick: (x, y), buttons, ..Default::default() }]);
}

fn feet(p: &Skel) -> (f64, f64) {
    (p.foot_f[1], p.foot_b[1])
}

#[test]
fn jumpsquat_compresses_then_extends_and_keeps_feet_planted() {
    let d = common::data();
    let mut w = world(&d);
    tick(&mut w, &d, 0, 0, 0);
    tick(&mut w, &d, 0, 0, JUMP);
    assert_eq!(w.fighters[0].state, State::Jumpsquat);
    let p0 = fighter_pose(&d, &w.fighters[0], w.frame);
    tick(&mut w, &d, 0, 0, 0);
    let p1 = fighter_pose(&d, &w.fighters[0], w.frame);
    tick(&mut w, &d, 0, 0, 0);
    let p2 = fighter_pose(&d, &w.fighters[0], w.frame);

    for p in [&p0, &p1, &p2] {
        assert_eq!(feet(p), (0., 0.), "jumpsquat feet leave the ground early");
    }
    assert!(p1.head[1] < p0.head[1], "jumpsquat should load lower on its second frame");
    assert!(p2.head[1] > p1.head[1], "jumpsquat should extend before takeoff");
    assert!(p2.hip[1] > p1.hip[1]);

    tick(&mut w, &d, 0, 0, 0);
    assert_eq!(w.fighters[0].state, State::Air);
    let rise = fighter_pose(&d, &w.fighters[0], w.frame);
    assert!(rise.foot_f[1] > 0. && rise.foot_b[1] >= 0., "takeoff should tuck the legs clear of the ground");
}

#[test]
fn short_landing_recovers_on_its_last_visible_tick() {
    let d = common::data();
    for id in ["brawler", "king", "whiplash"] {
        let mut w = common::fighters(&d, id, &["brawler"]);
        tick(&mut w, &d, 0, 0, 0);
        sim::physics::landing(&mut w.fighters[0], d.global.landing.hard);
        let mut poses = vec![];
        while w.fighters[0].state == State::Landing {
            poses.push(fighter_pose(&d, &w.fighters[0], w.frame));
            tick(&mut w, &d, 0, 0, 0);
            assert!(poses.len() <= 3);
        }
        assert_eq!(poses.len(), 3);
        assert_eq!(w.fighters[0].state, State::Idle);
        for p in &poses {
            assert_eq!(feet(p), (0., 0.));
        }
        assert!(poses[0].head[1] < poses[1].head[1] && poses[1].head[1] < poses[2].head[1]);
        let idle = fighter_pose(&d, &w.fighters[0], w.frame);
        assert!((idle.head[1] - poses.last().unwrap().head[1]).abs() < 3.);
    }
}

#[test]
fn run_cycle_has_a_body_bob_and_a_foot_down_at_each_contact() {
    let d = common::data();
    let w = world(&d);
    let mut f = w.fighters[0].clone();
    f.state = State::Idle;
    f.ground = true;
    f.gait = Gait::Run;
    f.vx = 4.;
    let sample = |degrees: f64| {
        let mut f = f.clone();
        f.run_ph = degrees.to_radians();
        fighter_pose(&d, &f, 0)
    };
    let passing = sample(0.);
    let contact = sample(90.);
    let other_contact = sample(270.);

    assert_eq!(state_anim(&d, &f).0, "run");
    assert!(passing.head[1] > contact.head[1], "the body should rise over the passing leg");
    assert_eq!(feet(&contact), (0., 0.), "both feet land at contact");
    assert_eq!(feet(&other_contact), (0., 0.), "both feet land at the mirrored contact");
    assert!(passing.foot_f[1] > 0. || passing.foot_b[1] > 0.);
    assert_ne!(passing.foot_f, sample(180.).foot_f);
}

#[test]
fn crouch_breathing_keeps_its_feet_planted() {
    let d = common::data();
    let w = world(&d);
    let mut f = w.fighters[0].clone();
    f.state = State::Idle;
    f.ground = true;
    f.gait = Gait::Crouch;
    f.gait_t = d.global.gait.crouch_frames;
    f.vx = 0.;
    let low = fighter_pose(&d, &f, 0);
    let high = fighter_pose(&d, &f, 30);

    assert_eq!(state_anim(&d, &f).0, "crouch");
    assert!(high.head[1] < low.head[1], "the crouch breath should visibly settle");
    assert_eq!(feet(&low), (0., 0.));
    assert_eq!(feet(&high), (0., 0.));
}

#[test]
fn spot_dodge_ducks_on_acceptance_and_recovers_before_exit() {
    let d = common::data();
    let mut w = world(&d);
    tick(&mut w, &d, 0, 0, SHIELD);
    assert_eq!(w.fighters[0].state, State::Shield);
    tick(&mut w, &d, 0, -127, SHIELD);
    assert_eq!(w.fighters[0].state, State::Spot);
    assert_eq!(w.fighters[0].t, 0);
    let dodge = fighter_pose(&d, &w.fighters[0], w.frame);
    assert!(dodge.head[0] < -5. && dodge.head[1] < 60.);
    let mut last = dodge;
    for _ in 0..d.global.dodge.spot.frames {
        last = fighter_pose(&d, &w.fighters[0], w.frame);
        assert_eq!(feet(&last), (0., 0.));
        tick(&mut w, &d, 0, 0, 0);
    }
    assert_eq!(w.fighters[0].state, State::Idle);
    assert!(last.head[1] > dodge.head[1]);
    assert!((last.head[1] - fighter_pose(&d, &w.fighters[0], w.frame).head[1]).abs() < 2.);
}

#[test]
fn hitstun_is_state_timed_so_hitlag_holds_the_recoil() {
    let d = common::data();
    let w = world(&d);
    let mut f = w.fighters[0].clone();
    f.set_state(State::Hitstun);
    f.tumble = false;
    let recoil = fighter_pose(&d, &f, 0);
    assert_eq!(recoil, fighter_pose(&d, &f, 500), "hitstun must not keep flailing during hitlag");
    f.t = 24;
    let recovered = fighter_pose(&d, &f, 0);
    assert!(recoil.rot < recovered.rot, "hitstun should settle out of its initial recoil");
    assert!(recoil.head[0] < recovered.head[0]);
}

#[test]
fn airdodge_unfolds_before_neutral_and_directional_exits_and_can_land_early() {
    let d = common::data();
    for x in [0, 127] {
        let mut w = world(&d);
        let f = &mut w.fighters[0];
        f.set_state(State::Air);
        f.ground = false;
        f.surf = None;
        f.y = 0.;
        tick(&mut w, &d, x, 0, SHIELD);
        assert_eq!(w.fighters[0].state, State::Airdodge);
        let entry = fighter_pose(&d, &w.fighters[0], w.frame);
        let duration = if x == 0 { d.global.dodge.air.frames } else { d.global.dodge.dir_air.frames };
        let mut tuck = entry;
        let mut last = entry;
        for t in 0..duration {
            let f = &w.fighters[0];
            assert_eq!(f.state, State::Airdodge);
            last = fighter_pose(&d, f, w.frame);
            if t == 3 {
                tuck = last;
            }
            // Keep the exit test clear of platforms; landing is exercised separately below.
            w.fighters[0].y = 0.;
            tick(&mut w, &d, 0, 0, 0);
        }
        assert_eq!(w.fighters[0].state, State::Air);
        assert!(entry.head[1] > tuck.head[1]);
        assert_eq!(last, entry, "dodge should fully unfold on its last visible tick");
    }
    let mut w = world(&d);
    let y = w.fighters[0].y;
    let f = &mut w.fighters[0];
    f.set_state(State::Air);
    f.ground = false;
    f.surf = None;
    f.y = y - 4.;
    f.vy = 2.;
    tick(&mut w, &d, 0, 0, SHIELD);
    assert_eq!(w.fighters[0].state, State::Airdodge);
    tick(&mut w, &d, 0, 0, 0);
    assert_eq!(w.fighters[0].state, State::Landing);
    assert_eq!(feet(&fighter_pose(&d, &w.fighters[0], w.frame)), (0., 0.));
}
