//! The stage editor's state and actions: selection, view options, the live preview World (optionally with CPUs
//! playing on the edited stage through the real sim), and the edits shortcuts and buttons make.

use std::sync::Arc;

use sim::data::stage::Stage;
use sim::world::State;
use sim::{GameData, Input, MatchConfig, PlayerConfig, World, step_world};

use crate::Editor;
use crate::pane::Pane;
use crate::stage_view::{self, Grip, Part, Shape, Snap};
use crate::view::{Cam, Zoom};

/// Inspector / browser sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Overview,
    Blocks,
    Platforms,
    Ledge,
    Bounds,
    Spawns,
    Respawns,
    Art,
    Background,
}

impl Section {
    pub const ALL: [Section; 9] = [
        Section::Overview,
        Section::Blocks,
        Section::Platforms,
        Section::Ledge,
        Section::Bounds,
        Section::Spawns,
        Section::Respawns,
        Section::Art,
        Section::Background,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Section::Overview => "Overview",
            Section::Blocks => "Blocks",
            Section::Platforms => "Platforms",
            Section::Ledge => "Ledge zone",
            Section::Bounds => "Blast & camera",
            Section::Spawns => "Spawns",
            Section::Respawns => "Respawns",
            Section::Art => "Art",
            Section::Background => "Background",
        }
    }
    /// JSON keys it covers (for dirty and problem markers).
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            Section::Overview => &["name", "width", "height"],
            Section::Blocks => &["blocks"],
            Section::Platforms => &["platforms"],
            Section::Ledge => &["ledge"],
            Section::Bounds => &["blast", "camera", "zoom"],
            Section::Spawns => &["spawns"],
            Section::Respawns => &["respawns"],
            Section::Art => &["art"],
            Section::Background => &["art.background"],
        }
    }
    pub fn of(p: Part) -> Section {
        match p {
            Part::Block(_) => Section::Blocks,
            Part::Platform(_) | Part::Waypoint(..) => Section::Platforms,
            Part::Spawn(_) => Section::Spawns,
            Part::Respawn(_) => Section::Respawns,
            Part::Ledge => Section::Ledge,
            Part::Blast | Part::Camera => Section::Bounds,
        }
    }
}

impl crate::section::Section for Section {
    const ALL: &'static [Self] = &Self::ALL;
    fn keys(self) -> &'static [&'static str] {
        self.keys()
    }
}

pub(crate) use crate::Drag;

pub struct StageEd {
    pub cur: String,
    pub section: Section,
    pub sel: Option<Part>,
    pub pane: Pane<stage_view::Handle, ()>,
    /// Draw the grid and snap to it.
    pub grid: bool,
    pub grid_size: f64,
    /// Mirror edits around the stage's centre x.
    pub symmetry: bool,
    pub show_bounds: bool,
    pub show_spawns: bool,
    /// CPUs playing in the preview (0 = just the stage).
    pub sandbox: usize,
    pub level: u8,
    pub paused: bool,
    /// The preview World and the content it runs on (the last valid edit).
    pub(crate) world: Option<World>,
    pub(crate) data: Option<Arc<GameData>>,
    /// The stage `world` was built on: any geometry or capacity change rebuilds it, since the sim holds indices
    /// into blocks, platforms and spawn slots.
    built: Option<Stage>,
    /// Moving platforms play (with the sandbox off) on the preview's clock.
    pub clock_play: bool,
    /// Jump the preview clock to this frame.
    pub scrub: Option<u64>,
    /// Selection the viewport last saw (selecting a bound frames it).
    pub(crate) seen_sel: Option<Part>,
}

impl StageEd {
    pub fn new(cur: &str) -> Self {
        StageEd {
            cur: cur.into(),
            section: Section::Overview,
            sel: None,
            pane: Pane::new(Cam { center: [640., 360.], zoom: 0.5 }),
            grid: false,
            grid_size: 10.,
            symmetry: false,
            show_bounds: true,
            show_spawns: true,
            sandbox: 0,
            level: 2,
            paused: false,
            world: None,
            data: None,
            built: None,
            clock_play: true,
            scrub: None,
            seen_sel: None,
        }
    }

    pub fn select(&mut self, p: Option<Part>) {
        self.sel = p;
        if let Some(p) = p {
            self.section = Section::of(p);
        }
    }

    pub fn set_sandbox(&mut self, n: usize) {
        if n != self.sandbox {
            self.sandbox = n;
            self.world = None;
            self.pane.stale = true;
        }
    }

