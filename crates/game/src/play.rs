//! The running world (a match, training, the title-screen attract match or the character-select showcase):
//! ticking with slow-motion and training speed, per-device input, pause, match stats and the end of a match.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use render::Scene;
use sim::world::State as FState;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World as SimWorld, step_world};

use crate::input::{Act, Dev, Devices};
use crate::settings::DummyMode;
use crate::{Data, Screen};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Versus,
    Replay,
    Training,
    /// CPUs fighting behind the title menus.
    Attract,
    /// Character select: the picked fighters standing on the stage.
    Showcase,
}

/// One player of a match.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub dev: Option<Dev>,
    pub fighter: usize,
    pub cpu: Option<u8>,
    pub team: Option<u8>,
    pub tap_jump: bool,
    /// Control options of the device playing it (copied into the match config).
    pub controls: sim::input::Controls,
    /// "P1", "CPU", "Dummy".
    pub tag: String,
    /// Palette index the fighter is drawn with (its slot, or its team's color).
    pub color: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub kos: u32,
    pub falls: u32,
    pub sds: u32,
    pub dealt: f64,
    pub taken: f64,
    pub out_at: Option<u64>,
    last_hit: Option<(usize, u64)>,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub stocks: i32,
    pub stage: String,
    pub friendly_fire: bool,
    /// Time limit in seconds; `None` plays until one side runs out of stocks.
    pub time_limit: Option<u32>,
}

/// Time limit choices, SSBU style: Off, then 1:00 to 10:00 in 30-second steps.
pub fn time_choices() -> impl Iterator<Item = Option<u32>> {
    std::iter::once(None).chain((2..=20).map(|half_minutes| Some(half_minutes * 30)))
}

/// The choice `by` steps from `t` (wrapping).
pub fn cycle_time(t: Option<u32>, by: i32) -> Option<u32> {
    let all: Vec<_> = time_choices().collect();
    let at = all.iter().position(|c| *c == t).unwrap_or(0) as i32;
    all[(at + by).rem_euclid(all.len() as i32) as usize]
}

/// "Off" or "M:SS".
pub fn time_label(t: Option<u32>) -> String {
    t.map_or_else(|| "Off".into(), |s| format!("{}:{:02}", s / 60, s % 60))
}

/// The timer's remaining time as HUD text: `M:SS`, rounded up so the limit shows in full at the start.
pub fn time_remaining(ticks: u32) -> String {
    let s = ticks.div_ceil(sim::TICK_HZ);
    format!("{}:{:02}", s / 60, s % 60)
}

/// Training dummy settings.
#[derive(Clone, Debug)]
pub struct Dummy {
    pub mode: DummyMode,
    pub level: u8,
    /// Device controlling the dummy in `Human` mode (the first other device that presses a button).
    pub dev: Option<Dev>,
    pub reset_pct: f64,
}

#[derive(Resource)]
pub struct Match {
    pub mode: Mode,
    pub world: SimWorld,
    /// Content for this match: the loaded data with the palette reordered so each player draws in its color.
    pub data: Arc<GameData>,
    pub slots: Vec<Slot>,
    pub rules: Rules,
    pub stats: Vec<Stats>,
    pub paused: Option<Dev>,
    /// Training: speed multiplier, freeze and single-step.
    pub speed: f64,
    pub frozen: bool,
    pub step: bool,
    pub dummy: Dummy,
    /// Ticks to run immediately (screenshot tour fast-forward).
    pub ff: u32,
    acc: f64,
    pub over_t: u32,
    end_t: u32,
    /// Real seconds the end-of-match banner has shown (`0` while the match runs).
    pub banner_secs: f32,
    /// The results screen has opened for this match (returning from a watched replay doesn't reset it).
    pub results_shown: bool,
    pub debug: bool,
    pub sounds: Vec<sim::Fx>,
    pub recording: Option<crate::replay::Replay>,
    pub recording_done: bool,
    /// Never record (editor test matches, rematches included), so tests don't evict real auto-saves.
    pub unrecorded: bool,
    pub playback: Option<crate::replay::Playback>,
    /// Training: P1's recent controls and accepted actions.
    pub log: crate::training::InputLog,
}

/// Nonzero: every new match uses this seed (screenshot mode, so shots reproduce).
pub static FIXED_SEED: AtomicU32 = AtomicU32::new(0);

pub fn seed() -> u32 {
    let fixed = FIXED_SEED.load(Ordering::Relaxed);
    if fixed != 0 {
        return fixed;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.subsec_nanos() ^ d.as_secs() as u32)
}

impl Match {
    pub fn new(base: &Arc<GameData>, mode: Mode, slots: Vec<Slot>, rules: Rules, seed: u32) -> Result<Self, String> {
        Self::build(base, mode, slots, rules, seed, false, &[])
    }

