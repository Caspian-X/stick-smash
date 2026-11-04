mod common;

use sim::{
    GameData, World,
    data::{ChargeNeed, Event},
    input::Intent,
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
fn detonate_event_blasts_all_owned_gadgets_and_no_gadget_is_noop() {
    let mut d = common::data();
    let mut w = setup(&d);
    let di = w.fighters[0].data;
    d.fighters[di].moves.jab.phases[0].events = vec![Event { at: Some(0), detonate: true, ..Default::default() }];
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
fn float_caps_held_jump_releases_and_excludes_other_states() {
    let mut d = common::data();
    let mut w = setup(&d);
    d.fighters[w.fighters[0].data].stats.float_speed = Some(1.);
    for state in
        [State::Air, State::Hitstun, State::Helpless, State::Ledge, State::Climb, State::Attack, State::Airdodge]
    {
        let mut f = w.fighters[0].clone();
        f.state = state;
        f.ground = false;
        f.surf = None;
        f.y = 100.;
        f.vy = 8.;
        sim::physics::air_phys(&mut w, &d, 0, &mut f, Intent { jump_held: true, ..Default::default() }, 0., 0.);
        assert_eq!(f.vy == 1., state == State::Air, "{state:?}");
        if state == State::Air {
            sim::physics::air_phys(&mut w, &d, 0, &mut f, Intent::default(), 0., 0.);
            assert!(f.vy > 1.);
            f.vy = -5.;
            sim::physics::air_phys(&mut w, &d, 0, &mut f, Intent { jump_held: true, ..Default::default() }, 0., 0.);
            assert!(f.vy < 0.);
        }
    }
}

#[test]
fn validation_reports_resource_paths() {
    let mut d = common::data();
    let di = setup(&d).fighters[0].data;
    for speed in [0., -1., f64::INFINITY, f64::NAN] {
        d.fighters[di].stats.float_speed = Some(speed);
        assert!(sim::data::validate(&d).iter().any(|e| e.path == "stats.floatSpeed"));
    }
    d.fighters[di].stats.float_speed = Some(1.);
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
