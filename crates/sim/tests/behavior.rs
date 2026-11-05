mod common;
use common::run_draining as run;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, combat, moves, step_world, world::State};
fn world(d: &GameData, id: &str) -> World {
    common::fighters(d, id, &[id])
}
fn press(w: &mut World, d: &GameData, stick: (i8, i8), buttons: u8) {
    run(w, d, 1, &[Input { stick, buttons, ..Default::default() }]);
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
    run(&mut w, &d, 40, &[Input { stick: (0, 0), buttons: 2, ..Default::default() }]);
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
        f.x = s.blocks[0].right + offset;
        f.y = s.blocks[0].top - 20.;
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
    let mut cfg = MatchConfig::sandbox("arena", MatchConfig::default().players, MatchConfig::default().seed);
    cfg.players = vec![PlayerConfig::human("brawler"); 4];
    let mut w = World::new(cfg, &d).unwrap();
    w.fighters[3].set_state(State::Ledge);
    w.fighters[3].sd.side = 1.;
    let mut f = w.fighters[0].clone();
    f.x = s.blocks[0].right + 30.;
    f.y = s.blocks[0].top + s.ledge.hand_y + s.ledge.cy;
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
            let mut w = World::new(cfg, &d).unwrap();
            let mut copy = w.clone();
            for _ in 0..6000 {
                common::step_draining(&mut w, &d, &[]);
                common::step_draining(&mut copy, &d, &[]);
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
                .map(|(i, id)| PlayerConfig {
                    cpu: Some(2),
                    team: teams.then_some((i % 2) as u8),
                    ..PlayerConfig::human(id)
                })
                .collect();
            let mut w = World::new(MatchConfig { friendly_fire: ff, ..MatchConfig::sandbox("arena", players, 42) }, &d)
                .unwrap();
            let mut initial = w.clone();
            run(&mut w, &d, 3000, &[]);
            let mut rollback = w.clone();
            for _ in 0..3000 {
                common::step_draining(&mut w, &d, &[]);
                common::step_draining(&mut rollback, &d, &[]);
                assert_eq!(w, rollback);
            }
            run(&mut initial, &d, 6000, &[]);
            assert_eq!(w, initial);
            assert!(w.fighters.iter().any(|f| f.dmg > 0. || f.stocks < 1));
            assert!(w.fighters.iter().all(|f| f.x.is_finite() && f.y.is_finite()));
        }
    }
}
#[test]
fn friendly_fire_filters_melee_grabs_projectiles_and_cancels() {
    let d = common::data();
    for ff in [false, true] {
        let mut cfg = MatchConfig {
            friendly_fire: ff,
            ..MatchConfig::sandbox("arena", MatchConfig::default().players, MatchConfig::default().seed)
        };
        cfg.players = vec![
            PlayerConfig { team: Some(0), ..PlayerConfig::human("brawler") },
            PlayerConfig { team: Some(0), ..PlayerConfig::human("brawler") },
            PlayerConfig { team: Some(1), ..PlayerConfig::human("king") },
            PlayerConfig { team: Some(1), ..PlayerConfig::human("king") },
        ];
        let mut w = World::new(cfg.clone(), &d).unwrap();
        w.fighters[1].x = w.fighters[0].x + 40.;
        w.fighters[2].x = 1100.;
        w.fighters[3].x = 1200.;
        press(&mut w, &d, (0, 0), 2);
        run(&mut w, &d, 12, &[]);
        assert_eq!(w.fighters[1].dmg > 0., ff);
        let mut w = World::new(cfg.clone(), &d).unwrap();
        w.fighters[1].x = w.fighters[0].x + 35.;
        w.fighters[2].x = 1100.;
        w.fighters[3].x = 1200.;
        press(&mut w, &d, (0, 0), 8);
        run(&mut w, &d, 8, &[]);
        assert_eq!(w.fighters[1].state == State::Grabbed, ff);
        let mut w = World::new(cfg.clone(), &d).unwrap();
        let f = w.fighters[0].clone();
        let id = d.fighters[f.data].projectiles.keys().next().unwrap();
        sim::projectiles::spawn(&mut w, &d, 0, &f, id);
        let p = &mut w.projs[0];
        p.x = w.fighters[1].x;
        p.y = w.fighters[1].y - 40.;
        p.vx = 0.;
        sim::projectiles::step_projectiles(&mut w, &d);
        assert_eq!(w.fighters[1].dmg > 0., ff);
        let mut w = World::new(cfg, &d).unwrap();
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
    let mut cfg = MatchConfig {
        friendly_fire: true,
        ..MatchConfig::sandbox("arena", MatchConfig::default().players, MatchConfig::default().seed)
    };
    cfg.players = vec![PlayerConfig::human("brawler"); 4];
    cfg.players[0].team = Some(0);
    cfg.players[1].team = Some(0);
    let mut w = World::new(cfg, &d).unwrap();
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
    cfg.players =
        (0..4).map(|i| PlayerConfig { team: Some((i % 2) as u8), ..PlayerConfig::human("brawler") }).collect();
    let mut w = World::new(cfg, &d).unwrap();
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
    // Grounded attacks slow by the fighter's traction, doubled above walk speed (`movement.ultimate`).
    let u = d.global.movement.ultimate.as_ref().unwrap();
    let slowed = |v: f64| v - u.traction * u.fast_traction;
    assert_eq!(f.vx, slowed(20.));
    let before = f.vx;
    moves::st_attack(&mut w, &d, 0, &mut f, Default::default(), 0.);
    assert_eq!(f.state, State::Idle);
    assert_eq!(f.vx, slowed(before));
}

#[test]
fn human_history_consumes_frozen_presses() {
    let d = common::data();
    let mut w = world(&d, "brawler");
    w.count = 3;
    run(&mut w, &d, 3, &[Input { stick: (0, 0), buttons: 2, ..Default::default() }]);
    assert_eq!(w.count, 0);
    assert_eq!(w.fighters[0].state, State::Idle);
    run(&mut w, &d, 1, &[]);
    press(&mut w, &d, (0, 0), 2);
    assert_eq!(w.fighters[0].state, State::Attack);
}

#[test]
fn a_grab_takes_one_victim_in_index_order() {
    let d = common::data();
    let cfg = MatchConfig::sandbox("arena", vec![PlayerConfig::human("brawler"); 4], MatchConfig::default().seed);
    let mut w = World::new(cfg, &d).unwrap();
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

#[test]
fn committed_winner_keeps_final_stock() {
    let d = common::data();
    let mut w = World::new(MatchConfig { stocks: 1, ..Default::default() }, &d).unwrap();
    w.count = 0;
    let blast = &d.stages[w.stage].blast;
    let f = &mut w.fighters[0];
    f.x = blast.left + 10.;
    f.y = blast.top + 100.;
    f.vx = -5.;
    f.ground = false;
    f.surf = None;
    f.set_state(State::Air);
    w.fighters[1].x = blast.right + 100.;
    step_world(&mut w, &d, &[]);
    assert_eq!(w.winner, Some(0));
    assert!(w.end_t > 0);
    run(&mut w, &d, d.global.match_.end_frames as usize, &[]);
    assert_eq!(w.fighters[0].state, State::Win);
    assert_eq!(w.fighters[0].stocks, 1);
}

#[test]
fn stage_capacity_and_invalid_match_configs_return_errors() {
    let mut d = common::data();
    d.stages[0].respawns.truncate(2);
    assert_eq!(d.stages[0].max_players(), 2);
    let cfg = MatchConfig { players: vec![PlayerConfig::human("brawler"); 3], ..Default::default() };
    let err = World::new(cfg, &d).unwrap_err();
    assert!(err.contains("supports 1..=2 players, got 3"), "{err}");
    assert!(World::new(MatchConfig { players: vec![], ..Default::default() }, &d).is_err());
    assert!(World::new(MatchConfig { stage: "missing".into(), ..Default::default() }, &d).is_err());
    assert!(World::new(MatchConfig { stocks: 0, ..Default::default() }, &d).is_err());
    assert!(
        World::new(MatchConfig { players: vec![PlayerConfig::human("missing")], ..Default::default() }, &d).is_err()
    );
}

#[test]
fn spawn_support_resolves_ground_platform_and_air() {
    let mut d = common::data();
    let s = &mut d.stages[0];
    let p = &s.platforms[0];
    s.spawns[0].x = (p.x1 + p.x2) / 2.;
    s.spawns[0].y = Some(p.y);
    s.spawns[1].y = Some(s.blocks[0].top - 100.);
    let mut w = World::new(
        MatchConfig::sandbox("arena", vec![PlayerConfig::human("brawler"); 3], MatchConfig::default().seed),
        &d,
    )
    .unwrap();
    assert_eq!((w.fighters[0].state, w.fighters[0].ground, w.fighters[0].surf), (State::Idle, true, Some(1)));
    assert_eq!((w.fighters[1].state, w.fighters[1].ground, w.fighters[1].surf), (State::Air, false, None));
    assert_eq!((w.fighters[2].state, w.fighters[2].ground, w.fighters[2].surf), (State::Idle, true, Some(0)));
    let y = w.fighters[1].y;
    step_world(&mut w, &d, &[]);
    assert!(w.fighters[1].y > y);
    assert_eq!(w.fighters[0].surf, Some(1));
}

#[test]
fn one_surviving_side_wins_even_without_an_elimination() {
    let d = common::data();
    let mut cfg = MatchConfig { players: vec![], ..Default::default() };
    assert_eq!(cfg.opposing_sides(), 0);
    cfg.players = vec![PlayerConfig::human("brawler"); 4];
    assert_eq!(cfg.opposing_sides(), 4);
    for p in &mut cfg.players {
        p.team = Some(0);
    }
    assert_eq!(cfg.opposing_sides(), 1);
    cfg.players[3].team = None;
    assert_eq!(cfg.opposing_sides(), 2);
    cfg.players[2].team = Some(1);
    assert_eq!(cfg.opposing_sides(), 3);
    for p in &mut cfg.players {
        p.team = Some(0);
    }
    let mut w = World::new(cfg, &d).unwrap();
    w.count = 0;
    step_world(&mut w, &d, &[]);
    assert!(w.end_t > 0);
    run(&mut w, &d, d.global.match_.end_frames as usize, &[]);
    assert!(w.fighters.iter().all(|f| f.stocks == 3 && f.state == State::Win));
    let mut w =
        World::new(MatchConfig { players: vec![PlayerConfig::human("brawler")], ..Default::default() }, &d).unwrap();
    w.count = 0;
    step_world(&mut w, &d, &[]);
    assert!(w.end_t > 0);
}

#[test]
fn eight_player_cpu_ffa_determinism_and_rollback() {
    let d = common::data();
    let cfg = MatchConfig {
        seed: 9821,
        players: (0..8)
            .map(|i| PlayerConfig {
                cpu: Some((i % 3) as u8),
                ..PlayerConfig::human(&d.fighters[i % d.fighters.len()].id)
            })
            .collect(),
        ..Default::default()
    };
    assert_eq!(cfg.opposing_sides(), 8);
    let mut w = World::new(cfg, &d).unwrap();
    let initial = w.clone();
    let mut copy = w.clone();
    let mut snapshots = vec![];
    for tick in 0..6000 {
        if tick % 1000 == 0 {
            snapshots.push((tick, w.clone()));
        }
        common::step_draining(&mut w, &d, &[]);
        common::step_draining(&mut copy, &d, &[]);
        assert_eq!(w, copy);
    }
    assert!(w.fighters.iter().any(|f| f.dmg > 0. || f.stocks < 3));
    assert!(w.fighters.iter().all(|f| f.x.is_finite() && f.y.is_finite()));
    snapshots.push((0, initial));
    for (tick, mut restored) in snapshots {
        for _ in tick..6000 {
            common::step_draining(&mut restored, &d, &[]);
        }
        assert_eq!(w, restored, "rollback from {tick}");
    }
}

#[test]
fn bat_without_bounce_recovers_cooldown_and_can_be_batted_again() {
    let mut d = common::data();
    let di = d.fighters.iter().position(|f| f.id == "king").unwrap();
    let id = d.fighters[di].projectiles.keys().next().unwrap().clone();
    let def = d.fighters[di].projectiles.get_mut(&id).unwrap();
    def.bounce = None;
    def.gravity = 0.;
    let bat = def.bat.as_mut().unwrap();
    bat.cooldown = 3;
    bat.speed = 1.;
    bat.vy = 0.;
    let mut w = world(&d, "king");
    let f = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &f, &id);
    w.projs[0].x = 650.;
    w.projs[0].y = 100.;
    let hit = d.fighters[di].moves.jab.phases[0].hitboxes[0].clone();
    let boxes = [combat::ActiveBox {
        shape: sim::geometry::Shape::Circle { center: [650., 100.], radius: hit.r },
        hit,
        x: 650.,
        y: 100.,
    }];
    sim::projectiles::bat(&mut w, &d, 0, &boxes);
    assert_eq!(w.projs[0].cooldown, 3);
    for _ in 0..3 {
        sim::projectiles::step_projectiles(&mut w, &d);
    }
    assert_eq!(w.projs[0].cooldown, 0);
    sim::projectiles::bat(&mut w, &d, 1, &boxes);
    assert_eq!(w.projs[0].owner, 1);
    assert_eq!(w.projs[0].cooldown, 3);
}

#[test]
fn hit_records_keep_every_victim_and_last_attacker() {
    let d = common::data();
    let cfg = MatchConfig::sandbox("arena", vec![PlayerConfig::human("brawler"); 4], MatchConfig::default().seed);
    let mut w = World::new(cfg, &d).unwrap();
    for i in 1..4 {
        w.fighters[i].x = w.fighters[0].x + 35.;
    }
    w.fighters[3].set_state(State::Shield);
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "jab");
    f.mov.as_mut().unwrap().t = 6;
    w.fighters[0] = f;
    combat::hit_pass(&mut w, &d);
    assert_eq!(w.hits.len(), 3);
    for (record, victim) in w.hits.iter().zip(1..4) {
        assert_eq!(record.attacker, Some(0));
        assert_eq!(record.victim, victim);
        assert!(record.damage > 0.);
        assert_eq!(record.shielded, victim == 3);
        assert_eq!(w.fighters[victim].last_hit_by, (victim != 3).then_some(0));
    }
    assert_eq!(w.trace.as_ref().unwrap().defender, 3);
    w.fighters[1].set_state(State::Hitstun);
    assert_eq!(w.fighters[1].last_hit_by, Some(0));
    w.hits.clear();
    step_world(&mut w, &d, &[]);
    assert!(w.hits.is_empty());
}

