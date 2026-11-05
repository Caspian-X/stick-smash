//! Smash-style control feel: gait, turnarounds, hops, shield options, parry, DI, fast fall, platforms, ledges.
mod common;
use sim::{
    GameData, Input, World,
    actions::Action,
    combat,
    input::{ATTACK, JUMP, JUMP2, SHIELD},
    physics,
    world::{Gait, State},
};

fn mike(d: &GameData) -> World {
    let mut w = common::fighters(d, "mike", &["brawler"]);
    w.fighters[0].x = 640.;
    w.fighters[1].x = 320.;
    w
}
fn inp(x: i8, y: i8, buttons: u8) -> Input {
    Input { stick: (x, y), buttons, ..Default::default() }
}
/// Step with player 0's input; returns player 0's accepted actions.
fn tick(w: &mut World, d: &GameData, i: Input) -> Vec<Action> {
    common::step_preserving(w, d, &[i]);
    w.fx.clear();
    w.hits.clear();
    std::mem::take(&mut w.acts).into_iter().filter(|a| a.player == 0).map(|a| a.action).collect()
}
fn run(w: &mut World, d: &GameData, n: usize, i: Input) {
    for _ in 0..n {
        tick(w, d, i);
    }
}
fn slot(w: &World) -> Option<&str> {
    w.fighters[0].mov.as_ref().map(|m| m.slot.as_str())
}
fn airborne(w: &mut World) {
    let f = &mut w.fighters[0];
    f.set_state(State::Air);
    f.ground = false;
    f.surf = None;
    f.y -= 150.;
}

#[test]
fn slow_tilt_walks_proportionally_and_a_flick_dashes_into_a_run() {
    let d = common::data();
    let s = &d.fighter("mike").unwrap().stats;
    let walk = s.walk_speed.unwrap_or(s.run_speed * d.global.gait.walk_speed);
    let mut w = mike(&d);
    for x in [20, 40, 60, 80, 90] {
        tick(&mut w, &d, inp(x, 0, 0));
    }
    run(&mut w, &d, 30, inp(90, 0, 0));
    assert_eq!(w.fighters[0].gait, Gait::Walk);
    assert!((w.fighters[0].vx - walk * sim::input::analog(90, d.global.input.deadzone)).abs() < 1e-9);
    run(&mut w, &d, 30, inp(18 + 109 / 2, 0, 0));
    assert!((w.fighters[0].vx - walk * 0.5).abs() < 0.02);
    run(&mut w, &d, 20, inp(0, 0, 0));
    assert!(tick(&mut w, &d, inp(127, 0, 0)).contains(&Action::Dash));
    assert_eq!(w.fighters[0].vx, s.run_speed * d.global.gait.dash_speed);
    run(&mut w, &d, d.global.gait.dash_frames as usize, inp(127, 0, 0));
    assert_eq!(w.fighters[0].gait, Gait::Run);
    // Dash dance: flick back during a dash.
    let mut w = mike(&d);
    tick(&mut w, &d, inp(127, 0, 0));
    tick(&mut w, &d, inp(-127, 0, 0));
    assert_eq!((w.fighters[0].gait, w.fighters[0].face), (Gait::Dash, -1.));
}

#[test]
fn mike_reverse_aerial_rush_keeps_momentum_both_ways() {
    let d = common::data();
    for side in [1i8, -1] {
        let fwd = 127 * side;
        let mut w = mike(&d);
        w.fighters[1].x = 2000.;
        w.fighters[0].x = 640. - 200. * f64::from(side);
        run(&mut w, &d, 20, inp(fwd, 0, 0));
        assert_eq!(w.fighters[0].gait, Gait::Run);
        assert!(tick(&mut w, &d, inp(-fwd, 0, 0)).contains(&Action::Turn));
        tick(&mut w, &d, inp(0, 0, JUMP));
        run(&mut w, &d, d.global.movement.jumpsquat as usize, inp(0, 0, JUMP));
        assert_eq!(w.fighters[0].state, State::Air);
        // Drifting forward never flips facing; attacking forward is a back air.
        run(&mut w, &d, 3, inp(fwd, 0, 0));
        tick(&mut w, &d, inp(fwd, 0, ATTACK));
        let f = &w.fighters[0];
        assert_eq!(slot(&w), Some("bair"));
        assert_eq!(f.face, -f64::from(side));
        assert!(f.vx * f64::from(side) > 0., "still carrying the run");
    }
}

