mod common;

use sim::{
    GameData, World,
    data::{ChargeNeed, Event},
    world::State,
};

fn setup(d: &GameData) -> World {
    common::fighters(d, "throbbin", &["brawler", "brawler"])
}

fn event(w: &mut World, d: &GameData) {
    let mut f = w.fighters[0].clone();
    sim::moves::start_move(w, d, 0, &mut f, "jab");
    w.fighters[0] = f;
}

#[test]
fn restore_event_caps_and_preserves_empty_pool_recharge() {
    let mut d = common::data();
    let mut w = setup(&d);
    let di = w.fighters[0].data;
    d.fighters[di].moves.jab.phases[0].events = vec![Event {
        at: Some(0),
        restore: Some(ChargeNeed { charge: "levin".into(), uses: 2 }),
        ..Default::default()
    }];
    w.fighters[0].charges[0] = 0;
    w.fighters[0].charge_wait[0] = 40;
    event(&mut w, &d);
    assert_eq!((w.fighters[0].charges[0], w.fighters[0].charge_wait[0]), (2, 40));
    for _ in 0..40 {
        sim::charges::tick(&d, &mut w.fighters[0]);
    }
    assert_eq!(w.fighters[0].charges[0], 3);
    w.fighters[0].charges[0] = 7;
    event(&mut w, &d);
    assert_eq!((w.fighters[0].charges[0], w.fighters[0].charge_wait[0]), (8, 0));
    event(&mut w, &d);
    assert_eq!(w.fighters[0].charges[0], 8);
    w.fighters[0].charges[0] = 0;
    w.fighters[0].charge_wait[0] = 0;
    event(&mut w, &d);
    assert_eq!((w.fighters[0].charges[0], w.fighters[0].charge_wait[0]), (2, 0));
}

fn gadgets(d: &mut GameData, w: &mut World, manual: bool) -> String {
    let di = w.fighters[0].data;
    let id = d.fighters[di].projectiles.keys().next().unwrap().clone();
    let p = d.fighters[di].projectiles.get_mut(&id).unwrap();
    p.vel = [0., 0.];
    p.gravity = 0.;
    p.bounce = None;
    p.life = 2;
    p.max = 5;
    p.explosion = Some(sim::data::fighter::Explosion {
        manual,
        arm_ticks: 0,
        proximity: Some(100.),
        radius: 100.,
        sfx: Some("boom".into()),
        fx: vec![],
    });
    for owner in [0, 0, 1] {
        // Give the other owner the same gadget definition through the spawned projectile.
        sim::projectiles::spawn(w, d, 0, &w.fighters[0].clone(), &id);
        let p = w.projs.last_mut().unwrap();
        p.owner = owner;
        p.x = 640.;
        p.y = 480.;
    }
    for f in &mut w.fighters {
        f.x = 640.;
        f.y = 520.;
        f.inv = 0;
    }
    id
}

#[test]
fn detonate_event_blasts_owned_remote_gadgets_and_no_gadget_is_noop() {
    let mut d = common::data();
    let mut w = setup(&d);
    let di = w.fighters[0].data;
    d.fighters[di].moves.jab.phases[0].events =
        vec![Event { at: Some(0), detonate: Some(vec![]), ..Default::default() }];
    event(&mut w, &d);
    assert!(w.projs.is_empty());
    gadgets(&mut d, &mut w, true);
    event(&mut w, &d);
    assert_eq!(w.projs.len(), 1);
    assert_eq!(w.projs[0].owner, 1);
    assert!(w.fighters[1].dmg > 0.);
    assert!(w.fighters[2].dmg > 0.);
    assert_eq!(w.fighters[0].dmg, 0.);
    assert!(w.hits.iter().all(|h| h.attacker == Some(0)));
    assert!(w.fx.iter().any(|fx| matches!(fx, sim::Fx::Sfx { name, .. } if name == "boom")));
    w.fighters[0].dmg = 50.;
    gadgets(&mut d, &mut w, true);
    for p in &mut w.projs {
        p.hit.heal = 0.5;
    }
    for f in &mut w.fighters[1..] {
        f.lag = 0;
    }
    event(&mut w, &d);
    assert!(w.fighters[0].dmg < 50., "blast healing must survive the move's local fighter update");
}

#[test]
fn detonate_leaves_timed_fuses_running() {
    let mut d = common::data();
    let mut w = setup(&d);
    let di = w.fighters[0].data;
    let id = gadgets(&mut d, &mut w, false);
    let _ = id;
    d.fighters[di].moves.jab.phases[0].events =
        vec![Event { at: Some(0), detonate: Some(vec![]), ..Default::default() }];
    let before: Vec<_> = w.projs.iter().filter(|p| p.owner == 0).map(|p| (p.x, p.y)).collect();
    assert!(!before.is_empty());
    event(&mut w, &d);
    let after: Vec<_> = w.projs.iter().filter(|p| p.owner == 0).map(|p| (p.x, p.y)).collect();
    assert_eq!(before, after, "a timed fuse is not fired by the remote detonator");
    assert!(w.fighters[1].dmg == 0. && w.fighters[2].dmg == 0.);
}

