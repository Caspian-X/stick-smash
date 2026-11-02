mod common;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World};

fn world(d: &GameData, fighter: &str, opponents: usize) -> World {
    let mut w = common::fighters(d, fighter, &vec!["brawler"; opponents]);
    w.count = 0;
    for f in &mut w.fighters {
        f.inv = 0;
    }
    w
}

#[test]
fn held_arrow_scales_speed_and_damage_and_keeps_its_arc() {
    let d = common::data();
    let fire = |held| {
        let mut w = world(&d, "archer", 0);
        for t in 0..100 {
            common::step_preserving(
                &mut w,
                &d,
                &[Input { buttons: if t < held { 4 } else { 0 }, ..Default::default() }],
            );
            if let Some(p) = w.projs.first() {
                return p.clone();
            }
        }
        panic!("arrow never spawned");
    };
    let quick = fire(1);
    let charged = fire(40);
    assert!(charged.vx > quick.vx * 1.5);
    assert!(charged.hit.damage > quick.hit.damage * 1.5);
    let mut w = world(&d, "archer", 0);
    w.projs.push(charged);
    let vy = w.projs[0].vy;
    sim::projectiles::step_projectiles(&mut w, &d);
    assert!(w.projs[0].vy > vy, "gravity bends the arrow down");
}

#[test]
fn fuse_bomb_bounces_then_blasts_multiple_opponents_once() {
    let d = common::data();
    let mut w = world(&d, "tinker", 2);
    let f = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &f, "bomb");
    for f in &mut w.fighters[1..] {
        f.x = -1000.;
    }
    for _ in 0..45 {
        sim::projectiles::step_projectiles(&mut w, &d);
    }
    assert!(w.projs[0].bounces > 0);
    assert!(w.fighters.iter().all(|f| f.dmg == 0.), "no pre-fuse contact damage");
    let p = w.projs[0].clone();
    for f in &mut w.fighters[1..] {
        f.x = p.x;
        f.y = p.y + 30.;
    }
    w.projs[0].life = 1;
    sim::projectiles::step_projectiles(&mut w, &d);
    assert!(w.projs.is_empty());
    assert!(w.fighters[1..].iter().all(|f| f.dmg == p.hit.damage));
    assert_eq!(w.fighters[0].dmg, 0.);
}

#[test]
fn proximity_mine_arms_limits_ownership_and_replays_from_a_clone() {
    let d = common::data();
    let mut w = world(&d, "tinker", 1);
    let f = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &f, "mine");
    sim::projectiles::spawn(&mut w, &d, 0, &f, "mine");
    assert_eq!(w.projs.len(), 1);
    for f in &mut w.fighters[1..] {
        f.x = w.projs[0].x;
        f.y = w.projs[0].y + 30.;
    }
    for _ in 0..29 {
        sim::projectiles::step_projectiles(&mut w, &d);
    }
    assert_eq!(w.projs.len(), 1);
    let p = &w.projs[0];
    w.fighters[1].x = p.x;
    w.fighters[1].y = p.y + 30.;
    let mut replay = w.clone();
    sim::projectiles::step_projectiles(&mut w, &d);
    sim::projectiles::step_projectiles(&mut replay, &d);
    assert!(w.projs.is_empty());
    assert!(w.fighters[1].dmg > 0.);
    assert_eq!(serde_json::to_string(&w).unwrap(), serde_json::to_string(&replay).unwrap());
}

#[test]
fn cpu_tactics_can_choose_neutral_with_a_distinct_side_special() {
    let mut d = common::data();
    d.global.ai.aggression = [1.; 3];
    let mut w = world(&d, "archer", 1);
    w.fighters[0].x = 450.;
    w.fighters[1].x = 800.;
    let data = w.fighters[0].data;
    d.fighters[data].ai.specials[0].chance = 1.;
    for neutral in [false, true] {
        let mut w = w.clone();
        w.fighters[0].ai.cool = 0.;
        d.fighters[data].ai.specials[0].neutral = neutral;
        let inp = sim::ai::ai_input(&mut w, &d, 0, 2);
        assert!(inp.sp);
        assert_eq!(inp.right, !neutral);
        assert!(!inp.left && !inp.up && !inp.down);
    }
}

#[test]
fn blasts_push_hits_and_shields_away_with_motion_and_facing_fallbacks() {
    let d = common::data();
    for shield in [false, true] {
        for (vx, offset, expected) in
            [(0., -20., -1.), (0., 20., 1.), (5., -20., -1.), (5., 20., 1.), (5., 0., 1.), (0., 0., -1.)]
        {
            let mut w = world(&d, "tinker", 1);
            w.fighters[0].face = -1.;
            let f = w.fighters[0].clone();
            sim::projectiles::spawn(&mut w, &d, 0, &f, "mine");
            let p = &mut w.projs[0];
            p.x = 600. - vx;
            p.y = 400.;
            p.vx = vx;
            p.vy = 0.;
            p.life = 1;
            let f = &mut w.fighters[1];
            f.x = 600. + offset;
            f.y = 430.;
            f.ground = false;
            f.set_state(if shield { sim::world::State::Shield } else { sim::world::State::Air });
            sim::projectiles::step_projectiles(&mut w, &d);
            assert_eq!(sim::world::sign(w.fighters[1].vx), expected, "shield {shield}, vx {vx}, offset {offset}");
            assert_eq!(w.fighters[1].dmg > 0., !shield);
        }
    }
}

