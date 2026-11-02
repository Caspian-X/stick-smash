mod common;
use sim::{
    GameData, Input, MatchConfig, PlayerConfig, World,
    input::{Buffer, Intent},
    world::State,
};

fn landing(d: &GameData, frames: u32) -> World {
    let mut w = common::fighters(d, "brawler", &[]);
    w.fighters[0].set_state(State::Landing);
    w.fighters[0].sd.n = f64::from(frames);
    w
}
fn tick(w: &mut World, d: &GameData, input: Input) {
    common::step_preserving(w, d, &[input]);
}
fn neutral(w: &mut World, d: &GameData, frames: u32) {
    common::run_preserving(w, d, frames as usize, &[Input::default()]);
}
fn slot(w: &World) -> Option<&str> {
    w.fighters[0].mov.as_ref().map(|m| m.slot.as_str())
}
#[test]
fn attack_waits_for_first_actionable_update_and_zero_disables_it() {
    for frames in [0, 5] {
        let mut d = common::data();
        d.global.input.buffer_frames = frames;
        for remaining in 2..=4 {
            let mut w = landing(&d, remaining);
            tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
            neutral(&mut w, &d, remaining - 1);
            assert_eq!(w.fighters[0].state, State::Idle);
            tick(&mut w, &d, Input::default());
            assert_eq!(slot(&w), if frames == 0 { None } else { Some("jab") });
        }
    }
}
#[test]
fn smash_keeps_press_time_tap_and_direction() {
    let d = common::data();
    let mut w = landing(&d, 4);
    tick(&mut w, &d, Input { stick: (127, 0), buttons: 2, ..Default::default() });
    neutral(&mut w, &d, 4);
    assert_eq!(slot(&w), Some("fsmash"));
    assert_eq!(w.fighters[0].face, 1.);
}
#[test]
fn expired_press_does_nothing() {
    let d = common::data();
    let mut w = landing(&d, 6);
    tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    neutral(&mut w, &d, 7);
    assert_eq!(w.fighters[0].state, State::Idle);
    assert!(w.fighters[0].buffer.cmd.is_none());
}
#[test]
fn lifetime_is_inclusive_and_a_press_is_used_once() {
    let d = common::data();
    let mut w = landing(&d, 5);
    tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    let mut jabs = 0;
    let mut was_jab = false;
    for _ in 0..100 {
        tick(&mut w, &d, Input::default());
        let jab = slot(&w) == Some("jab");
        jabs += u32::from(jab && !was_jab);
        was_jab = jab;
    }
    assert_eq!(jabs, 1);
    assert_eq!(w.fighters[0].state, State::Idle);
}
#[test]
fn buffered_jump_uses_hold_at_takeoff() {
    let d = common::data();
    for held in [false, true] {
        let mut w = landing(&d, 3);
        let jump = Input { buttons: 1, ..Default::default() };
        tick(&mut w, &d, jump);
        for _ in 0..3 {
            tick(&mut w, &d, jump);
        }
        assert_eq!(w.fighters[0].state, State::Jumpsquat);
        let input = if held { jump } else { Input::default() };
        for _ in 0..d.global.movement.jumpsquat {
            tick(&mut w, &d, input);
        }
        let f = &w.fighters[0];
        let stats = &d.fighters[f.data].stats;
        assert_eq!(f.state, State::Air);
        assert_eq!(f.vy, if held { -stats.jump_speed } else { -stats.short_hop_speed });
    }
}
#[test]
fn rollback_preserves_live_buffer_and_replay() {
    let d = common::data();
    let mut w = landing(&d, 4);
    tick(&mut w, &d, Input { stick: (-127, 0), buttons: 2, ..Default::default() });
    assert!(w.fighters[0].buffer.cmd.is_some());
    let mut copy = w.clone();
    for _ in 0..60 {
        tick(&mut w, &d, Input::default());
        tick(&mut copy, &d, Input::default());
        assert_eq!(w, copy);
    }
}
fn rules(frames: u32) -> sim::data::global::InputRules {
    let mut r = common::data().global.input;
    r.buffer_frames = frames;
    r.combo_frames = 2;
    r
}
#[test]
fn fresh_presses_supersede_and_replay_preserves_current_holds() {
    let r = rules(5);
    let mut b = Buffer::default();
    b.deliver(&mut Intent { atk: true, left: true, tap_l: true, ..Default::default() }, 1, &r, false, false);
    // A fresh unrelated press (outside the combo window) wins and drops the old command.
    let mut fresh = Intent { grab: true, right: true, ..Default::default() };
    assert_eq!(b.deliver(&mut fresh, 4, &r, true, false), None);
    assert!(fresh.grab && fresh.right && !fresh.atk);
    assert!(b.cmd.is_none());
    b.deliver(&mut Intent { atk: true, left: true, tap_l: true, ..Default::default() }, 5, &r, false, false);
    let mut held = Intent { jump_held: true, sp_held: true, shield: true, x: 0.5, ..Default::default() };
    assert_eq!(b.deliver(&mut held, 6, &r, true, false), Some(5));
    assert!(held.atk && held.left && held.tap_l && held.jump_held && held.sp_held && held.shield);
    assert!(!held.atk_held && held.x == 0.5, "holds and analog movement stay live");
    assert!(b.cmd.is_some(), "kept until an action accepts it");
    b.accept();
    assert!(b.cmd.is_none());
}
#[test]
fn adjacent_buffered_presses_combine_into_one_command() {
    let r = rules(5);
    for (first, second, check) in [
        (Intent { jump: true, ..Default::default() }, Intent { atk: true, ..Default::default() }, 0),
        (Intent { atk: true, ..Default::default() }, Intent { jump: true, ..Default::default() }, 0),
        (Intent { jump: true, ..Default::default() }, Intent { jump: true, ..Default::default() }, 1),
        (Intent { atk: true, ..Default::default() }, Intent { sp: true, ..Default::default() }, 2),
    ] {
        let mut b = Buffer::default();
        b.deliver(&mut first.clone(), 1, &r, false, true);
        b.deliver(&mut second.clone(), 3, &r, false, true);
        let c = b.cmd.unwrap().intent;
        match check {
            0 => assert!(c.jump && c.atk && c.jump_attack),
            1 => assert!(c.jump && c.short_hop),
            _ => assert!(c.atk && !c.sp && c.strike == sim::input::Strike::Combo),
        }
    }
    // Too far apart: the later press replaces the earlier one.
    let mut b = Buffer::default();
    b.deliver(&mut Intent { jump: true, ..Default::default() }, 1, &r, false, true);
    b.deliver(&mut Intent { atk: true, ..Default::default() }, 4, &r, false, true);
    let c = b.cmd.unwrap().intent;
    assert!(c.atk && !c.jump && !c.jump_attack);
    // Attack + special combine only with the option on.
    let mut b = Buffer::default();
    b.deliver(&mut Intent { atk: true, ..Default::default() }, 1, &r, false, false);
    b.deliver(&mut Intent { sp: true, ..Default::default() }, 2, &r, false, false);
    assert!(b.cmd.unwrap().intent.sp);
    // A buffered press combines with the first fresh press once actionable.
    let mut b = Buffer::default();
    b.deliver(&mut Intent { jump: true, ..Default::default() }, 1, &r, false, false);
    let mut fresh = Intent { atk: true, ..Default::default() };
    b.deliver(&mut fresh, 2, &r, true, false);
    assert!(fresh.jump_attack && fresh.jump && fresh.atk && b.cmd.is_none());
}
#[test]
fn simultaneous_buffered_presses_act_once() {
    let d = common::data();
    let mut w = landing(&d, 3);
    tick(&mut w, &d, Input { buttons: 2 | 4 | 8, ..Default::default() });
    let mut starts = 0;
    let mut was = None::<String>;
    for _ in 0..90 {
        tick(&mut w, &d, Input::default());
        let s = slot(&w).map(str::to_string);
        starts += u32::from(s.is_some() && s != was);
        was = s;
        w.fx.clear();
    }
    assert_eq!(starts, 1, "one buffered command, one action");
}
#[test]
fn fresh_actionable_press_is_not_queued() {
    let d = common::data();
    let mut w = landing(&d, 0);
    w.fighters[0].set_state(State::Idle);
    tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    assert_eq!(slot(&w), Some("jab"));
    assert!(w.fighters[0].buffer.cmd.is_none());
}
#[test]
fn hitlag_delays_delivery_and_cpus_do_not_buffer_raw_input() {
    let d = common::data();
    let mut w = landing(&d, 0);
    w.fighters[0].set_state(State::Idle);
    w.fighters[0].lag = 2;
    tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    tick(&mut w, &d, Input::default());
    assert_eq!(w.fighters[0].state, State::Idle);
    tick(&mut w, &d, Input::default());
    assert_eq!(slot(&w), Some("jab"));
    let mut cpu = landing(&d, 4);
    cpu.cfg.players[0].cpu = Some(0);
    tick(&mut cpu, &d, Input { buttons: 31, ..Default::default() });
    assert!(cpu.fighters[0].buffer.cmd.is_none());
}
#[test]
fn tap_jump_defaults_on() {
    assert!(PlayerConfig::human("brawler").tap_jump);
}
#[test]
fn replay_matches_ledge_knockdown_and_shield_stun_boundaries() {
    let d = common::data();
    for state in [State::Ledge, State::Knockdown, State::Shield] {
        let mut w = landing(&d, 0);
        let f = &mut w.fighters[0];
        f.set_state(state);
        match state {
            State::Ledge => {
                f.t = d.global.ledge.min_hang - 2;
                f.sd.side = -1.;
                f.ground = false;
                f.surf = None;
            }
            State::Knockdown => f.t = d.global.knockdown.min_frames - 1,
            State::Shield => f.sd.stun = 1,
            _ => unreachable!(),
        }
        tick(&mut w, &d, Input { buttons: 2 | 16, ..Default::default() });
        assert_eq!(w.fighters[0].state, state);
        tick(&mut w, &d, Input { buttons: 16, ..Default::default() });
        let f = &w.fighters[0];
        match state {
            State::Ledge => {
                assert_eq!(f.state, State::Climb);
                assert_eq!(f.sd.kind, "attack");
            }
            State::Knockdown => {
                assert_eq!(f.state, State::Getup);
                assert_eq!(f.sd.kind, "atk");
            }
            State::Shield => assert_eq!(slot(&w), Some("grab")),
            _ => unreachable!(),
        }
    }
}
#[test]
fn buffered_special_and_back_air_keep_press_time_direction() {
    let d = common::data();
    let mut w = World::new(
        MatchConfig::sandbox("arena", vec![PlayerConfig::human("swordfighter")], MatchConfig::default().seed),
        &d,
    )
    .unwrap();
    w.fighters[0].set_state(State::Landing);
    w.fighters[0].sd.n = 3.;
    tick(&mut w, &d, Input { stick: (-127, 0), buttons: 4, ..Default::default() });
    neutral(&mut w, &d, 3);
    assert_eq!(slot(&w), Some("sspec"));
    assert_eq!(w.fighters[0].face, -1.);

    let mut w = landing(&d, 0);
    let f = &mut w.fighters[0];
    f.set_state(State::Air);
    f.ground = false;
    f.surf = None;
    f.y -= 100.;
    f.face = 1.;
    f.lag = 2;
    tick(&mut w, &d, Input { stick: (-127, 0), buttons: 2, ..Default::default() });
    neutral(&mut w, &d, 2);
    assert_eq!(slot(&w), Some("bair"));
}
#[test]
fn legacy_jumpsquat_attack_still_becomes_up_smash() {
    let mut d = common::data();
    d.global.input.jump_aerials = false;
    let mut w = landing(&d, 0);
    w.fighters[0].set_state(State::Jumpsquat);
    tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    assert_eq!(slot(&w), Some("usmash"));
}

