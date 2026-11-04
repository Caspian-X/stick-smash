//! Sneak (SSBU Snake) reference behavior, gadgets and raw-input coverage.
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

fn ticks(w: &mut World, d: &sim::GameData, input: Input, n: usize) {
    for _ in 0..n {
        tick(w, d, input);
    }
}

/// Step only player 0's move so AI, hitlag and incidental hits cannot blur frame boundaries.
fn step_move(w: &mut World, d: &sim::GameData, input: sim::input::Intent) {
    let mut f = w.fighters[0].clone();
    moves::st_attack(w, d, 0, &mut f, input, input.dir());
    w.fighters[0] = f;
    w.frame += 1;
}

fn setup(d: &sim::GameData, fighters: &[&str]) -> World {
    let mut w = sandbox(d, fighters);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].face = 1.;
    if w.fighters.len() > 1 {
        w.fighters[1].x = 900.;
        w.fighters[1].y = 520.;
    }
    w
}

fn start(d: &sim::GameData, slot: &str) -> World {
    start_with(d, slot, &["sneak", "brawler"])
}

fn start_with(d: &sim::GameData, slot: &str, fighters: &[&str]) -> World {
    let mut w = setup(d, fighters);
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, d, 0, &mut f, slot);
    w.fighters[0] = f;
    w
}

fn phase_name(d: &sim::GameData, w: &World) -> Option<String> {
    let m = w.fighters[0].mov.as_ref()?;
    Some(moves::timeline(d, &w.fighters[0])[m.phase].name.clone())
}

fn boxes(d: &sim::GameData, slot: &str) -> Vec<sim::data::Hitbox> {
    let f = d.fighter("sneak").unwrap();
    let m = f.moves.get(slot).unwrap();
    m.timelines().flat_map(|p| p.iter()).flat_map(|p| p.hitboxes.clone()).collect()
}

/// Active windows inside one phase (phase frames).
fn phase_spans(d: &sim::GameData, slot: &str, index: usize) -> Vec<(u32, u32)> {
    let f = d.fighter("sneak").unwrap();
    f.moves.get(slot).unwrap().phases[index].hitboxes.iter().map(|h| (h.start, h.end.unwrap())).collect()
}

/// Active windows in move frames (phase-relative box frames plus every earlier phase's length).
fn spans(d: &sim::GameData, slot: &str) -> Vec<(u32, u32)> {
    let f = d.fighter("sneak").unwrap();
    let mut base = 0;
    let mut out = Vec::new();
    for p in &f.moves.get(slot).unwrap().phases {
        for h in &p.hitboxes {
            out.push((base + h.start, base + h.end.unwrap()));
        }
        base += p.frames.unwrap_or(0);
    }
    out
}

fn damages(d: &sim::GameData, slot: &str) -> Vec<f64> {
    boxes(d, slot).iter().map(|h| h.damage).collect()
}

fn duration(d: &sim::GameData, slot: &str) -> u32 {
    let f = d.fighter("sneak").unwrap();
    f.moves.get(slot).unwrap().phases.iter().map(|p| p.frames.unwrap_or(0)).sum()
}

#[test]
fn package_identity_and_slots() {
    let d = common::data();
    let f = d.fighter("sneak").expect("sneak package");
    assert_eq!(f.id, "sneak");
    assert_eq!(f.name, "Sneak");
    assert_eq!(f.extends.as_deref(), Some("base"));
    for slot in [
        "jab", "ftilt", "utilt", "dtilt", "dashatk", "fsmash", "usmash", "dsmash", "nair", "fair", "bair", "uair",
        "dair", "grab", "dashgrab", "nspec", "sspec", "uspec", "dspec",
    ] {
        assert!(f.moves.get(slot).is_some(), "missing {slot}");
    }
    assert_eq!(f.charges.len(), 1);
    assert_eq!((f.charges[0].id.as_str(), f.charges[0].uses), ("c4", 1));
    // Weight 106 is the reciprocal knockback multiplier.
    assert!((f.stats.weight - 100. / 106.).abs() < 1e-6);
    for id in ["grenadeMid", "grenadeStrong", "grenadeLob", "c4", "cypherShot", "mortarShell", "nikita"] {
        assert!(f.projectiles.contains_key(id), "missing projectile {id}");
    }
    // Gadgets are data: gravity/bounce/fuse, a manual remote blast, and a steered article.
    let g = &f.projectiles["grenadeMid"];
    assert!(g.bounce.is_some() && g.gravity > 0.);
    assert!(g.explosion.as_ref().is_some_and(|e| !e.manual));
    let c4 = f.projectiles["c4"].explosion.as_ref().unwrap();
    assert!(c4.manual && c4.arm_ticks == 0 && c4.radius > 60.);
    let nikita = f.projectiles["nikita"].article.as_ref().unwrap();
    assert_eq!(nikita.slot, "sspec");
    assert!(nikita.release_on_up && nikita.steer > 0.);
}

