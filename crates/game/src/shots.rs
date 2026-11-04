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
    /// Navigate through every roster row and out to the neighboring controls using bot input.
    LeaveRoster(u8, Act),
    Match(i32),
    Training(&'static str),
    /// Freeze a state at an exact tick for animation boundary inspection.
    Animation(&'static str, FState, u32, f64),
    Interaction(u8),
    /// Expand Arena's camera to expose all four KO boundaries.
    WideCamera,
    Debug(bool),
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
    /// Tilt a bot's analog stick (y up; zero releases it).
    Stick(u8, Vec2),
    /// Let a bot's pointer go just off this target, as if its stick stopped there (the magnet pulls it on).
    PointAt(u8, u32),
    /// Put a bot's pointer at this screen position like a mouse (no magnet).
    Mouse(u8, Vec2),
}

type Entry = (&'static str, fn() -> Vec<Step>);

/// Every section in full-tour order; `--shots` picks a subset in the order given.
const TOUR: [Entry; 15] = [
    ("menus", menus),
    ("fighter-editor", fighter_editor),
    ("stage-editor", stage_editor),
    ("select", select),
    ("roster", roster),
    ("match", match_shots),
    ("results-replay", results_replay),
    ("training", training),
    ("crouch", crouch),
    ("blast-zones", blast_zones),
    ("fighter-kits", fighter_kits),
    ("stages", stages),
    ("interactions", interaction_shots),
    ("animations", animation_shots),
    ("plant", plant_shots),
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

fn sneak_projectile(m: &Match, _: &Framing, screen: Screen) -> bool {
    screen == Screen::Match && m.world.projs.iter().any(|p| p.id.starts_with("grenade") && p.age > 12)
}

fn sneak_missile(m: &Match, _: &Framing, screen: Screen) -> bool {
    screen == Screen::Match && m.world.projs.iter().any(|p| p.id == "nikita" && p.age > 10)
}

fn sneak_c4_blast(m: &Match, _: &Framing, screen: Screen) -> bool {
    screen == Screen::Match
        && !m.world.projs.iter().any(|p| p.id == "c4")
        && m.world.fighters[0].mov.as_ref().is_some_and(|m| m.slot == "dspec" && m.t >= 25)
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

fn interaction_shots() -> Vec<Step> {
    vec![
        Step::Interaction(3),
        Step::Wait(20),
        Step::Shot("24d-standard-hold"),
        Step::Interaction(0),
        Step::Wait(20),
        Step::Shot("24a-unequal-hold"),
        Step::Interaction(1),
        Step::Wait(20),
        Step::Shot("24b-unequal-ledge"),
        Step::Interaction(2),
        Step::Wait(20),
        Step::Shot("24c-unequal-climb"),
    ]
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
                    min_charge: 0,
                    needs: None,
                    to: "nowhere".into(),
                    lag: None,
                    preserve_hits: false,
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
        ed(
            &mut v,
            Cmd::Edit(|f| {
                f.rig =
                    Some(serde_json::from_str(include_str!("../../../assets/templates/rigs/humanoid.json")).unwrap());
            }),
        );
        ed(&mut v, Cmd::Subject(Subject::Data(Section::Rig)));
        shot(&mut v, "03e2-editor-humanoid-rig");
        ed(
            &mut v,
            Cmd::Edit(|f| {
                f.rig =
                    Some(serde_json::from_str(include_str!("../../../assets/templates/rigs/quadruped.json")).unwrap());
            }),
        );
        shot(&mut v, "03e3-editor-quadruped-rig");
        ed(
            &mut v,
            Cmd::Edit(|f| {
                let mut p = sim::data::visual::Pose::default();
                p.joints.insert("tailBase".into(), sim::data::rig::JointPose { angle: Some(65.), translation: None });
                f.poses.insert("rigTail".into(), p);
            }),
        );
        ed(&mut v, Cmd::Subject(Subject::Pose("rigTail".into())));
        shot(&mut v, "03e4-editor-named-pose-handles");
        ed(&mut v, Cmd::Subject(Subject::Data(Section::Rig)));
        shot(&mut v, "03e5-editor-rest-despite-pose");
        ed(
            &mut v,
            Cmd::Edit(|f| {
                use sim::data::hitbox::{BoundPoint, CombatGeometry, HurtRegion};
                let point = |id: &str| BoundPoint { joint: Some(id.into()), offset: [0., 0.] };
                f.hurt_regions = Some(vec![
                    HurtRegion {
                        enabled: true,
                        states: vec![],
                        id: "torso".into(),
                        geometry: CombatGeometry::Capsule { a: point("hips"), b: point("chest"), radius: 9. },
                    },
                    HurtRegion {
                        enabled: true,
                        states: vec![],
                        id: "head".into(),
                        geometry: CombatGeometry::Circle { center: point("head"), radius: 10. },
                    },
                    HurtRegion {
                        enabled: true,
                        states: vec![],
                        id: "leg".into(),
                        geometry: CombatGeometry::Capsule {
                            a: point("nearForeKnee"),
                            b: point("nearForeFoot"),
                            radius: 4.,
                        },
                    },
                ]);
                f.combat_center = Some([0., 35.]);
                let p = &mut f.moves.get_mut("jab").unwrap().phases[0];
                p.anim.clear();
                let h = &mut p.hitboxes[0];
                h.start = 0;
                h.end = None;
                h.frames.clear();
                h.geometry = Some(CombatGeometry::Capsule { a: point("tailBase"), b: point("tailTip"), radius: 6. });
            }),
        );
        ed(&mut v, Cmd::Subject(Subject::Move("jab".into())));
        ed(&mut v, Cmd::Hitbox(Some(0)));
        shot(&mut v, "03e6-editor-attached-collision");
        ed(
            &mut v,
            Cmd::Edit(|f| {
                f.moves.get_mut("jab").unwrap().phases[0].hurt_regions =
                    vec![sim::data::hitbox::HurtRegionWindow { region: "head".into(), enabled: false, from: 0, to: 1 }];
            }),
        );
        ed(&mut v, Cmd::Frame(1));
        shot(&mut v, "03e7-editor-region-disabled");
        ed(&mut v, Cmd::Frame(2));
        shot(&mut v, "03e8-editor-region-restored");
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
    use crate::select::{FIGHTER, TILE};
    use crate::ui::key;
    let mut v = Vec::new();
    // Versus: bot 0 opens it (and joins); its pointer starts on its fighter's tile, hints showing.
    press(&mut v, 0, Attack);
    v.push(Step::Wait(20));
    shot(&mut v, "04a-select-solo");
    // Bot 1 joins. Bot 0 hops down to "Add CPU", adds two, then Special slides it home to the roster.
    press(&mut v, 1, Attack);
    v.push(Step::LeaveRoster(0, Down));
    press(&mut v, 0, Attack);
    press(&mut v, 0, Attack);
    press(&mut v, 0, Special);
    v.extend([
        Step::Set(0, Special, true),
        Step::Wait(80),
        Step::Shot("04d-back-progress"),
        Step::Wait(300),
        Step::Set(0, Special, false),
        Step::Wait(5),
    ]);
    shot(&mut v, "04e-stable-p2");
    press(&mut v, 0, Attack);
    // Bot 0 previews the next tile and drops its chip there; bot 1 opens its card options.
    press(&mut v, 0, Right);
    press(&mut v, 0, Attack);
    press(&mut v, 1, Grab);
    press(&mut v, 1, Down);
    // Bot 0's free pointer glides off toward the cards.
    v.extend([Step::Stick(0, Vec2::new(0.45, -0.85)), Step::Wait(8)]);
    shot(&mut v, "04-select-choosing");
    v.extend([Step::Stick(0, Vec2::ZERO), Step::Wait(30)]);
    // Bot 0 picks up the first CPU's chip and carries it over the roster, then drops it.
    v.extend([Step::PointAt(0, key(FIGHTER, 2)), Step::Wait(20)]);
    press(&mut v, 0, Attack);
    v.extend([Step::PointAt(0, key(TILE, 6)), Step::Wait(20)]);
    shot(&mut v, "04b-select-carry-cpu");
    // Mid-glide the arrow leans and stretches, the chip trailing; it comes back to drop the chip.
    v.extend([Step::Stick(0, Vec2::new(-1., 0.1)), Step::Wait(9), Step::Shot("04c-select-carry-glide")]);
    v.extend([Step::Stick(0, Vec2::ZERO), Step::PointAt(0, key(TILE, 6)), Step::Wait(20)]);
    press(&mut v, 0, Attack);
    // Bot 1 closes the options, changes its pick and locks in. Bot 0's pointer rests mouse-like on the stage strip.
    press(&mut v, 1, Special);
    press(&mut v, 1, Right);
    press(&mut v, 1, Right);
    press(&mut v, 1, Attack);
    v.push(Step::Mouse(0, Vec2::new(700., 430.)));
    v.push(Step::Wait(40));
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
    v.push(Step::LeaveRoster(0, Up));
    press(&mut v, 0, Attack);
    shot(&mut v, "15b-select-teams");
    v
}

/// A thirty-fighter, thirty-stage roster (the built-ins repeated): the grids shrink and wrap into rows.
fn roster() -> Vec<Step> {
    use crate::select::TILE;
    use crate::ui::key;
    let mut v = vec![Step::Big(30), Step::Wait(2), Step::Select(Mode::Versus, 2), Step::Wait(20)];
    // Four players: everyone gathers on one tile (the pointers fan out), bot 0 hops in from below.
    for b in 1..4 {
        press(&mut v, b, Attack);
    }
    for b in 1..4 {
        v.push(Step::PointAt(b, key(TILE, 9)));
    }
    v.push(Step::Wait(10));
    press(&mut v, 0, Down);
    press(&mut v, 0, Down);
    press(&mut v, 0, Left);
    v.push(Step::Wait(20));
    shot(&mut v, "15c-select-roster-30");
    for b in 0..4 {
        press(&mut v, b, Attack);
    }
    v.push(Step::Wait(20));
    shot(&mut v, "15e-select-four-ready");
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

fn crouch() -> Vec<Step> {
    let mut v = vec![Step::Training("mike"), Step::Wait(120)];
    shot(&mut v, "14k-crouch-standing");
    v.push(Step::Set(0, Down, true));
    v.push(Step::Until(|m, _, _| m.world.fighters[0].gait == sim::world::Gait::Crouch, 30));
    shot(&mut v, "14l-crouch-held");
    v.push(Step::Set(0, Right, true));
    v.push(Step::Wait(12));
    shot(&mut v, "14o-crouch-moving");
    v.push(Step::Set(0, Right, false));
    v.push(Step::Set(0, Down, false));
    v.push(Step::Until(|m, _, _| m.world.fighters[0].gait == sim::world::Gait::Stand, 30));
    shot(&mut v, "14m-crouch-released");
    v
}

fn blast_zones() -> Vec<Step> {
    let mut v = vec![Step::WideCamera, Step::Wait(4), Step::Training("mike"), Step::Wait(120)];
    v.push(Step::Debug(false));
    shot(&mut v, "14n-blast-zones");
    v
}

fn training() -> Vec<Step> {
    // Fast-forward move waits: without a presenting display the update count does not track fixed ticks,
    // so counting frames here makes section captures timing-dependent.
    let mut v = Vec::new();
    v.extend([Step::Select(Mode::Training, 2), Step::Wait(20)]);
    press(&mut v, 0, Attack);
    shot(&mut v, "13-training-select");
    press(&mut v, 0, Start);
    v.push(Step::Until(screen_is(Screen::Match), 10));
    v.push(Step::Set(0, Right, true));
    v.push(Step::Fast(|m, _, _| m.world.fighters[0].x > m.world.fighters[1].x - 60., 200));
    v.push(Step::Set(0, Right, false));
    press(&mut v, 0, Attack);
    v.push(Step::Fast(|m, _, _| m.world.trace.as_ref().is_some_and(|t| t.done), 120));
    press(&mut v, 0, Special);
    v.push(Step::Fast(
        |m, _, _| {
            m.world.fighters[0].state == FState::Attack
                && m.world.fighters[0].mov.as_ref().is_some_and(|mv| mv.elapsed > 6)
        },
        60,
    ));
    shot(&mut v, "14-training");
    for (id, idle, special, recovery) in [
        ("archer", "14a-archer", "14b-archer-draw", "14c-archer-kite"),
        ("wizard", "14d-wizard", "14e-wizard-love", "14f-wizard-wind"),
        ("tinker", "14g-tinker", "14h-tinker-bomb", "14i-tinker-jetpack"),
        ("king", "25i-king-training", "25j-king-inhale", "25k-king-recovery"),
        ("whiplash", "24a-whiplash-training", "24b-whiplash-charge", "24c-whiplash-recovery"),
    ] {
        v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training(id), Step::Wait(30)]);
        shot(&mut v, idle);
        v.push(Step::Set(0, Special, true));
        v.push(Step::Fast(
            |m, _, _| m.world.fighters[0].mov.as_ref().is_some_and(|mv| mv.charge >= 24 || mv.elapsed >= 24),
            100,
        ));
        shot(&mut v, special);
        v.extend([Step::Set(0, Special, false), Step::Wait(70)]);
        v.extend([Step::Set(0, Up, true), Step::Set(0, Special, true), Step::Wait(8)]);
        shot(&mut v, recovery);
        v.extend([Step::Set(0, Up, false), Step::Set(0, Special, false)]);
    }
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("king"), Step::Wait(30)]);
    v.push(Step::Set(0, Down, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Fast(
        |m, _, _| {
            m.world.fighters[0].state == FState::Attack
                && m.world.fighters[0].mov.as_ref().is_some_and(|mv| mv.slot == "dspec" && mv.charge >= 90)
        },
        180,
    ));
    v.push(Step::Shot("25l-king-jet-charge"));
    v.push(Step::Set(0, Special, false));
    v.push(Step::Set(0, Down, false));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("king"), Step::Wait(30)]);
    v.push(Step::Set(0, Right, true));
    v.push(Step::Wait(4));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Fast(|m, _, _| !m.world.projs.is_empty(), 80));
    v.push(Step::Wait(10));
    v.push(Step::Shot("25m-king-gordo"));
    v.push(Step::Set(0, Special, false));
    v.push(Step::Set(0, Right, false));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("king"), Step::Wait(30)]);
    v.push(Step::Set(0, Up, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Fast(
        |m, _, _| m.world.fighters[0].mov.as_ref().is_some_and(|mv| mv.slot == "uspec" && mv.phase >= 2),
        200,
    ));
    v.push(Step::Shot("25n-king-super-jump"));
    v.push(Step::Fast(|m, _, _| m.world.projs.len() >= 2, 500));
    v.push(Step::Shot("25o-king-landing-stars"));
    v.push(Step::Set(0, Up, false));
    v.push(Step::Set(0, Special, false));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("king"), Step::Wait(30)]);
    v.push(Step::Set(0, Special, true));
    v.push(Step::Wait(24));
    v.push(Step::Shot("25p-king-inhale"));
    v.push(Step::Set(0, Special, false));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("mike"), Step::Wait(30)]);
    v.push(Step::Set(0, Special, true));
    v.push(Step::Fast(|m, _, _| m.world.fighters[0].mov.as_ref().is_some_and(|mv| mv.charge == 209), 260));
    v.push(Step::Set(0, Special, false));
    v.push(Step::Fast(|m, _, _| m.world.fighters[0].mov.as_ref().is_some_and(|mv| mv.t >= 39), 80));
    v.push(Step::Shot("21i-mike-full-eruption"));
    // A settled banana is a world state, not a pose: throw one and wait for it to come to rest.
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("bongo"), Step::Wait(30)]);
    v.push(Step::Set(0, Down, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Wait(6));
    v.push(Step::Set(0, Special, false));
    v.push(Step::Set(0, Down, false));
    v.push(Step::Fast(
        |m, _, _| m.world.projs.iter().any(|p| p.id.starts_with("banana") && p.age > 40 && p.vy == 0.),
        300,
    ));
    v.push(Step::Shot("33r-bongo-banana-settled"));
    // Duchess float, turnip flight and parasol descent are state/timing captures, not move poses.
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("duchess"), Step::Wait(30)]);
    v.push(Step::Set(0, Jump, true));
    v.push(Step::Fast(|m, _, _| m.world.fighters[0].state == FState::Air && m.world.fighters[0].vy > 0.9, 160));
    v.push(Step::Shot("35o-duchess-float"));
    v.push(Step::Set(0, Jump, false));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("duchess"), Step::Wait(30)]);
    v.push(Step::Set(0, Down, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Wait(6));
    v.push(Step::Set(0, Special, false));
    v.push(Step::Set(0, Down, false));
    v.push(Step::Fast(|m, _, _| m.world.projs.iter().any(|p| p.id.starts_with("turnip")), 200));
    v.push(Step::Wait(10));
    v.push(Step::Shot("35p-duchess-turnip-flight"));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("duchess"), Step::Wait(30)]);
    v.push(Step::Set(0, Up, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Fast(
        |m, _, _| m.world.fighters[0].mov.as_ref().is_some_and(|mv| mv.slot == "uspec" && mv.phase >= 1),
        200,
    ));
    v.push(Step::Wait(16));
    v.push(Step::Shot("35q-duchess-parasol-descent"));
    v.extend([Step::Set(0, Up, false), Step::Set(0, Special, false)]);
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Wait(4), Step::OpenEditor, Step::Wait(20)]);
    for (id, slot, frame, name) in [
        ("ninja", "fsmash", 14, "14b-ninja-heel"),
        ("ninja", "nspec", 18, "14c-ninja-stars"),
        ("ninja", "uspec", 5, "14d-ninja-smoke"),
        ("yoyo", "fsmash", 17, "14e-yoyo-world"),
        ("yoyo", "nspec", 16, "14f-yoyo-out"),
        ("yoyo", "nspec", 24, "14g-yoyo-return"),
        ("wrangler", "fsmash", 21, "14h-wrangler-tip"),
        ("wrangler", "nspec", 15, "14i-wrangler-pull"),
        ("wrangler", "uspec", 5, "14j-wrangler-hitch"),
    ] {
        ed(&mut v, Cmd::Fighter(id.into()));
        ed(&mut v, Cmd::Subject(editor::preview::Subject::Move(slot.into())));
        ed(&mut v, Cmd::Dummy(!(id == "ninja" && slot == "nspec")));
        v.push(Step::Wait(4));
        ed(&mut v, Cmd::Frame(frame));
        ed(&mut v, Cmd::Hitbox(None));
        shot(&mut v, name);
    }
    v
}

