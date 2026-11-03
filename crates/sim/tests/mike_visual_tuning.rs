mod common;
use sim::{GameData, World, combat, input::Intent, moves};

fn setup(slot: &str) -> (GameData, World) {
    let d = common::data();
    let mut w = common::fighters(&d, "mike", &["brawler"]);
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, slot);
    w.fighters[0] = f;
    (d, w)
}

#[test]
fn down_tilt_follows_the_low_downward_blade_in_both_facings() {
    let (d, mut w) = setup("dtilt");
    for face in [-1., 1.] {
        let f = &mut w.fighters[0];
        f.face = face;
        f.mov.as_mut().unwrap().t = 7;
        let boxes = combat::active_boxes(&d, f);
        assert_eq!(boxes.len(), 4);
        for pair in boxes.windows(2) {
            assert!((pair[1].x - pair[0].x) * face > 0.);
            assert!(pair[1].y > pair[0].y, "the outer blade must descend in world space");
        }
        assert!(boxes.iter().all(|b| (0. ..25.).contains(&b.hit.y)));
    }
}

#[test]
fn up_air_reaches_behind_and_below_the_rear_shoulder() {
    let (d, mut w) = setup("uair");
    let f = &mut w.fighters[0];
    f.mov.as_mut().unwrap().phase = 1;
    f.mov.as_mut().unwrap().t = 12;
    for face in [-1., 1.] {
        f.face = face;
        let boxes = combat::active_boxes(&d, f);
        // Rear target near shoulder height, beyond the old overhead arc's coverage.
        let target =
            sim::data::Rect { x1: f.x - face * 110. - 3., x2: f.x - face * 110. + 3., y1: f.y - 58., y2: f.y - 42. };
        assert!(boxes.iter().any(|b| combat::circ_rect(b.x, b.y, b.hit.r, target)));
    }
}

#[test]
fn forward_smash_preserves_verified_commitment_before_the_downward_hit() {
    let (d, mut w) = setup("fsmash");
    let f = &mut w.fighters[0];
    for t in 0..31 {
        f.mov.as_mut().unwrap().t = t;
        assert!(combat::active_boxes(&d, f).is_empty(), "frame {t}");
    }
    f.mov.as_mut().unwrap().t = 31;
    assert!(combat::active_boxes(&d, f).iter().all(|b| b.hit.damage == 19.));
    f.mov.as_mut().unwrap().t = 33;
    assert!(combat::active_boxes(&d, f).iter().any(|b| b.hit.damage == 22.));
}

#[test]
fn eruption_flames_mark_each_unlocked_column_through_its_active_window() {
    for charge in [0, 100, 209] {
        let (d, mut w) = setup("nspec");
        let mut f = w.fighters[0].clone();
        f.mov.as_mut().unwrap().charge = charge;
        f.mov.as_mut().unwrap().t = 29;
        let mut columns = [false; 3];
        for _ in 30..=41 {
            w.fx.clear();
            moves::st_attack(&mut w, &d, 0, &mut f, Intent::default(), 0.);
            for effect in &w.fx {
                if let sim::Fx::Part { x, y, vy, .. } = effect {
                    assert!(*vy < 0., "flames must rise");
                    let x = (x - f.x) * f.face;
                    if *y < f.y - 90. {
                        if x < 110. {
                            columns[0] = true;
                        } else if x < 180. {
                            columns[1] = true;
                        } else {
                            columns[2] = true;
                        }
                    }
                }
            }
        }
        assert_eq!(columns, [true, charge >= 100, charge >= 209]);
    }
}