#[test]
fn reference_windows_and_damage() {
    let d = common::data();
    // Engine phase frame = source frame - 1, so UFD "3-4" is phase 2-3.
    assert!(phase_spans(&d, "jab", 0).contains(&(2, 3)), "jab 1 window");
    assert!(phase_spans(&d, "jab", 1).contains(&(3, 4)), "jab 2 window");
    assert!(phase_spans(&d, "jab", 2).contains(&(7, 8)), "jab 3 window");
    assert!(damages(&d, "jab").contains(&6.0));
    assert_eq!(phase_spans(&d, "ftilt", 0)[0], (3, 4), "knee 4-5");
    assert!(phase_spans(&d, "ftilt", 1).contains(&(7, 7)), "body blow 8");
    let ft = damages(&d, "ftilt");
    for dmg in [4.0, 9.0, 11.0, 10.0] {
        assert!(ft.contains(&dmg), "ftilt damage {dmg}: {ft:?}");
    }
    // Up tilt: 14.5 through 6-8, 13.5 through 9-13.
    let ut = boxes(&d, "utilt");
    assert!(ut.iter().all(|h| h.damage == 14.5) || ut.iter().any(|h| h.damage == 13.5));
    assert!(ut.iter().filter(|h| h.damage == 14.5).all(|h| h.start <= 7 && h.end.unwrap() <= 7));
    assert!(ut.iter().filter(|h| h.damage == 13.5).all(|h| (h.start, h.end.unwrap()) == (8, 12)));
    assert_eq!(spans(&d, "dtilt"), vec![(5, 5), (5, 5), (5, 5), (6, 6), (6, 6), (6, 6)]);
    assert!(damages(&d, "dtilt").iter().all(|d| *d == 12.0));
    // Down smash: front 8-10 then back 20-22.
    let ds = boxes(&d, "dsmash");
    assert!(ds.iter().any(|h| h.damage == 12.0 && (h.start, h.end.unwrap()) == (7, 9)));
    assert!(ds.iter().any(|h| h.damage == 14.0 && (h.start, h.end.unwrap()) == (19, 21) && h.x < 0.));
    // Forward smash: 22% on 41-43 with the documented charge hold at 38.
    let fs = boxes(&d, "fsmash");
    assert!(fs.iter().all(|h| h.damage == 22.0 && (h.start, h.end.unwrap()) == (40, 42)));
    assert_eq!(d.fighter("sneak").unwrap().moves.fsmash.phases[0].charge.as_ref().unwrap().frame, 37);
    // Up smash: fixed-knockback launcher on 11-12, the shell spawns on 35.
    let us = d.fighter("sneak").unwrap().moves.usmash.phases[0].clone();
    assert!(us.hitboxes.iter().all(|h| h.damage == 4.0 && h.set_kb && h.start == 10 && h.end == Some(11)));
    assert!(us.events.iter().any(|e| e.at == Some(34) && e.spawn.as_deref() == Some("mortarShell")));
    // Aerials: active windows, landing lag and autocancel splits.
    assert!(spans(&d, "nair").contains(&(9, 10)) && spans(&d, "nair").contains(&(35, 37)));
    assert!(damages(&d, "nair").contains(&12.0));
    assert!(spans(&d, "fair").contains(&(22, 25)));
    assert!(boxes(&d, "fair").iter().any(|h| h.damage == 15.0 && h.kind == HitType::Spike));
    assert!(spans(&d, "bair").contains(&(6, 8)) && spans(&d, "bair").contains(&(9, 25)));
    assert!(damages(&d, "bair").contains(&16.0) && damages(&d, "bair").contains(&9.0));
    assert!(spans(&d, "uair").contains(&(9, 11)) && spans(&d, "uair").contains(&(12, 22)));
    assert!(spans(&d, "dair").contains(&(2, 3)) && spans(&d, "dair").contains(&(24, 25)));
    assert!(damages(&d, "dair").contains(&10.0));
    // Totals from the pinned UFD rows.
    for (slot, total) in [
        ("jab", 15 + 24 + 46),
        ("ftilt", 31 + 39),
        ("utilt", 37),
        ("dtilt", 30),
        ("dashatk", 41),
        ("fsmash", 73),
        ("usmash", 54),
        ("dsmash", 44),
        ("nair", 59),
        ("fair", 69),
        ("bair", 42),
        ("uair", 47),
        ("dair", 59),
        ("grab", 34),
        ("dashgrab", 42),
    ] {
        assert_eq!(duration(&d, slot), total, "{slot} duration");
    }
    // Landing lag lives on the main aerial phase; the early and late spans land without lag.
    let nair = &d.fighter("sneak").unwrap().moves.nair.phases;
    assert_eq!(nair[1].transitions[0].lag, Some(16));
    assert_eq!(nair[0].transitions[0].lag, None);
    // Source autocancel "50 onward" is engine frame 49.
    assert_eq!(nair.last().unwrap().transitions[0].lag, None);
    assert_eq!(nair[nair.len() - 2].transitions[0].lag, Some(16));
    assert_eq!(d.fighter("sneak").unwrap().moves.fair.phases[1].transitions[0].lag, Some(19));
    assert_eq!(d.fighter("sneak").unwrap().moves.dair.phases[0].transitions[0].lag, Some(20));
}

