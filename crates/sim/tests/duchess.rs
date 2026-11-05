//! Duchess (SSBU Peach) reference behavior: float, Vegetable tiers, Toad, Peach Bomber, Parasol and raw input.
mod common;

use sim::data::HitType;
use sim::input::Intent;
use sim::world::State;
use sim::{GameData, Input, PlayerConfig, World, combat, moves, step_world};

fn sandbox(d: &GameData, fighters: &[&str]) -> World {
    let players = fighters.iter().copied().map(PlayerConfig::human).collect();
    common::sandbox(d, "arena", players)
}

fn tick(w: &mut World, d: &GameData, input: Input) {
    step_world(w, d, &[input]);
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
}

/// Step only the fighter's move: no opponent AI, hitlag or incidental hits can blur frame boundaries.
fn step_move(w: &mut World, d: &GameData, input: Intent) {
    let mut f = w.fighters[0].clone();
    moves::st_attack(w, d, 0, &mut f, input, input.dir());
    w.fighters[0] = f;
    w.frame += 1;
}

fn start(d: &GameData, slot: &str) -> World {
    let mut w = sandbox(d, &["duchess", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].face = 1.;
    w.fighters[1].x = 1500.;
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, d, 0, &mut f, slot);
    w.fighters[0] = f;
    w
}

fn start_air(d: &GameData, slot: &str) -> World {
    let mut w = start(d, slot);
    let f = &mut w.fighters[0];
    f.ground = false;
    f.surf = None;
    // High above the arena floor so a 38-frame aerial cannot land during the frame-boundary walk.
    f.y = 120.;
    f.vy = 0.;
    w
}

/// Keep a fighter off the stage while a frame-boundary test only cares about the move timeline.
fn stay_air_at(w: &mut World, i: usize, y: f64) {
    let f = &mut w.fighters[i];
    f.ground = false;
    f.surf = None;
    f.y = y;
}

fn stay_air(w: &mut World, y: f64) {
    stay_air_at(w, 0, y);
}

#[track_caller]
fn phase_name(d: &GameData, w: &World) -> Option<String> {
    let m = w.fighters[0].mov.as_ref()?;
    Some(moves::timeline(d, &w.fighters[0])[m.phase].name.clone())
}

#[track_caller]
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

/// Every move's total duration: `frames` may be split across phases (autocancel/aerial splits, specials).
fn total(d: &GameData, slot: &str) -> u32 {
    d.fighter("duchess").unwrap().moves.get(slot).unwrap().phases.iter().filter_map(|p| p.frames).sum()
}

/// Absolute (move-frame) active windows, with each phase's own offset added.
fn windows(d: &GameData, slot: &str) -> Vec<(u32, u32, f64)> {
    let m = d.fighter("duchess").unwrap().moves.get(slot).unwrap();
    let mut out = vec![];
    let mut base = 0;
    for p in &m.phases {
        for h in &p.hitboxes {
            out.push((base + h.start, base + h.end.unwrap_or(base + p.frames.unwrap_or(0)), h.damage));
        }
        base += p.frames.unwrap_or(0);
    }
    out
}

#[test]
fn package_identity_slots_and_reference_stats() {
    let d = common::data();
    let f = d.fighter("duchess").expect("duchess package");
    assert_eq!(f.id, "duchess");
    assert_eq!(f.name, "Duchess");
    assert_eq!(f.extends.as_deref(), Some("base"));
    for slot in [
        "jab", "ftilt", "utilt", "dtilt", "dashatk", "fsmash", "usmash", "dsmash", "nair", "fair", "bair", "uair",
        "dair", "grab", "dashgrab", "nspec", "sspec", "uspec", "dspec",
    ] {
        assert!(f.moves.get(slot).is_some(), "missing {slot}");
    }
    assert!(f.moves.zair.is_none(), "Peach has no tether aerial");
    assert!(f.charges.is_empty(), "no limited-use resources");
    assert_eq!((f.throws.f.total, f.throws.b.total, f.throws.u.total, f.throws.d.total), (39, 49, 49, 64));
    // UFD: weight 89, air 1.029, air accel 0.12, gravity 0.070, fall 1.19 / 1.904; 5 px per unit.
    close(f.stats.weight, 100. / 89.);
    close(f.stats.pummel_damage.unwrap(), 1.3);
    close(f.stats.air_speed, 1.029 * 5.);
    close(f.stats.air_accel, 0.12 * 5.);
    close(f.stats.gravity, 0.070 * 5.);
    close(f.stats.fall_speed, 1.19 * 5.);
    close(f.stats.fast_fall_speed, 1.904 * 5.);
    // Source float: no vertical movement for up to 150 frames (2.5 s), once per airtime.
    let fl = f.stats.float.as_ref().unwrap();
    assert_eq!((fl.speed, fl.frames), (0., 150));
    // Ultimate ground_brake 0.111 / air_brake_x 0.01; full hop initial height 0.55 × 30.03 units.
    close(f.stats.traction.unwrap(), 0.111 * 5.);
    close(f.stats.air_friction.unwrap(), 0.01 * 5.);
    close(f.stats.jump_initial_height.unwrap(), 0.55 * 30.03 * 5.);
    assert!(f.moves.uspec.once_per_airtime);
}

