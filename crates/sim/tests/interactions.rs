mod common;
use sim::data::fighter::{GrabInteraction, Interactions, LedgeInteraction};
use sim::{combat, input::Intent, interactions as geo, states::update_fighter, world::State};

fn overrides(d: &mut sim::GameData, holder: usize, victim: usize) {
    d.fighters[holder].interactions = Some(Interactions {
        grab: Some(GrabInteraction {
            catch_point: Some([4., 80.]),
            hold_point: Some([39., 75.]),
            ..Default::default()
        }),
        ..Default::default()
    });
    d.fighters[victim].body.half_width = 7.;
    d.fighters[victim].body.height = 24.;
    d.fighters[victim].rig =
        Some(serde_json::from_str(include_str!("../../../assets/templates/rigs/quadruped.json")).unwrap());
    d.fighters[victim].interactions = Some(Interactions {
        grab: Some(GrabInteraction { held_anchor: Some([13., 21.]), ..Default::default() }),
        ..Default::default()
    });
}
fn anchors(d: &sim::GameData, w: &sim::World, a: usize, v: usize) {
    assert_eq!(
        geo::forward_point(&w.fighters[a], geo::hold_point(d, &w.fighters[a])),
        geo::forward_point(&w.fighters[v], geo::held_anchor(d, &w.fighters[v]))
    );
}
#[test]
fn asymmetric_hold_both_facings_indices_current_holder_physics_and_detachment() {
    let mut d = common::data();
    for a in [0, 1] {
        for face in [-1., 1.] {
            let mut w = common::fighters(&d, "brawler", &["ninja"]);
            let v = 1 - a;
            overrides(&mut d, w.fighters[a].data, w.fighters[v].data);
            w.fighters[a].face = face;
            w.fighters[v].surf = Some(3);
            combat::start_hold(&mut w, &d, a, v);
            anchors(&d, &w, a, v);
            assert_eq!(w.fighters[v].surf, None);
            w.fighters[a].vx = 3.;
            let old = w.fighters[a].x;
            update_fighter(&mut w, &d, a, Intent::default());
            assert_ne!(w.fighters[a].x, old);
            anchors(&d, &w, a, v);
            let timer = w.fighters[a].sd.timer;
            update_fighter(&mut w, &d, v, Intent { any_p: true, ..Default::default() });
            assert_eq!(w.fighters[a].sd.timer, timer - d.global.grab.mash_per_press);
            w.fighters[a].sd.timer = 0.;
            update_fighter(&mut w, &d, a, Intent::default());
            assert_eq!(w.fighters[v].state, State::Air);
            assert_eq!(w.fighters[v].surf, None);
        }
    }
}
#[test]
fn vertical_points_and_validation_paths() {
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &["ninja"]);
    overrides(&mut d, w.fighters[0].data, w.fighters[1].data);
    w.fighters[0].y = 400.;
    let tolerance = d.global.grab.height_tolerance;
    for delta in [tolerance - 0.001, tolerance, tolerance + 0.001] {
        w.fighters[1].y = 341. + delta;
        let dist = geo::grab_vertical_distance(&d, &w.fighters[0], &w.fighters[1]);
        assert!((dist - delta).abs() < 1e-10);
    }
    let f = &mut d.fighters[w.fighters[0].data];
    f.interactions.as_mut().unwrap().grab.as_mut().unwrap().hold_point = Some([f64::NAN, 0.]);
    assert!(
        sim::data::load::check_fighter("test", f).unwrap_err().to_string().contains("interactions.grab.holdPoint[0]")
    );
}
#[test]
fn throws_release_at_current_tick_and_clone_restore() {
    for enabled in [false, true] {
        for face in [-1., 1.] {
            for a in [0, 1] {
                for kind in ["f", "b", "u", "d"] {
                    let mut d = common::data();
                    let v = 1 - a;
                    let mut w = common::fighters(&d, "brawler", &["ninja"]);
                    if enabled {
                        overrides(&mut d, w.fighters[a].data, w.fighters[v].data);
                    }
                    w.fighters[a].face = face;
                    combat::start_hold(&mut w, &d, a, v);
                    if !enabled {
                        assert_eq!(w.fighters[v].x, w.fighters[a].x + face * d.global.sim.grab_offset);
                    }
                    w.fighters[a].set_state(State::Throw);
                    w.fighters[a].sd.victim = Some(v);
                    w.fighters[a].sd.kind = kind.into();
                    let ts = &d.fighters[w.fighters[a].data].throws;
                    let tt = match kind {
                        "b" => &ts.b,
                        "u" => &ts.u,
                        "d" => &ts.d,
                        _ => &ts.f,
                    };
                    let release = tt.release;
                    w.fighters[a].vx = 3.;
                    w.fighters[a].t = release / 2 - 1;
                    let old_x = w.fighters[a].x;
                    update_fighter(&mut w, &d, a, Intent::default());
                    assert_ne!(w.fighters[a].x, old_x);
                    let p = f64::from(w.fighters[a].t) / f64::from(release);
                    let h = geo::hold_point(&d, &w.fighters[a]);
                    let r = &d.global.sim;
                    let target = match kind {
                        "f" => [h[0] + p * r.throw_forward, h[1]],
                        "b" => [
                            h[0] * libm::cos(p * std::f64::consts::PI),
                            h[1] + r.throw_back_height * libm::sin(p * std::f64::consts::PI),
                        ],
                        "u" => [h[0] * (1. - p), h[1] + r.throw_up_height * p],
                        _ => [r.throw_down_offset, h[1]],
                    };
                    let expected = geo::forward_point(&w.fighters[a], target);
                    let actual = geo::forward_point(&w.fighters[v], geo::held_anchor(&d, &w.fighters[v]));
                    assert!((expected[0] - actual[0]).abs() < 1e-9 && (expected[1] - actual[1]).abs() < 1e-9);
                    w.fighters[a].t = release - 1;
                    let mut restored = w.clone();
                    let inputs = [sim::Input::default(); 2];
                    sim::step_world(&mut w, &d, &inputs);
                    sim::step_world(&mut restored, &d, &inputs);
                    assert_eq!(serde_json::to_value(&w).unwrap(), serde_json::to_value(&restored).unwrap());
                    assert_ne!(w.fighters[v].state, State::Grabbed);
                    let h = geo::hold_point(&d, &w.fighters[a]);
                    let r = &d.global.sim;
                    let target = match kind {
                        "f" => [h[0] + r.throw_forward, h[1]],
                        "b" => [-h[0], h[1] + r.throw_back_height * libm::sin(std::f64::consts::PI)],
                        "u" => [0., h[1] + r.throw_up_height],
                        _ => [r.throw_down_offset, h[1]],
                    };
                    let expected = geo::forward_point(&w.fighters[a], target);
                    let actual = geo::forward_point(&w.fighters[v], geo::held_anchor(&d, &w.fighters[v]));
                    assert!(
                        (expected[0] - actual[0]).abs() < 1e-9 && (expected[1] - actual[1]).abs() < 1e-9,
                        "{kind}: {expected:?} {actual:?}"
                    );
                }
            }
        }
    }
}
#[test]
fn mirrored_hang_clearance_and_climb_segments() {
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &[]);
    let fd = w.fighters[0].data;
    d.fighters[fd].interactions = Some(Interactions {
        ledge: Some(LedgeInteraction {
            hang_anchor: Some([26., 32.]),
            pull_in: Some(19.),
            climb_distance: Some(51.),
            roll_distance: Some(67.),
            attack_distance: Some(43.),
            ..Default::default()
        }),
        ..Default::default()
    });
    for side in [-1., 1.] {
        for kind in ["climb", "roll", "attack", "jump"] {
            let s = &d.stages[w.stage];
            let b = &s.blocks[0];
            let ex = if side < 0. { b.left } else { b.right };
            let f = &mut w.fighters[0];
            f.face = -side;
            f.set_state(State::Ledge);
            f.sd.side = side;
            f.sd.ledge_block = 0;
            update_fighter(&mut w, &d, 0, Intent::default());
            assert_eq!(geo::forward_point(&w.fighters[0], [26., 32.]), [ex, b.top]);
            let c = match kind {
                "roll" => &d.global.ledge.roll,
                "attack" => &d.global.ledge.attack,
                "jump" => &d.global.ledge.jump,
                _ => &d.global.ledge.climb,
            };
            let f = &mut w.fighters[0];
            f.set_state(State::Climb);
            f.sd.side = side;
            f.sd.ledge_block = 0;
            f.sd.kind = kind.into();
            f.t = c.a - 1;
            update_fighter(&mut w, &d, 0, Intent::default());
            assert_eq!(w.fighters[0].x, ex - side * 19.);
            assert_eq!(w.fighters[0].y, b.top);
            let mut restored = w.clone();
            sim::step_world(&mut w, &d, &[sim::Input::default()]);
            sim::step_world(&mut restored, &d, &[sim::Input::default()]);
            assert_eq!(w, restored);
            if kind == "jump" {
                assert_eq!(w.fighters[0].state, State::Air);
                assert_eq!(w.fighters[0].vy, -d.global.ledge.jump_speed);
            } else {
                w.fighters[0].t = c.b - 1;
                update_fighter(&mut w, &d, 0, Intent::default());
                let distance = match kind {
                    "roll" => 67.,
                    "attack" => 43.,
                    _ => 51.,
                };
                assert_eq!(w.fighters[0].x, ex - side * distance);
                assert_eq!(w.fighters[0].surf, Some(0));
            }
        }
    }
    d.fighters[fd].interactions.as_mut().unwrap().ledge.as_mut().unwrap().hang_anchor = Some([-20., 32.]);
    let err =
        sim::data::load::check_stage("test", &d.stages[w.stage], &d.fighters, d.global.sim.platform_pad).unwrap_err();
    assert!(err.to_string().contains("hanging body intersects"));
}

