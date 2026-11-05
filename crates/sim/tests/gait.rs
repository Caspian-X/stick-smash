//! Per-fighter ground gait: run turnaround, run brake and dash-back turn windows (`stats.turnFrames`,
//! `brakeFrames`, `dashTurnFrames`, falling back to `global.gait`).
mod common;
use sim::{
    GameData, Input, World,
    actions::Action,
    input::{ATTACK, JUMP},
    world::{Gait, State},
};

fn inp(x: i8, buttons: u8) -> Input {
    Input { stick: (x, 0), buttons, ..Default::default() }
}
fn tick(w: &mut World, d: &GameData, i: Input) -> Vec<Action> {
    common::step_preserving(w, d, &[i]);
    w.fx.clear();
    w.hits.clear();
    std::mem::take(&mut w.acts).into_iter().filter(|a| a.player == 0).map(|a| a.action).collect()
}
fn stand(d: &GameData, id: &str) -> World {
    let mut w = common::fighters(d, id, &["brawler"]);
    w.count = 0;
    (w.fighters[0].x, w.fighters[1].x, w.fighters[0].face) = (340., 2000., 1.);
    w
}
/// Player 0 at full run to the right.
fn running(d: &GameData, id: &str) -> World {
    let mut w = stand(d, id);
    for _ in 0..40 {
        tick(&mut w, d, inp(127, 0));
    }
    assert_eq!(w.fighters[0].gait, Gait::Run, "{id}");
    w
}
fn frames(d: &GameData, id: &str) -> (u32, u32, u32) {
    let (s, g) = (&d.fighter(id).unwrap().stats, &d.global.gait);
    (
        s.turn_frames.unwrap_or(g.turn_frames),
        s.brake_frames.unwrap_or(g.brake_frames),
        s.dash_turn_frames.unwrap_or(g.dash_turn_frames),
    )
}
fn dash_frames(d: &GameData, id: &str) -> u32 {
    d.fighter(id).unwrap().stats.dash_frames.unwrap_or(d.global.gait.dash_frames)
}
fn dash_speed(d: &GameData, id: &str) -> f64 {
    let s = &d.fighter(id).unwrap().stats;
    s.dash_speed.unwrap_or(s.run_speed * d.global.gait.dash_speed)
}
/// A one-tick flick right from standing at x 340, then `then` until standing still: the distance travelled.
fn tap_dash(d: &GameData, id: &str) -> f64 {
    let mut w = stand(d, id);
    tick(&mut w, d, inp(127, 0));
    for _ in 0..200 {
        if w.fighters[0].gait == Gait::Stand && w.fighters[0].vx == 0. {
            break;
        }
        tick(&mut w, d, Input::default());
    }
    w.fighters[0].x - 340.
}

#[test]
fn imports_carry_source_gait_frames() {
    let d = common::data();
    // The frames before the 1-indexed Dash / TurnDash / TurnRun / RunBrakeR markers (scripts.json); the dash
    // frames match SmashWiki's Ultimate table (Peach 15, King Dedede 10, Cloud 10).
    for (id, dash, turn, brake, dash_turn) in
        [("duchess", 15, 10, 10, 2), ("king", 10, 16, 10, 2), ("stratus", 10, 14, 10, 2)]
    {
        assert_eq!(frames(&d, id), (turn, brake, dash_turn), "{id}");
        assert_eq!(dash_frames(&d, id), dash, "{id}");
    }
}

#[test]
fn run_turnaround_flips_facing_slides_on_traction_then_runs_back() {
    let d = common::data();
    for id in ["duchess", "king", "stratus", "brawler"] {
        let (turn, ..) = frames(&d, id);
        let s = &d.fighter(id).unwrap().stats;
        let mut w = running(&d, id);
        assert!(tick(&mut w, &d, inp(-127, 0)).contains(&Action::Turn), "{id}");
        assert_eq!((w.fighters[0].gait, w.fighters[0].face), (Gait::Turn, -1.), "{id}");
        let mut vx = w.fighters[0].vx;
        assert!(vx > 0., "{id}: speed carries the old way");
        for n in 1..turn {
            tick(&mut w, &d, inp(-127, 0));
            assert_eq!(w.fighters[0].gait, Gait::Turn, "{id}: tick {n}");
            assert!(w.fighters[0].vx <= vx && w.fighters[0].vx >= 0., "{id}: slides under traction");
            vx = w.fighters[0].vx;
        }
        tick(&mut w, &d, inp(-127, 0));
        assert_eq!(w.fighters[0].gait, Gait::Run, "{id}: runs back after turnFrames");
        let full = (1..120).find(|_| {
            tick(&mut w, &d, inp(-127, 0));
            w.fighters[0].vx <= -0.95 * s.run_speed
        });
        assert!(full.is_some(), "{id}: reaches run speed the other way");
    }
}

