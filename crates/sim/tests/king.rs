mod common;

use sim::{
    GameData, Input, MatchConfig, PlayerConfig, World, combat,
    data::{Trigger, hitbox::HitType},
    moves, projectiles,
    world::State,
};

fn arena() -> (GameData, World) {
    let d = common::data();
    let mut w = common::fighters(&d, "king", &["brawler"]);
    w.count = 0;
    w.fighters[0].x = 500.;
    w.fighters[1].x = 1000.;
    w.fighters[0].y = 520.;
    w.fighters[1].y = 520.;
    w.fighters[0].ground = true;
    w.fighters[1].ground = true;
    (d, w)
}

fn start(w: &mut World, d: &GameData, slot: &str) {
    let mut f = w.fighters[0].clone();
    moves::start_move(w, d, 0, &mut f, slot);
    w.fighters[0] = f;
}

fn set_phase(w: &mut World, d: &GameData, name: &str, t: u32) {
    let phase = d.fighters[w.fighters[0].data]
        .moves
        .get(w.fighters[0].mov.as_ref().unwrap().slot.as_str())
        .unwrap()
        .phases
        .iter()
        .position(|p| p.name == name)
        .unwrap();
    let m = w.fighters[0].mov.as_mut().unwrap();
    m.phase = phase;
    m.t = t;
}

fn boxes(d: &GameData, w: &World) -> Vec<combat::ActiveBox> {
    combat::active_boxes_at(d, &w.fighters[0], w.frame)
}

fn raw(w: &mut World, d: &GameData, stick: (i8, i8), buttons: u8) {
    sim::step_world(w, d, &[Input { stick, buttons, ..Default::default() }, Input::default()]);
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
}

#[test]
fn package_identity_inventory_and_mobility() {
    let d = common::data();
    let f = d.fighter("king").unwrap();
    assert_eq!(f.name, "Gordo Ramsay");
    assert_eq!(f.stats.air_jumps, 4);
    assert!((f.stats.weight - 100. / 127.).abs() < 1e-6);
    for slot in sim::data::moves::Moves::SLOTS {
        if slot != "zair" {
            assert!(f.moves.get(slot).is_some(), "missing {slot}");
        }
    }
    assert!(f.moves.sspec.is_some(), "Gordo Throw needs the side special slot");
    for t in [&f.throws.f, &f.throws.b, &f.throws.u, &f.throws.d] {
        assert!(t.total > t.release);
    }
    assert!(f.projectiles.contains_key("gordo"));
    assert!(f.projectiles["gordo"].bounce.is_some() && f.projectiles["gordo"].bat.is_some());
    assert_eq!(f.stats.pummel_damage, Some(1.6));
}

#[test]
fn ground_attack_windows_follow_the_ledger() {
    let (d, mut w) = arena();
    for (slot, windows) in [
        ("jab", vec![(9, 10, 2.5)]),
        ("ftilt", vec![(11, 20, 2.0), (22, 22, 3.0)]),
        ("utilt", vec![(6, 12, 9.5), (6, 12, 11.5)]),
        ("dtilt", vec![(5, 6, 10.0), (7, 10, 6.0)]),
        ("dashatk", vec![(25, 26, 16.0), (27, 40, 13.0)]),
        ("fsmash", vec![(39, 40, 16.0), (41, 42, 25.0), (41, 42, 18.5), (43, 43, 11.0)]),
        ("usmash", vec![(16, 23, 16.0), (16, 23, 10.0)]),
        ("dsmash", vec![(13, 21, 13.0)]),
    ] {
        start(&mut w, &d, slot);
        let total = d.fighter("king").unwrap().moves.get(slot).unwrap().phases[0].frames.unwrap();
        for t in 0..total {
            w.fighters[0].mov.as_mut().unwrap().t = t;
            w.fighters[0].mov.as_mut().unwrap().elapsed = t;
            let active = boxes(&d, &w);
            let expected = windows.iter().any(|(a, b, _)| (*a..=*b).contains(&t));
            assert_eq!(!active.is_empty(), expected, "{slot} frame {t}");
            if let Some((_, _, damage)) = windows.iter().find(|(a, b, _)| (*a..=*b).contains(&t)) {
                assert!(active.iter().any(|b| (b.hit.damage - damage).abs() < 1e-9), "{slot} {t}: {active:?}");
            }
        }
        assert!(w.fighters[0].mov.is_some(), "{slot} direct inspection");
    }
}