    pub fn snap(&self, coarse: bool, fine: bool) -> Snap {
        Snap { grid: (self.grid || coarse).then_some(self.grid_size), fine }
    }
}

/// A preview World on stage `id`: `cpus` CPUs (cycling through the fighters), or one hidden player for just the
/// stage. Training rules, so nobody runs out of stocks.
pub fn sandbox(data: &GameData, id: &str, cpus: usize, level: u8) -> Result<World, String> {
    let st = data.stage(id).ok_or("stage didn't load")?;
    let n = cpus.clamp(1, st.max_players().max(1));
    let players = (0..n)
        .map(|i| {
            let f = &data.fighters[i % data.fighters.len()].id;
            PlayerConfig { cpu: (cpus > 0).then_some(level), ..PlayerConfig::human(f) }
        })
        .collect();
    let cfg = MatchConfig::sandbox(id, players, 7);
    let mut w = World::new(cfg, data)?;
    if cpus == 0 {
        w.fighters[0].state = State::Out;
    }
    Ok(w)
}

/// Brings the preview up to date with the edits and steps the sandbox; feeds the render scene.
pub(crate) fn sync_scene(ed: &mut Editor, scene: &mut render::Scene, dt: f32) {
    let id = ed.st.cur.clone();
    if ed.st.pane.stale {
        ed.st.pane.stale = false;
        let valid = ed.ws.stages.get(&id).is_some_and(|d| d.errors.is_empty());
        if valid || ed.st.data.is_none() {
            let data = Arc::new(ed.ws.live_data());
            let idx = data.stages.iter().position(|s| s.id == id);
            let stage = idx.map(|i| &data.stages[i]);
            let keep = ed.st.world.as_ref().is_some_and(|w| Some(w.stage) == idx && w.cfg.stage == id)
                && ed.st.built.as_ref().zip(stage).is_some_and(|(a, b)| same_geometry(a, b));
            if !keep {
                // Just the stage keeps its clock, so moving platforms don't jump back while editing. With CPUs the
                // world restarts at frame 0, where they were placed: a later clock would move their platforms
                // out from under them.
                let frame = ed.st.world.as_ref().filter(|_| ed.st.sandbox == 0).map_or(0, |w| w.frame);
                match sandbox(&data, &id, ed.st.sandbox, ed.st.level) {
                    Ok(mut w) => {
                        w.frame = frame;
                        ed.st.world = Some(w);
                        ed.st.built = stage.cloned();
                    }
                    Err(e) => ed.say(e, true),
                }
            }
            ed.st.data = Some(data);
            ed.st.pane.shown = None;
        }
    }
    let (Some(w), Some(data)) = (ed.st.world.as_mut(), ed.st.data.clone()) else { return };
    // Scrubbing only moves the clock, so it's for the stage alone; with CPUs it would desync riders.
    if let Some(f) = ed.st.scrub.take().filter(|_| ed.st.sandbox == 0) {
        w.frame = f;
        ed.st.clock_play = false;
        ed.st.pane.shown = None;
    }
    if ed.st.pane.shown.is_none() {
        scene.data = data.clone();
        scene.reset(w);
        ed.st.pane.shown = Some(());
    }
    let moving = data.stages[w.stage].platforms.iter().any(|p| p.motion.is_some());
    if ed.st.sandbox == 0 && moving && ed.st.clock_play {
        // Just the stage: advance the clock so moving platforms play.
        ed.st.pane.acc += f64::from(dt) * f64::from(sim::TICK_HZ);
        let mut steps = 0;
        while ed.st.pane.acc >= 1. && steps < 4 {
            ed.st.pane.acc -= 1.;
            steps += 1;
            w.frame += 1;
            scene.push(w);
        }
        ed.st.pane.acc = ed.st.pane.acc.min(1.);
        scene.alpha = ed.st.pane.acc;
    } else if ed.st.sandbox > 0 && !ed.st.paused {
        ed.st.pane.acc += f64::from(dt) * f64::from(sim::TICK_HZ);
        let mut steps = 0;
        while ed.st.pane.acc >= 1. && steps < 4 {
            ed.st.pane.acc -= 1.;
            steps += 1;
            let inputs = vec![Input::default(); w.fighters.len()];
            step_world(w, &data, &inputs);
            scene.push(w);
            w.fx.clear();
            w.hits.clear();
        }
        ed.st.pane.acc = ed.st.pane.acc.min(1.);
        scene.alpha = ed.st.pane.acc;
    } else {
        ed.st.pane.acc = 0.;
        scene.alpha = 1.;
    }
    crate::pane::sync_scene(scene, ed.st.pane.cam, &ed.port);
    scene.pose_override = None;
}

