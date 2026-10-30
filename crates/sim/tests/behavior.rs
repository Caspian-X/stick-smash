mod common;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, combat, moves, step_world, world::State};
fn world(d: &GameData, id: &str) -> World {
    World::new(MatchConfig { training: true, players: vec![PlayerConfig::human(id), PlayerConfig::human(id)], ..Default::default() }, d)
}
fn run(w: &mut World, d: &GameData, n: usize, inputs: &[Input]) {
    for _ in 0..n {
        step_world(w, d, inputs);
        w.fx.clear();
    }
}
fn press(w: &mut World, d: &GameData, stick: (i8, i8), buttons: u8) {
    run(w, d, 1, &[Input { stick, buttons }]);
}
#[test]
fn jab_hits_dummy_and_records_advantage() {
    let d = common::data();
    let mut w = world(&d, "brawler");
    w.fighters[1].x = w.fighters[0].x + 40.;
    run(&mut w, &d, 1, &[]);
    press(&mut w, &d, (0, 0), 2);
    assert_eq!(w.fighters[0].state, State::Attack);
    run(&mut w, &d, 10, &[]);
    let jab = &d.fighter("brawler").unwrap().moves.jab.phases[0].hitboxes[0];
    assert_eq!(w.fighters[1].dmg, jab.damage);
    assert!(matches!(w.fighters[1].state, State::Hitstun | State::Idle));
    assert_eq!(w.trace.as_ref().unwrap().label, "Jab");
    run(&mut w, &d, 80, &[]);
    let tr = w.trace.unwrap();
    assert!(tr.done && tr.a_free >= 0 && tr.d_free >= 0);
}
#[test]
fn all_hitboxes_fit_phase_windows() {
    let d = common::data();
    for f in &d.fighters {
        for (slot, m) in f.moves.iter() {
            for p in &m.phases {
                for b in &p.hitboxes {
                    if let Some(end) = b.end {
                        assert!(b.start <= end, "{}.{}.{}", f.id, slot, p.name);
                        if let Some(total) = p.frames {
                            assert!(end <= total, "{}.{}.{}", f.id, slot, p.name);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn charge_scales_active_damage() {
    let d = common::data();
    let mut w = world(&d, "brawler");
    w.fighters[1].x = 2000.;
    press(&mut w, &d, (127, 0), 2);
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "fsmash");
    run(&mut w, &d, 40, &[Input { stick: (0, 0), buttons: 2 }]);
    assert!(w.fighters[0].mov.as_ref().unwrap().charge > 25);
    let mut max: f64 = 0.;
    for _ in 0..20 {
        run(&mut w, &d, 1, &[]);
        for b in combat::active_boxes(&d, &w.fighters[0]) {
            max = max.max(b.hit.damage);
        }
    }
    assert!(max > d.fighter("brawler").unwrap().moves.fsmash.phases[0].hitboxes[0].damage);
}
#[test]
fn short_and_full_hops_decided_at_takeoff() {
    let d = common::data();
    let apex = |held: usize| {
        let mut w = world(&d, "brawler");
        w.fighters[1].x = 2000.;
        let y = w.fighters[0].y;
        let mut top = y;
        for n in 0..90 {
            press(&mut w, &d, (0, 0), u8::from(n < held));
            top = top.min(w.fighters[0].y);
        }
        y - top
    };
    let short = apex(1);
    let full = apex(90);
    assert!(full > short * 1.5);
    assert!((apex(d.global.movement.jumpsquat as usize + 2) - full).abs() < 1e-5);
    assert!((apex(d.global.movement.jumpsquat as usize - 1) - short).abs() < 1e-5);
}
#[test]
fn ledge_half_circle_and_occupancy() {
    let d = common::data();
    let s = &d.stages[0];
    let setup = |offset: f64| {
        let mut w = world(&d, "brawler");
        w.fighters[1].x = 2000.;
        let f = &mut w.fighters[0];
        f.x = s.ground.right + offset;
        f.y = s.ground.top - 20.;
        f.vy = 2.;
        f.ground = false;
        f.surf = None;
        f.state = State::Air;
        w
    };
    let mut w = setup(30.);
    for _ in 0..40 {
        run(&mut w, &d, 1, &[]);
        if w.fighters[0].state == State::Ledge {
            break;
        }
    }
    assert_eq!(w.fighters[0].state, State::Ledge);
    let mut far = setup(s.ledge.r + 10.);
    run(&mut far, &d, 40, &[]);
    assert_ne!(far.fighters[0].state, State::Ledge);
    let mut cfg = MatchConfig { training: true, ..Default::default() };
    cfg.players = vec![PlayerConfig::human("brawler"); 4];
    let mut w = World::new(cfg, &d);
    w.fighters[3].set_state(State::Ledge);
    w.fighters[3].sd.side = 1.;
    let mut f = w.fighters[0].clone();
    f.x = s.ground.right + 30.;
    f.y = s.ground.top + s.ledge.hand_y + s.ledge.cy;
    f.ground = false;
    f.vy = 1.;
    sim::physics::try_ledge(&mut w, &d, 0, &mut f, Default::default(), false);
    assert_ne!(f.state, State::Ledge);
}
#[test]
fn special_routing_and_b_reverse() {
    let d = common::data();
    for (id, slot) in [("swordfighter", "sspec"), ("brawler", "nspec")] {
        let mut w = world(&d, id);
        press(&mut w, &d, (-127, 0), 4);
        assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, slot);
        assert_eq!(w.fighters[0].face, -1.);
    }
    let mut w = world(&d, "swordfighter");
    press(&mut w, &d, (0, 0), 4);
    press(&mut w, &d, (-127, 0), 0);
    assert_eq!(w.fighters[0].face, -1.);
    let mut w = world(&d, "swordfighter");
    press(&mut w, &d, (0, 0), 4);
    run(&mut w, &d, d.global.movement.b_reverse_window as usize + 1, &[]);
    press(&mut w, &d, (-127, 0), 0);
    assert_eq!(w.fighters[0].face, 1.);
    let mut w = world(&d, "swordfighter");
    let f = &mut w.fighters[0];
    f.ground = false;
    f.surf = None;
    f.y -= 200.;
    f.vx = 3.;
    f.state = State::Air;
    press(&mut w, &d, (-127, 0), 4);
    assert_eq!(w.fighters[0].face, -1.);
    assert!(w.fighters[0].vx < 0.);
    let mut w = world(&d, "swordfighter");
    press(&mut w, &d, (-127, 127), 4);
    assert_eq!(w.fighters[0].face, -1.);
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "uspec");
}
#[test]
fn every_cpu_matchup_is_deterministic_and_deals_damage() {
    let d = common::data();
    for a in &d.fighters {
        for b in &d.fighters {
            let mut cfg = MatchConfig { seed: 7, ..Default::default() };
            cfg.players = vec![
                PlayerConfig { cpu: Some(2), ..PlayerConfig::human(&a.id) },
                PlayerConfig { cpu: Some(2), ..PlayerConfig::human(&b.id) },
            ];
            let mut w = World::new(cfg, &d);
            let mut copy = w.clone();
            for _ in 0..6000 {
                step_world(&mut w, &d, &[]);
                step_world(&mut copy, &d, &[]);
                w.fx.clear();
                copy.fx.clear();
                if w.over {
                    break;
                }
            }
            assert_eq!(w, copy, "{} vs {}", a.id, b.id);
            assert!(w.fighters.iter().any(|f| f.dmg > 0. || f.stocks < 3));
        }
    }
}
#[test]
fn four_player_cpu_and_teams_rollback() {
    let d = common::data();
    for teams in [false, true] {
        for ff in [false, true] {
            let players = ["brawler", "king", "swordfighter", "king"]
                .into_iter()
                .enumerate()
                .map(|(i, id)| PlayerConfig { cpu: Some(2), team: teams.then_some((i % 2) as u8), ..PlayerConfig::human(id) })
                .collect();
            let mut w = World::new(MatchConfig { seed: 42, training: true, players, friendly_fire: ff, ..Default::default() }, &d);
            let mut initial = w.clone();
            run(&mut w, &d, 3000, &[]);
            let mut rollback = w.clone();
            for _ in 0..3000 {
                step_world(&mut w, &d, &[]);
                step_world(&mut rollback, &d, &[]);
                w.fx.clear();
                rollback.fx.clear();
                assert_eq!(w, rollback);
            }
            run(&mut initial, &d, 6000, &[]);
            assert_eq!(w, initial);
            assert!(w.fighters.iter().any(|f| f.dmg > 0. || f.stocks < 3));
            assert!(w.fighters.iter().all(|f| f.x.is_finite() && f.y.is_finite()));
        }
    }
}
#[test]
fn friendly_fire_filters_melee_grabs_projectiles_and_cancels() {
    let d = common::data();
    for ff in [false, true] {
        let mut cfg = MatchConfig { training: true, friendly_fire: ff, ..Default::default() };
        cfg.players = vec![
            PlayerConfig { team: Some(0), ..PlayerConfig::human("brawler") },
            PlayerConfig { team: Some(0), ..PlayerConfig::human("brawler") },
            PlayerConfig { team: Some(1), ..PlayerConfig::human("king") },
            PlayerConfig { team: Some(1), ..PlayerConfig::human("king") },
        ];
        let mut w = World::new(cfg.clone(), &d);
        w.fighters[1].x = w.fighters[0].x + 40.;
        w.fighters[2].x = 1100.;
        w.fighters[3].x = 1200.;
        press(&mut w, &d, (0, 0), 2);
        run(&mut w, &d, 12, &[]);
        assert_eq!(w.fighters[1].dmg > 0., ff);
        let mut w = World::new(cfg.clone(), &d);
        w.fighters[1].x = w.fighters[0].x + 35.;
        w.fighters[2].x = 1100.;
        w.fighters[3].x = 1200.;
        press(&mut w, &d, (0, 0), 8);
        run(&mut w, &d, 8, &[]);
        assert_eq!(w.fighters[1].state == State::Grabbed, ff);
        let mut w = World::new(cfg.clone(), &d);
        let f = w.fighters[0].clone();
        let id = d.fighters[f.data].projectiles.keys().next().unwrap();
        sim::projectiles::spawn(&mut w, &d, 0, &f, id);
        let p = &mut w.projs[0];
        p.x = w.fighters[1].x;
        p.y = w.fighters[1].y - 40.;
        p.vx = 0.;
        sim::projectiles::step_projectiles(&mut w, &d);
        assert_eq!(w.fighters[1].dmg > 0., ff);
        let mut w = World::new(cfg, &d);
        for i in 0..2 {
            let f = w.fighters[i].clone();
            sim::projectiles::spawn(&mut w, &d, i, &f, id);
        }
        for p in &mut w.projs {
            p.x = 650.;
            p.y = 100.;
            p.vx = 0.;
        }
        sim::projectiles::step_projectiles(&mut w, &d);
        assert_eq!(w.projs.is_empty(), ff);
    }
}
#[test]
fn ai_target_is_nearest_enemy_and_ties_use_index() {
    let d = common::data();
    let mut cfg = MatchConfig { training: true, friendly_fire: true, ..Default::default() };
    cfg.players = vec![PlayerConfig::human("brawler"); 4];
    cfg.players[0].team = Some(0);
    cfg.players[1].team = Some(0);
    let mut w = World::new(cfg, &d);
    for f in &mut w.fighters {
        f.x = 640.;
        f.y = 520.;
    }
    w.fighters[2].x = 600.;
    w.fighters[3].x = 680.;
    assert_eq!(sim::ai::target(&w, 0), Some(2));
    w.fighters[2].state = State::Dead;
    assert_eq!(sim::ai::target(&w, 0), Some(3));
}
#[test]
fn winner_is_last_team_and_training_never_ends() {
    let d = common::data();
    let mut cfg = MatchConfig { stocks: 1, ..Default::default() };
    cfg.players = (0..4).map(|i| PlayerConfig { team: Some((i % 2) as u8), ..PlayerConfig::human("brawler") }).collect();
    let mut w = World::new(cfg, &d);
    w.count = 0;
    w.fighters[1].x = -1000.;
    run(&mut w, &d, 1, &[]);
    assert_eq!(w.end_t, 0);
    w.fighters[3].x = -1000.;
    run(&mut w, &d, 1, &[]);
    assert!(w.end_t > 0);
    run(&mut w, &d, d.global.match_.end_frames as usize, &[]);
    assert!(w.over);
    assert_eq!(w.fighters[0].state, State::Win);
    assert_eq!(w.fighters[2].state, State::Win);
    let mut w = world(&d, "brawler");
    w.fighters[0].stocks = 1;
    w.fighters[0].x = -1000.;
    run(&mut w, &d, 1, &[]);
    assert_eq!(w.fighters[0].stocks, 1);
    assert_eq!(w.fighters[0].state, State::Dead);
    assert!(!w.over && w.end_t == 0);
}
#[test]
fn phase_zero_events_transitions_and_ending_tick() {
    let mut d = common::data();
    let di = d.fighters.iter().position(|f| f.id == "brawler").unwrap();
    let m = &mut d.fighters[di].moves.jab;
    m.phases = serde_json::from_value(serde_json::json!([
        {"name":"start","frames":1,"events":[{"at":0,"setVel":{"x":10}}],"transitions":[{"on":"timeout","to":"tail"}]},
        {"name":"tail","frames":1,"airAccelMult":0,"events":[{"at":0,"setVel":{"x":20}}]}
    ]))
    .unwrap();
    let mut w = world(&d, "brawler");
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "jab");
    assert_eq!(f.vx, 10.);
    assert_eq!(f.mov.as_ref().unwrap().t, 0);
    moves::st_attack(&mut w, &d, 0, &mut f, Default::default(), 0.);
    assert_eq!(f.mov.as_ref().unwrap().phase, 1);
    assert_eq!(f.mov.as_ref().unwrap().t, 0);
    assert_eq!(f.vx, 20. * d.global.movement.attack_friction);
    let before = f.vx;
    moves::st_attack(&mut w, &d, 0, &mut f, Default::default(), 0.);
    assert_eq!(f.state, State::Idle);
    assert_eq!(f.vx, before * d.global.movement.attack_friction);
}

#[test]
fn human_history_consumes_frozen_presses() {
    let d = common::data();
    let mut w = world(&d, "brawler");
    w.count = 3;
    run(&mut w, &d, 3, &[Input { stick: (0, 0), buttons: 2 }]);
    assert_eq!(w.count, 0);
    assert_eq!(w.fighters[0].state, State::Idle);
    run(&mut w, &d, 1, &[]);
    press(&mut w, &d, (0, 0), 2);
    assert_eq!(w.fighters[0].state, State::Attack);
}

#[test]
fn a_grab_takes_one_victim_in_index_order() {
    let d = common::data();
    let cfg = MatchConfig { training: true, players: vec![PlayerConfig::human("brawler"); 4], ..Default::default() };
    let mut w = World::new(cfg, &d);
    for i in 1..4 {
        w.fighters[i].x = w.fighters[0].x + 34.;
    }
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "grab");
    f.mov.as_mut().unwrap().t = 6;
    w.fighters[0] = f;
    combat::hit_pass(&mut w, &d);
    assert_eq!(w.fighters[0].sd.victim, Some(1));
    assert_eq!(w.fighters[1].state, State::Grabbed);
    assert_eq!(w.fighters[2].state, State::Idle);
    assert_eq!(w.fighters[3].state, State::Idle);
}