#[test]
fn aerial_windows_autocancels_and_landing_lag() {
    let (d, mut w) = arena();
    for (slot, active_phase, windows, lag) in [
        ("nair", 1, vec![(4, 5, 12.0), (6, 26, 7.0)], 9),
        ("fair", 1, vec![(8, 10, 12.0)], 18),
        ("bair", 1, vec![(12, 14, 16.0)], 13),
        ("uair", 1, vec![(5, 17, 1.0), (19, 20, 5.0)], 13),
        ("dair", 1, vec![(15, 17, 15.0), (15, 17, 8.0)], 18),
    ] {
        start(&mut w, &d, slot);
        let def = d.fighter("king").unwrap().moves.get(slot).unwrap();
        let phase = &def.phases[active_phase];
        assert_eq!(phase.name, "active", "{slot}");
        let landing = &phase.transitions[0];
        assert_eq!(
            (landing.on, landing.to.as_str(), landing.lag),
            (Trigger::Land, "landing", Some(lag)),
            "{slot} landing lag"
        );
        for t in 0..phase.frames.unwrap() {
            w.fighters[0].mov.as_mut().unwrap().phase = active_phase;
            w.fighters[0].mov.as_mut().unwrap().t = t;
            let active = boxes(&d, &w);
            let expected = windows.iter().any(|(a, b, _)| (*a..=*b).contains(&t));
            assert_eq!(!active.is_empty(), expected, "{slot} active frame {t}");
        }
        for index in [0, 2] {
            let mut f = w.fighters[0].clone();
            f.mov.as_mut().unwrap().phase = index;
            f.mov.as_mut().unwrap().t = 0;
            f.ground = true;
            moves::trigger(&mut w, &d, 0, &mut f, Trigger::Land);
            assert_eq!(f.state, State::Idle, "{slot} autocancel phase {index}");
        }
    }
}

#[test]
fn smash_charge_scales_damage_and_launch() {
    let (d, mut w) = arena();
    start(&mut w, &d, "fsmash");
    set_phase(&mut w, &d, "main", 41);
    let base = boxes(&d, &w).into_iter().find(|b| b.hit.damage > 20.).unwrap().hit;
    let divisor = d.global.knockback.charge_damage_divisor;
    for charge in [0, 30, 60] {
        w.fighters[0].mov.as_mut().unwrap().charge = charge;
        let hit = boxes(&d, &w).into_iter().find(|b| b.hit.damage > 20.).unwrap().hit;
        let expected = base.damage * (1. + f64::from(charge) / divisor);
        assert!((hit.damage - expected).abs() < 1e-9, "charge {charge}");
        assert!(combat::knockback(&d, &hit, 100., 1.) >= combat::knockback(&d, &base, 100., 1.));
    }
}

#[test]
fn jet_hammer_charges_to_full_tier_and_has_armor() {
    let (d, mut w) = arena();
    start(&mut w, &d, "dspec");
    let def = d.fighter("king").unwrap().moves.dspec.clone();
    assert_eq!(def.charge_damage, Some((29.8 / 11. - 1.) / 136.));
    set_phase(&mut w, &d, "swing", 10);
    w.fighters[0].mov.as_mut().unwrap().charge = 0;
    let partial = boxes(&d, &w)[0].hit.clone();
    assert!((partial.damage - 11.).abs() < 1e-9);
    w.fighters[0].mov.as_mut().unwrap().charge = 136;
    let full = boxes(&d, &w)[0].hit.clone();
    assert!((full.damage - 40.).abs() < 1e-6, "{full:?}");
    assert_eq!(full.kbg, 0.46);
    w.fighters[0].mov.as_mut().unwrap().charge = 68;
    let mid = boxes(&d, &w)[0].hit.clone();
    assert!(mid.damage > 11. && mid.damage < 29.8);
    set_phase(&mut w, &d, "swing", 0);
    assert_eq!(d.fighter("king").unwrap().moves.dspec.phases[2].armor_threshold, Some(14.));
    assert_eq!(def.phases[2].frames, Some(14));
    assert_eq!(def.phases[3].frames, Some(45));
    assert_eq!(def.phases[3].charge_frames, 10. / 136.);
    assert_eq!(def.phases[3].armor_threshold, None);
}

