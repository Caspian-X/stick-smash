//! Fighter editor: a mode of the game, reached from the main menu (keyboard and mouse). Browse a fighter's moves,
//! throws and poses; scrub a real-sim preview; edit hitboxes, phases, poses and stats directly; undo; validate with
//! the real loader; save into the user content dir with `extends` kept intact; test in training.
//!
//! The model (`doc`, `preview`, `view`) is plain Rust; `ui` is the egui layer. See docs/DESIGN.md "Editor".

pub mod doc;
mod inspect;
pub mod preview;
mod style;
mod ui;
pub mod view;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use sim::GameData;
use sim::data::moves::{Move, Moves, Throw, Throws};

use doc::{Files, Workspace};
use preview::{Run, Setup, Subject};
use view::{Cam, Hit, Port};

/// Where content comes from. Inserted by the game.
#[derive(Resource, Clone, Debug)]
pub struct EditorConfig {
    /// Content roots in load order (built-in assets first).
    pub roots: Vec<PathBuf>,
    /// The built-in assets (fonts, and the dev save target).
    pub assets: PathBuf,
    /// Saves go here (`None`: saving is disabled, e.g. in the screenshot tour).
    pub user_dir: Option<PathBuf>,
    /// Offer saving into `assets/` (dev builds).
    pub dev: bool,
}

/// What the editor asks of the game.
#[derive(Clone)]
pub enum Request {
    /// Leave to the main menu.
    Back,
    /// Play training with this (unsaved) content and fighter, then come back.
    Test { data: Arc<GameData>, fighter: String },
}

#[derive(Resource, Default)]
pub struct Outbox {
    pub request: Option<Request>,
    /// Content after saves, for the game to play from now on (taken when leaving).
    pub reload: Option<Arc<GameData>>,
}

/// Scripted commands (screenshot tour).
#[derive(Clone, Debug)]
pub enum Cmd {
    Fighter(String),
    Subject(Subject),
    Frame(usize),
    Hitbox(Option<usize>),
    Dummy(bool),
    Back,
}

#[derive(Resource, Default)]
pub struct Inbox(pub Vec<Cmd>);

pub struct EditorPlugin<S: States + Copy> {
    pub state: S,
}

impl<S: States + Copy> Plugin for EditorPlugin<S> {
    fn build(&self, app: &mut App) {
        app.init_resource::<Outbox>()
            .init_resource::<SavedStyle>()
            .init_resource::<Inbox>()
            .add_systems(OnEnter(self.state), enter)
            .add_systems(OnExit(self.state), exit)
            .add_systems(EguiPrimaryContextPass, frame.run_if(in_state(self.state)));
    }
}

/// Reads every `.json` under each root (skipping `schema/`), later roots replacing earlier files.
pub fn read_roots(roots: &[PathBuf]) -> Files {
    fn walk(root: &Path, dir: &Path, out: &mut Files) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut paths: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() && p.file_name().is_some_and(|n| n != "schema") {
                walk(root, &p, out);
            } else if p.extension().is_some_and(|e| e == "json")
                && let Ok(text) = std::fs::read_to_string(&p)
            {
                let path = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                out.0.insert(path, text);
            }
        }
    }
    let mut files = Files::default();
    for r in roots {
        walk(r, r, &mut files);
    }
    files
}

pub fn move_ref(m: &Moves, slot: impl AsRef<str>) -> Option<&Move> {
    m.iter().find(|(s, _)| *s == slot.as_ref()).map(|(_, m)| m)
}
pub fn move_mut(m: &mut Moves, slot: impl AsRef<str>) -> Option<&mut Move> {
    Some(match slot.as_ref() {
        "jab" => &mut m.jab,
        "ftilt" => &mut m.ftilt,
        "utilt" => &mut m.utilt,
        "dtilt" => &mut m.dtilt,
        "dashatk" => &mut m.dashatk,
        "fsmash" => &mut m.fsmash,
        "usmash" => &mut m.usmash,
        "dsmash" => &mut m.dsmash,
        "nair" => &mut m.nair,
        "fair" => &mut m.fair,
        "bair" => &mut m.bair,
        "uair" => &mut m.uair,
        "dair" => &mut m.dair,
        "grab" => &mut m.grab,
        "dashgrab" => &mut m.dashgrab,
        "nspec" => &mut m.nspec,
        "sspec" => return m.sspec.as_mut(),
        "uspec" => &mut m.uspec,
        "dspec" => &mut m.dspec,
        _ => return None,
    })
}
pub fn throw_mut<'a>(t: &'a mut Throws, k: &str) -> &'a mut Throw {
    match k {
        "b" => &mut t.b,
        "u" => &mut t.u,
        "d" => &mut t.d,
        _ => &mut t.f,
    }
}

