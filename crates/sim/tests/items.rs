//! Held items (`items.rs`) with Duchess's turnip: pluck, throws by direction and strength, loose items, pickup,
//! catches, what stays available while holding, the CPU and determinism.
mod common;

use sim::actions::Action;
use sim::input::{ATTACK, GRAB, Intent, JUMP, SHIELD, SPECIAL, Strike};
use sim::world::{Held, LooseItem, Projectile, State};
use sim::{GameData, Input, PlayerConfig, World, items, moves, step_world};

const FLOOR: f64 = 520.;

fn setup(d: &GameData) -> World {
    let mut w = common::fighters(d, "duchess", &["brawler"]);
    w.count = 0;
    (w.fighters[0].x, w.fighters[0].face) = (440., 1.);
    w.fighters[1].x = 300.;
    w
}

fn turnip(d: &GameData) -> Held {
    Held { data: d.fighters.iter().position(|f| f.id == "duchess").unwrap(), id: "turnip".into() }
}

fn holding(d: &GameData) -> World {
    let mut w = setup(d);
    w.fighters[0].item = Some(turnip(d));
    w
}

fn press(buttons: u8) -> Input {
    Input { buttons, ..Default::default() }
}

/// One tick with these inputs for fighter 0 (fighter 1 idles); returns the actions accepted this tick.
fn tick(w: &mut World, d: &GameData, input: Input) -> Vec<Action> {
    step_world(w, d, &[input, Input::default()]);
    w.fx.clear();
    w.hits.clear();
    w.acts.drain(..).filter(|a| a.player == 0).map(|a| a.action).collect()
}

fn idle(w: &mut World, d: &GameData, n: usize) {
    for _ in 0..n {
        tick(w, d, Input::default());
    }
}

fn airborne(w: &mut World, i: usize, y: f64) {
    let f = &mut w.fighters[i];
    (f.ground, f.surf, f.y, f.vy) = (false, None, y, 0.);
    f.set_state(State::Air);
}

/// The thrown item, once it leaves the hand.
fn thrown(w: &World) -> &Projectile {
    w.projs.iter().find(|p| p.item.is_some()).expect("a thrown item")
}

#[track_caller]
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-3, "{a} != {b}");
}

/// Start a throw directly (as attack / grab presses would) and run it to its release frame.
fn throw(w: &mut World, d: &GameData, kind: &str, smash: bool) {
    let mut f = w.fighters[0].clone();
    items::toss(w, d, 0, &mut f, kind, smash);
    w.fighters[0] = f;
    idle(w, d, 5);
}

#[test]
fn pluck_puts_a_turnip_in_hand_and_ends_into_idle() {
    let d = common::data();
    let mut w = setup(&d);
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "dspec");
    w.fighters[0] = f;
    for t in 1..=39 {
        tick(&mut w, &d, Input::default());
        // Out of the ground on source frame 30 (phase frame 29).
        assert_eq!(w.fighters[0].item.is_some(), t >= 29, "frame {t}");
        assert!(w.projs.is_empty(), "plucking throws nothing");
    }
    assert_eq!(w.fighters[0].state, State::Idle, "the pluck ends into idle, turnip in hand");
    assert_eq!(w.fighters[0].item, Some(turnip(&d)));
    // Down special while holding plucks nothing more.
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "dspec");
    w.fighters[0] = f;
    idle(&mut w, &d, 40);
    assert_eq!(w.fighters[0].item, Some(turnip(&d)));
}

#[test]
fn forward_tilt_throw_launches_the_turnip_at_tilt_speed() {
    let d = common::data();
    let mut w = holding(&d);
    let acts = tick(&mut w, &d, press(ATTACK));
    assert_eq!(acts, [Action::Toss("f".into())], "attack throws instead of jabbing");
    assert_eq!((w.fighters[0].state, w.fighters[0].sd.kind.as_str()), (State::Toss, "f"));
    idle(&mut w, &d, 4);
    assert!(w.projs.is_empty(), "the item leaves the hand on toss frame 5");
    tick(&mut w, &d, Input::default());
    assert_eq!(w.fighters[0].item, None);
    let p = thrown(&w);
    assert_eq!((p.id.as_str(), p.item.as_deref()), ("turnip", Some("turnip")));
    close(p.hit.damage, 8.3);
    // Launched from the hold point at the tilt velocity, then one projectile step (gravity) before we see it.
    let f = &w.fighters[0];
    close(p.vx, 11.4286);
    close(p.x, f.x + 24. + 11.4286);
    close(p.vy, -3.3333 + 0.2667);
    idle(&mut w, &d, 16);
    assert_eq!(w.fighters[0].state, State::Idle, "the throw is 22 frames");
}