#[test]
fn throws_pummels_and_projectiles_emit_hit_records() {
    let d = common::data();
    let mut w = world(&d, "brawler");
    combat::start_hold(&mut w, &d, 0, 1);
    w.fighters[0].sd.pt = d.global.grab.pummel_hit_frame + 1;
    step_world(&mut w, &d, &[]);
    assert_eq!(
        w.hits,
        [sim::HitRecord { attacker: Some(0), victim: 1, damage: d.global.grab.pummel_damage, shielded: false }]
    );
    assert_eq!(w.fighters[1].last_hit_by, Some(0));
    assert!(w.trace.is_none());
    w.hits.clear();
    let f = &mut w.fighters[0];
    f.set_state(State::Throw);
    f.sd.victim = Some(1);
    f.sd.kind = "f".into();
    f.t = d.fighters[f.data].throws.f.release - 1;
    f.lag = 0;
    w.fighters[1].lag = 0;
    step_world(&mut w, &d, &[]);
    assert_eq!(w.hits.len(), 1);
    assert_eq!(w.hits[0].damage, d.fighter("brawler").unwrap().throws.f.hit.damage);
    assert_eq!(w.hits[0].victim, 1);
    assert_eq!(w.fighters[1].last_hit_by, Some(0));
    for shielded in [false, true] {
        let mut w = world(&d, "brawler");
        if shielded {
            w.fighters[1].set_state(State::Shield);
        }
        let f = w.fighters[0].clone();
        let id = d.fighters[f.data].projectiles.keys().next().unwrap();
        sim::projectiles::spawn(&mut w, &d, 0, &f, id);
        let p = &mut w.projs[0];
        p.x = w.fighters[1].x;
        p.y = w.fighters[1].y - 40.;
        p.vx = 0.;
        p.vy = 0.;
        sim::projectiles::step_projectiles(&mut w, &d);
        assert_eq!(w.hits.len(), 1);
        assert_eq!(w.hits[0].attacker, Some(0));
        assert_eq!(w.hits[0].victim, 1);
        assert_eq!(w.hits[0].shielded, shielded);
        assert_eq!(w.fighters[1].last_hit_by, (!shielded).then_some(0));
    }
}