    /// Sudden death gives one stock at `match.suddenDeathDamage` and no timer, whatever `rules` say; `out` (the
    /// slots that lost the time-up) start eliminated.
    fn build(
        base: &Arc<GameData>,
        mode: Mode,
        slots: Vec<Slot>,
        rules: Rules,
        seed: u32,
        sudden_death: bool,
        out: &[usize],
    ) -> Result<Self, String> {
        if base.global.palette.is_empty() {
            return Err("content palette must not be empty".into());
        }
        let mut data = (**base).clone();
        data.global.palette =
            slots.iter().map(|s| base.global.palette[s.color % base.global.palette.len()].clone()).collect();
        let data = Arc::new(data);
        let cfg = MatchConfig {
            seed,
            stage: rules.stage.clone(),
            stocks: if sudden_death { 1 } else { rules.stocks },
            training: matches!(mode, Mode::Training | Mode::Showcase),
            friendly_fire: rules.friendly_fire,
            time_limit: rules.time_limit.filter(|_| !sudden_death),
            sudden_death,
            players: slots
                .iter()
                .enumerate()
                .map(|(i, s)| PlayerConfig {
                    fighter: base.fighters[s.fighter].id.clone(),
                    cpu: s.cpu,
                    team: s.team,
                    tap_jump: s.tap_jump,
                    controls: s.controls,
                    eliminated: out.contains(&i),
                })
                .collect(),
        };
        let mut world = SimWorld::new(cfg, &data)?;
        if mode == Mode::Showcase {
            world.count = 0;
        }
        let recording = (mode == Mode::Versus).then(|| crate::replay::Replay::new(&data, &world));
        Ok(Match {
            mode,
            world,
            data,
            stats: vec![Stats::default(); slots.len()],
            slots,
            rules,
            paused: None,
            speed: 1.,
            frozen: false,
            step: false,
            dummy: Dummy { mode: DummyMode::Stand, level: 1, dev: None, reset_pct: 0. },
            ff: 0,
            acc: 0.,
            over_t: 0,
            end_t: 0,
            banner_secs: 0.,
            results_shown: false,
            debug: mode == Mode::Training,
            sounds: Vec::new(),
            recording,
            recording_done: false,
            unrecorded: false,
            playback: None,
            log: Default::default(),
        })
    }

    pub fn unrecord(&mut self) {
        self.unrecorded = true;
        self.recording = None;
    }

    /// Same players and rules, fresh world.
    pub fn restart(&mut self, base: &Arc<GameData>, seed: u32) {
        let Ok(mut m) = Match::new(base, self.mode, self.slots.clone(), self.rules.clone(), seed) else { return };
        m.dummy = self.dummy.clone();
        m.debug = self.debug;
        m.speed = self.speed;
        if self.unrecorded {
            m.unrecord();
        }
        *self = m;
        self.apply_dummy();
        if self.mode == Mode::Training && self.world.fighters.len() > 1 {
            self.world.fighters[1].dmg = self.dummy.reset_pct;
        }
    }

    /// Sudden death after a time-up tie: the sides still tied for the lead start at `match.suddenDeathDamage` with
    /// one stock each and no timer; everyone else starts out. Returns false (and changes nothing) when nobody is
    /// left to fight. Replaces the recording, so a replay of a time-up tie only covers the sudden death.
    pub fn start_sudden_death(&mut self, base: &Arc<GameData>, seed: u32) -> bool {
        // Who tied is fixed when the clock hit zero, not recomputed after the ending animation.
        let leaders = self.world.time_tied.clone();
        if leaders.is_empty() {
            return false;
        }
        let out: Vec<usize> = (0..self.slots.len()).filter(|i| !leaders.contains(i)).collect();
        let Ok(mut m) = Self::build(base, self.mode, self.slots.clone(), self.rules.clone(), seed, true, &out) else {
            return false;
        };
        m.debug = self.debug;
        m.speed = self.speed;
        // Regulation stats carry over; the sides that lost the time up are out from the first frame.
        m.stats = self.stats.clone();
        for i in &out {
            m.stats[*i].out_at = Some(0);
        }
        if self.unrecorded {
            m.unrecord();
        }
        *self = m;
        true
    }

    /// Training: the dummy behaves like `mode` from now on (and next time).
    pub fn set_dummy_mode(&mut self, mode: DummyMode, devs: &mut Devices, saved: &mut crate::settings::Saved) {
        self.dummy.mode = mode;
        self.dummy.dev = None;
        self.clear_input(devs);
        saved.training.mode = mode;
        self.apply_dummy();
    }

    pub fn apply_dummy(&mut self) {
        if self.mode == Mode::Training && self.world.cfg.players.len() > 1 {
            self.world.cfg.players[1].cpu = (self.dummy.mode == DummyMode::Cpu).then_some(self.dummy.level);
        }
    }

    pub fn attract(base: &Arc<GameData>, seed: u32) -> Result<Self, String> {
        let n = base.fighters.len();
        let stage = &base.stages[seed as usize % base.stages.len()];
        let players = 4.min(stage.max_players());
        let slots = (0..players)
            .map(|i| Slot {
                dev: None,
                fighter: (i + seed as usize) % n,
                cpu: Some(2),
                team: None,
                tap_jump: true,
                controls: Default::default(),
                tag: "CPU".into(),
                color: i,
            })
            .collect();
        let rules = Rules { stocks: 2, stage: stage.id.clone(), friendly_fire: false, time_limit: None };
        Match::new(base, Mode::Attract, slots, rules, seed)
    }

    fn inputs(&self, devs: &mut Devices) -> Vec<Input> {
        let w = &self.world;
        (0..w.fighters.len())
            .map(|i| {
                if self.mode == Mode::Training && i == 1 {
                    let f = &w.fighters[1];
                    return match self.dummy.mode {
                        DummyMode::Shield => Input { stick: (0, 0), buttons: 16, ..Default::default() },
                        DummyMode::Jump if f.state == FState::Idle && f.t > 20 => {
                            Input { stick: (0, 0), buttons: 1, ..Default::default() }
                        }
                        DummyMode::Human => self.dummy.dev.map_or_else(Input::default, |d| devs.take(d)),
                        _ => Input::default(),
                    };
                }
                match (self.mode, self.slots.get(i).and_then(|s| s.dev)) {
                    (Mode::Versus | Mode::Training, Some(d)) => devs.take(d),
                    _ => Input::default(),
                }
            })
            .collect()
    }

