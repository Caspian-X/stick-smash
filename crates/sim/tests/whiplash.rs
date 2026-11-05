mod common;
use sim::{
    GameData, Input, World, combat,
    data::{HitType, Trigger},
    input::{ATTACK, GRAB, Intent, JUMP, SPECIAL},
    moves,
    world::State,
};

fn setup(slot: &str) -> (GameData, World) {
    let mut d = common::data();
    for stage in &mut d.stages {
        stage.blast.top = -10000.;
        stage.blast.left = -10000.;
        stage.blast.right = 10000.;
    }
    let mut w = common::fighters(&d, "whiplash", &["brawler"]);
    w.count = 0;
    for (i, f) in w.fighters.iter_mut().enumerate() {
        f.x = 640. + i as f64 * 1000.;
        f.y = -1000.;
        f.ground = false;
        f.surf = None;
        f.face = 1.;
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
fn reference_normals_and_aerials_keep_their_gameplay_windows() {
    // UFD gameplay frames, independently specified from the generated phase data.
    for (slot, total, windows) in [
        ("jab", 23, vec![(1, 1, 1.5)]),
        ("ftilt", 28, vec![(6, 8, 7.)]),
        ("utilt", 45, vec![(3, 4, 5.), (9, 10, 7.)]),
        ("dtilt", 29, vec![(8, 9, 8.)]),
        ("dashatk", 51, vec![(7, 9, 8.), (10, 19, 5.)]),
        ("fsmash", 64, vec![(13, 14, 5.), (26, 28, 11.)]),
        (
            "usmash",
            48,
            vec![(10, 11, 4.), (14, 14, 0.8), (17, 17, 0.8), (20, 20, 0.8), (23, 23, 0.8), (26, 26, 0.8), (28, 29, 3.)],
        ),
        ("dsmash", 41, vec![(20, 24, 8.)]),
        ("nair", 51, vec![(10, 11, 8.), (15, 16, 8.)]),
        ("fair", 46, vec![(6, 7, 5.), (13, 14, 7.)]),
        ("bair", 40, vec![(8, 9, 12.)]),
        ("uair", 34, vec![(6, 12, 6.5)]),
        ("dair", 67, vec![(14, 49, 6.)]),
        ("zair", 49, vec![(9, 19, 5.)]),
        ("grab", 58, vec![(15, 25, 0.)]),
        ("dashgrab", 66, vec![(17, 26, 0.)]),
    ] {
        let (d, mut w) = setup(slot);
        for t in 0..total {
            let bs = combat::active_boxes(&d, &w.fighters[0]);
            let expected = windows.iter().find(|(a, b, _)| (*a..=*b).contains(&t));
            assert_eq!(!bs.is_empty(), expected.is_some(), "{slot} frame {t}: {bs:?}");
            if let Some((_, _, damage)) = expected {
                assert!(bs.iter().any(|b| (b.hit.damage - damage).abs() < 1e-9), "{slot} frame {t}: {bs:?}");
            }
            tick(&mut w, &d, Intent::default());
        }
        assert!(w.fighters[0].mov.is_none(), "{slot}: total {total}");
    }
}

#[test]
fn multihit_finishers_connect_after_the_last_linking_hit() {
    for (slot, hits) in [
        ("usmash", vec![(10, 4.), (14, 0.8), (17, 0.8), (20, 0.8), (23, 0.8), (26, 0.8), (28, 3.)]),
        ("uspec", vec![(6, 5.), (12, 1.3), (16, 1.3), (20, 1.3), (24, 1.3), (28, 1.3), (34, 4.)]),
    ] {
        let (d, mut w) = setup(slot);
        for t in 0..=hits.last().unwrap().0 {
            if let Some((_, damage)) = hits.iter().find(|(at, _)| *at == t) {
                let b = combat::active_boxes(&d, &w.fighters[0])[0].clone();
                let victim = &mut w.fighters[1];
                victim.set_state(State::Air);
                victim.x = b.x;
                victim.y = b.y + 40.;
                victim.inv = 0;
                victim.lag = 0;
                w.fighters[0].lag = 0;
                let before = w.fighters[1].dmg;
                combat::hit_pass(&mut w, &d);
                assert!((w.fighters[1].dmg - before - damage).abs() < 1e-9, "{slot}: hit on frame {t}");
            }
            tick(&mut w, &d, Intent::default());
        }
    }
}

#[test]
fn plasma_whip_preserves_each_window_and_the_upward_finisher_choice() {
    for rising in [false, true] {
        let (d, mut w) = setup("sspec");
        for t in 0..55 {
            let bs = combat::active_boxes(&d, &w.fighters[0]);
            let active = (22..=29).contains(&t)
                || if rising {
                    (31..=32).contains(&t) || (37..=38).contains(&t) || (42..=43).contains(&t) || (46..=47).contains(&t)
                } else {
                    (31..=35).contains(&t)
                };
            assert_eq!(!bs.is_empty(), active, "Plasma Whip frame {t}, rising={rising}");
            if t == 31 {
                assert!(bs.iter().all(|b| b.hit.damage == if rising { 4. } else { 8. }));
            }
            tick(&mut w, &d, Intent { up: rising, ..Default::default() });
        }
        assert_eq!(w.fighters[0].state, State::Air);
    }
}

#[test]
fn autocancel_and_landing_lag_boundaries_follow_the_reference() {
    for (slot, start, end, lag) in [
        ("nair", 4, 42, 8),
        ("fair", 4, 38, 10),
        ("bair", 4, 31, 10),
        ("uair", 0, 25, 5),
        ("dair", 4, 50, 24),
        ("zair", 10, 40, 8),
    ] {
        let mut cases = vec![(start, lag), (end - 1, lag), (end, 0)];
        if start > 0 {
            cases.push((start - 1, 0));
        }
        for (at, expected) in cases {
            let (d, mut w) = setup(slot);
            for _ in 0..at {
                tick(&mut w, &d, Intent::default());
            }
            let mut f = w.fighters[0].clone();
            f.ground = true;
            moves::trigger(&mut w, &d, 0, &mut f, Trigger::Land);
            if expected == 0 {
                assert_eq!(f.state, State::Idle, "{slot} landing frame {at}");
            } else if slot == "dair" {
                assert_eq!(f.state, State::Attack);
                assert_eq!(moves::phase(&d, &f).frames, Some(24));
            } else {
                assert_eq!(f.state, State::Landing, "{slot} landing frame {at}");
                assert_eq!(f.sd.n, f64::from(expected));
            }
        }
    }
}

#[test]
fn three_part_jab_requires_the_chain_input_and_has_separate_hit_registries() {
    let (d, mut w) = setup("jab");
    for _ in 0..5 {
        tick(&mut w, &d, Intent { atk_held: true, ..Default::default() });
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 0);
    tick(&mut w, &d, Intent { atk_held: true, ..Default::default() });
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 1);
    for _ in 0..2 {
        tick(&mut w, &d, Intent { atk_held: true, ..Default::default() });
    }
    assert!(combat::active_boxes(&d, &w.fighters[0]).iter().any(|b| b.hit.damage == 1.5));
    for _ in 0..4 {
        tick(&mut w, &d, Intent { atk_held: true, ..Default::default() });
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 2);
    for _ in 0..3 {
        tick(&mut w, &d, Intent::default());
    }
    assert!(combat::active_boxes(&d, &w.fighters[0]).iter().any(|b| b.hit.damage == 3.));
}

#[test]
fn smash_charge_preserves_bkb_and_scales_both_forward_smash_hits() {
    for (slot, hold, first, damage) in [("fsmash", 9, 13, 5.), ("usmash", 4, 10, 4.), ("dsmash", 15, 20, 8.)] {
        let (d, mut w) = setup(slot);
        for _ in 0..hold {
            tick(&mut w, &d, Intent::default());
        }
        for _ in 0..60 {
            tick(&mut w, &d, Intent { atk_held: true, ..Default::default() });
        }
        assert_eq!(w.fighters[0].mov.as_ref().unwrap().charge, 60, "{slot}");
        for _ in hold..first {
            tick(&mut w, &d, Intent::default());
        }
        let bs = combat::active_boxes(&d, &w.fighters[0]);
        assert!(bs.iter().any(|b| (b.hit.damage - damage * 1.4).abs() < 1e-9));
        let mut plain = w.fighters[0].clone();
        plain.mov.as_mut().unwrap().charge = 0;
        for (charged, normal) in bs.iter().zip(combat::active_boxes(&d, &plain)) {
            assert_eq!(charged.hit.bkb, normal.hit.bkb);
            assert_eq!(charged.hit.kbg, normal.hit.kbg);
        }
        if slot == "fsmash" {
            for _ in first..26 {
                tick(&mut w, &d, Intent::default());
            }
            assert!(combat::active_boxes(&d, &w.fighters[0]).iter().any(|b| (b.hit.damage - 15.4).abs() < 1e-9));
        }
    }
}

#[test]
fn jump_attack_and_aerial_grab_use_whiplashs_actual_aerial_kit() {
    let d = common::data();
    let mut w = common::fighters(&d, "whiplash", &["brawler"]);
    w.count = 0;
    raw(&mut w, &d, (0, 0), JUMP | ATTACK);
    for _ in 0..4 {
        raw(&mut w, &d, (0, 0), 0);
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "nair");
    w.fighters[0].set_state(State::Air);
    w.fighters[0].lag = 0;
    w.fighters[0].history = Default::default();
    raw(&mut w, &d, (0, 0), GRAB);
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "zair");
}