/// Whether the sim-relevant geometry (blocks, platforms, spawn and respawn slots) is the same; art, names and
/// bounds don't hold indices.
fn same_geometry(a: &Stage, b: &Stage) -> bool {
    a.blocks == b.blocks && a.platforms == b.platforms && a.spawns == b.spawns && a.respawns == b.respawns
}

/// Frames the stage: the blocks, platforms (and their paths) and respawn points with room around them (the blast zone is a wheel
/// away).
pub(crate) fn frame_stage(ed: &mut Editor) {
    let Some(s) = ed.ws.stages.get(&ed.st.cur).map(|d| &d.item) else { return };
    let mut bb = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    let blocks = s.blocks.iter().flat_map(|g| [[g.left, g.top], [g.right, g.bottom + g.under_depth]]);
    let platforms = s.platforms.iter().flat_map(|p| {
        let offsets: Vec<(f64, f64)> =
            p.motion.as_ref().map_or_else(|| vec![(0., 0.)], |m| m.waypoints.iter().map(|w| (w.x, w.y)).collect());
        offsets.into_iter().flat_map(move |(dx, dy)| [[p.x1 + dx, p.y + dy], [p.x2 + dx, p.y + dy]])
    });
    for [x, y] in blocks.chain(platforms).chain(s.respawns.iter().map(|r| [r.x, r.start_y])) {
        bb = [bb[0].min(x), bb[1].min(y), bb[2].max(x), bb[3].max(y)];
    }
    let pad = crate::style::FRAME_PAD;
    frame_bbox(&mut ed.st.pane.cam, &ed.port, [bb[0] - pad, bb[1] - pad, bb[2] + pad, bb[3] + pad]);
}

/// Fits stage rect `[l, t, r, b]` in the viewport (below the toolbars).
pub(crate) fn frame_rect(ed: &mut Editor, [l, t, r, b]: [f64; 4]) {
    let (dx, dy) = ((r - l) * crate::style::BOUND_PAD_RATIO, (b - t) * crate::style::BOUND_PAD_RATIO);
    frame_bbox(&mut ed.st.pane.cam, &ed.port, [l - dx, t - dy, r + dx, b + dy]);
}

fn frame_bbox(cam: &mut Cam, port: &crate::view::Port, [l, t, r, b]: [f64; 4]) {
    let [pl, pt, pr, pb] = port.rect;
    let (w, h) = ((pr - pl).max(1.), (pb - pt - crate::style::FRAME_TOP).max(1.));
    cam.zoom = Zoom::STAGE.clamp((w / (r - l)).min(h / (b - t)));
    cam.center = [(l + r) / 2., (t + b) / 2. - crate::style::FRAME_OFFSET / cam.zoom];
}

pub(crate) fn stage_mut(ed: &mut Editor) -> &mut Stage {
    &mut ed.stage_doc().item
}

/// One undoable edit of the current stage.
pub(crate) fn edit(ed: &mut Editor, f: impl FnOnce(&mut Stage)) {
    ed.stage_doc().edit(f);
    ed.st.pane.stale = true;
}

pub(crate) fn add_platform(ed: &mut Editor) {
    let at = ed.st.pane.cam.center;
    let (sym, sn) = (ed.st.symmetry, ed.st.snap(false, false));
    let mut made = None;
    edit(ed, |s| made = Some(stage_view::add_platform(s, at, sym, sn)));
    ed.st.select(made);
    ed.say("Added a platform · drag it into place", false);
}

pub(crate) fn add_spawn(ed: &mut Editor, respawn: bool) {
    let x = ed.st.pane.cam.center[0].round();
    let mut made = None;
    edit(ed, |s| made = Some(if respawn { stage_view::add_respawn(s, x) } else { stage_view::add_spawn(s, x) }));
    ed.st.select(made);
}

pub(crate) fn delete_selected(ed: &mut Editor) {
    let Some(p) = ed.st.sel else { return };
    let partner = if ed.st.symmetry { stage_view::partner(&ed.stage_doc().item, p) } else { None };
    let mut res = Ok(());
    edit(ed, |s| res = stage_view::delete(s, p, partner));
    match res {
        Ok(()) => {
            ed.st.sel = match p {
                Part::Waypoint(i, _) => Some(Part::Platform(i)),
                _ => None,
            };
            let both = partner.is_some_and(|q| q != p);
            ed.say(format!("Deleted {}{} · ⌘Z to undo", p.label(), if both { " and its mirror" } else { "" }), false);
        }
        Err(e) => ed.say(e, true),
    }
}