    pub(crate) fn tick(&mut self, devs: &mut Devices, scene: &mut Scene, audible: bool) {
        let inputs = if let Some(p) = &mut self.playback {
            let Some(inputs) = p.inputs(&mut self.world) else {
                self.frozen = true;
                return;
            };
            inputs
        } else if self.world.over {
            vec![Input::default(); self.world.fighters.len()]
        } else {
            self.inputs(devs)
        };
        let before: Vec<(f64, i32)> = self.world.fighters.iter().map(|f| (f.dmg, f.stocks)).collect();
        step_world(&mut self.world, &self.data, &inputs);
        self.track(&before);
        if self.world.end_t > 0 {
            self.end_t += 1;
        }
        if self.world.over {
            self.over_t += 1;
        }
        if audible && matches!(self.mode, Mode::Versus | Mode::Training | Mode::Replay) {
            self.sounds.extend(self.world.fx.iter().filter(|fx| matches!(fx, sim::Fx::Sfx { .. })).cloned());
        }
        scene.push(&self.world);
        if self.mode == Mode::Training {
            self.log.record(self.world.frame, inputs[0], &self.world.acts);
        }
        self.world.fx.clear();
        self.world.hits.clear();
        self.world.acts.clear();
        if !self.recording_done
            && let Some(r) = &mut self.recording
        {
            r.record(inputs, &self.world);
        }
        if let Some(p) = &mut self.playback {
            p.after_tick(&self.world);
        }
    }

    /// KOs, falls, self-destructs and damage, credited from damage and stock changes plus the sim's hit trace.
    fn track(&mut self, before: &[(f64, i32)]) {
        let w = &self.world;
        for hit in &w.hits {
            if hit.shielded || hit.damage <= 0. {
                continue;
            }
            let Some(victim) = self.stats.get_mut(hit.victim) else { continue };
            victim.taken += hit.damage;
            if let Some(by) = hit.attacker.filter(|b| *b != hit.victim && *b < w.fighters.len()) {
                victim.last_hit = Some((by, w.frame));
                self.stats[by].dealt += hit.damage;
            }
        }
        for (i, f) in w.fighters.iter().enumerate() {
            let (_, stocks0) = before[i];
            if f.stocks < stocks0 {
                let s = &mut self.stats[i];
                s.falls += 1;
                match s.last_hit.take().filter(|(b, t)| {
                    Some(*b) == f.last_hit_by && w.frame - t < self.data.global.render.hud.ko_credit_ticks
                }) {
                    Some((b, _)) => self.stats[b].kos += 1,
                    None => self.stats[i].sds += 1,
                }
                if f.stocks <= 0 {
                    self.stats[i].out_at = Some(w.frame);
                }
            }
        }
    }

    pub fn clear_input(&mut self, devs: &mut Devices) {
        devs.clear();
        if !self.recording_done
            && let Some(r) = &mut self.recording
        {
            r.clear_inputs();
        }
        crate::replay::clear_world_inputs(&mut self.world);
    }

    /// Placement (1-based) per player: the winner (and teammates) first, then by how long they lasted.
    pub fn placements(&self) -> Vec<usize> {
        let w = &self.world;
        let team_of = |i: usize| w.cfg.players[i].team;
        let won =
            |i: usize| w.winner.is_some_and(|win| win == i || team_of(win).is_some() && team_of(win) == team_of(i));
        // Winners tie for first.
        let score = |i: usize| -> (bool, u64, i32, i64) {
            let f = &w.fighters[i];
            if won(i) {
                (true, 0, 0, 0)
            } else {
                (false, self.stats[i].out_at.unwrap_or(u64::MAX), f.stocks, -(f.dmg as i64))
            }
        };
        (0..w.fighters.len()).map(|i| 1 + (0..w.fighters.len()).filter(|&j| score(j) > score(i)).count()).collect()
    }

    pub fn alpha(&self, overstep: f64) -> f64 {
        (self.acc + overstep * self.tick_speed()).clamp(0., 1.)
    }

    fn tick_speed(&self) -> f64 {
        let slow = &self.data.global.render.hud.end_slowmo;
        let mut s = if matches!(self.mode, Mode::Training | Mode::Replay) { self.speed } else { 1. };
        if self.mode != Mode::Replay && self.world.end_t > 0 && self.end_t < slow.ticks {
            s *= slow.speed;
        }
        s
    }
}

/// Steps the world at `sim::TICK_HZ` (scaled by slow motion / training speed).
pub fn tick(
    mut m: ResMut<Match>,
    mut devs: ResMut<Devices>,
    mut scene: ResMut<Scene>,
    screen: Res<State<Screen>>,
    pace: Option<ResMut<Pace>>,
) {
    let catch_up = pace.as_ref().is_some_and(|p| p.catch_up());
    let (ticks, normal) = if matches!(screen.get(), Screen::Replays) {
        (0, false)
    } else {
        run_frame(&mut m, &mut devs, &mut scene, catch_up)
    };
    if let Some(mut p) = pace {
        p.settle(ticks, normal);
    }
}

