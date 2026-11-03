mod common;
use sim::{GameData, Input, World, moves};

fn scenario(x: f64, face: f64, percent: f64) -> (GameData, World) {
    let mut d = common::data();
    // Flat floor isolates move capture from intervening platforms.
    for stage in &mut d.stages {
        stage.platforms.clear();
    }
    let mut w = common::fighters(&d, "mike", &["brawler"]);
    w.count = 0;
    for (i, f) in w.fighters.iter_mut().enumerate() {
        f.x = 640. + if i == 1 { x * face } else { 0. };
        f.y = 520.;
        f.ground = true;
        f.surf = Some(0);
        f.inv = 0;
        f.vx = 0.;
        f.vy = 0.;
        f.face = face;
    }
    w.fighters[1].dmg = percent;
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "uspec");
    w.fighters[0] = f;
    (d, w)
}

#[test]
fn grounded_aether_carries_a_neutral_target_through_landing() {
    for face in [-1., 1.] {
        for distance in [35., 65., 90.] {
            for percent in [0., 50., 100.] {
                let (d, mut w) = scenario(distance, face, percent);
                let mut contacts = vec![];
                // No target repositioning or cleared hitlag: real movement/contact throughout.
                for _ in 0..150 {
                    sim::step_world(&mut w, &d, &[Input::default(); 2]);
                    contacts.extend(w.hits.iter().filter(|h| h.attacker == Some(0)).map(|h| h.damage));
                    w.fx.clear();
                    w.hits.clear();
                    w.acts.clear();
                }
                assert_eq!(contacts, [6., 2., 1., 3., 6.], "distance {distance}, face {face}, percent {percent}");
            }
        }
    }
}