#[test]
fn run_turnaround_is_cancellable() {
    let d = common::data();
    for (raw, state) in [(inp(-127, JUMP), State::Jumpsquat), (inp(0, ATTACK), State::Attack)] {
        let mut w = running(&d, "duchess");
        for _ in 0..3 {
            tick(&mut w, &d, inp(-127, 0));
        }
        assert_eq!(w.fighters[0].gait, Gait::Turn);
        tick(&mut w, &d, raw);
        assert_eq!(w.fighters[0].state, state);
    }
}

#[test]
fn run_brake_lasts_brake_frames_and_dashes_out() {
    let d = common::data();
    for id in ["duchess", "king", "brawler"] {
        let (_, brake, _) = frames(&d, id);
        let mut w = running(&d, id);
        tick(&mut w, &d, Input::default());
        assert_eq!(w.fighters[0].gait, Gait::Brake, "{id}");
        for _ in 1..brake {
            tick(&mut w, &d, Input::default());
            assert_eq!(w.fighters[0].gait, Gait::Brake, "{id}");
        }
        tick(&mut w, &d, Input::default());
        assert_eq!(w.fighters[0].gait, Gait::Stand, "{id}: stands after brakeFrames");
    }
    // Inside the brake a forward flick dashes out; holding back turns the run around (Ultimate's TURN_RUN window).
    let s = &d.fighter("duchess").unwrap().stats;
    let braking = || {
        let mut w = running(&d, "duchess");
        for _ in 0..3 {
            tick(&mut w, &d, Input::default());
        }
        assert_eq!(w.fighters[0].gait, Gait::Brake);
        w
    };
    let mut w = braking();
    assert!(tick(&mut w, &d, inp(127, 0)).contains(&Action::Dash));
    assert_eq!((w.fighters[0].gait, w.fighters[0].face), (Gait::Dash, 1.));
    assert_eq!(w.fighters[0].vx, s.dash_speed.unwrap());
    let mut w = braking();
    assert!(tick(&mut w, &d, inp(-127, 0)).contains(&Action::Turn));
    assert_eq!((w.fighters[0].gait, w.fighters[0].face), (Gait::Turn, -1.));
}

#[test]
fn dash_back_flips_speed_at_once_inside_a_turn_window() {
    let d = common::data();
    for id in ["duchess", "brawler"] {
        let (.., window) = frames(&d, id);
        let s = &d.fighter(id).unwrap().stats;
        let dash_speed = s.dash_speed.unwrap_or(s.run_speed * d.global.gait.dash_speed);
        let dash_frames = s.dash_frames.unwrap_or(d.global.gait.dash_frames);
        let mut w = stand(&d, id);
        w.fighters[0].x = 640.;
        for _ in 0..3 {
            tick(&mut w, &d, inp(127, 0));
        }
        assert_eq!(w.fighters[0].gait, Gait::Dash);
        assert!(tick(&mut w, &d, inp(-127, 0)).contains(&Action::Dash), "{id}");
        assert_eq!((w.fighters[0].gait, w.fighters[0].face), (Gait::DashTurn, -1.), "{id}");
        assert_eq!(w.fighters[0].vx, -dash_speed, "{id}: dash-back speed is instant");
        // The turn pose: a backward lean that eases out (no `turn` clip on these figures).
        let mut plain = w.fighters[0].clone();
        plain.gait = Gait::Dash;
        let lean =
            sim::pose::fighter_pose(&d, &w.fighters[0], w.frame).rot - sim::pose::fighter_pose(&d, &plain, w.frame).rot;
        assert!((lean + d.global.gait.dash_turn_lean).abs() < 1e-9, "{id}: lean {lean}");
        for _ in 1..window {
            tick(&mut w, &d, inp(-127, 0));
            assert_eq!(w.fighters[0].gait, Gait::DashTurn, "{id}");
        }
        tick(&mut w, &d, inp(-127, 0));
        assert_eq!(w.fighters[0].gait, Gait::Dash, "{id}");
        // The window is part of the dash: it still lasts dashFrames.
        for _ in window + 1..dash_frames {
            tick(&mut w, &d, inp(-127, 0));
            assert_eq!(w.fighters[0].gait, Gait::Dash, "{id}");
        }
        tick(&mut w, &d, inp(-127, 0));
        assert_eq!(w.fighters[0].gait, Gait::Run, "{id}");
    }
}

#[test]
fn gait_frames_fall_back_to_global_and_zero_window_is_a_plain_dash() {
    let mut d = common::data();
    let brawler = d.fighters.iter().position(|f| f.id == "brawler").unwrap();
    let s = &d.fighters[brawler].stats;
    assert_eq!((s.turn_frames, s.brake_frames, s.dash_turn_frames), (None, None, None));
    let g = &d.global.gait;
    assert_eq!(frames(&d, "brawler"), (g.turn_frames, g.brake_frames, g.dash_turn_frames));
    d.global.gait.dash_turn_frames = 0;
    let mut w = stand(&d, "brawler");
    w.fighters[0].x = 640.;
    tick(&mut w, &d, inp(127, 0));
    tick(&mut w, &d, inp(-127, 0));
    assert_eq!((w.fighters[0].gait, w.fighters[0].face), (Gait::Dash, -1.));
    // A per-fighter value overrides the global one.
    d.fighters[brawler].stats.turn_frames = Some(9);
    let mut w = running(&d, "brawler");
    tick(&mut w, &d, inp(-127, 0));
    for _ in 1..9 {
        tick(&mut w, &d, inp(-127, 0));
        assert_eq!(w.fighters[0].gait, Gait::Turn);
    }
    tick(&mut w, &d, inp(-127, 0));
    assert_eq!(w.fighters[0].gait, Gait::Run);
}

