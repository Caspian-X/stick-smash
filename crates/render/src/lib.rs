//! Draws a `sim::World` read-only: stick figures from pose data, attachments, projectiles, stage and backdrop,
//! effects from `world.fx`, the follow camera with shake, and the F1 debug overlay. Shared by gameplay and the
//! editor (which sets a fixed view, a pose override and its own overlay geometry).
//!
//! Feed it with [`Scene::push`] once per sim tick (before clearing `world.fx`) and set [`Scene::alpha`] each frame
//! to the fraction of a tick elapsed since the last push; frames between ticks interpolate. Requires `EguiPlugin`
//! for the debug text.

pub use sim::data::camera;
pub mod canvas;
pub mod debug;
pub mod draw;
pub mod fx;
pub mod portrait;
pub mod pose;
pub mod stage;

use std::collections::VecDeque;
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::camera::ScalingMode;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use sim::world::State;
use sim::{GameData, World};

use canvas::Canvas;
use draw::FighterView;
use fx::{CameraRig, FxLayer};

/// Small non-sim RNG for cosmetic randomness (particle spread, jitter, shake, star layout).
#[derive(Clone, Copy, Debug)]
pub struct Rand(pub u64);
impl Default for Rand {
    fn default() -> Self {
        Rand(0x9e37_79b9_7f4a_7c15)
    }
}
impl Rand {
    pub fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + self.unit() * (b - a)
    }
}

/// What to draw. The game owns the live `World`; this keeps the last two tick snapshots.
#[derive(Resource)]
pub struct Scene {
    pub data: Arc<GameData>,
    /// Fraction of a tick elapsed since the last push, 0..1.
    pub alpha: f64,
    /// Debug overlay (hitboxes, hurtboxes, ledge and blast zones, state text).
    pub debug: bool,
    /// Fraction of the window reserved below gameplay (measured by the host UI).
    pub bottom_safe_area: f64,
    /// Fixed camera for the whole window (the editor's pan/zoom) instead of the follow camera. No shake.
    pub fixed_view: Option<fx::Cam>,
    /// Extra geometry in stage pixels drawn above everything (editor handles). The owner clears it.
    pub overlay: Canvas,
    /// Draw fighter `i` with this skeleton instead of its animation (editor pose editing).
    pub pose_override: Option<(usize, pose::Skel)>,
    pending: VecDeque<World>,
    reset: bool,
}

impl Scene {
    pub fn new(data: Arc<GameData>) -> Self {
        Scene {
            data,
            alpha: 1.,
            debug: false,
            bottom_safe_area: 0.,
            fixed_view: None,
            overlay: Canvas::default(),
            pose_override: None,
            pending: VecDeque::new(),
            reset: true,
        }
    }
    /// Record the world after a tick. Its pending `fx` become effects.
    pub fn push(&mut self, w: &World) {
        self.pending.push_back(w.clone());
    }
    /// Start over (new match): no interpolation from the old world, no leftover effects, camera snaps.
    pub fn reset(&mut self, w: &World) {
        self.pending.clear();
        self.reset = true;
        self.push(w);
    }
}

/// What was drawn this frame, for screen-space UI (HUD bubbles, name tags): the visible stage rectangle
/// `[left, top, right, bottom]` (stage pixels, y down, shake included) and each fighter's drawn feet position.
#[derive(Resource, Default, Clone)]
pub struct Framing {
    pub view: [f64; 4],
    pub fighters: Vec<(f64, f64)>,
    pub bottom_safe_area: f64,
}

/// Run after the game's sim step so this frame shows the newest tick.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderSystems;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Drawn>()
            .init_resource::<Framing>()
            .init_resource::<portrait::Portraits>()
            // The portrait camera comes second, so the window camera stays egui's primary context.
            .add_systems(Startup, (setup, portrait::setup).chain())
            .add_systems(Update, ((advance, draw).chain(), portrait::draw).in_set(RenderSystems))
            .add_systems(EguiPrimaryContextPass, debug_labels);
    }
}

