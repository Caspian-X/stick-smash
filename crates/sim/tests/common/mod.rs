#![allow(dead_code)]
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, data::load, step_world};

pub fn data() -> GameData {
    load_data(false)
}

/// Authored pre-migration damage/BKB/growth and the damage used to fit dynamic counters.
pub fn knockback_baseline() -> std::collections::BTreeMap<String, std::collections::BTreeMap<String, [f64; 4]>> {
    serde_json::from_str(include_str!("../fixtures/knockback-linear.json")).unwrap()
}

pub fn linear_data() -> GameData {
    load_data(true)
}

/// Fighters whose balance pass overrides the shared base throws.
const THROW_OVERRIDE_FIGHTERS: [&str; 7] = [
    "fighters/archer/fighter.json",
    "fighters/brawler/fighter.json",
    "fighters/dancer/fighter.json",
    "fighters/golem/fighter.json",
    "fighters/lancer/fighter.json",
    "fighters/swordfighter/fighter.json",
    "fighters/tinker/fighter.json",
];

/// Pre-import packages the TS traces compare against; they replace (or add to) the frozen content.
const TS_FIGHTERS: [(&str, &str); 5] = [
    ("fighters/lancer/fighter.json", include_str!("../fixtures/lancer-ts.json")),
    ("fighters/ninja/fighter.json", include_str!("../fixtures/ninja-ts.json")),
    ("fighters/dancer/fighter.json", include_str!("../fixtures/dancer-ts.json")),
    ("fighters/wizard/fighter.json", include_str!("../fixtures/wizard-linear.json")),
    // The production King package is a King Dedede import; keep the original TS King for baseline traces.
    ("fighters/king/fighter.json", include_str!("../fixtures/king-ts.json")),
];

/// Linear data loads `fixtures/ts-content`, a frozen copy of the content the TS traces were matched against
/// (`just freeze-ts-content`), so rescaling or retuning `assets/` leaves the traces alone.
fn load_data(linear: bool) -> GameData {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = if linear { dir.join("tests/fixtures/ts-content") } else { dir.join("../../assets") };
    let (mut sources, errors) = sim::data::sources(root);
    assert!(errors.is_empty(), "{errors:?}");
    if linear {
        for (path, text) in TS_FIGHTERS {
            sources.retain(|s| s.path != path);
            sources.push(sim::data::Source { path: path.into(), text: text.into() });
        }
        sources.sort_by(|a, b| a.path.cmp(&b.path));
        let baseline = knockback_baseline();
        for s in &mut sources {
            let mut json: serde_json::Value = serde_json::from_str(&s.text).unwrap();
            if let Some(hits) = baseline.get(&s.path) {
                for (path, values) in hits {
                    let h = json.pointer_mut(path).unwrap();
                    h["damage"] = values[0].into();
                    h["bkb"] = values[1].into();
                    h["kbg"] = values[2].into();
                }
            }
            if s.path == "global.json" {
                json["knockback"].as_object_mut().unwrap().remove("ultimate");
            }
            // Balance passes scale inherited base throws inside the fighter package. TS traces compare
            // against the original shared throws, so drop those overrides and let the patched base win.
            if THROW_OVERRIDE_FIGHTERS.contains(&s.path.as_str()) {
                json.as_object_mut().unwrap().remove("throws");
            }
            s.text = serde_json::to_string(&json).unwrap();
        }
    }
    let (data, errors) = load(sources).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    data
}

/// The reference's defensive timings: invulnerable from the entry tick, no dodge staling, 7-frame shield drop.
pub fn reference_defence(d: &mut GameData) {
    use sim::data::global::{DodgeStaling, Window};
    let g = &mut d.global;
    let w = |frames, inv: u32| Window { frames, invuln_start: 0, invuln_end: inv - 1, ..Default::default() };
    g.dodge.roll = Window { move_start: 3, move_end: 18, speed: 7., ..w(30, 31) };
    g.dodge.spot = w(26, 16);
    g.dodge.air = Window { landing_lag: 4, ..w(30, 28) };
    g.dodge.dir_air = Window { speed: 10., decay: 0.88, decay_frames: 18, landing_lag: 10, ..w(34, 24) };
    g.dodge.staling = DodgeStaling::default();
    (g.tech.invuln_start, g.tech.invuln_end, g.tech.frames) = (0, 19, 24);
    g.knockdown.getup = w(24, 22);
    g.knockdown.getup_attack = w(32, 14);
    g.shield.drop_lag = 7;
}

pub fn sandbox(data: &GameData, stage: &str, players: Vec<PlayerConfig>) -> World {
    World::new(MatchConfig::sandbox(stage, players, MatchConfig::default().seed), data).unwrap()
}

pub fn fighters(data: &GameData, fighter: &str, opponents: &[&str]) -> World {
    let players = std::iter::once(fighter).chain(opponents.iter().copied()).map(PlayerConfig::human).collect();
    sandbox(data, "arena", players)
}

/// Keep both FX and hit records so scenarios can inspect all emitted events.
pub fn step_preserving(w: &mut World, data: &GameData, inputs: &[Input]) {
    step_world(w, data, inputs);
}

/// Match the game tick's event drain, keeping long-running worlds bounded.
pub fn step_draining(w: &mut World, data: &GameData, inputs: &[Input]) {
    step_world(w, data, inputs);
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
}

pub fn run_preserving(w: &mut World, data: &GameData, ticks: usize, inputs: &[Input]) {
    for _ in 0..ticks {
        step_preserving(w, data, inputs);
    }
}

pub fn run_draining(w: &mut World, data: &GameData, ticks: usize, inputs: &[Input]) {
    for _ in 0..ticks {
        step_draining(w, data, inputs);
    }
}

/// Follow a move's coverage with one victim, isolating contact identity from launch and hitlag.
pub fn contact_sequence(d: &GameData, fighter: &str, slot: &str) -> Vec<f64> {
    let mut w = fighters(d, fighter, &["brawler"]);
    w.count = 0;
    if fighter == "stratus" {
        w.fighters[0].charges[0] = 0;
    }
    let mut f = w.fighters[0].clone();
    f.x = 640.;
    f.y = 100.;
    f.ground = false;
    f.surf = None;
    sim::moves::start_move(&mut w, d, 0, &mut f, slot);
    w.fighters[0] = f;
    let mut damage = vec![];
    for _ in 0..200 {
        if w.fighters[0].mov.is_none() {
            return damage;
        }
        let f = &mut w.fighters[0];
        f.x = 640.;
        f.y = 100.;
        f.vx = 0.;
        f.vy = 0.;
        f.lag = 0;
        if let Some(b) = sim::combat::active_boxes_at(d, f, w.frame).first() {
            let v = &mut w.fighters[1];
            v.set_state(sim::world::State::Idle);
            v.inv = 0;
            v.blink = 0;
            v.lag = 0;
            v.x = b.x;
            v.y = b.y + 35.;
            v.ground = b.hit.targets(true);
            v.surf = None;
        }
        sim::combat::hit_pass(&mut w, d);
        damage.extend(w.hits.drain(..).filter(|h| h.attacker == Some(0)).map(|h| h.damage));
        let mut f = w.fighters[0].clone();
        sim::moves::st_attack(&mut w, d, 0, &mut f, Default::default(), 0.);
        w.fighters[0] = f;
        w.frame += 1;
        w.fx.clear();
        w.acts.clear();
    }
    panic!("{fighter}/{slot} did not finish");
}