#[test]
fn manual_gadgets_survive_proximity_and_fuse_but_normal_fuse_still_blasts() {
    for (manual, proximity) in [(true, Some(100.)), (false, Some(100.)), (false, None)] {
        let mut d = common::data();
        let mut w = setup(&d);
        let id = gadgets(&mut d, &mut w, manual);
        let di = w.fighters[0].data;
        d.fighters[di].projectiles.get_mut(&id).unwrap().explosion.as_mut().unwrap().proximity = proximity;
        w.projs.retain(|p| p.owner == 0);
        for _ in 0..4 {
            sim::projectiles::step_projectiles(&mut w, &d);
        }
        assert_eq!(w.projs.len(), if manual { 2 } else { 0 });
        assert_eq!(w.fighters[2].dmg == 0., manual);
    }
}

#[test]
fn validation_reports_resource_paths() {
    let mut d = common::data();
    let di = setup(&d).fighters[0].data;
    for (speed, frames, path) in
        [(-1., 10, "stats.float.speed"), (f64::NAN, 10, "stats.float.speed"), (0., 0, "stats.float.frames")]
    {
        d.fighters[di].stats.float = Some(sim::data::fighter::Float { speed, frames, ground_height: 0. });
        assert!(sim::data::validate(&d).iter().any(|e| e.path == path), "{path}");
    }
    d.fighters[di].stats.float = Some(sim::data::fighter::Float { speed: 0., frames: 10, ground_height: 0. });
    for (charge, uses, suffix) in [("missing", 1, "charge"), ("levin", 0, "uses")] {
        d.fighters[di].moves.jab.phases[0].events = vec![Event {
            at: Some(0),
            restore: Some(ChargeNeed { charge: charge.into(), uses }),
            ..Default::default()
        }];
        assert!(
            sim::data::validate(&d).iter().any(|e| e.path == format!("moves.jab.phases[0].events[0].restore.{suffix}"))
        );
    }
}

#[test]
fn groups_share_contact_and_rehit_cooldowns_per_victim() {
    let mut d = common::data();
    let mut w = setup(&d);
    let di = w.fighters[0].data;
    let h = sim::data::Hitbox {
        start: 0,
        end: Some(20),
        x: 0.,
        y: 40.,
        r: 100.,
        damage: 2.,
        rehit: Some(5),
        ..Default::default()
    };
    d.fighters[di].moves.jab.phases[0].hitboxes = vec![h.clone(), h.clone(), sim::data::Hitbox { group: 1, ..h }];
    event(&mut w, &d);
    for f in &mut w.fighters {
        f.x = 640.;
        f.y = 520.;
        f.inv = 0;
        f.blink = 0;
    }
    for (at, want) in [(0, 2.), (1, 4.), (4, 4.), (5, 6.), (6, 8.)] {
        w.fighters[0].mov.as_mut().unwrap().elapsed = at;
        w.fighters[0].mov.as_mut().unwrap().t = at;
        for f in &mut w.fighters {
            f.lag = 0;
        }
        sim::combat::hit_pass(&mut w, &d);
        for f in &w.fighters[1..] {
            assert_eq!(f.dmg, want, "tick {at}");
        }
    }
}

#[test]
fn remote_blast_respects_shields_counters_and_other_victims() {
    let mut d = common::data();
    let mut w = setup(&d);
    let id = gadgets(&mut d, &mut w, true);
    w.projs.truncate(1);
    w.fighters[1].set_state(State::Shield);
    w.fighters[1].t = 100;
    let shield = w.fighters[1].sh;
    let di = w.fighters[2].data;
    d.fighters[di].moves.jab.phases[0].counter = Some(sim::data::moves::Counter {
        from: 0,
        to: 10,
        damage_mult: 1.,
        min_damage: 1.,
        attacker_freeze: 1,
        invuln: 12,
        reflect_speed: 1.,
        reflect_life: 60,
    });
    let mut f = w.fighters[2].clone();
    sim::moves::start_move(&mut w, &d, 2, &mut f, "jab");
    w.fighters[2] = f;
    sim::projectiles::detonate(&mut w, &d, 0, &[id]);
    assert!(w.projs.is_empty(), "countering a blast must not reflect the gadget");
    assert_eq!(w.fighters[1].dmg, 0.);
    assert!(w.fighters[1].sh < shield, "the other victim still takes shield damage");
    assert_eq!(w.fighters[2].dmg, 0.);
    assert_eq!(w.fighters[2].inv, 12);
    assert!(w.hits.iter().any(|h| h.victim == 1 && h.shielded));
}

