mod common;
use sim::{GameData, Input, World, moves};

fn setup(id: &str, distance: f64) -> (GameData, World) {
    let d = common::data();
    let mut w = common::fighters(&d, id, &["brawler"]);
    w.count = 0;
    w.fighters[0].x = 450.;
    w.fighters[0].face = 1.;
    w.fighters[1].x = 450. + distance;
    (d, w)
}

fn start(w: &mut World, d: &GameData, slot: &str) {
    let mut f = w.fighters[0].clone();
    moves::start_move(w, d, 0, &mut f, slot);
    w.fighters[0] = f;
}

#[test]
fn ninja_throws_three_separate_stars() {
    let (d, mut w) = setup("ninja", 400.);
    start(&mut w, &d, "nspec");
    for _ in 0..26 {
        common::step_preserving(&mut w, &d, &[Input::default(); 2]);
    }
    assert_eq!(w.projs.len(), 3);
    assert!(w.projs.windows(2).all(|p| p[0].x > p[1].x));
}

#[test]
fn yoyo_can_hit_on_both_outward_and_return_passes() {
    let (d, mut w) = setup("yoyo", 95.);
    start(&mut w, &d, "nspec");
    let mut phases = vec![];
    for _ in 0..45 {
        // Keep the dummy at the same point so both passes exercise their hit registration.
        w.fighters[1].x = 545.;
        w.fighters[1].y = w.fighters[0].y;
        w.fighters[1].vx = 0.;
        w.fighters[1].vy = 0.;
        common::step_preserving(&mut w, &d, &[Input::default(); 2]);
        if w.hits.drain(..).any(|h| h.attacker == Some(0) && !h.shielded) {
            phases.push(moves::phase(&d, &w.fighters[0]).name.clone());
        }
    }
    assert_eq!(phases, ["out", "return"]);
}

#[test]
fn wrangler_lasso_pulls_a_distant_victim_inward() {
    let (d, mut w) = setup("wrangler", 90.);
    start(&mut w, &d, "nspec");
    for _ in 0..20 {
        common::step_preserving(&mut w, &d, &[Input::default(); 2]);
        if !w.hits.is_empty() {
            assert!(w.fighters[1].vx < 0., "lasso must pull toward its owner");
            return;
        }
    }
    panic!("lasso missed the distant victim");
}

#[test]
fn neutral_cpu_tactics_preserve_rng_and_can_select_down_special() {
    let (mut d, mut w) = setup("ninja", 250.);
    d.global.ai.aggression = [1.; 3];
    w.fighters[0].ai.cool = 0.;
    w.fighters[0].face = -1.;
    let data = w.fighters[0].data;
    d.fighters[data].ai.specials[0].chance = 1.;
    for down in [false, true] {
        d.fighters[data].ai.specials[0].down = down;
        let mut neutral = w.clone();
        let input = sim::ai::ai_input(&mut neutral, &d, 0, 2);
        assert!(input.sp && !input.left && !input.right && !input.up);
        assert_eq!(input.down, down);
        assert_eq!(neutral.fighters[0].face, 1.);
        let mut toward = w.clone();
        d.fighters[data].ai.specials[0].neutral = false;
        let other = sim::ai::ai_input(&mut toward, &d, 0, 2);
        assert!(other.sp && other.right);
        assert_eq!(neutral.rng.next_f64(), toward.rng.next_f64());
        d.fighters[data].ai.specials[0].neutral = true;
    }
}

#[test]
fn ninja_shadow_dash_crosses_an_opponent() {
    let (d, mut w) = setup("ninja", 45.);
    start(&mut w, &d, "sspec");
    let mut protected = false;
    for _ in 0..17 {
        common::step_preserving(&mut w, &d, &[Input::default(); 2]);
        protected |= w.fighters[0].inv > 0;
    }
    assert!(protected && w.fighters[0].x > w.fighters[1].x);
}

#[test]
fn ninja_substitution_advances_and_strikes_back() {
    let (d, mut w) = setup("ninja", 40.);
    w.fighters[1].face = -1.;
    start(&mut w, &d, "dspec");
    w.fighters[0].mov.as_mut().unwrap().t = 5;
    let mut attacker = w.fighters[1].clone();
    moves::start_move(&mut w, &d, 1, &mut attacker, "jab");
    w.fighters[1] = attacker;
    let mut strike = false;
    let mut furthest = w.fighters[0].x;
    for _ in 0..40 {
        common::step_preserving(&mut w, &d, &[Input::default(); 2]);
        furthest = furthest.max(w.fighters[0].x);
        strike |= w.hits.drain(..).any(|h| h.attacker == Some(0));
    }
    assert!(furthest > 450., "substitution must advance toward a melee attacker");
    assert!(strike, "substitution must hit back after crossing");
}