#[test]
fn flip_jump_is_intangible_only_on_frames_three_through_twelve() {
    let (d, mut w) = setup("dspec");
    assert_eq!(w.fighters[0].inv, 0);
    for frame in 1..=13 {
        raw(&mut w, &d, (0, 0), 0);
        assert_eq!(w.fighters[0].inv > 0, (3..=12).contains(&frame), "frame {frame}");
    }
    assert_eq!(w.fighters[0].jumps, d.fighter("whiplash").unwrap().stats.air_jumps);
    assert!(!w.fighters[0].up_b);
}

#[test]
fn flip_kick_accepts_new_attack_or_special_from_frame_fourteen() {
    for special in [false, true] {
        let (d, mut w) = setup("dspec");
        for _ in 0..12 {
            tick(&mut w, &d, Intent::default());
        }
        let press = Intent { atk: !special, sp: special, ..Default::default() };
        tick(&mut w, &d, press);
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, "flip");
        tick(&mut w, &d, press);
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, "kick");
        for frame in 1..=13 {
            tick(&mut w, &d, Intent::default());
            let bs = combat::active_boxes(&d, &w.fighters[0]);
            assert_eq!(!bs.is_empty(), (9..=12).contains(&frame), "kick frame {frame}");
            if !bs.is_empty() {
                assert!(bs.iter().all(|b| b.hit.damage == 14.));
            }
        }
    }
}