#[test]
fn reference_durations_and_hit_windows_match_the_ledger() {
    let d = common::data();
    // UFD totals (phase sums), one per slot. Jab 1 + jab 2 share the jab slot.
    for (slot, frames) in [
        ("jab", 27 + 29),
        ("ftilt", 36),
        ("utilt", 37),
        ("dtilt", 23),
        ("dashatk", 37),
        ("fsmash", 45),
        ("usmash", 44),
        ("dsmash", 54),
        ("nair", 48),
        ("fair", 57),
        ("bair", 53),
        ("uair", 45),
        ("dair", 38),
        ("grab", 36),
        ("dashgrab", 44),
        ("nspec", 62 + 60),
        ("sspec", 12 + 24 + 25 + 26),
        ("uspec", 39 + 480),     // closed parasol is an open-ended special fall
        ("dspec", 39), // the pluck; the turnip is then a held item (tests/items.rs)
    ] {
        assert_eq!(total(&d, slot), frames, "{slot} total");
    }
    // Player-local phase frame = source frame - 1; the reference page's inclusive windows shift with it.
    let jab = windows(&d, "jab");
    assert!(jab.contains(&(1, 1, 2.0)), "jab 1 source frame 2 -> phase 1: {jab:?}");
    assert!(jab.contains(&(28, 29, 3.0)), "jab 2 (its own phase) 1-2 within that phase: {jab:?}");
    let ftilt = windows(&d, "ftilt");
    assert!(ftilt.contains(&(6, 8, 7.0)), "ftilt close 7-9: {ftilt:?}");
    assert!(ftilt.contains(&(9, 14, 6.0)), "ftilt late 10-15: {ftilt:?}");
    assert!(windows(&d, "dtilt").contains(&(7, 8, 7.0)), "dtilt 8-9");
    assert!(windows(&d, "dsmash").contains(&(5, 6, 2.0)), "dsmash first multi 6-7");
    assert!(windows(&d, "dsmash").contains(&(25, 26, 2.0)), "dsmash last multi 26-27");
    assert!(windows(&d, "dsmash").contains(&(29, 30, 3.0)), "dsmash final 30-31");
    let nair = windows(&d, "nair");
    assert!(nair.contains(&(4, 7, 13.0)), "nair clean 5-8: {nair:?}");
    assert!(nair.contains(&(8, 18, 6.0)), "nair late 9-19");
    assert!(nair.contains(&(8, 18, 5.0)), "nair late hip 5.0");
    let fair = windows(&d, "fair");
    assert!(fair.contains(&(15, 19, 15.0)), "fair arm 16-20: {fair:?}");
    assert!(fair.contains(&(15, 19, 14.0)), "fair sourspot arm 14.0");
    let bair = windows(&d, "bair");
    assert!(bair.contains(&(5, 6, 12.0)), "bair early 6-7");
    assert!(bair.contains(&(7, 12, 7.0)), "bair late 8-13");
    let uair = windows(&d, "uair");
    assert!(uair.iter().any(|&(s, e, dmg)| (s, e) == (9, 12) && (dmg - 4.0).abs() < 1e-9), "uair hit 1");
    assert!(uair.contains(&(14, 16, 6.0)), "uair finisher 15-17: {uair:?}");
    assert!(uair.contains(&(15, 18, 6.0)), "uair late finisher 16-19: {uair:?}");
    let dair = windows(&d, "dair");
    for (start, end, damage) in [(11, 12, 2.0), (11, 12, 1.0), (29, 30, 5.0)] {
        assert!(dair.contains(&(start, end, damage)), "dair {start}-{end} {damage}: {dair:?}");
    }
    let usmash = windows(&d, "usmash");
    assert!(usmash.contains(&(13, 15, 17.0)), "usmash arm tip 14-16");
    assert!(usmash.contains(&(16, 19, 10.0)), "usmash late 17-20");
    assert!(windows(&d, "fsmash").contains(&(14, 16, 15.0)), "golf club 15-17");
    // Smash charge holds: fsmash 5, usmash 9, dsmash 2 in source frames.
    let charge =
        |slot: &str| d.fighter("duchess").unwrap().moves.get(slot).unwrap().phases[0].charge.as_ref().unwrap().frame;
    assert_eq!((charge("fsmash"), charge("usmash"), charge("dsmash")), (4, 8, 1));
}

#[test]
fn aerial_landing_lag_and_autocancel_windows() {
    let d = common::data();
    // UFD landing lag and autocancel windows: 1-4 / 36+ (nair), 1-9 / 41+ (fair), 1-4 / 30+ (bair),
    // 1-4 / 41+ (uair), 1-11 / 38+ (dair).
    for (slot, lag, early, late) in
        [("nair", 7, 3, 35), ("fair", 13, 8, 40), ("bair", 11, 3, 29), ("uair", 7, 3, 40), ("dair", 8, 10, 37)]
    {
        for (frame, expected) in [(0, 0), (early, 0), (early + 1, lag), (late - 1, lag), (late, 0)] {
            // Advance the real timeline so the move is in the phase that owns this absolute frame.
            let mut w = start_air(&d, slot);
            for n in 0..frame {
                if w.fighters[0].mov.is_none() {
                    panic!("{slot} ended at tick {n} of {frame} (last state {:?})", w.fighters[0].state);
                }
                step_move(&mut w, &d, Intent::default());
                stay_air(&mut w, 120.);
            }
            let m = w.fighters[0].mov.as_ref().unwrap_or_else(|| panic!("{slot} ended before frame {frame}"));
            assert_eq!(m.elapsed, frame, "{slot} elapsed");
            let mut f = w.fighters[0].clone();
            assert!(moves::trigger(&mut w, &d, 0, &mut f, sim::data::Trigger::Land), "{slot} frame {frame}");
            if expected == 0 {
                assert_ne!(f.state, State::Landing, "{slot} frame {frame} is an autocancel window");
            } else {
                assert_eq!(f.state, State::Landing, "{slot} frame {frame}");
                assert_eq!(f.sd.n, f64::from(expected), "{slot} frame {frame}");
            }
        }
    }
}