/// One frame's ticks: how many ran (fast-forward aside) and whether this was a normal-speed frame. `catch_up`
/// adds one tick to a normal-speed frame.
fn run_frame(m: &mut Match, devs: &mut Devices, scene: &mut Scene, catch_up: bool) -> (u32, bool) {
    if m.playback.as_ref().is_some_and(|p| p.seeking()) {
        return (0, false);
    }
    let ff = std::mem::take(&mut m.ff);
    for _ in 0..ff {
        m.tick(devs, scene, false);
    }
    if m.paused.is_some() {
        return (0, false);
    }
    if matches!(m.mode, Mode::Training | Mode::Replay) && m.frozen {
        let step = std::mem::take(&mut m.step);
        if step {
            m.tick(devs, scene, true);
        }
        return (u32::from(step), false);
    }
    let speed = m.tick_speed();
    let normal = speed == 1.;
    m.acc += speed + if catch_up && normal { 1. } else { 0. };
    let mut n = 0;
    while m.acc >= 1. {
        m.acc -= 1.;
        m.tick(devs, scene, true);
        n += 1;
    }
    (n, normal)
}

/// Dev builds: with the debug overlay (F1) on, how many sim ticks recent frames ran, to check frame pacing.
#[cfg(feature = "dev")]
pub fn pace_overlay(mut ctx: bevy_egui::EguiContexts, m: Option<Res<Match>>, pace: Res<Pace>) -> Result {
    use bevy_egui::egui;
    if !m.is_some_and(|m| m.debug) {
        return Ok(());
    }
    let ([zero, one, more], dt) = pace.summary();
    let mode = if pace.locked { "locked to display" } else { "fixed step" };
    let text = format!(
        "ticks/frame (1 s): 0×{zero}  1×{one}  2+×{more}   mean frame {:.2} ms ({:.1} Hz)   {mode}",
        dt * 1000.,
        if dt > 0. { 1. / dt } else { 0. }
    );
    egui::Area::new(egui::Id::new("pace")).anchor(egui::Align2::LEFT_BOTTOM, [12., -12.]).show(ctx.ctx_mut()?, |ui| {
        egui::Frame::new().fill(egui::Color32::from_black_alpha(180)).corner_radius(6).inner_margin(6).show(ui, |ui| {
            ui.label(egui::RichText::new(text).monospace().size(12.).color(egui::Color32::WHITE));
        });
    });
    Ok(())
}

/// Frame pacing. On a display refreshing at about `TICK_HZ`, the fixed-step accumulator aliases against vsync
/// (frames with 0 or 2 ticks: stutter, and inputs landing a frame late at random), so while [`Pace::locked`] the sim
/// steps once per rendered frame instead and draws the newest tick.
#[derive(Resource)]
pub struct Pace {
    /// Recent frames, newest last: real interval in seconds and sim ticks run.
    frames: VecDeque<(f32, u32)>,
    /// Sim ticks run so far this frame.
    ticks: u32,
    /// Locked: real time not yet simulated, in ticks (normal speed only), so dropped frames are caught up.
    behind: f64,
    /// A match ran last frame; otherwise nothing is owed.
    settled: bool,
    /// The setting is on, pacing is allowed and the display runs at about `TICK_HZ`.
    pub locked: bool,
    /// Off in the screenshot tours, whose timing must not depend on the display.
    pub allowed: bool,
}

impl Default for Pace {
    fn default() -> Self {
        Pace { frames: VecDeque::new(), ticks: 0, behind: 0., settled: false, locked: false, allowed: true }
    }
}

/// Frames measured for the refresh rate (median interval), and how far from `TICK_HZ` still counts as a match.
const PACE_SAMPLES: usize = 30;
const PACE_TOLERANCE_HZ: f32 = 2.;
/// Locked, at most this many ticks of real time are owed; a longer stall is not caught up.
const PACE_MAX_BEHIND: f64 = 3.;

impl Pace {
    fn push(&mut self, dt: f32) {
        let ticks = std::mem::take(&mut self.ticks);
        if dt > 0. {
            self.frames.push_back((dt, ticks));
        }
        while self.frames.len() > 360 {
            self.frames.pop_front();
        }
    }
    /// Display refresh rate from the median of recent frame intervals.
    pub fn refresh_hz(&self) -> Option<f32> {
        if self.frames.len() < PACE_SAMPLES {
            return None;
        }
        let mut dts: Vec<f32> = self.frames.iter().rev().take(PACE_SAMPLES).map(|f| f.0).collect();
        dts.sort_by(f32::total_cmp);
        Some(1. / dts[PACE_SAMPLES / 2])
    }
    /// Over about the last second: frames that ran 0, 1 and 2+ sim ticks, and the mean frame interval in seconds.
    #[cfg(feature = "dev")]
    pub fn summary(&self) -> ([u32; 3], f32) {
        let (mut counts, mut total, mut n) = ([0; 3], 0., 0);
        for (dt, ticks) in self.frames.iter().rev() {
            if total >= 1. {
                break;
            }
            counts[(*ticks as usize).min(2)] += 1;
            total += dt;
            n += 1;
        }
        (counts, if n > 0 { total / n as f32 } else { 0. })
    }
    fn update(&mut self, dt: f32, enabled: bool) {
        self.push(dt);
        self.locked = self.allowed
            && enabled
            && self.refresh_hz().is_some_and(|hz| (hz - sim::TICK_HZ as f32).abs() <= PACE_TOLERANCE_HZ);
        let owed = if std::mem::take(&mut self.settled) { self.behind } else { 0. };
        self.behind =
            if self.locked { (owed + f64::from(dt) * f64::from(sim::TICK_HZ)).min(PACE_MAX_BEHIND) } else { 0. };
    }
    /// More than one tick behind real time: this frame runs a second tick.
    fn catch_up(&self) -> bool {
        self.locked && self.behind >= 2.
    }
    /// Books a frame's ticks. Paused, frozen or off-speed frames keep their own pace and owe nothing.
    fn settle(&mut self, ticks: u32, normal: bool) {
        self.ticks += ticks;
        self.settled = true;
        self.behind = if self.locked && normal { (self.behind - f64::from(ticks)).max(0.) } else { 0. };
    }
}