#[test]
fn flip_kick_consumes_a_press_buffered_just_before_its_window() {
    for button in [ATTACK, SPECIAL] {
        let (d, mut w) = setup("dspec");
        for _ in 0..12 {
            raw(&mut w, &d, (0, 0), 0);
        }
        raw(&mut w, &d, (0, 0), button);
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, "flip");
        assert!(w.fighters[0].buffer.cmd.is_some());
        common::step_preserving(&mut w, &d, &[Input::default(), Input::default()]);
        assert_eq!(moves::phase(&d, &w.fighters[0]).name, "kick");
        let action = w
            .acts
            .iter()
            .find(|a| a.player == 0 && matches!(&a.action, sim::actions::Action::Followup { slot, phase } if slot == "dspec" && phase == "kick"))
            .unwrap();
        assert_eq!(action.pressed, Some(w.frame - 1));
        assert!(w.fighters[0].buffer.cmd.is_none());
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        for _ in 0..65 {
            common::step_preserving(&mut w, &d, &[Input::default(), Input::default()]);
            assert!(
                !w.acts.iter().any(|a| a.player == 0 && a.action.command()),
                "no leftover aerial or special after kick recovery"
            );
            w.fx.clear();
            w.hits.clear();
            w.acts.clear();
        }
        assert_eq!(w.fighters[0].state, State::Air);
    }
}

