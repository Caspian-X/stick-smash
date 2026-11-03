mod common;
use sim::data::hitbox::{BoundPoint, CombatGeometry, HurtRegion};
use sim::{combat, geometry::Shape};

fn point(joint: Option<&str>, x: f64, y: f64) -> BoundPoint {
    BoundPoint { joint: joint.map(str::to_owned), offset: [x, y] }
}
fn circle(x: f64, y: f64) -> CombatGeometry {
    CombatGeometry::Circle { center: point(None, x, y), radius: 6. }
}
fn regions(d: &mut sim::GameData, index: usize) {
    d.fighters[index].hurt_regions = Some(vec![
        HurtRegion { enabled: true, states: vec![], id: "torso".into(), geometry: circle(140., 40.) },
        HurtRegion { enabled: true, states: vec![], id: "head".into(), geometry: circle(140., 40.) },
    ]);
    d.fighters[index].combat_center = Some([140., 40.]);
}
#[test]
fn bound_placement_parent_rotation_body_pivot_and_facing() {
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &[]);
    let f = &mut w.fighters[0];
    let fd = &mut d.fighters[f.data];
    fd.rig = Some(serde_json::from_value(serde_json::json!({"joints":[{"id":"root","translation":[10,20],"angle":90},{"id":"tip","parent":"root","translation":[4,0]}]})).unwrap());
    fd.poses.clear();
    fd.anims.clear();
    fd.poses
        .insert("stand".into(), serde_json::from_value(serde_json::json!({"rot":90,"pivot":10,"offsetY":3})).unwrap());
    let g = CombatGeometry::Capsule { a: point(Some("tip"), 2., 0.), b: point(None, 30., 40.), radius: 2. };
    for face in [-1., 1.] {
        f.face = face;
        f.x = 100.;
        f.y = 200.;
        let Shape::Capsule { a, b, .. } = combat::resolve_geometry(&d, f, 0, &g) else { panic!() };
        assert!((a[0] - (100. + 16. * face)).abs() < 1e-9, "{a:?}");
        assert!((a[1] - 197.).abs() < 1e-9, "{a:?}");
        assert_eq!(b, [100. + 30. * face, 160.]);
    }
}
#[test]
fn multiple_regions_contact_once_priority_history_and_clone_restore() {
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &["brawler"]);
    let index = w.fighters[0].data;
    regions(&mut d, index);
    let ph = &mut d.fighters[index].moves.get_mut("jab").unwrap().phases[0];
    let mut b = ph.hitboxes[0].clone();
    b.start = 0;
    b.end = None;
    b.frames.clear();
    b.geometry = Some(CombatGeometry::Capsule { a: point(None, 130., 40.), b: point(None, 150., 40.), radius: 3. });
    b.damage = 3.;
    let mut second = b.clone();
    second.damage = 19.;
    ph.hitboxes = vec![b, second];
    w.fighters[1].x = w.fighters[0].x;
    w.fighters[1].y = w.fighters[0].y;
    w.fighters[1].face = 1.;
    let mut f = w.fighters[0].clone();
    sim::moves::start_move(&mut w, &d, 0, &mut f, "jab");
    w.fighters[0] = f;
    assert!(
        !combat::active_boxes_at(&d, &w.fighters[0], w.frame)[0].shape.overlaps_rect(combat::hurt(&d, &w.fighters[1]))
    );
    let before = w.clone();
    combat::hit_pass(&mut w, &d);
    assert_eq!(w.hits.len(), 1);
    assert_eq!(w.hits[0].damage, 3.);
    assert!(w.fighters[1].lag > 0);
    let after = w.clone();
    let mut restored = before;
    combat::hit_pass(&mut restored, &d);
    assert_eq!(w, restored);
    combat::hit_pass(&mut w, &d);
    assert_eq!(w.hits, after.hits);
}
#[test]
fn region_queries_render_invariance_and_legacy_fallback() {
    let mut d = common::data();
    let w = common::fighters(&d, "brawler", &[]);
    let f = &w.fighters[0];
    assert!(!combat::circle_hurt(&d, f, 0, f.x + 140., f.y - 40., 1.));
    regions(&mut d, f.data);
    assert!(combat::circle_hurt(&d, f, 0, f.x + 140., f.y - 40., 1.));
    assert!(combat::rect_hurt(
        &d,
        f,
        0,
        sim::data::Rect { x1: f.x + 139., x2: f.x + 141., y1: f.y - 41., y2: f.y - 39. }
    ));
    assert!(!combat::circle_hurt(&d, f, 0, f.x, f.y - 40., 1.));
    let shapes = combat::hurt_shapes(&d, f, 80);
    d.global.render.overlays.run_speed = 999.;
    d.fighters[f.data].style.head_radius = 999.;
    assert_eq!(shapes, combat::hurt_shapes(&d, f, 80));
}
#[test]
fn quadruped_regions_move_with_joints_without_weapon_hurt() {
    let mut d = common::data();
    let w = common::fighters(&d, "brawler", &[]);
    let f = &w.fighters[0];
    let fd = &mut d.fighters[f.data];
    fd.rig = Some(serde_json::from_str(include_str!("../../../assets/templates/rigs/quadruped.json")).unwrap());
    fd.hurt_regions = Some(
        [("torso", "hips", "chest"), ("head", "neck", "head"), ("leg", "nearForeKnee", "nearForeFoot")]
            .into_iter()
            .map(|(id, a, b)| HurtRegion {
                enabled: true,
                states: vec![],
                id: id.into(),
                geometry: CombatGeometry::Capsule { a: point(Some(a), 0., 0.), b: point(Some(b), 0., 0.), radius: 3. },
            })
            .collect(),
    );
    fd.combat_center = Some([0., 35.]);
    sim::data::load::check_fighter("fixture", fd).unwrap();
    assert_eq!(combat::hurt_shapes(&d, f, 0).unwrap().len(), 3);
    assert!(!combat::circle_hurt(&d, f, 0, f.x + 160., f.y - 40., 3.));
}
#[test]
fn validation_rejects_unknown_empty_mixed_and_unsupported_geometry() {
    let d = common::data();
    let mut f = d.fighter("brawler").unwrap().clone();
    f.hurt_regions = Some(vec![]);
    assert!(sim::data::load::check_fighter("fixture", &f).unwrap_err().path.contains("hurtRegions"));
    f.hurt_regions = Some(vec![HurtRegion {
        enabled: true,
        states: vec![],
        id: "head".into(),
        geometry: CombatGeometry::Circle { center: point(Some("missing"), 0., 0.), radius: 2. },
    }]);
    f.combat_center = Some([0., 40.]);
    assert!(sim::data::load::check_fighter("fixture", &f).unwrap_err().path.contains("joint"));
    f.hurt_regions = None;
    let b = &mut f.moves.get_mut("jab").unwrap().phases[0].hitboxes[0];
    b.geometry = Some(circle(0., 0.));
    b.frames = vec![[0., 0., 0., 2.]];
    assert!(sim::data::load::check_fighter("fixture", &f).unwrap_err().path.contains("frames"));
}

