mod common;
use sim::{
    GameData, Input, World, combat,
    input::{ATTACK, Intent, JUMP, SPECIAL},
    moves,
    world::State,
};

fn setup(slot: &str) -> (GameData, World) {
    let d = common::data();
    let mut w = common::fighters(&d, "wizard", &["brawler"]);
    w.count = 0;
    for (i, f) in w.fighters.iter_mut().enumerate() {
        f.x = 640. + i as f64 * 400.;
        f.y = 200.;
        f.ground = false;
        f.surf = None;
        f.inv = 0;
    }
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, slot);
    w.fighters[0] = f;
    (d, w)
}
fn tick(w: &mut World, d: &GameData, inp: Intent) {
    let mut f = w.fighters[0].clone();
    moves::st_attack(w, d, 0, &mut f, inp, inp.dir());
    w.fighters[0] = f;
    w.frame += 1;
}
fn raw(w: &mut World, d: &GameData, stick: (i8, i8), buttons: u8) {
    common::step_draining(w, d, &[Input { stick, buttons, ..Default::default() }, Input::default()]);
}

#[test]
fn reference_attack_windows_and_durations() {
    for (slot, total, windows) in [
        ("jab", 24, vec![(4, 5, 2.5), (7, 7, 2.5)]),
        ("ftilt", 36, vec![(12, 13, 15.)]),
        ("utilt", 29, vec![(7, 19, 7.2)]),
        ("dtilt", 21, vec![(5, 11, 5.5)]),
        ("dashatk", 35, vec![(6, 7, 12.), (8, 12, 6.)]),
        ("fsmash", 49, vec![(16, 16, 1.), (18, 18, 1.), (20, 20, 1.), (22, 22, 1.), (24, 24, 13.)]),
        ("usmash", 63, vec![(9, 23, 2.), (25, 32, 0.8), (34, 34, 5.)]),
        ("dsmash", 37, vec![(5, 6, 12.), (13, 14, 10.)]),
        ("nair", 50, vec![(6, 7, 2.5), (10, 11, 2.5), (14, 15, 2.5), (18, 19, 2.5), (22, 23, 5.)]),
        ("fair", 49, vec![(6, 6, 20.), (7, 10, 4.)]),
        ("bair", 49, vec![(6, 6, 20.), (7, 10, 4.)]),
        ("uair", 54, vec![(14, 16, 17.), (17, 19, 12.)]),
        ("dair", 44, vec![(14, 14, 16.), (15, 24, 5.)]),
        ("grab", 39, vec![(10, 11, 0.)]),
        ("dashgrab", 47, vec![(13, 14, 0.)]),
        ("nspec", 57, vec![(11, 22, 2.), (26, 26, 5.)]),
    ] {
        let (d, mut w) = setup(slot);
        w.fighters[0].y = -1000.;
        for t in 0..total {
            let bs = combat::active_boxes(&d, &w.fighters[0]);
            let expected = windows.iter().find(|(a, b, _)| (*a..=*b).contains(&t));
            assert_eq!(!bs.is_empty(), expected.is_some(), "{slot} tick {t}");
            if let Some((_, _, damage)) = expected {
                assert!(bs.iter().any(|b| b.hit.damage == *damage), "{slot} tick {t}: {bs:?}");
            }
            tick(&mut w, &d, Intent::default());
        }
        assert!(w.fighters[0].mov.is_none(), "{slot}");
    }
}

#[test]
fn lightning_sweet_spots_win_overlap_and_keep_reference_launch() {
    for (slot, dir, bkb, kbg) in [("fair", 1., 24., 0.95), ("bair", -1., 30., 0.96)] {
        let (d, mut w) = setup(slot);
        w.fighters[0].y = -1000.;
        for _ in 0..6 {
            tick(&mut w, &d, Intent::default());
        }
        let bs = combat::active_boxes(&d, &w.fighters[0]);
        let sweet = &bs[0];
        assert_eq!((sweet.hit.damage, sweet.hit.r, sweet.hit.bkb, sweet.hit.kbg), (20., 11., bkb, kbg));
        let y = w.fighters[0].y;
        w.fighters[1].x = sweet.x;
        w.fighters[1].y = y;
        w.fighters[1].dmg = 100.;
        combat::hit_pass(&mut w, &d);
        assert_eq!(w.fighters[1].dmg, 120.);
        assert_eq!(sim::world::sign(w.fighters[1].vx), dir);
        let at100 = combat::knockback(&d, &sweet.hit, 120., 1.);
        let at0 = combat::knockback(&d, &sweet.hit, 20., 1.);
        assert!(at100 > at0);
        assert_eq!(bs[1].hit.damage, 4.);
    }
}