/// An in-progress viewport drag.
#[derive(Clone, Debug)]
pub(crate) enum Drag {
    Pan,
    /// Hitbox `i` of phase `phase`: move, resize or aim, from the fighter as it was on the dragged frame.
    Box {
        kind: Hit,
        phase: usize,
        t: u32,
        f: Box<sim::world::Fighter>,
        grab: [f64; 2],
        orig: (f64, f64, f64),
    },
    Joint {
        j: usize,
        target: ui::PoseTarget,
        f: Box<sim::world::Fighter>,
        skel: render::pose::Skel,
    },
}

#[derive(Resource)]
pub struct Editor {
    pub ws: Workspace,
    /// Problems with other content at load (shown, not fatal).
    pub load_errors: Vec<String>,
    pub cur: String,
    pub subject: Subject,
    pub setup: Setup,
    /// Inspector selection: phase of the current move and a hitbox in it.
    pub phase: usize,
    pub hitbox: Option<usize>,
    pub run: Option<Run>,
    stale: bool,
    pub frame: usize,
    followed: usize,
    pub playing: bool,
    pub speed: f64,
    acc: f64,
    shown: Option<(usize, u32)>,
    run_gen: u32,
    pub cam: Cam,
    pub port: Port,
    reframe: bool,
    drag: Option<Drag>,
    hover: Option<Hit>,
    pub show_hurt: bool,
    pub show_inactive: bool,
    /// Message, is-error, seconds left.
    status: Option<(String, bool, f32)>,
    /// Problems that refused the last save.
    pub problems: Vec<sim::data::ContentError>,
    pub to_assets: bool,
    help: bool,
    pub config: EditorConfig,
}

impl Editor {
    pub fn new(config: EditorConfig) -> Result<Self, String> {
        let files = read_roots(&config.roots);
        let (ws, errors) =
            Workspace::new(files).map_err(|e| e.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"))?;
        let cur = ws.data.fighters[0].id.clone();
        let mut ed = Editor {
            ws,
            load_errors: errors.iter().map(ToString::to_string).collect(),
            cur: String::new(),
            subject: Subject::Move("jab".into()),
            setup: Setup::default(),
            phase: 0,
            hitbox: None,
            run: None,
            stale: true,
            frame: 0,
            followed: usize::MAX,
            playing: false,
            speed: 1.,
            acc: 0.,
            shown: None,
            run_gen: 0,
            cam: Cam { center: [0., 0.], zoom: 3. },
            port: Port::default(),
            reframe: true,
            drag: None,
            hover: None,
            show_hurt: true,
            show_inactive: true,
            status: None,
            problems: vec![],
            to_assets: false,
            help: false,
            config,
        };
        ed.open(&cur);
        Ok(ed)
    }

    pub fn doc(&mut self) -> &mut doc::Doc {
        let id = self.cur.clone();
        self.ws.doc(&id).expect("current fighter is loaded")
    }

    pub fn open(&mut self, id: &str) {
        if self.ws.doc(id).is_none() {
            return;
        }
        self.cur = id.into();
        self.select(self.subject.clone());
        self.reframe = true;
    }

    pub fn select(&mut self, s: Subject) {
        if let Subject::Move(slot) = &s {
            self.setup.air = preview::AERIALS.contains(&slot.as_str()).then_some(120.);
        }
        if s != self.subject {
            self.phase = 0;
            self.hitbox = None;
            self.frame = 0;
        }
        self.playing = s == Subject::Fighter;
        self.subject = s;
        self.stale = true;
        self.followed = usize::MAX;
    }

    /// Frames in the preview.
    pub fn frames(&self) -> usize {
        self.run.as_ref().map_or(1, |r| r.frames.len())
    }

    pub fn set_frame(&mut self, f: usize) {
        self.frame = f.min(self.frames() - 1);
        self.playing = false;
    }

    /// Re-simulate if anything changed. Invalid data keeps the last good preview.
    fn refresh(&mut self) {
        if !self.stale {
            return;
        }
        self.stale = false;
        if !self.doc().errors.is_empty() && self.run.is_some() {
            return;
        }
        let data = Arc::new(self.ws.live_data());
        match preview::simulate(data, &self.cur, &self.subject, &self.setup) {
            Ok(r) => {
                self.run = Some(r);
                self.run_gen = self.run_gen.wrapping_add(1);
                self.frame = self.frame.min(self.frames() - 1);
            }
            Err(e) => self.say(e, true),
        }
    }

    fn say(&mut self, msg: impl Into<String>, error: bool) {
        self.status = Some((msg.into(), error, if error { 8. } else { 4. }));
    }

    pub fn save(&mut self) {
        let id = self.cur.clone();
        let dir = if self.to_assets { Some(self.config.assets.clone()) } else { self.config.user_dir.clone() };
        let Some(dir) = dir else {
            self.say("Saving is disabled (no user content dir)", true);
            return;
        };
        match self.ws.prepare_save(&id) {
            doc::Saved::Refused(errors) => {
                self.say(format!("Not saved: {} problem(s)", errors.len().max(1)), true);
                self.problems = errors;
            }
            doc::Saved::Ok { text, data } => {
                let path = dir.join(doc::fighter_path(&id));
                let res =
                    path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, &text));
                match res {
                    Ok(()) => {
                        self.ws.saved(&id, text, data);
                        self.problems.clear();
                        self.say(format!("Saved {}", path.display()), false);
                    }
                    Err(e) => self.say(format!("Couldn't write {}: {e}", path.display()), true),
                }
            }
        }
    }
}