#[test]
fn counter_catches_a_blast_without_reflecting_or_protecting_other_victims() {
    let d = common::data();
    let mut w = world(&d, "tinker", 2);
    w.fighters[1].data = d.fighters.iter().position(|f| f.id == "swordfighter").unwrap();
    let mut f = w.fighters[1].clone();
    sim::moves::start_move(&mut w, &d, 1, &mut f, "dspec");
    f.mov.as_mut().unwrap().t = 5;
    w.fighters[1] = f;
    let f = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &f, "bomb");
    w.projs[0].life = 1;
    w.projs[0].x = 600.;
    w.projs[0].y = 400.;
    w.projs[0].vx = 0.;
    for f in &mut w.fighters[1..] {
        f.x = 600.;
        f.y = 430.;
    }
    sim::projectiles::step_projectiles(&mut w, &d);
    assert!(w.projs.is_empty());
    assert_eq!(w.fighters[1].dmg, 0.);
    assert!(w.fighters[1].inv > 0);
    assert_eq!(w.fighters[1].mov.as_ref().unwrap().phase, 1);
    assert!(w.fighters[2].dmg > 0.);
}

#[test]
fn shortened_life_does_not_arm_a_bomb_early() {
    let d = common::data();
    let mut w = world(&d, "tinker", 1);
    let f = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &f, "bomb");
    w.projs[0].life = 51;
    w.projs[0].x = 600.;
    w.projs[0].y = 400.;
    w.projs[0].vx = 0.;
    w.projs[0].vy = 0.;
    w.fighters[1].x = 600.;
    w.fighters[1].y = 430.;
    for _ in 0..9 {
        sim::projectiles::step_projectiles(&mut w, &d);
    }
    assert_eq!(w.projs[0].age, 9);
    assert_eq!(w.fighters[1].dmg, 0.);
    w.projs[0].life = 100;
    sim::projectiles::step_projectiles(&mut w, &d);
    assert!(w.projs.is_empty());
    assert!(w.fighters[1].dmg > 0.);
}

#[test]
fn settled_mines_ride_vertical_and_horizontal_lifts_without_extra_bounces() {
    let mut d = common::data();
    let data = d.fighters.iter().position(|f| f.id == "tinker").unwrap();
    d.fighters[data].projectiles.get_mut("mine").unwrap().bounce.as_mut().unwrap().max_bounces = 1;
    for platform in [0, 2] {
        let mut w = World::new(
            MatchConfig::sandbox("foundry", vec![PlayerConfig::human("tinker")], MatchConfig::default().seed),
            &d,
        )
        .unwrap();
        let f = w.fighters[0].clone();
        sim::projectiles::spawn(&mut w, &d, 0, &f, "mine");
        let surface = d.stages[w.stage].blocks.len() + platform;
        let (x1, x2, y) = sim::physics::surface(&d, &w, surface);
        let p = &mut w.projs[0];
        p.x = (x1 + x2) / 2.;
        p.y = y - p.r;
        p.vx = 0.;
        p.vy = 0.;
        p.surf = Some(surface);
        let start = (p.x, p.y);
        for _ in 0..120 {
            w.frame += 1;
            sim::projectiles::step_projectiles(&mut w, &d);
            let p = &w.projs[0];
            let (x1, x2, y) = sim::physics::surface(&d, &w, surface);
            assert!((p.x - (x1 + x2) / 2.).abs() < 1e-9);
            assert!((p.y + p.r - y).abs() < 1e-9);
            assert_eq!(p.bounces, 0);
        }
        assert_ne!(start, (w.projs[0].x, w.projs[0].y));
        assert_eq!(w.projs[0].age, 120);
    }
}

#[test]
fn batting_resets_fuse_and_support_but_preserves_arming_age() {
    let mut d = common::data();
    let bat = d.fighter("king").unwrap().projectiles["gordo"].bat.clone();
    let data = d.fighters.iter().position(|f| f.id == "tinker").unwrap();
    d.fighters[data].projectiles.get_mut("mine").unwrap().bat = bat;
    let mut w = world(&d, "tinker", 0);
    let f = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &f, "mine");
    w.projs[0].age = 17;
    w.projs[0].life = 50;
    w.projs[0].surf = Some(0);
    let p = &w.projs[0];
    let bx = sim::combat::ActiveBox { hit: d.fighters[data].moves.jab.phases[0].hitboxes[0].clone(), x: p.x, y: p.y };
    sim::projectiles::bat(&mut w, &d, 0, &[bx]);
    assert_eq!(w.projs[0].age, 17);
    assert_eq!(w.projs[0].life, 120);
    assert_eq!(w.projs[0].surf, None);
}