#[test]
fn grenade_is_pulled_held_and_thrown_by_the_stick() {
    let d = common::data();
    for (stick, id, phase) in
        [(0.0, "grenadeMid", "throwMid"), (1.0, "grenadeStrong", "throwSmash"), (-1.0, "grenadeLob", "throwLob")]
    {
        let mut w = start(&d, "nspec");
        // The pin pull cannot throw before frame 21, then holds the grenade.
        for _ in 0..20 {
            step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
        }
        assert_eq!(phase_name(&d, &w).as_deref(), Some("pull"), "still pulling at frame 20");
        assert!(w.projs.is_empty(), "nothing spawned while the grenade is held");
        step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
        assert_eq!(phase_name(&d, &w).as_deref(), Some("hold"));
        // Releasing special throws it along the stick's direction.
        let release = sim::input::Intent { x: stick, sp_held: false, ..Default::default() };
        step_move(&mut w, &d, release);
        assert_eq!(phase_name(&d, &w).as_deref(), Some(phase), "stick {stick} picks {phase}");
        for _ in 0..10 {
            step_move(&mut w, &d, sim::input::Intent::default());
        }
        assert_eq!(w.projs.len(), 1, "{id} should be in flight");
        assert_eq!(w.projs[0].id, id);
        assert!(w.projs[0].vx * w.fighters[0].face > 0., "thrown forward: {}", w.projs[0].vx);
        let def = &d.fighter("sneak").unwrap().projectiles[id];
        assert!(def.bounce.as_ref().unwrap().restitution > 0.);
        assert!(def.explosion.is_some());
    }
}

#[test]
fn grenade_bounces_and_explodes_on_its_fuse() {
    let d = common::data();
    let mut w = setup(&d, &["sneak", "brawler"]);
    let id = "grenadeMid";
    let f = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &f, id);
    let p = w.projs[0].clone();
    assert!(p.vy < 0., "thrown upward");
    let life = p.life;
    let mut bounced = false;
    let mut y_min = f64::MAX;
    for _ in 0..(life as usize - 20) {
        sim::projectiles::step_projectiles(&mut w, &d);
        w.fx.clear();
        if let Some(p) = w.projs.first() {
            y_min = y_min.min(p.y);
            if p.bounces > 0 {
                bounced = true;
            }
        }
    }
    assert!(bounced, "the grenade should bounce off the floor");
    assert!(y_min < 520. - 40., "it should arc above the floor: {y_min}");
    assert_eq!(w.projs.len(), 1, "fuse has not run out yet");
    // Standing next to the dying grenade is a 9% blast.
    w.fighters[1].x = w.projs[0].x;
    w.fighters[1].y = 520.;
    w.fighters[1].ground = true;
    w.fighters[1].dmg = 0.;
    for _ in 0..40 {
        sim::projectiles::step_projectiles(&mut w, &d);
        w.fx.clear();
    }
    assert!(w.projs.is_empty(), "grenade should be consumed by its blast");
    assert!((w.fighters[1].dmg - 9.0).abs() < 1e-6, "blast damage {}", w.fighters[1].dmg);
    assert_eq!(w.fighters[0].dmg, 0., "the owner is not hit by their own grenade");
}

