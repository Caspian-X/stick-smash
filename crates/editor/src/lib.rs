//! Content editor: a mode of the game, reached from the main menu (keyboard and mouse), with two halves sharing one
//! frame (top bar, undo, saving, dialogs, toasts, shortcut sheet):
//! - **Fighters**: browse a fighter's moves, throws and poses; scrub a real-sim preview; edit hitboxes, phases,
//!   poses and stats directly; test in training.
//! - **Stages**: drag the ground, platforms, spawns and bounds over the real rendered stage (snapping, symmetry), a
//!   sandbox of CPUs playing on it, art through schema forms; test in training or a match.
//!
//! Both undo, validate with the real loader and save into the user content dir, patching only what changed.
//! The model (`doc`, `preview`, `view`, `stage_view`) is plain Rust; the egui layer is `ui` (frame, top bar,
//! shortcuts, dialogs), `browser`, `viewport`, `timeline`, `inspect`, `form` (schema-driven forms) and the stage
//! editor's `stage_ed` (state, sandbox, actions), `stage_port` (viewport) and `stage_panels` (browser, inspector).
//! See docs/DESIGN.md "Editor".

mod browser;
pub mod doc;
mod form;
mod inspect;
mod overlay;
mod pane;
pub mod preview;
mod section;
pub mod stage_ed;
mod stage_panels;
mod stage_port;
pub mod stage_view;
mod style;
mod timeline;
mod ui;
pub mod view;
mod viewport;

use std::path::PathBuf;
use std::sync::Arc;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use sim::GameData;
use sim::data::moves::{Throw, Throws};
use sim::data::stage::Stage;

use doc::{Content, Files, Kind, Workspace};
pub use pane::Pane;
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
    /// Quit the game (after the unsaved-changes prompt).
    Quit,
    /// Play with this (unsaved) content, then come back: training, or a match against a CPU (`versus`). `None`
    /// fighter or stage: the player's usual pick.
    Test { data: Arc<GameData>, fighter: Option<String>, stage: Option<String>, versus: bool },
}

/// Which half of the editor is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Fighters,
    Stages,
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
    /// Leave, asking first if anything is unsaved.
    Back,
    Help(bool),
    Search(String),
    Onion(bool),
    /// An undoable edit of the current fighter.
    Edit(fn(&mut sim::data::Fighter)),
    Dialog(Dialog),
    /// Close any dialog.
    Dismiss,
    Undo,
    Mode(Mode),
    Stage(String),
    /// Select a stage part (and show its section).
    Part(Option<stage_view::Part>),
    Section(stage_ed::Section),
    /// An undoable edit of the current stage.
    EditStage(fn(&mut Stage)),
    /// CPUs in the stage sandbox (0 = off).
    Sandbox(usize),
    Grid(bool),
    Symmetry(bool),
    /// Frame the stage in the viewport.
    Reframe,
}

/// A modal dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    /// Unsaved changes before leaving (`quit`: the game).
    Leave {
        quit: bool,
    },
    NewFighter {
        id: String,
        name: String,
        extends: String,
        error: Option<String>,
    },
    RenamePose {
        old: String,
        new: String,
    },
    /// Copy the move in `from` over another slot.
    CopyMove {
        from: String,
        to: Option<String>,
    },
    /// A new stage copied from `from` (also duplicate).
    NewStage {
        id: String,
        name: String,
        from: String,
        error: Option<String>,
    },
    RenameStage {
        id: String,
        new_id: String,
        name: String,
        error: Option<String>,
    },
}

pub(crate) struct Toast {
    pub msg: String,
    pub error: bool,
    pub left: f32,
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
    let mut files = Files::default();
    for root in roots {
        let (sources, errors) = sim::data::sources(root);
        for e in errors {
            error!("{e}");
        }
        for source in sources {
            files.0.insert(source.path, source.text);
        }
    }
    files
}

/// Every move slot, in browser order.
pub const SLOTS: [&str; 19] = [
    "jab", "ftilt", "utilt", "dtilt", "dashatk", "fsmash", "usmash", "dsmash", "nair", "fair", "bair", "uair", "dair",
    "nspec", "sspec", "uspec", "dspec", "grab", "dashgrab",
];