#[test]
fn autolink_uses_default_hurtbox_center() {
    let mut d = common::data();
    let mut w = world(&d, "brawler");
    let di = w.fighters[1].data;
    d.fighters[di].hurtboxes.default = sim::data::Rect { x1: 10., x2: 30., y1: 20., y2: 80. };
    w.fighters[1].face = -1.;
    let mut hit = d.fighters[di].moves.jab.phases[0].hitboxes[0].clone();
    hit.kind = sim::data::HitType::Autolink;
    let point = (w.fighters[1].x - 20., w.fighters[1].y - 50.);
    combat::apply_hit(
        &mut w,
        &d,
        1,
        &hit,
        combat::HitSource { attacker: 0, direction: 1., point, projectile: false, label: None },
    );
    assert_eq!((w.fighters[1].vx, w.fighters[1].vy), (0., 0.));
}

#[test]
fn narrow_stage_ai_target_padding_is_safe() {
    let mut d = common::data();
    d.stages[0].blocks[0].right = d.stages[0].blocks[0].left + 10.;
    let cfg = MatchConfig::sandbox(
        "arena",
        vec![PlayerConfig { cpu: Some(2), ..PlayerConfig::human("brawler") }; 2],
        MatchConfig::default().seed,
    );
    let mut w = World::new(cfg, &d).unwrap();
    for f in &mut w.fighters {
        f.x = d.stages[0].blocks[0].left + 5.;
    }
    run(&mut w, &d, 300, &[]);
    assert!(w.fighters.iter().all(|f| f.x.is_finite() && f.y.is_finite()));
}

