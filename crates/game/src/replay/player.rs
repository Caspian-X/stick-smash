//! Input-stream playback, checksum verification and bounded snapshot seeking.
use super::model::*;
use sim::{Input, World, step_world};

pub fn clear_world_inputs(w: &mut World) {
    for f in &mut w.fighters {
        f.history = Default::default();
        f.buffer = Default::default();
    }
}
pub struct Playback {
    pub replay: Replay,
    pub(super) snapshots: Vec<(u64, World)>,
    run: usize,
    offset: u64,
    pub position: u64,
    pub warning: Option<String>,
    pending_seek: Option<u64>,
}
impl Playback {
    pub fn new(replay: Replay) -> Result<Self, String> {
        replay.validate()?;
        let w = World::new(replay.config.clone(), &replay.data)?;
        let mut p = Self {
            replay,
            snapshots: vec![(0, w.clone())],
            run: 0,
            offset: 0,
            position: 0,
            warning: None,
            pending_seek: None,
        };
        p.verify(&w);
        Ok(p)
    }
    fn verify(&mut self, w: &World) {
        let checkpoint = interval(&self.replay.data);
        let expected = if self.position.is_multiple_of(checkpoint) {
            self.replay.checksums.get((self.position / checkpoint) as usize).map(|(_, h)| *h)
        } else if self.position == self.replay.ticks {
            Some(self.replay.final_hash)
        } else {
            None
        };
        if expected.is_some_and(|h| h != checksum(w)) && self.warning.is_none() {
            self.warning = Some(format!("Replay desync at tick {}: simulation differs from recording", self.position));
        }
    }
    pub fn inputs(&mut self, w: &mut World) -> Option<Vec<Input>> {
        if self.replay.clears.binary_search(&self.position).is_ok() {
            clear_world_inputs(w);
        }
        let r = self.replay.runs.get(self.run)?;
        let inputs = r.inputs.clone();
        self.offset += 1;
        if self.offset == r.ticks {
            self.offset = 0;
            self.run += 1;
        }
        self.position += 1;
        Some(inputs)
    }
    pub fn after_tick(&mut self, w: &World) {
        self.verify(w);
        if self.position.is_multiple_of(interval(&self.replay.data))
            && self.snapshots.last().is_none_or(|(t, _)| *t < self.position)
        {
            self.snapshots.push((self.position, w.clone()));
        }
    }
    pub fn seeking(&self) -> bool {
        self.pending_seek.is_some()
    }
    pub fn request_seek(&mut self, target: u64, w: &mut World) -> bool {
        let target = target.min(self.replay.ticks);
        if self.pending_seek == Some(target) || self.pending_seek.is_none() && target == self.position {
            return false;
        }
        let (tick, snapshot) = self.snapshots.iter().rev().find(|(t, _)| *t <= target).unwrap();
        *w = snapshot.clone();
        self.position = *tick;
        self.run = 0;
        self.offset = *tick;
        while self.run < self.replay.runs.len() && self.offset >= self.replay.runs[self.run].ticks {
            self.offset -= self.replay.runs[self.run].ticks;
            self.run += 1;
        }
        self.pending_seek = Some(target);
        true
    }
    pub fn advance_seek(&mut self, w: &mut World, budget: u32) -> bool {
        let Some(target) = self.pending_seek else { return false };
        for _ in 0..budget {
            if self.position == target {
                break;
            }
            let inputs = self.inputs(w).unwrap();
            step_world(w, &self.replay.data, &inputs);
            drain(w);
            self.after_tick(w);
        }
        if self.position == target {
            self.pending_seek = None;
        }
        true
    }
    pub fn seek(&mut self, target: u64, w: &mut World) {
        self.request_seek(target, w);
        while self.seeking() {
            self.advance_seek(w, sim::TICK_HZ * 4);
        }
    }
}