#[derive(Component)]
pub struct RenderCamera;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Layer {
    Backdrop,
    Stars,
    Stage,
    World,
    Debug,
    Overlay,
}

/// Render-side state advanced per tick: snapshots, effects, camera.
#[derive(Resource, Default)]
struct Drawn {
    prev: Option<World>,
    cur: Option<World>,
    fx: FxLayer,
    cam: CameraRig,
    stage: Option<usize>,
    /// Content the static layers were built from (the editor swaps in edited stages).
    built: Option<Arc<GameData>>,
    canvas: Canvas,
    rng: Rand,
    shake: (f64, f64),
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<ColorMaterial>>) {
    commands.spawn((Camera2d, RenderCamera, Msaa::Sample4));
    let mat = mats.add(ColorMaterial::default());
    for (layer, z) in [
        (Layer::Backdrop, -40.),
        (Layer::Stars, -30.),
        (Layer::Stage, -20.),
        (Layer::World, 0.),
        (Layer::Debug, 10.),
        (Layer::Overlay, 20.),
    ] {
        let mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
        commands.spawn((
            Mesh2d(mesh),
            MeshMaterial2d(mat.clone()),
            Transform::from_xyz(0., 0., z),
            NoFrustumCulling,
            layer,
        ));
    }
}

fn aspect(win: &Window) -> f64 {
    f64::from(win.width().max(1.)) / f64::from(win.height().max(1.))
}

/// Consume pushed ticks: effects, camera, snapshots.
fn advance(mut scene: ResMut<Scene>, mut st: ResMut<Drawn>, win: Option<Single<&Window, With<PrimaryWindow>>>) {
    let asp = win.as_deref().map_or(16. / 9., |w| aspect(w));
    let scene = &mut *scene;
    let d = scene.data.clone();
    if std::mem::take(&mut scene.reset) {
        st.prev = None;
        st.cur = None;
        st.fx.clear();
        st.cam = CameraRig::default();
    }
    while let Some(mut w) = scene.pending.pop_front() {
        let view = st.cam.cur.map_or([0., 0., d.stages[w.stage].width, d.stages[w.stage].height], |c| c.view(asp));
        let fx = std::mem::take(&mut w.fx);
        st.fx.consume(&d, &w, &fx, view);
        st.fx.tick(&d);
        st.cam.tick(&d, &w, asp / (1. - scene.bottom_safe_area.clamp(0., 0.5)));
        st.prev = st.cur.take();
        st.cur = Some(w);
    }
}

pub(crate) fn upload(mesh: &mut Mesh, cv: &Canvas) {
    let (mut pos, mut col, mut idx) = (cv.pos.clone(), cv.col.clone(), cv.idx.clone());
    if idx.is_empty() {
        pos = vec![[0.; 3]; 3];
        col = vec![[0.; 4]; 3];
        idx = vec![0, 1, 2];
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
}

/// Interpolated fighter views for this frame.
fn fighter_views<'a>(
    d: &GameData,
    prev: Option<&World>,
    cur: &'a World,
    t: f64,
    over: Option<(usize, pose::Skel)>,
) -> Vec<FighterView<'a>> {
    let mut views: Vec<FighterView<'a>> = cur
        .fighters
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let skel = pose::fighter_pose(d, f, cur.frame);
            let p = prev.and_then(|w| w.fighters.get(i)).filter(|p| {
                p.data == f.data && p.face == f.face && p.state == f.state && (p.x - f.x).hypot(p.y - f.y) < 150.
            });
            match p {
                Some(p) => FighterView {
                    i,
                    f,
                    x: p.x + (f.x - p.x) * t,
                    y: p.y + (f.y - p.y) * t,
                    skel: pose::fighter_pose(d, p, prev.map_or(cur.frame, |w| w.frame)).blend(&skel, t),
                },
                None => FighterView { i, f, x: f.x, y: f.y, skel },
            }
        })
        .collect();
    if let Some((i, skel)) = over
        && let Some(v) = views.get_mut(i)
    {
        v.skel = skel;
    }
    views
}