#[test]
fn jet_hammer_armor_expires_at_release_frame_fourteen() {
    let (d, mut initial) = arena();
    start(&mut initial, &d, "dspec");
    for charge in [0, 136] {
        for (advance, armored) in [(false, true), (true, false)] {
            let mut w = initial.clone();
            set_phase(&mut w, &d, "swing", 13);
            w.fighters[0].mov.as_mut().unwrap().charge = charge;
            if advance {
                raw(&mut w, &d, (0, 0), 0);
                assert_eq!(moves::phase(&d, &w.fighters[0]).name, "recovery");
            }
            let mut hit = d.fighter("brawler").unwrap().moves.jab.phases[0].hitboxes[0].clone();
            hit.damage = 14.;
            combat::apply_hit(
                &mut w,
                &d,
                0,
                &hit,
                combat::HitSource { attacker: 1, direction: -1., point: (500., 480.), projectile: false, label: None },
            );
            assert_eq!(w.fighters[0].state == State::Attack, armored);
            assert_eq!(w.fighters[0].dmg, 14.);
        }
    }
}

#[test]
fn jet_hammer_active_centers_follow_the_attachment_angle() {
    let (d, mut w) = arena();
    start(&mut w, &d, "dspec");
    for face in [-1., 1.] {
        w.fighters[0].face = face;
        for t in 10..=12 {
            set_phase(&mut w, &d, "swing", t);
            let keys = &moves::phase(&d, &w.fighters[0]).attach["hammer"].angle;
            let pair = keys.windows(2).find(|k| k[0][0] <= f64::from(t) && f64::from(t) <= k[1][0]).unwrap();
            let [a, b] = [pair[0], pair[1]];
            let angle = (a[1] + (b[1] - a[1]) * (f64::from(t) - a[0]) / (b[0] - a[0])).to_radians();
            let expected =
                (w.fighters[0].x + face * 80. * libm::cos(angle), w.fighters[0].y - (64. + 80. * libm::sin(angle)));
            for active in boxes(&d, &w) {
                assert!((active.x - expected.0).abs() < 1e-4);
                assert!((active.y - expected.1).abs() < 1e-4);
            }
        }
    }
}

#[test]
fn meteor_and_sour_coverage_are_simultaneous_on_source_frames_twenty_two_to_twenty_four() {
    let (d, mut w) = arena();
    start(&mut w, &d, "dair");
    for t in 14..=18 {
        set_phase(&mut w, &d, "active", t);
        let hits = boxes(&d, &w);
        if (15..=17).contains(&t) {
            assert!(hits.iter().any(|b| b.hit.damage == 15. && b.hit.kind == HitType::Spike));
            assert!(
                hits.iter().any(|b| b.hit.damage == 15. && b.hit.situation == sim::data::hitbox::Situation::Ground)
            );
            assert!(hits.iter().any(|b| b.hit.damage == 8.));
        } else {
            assert!(hits.is_empty());
        }
    }
}