pub fn throw_ref<'a>(t: &'a Throws, k: &str) -> &'a Throw {
    match k {
        "b" => &t.b,
        "u" => &t.u,
        "d" => &t.d,
        _ => &t.f,
    }
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
#[derive(Debug)]
pub(crate) enum Drag {
    Pan,
    Part {
        h: stage_view::Handle,
        orig: Box<Stage>,
        grab: [f64; 2],
        partner: Option<stage_view::Part>,
    },
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
    pub mode: Mode,
    /// Undo or redo moved a document this frame: its change isn't a new edit.
    moved_history: bool,
    /// The stage editor's state.
    pub st: stage_ed::StageEd,
    /// Problems with other content at load (shown, not fatal).
    pub load_errors: Vec<String>,
    pub cur: String,
    pub subject: Subject,
    pub setup: Setup,
    /// Inspector selection: phase of the current move and a hitbox in it.
    pub phase: usize,
    pub hitbox: Option<usize>,
    pub run: Option<Run>,
    pub pane: Pane<Hit, (usize, u32)>,
    pub frame: usize,
    followed: usize,
    pub playing: bool,
    pub speed: f64,
    run_gen: u32,
    pub port: Port,
    pub show_hurt: bool,
    pub show_inactive: bool,
    pub onion: bool,
    pub looping: bool,
    pub search: String,
    focus_search: bool,
    pub dialog: Option<Dialog>,
    toasts: Vec<Toast>,
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
        let cur = ws.data.fighters.first().ok_or("no playable fighters")?.id.clone();
        let stage = ws.data.stages.first().ok_or("no stages")?.id.clone();
        let mut ed = Editor {
            ws,
            mode: Mode::Fighters,
            moved_history: false,
            st: stage_ed::StageEd::new(&stage),
            load_errors: errors.iter().map(ToString::to_string).collect(),
            cur: String::new(),
            subject: Subject::Move("jab".into()),
            setup: Setup::default(),
            phase: 0,
            hitbox: None,
            run: None,
            pane: Pane::new(Cam { center: [0., 0.], zoom: 3. }),
            frame: 0,
            followed: usize::MAX,
            playing: false,
            speed: 1.,
            run_gen: 0,
            port: Port::default(),
            show_hurt: true,
            show_inactive: true,
            onion: false,
            looping: true,
            search: String::new(),
            focus_search: false,
            dialog: None,
            toasts: vec![],
            problems: vec![],
            to_assets: false,
            help: false,
            config,
        };
        ed.open(&cur);
        ed.open_stage(&stage);
        Ok(ed)
    }

    pub fn stage_doc(&mut self) -> &mut doc::Doc<Stage> {
        let id = self.st.cur.clone();
        self.ws.stage(&id).expect("current stage is loaded")
    }

    pub fn open_stage(&mut self, id: &str) {
        if self.ws.stage(id).is_none() {
            return;
        }
        if self.st.cur != id {
            self.toasts.retain(|t| t.error);
            self.st.sel = None;
            self.st.pane.reframe = true;
            self.problems.clear();
        }
        self.st.cur = id.into();
        self.st.pane.stale = true;
    }

    /// The kind and id of the document being edited.
    pub fn current(&self) -> (Kind, String) {
        match self.mode {
            Mode::Fighters => (Kind::Fighter, self.cur.clone()),
            Mode::Stages => (Kind::Stage, self.st.cur.clone()),
        }
    }

    pub fn set_mode(&mut self, m: Mode) {
        if m != self.mode {
            self.mode = m;
            self.problems.clear();
            self.toasts.retain(|t| t.error);
            self.dialog = None;
            self.invalidate();
        }
    }

    pub fn fighter(&mut self) -> &mut sim::data::Fighter {
        &mut self.doc().item
    }

    fn invalidate(&mut self) {
        self.pane.invalidate();
        self.st.pane.invalidate();
    }

    pub fn doc(&mut self) -> &mut doc::Doc {
        let id = self.cur.clone();
        self.ws.doc(&id).expect("current fighter is loaded")
    }

    pub fn open(&mut self, id: &str) {
        if self.ws.doc(id).is_none() {
            return;
        }
        if self.cur != id {
            // Notes about the previous fighter (undo, saved…) don't apply to this one.
            self.toasts.retain(|t| t.error);
        }
        self.cur = id.into();
        self.select(self.subject.clone());
        self.pane.reframe = true;
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
        self.playing = s.idle();
        self.subject = s;
        self.pane.stale = true;
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
        if !self.pane.stale {
            return;
        }
        self.pane.stale = false;
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

    pub fn say(&mut self, msg: impl Into<String>, error: bool) {
        let msg = msg.into();
        self.toasts.retain(|t| t.msg != msg);
        self.toasts.push(Toast { msg, error, left: if error { style::TOAST_ERR_SECS } else { style::TOAST_SECS } });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    /// Fighters with unsaved changes (names).
    pub fn unsaved(&self) -> Vec<String> {
        self.ws.unsaved()
    }

    /// Asks before leaving the editor or quitting if anything is unsaved; otherwise requests it right away.
    pub fn leave(&mut self, quit: bool, out: &mut Outbox) {
        if self.unsaved().is_empty() {
            out.request = Some(if quit { Request::Quit } else { Request::Back });
        } else {
            self.dialog = Some(Dialog::Leave { quit });
        }
    }

    pub fn save(&mut self) -> bool {
        let (kind, id) = self.current();
        self.save_id(kind, &id)
    }

    /// Saves every fighter and stage with unsaved changes; false if any was refused.
    pub fn save_all(&mut self) -> bool {
        let mut ok = true;
        for (kind, id) in self.ws.dirty() {
            ok &= self.save_id(kind, &id);
        }
        ok
    }

    /// Where saves go.
    pub fn save_dir(&self) -> Option<PathBuf> {
        if self.to_assets { Some(self.config.assets.clone()) } else { self.config.user_dir.clone() }
    }

    fn save_id(&mut self, kind: Kind, id: &str) -> bool {
        match kind {
            Kind::Fighter => self.save_as::<sim::data::Fighter>(id),
            Kind::Stage => self.save_as::<Stage>(id),
        }
    }

    fn save_as<T: Content>(&mut self, id: &str) -> bool {
        let id = id.to_string();
        let Some(dir) = self.save_dir() else {
            self.say("Saving is disabled (no user content dir)", true);
            return false;
        };
        match self.ws.prepare_save::<T>(&id) {
            doc::Saved::Refused(errors) => {
                let n = errors.len().max(1);
                self.say(format!("{id} not saved: {n} problem{}", if n == 1 { "" } else { "s" }), true);
                if self.current() == (T::KIND, id.clone()) {
                    self.problems = errors;
                }
                false
            }
            doc::Saved::Ok { text, data } => {
                let path = dir.join(T::path(&id));
                let res =
                    path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, &text));
                match res {
                    Ok(()) => {
                        self.ws.saved::<T>(&id, text, data);
                        self.invalidate();
                        self.problems.clear();
                        self.say(format!("✔ Saved {}", path.display()), false);
                        true
                    }
                    Err(e) => {
                        self.say(format!("Couldn't write {}: {e}", path.display()), true);
                        false
                    }
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
            ed.invalidate();
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
        ed.pane.shown = None;
        ed.st.pane.shown = None;
        ed.playing = false;
        ed.dialog = None;
    }
}

/// The game's egui style, restored when leaving.
#[derive(Resource, Default)]
struct SavedStyle(Option<Arc<egui::Style>>);

#[derive(SystemParam)]
struct Frame<'w, 's> {
    ctx: EguiContexts<'w, 's>,
    ed: Option<ResMut<'w, Editor>>,
    scene: ResMut<'w, render::Scene>,
    time: Res<'w, Time>,
    out: ResMut<'w, Outbox>,
    inbox: ResMut<'w, Inbox>,
    saved_style: Local<'s, Option<Arc<egui::Style>>>,
    commands: Commands<'w, 's>,
}

fn frame(params: Frame) -> Result {
    let Frame { mut ctx, ed, mut scene, time, mut out, mut inbox, mut saved_style, mut commands } = params;
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
                ed.pane.stale = true;
            }
            Cmd::Back => ed.leave(false, &mut out),
            Cmd::Help(on) => ed.help = on,
            Cmd::Search(q) => ed.search = q,
            Cmd::Onion(on) => ed.onion = on,
            Cmd::Edit(f) => {
                ed.doc().edit(f);
                ed.pane.stale = true;
            }
            Cmd::Dialog(d) => ed.dialog = Some(d),
            Cmd::Dismiss => ed.dialog = None,
            Cmd::Undo => ui::undo(&mut ed, false),
            Cmd::Mode(m) => ed.set_mode(m),
            Cmd::Stage(id) => ed.open_stage(&id),
            Cmd::Part(p) => ed.st.select(p),
            Cmd::Section(s) => ed.st.section = s,
            Cmd::EditStage(f) => {
                ed.stage_doc().edit(f);
                ed.st.pane.stale = true;
            }
            Cmd::Sandbox(n) => ed.st.set_sandbox(n),
            Cmd::Grid(on) => ed.st.grid = on,
            Cmd::Symmetry(on) => ed.st.symmetry = on,
            Cmd::Reframe => ed.st.pane.reframe = true,
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
    let w =
        sim::World::new(sim::MatchConfig::sandbox("arena", vec![sim::PlayerConfig::human(&id)], 12345), &d).unwrap();
    w.fighters[0].clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use preview::Section;
    use stage_view::Part;
    use std::path::Path;
    const SANDBOX_MAX_TEST: usize = 1;

    /// Drives the whole egui layer headlessly through every kind of subject and dialog, a few frames each.
    #[test]
    fn every_screen_draws() {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let config = EditorConfig { roots: vec![assets.clone()], assets: assets.clone(), user_dir: None, dev: false };
        let mut ed = Editor::new(config).unwrap();
        let mut scene = render::Scene::new(ed.ws.data.clone());
        let mut out = Outbox::default();
        let mut leave = Outbox::default();
        let ctx = egui::Context::default();
        ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
        style::install_fonts(&ctx, &assets);
        let base = ctx.global_style();
        let mut frame = |ed: &mut Editor| {
            for _ in 0..3 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280., 720.))),
                    ..Default::default()
                };
                ctx.begin_pass(input);
                ctx.set_global_style(style::style(&base));
                ui::run(&ctx, ed, &mut scene, 1. / 60., &mut out);
                ctx.end_pass().textures_delta.clear();
            }
        };
        let ids: Vec<String> = ed.ws.data.fighters.iter().map(|f| f.id.clone()).collect();
        for id in &ids {
            ed.open(id);
            let mut subjects: Vec<Subject> = SLOTS.iter().map(|s| Subject::Move(s.to_string())).collect();
            subjects.extend(["f", "b", "u", "d"].map(|k| Subject::Throw(k.into())));
            subjects.push(Subject::Fighter);
            subjects.extend(Section::ALL.map(Subject::Data));
            subjects.extend(ed.ws.docs[id].item.poses.keys().take(2).map(|n| Subject::Pose(n.clone())));
            for s in subjects {
                ed.select(s);
                ed.hitbox = Some(0);
                ed.onion = true;
                frame(&mut ed);
                ed.set_frame(8);
                frame(&mut ed);
            }
        }
        ed.search = "air".into();
        ed.help = true;
        ed.doc().edit(|f| {
            f.moves.jab.phases[0].transitions.push(sim::data::Transition {
                on: sim::data::Trigger::Timeout,
                cond: None,
                from: None,
                to: "nowhere".into(),
                lag: None,
            })
        });
        ed.select(Subject::Move("jab".into()));
        frame(&mut ed);
        for d in [
            Dialog::Leave { quit: true },
            Dialog::NewFighter { id: "x".into(), name: "X".into(), extends: "base".into(), error: Some("e".into()) },
            Dialog::RenamePose { old: "stand".into(), new: "stand2".into() },
            Dialog::CopyMove { from: "jab".into(), to: Some("ftilt".into()) },
        ] {
            ed.dialog = Some(d);
            frame(&mut ed);
        }
        ed.leave(false, &mut leave);
        assert!(matches!(ed.dialog, Some(Dialog::Leave { quit: false })), "unsaved edits ask before leaving");
        ed.dialog = None;
        ed.doc().revert();
        // The stage editor: every section, selections, the sandbox, aids, dialogs.
        ed.set_mode(Mode::Stages);
        let id = ed.st.cur.clone();
        for sec in stage_ed::Section::ALL {
            ed.st.section = sec;
            frame(&mut ed);
        }
        for p in [
            Part::Block(0),
            Part::Platform(1),
            Part::Waypoint(1, 1),
            Part::Spawn(2),
            Part::Respawn(0),
            Part::Ledge,
            Part::Blast,
        ] {
            ed.st.select(Some(p));
            ed.st.grid = true;
            ed.st.symmetry = true;
            frame(&mut ed);
        }
        ed.st.set_sandbox(2);
        frame(&mut ed);
        assert!(ed.st.world.as_ref().is_some_and(|w| w.fighters.len() == 2));
        stage_ed::add_platform(&mut ed);
        ed.stage_doc().edit(|s| s.spawns.truncate(3));
        frame(&mut ed);
        assert!(ed.ws.unsaved().iter().any(|n| n.contains("stage")));
        for d in [
            Dialog::Leave { quit: false },
            Dialog::NewStage { id: "x".into(), name: "X".into(), from: id.clone(), error: Some("e".into()) },
            Dialog::RenameStage { id: id.clone(), new_id: id.clone(), name: "Y".into(), error: None },
        ] {
            ed.dialog = Some(d);
            frame(&mut ed);
        }
        ed.dialog = None;
        ed.help = true;
        frame(&mut ed);
        ed.leave(false, &mut leave);
        assert!(matches!(ed.dialog, Some(Dialog::Leave { quit: false })), "unsaved stages ask too");
        ed.dialog = None;
        ed.stage_doc().revert();
        ed.leave(false, &mut leave);
        assert!(matches!(leave.request, Some(Request::Back)));
    }

    fn editor() -> Editor {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        Editor::new(EditorConfig { roots: vec![assets.clone()], assets, user_dir: None, dev: false }).unwrap()
    }

    #[test]
    fn viewing_empty_zoom_does_not_edit_the_stage() {
        let mut ed = editor();
        ed.set_mode(Mode::Stages);
        ed.st.section = stage_ed::Section::Bounds;
        ed.st.data = Some(ed.ws.data.clone());
        ed.stage_doc().item.zoom = Some(sim::data::stage::Zoom::default());
        let before = ed.stage_doc().item.clone();
        let ctx = egui::Context::default();
        ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
        style::install_fonts(&ctx, &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
        ctx.run_ui(Default::default(), |ui| stage_panels::inspector(ui, &mut ed)).textures_delta.clear();
        assert_eq!(ed.stage_doc().item, before);
        assert!(!ed.stage_doc().can_undo());
    }

    #[test]
    fn dragging_reports_changes_to_only_the_mirror_partner() {
        let mut ed = editor();
        ed.open_stage("arena");
        let orig = ed.stage_doc().item.clone();
        let p = &orig.platforms[0];
        let grab = [(p.x1 + p.x2) / 2., p.y];
        let drag = Drag::Part {
            h: stage_view::Handle { part: Part::Platform(0), grip: stage_view::Grip::Move },
            orig: Box::new(orig),
            grab,
            partner: Some(Part::Platform(1)),
        };
        ed.stage_doc().item.platforms[1].y += 10.;
        let snap = stage_view::Snap { grid: None, fine: false };
        assert!(stage_ed::drag_to(&mut ed, &drag, grab, snap));
        assert!(!stage_ed::drag_to(&mut ed, &drag, grab, snap));
    }

    #[test]
    fn toolbar_undo_isnt_recorded_as_an_edit() {
        let mut ed = editor();
        ed.set_mode(Mode::Stages);
        ed.stage_doc().edit(|s| s.blocks[0].top -= 10.);
        // As in a frame: snapshots, then the toolbar's undo, then recording.
        let before = ui::Snapshot::take(&mut ed);
        ui::undo(&mut ed, false);
        ui::record(&mut ed, before, false);
        let d = ed.stage_doc();
        assert!(d.can_redo() && !d.can_undo() && !d.dirty(), "undo stays undone and redo stays available");
    }

    #[test]
    fn sandbox_survives_geometry_edits_under_the_cpus() {
        let mut ed = editor();
        ed.set_mode(Mode::Stages);
        ed.st.set_sandbox(SANDBOX_MAX_TEST);
        let mut scene = render::Scene::new(ed.ws.data.clone());
        let step = |ed: &mut Editor, scene: &mut render::Scene, n: usize| {
            for _ in 0..n {
                stage_ed::sync_scene(ed, scene, 1. / 60.);
            }
        };
        step(&mut ed, &mut scene, 600);
        // Put a CPU on platform 2, then delete every platform and most respawns while it stands there.
        {
            let w = ed.st.world.as_mut().unwrap();
            let p = &ed.ws.data.stage("arena").unwrap().platforms[2];
            let f = &mut w.fighters[0];
            (f.x, f.y, f.vx, f.vy, f.ground) = ((p.x1 + p.x2) / 2., p.y, 0., 0., true);
            f.surf = Some(ed.ws.data.stage("arena").unwrap().blocks.len() + 2);
        }
        ed.stage_doc().edit(|s| {
            s.platforms.clear();
            s.respawns.truncate(1);
        });
        ed.st.pane.stale = true;
        step(&mut ed, &mut scene, 120);
        let w = ed.st.world.as_ref().unwrap();
        assert!(w.fighters.iter().all(|f| f.surf.is_none_or(|i| i < 1)), "the world was rebuilt on the new geometry");
    }

    #[test]
    fn mirror_and_the_motion_clock_stay_coherent() {
        let mut ed = editor();
        ed.set_mode(Mode::Stages);
        // Adding a path with Mirror on gives the partner platform the reflected path in one undo step.
        ed.st.symmetry = true;
        ed.st.select(Some(Part::Platform(0)));
        stage_ed::add_waypoint(&mut ed);
        stage_ed::add_waypoint(&mut ed);
        let s = &ed.stage_doc().item;
        let (a, b) = (s.platforms[0].motion.as_ref().unwrap(), s.platforms[1].motion.as_ref().unwrap());
        assert_eq!(a.waypoints.len(), 3);
        assert!(a.waypoints.iter().zip(&b.waypoints).all(|(p, q)| p.x == -q.x && p.y == q.y));
        ed.stage_doc().undo();
        assert_eq!(ed.stage_doc().item.platforms[1].motion.as_ref().unwrap().waypoints.len(), 2);
        // Ledge toggles go through the mirror too.
        stage_ed::toggle_ledge(&mut ed, 0, false);
        assert!(!ed.stage_doc().item.blocks[0].right_ledge, "the centred block's corners follow each other");
        // With CPUs on, scrubbing is ignored and rebuilds start the world at frame 0.
        let mut scene = render::Scene::new(ed.ws.data.clone());
        ed.st.set_sandbox(1);
        for _ in 0..30 {
            stage_ed::sync_scene(&mut ed, &mut scene, 1. / 60.);
        }
        let f = ed.st.world.as_ref().unwrap().frame;
        ed.st.scrub = Some(f + 100);
        stage_ed::sync_scene(&mut ed, &mut scene, 0.);
        assert!(ed.st.world.as_ref().unwrap().frame < f + 100);
        ed.stage_doc().edit(|s| s.platforms[0].y -= 10.);
        ed.st.pane.stale = true;
        stage_ed::sync_scene(&mut ed, &mut scene, 0.);
        assert_eq!(ed.st.world.as_ref().unwrap().frame, 0);
    }
}
