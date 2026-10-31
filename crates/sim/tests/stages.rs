//! Built-in stage jump routes, ledge recovery and CPU smoke matches.
//! `cargo test -p sim --test stages -- --nocapture` prints per-stage match stats.
mod common;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, step_world, world::State};

fn sandbox(d: &GameData, stage: &str, fighter: &str) -> World {
    let cfg = MatchConfig {
        stage: stage.into(),
        training: true,
        players: vec![PlayerConfig { tap_jump: false, ..PlayerConfig::human(fighter) }],
        ..Default::default()
    };
    let mut w = World::new(cfg, d).unwrap();
    w.count = 0;
    w
}

fn tick(w: &mut World, d: &GameData, input: Input) {
    step_world(w, d, &[input]);
    w.fx.clear();
    w.hits.clear();
}

// One joint motion cycle covers the relative timing of launch and destination platforms.
fn motion_cycle(s: &sim::data::Stage) -> u64 {
    fn gcd(a: u64, b: u64) -> u64 {
        if b == 0 { a } else { gcd(b, a % b) }
    }
    s.platforms.iter().filter_map(|p| p.motion.as_ref()).fold(1, |cycle, m| {
        let n = m.waypoints.len();
        let duration = match m.mode {
            sim::data::stage::PathMode::Loop => {
                m.waypoints.iter().map(|p| u64::from(p.travel_ticks + p.wait_ticks)).sum()
            }
            sim::data::stage::PathMode::PingPong => {
                2 * m.waypoints[..n - 1].iter().map(|p| u64::from(p.travel_ticks)).sum::<u64>()
                    + u64::from(m.waypoints[0].wait_ticks + m.waypoints[n - 1].wait_ticks)
                    + 2 * m.waypoints[1..n - 1].iter().map(|p| u64::from(p.wait_ticks)).sum::<u64>()
            }
        };
        cycle / gcd(cycle, duration) * duration
    })
}

/// A witness is an actual landing after one held ground jump, with no air jumps or specials.
fn jump_to(d: &GameData, start: &World, target: usize) -> Option<World> {
    let mut w = start.clone();
    let mut airborne = false;
    for _ in 0..120 {
        let (l, r, _) = sim::physics::surface(d, &w, target);
        let f = &w.fighters[0];
        let dx = (l + r) / 2. - f.x;
        let stick = if dx.abs() < 4. {
            0
        } else if dx > 0. {
            127
        } else {
            -127
        };
        tick(&mut w, d, Input { stick: (stick, 0), buttons: 1 });
        let f = &w.fighters[0];
        airborne |= !f.ground;
        if matches!(f.state, State::Dead | State::Respawn | State::Ledge) {
            return None;
        }
        if airborne && f.ground {
            return (f.surf == Some(target)).then_some(w);
        }
    }
    None
}

fn reachable_platforms(d: &GameData, stage: &str, fighter: &str) -> Vec<bool> {
    let base = sandbox(d, stage, fighter);
    let s = &d.stages[base.stage];
    let body = &d.fighters[base.fighters[0].data].body;
    let mut launches = std::collections::VecDeque::new();
    for (i, b) in s.blocks.iter().enumerate() {
        for fraction in [0.05, 0.25, 0.5, 0.75, 0.95] {
            let mut w = base.clone();
            let f = &mut w.fighters[0];
            f.x = b.left + (b.right - b.left) * fraction;
            f.y = b.top;
            if s.blocks.iter().any(|b| b.intersects_body(f.x, f.y, body.half_width, body.height)) {
                continue;
            }
            f.set_state(State::Idle);
            f.ground = true;
            f.surf = Some(i);
            launches.push_back(w);
        }
    }
    let mut reached = vec![false; s.platforms.len()];
    let cycle = motion_cycle(s);
    while let Some(mut launch) = launches.pop_front() {
        let source = launch.fighters[0].surf;
        // Release jump and finish landing lag before searching departure times. Waiting in the real
        // world preserves platform carry and the timing of routes built on earlier landings.
        for wait in 0..cycle + 30 {
            if wait >= 30 && (wait - 30) % 15 == 0 {
                for (i, reached) in reached.iter_mut().enumerate() {
                    if !*reached && let Some(w) = jump_to(d, &launch, s.blocks.len() + i) {
                        *reached = true;
                        launches.push_back(w);
                    }
                }
                if reached.iter().all(|r| *r) {
                    return reached;
                }
            }
            tick(&mut launch, d, Input::default());
            if !launch.fighters[0].ground || launch.fighters[0].surf != source {
                break;
            }
        }
    }
    reached
}