fn fighter_kits() -> Vec<Step> {
    use editor::preview::Subject;
    let mut v = vec![Step::OpenEditor, Step::Wait(20)];
    for (fighter, slot, frame, name) in [
        ("golem", "fsmash", 20, "18a-golem-smash"),
        ("golem", "ftilt", 5, "18e-golem-coil"),
        ("golem", "nspec", 18, "18b-golem-pound"),
        ("golem", "dspec", 12, "18c-golem-skin"),
        ("golem", "dspec", 22, "18d-golem-retaliation"),
        ("lancer", "ftilt", 9, "19a-lancer-thrust"),
        ("lancer", "uspec", 4, "19b-lancer-vault"),
        ("lancer", "dair", 11, "19c-lancer-pogo"),
        ("lancer", "dtilt", 8, "19d-lancer-low-thrust"),
        ("dancer", "fsmash", 14, "20a-dancer-kick"),
        ("dancer", "nair", 4, "20e-dancer-spin-contact"),
        ("dancer", "sspec", 5, "20f-dancer-cartwheel-contact"),
        ("dancer", "uspec", 4, "20g-dancer-recovery-spin"),
        ("dancer", "usmash", 13, "20b-dancer-handstand"),
        ("dancer", "dspec", 9, "20c-dancer-sweep"),
        ("dancer", "nspec", 10, "20d-dancer-travelling-spin"),
        ("mike", "fsmash", 33, "21a-mike-smash"),
        ("mike", "nair", 18, "21b-mike-aerial"),
        ("mike", "nspec", 30, "21c-mike-eruption"),
        ("mike", "uspec", 35, "21d-mike-aether"),
        ("mike", "dtilt", 8, "21e-mike-low-sweep"),
        ("mike", "uair", 18, "21f-mike-rear-sweep"),
        ("mike", "fsmash", 24, "21g-mike-smash-windup"),
        ("mike", "uspec", 20, "21h-mike-sword-toss"),
        ("wizard", "fair", 6, "22a-wizard-lightning"),
        ("wizard", "usmash", 17, "22b-wizard-storm"),
        ("wizard", "nspec", 11, "22c-wizard-love"),
        ("wizard", "uspec", 35, "22d-wizard-wind"),
        ("wizard", "dspec", 52, "22e-wizard-phantom"),
        ("wizard", "dspec", 127, "22g-wizard-phantom-strike"),
        ("wizard", "sspec", 44, "22f-wizard-fire"),
        ("whiplash", "nair", 10, "23a-whiplash-nair"),
        ("whiplash", "nair", 16, "23p-whiplash-nair-late"),
        ("whiplash", "zair", 22, "23q-whiplash-tether-late"),
        ("whiplash", "fair", 13, "23b-whiplash-fair"),
        ("whiplash", "usmash", 23, "23c-whiplash-up-smash"),
        ("whiplash", "sspec", 31, "23d-whiplash-plasma"),
        ("whiplash", "uspec", 22, "23e-whiplash-boost"),
        ("whiplash", "dspec", 10, "23f-whiplash-flip"),
        ("whiplash", "dspec", 28, "23i-whiplash-flip-tuck"),
        ("whiplash", "dspec", 37, "23j-whiplash-flip-open"),
        ("whiplash", "zair", 15, "23g-whiplash-tether"),
        ("whiplash", "grab", 20, "23n-whiplash-grab-extend"),
        ("whiplash", "grab", 31, "23o-whiplash-grab-retract"),
        ("whiplash", "nspec", 22, "23h-whiplash-paralyzer"),
        ("king", "fsmash", 39, "25a-king-smash"),
        ("king", "sspec", 31, "25b-king-gordo"),
        ("king", "nspec", 24, "25c-king-inhale"),
        ("king", "uspec", 30, "25d-king-super-jump"),
        ("king", "dair", 21, "25e-king-meteor"),
        ("king", "dspec", 28, "25f-king-jet"),
        ("king", "grab", 8, "25g-king-grab"),
        ("king", "utilt", 8, "25h-king-head-lift"),
        ("throbbin", "fsmash", 16, "30a-throbbin-levin-smash"),
        ("throbbin", "usmash", 14, "30b-throbbin-levin-rising"),
        ("throbbin", "dsmash", 20, "30c-throbbin-levin-sweep"),
        ("throbbin", "nair", 9, "30d-throbbin-levin-nair"),
        ("throbbin", "fair", 12, "30e-throbbin-levin-fair"),
        ("throbbin", "dair", 15, "30f-throbbin-levin-drop"),
        ("throbbin", "nspec", 8, "30g-throbbin-thunder-charge"),
        ("throbbin", "nspec", 20, "30h-throbbin-thunder-cast"),
        ("throbbin", "sspec", 20, "30i-throbbin-arcfire"),
        ("throbbin", "uspec", 12, "30j-throbbin-elwind"),
        ("throbbin", "dspec", 18, "30k-throbbin-nosferatu"),
        ("throbbin", "jab", 5, "30l-throbbin-jab"),
        ("sneak", "jab", 2, "32a-sneak-jab"),
        ("sneak", "ftilt", 3, "32b-sneak-knee"),
        ("sneak", "utilt", 7, "32d-sneak-rising-kick"),
        ("sneak", "fsmash", 41, "32e-sneak-rpg"),
        ("sneak", "usmash", 37, "32f-sneak-mortar"),
        ("sneak", "dsmash", 8, "32g-sneak-double-kick"),
        ("sneak", "nair", 36, "32h-sneak-air-kick"),
        ("sneak", "fair", 24, "32i-sneak-knee-drop"),
        ("sneak", "dair", 25, "32j-sneak-drill"),
        ("sneak", "nspec", 8, "32k-sneak-grenade-pull"),
        ("sneak", "sspec", 45, "32l-sneak-nikita"),
        ("sneak", "uspec", 20, "32m-sneak-cypher"),
        ("sneak", "dspec", 4, "32n-sneak-c4-plant"),
        ("bongo", "jab", 4, "33a-bongo-jab"),
        ("bongo", "ftilt", 10, "33b-bongo-double-arm"),
        ("bongo", "fsmash", 21, "33c-bongo-slap-beat"),
        ("bongo", "usmash", 19, "33d-bongo-triple-attack"),
        ("bongo", "dsmash", 12, "33e-bongo-turning-leg"),
        ("bongo", "nair", 10, "33f-bongo-roll"),
        ("bongo", "fair", 7, "33g-bongo-screw-kick"),
        ("bongo", "bair", 5, "33h-bongo-backspin"),
        ("bongo", "uair", 5, "33i-bongo-turn-kick"),
        ("bongo", "dair", 15, "33j-bongo-meteor"),
        ("bongo", "nspec", 12, "33k-bongo-peanut-charge"),
        ("bongo", "nspec", 20, "33l-bongo-peanut-shot"),
        ("bongo", "sspec", 22, "33m-bongo-monkey-flip"),
        ("bongo", "uspec", 20, "33n-bongo-rocketbarrel"),
        ("bongo", "dspec", 20, "33o-bongo-banana"),
        ("bongo", "grab", 7, "33p-bongo-grab"),
        ("duchess", "fsmash", 15, "35a-duchess-golf-club"),
        ("duchess", "fair", 17, "35b-duchess-crown-knuckle"),
        ("duchess", "nair", 6, "35c-duchess-royal-spin"),
        ("duchess", "uair", 11, "35d-duchess-rainbow-arc"),
        ("duchess", "dair", 12, "35e-duchess-piston-kick"),
        ("duchess", "bair", 6, "35f-duchess-iron-hip"),
        ("duchess", "nspec", 9, "35g-duchess-toad-guard"),
        ("duchess", "sspec", 16, "35h-duchess-peach-bomber"),
        ("duchess", "uspec", 20, "35i-duchess-parasol"),
        ("duchess", "dspec", 25, "35j-duchess-vegetable-pluck"),
        ("duchess", "dspec", 45, "35k-duchess-turnip-toss"),
        ("duchess", "dtilt", 8, "35l-duchess-underfoot-kick"),
        ("duchess", "dsmash", 13, "35m-duchess-ribbon-sweep"),
        ("duchess", "utilt", 10, "35n-duchess-royal-wave"),
        ("verdant", "fsmash", 22, "34a-verdant-areadbhar-smash"),
        ("verdant", "usmash", 16, "34b-verdant-creator-pillar"),
        ("verdant", "dsmash", 18, "34c-verdant-aymr-sweep"),
        ("verdant", "dtilt", 13, "34d-verdant-whip-sweep"),
        ("verdant", "ftilt", 8, "34e-verdant-creator-slash"),
        ("verdant", "fair", 12, "34f-verdant-lance-cut"),
        ("verdant", "bair", 13, "34g-verdant-reverse-lance"),
        ("verdant", "dair", 7, "34h-verdant-aymr-drop"),
        ("verdant", "nair", 8, "34i-verdant-quiver-spin"),
        ("verdant", "nspec", 40, "34j-verdant-failnaught-draw"),
        ("verdant", "nspec", 46, "34k-verdant-failnaught-loose"),
        ("verdant", "sspec", 21, "34l-verdant-areadbhar-rush"),
        ("verdant", "uspec", 12, "34m-verdant-creator-tether"),
        ("verdant", "dspec", 63, "34n-verdant-aymr-slam"),
        ("verdant", "jab", 4, "34o-verdant-creator-jab"),
    ] {
        ed(&mut v, Cmd::Subject(Subject::Move(slot.into())));
        ed(&mut v, Cmd::Fighter(fighter.into()));
        // Keep the editor's launch-angle handle out of the capture.
        ed(&mut v, Cmd::Hitbox(None));
        ed(&mut v, Cmd::Frame(frame));
        shot(&mut v, name);
    }
    // Pin the preview to the move's default (Bronze) timeline instead of the resource-selected Levin one.
    ed(&mut v, Cmd::Fighter("throbbin".into()));
    ed(&mut v, Cmd::Subject(Subject::Move("fsmash".into())));
    ed(&mut v, Cmd::Variant(Some(0)));
    ed(&mut v, Cmd::Frame(16));
    shot(&mut v, "30m-throbbin-bronze-smash");
    ed(&mut v, Cmd::Variant(None));
    // The detonate timeline is only previewed by forcing it; the pool would plant a C4 instead.
    ed(&mut v, Cmd::Fighter("sneak".into()));
    ed(&mut v, Cmd::Subject(Subject::Move("dspec".into())));
    ed(&mut v, Cmd::Variant(Some(0)));
    ed(&mut v, Cmd::Frame(25));
    shot(&mut v, "32o-sneak-c4-detonate");
    ed(&mut v, Cmd::Variant(None));
    // The forward tilt's second blow only follows a held attack; preview it by trimming the knee phase.
    ed(&mut v, Cmd::Fighter("sneak".into()));
    ed(&mut v, Cmd::Subject(Subject::Move("ftilt".into())));
    ed(
        &mut v,
        Cmd::Edit(|f| {
            f.moves.ftilt.phases.remove(0);
        }),
    );
    ed(&mut v, Cmd::Frame(7));
    shot(&mut v, "32c-sneak-body-blow");
    ed(&mut v, Cmd::Undo);
    // Live reads of the thrown gadgets, which the editor preview cannot hold or steer.
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("sneak"), Step::Wait(20)]);
    v.push(Step::Set(0, Special, true));
    v.push(Step::Wait(26));
    v.push(Step::Set(0, Special, false));
    v.push(Step::Until(sneak_projectile, 240));
    v.push(Step::Shot("32p-sneak-grenade-flight"));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("sneak"), Step::Wait(20)]);
    v.push(Step::Set(0, Right, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Until(sneak_missile, 240));
    v.push(Step::Shot("32q-sneak-nikita-flight"));
    v.push(Step::Set(0, Right, false));
    v.push(Step::Set(0, Special, false));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("sneak"), Step::Wait(20)]);
    v.push(Step::Set(0, Up, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Wait(22));
    v.push(Step::Shot("32r-sneak-cypher-rise"));
    v.push(Step::Set(0, Up, false));
    v.push(Step::Set(0, Special, false));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("sneak"), Step::Wait(20)]);
    v.push(Step::Set(0, Down, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Wait(30));
    v.push(Step::Set(0, Down, false));
    v.push(Step::Set(0, Special, false));
    v.push(Step::Wait(24));
    v.push(Step::Set(0, Down, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Until(sneak_c4_blast, 240));
    v.push(Step::Shot("32s-sneak-c4-blast"));
    v.push(Step::Set(0, Down, false));
    v.push(Step::Set(0, Special, false));
    // The kick branch needs an attack press, which the preview does not send: drop the leap and flip phases
    // from the preview copy so the kick timeline is visible.
    ed(&mut v, Cmd::Fighter("bongo".into()));
    ed(&mut v, Cmd::Subject(Subject::Move("sspec".into())));
    ed(
        &mut v,
        Cmd::Edit(|f| {
            let phases = &mut f.moves.sspec.as_mut().unwrap().phases;
            phases.remove(0);
            phases.remove(0);
        }),
    );
    ed(&mut v, Cmd::Frame(10));
    shot(&mut v, "33q-bongo-monkey-kick");
    ed(&mut v, Cmd::Undo);
    ed(&mut v, Cmd::Fighter("stratus".into()));
    ed(&mut v, Cmd::Dummy(true));
    for (slot, frame, variant, name) in [
        ("fsmash", 27, None, "31a-stratus-fsmash"),
        ("usmash", 12, None, "31b-stratus-usmash"),
        ("dsmash", 21, None, "31c-stratus-dsmash"),
        ("nair", 8, None, "31d-stratus-nair"),
        ("fair", 18, None, "31e-stratus-fair"),
        ("bair", 11, None, "31f-stratus-bair"),
        ("uair", 9, None, "31g-stratus-uair"),
        ("dair", 12, None, "31h-stratus-dair"),
        ("jab", 6, None, "31i-stratus-jab"),
        ("ftilt", 9, None, "31j-stratus-ftilt"),
        ("utilt", 7, None, "31k-stratus-utilt"),
        ("dtilt", 7, None, "31l-stratus-dtilt"),
        ("dashatk", 10, None, "31m-stratus-dashatk"),
        ("grab", 9, None, "31n-stratus-grab"),
        ("nspec", 20, Some(0), "31o-stratus-blade-beam"),
        ("nspec", 20, Some(1), "31p-stratus-limit-blade-beam"),
        ("sspec", 11, Some(0), "31q-stratus-cross-slash"),
        ("sspec", 38, Some(1), "31r-stratus-limit-cross-slash"),
        ("uspec", 12, Some(0), "31s-stratus-climhazzard"),
        ("uspec", 12, Some(1), "31t-stratus-limit-climhazzard"),
        ("dspec", 10, Some(0), "31u-stratus-limit-charge"),
        ("dspec", 14, Some(1), "31v-stratus-finishing-touch"),
    ] {
        ed(&mut v, Cmd::Subject(Subject::Move(slot.into())));
        ed(&mut v, Cmd::Variant(variant));
        ed(&mut v, Cmd::Frame(frame));
        shot(&mut v, name);
    }
    ed(&mut v, Cmd::Variant(None));
    ed(&mut v, Cmd::Dummy(false));
    ed(&mut v, Cmd::Fighter("whiplash".into()));
    ed(&mut v, Cmd::Subject(Subject::Move("dspec".into())));
    ed(
        &mut v,
        Cmd::Edit(|f| {
            f.moves.dspec.phases.remove(0);
        }),
    );
    for (frame, name) in
        [(7, "23k-whiplash-kick-coil"), (9, "23l-whiplash-kick-strike"), (18, "23m-whiplash-kick-recoil")]
    {
        ed(&mut v, Cmd::Frame(frame));
        shot(&mut v, name);
    }
    ed(&mut v, Cmd::Undo);
    v
}

/// Roster tile key for a fighter id: the grid follows the resolved content order, which grows with imports.
fn tile(id: &str) -> u32 {
    static DATA: std::sync::OnceLock<sim::GameData> = std::sync::OnceLock::new();
    let d = DATA.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        crate::load_data(&[root]).0
    });
    let at = d.fighters.iter().position(|f| f.id == id).unwrap_or_else(|| panic!("no fighter {id}"));
    crate::ui::key(crate::select::TILE, at)
}

