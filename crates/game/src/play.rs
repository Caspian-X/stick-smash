//! The running world (a match, training, the title-screen attract match or the character-select showcase):
//! ticking with slow-motion and training speed, per-device input, pause, match stats and the end of a match.

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
    pub debug: bool,
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
    pub fn new(base: &Arc<GameData>, mode: Mode, slots: Vec<Slot>, rules: Rules, seed: u32) -> Self {
        let mut data = (**base).clone();
        data.global.palette =
            slots.iter().map(|s| base.global.palette[s.color % base.global.palette.len()].clone()).collect();
        let data = Arc::new(data);
        let cfg = MatchConfig {
            seed,
            stage: rules.stage.clone(),
            stocks: rules.stocks,
            training: matches!(mode, Mode::Training | Mode::Showcase),
            friendly_fire: rules.friendly_fire,
            players: slots
                .iter()
                .map(|s| PlayerConfig {
                    fighter: base.fighters[s.fighter].id.clone(),
                    cpu: s.cpu,
                    team: s.team,
                    tap_jump: s.tap_jump,
                })
                .collect(),
        };
        let mut world = SimWorld::new(cfg, &data);
        if mode == Mode::Showcase {
            world.count = 0;
        }
        Match {
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
            debug: mode == Mode::Training,
        }
    }

    /// Same players and rules, fresh world.
    pub fn restart(&mut self, base: &Arc<GameData>, seed: u32) {
        let mut m = Match::new(base, self.mode, self.slots.clone(), self.rules.clone(), seed);
        m.dummy = self.dummy.clone();
        m.debug = self.debug;
        m.speed = self.speed;
        *self = m;
        self.apply_dummy();
        if self.mode == Mode::Training && self.world.fighters.len() > 1 {
            self.world.fighters[1].dmg = self.dummy.reset_pct;
        }
    }

    pub fn apply_dummy(&mut self) {
        if self.mode == Mode::Training && self.world.cfg.players.len() > 1 {
            self.world.cfg.players[1].cpu = (self.dummy.mode == DummyMode::Cpu).then_some(self.dummy.level);
        }
    }

    pub fn attract(base: &Arc<GameData>, seed: u32) -> Self {
        let n = base.fighters.len();
        let players = 4.min(base.stages[0].spawns.len());
        let slots = (0..players)
            .map(|i| Slot {
                dev: None,
                fighter: (i + seed as usize) % n,
                cpu: Some(2),
                team: None,
                tap_jump: true,
                tag: "CPU".into(),
                color: i,
            })
            .collect();
        let rules =
            Rules { stocks: 2, stage: base.stages[seed as usize % base.stages.len()].id.clone(), friendly_fire: false };
        Match::new(base, Mode::Attract, slots, rules, seed)
    }

    fn inputs(&self, devs: &mut Devices) -> Vec<Input> {
        let w = &self.world;
        (0..w.fighters.len())
            .map(|i| {
                if self.mode == Mode::Training && i == 1 {
                    let f = &w.fighters[1];
                    return match self.dummy.mode {
                        DummyMode::Shield => Input { stick: (0, 0), buttons: 16 },
                        DummyMode::Jump if f.state == FState::Idle && f.t > 20 => Input { stick: (0, 0), buttons: 1 },
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

    fn tick(&mut self, devs: &mut Devices, scene: &mut Scene) {
        let inputs =
            if self.world.over { vec![Input::default(); self.world.fighters.len()] } else { self.inputs(devs) };
        let before: Vec<(f64, i32)> = self.world.fighters.iter().map(|f| (f.dmg, f.stocks)).collect();
        step_world(&mut self.world, &self.data, &inputs);
        self.track(&before);
        if self.world.end_t > 0 {
            self.end_t += 1;
        }
        if self.world.over {
            self.over_t += 1;
        }
        scene.push(&self.world);
        self.world.fx.clear();
    }

    /// KOs, falls, self-destructs and damage, credited from damage and stock changes plus the sim's hit trace.
    fn track(&mut self, before: &[(f64, i32)]) {
        let w = &self.world;
        for (i, f) in w.fighters.iter().enumerate() {
            let (dmg0, stocks0) = before[i];
            if f.dmg > dmg0 {
                let by = w
                    .trace
                    .as_ref()
                    .filter(|t| t.defender == i && t.hit_frame == w.frame)
                    .map(|t| t.attacker)
                    .or(f.sd.by)
                    .filter(|b| *b != i);
                self.stats[i].taken += f.dmg - dmg0;
                if let Some(b) = by {
                    self.stats[b].dealt += f.dmg - dmg0;
                    self.stats[i].last_hit = Some((b, w.frame));
                }
            }
            if f.stocks < stocks0 {
                let s = &mut self.stats[i];
                s.falls += 1;
                match s.last_hit.take().filter(|(_, t)| w.frame - t < 60 * 8) {
                    Some((b, _)) => self.stats[b].kos += 1,
                    None => self.stats[i].sds += 1,
                }
                if f.stocks <= 0 {
                    self.stats[i].out_at = Some(w.frame);
                }
            }
        }
    }

    /// Placement (1-based) per player: the winner (and teammates) first, then by how long they lasted.
    pub fn placements(&self) -> Vec<usize> {
        let w = &self.world;
        let team_of = |i: usize| w.cfg.players[i].team;
        let won =
            |i: usize| w.winner.is_some_and(|win| win == i || team_of(win).is_some() && team_of(win) == team_of(i));
        let score = |i: usize| -> (bool, u64, i32, i64) {
            let f = &w.fighters[i];
            (won(i), self.stats[i].out_at.unwrap_or(u64::MAX), f.stocks, -(f.dmg as i64))
        };
        (0..w.fighters.len()).map(|i| 1 + (0..w.fighters.len()).filter(|&j| score(j) > score(i)).count()).collect()
    }

    pub fn alpha(&self, overstep: f64) -> f64 {
        (self.acc + overstep * self.tick_speed()).clamp(0., 1.)
    }

    fn tick_speed(&self) -> f64 {
        let slow = &self.data.global.render.hud.end_slowmo;
        let mut s = if self.mode == Mode::Training { self.speed } else { 1. };
        if self.world.end_t > 0 && self.end_t < slow.ticks {
            s *= slow.speed;
        }
        s
    }
}

/// Steps the world at `sim::TICK_HZ` (scaled by slow motion / training speed).
pub fn tick(mut m: ResMut<Match>, mut devs: ResMut<Devices>, mut scene: ResMut<Scene>) {
    let m = &mut *m;
    let ff = std::mem::take(&mut m.ff);
    for _ in 0..ff {
        m.tick(&mut devs, &mut scene);
    }
    if m.paused.is_some() {
        return;
    }
    if m.mode == Mode::Training && m.frozen {
        if std::mem::take(&mut m.step) {
            m.tick(&mut devs, &mut scene);
        }
        return;
    }
    m.acc += m.tick_speed();
    while m.acc >= 1. {
        m.acc -= 1.;
        m.tick(&mut devs, &mut scene);
    }
}

pub fn interpolate(time: Res<Time<Fixed>>, m: Res<Match>, mut scene: ResMut<Scene>) {
    if m.paused.is_none() && !(m.mode == Mode::Training && m.frozen) {
        scene.alpha = m.alpha(time.overstep_fraction_f64());
    }
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
        install(&mut commands, &mut scene, Match::attract(&data.0, seed));
    }
}

/// Pause on Start from a player's device; the pause menu takes over from there. Also the F1 debug toggle and
/// the move to the results screen.
pub fn match_flow(
    mut m: ResMut<Match>,
    devs: Res<Devices>,
    keys: Res<ButtonInput<KeyCode>>,
    mut scene: ResMut<Scene>,
    mut next: ResMut<NextState<Screen>>,
) {
    if keys.just_pressed(KeyCode::F1) {
        m.debug = !m.debug;
    }
    scene.debug = m.debug;
    if m.mode == Mode::Training && m.dummy.mode == DummyMode::Human && m.dummy.dev.is_none() {
        let p1 = m.slots[0].dev;
        if let Some((d, _)) = devs.map.iter().find(|(d, s)| Some(**d) != p1 && s.any_press()) {
            m.dummy.dev = Some(*d);
        }
    }
    if m.paused.is_none() && m.world.end_t == 0 && !m.world.over {
        let human = |d: Dev| m.slots.iter().any(|s| s.dev == Some(d)) || m.dummy.dev == Some(d);
        if let Some((d, _)) = devs.map.iter().find(|(d, s)| s.pressed[Act::Start as usize] && human(**d)) {
            m.paused = Some(*d);
        }
    }
    if m.mode == Mode::Versus && m.world.over && m.over_t >= m.data.global.render.hud.results_delay {
        next.set(Screen::Results);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_match_tracks_stats_and_placements() {
        let (data, errors) = crate::load_data(&[std::path::PathBuf::from("../../assets")]);
        assert!(errors.is_empty(), "{errors:?}");
        let data = Arc::new(data);
        let slot = |i: usize| Slot {
            dev: None,
            fighter: i % data.fighters.len(),
            cpu: Some(2),
            team: None,
            tap_jump: true,
            tag: "CPU".into(),
            color: i,
        };
        let rules = Rules { stocks: 1, stage: "arena".into(), friendly_fire: false };
        let mut m = Match::new(&data, Mode::Versus, (0..3).map(slot).collect(), rules, 7);
        let (mut devs, mut scene) = (Devices::default(), Scene::new(data.clone()));
        while !m.world.over && m.world.frame < 60 * 60 * 10 {
            m.tick(&mut devs, &mut scene);
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
    }
}