#[test]
fn smash_throw_is_faster_and_stronger() {
    let d = common::data();
    let mut w = holding(&d);
    let acts = tick(&mut w, &d, Input { attack_stick: (127, 0), ..Default::default() });
    assert_eq!(acts, [Action::Toss("f".into())]);
    assert_eq!(w.fighters[0].sd.n, 1., "the attack stick smash-throws");
    idle(&mut w, &d, 5);
    let p = thrown(&w);
    assert_eq!(p.id, "turnip_strong");
    close(p.vx, 14.2857);
    close(p.hit.damage, 11.4);
}

#[test]
fn up_down_back_and_drop_throws_use_their_own_velocities() {
    let d = common::data();
    let up = f64::hypot(11.4286, 3.3333);
    // In the air, so down throws fly instead of landing at once.
    for (kind, smash, v, damage) in [
        ("u", false, (0., -11.9048), 8.3),
        ("u", true, (0., -14.7281), 11.4),
        ("d", false, (0., 11.9048), 8.3),
        ("b", false, (-11.4286, -3.3333), 8.3),
        ("drop", false, (0., 0.), 8.3 - 0.6 * up),
    ] {
        let mut w = holding(&d);
        airborne(&mut w, 0, 200.);
        throw(&mut w, &d, kind, smash);
        let p = thrown(&w);
        close(p.vx, v.0);
        close(p.vy, v.1 + 0.2667);
        close(p.hit.damage, damage);
        if kind == "b" {
            assert_eq!(w.fighters[0].face, -1., "a back throw turns around");
        }
    }
    // Input mapping: aerial directions pick the throw; flicks and the attack stick make it a smash throw.
    let w = holding(&d);
    let f = &w.fighters[0];
    let aim = |i: Intent| items::aim(&d, f, &i);
    assert_eq!(aim(Intent::default()), ("f", false));
    assert_eq!(aim(Intent { up: true, ..Default::default() }), ("u", false));
    assert_eq!(aim(Intent { down: true, ..Default::default() }), ("d", false));
    assert_eq!(aim(Intent { left: true, ..Default::default() }), ("b", false));
    assert_eq!(aim(Intent { right: true, tap_r: true, ..Default::default() }), ("f", true));
    assert_eq!(aim(Intent { tap_u: true, ..Default::default() }), ("u", true));
    assert_eq!(aim(Intent { strike: Strike::Stick { x: -1, y: 0, smash: true }, ..Default::default() }), ("b", true));
    assert_eq!(aim(Intent { strike: Strike::Combo, down: true, ..Default::default() }), ("d", true));
}

#[test]
fn air_grab_drops_and_short_hop_aerials_throw() {
    let d = common::data();
    let mut w = holding(&d);
    airborne(&mut w, 0, 200.);
    assert_eq!(tick(&mut w, &d, press(GRAB)), [Action::Toss("drop".into())], "Z in the air lets go");
    // Jump + attack together: the short-hop aerial becomes an air throw.
    let mut w = holding(&d);
    tick(&mut w, &d, press(JUMP | ATTACK));
    assert_eq!(w.fighters[0].state, State::Jumpsquat);
    let mut acts = vec![];
    for _ in 0..4 {
        acts.extend(tick(&mut w, &d, Input::default()));
    }
    assert!(acts.contains(&Action::Toss("f".into())), "{acts:?}");
    assert_eq!(w.fighters[0].state, State::Toss);
    assert!(!w.fighters[0].ground);
    // Grab on the ground throws.
    let mut w = holding(&d);
    assert_eq!(tick(&mut w, &d, press(GRAB)), [Action::Toss("f".into())]);
}

#[test]
fn a_thrown_item_lands_loose_and_is_picked_up() {
    let d = common::data();
    let mut w = holding(&d);
    throw(&mut w, &d, "f", false);
    for _ in 0..200 {
        idle(&mut w, &d, 1);
        if w.items.first().is_some_and(|it| it.surf.is_some()) {
            break;
        }
    }
    assert!(w.projs.is_empty(), "the turnip lost its hitbox on the floor");
    let it = w.items[0].clone();
    assert_eq!(it.item, turnip(&d));
    assert_eq!(it.surf, Some(0), "resting on the main floor");
    close(it.y, FLOOR - 9.);
    // Walk over and pick it up with attack: a short pickup, no jab.
    w.fighters[0].x = it.x - 20.;
    w.fighters[0].face = 1.;
    let acts = tick(&mut w, &d, press(ATTACK));
    assert_eq!(acts, [Action::Pickup]);
    assert!(w.items.is_empty());
    assert_eq!(w.fighters[0].item, Some(turnip(&d)));
    assert_eq!((w.fighters[0].state, w.fighters[0].sd.kind.as_str()), (State::Landing, "pickup"));
    // Loose items expire.
    let mut w = setup(&d);
    w.items.push(LooseItem { item: turnip(&d), owner: 0, x: 700., y: FLOOR - 9., vx: 0., vy: 0., surf: Some(0), life: 3, age: 0 });
    idle(&mut w, &d, 3);
    assert!(w.items.is_empty());
}