#[test]
fn neutral_cpu_tactics_never_turn_locked_or_airborne_fighters() {
    use sim::world::State;
    let (mut d, base) = setup("ninja", 250.);
    d.global.ai.aggression = [1.; 3];
    for state in [
        State::Throw,
        State::Attack,
        State::Roll,
        State::Landing,
        State::Tech,
        State::Getup,
        State::Climb,
        State::Air,
        State::Idle,
    ] {
        let mut w = base.clone();
        if state == State::Attack {
            start(&mut w, &d, "nspec");
        } else {
            w.fighters[0].set_state(state);
        }
        w.fighters[0].face = -1.;
        w.fighters[0].vx = 2.5;
        w.fighters[0].ground = state != State::Air;
        w.fighters[0].lag = u32::from(state == State::Idle);
        w.fighters[0].ai.cool = 0.;
        let mut without = w.clone();
        let mut no_tactics = d.clone();
        no_tactics.fighters[w.fighters[0].data].ai.specials.clear();
        let intent = sim::ai::ai_input(&mut w, &d, 0, 2);
        sim::ai::ai_input(&mut without, &no_tactics, 0, 2);
        assert_eq!(w.fighters[0].face, -1., "{state:?}");
        assert_eq!(w.fighters[0].vx, 2.5, "{state:?}");
        assert!(!intent.sp, "{state:?}");
        assert_eq!(w.rng.next_f64(), without.rng.next_f64(), "{state:?}: rolled a blocked tactic");
    }
}

#[test]
fn ninja_substitution_reaches_a_far_whip_attacker() {
    let (d, mut w) = setup("ninja", 115.);
    w.fighters[1].data = d.fighters.iter().position(|f| f.id == "wrangler").unwrap();
    w.fighters[1].face = -1.;
    start(&mut w, &d, "dspec");
    w.fighters[0].mov.as_mut().unwrap().t = 5;
    let mut attacker = w.fighters[1].clone();
    moves::start_move(&mut w, &d, 1, &mut attacker, "fsmash");
    attacker.mov.as_mut().unwrap().t = 15;
    w.fighters[1] = attacker;
    let mut strike = false;
    for _ in 0..40 {
        common::step_preserving(&mut w, &d, &[Input::default(); 2]);
        strike |= w.hits.drain(..).any(|h| h.attacker == Some(0));
    }
    assert!(strike, "substitution must reach the whip's owner after countering its tip");
}

#[test]
fn ninja_projectile_substitution_does_not_dash_off_a_ledge() {
    let (d, mut w) = setup("ninja", 60.);
    let x = d.stages[w.stage].blocks[0].left + 8.;
    w.fighters[0].x = x;
    w.fighters[0].face = -1.;
    w.fighters[1].x = x + 60.;
    w.fighters[1].face = -1.;
    start(&mut w, &d, "dspec");
    w.fighters[0].mov.as_mut().unwrap().t = 5;
    let attacker = w.fighters[1].clone();
    sim::projectiles::spawn(&mut w, &d, 1, &attacker, "energyBall");
    let mut reflected = false;
    for _ in 0..20 {
        common::step_preserving(&mut w, &d, &[Input::default(); 2]);
        reflected |= w.projs.iter().any(|p| p.owner == 0);
    }
    assert!(reflected);
    assert_eq!(w.fighters[0].x, x);
    assert!(w.fighters[0].ground);
}

fn kit_tick(w: &mut World, d: &GameData) {
    let mut f = w.fighters[0].clone();
    moves::st_attack(w, d, 0, &mut f, Default::default(), 0.);
    w.fighters[0] = f;
    w.frame += 1;
}

