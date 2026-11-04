//! Built-in stage jump routes, ledge recovery and CPU smoke matches.
//! `cargo test -p sim --test stages -- --nocapture` prints per-stage match stats.
mod common;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, step_world, world::State};

fn sandbox(d: &GameData, stage: &str, fighter: &str) -> World {
    let mut w = common::sandbox(d, stage, vec![PlayerConfig { tap_jump: false, ..PlayerConfig::human(fighter) }]);
    w.count = 0;
    w
}

fn tick(w: &mut World, d: &GameData, input: Input) {
    common::step_draining(w, d, &[input]);
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
        tick(&mut w, d, Input { stick: (stick, 0), buttons: 1, ..Default::default() });
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
                        let aiming = fighter.moves.uspec.phases.iter().any(|p| p.travel_speed.is_some())
                            && w.fighters[0].mov.as_ref().is_some_and(|m| m.slot == "uspec" && m.phase == 0);
                        let stick = ((-side * 127.) as i8, if sp || aiming { 127 } else { 0 });
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
                                ..Default::default()
                            },
                        );
                        let f = &w.fighters[0];
                        if f.state == State::Ledge && f.sd.ledge_block == block && f.sd.side == side
                            || special && f.ground && f.x >= b.left && f.x <= b.right && f.y <= b.top
                        {
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

#[test]
fn new_fighter_cpu_matchups() {
    let d = common::data();
    for stage in &d.stages {
        for fighter in ["golem", "lancer", "dancer", "plant", "throbbin"] {
            let mut wins = 0;
            let mut kos = 0;
            let mut self_kos = 0;
            for opponent in ["brawler", "king", "swordfighter"] {
                for seed in 1..=4 {
                    let ids = if seed % 2 == 0 { [opponent, fighter] } else { [fighter, opponent] };
                    let (st, w) = cpu_match(&d, &stage.id, &ids, seed, 2);
                    assert!(
                        w.over,
                        "{}: {fighter} vs {opponent} seed {seed} unfinished: {:?}",
                        stage.id,
                        w.fighters.iter().map(|f| (f.x, f.y, f.dmg, f.stocks, f.state)).collect::<Vec<_>>()
                    );
                    wins += usize::from(w.fighters[usize::from(seed % 2 == 0)].state == State::Win);
                    kos += st.kos;
                    self_kos += st.self_kos;
                }
            }
            println!("{} {fighter}: {wins}/12 wins, {kos} KOs, {self_kos} self-KOs", stage.id);
            assert!(self_kos * 3 <= kos, "{} {fighter}: too many self-destructs", stage.id);
        }
    }
}

#[derive(Default, serde::Serialize)]
struct MoveStats {
    starts: usize,
    hits: usize,
    tips: usize,
    blocked: usize,
    whiffs: usize,
    punished_whiffs: usize,
    punish_starts: usize,
    punish_hits: usize,
    damage: f64,
    kos: usize,
    start_distance: f64,
}

#[derive(Default, serde::Serialize)]
struct FighterStats {
    wins: usize,
    matches: usize,
    unfinished: usize,
    dealt: f64,
    taken: f64,
    deaths: usize,
    self_kos: usize,
    recovery_deaths: usize,
    moves: std::collections::BTreeMap<String, MoveStats>,
}

#[derive(Default)]
struct Attempt {
    slot: String,
    hit: bool,
    punished: bool,
    punish: bool,
}

fn diagnose_match(d: &GameData, stage: &str, ids: [&str; 2], seed: u32, stats: &mut FighterStats, own: usize) {
    let mut w = World::new(
        MatchConfig {
            stage: stage.into(),
            seed,
            stocks: 2,
            players: ids.iter().map(|id| PlayerConfig { cpu: Some(2), ..PlayerConfig::human(id) }).collect(),
            ..Default::default()
        },
        d,
    )
    .unwrap();
    let enemy = 1 - own;
    let mut attempt: Option<Attempt> = None;
    let mut last_hit = [None; 2];
    let mut last_move = [String::new(), String::new()];
    let mut recovery = None;
    for tick in 0..30_000 {
        let before = w.fighters.clone();
        step_world(&mut w, d, &[]);
        let f = &w.fighters[own];
        let prev = &before[own];
        let started = f.mov.as_ref().is_some_and(|m| {
            f.state == State::Attack
                && (prev.state != State::Attack || prev.mov.as_ref().is_none_or(|p| m.elapsed < p.elapsed))
        });
        let ended = attempt
            .as_ref()
            .is_some_and(|a| f.state != State::Attack || f.mov.as_ref().is_none_or(|m| m.slot != a.slot) || started);
        if ended && let Some(a) = attempt.take() {
            let m = stats.moves.entry(a.slot).or_default();
            let contact = a.hit || f.state == State::Hold && f.sd.victim == Some(enemy);
            m.whiffs += usize::from(!contact);
            m.punished_whiffs +=
                usize::from(!contact && (a.punished || w.hits.iter().any(|h| h.victim == own && !h.shielded)));
        }
        if started {
            let slot = f.mov.as_ref().unwrap().slot.clone();
            let o = &before[enemy];
            let punish = o.state == State::Attack
                && o.mov.as_ref().is_some_and(|m| {
                    let p = sim::moves::phase(d, o);
                    !p.hitboxes.is_empty() && p.hitboxes.iter().all(|h| h.end.is_some_and(|end| m.t > end))
                });
            let m = stats.moves.entry(slot.clone()).or_default();
            m.starts += 1;
            m.start_distance += (f.x - w.fighters[enemy].x).abs();
            m.punish_starts += usize::from(punish);
            attempt = Some(Attempt { slot, punish, ..Default::default() });
        }
        let offstage = !d.stages[w.stage]
            .blocks
            .iter()
            .any(|b| f.x >= b.left && f.x <= b.right && f.y <= b.top + d.global.ai.thresholds.offstage_pad);
        if f.ground || f.state == State::Ledge {
            recovery = None;
        } else if offstage
            && (f.state == State::Helpless
                || f.state == State::Attack && f.mov.as_ref().is_some_and(|m| m.slot == "uspec"))
        {
            recovery = Some(tick);
        }
        for h in &w.hits {
            if h.shielded {
                if h.attacker == Some(own)
                    && let Some(a) = &mut attempt
                {
                    a.hit = true;
                    stats.moves.entry(a.slot.clone()).or_default().blocked += 1;
                }
                continue;
            }
            last_hit[h.victim] = Some(tick);
            let Some(a) = h.attacker else { continue };
            let label = w
                .trace
                .as_ref()
                .filter(|t| t.hit_frame == w.frame && t.attacker == a && t.defender == h.victim)
                .map(|t| t.label.as_str());
            let fighter = &d.fighters[w.fighters[a].data];
            let slot = label
                .and_then(|name| fighter.moves.iter().find(|(_, m)| m.name == name).map(|(s, _)| s.to_string()))
                .or_else(|| before[a].mov.as_ref().map(|m| m.slot.clone()))
                .unwrap_or_else(|| {
                    if before[a].state == State::Hold {
                        "Pummel".into()
                    } else {
                        sim::combat::attack_label(d, &before[a])
                    }
                });
            last_move[h.victim] = slot.clone();
            if a == own {
                stats.dealt += h.damage;
                let m = stats.moves.entry(slot.clone()).or_default();
                m.hits += 1;
                m.damage += h.damage;
                // Tip damage is distinct from shaft/handle damage; include smash charge scaling.
                if let Some((_, def)) = fighter.moves.iter().find(|(s, _)| *s == slot) {
                    let charge = w.fighters[a].mov.as_ref().or(before[a].mov.as_ref()).map_or(0, |m| m.charge);
                    let mult = 1. + f64::from(charge) / d.global.knockback.charge_damage_divisor;
                    m.tips += usize::from(
                        slot != "uspec"
                            && slot != "dspec"
                            && def
                                .phases
                                .iter()
                                .any(|p| p.hitboxes.len() > 1 && (p.hitboxes[0].damage * mult - h.damage).abs() < 1e-6),
                    );
                }
                if let Some(a) = &mut attempt {
                    a.hit = true;
                    m.punish_hits += usize::from(a.punish);
                }
            }
            if h.victim == own {
                stats.taken += h.damage;
                if let Some(a) = &mut attempt {
                    a.punished = true;
                }
            }
        }
        for (i, old) in before.iter().enumerate() {
            if w.fighters[i].stocks >= old.stocks {
                continue;
            }
            if i == own {
                stats.deaths += 1;
                stats.self_kos += usize::from(last_hit[i].is_none_or(|t| tick - t > 240));
                stats.recovery_deaths += usize::from(recovery.is_some_and(|t| tick - t < 240));
                recovery = None;
            } else if last_hit[i].is_some_and(|t| tick - t <= 240) {
                stats.moves.entry(last_move[i].clone()).or_default().kos += 1;
            }
        }
        w.hits.clear();
        w.fx.clear();
        if w.over {
            break;
        }
    }
    if !w.over {
        println!(
            "TIMEOUT {stage} {ids:?} seed {seed}: {:?}",
            w.fighters.iter().map(|f| (f.x, f.y, f.dmg)).collect::<Vec<_>>()
        );
        stats.unfinished += 1;
    }
    assert!(w.over, "{stage} {ids:?} seed {seed}: balance duel did not finish");
    stats.matches += 1;
    stats.wins += usize::from(w.fighters[own].state == State::Win);
}

/// Opt-in detailed balance log; `just balance 16` repeats the same seat-swapped seed set per matchup.
#[test]
#[ignore]
fn fighter_balance_diagnostics() {
    let d = common::data();
    let seeds: u32 = std::env::var("BALANCE_SEEDS").ok().map_or(4, |v| v.parse().unwrap());
    assert!(seeds > 0);
    let only = std::env::var("BALANCE_FIGHTER").unwrap_or_default();
    for id in d.fighters.iter().map(|f| f.id.as_str()).filter(|id| only.is_empty() || only == *id) {
        let mut stats = FighterStats::default();
        for stage in &d.stages {
            for opponent in d.fighters.iter().map(|f| f.id.as_str()).filter(|other| *other != id) {
                for seed in 1..=seeds {
                    let own = usize::from(seed % 2 == 0);
                    let ids = if own == 0 { [id, opponent] } else { [opponent, id] };
                    diagnose_match(&d, &stage.id, ids, seed, &mut stats, own);
                }
            }
        }
        println!("BALANCE {id} {}", serde_json::to_string(&stats).unwrap());
    }
}

#[test]
fn roster_cpu_matchups() {
    let d = common::data();
    let roster = ["brawler", "king", "ninja", "swordfighter", "wrangler", "yoyo"];
    for id in roster {
        let (mut total_wins, mut total_finished) = (0, 0);
        for s in &d.stages {
            let (mut wins, mut kos, mut self_kos, mut finished) = (0, 0, 0, 0);
            for opponent in roster.into_iter().filter(|opponent| *opponent != id) {
                for seed in 1..=4 {
                    let pair = if seed % 2 == 0 { [opponent, id] } else { [id, opponent] };
                    let own = usize::from(seed % 2 == 0);
                    let (st, w) = cpu_match(&d, &s.id, &pair, seed, 3);
                    if !w.over {
                        println!(
                            "unfinished {id} vs {opponent} {} seed {seed}: {:?}",
                            s.id,
                            w.fighters.iter().map(|f| (f.x, f.y, f.stocks, f.dmg, f.state)).collect::<Vec<_>>()
                        );
                    }
                    finished += usize::from(w.over);
                    wins += usize::from(w.fighters[own].state == State::Win);
                    kos += st.kos;
                    self_kos += st.self_kos;
                }
            }
            println!("{id:8} {:8}: {wins}/20 wins, {finished}/20 finished, {kos} KOs, {self_kos} self-KOs", s.id);
            total_wins += wins;
            total_finished += finished;
            assert!(self_kos * 3 <= kos, "{id} on {}: excessive self-KOs", s.id);
        }
        println!("{id:8} total: {total_wins}/80 wins, {total_finished}/80 finished");
        if matches!(id, "ninja" | "yoyo" | "wrangler") {
            assert!((20..=60).contains(&total_wins), "{id}: {total_wins}/80 wins is outside the balance band");
        }
        assert!(total_finished == 80, "{id}: only {total_finished}/80 duels finished");
    }
}

#[test]
fn ranged_roster_cpu_matchups() {
    let d = common::data();
    for id in ["archer", "wizard", "tinker"] {
        let mut wins = 0;
        let mut matches = 0;
        let mut kos = 0;
        let mut self_kos = 0;
        let mut own_losses = 0;
        let mut draws = 0;
        for s in &d.stages {
            for opponent in ["brawler", "king", "swordfighter"] {
                let mut pair_wins = 0;
                for seed in 1..=4 {
                    let slot = (seed % 2) as usize;
                    let fighters = if slot == 0 { [id, opponent] } else { [opponent, id] };
                    let (st, w) = cpu_match(&d, &s.id, &fighters, seed, 3);
                    assert!(
                        w.over,
                        "{} {id} vs {opponent} seed {seed}: duel reached tick cap {:?}",
                        s.id,
                        w.fighters.iter().map(|f| (f.x, f.y, f.surf, f.state, &f.ai)).collect::<Vec<_>>()
                    );
                    draws += usize::from(!w.over);
                    let won = usize::from(w.fighters[slot].state == State::Win);
                    pair_wins += won;
                    wins += won;
                    matches += 1;
                    kos += st.kos;
                    self_kos += st.self_kos;
                    own_losses += (3 - w.fighters[slot].stocks) as usize;
                }
                println!("{id:6} {:8} vs {opponent:12}: {pair_wins}/4 wins", s.id);
            }
        }
        println!(
            "{id}: {wins}/{matches} wins, {draws} draws, {kos} total KOs, {self_kos} total self-KOs, {own_losses} own stock losses"
        );
        assert!(self_kos * 5 <= kos, "{id}: too many matchup self-destructs");
    }
}

#[test]
fn ranged_roster_four_player_matches_finish() {
    let d = common::data();
    for s in &d.stages {
        let mut kos = 0;
        let mut self_kos = 0;
        let mut wins = [0; 4];
        for seed in 1..=4 {
            let (st, w) = cpu_match(&d, &s.id, &["archer", "wizard", "tinker", "brawler"], seed, 3);
            assert!(w.over, "{}: ranged roster seed {seed} timed out", s.id);
            for (i, f) in w.fighters.iter().enumerate() {
                wins[i] += usize::from(f.state == State::Win);
            }
            kos += st.kos;
            self_kos += st.self_kos;
        }
        println!("{} ranged 4p: winners archer/wizard/tinker/brawler {wins:?}, {kos} KOs, {self_kos} self-KOs", s.id);
        assert!(self_kos * 5 <= kos, "{}: too many ranged roster self-destructs", s.id);
    }
}

#[test]
fn isles_duels_leave_separate_platforms_and_finish() {
    let d = common::data();
    for seed in 1..=4 {
        let players = ["brawler", "king"].map(|id| PlayerConfig { cpu: Some(2), ..PlayerConfig::human(id) }).to_vec();
        let mut w = World::new(MatchConfig { stage: "isles".into(), players, seed, ..Default::default() }, &d).unwrap();
        w.count = 0;
        let blocks = d.stages[w.stage].blocks.len();
        for (i, f) in w.fighters.iter_mut().enumerate() {
            let surface = blocks + i;
            let (x1, x2, y) = d.stages[w.stage].platforms[i].position(0.);
            f.x = (x1 + x2) / 2.;
            f.y = y;
            f.ground = true;
            f.surf = Some(surface);
            f.set_state(State::Idle);
            f.inv = 0;
            f.blink = 0;
        }
        let mut crossed = false;
        for tick in 0..60_000 {
            step_world(&mut w, &d, &[]);
            w.fx.clear();
            w.hits.clear();
            crossed |= w.fighters.iter().enumerate().any(|(i, f)| f.surf.is_some_and(|id| id != blocks + i));
            if w.over {
                break;
            }
            assert!(
                tick + 1 < 60_000,
                "Isles separated Brawler/King seed {seed} stalled {:?}",
                w.fighters.iter().map(|f| (f.x, f.y, f.surf, f.state, &f.ai)).collect::<Vec<_>>()
            );
        }
        assert!(crossed);
    }
}

#[test]
fn blocked_surface_pursuit_waits_for_timeout_then_jumps_at_the_edge() {
    let mut d = common::data();
    d.global.ai.thresholds.surface_chase_ticks = 3;
    d.global.ai.aggression = [0.; 3];
    let mut w = World::new(
        MatchConfig::sandbox(
            "isles",
            vec![PlayerConfig::human("brawler"), PlayerConfig::human("king")],
            MatchConfig::default().seed,
        ),
        &d,
    )
    .unwrap();
    let blocks = d.stages[w.stage].blocks.len();
    for (i, f) in w.fighters.iter_mut().enumerate() {
        f.x = if i == 0 { 400. } else { 880. };
        f.y = 430.;
        f.ground = true;
        f.surf = Some(blocks + i);
        f.ai.pursuit_x = f.x;
    }
    for _ in 0..2 {
        sim::ai::ai_input(&mut w, &d, 0, 2);
        assert!(!w.fighters[0].ai.pursuit);
    }
    let mut replay = w.clone();
    assert_eq!(sim::ai::ai_input(&mut w, &d, 0, 2), sim::ai::ai_input(&mut replay, &d, 0, 2));
    assert_eq!(w, replay);
    assert!(w.fighters[0].ai.pursuit);
    w.fighters[0].x = 480.;
    let inp = sim::ai::ai_input(&mut w, &d, 0, 2);
    assert!(inp.right && inp.jump && inp.jump_held);
    w.fighters[0].surf = w.fighters[1].surf;
    sim::ai::ai_input(&mut w, &d, 0, 2);
    assert!(!w.fighters[0].ai.pursuit);
}

#[test]
fn mike_cpu_matchups_finish_on_every_stage() {
    let d = common::data();
    for s in &d.stages {
        let (mut finished, mut wins, mut kos, mut self_kos) = (0, 0, 0, 0);
        for opponent in d.fighters.iter().filter(|f| f.id != "mike") {
            for seed in 1..=2 {
                let own = usize::from(seed == 2);
                let pair = if own == 0 { ["mike", opponent.id.as_str()] } else { [opponent.id.as_str(), "mike"] };
                let (st, w) = cpu_match(&d, &s.id, &pair, seed, 3);
                assert!(w.over, "Mike vs {} on {} seed {seed} timed out", opponent.id, s.id);
                finished += 1;
                wins += usize::from(w.fighters[own].state == State::Win);
                kos += st.kos;
                self_kos += st.self_kos;
            }
        }
        println!("Mike on {}: {wins}/{finished} wins, {kos} KOs, {self_kos} self-KOs", s.id);
        assert!(self_kos * 3 <= kos, "Mike matchups on {}: excessive self-KOs", s.id);
    }
}

#[test]
fn whiplash_cpu_matches_finish_on_every_stage_in_both_seats() {
    let d = common::data();
    for s in &d.stages {
        let (mut kos, mut self_kos) = (0, 0);
        for opponent in ["brawler", "mike", "wizard"] {
            for own in 0..2 {
                let pair = if own == 0 { ["whiplash", opponent] } else { [opponent, "whiplash"] };
                let (st, w) = cpu_match(&d, &s.id, &pair, 17 + own as u32, 3);
                assert!(w.over, "Whiplash vs {opponent} on {} seat {own}: timed out", s.id);
                kos += st.kos;
                self_kos += st.self_kos;
            }
        }
        assert!(self_kos * 3 <= kos, "Whiplash on {}: {self_kos}/{kos} self-KOs", s.id);
    }
}
