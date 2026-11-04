//! Piranha Plant reference behavior, named rig, and raw-input coverage.
mod common;

use sim::data::{HitType, moves::Trigger};
use sim::world::{Gait, State};
use sim::{Input, PlayerConfig, World, combat, step_world};

fn sandbox(d: &sim::GameData, fighter: &str, opponent: Option<&str>) -> World {
    let mut players = vec![PlayerConfig::human(fighter)];
    if let Some(o) = opponent {
        players.push(PlayerConfig::human(o));
    }
    common::sandbox(d, "arena", players)
}

fn tick(w: &mut World, d: &sim::GameData, input: Input) {
    step_world(w, d, &[input]);
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
}

fn rig_pos(d: &sim::GameData, f: &sim::world::Fighter, frame: u64, joint: &str) -> [f64; 2] {
    sim::pose::fighter_rig_transform(d, f, frame).unwrap().unwrap().joint_to_world(joint, [0., 0.]).unwrap()
}

fn fdata(d: &sim::GameData) -> &sim::data::Fighter {
    d.fighter("plant").unwrap()
}

fn start_at(d: &sim::GameData, slot: &str, phase: usize, t: u32, charge: u32) -> World {
    let mut w = sandbox(d, "plant", None);
    let mut f = w.fighters[0].clone();
    sim::moves::start_move(&mut w, d, 0, &mut f, slot);
    let m = f.mov.as_mut().unwrap();
    m.phase = phase;
    m.t = t;
    m.charge = charge;
    w.fighters[0] = f;
    w
}

#[test]
fn package_identity_ledger_and_projectiles() {
    let d = common::data();
    let f = fdata(&d);
    assert_eq!(f.name, "Piranha Plant");
    assert_eq!(f.id, "plant");
    for slot in [
        "jab", "ftilt", "utilt", "dtilt", "dashatk", "fsmash", "usmash", "dsmash", "nair", "fair", "bair", "uair",
        "dair", "grab", "dashgrab", "nspec", "sspec", "uspec", "dspec",
    ] {
        assert!(f.moves.get(slot).is_some(), "missing {slot}");
    }
    assert!(f.rig.is_some(), "plant must be a named-rig figure");
    let ids: Vec<_> = f.rig.as_ref().unwrap().joints.iter().map(|j| j.id.as_str()).collect();
    for id in ["pot", "stem0", "stem1", "stem2", "neck", "head", "jaw", "mouth", "top", "leafL", "leafR"] {
        assert!(ids.contains(&id), "rig joint {id}");
    }
    assert!(f.hurt_regions.as_ref().is_some_and(|r| r.len() >= 4));
    assert!(f.projectiles["ptooie"].article.is_some());
    assert_eq!(f.projectiles["poison"].hit.kind, HitType::DamageOnly);

    // Normalized source facts stay separate from the generated phase rows. These are the pinned UFD and
    // script values the conversion is checked against, not values read back out of fighter.json.
    let ledger: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/imports/sources/plant/ledger.json")).unwrap();
    let jab1 = &ledger["sourceFacts"]["ufd"]["jab"][0];
    assert_eq!(jab1["movename"], "Jab 1");
    assert_eq!(jab1["startup"], "2");
    assert_eq!(jab1["activeframes"], "2—3");
    assert_eq!(jab1["basedamage"], "2.4");
    let attack11 = &ledger["sourceFacts"]["scripts"]["jab"]["Attack11"][0];
    assert_eq!((attack11["start"].as_u64(), attack11["end"].as_u64()), (Some(2), Some(3)));
    assert_eq!(attack11["source"]["Angle"], "361");
    assert_eq!(attack11["source"]["Damage"], "2.4");
    let fsmash = &ledger["sourceFacts"]["ufd"]["fsmash"][0];
    assert_eq!(fsmash["startup"], "16");
    assert_eq!(fsmash["activeframes"], "16—17");
    assert_eq!(fsmash["basedamage"], "15.0/19.0");
    let lss = &ledger["sourceFacts"]["ufd"]["dspec"][0];
    assert_eq!(lss["totalframes"], "39/49");
    assert_eq!(lss["basedamage"], "8.4—26.0");
    let ptooie = &ledger["sourceFacts"]["ufd"]["nspec"][0];
    assert_eq!(ptooie["basedamage"], "14.0/18.0/15.3");
    let spike = &ledger["sourceFacts"]["articles"]["packun_spikeball"]["Shoot"][0];
    // The retained Shoot script carries both the reactive 14% and normal 18% variants; the parser keeps the
    // last matching block. The blown-away early window is authored as 18% in the package.
    assert_eq!(spike["source"]["Damage"], "14.0");

    // Source frame 1 is engine frame 0; the source jab window 2-3 is engine 1-2.
    let h = &f.moves.jab.phases[0].hitboxes[0];
    assert_eq!((h.start, h.end), (1, Some(2)));
    assert_eq!(h.damage, 2.4);
    assert_eq!(h.kind, HitType::Sakurai);
    assert_eq!(f.moves.fsmash.phases[0].hitboxes.iter().map(|h| h.damage).fold(0., f64::max), 19.0);
    assert_eq!(f.moves.usmash.phases[0].hitboxes[0].start, 11);
    // UFD: rapid jab hits every 4th frame for 0.6%, and F-tilt 2 is available from source frame 12.
    let rapid = &f.moves.jab.phases[3];
    assert_eq!(rapid.frames, Some(4));
    assert_eq!(rapid.hitboxes[0].damage, 0.6);
    assert_eq!((rapid.hitboxes[0].start, rapid.hitboxes[0].end), (0, Some(0)));
    assert_eq!(f.moves.ftilt.phases[0].transitions[0].from, Some(11));

    // Long-Stem Strike's four charge tiers are engine data, not hard-coded fighter dispatch.
    let bite = &f.moves.dspec.phases[3].hitboxes;
    assert_eq!(bite.len(), 4);
    assert_eq!((bite[0].min_charge, bite[0].damage), (45, 12.0));
    assert_eq!((bite[3].min_charge, bite[3].damage), (0, 8.4));
    assert!(f.moves.dspec.b_reverse);
    assert!(f.moves.dspec.phases[0].aim && f.moves.dspec.phases[1].aim);
    assert!(!f.moves.dspec.phases[2].aim, "the lunge is committed");
    let p = &f.projectiles["ptooie"].article.as_ref().unwrap();
    assert!(p.release_on_dir && p.hold.is_some());
    assert_eq!(p.stages[1].hitboxes[0].damage, 18.0);
    assert_eq!(p.stages[1].hitboxes[1].damage, 13.0);
    assert_eq!(p.stages[1].hitboxes[1].start, 4);
}