#[test]
fn gordo_spawns_bounces_and_is_battable() {
    let (d, mut w) = arena();
    start(&mut w, &d, "sspec");
    for t in 0..29 {
        if t == 10 {
            w.fighters[0].mov.as_mut().unwrap().t = 27;
            w.fighters[0].mov.as_mut().unwrap().elapsed = 27;
        }
        raw(&mut w, &d, (0, 0), 0);
    }
    // The spawn is an event on frame 28; allow the tick that runs it.
    for _ in 0..3 {
        raw(&mut w, &d, (0, 0), 0);
        if !w.projs.is_empty() {
            break;
        }
    }
    assert_eq!(w.projs.len(), 1, "Gordo spawn");
    let g = &w.projs[0];
    assert_eq!((g.owner, g.hit.damage, g.hit.angle), (0, 14., 60.));
    assert!(g.life > 0);
    let gordo = &d.fighter("king").unwrap().projectiles["gordo"];
    assert!(gordo.gravity > 0.);
    assert!(gordo.bounce.is_some() && gordo.bat.is_some());
    let mut saved = w.clone();
    let p = &w.projs[0];
    let bx = combat::ActiveBox {
        hit: d.fighter("king").unwrap().moves.jab.phases[0].hitboxes[0].clone(),
        x: p.x,
        y: p.y,
        shape: sim::geometry::Shape::Circle { center: [p.x, p.y], radius: 20. },
    };
    projectiles::bat(&mut w, &d, 1, std::slice::from_ref(&bx));
    assert_eq!(w.projs[0].owner, 1);
    assert!(w.projs[0].cooldown > 0);
    let after = w.clone();
    projectiles::bat(&mut saved, &d, 1, std::slice::from_ref(&bx));
    assert_eq!(saved.projs, after.projs);
}

#[test]
fn inhale_pulls_and_captures_with_the_shared_hold() {
    let (d, mut w) = arena();
    w.fighters[1].x = 570.;
    w.fighters[1].y = w.fighters[0].y;
    w.fighters[0].face = 1.;
    start(&mut w, &d, "nspec");
    for _ in 0..16 {
        raw(&mut w, &d, (0, 0), 4);
    }
    let phase = &d.fighter("king").unwrap().moves.nspec.phases[1];
    assert_eq!(phase.name, "inhale");
    assert!(phase.hitboxes.iter().any(|b| b.kind == HitType::Grab));
    assert!(phase.hitboxes.iter().any(|b| b.kind == HitType::Windbox));
    for _ in 0..8 {
        raw(&mut w, &d, (0, 0), 4);
    }
    assert!(matches!(w.fighters[0].state, State::Hold | State::Attack), "{:?}", w.fighters[0].state);
    assert!(matches!(w.fighters[1].state, State::Grabbed | State::Hitstun), "{:?}", w.fighters[1].state);

    let (d2, mut w2) = arena();
    w2.fighters[1].x = 630.;
    w2.fighters[1].y = w2.fighters[0].y;
    w2.fighters[0].face = 1.;
    start(&mut w2, &d2, "nspec");
    for _ in 0..20 {
        raw(&mut w2, &d2, (0, 0), 4);
    }
    assert_eq!(w2.fighters[1].state, State::Idle);
    assert!(w2.fighters[1].vx < 0., "windbox should pull toward Dedede");
}

