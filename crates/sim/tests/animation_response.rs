//! Presentation guards for the fitted King/Whiplash locomotion states. These sample resolved poses only:
//! the aim is that takeoff, apex, landing and the Jet Hammer carry read as motion weights without
//! touching move timing or contact geometry.

mod common;
use sim::{
    GameData,
    pose::{fighter_pose, state_anim},
    world::{Gait, State},
};

fn fighter(d: &GameData, id: &str) -> sim::world::Fighter {
    common::fighters(d, id, &["brawler"]).fighters[0].clone()
}

fn at(d: &GameData, f: &mut sim::world::Fighter, state: State, t: u32) -> sim::pose::Skel {
    f.state = state;
    f.t = t;
    fighter_pose(d, f, 0)
}

#[test]
fn king_takeoff_stretch_and_landing_absorb_are_ordered_by_height() {
    let d = common::data();
    let mut f = fighter(&d, "king");
    f.state = State::Idle;
    f.gait = Gait::Stand;
    let stand = at(&d, &mut f, State::Idle, 0);
    let stand_head = stand.head[1];

    // The three-tick jumpsquat sinks below the idle stance and keeps both feet planted.
    let load_start = at(&d, &mut f, State::Jumpsquat, 0);
    let load_end = at(&d, &mut f, State::Jumpsquat, 2);
    assert_eq!(state_anim(&d, &f).0, "jumpsquat");
    assert!(load_end.head[1] < load_start.head[1] && load_start.head[1] < stand_head);
    for s in [&load_start, &load_end] {
        assert_eq!((s.foot_f[1], s.foot_b[1]), (0., 0.));
    }

    // Takeoff extends past the standing height, then tucks back into the ordinary rise.
    f.vy = -1.;
    let launch = at(&d, &mut f, State::Air, 0);
    assert_eq!(state_anim(&d, &f).0, "air.rise");
    let tuck = at(&d, &mut f, State::Air, 4);
    let settled = at(&d, &mut f, State::Air, 16);
    assert!(launch.head[1] > stand_head + 4.);
    assert!(launch.foot_f[1] < 10.);
    assert!(tuck.head[1] < launch.head[1] && tuck.head[1] > load_end.head[1]);
    // Keys hold after the last lerp, so the settled rise is the ordinary airborne pose.
    assert_eq!(
        settled.head,
        sim::pose::resolve(d.fighter("king").unwrap(), &d.fighter("king").unwrap().poses["air"]).head
    );
    assert!(settled.head[1] < tuck.head[1] && settled.head[1] > load_end.head[1]);

    // Landing absorbs deeper than the jump load and recovers through a settle to the standing height.
    f.vy = 0.;
    let absorb = at(&d, &mut f, State::Landing, 0);
    let settle = at(&d, &mut f, State::Landing, 3);
    let recovered = at(&d, &mut f, State::Landing, 10);
    assert!(absorb.head[1] < load_end.head[1]);
    assert!(absorb.head[1] < settle.head[1] && settle.head[1] < recovered.head[1]);
    assert_eq!(recovered.head[1], stand_head);
    assert_eq!((absorb.foot_f[1], absorb.foot_b[1]), (0., 0.));

    // A held fall keeps a live silhouette instead of one frozen pose.
    f.vy = 1.;
    f.state = State::Air;
    assert_eq!(state_anim(&d, &f).0, "air.fall");
    let a = fighter_pose(&d, &f, 0);
    let b = fighter_pose(&d, &f, 24);
    assert_ne!(a.hand_f, b.hand_f);
    assert!(a.foot_f[1] < tuck.foot_f[1], "the braced fall extends below the rise tuck");
}