/// Duchess in the air at rest, facing right, far from the opponent.
fn airborne(d: &GameData) -> World {
    let mut w = sandbox(d, &["duchess", "brawler"]);
    w.count = 0;
    w.fighters[1].x = 1500.;
    let f = &mut w.fighters[0];
    (f.x, f.y, f.vx, f.vy, f.ground, f.surf) = (640., 200., 0., 0., false, None);
    f.set_state(State::Air);
    w
}
const JUMP: Input = Input { stick: (0, 0), buttons: sim::input::JUMP, attack_stick: (0, 0) };

#[test]
fn float_holds_height_through_a_nair_and_ends_on_the_frame_limit() {
    let d = common::data();
    let mut w = sandbox(&d, &["duchess", "brawler"]);
    w.count = 0;
    w.fighters[1].x = 1500.;
    // A full hop with jump held throughout: once she stops rising, the float holds her there.
    let mut rose = 0;
    while w.fighters[0].floating == 0 {
        tick(&mut w, &d, JUMP);
        rose += 1;
        assert!(rose < 60, "no float after the full hop");
    }
    let f = &w.fighters[0];
    assert!(f.vy == 0. && f.float_used && f.jumps == 1);
    let y = f.y;
    let mut held = 1;
    for _ in 0..10 {
        tick(&mut w, &d, JUMP);
        held += 1;
        assert_eq!(w.fighters[0].y, y, "the float holds height");
    }
    // A neutral air started during the float keeps it: the height stays put through the aerial.
    tick(&mut w, &d, Input { buttons: sim::input::JUMP | sim::input::ATTACK, ..Default::default() });
    held += 1;
    assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some("nair"));
    while w.fighters[0].state == State::Attack {
        tick(&mut w, &d, JUMP);
        held += 1;
        assert_eq!(w.fighters[0].y, y, "height held through the nair");
    }
    // It runs out after 150 ticks, then she falls; the air jump is still available.
    while w.fighters[0].floating > 0 {
        tick(&mut w, &d, JUMP);
        held += 1;
    }
    assert_eq!(held, 150, "float length");
    tick(&mut w, &d, JUMP);
    tick(&mut w, &d, JUMP);
    assert!(w.fighters[0].vy > 0. && w.fighters[0].floating == 0, "falls after the frame limit");
    assert_eq!(w.fighters[0].jumps, 1, "the air jump survives the float");
}

#[test]
fn full_hop_climbs_its_initial_height_in_four_ticks_and_lands_on_the_source_frame() {
    let d = common::data();
    let s = &d.fighter("duchess").unwrap().stats;
    let hi = s.jump_initial_height.unwrap();
    let hop = |held: bool| {
        let mut w = sandbox(&d, &["duchess", "brawler"]);
        w.count = 0;
        // Clear of Arena's platforms.
        (w.fighters[0].x, w.fighters[1].x) = (290., 1500.);
        let y0 = w.fighters[0].y;
        tick(&mut w, &d, JUMP);
        while w.fighters[0].ground {
            tick(&mut w, &d, if held { JUMP } else { Input::default() });
        }
        let mut ys = vec![y0 - w.fighters[0].y];
        while !w.fighters[0].ground {
            tick(&mut w, &d, Input::default());
            ys.push(y0 - w.fighters[0].y);
        }
        ys
    };
    let full = hop(true);
    for (t, y) in full.iter().take(4).enumerate() {
        assert!((y - hi * (t + 1) as f64 / 4.).abs() < 1e-9, "tick {t}: {y}");
    }
    let apex = full.iter().copied().fold(0., f64::max);
    assert!((apex - 30.03 * 5.).abs() < 0.01, "full hop apex {apex}");
    // UFD air time: full hop 57, short hop 41 airborne frames (the landing tick is the last entry).
    assert_eq!(full.len() - 1, 57);
    let short = hop(false);
    assert!((short.iter().copied().fold(0., f64::max) - 14.5 * 5.).abs() < 0.01);
    assert_eq!(short.len() - 1, 41);
}