#[test]
fn ground_attacks_follow_tilt_flick_and_dash() {
    let d = common::data();
    let start = |pre: &[Input], last: Input| {
        let mut w = mike(&d);
        for i in pre {
            tick(&mut w, &d, *i);
        }
        tick(&mut w, &d, last);
        slot(&w).map(str::to_string)
    };
    assert_eq!(start(&[], inp(127, 0, ATTACK)).as_deref(), Some("fsmash"));
    assert_eq!(start(&[inp(40, 0, 0), inp(80, 0, 0)], inp(80, 0, ATTACK)).as_deref(), Some("ftilt"));
    let running = vec![inp(127, 0, 0); 12];
    assert_eq!(start(&running, inp(127, 0, ATTACK)).as_deref(), Some("dashatk"));
    assert_eq!(
        start(&running[..2], inp(127, 0, ATTACK)).as_deref(),
        Some("dashatk"),
        "attack after a dash starts is a dash attack"
    );
    assert_eq!(start(&[], inp(0, 0, ATTACK)).as_deref(), Some("jab"));
}

#[test]
fn two_jump_controls_short_hop_without_spending_an_air_jump() {
    let d = common::data();
    let s = &d.fighter("mike").unwrap().stats;
    let n = d.global.movement.jumpsquat as usize;
    for (presses, short) in [
        (vec![JUMP], false),
        (vec![JUMP | JUMP2], true),
        (vec![JUMP, JUMP, JUMP | JUMP2], true),
        (vec![JUMP, 0, JUMP], true),
    ] {
        let mut w = mike(&d);
        for b in &presses {
            tick(&mut w, &d, inp(0, 0, *b));
        }
        run(&mut w, &d, n + 1 - presses.len(), inp(0, 0, JUMP));
        let f = &w.fighters[0];
        assert_eq!(f.state, State::Air, "{presses:?}");
        assert_eq!(f.vy < -s.short_hop_speed, !short, "{presses:?}");
        run(&mut w, &d, 20, inp(0, 0, JUMP));
        assert_eq!(w.fighters[0].jumps, s.air_jumps, "{presses:?}: no leftover air jump");
    }
}

#[test]
fn mike_jump_and_attack_on_adjacent_frames_combine_through_landing_and_hitlag() {
    let d = common::data();
    for (a, b) in [(JUMP, ATTACK), (ATTACK, JUMP)] {
        for hitlag in [false, true] {
            let mut w = mike(&d);
            if hitlag {
                w.fighters[0].lag = 1;
            } else {
                w.fighters[0].set_state(State::Landing);
                w.fighters[0].sd.n = 2.;
                tick(&mut w, &d, inp(0, 0, 0));
            }
            tick(&mut w, &d, inp(0, 0, a));
            tick(&mut w, &d, inp(0, 0, a | b));
            assert_eq!(w.fighters[0].state, State::Jumpsquat, "{a} {b} {hitlag}");
            assert_eq!(w.fighters[0].sd.jump_aerial.as_deref(), Some("nair"));
            assert!(w.fighters[0].buffer.cmd.is_none());
        }
    }
}

#[test]
fn shield_routes_up_attack_to_up_smash_and_attack_to_grab() {
    let d = common::data();
    for (stick, buttons, expect) in [
        ((0, 127), ATTACK | SHIELD, "usmash"),
        ((0, 0), ATTACK | SHIELD, "grab"),
        ((0, 0), sim::input::GRAB | SHIELD, "grab"),
    ] {
        let mut w = mike(&d);
        tick(&mut w, &d, inp(0, 0, SHIELD));
        assert_eq!(w.fighters[0].state, State::Shield);
        tick(&mut w, &d, inp(stick.0, stick.1, buttons));
        assert_eq!(slot(&w), Some(expect));
    }
}