fn plant_shots() -> Vec<Step> {
    use editor::preview::Subject;
    let mut v = vec![Step::Select(Mode::Training, 2), Step::Wait(20)];
    v.push(Step::PointAt(0, tile("plant")));
    v.push(Step::Wait(30));
    shot(&mut v, "29a-select-plant-hover");
    // Training: plant's own movement clips and the held/growing/rising specials.
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("plant"), Step::Wait(30)]);
    shot(&mut v, "29b-plant-idle");
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("plant"), Step::Wait(20)]);
    v.push(Step::Set(0, Right, true));
    v.push(Step::Wait(32));
    shot(&mut v, "29s-plant-walk");
    v.push(Step::Wait(60));
    shot(&mut v, "29t-plant-run");
    v.push(Step::Set(0, Right, false));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("plant"), Step::Wait(20)]);
    v.push(Step::Set(0, Special, true));
    v.push(Step::Wait(18));
    v.push(Step::Shot("29l-plant-ptooie-hold"));
    // Blow the held ball with a direction; letting go of special would swallow it instead.
    v.push(Step::Set(0, Right, true));
    v.push(Step::Wait(8));
    v.push(Step::Shot("29m-plant-ptooie-shot"));
    v.push(Step::Set(0, Right, false));
    v.push(Step::Set(0, Special, false));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("plant"), Step::Wait(20)]);
    v.push(Step::Set(0, Right, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Wait(80));
    v.push(Step::Shot("29n-plant-poison-charge"));
    v.push(Step::Set(0, Special, false));
    v.push(Step::Wait(24));
    v.push(Step::Shot("29o-plant-poison-cloud"));
    v.push(Step::Set(0, Right, false));
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Training("plant"), Step::Wait(20)]);
    v.push(Step::Set(0, Up, true));
    v.push(Step::Set(0, Special, true));
    v.push(Step::Wait(20));
    v.push(Step::Shot("29p-plant-copter"));
    v.push(Step::Set(0, Up, false));
    v.push(Step::Set(0, Special, false));
    // Nonhumanoid hold and ledge interaction anchors.
    v.push(Step::Interaction(4));
    v.push(Step::Wait(20));
    v.push(Step::Shot("29q-plant-hold-brawler"));
    v.push(Step::Interaction(5));
    v.push(Step::Wait(20));
    v.push(Step::Shot("29r-plant-ledge"));
    // Editor: startup/contact/recovery, the extending stem, and the mouth grab.
    v.extend([Step::Home, Step::Wait(4), Step::Reset, Step::Wait(4), Step::OpenEditor, Step::Wait(20)]);
    ed(&mut v, Cmd::Fighter("plant".into()));
    ed(&mut v, Cmd::Subject(Subject::Move("fsmash".into())));
    ed(&mut v, Cmd::Dummy(true));
    v.push(Step::Wait(4));
    for (frame, name) in
        [(0, "29c-plant-fsmash-startup"), (15, "29g-plant-fsmash-contact"), (34, "29h-plant-fsmash-recovery")]
    {
        ed(&mut v, Cmd::Frame(frame));
        if frame == 15 {
            ed(&mut v, Cmd::Hitbox(Some(2)));
        } else {
            ed(&mut v, Cmd::Hitbox(None));
        }
        shot(&mut v, name);
    }
    ed(&mut v, Cmd::Subject(Subject::Move("dspec".into())));
    ed(&mut v, Cmd::Hitbox(None));
    for (frame, name) in [(6, "29i-plant-stem-charge"), (26, "29j-plant-stem-extend")] {
        ed(&mut v, Cmd::Frame(frame));
        shot(&mut v, name);
    }
    ed(&mut v, Cmd::Subject(Subject::Move("grab".into())));
    ed(&mut v, Cmd::Frame(8));
    shot(&mut v, "29k-plant-grab");
    // Animation boundaries through the shared freeze step.
    for (state, tick, vy, name) in [
        (FState::Jumpsquat, 0, 0., "29u-plant-jump-load"),
        (FState::Jumpsquat, 2, 0., "29v-plant-jump-extend"),
        (FState::Air, 3, -6., "29w-plant-air-rise"),
        (FState::Air, 24, 5., "29x-plant-air-fall"),
        (FState::Landing, 0, 0., "29y-plant-landing-impact"),
        (FState::Landing, 2, 0., "29z-plant-landing-recover"),
    ] {
        v.push(Step::Animation("plant", state, tick, vy));
        shot(&mut v, name);
    }
    let _ = crate::ui::key(crate::select::TILE, 0);
    v
}