#[test]
fn projectile_hits_explosion_and_proximity_reach_only_new_regions() {
    for (explosion, disabled) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut d = common::data();
        let source = d.fighters.iter().find(|f| f.projectiles.values().any(|p| p.article.is_none())).unwrap();
        let id = source.projectiles.iter().find(|(_, p)| p.article.is_none()).unwrap().0.clone();
        let mut w = common::fighters(&d, &source.id, &["brawler"]);
        let target = w.fighters[1].data;
        regions(&mut d, target);
        if disabled {
            for r in d.fighters[target].hurt_regions.as_mut().unwrap() {
                r.states = vec![sim::data::hitbox::HurtStateOverride {
                    state: sim::data::hitbox::HurtState::State(sim::world::State::Idle),
                    enabled: false,
                }];
            }
        }
        w.fighters[1].face = 1.;
        let source_index = w.fighters[0].data;
        let def = d.fighters[source_index].projectiles.get_mut(&id).unwrap();
        def.gravity = 0.;
        def.bounce = None;
        def.vel = [0., 0.];
        def.explosion = explosion.then(|| sim::data::fighter::Explosion {
            arm_ticks: 0,
            proximity: Some(2.),
            radius: 10.,
            sfx: None,
            fx: vec![],
        });
        let f = w.fighters[0].clone();
        sim::projectiles::spawn(&mut w, &d, 0, &f, &id);
        w.projs[0].x = w.fighters[1].x + 140.;
        w.projs[0].y = w.fighters[1].y - 40.;
        assert!(!combat::circ_rect(w.projs[0].x, w.projs[0].y, w.projs[0].r, combat::hurt(&d, &w.fighters[1])));
        sim::projectiles::step_projectiles(&mut w, &d);
        assert_eq!(w.hits.len(), usize::from(!disabled), "explosion={explosion}, disabled={disabled}");
    }
}
#[test]
fn batting_uses_capsule_endpoint_far_from_representative_center() {
    let d = common::data();
    let source = d.fighters.iter().find(|f| f.projectiles.values().any(|p| p.bat.is_some())).unwrap();
    let id = source.projectiles.iter().find(|(_, p)| p.bat.is_some()).unwrap().0.clone();
    let mut w = common::fighters(&d, &source.id, &["brawler"]);
    let f = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &f, &id);
    w.projs[0].x = 400.;
    w.projs[0].y = 300.;
    let b = sim::data::Hitbox { r: 1., geometry: Some(circle(0., 0.)), ..Default::default() };
    let shape = Shape::Capsule { a: [400., 300.], b: [700., 300.], radius: 2. };
    sim::projectiles::bat(&mut w, &d, 1, &[combat::ActiveBox { hit: b, shape, x: 550., y: 300. }]);
    assert_eq!(w.projs[0].owner, 1);
}
#[test]
fn article_attack_regions_and_melee_capsule_against_article_body() {
    let mut d = common::data();
    let source = d
        .fighters
        .iter()
        .find(|f| f.projectiles.values().any(|p| p.article.as_ref().is_some_and(|a| a.hurtbox.is_some())))
        .unwrap();
    let id = source
        .projectiles
        .iter()
        .find(|(_, p)| p.article.as_ref().is_some_and(|a| a.hurtbox.is_some()))
        .unwrap()
        .0
        .clone();
    let mut w = common::fighters(&d, &source.id, &["brawler"]);
    let si = w.fighters[0].data;
    let vi = w.fighters[1].data;
    regions(&mut d, vi);
    w.fighters[1].face = 1.;
    let def = d.fighters[si].projectiles.get_mut(&id).unwrap();
    let a = def.article.as_mut().unwrap();
    a.stages[0].vel = [0., 0.];
    a.stages[0].frames = 100;
    a.stages[0].health = 1000.;
    let mut hit = a.stages[0].hitboxes[0].clone();
    hit.start = 0;
    hit.end = None;
    hit.frames.clear();
    hit.x = 0.;
    hit.y = 0.;
    hit.r = 3.;
    a.stages[0].hitboxes = vec![hit];
    let f = w.fighters[0].clone();
    sim::projectiles::spawn(&mut w, &d, 0, &f, &id);
    w.frame += 1;
    w.projs[0].age = 1;
    w.projs[0].released = Some(1);
    w.projs[0].x = w.fighters[1].x + 140.;
    w.projs[0].y = w.fighters[1].y - 40.;
    let mut disabled_data = d.clone();
    for r in disabled_data.fighters[vi].hurt_regions.as_mut().unwrap() {
        r.states = vec![sim::data::hitbox::HurtStateOverride {
            state: sim::data::hitbox::HurtState::State(sim::world::State::Idle),
            enabled: false,
        }];
    }
    let mut disabled_world = w.clone();
    sim::articles::step(&mut disabled_world, &disabled_data, 0);
    assert!(disabled_world.hits.is_empty());
    sim::articles::step(&mut w, &d, 0);
    assert_eq!(w.hits.len(), 1);
    let ph = &mut d.fighters[vi].moves.get_mut("jab").unwrap().phases[0];
    let mut hit = ph.hitboxes[0].clone();
    hit.start = 0;
    hit.end = None;
    hit.frames.clear();
    hit.geometry = Some(CombatGeometry::Capsule { a: point(None, 130., 40.), b: point(None, 400., 40.), radius: 3. });
    ph.hitboxes = vec![hit];
    w.fighters[1].lag = 0;
    w.fighters[1].state = sim::world::State::Idle;
    let mut f = w.fighters[1].clone();
    sim::moves::start_move(&mut w, &d, 1, &mut f, "jab");
    w.fighters[1] = f;
    let before = w.projs[0].damage;
    sim::articles::step(&mut w, &d, 0);
    assert!(w.projs[0].damage > before);
}
#[test]
fn rehit_clock_survives_preserving_phase_transition() {
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &["brawler"]);
    let index = w.fighters[0].data;
    regions(&mut d, index);
    w.fighters[1].x = w.fighters[0].x;
    w.fighters[1].y = w.fighters[0].y;
    w.fighters[1].face = 1.;
    let m = d.fighters[index].moves.get_mut("jab").unwrap();
    let mut p = m.phases[0].clone();
    p.name = "first".into();
    let mut b = p.hitboxes[0].clone();
    b.geometry = Some(circle(140., 40.));
    b.frames.clear();
    b.start = 0;
    b.end = None;
    b.rehit = Some(3);
    b.damage = 1.;
    p.hitboxes = vec![b];
    p.transitions = vec![sim::data::moves::Transition {
        on: sim::data::Trigger::Hit,
        cond: None,
        from: None,
        to: "second".into(),
        lag: None,
        preserve_hits: true,
    }];
    let mut q = p.clone();
    q.name = "second".into();
    q.transitions.clear();
    p.hurt_regions =
        vec![sim::data::hitbox::HurtRegionWindow { region: "torso".into(), enabled: false, from: 0, to: 1 }];
    m.phases = vec![p, q];
    let mut f = w.fighters[0].clone();
    sim::moves::start_move(&mut w, &d, 0, &mut f, "jab");
    w.fighters[0] = f;
    combat::hit_pass(&mut w, &d);
    assert_eq!(w.hits.len(), 1);
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 1);
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().t, 0);
    assert_eq!(combat::hurt_shapes(&d, &w.fighters[0], w.frame).unwrap().len(), 2);
    w.fighters[0].lag = 0;
    w.fighters[1].lag = 0;
    w.fighters[0].mov.as_mut().unwrap().elapsed = 2;
    combat::hit_pass(&mut w, &d);
    assert_eq!(w.hits.len(), 1);
    w.fighters[0].mov.as_mut().unwrap().elapsed = 3;
    combat::hit_pass(&mut w, &d);
    assert_eq!(w.hits.len(), 2);
}