#[test]
fn right_stick_selects_smashes_and_aerials_without_steering() {
    let d = common::data();
    for (stick, ground, slot) in [
        ((127, 0), true, "fsmash"),
        ((-127, 0), true, "fsmash"),
        ((0, 127), true, "usmash"),
        ((0, -127), true, "dsmash"),
        ((127, 0), false, "fair"),
        ((-127, 0), false, "bair"),
        ((0, 127), false, "uair"),
        ((0, -127), false, "dair"),
    ] {
        let mut w = common::fighters(&d, "brawler", &[]);
        let f = &mut w.fighters[0];
        f.face = 1.;
        if !ground {
            f.set_state(State::Air);
            f.ground = false;
            f.surf = None;
            f.y -= 200.;
        }
        let raw = Input { stick: (-127, 0), attack_stick: stick, ..Default::default() };
        common::step_draining(&mut w, &d, &[raw]);
        let f = &w.fighters[0];
        assert_eq!(f.mov.as_ref().unwrap().slot, slot, "{stick:?}, ground={ground}");
        assert_eq!(f.in_dir, -1.);
        if !ground {
            assert_eq!(f.face, 1.);
            assert!(f.vx < 0., "left-stick air drift survives a right-stick attack");
        }
    }
}

#[test]
fn right_stick_charges_then_releases_without_repeating() {
    let d = common::data();
    let mut w = common::fighters(&d, "brawler", &[]);
    let raw = Input { attack_stick: (127, 0), ..Default::default() };
    run(&mut w, &d, 40, &[raw]);
    assert!(w.fighters[0].mov.as_ref().unwrap().charge > 25);
    // A held stick never starts a second attack once the charged attack finishes.
    run(&mut w, &d, 180, &[raw]);
    assert_eq!(w.fighters[0].state, State::Idle);
    run(&mut w, &d, 1, &[Input::default()]);
    run(&mut w, &d, 1, &[raw]);
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "fsmash");
    run(&mut w, &d, 80, &[Input::default()]);
    assert_eq!(w.fighters[0].state, State::Idle);
}