#[test]
fn flip_jump_resource_survives_hits_and_does_not_spend_the_double_jump() {
    let (d, mut w) = setup("dspec");
    let jumps = w.fighters[0].jumps;
    assert_eq!(w.fighters[0].used_air_moves, ["dspec"]);
    w.fighters[0].set_state(State::Air);
    let mut f = w.fighters[0].clone();
    moves::special(&mut w, &d, 0, &mut f, Intent { down: true, sp: true, ..Default::default() }, 0.);
    assert_eq!(f.state, State::Air, "the same airborne Flip Jump cannot repeat");
    w.fighters[0] = f;
    raw(&mut w, &d, (0, 0), JUMP);
    assert_eq!(w.fighters[0].jumps, jumps - 1, "Flip Jump leaves the normal double jump available");

    let hit = d.fighter("brawler").unwrap().moves.jab.phases[0].hitboxes[0].clone();
    combat::apply_hit(
        &mut w,
        &d,
        0,
        &hit,
        combat::HitSource { attacker: 1, direction: 1., point: (640., -1040.), projectile: false, label: None },
    );
    assert_eq!(w.fighters[0].used_air_moves, ["dspec"], "being hit does not restore Flip Jump");
    let mut f = w.fighters[0].clone();
    let block = &d.stages[w.stage].blocks[0];
    f.x = (block.left + block.right) / 2.;
    f.y = block.top;
    f.ground = true;
    f.surf = Some(0);
    f.vy = 0.;
    sim::physics::on_land(&mut w, &d, 0, &mut f, 0.);
    assert!(f.used_air_moves.is_empty(), "landing restores Flip Jump");
}

#[test]
fn boost_kick_has_the_post_patch_startup_and_multihit_windows() {
    let (d, mut w) = setup("uspec");
    for frame in 0..77 {
        let bs = combat::active_boxes(&d, &w.fighters[0]);
        let expected = frame == 6 || (12..=24).contains(&frame) || frame == 28 || (34..=35).contains(&frame);
        assert_eq!(!bs.is_empty(), expected, "Boost Kick frame {frame}: {bs:?}");
        if frame == 34 {
            assert!(bs.iter().any(|b| b.hit.damage == 4.));
        }
        tick(&mut w, &d, Intent::default());
    }
    assert_eq!(w.fighters[0].state, State::Helpless);
}

