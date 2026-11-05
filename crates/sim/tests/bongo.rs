//! Bongo Boy (SSBU Diddy Kong) reference behavior, gadget semantics and raw-input coverage.
mod common;

use sim::data::HitType;
use sim::world::State;
use sim::{Input, PlayerConfig, World, combat, moves, step_world};

fn sandbox(d: &sim::GameData, fighters: &[&str]) -> World {
    let players = fighters.iter().copied().map(PlayerConfig::human).collect();
    common::sandbox(d, "arena", players)
}

fn tick(w: &mut World, d: &sim::GameData, input: Input) {
    step_world(w, d, &[input]);
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
}

/// Step only the fighter's move: no opponent AI, hitlag or incidental hits can blur frame boundaries.
fn step_move(w: &mut World, d: &sim::GameData, input: sim::input::Intent) {
    let mut f = w.fighters[0].clone();
    moves::st_attack(w, d, 0, &mut f, input, input.dir());
    w.fighters[0] = f;
    w.frame += 1;
}

fn start(d: &sim::GameData, slot: &str) -> World {
    let mut w = sandbox(d, &["bongo", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].face = 1.;
    w.fighters[1].x = 1300.;
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, d, 0, &mut f, slot);
    w.fighters[0] = f;
    w
}

fn start_air(d: &sim::GameData, slot: &str) -> World {
    let mut w = sandbox(d, &["bongo", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 100.;
    w.fighters[0].ground = false;
    w.fighters[0].surf = None;
    w.fighters[0].state = State::Air;
    w.fighters[0].face = 1.;
    w.fighters[1].x = 1300.;
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, d, 0, &mut f, slot);
    w.fighters[0] = f;
    w
}

fn begin(w: &mut World, d: &sim::GameData, slot: &str) {
    let mut f = w.fighters[0].clone();
    f.set_state(State::Idle);
    moves::start_move(w, d, 0, &mut f, slot);
    w.fighters[0] = f;
}

fn active_frames(d: &sim::GameData, slot: &str) -> Vec<u32> {
    let mut w = start(d, slot);
    let mut seen = vec![];
    for _ in 0..80 {
        let boxes = combat::active_boxes_at(d, &w.fighters[0], w.frame);
        let elapsed = w.fighters[0].mov.as_ref().unwrap().elapsed;
        if !boxes.is_empty() && !seen.contains(&elapsed) {
            seen.push(elapsed);
        }
        step_move(&mut w, d, sim::input::Intent::default());
        if w.fighters[0].mov.is_none() {
            break;
        }
    }
    seen
}

#[track_caller]
fn phase_name(d: &sim::GameData, w: &World) -> Option<String> {
    let m = w.fighters[0].mov.as_ref()?;
    Some(moves::timeline(d, &w.fighters[0])[m.phase].name.clone())
}

#[track_caller]
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn total(phases: &[sim::data::Phase]) -> u32 {
    phases.iter().map(|p| p.frames.unwrap_or(0)).sum()
}

#[test]
fn package_identity_and_slots() {
    let d = common::data();
    let f = d.fighter("bongo").expect("bongo package");
    assert_eq!(f.id, "bongo");
    assert_eq!(f.name, "Bongo Boy");
    assert_eq!(f.extends.as_deref(), Some("base"));
    for slot in [
        "jab", "ftilt", "utilt", "dtilt", "dashatk", "fsmash", "usmash", "dsmash", "nair", "fair", "bair", "uair",
        "dair", "grab", "dashgrab", "nspec", "sspec", "uspec", "dspec",
    ] {
        assert!(f.moves.get(slot).is_some(), "missing {slot}");
    }
    assert_eq!(f.throws.f.release, 20);
    assert_eq!(f.throws.b.release, 17);
    assert_eq!(f.throws.u.release, 18);
    assert_eq!(f.throws.d.release, 21);
    assert_eq!(f.throws.u.ticks.len(), 1, "up throw keeps its 1% pre-release hit");
    let pool = &f.charges[0];
    assert_eq!((pool.id.as_str(), pool.uses, pool.recharge), ("banana", 1, 85));
    assert!(f.projectiles.contains_key("banana") && f.projectiles.contains_key("peanut"));
}

#[test]
fn reference_active_windows_match_the_ledger() {
    let d = common::data();
    assert_eq!(active_frames(&d, "jab"), [2], "jab 1 source frame 3");
    let jab = &d.fighter("bongo").unwrap().moves.jab;
    assert_eq!(jab.phases.iter().map(|p| p.hitboxes[0].start).collect::<Vec<_>>(), [2, 4, 4]);
    assert_eq!(active_frames(&d, "ftilt"), (9..=15).collect::<Vec<_>>(), "clean 10-11, late 12-16");
    assert_eq!(active_frames(&d, "dtilt"), [3, 4], "source 4-5");
    assert_eq!(active_frames(&d, "fsmash"), [11, 12, 20, 21, 22], "first 12-13, second 21-23");
    assert_eq!(active_frames(&d, "usmash"), [4, 5, 6, 7, 11, 12, 13, 14, 18, 19, 20, 21, 22]);
    assert_eq!(active_frames(&d, "dsmash"), [6, 7, 11, 12]);
    // Aerials split into autocancel segments; the active move frames are unchanged.
    assert_eq!(active_frames(&d, "nair"), (7..=19).collect::<Vec<_>>());
    assert_eq!(active_frames(&d, "fair"), (5..=15).collect::<Vec<_>>());
    assert_eq!(active_frames(&d, "bair"), (4..=7).collect::<Vec<_>>());
    assert_eq!(active_frames(&d, "dair"), [14, 15]);
}

#[test]
fn durations_and_damage_match_the_source_ledger() {
    let d = common::data();
    let f = d.fighter("bongo").unwrap();
    assert_eq!(f.moves.jab.phases.iter().map(|p| p.frames.unwrap()).collect::<Vec<_>>(), [19, 20, 34]);
    for (slot, total_frames) in [
        ("ftilt", 36),
        ("utilt", 29),
        ("dtilt", 18),
        ("dashatk", 40),
        ("fsmash", 50),
        ("usmash", 53),
        ("dsmash", 50),
        ("nair", 48),
        ("fair", 46),
        ("bair", 30),
        ("uair", 36),
        ("dair", 46),
        ("grab", 36),
        ("dashgrab", 44),
    ] {
        assert_eq!(total(&f.moves.get(slot).unwrap().phases), total_frames, "{slot}");
        for (i, v) in f.moves.get(slot).unwrap().variants.iter().enumerate() {
            assert_eq!(total(&v.phases), total_frames, "{slot} variant {}", i + 1);
        }
    }
    assert_eq!(total(&f.moves.nspec.phases), 16 + 33 + 104, "charge + shot + overcharge");
    assert_eq!(17 + 19 + 11, 47, "default Monkey Flip path");
    assert_eq!(total(&f.moves.sspec.as_ref().unwrap().phases), 17 + 19 + 32 + 11, "leap + flip + kick + recover");
    assert_eq!(total(f.moves.uspec.variants.first().map_or(&f.moves.uspec.phases, |v| &v.phases)), 8 + 34 + 59 + 30);
    assert_eq!(total(&f.moves.dspec.phases), 39);
    assert_eq!(total(&f.moves.dspec.variants[0].phases), 39);
    for (slot, values) in [
        ("jab", vec![2.0, 1.5, 4.0]),
        ("ftilt", vec![10.0, 7.0]),
        ("dtilt", vec![5.5]),
        ("fsmash", vec![5.0, 11.0, 9.0]),
        ("usmash", vec![2.5, 2.5, 6.0]),
        ("dsmash", vec![12.0, 15.0]),
        ("nair", vec![6.0]),
        ("fair", vec![10.0, 8.0]),
        ("bair", vec![9.0]),
        ("uair", vec![7.0]),
        ("dair", vec![13.0, 10.0]),
    ] {
        let found: Vec<f64> =
            f.moves.get(slot).unwrap().phases.iter().flat_map(|p| &p.hitboxes).map(|h| h.damage).collect();
        for dmg in values {
            assert!(found.contains(&dmg), "{slot} is missing {dmg}: {found:?}");
        }
    }
    // Smash charge holds come from UFD: f-smash 4, up smash 1, down smash 2 (source frames).
    assert_eq!(f.moves.fsmash.phases[0].charge.as_ref().unwrap().frame, 3);
    assert_eq!(f.moves.usmash.phases[0].charge.as_ref().unwrap().frame, 0);
    assert_eq!(f.moves.dsmash.phases[0].charge.as_ref().unwrap().frame, 1);
    // Reference launch values: jab is a weak Sakurai hit, bair is a 9% back kick, dair is a spike.
    let jab = &f.moves.jab.phases[0].hitboxes[0];
    close(jab.bkb, 25.0);
    close(jab.kbg, 0.25);
    assert_eq!(jab.kind, HitType::Sakurai);
    let bair = &f.moves.bair.phases.iter().find(|p| !p.hitboxes.is_empty()).unwrap().hitboxes[0];
    close(bair.damage, 9.0);
    close(bair.bkb, 30.0);
    close(bair.kbg, 1.02);
    assert_eq!(bair.dir, sim::data::LaunchDir::Back);
    let dair = &f.moves.dair.phases.iter().find(|p| !p.hitboxes.is_empty()).unwrap().hitboxes[0];
    assert_eq!(dair.kind, HitType::Spike);
}

#[test]
fn banana_trips_a_grounded_victim_and_is_consumed() {
    let d = common::data();
    let mut w = sandbox(&d, &["bongo", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[1].x = 640.;
    w.fighters[1].y = 520.;
    let shooter = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &shooter, "banana");
    w.projs[0].x = 640.;
    w.projs[0].y = 480.;
    let mut tripped = false;
    for _ in 0..30 {
        w.fighters[1].x = 640.;
        w.fighters[1].y = 520.;
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        if w.fighters[1].state == State::Knockdown {
            tripped = true;
            break;
        }
    }
    assert!(tripped, "banana must trip a grounded victim");
    assert!(w.projs.is_empty(), "the single-contact peel is consumed");
    // One peel per owner: a second spawn while one is live does nothing.
    let shooter = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &shooter, "banana");
    sim::projectiles::spawn(&mut w, &d, 0, &shooter, "banana");
    assert_eq!(w.projs.len(), 1, "one banana at a time");
}

#[test]
fn banana_settles_on_the_ground_and_stays_there() {
    let d = common::data();
    let mut w = sandbox(&d, &["bongo", "brawler"]);
    w.count = 0;
    w.fighters[1].x = 1500.;
    let shooter = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &shooter, "banana");
    let start_y = w.projs[0].y;
    for _ in 0..80 {
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert_eq!(w.projs.len(), 1);
    let b = &w.projs[0];
    assert_eq!(b.vy, 0., "settled peels do not keep bouncing");
    assert!(b.vx.abs() < 1e-9, "settled peels stop sliding (vx {})", b.vx);
    assert!(b.surf.is_some(), "the peel rests on a surface");
    assert!(b.y > start_y, "it fell toward the ground");
    assert!(b.life > 0, "and remains until its lifetime");
}

#[test]
fn banana_expires_after_its_lifetime() {
    let d = common::data();
    let mut w = sandbox(&d, &["bongo", "brawler"]);
    w.count = 0;
    w.fighters[1].x = 1500.;
    let shooter = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &shooter, "banana");
    for _ in 0..600 {
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert!(w.projs.is_empty(), "the peel expires after its 600-tick lifetime");
}

#[test]
fn banana_generation_cooldown_and_one_at_a_time_pool() {
    let d = common::data();
    let mut w = sandbox(&d, &["bongo", "brawler"]);
    w.count = 0;
    w.fighters[1].x = 1500.;
    assert_eq!(w.fighters[0].charges[0], 1);
    begin(&mut w, &d, "dspec");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().variant, 1, "first use draws the banana");
    assert_eq!(w.fighters[0].charges[0], 0, "the pool is spent on use");
    for _ in 0..19 {
        assert_eq!(w.projs.len(), 0, "the peel appears at source frame 20");
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert_eq!(w.projs.len(), 1, "the peel spawns at source frame 20");
    // Reach tick 50 from the spend, then try the move during the cooldown: it runs the empty animation
    // and does not spawn a second peel.
    for _ in 19..50 {
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    begin(&mut w, &d, "dspec");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().variant, 0, "during the cooldown the empty animation runs");
    for _ in 0..34 {
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert_eq!(w.fighters[0].charges[0], 0, "still empty at tick 84");
    assert_eq!(w.projs.len(), 1, "the empty animation spawned nothing");
    step_world(&mut w, &d, &[]);
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
    assert_eq!(w.fighters[0].charges[0], 1, "one use returns at tick 85");
    for _ in 0..5 {
        step_world(&mut w, &d, &[]);
    }
    assert!(w.fighters[0].mov.is_none());
    // The pool is full but the first peel is still out: `max: 1` blocks the replacement.
    begin(&mut w, &d, "dspec");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().variant, 1);
    for _ in 0..19 {
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert_eq!(w.projs.len(), 1, "a second peel cannot replace the live one");
}

fn cast_peanut(held: u32) -> (sim::world::Projectile, World) {
    let d = common::data();
    let mut w = start(&d, "nspec");
    for _ in 0..400 {
        let m = w.fighters[0].mov.as_ref().unwrap();
        if m.t == 15 && m.charge == held {
            break;
        }
        step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
    }
    step_move(&mut w, &d, sim::input::Intent::default());
    for _ in 0..20 {
        if let Some(p) = w.projs.first() {
            return (p.clone(), w);
        }
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    panic!("no peanut for {held} held ticks");
}

#[test]
fn peanut_charge_scales_damage_and_speed() {
    let (weak, w) = cast_peanut(0);
    close(weak.hit.damage, 3.3);
    close(weak.hit.angle, 70.0);
    // Fitted peanut speed 13 → 21.5 at the old x3.5 scale.
    let speed = 13.0 * 5. / 3.5;
    assert!((weak.vx - speed).abs() < 0.1, "uncharged speed {}", weak.vx);
    assert_eq!(phase_name(&common::data(), &w).as_deref(), Some("shot"));
    let (mid, _) = cast_peanut(52);
    assert!(mid.hit.damage > 8.9 && mid.hit.damage < 9.3, "mid damage {}", mid.hit.damage);
    let (full, _) = cast_peanut(105);
    assert!((full.hit.damage - 15.0).abs() < 1e-3, "full damage {}", full.hit.damage);
    assert!((full.hit.angle - 45.0).abs() < 1e-3, "full angle {}", full.hit.angle);
    assert!(full.vx > 21.5 * 5. / 3.5 - 0.2, "fully charged speed {}", full.vx);
}

#[test]
fn peanut_overcharge_after_the_held_limit() {
    let d = common::data();
    let mut w = start(&d, "nspec");
    for _ in 0..400 {
        let m = w.fighters[0].mov.as_ref().unwrap();
        if m.t == 15 && m.charge == 105 && m.charge_hold == 3 {
            break;
        }
        step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
    }
    let m = w.fighters[0].mov.as_ref().unwrap();
    assert_eq!((m.t, m.charge, m.charge_hold), (15, 105, 3), "max charge plus three held ticks");
    step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
    assert_eq!(phase_name(&d, &w).as_deref(), Some("overcharge"), "overcharge self-stun");
    assert!(w.projs.is_empty(), "no peanut is fired");
    let m = w.fighters[0].mov.as_ref().unwrap();
    let phase = &moves::timeline(&d, &w.fighters[0])[m.phase];
    assert_eq!(phase.hitboxes[0].damage, 23.0);
    assert_eq!(phase.hitboxes[0].r, 41.0);
}

#[test]
fn monkey_flip_branches_to_kick_flip_or_grab() {
    let d = common::data();
    // Attack pressed during the flip selects the 14%/12% spinning kick.
    let mut w = start_air(&d, "sspec");
    for _ in 0..17 {
        w.fighters[0].y = 100.;
        w.fighters[0].vy = 0.;
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    assert_eq!(phase_name(&d, &w).as_deref(), Some("flip"));
    w.fighters[0].y = 100.;
    w.fighters[0].vy = 0.;
    step_move(&mut w, &d, sim::input::Intent { atk: true, ..Default::default() });
    assert_eq!(phase_name(&d, &w).as_deref(), Some("kick"));
    let phase = moves::phase(&d, &w.fighters[0]);
    assert_eq!(phase.hitboxes[0].damage, 14.0);
    assert_eq!(phase.hitboxes[1].damage, 12.0);
    // No input falls through to the flip recovery.
    let mut w = start_air(&d, "sspec");
    for _ in 0..17 + 19 {
        w.fighters[0].y = 100.;
        w.fighters[0].vy = 0.;
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    assert_eq!(phase_name(&d, &w).as_deref(), Some("recover"));
    let mut n = 0;
    while w.fighters[0].mov.is_some() && n < 30 {
        w.fighters[0].y = 100.;
        w.fighters[0].vy = 0.;
        step_move(&mut w, &d, sim::input::Intent::default());
        n += 1;
    }
    assert!(w.fighters[0].mov.is_none(), "the default flip completes in 47 frames");
    assert_ne!(w.fighters[0].state, State::Helpless, "Monkey Flip does not leave helpless");
    // A grounded victim caught in the grab window enters the shared hold.
    let mut w = sandbox(&d, &["bongo", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 300.;
    w.fighters[0].ground = false;
    w.fighters[0].surf = None;
    w.fighters[1].x = 690.;
    w.fighters[1].y = 300.;
    w.fighters[1].ground = false;
    w.fighters[1].surf = None;
    begin(&mut w, &d, "sspec");
    let mut grabbed = false;
    for _ in 0..40 {
        w.fighters[0].x = 640.;
        w.fighters[0].y = 300.;
        w.fighters[1].x = 690.;
        w.fighters[1].y = 300.;
        w.fighters[1].vx = 0.;
        w.fighters[1].vy = 0.;
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        if w.fighters[0].state == State::Hold && w.fighters[1].state == State::Grabbed {
            grabbed = true;
            break;
        }
    }
    assert!(grabbed, "Monkey Flip's command grab maps to the shared hold");
}

#[test]
fn rocketbarrel_charge_boost_and_landing_explosion() {
    let d = common::data();
    let mut w = start(&d, "uspec");
    for _ in 0..120 {
        let m = w.fighters[0].mov.as_ref().unwrap();
        if m.charge == 30 {
            break;
        }
        step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().charge, 30);
    step_move(&mut w, &d, sim::input::Intent::default());
    assert_eq!(phase_name(&d, &w).as_deref(), Some("boost"));
    assert!(w.fighters[0].vy < -10., "boost leaves the ground upward (vy {})", w.fighters[0].vy);
    let phase = moves::phase(&d, &w.fighters[0]);
    assert_eq!(phase.hitboxes[0].damage, 10.0);
    assert_eq!(phase.hitboxes[1].damage, 8.0);
    assert_eq!(phase.hitboxes[2].damage, 6.0);
    // Landing during the boost triggers the 18% explosion and its long ending.
    w.fighters[0].y = 514.;
    w.fighters[0].vy = 6.;
    w.fighters[0].ground = false;
    w.fighters[0].surf = None;
    step_world(&mut w, &d, &[]);
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
    assert_eq!(phase_name(&d, &w).as_deref(), Some("explosion"));
    let phase = moves::phase(&d, &w.fighters[0]);
    assert_eq!(phase.hitboxes[0].damage, 18.0);
    assert_eq!(phase.hitboxes[0].r, 100.0);
}

#[test]
fn rocketbarrel_auto_launches_at_full_charge() {
    let d = common::data();
    let mut w = start(&d, "uspec");
    for _ in 0..200 {
        let m = w.fighters[0].mov.as_ref().unwrap();
        if m.t == 7 && m.charge == 60 {
            break;
        }
        if phase_name(&d, &w).as_deref() == Some("boost") {
            break;
        }
        step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().charge, 60);
    step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
    assert_eq!(phase_name(&d, &w).as_deref(), Some("boost"), "holding past max launches automatically");
}

#[test]
fn raw_input_routing_starts_the_expected_moves() {
    let d = common::data();
    for (input, slot) in [
        (Input { buttons: 2, ..Default::default() }, "jab"),
        (Input { buttons: 2, stick: (0, 127), ..Default::default() }, "utilt"),
        (Input { buttons: 2, stick: (0, -127), ..Default::default() }, "dtilt"),
        (Input { buttons: 4, ..Default::default() }, "nspec"),
        (Input { buttons: 4, stick: (127, 0), ..Default::default() }, "sspec"),
        (Input { buttons: 4, stick: (0, 127), ..Default::default() }, "uspec"),
        (Input { buttons: 4, stick: (0, -127), ..Default::default() }, "dspec"),
        (Input { buttons: 8, ..Default::default() }, "grab"),
    ] {
        let players = vec![
            PlayerConfig { tap_jump: false, ..PlayerConfig::human("bongo") },
            PlayerConfig { tap_jump: false, ..PlayerConfig::human("brawler") },
        ];
        let mut w = common::sandbox(&d, "arena", players);
        w.count = 0;
        if input.stick != (0, 0) {
            for _ in 0..8 {
                tick(&mut w, &d, Input { stick: input.stick, ..Default::default() });
            }
        }
        tick(&mut w, &d, input);
        let m = w.fighters[0].mov.as_ref().unwrap_or_else(|| panic!("{slot} did not start"));
        assert_eq!(m.slot, slot);
    }
}

#[test]
fn banana_has_the_source_trip_hit_without_damage() {
    let d = common::data();
    let f = d.fighter("bongo").unwrap();
    let b = &f.projectiles["banana"];
    assert_eq!(b.hit.kind, HitType::Trip);
    close(b.hit.damage, 0.0);
    close(b.hit.bkb, 50.0);
    assert_eq!(b.max, 1);
    assert_eq!(b.life, 600);
}

#[test]
fn successive_hits_connect_on_the_same_victim() {
    let d = common::data();
    // Pin a grounded dummy so each move's successive phases can reach it again. Contact history must be
    // cleared by phase entry for the later hits to deal damage.
    let dealt = |slot: &str, dx: f64| -> f64 {
        let mut w = sandbox(&d, &["bongo", "brawler"]);
        w.count = 0;
        w.fighters[0].x = 640.;
        w.fighters[0].y = 520.;
        w.fighters[0].face = 1.;
        w.fighters[1].x = 640. + dx;
        w.fighters[1].y = 520.;
        let mut f = w.fighters[0].clone();
        moves::start_move(&mut w, &d, 0, &mut f, slot);
        w.fighters[0] = f;
        for t in 0..60 {
            w.fighters[1].x = 640. + dx;
            w.fighters[1].y = 520.;
            w.fighters[1].vx = 0.;
            w.fighters[1].vy = 0.;
            w.fighters[1].ground = true;
            w.fighters[1].inv = 0;
            let inputs = vec![Input { buttons: if t == 0 { 2 } else { 0 }, ..Default::default() }, Input::default()];
            step_world(&mut w, &d, &inputs);
            w.fx.clear();
            w.hits.clear();
            w.acts.clear();
            if w.fighters[0].mov.is_none() && t > 2 {
                break;
            }
        }
        w.fighters[1].dmg
    };
    // Source f-smash: 5% first hit then 11% second; dash attack 2+2+3; up smash 2.5+2.5+6.
    assert!(dealt("fsmash", 50.) >= 14., "f-smash successive hits must both land");
    assert!(dealt("dashatk", 50.) >= 6.5, "dash attack's three hits must all land");
    assert!(dealt("usmash", 40.) >= 10., "up smash's multihit and final hit must all land");
}

#[test]
fn up_smash_first_hit_reaches_a_grounded_victim() {
    let d = common::data();
    let mut w = sandbox(&d, &["bongo", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].face = 1.;
    w.fighters[1].x = 680.;
    w.fighters[1].y = 520.;
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "usmash");
    w.fighters[0] = f;
    let mut hit = false;
    for t in 0..8 {
        w.fighters[1].x = 680.;
        w.fighters[1].y = 520.;
        w.fighters[1].vx = 0.;
        w.fighters[1].vy = 0.;
        w.fighters[1].ground = true;
        w.fighters[1].inv = 0;
        let inputs = vec![Input { buttons: if t == 0 { 2 } else { 0 }, ..Default::default() }, Input::default()];
        step_world(&mut w, &d, &inputs);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        if w.fighters[1].dmg > 0. {
            hit = true;
            break;
        }
    }
    assert!(hit, "the first up-smash tier must connect on a grounded victim");
}

#[test]
fn rocketbarrel_charge_does_not_scale_hit_damage() {
    let d = common::data();
    let mut w = start(&d, "uspec");
    for _ in 0..200 {
        let m = w.fighters[0].mov.as_ref().unwrap();
        if m.t == 7 && m.charge == 60 {
            break;
        }
        step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
    }
    step_move(&mut w, &d, sim::input::Intent::default());
    assert_eq!(phase_name(&d, &w).as_deref(), Some("boost"));
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().charge, 60);
    let boxes = combat::active_boxes_at(&d, &w.fighters[0], w.frame);
    let damages: Vec<f64> = boxes.iter().map(|b| b.hit.damage).collect();
    assert_eq!(damages, vec![10.0], "full charge leaves the 10% boost hit unscaled");
    // The landing explosion is also fixed at 18%.
    w.fighters[0].y = 514.;
    w.fighters[0].vy = 6.;
    w.fighters[0].ground = false;
    w.fighters[0].surf = None;
    step_world(&mut w, &d, &[]);
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
    assert_eq!(phase_name(&d, &w).as_deref(), Some("explosion"));
    let boxes = combat::active_boxes_at(&d, &w.fighters[0], w.frame);
    assert_eq!(boxes[0].hit.damage, 18.0);
}

#[test]
fn tapped_popgun_runs_the_source_total() {
    let d = common::data();
    let mut w = start(&d, "nspec");
    let mut ticks = 0;
    while w.fighters[0].mov.is_some() && ticks < 100 {
        step_move(&mut w, &d, sim::input::Intent::default());
        ticks += 1;
    }
    assert_eq!(ticks, 48, "a tapped neutral special lasts 48 ticks, not 47");
}

#[test]
fn raw_jump_attack_attack_stick_hold_and_buffer() {
    let d = common::data();
    let mut w = common::fighters(&d, "bongo", &["brawler"]);
    w.count = 0;
    w.fighters[1].x = 1400.;
    // Jump + attack arrives as a short-hop aerial.
    tick(&mut w, &d, Input { buttons: sim::input::JUMP | sim::input::ATTACK, ..Default::default() });
    for _ in 0..8 {
        tick(&mut w, &d, Input::default());
    }
    assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some("nair"));
    // The right stick selects a fixed aerial.
    w.fighters[0].set_state(State::Air);
    w.fighters[0].ground = false;
    w.fighters[0].surf = None;
    w.fighters[0].history = Default::default();
    tick(&mut w, &d, Input { attack_stick: (127, 0), ..Default::default() });
    assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some("fair"));
    // Hold and release the special: the charged peanut is stronger than the tapped one.
    let mut w = common::fighters(&d, "bongo", &["brawler"]);
    w.count = 0;
    w.fighters[1].x = 1400.;
    for _ in 0..24 {
        tick(&mut w, &d, Input { buttons: sim::input::SPECIAL, ..Default::default() });
    }
    let mut charged = false;
    for _ in 0..12 {
        tick(&mut w, &d, Input::default());
        if let Some(p) = w.projs.first() {
            assert!(p.hit.damage > 3.3, "held shot damage {}", p.hit.damage);
            charged = true;
            break;
        }
    }
    assert!(charged, "the held shot must fire");
    // A press during landing lag is buffered and starts a jab once the landing ends.
    let mut w = common::fighters(&d, "bongo", &["brawler"]);
    w.count = 0;
    w.fighters[0].set_state(State::Landing);
    w.fighters[0].t = 0;
    w.fighters[0].sd.n = 3.;
    tick(&mut w, &d, Input { buttons: sim::input::ATTACK, ..Default::default() });
    for _ in 0..8 {
        tick(&mut w, &d, Input::default());
    }
    assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some("jab"));
}

#[test]
fn contact_reach_matches_fitted_geometry() {
    let d = common::data();
    for (slot, side, above, must) in [
        ("jab", 1., false, &[40., 55.][..]),
        ("ftilt", 1., false, &[55., 70.]),
        ("dtilt", 1., false, &[45., 60.]),
        ("fsmash", 1., false, &[45., 65.]),
        ("nair", 1., false, &[40., 55.]),
        ("fair", 1., false, &[55., 70.]),
        ("bair", -1., false, &[55., 70.]),
        ("uair", -1., true, &[40.]),
        ("dair", 1., false, &[40.]),
    ] {
        for want in must {
            assert!(connects(&d, slot, side * want, above), "{slot} must connect at {want} px");
        }
        assert!(!connects(&d, slot, side * 170., above), "{slot} must not reach 170 px");
    }
}

fn connects(d: &sim::GameData, slot: &str, dx: f64, above: bool) -> bool {
    let mut w = sandbox(d, &["bongo", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].face = 1.;
    let aerial = matches!(slot, "nair" | "fair" | "bair" | "uair" | "dair");
    if aerial {
        let f = &mut w.fighters[0];
        f.y = match slot {
            "uair" => 470.,
            "dair" => 470.,
            _ => 480.,
        };
        f.ground = false;
        f.surf = None;
        f.state = State::Air;
    }
    w.fighters[1].x = 640. + dx;
    w.fighters[1].y = if above { 460. } else { 520. };
    if above {
        w.fighters[1].ground = false;
        w.fighters[1].surf = None;
        w.fighters[1].state = State::Air;
    }
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, d, 0, &mut f, slot);
    w.fighters[0] = f;
    for t in 0..40 {
        if matches!(slot, "uair" | "dair") {
            w.fighters[0].y = 470.;
            w.fighters[0].vy = 0.;
            w.fighters[0].ground = false;
            w.fighters[0].surf = None;
        }
        let before = w.fighters[1].dmg;
        let inputs = vec![Input { buttons: if t == 0 { 2 } else { 0 }, ..Default::default() }, Input::default()];
        step_world(&mut w, d, &inputs);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        if w.fighters[0].mov.is_none() && t > 2 {
            break;
        }
        if w.fighters[1].dmg > before {
            return true;
        }
    }
    false
}

#[test]
fn separated_windows_each_hit_one_victim() {
    let d = common::data();
    for (slot, want) in [("fsmash", vec![5., 11.]), ("usmash", vec![2.5, 2.5, 6.]), ("dashatk", vec![2., 2., 3.])] {
        assert_eq!(common::contact_sequence(&d, "bongo", slot), want, "{slot}");
    }
}
