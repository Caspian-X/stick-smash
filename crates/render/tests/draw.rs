//! Drives CPU matches and tessellates every tick without Bevy: every state/move pose resolves, every attachment,
//! projectile and effect draws, and the camera stays inside the stage's camera bounds.

use std::fs;
use std::path::Path;

use render::canvas::Canvas;
use render::draw::{FighterView, draw_fighter, draw_projectile};
use render::fx::{CameraRig, FxLayer};
use render::{Rand, debug, pose, stage};
use sim::data::{Source, load};
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, step_world};

fn data() -> GameData {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut files = vec![];
    let mut dirs = vec![root.clone()];
    while let Some(d) = dirs.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() && p.file_name().is_some_and(|n| n != "schema") {
                dirs.push(p);
            } else if p.extension().is_some_and(|x| x == "json") {
                let path = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                files.push(Source { path, text: fs::read_to_string(&p).unwrap() });
            }
        }
    }
    load(files).unwrap().0
}

#[test]
fn cpu_matches_draw_cleanly() {
    let d = data();
    let anims: Vec<String> = d.fighters[0].anims.keys().cloned().collect();
    for seed in [1, 2] {
        let players = d.fighters.iter().map(|f| PlayerConfig { cpu: Some(2), ..PlayerConfig::human(&f.id) }).collect();
        let mut w = World::new(MatchConfig { seed, stocks: 2, players, ..Default::default() }, &d);
        let (mut fx, mut cam, mut rng, mut cv) =
            (FxLayer::default(), CameraRig::default(), Rand::default(), Canvas::new(1.));
        let s = &d.stages[w.stage];
        let inputs = vec![Input::default(); w.fighters.len()];
        for _ in 0..60 * 90 {
            step_world(&mut w, &d, &inputs);
            fx.consume(&d, &w, &w.fx.clone(), [0., 0., 1280., 720.]);
            fx.tick(&d);
            w.fx.clear();
            cam.tick(&d, &w, 16. / 9.);
            let c = cam.at(1.).unwrap();
            let v = c.view(16. / 9.);
            assert!(
                v[0] >= s.camera.left - 1e-6
                    && v[2] <= s.camera.right + 1e-6
                    && v[1] >= s.camera.top - 1e-6
                    && v[3] <= s.camera.bottom + 1e-6
            );
            cv.clear(1., d.global.render.alpha_gamma);
            for p in &w.projs {
                draw_projectile(&mut cv, &d, p, p.x, p.y, w.frame);
            }
            for (i, f) in w.fighters.iter().enumerate() {
                if !matches!(f.state, sim::world::State::Attack) {
                    let (name, _) = pose::state_anim(&d, f);
                    assert!(
                        anims.contains(&name) || matches!(f.state, sim::world::State::Dead | sim::world::State::Out),
                        "no anim {name}"
                    );
                }
                let skel = pose::fighter_pose(&d, f, w.frame);
                draw_fighter(&mut cv, &d, w.frame, &FighterView { i, f, x: f.x, y: f.y, skel }, &mut rng);
                let _ = debug::info(&d, f);
            }
            fx.draw(&mut cv, &d, 1.);
            debug::draw_debug(&mut cv, &d, &w, &[]);
            assert!(cv.pos.iter().flatten().all(|v| v.is_finite()));
            assert!(cv.idx.iter().all(|&i| (i as usize) < cv.pos.len()));
        }
        cv.clear(1., 1.);
        stage::draw_backdrop(&mut cv, &d, s);
        stage::draw_stage(&mut cv, &d, s);
        assert!(!cv.idx.is_empty());
    }
}

#[test]
fn poses_resolve_from_chains_and_conditions() {
    let d = data();
    let fd = d.fighters.iter().find(|f| f.id == "brawler").unwrap();
    let stand = pose::resolve(fd, &fd.poses["stand"]);
    assert_eq!(stand.head, [0., 64.]);
    let crouch = pose::resolve(fd, &fd.poses["crouch"]);
    assert_eq!(crouch.hip, [0., 18.]);
    assert_eq!(crouch.pivot, stand.pivot, "missing fields come from stand");
    // Grounded ground pound startup must not spin (keys gated `if: airborne`).
    let pound = &fd.moves.dspec.phases[0];
    let w = World::new(
        MatchConfig {
            players: vec![PlayerConfig::human("brawler"), PlayerConfig::human("king")],
            ..Default::default()
        },
        &d,
    );
    let mut f = w.fighters[0].clone();
    f.ground = true;
    assert_eq!(pose::sample(fd, &pound.anim, 6., None, &f).rot, 0.);
    f.ground = false;
    assert!(pose::sample(fd, &pound.anim, 6., None, &f).rot > 100.);
}
