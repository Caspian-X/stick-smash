mod common;
use sim::{GameData, World, combat, input::Intent, moves, world::State};

fn setup(slot: &str) -> (GameData, World) {
    let d = common::data();
    let mut w = common::fighters(&d, "mike", &["brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].face = 1.;
    w.fighters[1].x = 950.;
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, slot);
    w.fighters[0] = f;
    (d, w)
}

// Advance only Mike's move: no opponent AI, hitlag or incidental hits can obscure frame boundaries.
fn tick(w: &mut World, d: &GameData, input: Intent) {
    let mut f = w.fighters[0].clone();
    moves::st_attack(w, d, 0, &mut f, input, input.dir());
    w.fighters[0] = f;
    w.frame += 1;
}

#[test]
fn imported_knockback_uses_damage_percent_and_reference_weight_formula() {
    let d = common::data();
    // Independent fixtures from the no-staleness/no-rage Ultimate equation, converted at the tuned .14 launch scale.
    let b = sim::data::Hitbox { damage: 19., bkb: 40., kbg: 0.85, ..Default::default() };
    for (after, expected) in [(19., 11.06567), (69., 19.81217), (119., 28.55867), (169., 37.30517)] {
        assert!((combat::knockback(&d, &b, after, 1.) - expected).abs() < 1e-9);
    }
    let light = combat::knockback(&d, &b, 119., 100. / 80.);
    let heavy = combat::knockback(&d, &b, 119., 100. / 120.);
    assert!(light > heavy);
    let mut weaker = b.clone();
    weaker.damage = 5.;
    assert!(combat::knockback(&d, &weaker, 119., 1.) < combat::knockback(&d, &b, 119., 1.));
    let f = d.fighter("mike").unwrap();
    assert_eq!((f.moves.fsmash.phases[0].hitboxes[0].bkb, f.moves.fsmash.phases[0].hitboxes[0].kbg), (40., 0.9));
    let mut w = common::fighters(&d, "mike", &["brawler"]);
    w.fighters[1].dmg = 100.;
    combat::apply_hit(
        &mut w,
        &d,
        1,
        &b,
        combat::HitSource { attacker: 0, direction: 1., point: (0., 0.), projectile: false, label: None },
    );
    assert_eq!(w.fighters[1].dmg, 119.);
    assert!((w.fighters[1].vx - 28.55867).abs() < 1e-9);
}

#[test]
fn quickdraw_charge_falls_and_horizontal_rush_hovers() {
    let (d, mut w) = setup("sspec");
    let f = &mut w.fighters[0];
    (f.x, f.y, f.ground, f.surf, f.vy) = (100., -1000., false, None, 0.);
    let gravity = d.fighters[f.data].stats.gravity;
    let terminal = d.fighters[f.data].stats.fall_speed;
    for n in 1..=40 {
        let y = w.fighters[0].y;
        tick(&mut w, &d, Intent { sp_held: true, ..Default::default() });
        assert!((w.fighters[0].vy - (gravity * f64::from(n)).min(terminal)).abs() < 1e-9);
        assert!(w.fighters[0].y > y);
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().charge, 25);
    let mut restored = w.clone();
    for _ in 0..4 {
        tick(&mut w, &d, Intent { sp_held: true, ..Default::default() });
        tick(&mut restored, &d, Intent { sp_held: true, ..Default::default() });
    }
    assert_eq!(format!("{w:?}"), format!("{restored:?}"));
    tick(&mut w, &d, Intent::default());
    assert_eq!(moves::phase(&d, &w.fighters[0]).name, "rush");
    assert_eq!(w.fighters[0].vy, 0.);
}

#[test]
fn smash_charge_scales_damage_without_double_counting_base_knockback() {
    let (d, mut w) = setup("fsmash");
    let m = w.fighters[0].mov.as_mut().unwrap();
    m.t = 31;
    let normal = combat::active_boxes(&d, &w.fighters[0])[0].hit.clone();
    w.fighters[0].mov.as_mut().unwrap().charge = 60;
    let charged = combat::active_boxes(&d, &w.fighters[0])[0].hit.clone();
    assert!((charged.damage - normal.damage * 1.4).abs() < 1e-9);
    assert_eq!(charged.bkb, normal.bkb);
    assert!(
        combat::knockback(&d, &charged, 100. + charged.damage, 1.)
            > combat::knockback(&d, &normal, 100. + normal.damage, 1.)
    );
}

#[test]
fn normal_attack_windows_and_damage_follow_the_reference() {
    // Absolute move frames, including initial aerial autocancel phases and both down-smash swings.
    for (slot, total, windows) in [
        ("jab", 23, vec![(4, 5, 2.5)]),
        ("ftilt", 41, vec![(12, 13, 12.5)]),
        ("utilt", 39, vec![(11, 16, 12.), (17, 21, 8.)]),
        ("dtilt", 28, vec![(7, 8, 8.)]),
        ("dashatk", 47, vec![(15, 19, 14.)]),
        ("fsmash", 83, vec![(31, 32, 19.), (33, 35, 22.)]),
        ("usmash", 70, vec![(25, 29, 17.), (30, 31, 10.)]),
        ("dsmash", 70, vec![(13, 15, 16.), (32, 33, 19.), (34, 36, 9.)]),
        ("nair", 59, vec![(10, 14, 7.5), (15, 22, 6.)]),
        ("fair", 51, vec![(11, 15, 13.)]),
        ("bair", 54, vec![(7, 9, 14.)]),
        ("uair", 56, vec![(13, 18, 11.)]),
        ("dair", 54, vec![(16, 17, 15.)]),
        ("grab", 36, vec![(7, 8, 0.)]),
        ("dashgrab", 44, vec![(10, 11, 0.)]),
    ] {
        let (d, mut w) = setup(slot);
        for frame in 0..total {
            let boxes = combat::active_boxes(&d, &w.fighters[0]);
            let expected = windows.iter().find(|(a, b, _)| (*a..=*b).contains(&frame));
            assert_eq!(!boxes.is_empty(), expected.is_some(), "{slot} frame {frame}");
            if let Some((_, _, damage)) = expected {
                assert!(boxes.iter().any(|b| b.hit.damage == *damage), "{slot} frame {frame}");
                assert!(
                    boxes
                        .iter()
                        .all(|b| b.hit.damage == *damage || slot == "fsmash" && frame >= 33 && b.hit.damage == 19.)
                );
            }
            tick(&mut w, &d, Intent::default());
        }
        assert_eq!(w.fighters[0].state, State::Idle, "{slot}: total duration");
    }
}

#[test]
fn holding_jab_chains_but_releasing_preserves_each_jabs_recovery() {
    for (release, end, last_phase) in [(0, 23, "jab1"), (10, 36, "jab2"), (20, 58, "jab3")] {
        let (d, mut w) = setup("jab");
        for frame in 1..end {
            tick(&mut w, &d, Intent { atk_held: frame <= release, ..Default::default() });
        }
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, last_phase);
        tick(&mut w, &d, Intent::default());
        assert_eq!(w.fighters[0].state, State::Idle);
    }
}

