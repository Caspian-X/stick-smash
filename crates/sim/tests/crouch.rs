mod common;
use sim::{
    GameData, Input, World,
    input::{ATTACK, JUMP},
    world::{Gait, State},
};

fn world(d: &GameData) -> World {
    let mut w = common::fighters(d, "mike", &["brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[1].x = 320.;
    w
}
fn tick(w: &mut World, d: &GameData, x: i8, y: i8, buttons: u8) {
    common::step_draining(w, d, &[Input { stick: (x, y), buttons, ..Default::default() }]);
}

#[test]
fn holding_down_crouches_and_releasing_rises_without_locking_actions() {
    let d = common::data();
    let mut w = world(&d);
    let frames = d.global.gait.crouch_frames;
    for _ in 0..frames + 10 {
        tick(&mut w, &d, 0, -127, 0);
    }
    assert_eq!((w.fighters[0].state, w.fighters[0].gait), (State::Idle, Gait::Crouch));
    tick(&mut w, &d, 0, 0, 0);
    assert_eq!((w.fighters[0].gait, w.fighters[0].gait_t), (Gait::Rise, 1));
    tick(&mut w, &d, 0, -127, 0);
    assert_eq!((w.fighters[0].gait, w.fighters[0].gait_t), (Gait::Crouch, frames));
    for _ in 0..frames {
        tick(&mut w, &d, 0, 0, 0);
    }
    assert_eq!(w.fighters[0].gait, Gait::Stand);

    for buttons in [ATTACK, JUMP] {
        let mut w = world(&d);
        for _ in 0..10 {
            tick(&mut w, &d, 0, -127, 0);
        }
        tick(&mut w, &d, 0, -127, buttons);
        assert_ne!(w.fighters[0].state, State::Idle);
        assert_eq!(w.fighters[0].gait, Gait::Stand);
        if buttons == ATTACK {
            assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "dtilt");
        }
    }
    let mut w = world(&d);
    tick(&mut w, &d, 0, -127, 0);
    tick(&mut w, &d, 127, -127, 0);
    assert_eq!(w.fighters[0].gait, Gait::Crouch);
    assert!(w.fighters[0].vx > 0.);
}

#[test]
fn slow_down_on_a_platform_crouches_and_a_flick_still_drops() {
    let d = common::data();
    let mut w = world(&d);
    let stage = &d.stages[w.stage];
    let p = &stage.platforms[0];
    let f = &mut w.fighters[0];
    f.x = (p.x1 + p.x2) / 2.;
    f.y = p.y;
    f.surf = Some(stage.blocks.len());
    for y in [-20, -45, -70, -95, -127] {
        tick(&mut w, &d, 0, y, 0);
    }
    assert!(w.fighters[0].ground);
    assert_eq!(w.fighters[0].gait, Gait::Crouch);
    tick(&mut w, &d, 0, 0, 0);
    tick(&mut w, &d, 0, -127, 0);
    assert!(!w.fighters[0].ground);
    assert_eq!(w.fighters[0].state, State::Air);
}

#[test]
fn crouched_motion_is_slow_analog_reversible_and_keeps_actions_available() {
    let d = common::data();
    for id in ["whiplash", "king", "mike"] {
        let mut w = common::fighters(&d, id, &["brawler"]);
        w.fighters[0].x = 640.;
        w.fighters[1].x = 2000.;
        for _ in 0..10 {
            tick(&mut w, &d, 0, -127, 0);
        }
        let x = w.fighters[0].x;
        for _ in 0..10 {
            tick(&mut w, &d, 127, -127, 0);
        }
        let f = &w.fighters[0];
        assert_eq!(f.gait, Gait::Crouch);
        assert!(f.x > x && f.vx < sim::gait::walk_speed(&d, f));
        let mut restored = w.clone();
        tick(&mut w, &d, -80, -127, 0);
        tick(&mut restored, &d, -80, -127, 0);
        assert_eq!(format!("{w:?}"), format!("{restored:?}"));
        assert!(w.fighters[0].vx < 0.);
        tick(&mut w, &d, -80, -127, JUMP);
        assert_eq!(w.fighters[0].state, State::Jumpsquat);
    }
}
