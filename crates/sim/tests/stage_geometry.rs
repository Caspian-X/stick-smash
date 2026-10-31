mod common;

use sim::data::stage::{Ground, Motion, PathMode, Platform, Waypoint};
use sim::{GameData, MatchConfig, PlayerConfig, World, input::Intent, physics, step_world, world::State};

fn world(d: &GameData) -> World {
    World::new(MatchConfig { training: true, players: vec![PlayerConfig::human("brawler")], ..Default::default() }, d)
        .unwrap()
}
fn island(left: f64, right: f64, top: f64) -> Ground {
    Ground {
        left,
        right,
        top,
        bottom: top + 60.,
        under_depth: 100.,
        under_inset: 40.,
        left_ledge: true,
        right_ledge: false,
    }
}
fn motion(mode: PathMode) -> Motion {
    Motion {
        mode,
        phase: 0,
        waypoints: vec![
            Waypoint { x: 0., y: 0., travel_ticks: 20, wait_ticks: 5 },
            Waypoint { x: 100., y: -40., travel_ticks: 10, wait_ticks: 3 },
            Waypoint { x: 200., y: 0., travel_ticks: 40, wait_ticks: 2 },
        ],
    }
}
fn integrate(w: &mut World, d: &GameData) {
    let mut f = w.fighters[0].clone();
    physics::integrate(w, d, 0, &mut f, false);
    w.fighters[0] = f;
}
#[test]
fn land_on_second_block_and_walk_off() {
    let mut d = common::data();
    d.stages[0].blocks.push(island(1100., 1300., 400.));
    let mut w = world(&d);
    let f = &mut w.fighters[0];
    f.x = 1200.;
    f.y = 395.;
    f.vy = 10.;
    f.ground = false;
    f.surf = None;
    integrate(&mut w, &d);
    assert_eq!((w.fighters[0].y, w.fighters[0].surf), (400., Some(1)));
    w.fighters[0].vx = 150.;
    w.fighters[0].vy = 1.;
    integrate(&mut w, &d);
    assert!(!w.fighters[0].ground);
    assert_eq!(w.fighters[0].surf, None);
    assert!(w.fighters[0].y > 400.);
}
#[test]
fn wall_stops_grounded_fighter_and_fast_crossing() {
    let mut d = common::data();
    let mut wall = island(700., 800., 300.);
    wall.bottom = 520.;
    d.stages[0].blocks.push(wall);
    let mut w = world(&d);
    let half = d.fighters[0].body.half_width;
    w.fighters[0].x = 680.;
    w.fighters[0].vx = 200.;
    integrate(&mut w, &d);
    assert_eq!(w.fighters[0].x, 700. - half);
    assert_eq!(w.fighters[0].vx, 0.);
}
#[test]
fn secondary_ledge_occupancy_and_disabled_corner() {
    let mut d = common::data();
    d.stages[0].blocks.push(island(1100., 1300., 400.));
    let mut w = world(&d);
    let mut f = w.fighters[0].clone();
    f.ground = false;
    f.surf = None;
    f.vy = 1.;
    f.x = 1090.;
    f.y = 400. + d.stages[0].ledge.hand_y + d.stages[0].ledge.cy;
    // A fighter occupying the same side of a different block must not block the grab.
    let mut other = f.clone();
    other.set_state(State::Ledge);
    other.sd.side = -1.;
    w.fighters.push(other);
    physics::try_ledge(&mut w, &d, 0, &mut f, Intent::default(), false);
    assert_eq!(f.state, State::Ledge);
    assert_eq!(f.sd.ledge_block, 1);
    assert_eq!(f.x, 1100. - d.stages[0].ledge.hang_x);
    w.fighters[1].sd.ledge_block = 1;
    f.set_state(State::Air);
    physics::try_ledge(&mut w, &d, 0, &mut f, Intent::default(), false);
    assert_eq!(f.state, State::Air);
    f.x = 1310.;
    physics::try_ledge(&mut w, &d, 0, &mut f, Intent::default(), false);
    assert_eq!(f.state, State::Air);
}
#[test]
fn waypoint_wait_loop_ping_pong_and_phase() {
    let mut m = motion(PathMode::Loop);
    assert_eq!(m.offset(4.), (0., 0.));
    assert_eq!(m.offset(15.), (50., -20.));
    assert_eq!(m.offset(27.), (100., -40.));
    assert_eq!(m.offset(33.), (150., -20.));
    assert_eq!(m.offset(60.), (100., 0.));
    assert_eq!(m.offset(80.), (0., 0.));
    m.mode = PathMode::PingPong;
    assert_eq!(m.offset(45.), (150., -20.));
    assert_eq!(m.offset(63.), (50., -20.));
    assert_eq!(m.offset(73.), (0., 0.));
    m.phase = 15;
    assert_eq!(m.offset(0.), (50., -20.));
}
#[test]
fn ride_moving_platform_and_replay_snapshot() {
    let mut d = common::data();
    let p = Platform { x1: 400., x2: 600., y: 300., motion: Some(motion(PathMode::PingPong)) };
    d.stages[0].platforms = vec![p];
    d.stages[0].spawns[0].x = 500.;
    d.stages[0].spawns[0].y = Some(300.);
    let mut w = world(&d);
    for _ in 0..15 {
        step_world(&mut w, &d, &[]);
    }
    assert_eq!((w.fighters[0].x, w.fighters[0].y), (550., 280.));
    assert_eq!(w.fighters[0].surf, Some(1));
    let mut rollback = w.clone();
    for _ in 0..180 {
        step_world(&mut w, &d, &[]);
        step_world(&mut rollback, &d, &[]);
        assert_eq!(w, rollback);
        let (x1, _, y) = d.stages[0].platforms[0].position(w.frame as f64);
        assert_eq!((w.fighters[0].x, w.fighters[0].y), (x1 + 100., y));
    }
}
#[test]
fn rising_platform_lands_and_drop_ignores_it() {
    let mut d = common::data();
    d.stages[0].platforms = vec![Platform { x1: 400., x2: 600., y: 300., motion: Some(motion(PathMode::Loop)) }];
    let mut w = world(&d);
    w.frame = 15;
    let f = &mut w.fighters[0];
    f.x = 500.;
    f.y = 281.;
    f.vy = 0.;
    f.ground = false;
    f.surf = None;
    integrate(&mut w, &d);
    assert_eq!(w.fighters[0].surf, Some(1));
    w.fighters[0].drop = 10;
    w.fighters[0].vy = 2.;
    integrate(&mut w, &d);
    assert!(!w.fighters[0].ground);
    assert_eq!(w.fighters[0].y, 282.);
}
#[test]
fn corridor_walls_and_underside() {
    let mut d = common::data();
    d.stages[0].blocks = vec![island(0., 200., 100.), island(300., 500., 100.)];
    let mut w = world(&d);
    let half = d.fighters[0].body.half_width;
    let height = d.fighters[0].body.height;
    let f = &mut w.fighters[0];
    f.ground = false;
    f.surf = None;
    f.x = 250.;
    f.y = 150.;
    f.vx = 100.;
    f.vy = 0.;
    integrate(&mut w, &d);
    assert_eq!(w.fighters[0].x, 300. - half);
    w.fighters[0].vx = -100.;
    integrate(&mut w, &d);
    assert_eq!(w.fighters[0].x, 200. + half);
    let f = &mut w.fighters[0];
    f.x = 100.;
    f.y = 260. + height + 2.;
    f.vx = 0.;
    f.vy = -8.;
    integrate(&mut w, &d);
    assert_eq!(w.fighters[0].y, 260. + height);
    assert_eq!(w.fighters[0].vy, 0.);
    // Near the underside corner the taper ejects the fighter outward.
    let f = &mut w.fighters[0];
    f.x = 15.;
    f.y = 190. + height;
    f.vy = -5.;
    integrate(&mut w, &d);
    assert!(w.fighters[0].x < 15.);
}

