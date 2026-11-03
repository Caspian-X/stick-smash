//! Plain recording model and input/checksum validation.
use super::content;
use serde::{Deserialize, Serialize};
use sim::{GameData, Input, MatchConfig, World};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) const FORMAT_VERSION: u32 = 2;
// Bump when simulation behavior or the checksum representation changes.
pub(super) const SIM_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-sim-22");
pub(super) fn interval(data: &GameData) -> u64 {
    data.global.replay.checkpoint_ticks.max(u64::from(sim::TICK_HZ))
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Run {
    pub(super) ticks: u64,
    pub(super) inputs: Vec<Input>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Replay {
    pub(super) version: u32,
    pub(super) created: u64,
    pub(super) sim_version: String,
    pub(super) clears: Vec<u64>,
    #[serde(with = "content")]
    pub data: GameData,
    pub config: MatchConfig,
    pub(super) runs: Vec<Run>,
    pub(super) checksums: Vec<(u64, u64)>,
    pub ticks: u64,
    pub(super) final_hash: u64,
    #[serde(skip)]
    pub(super) saved_path: Option<PathBuf>,
}
pub(super) fn checksum(w: &World) -> u64 {
    serde_json::to_vec(w)
        .expect("world serializes")
        .iter()
        .fold(0xcbf29ce484222325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3))
}
pub(super) fn drain(w: &mut World) {
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
}
impl Replay {
    pub fn new(data: &GameData, w: &World) -> Self {
        Self {
            version: FORMAT_VERSION,
            sim_version: SIM_VERSION.into(),
            clears: vec![],
            saved_path: None,
            created: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
            data: data.clone(),
            config: w.cfg.clone(),
            runs: vec![],
            checksums: vec![(0, checksum(w))],
            ticks: 0,
            final_hash: checksum(w),
        }
    }
    pub fn record(&mut self, inputs: Vec<Input>, w: &World) {
        if let Some(last) = self.runs.last_mut().filter(|r| r.inputs == inputs) {
            last.ticks += 1;
        } else {
            self.runs.push(Run { ticks: 1, inputs });
        }
        self.ticks += 1;
        if self.ticks.is_multiple_of(interval(&self.data)) {
            self.checksums.push((self.ticks, checksum(w)));
        }
        // Only recompute at checkpoints or the end of the match.
        if w.over {
            self.final_hash = checksum(w);
        }
    }
    pub fn clear_inputs(&mut self) {
        if self.clears.last() != Some(&self.ticks) {
            self.clears.push(self.ticks);
        }
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.version != FORMAT_VERSION || self.sim_version != SIM_VERSION {
            return Err(different_version());
        }
        if self.ticks == 0
            || self.runs.iter().any(|r| r.ticks == 0 || r.inputs.len() != self.config.players.len())
            || self.runs.iter().try_fold(0u64, |n, r| n.checked_add(r.ticks)) != Some(self.ticks)
        {
            return Err("invalid replay input stream".into());
        }
        if self.data.global.palette.is_empty() {
            return Err("content palette must not be empty".into());
        }
        if self.clears.iter().any(|t| *t >= self.ticks) || self.clears.windows(2).any(|w| w[0] >= w[1]) {
            return Err("invalid replay input resets".into());
        }
        let interval = interval(&self.data);
        if self.checksums.len() as u64 != self.ticks / interval + 1
            || self.checksums.iter().enumerate().any(|(i, (tick, _))| *tick != i as u64 * interval)
        {
            return Err("invalid replay checksum stream".into());
        }
        Ok(())
    }
}
pub(super) fn different_version() -> String {
    "Replay recorded with a different version".into()
}