#[test]
fn ninja_star_spacing_leaves_a_break_in_low_percent_hitstun() {
    let (d, mut w) = setup("ninja", 1000.);
    start(&mut w, &d, "nspec");
    let mut spawned = vec![];
    let mut count = 0;
    for t in 0..45 {
        if w.projs.len() > count {
            spawned.push(t);
            count = w.projs.len();
        }
        kit_tick(&mut w, &d);
    }
    assert_eq!(spawned, [7, 16, 25]);
    assert!(w.fighters[0].mov.is_none());
    let star = &d.fighter("ninja").unwrap().projectiles["star"].hit;
    let kb = sim::combat::knockback(&d, star, star.damage, 1.);
    let stun = sim::combat::hitstun_of(&d, star, kb);
    assert!(stun > 0 && stun < 9, "stun {stun} must end before the next star");
}

#[test]
fn lancer_thrusts_reward_exact_tip_spacing_and_only_the_first_active_frame() {
    for (slot, first, end, sweet, late, shaft) in [
        ("ftilt", 6, 10, 17., 12., 9.),
        ("fair", 6, 10, 18., 10., 8.),
        ("fsmash", 12, 16, 25., 20., 14.),
        ("nspec", 14, 18, 22., 15., 13.),
    ] {
        for face in [-1., 1.] {
            let (d, mut base) = setup("lancer", 1000.);
            base.fighters[0].face = face;
            start(&mut base, &d, slot);
            for t in 0..=end + 1 {
                let mut w = base.clone();
                w.fighters[0].mov.as_mut().unwrap().t = t;
                let boxes = sim::combat::active_boxes(&d, &w.fighters[0]);
                assert_eq!(!boxes.is_empty(), (first..=end).contains(&t), "{slot} {t}");
                if boxes.is_empty() {
                    continue;
                }
                let tip = &boxes[0];
                assert_eq!(tip.hit.damage, if t == first { sweet } else { late });
                let shaft_box = &boxes[1];
                let tip_kb = sim::combat::knockback(&d, &tip.hit, 80. + tip.hit.damage, 1.);
                let shaft_kb = sim::combat::knockback(&d, &shaft_box.hit, 80. + shaft_box.hit.damage, 1.);
                assert!(tip_kb > shaft_kb, "{slot}: tip must launch harder");
                for (x, y, expected) in [(tip.x, tip.y, tip.hit.damage), (shaft_box.x, shaft_box.y, shaft)] {
                    let mut contact = w.clone();
                    let victim = &mut contact.fighters[1];
                    victim.x = x;
                    victim.y = y + 45.;
                    victim.inv = 0;
                    sim::combat::hit_pass(&mut contact, &d);
                    assert_eq!(contact.fighters[1].dmg, expected, "{slot} frame {t} face {face}");
                }
            }
        }
    }
}

#[test]
fn dancer_spiral_has_bounded_hits_and_punishable_endings() {
    use sim::world::State;
    let (d, mut w) = setup("dancer", 1000.);
    w.fighters[0].ground = false;
    w.fighters[0].y = -1000.;
    start(&mut w, &d, "uspec");
    let mut contacts = vec![];
    for t in 0..43 {
        let boxes = sim::combat::active_boxes(&d, &w.fighters[0]);
        assert_eq!(!boxes.is_empty(), (4..=18).contains(&t) || (20..=23).contains(&t), "frame {t}");
        if let Some(b) = boxes.first() {
            assert!(b.hit.r <= 38.);
            if t < 20 {
                assert_eq!(b.hit.rehit, Some(7));
                assert_eq!(b.hit.damage, 1.5);
            } else {
                assert_eq!(b.hit.damage, 6.);
            }
        }
        if let Some(b) = boxes.first() {
            w.fighters[0].lag = 0;
            let victim = &mut w.fighters[1];
            victim.x = b.x;
            victim.y = b.y + 45.;
            victim.inv = 0;
            sim::combat::hit_pass(&mut w, &d);
            if w.hits.drain(..).any(|h| h.attacker == Some(0)) {
                contacts.push(t);
            }
        }
        if t == 10 || t == 25 {
            let mut landed = w.fighters[0].clone();
            landed.ground = true;
            moves::trigger(&mut w, &d, 0, &mut landed, sim::data::Trigger::Land);
            assert_eq!(landed.state, State::Landing);
            assert_eq!(landed.sd.n, 16.);
        }
        kit_tick(&mut w, &d);
    }
    assert_eq!(w.fighters[0].state, State::Helpless);
    assert_eq!(contacts, [4, 11, 18, 20]);
    assert_eq!(w.fighters[1].dmg, 10.5);
}