#[test]
fn aerial_autocancels_and_landing_lag_match_boundaries() {
    for (slot, start, end, lag) in
        [("nair", 4, 38, 12), ("fair", 3, 43, 15), ("bair", 3, 45, 16), ("uair", 4, 54, 12), ("dair", 4, 40, 12)]
    {
        for (at, expected) in [(start - 1, 0), (start, lag), (end - 1, lag)] {
            let (d, mut w) = setup(slot);
            w.fighters[0].y = -1000.;
            for _ in 0..at {
                tick(&mut w, &d, Intent::default());
            }
            let mut f = w.fighters[0].clone();
            f.ground = true;
            moves::trigger(&mut w, &d, 0, &mut f, sim::data::Trigger::Land);
            assert_eq!(f.state, if expected == 0 { State::Idle } else { State::Landing }, "{slot} {at}");
            if expected > 0 {
                assert_eq!(f.sd.n, expected as f64);
            }
        }
        if end < 54 {
            let (d, mut w) = setup(slot);
            w.fighters[0].y = -1000.;
            for _ in 0..end {
                tick(&mut w, &d, Intent::default());
            }
            let mut f = w.fighters[0].clone();
            f.ground = true;
            moves::trigger(&mut w, &d, 0, &mut f, sim::data::Trigger::Land);
            assert_eq!(f.state, State::Idle);
        }
    }
}

#[test]
fn reflector_survives_contacts_and_operates_during_intangibility() {
    let (d, mut w) = setup("nspec");
    w.fighters[1].data = d.fighters.iter().position(|f| f.id == "archer").unwrap();
    w.fighters[0].mov.as_mut().unwrap().t = 4;
    w.fighters[0].inv = 10;
    for _ in 0..2 {
        let f = w.fighters[1].clone();
        sim::projectiles::spawn(&mut w, &d, 1, &f, "arrow");
        let p = w.projs.last_mut().unwrap();
        p.x = w.fighters[0].x + 40.;
        p.y = w.fighters[0].y - 40.;
        p.vx = -8.;
        p.vy = 0.;
        sim::projectiles::step_projectiles(&mut w, &d);
        let p = w.projs.last().unwrap();
        assert_eq!(p.owner, 0);
        assert!(p.vx > 0.);
        assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 0);
        w.projs.clear();
    }
    for (t, yes) in [(3, false), (4, true), (40, true), (41, false)] {
        w.fighters[0].mov.as_mut().unwrap().t = t;
        assert_eq!(combat::reflecting(&d, &w.fighters[0]).is_some(), yes);
    }
}

#[test]
fn raw_jump_attack_and_attack_stick_route_to_imported_aerials() {
    let d = common::data();
    let mut w = common::fighters(&d, "wizard", &["brawler"]);
    w.count = 0;
    raw(&mut w, &d, (0, 0), JUMP | ATTACK);
    for _ in 0..4 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "nair");
    w.fighters[0].set_state(State::Air);
    w.fighters[0].lag = 0;
    w.fighters[0].history = Default::default();
    common::step_draining(&mut w, &d, &[Input { attack_stick: (127, 0), ..Default::default() }, Input::default()]);
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "fair");
}

#[test]
fn farore_aim_is_captured_once_and_ends_helpless() {
    for (x, y) in [(1., 0.), (-1., 1.), (0., 0.)] {
        let (d, mut w) = setup("uspec");
        w.fighters[0].y = -1000.;
        for _ in 0..17 {
            tick(&mut w, &d, Intent { x, y, ..Default::default() });
        }
        assert!(moves::phase(&d, &w.fighters[0]).hidden);
        let vel = (w.fighters[0].vx, w.fighters[0].vy);
        let start = (w.fighters[0].x - vel.0, w.fighters[0].y - vel.1);
        assert!((libm::hypot(vel.0, vel.1) - 21.).abs() < 1e-9);
        for _ in 0..16 {
            tick(&mut w, &d, Intent { x: -x, y: -y, ..Default::default() });
        }
        assert_eq!((w.fighters[0].vx, w.fighters[0].vy), vel);
        assert!(libm::hypot(w.fighters[0].x - start.0, w.fighters[0].y - start.1) > 350.);
        tick(&mut w, &d, Intent::default());
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, "airEnd");
        assert_eq!(combat::active_boxes(&d, &w.fighters[0]).len(), 0);
        tick(&mut w, &d, Intent::default());
        assert_eq!(combat::active_boxes(&d, &w.fighters[0])[0].hit.damage, 12.);
        for _ in 0..40 {
            tick(&mut w, &d, Intent::default());
        }
        assert_eq!(w.fighters[0].state, State::Helpless);
    }
}

