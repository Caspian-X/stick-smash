//! `--screenshot <dir>`: a scripted tour through the real flow, driven by bot devices pressing buttons like
//! players would (title, settings, controls, a 4-player versus match with CPUs, pause, results, training).
//! Matches fast-forward between shots. Saves PNGs, then exits.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Capturing, Screenshot, save_to_disk};
use render::Framing;
use sim::world::State as FState;

use crate::Screen;
use crate::input::{Act, Devices};
use crate::play::Match;

/// Editor tour frame on the swordfighter's forward smash.
const SHOT_FRAME: usize = 16;

type Pred = fn(&Match, &Framing, Screen) -> bool;

pub enum Step {
    Set(u8, Act, bool),
    Wait(u32),
    /// Run the match fast until the condition holds (or `max` ticks pass).
    Until(Pred, u32),
    Shot(&'static str),
    /// Scripted editor command.
    Editor(editor::Cmd),
}

#[derive(Resource)]
pub struct Script {
    dir: PathBuf,
    steps: Vec<Step>,
    at: usize,
    wait: u32,
    budget: u32,
    capturing: bool,
}

fn press(v: &mut Vec<Step>, bot: u8, act: Act) {
    v.extend([Step::Set(bot, act, true), Step::Wait(3), Step::Set(bot, act, false), Step::Wait(4)]);
}

fn shot(v: &mut Vec<Step>, name: &'static str) {
    v.extend([Step::Wait(6), Step::Shot(name)]);
}

fn screen_is(s: Screen) -> Pred {
    match s {
        Screen::Results => |_, _, s| s == Screen::Results,
        Screen::Match => |_, _, s| s == Screen::Match,
        Screen::Select => |_, _, s| s == Screen::Select,
        Screen::Playback => |_, _, s| s == Screen::Playback,
        Screen::Editor => |_, _, s| s == Screen::Editor,
        _ => |_, _, s| s == Screen::Title,
    }
}

fn offscreen(m: &Match, fr: &Framing, _: Screen) -> bool {
    let [x0, y0, x1, _] = fr.view;
    let w = &m.world;
    w.frame > 400
        && w.fighters
            .iter()
            .any(|f| f.state == FState::Hitstun && (f.x < x0 - 30. || f.x > x1 + 30. || f.y - 40. < y0 - 30.))
        && w.fighters.iter().filter(|f| f.dmg > 20.).count() >= 2
}

impl Script {
    pub fn tour(dir: PathBuf) -> Self {
        use Act::*;
        let mut v = vec![Step::Wait(40)];
        shot(&mut v, "01-title");
        for _ in 0..4 {
            press(&mut v, 0, Down);
        }
        press(&mut v, 0, Attack);
        shot(&mut v, "02-settings");
        press(&mut v, 0, Special);
        for _ in 0..3 {
            press(&mut v, 0, Down);
        }
        press(&mut v, 0, Attack);
        press(&mut v, 0, Down);
        press(&mut v, 0, Down);
        shot(&mut v, "03-controls");
        press(&mut v, 0, Special);
        // Editor: a move with a dummy and a selected hitbox, a pose, the stats.
        press(&mut v, 0, Down);
        press(&mut v, 0, Down);
        press(&mut v, 0, Attack);
        v.push(Step::Wait(20));
        {
            use editor::preview::{Section, Subject};
            use editor::{Cmd, Dialog};
            let ed = |v: &mut Vec<Step>, c: Cmd| v.push(Step::Editor(c));
            ed(&mut v, Cmd::Fighter("swordfighter".into()));
            ed(&mut v, Cmd::Subject(Subject::Move("fsmash".into())));
            ed(&mut v, Cmd::Dummy(true));
            v.push(Step::Wait(4));
            ed(&mut v, Cmd::Frame(SHOT_FRAME));
            ed(&mut v, Cmd::Hitbox(Some(0)));
            shot(&mut v, "03b-editor-move");
            ed(&mut v, Cmd::Onion(true));
            ed(&mut v, Cmd::Frame(SHOT_FRAME - 8));
            ed(&mut v, Cmd::Hitbox(None));
            shot(&mut v, "03b2-editor-onion");
            ed(&mut v, Cmd::Onion(false));
            // A broken transition: inline problem, dirty markers, then the leave prompt.
            ed(
                &mut v,
                Cmd::Edit(|f| {
                    f.moves.fsmash.phases[0].transitions.push(sim::data::Transition {
                        on: sim::data::Trigger::Timeout,
                        cond: None,
                        from: None,
                        to: "nowhere".into(),
                        lag: None,
                    })
                }),
            );
            shot(&mut v, "03b3-editor-problem");
            ed(&mut v, Cmd::Back);
            shot(&mut v, "03b4-editor-leave");
            ed(&mut v, Cmd::Dismiss);
            ed(&mut v, Cmd::Undo);
            shot(&mut v, "03b4b-editor-toast");
            // Let the toast expire (it lives a few seconds) so the next shots show only what they're about.
            v.push(Step::Wait(800));
            ed(&mut v, Cmd::Help(true));
            shot(&mut v, "03b5-editor-help");
            ed(&mut v, Cmd::Help(false));
            ed(&mut v, Cmd::Subject(Subject::Data(Section::Attachments)));
            ed(&mut v, Cmd::Search("a".into()));
            shot(&mut v, "03b6-editor-attachments");
            ed(&mut v, Cmd::Search(String::new()));
            ed(&mut v, Cmd::Fighter("brawler".into()));
            ed(&mut v, Cmd::Subject(Subject::Data(Section::Projectiles)));
            shot(&mut v, "03b7-editor-projectiles");
            ed(&mut v, Cmd::Subject(Subject::Pose("stand".into())));
            shot(&mut v, "03c-editor-pose");
            ed(&mut v, Cmd::Subject(Subject::Fighter));
            ed(
                &mut v,
                Cmd::Dialog(Dialog::NewFighter {
                    id: "ninja".into(),
                    name: "Ninja".into(),
                    extends: "brawler".into(),
                    error: None,
                }),
            );
            shot(&mut v, "03d-editor-new-fighter");
            ed(&mut v, Cmd::Dismiss);
            shot(&mut v, "03e-editor-stats");
            // Stage editor: overview, a selected platform with grid and mirror, spawns with a note, art, backdrop,
            // the CPU sandbox, a new stage, the shortcut sheet.
            {
                use editor::stage_ed::Section as S;
                use editor::stage_view::Part;
                ed(&mut v, Cmd::Mode(editor::Mode::Stages));
                ed(&mut v, Cmd::Stage("arena".into()));
                shot(&mut v, "03f-stage-overview");
                ed(&mut v, Cmd::Grid(true));
                ed(&mut v, Cmd::Symmetry(true));
                ed(&mut v, Cmd::Part(Some(Part::Platform(0))));
                ed(
                    &mut v,
                    Cmd::EditStage(|s| {
                        s.platforms[0].y -= 30.;
                        s.platforms[1].y -= 30.;
                    }),
                );
                shot(&mut v, "03g-stage-platform");
                ed(&mut v, Cmd::Undo);
                ed(&mut v, Cmd::Grid(false));
                ed(&mut v, Cmd::Symmetry(false));
                ed(&mut v, Cmd::EditStage(|s| s.spawns.truncate(6)));
                ed(&mut v, Cmd::Part(Some(Part::Spawn(1))));
                shot(&mut v, "03h-stage-spawns");
                ed(&mut v, Cmd::Undo);
                v.push(Step::Wait(800));
                ed(&mut v, Cmd::Part(Some(Part::Blast)));
                shot(&mut v, "03i-stage-bounds");
                ed(&mut v, Cmd::Part(None));
                ed(&mut v, Cmd::Section(S::Art));
                shot(&mut v, "03j-stage-art");
                ed(&mut v, Cmd::Section(S::Background));
                shot(&mut v, "03k-stage-background");
                ed(&mut v, Cmd::Section(S::Overview));
                ed(&mut v, Cmd::Sandbox(3));
                v.push(Step::Wait(150));
                shot(&mut v, "03l-stage-sandbox");
                ed(&mut v, Cmd::Sandbox(0));
                ed(
                    &mut v,
                    Cmd::Dialog(Dialog::NewStage {
                        id: "sky-fortress".into(),
                        name: "Sky Fortress".into(),
                        from: "arena".into(),
                        error: None,
                    }),
                );
                shot(&mut v, "03m-stage-new");
                ed(&mut v, Cmd::Dismiss);
                ed(&mut v, Cmd::Help(true));
                shot(&mut v, "03n-stage-help");
                ed(&mut v, Cmd::Help(false));
                // Format 2: a raised centre between two blocks (inner ledges off) and a platform on a path.
                ed(
                    &mut v,
                    Cmd::EditStage(|s| {
                        let mut b = s.blocks[0].clone();
                        let (l, r) = (b.left, b.right);
                        let (a, z) = (l + 240., r - 240.);
                        (b.right, b.right_ledge) = (a, false);
                        let mut c = b.clone();
                        (c.left, c.right, c.top, c.left_ledge, c.right_ledge) = (a, z, b.top - 40., false, false);
                        let mut d = b.clone();
                        (d.left, d.right, d.left_ledge, d.right_ledge) = (z, r, false, true);
                        s.blocks = vec![b, c, d];
                        s.platforms[2].motion = Some(sim::data::stage::Motion {
                            waypoints: vec![
                                sim::data::stage::Waypoint { x: -170., y: 0., travel_ticks: 150, wait_ticks: 40 },
                                sim::data::stage::Waypoint { x: 170., y: 0., travel_ticks: 150, wait_ticks: 40 },
                            ],
                            mode: sim::data::stage::PathMode::PingPong,
                            phase: 0,
                        });
                    }),
                );
                ed(&mut v, Cmd::Symmetry(true));
                ed(&mut v, Cmd::Part(Some(Part::Block(1))));
                ed(&mut v, Cmd::Reframe);
                shot(&mut v, "03o-stage-blocks");
                ed(&mut v, Cmd::Symmetry(false));
                ed(&mut v, Cmd::Part(Some(Part::Waypoint(2, 1))));
                v.push(Step::Wait(90));
                shot(&mut v, "03p-stage-moving");
                ed(&mut v, Cmd::Sandbox(2));
                v.push(Step::Wait(200));
                shot(&mut v, "03q-stage-moving-sandbox");
                ed(&mut v, Cmd::Sandbox(0));
                ed(&mut v, Cmd::Undo);
                ed(&mut v, Cmd::Part(None));
                ed(&mut v, Cmd::Mode(editor::Mode::Fighters));
            }
            ed(&mut v, Cmd::Back);
            v.push(Step::Wait(10));
        }
        // Versus: bot 0 opens it (and joins), bot 1 joins.
        press(&mut v, 0, Attack);
        press(&mut v, 1, Attack);
        press(&mut v, 0, Right);
        press(&mut v, 1, Right);
        press(&mut v, 1, Right);
        // Bot 0: down to its footer, over to "Add CPU", add two.
        press(&mut v, 0, Down);
        press(&mut v, 0, Down);
        press(&mut v, 0, Right);
        press(&mut v, 0, Attack);
        press(&mut v, 0, Attack);
        // Left lands on the newest CPU's level: make it hard.
        press(&mut v, 0, Left);
        press(&mut v, 0, Right);
        shot(&mut v, "04-select-choosing");
        // Back to its own card, then stocks: up into the rules.
        press(&mut v, 0, Special);
        press(&mut v, 1, Attack);
        press(&mut v, 0, Attack);
        shot(&mut v, "05-select-ready");
        press(&mut v, 0, Start);
        v.push(Step::Until(screen_is(Screen::Match), 10));
        v.push(Step::Wait(70));
        shot(&mut v, "06-countdown");
        v.push(Step::Until(|m, _, _| m.world.go_t > 0 && m.world.go_t < 40, 400));
        shot(&mut v, "07-go");
        v.push(Step::Until(offscreen, 5000));
        v.push(Step::Shot("08-hud-offscreen"));
        v.push(Step::Until(|m, _, _| m.world.fighters.iter().map(|f| f.dmg).sum::<f64>() > 200., 4000));
        shot(&mut v, "09-hud");
        press(&mut v, 0, Start);
        press(&mut v, 0, Down);
        shot(&mut v, "10-pause");
        press(&mut v, 0, Start);
        v.push(Step::Until(|m, _, _| m.world.end_t > 0, 60 * 60 * 6));
        v.push(Step::Wait(20));
        v.push(Step::Shot("11-game"));
        v.push(Step::Until(screen_is(Screen::Results), 600));
        v.push(Step::Wait(30));
        v.push(Step::Shot("12-results"));
        for _ in 0..4 {
            press(&mut v, 0, Right);
        }
        press(&mut v, 0, Attack);
        v.push(Step::Until(screen_is(Screen::Playback), 30));
        v.push(Step::Until(|m, _, _| m.playback.as_ref().is_some_and(|p| p.position >= 600), 2000));
        press(&mut v, 0, Start);
        shot(&mut v, "12b-replay-controls");
        press(&mut v, 0, Right);
        press(&mut v, 0, Right);
        press(&mut v, 0, Left);
        press(&mut v, 0, Down);
        press(&mut v, 0, Right);
        shot(&mut v, "12c-replay-seek");
        press(&mut v, 0, Special);
        v.push(Step::Until(screen_is(Screen::Results), 30));
        shot(&mut v, "12d-results-return");
        press(&mut v, 0, Left);
        press(&mut v, 0, Left);
        press(&mut v, 0, Attack);
        // Training.
        v.push(Step::Until(screen_is(Screen::Title), 10));
        press(&mut v, 0, Down);
        press(&mut v, 0, Attack);
        press(&mut v, 0, Attack);
        shot(&mut v, "13-training-select");
        press(&mut v, 0, Start);
        v.push(Step::Until(screen_is(Screen::Match), 10));
        v.push(Step::Set(0, Right, true));
        v.push(Step::Until(|m, _, _| m.world.fighters[0].x > m.world.fighters[1].x - 60., 200));
        v.push(Step::Set(0, Right, false));
        press(&mut v, 0, Attack);
        v.push(Step::Until(|m, _, _| m.world.trace.as_ref().is_some_and(|t| t.done), 120));
        press(&mut v, 0, Special);
        v.push(Step::Until(
            |m, _, _| {
                m.world.fighters[0].state == FState::Attack
                    && m.world.fighters[0].mov.as_ref().is_some_and(|mv| mv.elapsed > 6)
            },
            60,
        ));
        shot(&mut v, "14-training");
        // Quit from the pause menu, then fill all eight slots.
        press(&mut v, 0, Start);
        for _ in 0..6 {
            press(&mut v, 0, Down);
        }
        press(&mut v, 0, Attack);
        v.push(Step::Until(screen_is(Screen::Title), 10));
        press(&mut v, 0, Attack);
        // Footer, then right past the two remembered CPUs' "Remove CPU" to "Add CPU".
        press(&mut v, 0, Down);
        press(&mut v, 0, Down);
        for _ in 0..3 {
            press(&mut v, 0, Right);
        }
        for _ in 0..5 {
            press(&mut v, 0, Attack);
        }
        shot(&mut v, "15-select-eight");
        Script { dir, steps: v, at: 0, wait: 0, budget: 0, capturing: false }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    mut commands: Commands,
    mut s: ResMut<Script>,
    mut devs: ResMut<Devices>,
    m: Option<ResMut<Match>>,
    fr: Res<Framing>,
    screen: Res<State<Screen>>,
    capturing: Query<(), With<Capturing>>,
    mut exit: MessageWriter<AppExit>,
    mut inbox: ResMut<editor::Inbox>,
) {
    for b in 0..2 {
        devs.bot(b);
    }
    if s.capturing {
        if capturing.is_empty() {
            s.capturing = false;
        } else {
            return;
        }
    }
    if s.wait > 0 {
        s.wait -= 1;
        return;
    }
    let Some(step) = s.steps.get(s.at) else {
        exit.write(AppExit::Success);
        return;
    };
    match step {
        Step::Set(b, act, on) => {
            devs.bot(*b)[*act as usize] = *on;
            s.at += 1;
        }
        Step::Wait(n) => {
            s.wait = *n;
            s.at += 1;
        }
        Step::Until(pred, max) => {
            let (pred, max) = (*pred, *max);
            let done = m.as_ref().is_some_and(|m| pred(m, &fr, *screen.get()));
            if done || s.budget >= max {
                if !done {
                    warn!("screenshot step {} gave up waiting", s.at);
                }
                s.budget = 0;
                s.at += 1;
            } else if let Some(mut m) = m {
                let n = if *screen.get() == Screen::Match && m.paused.is_none() && m.world.end_t == 0 { 3 } else { 0 };
                m.ff += n;
                s.budget += n.max(1);
            } else {
                s.budget += 1;
            }
        }
        Step::Editor(c) => {
            inbox.0.push(c.clone());
            s.at += 1;
        }
        Step::Shot(name) => {
            let path = s.dir.join(format!("{name}.png"));
            info!("screenshot {}", path.display());
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            s.capturing = true;
            s.wait = 2;
            s.at += 1;
        }
    }
}