fn timed(d: &GameData, seconds: u32, damages: [f64; 2]) -> World {
    let players = vec![PlayerConfig::human("brawler"), PlayerConfig::human("brawler")];
    let cfg = MatchConfig { training: false, time_limit: Some(seconds), ..MatchConfig::sandbox("arena", players, 9) };
    let mut w = World::new(cfg, d).unwrap();
    for (f, dmg) in w.fighters.iter_mut().zip(damages) {
        f.dmg = dmg;
    }
    w
}

#[test]
fn time_up_goes_to_most_stocks_then_lowest_damage() {
    let d = common::data();
    let mut w = timed(&d, 3, [40., 10.]);
    for _ in 0..400 {
        run(&mut w, &d, 1, &[]);
    }
    assert!(w.time_up, "the timer ran out");
    assert_eq!(w.time_left, Some(0));
    assert_eq!(w.winner, Some(1), "lower damage wins the tie on stocks");
    assert!(w.end_t > 0);
    assert!(w.time_tied.is_empty(), "a decided time up has no sudden death");
    w.fighters[1].stocks = 2;
    assert_eq!(w.time_winner(), Some(1), "more stocks beats less damage");
    w.fighters[0].stocks = 2;
    w.fighters[1].dmg = 40.;
    assert_eq!(w.time_winner(), None, "level stocks and damage is a tie");
    assert_eq!(w.time_leaders(), [0, 1], "the tied sides meet again in sudden death");
    // A fighter in the death sequence has spent the stock they died on: that damage no longer counts.
    w.fighters[1].set_state(State::Dead);
    w.fighters[1].dmg = 300.;
    assert_eq!(w.time_winner(), Some(1), "nobody leads once the tie is broken");
    w.fighters[1].set_state(State::Out);
    w.fighters[1].stocks = 0;
    assert_eq!(w.time_winner(), Some(0));
    w.fighters[0].stocks = 0;
    assert_eq!(w.time_winner(), None, "everyone out is a draw");
    assert!(w.time_leaders().is_empty());
    run(&mut w, &d, 60 * 4, &[]);
    assert!(w.over);
}