#[test]
fn dins_fire_guides_releases_and_explodes_on_its_own_timeline() {
    let (d, mut w) = setup("sspec");
    for _ in 0..15 {
        raw(&mut w, &d, (127, 64), SPECIAL);
    }
    assert_eq!(w.projs.len(), 1);
    assert!(w.projs[0].vy < 0.);
    let y = w.projs[0].y;
    for _ in 0..16 {
        raw(&mut w, &d, (127, 64), 0);
    }
    assert!(w.projs[0].released.is_some());
    assert!(w.projs[0].y < y);
    assert_eq!(moves::phase(&d, &w.fighters[0]).name, "release");
    let at = w.projs[0].released.unwrap();
    assert_eq!(at, 16);
    let pos = (w.projs[0].x, w.projs[0].y);
    for _ in 0..12 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert_eq!(w.fighters[1].dmg, 0.);
    w.fighters[1].x = pos.0;
    w.fighters[1].y = pos.1 + 35.;
    w.fighters[1].inv = 0;
    for _ in 0..3 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert!(w.fighters[1].dmg >= 7.);
    let damage = w.fighters[1].dmg;
    for _ in 0..10 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert_eq!(w.fighters[1].dmg, damage);
    assert!(w.projs.is_empty());
}

#[test]
fn phantom_has_all_release_levels_and_waits_after_wizard_is_free() {
    for (age, damage) in [(15, 5.9), (20, 8.2), (28, 10.5), (33, 11.8), (38, 14.1), (45, 15.4), (50, 17.7)] {
        let (d, mut w) = setup("dspec");
        while w.projs[0].age < age {
            raw(&mut w, &d, (0, 0), 0);
        }
        assert!(sim::articles::control(&mut w, &d, 0, Intent { sp: true, ..Default::default() }));
        let p = &w.projs[0];
        let a = d.fighter("wizard").unwrap().projectiles["phantom"].article.as_ref().unwrap();
        assert_eq!(a.stages[p.stage].hitboxes.last().unwrap().damage, damage);
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, "release");
    }
    let (d, mut w) = setup("dspec");
    for _ in 0..67 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert!(w.fighters[0].mov.is_none());
    assert_eq!(w.projs[0].released, None);
    let mut saved = w.clone();
    for _ in 0..60 {
        raw(&mut w, &d, (0, 0), 0);
        raw(&mut saved, &d, (0, 0), 0);
    }
    assert_eq!(w, saved);
    assert_eq!(w.projs[0].released, Some(121));
}

#[test]
fn phantom_can_be_destroyed_and_survives_its_casters_other_actions() {
    let (d, mut w) = setup("dspec");
    w.fighters[0].set_state(State::Air);
    let p = w.projs[0].clone();
    let mut f = w.fighters[1].clone();
    f.x = p.x - 39.;
    f.y = p.y;
    f.face = 1.;
    moves::start_move(&mut w, &d, 1, &mut f, "jab");
    f.mov.as_mut().unwrap().t = 3;
    w.fighters[1] = f;
    let owner = w.fighters[0].clone();
    w.frame += 1;
    sim::projectiles::step_projectiles(&mut w, &d);
    assert!(w.projs.first().is_some_and(|p| p.damage > 0.));
    assert_eq!(w.fighters[0], owner);
    // A different attack instance can damage it again; one sustained hit can't repeat every tick.
    let damage = w.projs[0].damage;
    w.frame += 1;
    sim::projectiles::step_projectiles(&mut w, &d);
    assert_eq!(w.projs[0].damage, damage);
    w.frame += 100;
    w.fighters[1].mov.as_mut().unwrap().started = w.frame;
    sim::projectiles::step_projectiles(&mut w, &d);
    assert!(w.projs.is_empty());
}

#[test]
fn down_throw_has_four_damage_ticks_before_the_launch() {
    let d = common::data();
    let mut w = common::fighters(&d, "wizard", &["brawler"]);
    w.count = 0;
    let f = &mut w.fighters[0];
    f.set_state(State::Throw);
    f.sd.kind = "d".into();
    f.sd.victim = Some(1);
    w.fighters[1].set_state(State::Grabbed);
    w.fighters[1].sd.by = Some(0);
    for t in 1..=42 {
        raw(&mut w, &d, (0, 0), 0);
        let ticks = [25, 30, 35, 39].iter().filter(|&&n| n <= t).count();
        assert_eq!(w.fighters[1].dmg, ticks as f64 * 1.5 + if t == 42 { 2. } else { 0. }, "tick {t}");
    }
    assert!(w.fighters[1].vy < 0.);
}