#[test]
fn moving_holder_captive_support_and_pummel_hitlag_order() {
    let mut d = common::data();
    let players = vec![sim::PlayerConfig::human("brawler"), sim::PlayerConfig::human("ninja")];
    let mut w = common::sandbox(&d, "foundry", players);
    overrides(&mut d, w.fighters[0].data, w.fighters[1].data);
    let s = &d.stages[w.stage];
    let p = s.platforms.iter().position(|p| p.motion.is_some()).unwrap();
    let surf = s.blocks.len() + p;
    let (x1, x2, y) = s.platforms[p].position(w.frame as f64);
    w.fighters[0].x = (x1 + x2) * 0.5;
    w.fighters[0].y = y;
    w.fighters[0].ground = true;
    w.fighters[0].surf = Some(surf);
    w.fighters[1].surf = Some(surf);
    combat::start_hold(&mut w, &d, 0, 1);
    assert_eq!(w.fighters[1].surf, None);
    w.fighters[0].sd.pt = d.global.grab.pummel_hit_frame + 1;
    let inputs = [sim::Input::default(); 2];
    sim::step_world(&mut w, &d, &inputs);
    assert_eq!(w.fighters[0].lag, d.global.grab.pummel_hitlag);
    anchors(&d, &w, 0, 1);
    let captive = (w.fighters[1].x, w.fighters[1].y);
    let mut restored = w.clone();
    for _ in 0..d.global.grab.pummel_hitlag {
        sim::step_world(&mut w, &d, &inputs);
        sim::step_world(&mut restored, &d, &inputs);
        assert_eq!(w, restored);
        assert_eq!((w.fighters[1].x, w.fighters[1].y), captive);
        assert_eq!(w.fighters[1].surf, None);
    }
    sim::step_world(&mut w, &d, &inputs);
    anchors(&d, &w, 0, 1);
}