#[test]
fn c4_plants_once_then_detonates_remotely_without_self_damage() {
    let d = common::data();
    let mut w = setup(&d, &["sneak", "brawler"]);
    assert_eq!(w.fighters[0].charges[0], 1);
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "dspec");
    assert_eq!(f.mov.as_ref().unwrap().variant, 1, "plant timeline");
    w.fighters[0] = f;
    for _ in 0..40 {
        tick(&mut w, &d, Input::default());
    }
    assert_eq!(w.fighters[0].charges[0], 0, "planting spends the pool");
    assert_eq!(w.projs.len(), 1, "a C4 is planted");
    assert_eq!(w.projs[0].id, "c4");
    assert!(w.projs[0].released.is_none());
    let planted = w.projs[0].clone();
    assert!(planted.life > 1000, "manual gadgets keep a long fuse");
    // It settles on the floor in front of Snake's feet.
    for _ in 0..20 {
        tick(&mut w, &d, Input::default());
    }
    assert!((w.projs[0].y - (520. - 10.)).abs() < 2., "settled on the ground: {}", w.projs[0].y);
    assert!(w.projs[0].surf.is_some(), "settled on a surface");
    // A second down special presses the switch.
    w.fighters[1].x = w.projs[0].x + 20.;
    w.fighters[1].y = 520.;
    w.fighters[1].ground = true;
    w.fighters[1].dmg = 0.;
    let mut f = w.fighters[0].clone();
    f.set_state(State::Idle);
    moves::start_move(&mut w, &d, 0, &mut f, "dspec");
    assert_eq!(f.mov.as_ref().unwrap().variant, 0, "detonate timeline");
    w.fighters[0] = f;
    let mut damage = 0.;
    for _ in 0..30 {
        tick(&mut w, &d, Input::default());
        damage = w.fighters[1].dmg;
        if w.projs.is_empty() {
            break;
        }
    }
    assert!(w.projs.is_empty(), "the blast removes the C4");
    assert!((damage - 17.0).abs() < 1e-6, "C4 blast damage {damage}");
    assert_eq!(w.fighters[0].dmg, 0., "Snake is not damaged by his own C4");
    assert_eq!(w.fighters[0].charges[0], 1, "detonating restores the pool for the next plant");
    // And the pool's use really plants another one.
    let mut f = w.fighters[0].clone();
    f.set_state(State::Idle);
    moves::start_move(&mut w, &d, 0, &mut f, "dspec");
    assert_eq!(f.mov.as_ref().unwrap().variant, 1);
}

#[test]
fn nikita_is_steered_while_held_and_flies_when_released() {
    let d = common::data();
    let mut w = setup(&d, &["sneak", "brawler"]);
    let hold = Input { buttons: 4, ..Default::default() };
    // Side special needs a direction on the starting press; then hold special to steer.
    tick(&mut w, &d, Input { buttons: 4, stick: (127, 0), ..Default::default() });
    assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some("sspec"));
    ticks(&mut w, &d, hold, 41);
    assert_eq!(phase_name(&d, &w).as_deref(), Some("guide"));
    assert_eq!(w.projs.len(), 1, "missile spawned");
    assert!(w.projs[0].released.is_none(), "still under control");
    let y0 = w.projs[0].y;
    let up = Input { buttons: 4, stick: (0, 127), ..Default::default() };
    ticks(&mut w, &d, up, 10);
    let risen = w.projs[0].y;
    assert!(risen < y0 - 15., "steering up should raise the missile: {y0} -> {risen}");
    let down = Input { buttons: 4, stick: (0, -127), ..Default::default() };
    ticks(&mut w, &d, down, 20);
    assert!(w.projs[0].y > risen + 30., "steering down should lower it");
    // Releasing special launches it forward.
    tick(&mut w, &d, Input::default());
    assert!(w.projs[0].released.is_some(), "released");
    assert_eq!(phase_name(&d, &w).as_deref(), Some("fire"));
    let x0 = w.projs[0].x;
    ticks(&mut w, &d, Input::default(), 6);
    assert!(w.projs[0].x > x0 + 20., "released missile flies forward");
    // The released shot carries the source's strong 14% explosion, one box so a straight hit cannot be stolen
    // by an outer ring.
    let a = d.fighter("sneak").unwrap().projectiles["nikita"].article.as_ref().unwrap();
    let stage = &a.stages[w.projs[0].stage];
    assert_eq!(stage.hitboxes.len(), 1);
    assert_eq!(stage.hitboxes[0].damage, 14.0);
    assert!(stage.hitboxes[0].r > 60.);
}

