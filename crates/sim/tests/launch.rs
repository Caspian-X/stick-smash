//! The decaying launch model (`hitstun.ultimate`): launch shape, gravity, hitstun/hitlag formulas, tech lockout,
//! hitstun cancel, and the untouched legacy path.
mod common;
use sim::{
    GameData, Input, World, combat,
    data::{Hitbox, global::HitstunCancel},
    input::{ATTACK, SHIELD},
    world::State,
};

/// A fixed-knockback hit: reference knockback `kb` against a weight-1 victim.
fn hit(kb: f64, angle: f64) -> Hitbox {
    serde_json::from_value(serde_json::json!({
        "x": 0, "y": 0, "r": 10, "damage": 1, "bkb": kb, "kbg": 0, "angle": angle, "setKb": true
    }))
    .unwrap()
}
fn src() -> combat::HitSource {
    combat::HitSource { attacker: 1, direction: 1., point: (0., 0.), projectile: false, label: None }
}
/// Blast zones far away, so only the launch decides travel.
fn wide(mut d: GameData) -> GameData {
    for s in &mut d.stages {
        s.blast = sim::data::stage::Bounds { left: -1e6, right: 1e6, top: -1e6, bottom: 1e6 };
    }
    d
}
/// Victim (player 0) high above the stage.
fn sky(d: &GameData, fighter: &str) -> World {
    let mut w = common::fighters(d, fighter, &["brawler"]);
    w.fighters[1].x = -5000.;
    let f = &mut w.fighters[0];
    f.set_state(State::Air);
    f.ground = false;
    f.surf = None;
    f.y -= 3000.;
    w
}
fn run(w: &mut World, d: &GameData, n: usize) {
    common::run_draining(w, d, n, &[Input::default()]);
}

#[test]
fn horizontal_travel_grows_quadratically_with_knockback() {
    let d = wide(common::data());
    let travel = |kb: f64| {
        let mut w = sky(&d, "brawler");
        let x = w.fighters[0].x;
        combat::apply_hit(&mut w, &d, 0, &hit(kb, 0.), src());
        let l = w.fighters[0].launch;
        assert!(l.0 > 0. && l.1.abs() < 1e-9, "{l:?}");
        run(&mut w, &d, 200);
        assert_eq!(w.fighters[0].launch, (0., 0.), "the launch decays to zero and stays there");
        w.fighters[0].x - x
    };
    let (low, high) = (travel(80.), travel(160.));
    let ratio = high / low;
    assert!((3.5..=4.1).contains(&ratio), "travel {low:.0} → {high:.0} px, ratio {ratio:.2}");
}

#[test]
fn a_spike_keeps_its_speed_past_every_fall_cap() {
    let d = wide(common::data());
    let mut w = sky(&d, "brawler");
    combat::apply_hit(&mut w, &d, 0, &hit(200., 270.), src());
    let lag = w.fighters[0].lag as usize;
    run(&mut w, &d, lag);
    let cap = d.global.hitstun.max_fall.max(d.fighter("brawler").unwrap().stats.fall_speed);
    for tick in 0..3 {
        run(&mut w, &d, 1);
        let f = &w.fighters[0];
        assert_eq!(f.state, State::Hitstun);
        assert!(f.vy > cap && f.vx.abs() < 1e-9, "tick {tick}: vy {} vs cap {cap}", f.vy);
    }
}

#[test]
fn victim_gravity_decides_when_the_same_hit_lands() {
    let d = common::data();
    let land = |fighter: &str| {
        let mut w = common::fighters(&d, fighter, &["brawler"]);
        w.fighters[1].x = 100.;
        assert_eq!(d.fighters[w.fighters[0].data].stats.weight, 1.);
        combat::apply_hit(&mut w, &d, 0, &hit(50., 90.), src());
        let mut ticks = 0;
        while !w.fighters[0].ground && ticks < 300 {
            run(&mut w, &d, 1);
            ticks += 1;
        }
        assert_eq!(w.fighters[0].launch, (0., 0.), "landing clears the launch");
        ticks
    };
    let (heavy, floaty) = (land("brawler"), land("stratus"));
    assert!(floaty > heavy, "stratus (lower gravity) lands at {floaty}, brawler at {heavy}");
}

#[test]
fn hitstun_and_tumble_use_reference_knockback() {
    let d = common::data();
    let u = d.global.hitstun.ultimate.as_ref().unwrap();
    assert_eq!((u.per_reference_knockback, u.base, u.tumble_threshold), (0.4, 0., 80.));
    for kb in [20., 79., 80., 81., 97., 160., 250.] {
        let mut b = hit(kb, 45.);
        b.hitstun_mult = 1.5;
        b.hitstun_add = 2.;
        let mut w = common::fighters(&d, "brawler", &["brawler"]);
        combat::apply_hit(&mut w, &d, 0, &b, src());
        let expected = (libm::floor(0.4 * kb) * 1.5 + 2.) as i32;
        let f = &w.fighters[0];
        assert_eq!(f.hitstun, expected, "KB {kb}");
        assert_eq!(f.tumble, kb > 80., "KB {kb}");
    }
}