#[test]
fn legacy_continuing_hold_and_ledge_defaults_are_exact() {
    let d = common::data();
    let mut w = common::fighters(&d, "brawler", &["ninja"]);
    for face in [-1., 1.] {
        w.fighters[0].face = face;
        combat::start_hold(&mut w, &d, 0, 1);
        for _ in 0..4 {
            update_fighter(&mut w, &d, 0, Intent::default());
            assert_eq!(w.fighters[1].x, w.fighters[0].x + face * d.global.sim.grab_offset);
            assert_eq!(w.fighters[1].y, w.fighters[0].y);
        }
    }
    for side in [-1., 1.] {
        let s = &d.stages[w.stage];
        let b = &s.blocks[0];
        let ex = if side < 0. { b.left } else { b.right };
        assert_eq!(
            geo::ledge_hang_root(&d.fighters[w.fighters[0].data], s, ex, b.top, side),
            [ex + side * s.ledge.hang_x, b.top + s.ledge.hang_y]
        );
    }
}

#[test]
fn interaction_inheritance_null_removal_and_distance_errors() {
    let mut d = common::data();
    let f = &mut d.fighters[0];
    let mut value = serde_json::to_value(&*f).unwrap();
    sim::data::json::merge(
        &mut value,
        &serde_json::json!({"interactions":{"grab":{"holdPoint":[34,71],"heldAnchor":[7,23]},"ledge":{"pullIn":17,"climbDistance":54}}}),
    );
    sim::data::json::merge(
        &mut value,
        &serde_json::json!({"interactions":{"grab":{"holdPoint":null},"ledge":{"climbDistance":null,"rollDistance":81}}}),
    );
    let mut inherited: sim::data::Fighter = serde_json::from_value(value).unwrap();
    assert_eq!(geo::grab(&inherited).unwrap().hold_point, None);
    assert_eq!(geo::grab(&inherited).unwrap().held_anchor, Some([7., 23.]));
    assert_eq!(geo::ledge(&inherited).unwrap().pull_in, Some(17.));
    assert_eq!(geo::ledge(&inherited).unwrap().climb_distance, None);
    for n in [-1., f64::INFINITY, f64::NAN] {
        inherited.interactions.as_mut().unwrap().ledge.as_mut().unwrap().roll_distance = Some(n);
        assert!(
            sim::data::load::check_fighter("test", &inherited)
                .unwrap_err()
                .to_string()
                .contains("interactions.ledge.rollDistance")
        );
    }
}