#[test]
fn platforms_are_in_full_jump_reach() {
    let d = common::data();
    for s in &d.stages {
        for f in &d.fighters {
            for (i, reached) in reachable_platforms(&d, &s.id, &f.id).into_iter().enumerate() {
                assert!(reached, "{} platforms[{i}]: no full-jump route for {}", s.id, f.id);
            }
        }
    }
}

#[test]
fn reachability_rejects_disconnected_platforms() {
    let mut d = common::data();
    let s = d.stages.iter_mut().find(|s| s.id == "foundry").unwrap();
    s.platforms[0].x1 -= 10_000.;
    s.platforms[0].x2 -= 10_000.;
    // Two nearby platforms cannot establish each other's reachability without a route from a block.
    s.platforms[1].motion = None;
    s.platforms[1].y = -1000.;
    s.platforms[2].motion = None;
    s.platforms[2].y = -1100.;
    assert_eq!(reachable_platforms(&d, "foundry", "brawler"), [false; 3]);
}

#[test]
fn reachability_waits_for_a_moving_platform() {
    let mut d = common::data();
    let s = d.stages.iter_mut().find(|s| s.id == "foundry").unwrap();
    s.platforms.truncate(1);
    let m = s.platforms[0].motion.as_mut().unwrap();
    m.waypoints[0].x = -1000.;
    m.waypoints[1].y = 0.;
    let mut w = sandbox(&d, "foundry", "brawler");
    let f = &mut w.fighters[0];
    f.x = 451.;
    f.y = 520.;
    f.set_state(State::Idle);
    f.ground = true;
    f.surf = Some(0);
    assert!(jump_to(&d, &w, 1).is_none(), "platform should start out of jump range");
    assert_eq!(reachable_platforms(&d, "foundry", "brawler"), [true]);
}