#[test]
fn releasing_shield_opens_a_parry_window_for_melee_and_projectiles() {
    let d = common::data();
    let window = d.global.parry.window;
    let setup = |t: u32| {
        let mut w = mike(&d);
        w.fighters[1].x = w.fighters[0].x - 40.;
        w.fighters[1].face = 1.;
        w.fighters[0].set_state(State::Landing);
        w.fighters[0].sd.kind = "shield".into();
        w.fighters[0].sd.n = 7.;
        w.fighters[0].t = t;
        w
    };
    for t in [0, window - 1, window] {
        let mut w = setup(t);
        let mut f = w.fighters[1].clone();
        sim::moves::start_move(&mut w, &d, 1, &mut f, "jab");
        w.fighters[1] = f;
        while combat::active_boxes(&d, &w.fighters[1]).is_empty() {
            w.fighters[1].mov.as_mut().unwrap().t += 1;
        }
        combat::hit_pass(&mut w, &d);
        let parried = t < window;
        assert_eq!(w.fighters[0].dmg == 0., parried, "t {t}");
        if parried {
            assert!(w.acts.iter().any(|a| a.action == Action::Parry));
            assert_eq!(w.fighters[1].lag, d.global.parry.freeze + d.global.parry.attacker_freeze);
            // The rest of a multi-hit move whiffs.
            w.fighters[1].lag = 0;
            w.fighters[1].sd = Default::default();
            combat::hit_pass(&mut w, &d);
            assert_eq!(w.fighters[0].dmg, 0.);
        }
    }
    let mut w = common::fighters(&d, "mike", &["archer"]);
    w.fighters[0].set_state(State::Landing);
    w.fighters[0].sd.kind = "shield".into();
    w.fighters[0].sd.n = 7.;
    let f = w.fighters[1].clone();
    sim::projectiles::spawn(&mut w, &d, 1, &f, "arrow");
    let h = combat::hurt(&d, &w.fighters[0]);
    (w.projs[0].x, w.projs[0].y) = ((h.x1 + h.x2) / 2., (h.y1 + h.y2) / 2.);
    sim::projectiles::step_projectiles(&mut w, &d);
    assert_eq!(w.fighters[0].dmg, 0.);
    assert!(w.acts.iter().any(|a| a.action == Action::Parry));
    // A held shield, released for real, is in the window for exactly `window` ticks.
    let mut w = mike(&d);
    run(&mut w, &d, 10, inp(0, 0, SHIELD));
    let open: Vec<bool> = (0..window + 2)
        .map(|_| {
            tick(&mut w, &d, inp(0, 0, 0));
            combat::parrying(&d, &w.fighters[0])
        })
        .collect();
    assert_eq!(open.iter().filter(|o| **o).count() as u32, window);
}

