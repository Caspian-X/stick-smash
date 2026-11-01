//! Independent screenshot sections: `--screenshot <dir> [--shots menus,match]`.
//! Each section resets through title before injecting its setup. Navigation still uses real bot input.

use std::{path::PathBuf, time::Instant};

use bevy::prelude::*;
use bevy::render::view::screenshot::{Capturing, Screenshot, save_to_disk};
use render::Framing;
use sim::world::State as FState;

use crate::input::{Act, Act::*, Dev, Devices};
use crate::play::{self, Match, Mode, Rules, Slot};
use crate::select::Select;
use crate::{Data, Screen};
use editor::{Cmd, Dialog};

/// Editor tour frame on the swordfighter's forward smash.
const SHOT_FRAME: usize = 16;

type Pred = fn(&Match, &Framing, Screen) -> bool;

pub enum Step {
    Home,
    Reset,
    OpenEditor,
    Select(Mode, usize),
    Match(i32),
    /// Simulate recorded game ticks in bounded batches, stopping at the exact predicate.
    Fast(Pred, u32),
    Set(u8, Act, bool),
    Wait(u32),
    /// Run the match fast until the condition holds (or `max` ticks pass).
    Until(Pred, u32),
    Shot(&'static str),
    /// Scripted editor command.
    Editor(editor::Cmd),
    /// Show this stage behind the select cards.
    Stage(&'static str),
    /// Start a four-CPU versus match on this stage.
    Versus(&'static str),
    /// Swap in content with the built-in fighters and stages repeated up to this many each (until the section ends).
    Big(usize),
    /// Wait until the stage roulette has run this many seconds.
    Spin(f32),
}

type Entry = (&'static str, fn() -> Vec<Step>);

/// Every section in full-tour order; `--shots` picks a subset in the order given.
const TOUR: [Entry; 9] = [
    ("menus", menus),
    ("fighter-editor", fighter_editor),
    ("stage-editor", stage_editor),
    ("select", select),
    ("roster", roster),
    ("match", match_shots),
    ("results-replay", results_replay),
    ("training", training),
    ("stages", stages),
];

pub fn section_names() -> String {
    TOUR.map(|(n, _)| n).join(", ")
}

struct Section {
    name: &'static str,
    steps: Vec<Step>,
}

#[derive(Resource)]
pub struct Script {
    dir: PathBuf,
    sections: Vec<Section>,
    section: usize,
    at: usize,
    wait: u32,
    budget: u32,
    capturing: bool,
    started: Instant,
    failures: Vec<&'static str>,
    /// Content before a [`Step::Big`], put back on reset.
    original: Option<std::sync::Arc<sim::GameData>>,
}

impl Section {
    fn new(name: &'static str, mut steps: Vec<Step>) -> Self {
        let mut setup = vec![Step::Home, Step::Wait(4), Step::Reset, Step::Wait(4)];
        setup.append(&mut steps);
        Self { name, steps: setup }
    }
}

fn ed(v: &mut Vec<Step>, c: editor::Cmd) {
    v.push(Step::Editor(c));
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
        Screen::Stages => |_, _, s| s == Screen::Stages,
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

fn menus() -> Vec<Step> {
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
    v
}

fn fighter_editor() -> Vec<Step> {
    let mut v = Vec::new();
    v.extend([Step::OpenEditor, Step::Wait(20)]);
    {
        use editor::preview::{Section, Subject};
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
    }
    v
}

fn stage_editor() -> Vec<Step> {
    let mut v = Vec::new();
    v.extend([Step::OpenEditor, Step::Wait(20)]);
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
    v
}

fn select() -> Vec<Step> {
    let mut v = Vec::new();
    // Versus: bot 0 opens it (and joins), bot 1 joins. Chips start on their fighters' roster tiles.
    press(&mut v, 0, Attack);
    press(&mut v, 1, Attack);
    // Bot 0: down to "Add CPU", add two, then Special takes its chip home to the roster.
    press(&mut v, 0, Down);
    press(&mut v, 0, Attack);
    press(&mut v, 0, Attack);
    press(&mut v, 0, Special);
    // Both chips on one tile; bot 1 opens its card options.
    press(&mut v, 0, Right);
    press(&mut v, 1, Grab);
    press(&mut v, 1, Down);
    shot(&mut v, "04-select-choosing");
    // Bot 1 closes the options, locks in on the random tile; bot 0 locks in its tile.
    press(&mut v, 1, Special);
    press(&mut v, 1, Right);
    press(&mut v, 1, Right);
    press(&mut v, 1, Attack);
    v.push(Step::Wait(40));
    press(&mut v, 0, Attack);
    v.push(Step::Wait(14));
    shot(&mut v, "05-select-ready");
    // Start: on to the stage vote. Bot 0 votes for the stage its chip starts on, bot 1 moves on.
    press(&mut v, 0, Start);
    v.push(Step::Until(screen_is(Screen::Stages), 10));
    v.push(Step::Wait(10));
    press(&mut v, 0, Attack);
    press(&mut v, 1, Right);
    shot(&mut v, "05b-stage-votes");
    // Bot 1's vote completes the votes: the roulette runs, lands, and the match starts.
    press(&mut v, 1, Attack);
    v.push(Step::Spin(0.9));
    v.push(Step::Shot("05c-stage-roulette"));
    v.push(Step::Spin(2.5));
    v.push(Step::Shot("05d-stage-landed"));
    v.push(Step::Until(screen_is(Screen::Match), 300));
    // Keep title → select → match navigation covered by real button presses.
    v.extend([Step::Select(Mode::Versus, 8), Step::Wait(20)]);
    shot(&mut v, "15-select-eight");
    // Up from the roster reaches the rules: CPU cards gain a Team row.
    press(&mut v, 0, Up);
    press(&mut v, 0, Attack);
    shot(&mut v, "15b-select-teams");
    v
}

/// A thirty-fighter, thirty-stage roster (the built-ins repeated): the grids shrink and wrap into rows.
fn roster() -> Vec<Step> {
    let mut v = vec![Step::Big(30), Step::Wait(2), Step::Select(Mode::Versus, 4), Step::Wait(20)];
    press(&mut v, 0, Down);
    press(&mut v, 0, Down);
    press(&mut v, 0, Left);
    shot(&mut v, "15c-select-roster-30");
    press(&mut v, 0, Attack);
    press(&mut v, 0, Start);
    v.push(Step::Until(screen_is(Screen::Stages), 10));
    v.push(Step::Wait(10));
    press(&mut v, 0, Down);
    press(&mut v, 0, Right);
    press(&mut v, 0, Right);
    shot(&mut v, "15d-stage-select-30");
    v
}

fn match_shots() -> Vec<Step> {
    let mut v = Vec::new();
    v.extend([Step::Match(3), Step::Wait(70)]);
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
    v.push(Step::Fast(|m, _, _| m.world.end_t > 0, 60 * 60 * 6));
    v.push(Step::Wait(20));
    v.push(Step::Shot("11-game"));
    v
}

fn results_replay() -> Vec<Step> {
    let mut v = vec![
        Step::Match(1),
        Step::Fast(|m, _, _| m.world.over && m.over_t >= m.data.global.render.hud.results_delay, 60 * 60 * 6),
        Step::Until(screen_is(Screen::Results), 30),
        Step::Wait(30),
        Step::Shot("12-results"),
    ];
    for _ in 0..4 {
        press(&mut v, 0, Right);
    }
    press(&mut v, 0, Attack);
    v.push(Step::Until(screen_is(Screen::Playback), 30));
    v.push(Step::Fast(|m, _, _| m.playback.as_ref().is_some_and(|p| p.position >= 600), 2000));
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
    v
}

fn training() -> Vec<Step> {
    let mut v = Vec::new();
    v.extend([Step::Select(Mode::Training, 2), Step::Wait(20)]);
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
    v
}

fn stages() -> Vec<Step> {
    let mut v = Vec::new();
    v.extend([Step::Select(Mode::Versus, 8), Step::Wait(20)]);
    // The other built-in stages: behind the eight select cards, then a CPU match on each.
    const STAGES: [&str; 3] = ["citadel", "isles", "foundry"];
    for (i, id) in STAGES.into_iter().enumerate() {
        v.push(Step::Stage(id));
        v.push(Step::Wait(20));
        v.push(Step::Shot(["16a-select-citadel", "16b-select-isles", "16c-select-foundry"][i]));
    }
    for (i, id) in STAGES.into_iter().enumerate() {
        v.push(Step::Versus(id));
        v.push(Step::Until(|m, _, _| m.world.go_t > 0, 400));
        v.push(Step::Until(|m, _, _| m.world.fighters.iter().map(|f| f.dmg).sum::<f64>() > 150., 4000));
        shot(&mut v, ["17a-match-citadel", "17b-match-isles", "17c-match-foundry"][i]);
        if id == "isles" {
            // Fighters on opposite islands: the zoomed-out camera still frames both.
            v.push(Step::Until(
                |m, _, _| {
                    let xs = m.world.fighters.iter().filter(|f| f.state != FState::Dead).map(|f| f.x);
                    xs.clone().fold(f64::MIN, f64::max) - xs.fold(f64::MAX, f64::min) > 950.
                },
                8000,
            ));
            shot(&mut v, "17b2-match-isles-wide");
        }
    }
    v
}

impl Script {
    pub fn tour(dir: PathBuf, filter: Option<&str>) -> Result<Self, String> {
        let names: Vec<_> = filter.map_or_else(|| TOUR.map(|(n, _)| n).to_vec(), |f| f.split(',').collect());
        let sections = names
            .into_iter()
            .map(|name| match TOUR.iter().find(|(n, _)| *n == name) {
                Some(&(name, steps)) => Ok(Section::new(name, steps())),
                None => Err(format!("unknown screenshot section {name:?}; sections: {}", section_names())),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Script {
            dir,
            sections,
            section: 0,
            at: 0,
            wait: 0,
            budget: 0,
            capturing: false,
            started: Instant::now(),
            failures: vec![],
            original: None,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    mut commands: Commands,
    mut s: ResMut<Script>,
    mut devs: ResMut<Devices>,
    mut m: Option<ResMut<Match>>,
    fr: Res<Framing>,
    screen: Res<State<Screen>>,
    capturing: Query<(), With<Capturing>>,
    mut exit: MessageWriter<AppExit>,
    mut inbox: ResMut<editor::Inbox>,
    (sel, clock): (Option<ResMut<Select>>, Option<Res<crate::stage_select::Clock>>),
    data: Res<Data>,
    mut scene: ResMut<render::Scene>,
    mut next: ResMut<NextState<Screen>>,
    mut egui: Query<&mut bevy_egui::EguiContext>,
) {
    for b in 0..2 {
        devs.bot(b);
    }
    let Some(current) = s.sections.get(s.section) else {
        if s.failures.is_empty() {
            info!("screenshot tour passed");
            exit.write(AppExit::Success);
        } else {
            error!("screenshot sections failed: {}", s.failures.join(", "));
            exit.write(AppExit::error());
        }
        return;
    };
    let name = current.name;
    if s.started.elapsed().as_secs() >= 120 {
        error!("screenshot section {name}: timed out at step {}", s.at);
        s.failures.push(name);
        s.at = s.sections[s.section].steps.len();
        s.capturing = false;
        s.wait = 0;
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
    let Some(step) = s.sections[s.section].steps.get(s.at) else {
        let missing = s.sections[s.section].steps.iter().find_map(|step| {
            let Step::Shot(shot) = step else { return None };
            let path = s.dir.join(format!("{shot}.png"));
            (!path.metadata().is_ok_and(|m| m.len() > 0)).then_some(path)
        });
        if let Some(path) = missing {
            error!("screenshot section {name}: missing {}", path.display());
            if !s.failures.contains(&name) {
                s.failures.push(name);
            }
        }
        info!("screenshot section {name}: finished in {:.2}s", s.started.elapsed().as_secs_f64());
        s.section += 1;
        s.at = 0;
        s.budget = 0;
        s.started = Instant::now();
        return;
    };
    match step {
        Step::Home => {
            devs.bots.clear();
            devs.clear();
            next.set(Screen::Title);
            s.at += 1;
        }
        Step::Reset => {
            commands.remove_resource::<Match>();
            commands.remove_resource::<Select>();
            commands.remove_resource::<crate::select::SelectInit>();
            commands.remove_resource::<crate::replay::ReturnTo>();
            commands.remove_resource::<crate::editing::Testing>();
            commands.remove_resource::<editor::Editor>();
            commands.insert_resource(editor::Inbox::default());
            commands.insert_resource(editor::Outbox::default());
            commands.insert_resource(crate::settings::Saved::default());
            if let Some(o) = s.original.take() {
                commands.insert_resource(Data(o));
            }
            commands.insert_resource(crate::menu::MenuState {
                cursor: crate::ui::Cursor { key: 1, owner: None },
                ..default()
            });
            commands.insert_resource(crate::hud::PauseMenu::default());
            commands.insert_resource(crate::results::ResultsMenu::default());
            *devs = Devices::default();
            *scene = render::Scene::new(s.original.clone().unwrap_or_else(|| data.0.clone()));
            // Panel widths, scroll offsets and open headers would otherwise carry over from earlier sections.
            for mut ctx in &mut egui {
                ctx.get_mut().memory_mut(|m| {
                    m.data = default();
                    m.reset_areas();
                });
            }
            info!("screenshot section {name}: starting");
            s.at += 1;
        }
        Step::OpenEditor => {
            next.set(Screen::Editor);
            s.at += 1;
        }
        Step::Select(mode, count) => {
            let (mode, count) = (*mode, *count);
            // Select::enter builds the normal cards and showcase from preferences.
            let mut saved = crate::settings::Saved::default();
            saved.versus.cpus = vec![crate::settings::PlayerPref::default(); count.saturating_sub(1)];
            commands.insert_resource(saved);
            commands.insert_resource(crate::select::SelectInit { mode, dev: Some(Dev::Bot(0)) });
            next.set(Screen::Select);
            s.at += 1;
        }
        Step::Set(b, act, on) => {
            devs.bot(*b)[*act as usize] = *on;
            s.at += 1;
        }
        Step::Wait(n) => {
            s.wait = *n;
            s.at += 1;
        }
        Step::Until(pred, max) | Step::Fast(pred, max) => {
            let (pred, max, fast) = (*pred, *max, matches!(step, Step::Fast(..)));
            let mut done = m.as_ref().is_some_and(|m| pred(m, &fr, *screen.get()));
            if fast
                && !done
                && let Some(m) = m.as_deref_mut()
            {
                for _ in 0..1000.min(max.saturating_sub(s.budget)) {
                    m.tick(&mut devs, &mut scene, false);
                    s.budget += 1;
                    if pred(m, &fr, *screen.get()) {
                        done = true;
                        break;
                    }
                }
            }
            if fast && let Some(m) = m.as_deref() {
                scene.reset(&m.world);
            }
            if done {
                s.budget = 0;
                s.at += 1;
            } else if s.budget >= max {
                error!("screenshot section {name}: step {} gave up after {max} ticks", s.at);
                s.failures.push(name);
                s.at = s.sections[s.section].steps.len();
            } else if !fast {
                let n = if *screen.get() == Screen::Match
                    && m.as_ref().is_some_and(|m| m.paused.is_none() && m.world.end_t == 0)
                {
                    3
                } else {
                    0
                };
                if let Some(m) = m.as_deref_mut() {
                    m.ff += n;
                }
                s.budget += n.max(1);
            } else if m.is_none() {
                s.budget += 1;
            }
        }
        Step::Editor(c) => {
            inbox.0.push(c.clone());
            s.at += 1;
        }
        Step::Stage(id) => {
            if let Some(mut sel) = sel
                && let Some(i) = data.0.stages.iter().position(|st| st.id == *id)
            {
                sel.stage = i;
            }
            s.at += 1;
        }
        Step::Versus(_) | Step::Match(_) => {
            let (id, stocks) = match step {
                Step::Versus(id) => (*id, 3),
                Step::Match(stocks) => ("arena", *stocks),
                _ => unreachable!(),
            };
            let slots = (0..4)
                .map(|i| Slot {
                    dev: (i < 2).then_some(Dev::Bot(i as u8)),
                    fighter: i % data.0.fighters.len(),
                    cpu: Some(2),
                    team: None,
                    tap_jump: true,
                    tag: "CPU".into(),
                    color: i,
                })
                .collect();
            let rules = Rules { stocks, stage: id.into(), friendly_fire: false };
            match Match::new(&data.0, Mode::Versus, slots, rules, 7) {
                Ok(m) => {
                    // Otherwise the select screen would put its showcase back this frame.
                    commands.remove_resource::<Select>();
                    play::install(&mut commands, &mut scene, m);
                    next.set(Screen::Match);
                }
                Err(e) => {
                    error!("screenshot section {name}: match on {id}: {e}");
                    s.failures.push(name);
                    s.at = s.sections[s.section].steps.len();
                    return;
                }
            }
            s.at += 1;
        }
        Step::Big(n) => {
            let n = *n;
            let base = s.original.get_or_insert_with(|| data.0.clone()).clone();
            commands.insert_resource(Data(std::sync::Arc::new(big(&base, n))));
            s.at += 1;
        }
        Step::Spin(secs) => {
            let secs = *secs;
            if clock.is_some_and(|c| c.secs().is_some_and(|t| t >= secs)) {
                s.budget = 0;
                s.at += 1;
            } else if s.budget > 600 {
                error!("screenshot section {name}: the roulette never reached {secs}s");
                s.failures.push(name);
                s.at = s.sections[s.section].steps.len();
            } else {
                s.budget += 1;
            }
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

/// The built-in fighters and stages repeated (numbered copies) up to `n` each.
fn big(d: &sim::GameData, n: usize) -> sim::GameData {
    let mut b = d.clone();
    for k in d.fighters.len()..n {
        let mut f = d.fighters[k % d.fighters.len()].clone();
        (f.id, f.name) = (format!("{}-{k}", f.id), format!("{} {}", f.name, k / d.fighters.len() + 1));
        b.fighters.push(f);
    }
    for k in d.stages.len()..n {
        let mut s = d.stages[k % d.stages.len()].clone();
        (s.id, s.name) = (format!("{}-{k}", s.id), format!("{} {}", s.name, k / d.stages.len() + 1));
        b.stages.push(s);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_sections_and_reports_available_names() {
        let script = Script::tour(PathBuf::new(), Some("stages,stage-editor")).unwrap();
        assert_eq!(script.sections.iter().map(|s| s.name).collect::<Vec<_>>(), ["stages", "stage-editor"]);
        for (name, _) in &TOUR {
            let script = Script::tour(PathBuf::new(), Some(name)).unwrap();
            assert_eq!(script.sections.len(), 1);
            assert_eq!(script.sections[0].name, *name);
        }
        for filter in ["unknown", "menus,unknown", ""] {
            let error = Script::tour(PathBuf::new(), Some(filter)).err().unwrap();
            for (name, _) in TOUR {
                assert!(error.contains(name));
            }
        }
    }

    #[test]
    fn failed_section_continues_and_exits_with_error() {
        let (data, errors) = crate::load_data(&[PathBuf::from("../../assets")]);
        assert!(errors.is_empty());
        let data = std::sync::Arc::new(data);
        let mut script = Script::tour(PathBuf::new(), Some("menus")).unwrap();
        script.sections = vec![
            Section::new("fails", vec![Step::Until(|_, _, _| false, 1)]),
            Section::new("continues", vec![Step::Wait(0)]),
        ];
        let mut app = App::new();
        app.insert_resource(script)
            .insert_resource(Data(data.clone()))
            .insert_resource(render::Scene::new(data))
            .insert_resource(State::new(Screen::Title))
            .insert_resource(NextState::<Screen>::default())
            .init_resource::<Devices>()
            .init_resource::<Framing>()
            .init_resource::<editor::Inbox>()
            .add_systems(Update, run);
        for _ in 0..100 {
            app.update();
        }
        let script = app.world().resource::<Script>();
        assert_eq!(script.section, 2);
        assert_eq!(script.failures, ["fails"]);
        assert!(app.world_mut().resource_mut::<Messages<AppExit>>().drain().any(|exit| !exit.is_success()));
    }

    #[test]
    fn preserves_screenshot_names() {
        let script = Script::tour(PathBuf::new(), None).unwrap();
        let mut names: Vec<_> = script
            .sections
            .iter()
            .flat_map(|s| &s.steps)
            .filter_map(|s| if let Step::Shot(name) = s { Some(*name) } else { None })
            .collect();
        names.sort_unstable();
        let expected = "01-title
02-settings
03-controls
03b-editor-move
03b2-editor-onion
03b3-editor-problem
03b4-editor-leave
03b4b-editor-toast
03b5-editor-help
03b6-editor-attachments
03b7-editor-projectiles
03c-editor-pose
03d-editor-new-fighter
03e-editor-stats
03f-stage-overview
03g-stage-platform
03h-stage-spawns
03i-stage-bounds
03j-stage-art
03k-stage-background
03l-stage-sandbox
03m-stage-new
03n-stage-help
03o-stage-blocks
03p-stage-moving
03q-stage-moving-sandbox
04-select-choosing
05-select-ready
05b-stage-votes
05c-stage-roulette
05d-stage-landed
06-countdown
07-go
08-hud-offscreen
09-hud
10-pause
11-game
12-results
12b-replay-controls
12c-replay-seek
12d-results-return
13-training-select
14-training
15-select-eight
15b-select-teams
15c-select-roster-30
15d-stage-select-30
16a-select-citadel
16b-select-isles
16c-select-foundry
17a-match-citadel
17b-match-isles
17b2-match-isles-wide
17c-match-foundry";
        assert_eq!(names, expected.lines().collect::<Vec<_>>());
    }
}
