//! Live fighter figures for UI tiles (the character select roster). Every figure is drawn off-screen into one
//! image, a cell each; UI nodes show a cell with `ImageNode::rect`. Set [`Portraits::cells`] each frame; leave it
//! empty to stop rendering.

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
use crate::{Rand, Scene, pose, upload};

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

/// One idle fighter per roster entry and the palette with the neutral color appended, rebuilt when content changes.
#[derive(Default)]
pub(crate) struct Cache {
    data: Option<Arc<GameData>>,
    drawn: Option<Arc<GameData>>,
    fighters: Vec<Fighter>,
}

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
    commands.insert_resource(Portraits { image, cells: vec![] });
}

pub(crate) fn draw(
    portraits: Res<Portraits>,
    scene: Res<Scene>,
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
    let base = &scene.data;
    if !cache.data.as_ref().is_some_and(|d| Arc::ptr_eq(d, base)) {
        let mut d = (**base).clone();
        d.global.palette.push(d.global.render.select.roster_color.clone());
        cache.drawn = Some(Arc::new(d));
        cache.data = Some(base.clone());
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
                World::new(cfg, base).ok().map(|w| w.fighters[0].clone())
            })
            .collect();
    }
    let Some(d) = cache.drawn.clone() else { return };
    let neutral = d.global.palette.len() - 1;
    let frame = (time.elapsed_secs_f64() * f64::from(sim::TICK_HZ)) as u64;
    let mut cv = Canvas::new(f64::from(PX));
    cv.alpha_gamma = d.global.render.alpha_gamma;
    let mut rng = Rand::default();
    for (k, p) in portraits.cells.iter().enumerate().take(Portraits::MAX) {
        let Some(mut f) = cache.fighters.iter().find(|f| f.data == p.fighter).cloned() else { continue };
        f.face = 1.;
        if p.win {
            f.state = State::Win;
        }
        // Left of center: weapons reach forward.
        let x = f64::from(((k % COLS) as f32 + 0.42) * CELL.x);
        let y = f64::from(((k / COLS) as f32 + 1.) * CELL.y) - 25.;
        let skel = pose::fighter_pose(&d, &f, frame);
        draw_fighter(&mut cv, &d, frame, &FighterView { i: p.color.unwrap_or(neutral), f: &f, x, y, skel }, &mut rng);
    }
    if let Ok(m) = mesh.single()
        && let Some(mut m) = meshes.get_mut(&m.0)
    {
        upload(&mut m, &cv);
    }
}