/// Measures the frame rate and decides whether the sim follows the display this frame. Runs in `First`.
pub fn pace(time: Res<Time<Real>>, saved: Res<crate::settings::Saved>, mut p: ResMut<Pace>) {
    p.update(time.delta_secs(), saved.lock_60);
}

pub fn locked(p: Option<Res<Pace>>) -> bool {
    p.is_some_and(|p| p.locked)
}

pub fn interpolate(time: Res<Time<Fixed>>, pace: Option<Res<Pace>>, m: Res<Match>, mut scene: ResMut<Scene>) {
    if m.mode == Mode::Replay && m.frozen {
        scene.alpha = 1.;
        return;
    }
    if m.paused.is_none() && !(matches!(m.mode, Mode::Training | Mode::Replay) && m.frozen) {
        // Locked, each frame is a whole tick period that `tick` has already consumed.
        let overstep = if pace.is_some_and(|p| p.locked) { 1. } else { time.overstep_fraction_f64() };
        scene.alpha = m.alpha(overstep);
    }
}

/// Same players and rules on a fresh world (drawn afresh), with no input carried over.
pub fn restart(m: &mut Match, data: &Arc<GameData>, scene: &mut Scene, devs: &mut Devices) {
    m.restart(data, seed());
    scene.reset(&m.world);
    m.clear_input(devs);
}

/// Swap in a match: the renderer starts fresh with its data.
pub fn install(commands: &mut Commands, scene: &mut Scene, m: Match) {
    scene.data = m.data.clone();
    scene.reset(&m.world);
    scene.debug = m.debug;
    commands.insert_resource(m);
}

/// Attract match behind the title menus; restarts when it ends.
pub fn attract(mut commands: Commands, m: Option<Res<Match>>, data: Res<Data>, mut scene: ResMut<Scene>) {
    let restart = m.as_ref().is_none_or(|m| m.mode != Mode::Attract || m.over_t > 90);
    if restart {
        let seed = m.as_ref().map_or_else(seed, |m| m.world.cfg.seed.wrapping_mul(747_796_405).wrapping_add(1));
        match Match::attract(&data.0, seed) {
            Ok(m) => install(&mut commands, &mut scene, m),
            Err(e) => error!("attract match: {e}"),
        }
    }
}

/// Pause on Start from a player's device; the pause menu takes over from there. Also the F1 debug toggle, sudden
/// death after a time-up tie, and the move to the results screen.
pub fn match_flow(
    mut m: ResMut<Match>,
    mut devs: ResMut<Devices>,
    saved: Res<crate::settings::Saved>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut scene: ResMut<Scene>,
    data: Res<Data>,
    mut next: ResMut<NextState<Screen>>,
) {
    // Real seconds the end banner has shown, so the results screen's arrival never depends on the end freeze. The
    // clock keeps running past the freeze, so a short `match.endFrames` never cuts the banner's fade-out short.
    m.banner_secs = if m.world.end_t > 0 || m.world.over { m.banner_secs + time.delta_secs() } else { 0. };
    if keys.just_pressed(KeyCode::F1) {
        m.debug = !m.debug;
    }
    scene.debug = m.debug;
    if m.dummy.dev.is_some_and(|d| !devs.map.contains_key(&d)) {
        m.dummy.dev = None;
        m.clear_input(&mut devs);
    }
    if m.mode == Mode::Training && m.dummy.mode == DummyMode::Human && m.dummy.dev.is_none() {
        let p1 = m.slots[0].dev;
        if let Some((d, _)) = devs.map.iter().find(|(d, s)| Some(**d) != p1 && s.any_press()) {
            let prefs = crate::settings::control_prefs(&saved, &devs.profile(*d));
            m.dummy.dev = Some(*d);
            m.world.cfg.players[1].controls = prefs.controls();
            m.world.cfg.players[1].tap_jump = prefs.tap_jump;
            m.clear_input(&mut devs);
        }
    }
    if m.paused.is_none() && m.world.end_t == 0 && !m.world.over {
        let human = |d: Dev| m.slots.iter().any(|s| s.dev == Some(d)) || m.dummy.dev == Some(d);
        if let Some((d, _)) = devs.map.iter().find(|(d, s)| s.pressed[Act::Start as usize] && human(**d)) {
            m.paused = Some(*d);
            m.clear_input(&mut devs);
        }
    }
    // Time up with the tied sides level: they start again at 300%, one stock each.
    if m.mode == Mode::Versus
        && m.world.over
        && m.world.time_up
        && m.world.winner.is_none()
        && !m.world.cfg.sudden_death
        && m.start_sudden_death(&data.0, seed())
    {
        scene.reset(&m.world);
        m.clear_input(&mut devs);
        return;
    }
    if results_ready(&m) {
        next.set(Screen::Results);
    }
}