#[test]
fn eruption_charge_matches_startup_damage_and_adds_distant_columns() {
    for charge in [0_u32, 100, 209] {
        let (d, mut w) = setup("nspec");
        for frame in 0..30 + charge {
            assert!(combat::active_boxes(&d, &w.fighters[0]).is_empty(), "charge {charge}, frame {frame}");
            tick(&mut w, &d, Intent { sp_held: frame < 19 + charge, ..Default::default() });
        }
        let boxes = combat::active_boxes(&d, &w.fighters[0]);
        assert_eq!(boxes.len(), 3, "initial flame and sword hitboxes");
        assert_eq!(w.fx.iter().filter(|fx| matches!(fx, sim::Fx::Burst { .. })).count(), 1);
        let damage = 10. * (1. + f64::from(charge) * 2.5 / 209.);
        assert!(boxes.iter().all(|b| (b.hit.damage - damage).abs() < 1e-9));
        if charge == 209 {
            assert!((damage - 35.).abs() < 1e-9);
        }
        for frame in 31..=42 {
            tick(&mut w, &d, Intent::default());
            let boxes = combat::active_boxes(&d, &w.fighters[0]);
            if frame == 34 {
                assert_eq!(boxes.iter().any(|b| b.hit.x == 140.), charge >= 100);
            }
            if frame == 38 {
                assert_eq!(boxes.iter().any(|b| b.hit.x == 220.), charge == 209);
            }
        }
        let bursts = w.fx.iter().filter(|fx| matches!(fx, sim::Fx::Burst { .. })).count();
        assert_eq!(
            bursts,
            if charge == 0 {
                1
            } else if charge == 100 {
                2
            } else {
                3
            }
        );
        while w.fighters[0].mov.is_some() {
            tick(&mut w, &d, Intent::default());
        }
        assert_eq!(w.frame, u64::from(78 + charge));
    }
}

