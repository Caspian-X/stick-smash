mod common;
use sim::{
    GameData, Input, MatchConfig, PlayerConfig, World,
    input::{Buffer, Intent},
    step_world,
    world::State,
};

fn landing(d: &GameData, frames: u32) -> World {
    let mut w = World::new(MatchConfig { training: true, players: vec![PlayerConfig::human("brawler")], ..Default::default() }, d);
    w.fighters[0].set_state(State::Landing);
    w.fighters[0].sd.n = f64::from(frames);
    w
}
fn tick(w: &mut World, d: &GameData, input: Input) {
    step_world(w, d, &[input]);
}
fn neutral(w: &mut World, d: &GameData, frames: u32) {
    for _ in 0..frames {
        tick(w, d, Input::default());
    }
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
    tick(&mut w, &d, Input { stick: (127, 0), buttons: 2 });
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
    assert!(w.fighters[0].buffer.entries.is_empty());
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
    tick(&mut w, &d, Input { stick: (-127, 0), buttons: 2 });
    assert_eq!(w.fighters[0].buffer.entries.len(), 1);
    let mut copy = w.clone();
    for _ in 0..60 {
        tick(&mut w, &d, Input::default());
        tick(&mut copy, &d, Input::default());
        assert_eq!(w, copy);
    }
}
#[test]
fn fresh_presses_win_and_replay_preserves_current_holds() {
    let mut b = Buffer::default();
    b.deliver(&mut Intent { atk: true, left: true, tap_l: true, ..Default::default() }, 1, 5, false);
    let mut fresh = Intent { jump: true, right: true, ..Default::default() };
    b.deliver(&mut fresh, 2, 5, true);
    assert!(fresh.jump && fresh.right && !fresh.atk);
    assert_eq!(b.entries.len(), 1);
    let mut held = Intent { jump_held: true, sp_held: true, shield: true, ..Default::default() };
    b.deliver(&mut held, 3, 5, true);
    assert!(held.atk && held.left && held.tap_l && held.jump_held && held.sp_held && held.shield);
    assert!(!held.atk_held);
    assert!(b.entries.is_empty());
}
#[test]
fn simultaneous_presses_replay_oldest_first_one_per_tick() {
    let mut b = Buffer::default();
    b.deliver(&mut Intent { atk: true, sp: true, jump: true, ..Default::default() }, 1, 5, false);
    for (tick, kind) in [(2, 0), (3, 1), (4, 2)] {
        let mut inp = Intent::default();
        b.deliver(&mut inp, tick, 5, true);
        assert_eq!([inp.atk, inp.sp, inp.jump], [kind == 0, kind == 1, kind == 2]);
    }
    assert!(b.entries.is_empty());
}
#[test]
fn fresh_actionable_press_is_not_queued() {
    let d = common::data();
    let mut w = landing(&d, 0);
    w.fighters[0].set_state(State::Idle);
    tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    assert_eq!(slot(&w), Some("jab"));
    assert!(w.fighters[0].buffer.entries.is_empty());
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
    assert!(cpu.fighters[0].buffer.entries.is_empty());
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
    let mut w = World::new(MatchConfig { training: true, players: vec![PlayerConfig::human("swordfighter")], ..Default::default() }, &d);
    w.fighters[0].set_state(State::Landing);
    w.fighters[0].sd.n = 3.;
    tick(&mut w, &d, Input { stick: (-127, 0), buttons: 4 });
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
    tick(&mut w, &d, Input { stick: (-127, 0), buttons: 2 });
    neutral(&mut w, &d, 2);
    assert_eq!(slot(&w), Some("bair"));
}
#[test]
fn fresh_jumpsquat_attack_still_becomes_up_smash() {
    let d = common::data();
    let mut w = landing(&d, 0);
    w.fighters[0].set_state(State::Jumpsquat);
    tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    assert_eq!(slot(&w), Some("usmash"));
}