#[test]
fn target_near_transition_uses_regions() {
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &["brawler"]);
    let index = w.fighters[0].data;
    regions(&mut d, index);
    w.fighters[1].x = w.fighters[0].x;
    w.fighters[1].y = w.fighters[0].y;
    w.fighters[1].face = 1.;
    let p = &mut d.fighters[index].moves.get_mut("jab").unwrap().phases[0];
    p.detection = Some(sim::data::Rect { x1: 139., x2: 141., y1: 39., y2: 41. });
    p.hitboxes.clear();
    p.transitions = vec![sim::data::moves::Transition {
        on: sim::data::Trigger::TargetNear,
        cond: None,
        from: None,
        to: "end".into(),
        lag: None,
        preserve_hits: false,
    }];
    let mut f = w.fighters[0].clone();
    sim::moves::start_move(&mut w, &d, 0, &mut f, "jab");
    sim::moves::st_attack(&mut w, &d, 0, &mut f, sim::input::Intent::default(), 0.);
    assert_ne!(f.state, sim::world::State::Attack);
}

#[test]
fn invalid_regions_and_projectile_article_throw_bindings_report_paths() {
    let d = common::data();
    let mut f = d.fighter("brawler").unwrap().clone();
    f.combat_center = Some([0., 0.]);
    let region = HurtRegion { enabled: true, states: vec![], id: "same".into(), geometry: circle(0., 0.) };
    f.hurt_regions = Some(vec![region.clone(), region]);
    assert!(sim::data::load::check_fighter("fixture", &f).unwrap_err().path.contains("hurtRegions[1].id"));
    f.hurt_regions.as_mut().unwrap().truncate(1);
    f.hurt_regions.as_mut().unwrap()[0].geometry =
        CombatGeometry::Circle { center: point(None, f64::INFINITY, 0.), radius: 3. };
    assert!(sim::data::load::check_fighter("fixture", &f).unwrap_err().path.contains("offset"));
    for source in &d.fighters {
        for (id, projectile) in &source.projectiles {
            let mut f = source.clone();
            f.projectiles.get_mut(id).unwrap().hit.geometry = Some(circle(0., 0.));
            assert!(sim::data::load::check_fighter("fixture", &f).unwrap_err().path.ends_with("hit.geometry"));
            if projectile.article.is_some() {
                let mut f = source.clone();
                f.projectiles.get_mut(id).unwrap().article.as_mut().unwrap().stages[0].hitboxes[0].geometry =
                    Some(circle(0., 0.));
                assert!(
                    sim::data::load::check_fighter("fixture", &f)
                        .unwrap_err()
                        .path
                        .contains("article.stages[0].hitboxes[0].geometry")
                );
            }
        }
    }
    let mut encoded = serde_json::to_value(d.fighter("brawler").unwrap()).unwrap();
    let throws = encoded["throws"].as_object_mut().unwrap();
    let (_, throw) = throws.iter_mut().next().unwrap();
    throw["hit"]["geometry"] = serde_json::to_value(circle(0., 0.)).unwrap();
    let f = serde_json::from_value(encoded).unwrap();
    assert!(sim::data::load::check_fighter("fixture", &f).unwrap_err().path.contains("hit.geometry"));
}