#[test]
fn named_rig_state_motion_keeps_pot_grounded_and_orders_jump_landing() {
    let d = common::data();
    let mut w = sandbox(&d, "plant", None);
    let ground = |w: &World, joint: &str| rig_pos(&d, &w.fighters[0], w.frame, joint)[1];
    let idle_head = ground(&w, "head");
    let idle_pot = ground(&w, "pot");
    assert!((idle_pot - w.fighters[0].y).abs() < 1e-9);
    w.fighters[0].set_state(State::Jumpsquat);
    w.fighters[0].ground = true;
    let mut squat = Vec::new();
    for t in 0..3 {
        w.fighters[0].t = t;
        squat.push(rig_pos(&d, &w.fighters[0], w.frame, "head")[1]);
        assert!((ground(&w, "pot") - w.fighters[0].y).abs() < 1e-9);
    }
    assert!(squat[0] > idle_head && squat[1] > squat[0] && squat[2] < squat[1]);
    w.fighters[0].set_state(State::Landing);
    w.fighters[0].lag = 3;
    w.fighters[0].t = 0;
    let land0 = rig_pos(&d, &w.fighters[0], w.frame, "head")[1];
    w.fighters[0].t = 2;
    let land2 = rig_pos(&d, &w.fighters[0], w.frame, "head")[1];
    assert!(land0 > idle_head && land2 < land0);
    for (gait, clip) in [(Gait::Walk, "walk"), (Gait::Run, "run"), (Gait::Turn, "turn"), (Gait::Brake, "brake")] {
        w.fighters[0].state = State::Idle;
        w.fighters[0].gait = gait;
        w.fighters[0].ground = true;
        w.fighters[0].vx = if matches!(gait, Gait::Run | Gait::Turn | Gait::Brake) { 6. } else { 0. };
        assert_eq!(sim::pose::state_anim(&d, &w.fighters[0]).0, clip);
    }
    let mut legacy = d.clone();
    let id = legacy.fighters.iter().position(|f| f.id == "plant").unwrap();
    legacy.fighters[id].anims.remove("walk");
    legacy.fighters[id].anims.remove("crawl");
    legacy.fighters[id].anims.remove("turn");
    legacy.fighters[id].anims.remove("brake");
    let f = &mut w.fighters[0];
    f.state = State::Idle;
    f.gait = Gait::Walk;
    f.vx = 0.;
    assert_eq!(sim::pose::state_anim(&legacy, f).0, "idle");
    f.gait = Gait::Crouch;
    f.vx = 2.;
    assert_eq!(sim::pose::state_anim(&legacy, f).0, "crouch");
}

#[test]
fn every_authored_clip_and_move_pose_resolves() {
    let d = common::data();
    let f = fdata(&d);
    let check = |pose: &sim::data::visual::PoseRef| match pose {
        sim::data::visual::PoseRef::Named(n) => assert!(f.poses.contains_key(n), "missing pose {n}"),
        sim::data::visual::PoseRef::Inline(_) => {}
    };
    for (name, a) in &f.anims {
        for k in &a.keys {
            check(&k.pose);
        }
        assert!(!name.trim().is_empty());
    }
    for (slot, m) in f.moves.iter() {
        for phase in &m.phases {
            for k in &phase.anim {
                check(&k.pose);
            }
            assert!(!phase.hitboxes.is_empty() || !phase.name.is_empty(), "{slot} phase");
        }
    }
    // Every move state the shared state machine can request has an authored clip.
    for clip in [
        "idle",
        "walk",
        "run",
        "crawl",
        "turn",
        "brake",
        "crouch",
        "jumpsquat",
        "landing",
        "air.rise",
        "air.fall",
        "air.flip",
        "helpless",
        "hitstun",
        "tumble",
        "shield",
        "shieldDrop",
        "roll",
        "spot",
        "airdodge",
        "knockdown",
        "getup",
        "getupAttack",
        "tech",
        "ledge",
        "climb",
        "ledgeAttack",
        "ledgeRoll",
        "ledgeJump",
        "dizzy",
        "win",
        "respawn",
        "hold",
        "pummel",
        "throw.f",
        "throw.b",
        "throw.u",
        "throw.d",
        "grabbed",
        "buried",
        "shieldbreak",
    ] {
        assert!(f.anims.contains_key(clip), "missing state clip {clip}");
    }
}

#[test]
fn joint_bound_contact_geometry_follows_the_animated_head_both_facings() {
    let d = common::data();
    let mut w = sandbox(&d, "plant", Some("brawler"));
    let f = &mut w.fighters[0];
    f.face = 1.;
    let mut m = f.clone();
    sim::moves::start_move(&mut w, &d, 0, &mut m, "fsmash");
    w.fighters[0] = m;
    w.fighters[0].mov.as_mut().unwrap().t = 15;
    let tf = sim::pose::fighter_rig_transform(&d, &w.fighters[0], w.frame).unwrap().unwrap();
    let boxes = combat::active_boxes_at(&d, &w.fighters[0], w.frame);
    assert!(!boxes.is_empty(), "fsmash must be active on phase 15");
    let mouth = tf.joint_to_world("mouth", [0., 0.]).unwrap();
    assert!(boxes.iter().any(|b| {
        let sim::geometry::Shape::Circle { center, radius } = b.shape else { return false };
        (center[0] - mouth[0]).abs() < 40. && (center[1] - mouth[1]).abs() < 40. && radius > 20.
    }));
    w.fighters[0].face = -1.;
    let tf = sim::pose::fighter_rig_transform(&d, &w.fighters[0], w.frame).unwrap().unwrap();
    let mouth_back = tf.joint_to_world("mouth", [0., 0.]).unwrap();
    assert!((mouth_back[0] - (2. * w.fighters[0].x - mouth[0])).abs() < 1e-6);

    // Every active Long-Stem tier keeps its mouth-anchored box on the animated head in both facings.
    for (charge, at_least) in [(0, 190.), (20, 245.), (35, 300.), (50, 390.)] {
        for face in [1., -1.] {
            let mut w = start_at(&d, "dspec", 3, 0, charge);
            w.fighters[0].face = face;
            let tf = sim::pose::fighter_rig_transform(&d, &w.fighters[0], w.frame).unwrap().unwrap();
            let mouth = tf.joint_to_world("mouth", [0., 0.]).unwrap();
            assert!((mouth[0] - w.fighters[0].x) * face > at_least, "charge {charge} face {face} mouth {mouth:?}");
            let boxes = combat::active_boxes_at(&d, &w.fighters[0], w.frame);
            assert!(!boxes.is_empty(), "active tier");
            let b = &boxes[0];
            assert!(libm::hypot(b.x - mouth[0], b.y - mouth[1]) < 20., "box {:?} mouth {mouth:?}", (b.x, b.y));
            // The head/jaw hurt regions are intangible during the bite.
            for region in fdata(&d).hurt_regions.as_ref().unwrap().iter().filter(|r| r.id == "head" || r.id == "jaw") {
                assert!(!combat::region_enabled(&d, &w.fighters[0], region));
            }
        }
    }
}

#[test]
fn hit_windows_open_and_close_on_the_converted_source_frames() {
    let d = common::data();
    // Jab 1: source 2–3 → engine 1–2, nothing on 0 or 3.
    for (t, active) in [(0, false), (1, true), (2, true), (3, false)] {
        let w = start_at(&d, "jab", 0, t, 0);
        assert_eq!(!combat::active_boxes_at(&d, &w.fighters[0], w.frame).is_empty(), active, "jab1 t {t}");
    }
    // Forward smash: source 16–17 → engine 15–16.
    for (t, active) in [(14, false), (15, true), (16, true), (17, false)] {
        let w = start_at(&d, "fsmash", 0, t, 0);
        assert_eq!(!combat::active_boxes_at(&d, &w.fighters[0], w.frame).is_empty(), active, "fsmash t {t}");
    }
    // Long-Stem bite: one source frame only.
    for (t, active) in [(0, true), (1, false), (3, false)] {
        let w = start_at(&d, "dspec", 3, t, 60);
        assert_eq!(!combat::active_boxes_at(&d, &w.fighters[0], w.frame).is_empty(), active, "bite t {t}");
    }
    // Rapid jab's 4-tick loop body carries its own 0.6% contact.
    let w = start_at(&d, "jab", 3, 0, 0);
    let b = combat::active_boxes_at(&d, &w.fighters[0], w.frame);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].hit.damage, 0.6);
}