#[test]
fn every_ledge_is_recoverable() {
    let d = common::data();
    for s in &d.stages {
        for fighter in &d.fighters {
            for (block, b) in s.blocks.iter().enumerate() {
                for (side, enabled, edge) in [(-1., b.left_ledge, b.left), (1., b.right_ledge, b.right)] {
                    if !enabled {
                        continue;
                    }
                    let mut w = sandbox(&d, &s.id, &fighter.id);
                    let f = &mut w.fighters[0];
                    f.set_state(State::Air);
                    f.x = edge + side * 40.;
                    f.y = b.top + 220.;
                    f.ground = false;
                    f.surf = None;
                    f.vx = 0.;
                    f.vy = 0.;
                    let mut special = false;
                    let mut recovered = false;
                    for t in 0..240 {
                        // Start before falling can grab the ledge without using up-special.
                        let sp = t > 1 && w.fighters[0].vy >= -2. && !special;
                        special |= sp;
                        let stick = ((-side * 127.) as i8, if sp { 127 } else { 0 });
                        tick(
                            &mut w,
                            &d,
                            Input {
                                stick,
                                buttons: if t == 0 {
                                    1
                                } else if sp {
                                    4
                                } else {
                                    0
                                },
                            },
                        );
                        let f = &w.fighters[0];
                        if f.state == State::Ledge && f.sd.ledge_block == block && f.sd.side == side {
                            recovered = true;
                            break;
                        }
                        if matches!(f.state, State::Dead | State::Respawn) {
                            break;
                        }
                    }
                    assert!(recovered && special, "{} blocks[{block}] side {side}: {} recovery", s.id, fighter.id);
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct Stats {
    ticks: usize,
    kos: usize,
    /// Sum of the victims' percents just before each KO.
    ko_percent: f64,
    self_kos: usize,
    /// Ticks where the living fighters spread wider than the fully zoomed-out camera can frame.
    offscreen: usize,
    widest: f64,
}

/// Runs a CPU match; a KO counts as a self-destruct when the victim took no hit in the previous `recent` ticks.
fn cpu_match(d: &GameData, stage: &str, fighters: &[&str], seed: u32, stocks: i32) -> (Stats, World) {
    let players = fighters.iter().map(|id| PlayerConfig { cpu: Some(2), ..PlayerConfig::human(id) }).collect();
    let cfg = MatchConfig { seed, stage: stage.into(), stocks, players, ..Default::default() };
    let mut w = World::new(cfg, d).unwrap();
    let recent = 240;
    let mut last_hit = vec![None; fighters.len()];
    let mut st = Stats::default();
    let cam = &d.global.render.camera;
    let aspect = 16. / 9.;
    let max_spread = sim::data::camera::zoom_range(&d.stages[w.stage], cam, aspect).max * aspect - 2. * cam.pad_x;
    let mut dmg_before = vec![0.; fighters.len()];
    let mut stocks_before: Vec<i32> = w.fighters.iter().map(|f| f.stocks).collect();
    while !w.over && st.ticks < 30_000 {
        step_world(&mut w, d, &[]);
        st.ticks += 1;
        for h in w.hits.drain(..).filter(|h| !h.shielded) {
            last_hit[h.victim] = Some(st.ticks);
        }
        w.fx.clear();
        let alive = || w.fighters.iter().filter(|f| !matches!(f.state, State::Dead | State::Out)).map(|f| f.x);
        let spread = alive().fold(f64::MIN, f64::max) - alive().fold(f64::MAX, f64::min);
        st.widest = st.widest.max(spread);
        st.offscreen += usize::from(spread > max_spread);
        for (i, f) in w.fighters.iter().enumerate() {
            assert!(f.x.is_finite() && f.y.is_finite(), "{stage}: fighter {i} position");
            if f.stocks < stocks_before[i] {
                st.kos += 1;
                st.ko_percent += dmg_before[i];
                if last_hit[i].is_none_or(|t| st.ticks - t > recent) {
                    st.self_kos += 1;
                }
                stocks_before[i] = f.stocks;
            }
            if f.dmg > 0. {
                dmg_before[i] = f.dmg;
            }
        }
    }
    (st, w)
}

#[test]
fn cpu_matches_finish_on_every_stage() {
    let d = common::data();
    let four = ["brawler", "king", "swordfighter", "brawler"];
    let eight = ["brawler", "king", "swordfighter", "king", "swordfighter", "brawler", "king", "swordfighter"];
    for s in &d.stages {
        let mut total = Stats::default();
        let mut finished = 0;
        for seed in 1..=4 {
            let (st, w) = cpu_match(&d, &s.id, &four, seed, 3);
            finished += usize::from(w.over);
            total.ticks += st.ticks;
            total.kos += st.kos;
            total.ko_percent += st.ko_percent;
            total.self_kos += st.self_kos;
            total.offscreen += st.offscreen;
            total.widest = total.widest.max(st.widest);
        }
        let (big, w8) = cpu_match(&d, &s.id, &eight, 99, 2);
        println!(
            "{:8} 4p×4: finished {finished}/4, avg {:.0} ticks, {} KOs at avg {:.0}%, {} self-KOs, widest {:.0} px, {} offscreen ticks | 8p: {} ticks, {} KOs, {} self-KOs, widest {:.0}, over {}",
            s.id,
            total.ticks as f64 / 4.,
            total.kos,
            total.ko_percent / total.kos.max(1) as f64,
            total.self_kos,
            total.widest,
            total.offscreen,
            big.ticks,
            big.kos,
            big.self_kos,
            big.widest,
            w8.over,
        );
        assert_eq!(finished, 4, "{}: 4-CPU matches should finish", s.id);
        assert!(w8.over, "{}: 8-CPU match should finish", s.id);
        assert!(total.self_kos * 3 <= total.kos, "{}: too many 4p self-destructs {total:?}", s.id);
        assert!(big.self_kos * 3 <= big.kos, "{}: too many 8p self-destructs {big:?}", s.id);
        assert!((total.offscreen + big.offscreen) * 100 <= total.ticks + big.ticks, "{}: fighters off camera", s.id);
        assert!(w8.fighters.iter().any(|f| f.state == State::Win));
    }
}