#[test]
fn super_dedede_jump_has_armor_falling_hits_landing_stars_and_cancel() {
    let (d, mut w) = arena();
    start(&mut w, &d, "uspec");
    let def = d.fighter("king").unwrap().moves.uspec.clone();
    let launch = def.phases.iter().find(|p| p.name == "launch").unwrap();
    let plunge = def.phases.iter().find(|p| p.name == "plungeArmor").unwrap();
    assert_eq!(launch.armor_threshold, Some(9999.));
    assert_eq!(plunge.armor_threshold, Some(9999.));
    assert!(
        plunge.hitboxes.iter().any(|b| b.kind == HitType::Bury && b.situation == sim::data::hitbox::Situation::Ground)
    );
    assert!(
        plunge.hitboxes.iter().any(|b| b.kind == HitType::Spike && b.situation == sim::data::hitbox::Situation::Air)
    );
    let wave = def.phases.iter().find(|p| p.name == "wave").unwrap();
    assert_eq!(wave.frames, Some(60));
    assert!(wave.events.iter().any(|e| e.spawn.as_deref() == Some("starR")));
    assert!(wave.events.iter().any(|e| e.spawn.as_deref() == Some("starL")));

    set_phase(&mut w, &d, "plunge", 1);
    w.fighters[0].ground = false;
    let mut f = w.fighters[0].clone();
    moves::trigger(&mut w, &d, 0, &mut f, Trigger::ShieldPress);
    assert_eq!(f.state, State::Helpless);

    set_phase(&mut w, &d, "wave", 0);
    for _ in 0..6 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert_eq!(w.projs.len(), 2);
    let x = w.fighters[0].x;
    let mut offsets: Vec<_> = w.projs.iter().map(|p| p.x - x).collect();
    offsets.sort_by(f64::total_cmp);
    assert!(offsets[0] < 0. && offsets[1] > 0.);
    assert!((offsets[0] + offsets[1]).abs() < 1e-9);
    let f = d.fighter("king").unwrap();
    assert_eq!(f.projectiles["starL"].spawn[0], -f.projectiles["starR"].spawn[0]);
}

#[test]
fn throws_and_pummel_facts_are_imported() {
    let d = common::data();
    let f = d.fighter("king").unwrap();
    for (key, t, release, total, damage, tick) in [
        ("f", &f.throws.f, 14, 37, 6., Some((12, 4.))),
        ("b", &f.throws.b, 19, 41, 9., Some((16, 4.))),
        ("u", &f.throws.u, 19, 39, 5., Some((16, 4.))),
        ("d", &f.throws.d, 26, 41, 6., None),
    ] {
        assert_eq!((t.release, t.total, t.hit.damage), (release, total, damage), "{key}");
        if let Some((at, damage)) = tick {
            assert_eq!(t.ticks, vec![sim::data::moves::ThrowTick { at, damage }], "{key}");
        } else {
            assert!(t.ticks.is_empty(), "{key}");
        }
    }
    assert_eq!(f.stats.pummel_damage, Some(1.6));
}

#[test]
fn jab_chain_rapid_loop_and_finisher_are_reachable() {
    let (d, mut w) = arena();
    start(&mut w, &d, "jab");
    w.fighters[0].mov.as_mut().unwrap().t = 12;
    let mut f = w.fighters[0].clone();
    assert!(moves::trigger(&mut w, &d, 0, &mut f, Trigger::AttackHeld));
    assert_eq!(f.mov.as_ref().unwrap().phase, 1);
    w.fighters[0] = f;
    w.fighters[0].mov.as_mut().unwrap().t = 21;
    let mut f = w.fighters[0].clone();
    assert!(moves::trigger(&mut w, &d, 0, &mut f, Trigger::AttackHeld));
    assert_eq!(moves::phase(&d, &f).name, "rapidStart");
    w.fighters[0] = f;
    w.fighters[0].mov.as_mut().unwrap().t = 3;
    let mut f = w.fighters[0].clone();
    moves::trigger(&mut w, &d, 0, &mut f, Trigger::Timeout);
    assert_eq!(moves::phase(&d, &f).name, "rapid");
    assert_eq!(f.mov.as_ref().unwrap().t, 0);
    w.fighters[0] = f;
    w.fighters[0].mov.as_mut().unwrap().t = 2;
    let mut f = w.fighters[0].clone();
    assert!(moves::trigger(&mut w, &d, 0, &mut f, Trigger::Release));
    assert_eq!(moves::phase(&d, &f).name, "finish");
    f.mov.as_mut().unwrap().t = 3;
    assert_eq!(combat::active_boxes(&d, &f).iter().map(|b| b.hit.damage).collect::<Vec<_>>(), vec![3.0, 3.0]);
}