#[test]
fn charged_paralyzer_scales_the_projectile_and_freezes_before_launching() {
    let (d, w) = setup("nspec");
    let def = d.fighter("whiplash").unwrap();
    let id = def.projectiles.keys().next().unwrap();
    let mut freezes = Vec::new();
    for (charge, damage, radius, life) in [(0, 4., 6.5, 24), (22, 6., 10., 48)] {
        for ground in [false, true] {
            let mut w = w.clone();
            w.fighters[0].mov.as_mut().unwrap().charge = charge;
            let f = w.fighters[0].clone();
            sim::projectiles::spawn(&mut w, &d, 0, &f, id);
            let p = w.projs.last().unwrap();
            assert!((p.hit.damage - damage).abs() < 1e-9);
            assert!((p.r - radius).abs() < 1e-9);
            assert_eq!(p.life, life);
            let hit = p.hit.clone();
            assert_eq!(hit.kind, HitType::Paralyze);
            w.fighters[1].ground = ground;
            let source = || combat::HitSource {
                attacker: 0,
                direction: 1.,
                point: (1640., -1040.),
                projectile: true,
                label: None,
            };
            combat::apply_hit(&mut w, &d, 1, &hit, source());
            assert_eq!(w.fighters[1].dmg, damage);
            assert!(w.fighters[1].paralyze_used);
            let freeze = w.fighters[1].lag;
            assert!(freeze > 10);
            let at = (w.fighters[1].x, w.fighters[1].y);
            for _ in 0..freeze {
                raw(&mut w, &d, (0, 0), 0);
                assert_eq!((w.fighters[1].x, w.fighters[1].y), at);
            }
            assert_eq!(w.fighters[1].lag, 0);
            combat::apply_hit(&mut w, &d, 1, &hit, source());
            assert!(w.fighters[1].lag < freeze, "a second Paralyzer hit launches instead of re-paralyzing");
            freezes.push(freeze);
        }
    }
    assert!(freezes[2] > freezes[0]);
    assert_eq!(freezes[0], freezes[1]);
    assert_eq!(freezes[2], freezes[3]);
}

#[test]
fn paralyzer_hold_changes_the_actual_shoot_and_recovery_timing() {
    for (held, first, total, damage) in [(false, 21, 48, 4.), (true, 43, 70, 6.)] {
        let (d, mut w) = setup("nspec");
        let mut shot = None;
        for frame in 1..=total {
            raw(&mut w, &d, (0, 0), if held { SPECIAL } else { 0 });
            if shot.is_none() && !w.projs.is_empty() {
                shot = Some(frame);
                assert!((w.projs[0].hit.damage - damage).abs() < 1e-9);
            }
            assert_eq!(w.fighters[0].mov.is_some(), frame < total, "held {held}, frame {frame}");
        }
        assert_eq!(shot, Some(first), "held {held}");
    }
}

#[test]
fn flip_footstool_buries_grounded_targets_and_spikes_airborne_targets() {
    let (d, w) = setup("dspec");
    let def = d.fighter("whiplash").unwrap();
    let all: Vec<_> = def.moves.dspec.phases.iter().flat_map(|p| &p.hitboxes).collect();
    let bury = all.iter().find(|b| b.kind == HitType::Bury).unwrap();
    let spike = all.iter().find(|b| b.kind == HitType::Spike && b.damage == 8.).unwrap();
    assert_eq!(bury.damage, 8.);
    assert!(bury.targets(true) && !bury.targets(false));
    assert!(spike.targets(false) && !spike.targets(true));
    for (ground, hit) in [(true, *bury), (false, *spike)] {
        let mut w = w.clone();
        w.fighters[1].ground = ground;
        combat::apply_hit(
            &mut w,
            &d,
            1,
            hit,
            combat::HitSource { attacker: 0, direction: 1., point: (1640., -1040.), projectile: false, label: None },
        );
        if ground {
            assert_eq!(w.fighters[1].state, State::Buried);
            assert_eq!((w.fighters[1].vx, w.fighters[1].vy), (0., 0.));
            assert!(w.fighters[1].sd.n > 0.);
        } else {
            assert_eq!(w.fighters[1].state, State::Hitstun);
            assert!(w.fighters[1].vy > 0.);
        }
    }
}

#[test]
fn down_air_landing_burst_has_its_own_damage_and_three_frame_window() {
    let (d, mut w) = setup("dair");
    for _ in 0..20 {
        tick(&mut w, &d, Intent::default());
    }
    let mut f = w.fighters[0].clone();
    let block = &d.stages[w.stage].blocks[0];
    f.x = (block.left + block.right) / 2.;
    f.y = block.top;
    f.ground = true;
    f.surf = Some(0);
    f.vy = 0.;
    moves::trigger(&mut w, &d, 0, &mut f, Trigger::Land);
    w.fighters[0] = f;
    for frame in 0..24 {
        let bs = combat::active_boxes(&d, &w.fighters[0]);
        assert_eq!(!bs.is_empty(), (1..=3).contains(&frame), "landing frame {frame}");
        assert!(bs.iter().all(|b| b.hit.damage == 5.5));
        tick(&mut w, &d, Intent::default());
    }
    assert_eq!(w.fighters[0].state, State::Idle);
}