#[test]
fn source_head_intangibility_windows_disable_regions_on_the_authored_ticks() {
    let d = common::data();
    let f = fdata(&d);
    let region = |id: &str| f.hurt_regions.as_ref().unwrap().iter().find(|r| r.id == id).unwrap();
    for (slot, id, cases) in [
        ("utilt", "head", [(4, true), (5, false), (12, false), (13, true)]),
        ("fsmash", "head", [(14, true), (15, false), (16, false), (17, true)]),
        ("usmash", "head", [(10, true), (11, false), (17, false), (18, true)]),
        ("uair", "head", [(5, true), (6, false), (8, false), (9, true)]),
        ("uair", "stem", [(5, true), (6, false), (8, false), (9, true)]),
    ] {
        for (t, enabled) in cases {
            let w = start_at(&d, slot, 0, t, 0);
            assert_eq!(combat::region_enabled(&d, &w.fighters[0], region(id)), enabled, "{slot} {id} t {t}");
        }
    }
}

#[test]
fn raw_input_fsmash_charges_and_releases() {
    let d = common::data();
    let mut w = sandbox(&d, "plant", Some("brawler"));
    w.fighters[1].x = w.fighters[0].x + 55.;
    w.fighters[1].face = -1.;
    let attack = Input { buttons: 2, stick: (127, 0), ..Default::default() };
    tick(&mut w, &d, attack);
    assert_eq!(w.fighters[0].state, State::Attack);
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "fsmash");
    for _ in 0..20 {
        tick(&mut w, &d, Input { buttons: 2, stick: (127, 0), ..Default::default() });
    }
    let charge = w.fighters[0].mov.as_ref().unwrap().charge;
    assert!(charge >= 10, "fsmash should store charge, got {charge}");
    let staged = w.fighters[0].clone();
    tick(&mut w, &d, Input::default());
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().charge, charge);
    assert!(w.fighters[0].mov.as_ref().unwrap().t >= staged.mov.as_ref().unwrap().t);
}

#[test]
fn ptooie_held_ball_follows_hits_and_launches_both_ways() {
    let d = common::data();
    let mut w = sandbox(&d, "plant", Some("brawler"));
    w.fighters[1].x = 900.;
    tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
    for _ in 0..20 {
        tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "nspec");
    assert_eq!(w.projs.len(), 1, "Ptooie article should exist");
    let ball = w.projs[0].clone();
    assert!(ball.released.is_none());
    let p = &d.fighter("plant").unwrap().projectiles["ptooie"];
    let hold = p.article.as_ref().unwrap().hold.as_ref().unwrap();
    assert!(
        (ball.x - (w.fighters[0].x + w.fighters[0].face * hold.offset[0])).abs() < 1e-9,
        "held ball follows the mouth side"
    );
    // The ball breathes instead of sitting still.
    let y0 = ball.y;
    let mut ys = vec![];
    for _ in 0..25 {
        tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
        ys.push(w.projs[0].y);
    }
    assert!(ys.iter().any(|y| (y - y0).abs() > 4.), "held ball should bob: {ys:?}");

    // It tracks the caster's placement.
    w.fighters[0].x += 40.;
    tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
    assert!((w.projs[0].x - (w.fighters[0].x + hold.offset[0] * w.fighters[0].face)).abs() < 1e-9);

    // The held ball is an active 14% hitbox.
    w.fighters[1].x = w.projs[0].x;
    w.fighters[1].y = w.projs[0].y + 40.;
    w.fighters[1].ground = false;
    w.fighters[1].dmg = 0.;
    let before = w.fighters[1].dmg;
    tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
    assert!((w.fighters[1].dmg - before - 14.).abs() < 1e-6, "held ball damage {}", w.fighters[1].dmg);

    // A stick direction while holding special blows it away in that direction.
    tick(&mut w, &d, Input { buttons: 4, stick: (-127, 0), ..Default::default() });
    assert!(w.projs[0].released.is_some(), "direction should blow the ball");
    assert_eq!(w.projs[0].stage, 1);
    assert_eq!(w.projs[0].face, -1., "left stick blows behind a right-facing plant");
    let x0 = w.projs[0].x;
    for _ in 0..5 {
        tick(&mut w, &d, Input::default());
    }
    assert!(w.projs[0].x < x0 - 20., "ball should travel left");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 2, "release enters shoot");

    // Releasing special instead swallows the ball and enters the short end phase.
    let mut w = sandbox(&d, "plant", None);
    tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
    for _ in 0..20 {
        tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
    }
    assert_eq!(w.projs.len(), 1);
    tick(&mut w, &d, Input::default());
    assert!(w.projs.is_empty(), "swallowed ball disappears");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 3, "swallow phase");
}

#[test]
fn poison_breath_is_flinchless_and_charge_scales_damage_life_and_rehit() {
    let d = common::data();
    for (hold_ticks, expect) in [(0usize, (1.2, 4, 82)), (130, (2.8, 7, 122))] {
        let mut w = sandbox(&d, "plant", Some("brawler"));
        tick(&mut w, &d, Input { buttons: 4, stick: (127, 0), ..Default::default() });
        for _ in 0..hold_ticks {
            tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
        }
        tick(&mut w, &d, Input::default());
        for _ in 0..24 {
            tick(&mut w, &d, Input::default());
        }
        assert_eq!(w.projs.len(), 1, "poison cloud");
        let p = &w.projs[0];
        assert_eq!(p.hit.kind, HitType::DamageOnly);
        assert!((p.hit.damage - expect.0).abs() < 0.05, "damage {} vs {}", p.hit.damage, expect.0);
        assert_eq!(p.hit.rehit, Some(expect.1));
        assert!(p.life <= expect.2 && p.life >= expect.2 - 18, "life {} near {}", p.life, expect.2);
        assert_eq!(p.r, 50.);

        // Damage with no hitstun, hitlag or state change, re-applied on the source rehit interval.
        let cloud = p.clone();
        let mut hits = 0;
        for t in 0..20 {
            let p = w.projs[0].clone();
            let ground_y = w.fighters[0].y;
            let v = &mut w.fighters[1];
            v.x = p.x;
            v.y = ground_y;
            v.ground = true;
            v.surf = None;
            let before = v.dmg;
            let state = v.state;
            step_world(&mut w, &d, &[Input::default()]);
            w.fx.clear();
            w.acts.clear();
            if w.fighters[1].dmg > before + 1e-9 {
                hits += 1;
                assert_eq!(w.fighters[1].state, state, "poison must not flinch at tick {t}");
                assert_eq!(w.fighters[1].lag, 0, "poison must not add hitlag");
            }
        }
        let expected_hits = 20 / expect.1 as usize + 1;
        assert!(hits >= expected_hits.min(2), "rehit contacts {hits}");
        assert_eq!(d.fighter("plant").unwrap().projectiles["poison"].gravity, 0.);
        assert!(cloud.vx > 0., "cloud drifts");
    }
}