#[test]
fn throws_and_pummel_can_be_reached_from_a_capture() {
    let (d, mut w) = arena();
    w.fighters[1].x = 570.;
    w.fighters[1].y = w.fighters[0].y;
    w.fighters[0].face = 1.;
    start(&mut w, &d, "nspec");
    for _ in 0..24 {
        raw(&mut w, &d, (0, 0), 4);
    }
    assert_eq!(w.fighters[0].state, State::Hold);
    assert_eq!(w.fighters[1].state, State::Grabbed);
    for _ in 0..12 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert!(w.fighters[0].sd.timer > 0.);
    for _ in 0..12 {
        raw(&mut w, &d, (127, 0), 0);
    }
    assert_eq!(w.fighters[0].state, State::Throw);
    assert_eq!(w.fighters[0].sd.kind, "f");
    for _ in 0..20 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert!((w.fighters[1].dmg - 10.).abs() < 1e-9, "hammer tick + throw damage: {}", w.fighters[1].dmg);
    assert!(w.fighters[1].state != State::Grabbed);
}

#[test]
fn launch_grows_with_percent_weight_and_charge() {
    let (d, mut w) = arena();
    start(&mut w, &d, "jab");
    w.fighters[0].mov.as_mut().unwrap().t = 9;
    let weak = boxes(&d, &w).into_iter().next().unwrap().hit;
    start(&mut w, &d, "fsmash");
    set_phase(&mut w, &d, "main", 41);
    let strong = boxes(&d, &w).into_iter().find(|b| b.hit.damage > 20.).unwrap().hit;
    for percent in [0., 50., 100., 200.] {
        assert!(combat::knockback(&d, &strong, percent, 1.) > combat::knockback(&d, &weak, percent, 1.));
    }
    assert!(combat::knockback(&d, &strong, 100., 0.78740157) < combat::knockback(&d, &strong, 100., 1.5));
    let base = combat::knockback(&d, &strong, 100., 1.);
    w.fighters[0].mov.as_mut().unwrap().charge = 60;
    let charged = boxes(&d, &w).into_iter().find(|b| b.hit.damage > 20.).unwrap().hit;
    assert!(combat::knockback(&d, &charged, 100., 1.) > base);
}

#[test]
fn raw_input_routes_jump_aerials_and_all_specials() {
    let d = common::data();
    // Jump + attack is a short-hop aerial.
    let mut w = common::fighters(&d, "king", &["brawler"]);
    w.count = 0;
    for _ in 0..2 {
        raw(&mut w, &d, (0, 0), 0);
    }
    raw(&mut w, &d, (0, 0), 1);
    raw(&mut w, &d, (0, 0), 2);
    for _ in 0..12 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert_eq!(w.fighters[0].state, State::Attack);
    assert!(w.fighters[0].mov.as_ref().is_some_and(|m| m.slot == "nair"));

    // Direction + special selects side/up/down specials; plain special selects Inhale.
    for (stick, slot) in [((0, 0), "nspec"), ((127, 0), "sspec"), ((0, 127), "uspec"), ((0, -127), "dspec")] {
        let mut w = common::fighters(&d, "king", &["brawler"]);
        w.count = 0;
        raw(&mut w, &d, (0, 0), 0);
        raw(&mut w, &d, stick, 4);
        raw(&mut w, &d, stick, 4);
        assert_eq!(w.fighters[0].mov.as_ref().map(|m| m.slot.as_str()), Some(slot), "stick {stick:?}");
    }
}

#[test]
fn cpu_matches_finish_with_king_in_either_seat_on_every_stage() {
    let d = common::data();
    for stage in &d.stages {
        for seat in 0..2 {
            let ids = if seat == 0 { ["king", "brawler"] } else { ["brawler", "king"] };
            let players = ids.map(|id| PlayerConfig { cpu: Some(2), ..PlayerConfig::human(id) }).to_vec();
            let mut w = World::new(
                MatchConfig { stage: stage.id.clone(), seed: 3, stocks: 2, players, ..Default::default() },
                &d,
            )
            .unwrap();
            for _ in 0..30_000 {
                sim::step_world(&mut w, &d, &[]);
                w.fx.clear();
                w.hits.clear();
                w.acts.clear();
                if w.over {
                    break;
                }
            }
            assert!(w.over, "{} king seat {seat} did not finish", stage.id);
            assert!(w.fighters.iter().all(|f| f.x.is_finite() && f.y.is_finite()));
        }
    }
}