#[test]
fn float_is_once_per_airtime_and_ends_on_release() {
    let d = common::data();
    let mut w = airborne(&d);
    let f = &mut w.fighters[0];
    assert!(sim::physics::start_float(&d, f), "a fresh airtime can float");
    assert!(!sim::physics::start_float(&d, f), "only once per airtime");
    // Releasing jump ends it at once, and holding again does not restart it.
    tick(&mut w, &d, Input::default());
    assert_eq!(w.fighters[0].floating, 0);
    for _ in 0..5 {
        tick(&mut w, &d, Input::default());
    }
    let vy = w.fighters[0].vy;
    assert!(vy > 0.);
    tick(&mut w, &d, JUMP);
    tick(&mut w, &d, JUMP);
    assert_eq!(w.fighters[0].floating, 0, "no second float this airtime");
    // Landing restores it.
    for _ in 0..200 {
        tick(&mut w, &d, Input::default());
        if w.fighters[0].ground {
            break;
        }
    }
    assert!(w.fighters[0].ground && !w.fighters[0].float_used);
    // Jump with down held starts the float at once instead of spending the air jump.
    let mut w = airborne(&d);
    w.fighters[0].vy = -4.;
    tick(&mut w, &d, Input { stick: (0, -127), buttons: sim::input::JUMP, ..Default::default() });
    let f = &w.fighters[0];
    assert!(f.floating > 0 && f.jumps == 1 && f.vy == 0., "down + jump floats: {} {} {}", f.floating, f.jumps, f.vy);
    // A hit ends it.
    let mut w = airborne(&d);
    assert!(sim::physics::start_float(&d, &mut w.fighters[0]));
    let b: sim::data::Hitbox = serde_json::from_value(
        serde_json::json!({"x": 0, "y": 0, "r": 10, "damage": 5, "bkb": 30, "kbg": 0.5, "angle": 45}),
    )
    .unwrap();
    let src = combat::HitSource { attacker: 1, direction: 1., point: (600., 160.), projectile: true, label: None };
    combat::apply_hit(&mut w, &d, 0, &b, src);
    assert_eq!(w.fighters[0].floating, 0);
}

#[test]
fn vegetable_plucks_a_turnip_item_thrown_in_deterministic_tiers() {
    let d = common::data();
    // The pluck is 39 frames, spawns nothing and leaves the turnip in hand (out of the ground at phase frame 29).
    let mut w = start(&d, "dspec");
    assert_eq!(phase_name(&d, &w), Some("pluck".into()));
    for t in 1..=39 {
        tick(&mut w, &d, Input::default());
        assert!(w.projs.is_empty(), "no turnip flies before a throw");
        assert_eq!(w.fighters[0].item.is_some(), t >= 29, "frame {t}");
    }
    assert_eq!(w.fighters[0].state, State::Idle);
    let it = &d.fighter("duchess").unwrap().items["turnip"];
    assert_eq!((it.projectile.as_str(), it.smash_projectile.as_deref()), ("turnip", Some("turnip_strong")));
    // A tilt throw is the common (8.3%) turnip; a smash throw adds the item-throw strength difference (+3.1%).
    for (input, id, damage) in [
        (Input { buttons: 2, ..Default::default() }, "turnip", 8.3),
        (Input { attack_stick: (127, 0), ..Default::default() }, "turnip_strong", 11.4),
    ] {
        let mut w = w.clone();
        let before = w.rng;
        tick(&mut w, &d, input);
        for _ in 0..5 {
            tick(&mut w, &d, Input::default());
        }
        let p = w.projs.first().expect("turnip thrown");
        assert_eq!(p.id, id);
        close(p.hit.damage, damage);
        close(p.hit.angle, 30.);
        close(p.hit.bkb, 25.);
        close(p.hit.kbg, 0.45);
        if id == "turnip_strong" {
            assert!((p.hit.shield_dmg_mult - 0.7807).abs() < 1e-6, "{}", p.hit.shield_dmg_mult);
        }
        // Deterministic: neither tier consumes the gameplay RNG (the pluck picks no face at random).
        assert_eq!(w.rng, before);
    }
}

#[test]
fn toad_counters_only_inside_its_window_then_sprays_spores() {
    let d = common::data();
    // The counter window is source 9-34 -> phase 8-33, over a 62-frame animation.
    for frame in [7, 8, 33, 34] {
        let mut w = start(&d, "nspec");
        w.fighters[0].mov.as_mut().unwrap().t = frame;
        assert_eq!(combat::countering(&d, &w.fighters[0]).is_some(), (8..=33).contains(&frame), "frame {frame}");
    }
    let mut w = start(&d, "nspec");
    w.fighters[0].mov.as_mut().unwrap().t = 8;
    w.fighters[1].x = 680.;
    w.fighters[1].y = 520.;
    w.fighters[1].face = -1.;
    let mut attacker = w.fighters[1].clone();
    moves::start_move(&mut w, &d, 1, &mut attacker, "jab");
    attacker.mov.as_mut().unwrap().t = 3;
    w.fighters[1] = attacker;
    combat::hit_pass(&mut w, &d);
    assert_eq!(phase_name(&d, &w), Some("spores".into()));
    assert_eq!(w.fighters[1].lag, 18, "counter activation freezes the attacker");
    // Six spores leave the Toad as independently reflectable projectiles.
    w.fighters[1].x = 1200.;
    let mut births = std::collections::BTreeSet::new();
    for _ in 0..24 {
        tick(&mut w, &d, Input::default());
        for p in w.projs.iter().filter(|p| p.id.starts_with("spore_")) {
            births.insert((p.id.clone(), p.born));
        }
    }
    assert_eq!(births.len(), 6, "six spore births (three directions, two waves): {births:?}");
    let ids: std::collections::BTreeSet<_> = births.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(ids.len(), 3, "three fixed directions");
    assert!(w.projs.iter().filter(|p| p.id.starts_with("spore_")).all(|p| (p.hit.damage - 3.5).abs() < 1e-9));
    // A whiff runs the whole 62-frame animation.
    let mut w = start(&d, "nspec");
    let mut ticks = 0;
    while w.fighters[0].mov.is_some() && ticks < 200 {
        step_move(&mut w, &d, Intent::default());
        ticks += 1;
    }
    assert_eq!(ticks, 62, "62-frame Toad animation");
}

