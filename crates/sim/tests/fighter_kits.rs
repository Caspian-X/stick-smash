mod common;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, moves, step_world};

fn setup(id: &str, distance: f64) -> (GameData, World) {
    let d = common::data();
    let mut w = World::new(
        MatchConfig {
            training: true,
            players: vec![PlayerConfig::human(id), PlayerConfig::human("brawler")],
            ..Default::default()
        },
        &d,
    )
    .unwrap();
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
    for _ in 0..18 {
        step_world(&mut w, &d, &[Input::default(); 2]);
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
        step_world(&mut w, &d, &[Input::default(); 2]);
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
        step_world(&mut w, &d, &[Input::default(); 2]);
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
        step_world(&mut w, &d, &[Input::default(); 2]);
        protected |= w.fighters[0].inv > 0;
    }
    assert!(protected && w.fighters[0].x > w.fighters[1].x);
}

#[test]
fn ninja_substitution_crosses_and_strikes_back() {
    let (d, mut w) = setup("ninja", 40.);
    w.fighters[1].face = -1.;
    start(&mut w, &d, "dspec");
    w.fighters[0].mov.as_mut().unwrap().t = 5;
    let mut attacker = w.fighters[1].clone();
    moves::start_move(&mut w, &d, 1, &mut attacker, "jab");
    w.fighters[1] = attacker;
    let mut strike = false;
    for _ in 0..40 {
        step_world(&mut w, &d, &[Input::default(); 2]);
        strike |= w.hits.drain(..).any(|h| h.attacker == Some(0));
    }
    assert!(w.fighters[0].x > w.fighters[1].x, "substitution must cross the attacker");
    assert!(strike, "substitution must hit back after crossing");
}