#[test]
fn jet_hammer_raw_movement_carries_partial_and_full_charge() {
    for ticks in [20, 140] {
        for direction in [-127, 127] {
            let (d, mut w) = arena();
            raw(&mut w, &d, (0, 0), 0);
            raw(&mut w, &d, (0, -127), 4);
            for _ in 0..17 + ticks {
                raw(&mut w, &d, (0, 0), 4);
            }
            let x = w.fighters[0].x;
            let face = w.fighters[0].face;
            let charge = w.fighters[0].mov.as_ref().unwrap().charge;
            for _ in 0..5 {
                raw(&mut w, &d, (direction, 0), 4);
            }
            let f = &w.fighters[0];
            assert_eq!(f.state, State::Attack);
            assert_eq!(f.mov.as_ref().unwrap().phase, 1);
            assert!(f.mov.as_ref().unwrap().charge >= charge);
            assert_eq!(f.face, face);
            assert!((f.x - x) * f64::from(direction) > 0.);
            raw(&mut w, &d, (direction, 0), 0);
            assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 2);
            assert!(w.fighters[0].mov.as_ref().unwrap().charge >= charge);
        }
    }
}

#[test]
fn jet_hammer_raw_release_preserves_charge_and_cannot_cancel_swing() {
    let (d, mut w) = arena();
    raw(&mut w, &d, (0, 0), 0);
    raw(&mut w, &d, (0, -127), 4);
    for _ in 0..37 {
        raw(&mut w, &d, (0, 0), 4);
    }
    let charge = w.fighters[0].mov.as_ref().unwrap().charge;
    raw(&mut w, &d, (0, 0), 0);
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 2);
    raw(&mut w, &d, (127, 0), 18);
    let m = w.fighters[0].mov.as_ref().unwrap();
    assert_eq!(m.phase, 2);
    assert_eq!(m.charge, charge);
}

#[test]
fn jet_hammer_raw_air_drift_preserves_charge() {
    let (d, mut w) = arena();
    raw(&mut w, &d, (0, 0), 0);
    raw(&mut w, &d, (0, -127), 4);
    for _ in 0..37 {
        raw(&mut w, &d, (0, 0), 4);
    }
    let charge = w.fighters[0].mov.as_ref().unwrap().charge;
    let f = &mut w.fighters[0];
    f.y = 100.;
    f.ground = false;
    f.surf = None;
    f.vx = 0.;
    for _ in 0..5 {
        raw(&mut w, &d, (127, 0), 4);
    }
    let f = &w.fighters[0];
    assert_eq!(f.state, State::Attack);
    assert!(f.vx > 0.);
    assert!(f.mov.as_ref().unwrap().charge >= charge);
}

#[test]
fn raw_jump_ascent_is_measured_and_midair_jumps_are_gentler() {
    let (d, mut w) = arena();
    raw(&mut w, &d, (0, 0), 0);
    raw(&mut w, &d, (0, 0), 1);
    for _ in 0..3 {
        raw(&mut w, &d, (0, 0), 1);
    }
    assert_eq!(w.fighters[0].state, State::Air);
    assert!((w.fighters[0].vy + 13.5).abs() < 0.001);
    raw(&mut w, &d, (0, 0), 0);
    raw(&mut w, &d, (0, 0), 1);
    assert_eq!(w.fighters[0].jumps, 3);
    assert!(w.fighters[0].vy < -12. && w.fighters[0].vy >= -13.);
    let (d, mut w) = arena();
    raw(&mut w, &d, (0, 0), 0);
    raw(&mut w, &d, (0, 0), 1);
    for _ in 0..3 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert_eq!(w.fighters[0].state, State::Air);
    assert!((w.fighters[0].vy + 9.4).abs() < 0.001);
}