#[test]
fn toad_reflects_a_projectile_and_still_answers() {
    let d = common::data();
    let mut w = sandbox(&d, &["duchess", "throbbin"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].face = 1.;
    w.fighters[1].x = 900.;
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "nspec");
    w.fighters[0] = f;
    w.fighters[0].mov.as_mut().unwrap().t = 8;
    let owner = w.fighters[1].clone();
    sim::projectiles::spawn(&mut w, &d, 1, &owner, "thunder");
    let p = &mut w.projs[0];
    p.x = w.fighters[0].x + 10.;
    p.y = w.fighters[0].y - 40.;
    p.vx = -6.;
    p.vy = 0.;
    assert_eq!(p.life, 16);
    sim::projectiles::step_projectiles(&mut w, &d);
    let p = &w.projs[0];
    assert_eq!(p.owner, 0, "the counter steals the projectile");
    assert!(p.vx > 0., "reflected back: {}", p.vx);
    assert_eq!(p.life, 24, "counter reflectLife");
    assert_eq!(phase_name(&d, &w), Some("spores".into()));
}

#[test]
fn peach_bomber_travel_detects_targets_and_whiffs_for_the_source_total() {
    let d = common::data();
    // Whiff: windup 12 + thrust 24 + recovery 26 = 62, ending idle on flat ground.
    let mut w = start(&d, "sspec");
    let mut ticks = 0;
    let mut travelled = 0.;
    let start_x = w.fighters[0].x;
    while w.fighters[0].mov.is_some() && ticks < 200 {
        if phase_name(&d, &w) == Some("thrust".into()) && travelled == 0. {
            travelled = w.fighters[0].x;
        }
        step_move(&mut w, &d, Intent::default());
        ticks += 1;
    }
    assert_eq!(ticks, 62, "grounded whiff runs the whole 62-frame animation");
    assert!(w.fighters[0].x - start_x > 200., "the lunge covers ground: {}", w.fighters[0].x - start_x);
    assert_eq!(w.fighters[0].state, State::Idle);
    // On contact: the search box transitions to the 12% burst and Peach rebounds.
    let mut w = start(&d, "sspec");
    w.fighters[1].x = 700.;
    for _ in 0..20 {
        step_move(&mut w, &d, Intent::default());
        if phase_name(&d, &w) == Some("burst".into()) {
            break;
        }
    }
    assert_eq!(phase_name(&d, &w), Some("burst".into()));
    for _ in 0..3 {
        tick(&mut w, &d, Input::default());
    }
    assert!(w.fighters[1].dmg > 0., "bomber hit connects");
    close(w.fighters[1].dmg, 12.0);
    assert!(w.fighters[0].vx < 0., "rebound: {}", w.fighters[0].vx);
}

#[test]
fn peach_parasol_opens_slow_descent_and_closes_on_release() {
    let d = common::data();
    let mut w = start_air(&d, "uspec");
    // Rising: the attack phase launches upward and its final hit is at source 31-32.
    for _ in 0..8 {
        step_move(&mut w, &d, Intent { sp_held: true, ..Default::default() });
        stay_air(&mut w, 300.);
    }
    assert!(w.fighters[0].vy < 0., "parasol rise (world y down): {}", w.fighters[0].vy);
    let mut ticks = 0;
    while phase_name(&d, &w) != Some("open".into()) && ticks < 100 {
        step_move(&mut w, &d, Intent { sp_held: true, ..Default::default() });
        stay_air(&mut w, 300.);
        ticks += 1;
    }
    assert_eq!(phase_name(&d, &w), Some("open".into()));
    assert_eq!(ticks + 8, 39, "39-frame rising attack");
    for _ in 0..20 {
        step_move(&mut w, &d, Intent { sp_held: true, ..Default::default() });
        stay_air(&mut w, 300.);
    }
    let slow = w.fighters[0].vy;
    assert!(slow <= 2.0 * 5. / 6. + 1e-3, "open parasol caps descent at fallCap: {slow}");
    // Pressing special again closes it: normal gravity returns and the move ends in helplessness.
    for _ in 0..4 {
        step_move(&mut w, &d, Intent { sp: true, ..Default::default() });
        stay_air(&mut w, 300.);
    }
    assert_eq!(phase_name(&d, &w), Some("closed".into()));
    // Closed parasol keeps falling (special fall) well past the 15-frame closing animation.
    for _ in 0..30 {
        step_move(&mut w, &d, Intent::default());
        stay_air(&mut w, 300.);
    }
    assert_eq!(phase_name(&d, &w), Some("closed".into()), "closed special fall persists");
    assert!(w.fighters[0].vy > 2.5, "closed parasol falls: {}", w.fighters[0].vy);
    // Landing lag: 26 with the parasol open, 40 once closed (UFD). The closed phase itself must carry 40,
    // because shared helplessness would land with its own shorter lag.
    let mut f = w.fighters[0].clone();
    assert!(moves::trigger(&mut w, &d, 0, &mut f, sim::data::Trigger::Land));
    assert_eq!(f.state, State::Landing);
    assert_eq!(f.sd.n, 40.);
    let open = &d.fighter("duchess").unwrap().moves.uspec.phases[5];
    assert!(open.transitions.iter().any(|t| t.lag == Some(26)));
    let hit = &open.hitboxes[0];
    close(hit.damage, 2.0);
    assert_eq!(hit.rehit, Some(18));
}