#[test]
fn items_in_flight_are_caught_with_grab_attack_or_air_dodge() {
    let d = common::data();
    for buttons in [GRAB, ATTACK, SHIELD] {
        // A falling loose item in front of an airborne fighter.
        let mut w = setup(&d);
        airborne(&mut w, 0, 300.);
        w.items.push(LooseItem { item: turnip(&d), owner: 1, x: 470., y: 250., vx: 0., vy: 2., surf: None, life: 600, age: 40 });
        assert_eq!(tick(&mut w, &d, press(buttons)), [Action::Catch], "buttons {buttons}");
        assert_eq!(w.fighters[0].item, Some(turnip(&d)));
        assert!(w.items.is_empty());
        assert_eq!(w.fighters[0].state, State::Air, "a catch is not an air dodge");
    }
    // An opponent's thrown turnip is caught out of the air before it hits.
    let mut w = holding(&d);
    w.fighters[1].x = 900.;
    throw(&mut w, &d, "f", false);
    let p = thrown(&w).clone();
    airborne(&mut w, 1, p.y + 50.);
    (w.fighters[1].x, w.fighters[1].face) = (p.x + 30., -1.);
    step_world(&mut w, &d, &[Input::default(), press(GRAB)]);
    assert!(w.acts.iter().any(|a| a.player == 1 && a.action == Action::Catch));
    assert!(w.projs.is_empty() && w.items.is_empty());
    assert_eq!(w.fighters[1].item, Some(turnip(&d)));
    assert_eq!(w.fighters[1].dmg, 0.);
    // Its thrower cannot catch it straight back.
    let mut w = holding(&d);
    airborne(&mut w, 0, 200.);
    throw(&mut w, &d, "u", false);
    w.fighters[0].set_state(State::Air);
    assert!(!tick(&mut w, &d, press(GRAB)).contains(&Action::Catch));
}

#[test]
fn a_connecting_turnip_bounces_off_as_a_loose_item() {
    let d = common::data();
    let mut w = holding(&d);
    w.fighters[1].x = 600.;
    throw(&mut w, &d, "f", false);
    for _ in 0..30 {
        idle(&mut w, &d, 1);
        if !w.items.is_empty() {
            break;
        }
    }
    close(w.fighters[1].dmg, 8.3);
    let it = &w.items[0];
    assert!(it.vy < 0. && it.vx < 0., "bounces back and up: {it:?}");
}

#[test]
fn holding_keeps_jumps_float_shield_and_specials() {
    let d = common::data();
    let mut w = holding(&d);
    tick(&mut w, &d, press(SHIELD));
    assert_eq!(w.fighters[0].state, State::Shield);
    idle(&mut w, &d, 20);
    tick(&mut w, &d, press(JUMP));
    assert_eq!(w.fighters[0].state, State::Jumpsquat);
    idle(&mut w, &d, 6);
    assert_eq!(w.fighters[0].state, State::Air);
    let jumps = w.fighters[0].jumps;
    tick(&mut w, &d, press(JUMP));
    assert_eq!(w.fighters[0].jumps, jumps - 1, "air jump");
    for _ in 0..60 {
        tick(&mut w, &d, press(JUMP));
    }
    assert!(w.fighters[0].floating > 0, "float while holding");
    assert_eq!(tick(&mut w, &d, press(SPECIAL)), [Action::Move("nspec".into())]);
    assert_eq!(w.fighters[0].item, Some(turnip(&d)), "kept through all of it");
}

#[test]
fn a_cpu_holding_an_item_throws_it() {
    let d = common::data();
    let mut w = common::sandbox(&d, "arena", vec![PlayerConfig { cpu: Some(2), ..PlayerConfig::human("duchess") }, PlayerConfig::human("brawler")]);
    w.fighters[0].item = Some(turnip(&d));
    let mut tossed = false;
    for _ in 0..d.global.items.cpu_hold + 60 {
        step_world(&mut w, &d, &[]);
        tossed |= w.acts.iter().any(|a| a.player == 0 && matches!(a.action, Action::Toss(_)));
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        if tossed {
            break;
        }
    }
    assert!(tossed, "the CPU throws its turnip");
}

#[test]
fn item_play_is_deterministic() {
    let d = common::data();
    let mut a = holding(&d);
    a.fighters[1].x = 700.;
    a.items.push(LooseItem { item: turnip(&d), owner: 1, x: 500., y: 100., vx: 1., vy: 0., surf: None, life: 600, age: 0 });
    let mut b = a.clone();
    let script = |t: usize| match t {
        3 => Input { attack_stick: (127, 0), ..Default::default() },
        40 => press(ATTACK),
        80 => press(JUMP),
        100 => press(GRAB),
        _ => Input::default(),
    };
    for t in 0..240 {
        for w in [&mut a, &mut b] {
            step_world(w, &d, &[script(t), Input::default()]);
            w.fx.clear();
            w.hits.clear();
            w.acts.clear();
        }
        assert_eq!(a, b, "tick {t}");
    }
    assert!(a.fighters[1].dmg > 0. || !a.items.is_empty() || a.fighters[0].item.is_some());
}
