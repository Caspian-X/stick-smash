//! Live fighter figures for UI tiles (the character select roster). Every figure is drawn off-screen into one
//! image, a cell each; UI nodes show a cell with `ImageNode::rect`. Set [`Portraits::cells`] each frame; leave it
//! empty to stop rendering. Colors index [`Portraits::data`]'s palette (the content as loaded, not a match's
//! player-ordered copy).

use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use sim::world::{Fighter, State};
use sim::{GameData, MatchConfig, PlayerConfig, World};

use crate::canvas::Canvas;
use crate::draw::{FighterView, draw_fighter};
use crate::{Rand, pose, upload};

/// Cell size in stage pixels and texture pixels per stage pixel.
const CELL: Vec2 = Vec2::new(104., 128.);
const PX: f32 = 2.4;
const COLS: usize = 8;
const ROWS: usize = 2;
const LAYER: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Portrait {
    pub fighter: usize,
    /// Palette entry to draw with; `None`: the neutral `render.select.rosterColor`.
    pub color: Option<usize>,
    pub win: bool,
}

#[derive(Resource, Default)]
pub struct Portraits {
    pub image: Handle<Image>,
    pub cells: Vec<Portrait>,
    pub data: Option<Arc<GameData>>,
}

impl Portraits {
    pub const MAX: usize = COLS * ROWS;
    /// Cell height / width.
    pub const ASPECT: f32 = CELL.y / CELL.x;
    /// Region of `image` showing cell `k`.
    pub fn rect(k: usize) -> Rect {
        let min = Vec2::new((k % COLS) as f32, (k / COLS) as f32) * CELL * PX;
        Rect::from_corners(min, min + CELL * PX)
    }
}

#[derive(Component)]
pub(crate) struct PortraitCam;
#[derive(Component)]
pub(crate) struct PortraitMesh;

/// One idle fighter per roster entry with its horizontal draw offset, a scale that fits every fighter's idle
/// extent in a cell, and the palette with the neutral color appended; rebuilt when content changes.
#[derive(Default)]
pub(crate) struct Cache {
    data: Option<Arc<GameData>>,
    drawn: Option<Arc<GameData>>,
    fighters: Vec<(Fighter, f64)>,
    scale: f64,
}

/// Room for a figure: cell size less side margins and the name band.
const ROOM: (f64, f64) = (CELL.x as f64 - 8., CELL.y as f64 - 34.);
const FEET: f64 = 26.;

pub(crate) fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<ColorMaterial>>,
) {
    let size = CELL * PX * Vec2::new(COLS as f32, ROWS as f32);
    let image =
        images.add(Image::new_target_texture(size.x as u32, size.y as u32, TextureFormat::Rgba8UnormSrgb, None));
    let world = CELL * Vec2::new(COLS as f32, ROWS as f32);
    commands.spawn((
        Camera2d,
        Camera { order: -1, is_active: false, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
        RenderTarget::Image(image.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed { width: world.x, height: world.y },
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(world.x / 2., -world.y / 2., 0.),
        RenderLayers::layer(LAYER),
        PortraitCam,
    ));
    commands.spawn((
        Mesh2d(meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()))),
        MeshMaterial2d(mats.add(ColorMaterial::default())),
        NoFrustumCulling,
        RenderLayers::layer(LAYER),
        PortraitMesh,
    ));
    commands.insert_resource(Portraits { image, cells: vec![], data: None });
}

pub(crate) fn draw(
    portraits: Res<Portraits>,
    time: Res<Time>,
    mut cache: Local<Cache>,
    mut cam: Query<&mut Camera, With<PortraitCam>>,
    mesh: Query<&Mesh2d, With<PortraitMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let on = !portraits.cells.is_empty();
    if let Ok(mut c) = cam.single_mut()
        && c.is_active != on
    {
        c.is_active = on;
    }
    if !on {
        return;
    }
    let Some(base) = portraits.data.clone() else { return };
    let frame = (time.elapsed_secs_f64() * f64::from(sim::TICK_HZ)) as u64;
    let mut rng = Rand::default();
    if !cache.data.as_ref().is_some_and(|d| Arc::ptr_eq(d, &base)) {
        let mut d = (*base).clone();
        d.global.palette.push(d.global.render.select.roster_color.clone());
        let d = Arc::new(d);
        let mut scale: f64 = 1.;
        cache.fighters = base
            .fighters
            .iter()
            .filter_map(|f| {
                let cfg = MatchConfig {
                    stage: base.stages.first()?.id.clone(),
                    stocks: 1,
                    training: true,
                    players: vec![PlayerConfig::human(&f.id)],
                    ..default()
                };
                let mut f = World::new(cfg, &base).ok()?.fighters[0].clone();
                f.face = 1.;
                // Measure the idle figure (attachments included) once, drawn at the origin.
                let mut cv = Canvas::new(1.);
                let skel = pose::fighter_pose(&d, &f, 0);
                draw_fighter(&mut cv, &d, 0, &FighterView { i: 0, f: &f, x: 0., y: 0., skel }, &mut rng);
                let (mut x0, mut x1, mut top) = (0f64, 0f64, 0f64);
                for [x, y, _] in &cv.pos {
                    (x0, x1, top) = (x0.min(f64::from(*x)), x1.max(f64::from(*x)), top.max(f64::from(*y)));
                }
                scale = scale.min(ROOM.0 / (x1 - x0).max(1.)).min(ROOM.1 / top.max(1.));
                Some((f, -(x0 + x1) / 2.))
            })
            .collect();
        cache.scale = scale;
        cache.drawn = Some(d);
        cache.data = Some(base.clone());
    }
    let Some(d) = cache.drawn.clone() else { return };
    let neutral = d.global.palette.len() - 1;
    let mut cv = Canvas::new(f64::from(PX));
    cv.alpha_gamma = d.global.render.alpha_gamma;
    for (k, p) in portraits.cells.iter().enumerate().take(Portraits::MAX) {
        let Some((mut f, dx)) = cache.fighters.iter().find(|(f, _)| f.data == p.fighter).cloned() else { continue };
        if p.win {
            f.state = State::Win;
        }
        let x = f64::from(((k % COLS) as f32 + 0.5) * CELL.x);
        let y = f64::from(((k / COLS) as f32 + 1.) * CELL.y) - FEET;
        cv.save();
        cv.translate(x, y);
        cv.scale(cache.scale, cache.scale);
        let skel = pose::fighter_pose(&d, &f, frame);
        let i = p.color.unwrap_or(neutral);
        draw_fighter(&mut cv, &d, frame, &FighterView { i, f: &f, x: dx, y: 0., skel }, &mut rng);
        cv.restore();
    }
    if let Ok(m) = mesh.single()
        && let Some(mut m) = meshes.get_mut(&m.0)
    {
        upload(&mut m, &cv);
    }
}