#[test]
fn di_bends_the_launch_once_at_hitlag_end_and_replays() {
    let d = common::data();
    let hb = d.global.ledge.attack_hit[0].clone();
    let hit = |stick_during: (i8, i8), stick_after: (i8, i8)| {
        let mut w = mike(&d);
        airborne(&mut w);
        let src = combat::HitSource { attacker: 1, direction: 1., point: (600., 400.), projectile: false, label: None };
        combat::apply_hit(&mut w, &d, 0, &hb, src);
        let lag = w.fighters[0].lag;
        assert!(w.fighters[0].di && lag > 0);
        run(&mut w, &d, lag as usize, inp(stick_during.0, stick_during.1, 0));
        let copy: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        let acts = tick(&mut w, &d, inp(stick_after.0, stick_after.1, 0));
        assert!(!w.fighters[0].di);
        (w, copy, acts)
    };
    let (none, ..) = hit((0, 0), (0, 0));
    // Launch is up-right; holding down-right bends it lower, using the latest stick only.
    let (bent, mut copy, acts) = hit((-127, 127), (127, -127));
    assert!(acts.iter().any(|a| matches!(a, Action::Influence(n) if *n != 0)));
    let angle = |f: &sim::world::Fighter| libm::atan2(-f.vy, f.vx);
    assert!(angle(&bent.fighters[0]) < angle(&none.fighters[0]));
    // DI turns the launch without changing its speed; gravity and drift add separately.
    let speed = |f: &sim::world::Fighter| libm::hypot(f.launch.0, f.launch.1);
    assert!(speed(&none.fighters[0]) > 0.);
    assert!((speed(&bent.fighters[0]) - speed(&none.fighters[0])).abs() < 1e-9);
    // Restored snapshot replays identically.
    tick(&mut copy, &d, inp(127, -127, 0));
    assert_eq!(copy.fighters[0], bent.fighters[0]);
    // Parallel stick leaves the launch alone.
    let mut f = none.fighters[0].clone();
    let (vx, vy) = (f.vx, f.vy);
    let mut w = none.clone();
    combat::influence(&mut w, &d, 0, &mut f, (vx, -vy));
    assert!((f.vx - vx).abs() < 1e-9 && (f.vy - vy).abs() < 1e-9);
}

#[test]
fn fast_fall_takes_a_down_press_just_before_the_apex() {
    let d = common::data();
    for (vy, ff) in [(-2., true), (-12., false), (1., true)] {
        let mut w = mike(&d);
        airborne(&mut w);
        w.fighters[0].vy = vy;
        let mut fell = false;
        for _ in 0..12 {
            fell |= tick(&mut w, &d, inp(0, -127, 0)).contains(&Action::FastFall);
        }
        assert_eq!(fell, ff, "vy {vy}");
    }
}

#[test]
fn platforms_drop_on_a_flick_and_crouch_on_a_slow_press() {
    let d = common::data();
    let on_platform = || {
        let mut w = mike(&d);
        let f = &mut w.fighters[0];
        (f.x, f.y, f.surf, f.ground) = (480., 400., Some(d.stages[w.stage].blocks.len()), true);
        w
    };
    let mut w = on_platform();
    for y in [-20, -45, -70, -95, -127] {
        tick(&mut w, &d, inp(0, y, 0));
    }
    assert!(w.fighters[0].ground);
    tick(&mut w, &d, inp(0, -127, ATTACK));
    assert_eq!(slot(&w), Some("dtilt"));
    let mut w = on_platform();
    assert!(tick(&mut w, &d, inp(0, -127, 0)).contains(&Action::PlatformDrop));
    assert!(!w.fighters[0].ground);
    // Solid ground never drops.
    let mut w = mike(&d);
    tick(&mut w, &d, inp(0, -127, 0));
    assert!(w.fighters[0].ground);
}

#[test]
fn platform_drop_does_not_reuse_down_for_fast_fall() {
    let d = common::data();
    for hold in [false, true] {
        let mut w = mike(&d);
        let f = &mut w.fighters[0];
        (f.x, f.y, f.surf, f.ground) = (480., 400., Some(d.stages[w.stage].blocks.len()), true);
        tick(&mut w, &d, inp(0, -127, 0));
        for _ in 0..8 {
            let actions = tick(&mut w, &d, inp(0, if hold { -127 } else { 0 }, 0));
            assert!(!actions.contains(&Action::FastFall));
            assert!(!w.fighters[0].ff);
        }
        tick(&mut w, &d, Input::default());
        assert!(tick(&mut w, &d, inp(0, -127, 0)).contains(&Action::FastFall));
    }
}