#[test]
fn secondary_ledge_climb_keeps_its_block() {
    let mut d = common::data();
    d.stages[0].blocks.push(island(1100., 1300., 400.));
    let mut w = world(&d);
    let mut f = w.fighters[0].clone();
    f.ground = false;
    f.surf = None;
    f.vy = 1.;
    f.x = 1090.;
    f.y = 400. + d.stages[0].ledge.hand_y + d.stages[0].ledge.cy;
    physics::try_ledge(&mut w, &d, 0, &mut f, Intent::default(), false);
    f.t = d.global.ledge.min_hang;
    w.fighters[0] = f;
    sim::states::update_fighter(&mut w, &d, 0, Intent { up: true, ..Default::default() });
    assert_eq!(w.fighters[0].state, State::Climb);
    assert_eq!(w.fighters[0].sd.ledge_block, 1);
    for _ in 0..d.global.ledge.climb.end {
        sim::states::update_fighter(&mut w, &d, 0, Intent::default());
    }
    assert_eq!(w.fighters[0].surf, Some(1));
    assert_eq!(w.fighters[0].y, 400.);
    assert!(w.fighters[0].x > 1100. && w.fighters[0].x < 1300.);
}

#[test]
fn underside_stops_a_fast_rising_fighter() {
    let mut d = common::data();
    d.stages[0].blocks = vec![island(300., 980., 400.)];
    let mut w = world(&d);
    let height = d.fighters[0].body.height;
    let f = &mut w.fighters[0];
    f.x = 640.;
    f.y = 570. + height;
    f.vy = -500.;
    f.ground = false;
    f.surf = None;
    integrate(&mut w, &d);
    assert_eq!(w.fighters[0].y, 560. + height);
    assert_eq!(w.fighters[0].vy, 0.);
    assert!(!w.fighters[0].ground);
}