fn enter(mut commands: Commands, ed: Option<ResMut<Editor>>, config: Option<Res<EditorConfig>>, mut ctx: EguiContexts) {
    let Some(config) = config else {
        error!("editor: no EditorConfig");
        return;
    };
    if let Ok(ctx) = ctx.ctx_mut() {
        style::install_fonts(ctx, &config.assets);
    }
    match ed {
        Some(mut ed) => {
            ed.stale = true;
            ed.shown = None;
        }
        None => match Editor::new(config.clone()) {
            Ok(ed) => commands.insert_resource(ed),
            Err(e) => error!("editor: content failed to load: {e}"),
        },
    }
}

fn exit(mut scene: ResMut<render::Scene>, mut ctx: EguiContexts, ed: Option<ResMut<Editor>>, saved: Res<SavedStyle>) {
    scene.fixed_view = None;
    scene.pose_override = None;
    scene.overlay.clear(1., 1.);
    if let (Ok(ctx), Some(s)) = (ctx.ctx_mut(), &saved.0) {
        ctx.set_global_style(s.clone());
    }
    if let Some(mut ed) = ed {
        ed.shown = None;
        ed.playing = false;
    }
}

/// The game's egui style, restored when leaving.
#[derive(Resource, Default)]
struct SavedStyle(Option<Arc<egui::Style>>);

#[allow(clippy::too_many_arguments)]
fn frame(
    mut ctx: EguiContexts,
    ed: Option<ResMut<Editor>>,
    mut scene: ResMut<render::Scene>,
    time: Res<Time>,
    mut out: ResMut<Outbox>,
    mut inbox: ResMut<Inbox>,
    mut saved_style: Local<Option<Arc<egui::Style>>>,
    mut commands: Commands,
) -> Result {
    let ctx = ctx.ctx_mut()?;
    let Some(mut ed) = ed else {
        // Content didn't load: say so and offer the way out.
        egui::CentralPanel::default().show(
            &mut egui::Ui::new(ctx.clone(), "editor-failed".into(), egui::UiBuilder::new()),
            |ui| {
                ui.heading("The editor couldn't load content (see the log).");
                if ui.button("Back").clicked() {
                    out.request = Some(Request::Back);
                }
            },
        );
        return Ok(());
    };
    if saved_style.is_none() {
        let s = ctx.global_style();
        *saved_style = Some(s.clone());
        commands.insert_resource(SavedStyle(Some(s)));
    }
    if let Some(s) = saved_style.as_ref() {
        ctx.set_global_style(style::style(s));
    }
    for c in std::mem::take(&mut inbox.0) {
        match c {
            Cmd::Fighter(id) => ed.open(&id),
            Cmd::Subject(s) => ed.select(s),
            Cmd::Frame(n) => {
                ed.refresh();
                ed.set_frame(n);
            }
            Cmd::Hitbox(h) => ed.hitbox = h,
            Cmd::Dummy(on) => {
                ed.setup.dummy = on;
                ed.stale = true;
            }
            Cmd::Back => out.request = Some(Request::Back),
        }
    }
    ui::run(ctx, &mut ed, &mut scene, time.delta_secs(), &mut out);
    if matches!(out.request, Some(Request::Back)) && ed.ws.saves > 0 {
        out.reload = Some(ed.ws.data.clone());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn tests_fighter() -> sim::world::Fighter {
    let files = doc::tests_files();
    let (d, _) = sim::data::load(files.sources()).unwrap();
    let id = d.fighters[0].id.clone();
    let w = sim::World::new(
        sim::MatchConfig { players: vec![sim::PlayerConfig::human(&id)], training: true, ..Default::default() },
        &d,
    )
    .unwrap();
    w.fighters[0].clone()
}