#[test]
fn held_down_passes_platforms_only_in_ordinary_fall() {
    let d = common::data();
    for (state, tumble, down, lands) in [
        (State::Air, false, true, false),
        (State::Helpless, false, true, false),
        (State::Air, false, false, true),
        (State::Hitstun, true, true, true),
        (State::Air, true, true, true),
        (State::Attack, false, true, true),
    ] {
        let mut w = mike(&d);
        let mut f = w.fighters[0].clone();
        f.set_state(state);
        (f.x, f.y, f.vy, f.ground, f.surf) = (480., 395., 10., false, None);
        (f.ff, f.in_down, f.tumble) = (true, down, tumble);
        physics::integrate(&mut w, &d, 0, &mut f, false);
        assert_eq!(f.ground, lands, "{state:?} tumble {tumble} down {down}");
    }
    // Hold down before crossing, without refreshing the gesture on the collision frame.
    let mut w = mike(&d);
    airborne(&mut w);
    (w.fighters[0].x, w.fighters[0].y, w.fighters[0].vy) = (480., 350., 1.);
    run(&mut w, &d, 6, inp(0, -127, 0));
    assert!(w.fighters[0].y > 400. && !w.fighters[0].ground && w.fighters[0].ff);
}

#[test]
fn occupied_ledges_trump_and_regrabs_need_a_landing_for_invulnerability() {
    let d = common::data();
    let l = &d.stages[0].ledge;
    let mut w = common::fighters(&d, "mike", &["brawler"]);
    let near = |f: &mut sim::world::Fighter| {
        f.set_state(State::Air);
        (f.ground, f.surf, f.vy, f.x, f.y) = (false, None, 1., 296., 520. + l.hand_y + l.cy);
    };
    let grab = |w: &mut World, i: usize| {
        let mut f = w.fighters[i].clone();
        near(&mut f);
        physics::try_ledge(w, &d, i, &mut f, Default::default(), false);
        w.fighters[i] = f;
    };
    grab(&mut w, 0);
    assert_eq!(w.fighters[0].state, State::Ledge);
    assert_eq!(w.fighters[0].inv, d.global.ledge.invuln);
    // Same frame: blocked, no trade.
    grab(&mut w, 1);
    assert_eq!((w.fighters[0].state, w.fighters[1].state), (State::Ledge, State::Air));
    w.frame += u64::from(d.global.ledge.trump.as_ref().unwrap().min_hang);
    grab(&mut w, 1);
    let (m, b) = (&w.fighters[0], &w.fighters[1]);
    assert_eq!((m.state, b.state), (State::Air, State::Ledge));
    assert!(m.inv == 0 && m.ledge_cd > 0 && m.vx < 0.);
    // Mike regrabs without landing: no invulnerability until he lands or is hit.
    w.fighters[1].set_state(State::Idle);
    w.fighters[0].ledge_cd = 0;
    grab(&mut w, 0);
    assert_eq!((w.fighters[0].state, w.fighters[0].inv), (State::Ledge, 0));
    let mut f = w.fighters[0].clone();
    f.set_state(State::Air);
    physics::on_land(&mut w, &d, 0, &mut f, 0.);
    w.fighters[0] = f;
    grab(&mut w, 0);
    assert_eq!(w.fighters[0].inv, d.global.ledge.invuln);
    // A climbing fighter can't be trumped.
    w.fighters[0].set_state(State::Climb);
    w.fighters[0].sd.side = -1.;
    w.frame += 10;
    grab(&mut w, 1);
    assert_eq!(w.fighters[1].state, State::Air);
}

#[test]
fn stick_started_smash_charges_only_while_the_stick_is_held() {
    let d = common::data();
    let charge = |hold: Input| {
        let mut w = mike(&d);
        tick(&mut w, &d, Input { attack_stick: (127, 0), ..Default::default() });
        assert_eq!(slot(&w), Some("fsmash"));
        run(&mut w, &d, 40, hold);
        w.fighters[0].mov.as_ref().map_or(0, |m| m.charge)
    };
    assert!(charge(Input { attack_stick: (127, 0), ..Default::default() }) > 0);
    assert_eq!(charge(inp(0, 0, ATTACK)), 0, "the attack button doesn't hold a stick smash");
}