#[test]
fn independent_hit_windows_rehit_the_same_victim() {
    let d = common::data();
    // (slot, aerial attacker, dx, dy (positive = victim above the attacker), minimum contacts)
    let cases: [(&str, bool, f64, f64, usize); 4] = [
        ("dsmash", false, 40., 0., 2),
        ("dashatk", false, 55., 0., 2),
        ("dair", true, 30., -40., 2),
        ("uair", false, 10., 100., 2),
    ];
    for (slot, air, dx, dy, want) in cases {
        let mut w = if air { start_air(&d, slot) } else { start(&d, slot) };
        let ay = w.fighters[0].y;
        let mut contacts = 0;
        for _ in 0..90 {
            if w.fighters[0].mov.is_none() {
                break;
            }
            {
                let f = &mut w.fighters[0];
                if air {
                    f.ground = false;
                    f.surf = None;
                    f.y = ay;
                }
                let (x, y) = (f.x, f.y);
                let v = &mut w.fighters[1];
                v.ground = false;
                v.surf = None;
                v.x = x + dx;
                v.y = y - dy;
            }
            step_world(&mut w, &d, &[Input::default()]);
            contacts += w.hits.iter().filter(|h| h.victim == 1 && !h.shielded).count();
            w.fx.clear();
            w.hits.clear();
            w.acts.clear();
        }
        assert!(contacts >= want, "{slot}: separate windows must re-hit the same victim, saw {contacts}");
    }
}

#[test]
fn peach_bomber_search_box_window_matches_the_source() {
    let d = common::data();
    // A victim that arrives on the last thrust frame (source 36) is still detected.
    let mut w = start(&d, "sspec");
    w.fighters[1].x = 1400.;
    for _ in 0..80 {
        let m = w.fighters[0].mov.as_ref().unwrap();
        if phase_name(&d, &w).as_deref() == Some("thrust") && m.t == 23 {
            break;
        }
        step_move(&mut w, &d, Intent::default());
    }
    assert_eq!(phase_name(&d, &w).as_deref(), Some("thrust"));
    w.fighters[1].x = w.fighters[0].x + 40.;
    step_move(&mut w, &d, Intent::default());
    assert_eq!(phase_name(&d, &w).as_deref(), Some("burst"), "source frame 36 still detects");
    // After the thrust window closes (source 37) the same victim is no longer detected.
    let mut w = start(&d, "sspec");
    w.fighters[1].x = 1400.;
    for _ in 0..80 {
        if phase_name(&d, &w).as_deref() == Some("recover") {
            break;
        }
        step_move(&mut w, &d, Intent::default());
    }
    assert_eq!(phase_name(&d, &w).as_deref(), Some("recover"));
    w.fighters[1].x = w.fighters[0].x + 40.;
    for _ in 0..3 {
        step_move(&mut w, &d, Intent::default());
    }
    assert_eq!(phase_name(&d, &w).as_deref(), Some("recover"), "detector is off after source frame 36");
    // The exact boundary: a victim that appears on the phase's last tick (source 37) is too late even though
    // the timeout and the search box are both true that tick.
    let mut w = start(&d, "sspec");
    w.fighters[1].x = 1400.;
    for _ in 0..80 {
        let m = w.fighters[0].mov.as_ref().unwrap();
        if phase_name(&d, &w).as_deref() == Some("thrust") && m.t == 24 {
            break;
        }
        step_move(&mut w, &d, Intent::default());
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().t, 24);
    w.fighters[1].x = w.fighters[0].x + 40.;
    step_move(&mut w, &d, Intent::default());
    assert_eq!(phase_name(&d, &w).as_deref(), Some("recover"), "source frame 37 no longer detects");
}

#[test]
fn reference_throws_and_pummel() {
    let d = common::data();
    let f = d.fighter("duchess").unwrap();
    // Source ATTACK_ABS rows: 8.0/9.0/6.0/7.0 with their pre-release catch hits.
    for (throw, damage, angle, tick) in [
        (&f.throws.f, 8.0, 45., 2.0),
        (&f.throws.b, 9.0, 140., 2.0),
        (&f.throws.u, 6.0, 88., 2.0),
        (&f.throws.d, 7.0, 106., 1.0),
    ] {
        let t = throw;
        close(t.hit.damage, damage);
        close(t.hit.angle, if angle > 180. { angle - 360. } else { angle });
        assert_eq!(t.ticks.len(), 1, "throw pre-hit");
        close(t.ticks[0].damage, tick);
    }
    assert_eq!(f.throws.f.release, 15);
    assert_eq!(f.throws.b.release, 21);
    assert_eq!(f.throws.u.release, 25);
    assert_eq!(f.throws.d.release, 42);
    close(f.stats.pummel_damage.unwrap(), 1.3);
}