#[test]
fn remote_blast_uses_current_team_eligibility() {
    for friendly_fire in [false, true] {
        let mut d = common::data();
        let mut w = setup(&d);
        let id = gadgets(&mut d, &mut w, true);
        w.projs.truncate(1);
        w.cfg.players[0].team = Some(1);
        w.cfg.players[1].team = Some(1);
        w.cfg.players[2].team = Some(2);
        w.cfg.friendly_fire = friendly_fire;
        sim::projectiles::detonate(&mut w, &d, 0, &[id]);
        assert_eq!(w.fighters[0].dmg, 0.);
        assert_eq!(w.fighters[1].dmg > 0., friendly_fire);
        assert!(w.fighters[2].dmg > 0.);
    }
}

#[test]
fn reflected_gadget_is_only_remotely_owned_by_the_reflector() {
    let mut d = common::data();
    let mut w = setup(&d);
    let id = gadgets(&mut d, &mut w, true);
    w.projs.truncate(1);
    let di = w.fighters[1].data;
    d.fighters[di].moves.jab.phases[0].reflector =
        Some(sim::data::moves::Reflector { from: 0, to: 10, speed: 1.5, damage: 2., radius: 80., height: 40. });
    let mut f = w.fighters[1].clone();
    sim::moves::start_move(&mut w, &d, 1, &mut f, "jab");
    w.fighters[1] = f;
    sim::projectiles::step_projectiles(&mut w, &d);
    assert_eq!(w.projs[0].owner, 1);
    sim::projectiles::detonate(&mut w, &d, 0, std::slice::from_ref(&id));
    assert_eq!(w.projs.len(), 1, "original owner cannot remotely fire reflected ordnance");
    sim::projectiles::detonate(&mut w, &d, 1, &[id]);
    assert!(w.projs.is_empty());
    assert!(w.fighters[0].dmg > 0.);
    assert_eq!(w.fighters[1].dmg, 0.);
    assert!(w.hits.iter().all(|h| h.attacker == Some(1)));
}

#[test]
fn clone_and_serialized_restore_keep_partial_recharge_and_remote_detonation() {
    let mut d = common::data();
    let mut w = setup(&d);
    let id = gadgets(&mut d, &mut w, true);
    let di = w.fighters[0].data;
    d.fighters[di].moves.jab.phases[0].events = vec![Event {
        at: Some(0),
        restore: Some(ChargeNeed { charge: "levin".into(), uses: 2 }),
        ..Default::default()
    }];
    w.fighters[0].charges[0] = 0;
    w.fighters[0].charge_wait[0] = 40;
    event(&mut w, &d);
    assert_eq!((w.fighters[0].charges[0], w.fighters[0].charge_wait[0]), (2, 40));
    let cloned = w.clone();
    let restored: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    d.fighters[di].moves.jab.phases[0].events[0].detonate = Some(vec![id]);
    let mut results = vec![];
    for mut run in [w, cloned, restored] {
        for _ in 0..39 {
            sim::charges::tick(&d, &mut run.fighters[0]);
        }
        event(&mut run, &d);
        assert_eq!(run.projs.len(), 1);
        assert_eq!((run.fighters[0].charges[0], run.fighters[0].charge_wait[0]), (4, 1));
        sim::charges::tick(&d, &mut run.fighters[0]);
        assert_eq!(run.fighters[0].charges[0], 5);
        assert!(run.fighters[2].dmg > 0.);
        results.push(serde_json::to_value(run).unwrap());
    }
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0], results[2]);
}

#[test]
fn raw_directional_release_is_relative_to_either_facing() {
    let d = common::data();
    for face in [-1., 1.] {
        for (direction, phase, projectile) in
            [(-1, "throwLob", "grenadeLob"), (0, "throwMid", "grenadeMid"), (1, "throwSmash", "grenadeStrong")]
        {
            let mut w = common::fighters(&d, "sneak", &["brawler"]);
            w.count = 0;
            w.fighters[0].face = face;
            w.fighters[1].x = 1400.;
            for _ in 0..30 {
                common::step_draining(&mut w, &d, &[sim::Input { buttons: sim::input::SPECIAL, ..Default::default() }]);
            }
            let input = sim::Input { stick: ((direction as f64 * face * 127.) as i8, 0), ..Default::default() };
            common::step_draining(&mut w, &d, &[input]);
            assert_eq!(sim::moves::phase(&d, &w.fighters[0]).name, phase);
            assert_eq!(w.fighters[0].face, face);
            for _ in 0..12 {
                common::step_draining(&mut w, &d, &[sim::Input::default()]);
                if !w.projs.is_empty() {
                    break;
                }
            }
            assert_eq!(w.projs[0].id, projectile);
            assert_eq!(w.projs[0].vx.signum(), face);
        }
    }
}

#[test]
fn detonate_ids_validate_as_explosive_projectiles() {
    let mut d = common::data();
    let w = setup(&d);
    let di = w.fighters[0].data;
    for id in ["missing", "arcfire"] {
        d.fighters[di].moves.jab.phases[0].events =
            vec![Event { at: Some(0), detonate: Some(vec![id.into()]), ..Default::default() }];
        assert!(sim::data::validate(&d).iter().any(|e| e.path == "moves.jab.phases[0].events[0].detonate[0]"));
    }
}