#[test]
fn buffered_actions_report_their_press_tick() {
    let d = common::data();
    let mut w = mike(&d);
    w.fighters[0].set_state(State::Landing);
    w.fighters[0].sd.n = 3.;
    tick(&mut w, &d, inp(0, 0, ATTACK));
    let pressed = w.frame;
    let mut found = None;
    for _ in 0..4 {
        common::step_preserving(&mut w, &d, &[Input::default()]);
        found = found.or(w.acts.drain(..).find(|a| a.player == 0));
    }
    let a = found.unwrap();
    assert_eq!(a.action, Action::Move("jab".into()));
    assert_eq!(a.pressed, Some(pressed));
    assert!(a.tick > pressed);
}

#[test]
fn gradual_push_reaches_the_same_run_as_a_flick_and_easing_off_walks() {
    let d = common::data();
    for side in [-1, 1] {
        let mut slow = mike(&d);
        let mut fast = slow.clone();
        slow.fighters[1].x = 2000.;
        fast.fighters[1].x = 2000.;
        for x in [20, 40, 60, 80, 100, 115, 127] {
            run(&mut slow, &d, 4, inp(x * side, 0, 0));
        }
        run(&mut fast, &d, 28, inp(127 * side, 0, 0));
        assert_eq!(slow.fighters[0].gait, Gait::Run);
        assert_eq!(slow.fighters[0].vx, fast.fighters[0].vx);
        run(&mut slow, &d, 20, inp(70 * side, 0, 0));
        assert_eq!(slow.fighters[0].gait, Gait::Walk);
        let expected =
            sim::gait::walk_speed(&d, &slow.fighters[0]) * sim::input::analog(70 * side, d.global.input.deadzone);
        assert!((slow.fighters[0].vx - expected).abs() < 1e-9);
    }
}

#[test]
fn initial_dash_defers_standing_attacks_and_shield_but_allows_dash_cancels() {
    use sim::input::Controls;
    let mut d = common::data();
    d.global.gait.dash_lock = true;
    let dash = || {
        let mut w = mike(&d);
        w.fighters[1].x = 2000.;
        w.cfg.players[0].controls = Controls { stick_tilts: true, ..Default::default() };
        run(&mut w, &d, 7, inp(127, 0, 0));
        assert_eq!(w.fighters[0].gait, Gait::Dash);
        w
    };
    for shield in [false, true] {
        let mut w = dash();
        let raw = if shield {
            inp(127, 0, SHIELD)
        } else {
            Input { stick: (127, 0), attack_stick: (127, 0), ..Default::default() }
        };
        common::step_draining(&mut w, &d, &[raw]);
        let pressed = w.frame;
        assert_eq!(w.fighters[0].gait, Gait::Dash);
        assert!(w.fighters[0].mov.is_none());
        assert!(w.fighters[0].buffer.cmd.is_some());
        let mut restored = w.clone();
        let mut accepted = None;
        for _ in 0..4 {
            common::step_preserving(&mut w, &d, &[inp(127, 0, 0)]);
            common::step_preserving(&mut restored, &d, &[inp(127, 0, 0)]);
            assert_eq!(w, restored);
            accepted = accepted.or(w.acts.drain(..).find(|a| a.player == 0 && a.action.command()));
            restored.acts.clear();
        }
        let accepted = accepted.unwrap();
        assert_eq!(accepted.action, if shield { Action::Shield } else { Action::Move("ftilt".into()) });
        assert_eq!(accepted.pressed, Some(pressed));
        assert!(accepted.tick > pressed);
        assert!(w.fighters[0].buffer.cmd.is_none());
    }
    for (raw, state, expected) in [
        (inp(127, 0, ATTACK), State::Attack, Some("dashatk")),
        (inp(127, 0, JUMP), State::Jumpsquat, None),
        (inp(127, 0, JUMP | ATTACK), State::Jumpsquat, None),
        (Input { stick: (127, 0), attack_stick: (0, 127), ..Default::default() }, State::Attack, Some("usmash")),
    ] {
        let mut w = dash();
        w.cfg.players[0].controls.stick_tilts = false;
        tick(&mut w, &d, raw);
        assert_eq!(w.fighters[0].state, state);
        assert_eq!(slot(&w), expected);
    }
    let mut w = dash();
    tick(&mut w, &d, inp(-127, 0, 0));
    assert_eq!((w.fighters[0].gait, w.fighters[0].face), (Gait::Dash, -1.));
}