#[test]
fn raw_input_routing_starts_the_expected_slots() {
    let d = common::data();
    for (input, slot) in [
        (Input { buttons: 2, ..Default::default() }, "jab"),
        (Input { buttons: 2, stick: (0, 127), ..Default::default() }, "utilt"),
        (Input { buttons: 2, stick: (0, -127), ..Default::default() }, "dtilt"),
        (Input { buttons: 4, ..Default::default() }, "nspec"),
        (Input { buttons: 4, stick: (127, 0), ..Default::default() }, "sspec"),
        (Input { buttons: 4, stick: (0, -127), ..Default::default() }, "dspec"),
        (Input { buttons: 8, ..Default::default() }, "grab"),
    ] {
        let players = vec![
            PlayerConfig { tap_jump: false, ..PlayerConfig::human("duchess") },
            PlayerConfig { tap_jump: false, ..PlayerConfig::human("brawler") },
        ];
        let mut w = common::sandbox(&d, "arena", players);
        w.count = 0;
        w.fighters[1].x = 1400.;
        if input.stick != (0, 0) {
            for _ in 0..8 {
                tick(&mut w, &d, Input { stick: input.stick, ..Default::default() });
            }
        }
        tick(&mut w, &d, input);
        let m = w.fighters[0].mov.as_ref().unwrap_or_else(|| panic!("{slot} did not start"));
        assert_eq!(m.slot, slot);
    }
    // Up special in the air, then the float input (held jump) caps the descent.
    let players = vec![
        PlayerConfig { tap_jump: false, ..PlayerConfig::human("duchess") },
        PlayerConfig { tap_jump: false, ..PlayerConfig::human("brawler") },
    ];
    let mut w = common::sandbox(&d, "arena", players);
    w.count = 0;
    w.fighters[1].x = 1400.;
    let f = &mut w.fighters[0];
    f.ground = false;
    f.surf = None;
    f.y = 300.;
    f.set_state(State::Air);
    tick(&mut w, &d, Input { buttons: 1, stick: (0, 127), ..Default::default() });
    assert_eq!(w.fighters[0].state, State::Air);
    tick(&mut w, &d, Input { buttons: 4, stick: (0, 127), ..Default::default() });
    assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some("uspec"));
}

#[test]
fn contact_coverage_follows_the_fitted_reach() {
    let d = common::data();
    // The dummy stands still in front (or behind) at the distance each swing should cover.
    let cases: [(&str, f64, bool, &[f64]); 6] = [
        ("jab", 1., false, &[40., 55.]),
        ("ftilt", 1., false, &[55., 70.]),
        ("dtilt", 1., false, &[40., 55.]),
        ("dsmash", 1., false, &[35., 50.]),
        ("dsmash", -1., false, &[-35., -50.]),
        ("bair", 1., true, &[-55., -70.]),
    ];
    for (slot, side, above, distances) in cases {
        for &distance in distances {
            let mut w = start(&d, slot);
            if above {
                let f = &mut w.fighters[0];
                f.ground = false;
                f.surf = None;
                f.y = 400.;
            }
            w.fighters[1].x = w.fighters[0].x + side * distance;
            w.fighters[1].y = w.fighters[0].y;
            w.fighters[1].face = -1.;
            if above {
                stay_air_at(&mut w, 1, 400.);
            }
            let mut hit = false;
            for _ in 0..45 {
                if w.fighters[0].mov.is_none() {
                    break;
                }
                tick(&mut w, &d, Input::default());
                if above {
                    stay_air(&mut w, 400.);
                    stay_air_at(&mut w, 1, 400.);
                }
                if w.fighters[1].dmg > 0. {
                    hit = true;
                    break;
                }
            }
            assert!(hit, "{slot} should connect at {distance} px (side {side})");
            // Nothing reaches across the stage.
            let mut w = start(&d, slot);
            w.fighters[1].x = w.fighters[0].x + 240.;
            for _ in 0..45 {
                if w.fighters[0].mov.is_none() {
                    break;
                }
                tick(&mut w, &d, Input::default());
            }
            close(w.fighters[1].dmg, 0.);
        }
    }
}

#[test]
fn imported_moves_carry_their_source_launch_fields() {
    let d = common::data();
    let f = d.fighter("duchess").unwrap();
    let all = |slot: &str| f.moves.get(slot).unwrap().phases.iter().flat_map(|p| &p.hitboxes).collect::<Vec<_>>();
    // 361 keeps its Sakurai behavior (Robin/Whiplash convention); the golf club is a plain 45-degree hit.
    assert!(all("bair").iter().any(|h| h.kind == HitType::Sakurai));
    assert!(all("fsmash").iter().all(|h| h.kind == HitType::Normal && (h.angle - 45.).abs() < 1e-9));
    // Autolink multihit on up air; spike on the grounded down-tilt box.
    assert!(all("uair").iter().any(|h| h.kind == HitType::Autolink));
    assert!(all("dtilt").iter().any(|h| h.kind == HitType::Spike));
    // Down tilt's airborne box launches at 80 degrees instead.
    assert!(
        all("dtilt").iter().any(|h| (h.angle - 80.).abs() < 1e-9 && h.situation == sim::data::hitbox::Situation::Air)
    );
    // Ground/air masks from the source survive on the dash attack's two hit groups.
    assert!(all("dashatk").iter().any(|h| h.situation == sim::data::hitbox::Situation::Ground));
    assert!(all("dashatk").iter().any(|h| h.situation == sim::data::hitbox::Situation::Air));
    // Fixed-knockback rows use the shared setKb adaptation.
    assert!(all("dsmash").iter().any(|h| h.set_kb));
    // Toad's spores are reflectable projectiles carrying the article's 3.5% row.
    let spore = f.projectiles.get("spore_fwd").unwrap();
    close(spore.hit.damage, 3.5);
    close(spore.hit.kbg, 2.1);
    assert_eq!(spore.life, 16);
}