pub(crate) fn add_block(ed: &mut Editor) {
    let at = ed.st.pane.cam.center;
    let (sym, sn) = (ed.st.symmetry, ed.st.snap(false, false));
    let mut made = None;
    edit(ed, |s| made = Some(stage_view::add_block(s, at, sym, sn)));
    ed.st.select(made);
    ed.say("Added a block · blocks can't overlap (undersides included)", false);
}

/// Gives the selected platform a path, or another waypoint.
pub(crate) fn add_waypoint(ed: &mut Editor) {
    let Some(i) = ed.st.sel.and_then(Part::platform) else {
        ed.say("Select a platform first", true);
        return;
    };
    let sym = ed.st.symmetry;
    let mut made = None;
    edit(ed, |s| {
        // The mirror partner is found before the path changes, then gets the reflected path in the same step.
        let partner = if sym { stage_view::partner(s, Part::Platform(i)) } else { None };
        made = stage_view::add_waypoint(s, i);
        if let Some(q @ Part::Platform(k)) = partner
            && k != i
        {
            stage_view::apply_mirror(s, Part::Platform(i), q, Grip::Move);
        }
    });
    ed.st.select(made);
    ed.st.clock_play = true;
}

pub(crate) fn toggle_ledge(ed: &mut Editor, i: usize, right: bool) {
    let sym = ed.st.symmetry;
    edit(ed, |s| stage_view::toggle_ledge(s, i, right, sym));
}

pub(crate) fn duplicate_selected(ed: &mut Editor) {
    let Some(p) = ed.st.sel else { return };
    let mut made = None;
    edit(ed, |s| made = stage_view::duplicate(s, p));
    if made.is_some() {
        ed.st.select(made);
    }
}

/// Moves the selection by `d` stage pixels (mirrored with symmetry on).
pub(crate) fn nudge(ed: &mut Editor, d: [f64; 2]) {
    let Some(p) = ed.st.sel else { return };
    let sym = ed.st.symmetry;
    edit(ed, |s| {
        let Some(sh) = stage_view::shape(s, p) else { return };
        let partner = if sym { stage_view::partner(s, p) } else { None };
        let fine = Snap { grid: None, fine: true };
        stage_view::set_shape(s, p, stage_view::drag_shape(sh, Grip::Move, d, fine));
        if let Some(q) = partner {
            stage_view::apply_mirror(s, p, q, Grip::Move);
        }
    });
}

pub(crate) fn flip(ed: &mut Editor, i: usize) {
    let sym = ed.st.symmetry;
    edit(ed, |s| {
        let partner = if sym { stage_view::partner(s, Part::Spawn(i)) } else { None };
        if let Some(sp) = s.spawns.get_mut(i) {
            sp.face = -sp.face;
        }
        if let Some(j) = partner.and_then(|q| match q {
            Part::Spawn(j) if j != i => Some(j),
            _ => None,
        }) {
            s.spawns[j].face = -s.spawns[i].face;
        }
    });
}

pub(crate) struct MirrorPaths(Vec<(Option<sim::data::stage::Motion>, Option<Part>)>);

pub(crate) fn mirror_paths(s: &Stage) -> MirrorPaths {
    MirrorPaths(
        s.platforms
            .iter()
            .enumerate()
            .map(|(i, p)| (p.motion.clone(), stage_view::partner(s, Part::Platform(i))))
            .collect(),
    )
}

pub(crate) fn fix_mirror_paths(s: &mut Stage, before: MirrorPaths) {
    for (i, (motion, partner)) in before.0.into_iter().enumerate().take(s.platforms.len()) {
        if s.platforms[i].motion != motion
            && let Some(q @ Part::Platform(k)) = partner
            && k != i
        {
            stage_view::apply_mirror(s, Part::Platform(i), q, Grip::Move);
        }
    }
}

enum PartValue {
    Block(sim::data::stage::Ground),
    Platform(sim::data::stage::Platform),
    Spawn(sim::data::stage::Spawn),
    Respawn(sim::data::stage::RespawnPoint),
    Bounds(sim::data::stage::Bounds),
    Ledge(sim::data::stage::LedgeZone),
}