#[test]
fn walk_run_thresholds_have_hysteresis_and_early_dash_commands_expire() {
    let mut d = common::data();
    d.global.gait.dash_lock = true;
    let mut w = mike(&d);
    for x in [20, 40, 60, 80, 99] {
        tick(&mut w, &d, inp(x, 0, 0));
    }
    assert_eq!(w.fighters[0].gait, Gait::Walk);
    tick(&mut w, &d, inp(100, 0, 0));
    assert_eq!(w.fighters[0].gait, Gait::Run);
    tick(&mut w, &d, inp(84, 0, 0));
    assert_eq!(w.fighters[0].gait, Gait::Run);
    tick(&mut w, &d, inp(83, 0, 0));
    assert_eq!(w.fighters[0].gait, Gait::Walk);
    tick(&mut w, &d, inp(84, 0, 0));
    assert_eq!(w.fighters[0].gait, Gait::Walk);

    let mut w = mike(&d);
    tick(&mut w, &d, inp(127, 0, 0));
    tick(&mut w, &d, Input { attack_stick: (127, 0), ..Default::default() });
    assert_eq!(w.fighters[0].gait, Gait::Dash);
    run(&mut w, &d, d.global.gait.dash_frames as usize + 1, inp(127, 0, 0));
    assert_eq!(w.fighters[0].gait, Gait::Run);
    assert!(slot(&w).is_none(), "a standing attack pressed outside the buffer window expires");
    assert!(w.fighters[0].buffer.cmd.is_none());
}

#[test]
fn roster_raw_inputs_move_jump_and_shield_without_dash_delay() {
    let d = common::data();
    for fighter in &d.fighters {
        let mut w = common::fighters(&d, &fighter.id, &["brawler"]);
        w.fighters[0].x = 640.;
        w.fighters[1].x = 2000.;
        let x = w.fighters[0].x;
        assert!(tick(&mut w, &d, inp(127, 0, 0)).contains(&Action::Dash), "{}", fighter.id);
        assert!(w.fighters[0].x > x + fighter.stats.run_speed * 0.9, "{}", fighter.id);
        let mut jump = w.clone();
        tick(&mut w, &d, inp(127, 0, SHIELD));
        assert_eq!(w.fighters[0].state, State::Shield, "{}: shield on the next tick", fighter.id);
        tick(&mut jump, &d, inp(127, 0, JUMP));
        assert_eq!(jump.fighters[0].state, State::Jumpsquat, "{}", fighter.id);
        run(&mut jump, &d, 3, inp(127, 0, JUMP));
        assert_eq!(jump.fighters[0].state, State::Air, "{}: airborne within three ticks", fighter.id);
        assert!(jump.fighters[0].vy < 0., "{}", fighter.id);
    }
}

#[test]
fn raw_grab_escape_rewards_repeated_buttons_and_stick_changes_not_holding() {
    let d = common::data();
    let escape = |mode| {
        let mut w = mike(&d);
        combat::start_hold(&mut w, &d, 1, 0);
        for t in 0..600 {
            let input = match mode {
                1 => inp(0, 0, if t % 2 == 0 { JUMP } else { 0 }),
                2 => inp(if t % 2 == 0 { 127 } else { -127 }, 0, 0),
                3 => inp(127, 0, JUMP),
                _ => Input::default(),
            };
            tick(&mut w, &d, input);
            if w.fighters[0].state != State::Grabbed {
                return t;
            }
        }
        panic!("grab never expired")
    };
    let passive = escape(0);
    assert!(escape(1) < passive / 2);
    assert!(escape(2) < passive / 2);
    assert!(escape(3) > passive * 3 / 4);
}