#[test]
fn buffered_right_stick_keeps_attack_direction_after_release_and_rollback() {
    let d = common::data();
    let mut w = landing(&d, 4);
    tick(&mut w, &d, Input { stick: (127, 0), attack_stick: (0, -127), ..Default::default() });
    assert!(w.fighters[0].buffer.cmd.is_some());
    let mut copy = w.clone();
    for _ in 0..4 {
        tick(&mut w, &d, Input::default());
        tick(&mut copy, &d, Input::default());
        assert_eq!(w, copy);
    }
    assert_eq!(slot(&w), Some("dsmash"));
}

#[test]
fn mike_jump_attack_shortcuts_start_directional_aerials_at_takeoff() {
    let d = common::data();
    for (stick, attack_stick, expected) in [
        ((0, 0), (0, 0), "nair"),
        ((127, 0), (0, 0), "fair"),
        ((-127, 0), (0, 0), "bair"),
        ((0, 127), (0, 0), "uair"),
        ((0, -127), (0, 0), "dair"),
        ((-127, 0), (127, 0), "fair"),
        ((127, 0), (-127, 0), "bair"),
    ] {
        let mut w = common::fighters(&d, "mike", &[]);
        w.fighters[0].face = 1.;
        let raw = Input { stick, attack_stick, buttons: if attack_stick == (0, 0) { 3 } else { 1 } };
        tick(&mut w, &d, raw);
        assert_eq!(w.fighters[0].state, State::Jumpsquat);
        assert_eq!(w.fighters[0].sd.jump_aerial.as_deref(), Some(expected));
        // Keeping jump held still gives the shortcut's short hop; attack direction is remembered.
        for _ in 0..d.global.movement.jumpsquat {
            tick(&mut w, &d, Input { buttons: 1, ..Default::default() });
        }
        let f = &w.fighters[0];
        assert_eq!(slot(&w), Some(expected));
        assert!(!f.ground && f.surf.is_none());
        assert_eq!(f.vy, -d.fighter("mike").unwrap().stats.short_hop_speed);
        assert!(f.buffer.cmd.is_none());
    }
}

