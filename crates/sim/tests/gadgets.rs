mod common;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, step_world};

fn world(d: &GameData, fighter: &str, opponents: usize) -> World {
    let players = std::iter::once(PlayerConfig::human(fighter))
        .chain((0..opponents).map(|_| PlayerConfig::human("brawler")))
        .collect();
    let mut w = World::new(MatchConfig { players, training: true, ..Default::default() }, d).unwrap();
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
            step_world(&mut w, &d, &[Input { buttons: if t < held { 4 } else { 0 }, ..Default::default() }]);
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
    for _ in 0..60 {
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