#[test]
fn released_nikita_damages_the_opponent() {
    let d = common::data();
    let mut w = setup(&d, &["sneak", "brawler"]);
    let hold = Input { buttons: 4, ..Default::default() };
    tick(&mut w, &d, Input { buttons: 4, stick: (127, 0), ..Default::default() });
    ticks(&mut w, &d, hold, 41);
    let missile = w.projs[0].clone();
    w.fighters[1].x = missile.x + 120.;
    w.fighters[1].y = missile.y;
    w.fighters[1].ground = false;
    w.fighters[1].dmg = 0.;
    tick(&mut w, &d, Input::default());
    ticks(&mut w, &d, Input::default(), 25);
    assert!(w.fighters[1].dmg > 0., "missile should hit: {}", w.fighters[1].dmg);
    assert!((w.fighters[1].dmg - 14.0).abs() < 1e-6 || (w.fighters[1].dmg - 7.0).abs() < 1e-6);
}

#[test]
fn cypher_climbs_arms_and_detaches() {
    let d = common::data();
    let mut w = start(&d, "uspec");
    let y0 = w.fighters[0].y;
    for _ in 0..30 {
        tick(&mut w, &d, Input::default());
    }
    assert!(!w.fighters[0].ground, "Cypher lifts Snake off the ground");
    assert!(w.fighters[0].y < y0 - 80., "rising: {} -> {}", y0, w.fighters[0].y);
    // 7% damage-based armor keeps the ride: a 5-damage hit does not interrupt it.
    let mut f = w.fighters[0].clone();
    let hit = sim::data::Hitbox {
        damage: 5.,
        ..d.fighter("brawler").unwrap().moves.jab.phases[0].hitboxes.first().cloned().unwrap()
    };
    let source = combat::HitSource { attacker: 1, direction: 1., point: (f.x, f.y), projectile: false, label: None };
    combat::apply_hit(&mut w, &d, 0, &hit, source);
    w.hits.clear();
    assert_eq!(f.mov.as_ref().unwrap().slot, "uspec");
    f = w.fighters[0].clone();
    assert_eq!(f.mov.as_ref().unwrap().slot, "uspec", "armor preserves the ride");
    // Riding to the end of the hang detaches the Cypher as an upward 6% projectile.
    let mut peeled = false;
    for _ in 0..80 {
        tick(&mut w, &d, Input::default());
        if w.projs.iter().any(|p| p.id == "cypherShot") {
            peeled = true;
            break;
        }
    }
    assert!(peeled, "Cypher should detach into its own hitbox");
    let shot = w.projs.iter().find(|p| p.id == "cypherShot").unwrap();
    assert!(shot.vy < 0., "the detached Cypher flies upward");
    assert_eq!(shot.hit.damage, 6.0);
}

#[test]
fn raw_inputs_route_each_slot() {
    let d = common::data();
    let special = |stick: (i8, i8), up: bool| sim::input::Intent {
        sp: true,
        sp_held: true,
        x: f64::from(stick.0) / 127.,
        y: f64::from(stick.1) / 127.,
        left: stick.0 < 0,
        right: stick.0 > 0,
        up,
        down: stick.1 < 0,
        ..Default::default()
    };
    for (intent, slot) in [
        (special((0, 0), false), "nspec"),
        (special((127, 0), false), "sspec"),
        (special((0, -127), false), "dspec"),
        (special((0, 127), true), "uspec"),
    ] {
        let mut w = setup(&d, &["sneak", "brawler"]);
        let mut f = w.fighters[0].clone();
        sim::moves::ground_actions(&mut w, &d, 0, &mut f, intent, intent.dir());
        w.fighters[0] = f;
        assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some(slot));
    }
    // Attack press on the ground starts the jab, a held direction the forward tilt.
    let mut w = setup(&d, &["sneak", "brawler"]);
    tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some("jab"));
    // Jump + attack is the short-hop forward air.
    let mut w = setup(&d, &["sneak", "brawler"]);
    tick(&mut w, &d, Input { buttons: 1 | 2, stick: (127, 0), ..Default::default() });
    for _ in 0..6 {
        tick(&mut w, &d, Input { buttons: 2, stick: (127, 0), ..Default::default() });
    }
    assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some("fair"));
}

