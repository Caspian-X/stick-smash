//! Live stage thumbnails for UI tiles (stage select): backdrop, blocks, platforms (moving ones move) and spawn
//! points, fitted to the stage's camera bounds. One sheet cell per stage plus a large preview on its own sheet. Set
//! [`StageThumbs::cells`] (stage indices) each frame; leave it empty to stop rendering.

use std::sync::Arc;

use bevy::prelude::*;
use sim::GameData;
use sim::data::Color as DataColor;
use sim::data::stage::{Bounds, Stage};

use crate::canvas::{Canvas, Rgba};
use crate::draw::color;
use crate::sheet::{self, Cams, Meshes, Sheet};
use crate::stage::{draw_backdrop, draw_stage, draw_stars};

/// Cells in texture pixels.
pub const THUMBS: Sheet = Sheet { cell: Vec2::new(384., 216.), px: 1., cols: 6, layer: 8 };
pub const PREVIEW: Sheet = Sheet { cell: Vec2::new(1152., 648.), px: 1., cols: 1, layer: 9 };

#[derive(Resource, Default)]
pub struct StageThumbs {
    pub image: Handle<Image>,
    pub preview_image: Handle<Image>,
    pub cells: Vec<usize>,
    pub preview: Option<usize>,
    /// Palette entries for the spawn points, in player order (the rest are drawn faint).
    pub colors: Vec<usize>,
    pub data: Option<Arc<GameData>>,
}

impl StageThumbs {
    /// Cell height / width.
    pub const ASPECT: f32 = THUMBS.cell.y / THUMBS.cell.x;
    pub fn rect(k: usize) -> Rect {
        THUMBS.rect(k)
    }
    pub fn preview_rect() -> Rect {
        PREVIEW.rect(0)
    }
}

pub(crate) fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<ColorMaterial>>,
) {
    let image = THUMBS.spawn(&mut commands, &mut images, &mut meshes, &mut mats);
    let preview_image = PREVIEW.spawn(&mut commands, &mut images, &mut meshes, &mut mats);
    commands.insert_resource(StageThumbs { image, preview_image, ..default() });
}

/// The stage cropped to its play area (padded, at the cell's aspect): the backdrop, stars and moon are trimmed to the
/// crop so nothing spills into the neighboring cells.
fn cropped(s: &Stage, aspect: f64, crop: [f64; 2]) -> Stage {
    let [l, t, r, b] = sim::data::camera::play_area(s);
    let (cx, cy) = ((l + r) / 2., (t + b) / 2.);
    let (mut w, mut h) = ((r - l) * crop[0], (b - t) * crop[1]);
    if w / h < aspect {
        w = h * aspect;
    } else {
        h = w / aspect;
    }
    let mut s = s.clone();
    s.camera = Bounds { left: cx - w / 2., right: cx + w / 2., top: cy - h / 2., bottom: cy + h / 2. };
    let v = &s.camera;
    if let Some(bg) = &mut s.art.background {
        bg.moon.take_if(|m| {
            let r = m.r + f64::from(m.rings) * m.step;
            m.x - r < v.left || m.x + r > v.right || m.y - r < v.top || m.y + r > v.bottom
        });
        if let Some(st) = &mut bg.stars {
            let (x0, y0) = (st.x.max(v.left), st.y.max(v.top));
            let (x1, y1) = ((st.x + st.w).min(v.right - 4.), (st.y + st.h).min(v.bottom - 4.));
            let area = (st.w * st.h).max(1.);
            (st.x, st.y, st.w, st.h) = (x0, y0, (x1 - x0).max(0.), (y1 - y0).max(0.));
            st.n = (f64::from(st.n) * st.w * st.h / area).round() as u32;
        }
    }
    s
}

fn stage_cell(cv: &mut Canvas, d: &GameData, s: &Stage, at: Vec2, cell: Vec2, frame: u64, colors: &[usize]) {
    let b = &s.camera;
    let (w, h) = (b.right - b.left, b.bottom - b.top);
    let k = (f64::from(cell.x) / w).min(f64::from(cell.y) / h);
    let (x, y) = (f64::from(at.x), f64::from(at.y));
    cv.save();
    cv.translate(x + (f64::from(cell.x) - w * k) / 2., y + (f64::from(cell.y) - h * k) / 2.);
    cv.scale(k, k);
    cv.translate(-b.left, -b.top);
    draw_backdrop(cv, d, s);
    if let Some(bg) = &s.art.background {
        draw_stars(cv, d, bg, frame);
    }
    draw_stage(cv, d, s, frame as f64);
    // Spawn points: a dot per player slot, in player colors for the current lineup.
    let r = 3.5 / k * f64::from(cell.x) / f64::from(THUMBS.cell.x);
    for (i, sp) in s.spawns.iter().enumerate().take(s.max_players()) {
        let y = sp.y.unwrap_or_else(|| s.spawn_y(sp.x).unwrap_or(0.)) - r * 1.6;
        match colors.get(i) {
            Some(&c) => {
                cv.fill_circle([sp.x, y], r * 1.35, Rgba::hex(0x0b0d2a, 0.85));
                cv.fill_circle([sp.x, y], r, color(d, DataColor::Player, c, 1.));
            }
            None => cv.fill_circle([sp.x, y], r * 0.6, Rgba::hex(0xffffff, 0.45)),
        }
    }
    cv.restore();
}

/// Cropped stages, rebuilt when content changes.
#[derive(Default)]
pub(crate) struct Cache {
    data: Option<Arc<GameData>>,
    stages: Vec<Stage>,
}

pub(crate) fn draw(
    thumbs: Res<StageThumbs>,
    time: Res<Time>,
    mut cache: Local<Cache>,
    mut cams: Cams,
    meshes: Meshes,
    mut assets: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
) {
    let preview = thumbs.preview.filter(|_| !thumbs.cells.is_empty());
    sheet::prepare(&THUMBS, &thumbs.image, thumbs.cells.len(), &mut images, &mut cams);
    sheet::prepare(&PREVIEW, &thumbs.preview_image, usize::from(preview.is_some()), &mut images, &mut cams);
    let Some(base) = thumbs.data.clone().filter(|_| !thumbs.cells.is_empty()) else { return };
    if !cache.data.as_ref().is_some_and(|c| Arc::ptr_eq(c, &base)) {
        let aspect = f64::from(THUMBS.cell.x / THUMBS.cell.y);
        let crop = base.global.render.select.thumb_crop;
        cache.stages = base.stages.iter().map(|s| cropped(s, aspect, crop)).collect();
        cache.data = Some(base.clone());
    }
    let d = &*base;
    let frame = (time.elapsed_secs_f64() * f64::from(sim::TICK_HZ)) as u64;
    let mut cv = Canvas::new(0.5);
    cv.alpha_gamma = d.global.render.alpha_gamma;
    // Bottom row first: a cropped backdrop's sky and ridges can reach up past its cell, and the row above covers that.
    for (k, &i) in thumbs.cells.iter().enumerate().rev() {
        if let Some(s) = cache.stages.get(i) {
            stage_cell(&mut cv, d, s, THUMBS.origin(k), THUMBS.cell, frame, &thumbs.colors);
        }
    }
    sheet::show(&THUMBS, &cv, &meshes, &mut assets);
    if let Some(s) = preview.and_then(|i| cache.stages.get(i)) {
        cv.clear(1., d.global.render.alpha_gamma);
        stage_cell(&mut cv, d, s, Vec2::ZERO, PREVIEW.cell, frame, &thumbs.colors);
        sheet::show(&PREVIEW, &cv, &meshes, &mut assets);
    }
}