#[test]
fn long_stem_strike_reach_damage_and_aim_follow_charge() {
    let d = common::data();
    let f = fdata(&d);
    // Reach and tier damage grow with charge; the box stays on the animated mouth.
    for (charge, min_reach, max_reach, box_damage) in [
        (0u32, 190., 215., 8.4),
        (20, 245., 270., 9.3 * (1. + 20. * f.moves.dspec.charge_damage.unwrap())),
        (35, 300., 325., 10.2 * (1. + 35. * f.moves.dspec.charge_damage.unwrap())),
        (50, 390., 420., 12.0 * (1. + 50. * f.moves.dspec.charge_damage.unwrap())),
        (60, 390., 420., 26.0),
    ] {
        let w = start_at(&d, "dspec", 2, 7, charge);
        let mouth = rig_pos(&d, &w.fighters[0], w.frame, "mouth");
        let reach = mouth[0] - w.fighters[0].x;
        assert!(reach > min_reach && reach < max_reach, "charge {charge} reach {reach}");
        let wb = start_at(&d, "dspec", 3, 0, charge);
        let boxes = combat::active_boxes_at(&d, &wb.fighters[0], wb.frame);
        assert!(!boxes.is_empty(), "charge {charge} active tier");
        assert!((boxes[0].hit.damage - box_damage).abs() < 1e-6, "charge {charge} damage {}", boxes[0].hit.damage);
    }

    // Pot armor begins with the charge phase (source frame 18), not during startup.
    for (damage, armored) in [(10., true), (16., false)] {
        let mut w = sandbox(&d, "plant", Some("brawler"));
        let mut f = w.fighters[0].clone();
        sim::moves::start_move(&mut w, &d, 0, &mut f, "dspec");
        let m = f.mov.as_mut().unwrap();
        m.phase = 1;
        m.t = 5;
        w.fighters[0] = f;
        let mut b = fdata(&d).moves.fsmash.phases[0].hitboxes[0].clone();
        b.damage = damage;
        combat::apply_hit(
            &mut w,
            &d,
            0,
            &b,
            combat::HitSource { attacker: 1, direction: 1., point: (0., 0.), projectile: false, label: None },
        );
        assert_eq!(w.fighters[0].dmg, damage);
        assert_eq!(w.fighters[0].state == State::Attack, armored, "damage {damage}");
    }

    // Head intangibility starts at travel frame 3 and covers the bite.
    let region = f.hurt_regions.as_ref().unwrap().iter().find(|r| r.id == "head").unwrap();
    for (phase, t, enabled) in [(2, 2, true), (2, 3, false), (2, 7, false), (3, 0, false), (3, 3, false)] {
        let w = start_at(&d, "dspec", phase, t, 40);
        assert_eq!(combat::region_enabled(&d, &w.fighters[0], region), enabled, "phase {phase} t {t}");
    }

    // Raw input: a direction during the charge turns the pot once (then locks) and release extends.
    let mut w = sandbox(&d, "plant", None);
    tick(&mut w, &d, Input { buttons: 4, stick: (0, -127), ..Default::default() });
    for _ in 0..25 {
        tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "dspec");
    assert_eq!(w.fighters[0].face, 1.);
    tick(&mut w, &d, Input { buttons: 4, stick: (-127, 0), ..Default::default() });
    assert_eq!(w.fighters[0].face, -1., "charge direction aims the strike");
    tick(&mut w, &d, Input { buttons: 4, stick: (127, 0), ..Default::default() });
    assert_eq!(w.fighters[0].face, -1., "the pot cannot change direction twice");
    tick(&mut w, &d, Input::default());
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 2, "release lunges");
    tick(&mut w, &d, Input { stick: (127, 0), ..Default::default() });
    assert_eq!(w.fighters[0].face, -1., "the committed lunge ignores direction");
    for _ in 0..7 {
        tick(&mut w, &d, Input::default());
    }
    let mouth = rig_pos(&d, &w.fighters[0], w.frame, "mouth");
    assert!(mouth[0] < w.fighters[0].x - 150., "left-aimed head extends left: {mouth:?}");
}

#[test]
fn piranhacopter_climbs_steadily_and_steers_both_ways() {
    let d = common::data();
    let mut lasts = vec![];
    for dir in [-127i8, 127] {
        let mut w = sandbox(&d, "plant", None);
        w.fighters[0].set_state(State::Air);
        w.fighters[0].ground = false;
        w.fighters[0].y -= 100.;
        let (y0, x0) = (w.fighters[0].y, w.fighters[0].x);
        tick(&mut w, &d, Input { buttons: 4, stick: (0, 127), ..Default::default() });
        assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "uspec");
        let mut last_y = w.fighters[0].y;
        let mut climb = vec![];
        for _ in 0..56 {
            tick(&mut w, &d, Input { stick: (dir, 0), buttons: 4, ..Default::default() });
            climb.push(w.fighters[0].y);
            assert!(w.fighters[0].y <= last_y + 0.01, "copter must not sink mid-spin: {last_y} -> {}", w.fighters[0].y);
            last_y = w.fighters[0].y;
        }
        assert!(w.fighters[0].y < y0 - 280., "steady climb, got {} -> {}", y0, w.fighters[0].y);
        let dx = w.fighters[0].x - x0;
        assert!(dx * f64::from(dir.signum()) > 90., "steers {dir}: dx {dx}");
        assert!(climb.windows(2).all(|p| p[0] - p[1] > 1.5), "continuous ascent");
        lasts.push(dx.abs());
    }
    assert!((lasts[0] - lasts[1]).abs() < 30., "both directions drift similarly: {lasts:?}");
    // After the spin the fall phase ends helpless or lands.
    let mut w = sandbox(&d, "plant", None);
    w.fighters[0].set_state(State::Air);
    w.fighters[0].ground = false;
    w.fighters[0].y -= 100.;
    tick(&mut w, &d, Input { buttons: 4, stick: (0, 127), ..Default::default() });
    for _ in 0..120 {
        tick(&mut w, &d, Input::default());
    }
    assert!(w.fighters[0].ground || w.fighters[0].state == State::Helpless);
}

#[test]
fn nair_and_copter_multi_hits_retain_rehit_windows() {
    let d = common::data();
    let mut w = sandbox(&d, "plant", Some("brawler"));
    w.fighters[1].x = w.fighters[0].x + 45.;
    w.fighters[1].y = w.fighters[0].y;
    w.fighters[1].face = -1.;
    let mut m = w.fighters[0].clone();
    sim::moves::start_move(&mut w, &d, 0, &mut m, "nair");
    w.fighters[0] = m;
    let mut contacts = 0;
    for _ in 0..24 {
        step_world(&mut w, &d, &[Input::default()]);
        contacts += w.hits.len();
        w.hits.clear();
        w.fx.clear();
        w.acts.clear();
    }
    assert!(contacts >= 2, "nair should rehit, got {contacts} contacts");
}

#[test]
fn clone_restore_is_deterministic_through_charging_specials() {
    let d = common::data();
    let mut w = sandbox(&d, "plant", Some("king"));
    for _ in 0..20 {
        tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
    }
    let mut other = w.clone();
    for _ in 0..80 {
        tick(&mut w, &d, Input { buttons: 4, ..Default::default() });
        tick(&mut other, &d, Input { buttons: 4, ..Default::default() });
        assert_eq!(w, other);
    }
    let mut restored = w.clone();
    for _ in 0..60 {
        tick(&mut w, &d, Input::default());
        tick(&mut restored, &d, Input::default());
    }
    assert_eq!(w, restored);
}

#[test]
fn throw_ledger_and_pummel_damage() {
    let d = common::data();
    let f = fdata(&d);
    assert_eq!(f.stats.pummel_damage, Some(1.4));
    assert_eq!((f.throws.f.release, f.throws.f.total, f.throws.f.hit.damage), (13, 31, 8.0));
    assert_eq!((f.throws.b.release, f.throws.b.total, f.throws.b.hit.damage), (18, 34, 11.0));
    assert_eq!((f.throws.u.release, f.throws.u.total, f.throws.u.hit.damage), (19, 49, 12.0));
    assert_eq!((f.throws.d.release, f.throws.d.total, f.throws.d.hit.damage), (29, 47, 3.0));
    assert_eq!(f.throws.d.ticks.len(), 2);
    assert!(f.moves.grab.phases[0].hitboxes[0].kind == HitType::Grab);
}