#[test]
fn up_smash_shell_arcs_and_explodes_where_it_lands() {
    let d = common::data();
    let mut w = start(&d, "usmash");
    let mut spawned = false;
    for _ in 0..45 {
        tick(&mut w, &d, Input::default());
        if w.projs.iter().any(|p| p.id == "mortarShell") {
            spawned = true;
            break;
        }
    }
    assert!(spawned, "mortar shell should spawn on source frame 35");
    let shell = w.projs[0].clone();
    assert!(shell.vy < 0., "fired upward");
    let y0 = shell.y;
    for _ in 0..20 {
        tick(&mut w, &d, Input::default());
    }
    assert!(w.projs[0].y < y0 - 60., "the shell arcs upward first");
    // It comes down, detonates on the floor and hits a target standing where it lands.
    let mut landed = None;
    for _ in 0..120 {
        tick(&mut w, &d, Input::default());
        if let Some(p) = w.projs.first() {
            if p.bounces > 0 && landed.is_none() {
                landed = Some((p.x, p.y));
                w.fighters[1].x = p.x;
                w.fighters[1].y = 520.;
                w.fighters[1].ground = true;
                w.fighters[1].dmg = 0.;
            }
        } else {
            break;
        }
        w.hits.clear();
    }
    let (x, _) = landed.expect("shell should reach the floor");
    assert!((x - w.fighters[0].x).abs() > 100., "shell travels forward: {x}");
    assert!((w.fighters[1].dmg - 14.0).abs() < 1e-6, "shell blast damage {}", w.fighters[1].dmg);
    assert!(w.fighters[0].dmg == 0., "the shell does not hurt its owner");
}

#[test]
fn cypher_climbs_far_enough_to_recover() {
    let d = common::data();
    let mut w = start(&d, "uspec");
    let y0 = w.fighters[0].y;
    let mut peak = y0;
    for _ in 0..90 {
        tick(&mut w, &d, Input::default());
        peak = peak.min(w.fighters[0].y);
    }
    assert!(y0 - peak > 230., "Cypher should climb over 230 px: {}", y0 - peak);
    // Offstage and below the ledge, the rise catches the ledge while drifting in.
    let mut w = setup(&d, &["sneak", "brawler"]);
    w.fighters[0].x = 1035.;
    w.fighters[0].y = 585.;
    w.fighters[0].face = -1.;
    w.fighters[0].ground = false;
    w.fighters[0].surf = None;
    w.fighters[0].set_state(State::Air);
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "uspec");
    w.fighters[0] = f;
    let mut caught = false;
    for _ in 0..100 {
        tick(&mut w, &d, Input { stick: (-127, 0), ..Default::default() });
        if w.fighters[0].state == State::Ledge {
            caught = true;
            break;
        }
    }
    assert!(
        caught,
        "Cypher should be able to grab the ledge: {:?} {:?}",
        w.fighters[0].state,
        (w.fighters[0].x, w.fighters[0].y)
    );
}

#[test]
fn nair_and_dair_multihit_then_finish_on_the_same_target() {
    let d = common::data();
    for (slot, hits) in [("nair", vec![3.0, 3.0, 3.0, 12.0]), ("dair", vec![4.0, 3.0, 3.0, 10.0])] {
        let mut w = setup(&d, &["sneak", "brawler"]);
        // Both fighters hang in the air inside the move's reach; the victim is pinned so only the move matters.
        w.fighters[0].y = 320.;
        w.fighters[0].ground = false;
        w.fighters[0].surf = None;
        w.fighters[0].set_state(State::Air);
        w.fighters[1].x = 660.;
        w.fighters[1].y = 320.;
        w.fighters[1].ground = false;
        w.fighters[1].surf = None;
        w.fighters[1].set_state(State::Air);
        w.fighters[1].dmg = 0.;
        let mut f = w.fighters[0].clone();
        moves::start_move(&mut w, &d, 0, &mut f, slot);
        w.fighters[0] = f;
        let mut seen = vec![];
        let mut last = 0.;
        for _ in 0..60 {
            w.fighters[0].y = 320.;
            w.fighters[0].vy = 0.;
            w.fighters[1].x = 660.;
            w.fighters[1].y = 320.;
            w.fighters[1].vx = 0.;
            w.fighters[1].vy = 0.;
            tick(&mut w, &d, Input::default());
            let dmg = w.fighters[1].dmg;
            if dmg > last + 1e-6 {
                seen.push((dmg - last).round());
                last = dmg;
            }
        }
        assert_eq!(seen, hits, "{slot} should land every strike on one victim: {seen:?}");
        assert!((last - hits.iter().sum::<f64>()).abs() < 1e-6, "{slot} total damage {last}");
    }
}