#[test]
fn quickdraw_charge_scales_rush_speed_and_strike_damage() {
    let mut speeds = vec![];
    for charge in [0, 69] {
        let (d, mut w) = setup("sspec");
        for frame in 0..16 + charge {
            tick(&mut w, &d, Intent { sp_held: frame < 15 + charge, ..Default::default() });
        }
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, "rush");
        speeds.push(w.fighters[0].vx);
        assert!(combat::active_boxes(&d, &w.fighters[0]).is_empty());
        w.fighters[1].x = w.fighters[0].x + 65.;
        tick(&mut w, &d, Intent::default());
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, "groundSwing");
        for _ in 0..2 {
            tick(&mut w, &d, Intent::default());
        }
        let boxes = combat::active_boxes(&d, &w.fighters[0]);
        assert!((boxes[0].hit.damage - if charge == 0 { 6. } else { 13. }).abs() < 1e-9);
        combat::hit_pass(&mut w, &d);
        assert!(w.hits.iter().any(|h| h.attacker == Some(0)), "Quickdraw must hit the detected target");
    }
    assert!(speeds[1] > speeds[0] * 1.8);
}

#[test]
fn quickdraw_detection_respects_facing_height_teams_and_invulnerability() {
    for case in ["behind", "above", "team", "invulnerable", "ahead", "left"] {
        let (d, mut w) = setup("sspec");
        if case == "left" {
            w.fighters[0].face = -1.;
        }
        // Place directly in rush to isolate target selection from charge and travel.
        w.fighters[0].mov.as_mut().unwrap().phase = 1;
        let (x, y) = (w.fighters[0].x, w.fighters[0].y);
        w.fighters[1].x = x + if case == "behind" || case == "left" { -55. } else { 55. };
        w.fighters[1].y = y - if case == "above" { 180. } else { 0. };
        if case == "team" {
            for p in &mut w.cfg.players {
                p.team = Some(0);
            }
        }
        if case == "invulnerable" {
            w.fighters[1].inv = 10;
        }
        tick(&mut w, &d, Intent::default());
        assert_eq!(moves::phase(&d, &w.fighters[0]).name == "groundSwing", matches!(case, "ahead" | "left"), "{case}");
    }
}