#[test]
fn humanoid_weapon_exposure_and_animated_joints_ignore_render_tuning() {
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &[]);
    let f = &mut w.fighters[0];
    let fd = &mut d.fighters[f.data];
    fd.rig = Some(serde_json::from_str(include_str!("../../../assets/templates/rigs/humanoid.json")).unwrap());
    fd.hurt_regions = Some(vec![HurtRegion {
        enabled: true,
        states: vec![],
        id: "torso".into(),
        geometry: CombatGeometry::Capsule {
            a: point(Some("hip"), 0., 0.),
            b: point(Some("shoulder"), 0., 0.),
            radius: 7.,
        },
    }]);
    fd.combat_center = Some([0., 40.]);
    let weapon = CombatGeometry::Capsule {
        a: point(Some("nearHand"), 20., 0.),
        b: point(Some("nearHand"), 70., 0.),
        radius: 3.,
    };
    fd.anims.insert("idle".into(),serde_json::from_value(serde_json::json!({"clock":"world","keys":[{"at":0,"pose":{"joints":{"nearHand":{"angle":0}}}},{"at":10,"pose":{"joints":{"nearHand":{"angle":90}}},"lerp":true}]})).unwrap());
    let a = combat::resolve_geometry(&d, f, 0, &weapon);
    let b = combat::resolve_geometry(&d, f, 10, &weapon);
    assert_ne!(a, b);
    assert!(!combat::overlaps_hurt(&d, f, 0, a));
    f.vx = 1.;
    let before = combat::resolve_geometry(&d, f, 10, &weapon);
    d.global.render.overlays.run_speed = 999.;
    d.fighters[f.data].rig.as_mut().unwrap().bones.clear();
    d.fighters[f.data].style.head_radius = 999.;
    assert_eq!(before, combat::resolve_geometry(&d, f, 10, &weapon));
    let mut encoded = serde_json::to_value(&d.global).unwrap();
    encoded["gait"].as_object_mut().unwrap().remove("poseRunSpeed");
    let restored: sim::data::Global = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored.gait.pose_run_speed, 0.8);
    encoded.as_object_mut().unwrap().remove("gait");
    let restored: sim::data::Global = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored.gait.pose_run_speed, 0.8);
}