#[test]
fn catch_point_proximity_gates_drop_and_restore_hang_climb() {
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &[]);
    let fd = w.fighters[0].data;
    d.fighters[fd].interactions = Some(Interactions {
        ledge: Some(LedgeInteraction {
            catch_point: Some([11., 90.]),
            hang_anchor: Some([25., 35.]),
            ..Default::default()
        }),
        ..Default::default()
    });
    for side in [-1., 1.] {
        let s = &d.stages[w.stage];
        let b = &s.blocks[0];
        let ex = if side < 0. { b.left } else { b.right };
        let mut f = w.fighters[0].clone();
        f.set_state(State::Air);
        f.ledge_cd = 0;
        f.ground = false;
        f.surf = None;
        f.face = -side;
        f.x = ex + side * 11.;
        f.y = b.top + s.ledge.cy + 90.;
        f.vy = 1.;
        for gate in ["down", "cooldown", "rising"] {
            let mut blocked = f.clone();
            let mut input = Intent::default();
            match gate {
                "down" => input.down = true,
                "cooldown" => blocked.ledge_cd = 1,
                _ => blocked.vy = -1.,
            }
            sim::physics::try_ledge(&mut w, &d, 0, &mut blocked, input, false);
            assert_eq!(blocked.state, State::Air);
        }
        sim::physics::try_ledge(&mut w, &d, 0, &mut f, Intent::default(), false);
        assert_eq!(f.state, State::Ledge);
        assert_eq!(geo::forward_point(&f, [25., 35.]), [ex, b.top]);
        w.fighters[0] = f;
        let mut restored = w.clone();
        for _ in 0..3 {
            sim::step_world(&mut w, &d, &[sim::Input::default()]);
            sim::step_world(&mut restored, &d, &[sim::Input::default()]);
            assert_eq!(w, restored);
        }
        w.fighters[0].t = d.global.ledge.min_hang;
        update_fighter(&mut w, &d, 0, Intent { down: true, ..Default::default() });
        assert_eq!(w.fighters[0].state, State::Air);
        assert_eq!(w.fighters[0].ledge_cd, d.global.ledge.regrab_cooldown);
    }
}