#[test]
fn aether_throws_then_rises_plunges_and_lands_with_a_separate_hit() {
    let (d, mut w) = setup("uspec");
    let mut seen = vec![];
    for frame in 1..150 {
        if w.fighters[0].mov.is_none() {
            break;
        }
        tick(&mut w, &d, Intent::default());
        let name = moves::phase(&d, &w.fighters[0]).name.as_str();
        if seen.last().copied() != Some(name) {
            seen.push(name);
        }
        if frame == 15 {
            assert_eq!(w.projs.len(), 1);
            assert_eq!(w.projs[0].hit.damage, 2.);
            assert!(combat::active_boxes(&d, &w.fighters[0]).iter().all(|b| b.hit.damage == 6.));
        }
        if frame == 35 {
            assert!(
                w.fighters[0].vy < -18. && !w.fighters[0].ground,
                "frame {frame} phase {name} y {} vy {}",
                w.fighters[0].y,
                w.fighters[0].vy
            );
        }
        if frame == 44 {
            assert_eq!(combat::active_boxes(&d, &w.fighters[0])[0].hit.damage, 1.);
        }
        if frame == 51 {
            assert_eq!(combat::active_boxes(&d, &w.fighters[0])[0].hit.damage, 3.);
        }
        if frame == 61 {
            assert!(w.fighters[0].vy > 17., "frame {frame} phase {name} y {} vy {}", w.fighters[0].y, w.fighters[0].vy);
        }
        if name == "impact" {
            assert!(w.fighters[0].ground);
            assert_eq!(combat::active_boxes(&d, &w.fighters[0])[0].hit.damage, 6.);
            return;
        }
    }
    panic!("Aether failed to complete its landing: {seen:?}");
}

#[test]
fn aerial_landing_lag_and_autocancel_boundaries_match_the_reference() {
    for (slot, lag, early, late) in
        [("nair", 8, 2, 50), ("fair", 14, 0, 42), ("bair", 11, 2, 35), ("uair", 9, 5, 51), ("dair", 14, 4, 48)]
    {
        for (frame, expected) in [(early, 0), (early + 1, lag), (late - 1, lag), (late, 0)] {
            // Forward air has no initial autocancel window.
            if slot == "fair" && frame == 0 {
                continue;
            }
            let (d, mut w) = setup(slot);
            for _ in 0..frame {
                tick(&mut w, &d, Intent::default());
            }
            let mut f = w.fighters[0].clone();
            assert!(moves::trigger(&mut w, &d, 0, &mut f, sim::data::Trigger::Land));
            assert_eq!(f.state, State::Landing);
            assert_eq!(f.sd.n, f64::from(expected), "{slot} frame {frame}");
        }
    }
}

#[test]
fn counter_only_catches_hits_inside_its_window_then_strikes_on_frame_four() {
    for frame in [8, 9, 33, 34] {
        let (d, mut w) = setup("dspec");
        w.fighters[0].mov.as_mut().unwrap().t = frame;
        assert_eq!(combat::countering(&d, &w.fighters[0]).is_some(), (9..=33).contains(&frame));
        if frame != 9 {
            continue;
        }
        w.fighters[1].x = 680.;
        w.fighters[1].y = 520.;
        w.fighters[1].face = -1.;
        let mut attacker = w.fighters[1].clone();
        moves::start_move(&mut w, &d, 1, &mut attacker, "jab");
        attacker.mov.as_mut().unwrap().t = 3;
        w.fighters[1] = attacker;
        combat::hit_pass(&mut w, &d);
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, "strike");
        for t in 1..=4 {
            tick(&mut w, &d, Intent::default());
            let boxes = combat::active_boxes(&d, &w.fighters[0]);
            assert_eq!(!boxes.is_empty(), t == 4);
            if t == 4 {
                assert!(boxes.iter().all(|b| b.hit.damage >= 10.));
            }
        }
    }
}

#[test]
fn aether_armor_starts_later_in_the_air_and_ends_after_launch() {
    for airborne in [false, true] {
        let (d, mut w) = setup("uspec");
        if airborne {
            w.fighters[0].ground = false;
            w.fighters[0].surf = None;
            w.fighters[0].y = 200.;
        }
        for frame in 0..=36 {
            let armored = moves::phase(&d, &w.fighters[0]).armor_threshold.is_some();
            assert_eq!(
                armored,
                (if airborne { 15 } else { 5 }..=35).contains(&frame),
                "airborne {airborne} frame {frame}"
            );
            if frame < 36 {
                tick(&mut w, &d, Intent::default());
            }
        }
    }
}