#[test]
fn time_up_pools_teammates_stocks_and_damage() {
    let d = common::data();
    let players = vec![
        PlayerConfig { team: Some(0), ..PlayerConfig::human("brawler") },
        PlayerConfig { team: Some(0), ..PlayerConfig::human("brawler") },
        PlayerConfig { team: Some(1), ..PlayerConfig::human("brawler") },
        PlayerConfig { team: Some(1), ..PlayerConfig::human("brawler") },
    ];
    let cfg = MatchConfig { training: false, ..MatchConfig::sandbox("arena", players, 9) };
    let mut w = World::new(cfg, &d).unwrap();
    for (i, f) in w.fighters.iter_mut().enumerate() {
        f.dmg = [60., 5., 30., 40.][i];
    }
    assert_eq!(w.time_winner(), Some(0), "team 0 pools 65% against team 1's 70%");
    assert!(w.time_leaders().is_empty(), "a decided time up has no sudden death");
    w.fighters[3].dmg = 35.;
    assert_eq!(w.time_winner(), None, "level pooled damage is a tie");
    assert_eq!(w.time_leaders(), [0, 1, 2, 3]);
}

#[test]
fn the_timer_waits_for_go_and_beeps_through_its_warning_time() {
    let d = common::data();
    let warn = d.global.match_.timer_warn_seconds;
    let mut w = timed(&d, warn, [0., 0.]);
    let start = w.time_left;
    let mut ticks = 0;
    let mut counted_until = 0;
    while w.time_left != Some(0) {
        let before = w.count;
        common::step_preserving(&mut w, &d, &[]);
        if w.fx.iter().any(|fx| matches!(fx, sim::Fx::Sfx { name, .. } if name == "tick")) {
            ticks += 1;
        }
        w.fx.clear();
        if before > 1 {
            assert_eq!(w.time_left, start, "the timer waits for the countdown");
        }
        counted_until = w.count;
    }
    assert_eq!(counted_until, 0, "the clock started at GO");
    assert_eq!(ticks, warn, "one beep a second through the warning time");
}

#[test]
fn sudden_death_starts_everyone_at_high_damage_with_one_stock_and_no_timer() {
    let d = common::data();
    let players = vec![PlayerConfig::human("brawler"), PlayerConfig::human("brawler")];
    let mut cfg = MatchConfig { training: false, time_limit: Some(60), ..MatchConfig::sandbox("arena", players, 9) }
        .sudden_death();
    let w = World::new(cfg.clone(), &d).unwrap();
    assert_eq!(w.time_left, None);
    assert!(!w.cfg.training);
    for f in &w.fighters {
        assert_eq!(f.dmg, d.global.match_.sudden_death_damage);
        assert_eq!(f.stocks, 1);
    }
    // A side that lost the time-up starts out.
    cfg.players[1].eliminated = true;
    let w = World::new(cfg, &d).unwrap();
    assert_eq!((w.fighters[1].stocks, w.fighters[1].state, w.fighters[1].dmg), (0, State::Out, 0.));
    assert_eq!(w.time_winner(), Some(0));
}

#[test]
fn sudden_death_participants_are_fixed_when_the_clock_hits_zero() {
    let d = common::data();
    let mut w = timed(&d, 3, [10., 10.]);
    for _ in 0..400 {
        run(&mut w, &d, 1, &[]);
    }
    assert!(w.time_up);
    assert_eq!(w.winner, None, "level on stocks and damage");
    assert_eq!(w.time_tied, [0, 1], "both tied sides would meet again");
    // The ending animation can still land hits; that must not change who plays sudden death.
    w.fighters[0].dmg = 80.;
    assert_eq!(w.time_winner(), Some(1));
    assert_eq!(w.time_tied, [0, 1], "the lineup stays as it was at zero");
}