#[test]
fn authored_endpoints_and_extreme_anchors_are_rejected() {
    let mut d = common::data();
    let w = common::fighters(&d, "brawler", &[]);
    let fd = w.fighters[0].data;
    let b = &d.stages[w.stage].blocks[0];
    let past = b.right - b.left + 100.;
    d.fighters[fd].interactions = Some(Interactions {
        ledge: Some(LedgeInteraction { climb_distance: Some(past), ..Default::default() }),
        ..Default::default()
    });
    let check = |d: &sim::GameData| {
        sim::data::load::check_stage("test", &d.stages[w.stage], &d.fighters, d.global.sim.platform_pad)
    };
    assert!(check(&d).unwrap_err().to_string().contains("endpoint leaves supporting block"));
    d.fighters[fd].interactions.as_mut().unwrap().ledge.as_mut().unwrap().climb_distance = Some(60.);
    let stage = &mut d.stages[w.stage];
    let mut overhead = stage.blocks[0].clone();
    overhead.left += 40.;
    overhead.right = overhead.left + 40.;
    overhead.bottom = overhead.top - 10.;
    overhead.top -= 100.;
    overhead.under_depth = 1.;
    overhead.under_inset = 10.;
    overhead.left_ledge = false;
    overhead.right_ledge = false;
    stage.blocks.push(overhead);
    let error = check(&d).unwrap_err().to_string();
    assert!(error.contains("endpoint intersects"), "{error}");
    d.fighters[fd].interactions.as_mut().unwrap().grab =
        Some(GrabInteraction { hold_point: Some([1e308, 0.]), ..Default::default() });
    assert!(
        sim::data::load::check_fighter("test", &d.fighters[fd])
            .unwrap_err()
            .to_string()
            .contains("interactions.grab.holdPoint[0]")
    );
}

#[test]
fn grab_contact_uses_strict_authored_height_tolerance() {
    for offset in [-0.001, 0., 0.001] {
        let mut d = common::data();
        let mut w = common::fighters(&d, "brawler", &["ninja"]);
        overrides(&mut d, w.fighters[0].data, w.fighters[1].data);
        let delta = d.global.grab.height_tolerance + offset;
        w.fighters[0].y = 400.;
        w.fighters[1].y = 341. + delta;
        w.fighters[1].x = w.fighters[0].x + 34.;
        // A deliberately broad grab isolates the vertical gate from hurtbox overlap.
        for phase in &mut d.fighters[w.fighters[0].data].moves.grab.phases {
            for hit in &mut phase.hitboxes {
                hit.r = 300.;
            }
        }
        let mut f = w.fighters[0].clone();
        sim::moves::start_move(&mut w, &d, 0, &mut f, "grab");
        f.mov.as_mut().unwrap().t = 6;
        w.fighters[0] = f;
        combat::hit_pass(&mut w, &d);
        assert_eq!(w.fighters[1].state == State::Grabbed, offset < 0., "delta {delta}");
    }
}

#[test]
fn tether_catches_outside_authored_catch_radius() {
    let mut d = common::data();
    let mut w = common::fighters(&d, "whiplash", &[]);
    let fd = w.fighters[0].data;
    d.fighters[fd].interactions = Some(Interactions {
        ledge: Some(LedgeInteraction {
            catch_point: Some([0., 500.]),
            hang_anchor: Some([25., 35.]),
            ..Default::default()
        }),
        ..Default::default()
    });
    let mut f = w.fighters[0].clone();
    f.ground = false;
    f.surf = None;
    f.face = 1.;
    f.x = d.stages[w.stage].blocks[0].left - 100.;
    f.y = d.stages[w.stage].blocks[0].top + 70.;
    f.vy = 1.;
    f.set_state(State::Air);
    sim::physics::try_ledge(&mut w, &d, 0, &mut f, Intent::default(), false);
    assert_eq!(f.state, State::Air);
    let slot = sim::data::Moves::SLOTS
        .iter()
        .find(|slot| d.fighters[fd].moves.get(slot).is_some_and(|m| m.phases[0].tether.is_some()))
        .unwrap();
    sim::moves::start_move(&mut w, &d, 0, &mut f, slot);
    sim::physics::try_ledge(&mut w, &d, 0, &mut f, Intent::default(), false);
    assert_eq!(f.state, State::Ledge);
    assert_eq!(geo::forward_point(&f, [25., 35.]), [d.stages[w.stage].blocks[0].left, d.stages[w.stage].blocks[0].top]);
}
