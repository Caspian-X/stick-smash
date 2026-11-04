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

fn load_data(linear: bool) -> GameData {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let (mut sources, errors) = sim::data::sources(root);
    assert!(errors.is_empty(), "{errors:?}");
    if linear {
        let baseline = knockback_baseline();
        for s in &mut sources {
            let text = if s.path == "fighters/lancer/fighter.json" {
                include_str!("../fixtures/lancer-ts.json")
            } else if s.path == "fighters/ninja/fighter.json" {
                include_str!("../fixtures/ninja-ts.json")
            } else if s.path == "fighters/dancer/fighter.json" {
                include_str!("../fixtures/dancer-ts.json")
            } else if s.path == "fighters/wizard/fighter.json" {
                include_str!("../fixtures/wizard-linear.json")
            } else if s.path == "fighters/king/fighter.json" {
                // The production King package is a King Dedede import; keep the original TS King for baseline traces.
                include_str!("../fixtures/king-ts.json")
            } else {
                &s.text
            };
            let mut json: serde_json::Value = serde_json::from_str(text).unwrap();
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