#[test]
fn phantom_manual_and_automatic_first_hits_include_status_windup() {
    for (release, first) in [(15, 26), (20, 32), (28, 38), (38, 46), (50, 59)] {
        let (d, mut w) = setup("dspec");
        while w.projs[0].age < release {
            raw(&mut w, &d, (0, 0), 0);
        }
        sim::articles::control(&mut w, &d, 0, Intent { sp: true, ..Default::default() });
        let p = &w.projs[0];
        let s = &d.fighter("wizard").unwrap().projectiles["phantom"].article.as_ref().unwrap().stages[p.stage];
        assert_eq!(release + s.hitboxes.iter().filter(|b| b.damage > 0.).map(|b| b.start).min().unwrap(), first);
    }
    let (d, mut w) = setup("dspec");
    for _ in 0..126 {
        raw(&mut w, &d, (0, 0), 0);
    }
    let p = &w.projs[0];
    let pos = (p.x + 35., p.y);
    assert!(p.automatic);
    assert_eq!(p.released, Some(121));
    w.fighters[1].set_state(State::Air);
    w.fighters[1].vx = 0.;
    w.fighters[1].vy = 0.;
    w.fighters[1].x = pos.0;
    w.fighters[1].y = pos.1;
    w.fighters[1].ground = false;
    w.fighters[1].inv = 0;
    assert_eq!(w.fighters[1].dmg, 0.);
    raw(&mut w, &d, (0, 0), 0);
    assert_eq!(w.fighters[1].dmg, 17.7);
}

#[test]
fn initial_dash_and_run_use_distinct_reference_speeds() {
    let d = common::data();
    let mut w = common::fighters(&d, "wizard", &[]);
    w.count = 0;
    raw(&mut w, &d, (127, 0), 0);
    assert!((w.fighters[0].vx - 1.958 * 3.5).abs() < 1e-9);
    for _ in 0..20 {
        raw(&mut w, &d, (127, 0), 0);
    }
    assert!((w.fighters[0].vx - 1.43 * 3.5).abs() < 1e-9);
}

#[test]
fn all_smashes_scale_damage_from_their_own_attack_source() {
    for (slot, hold) in [("fsmash", 10), ("usmash", 3), ("dsmash", 2)] {
        let (d, mut w) = setup(slot);
        w.fighters[0].y = -1000.;
        for _ in 0..hold {
            tick(&mut w, &d, Intent::default());
        }
        for _ in 0..60 {
            tick(&mut w, &d, Intent { atk_held: true, ..Default::default() });
        }
        assert_eq!(w.fighters[0].mov.as_ref().unwrap().charge, 60);
        while combat::active_boxes(&d, &w.fighters[0]).is_empty() {
            tick(&mut w, &d, Intent::default());
        }
        let charged = combat::active_boxes(&d, &w.fighters[0]);
        let mut uncharged = w.fighters[0].clone();
        uncharged.mov.as_mut().unwrap().charge = 0;
        let normal = combat::active_boxes(&d, &uncharged);
        for (c, n) in charged.iter().zip(normal.iter()) {
            assert!((c.hit.damage - n.hit.damage * 1.4).abs() < 1e-9);
            assert_eq!(c.hit.bkb, n.hit.bkb);
        }
    }
}

#[test]
fn independent_phantom_body_intercepts_shots_and_takes_blast_damage() {
    for blast in [false, true] {
        let (d, mut w) = setup("dspec");
        let body = (w.projs[0].x, w.projs[0].y - 45.);
        w.fighters[0].x = body.0 + 500.;
        w.fighters[1].data = d.fighters.iter().position(|f| f.id == if blast { "tinker" } else { "archer" }).unwrap();
        let f = w.fighters[1].clone();
        sim::projectiles::spawn(&mut w, &d, 1, &f, if blast { "bomb" } else { "arrow" });
        let p = w.projs.last_mut().unwrap();
        p.x = body.0;
        p.y = body.1;
        p.vx = 0.;
        p.vy = 0.;
        if blast {
            p.life = 1;
        }
        w.frame += 1;
        sim::projectiles::step_projectiles(&mut w, &d);
        assert!(!w.projs.iter().any(|p| p.owner == 1));
        assert_eq!(w.fighters[0].dmg, 0.);
        if blast {
            assert!(w.projs.is_empty());
        } else {
            assert!(w.projs[0].damage > 0.);
        }
    }
}

#[test]
fn invalid_new_semantics_report_their_content_path() {
    let d = common::data();
    for field in ["stages", "autoRelease", "reflector", "travelSpeed", "ticks"] {
        let mut f = d.fighter("wizard").unwrap().clone();
        match field {
            "stages" => f.projectiles.get_mut("phantom").unwrap().article.as_mut().unwrap().stages.clear(),
            "autoRelease" => f.projectiles.get_mut("phantom").unwrap().article.as_mut().unwrap().auto_release = 1,
            "reflector" => f.moves.nspec.phases[0].reflector.as_mut().unwrap().from = 100,
            "travelSpeed" => f.moves.uspec.phases[1].travel_speed = Some(-1.),
            _ => f.throws.d.ticks[0].at = 100,
        }
        let error = sim::data::load::check_fighter("wizard", &f).unwrap_err();
        assert!(error.path.contains(field), "{error:?}");
    }
}