#[test]
fn mike_attack_on_every_jumpsquat_frame_waits_for_takeoff_once() {
    let d = common::data();
    for frame in 1..=d.global.movement.jumpsquat {
        for right_stick in [false, true] {
            let mut w = common::fighters(&d, "mike", &[]);
            tick(&mut w, &d, Input { buttons: 1, ..Default::default() });
            for n in 1..=d.global.movement.jumpsquat {
                let mut raw = Input { buttons: 1, ..Default::default() };
                if n == frame {
                    if right_stick {
                        raw.attack_stick = (0, 127);
                    } else {
                        raw.buttons |= 2;
                    }
                }
                tick(&mut w, &d, raw);
                if n < d.global.movement.jumpsquat {
                    assert_eq!(w.fighters[0].state, State::Jumpsquat);
                    assert!(slot(&w).is_none());
                }
            }
            assert_eq!(slot(&w), Some(if right_stick { "uair" } else { "nair" }));
            assert!(w.fighters[0].buffer.cmd.is_none());
            neutral(&mut w, &d, 100);
            assert_eq!(w.fighters[0].state, State::Idle);
        }
    }
}

#[test]
fn buffered_mike_jump_attack_stays_together_through_release_and_rollback() {
    let d = common::data();
    let mut w = common::fighters(&d, "mike", &[]);
    w.fighters[0].set_state(State::Landing);
    w.fighters[0].sd.n = 4.;
    tick(&mut w, &d, Input { stick: (-127, 0), buttons: 3, ..Default::default() });
    assert!(w.fighters[0].buffer.cmd.is_some());
    neutral(&mut w, &d, 4);
    assert_eq!(w.fighters[0].state, State::Jumpsquat);
    assert_eq!(w.fighters[0].sd.jump_aerial.as_deref(), Some("bair"));
    let mut restored: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    for _ in 0..80 {
        tick(&mut w, &d, Input::default());
        tick(&mut restored, &d, Input::default());
        assert_eq!(w, restored);
        assert_ne!(slot(&w), Some("jab"));
        assert_ne!(slot(&w), Some("usmash"));
    }
    assert!(w.fighters[0].buffer.cmd.is_none());
}