#[test]
fn activation_precedence_inclusive_phase_clock_and_restore() {
    use sim::data::hitbox::{HurtRegionWindow, HurtState, HurtStateOverride};
    use sim::world::State;
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &[]);
    let i = w.fighters[0].data;
    regions(&mut d, i);
    let fd = &mut d.fighters[i];
    let rs = fd.hurt_regions.as_mut().unwrap();
    rs[0].states = vec![HurtStateOverride { state: HurtState::State(State::Attack), enabled: false }];
    rs[1].enabled = false;
    fd.moves.get_mut("jab").unwrap().phases[0].hurt_regions = vec![
        HurtRegionWindow { region: "torso".into(), enabled: true, from: 2, to: 3 },
        HurtRegionWindow { region: "head".into(), enabled: true, from: 2, to: 3 },
    ];
    let mut next = fd.moves.get("jab").unwrap().phases[0].clone();
    next.name = "next".into();
    next.hurt_regions.clear();
    fd.moves.get_mut("jab").unwrap().phases.push(next);
    let mut f = w.fighters[0].clone();
    assert_eq!(combat::hurt_shapes(&d, &f, 0).unwrap().len(), 1);
    sim::moves::start_move(&mut w, &d, 0, &mut f, "jab");
    for (t, count) in [(1, 0), (2, 2), (3, 2), (4, 0)] {
        f.mov.as_mut().unwrap().t = t;
        f.mov.as_mut().unwrap().elapsed = 999;
        assert_eq!(combat::hurt_shapes(&d, &f, 500).unwrap().len(), count);
        let restored = f.clone();
        assert_eq!(combat::hurt_shapes(&d, &restored, 500), combat::hurt_shapes(&d, &f, 500));
    }
    f.mov.as_mut().unwrap().phase = 1;
    f.mov.as_mut().unwrap().t = 0;
    assert!(combat::hurt_shapes(&d, &f, 500).unwrap().is_empty());
}