#[test]
fn tap_dash_brakes_at_once_and_travels_far_less_than_a_held_dash() {
    let d = common::data();
    assert!(d.global.gait.dash_release);
    for id in ["duchess", "king", "stratus", "brawler"] {
        let (n, top) = (dash_frames(&d, id), dash_speed(&d, id));
        let mut w = stand(&d, id);
        assert!(tick(&mut w, &d, inp(127, 0)).contains(&Action::Dash), "{id}");
        assert_eq!(w.fighters[0].vx, top, "{id}");
        // Released: still the dash gait, but skidding by traction from the first neutral tick.
        let mut vx = top;
        for t in 1..n {
            tick(&mut w, &d, Input::default());
            assert_eq!(w.fighters[0].gait, Gait::Dash, "{id}: tick {t}");
            assert!(w.fighters[0].vx < vx || w.fighters[0].vx == 0., "{id}: tick {t} skids");
            vx = w.fighters[0].vx;
        }
        let (dashed, hold) = (w.fighters[0].x - 340., top * f64::from(n));
        assert!(dashed < 0.7 * hold, "{id}: released dash {dashed} vs held dash {hold}");
        tick(&mut w, &d, Input::default());
        assert_eq!(w.fighters[0].gait, Gait::Brake, "{id}: brakes when the dash frames end");
        let tap = tap_dash(&d, id);
        assert!(tap > top && tap < hold, "{id}: tap {tap} to rest vs held dash {hold}");
    }
}

#[test]
fn re_holding_a_released_dash_accelerates_back_to_dash_speed() {
    let d = common::data();
    for id in ["duchess", "brawler"] {
        let (n, top, acc) = (dash_frames(&d, id), dash_speed(&d, id), d.fighter(id).unwrap().stats.run_accel);
        let mut w = stand(&d, id);
        tick(&mut w, &d, inp(127, 0));
        for _ in 0..3 {
            tick(&mut w, &d, Input::default());
        }
        let mut vx = w.fighters[0].vx;
        assert!(vx < top, "{id}");
        for t in 4..n {
            tick(&mut w, &d, inp(127, 0));
            assert_eq!(w.fighters[0].gait, Gait::Dash, "{id}");
            assert_eq!(w.fighters[0].vx, (vx + acc).min(top), "{id}: tick {t} accelerates by runAccel");
            vx = w.fighters[0].vx;
        }
        tick(&mut w, &d, inp(127, 0));
        assert_eq!(w.fighters[0].gait, Gait::Run, "{id}: runs when the dash frames end");
    }
}

#[test]
fn dash_back_during_a_released_dash_still_flips() {
    let d = common::data();
    for id in ["duchess", "brawler"] {
        let mut w = stand(&d, id);
        w.fighters[0].x = 640.;
        tick(&mut w, &d, inp(127, 0));
        tick(&mut w, &d, Input::default());
        tick(&mut w, &d, Input::default());
        assert!(w.fighters[0].vx < dash_speed(&d, id), "{id}");
        assert!(tick(&mut w, &d, inp(-127, 0)).contains(&Action::Dash), "{id}");
        assert_eq!((w.fighters[0].gait, w.fighters[0].face), (Gait::DashTurn, -1.), "{id}");
        assert_eq!(w.fighters[0].vx, -dash_speed(&d, id), "{id}");
    }
}

#[test]
fn without_dash_release_a_tap_keeps_dash_speed_for_the_whole_dash() {
    let mut d = common::data();
    d.global.gait.dash_release = false;
    for id in ["duchess", "brawler"] {
        let (n, top) = (dash_frames(&d, id), dash_speed(&d, id));
        let mut w = stand(&d, id);
        tick(&mut w, &d, inp(127, 0));
        for _ in 1..n {
            tick(&mut w, &d, Input::default());
            assert_eq!(w.fighters[0].vx, top, "{id}");
        }
        assert!((w.fighters[0].x - 340. - top * f64::from(n)).abs() < 1e-9, "{id}");
        tick(&mut w, &d, Input::default());
        assert_eq!(w.fighters[0].gait, Gait::Brake, "{id}");
        let mut on = common::data();
        on.global.gait.dash_release = true;
        assert!(tap_dash(&d, id) > tap_dash(&on, id) + top, "{id}");
    }
}