#[test]
fn raw_jump_attack_short_hop_aerial_and_face_switch() {
    let d = common::data();
    let mut w = sandbox(&d, "plant", None);
    tick(&mut w, &d, Input { buttons: 1 | 2, ..Default::default() });
    assert_eq!(w.fighters[0].state, State::Jumpsquat);
    for _ in 0..4 {
        tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    }
    assert_eq!(w.fighters[0].state, State::Attack);
    assert!(w.fighters[0].mov.as_ref().is_some_and(|m| m.slot == "nair"));
    assert!(!w.fighters[0].ground);
    let mut w = sandbox(&d, "plant", None);
    w.fighters[0].face = 1.;
    tick(&mut w, &d, Input { buttons: 4, stick: (-127, 0), ..Default::default() });
    assert_eq!(w.fighters[0].face, -1., "special input turns the plant");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().slot, "sspec");
}

#[test]
fn grab_pummel_and_throw_use_the_mouth_anchor() {
    let d = common::data();
    let mut w = sandbox(&d, "plant", Some("brawler"));
    w.fighters[1].x = w.fighters[0].x + 42.;
    w.fighters[1].y = w.fighters[0].y;
    w.fighters[1].face = -1.;
    tick(&mut w, &d, Input { buttons: 8, ..Default::default() });
    for _ in 0..12 {
        tick(&mut w, &d, Input::default());
    }
    assert_eq!(w.fighters[1].state, State::Grabbed, "grab should reach the mouth");
    assert_eq!(w.fighters[0].state, State::Hold);
    let held_x = w.fighters[1].x;
    tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    for _ in 0..22 {
        tick(&mut w, &d, Input::default());
    }
    assert_eq!(w.fighters[0].state, State::Hold);
    assert!(w.fighters[1].dmg >= 1.4, "pummel damage {}", w.fighters[1].dmg);
    assert!((w.fighters[1].x - held_x).abs() < 25., "captive stays at the mouth anchor");
    tick(&mut w, &d, Input { stick: (127, 0), ..Default::default() });
    for _ in 0..16 {
        tick(&mut w, &d, Input::default());
    }
    assert!(w.fighters[1].state != State::Grabbed);
    assert!(w.fighters[1].dmg > 8.);
}

#[test]
fn same_attack_launches_harder_at_higher_percent_and_heavier_targets_less() {
    let d = common::data();
    let plant = d.fighters.iter().position(|f| f.id == "plant").unwrap();
    let b = d.fighters[plant].moves.fsmash.phases[0].hitboxes.iter().find(|h| h.damage > 15.).unwrap().clone();
    let mut outs = Vec::new();
    for dmg in [0., 80., 150.] {
        let mut w = sandbox(&d, "plant", Some("brawler"));
        w.fighters[1].dmg = dmg;
        w.fighters[1].x = w.fighters[0].x + 50.;
        w.fighters[1].face = -1.;
        combat::apply_hit(
            &mut w,
            &d,
            1,
            &b,
            combat::HitSource { attacker: 0, direction: 1., point: (0., 0.), projectile: false, label: None },
        );
        for _ in 0..30 {
            tick(&mut w, &d, Input::default());
        }
        outs.push(w.fighters[1].x);
    }
    assert!(outs[0] < outs[1] && outs[1] < outs[2], "launch {outs:?}");
    // Same percent, different victim weight: the heavier King travels less than the lighter Brawler.
    let mut distances = vec![];
    for victim in ["brawler", "king"] {
        let mut w = sandbox(&d, "plant", Some(victim));
        w.fighters[1].dmg = 90.;
        w.fighters[1].x = w.fighters[0].x + 50.;
        w.fighters[1].face = -1.;
        combat::apply_hit(
            &mut w,
            &d,
            1,
            &b,
            combat::HitSource { attacker: 0, direction: 1., point: (0., 0.), projectile: false, label: None },
        );
        for _ in 0..45 {
            tick(&mut w, &d, Input::default());
        }
        distances.push(w.fighters[1].x);
    }
    assert!(distances[1] < distances[0], "weight should reduce launch: {distances:?}");
}

#[test]
fn piranhacopter_rehits_and_jaw_opens_then_clamps() {
    let d = common::data();
    let mut w = sandbox(&d, "plant", Some("brawler"));
    w.fighters[1].x = w.fighters[0].x + 30.;
    w.fighters[1].y = w.fighters[0].y;
    w.fighters[1].face = -1.;
    w.fighters[0].set_state(State::Air);
    w.fighters[0].ground = false;
    w.fighters[0].y -= 60.;
    w.fighters[1].y -= 60.;
    w.fighters[1].ground = false;
    let mut m = w.fighters[0].clone();
    sim::moves::start_move(&mut w, &d, 0, &mut m, "uspec");
    w.fighters[0] = m;
    let mut contacts = 0;
    for _ in 0..40 {
        let (px, py) = (w.fighters[0].x, w.fighters[0].y);
        let v = &mut w.fighters[1];
        v.x = px + 30.;
        v.y = py - 30.;
        v.ground = false;
        v.surf = None;
        step_world(&mut w, &d, &[Input::default()]);
        contacts += w.hits.len();
        w.hits.clear();
        w.fx.clear();
        w.acts.clear();
    }
    assert!(contacts >= 2, "copter should rehit, got {contacts}");

    let mut w = sandbox(&d, "plant", None);
    let mut f = w.fighters[0].clone();
    sim::moves::start_move(&mut w, &d, 0, &mut f, "grab");
    w.fighters[0] = f;
    let tf = |w: &World, t: u32| {
        let mut f = w.fighters[0].clone();
        f.mov.as_mut().unwrap().t = t;
        let tf = sim::pose::fighter_rig_transform(&d, &f, w.frame).unwrap().unwrap();
        (tf.joints.get("jaw").unwrap().angle, tf.joints.get("head").unwrap().angle)
    };
    let (jaw4, head4) = tf(&w, 4);
    let (jaw12, head12) = tf(&w, 12);
    assert!(jaw4 - head4 < -25., "jaw should open: {jaw4} vs {head4}");
    assert!(jaw12 - head12 > -10., "jaw should clamp: {jaw12} vs {head12}");
}

#[test]
fn trigger_article_end_is_available_to_data() {
    // Guards the trigger spelling used by the generated nspec swallow transition.
    let d = common::data();
    let nspec = &fdata(&d).moves.nspec;
    assert!(nspec.phases[1].transitions.iter().any(|t| t.on == Trigger::ArticleEnd));
}

fn joint_probe(d: &sim::GameData, f: &sim::world::Fighter, frame: u64) -> Vec<[f64; 2]> {
    let tf = sim::pose::fighter_rig_transform(d, f, frame).unwrap().unwrap();
    let mut out: Vec<[f64; 2]> = ["head", "jaw", "mouth", "leafL", "leafR", "pot", "stem0", "top"]
        .iter()
        .filter_map(|j| tf.joint_to_world(j, [0., 0.]))
        .collect();
    for leaf in ["leafL", "leafR"] {
        if let Some(tip) = tf.joint_to_world(leaf, [30., 0.]) {
            out.push(tip);
        }
    }
    // The drawn pot is wider than its joint: probe its rim corners too.
    for point in [[-20., 0.], [20., 0.], [0., 26.]] {
        if let Some(p) = tf.joint_to_world("pot", point) {
            out.push(p);
        }
    }
    out
}

