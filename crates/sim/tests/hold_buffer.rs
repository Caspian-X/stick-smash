//! Jump buffered through the end of an aerial (Duchess full-hop up air), and held presses.
mod common;

use sim::actions::Action;
use sim::input::{ATTACK, Buffer, Intent, JUMP, Strike};
use sim::{GameData, Input, PlayerConfig, World, world::State};

const UP_AIR: Input = Input { stick: (0, 0), buttons: 0, attack_stick: (0, 127) };
const JUMP_IN: Input = Input { stick: (0, 0), buttons: JUMP, attack_stick: (0, 0) };

fn world(d: &GameData) -> World {
    let players = ["duchess", "brawler"].into_iter().map(PlayerConfig::human).collect();
    let mut w = common::sandbox(d, "arena", players);
    w.count = 0;
    // Clear of the side platforms above, so a full hop lands back on the main floor.
    w.fighters[0].x = 290.;
    w.fighters[1].x = 990.;
    common::run_draining(&mut w, d, 120, &[Input::default(), Input::default()]);
    assert!(w.fighters[0].ground && w.fighters[0].state == State::Idle);
    w
}

/// Air jumps (by tick offset from the up air input) and the up air's end tick, for a jump input schedule given
/// by tick offset from the up air input.
struct Run {
    uair_end: usize,
    air_jumps: Vec<usize>,
    landed: usize,
    /// Any command (move, jump, float…) after the air jump, up to 30 ticks after landing.
    after: Vec<Action>,
}

fn run(d: &GameData, jump: impl Fn(usize) -> bool) -> Run {
    let mut w = world(d);
    // Full hop: hold jump through jumpsquat.
    for _ in 0..30 {
        common::step_draining(&mut w, d, &[JUMP_IN, Input::default()]);
        if !w.fighters[0].ground {
            break;
        }
    }
    assert!(!w.fighters[0].ground);
    let (mut uair_end, mut air_jumps, mut landed, mut after) = (0, vec![], 0, vec![]);
    for t in 0..400 {
        let mut input = if t == 0 { UP_AIR } else { Input::default() };
        if jump(t) {
            input.buttons |= JUMP;
        }
        common::step_preserving(&mut w, d, &[input, Input::default()]);
        if t == 0 {
            assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some("uair"));
        }
        if uair_end == 0 && w.fighters[0].mov.is_none() {
            uair_end = t;
        }
        if w.acts.iter().any(|a| a.player == 0 && a.action == Action::AirJump) {
            air_jumps.push(t);
        } else if !air_jumps.is_empty() {
            after.extend(w.acts.iter().filter(|a| a.player == 0 && a.action.command()).map(|a| a.action.clone()));
        }
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        if w.fighters[0].ground && landed == 0 {
            landed = t;
        }
        if landed > 0 && t >= landed + 30 {
            break;
        }
    }
    Run { uair_end, air_jumps, landed, after }
}

fn uair_frames(d: &GameData) -> u32 {
    d.fighter("duchess").unwrap().moves.get("uair").unwrap().phases.iter().filter_map(|p| p.frames).sum()
}

#[test]
fn jump_buffered_through_full_hop_up_air() {
    let d = common::data();
    let probe = run(&d, |_| false);
    // The move ends on its last frame's update; the next tick is the first actionable one.
    let end = probe.uair_end + 1;
    eprintln!(
        "uair total {} frames, first actionable tick {end}; full hop landed {} ticks after the up air input; buffer {} hold {}",
        uair_frames(&d),
        probe.landed,
        d.global.input.buffer_frames,
        d.global.input.hold_buffer
    );
    assert!(probe.air_jumps.is_empty());
    assert!(probe.landed > end, "a full hop outlasts the up air");
    let early = end - 12;
    let cases: [(&str, Box<dyn Fn(usize) -> bool>); 3] = [
        ("tap 12 early", Box::new(move |t| t == early)),
        ("hold from 12 early", Box::new(move |t| t >= early)),
        ("tap in last 5", Box::new(move |t| t == end - 4)),
    ];
    let runs: Vec<_> = cases.into_iter().map(|(name, schedule)| (name, run(&d, schedule).air_jumps)).collect();
    eprintln!("air jumps by case (uair end {end}): {runs:?}");
    for (name, jumps) in runs {
        // Released 12 ticks early, the press expires inside the 9-tick window; held or late, it lands on the end tick.
        assert_eq!(jumps, if name == "tap 12 early" { vec![] } else { vec![end] }, "{name}");
    }
}

/// An accepted press held down never fires again: holding jump after the buffered air jump only floats (no jump
/// on landing), and a held attack in idle is one jab.
#[test]
fn accepted_held_press_does_not_repeat() {
    let d = common::data();
    let end = run(&d, |_| false).uair_end + 1;
    let r = run(&d, |t| t >= end - 12);
    assert_eq!(r.air_jumps, vec![end]);
    assert!(r.landed > 0);
    // Holding jump floats at the air jump's apex (a hold, not a press); nothing else starts, landing included.
    assert_eq!(r.after, vec![Action::Float], "the held jump does nothing more, through landing");
    let mut w = world(&d);
    let hold = Input { buttons: ATTACK, ..Default::default() };
    let mut starts = 0;
    for _ in 0..90 {
        common::step_preserving(&mut w, &d, &[hold, Input::default()]);
        starts += w.acts.iter().filter(|a| a.player == 0 && a.action.command()).count();
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert_eq!(starts, 1, "a held attack is one press");
}

/// A held press outlives the window and expires `bufferFrames` after release; without `holdBuffer` holding
/// changes nothing.
#[test]
fn held_press_expires_after_release() {
    let d = common::data();
    for hold in [false, true] {
        let mut r = d.global.input.clone();
        r.hold_buffer = hold;
        let n = u64::from(r.buffer_frames);
        let mut b = Buffer::default();
        b.deliver(&mut Intent { jump: true, jump_held: true, ..Default::default() }, 1, &r, false, false);
        for t in 2..=30 {
            b.deliver(&mut Intent { jump_held: true, ..Default::default() }, t, &r, false, false);
        }
        assert_eq!(b.cmd.is_some(), hold);
        for t in 31..=30 + n {
            b.deliver(&mut Intent::default(), t, &r, false, false);
            assert_eq!(b.cmd.is_some(), hold);
        }
        b.deliver(&mut Intent::default(), 31 + n, &r, false, false);
        assert!(b.cmd.is_none());
        // A held attack-stick press is kept by the stick, not the button.
        let mut stick = Intent { atk: true, strike: Strike::Stick { x: 0, y: 1, smash: true }, ..Default::default() };
        b.deliver(&mut stick, 100, &r, false, false);
        b.deliver(&mut Intent { atk_held: true, ..Default::default() }, 101 + n, &r, false, false);
        assert!(b.cmd.is_none());
    }
}