#[test]
fn mid_flip_and_paralysis_snapshots_resume_deterministically() {
    let (d, mut w) = setup("dspec");
    for _ in 0..10 {
        raw(&mut w, &d, (0, 0), 0);
    }
    let mut restored: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    for frame in 0..45 {
        let buttons = if frame == 4 { ATTACK } else { 0 };
        raw(&mut w, &d, (0, 0), buttons);
        raw(&mut restored, &d, (0, 0), buttons);
    }
    assert_eq!(w, restored);
    assert_eq!(w.fighters[0].used_air_moves, ["dspec"]);

    let id = d.fighter("whiplash").unwrap().projectiles.keys().next().unwrap();
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "nspec");
    f.mov.as_mut().unwrap().charge = 22;
    sim::projectiles::spawn(&mut w, &d, 0, &f, id);
    let hit = w.projs.last().unwrap().hit.clone();
    combat::apply_hit(
        &mut w,
        &d,
        1,
        &hit,
        combat::HitSource { attacker: 0, direction: 1., point: (1640., -1040.), projectile: true, label: None },
    );
    assert!(w.fighters[1].paralyze_used && w.fighters[1].lag > 0);
    let mut restored: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    for _ in 0..100 {
        raw(&mut w, &d, (0, 0), 0);
        raw(&mut restored, &d, (0, 0), 0);
    }
    assert_eq!(w, restored);
}

#[test]
fn back_air_retains_source_weight_and_percent_scaling() {
    let (d, mut w) = setup("bair");
    for _ in 0..8 {
        tick(&mut w, &d, Intent::default());
    }
    let hit = combat::active_boxes(&d, &w.fighters[0]).into_iter().find(|b| b.hit.damage == 12.).unwrap().hit;
    assert_eq!((hit.bkb, hit.kbg, hit.kind, hit.dir), (42., 0.98, HitType::Sakurai, sim::data::LaunchDir::Back));
    let speed = d.global.knockback.ultimate.as_ref().unwrap().launch_speed;
    for percent in [0., 80., 150.] {
        for weight in [80., 100., 125.] {
            let post_hit = percent + 12.;
            let expected =
                (((post_hit / 10. + post_hit * 12. / 20.) * 200. / (weight + 100.) * 1.4 + 18.) * 0.98 + 42.) * speed;
            assert!((combat::knockback(&d, &hit, post_hit, 100. / weight) - expected).abs() < 1e-9);
        }
    }
    assert!((d.fighter("whiplash").unwrap().stats.weight - 1.25).abs() < 1e-9);
}

#[test]
fn tether_reaches_a_distant_ledge_only_in_the_zair_window() {
    for (distance, frame, catches) in
        [(100., 1, false), (100., 2, true), (100., 39, true), (100., 40, false), (211., 2, false)]
    {
        let (d, mut w) = setup("zair");
        for _ in 0..frame - 1 {
            tick(&mut w, &d, Intent::default());
        }
        let block = &d.stages[w.stage].blocks[0];
        let f = &mut w.fighters[0];
        f.x = block.left - distance;
        f.y = block.top + 40.;
        f.vx = 0.;
        f.vy = -d.fighter("whiplash").unwrap().stats.gravity;
        f.face = 1.;
        f.ledge_cd = 0;
        tick(&mut w, &d, Intent::default());
        assert_eq!(w.fighters[0].state == State::Ledge, catches, "distance {distance}, frame {frame}");
    }
}