#[test]
fn back_throw_launches_behind_and_up_throw_keeps_both_hits() {
    let d = common::data();
    // The engine launches a back throw along `-face`: the source's backwards angle must not be reversed twice.
    let mut w = setup(&d, &["sneak", "brawler"]);
    combat::start_hold(&mut w, &d, 0, 1);
    {
        let f = &mut w.fighters[0];
        f.set_state(State::Throw);
        f.sd.victim = Some(1);
        f.sd.kind = "b".into();
        f.t = d.fighter("sneak").unwrap().throws.b.release - 1;
        f.lag = 0;
    }
    w.fighters[1].lag = 0;
    w.fighters[1].dmg = 0.;
    step_world(&mut w, &d, &[]);
    w.hits.clear();
    let vx = w.fighters[1].vx;
    assert!(vx < -1., "back throw must send the victim behind a right-facing thrower: vx {vx}");
    assert!((w.fighters[1].dmg - 9.0).abs() < 1e-6);
    // Up throw: the 7% preliminary tick, then the 4% release.
    let mut w = setup(&d, &["sneak", "brawler"]);
    combat::start_hold(&mut w, &d, 0, 1);
    {
        let f = &mut w.fighters[0];
        f.set_state(State::Throw);
        f.sd.victim = Some(1);
        f.sd.kind = "u".into();
        f.t = d.fighter("sneak").unwrap().throws.u.ticks[0].at - 1;
        f.lag = 0;
    }
    w.fighters[1].lag = 0;
    w.fighters[1].dmg = 0.;
    let mut total = 0.;
    for _ in 0..6 {
        w.fighters[1].lag = 0;
        step_world(&mut w, &d, &[]);
        w.hits.clear();
        if (w.fighters[1].dmg - total).abs() > 1e-6 {
            total = w.fighters[1].dmg;
        }
    }
    assert!((total - 11.0).abs() < 1e-6, "up throw 7% + 4%, got {total}");
    assert!(w.fighters[1].vy < 0., "up throw launches upward");
}

#[test]
fn grenade_cooking_shortens_the_fuse() {
    let d = common::data();
    for held in [1usize, 40, 100] {
        let mut w = start(&d, "nspec");
        for _ in 0..20 {
            step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
        }
        // Pull done: the next tick enters the hold, then each held tick cooks.
        step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
        for _ in 0..held {
            step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
        }
        let charge = w.fighters[0].mov.as_ref().unwrap().charge;
        assert_eq!(charge, held as u32, "hold should count cooking ticks");
        step_move(&mut w, &d, sim::input::Intent::default());
        for _ in 0..10 {
            step_move(&mut w, &d, sim::input::Intent::default());
        }
        let p = w.projs.first().expect("grenade thrown");
        let remaining = p.life;
        // life = 129 - cooked ticks (clamped to at least one tick).
        let expected = (129i64 - held as i64).max(1);
        assert!(
            (i64::from(remaining) - expected).abs() <= 1,
            "held {held}: life {remaining}, expected about {expected}"
        );
    }
    // Holding past the source's 2.5 s fuse leaves a grenade that detonates at once instead of never.
    let mut w = start(&d, "nspec");
    for _ in 0..170 {
        step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
    }
    step_move(&mut w, &d, sim::input::Intent::default());
    for _ in 0..9 {
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    assert!(w.projs.first().is_some_and(|p| p.life <= 2), "overcooked grenade should be about to blow");
}

#[test]
fn c4_switch_fires_only_the_remote_gadget() {
    let d = common::data();
    let mut w = setup(&d, &["sneak", "brawler"]);
    let f = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &f, "c4");
    sim::projectiles::spawn(&mut w, &d, 0, &f, "grenadeMid");
    assert_eq!(w.projs.len(), 2);
    // A planted C4 leaves the pool empty, which selects the detonate timeline.
    w.fighters[0].charges[0] = 0;
    // The planted C4's blast should not touch a live grenade's fuse.
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "dspec");
    assert_eq!(f.mov.as_ref().unwrap().variant, 0, "detonate timeline");
    w.fighters[0] = f;
    for _ in 0..30 {
        tick(&mut w, &d, Input::default());
    }
    assert_eq!(w.projs.len(), 1, "only the C4 explodes: {:?}", w.projs.iter().map(|p| &p.id).collect::<Vec<_>>());
    assert_eq!(w.projs[0].id, "grenadeMid");
}

#[test]
fn nikita_contact_is_the_strong_explosion_at_any_distance() {
    for distance in [0., 25., 50.] {
        let d = common::data();
        let mut w = setup(&d, &["sneak", "brawler"]);
        let hold = Input { buttons: 4, ..Default::default() };
        tick(&mut w, &d, Input { buttons: 4, stick: (127, 0), ..Default::default() });
        ticks(&mut w, &d, hold, 41);
        tick(&mut w, &d, Input::default());
        let p = w.projs[0].clone();
        w.fighters[1].x = p.x + p.vx * 3. + distance;
        w.fighters[1].y = p.y;
        w.fighters[1].ground = false;
        w.fighters[1].surf = None;
        w.fighters[1].dmg = 0.;
        ticks(&mut w, &d, Input::default(), 12);
        assert!(
            (w.fighters[1].dmg - 14.0).abs() < 1e-6,
            "distance {distance}: expected the 14% blast, got {}",
            w.fighters[1].dmg
        );
    }
}