#[test]
fn raw_held_jab_reaches_third_rapid_and_finisher_while_early_release_keeps_recovery() {
    let d = common::data();
    // Held attack continues through jab 2 and jab 3 into the rapid loop.
    let mut w = sandbox(&d, "plant", None);
    let held = Input { buttons: 2, ..Default::default() };
    let mut seen = Vec::new();
    for _ in 0..60 {
        tick(&mut w, &d, held);
        if let Some(m) = &w.fighters[0].mov
            && seen.last() != Some(&m.phase)
        {
            seen.push(m.phase);
        }
    }
    assert_eq!(seen, vec![0, 1, 2, 3], "held attack should enter jab 3 and the rapid loop: {seen:?}");
    let phase = w.fighters[0].mov.as_ref().unwrap().phase;
    for _ in 0..20 {
        tick(&mut w, &d, held);
        assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, phase, "holding keeps the rapid loop");
    }
    // Release ends the loop with the source 40-frame finisher, then Idle.
    tick(&mut w, &d, Input::default());
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 4, "rapid release enters the finisher");
    let mut ticks = 1;
    while w.fighters[0].state == State::Attack && ticks < 80 {
        tick(&mut w, &d, Input::default());
        ticks += 1;
    }
    assert_eq!(w.fighters[0].state, State::Idle);
    assert!((40..=42).contains(&ticks), "finisher recovery is source-true, got {ticks}");

    // A tap does not chain: jab 1 plays its full 19-frame recovery.
    let mut w = sandbox(&d, "plant", None);
    tick(&mut w, &d, held);
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 0);
    let mut ticks = 0;
    while w.fighters[0].state == State::Attack && ticks < 60 {
        tick(&mut w, &d, Input::default());
        ticks += 1;
        if w.fighters[0].state == State::Attack {
            assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 0, "a tap must not chain to jab 2");
        }
    }
    assert_eq!(w.fighters[0].state, State::Idle);
    assert!((17..=19).contains(&ticks), "jab 1 recovery is source-true, got {ticks}");

    // Chaining into jab 2 and releasing early keeps jab 2's full 20-frame recovery.
    let mut w = sandbox(&d, "plant", None);
    for _ in 0..20 {
        tick(&mut w, &d, held);
        if w.fighters[0].mov.as_ref().is_some_and(|m| m.phase == 1) {
            break;
        }
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 1, "held attack enters jab 2");
    let mut ticks = 0;
    while w.fighters[0].state == State::Attack && ticks < 60 {
        tick(&mut w, &d, Input::default());
        ticks += 1;
        if w.fighters[0].state == State::Attack {
            assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 1, "release keeps jab 2 recovery");
        }
    }
    assert_eq!(w.fighters[0].state, State::Idle);
    assert!((18..=20).contains(&ticks), "jab 2 recovery is source-true, got {ticks}");
}

#[test]
fn source_top_contacts_are_root_space_and_low_grounded_boxes_stay_low() {
    use sim::geometry::Shape;
    let d = common::data();
    let f = fdata(&d);
    // Jab 1 source `top` Z=5.3 Y=6.3 must resolve from the fighter root, not 31.5 px above the head.
    let GeometryProbe::Circle(center, radius) = geometry_probe(&f.moves.jab.phases[0].hitboxes[0].geometry) else {
        panic!("jab 1 is a circle")
    };
    assert!(center.joint.is_none(), "source `top` is the root bone");
    assert_eq!(center.offset, [26.5, 31.5]);
    assert_eq!(radius, 9.);
    for face in [1., -1.] {
        for t in [1, 2] {
            let mut w = start_at(&d, "jab", 0, t, 0);
            w.fighters[0].face = face;
            let f0 = w.fighters[0].clone();
            let boxes = combat::active_boxes_at(&d, &f0, w.frame);
            assert_eq!(boxes.len(), 4);
            let Shape::Circle { center, radius } = boxes[0].shape else { panic!("circle") };
            let want = [f0.x + face * 26.5, f0.y - 31.5];
            assert!((center[0] - want[0]).abs() < 1e-9 && (center[1] - want[1]).abs() < 1e-9, "{center:?}");
            assert_eq!(radius, 9.);
            // Root binding is stable across the animated frames, unlike a head-joint binding.
            let w2 = start_at(&d, "jab", 0, if t == 1 { 2 } else { 1 }, 0);
            let mut f2 = w2.fighters[0].clone();
            f2.face = face;
            let Shape::Circle { center: center2, .. } = combat::active_boxes_at(&d, &f2, w2.frame)[0].shape else {
                panic!("circle")
            };
            assert!((center[0] - center2[0]).abs() < 1e-9 && (center[1] - center2[1]).abs() < 1e-9);
        }
    }
    // Down tilt's low source boxes stay near the ground on every active frame and facing.
    for face in [1., -1.] {
        for t in [8, 9, 10] {
            let mut w = start_at(&d, "dtilt", 0, t, 0);
            w.fighters[0].face = face;
            let f0 = w.fighters[0].clone();
            let boxes = combat::active_boxes_at(&d, &f0, w.frame);
            assert_eq!(boxes.len(), 4);
            for b in boxes {
                let Shape::Circle { center, .. } = b.shape else { panic!("circle") };
                assert!(center[1] > f0.y - 25. && center[1] < f0.y, "low dtilt contact at {center:?}");
                assert!((center[0] - f0.x) * face > 20., "forward contact at {center:?}");
            }
        }
    }
    // Source capsules keep both endpoints (rapid jab, forward tilt, dash attack's `top`).
    let rapid = geometry_probe(&f.moves.jab.phases[3].hitboxes[0].geometry);
    let GeometryProbe::Capsule(a, b, r) = rapid else { panic!("rapid jab is a source capsule") };
    assert_eq!((a.offset, b.offset, r), ([40., 40.], [65., 40.], 26.));
    let ftilt = geometry_probe(&f.moves.ftilt.phases[0].hitboxes[0].geometry);
    let GeometryProbe::Capsule(a, b, r) = ftilt else { panic!("fwd tilt is a source capsule") };
    assert_eq!((a.offset, b.offset, r), ([60., 37.5], [65., 52.5], 31.5));
    let dash = geometry_probe(&f.moves.dashatk.phases[0].hitboxes[1].geometry);
    let GeometryProbe::Capsule(a, b, _) = dash else { panic!("dash top is a source capsule") };
    assert_eq!((a.offset, b.offset), ([0., 55.], [0., 25.]));

    // Converted contacts stay on the visible figure in both facings.
    for (slot, phase, at) in [
        ("jab", 0, 1),
        ("jab", 1, 1),
        ("jab", 2, 6),
        ("jab", 3, 0),
        ("jab", 4, 5),
        ("ftilt", 0, 6),
        ("ftilt", 1, 4),
        ("utilt", 0, 7),
        ("dtilt", 0, 8),
        ("dsmash", 0, 13),
        ("dsmash", 0, 17),
        ("dashatk", 0, 6),
        ("fsmash", 0, 15),
        ("usmash", 0, 11),
        ("usmash", 0, 15),
        ("nair", 0, 7),
        ("uair", 0, 6),
        ("bair", 0, 13),
        ("fair", 0, 6),
        ("dair", 0, 8),
        ("grab", 0, 5),
        ("uspec", 0, 45),
    ] {
        for face in [1., -1.] {
            let mut w = start_at(&d, slot, phase, at, 0);
            w.fighters[0].face = face;
            let f0 = w.fighters[0].clone();
            let boxes = combat::active_boxes_at(&d, &f0, w.frame);
            assert!(!boxes.is_empty(), "{slot} phase {phase} t {at} should be active");
            let probes = joint_probe(&d, &f0, w.frame);
            for b in boxes {
                assert!(
                    probes.iter().any(|p| b.shape.overlaps(Shape::Circle { center: *p, radius: 10. })),
                    "{slot} phase {phase} t {at} face {face} contact {:?} is not on the figure",
                    b.shape
                );
            }
        }
    }
}

