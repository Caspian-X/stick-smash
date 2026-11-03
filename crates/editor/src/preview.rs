//! Truthful preview: a sandbox `World` running the real sim with the fighter performing the subject from frame 0.
//! Every frame is kept, so scrubbing is an index and any data change re-simulates from the start.

use std::sync::Arc;

use sim::world::State;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, step_world};

/// What the editor is looking at.
#[derive(Clone, Debug, PartialEq)]
pub enum Subject {
    Move(String),
    /// Throw direction `f`, `b`, `u`, `d`.
    Throw(String),
    Pose(String),
    /// Fighter stats: the idle animation.
    Fighter,
    /// Another fighter-wide section (also previews the idle animation).
    Data(Section),
}

/// Fighter-wide data edited through schema forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Style,
    Attachments,
    Projectiles,
    Effects,
    Anims,
    Ai,
}

impl Section {
    pub const ALL: [Section; 6] =
        [Section::Style, Section::Attachments, Section::Projectiles, Section::Effects, Section::Anims, Section::Ai];
    pub fn name(self) -> &'static str {
        match self {
            Section::Style => "Style",
            Section::Attachments => "Attachments",
            Section::Projectiles => "Projectiles",
            Section::Effects => "Effects",
            Section::Anims => "Animations",
            Section::Ai => "CPU behavior",
        }
    }
    /// The fighter JSON key it edits.
    pub fn key(self) -> &'static str {
        <Self as crate::section::Section>::keys(self)[0]
    }
}

impl crate::section::Section for Section {
    const ALL: &'static [Self] = &Self::ALL;
    fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Style => &["style"],
            Self::Attachments => &["attachments"],
            Self::Projectiles => &["projectiles"],
            Self::Effects => &["fx"],
            Self::Anims => &["anims"],
            Self::Ai => &["ai"],
        }
    }
}

impl Subject {
    /// Plays the idle loop rather than a one-shot.
    pub fn idle(&self) -> bool {
        matches!(self, Subject::Fighter | Subject::Data(_))
    }
}

pub const AERIALS: [&str; 6] = ["nair", "fair", "bair", "uair", "dair", "zair"];

/// Sandbox options, shown in the timeline bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    /// Start airborne this high above the ground (`None`: grounded). Aerials default to airborne.
    pub air: Option<f64>,
    pub dummy: bool,
    pub dummy_dist: f64,
    pub dummy_pct: f64,
    /// Hold the move's button for this many frames (charge, `release` transitions).
    pub hold: u32,
}

impl Default for Setup {
    fn default() -> Self {
        Setup { air: None, dummy: false, dummy_dist: 70., dummy_pct: 0., hold: 0 }
    }
}

/// Per-frame summary of the edited fighter, for the timeline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    /// Phase index and phase frame while in the move.
    pub phase: Option<(usize, u32)>,
    pub state: State,
}

pub struct Run {
    pub data: Arc<GameData>,
    pub frames: Vec<World>,
    pub marks: Vec<Mark>,
}

const MAX_FRAMES: usize = 360;
const TAIL: usize = 8;

fn stage(d: &GameData, players: usize) -> Option<usize> {
    let ok = |s: &sim::data::Stage| s.max_players() >= players;
    d.stages.iter().position(|s| s.id == "arena" && ok(s)).or_else(|| d.stages.iter().position(ok))
}