#[test]
fn quickdraw_whiff_forces_special_fall_but_air_hit_allows_more_recovery() {
    for connected in [false, true] {
        let (d, mut w) = setup("sspec");
        w.fighters[0].ground = false;
        w.fighters[0].surf = None;
        w.fighters[0].y = -300.;
        for _ in 0..16 {
            tick(&mut w, &d, Intent::default());
        }
        if connected {
            w.fighters[1].x = w.fighters[0].x + 55.;
            w.fighters[1].y = w.fighters[0].y;
            tick(&mut w, &d, Intent::default());
            assert_eq!(moves::phase(&d, &w.fighters[0]).name, "airSwing");
            for _ in 0..20 {
                tick(&mut w, &d, Intent::default());
            }
            assert_eq!(w.fighters[0].state, State::Air);
            assert!(!w.fighters[0].up_b);
        } else {
            w.fighters[1].x = -1000.;
            for _ in 0..37 {
                tick(&mut w, &d, Intent::default());
            }
            assert_eq!(moves::phase(&d, &w.fighters[0]).name, "specialFall");
            let mut f = w.fighters[0].clone();
            assert!(moves::trigger(&mut w, &d, 0, &mut f, sim::data::Trigger::Land));
            assert_eq!(f.sd.n, 30.);
        }
    }
}

#[test]
fn full_eruption_breaks_a_fresh_shield_and_distant_columns_really_hit() {
    for (charge, distance, shield) in [(0, 43., true), (209, 43., true), (209, 220., false)] {
        let (d, mut w) = setup("nspec");
        w.fighters[0].mov.as_mut().unwrap().t = if shield { 30 } else { 38 };
        w.fighters[0].mov.as_mut().unwrap().charge = charge;
        w.fighters[1].x = w.fighters[0].x + distance;
        w.fighters[1].y = 520.;
        if shield {
            w.fighters[1].set_state(State::Shield);
        }
        combat::hit_pass(&mut w, &d);
        if shield {
            assert_eq!(w.fighters[1].state == State::Shieldbreak, charge == 209);
        } else {
            assert_eq!(w.fighters[1].dmg, 35.);
        }
    }
}

#[test]
fn numeric_root_offsets_and_capsule_endpoints_translate_at_five_pixels_per_unit() {
    let (d, mut w) = setup("jab");
    for _ in 0..4 {
        tick(&mut w, &d, Intent::default());
    }
    let b = &combat::active_boxes(&d, &w.fighters[0])[0].hit;
    assert_eq!((b.x, b.y, b.r), (31., 47., 10.)); // top: Z=6.2, Y=9.4, Size=2
    let (d, mut w) = setup("ftilt");
    for _ in 0..12 {
        tick(&mut w, &d, Intent::default());
    }
    let boxes = combat::active_boxes(&d, &w.fighters[0]);
    assert_eq!((boxes.first().unwrap().hit.x, boxes.first().unwrap().hit.y), (50., 50.));
    assert_eq!((boxes.last().unwrap().hit.x, boxes.last().unwrap().hit.y), (120., 40.));
    assert!(boxes.iter().all(|b| b.hit.r == 20.));
}

#[test]
fn late_up_smash_only_hits_grounded_targets() {
    for ground in [false, true] {
        let (d, mut w) = setup("usmash");
        for _ in 0..30 {
            tick(&mut w, &d, Intent::default());
        }
        w.fighters[1].x = 600.;
        w.fighters[1].y = 520.;
        w.fighters[1].ground = ground;
        combat::hit_pass(&mut w, &d);
        assert_eq!(!w.hits.is_empty(), ground);
    }
}

#[test]
fn down_air_has_a_launching_upper_hit_and_a_spiking_lower_hit() {
    for (y, spike) in [(300., false), (410., true)] {
        let (d, mut w) = setup("dair");
        for _ in 0..16 {
            tick(&mut w, &d, Intent::default());
        }
        w.fighters[0].y = 300.;
        w.fighters[0].ground = false;
        w.fighters[1].x = 645.;
        w.fighters[1].y = y;
        w.fighters[1].ground = false;
        combat::hit_pass(&mut w, &d);
        assert!(!w.hits.is_empty());
        assert_eq!(w.fighters[1].vy > 0., spike);
    }
}