#[test]
fn raw_down_input_swaps_standing_and_crouching_regions() {
    use sim::data::hitbox::{HurtState, HurtStateOverride};
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &[]);
    let i = w.fighters[0].data;
    regions(&mut d, i);
    let rs = d.fighters[i].hurt_regions.as_mut().unwrap();
    rs[0].states = vec![HurtStateOverride { state: HurtState::Crouch, enabled: false }];
    rs[1].enabled = false;
    rs[1].geometry = circle(0., 15.);
    rs[1].states = vec![HurtStateOverride { state: HurtState::Crouch, enabled: true }];
    let before = combat::hurt_shapes(&d, &w.fighters[0], w.frame);
    for _ in 0..20 {
        sim::step_world(&mut w, &d, &[sim::Input { stick: (0, -80), ..Default::default() }]);
    }
    assert_eq!(w.fighters[0].gait, sim::world::Gait::Crouch);
    assert_eq!(combat::hurt_shapes(&d, &w.fighters[0], w.frame).unwrap().len(), 1);
    assert_ne!(before, combat::hurt_shapes(&d, &w.fighters[0], w.frame));
    for _ in 0..30 {
        sim::step_world(&mut w, &d, &[sim::Input::default()]);
    }
    assert_ne!(w.fighters[0].gait, sim::world::Gait::Crouch);
}

