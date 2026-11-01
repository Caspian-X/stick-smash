#![allow(dead_code)]
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, data::load, step_world};

pub fn data() -> GameData {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let (sources, errors) = sim::data::sources(root);
    assert!(errors.is_empty(), "{errors:?}");
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