#[test]
fn king_jet_carry_braces_without_moving_the_hammer_grip_or_adding_hitboxes() {
    let d = common::data();
    let king = d.fighter("king").unwrap();
    let charge = king.moves.dspec.phases.iter().find(|p| p.name == "charge").unwrap();
    assert!(charge.hitboxes.is_empty(), "the carry itself must stay non-offensive");
    let keys = &charge.attach["hammer"].angle;
    // The strain wobble ends on the charge-entry angle, so release starts from the grip the player saw.
    assert!(keys.len() > 2);
    assert_eq!(keys.first().unwrap()[1], 105.);
    assert_eq!(keys.last().unwrap()[1], 105.);
    assert_eq!(charge.anim.last().unwrap().pose, sim::data::visual::PoseRef::Named("jetStrain".into()));

    // Hold IK hangs the hammer off the front shoulder, so the braced stance keeps that shoulder fixed.
    let brace = sim::pose::resolve(king, &king.poses["jetBrace"]);
    let crouch = sim::pose::resolve(king, &king.poses["crouch"]);
    assert_eq!(brace.shoulder, crouch.shoulder);

    // Fitted carry poses are cosmetic: neither fighter's collision follows a named rig.
    let whiplash = d.fighter("whiplash").unwrap();
    assert!(king.rig.is_none() && whiplash.rig.is_none());
}

#[test]
fn whiplash_air_states_trail_and_landing_recovers_before_idle() {
    let d = common::data();
    let mut f = fighter(&d, "whiplash");
    f.state = State::Idle;
    let stand = at(&d, &mut f, State::Idle, 0);

    // A coiled jump squat passes through a half crouch into a deeper load than the idle stance.
    let half = at(&d, &mut f, State::Jumpsquat, 0);
    let coil = at(&d, &mut f, State::Jumpsquat, 2);
    assert!(coil.head[1] < half.head[1] && half.head[1] < stand.head[1]);
    assert_eq!((coil.foot_f[1], coil.foot_b[1]), (0., 0.));

    // The rise reaches up with both hands and lifted feet; the fall spreads the legs and stays live.
    f.vy = -1.;
    let rise = at(&d, &mut f, State::Air, 0);
    assert!(rise.hand_f[1] > rise.head[1] && rise.hand_b[1] > rise.head[1]);
    assert!(rise.foot_f[1] > 0. && rise.foot_b[1] > 0.);
    f.vy = 1.;
    let fall = fighter_pose(&d, &f, 0);
    let sway = fighter_pose(&d, &f, 27);
    assert_ne!(fall.hand_f, sway.hand_f);
    assert!(fall.foot_f[0] - fall.foot_b[0] > rise.foot_f[0] - rise.foot_b[0]);
    // The launch trails the legs down; by the fall they are drawn up ready for a kick.
    assert!(fall.foot_f[1] > rise.foot_f[1] && fall.foot_b[1] > rise.foot_b[1]);

    // Landing absorbs further than the jump coil, then settles back to standing by the recovery key.
    let absorb = at(&d, &mut f, State::Landing, 0);
    let settle = at(&d, &mut f, State::Landing, 3);
    let recovered = at(&d, &mut f, State::Landing, 9);
    assert!(absorb.head[1] < coil.head[1]);
    assert!(absorb.head[1] < settle.head[1] && settle.head[1] < recovered.head[1]);
    assert_eq!(recovered.head[1], stand.head[1]);

    // A Flip Jump rebound has its own recoil silhouette rather than a rotated air pose.
    let dspec = &d.fighter("whiplash").unwrap().moves.dspec;
    let bounce = dspec.phases.iter().find(|p| p.name == "kickBounce").unwrap();
    assert!(bounce.hitboxes.is_empty());
    assert_eq!(bounce.anim.first().unwrap().pose, sim::data::visual::PoseRef::Named("flipBounce".into()));
}

#[test]
fn king_super_jump_windup_is_non_offensive_and_hands_over_to_the_leap() {
    let d = common::data();
    let uspec = &d.fighter("king").unwrap().moves.uspec;
    let squat = uspec.phases.iter().find(|p| p.name == "squat").unwrap();
    let launch = uspec.phases.iter().find(|p| p.name == "launch").unwrap();
    assert!(squat.hitboxes.is_empty() && launch.hitboxes.is_empty(), "the wind-up and takeoff are not attacks");
    assert_eq!(squat.anim.last().unwrap().pose, sim::data::visual::PoseRef::Named("uspecWindHold".into()));
    assert_eq!(launch.anim.first().unwrap().pose, sim::data::visual::PoseRef::Named("uspecWindHold".into()));

    // The wind-up leans the shoulder back while both feet stay planted; the leap then reaches above standing.
    let king = d.fighter("king").unwrap();
    let wind = sim::pose::resolve(king, &king.poses["uspecWind"]);
    let stand = sim::pose::resolve(king, &king.poses["stand"]);
    assert!(wind.shoulder[0] < stand.shoulder[0] && wind.head[1] < stand.head[1]);
    assert_eq!((wind.foot_f[1], wind.foot_b[1]), (0., 0.));
    let leap = sim::pose::resolve(king, &king.poses["win"]);
    assert!(leap.head[1] > stand.head[1]);
}