impl PartValue {
    fn of(s: &Stage, p: Part) -> Option<Self> {
        Some(match p {
            Part::Block(i) => Self::Block(s.blocks.get(i)?.clone()),
            Part::Platform(i) | Part::Waypoint(i, _) => Self::Platform(s.platforms.get(i)?.clone()),
            Part::Spawn(i) => Self::Spawn(s.spawns.get(i)?.clone()),
            Part::Respawn(i) => Self::Respawn(s.respawns.get(i)?.clone()),
            Part::Blast => Self::Bounds(s.blast.clone()),
            Part::Camera => Self::Bounds(s.camera.clone()),
            Part::Ledge => Self::Ledge(s.ledge.clone()),
        })
    }
    fn matches(&self, s: &Stage, p: Part) -> bool {
        match (self, p) {
            (Self::Block(v), Part::Block(i)) => s.blocks.get(i) == Some(v),
            (Self::Platform(v), Part::Platform(i) | Part::Waypoint(i, _)) => s.platforms.get(i) == Some(v),
            (Self::Spawn(v), Part::Spawn(i)) => s.spawns.get(i) == Some(v),
            (Self::Respawn(v), Part::Respawn(i)) => s.respawns.get(i) == Some(v),
            (Self::Bounds(v), Part::Blast) => s.blast == *v,
            (Self::Bounds(v), Part::Camera) => s.camera == *v,
            (Self::Ledge(v), Part::Ledge) => s.ledge == *v,
            _ => false,
        }
    }
}

/// Applies a viewport drag at stage point `m`. Returns whether the stage changed (snapping often holds it still).
pub(crate) fn drag_to(ed: &mut Editor, drag: &Drag, m: [f64; 2], sn: Snap) -> bool {
    let Drag::Part { h, orig, grab, partner } = drag else { return false };
    let s = stage_mut(ed);
    let before = PartValue::of(s, h.part);
    let mirrored = partner.filter(|q| *q != h.part).and_then(|q| PartValue::of(s, q));
    match h.grip {
        Grip::Rim => {
            s.ledge.r = sn
                .v(stage_view::ledges(orig)
                    .iter()
                    .map(|(c, ..)| (m[0] - c[0]).hypot(m[1] - c[1]))
                    .fold(f64::INFINITY, f64::min))
                .max(1.)
        }
        Grip::Start => {
            if let Part::Respawn(i) = h.part
                && let Some(r) = s.respawns.get_mut(i)
            {
                r.start_y = sn.v(m[1]);
            }
        }
        Grip::Face | Grip::Ledge(_) => return false,
        grip => {
            let Some(sh) = stage_view::shape(orig, h.part) else { return false };
            let d = [m[0] - grab[0], m[1] - grab[1]];
            // With symmetry, a self-mirrored part only moves vertically.
            let d = if *partner == Some(h.part) && grip == Grip::Move { [0., d[1]] } else { d };
            stage_view::set_shape(s, h.part, stage_view::drag_shape(sh, grip, d, sn));
        }
    }
    if let Some(q) = *partner {
        stage_view::apply_mirror(s, h.part, q, h.grip);
    }
    before.is_some_and(|v| !v.matches(s, h.part)) || mirrored.zip(*partner).is_some_and(|(v, q)| !v.matches(s, q))
}

/// Live problems (the loader's checks) and notes for the current stage.
pub(crate) fn notes(ed: &Editor) -> Vec<sim::data::ContentError> {
    ed.ws.stages.get(&ed.st.cur).map(|d| stage_view::notes(&d.item, &d.path)).unwrap_or_default()
}

/// Player colors (hex) for spawn markers.
pub(crate) fn colors(d: &GameData) -> Vec<u32> {
    d.global
        .palette
        .iter()
        .map(|p| match p.main {
            sim::data::Color::Rgb(v) => v,
            _ => 0xffffff,
        })
        .collect()
}

/// The shape's readout for the hint line: `x 390 → 570 · y 400`.
pub(crate) fn readout(s: &Stage, p: Part) -> String {
    match stage_view::shape(s, p) {
        Some(Shape::Rect([l, t, r, b])) => format!("{l:.0}, {t:.0} → {r:.0}, {b:.0}  ({:.0} × {:.0})", r - l, b - t),
        Some(Shape::Seg([x1, x2, y])) => format!("x {x1:.0} → {x2:.0} · y {y:.0}  ({:.0} wide)", x2 - x1),
        Some(Shape::Point([x, y])) => format!("{x:.0}, {y:.0}"),
        None => format!("r {:.0}", s.ledge.r),
    }
}

/// Grid sizes offered.
pub const GRIDS: [f64; 5] = [5., 10., 20., 40., 80.];

/// Most CPUs the sandbox runs.
pub(crate) const SANDBOX_MAX: usize = 4;