pub fn simulate(data: Arc<GameData>, fighter: &str, subj: &Subject, setup: &Setup) -> Result<Run, String> {
    let d = &*data;
    let throw = matches!(subj, Subject::Throw(_));
    let dummy = throw || setup.dummy && matches!(subj, Subject::Move(_));
    let n = if dummy { 2 } else { 1 };
    let st = stage(d, n).ok_or("no stage fits the sandbox")?;
    let players = (0..n).map(|_| PlayerConfig::human(fighter)).collect();
    let cfg = MatchConfig::sandbox(&d.stages[st].id, players, 1);
    let mut w = World::new(cfg, d)?;
    let s = &d.stages[st];
    let (cx, top) = ((s.blocks[0].left + s.blocks[0].right) / 2., s.blocks[0].top);
    for (i, f) in w.fighters.iter_mut().enumerate() {
        f.x = if i == 0 { cx } else { cx + setup.dummy_dist.max(0.) };
        f.y = top;
        f.face = if i == 0 { 1. } else { -1. };
        f.ground = true;
        f.surf = Some(0);
        f.state = State::Idle;
        f.vx = 0.;
        f.vy = 0.;
    }
    if dummy {
        w.fighters[1].dmg = setup.dummy_pct;
    }
    if let Some(h) = setup.air.filter(|_| matches!(subj, Subject::Move(_))) {
        let f = &mut w.fighters[0];
        f.y = top - h.max(1.);
        f.ground = false;
        f.surf = None;
        f.state = State::Air;
    }
    let mut button = 2u8;
    match subj {
        Subject::Move(slot) => {
            let Some(mv) = d.fighter(fighter).and_then(|f| f.moves.get(slot)) else {
                return Err(format!("no move {slot}"));
            };
            if mv
                .phases
                .iter()
                .any(|p| p.charge.as_ref().is_some_and(|c| c.button == sim::data::moves::Button::Special))
                || slot.ends_with("spec")
            {
                button = 4;
            }
            let mut f = w.fighters[0].clone();
            sim::moves::start_move(&mut w, d, 0, &mut f, slot);
            w.fighters[0] = f;
        }
        Subject::Throw(_) => {
            sim::combat::start_hold(&mut w, d, 0, 1);
            w.fighters[0].t = d.global.grab.min_hold;
        }
        Subject::Pose(_) | Subject::Fighter | Subject::Data(_) => {}
    }
    let stick = match subj {
        Subject::Throw(k) => match k.as_str() {
            "b" => (-127, 0),
            "u" => (0, 127),
            "d" => (0, -127),
            _ => (127, 0),
        },
        _ => (0, 0),
    };
    let mark = |w: &World| {
        let f = &w.fighters[0];
        Mark { phase: f.mov.as_ref().filter(|_| f.state == State::Attack).map(|m| (m.phase, m.t)), state: f.state }
    };
    let mut frames = vec![w.clone()];
    let mut marks = vec![mark(&w)];
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
    let limit = match subj {
        Subject::Pose(_) => 1,
        Subject::Fighter | Subject::Data(_) => 120,
        _ => MAX_FRAMES,
    };
    let mut free_since: Option<usize> = None;
    let mut started = false;
    while frames.len() < limit {
        let k = frames.len() as u32;
        let mut inp = Input::default();
        if k <= setup.hold {
            inp.buttons = button;
        }
        if w.fighters[0].state == State::Hold {
            inp.stick = stick;
        }
        let mut ins = vec![inp];
        ins.resize(n, Input::default());
        step_world(&mut w, d, &ins);
        for f in &mut w.fighters {
            f.buffer = Default::default();
        }
        frames.push(w.clone());
        marks.push(mark(&w));
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        if subj.idle() {
            continue;
        }
        let f = &w.fighters[0];
        started |= !matches!(f.state, State::Idle | State::Air);
        let dummy_busy = dummy && matches!(w.fighters[1].state, State::Hitstun | State::Grabbed | State::Shieldbreak);
        let article_busy =
            w.projs.iter().any(|p| p.owner == 0 && d.fighters[p.fighter_data].projectiles[&p.id].article.is_some());
        if started && f.actionable(&d.global) && !dummy_busy && !article_busy {
            let at = *free_since.get_or_insert(frames.len());
            if frames.len() >= at + TAIL {
                break;
            }
        } else {
            free_since = None;
        }
    }
    Ok(Run { data, frames, marks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_move_and_throw_simulates_and_ends() {
        let files = crate::doc::tests_files();
        let (data, _) = sim::data::load(files.sources()).unwrap();
        let data = Arc::new(data);
        for f in &data.fighters {
            for (slot, mv) in f.moves.iter() {
                let air = AERIALS.contains(&slot).then_some(120.);
                let run =
                    simulate(data.clone(), &f.id, &Subject::Move(slot.into()), &Setup { air, ..Default::default() })
                        .unwrap();
                assert_eq!(run.marks[0].phase, Some((0, 0)), "{} {slot}", f.id);
                assert!(run.frames.len() > 2);
                if mv.phases.iter().all(|p| p.frames.is_some()) && slot != "uspec" {
                    assert!(run.frames.len() < MAX_FRAMES, "{} {slot} never ended", f.id);
                }
            }
            for k in ["f", "b", "u", "d"] {
                let run = simulate(data.clone(), &f.id, &Subject::Throw(k.into()), &Setup::default()).unwrap();
                assert!(run.marks.iter().any(|m| m.state == State::Throw), "{} throw {k}", f.id);
            }
        }
    }
}