#[test]
fn golem_coils_keep_transformed_support_feet_on_the_floor() {
    let d = common::data();
    let mut w = common::fighters(&d, "golem", &["brawler"]);
    let mut f = w.fighters[0].clone();
    sim::moves::start_move(&mut w, &d, 0, &mut f, "ftilt");
    for facing in [-1., 1.] {
        f.face = facing;
        for t in 0..8 {
            f.mov.as_mut().unwrap().t = t;
            let p = fighter_pose(&d, &f, 0);
            let tr = sim::pose::BodyTransform::new(&p, [0., 0.], facing);
            for foot in [p.foot_f, p.foot_b] {
                assert!(tr.local_to_world(foot)[1].abs() < 0.8, "coil tick {t} loses the floor");
            }
            if t == 5 {
                for (foot, x) in [(p.foot_f, 16.), (p.foot_b, -18.)] {
                    let world = tr.local_to_world(foot);
                    assert!((world[0] - facing * x).abs() < 1e-5);
                    assert!(world[1].abs() < 1e-5);
                }
            }
        }
    }
}

#[test]
fn dancer_spins_keep_the_authored_rotation_at_each_contact_tick() {
    let d = common::data();
    for (slot, first, end, rate) in [("nair", 4, 16, 45.), ("uspec", 4, 18, 720. / 19.), ("sspec", 5, 20, 18.)] {
        let mut w = common::fighters(&d, "dancer", &["brawler"]);
        let mut f = w.fighters[0].clone();
        sim::moves::start_move(&mut w, &d, 0, &mut f, slot);
        for t in first..=end {
            f.mov.as_mut().unwrap().t = t;
            let p = fighter_pose(&d, &f, 0);
            assert!((p.rot - f64::from(t) * rate).abs() < 1e-8, "{slot} contact tick {t}");
            let pivot_end = if slot == "sspec" {
                20.
            } else if slot == "nair" {
                16.
            } else {
                19.
            };
            assert!((p.pivot - (38. + 7. * f64::from(t) / pivot_end)).abs() < 1e-8);
        }
    }
}

#[test]
fn whiplash_locomotion_does_not_replace_inherited_attack_bookends() {
    let d = common::data();
    let fd = d.fighter("whiplash").unwrap();
    let air = sim::pose::resolve(fd, &fd.poses["air"]);
    assert_eq!(air.hand_f, [18., 56.]);
    assert_eq!(air.foot_f, [6., 8.]);
    let crouch = sim::pose::resolve(fd, &fd.poses["crouch"]);
    assert_eq!(crouch.head, [6., 50.]);
    assert_ne!(air, sim::pose::resolve(fd, &fd.poses["locomotionAir"]));
    // Independent contact-window bookends: the lash remains root anchored while the hand recovers.
    for (slot, tick, hand) in [
        ("nair", 12, [42. - 24. * 12. / 17., 45. + 11. * 12. / 17.]),
        ("zair", 12, [42. - 24. * 12. / 14., 45. + 11. * 12. / 14.]),
        ("bair", 5, [25., 59.]),
    ] {
        let mut w = common::fighters(&d, "whiplash", &["brawler"]);
        let mut f = w.fighters[0].clone();
        sim::moves::start_move(&mut w, &d, 0, &mut f, slot);
        let m = f.mov.as_mut().unwrap();
        m.phase = 1;
        m.t = tick;
        let p = fighter_pose(&d, &f, 0);
        for (actual, expected) in p.hand_f.into_iter().zip(hand) {
            assert!((actual - expected).abs() < 1e-8, "{slot} hand during active window");
        }
    }
}
