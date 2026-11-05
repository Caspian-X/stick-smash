//! A full hop's initial rise (`stats.jumpInitialHeight`) when something interrupts it: a throw or a neutral air
//! dodge lets it finish, a non-aerial move ends it with the jump speed, so nobody keeps the rise's per-tick speed
//! (Isaiah's "throw the turnip as you jump and go flying").
mod common;

use sim::input::{ATTACK, JUMP, SHIELD, SPECIAL};
use sim::world::{Held, State};
use sim::{GameData, Input, World, step_world};

fn setup(d: &GameData, item: bool) -> World {
    let mut w = common::fighters(d, "duchess", &["brawler"]);
    w.count = 0;
    (w.fighters[0].x, w.fighters[0].face) = (440., 1.);
    w.fighters[1].x = 100.;
    if item {
        let data = d.fighters.iter().position(|f| f.id == "duchess").unwrap();
        w.fighters[0].item = Some(Held { data, id: "turnip".into(), life: None });
    }
    w
}

/// Full hop, then `buttons` (with the stick `up`) on the given airborne tick; returns the peak height and the tick
/// states.
fn hop_then(d: &GameData, item: bool, at: usize, buttons: u8, up: bool) -> (f64, Vec<State>) {
    let mut w = setup(d, item);
    let jump = Input { buttons: JUMP, ..Default::default() };
    let stats = &d.fighters[w.fighters[0].data].stats;
    assert!(stats.jump_initial_height.is_some_and(|h| h > 0.), "Duchess full hops with an initial rise");
    let mut peak = f64::INFINITY;
    let mut states = vec![];
    for t in 0..70 {
        let inp = if t == at {
            Input { buttons: JUMP | buttons, stick: (0, up as i8), ..Default::default() }
        } else if t < at {
            jump
        } else {
            Input { buttons: JUMP, stick: (0, up as i8), ..Default::default() }
        };
        step_world(&mut w, d, &[inp, Input::default()]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        let f = &w.fighters[0];
        peak = peak.min(f.y);
        states.push(f.state);
        let hop = stats.jump_speed.max(stats.short_hop_speed);
        assert!(f.vy > -hop - 1e-9 || f.rise > 0, "tick {t}: vy {} with no rise left", f.vy);
    }
    (peak, states)
}

#[test]
fn a_throw_during_the_initial_rise_finishes_the_hop() {
    let d = common::data();
    let (plain, _) = hop_then(&d, false, 60, 0, false);
    let floor = 520.;
    assert!(floor - plain > 100., "a full hop: peak {plain}");
    for at in 4..7 {
        let (peak, states) = hop_then(&d, true, at, ATTACK, true);
        assert!(states.contains(&State::Toss), "tick {at}: threw");
        assert!((peak - plain).abs() < 1., "tick {at}: the throw keeps the hop's height ({peak} vs {plain})");
    }
    // Attack on the jumpsquat's last tick picks a short-hop throw instead.
    let (peak, states) = hop_then(&d, true, 3, ATTACK, true);
    assert!(states.contains(&State::Toss) && peak > plain + 20., "a short-hop throw: peak {peak} vs {plain}");
}

#[test]
fn a_neutral_air_dodge_during_the_initial_rise_finishes_the_hop() {
    let d = common::data();
    let (plain, _) = hop_then(&d, false, 60, 0, false);
    for at in 3..7 {
        let (peak, states) = hop_then(&d, false, at, SHIELD, false);
        assert!(states.contains(&State::Airdodge), "tick {at}: dodged");
        assert!((peak - plain).abs() < 1., "tick {at}: the dodge keeps the hop's height ({peak} vs {plain})");
    }
}

#[test]
fn a_special_during_the_initial_rise_hands_over_the_jump_speed() {
    let d = common::data();
    let (plain, _) = hop_then(&d, false, 60, 0, false);
    for at in 3..7 {
        // Neutral special (the counter) ends the rise early; the fighter keeps a jump's speed, never more.
        let (peak, states) = hop_then(&d, false, at, SPECIAL, false);
        assert!(states.contains(&State::Attack), "tick {at}: started the special");
        assert!(peak > plain - 1., "tick {at}: not higher than the hop ({peak} vs {plain})");
    }
}