enum GeometryProbe {
    Circle(sim::data::hitbox::BoundPoint, f64),
    Capsule(sim::data::hitbox::BoundPoint, sim::data::hitbox::BoundPoint, f64),
}
fn geometry_probe(g: &Option<sim::data::hitbox::CombatGeometry>) -> GeometryProbe {
    use sim::data::hitbox::CombatGeometry;
    match g.as_ref().expect("converted attack geometry") {
        CombatGeometry::Circle { center, radius } => GeometryProbe::Circle(center.clone(), *radius),
        CombatGeometry::Capsule { a, b, radius } => GeometryProbe::Capsule(a.clone(), b.clone(), *radius),
    }
}

#[test]
fn hurt_regions_track_the_rig_and_follow_the_refitted_low_poses() {
    use sim::data::hitbox::CombatGeometry;
    use sim::geometry::Shape;
    let d = common::data();
    let f = fdata(&d);
    let region = |id: &str| f.hurt_regions.as_ref().unwrap().iter().find(|r| r.id == id).unwrap();
    assert!(matches!(region("head").geometry, CombatGeometry::Capsule { .. }));
    assert!(matches!(region("pot").geometry, CombatGeometry::Capsule { .. }));
    let centre =
        |w: &World, id: &str| combat::resolve_geometry(&d, &w.fighters[0], w.frame, &region(id).geometry).center();
    let mut w = sandbox(&d, "plant", None);
    let tf = sim::pose::fighter_rig_transform(&d, &w.fighters[0], w.frame).unwrap().unwrap();
    let head = tf.joint_to_world("head", [0., 0.]).unwrap();
    let jaw = tf.joint_to_world("jaw", [0., 0.]).unwrap();
    let mouth = tf.joint_to_world("mouth", [0., 0.]).unwrap();
    let pot = tf.joint_to_world("pot", [0., 0.]).unwrap();
    let head_overlaps = combat::resolve_geometry(&d, &w.fighters[0], w.frame, &region("head").geometry);
    assert!(head_overlaps.overlaps(Shape::Circle { center: head, radius: 10. }));
    let jaw_overlaps = combat::resolve_geometry(&d, &w.fighters[0], w.frame, &region("jaw").geometry);
    assert!(jaw_overlaps.overlaps(Shape::Circle { center: jaw, radius: 6. }));
    assert!(jaw_overlaps.overlaps(Shape::Circle { center: mouth, radius: 6. }));
    let pot_overlaps = combat::resolve_geometry(&d, &w.fighters[0], w.frame, &region("pot").geometry);
    assert!(pot_overlaps.overlaps(Shape::Circle { center: pot, radius: 8. }));
    // The body hurtbox still covers the standing figure from the pot to the head.
    let body = f.hurtboxes.default;
    assert!(body.y1 <= 0. && body.y2 >= 120.);
    assert!(head[1] > w.fighters[0].y - body.y2);
    assert!(pot[1] <= w.fighters[0].y);

    // Mirrored facing mirrors the hurt geometry about the fighter.
    let idle_head = centre(&w, "head");
    w.fighters[0].face = -1.;
    let mirrored = centre(&w, "head");
    assert!((mirrored[0] - (2. * w.fighters[0].x - idle_head[0])).abs() < 1e-9);

    // The hurt capsule follows the animated head when the refitted low sweep dips.
    let idle_y = idle_head[1];
    for t in [8, 9, 10] {
        let w = start_at(&d, "dtilt", 0, t, 0);
        let y = centre(&w, "head")[1];
        assert!(y > idle_y + 25., "dtilt head hurt region follows the low pose: {y} vs {idle_y}");
    }
    // The standing up strike keeps the head hurt region up with the raised head.
    let w = start_at(&d, "utilt", 0, 8, 0);
    assert!(centre(&w, "head")[1] < idle_y - 5.);
}

fn gait_clip_entry(d: &sim::GameData, idle_ticks: usize, turn: bool) -> (World, sim::data::rig::RigPose) {
    let mut w = sandbox(d, "plant", None);
    for _ in 0..idle_ticks {
        tick(&mut w, d, Input::default());
    }
    let right = Input { stick: (127, 0), ..Default::default() };
    for _ in 0..18 {
        tick(&mut w, d, right);
    }
    assert_eq!(w.fighters[0].gait, Gait::Run);
    let input = if turn { Input { stick: (-127, 0), ..Default::default() } } else { Input::default() };
    tick(&mut w, d, input);
    let clip = if turn { "turn" } else { "brake" };
    assert_eq!(w.fighters[0].gait, if turn { Gait::Turn } else { Gait::Brake });
    let (name, clock) = sim::pose::state_anim(d, &w.fighters[0]);
    assert_eq!(name, clip);
    assert_eq!(clock, Some(0.), "{clip} must sample the gait clock from its first frame");
    let pose = sim::pose::fighter_rig_pose(d, &w.fighters[0], w.frame);
    (w, pose)
}

#[test]
fn turn_and_brake_clips_sample_the_gait_clock_not_the_idle_state_clock() {
    let d = common::data();
    let idle = {
        let w = sandbox(&d, "plant", None);
        sim::pose::fighter_rig_pose(&d, &w.fighters[0], w.frame)
    };
    let head_x = |pose: &sim::data::rig::RigPose| pose.get("head").unwrap().translation.unwrap()[0];
    for turn in [true, false] {
        let clip = if turn { "turn" } else { "brake" };
        let (early, pose_early) = gait_clip_entry(&d, 3, turn);
        let (late, pose_late) = gait_clip_entry(&d, 12, turn);
        assert_ne!(early.fighters[0].t, late.fighters[0].t, "the idle state clocks differ");
        assert_eq!(pose_early, pose_late, "{clip} must enter at the same pose regardless of prior idle age");
        // The legacy skeleton path uses the same clock override.
        let skel_early = sim::pose::fighter_pose(&d, &early.fighters[0], early.frame);
        let skel_late = sim::pose::fighter_pose(&d, &late.fighters[0], late.frame);
        assert_eq!(skel_early, skel_late, "{clip} legacy skeleton entry");
        let follow = if turn { Input { stick: (-127, 0), ..Default::default() } } else { Input::default() };
        let mut w = early;
        let mut poses = vec![pose_early];
        for _ in 0..4 {
            tick(&mut w, &d, follow);
            let (name, clock) = sim::pose::state_anim(&d, &w.fighters[0]);
            if name == clip {
                assert_eq!(clock, Some(f64::from(w.fighters[0].gait_t)));
                poses.push(sim::pose::fighter_rig_pose(&d, &w.fighters[0], w.frame));
            }
        }
        assert!(poses.windows(2).any(|p| p[0] != p[1]), "{clip} should animate through its gait clock");
        assert!(
            poses.iter().any(|p| (head_x(p) - head_x(&idle)).abs() > 2.),
            "{clip} should move visibly: {:?}",
            poses.iter().map(head_x).collect::<Vec<_>>()
        );
        for _ in 0..10 {
            tick(&mut w, &d, follow);
        }
        let (name, _) = sim::pose::state_anim(&d, &w.fighters[0]);
        assert_ne!(name, clip, "{clip} recovers into the next gait");
    }
}

