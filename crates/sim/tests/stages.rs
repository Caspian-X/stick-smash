//! Layout sanity for every built-in stage: platforms are in full-jump reach and CPU matches play out.
//! `cargo test -p sim --test stages -- --nocapture` prints per-stage match stats.
mod common;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, step_world, world::State};

/// Highest the feet rise on a held full jump from flat ground (no air jumps).
fn full_jump_height(d: &GameData, id: &str) -> f64 {
    let cfg = MatchConfig { training: true, players: vec![PlayerConfig::human(id)], ..Default::default() };
    let mut w = World::new(cfg, d).unwrap();
    w.count = 0;
    for _ in 0..30 {
        step_world(&mut w, d, &[]);
    }
    let floor = w.fighters[0].y;
    let mut peak = floor;
    for t in 0..120 {
        step_world(&mut w, d, &[Input { stick: (0, 0), buttons: if t < 30 { 1 } else { 0 } }]);
        peak = peak.min(w.fighters[0].y);
    }
    floor - peak
}

#[test]
fn platforms_are_in_full_jump_reach() {
    let d = common::data();
    let reach = d.fighters.iter().map(|f| full_jump_height(&d, &f.id)).fold(f64::INFINITY, f64::min);
    for s in &d.stages {
        for (i, p) in s.platforms.iter().enumerate() {
            // Highest point on the path and the solid ground below any of its positions.
            let offsets =
                p.motion.as_ref().map_or(vec![(0., 0.)], |m| m.waypoints.iter().map(|w| (w.x, w.y)).collect());
            let top = offsets.iter().map(|o| p.y + o.1).fold(f64::INFINITY, f64::min);
            // Nearest static surface (block or stationary platform) under any position of the path.
            let surfaces = s
                .blocks
                .iter()
                .map(|b| (b.left, b.right, b.top))
                .chain(s.platforms.iter().filter(|q| q.motion.is_none()).map(|q| (q.x1, q.x2, q.y)));
            let surfaces: Vec<_> = surfaces.collect();
            let below = offsets
                .iter()
                .flat_map(|&(ox, oy)| {
                    surfaces.iter().filter(move |&&(l, r, y)| l < p.x2 + ox && r > p.x1 + ox && y > p.y + oy)
                })
                .map(|&(_, _, y)| y)
                .fold(f64::INFINITY, f64::min);
            if below.is_finite() {
                assert!(
                    below - top <= reach - 8.,
                    "{} platforms[{i}] is {} above ground, jump {reach}",
                    s.id,
                    below - top
                );
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
    let max_spread = cam.max_height * 16. / 9. - 2. * cam.pad_x;
    let mut dmg_before = vec![0.; fighters.len()];
    let mut stocks_before: Vec<i32> = w.fighters.iter().map(|f| f.stocks).collect();
    while !w.over && st.ticks < 30_000 {
        step_world(&mut w, d, &[]);
        st.ticks += 1;
        for h in w.hits.drain(..) {
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
        assert!(total.self_kos * 3 <= total.kos, "{}: too many self-destructs {total:?}", s.id);
        assert!((total.offscreen + big.offscreen) * 100 <= total.ticks + big.ticks, "{}: fighters off camera", s.id);
        assert!(w8.fighters.iter().any(|f| f.state == State::Win));
    }
}