#[test]
fn flip_footstool_detection_and_manual_kick_contacts_enter_their_bounces() {
    let (d, mut w) = setup("dspec");
    for _ in 0..18 {
        tick(&mut w, &d, Intent::default());
    }
    let (x, y) = (w.fighters[0].x, w.fighters[0].y);
    w.fighters[1].x = x;
    w.fighters[1].y = y + 45.;
    w.fighters[1].ground = false;
    tick(&mut w, &d, Intent::default());
    assert_eq!(moves::phase(&d, &w.fighters[0]).name, "stomp");
    combat::hit_pass(&mut w, &d);
    assert_eq!(w.fighters[1].dmg, 8.);
    assert!(w.fighters[1].vy > 0.);
    assert!(w.fighters[0].vy < 0.);
    assert_eq!(moves::phase(&d, &w.fighters[0]).frames, Some(58));

    let (d, mut w) = setup("dspec");
    for _ in 0..13 {
        tick(&mut w, &d, Intent::default());
    }
    tick(&mut w, &d, Intent { atk: true, ..Default::default() });
    for _ in 0..9 {
        tick(&mut w, &d, Intent::default());
    }
    let bs = combat::active_boxes(&d, &w.fighters[0]);
    let hit = bs.iter().find(|b| b.hit.kind == HitType::Spike).unwrap();
    w.fighters[1].x = hit.x;
    w.fighters[1].y = hit.y + 30.;
    w.fighters[1].ground = false;
    combat::hit_pass(&mut w, &d);
    assert_eq!(w.fighters[1].dmg, 14.);
    assert_eq!(moves::phase(&d, &w.fighters[0]).name, "kickBounce");
    assert_eq!(moves::phase(&d, &w.fighters[0]).frames, Some(49));
    assert!(w.fighters[0].vy < 0.);
}

#[test]
fn raw_flip_steering_can_brake_reverse_and_extend_the_arc_without_turning() {
    for face in [-1., 1.] {
        let (d, mut base) = setup("dspec");
        base.fighters[0].face = face;
        base.fighters[0].vx = 7.5 * 5. / 3.5 * face; // the fitted Flip Jump launch (7.5 at the old 3.5 scale)
        let run = |x: i8| {
            let mut w = base.clone();
            for _ in 0..16 {
                raw(&mut w, &d, (x, 0), 0);
            }
            w
        };
        let forward = run((127. * face) as i8);
        let neutral = run(0);
        let backward = run((-127. * face) as i8);
        assert!(backward.fighters[0].vx * face < 0., "pulling back reverses travel before the apex");
        assert!((forward.fighters[0].x - neutral.fighters[0].x) * face >= 0.);
        assert!((neutral.fighters[0].x - backward.fighters[0].x) * face > 45.);
        for w in [&forward, &neutral, &backward] {
            assert_eq!(w.fighters[0].face, face);
            assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 0);
            assert_eq!(w.fighters[0].vy, neutral.fighters[0].vy);
        }
    }
}

#[test]
fn flip_kick_visibly_coils_then_extends_the_foot_at_the_active_window() {
    let (d, mut w) = setup("dspec");
    let f = &mut w.fighters[0];
    let m = f.mov.as_mut().unwrap();
    m.phase = 1;
    m.t = 7;
    let coiled = sim::pose::fighter_pose(&d, f, w.frame);
    f.mov.as_mut().unwrap().t = 9;
    let strike = sim::pose::fighter_pose(&d, f, w.frame);
    assert!(coiled.foot_f[0] < 20. && coiled.foot_f[1] > 20.);
    assert_eq!(strike.foot_f, [61., -16.]);
    let boxes = combat::active_boxes(&d, f);
    assert!(boxes.iter().any(|b| libm::hypot(b.hit.x - 61., b.hit.y + 16.) < b.hit.r));
    f.mov.as_mut().unwrap().t = 18;
    let recoil = sim::pose::fighter_pose(&d, f, w.frame);
    assert!(recoil.foot_f[0] < 20.);
}