#[test]
fn pending_aerial_outlives_press_buffer_and_clears_when_jump_is_interrupted() {
    let mut d = common::data();
    d.global.input.buffer_frames = 0;
    d.global.movement.jumpsquat = 10;
    let mut w = common::fighters(&d, "mike", &[]);
    tick(&mut w, &d, Input { buttons: 3, ..Default::default() });
    let mut interrupted = w.clone();
    interrupted.fighters[0].set_state(State::Hitstun);
    assert!(interrupted.fighters[0].sd.jump_aerial.is_none());
    neutral(&mut w, &d, 10);
    assert_eq!(slot(&w), Some("nair"));
}

#[test]
fn tap_jump_attack_keeps_up_smash_and_air_jump_attack_performs_both() {
    let d = common::data();
    let mut w = common::fighters(&d, "mike", &[]);
    tick(&mut w, &d, Input { stick: (0, 127), buttons: 2, ..Default::default() });
    assert_eq!(slot(&w), Some("usmash"));
    let mut w = common::fighters(&d, "mike", &[]);
    let f = &mut w.fighters[0];
    f.set_state(State::Air);
    f.ground = false;
    f.surf = None;
    f.y -= 100.;
    let jumps = f.jumps;
    tick(&mut w, &d, Input { buttons: 3, ..Default::default() });
    assert_eq!(slot(&w), Some("nair"));
    assert_eq!(w.fighters[0].jumps, jumps - 1);
    assert!(w.fighters[0].vy < 0.);
}
