//! Off-screen images of equal cells for UI tiles. Each sheet has its own camera and render layer; UI nodes show a
//! cell with `ImageNode::rect`. A sheet grows by whole rows when more cells are needed.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat};

use crate::canvas::Canvas;
use crate::upload;

#[derive(Clone, Copy, Debug)]
pub struct Sheet {
    /// Cell size in drawing units, and texture pixels per unit.
    pub cell: Vec2,
    pub px: f32,
    pub cols: usize,
    pub layer: usize,
}

#[derive(Component)]
pub(crate) struct SheetCam(usize, usize);
#[derive(Component)]
pub(crate) struct SheetMesh(usize);

impl Sheet {
    /// Region of the image showing cell `k`.
    pub fn rect(&self, k: usize) -> Rect {
        let min = self.origin(k) * self.px;
        Rect::from_corners(min, min + self.cell * self.px)
    }
    /// Top-left corner of cell `k` in drawing units (y down).
    pub fn origin(&self, k: usize) -> Vec2 {
        Vec2::new((k % self.cols) as f32, (k / self.cols) as f32) * self.cell
    }
    fn size(&self, rows: usize) -> Vec2 {
        self.cell * Vec2::new(self.cols as f32, rows.max(1) as f32)
    }
    pub(crate) fn spawn(
        &self,
        commands: &mut Commands,
        images: &mut Assets<Image>,
        meshes: &mut Assets<Mesh>,
        mats: &mut Assets<ColorMaterial>,
    ) -> Handle<Image> {
        let px = self.size(1) * self.px;
        let image =
            images.add(Image::new_target_texture(px.x as u32, px.y as u32, TextureFormat::Rgba8UnormSrgb, None));
        let world = self.size(1);
        commands.spawn((
            Camera2d,
            Camera { order: -1, is_active: false, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
            RenderTarget::Image(image.clone().into()),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed { width: world.x, height: world.y },
                ..OrthographicProjection::default_2d()
            }),
            Transform::from_xyz(world.x / 2., -world.y / 2., 0.),
            RenderLayers::layer(self.layer),
            SheetCam(self.layer, 1),
        ));
        commands.spawn((
            Mesh2d(meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()))),
            MeshMaterial2d(mats.add(ColorMaterial::default())),
            NoFrustumCulling,
            RenderLayers::layer(self.layer),
            SheetMesh(self.layer),
        ));
        image
    }
}

pub(crate) type Cams<'w, 's> =
    Query<'w, 's, (&'static mut SheetCam, &'static mut Camera, &'static mut Projection, &'static mut Transform)>;
pub(crate) type Meshes<'w, 's> = Query<'w, 's, (&'static SheetMesh, &'static Mesh2d)>;

/// Turn the sheet's camera on or off; when on, grow the image to hold `cells`.
pub(crate) fn prepare(sheet: &Sheet, image: &Handle<Image>, cells: usize, images: &mut Assets<Image>, cams: &mut Cams) {
    let rows = cells.div_ceil(sheet.cols).max(1);
    for (mut sc, mut cam, mut proj, mut tf) in cams.iter_mut().filter(|c| c.0.0 == sheet.layer) {
        if cam.is_active != (cells > 0) {
            cam.is_active = cells > 0;
        }
        if cells == 0 || sc.1 >= rows {
            continue;
        }
        sc.1 = rows;
        let world = sheet.size(rows);
        let px = world * sheet.px;
        if let Some(mut img) = images.get_mut(image) {
            img.resize(Extent3d { width: px.x as u32, height: px.y as u32, depth_or_array_layers: 1 });
        }
        if let Projection::Orthographic(o) = &mut *proj {
            o.scaling_mode = ScalingMode::Fixed { width: world.x, height: world.y };
        }
        tf.translation = Vec3::new(world.x / 2., -world.y / 2., 0.);
    }
}

pub(crate) fn show(sheet: &Sheet, cv: &Canvas, meshes: &Meshes, assets: &mut Assets<Mesh>) {
    for (m, mesh) in meshes {
        if m.0 == sheet.layer
            && let Some(mut mesh) = assets.get_mut(&mesh.0)
        {
            upload(&mut mesh, cv);
        }
    }
}