#[test]
fn hitlag_follows_damage_only() {
    let mut d = common::data();
    for ultimate in [true, false] {
        d.global.hitlag.mult_before_cap = ultimate;
        for damage in [0., 1., 4.6, 10., 19.5, 30., 40., 60.] {
            for mult in [1., 0.5, 1.45] {
                let mut b = hit(100., 45.);
                b.set_kb = false;
                b.damage = damage;
                b.hitlag_mult = mult;
                // Ultimate multiplies before flooring and capping (10% × 1.45: floor(12.5 × 1.45) = 18, not 12 × 1.45).
                let expected = if ultimate {
                    libm::floor((damage * 0.65 + 6.) * mult).min(30.) as u32
                } else {
                    libm::floor(libm::floor(damage * 0.65 + 6.).min(30.) * mult) as u32
                };
                for kb in [0., 20.] {
                    assert_eq!(combat::hitlag_of(&d, &b, kb), expected, "{damage} × {mult}");
                }
                let mut w = common::fighters(&d, "brawler", &["brawler"]);
                combat::apply_hit(&mut w, &d, 0, &b, src());
                assert_eq!((w.fighters[0].lag, w.fighters[1].lag), (expected, expected), "{damage} × {mult}");
            }
        }
    }
}

/// Tumble onto the stage from above; `press(tick, landing)` decides player 0's shield each tick.
fn tumble_landing(d: &GameData, press: impl Fn(usize, usize) -> bool) -> State {
    let fall = |press: &dyn Fn(usize) -> bool| {
        let mut w = common::fighters(d, "brawler", &["brawler"]);
        w.fighters[1].x = 100.;
        let f = &mut w.fighters[0];
        f.set_state(State::Hitstun);
        f.tumble = true;
        f.hitstun = 500;
        f.ground = false;
        f.surf = None;
        f.y -= 600.;
        let mut tick = 0;
        while w.fighters[0].state == State::Hitstun && tick < 300 {
            tick += 1;
            let i = Input { buttons: if press(tick) { SHIELD } else { 0 }, ..Default::default() };
            common::step_draining(&mut w, d, &[i]);
        }
        (tick, w.fighters[0].state)
    };
    let (landing, state) = fall(&|_| false);
    assert_eq!(state, State::Knockdown);
    fall(&|tick| press(tick, landing)).1
}

#[test]
fn tech_lockout_stops_mashing_but_not_a_timed_press() {
    let mut d = common::data();
    assert_eq!(d.global.tech.lockout, 40);
    assert_eq!(tumble_landing(&d, |t, land| t == land - 4), State::Tech, "one press 4 ticks before landing");
    let mash = |t: usize, land: usize| t + 30 >= land && t % 2 == land % 2;
    assert_eq!(tumble_landing(&d, mash), State::Knockdown, "mashing from 30 ticks out locks the tech");
    d.global.tech.lockout = 0;
    assert_eq!(tumble_landing(&d, mash), State::Tech, "without the lockout mashing techs");
}

#[test]
fn hitstun_cancel_is_off_by_default_and_opens_after_its_frames() {
    let mut d = wide(common::data());
    assert!(d.global.hitstun.ultimate.as_ref().unwrap().cancel.is_none());
    d.global.hitstun.ultimate.as_mut().unwrap().cancel = Some(HitstunCancel { air_dodge_after: 10, aerial_after: 15 });
    let mut w = sky(&d, "brawler");
    combat::apply_hit(&mut w, &d, 0, &hit(150., 90.), src());
    assert!(w.fighters[0].tumble);
    let lag = w.fighters[0].lag as usize;
    run(&mut w, &d, lag);
    let step = |w: &mut World, buttons: u8| common::step_draining(w, &d, &[Input { buttons, ..Default::default() }]);
    // Early presses do nothing (and expire from the press buffer before the cancel opens).
    for t in 1..=10 {
        step(&mut w, if t == 1 || t == 3 { SHIELD | ATTACK } else { 0 });
        assert_eq!(w.fighters[0].state, State::Hitstun, "tick {t}");
    }
    step(&mut w, SHIELD);
    assert_eq!(w.fighters[0].state, State::Airdodge);
    assert_eq!(w.fighters[0].launch, (0., 0.), "leaving hitstun for another state clears the launch");
}