fn reflect_held_ptooie(invulnerable: bool) -> (World, f64) {
    let d = common::data();
    let players = ["plant", "wizard", "brawler"].map(PlayerConfig::human).to_vec();
    let mut w = common::sandbox(&d, "arena", players);
    w.fighters[2].x = w.fighters[0].x - 300.;
    let special = Input { buttons: 4, ..Default::default() };
    tick(&mut w, &d, special);
    for _ in 0..24 {
        tick(&mut w, &d, special);
    }
    // Wait for the held bob's low point so a grounded reflector can reach it.
    while w.projs[0].age % 36 != 27 {
        tick(&mut w, &d, special);
    }
    assert_eq!(w.projs.len(), 1, "plant should be holding Ptooie");
    assert!(w.projs[0].released.is_none());
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 1, "hold phase");
    if invulnerable {
        w.fighters[0].inv = 12;
    }
    // Nayru's Love reflector (phase 4..40) catches the held ball. The damaging window is over.
    // A grounded reflector reaches the low point of the held bob.
    let mut wz = w.fighters[1].clone();
    wz.x = w.projs[0].x;
    wz.y = w.fighters[0].y;
    wz.face = -1.;
    wz.vx = 0.;
    sim::moves::start_move(&mut w, &d, 1, &mut wz, "nspec");
    wz.mov.as_mut().unwrap().t = 30;
    w.fighters[1] = wz;
    let face = w.fighters[0].face;
    let staged = w.clone();
    let mut other = staged.clone();
    tick(&mut w, &d, special);
    tick(&mut other, &d, special);
    assert_eq!(w, other, "reflection is deterministic");
    assert_eq!(w.projs.len(), 1);
    assert_eq!(w.projs[0].owner, 1, "the reflector owns the reflected shot");
    assert!(w.projs[0].released.is_some(), "a reflected held ball becomes a shot");
    assert_eq!(w.projs[0].face, -face, "it travels away from the reflector");
    (w, face)
}

#[test]
fn reflected_held_ptooie_releases_the_caster_and_stays_an_independent_shot() {
    let d = common::data();
    let special = Input { buttons: 4, ..Default::default() };
    let (mut w, _) = reflect_held_ptooie(true);
    // The caster leaves the hold immediately instead of waiting for the 300-tick failsafe.
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 3, "caster swallows and recovers");
    let t0 = w.fighters[0].mov.as_ref().unwrap().t;
    tick(&mut w, &d, Input::default());
    assert!(w.fighters[0].mov.as_ref().is_none_or(|m| m.phase == 3 && m.t > t0), "no retriggered hold");
    // The shot keeps the reflector's ownership and travels away, then expires on its own.
    let x0 = w.projs[0].x;
    for _ in 0..10 {
        tick(&mut w, &d, Input::default());
        if let Some(p) = w.projs.first() {
            assert_eq!(p.owner, 1, "the shot must not bounce back into the same reflector");
        }
    }
    assert!(w.projs.first().is_none_or(|p| (p.x - x0) * w.fighters[0].face < 0.), "shot travels away");
    // The caster regains control well before the old 300-tick hold timeout.
    let mut recovered = 0;
    while recovered < 180 && !(w.fighters[0].state == State::Idle && w.fighters[0].lag == 0) {
        tick(&mut w, &d, Input::default());
        recovered += 1;
    }
    assert!(recovered < 180, "caster should regain control, was {:?}", w.fighters[0].state);
    tick(&mut w, &d, Input { buttons: 2, ..Default::default() });
    assert_eq!(w.fighters[0].state, State::Attack, "caster can act after the reflected hold");
    for _ in 0..120 {
        tick(&mut w, &d, Input::default());
        assert!(w.projs.iter().all(|p| p.owner == 1));
    }
    assert!(w.projs.is_empty(), "reflected shot expires");
    // Clone/restore through the reflection stays deterministic.
    let (staged, _) = reflect_held_ptooie(false);
    let mut a = staged.clone();
    let mut b = staged.clone();
    for _ in 0..24 {
        let input =
            if a.fighters[0].mov.as_ref().is_some_and(|m| m.slot == "nspec") { special } else { Input::default() };
        tick(&mut a, &d, input);
        tick(&mut b, &d, input);
        assert_eq!(a, b);
    }
}

#[test]
fn reflected_held_ptooie_damages_the_reflectors_opponent() {
    let d = common::data();
    let (mut w, _) = reflect_held_ptooie(false);
    // Put another opponent of the reflector in the shot's path.
    let ball = w.projs[0].clone();
    let brawler = &mut w.fighters[2];
    brawler.x = ball.x - 70.;
    brawler.y = ball.y + 40.;
    brawler.ground = false;
    brawler.surf = None;
    brawler.dmg = 0.;
    let mut damaged = false;
    for _ in 0..12 {
        tick(&mut w, &d, Input::default());
        damaged |= w.fighters[2].dmg > 1e-9;
        assert!(w.projs.iter().all(|p| p.owner == 1), "ownership stays with the reflector");
    }
    assert!(damaged, "the reflected shot should damage the reflector's opponent");
    assert_eq!(w.fighters[2].last_hit_by, Some(1), "damage is credited to the reflector");
}

#[test]
fn countered_held_ball_releases_caster_and_becomes_an_independent_shot() {
    let d = common::data();
    let mut w = common::sandbox(&d, "arena", ["plant", "swordfighter"].map(PlayerConfig::human).to_vec());
    let special = Input { buttons: 4, ..Default::default() };
    for _ in 0..25 {
        tick(&mut w, &d, special);
    }
    assert!(w.projs[0].released.is_none());
    let ball = w.projs[0].clone();
    let mut defender = w.fighters[1].clone();
    sim::moves::start_move(&mut w, &d, 1, &mut defender, "dspec");
    defender.mov.as_mut().unwrap().t = 5;
    defender.x = ball.x;
    defender.y = ball.y + 40.;
    defender.ground = false;
    defender.surf = None;
    defender.inv = 0;
    w.fighters[1] = defender;
    w.fighters[0].inv = 120;
    let mut restored = w.clone();
    sim::articles::step(&mut w, &d, 0);
    sim::articles::step(&mut restored, &d, 0);
    assert_eq!(w, restored);
    let shot = &w.projs[0];
    assert_eq!(shot.owner, 1);
    assert!(shot.released.is_some());
    assert_eq!(
        shot.cooldown,
        d.fighter("plant").unwrap().projectiles["ptooie"].article.as_ref().unwrap().reflect_cooldown
    );
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().phase, 3);
    let x = shot.x;
    for _ in 0..8 {
        tick(&mut w, &d, Input::default());
        tick(&mut restored, &d, Input::default());
        assert_eq!(w, restored);
        assert!(w.projs.iter().all(|p| p.owner == 1 && p.released.is_some()));
    }
    assert!(!w.projs.is_empty());
    assert_ne!(w.projs[0].x, x);
    for _ in 0..180 {
        tick(&mut w, &d, Input::default());
    }
    assert!(w.projs.is_empty());
    assert!(w.fighters[0].mov.as_ref().is_none_or(|m| m.slot != "nspec"));
}