fn animation_shots() -> Vec<Step> {
    let mut v = Vec::new();
    for (fighter, state, tick, vy, name) in [
        ("brawler", FState::Jumpsquat, 0, 0., "26a-brawler-jump-load"),
        ("brawler", FState::Jumpsquat, 1, 0., "26b-brawler-jump-compress"),
        ("brawler", FState::Jumpsquat, 2, 0., "26c-brawler-jump-extend"),
        ("brawler", FState::Air, 0, -5., "26d-brawler-takeoff"),
        ("brawler", FState::Landing, 0, 0., "26e-brawler-landing-impact"),
        ("brawler", FState::Landing, 2, 0., "26f-brawler-landing-recover"),
        ("brawler", FState::Spot, 0, 0., "26g-brawler-spot-entry"),
        ("brawler", FState::Spot, 25, 0., "26h-brawler-spot-exit"),
        ("brawler", FState::Airdodge, 3, 0., "26i-brawler-airdodge-fold"),
        ("brawler", FState::Airdodge, 29, 0., "26j-brawler-airdodge-exit"),
        ("king", FState::Jumpsquat, 2, 0., "27a-king-jump-load"),
        ("king", FState::Air, 0, -5., "27b-king-takeoff"),
        ("king", FState::Air, 4, -5., "27c-king-rise"),
        ("king", FState::Air, 24, 5., "27d-king-fall"),
        ("king", FState::Landing, 0, 0., "27e-king-landing-impact"),
        ("king", FState::Landing, 2, 0., "27f-king-landing-recover"),
        ("whiplash", FState::Jumpsquat, 2, 0., "28a-whiplash-jump-load"),
        ("whiplash", FState::Air, 0, -5., "28b-whiplash-takeoff"),
        ("whiplash", FState::Air, 24, 5., "28c-whiplash-fall"),
        ("whiplash", FState::Landing, 0, 0., "28d-whiplash-landing-impact"),
        ("whiplash", FState::Landing, 2, 0., "28e-whiplash-landing-recover"),
    ] {
        v.push(Step::Animation(fighter, state, tick, vy));
        shot(&mut v, name);
    }
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
    (mut ptrs, q): (ResMut<crate::pointer::Pointers>, crate::ui::FocusQuery),
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
            devs.bot_sticks.clear();
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
        Step::LeaveRoster(b, act) => {
            let on_roster = sel.as_ref().is_some_and(|s| {
                s.slots.iter().any(|p| p.dev == Some(Dev::Bot(*b)) && crate::ui::kind(p.cursor.key).0 == 1)
            });
            devs.bot(*b)[*act as usize] = on_roster && s.budget.is_multiple_of(2);
            if on_roster && s.budget < 60 {
                s.budget += 1;
                s.wait = 4;
            } else {
                if on_roster {
                    error!("screenshot section {name}: could not leave roster");
                    s.failures.push(name);
                }
                s.budget = 0;
                s.at += 1;
            }
        }
        Step::Stick(b, at) => {
            devs.bot_sticks.insert(*b, *at);
            s.at += 1;
        }
        Step::PointAt(b, k) => {
            let spot = crate::ui::spots(&q).into_iter().find(|sp| sp.f.key == *k);
            if let (Some(sp), Some(p)) = (spot, ptrs.map.get_mut(&Dev::Bot(*b))) {
                p.release(sp.center + Vec2::new(24., 14.));
            } else {
                error!("screenshot section {name}: nothing to point at");
            }
            s.at += 1;
        }
        Step::Mouse(b, at) => {
            if let Some(p) = ptrs.map.get_mut(&Dev::Bot(*b)) {
                p.point(*at);
            }
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
        Step::Interaction(kind) => {
            use sim::data::fighter::{GrabInteraction, Interactions, LedgeInteraction};
            let mut content = (*data.0).clone();
            let (aid, vid) = if *kind >= 4 { ("plant", "brawler") } else { ("brawler", "ninja") };
            let a = content.fighters.iter().position(|f| f.id == aid).unwrap();
            let v = content.fighters.iter().position(|f| f.id == vid).unwrap();
            if *kind >= 4 {
                let content = std::sync::Arc::new(content);
                let slots = [a, v]
                    .into_iter()
                    .enumerate()
                    .map(|(i, fighter)| Slot {
                        dev: Some(Dev::Bot(i as u8)),
                        fighter,
                        cpu: None,
                        team: None,
                        tap_jump: false,
                        controls: Default::default(),
                        tag: String::new(),
                        color: i,
                    })
                    .collect();
                let mut m = Match::new(
                    &content,
                    Mode::Training,
                    slots,
                    Rules { stocks: 3, stage: "arena".into(), friendly_fire: false, time_limit: None },
                    7,
                )
                .unwrap();
                m.world.count = 0;
                if *kind == 4 {
                    sim::combat::start_hold(&mut m.world, &content, 0, 1);
                } else {
                    let f = &mut m.world.fighters[0];
                    f.face = 1.;
                    f.ground = false;
                    f.set_state(FState::Ledge);
                    f.sd.side = -1.;
                    f.sd.ledge_block = 0;
                    sim::states::update_fighter(&mut m.world, &content, 0, sim::input::Intent::default());
                }
                m.frozen = true;
                commands.remove_resource::<Select>();
                play::install(&mut commands, &mut scene, m);
                next.set(Screen::Match);
                s.at += 1;
                return;
            }
            content.fighters[a].interactions = Some(Interactions {
                grab: Some(GrabInteraction { hold_point: Some([39., 75.]), ..Default::default() }),
                ..Default::default()
            });
            content.fighters[v].body.height = 24.;
            content.fighters[v].body.half_width = 7.;
            content.fighters[v].rig =
                Some(serde_json::from_str(include_str!("../../../assets/templates/rigs/quadruped.json")).unwrap());
            content.fighters[v].interactions = Some(Interactions {
                grab: Some(GrabInteraction { held_anchor: Some([13., 21.]), ..Default::default() }),
                ledge: Some(LedgeInteraction {
                    hang_anchor: Some([25., 35.]),
                    pull_in: Some(19.),
                    climb_distance: Some(51.),
                    ..Default::default()
                }),
            });
            let content = std::sync::Arc::new(if *kind == 3 { (*data.0).clone() } else { content });
            let slots = [a, v]
                .into_iter()
                .enumerate()
                .map(|(i, fighter)| Slot {
                    dev: Some(Dev::Bot(i as u8)),
                    fighter,
                    cpu: None,
                    team: None,
                    tap_jump: false,
                    controls: Default::default(),
                    tag: String::new(),
                    color: i,
                })
                .collect();
            let mut m = Match::new(
                &content,
                Mode::Training,
                slots,
                Rules { stocks: 3, stage: "arena".into(), friendly_fire: false, time_limit: None },
                7,
            )
            .unwrap();
            m.world.count = 0;
            if *kind == 0 || *kind == 3 {
                sim::combat::start_hold(&mut m.world, &content, 0, 1);
            } else {
                let f = &mut m.world.fighters[1];
                f.face = 1.;
                f.ground = false;
                f.set_state(if *kind == 1 { FState::Ledge } else { FState::Climb });
                f.sd.side = -1.;
                f.sd.ledge_block = 0;
                f.sd.kind = "climb".into();
                if *kind == 2 {
                    f.t = content.global.ledge.climb.a - 1;
                }
                sim::states::update_fighter(&mut m.world, &content, 1, sim::input::Intent::default());
            }
            m.frozen = true;
            commands.remove_resource::<Select>();
            play::install(&mut commands, &mut scene, m);
            next.set(Screen::Match);
            s.at += 1;
        }
        Step::Training(id) | Step::Animation(id, _, _, _) => {
            let slots = [*id, "brawler"]
                .into_iter()
                .enumerate()
                .map(|(i, id)| Slot {
                    dev: Some(Dev::Bot(i as u8)),
                    fighter: data.0.fighters.iter().position(|f| f.id == id).unwrap(),
                    cpu: None,
                    team: None,
                    tap_jump: false,
                    controls: Default::default(),
                    tag: String::new(),
                    color: i,
                })
                .collect();
            let rules = Rules { stocks: 3, stage: "arena".into(), friendly_fire: false, time_limit: None };
            let mut m = Match::new(&data.0, Mode::Training, slots, rules, 7).unwrap();
            m.world.count = 0;
            if let Step::Animation(_, state, tick, vy) = step {
                let f = &mut m.world.fighters[0];
                f.set_state(*state);
                f.t = *tick;
                f.vy = *vy;
                f.face = 1.;
                f.ground = !matches!(state, FState::Air | FState::Airdodge);
                if !f.ground {
                    f.y -= 120.;
                    f.surf = None;
                }
                m.world.frame = u64::from(*tick);
                m.frozen = true;
            }
            commands.remove_resource::<Select>();
            play::install(&mut commands, &mut scene, m);
            next.set(Screen::Match);
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
                    controls: Default::default(),
                    tag: "CPU".into(),
                    color: i,
                })
                .collect();
            // Three minutes on the clock, so the HUD shots show the timer.
            let rules = Rules { stocks, stage: id.into(), friendly_fire: false, time_limit: Some(180) };
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
        Step::WideCamera => {
            let mut wide = (**s.original.get_or_insert_with(|| data.0.clone())).clone();
            let st = wide.stages.iter_mut().find(|st| st.id == "arena").unwrap();
            st.camera = sim::data::stage::Bounds { left: -1400., right: 2700., top: -900., bottom: 1800. };
            st.zoom = Some(sim::data::stage::Zoom { min_height: Some(2100.), max_height: Some(2100.) });
            commands.insert_resource(Data(std::sync::Arc::new(wide)));
            s.at += 1;
        }
        Step::Debug(on) => {
            if let Some(mut m) = m {
                m.debug = *on;
            }
            scene.debug = *on;
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
        let data = crate::test_data();
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
            .init_resource::<crate::pointer::Pointers>()
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
03e2-editor-humanoid-rig
03e3-editor-quadruped-rig
03e4-editor-named-pose-handles
03e5-editor-rest-despite-pose
03e6-editor-attached-collision
03e7-editor-region-disabled
03e8-editor-region-restored
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
04a-select-solo
04b-select-carry-cpu
04c-select-carry-glide
04d-back-progress
04e-stable-p2
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
14a-archer
14b-archer-draw
14b-ninja-heel
14c-archer-kite
14c-ninja-stars
14d-ninja-smoke
14d-wizard
14e-wizard-love
14e-yoyo-world
14f-wizard-wind
14f-yoyo-out
14g-tinker
14g-yoyo-return
14h-tinker-bomb
14h-wrangler-tip
14i-tinker-jetpack
14i-wrangler-pull
14j-wrangler-hitch
14k-crouch-standing
14l-crouch-held
14m-crouch-released
14n-blast-zones
14o-crouch-moving
15-select-eight
15b-select-teams
15c-select-roster-30
15d-stage-select-30
15e-select-four-ready
16a-select-citadel
16b-select-isles
16c-select-foundry
17a-match-citadel
17b-match-isles
17b2-match-isles-wide
17c-match-foundry
18a-golem-smash
18b-golem-pound
18c-golem-skin
18d-golem-retaliation
18e-golem-coil
19a-lancer-thrust
19b-lancer-vault
19c-lancer-pogo
19d-lancer-low-thrust
20a-dancer-kick
20b-dancer-handstand
20c-dancer-sweep
20d-dancer-travelling-spin
20e-dancer-spin-contact
20f-dancer-cartwheel-contact
20g-dancer-recovery-spin
21a-mike-smash
21b-mike-aerial
21c-mike-eruption
21d-mike-aether
21e-mike-low-sweep
21f-mike-rear-sweep
21g-mike-smash-windup
21h-mike-sword-toss
21i-mike-full-eruption
22a-wizard-lightning
22b-wizard-storm
22c-wizard-love
22d-wizard-wind
22e-wizard-phantom
22f-wizard-fire
22g-wizard-phantom-strike
23a-whiplash-nair
23b-whiplash-fair
23c-whiplash-up-smash
23d-whiplash-plasma
23e-whiplash-boost
23f-whiplash-flip
23g-whiplash-tether
23h-whiplash-paralyzer
23i-whiplash-flip-tuck
23j-whiplash-flip-open
23k-whiplash-kick-coil
23l-whiplash-kick-strike
23m-whiplash-kick-recoil
23n-whiplash-grab-extend
23o-whiplash-grab-retract
23p-whiplash-nair-late
23q-whiplash-tether-late
24a-unequal-hold
24a-whiplash-training
24b-unequal-ledge
24b-whiplash-charge
24c-unequal-climb
24c-whiplash-recovery
24d-standard-hold
25a-king-smash
25b-king-gordo
25c-king-inhale
25d-king-super-jump
25e-king-meteor
25f-king-jet
25g-king-grab
25h-king-head-lift
25i-king-training
25j-king-inhale
25k-king-recovery
25l-king-jet-charge
25m-king-gordo
25n-king-super-jump
25o-king-landing-stars
25p-king-inhale
26a-brawler-jump-load
26b-brawler-jump-compress
26c-brawler-jump-extend
26d-brawler-takeoff
26e-brawler-landing-impact
26f-brawler-landing-recover
26g-brawler-spot-entry
26h-brawler-spot-exit
26i-brawler-airdodge-fold
26j-brawler-airdodge-exit
27a-king-jump-load
27b-king-takeoff
27c-king-rise
27d-king-fall
27e-king-landing-impact
27f-king-landing-recover
28a-whiplash-jump-load
28b-whiplash-takeoff
28c-whiplash-fall
28d-whiplash-landing-impact
28e-whiplash-landing-recover
29a-select-plant-hover
29b-plant-idle
29c-plant-fsmash-startup
29g-plant-fsmash-contact
29h-plant-fsmash-recovery
29i-plant-stem-charge
29j-plant-stem-extend
29k-plant-grab
29l-plant-ptooie-hold
29m-plant-ptooie-shot
29n-plant-poison-charge
29o-plant-poison-cloud
29p-plant-copter
29q-plant-hold-brawler
29r-plant-ledge
29s-plant-walk
29t-plant-run
29u-plant-jump-load
29v-plant-jump-extend
29w-plant-air-rise
29x-plant-air-fall
29y-plant-landing-impact
29z-plant-landing-recover
30a-throbbin-levin-smash
30b-throbbin-levin-rising
30c-throbbin-levin-sweep
30d-throbbin-levin-nair
30e-throbbin-levin-fair
30f-throbbin-levin-drop
30g-throbbin-thunder-charge
30h-throbbin-thunder-cast
30i-throbbin-arcfire
30j-throbbin-elwind
30k-throbbin-nosferatu
30l-throbbin-jab
30m-throbbin-bronze-smash
31a-stratus-fsmash
31b-stratus-usmash
31c-stratus-dsmash
31d-stratus-nair
31e-stratus-fair
31f-stratus-bair
31g-stratus-uair
31h-stratus-dair
31i-stratus-jab
31j-stratus-ftilt
31k-stratus-utilt
31l-stratus-dtilt
31m-stratus-dashatk
31n-stratus-grab
31o-stratus-blade-beam
31p-stratus-limit-blade-beam
31q-stratus-cross-slash
31r-stratus-limit-cross-slash
31s-stratus-climhazzard
31t-stratus-limit-climhazzard
31u-stratus-limit-charge
31v-stratus-finishing-touch
32a-sneak-jab
32b-sneak-knee
32c-sneak-body-blow
32d-sneak-rising-kick
32e-sneak-rpg
32f-sneak-mortar
32g-sneak-double-kick
32h-sneak-air-kick
32i-sneak-knee-drop
32j-sneak-drill
32k-sneak-grenade-pull
32l-sneak-nikita
32m-sneak-cypher
32n-sneak-c4-plant
32o-sneak-c4-detonate
32p-sneak-grenade-flight
32q-sneak-nikita-flight
32r-sneak-cypher-rise
32s-sneak-c4-blast
33a-bongo-jab
33b-bongo-double-arm
33c-bongo-slap-beat
33d-bongo-triple-attack
33e-bongo-turning-leg
33f-bongo-roll
33g-bongo-screw-kick
33h-bongo-backspin
33i-bongo-turn-kick
33j-bongo-meteor
33k-bongo-peanut-charge
33l-bongo-peanut-shot
33m-bongo-monkey-flip
33n-bongo-rocketbarrel
33o-bongo-banana
33p-bongo-grab
33q-bongo-monkey-kick
33r-bongo-banana-settled
34a-verdant-areadbhar-smash
34b-verdant-creator-pillar
34c-verdant-aymr-sweep
34d-verdant-whip-sweep
34e-verdant-creator-slash
34f-verdant-lance-cut
34g-verdant-reverse-lance
34h-verdant-aymr-drop
34i-verdant-quiver-spin
34j-verdant-failnaught-draw
34k-verdant-failnaught-loose
34l-verdant-areadbhar-rush
34m-verdant-creator-tether
34n-verdant-aymr-slam
34o-verdant-creator-jab
35a-duchess-golf-club
35b-duchess-crown-knuckle
35c-duchess-royal-spin
35d-duchess-rainbow-arc
35e-duchess-piston-kick
35f-duchess-iron-hip
35g-duchess-toad-guard
35h-duchess-peach-bomber
35i-duchess-parasol
35j-duchess-vegetable-pluck
35k-duchess-turnip-toss
35l-duchess-underfoot-kick
35m-duchess-ribbon-sweep
35n-duchess-royal-wave
35o-duchess-float
35p-duchess-turnip-flight
35q-duchess-parasol-descent
";
        assert_eq!(names, expected.lines().collect::<Vec<_>>());
    }
}