#[test]
fn legacy_hitstun_is_unchanged() {
    let mut d = wide(common::linear_data());
    d.global.hitstun.ultimate = None;
    d.global.hitlag =
        sim::data::global::Hitlag { base: 3., per_damage: 0.45, per_knockback: 0.1, max: 16., mult_before_cap: false };
    d.global.tech.lockout = 0;
    let mut w = sky(&d, "brawler");
    let b = hit(10., 45.);
    combat::apply_hit(&mut w, &d, 0, &b, src());
    let f = &w.fighters[0];
    // Linear fixed knockback 10 px: hitstun floor(10 × 1.7) + 4, hitlag floor(3 + 0.45 + 1).
    assert_eq!((f.hitstun, f.lag, f.launch), (21, 4, (0., 0.)));
    let (mut x, mut y, mut vx, mut vy) = (f.x, f.y, f.vx, f.vy);
    run(&mut w, &d, 4);
    let h = &d.global.hitstun;
    for tick in 1..=21 {
        run(&mut w, &d, 1);
        vx = vx * h.air_drag + 0. * h.drift_accel;
        vy = (vy + h.gravity).min(h.max_fall);
        x += vx;
        y += vy;
        let f = &w.fighters[0];
        assert_eq!((f.x, f.y, f.vx, f.vy), (x, y, vx, vy), "tick {tick}");
        assert_eq!(f.state, if tick < 21 { State::Hitstun } else { State::Air });
    }
}

#[test]
fn sakurai_angle_ramps_with_knockback_on_the_ground() {
    let d = common::data();
    let mut b = hit(0., 45.);
    b.kind = sim::data::HitType::Sakurai;
    let scale = combat::reference_scale(&d);
    let px = |kb: f64| kb * scale;
    // Ultimate: 0° up to 60 reference knockback, rising linearly to 38° at 88; 38° in the air.
    for (kb, angle) in [(40., 0.), (60., 0.), (74., 19.), (88., 38.), (150., 38.)] {
        assert!((combat::launch_angle(&d, &b, px(kb), true) - angle).abs() < 1e-9, "KB {kb}");
        assert_eq!(combat::launch_angle(&d, &b, px(kb), false), 38.);
    }
    // Without a ramp the tumble step rule remains.
    let mut d = d;
    d.global.hit_types.sakurai.ramp = None;
    assert_eq!(combat::launch_angle(&d, &b, px(74.), true), d.global.hit_types.sakurai.ground_low);
    assert_eq!(combat::launch_angle(&d, &b, px(81.), true), d.global.hit_types.sakurai.ground_high);
}

#[test]
fn weight_based_fixed_knockback_follows_ultimate() {
    let d = common::data();
    let mut b = hit(20., 45.);
    b.kbg = 1.;
    b.fkb = Some(30.);
    for weight in [80., 100., 127.] {
        // ((1 + FKB / 2) × 1.4 × 200 / (W + 100) + 18) × KBG / 100 + BKB, whatever the damage and percent.
        let reference = (16. * 1.4 * 200. / (weight + 100.) + 18.) + 20.;
        for (pct, damage) in [(0., 1.), (150., 20.)] {
            b.damage = damage;
            let kb = combat::knockback(&d, &b, pct, 100. / weight) / combat::reference_scale(&d);
            assert!((kb - reference).abs() < 1e-9, "W {weight}: {kb} vs {reference}");
        }
    }
}

#[test]
fn upward_tumble_uses_launch_gravity_then_slows_to_its_own_fall_speed() {
    let d = wide(common::data());
    let top = d.global.hitstun.ultimate.as_ref().unwrap().top_launch.clone().unwrap();
    let s = d.fighter("brawler").unwrap().stats.clone();
    let g = top.gravity + top.gravity_per_weight * 100. / s.weight;
    // A 90° tumble launch: the own component gains the launch gravity per tick, capped at the launch fall speed.
    let mut w = sky(&d, "brawler");
    combat::apply_hit(&mut w, &d, 0, &hit(150., 90.), src());
    assert!(w.fighters[0].top_launch);
    let lag = w.fighters[0].lag as usize;
    run(&mut w, &d, lag);
    let mut own = 0f64;
    while w.fighters[0].state == State::Hitstun {
        run(&mut w, &d, 1);
        let f = &w.fighters[0];
        if f.state != State::Hitstun {
            break;
        }
        own = (own + g).min(top.fall_speed);
        assert!((f.vy - f.launch.1 - own).abs() < 1e-9, "own {} vs {own}", f.vy - f.launch.1);
    }
    // After hitstun a fall faster than its own slows by its gravity per tick back to it.
    assert!(own > s.fall_speed);
    let mut prev = w.fighters[0].vy - w.fighters[0].launch.1;
    for _ in 0..10 {
        run(&mut w, &d, 1);
        let f = &w.fighters[0];
        let v = f.vy - f.launch.1;
        assert!((v - (prev - s.gravity).max(s.fall_speed)).abs() < 1e-9 || prev <= s.fall_speed, "{prev} → {v}");
        prev = v;
    }
    assert!((prev - s.fall_speed).abs() < 1e-9 && !w.fighters[0].top_launch);
    // A sideways tumble or a non-tumbling upward hit keeps the victim's own gravity.
    for (kb, angle) in [(150., 45.), (60., 90.)] {
        let mut w = sky(&d, "brawler");
        combat::apply_hit(&mut w, &d, 0, &hit(kb, angle), src());
        assert!(!w.fighters[0].top_launch, "{kb} at {angle}°");
    }
}