#[test]
fn platform_carry_cannot_push_through_a_wall_during_hitlag() {
    let mut d = common::data();
    let mut wall = island(700., 800., 100.);
    wall.bottom = 400.;
    d.stages[0].blocks.push(wall);
    let path = Motion {
        mode: PathMode::PingPong,
        phase: 0,
        waypoints: vec![
            Waypoint { x: 0., y: 0., travel_ticks: 1, wait_ticks: 0 },
            Waypoint { x: 400., y: 0., travel_ticks: 1, wait_ticks: 0 },
        ],
    };
    d.stages[0].platforms = vec![Platform { x1: 400., x2: 600., y: 300., motion: Some(path) }];
    d.stages[0].spawns[0].x = 500.;
    d.stages[0].spawns[0].y = Some(300.);
    let mut w = world(&d);
    w.fighters[0].lag = 3;
    step_world(&mut w, &d, &[]);
    assert_eq!(w.fighters[0].x, 700. - d.fighters[0].body.half_width);
    assert_eq!(w.fighters[0].surf, None);
    let x = w.fighters[0].x;
    step_world(&mut w, &d, &[]);
    assert_eq!(w.fighters[0].x, x);
}

#[test]
fn secondary_ledge_jump_uses_secondary_top() {
    let mut d = common::data();
    d.stages[0].blocks.push(island(1100., 1300., 400.));
    let mut w = world(&d);
    let f = &mut w.fighters[0];
    f.set_state(State::Climb);
    f.ground = false;
    f.surf = None;
    f.sd.ledge_block = 1;
    f.sd.side = -1.;
    f.sd.kind = "jump".into();
    f.t = d.global.ledge.jump.a;
    sim::states::update_fighter(&mut w, &d, 0, Intent::default());
    assert_eq!(w.fighters[0].state, State::Air);
    assert_eq!(w.fighters[0].y, 400. - d.global.sim.ledge_jump_offset);
}

#[test]
fn cpu_recovers_toward_nearest_secondary_block() {
    let mut d = common::data();
    d.stages[0].blocks.push(island(1100., 1300., 400.));
    let mut w = world(&d);
    let f = &mut w.fighters[0];
    f.set_state(State::Helpless);
    f.x = 1050.;
    f.y = 450.;
    f.ground = false;
    f.surf = None;
    let intent = sim::ai::ai_input(&mut w, &d, 0, 2);
    assert!(intent.right && !intent.left);
}

#[test]
fn rising_at_seams_and_body_width_gaps_cannot_enter_solids() {
    for gap in [0., 8., 20.] {
        for reverse in [false, true] {
            let mut d = common::data();
            d.stages[0].blocks = vec![island(300., 640., 520.), island(640. + gap, 980., 520.)];
            if reverse {
                d.stages[0].blocks.reverse();
            }
            let mut w = world(&d);
            let body = &d.fighters[0].body;
            let f = &mut w.fighters[0];
            f.x = 640. + gap / 2.;
            f.y = 700. + body.height;
            f.ground = false;
            f.surf = None;
            for _ in 0..60 {
                w.fighters[0].vy = -14.;
                integrate(&mut w, &d);
                let f = &w.fighters[0];
                assert!(f.y > 580. + body.height, "gap={gap}, reverse={reverse}, {f:?}");
                assert!(d.stages[0].blocks.iter().all(|b| !b.intersects_body(f.x, f.y, body.half_width, body.height)));
            }
        }
    }
}

#[test]
fn arena_corner_contact_keeps_legacy_side_exit() {
    let d = common::data();
    let mut w = world(&d);
    let f = &mut w.fighters[0];
    f.x = 980. + d.fighters[0].body.half_width - 4.;
    f.y = 513.;
    f.vy = 12.;
    f.ground = false;
    f.surf = None;
    integrate(&mut w, &d);
    let f = &w.fighters[0];
    assert_eq!((f.x, f.y, f.vy), (980. + d.fighters[0].body.half_width, 525., 12.));
    assert!(!f.ground);
}

#[test]
fn ping_pong_uses_forward_leg_duration_in_both_directions() {
    let mut m = motion(PathMode::PingPong);
    m.waypoints[2].travel_ticks = 0;
    assert_eq!(m.offset(33.), m.offset(45.));
    assert_eq!(m.offset(15.), m.offset(63.));
    assert_eq!(m.offset(73.), (0., 0.));
}

#[test]
fn cpu_crosses_to_an_opponents_island() {
    let mut d = common::data();
    d.stages[0].blocks = vec![island(300., 600., 520.), island(660., 980., 520.)];
    d.stages[0].platforms.clear();
    d.stages[0].spawns[0].x = 500.;
    d.stages[0].spawns[1].x = 850.;
    let mut w = World::new(
        MatchConfig {
            training: true,
            players: vec![
                PlayerConfig { cpu: Some(2), ..PlayerConfig::human("brawler") },
                PlayerConfig::human("brawler"),
            ],
            ..Default::default()
        },
        &d,
    )
    .unwrap();
    let mut arrived = false;
    for _ in 0..600 {
        step_world(&mut w, &d, &[]);
        if w.fighters[0].surf == Some(1) && w.fighters[0].x > 700. {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "{:?}", w.fighters[0]);
}