#[test]
fn activation_validation_reports_references_intervals_and_duplicates() {
    use sim::data::hitbox::{HurtRegionWindow, HurtState, HurtStateOverride};
    let mut d = common::data();
    let i = d.fighters.iter().position(|f| f.id == "brawler").unwrap();
    regions(&mut d, i);
    let f = &mut d.fighters[i];
    for (region, from, to) in [("missing", 0, 1), ("head", 3, 2), ("head", 0, 999)] {
        f.moves.get_mut("jab").unwrap().phases[0].hurt_regions =
            vec![HurtRegionWindow { region: region.into(), enabled: false, from, to }];
        assert!(sim::data::load::check_fighter("fixture", f).unwrap_err().path.contains("hurtRegions[0]"));
    }
    let p = &mut f.moves.get_mut("jab").unwrap().phases[0];
    p.hurt_regions = vec![HurtRegionWindow { region: "head".into(), enabled: false, from: 0, to: 1 }; 2];
    assert!(sim::data::load::check_fighter("fixture", f).unwrap_err().msg.contains("overlapping"));
    f.moves.get_mut("jab").unwrap().phases[0].hurt_regions.clear();
    let r = &mut f.hurt_regions.as_mut().unwrap()[0];
    r.states = vec![HurtStateOverride { state: HurtState::Crouch, enabled: false }; 2];
    assert!(sim::data::load::check_fighter("fixture", f).unwrap_err().path.contains("states[1]"));
    for r in f.hurt_regions.as_mut().unwrap() {
        r.states.clear();
        r.enabled = false;
    }
    assert!(sim::data::load::check_fighter("fixture", f).unwrap_err().msg.contains("default-enabled"));
    assert!(serde_json::from_value::<HurtState>(serde_json::json!({"state":"typo"})).is_err());
}

#[test]
fn empty_active_set_never_uses_legacy_rectangles() {
    let mut d = common::data();
    let mut w = common::fighters(&d, "brawler", &["brawler"]);
    let i = w.fighters[0].data;
    regions(&mut d, i);
    for r in d.fighters[i].hurt_regions.as_mut().unwrap() {
        r.states = vec![sim::data::hitbox::HurtStateOverride {
            state: sim::data::hitbox::HurtState::State(sim::world::State::Idle),
            enabled: false,
        }];
    }
    let f = &w.fighters[1];
    assert_eq!(combat::hurt_shapes(&d, f, 0), Some(vec![]));
    assert!(!combat::rect_hurt(&d, f, 0, combat::hurt(&d, f)));
    assert!(!combat::circle_hurt(&d, f, 0, f.x, f.y - 40., 1000.));
    let p = &mut d.fighters[i].moves.get_mut("jab").unwrap().phases[0];
    p.hitboxes[0].start = 0;
    p.hitboxes[0].geometry = Some(circle(140., 40.));
    p.hitboxes[0].frames.clear();
    w.fighters[1].x = w.fighters[0].x;
    w.fighters[1].face = 1.;
    let mut f = w.fighters[0].clone();
    sim::moves::start_move(&mut w, &d, 0, &mut f, "jab");
    w.fighters[0] = f;
    combat::hit_pass(&mut w, &d);
    assert!(w.hits.is_empty());
    d.fighters[i].moves.get_mut("jab").unwrap().phases[0].hitboxes[0].kind = sim::data::HitType::Grab;
    combat::hit_pass(&mut w, &d);
    assert_ne!(w.fighters[1].state, sim::world::State::Grabbed);
    let p = &mut d.fighters[i].moves.get_mut("jab").unwrap().phases[0];
    p.detection = Some(sim::data::Rect { x1: -1000., x2: 1000., y1: -1000., y2: 1000. });
    p.transitions = vec![sim::data::moves::Transition {
        on: sim::data::Trigger::TargetNear,
        cond: None,
        from: None,
        to: "end".into(),
        lag: None,
        preserve_hits: false,
    }];
    let mut f = w.fighters[0].clone();
    sim::moves::st_attack(&mut w, &d, 0, &mut f, sim::input::Intent::default(), 0.);
    assert_eq!(f.state, sim::world::State::Attack);
}

#[test]
fn omitted_activation_fields_preserve_region_defaults_and_encoding() {
    let original = HurtRegion { id: "body".into(), geometry: circle(0., 30.), enabled: true, states: vec![] };
    let encoded = serde_json::to_value(&original).unwrap();
    assert!(encoded.get("enabled").is_none());
    assert!(encoded.get("states").is_none());
    assert_eq!(serde_json::from_value::<HurtRegion>(encoded).unwrap(), original);
}