/// Whether the results screen takes over: once the end banner has shown for `render.hud.endBanner.secs` real seconds,
/// or `render.hud.resultsDelay` ticks after the winner's pose (which is how fast-forwarded matches get there). A
/// time-up tie waits for the pose, since sudden death starts from there.
fn results_ready(m: &Match) -> bool {
    let hud = &m.data.global.render.hud;
    let posed = m.world.over && m.over_t >= hud.results_delay;
    let banner = m.world.winner.is_some() && m.banner_secs >= hud.end_banner.secs;
    m.mode == Mode::Versus && (posed || banner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pace_locks_only_near_the_tick_rate_and_when_allowed() {
        let mut p = Pace::default();
        for _ in 0..PACE_SAMPLES - 1 {
            p.update(1. / 60., true);
        }
        assert!(!p.locked, "not enough samples yet");
        p.update(0.5, true);
        assert!(p.locked, "one hitch doesn't move the median");
        p.update(1. / 60., false);
        assert!(!p.locked, "setting off");
        for _ in 0..PACE_SAMPLES {
            p.update(1. / 144., true);
        }
        assert!(!p.locked, "144 Hz keeps the fixed step");
        for _ in 0..PACE_SAMPLES {
            p.update(1. / 59.94, true);
        }
        assert!(p.locked);
        p.allowed = false;
        p.update(1. / 60., true);
        assert!(!p.locked, "screenshot tours never lock");
    }

    #[test]
    fn locked_frames_tick_once_and_draw_the_newest_tick() {
        let data = crate::test_data();
        let mut m = Match::attract(&data, 1).unwrap();
        let (mut devs, mut scene) = (Devices::default(), Scene::new(data));
        for speed in [1., 0.5, 2.] {
            m.mode = Mode::Training;
            m.speed = speed;
            let mut seen = vec![];
            for _ in 0..4 {
                let frame = m.world.frame;
                m.acc += m.tick_speed();
                while m.acc >= 1. {
                    m.acc -= 1.;
                    m.tick(&mut devs, &mut scene, false);
                }
                seen.push(((m.world.frame - frame) as u32, m.alpha(1.)));
            }
            match speed {
                1. => assert_eq!(seen, [(1, 1.); 4]),
                0.5 => assert_eq!(seen, [(0, 1.), (1, 0.5), (0, 1.), (1, 0.5)]),
                _ => assert_eq!(seen, [(2, 1.); 4]),
            }
        }
    }

    #[test]
    fn locked_frames_catch_up_a_dropped_frame_with_at_most_two_ticks() {
        let data = crate::test_data();
        let mut m = Match::attract(&data, 1).unwrap();
        m.mode = Mode::Training;
        let (mut devs, mut scene) = (Devices::default(), Scene::new(data));
        let mut p = Pace::default();
        for _ in 0..PACE_SAMPLES {
            p.update(1. / 60., true);
        }
        let mut frame = |p: &mut Pace, m: &mut Match, dt: f32| {
            p.update(dt, true);
            let (n, normal) = run_frame(m, &mut devs, &mut scene, p.catch_up());
            p.settle(n, normal);
            n
        };
        let mut run =
            |p: &mut Pace, m: &mut Match, dts: &[f32]| dts.iter().map(|dt| frame(p, m, *dt)).collect::<Vec<_>>();
        let f = 1. / 60.;
        assert_eq!(run(&mut p, &mut m, &[f, f, f]), [1, 1, 1]);
        assert_eq!(run(&mut p, &mut m, &[2. * f, f, f]), [2, 1, 1], "one dropped frame is made up");
        assert_eq!(run(&mut p, &mut m, &[10. * f, f, f, f]), [2, 2, 1, 1], "never more than two, long stalls capped");
        m.paused = Some(Dev::Keys);
        assert_eq!(run(&mut p, &mut m, &[2. * f]), [0]);
        m.paused = None;
        assert_eq!(run(&mut p, &mut m, &[f]), [1], "pause owes nothing");
        m.frozen = true;
        assert_eq!(run(&mut p, &mut m, &[3. * f]), [0]);
        m.frozen = false;
        assert_eq!(run(&mut p, &mut m, &[f]), [1], "frozen owes nothing");
        m.speed = 0.5;
        assert_eq!(run(&mut p, &mut m, &[3. * f, f, f, f]), [0, 1, 0, 1], "slow motion keeps its own pace");
        m.speed = 1.;
        assert_eq!(run(&mut p, &mut m, &[f]), [1]);
    }

    #[test]
    fn time_labels_and_choices() {
        assert_eq!(time_label(None), "Off");
        assert_eq!(time_label(Some(60)), "1:00");
        assert_eq!(time_label(Some(450)), "7:30");
        assert_eq!(cycle_time(None, 1), Some(60));
        assert_eq!(cycle_time(Some(60), -1), None);
        assert_eq!(cycle_time(Some(60), 18), Some(600));
        assert_eq!(cycle_time(Some(600), 1), None);
        assert_eq!(time_remaining(60 * 180), "3:00");
        assert_eq!(time_remaining(60 * 60 - 1), "1:00");
        assert_eq!(time_remaining(1), "0:01");
        assert_eq!(time_remaining(0), "0:00");
    }

    /// The winner's pose follows "GAME!" in a beat, so the replay is finished and closed quickly.
    /// The results screen does not wait for it: `render.hud.endBanner` paces that.
    #[test]
    fn the_winner_poses_soon_after_game() {
        let data = crate::test_data();
        let slot = |i: usize| Slot {
            dev: None,
            fighter: i,
            cpu: Some(1),
            team: None,
            tap_jump: true,
            controls: Default::default(),
            tag: "CPU".into(),
            color: i,
        };
        let rules = Rules { stocks: 1, stage: "arena".into(), friendly_fire: false, time_limit: None };
        let mut m = Match::new(&data, Mode::Versus, (0..2).map(slot).collect(), rules, 7).unwrap();
        let (mut devs, mut scene) = (Devices::default(), Scene::new(data));
        m.world.count = 0;
        m.world.fighters[1].stocks = 0;
        m.world.fighters[1].set_state(sim::world::State::Out);
        m.world.winner = Some(0);
        m.world.end_t = m.data.global.match_.end_frames;
        let mut frames = 0;
        while !m.world.over && frames < 600 {
            m.acc += m.tick_speed();
            while m.acc >= 1. {
                m.acc -= 1.;
                m.tick(&mut devs, &mut scene, false);
            }
            frames += 1;
        }
        assert!(m.world.over, "the match ends");
        assert!(frames <= 100, "\"GAME!\" to the winner's pose took {frames} frames (1.7 s at 60 Hz)");
    }

    #[test]
    fn the_end_banner_paces_the_results_before_the_pose() {
        let data = crate::test_data();
        let slot = |i: usize| Slot {
            dev: None,
            fighter: i,
            cpu: Some(1),
            team: None,
            tap_jump: true,
            controls: Default::default(),
            tag: "CPU".into(),
            color: i,
        };
        let rules = Rules { stocks: 1, stage: "arena".into(), friendly_fire: false, time_limit: None };
        let mut m = Match::new(&data, Mode::Versus, (0..2).map(slot).collect(), rules, 7).unwrap();
        let banner = m.data.global.render.hud.end_banner.secs;
        m.world.winner = Some(0);
        m.world.end_t = m.data.global.match_.end_frames;
        assert!(!results_ready(&m), "not before the banner has shown");
        m.banner_secs = banner;
        assert!(results_ready(&m), "the banner paces it, well before the pose");
        assert!(!m.world.over);
        // A time-up tie plays sudden death first, however long the banner has been up.
        m.world.winner = None;
        m.world.time_up = true;
        m.banner_secs = banner * 4.;
        assert!(!results_ready(&m));
        // With the pose (or a draw), the handoff takes it.
        m.world.over = true;
        assert!(results_ready(&m));
    }

    #[test]
    fn a_time_up_tie_starts_sudden_death_and_a_decision_does_not() {
        let data = crate::test_data();
        let slot = |i: usize| Slot {
            dev: None,
            fighter: i,
            cpu: Some(1),
            team: None,
            tap_jump: true,
            controls: Default::default(),
            tag: "CPU".into(),
            color: i,
        };
        let rules = || Rules { stocks: 2, stage: "arena".into(), friendly_fire: false, time_limit: Some(60) };
        let m = Match::new(&data, Mode::Versus, (0..3).map(slot).collect(), rules(), 7).unwrap();
        let (devs, scene) = (Devices::default(), Scene::new(data.clone()));
        let mut app = App::new();
        app.insert_resource(m)
            .insert_resource(devs)
            .insert_resource(scene)
            .insert_resource(Data(data.clone()))
            .init_resource::<crate::settings::Saved>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Time>()
            .insert_resource(State::new(Screen::Match))
            .insert_resource(NextState::<Screen>::default())
            .add_systems(Update, match_flow);
        // Two sides level on stocks and damage; the third is out of stocks.
        {
            let mut m = app.world_mut().resource_mut::<Match>();
            m.world.fighters[2].stocks = 0;
            m.world.over = true;
            m.world.time_up = true;
            m.world.winner = None;
            m.world.time_tied = vec![0, 1];
            m.stats[0].kos = 4;
        }
        app.update();
        {
            let m = app.world().resource::<Match>();
            assert!(m.world.cfg.sudden_death, "a time-up tie goes to sudden death");
            assert_eq!(m.world.cfg.time_limit, None);
            assert!(!m.world.over);
            assert_eq!(m.world.time_left, None);
            for f in &m.world.fighters[..2] {
                assert_eq!(f.dmg, m.data.global.match_.sudden_death_damage);
                assert_eq!(f.stocks, 1);
            }
            let out = &m.world.fighters[2];
            assert_eq!(out.stocks, 0, "only the tied sides play it");
            assert_eq!(out.state, sim::world::State::Out);
            assert_eq!(m.stats[0].kos, 4, "regulation stats carry over");
            let place = m.placements();
            assert_eq!(place[2], 3, "the side that lost the time up places last");
        }
        // Everyone tied (no eliminated slot) is still sudden death, not a second timed match.
        let m = Match::new(&data, Mode::Versus, (0..2).map(slot).collect(), rules(), 7).unwrap();
        app.world_mut().insert_resource(m);
        {
            let mut m = app.world_mut().resource_mut::<Match>();
            m.world.over = true;
            m.world.time_up = true;
            m.world.winner = None;
            m.world.time_tied = vec![0, 1];
        }
        app.update();
        {
            let m = app.world().resource::<Match>();
            assert!(m.world.cfg.sudden_death);
            assert_eq!(m.world.cfg.stocks, 1);
            assert!(m.world.fighters.iter().all(|f| f.stocks == 1));
        }
        // A decided time up ends the match instead.
        let m = Match::new(&data, Mode::Versus, (0..3).map(slot).collect(), rules(), 7).unwrap();
        app.world_mut().insert_resource(m);
        {
            let mut m = app.world_mut().resource_mut::<Match>();
            m.world.over = true;
            m.world.time_up = true;
            m.world.winner = Some(1);
            m.over_t = m.data.global.render.hud.results_delay;
        }
        app.update();
        {
            let m = app.world().resource::<Match>();
            assert!(!m.world.cfg.sudden_death);
            assert!(matches!(app.world().resource::<NextState<Screen>>(), NextState::Pending(Screen::Results)));
        }
    }

    #[test]
    fn fast_forward_discards_countdown_sounds_but_live_ticks_keep_them() {
        let data = crate::test_data();
        let mut m = Match::attract(&data, 1).unwrap();
        m.mode = Mode::Versus;
        let (mut devs, mut scene) = (Devices::default(), Scene::new(data));
        while m.world.count > 0 {
            m.tick(&mut devs, &mut scene, false);
        }
        assert!(m.sounds.is_empty());
        m.world.count = sim::TICK_HZ;
        m.tick(&mut devs, &mut scene, true);
        assert!(m.sounds.iter().any(|fx| matches!(fx, sim::Fx::Sfx { name, .. } if name == "beep")));
    }

    #[test]
    fn hit_records_credit_every_victim_and_last_hitter() {
        let data = crate::test_data();
        let mut m = Match::attract(&data, 1).unwrap();
        let before: Vec<_> = m.world.fighters.iter().map(|f| (f.dmg, f.stocks)).collect();
        m.world.frame = 10;
        m.world.hits = vec![
            sim::HitRecord { attacker: Some(0), victim: 1, damage: 12., shielded: false },
            sim::HitRecord { attacker: Some(0), victim: 2, damage: 8., shielded: false },
            sim::HitRecord { attacker: Some(2), victim: 1, damage: 3., shielded: false },
            sim::HitRecord { attacker: Some(1), victim: 0, damage: 9., shielded: true },
        ];
        m.world.fighters[1].last_hit_by = Some(2);
        m.world.fighters[2].last_hit_by = Some(0);
        m.world.fighters[1].stocks -= 1;
        m.track(&before);
        assert_eq!(m.stats[0].dealt, 20.);
        assert_eq!(m.stats[1].taken, 15.);
        assert_eq!(m.stats[2].taken, 8.);
        assert_eq!(m.stats[0].taken, 0.);
        assert_eq!(m.stats[2].kos, 1);
        assert_eq!(m.stats[1].sds, 0);
        m.world.hits.clear();
        let before: Vec<_> = m.world.fighters.iter().map(|f| (f.dmg, f.stocks)).collect();
        m.world.frame += data.global.render.hud.ko_credit_ticks;
        m.world.fighters[2].stocks -= 1;
        m.track(&before);
        assert_eq!(m.stats[2].sds, 1);
    }

    #[test]
    fn winning_teammates_share_first() {
        let data = crate::test_data();
        let slot = |i: usize| Slot {
            dev: None,
            fighter: 0,
            cpu: Some(1),
            team: Some((i % 2) as u8),
            tap_jump: true,
            controls: Default::default(),
            tag: "CPU".into(),
            color: i,
        };
        let rules = Rules { stocks: 1, stage: "arena".into(), friendly_fire: false, time_limit: None };
        let mut m = Match::new(&data, Mode::Versus, (0..4).map(slot).collect(), rules, 7).unwrap();
        m.world.winner = Some(0);
        (m.stats[2].out_at, m.stats[1].out_at, m.stats[3].out_at) = (Some(50), Some(10), Some(90));
        assert_eq!(m.placements(), [1, 4, 1, 3]);
    }

    #[test]
    fn cpu_match_tracks_stats_and_placements() {
        let data = crate::test_data();
        let slot = |i: usize| Slot {
            dev: None,
            fighter: i % data.fighters.len(),
            cpu: Some(2),
            team: None,
            tap_jump: true,
            controls: Default::default(),
            tag: "CPU".into(),
            color: i,
        };
        let rules = Rules { stocks: 1, stage: "arena".into(), friendly_fire: false, time_limit: None };
        let mut m = Match::new(&data, Mode::Versus, (0..3).map(slot).collect(), rules, 7).unwrap();
        let (mut devs, mut scene) = (Devices::default(), Scene::new(data.clone()));
        while !m.world.over && m.world.frame < 60 * 60 * 10 {
            m.tick(&mut devs, &mut scene, true);
        }
        assert!(m.world.over, "match should end");
        let place = m.placements();
        let win = m.world.winner.expect("a winner");
        assert_eq!(place[win], 1);
        let mut sorted = place.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, [1, 2, 3]);
        let falls: u32 = m.stats.iter().map(|s| s.falls).sum();
        let kos: u32 = m.stats.iter().map(|s| s.kos + s.sds).sum();
        assert_eq!(falls, 2);
        assert_eq!(kos, falls, "every fall is a KO or a self-destruct");
        assert!(m.stats.iter().map(|s| s.dealt).sum::<f64>() > 0.);
        assert!(m.stats[win].out_at.is_none());
        let encoded = serde_json::to_vec(m.recording.as_ref().unwrap()).unwrap();
        let replay = serde_json::from_slice(&encoded).unwrap();
        let mut playback = crate::replay::Playback::new(replay).unwrap();
        let mut replayed = m.world.clone();
        playback.seek(m.world.frame, &mut replayed);
        assert_eq!(playback.warning, None);
        assert_eq!(replayed, m.world);
        m.recording_done = true;
        let ticks = m.recording.as_ref().unwrap().ticks;
        let frame = m.world.frame;
        let mut app = App::new();
        app.insert_resource(m)
            .insert_resource(devs)
            .insert_resource(scene)
            .insert_resource(State::new(Screen::Results))
            .add_systems(Update, tick);
        app.update();
        let mut m = app.world_mut().remove_resource::<Match>().unwrap();
        assert!(m.world.frame > frame);
        assert_eq!(m.recording.as_ref().unwrap().ticks, ticks);
        m.world.fighters[0].history.held[0] = true;
        m.world.fighters[0].history.last_press[0] = m.world.frame as i64;
        m.world.fighters[0].buffer.cmd = Some(sim::input::BufferedPress {
            tick: m.world.frame,
            intent: sim::input::Intent { atk: true, ..default() },
            held: m.world.frame,
        });
        m.clear_input(&mut Devices::default());
        assert_eq!(m.world.fighters[0].history, default());
        assert_eq!(m.world.fighters[0].buffer, default());
    }
}