#[allow(clippy::too_many_arguments)]
fn draw(
    scene: Res<Scene>,
    mut st: ResMut<Drawn>,
    mut meshes: ResMut<Assets<Mesh>>,
    layers: Query<(&Mesh2d, &Layer, &mut Transform), Without<RenderCamera>>,
    mut cam: Query<(&mut Transform, &mut Projection), With<RenderCamera>>,
    win: Option<Single<&Window, With<PrimaryWindow>>>,
    mut framing: ResMut<Framing>,
) {
    let st = &mut *st;
    let Some(cur) = st.cur.as_ref() else { return };
    let d = &*scene.data;
    let t = scene.alpha.clamp(0., 1.);
    let px_h = win.as_deref().map_or(720., |w| f64::from(w.physical_height().max(1)));
    let Some(mut c) = scene.fixed_view.or_else(|| st.cam.at(t)) else { return };
    let safe = if scene.fixed_view.is_some() { 0. } else { scene.bottom_safe_area.clamp(0., 0.5) };
    let full_height = c.h / (1. - safe);
    c.y += (full_height - c.h) / 2.;
    c.h = full_height;
    framing.bottom_safe_area = safe;
    let (sx, sy) = if scene.fixed_view.is_some() { (0., 0.) } else { st.fx.shake_offset() };
    st.shake = (sx, sy);
    let s = &d.stages[cur.stage];
    if let Ok((mut tf, mut proj)) = cam.single_mut() {
        tf.translation.x = (c.x + sx) as f32;
        tf.translation.y = -(c.y + sy) as f32;
        if let Projection::Orthographic(o) = &mut *proj {
            o.scaling_mode = ScalingMode::FixedVertical { viewport_height: c.h as f32 };
        }
    }
    let detail = px_h / c.h;
    let parallax = s.art.background.as_ref().map_or(0., |b| b.parallax);
    let par = Vec2::new(((c.x + sx - s.width / 2.) * parallax) as f32, (-(c.y + sy - s.height / 2.) * parallax) as f32);
    let rebuild_static = st.stage != Some(cur.stage) || !st.built.as_ref().is_some_and(|b| Arc::ptr_eq(b, &scene.data));
    st.stage = Some(cur.stage);
    st.built = Some(scene.data.clone());
    let views = fighter_views(d, st.prev.as_ref(), cur, t, scene.pose_override);
    let asp = win.as_deref().map_or(16. / 9., |w| aspect(w));
    let [x0, y0, x1, y1] = fx::Cam { x: c.x + sx, y: c.y + sy, h: c.h }.view(asp);
    framing.view = [x0, y0, x1, y1];
    framing.fighters = views.iter().map(|v| (v.x, v.y)).collect();
    let stage_frame = stage_frame(st.prev.as_ref().map(|p| p.frame), cur.frame, t);
    for (mesh, layer, mut tf) in layers {
        if matches!(layer, Layer::Backdrop | Layer::Stars) {
            tf.translation.x = par.x;
            tf.translation.y = par.y;
        }
        if *layer == Layer::Overlay {
            if let Some(mut m) = meshes.get_mut(&mesh.0) {
                upload(&mut m, &scene.overlay);
            }
            continue;
        }
        let cv = &mut st.canvas;
        cv.clear(detail, d.global.render.alpha_gamma);
        match layer {
            Layer::Backdrop if !rebuild_static => continue,
            Layer::Stage if !rebuild_static && s.platforms.iter().all(|p| p.motion.is_none()) => continue,
            Layer::Backdrop => stage::draw_backdrop(cv, d, s),
            Layer::Stage => stage::draw_stage(cv, d, s, stage_frame),
            Layer::Stars => {
                if let Some(bg) = &s.art.background {
                    stage::draw_stars(cv, d, bg, cur.frame);
                }
            }
            Layer::World => {
                for (k, p) in cur.projs.iter().enumerate() {
                    let pp = st
                        .prev
                        .as_ref()
                        .and_then(|w| w.projs.get(k))
                        .filter(|q| q.owner == p.owner && q.id == p.id && (q.x - p.x).hypot(q.y - p.y) < 80.);
                    let (x, y) = pp.map_or((p.x, p.y), |q| (q.x + (p.x - q.x) * t, q.y + (p.y - q.y) * t));
                    draw::draw_projectile(cv, d, p, x, y, cur.frame);
                }
                for v in &views {
                    draw::draw_fighter(cv, d, cur.frame, v, &mut st.rng);
                }
                st.fx.draw(cv, d, t);
            }
            Layer::Overlay => {}
            Layer::Debug => {
                if scene.debug {
                    let pos: Vec<_> = views.iter().map(|v| (v.x, v.y)).collect();
                    debug::draw_debug(cv, d, cur, &pos);
                }
            }
        }
        if let Some(mut m) = meshes.get_mut(&mesh.0) {
            upload(&mut m, cv);
        }
    }
}

