//! `--screenshot <dir>`: a CPU-vs-CPU match that saves PNGs at interesting ticks, then exits. The match is
//! simulated once up front to find the ticks (attacks, specials, projectiles, a KO, ...), then replayed while
//! rendering; the sim is deterministic, so both runs agree.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Capturing, Screenshot, save_to_disk};
use render::Scene;
use sim::world::State;
use sim::{Fx, GameData, Input, MatchConfig, PlayerConfig, World as SimWorld, step_world};

use crate::Match;

pub struct Shot {
    pub tick: u64,
    pub name: String,
    pub debug: bool,
}

pub enum Step {
    Advance,
    Settle,
    Capture,
}

#[derive(Resource)]
pub struct Shots {
    pub dir: PathBuf,
    pub list: Vec<Shot>,
    pub next: usize,
    pub wait: u32,
    pub step: Step,
}

const MAX_TICKS: u64 = 60 * 300;

fn config(d: &GameData) -> MatchConfig {
    let players = d
        .fighters
        .iter()
        .map(|f| PlayerConfig {
            cpu: Some(2),
            ..PlayerConfig::human(&f.id)
        })
        .collect();
    MatchConfig {
        seed: 7,
        stocks: 2,
        players,
        ..default()
    }
}

/// The starting world and the screenshots to take.
pub fn plan(d: &GameData) -> (SimWorld, Vec<Shot>) {
    let start = SimWorld::new(config(d), d);
    let mut w = start.clone();
    let mut list: Vec<Shot> = vec![];
    let add = |list: &mut Vec<Shot>, tick: u64, name: String, debug: bool| {
        if !list.iter().any(|s| s.name == name) {
            list.push(Shot { tick, name, debug });
        }
    };
    let inputs = vec![Input::default(); w.fighters.len()];
    let mut over_at = None;
    add(
        &mut list,
        u64::from(d.global.match_.countdown / 2),
        "00-countdown".into(),
        false,
    );
    while w.frame < MAX_TICKS {
        step_world(&mut w, d, &inputs);
        let t = w.frame;
        if w.fx.iter().any(|f| matches!(f, Fx::Beam { .. })) {
            let n = list.iter().filter(|s| s.name.starts_with("ko")).count();
            if n < 2 {
                add(&mut list, t + 3, format!("ko-{n}"), false);
                add(&mut list, t + 14, format!("ko-{n}-beam"), false);
            }
        }
        w.fx.clear();
        for p in &w.projs {
            if p.life > 0 && p.bounces == 0 {
                add(&mut list, t + 6, format!("proj-{}", p.id), false);
            }
        }
        for f in &w.fighters {
            let id = &d.fighters[f.data].id;
            match (f.state, &f.mov) {
                (State::Attack, Some(m)) => {
                    let active = !sim::combat::active_boxes(d, f).is_empty();
                    if active && !m.slot.ends_with("spec") && m.slot != "jab" && m.slot != "grab" {
                        add(&mut list, t, format!("attack-{id}"), false);
                        if list.iter().filter(|s| s.name.starts_with("attack")).count() >= 2 {
                            add(&mut list, t, format!("debug-{id}"), true);
                        }
                    }
                    if m.slot.ends_with("spec") && m.t == 4 {
                        add(&mut list, t, format!("special-{id}-{}", m.slot), false);
                    }
                    if m.charge > 10 {
                        add(&mut list, t, format!("charge-{id}"), false);
                    }
                }
                (State::Shield, _) if f.t == 8 => add(&mut list, t, "shield".into(), false),
                (State::Hitstun, _) if f.tumble && f.t == 8 => {
                    add(&mut list, t, "tumble".into(), false)
                }
                (State::Respawn, _) if f.t == 40 => add(&mut list, t, "respawn".into(), false),
                (State::Ledge, _) if f.t == 10 => add(&mut list, t, "ledge".into(), false),
                (State::Dizzy, _) if f.t == 30 => add(&mut list, t, "dizzy".into(), false),
                (State::Roll | State::Airdodge | State::Spot, _) if f.t == 6 => {
                    add(&mut list, t, "dodge".into(), false)
                }
                _ => {}
            }
        }
        if w.over && over_at.is_none() {
            over_at = Some(t);
            add(&mut list, t + 40, "zz-win".into(), false);
        }
        if over_at.is_some_and(|o| t > o + 60) {
            break;
        }
    }
    list.sort_by_key(|s| s.tick);
    (start, list)
}

pub fn run(
    mut commands: Commands,
    mut shots: ResMut<Shots>,
    mut m: ResMut<Match>,
    mut scene: ResMut<Scene>,
    capturing: Query<(), With<Capturing>>,
    mut exit: MessageWriter<AppExit>,
) {
    let s = &mut *shots;
    let Some(shot) = s.list.get(s.next) else {
        if capturing.is_empty() {
            exit.write(AppExit::Success);
        }
        return;
    };
    scene.alpha = 1.;
    match s.step {
        Step::Advance => {
            let Match { world, data, .. } = &mut *m;
            let inputs = vec![Input::default(); world.fighters.len()];
            while world.frame < shot.tick {
                step_world(world, data, &inputs);
                scene.push(world);
                world.fx.clear();
            }
            scene.debug = shot.debug;
            s.wait = 3;
            s.step = Step::Settle;
        }
        Step::Settle => {
            s.wait = s.wait.saturating_sub(1);
            if s.wait == 0 {
                let path = s.dir.join(format!("{:05}-{}.png", shot.tick, shot.name));
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
                s.wait = 2;
                s.step = Step::Capture;
            }
        }
        Step::Capture => {
            s.wait = s.wait.saturating_sub(1);
            if s.wait == 0 && capturing.is_empty() {
                s.next += 1;
                s.step = Step::Advance;
            }
        }
    }
}