#[test]
fn raw_inputs_arm_and_fire_the_gadgets() {
    let d = common::data();
    // Hold special, then release forward for the strong throw.
    let mut w = setup(&d, &["sneak", "brawler"]);
    tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
    ticks(&mut w, &d, Input { buttons: 4, ..Default::default() }, 24);
    tick(&mut w, &d, Input { stick: (127, 0), ..Default::default() });
    ticks(&mut w, &d, Input::default(), 14);
    assert!(
        w.projs.iter().any(|p| p.id == "grenadeStrong"),
        "raw hold + forward release should throw the strong arc: {:?}",
        w.projs.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
    // Down special plants, the second press fires.
    let mut w = setup(&d, &["sneak", "brawler"]);
    tick(&mut w, &d, Input { buttons: 4, stick: (0, -127), ..Default::default() });
    for _ in 0..30 {
        tick(&mut w, &d, Input::default());
    }
    assert_eq!(w.projs.iter().filter(|p| p.id == "c4").count(), 1, "raw down special plants a C4");
    tick(&mut w, &d, Input { buttons: 4, stick: (0, -127), ..Default::default() });
    for _ in 0..34 {
        tick(&mut w, &d, Input::default());
    }
    assert!(w.projs.is_empty(), "the second press detonates it");
}

#[test]
fn nikita_auto_releases_and_an_interrupted_hold_drops_the_grenade() {
    let d = common::data();
    let mut w = setup(&d, &["sneak", "brawler"]);
    tick(&mut w, &d, Input { buttons: 4, stick: (127, 0), ..Default::default() });
    let hold = Input { buttons: 4, ..Default::default() };
    ticks(&mut w, &d, hold, 200);
    assert_eq!(w.projs.len(), 1);
    assert!(w.projs[0].released.is_some(), "the missile auto-releases at fuel end");
    assert_eq!(phase_name(&d, &w).as_deref(), Some("fire"));
    // A hit during the grenade hold interrupts it without spawning anything.
    let mut w = start(&d, "nspec");
    for _ in 0..24 {
        step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
    }
    assert_eq!(phase_name(&d, &w).as_deref(), Some("hold"));
    let hit = d.fighter("brawler").unwrap().moves.jab.phases[0].hitboxes[0].clone();
    let point = (w.fighters[0].x, w.fighters[0].y);
    let source = combat::HitSource { attacker: 1, direction: 1., point, projectile: false, label: None };
    combat::apply_hit(&mut w, &d, 0, &hit, source);
    w.hits.clear();
    assert!(w.fighters[0].mov.is_none(), "the hit cancels the hold");
    assert!(w.projs.is_empty(), "an interrupted hold throws nothing");
}

#[test]
fn throws_use_the_source_release_frames_and_damage() {
    let d = common::data();
    let f = d.fighter("sneak").unwrap();
    for (throw, release, total, damage) in
        [(&f.throws.f, 20, 39, 9.0), (&f.throws.b, 19, 30, 9.0), (&f.throws.u, 22, 47, 4.0), (&f.throws.d, 34, 41, 9.0)]
    {
        assert_eq!(throw.release, release);
        assert_eq!(throw.total, total);
        assert!((throw.hit.damage - damage).abs() < 1e-6, "throw damage {}", throw.hit.damage);
    }
    assert!((f.stats.pummel_damage.unwrap() - 1.3).abs() < 1e-6);
    // Up throw's preliminary hit is a throw tick, not extra release damage.
    assert_eq!(f.throws.u.ticks.len(), 1);
    assert_eq!(f.throws.u.ticks[0].at, 21);
    assert!((f.throws.u.ticks[0].damage - 7.0).abs() < 1e-6);
    // Back throw is stored facing-relative: the engine flips it.
    assert!((f.throws.b.hit.angle - 40.0).abs() < 1e-6);
    // The airborne half of the forward tilt keeps its own angle and base knockback.
    let knee = &f.moves.ftilt.phases[0].hitboxes;
    assert!(knee.iter().any(|h| h.situation == sim::data::hitbox::Situation::Ground && h.angle == 78.));
    assert!(knee.iter().any(|h| h.situation == sim::data::hitbox::Situation::Air && h.angle == 65. && h.bkb == 20.));
}