fn debug_labels(
    mut ctx: EguiContexts,
    scene: Res<Scene>,
    st: Res<Drawn>,
    cam: Query<(&Camera, &GlobalTransform), With<RenderCamera>>,
) -> Result {
    if !scene.debug {
        return Ok(());
    }
    let (Some(cur), Ok((camera, gt))) = (st.cur.as_ref(), cam.single()) else {
        return Ok(());
    };
    let ctx = ctx.ctx_mut()?;
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("render-debug")));
    let views = fighter_views(&scene.data, st.prev.as_ref(), cur, scene.alpha.clamp(0., 1.), scene.pose_override);
    let font = egui::FontId::monospace(12.);
    let mut placed: Vec<egui::Rect> = vec![];
    let mut labels: Vec<(egui::Pos2, String)> = views
        .iter()
        .filter(|v| !matches!(v.f.state, State::Out | State::Dead))
        .filter_map(|v| {
            let p = camera.world_to_viewport(gt, Vec3::new(v.x as f32, -(v.y - 120.) as f32, 0.)).ok()?;
            Some((egui::pos2(p.x, p.y), debug::info(&scene.data, v.f)))
        })
        .collect();
    labels.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
    for (mut pos, text) in labels {
        let galley = painter.layout_no_wrap(text.clone(), font.clone(), egui::Color32::WHITE);
        let size = galley.size();
        // Stack labels that would overlap instead of drawing them on top of each other.
        let rect = |p: egui::Pos2| egui::Rect::from_min_size(p - egui::vec2(size.x / 2., size.y), size);
        while let Some(r) = placed.iter().find(|r| r.intersects(rect(pos))) {
            pos.y = r.top() - 2.;
        }
        placed.push(rect(pos));
        for (dx, dy) in [(-1., 0.), (1., 0.), (0., -1.), (0., 1.)] {
            painter.text(
                pos + egui::vec2(dx, dy),
                egui::Align2::CENTER_BOTTOM,
                &text,
                font.clone(),
                egui::Color32::from_black_alpha(200),
            );
        }
        painter.text(pos, egui::Align2::CENTER_BOTTOM, text, font.clone(), egui::Color32::from_rgb(0xe8, 0xff, 0xe8));
    }
    Ok(())
}

fn stage_frame(previous: Option<u64>, current: u64, t: f64) -> f64 {
    previous.filter(|p| p.checked_add(1) == Some(current)).map_or(current as f64, |p| p as f64 + t)
}

#[cfg(test)]
mod stage_frame_tests {
    #[test]
    fn only_consecutive_frames_interpolate() {
        assert_eq!(super::stage_frame(Some(10), 11, 0.25), 10.25);
        for (previous, current) in [(Some(10), 20), (Some(20), 10), (Some(10), 10), (None, 10), (Some(u64::MAX), 0)] {
            assert_eq!(super::stage_frame(previous, current, 0.25), current as f64);
        }
    }
}