#[test]
fn separated_spin_windows_connect_in_order() {
    assert_eq!(common::contact_sequence(&common::data(), "duchess", "dsmash"), [2., 2., 2., 2., 2., 2., 3.]);
}

#[test]
fn continuous_swings_contact_once_across_phase_splits() {
    let d = common::data();
    for (slot, damage) in [("ftilt", 7.), ("nair", 13.), ("usmash", 17.)] {
        assert_eq!(common::contact_sequence(&d, "duchess", slot), [damage], "{slot}");
    }
}

#[test]
fn parasol_rising_contacts_keep_the_loop_cooldown_across_phase_splits() {
    let d = common::data();
    let mut w = start_air(&d, "uspec");
    let mut hits = vec![];
    for _ in 0..39 {
        w.fighters[0].x = 640.;
        w.fighters[0].y = 100.;
        w.fighters[0].lag = 0;
        if let Some(b) = combat::active_boxes_at(&d, &w.fighters[0], w.frame).first() {
            let v = &mut w.fighters[1];
            v.set_state(State::Idle);
            v.inv = 0;
            v.blink = 0;
            v.lag = 0;
            v.x = b.x;
            v.y = b.y + 35.;
            v.ground = true;
        }
        combat::hit_pass(&mut w, &d);
        let elapsed = w.fighters[0].mov.as_ref().unwrap().elapsed;
        hits.extend(w.hits.drain(..).filter(|h| h.attacker == Some(0)).map(|h| (elapsed, h.damage)));
        step_move(&mut w, &d, Intent::default());
    }
    assert_eq!(hits, [(6, 3.), (10, 1.), (15, 1.), (20, 1.), (22, 1.), (27, 1.), (31, 4.)]);
}

#[test]
fn closed_parasol_can_still_grab_a_ledge() {
    let d = common::data();
    // `every_ledge_is_recoverable` covers the open parasol on every stage; this checks the closed special-fall
    // phase still runs the shared ledge check (it only runs when the phase declares a ledge-grab window).
    let s = &d.stages[0];
    let b = &s.blocks[0];
    assert!(b.right_ledge);
    let mut w = start_air(&d, "uspec");
    w.fighters[1].x = 1500.;
    for _ in 0..45 {
        if phase_name(&d, &w).as_deref() == Some("open") {
            break;
        }
        step_move(&mut w, &d, Intent { sp_held: true, ..Default::default() });
        stay_air(&mut w, 300.);
    }
    assert_eq!(phase_name(&d, &w).as_deref(), Some("open"));
    step_move(&mut w, &d, Intent { sp: true, ..Default::default() });
    stay_air(&mut w, 300.);
    assert_eq!(phase_name(&d, &w).as_deref(), Some("closed"));
    let (x, y) = (b.right + 30., b.top + s.ledge.hand_y + s.ledge.cy);
    let mut caught = false;
    for _ in 0..20 {
        {
            let f = &mut w.fighters[0];
            f.x = x;
            f.y = y;
            f.vx = 0.;
            f.vy = 2.;
        }
        step_move(&mut w, &d, Intent::default());
        if w.fighters[0].state == State::Ledge {
            caught = true;
            break;
        }
    }
    assert!(caught, "closed parasol grabs the ledge");
}

#[test]
fn parasol_rise_landing_does_not_trap() {
    let d = common::data();
    let uspec = &d.fighter("duchess").unwrap().moves.uspec;
    for p in uspec.phases.iter().filter(|p| p.name.starts_with("rise")) {
        assert!(
            p.transitions.iter().any(|t| t.on == sim::data::Trigger::Land && t.lag.is_some()),
            "{} needs a landing transition",
            p.name
        );
    }
    // A real landing during the rise (before the parasol opens) must enter landing lag, not continue on the ground.
    for ticks in [2, 12] {
        let mut w = start_air(&d, "uspec");
        for _ in 0..ticks {
            step_move(&mut w, &d, Intent { sp_held: true, ..Default::default() });
            stay_air(&mut w, 300.);
        }
        assert!(phase_name(&d, &w).as_deref().is_some_and(|name| name.starts_with("rise")));
        {
            let f = &mut w.fighters[0];
            f.ground = false;
            f.surf = None;
            f.y = 516.;
            f.vy = 8.;
        }
        step_move(&mut w, &d, Intent { sp_held: true, ..Default::default() });
        assert_eq!(w.fighters[0].state, State::Landing, "rise landing after {ticks} ticks");
        assert_eq!(w.fighters[0].sd.n, 26.);
    }
}
