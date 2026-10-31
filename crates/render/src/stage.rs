//! Stage art and the procedural backdrop (sky gradient, moon, ridges, twinkling stars).

use sim::GameData;
use sim::data::stage::{Background, Stage};

use crate::Rand;
use crate::canvas::Canvas;
use crate::draw::color;

/// Static backdrop: sky, moon, ridges. Covers the stage's camera bounds.
pub fn draw_backdrop(cv: &mut Canvas, d: &GameData, s: &Stage) {
    let Some(bg) = &s.art.background else { return };
    let b = &s.camera;
    let (x0, w) = (b.left, b.right - b.left);
    let c = |col| color(d, col, 0, 1.);
    if let (Some(first), Some(last)) = (bg.sky.first(), bg.sky.last()) {
        cv.fill_rect(x0, b.top, w, first.0 - b.top, c(first.1));
        for s in bg.sky.windows(2) {
            cv.fill_rect_v(x0, s[0].0, w, s[1].0 - s[0].0, c(s[0].1), c(s[1].1));
        }
        cv.fill_rect(x0, last.0, w, b.bottom - last.0, c(last.1));
    }
    if let Some(m) = &bg.moon {
        for k in (1..=m.rings).rev() {
            cv.fill_circle([m.x, m.y], m.r + f64::from(k) * m.step, color(d, m.glow, 0, m.alpha));
        }
        cv.fill_circle([m.x, m.y], m.r, c(m.color));
    }
    for r in &bg.ridges {
        let y =
            |x: f64| r.base - r.amp * (0.5 + 0.3 * (x * 0.006 + r.seed).sin() + 0.2 * (x * 0.017 + r.seed * 3.).sin());
        let col = c(r.color);
        let mut x = b.left;
        while x < b.right {
            let x1 = (x + 20.).min(b.right);
            cv.fill_convex(&[[x, y(x)], [x1, y(x1)], [x1, b.bottom], [x, b.bottom]], col);
            x = x1;
        }
    }
}

pub fn draw_stars(cv: &mut Canvas, d: &GameData, bg: &Background, frame: u64) {
    let Some(s) = &bg.stars else { return };
    let mut rng = Rand(0x5eed_57a5);
    for _ in 0..s.n {
        let (u, v) = (rng.unit(), rng.unit());
        let size = rng.range(s.size[0], s.size[1]);
        let ph = rng.range(0., 6.);
        let a = 0.5 + 0.5 * ((frame as f64 * s.twinkle).to_radians() + ph).sin();
        cv.fill_rect(s.x + u * s.w, s.y + v * s.h, size, size, color(d, s.color, 0, a));
    }
}

pub fn draw_stage(cv: &mut Canvas, d: &GameData, s: &Stage, frame: f64) {
    let a = &s.art;
    let c = |col, al| color(d, col, 0, al);
    for g in &s.blocks {
        cv.fill_rect_v(g.left, g.top, g.right - g.left, g.bottom - g.top, c(a.ground_top, 1.), c(a.ground_bottom, 1.));
        cv.fill_convex(
            &[
                [g.left, g.bottom],
                [g.right, g.bottom],
                [g.right - g.under_inset, g.bottom + g.under_depth],
                [g.left + g.under_inset, g.bottom + g.under_depth],
            ],
            c(a.underside, 1.),
        );
        cv.fill_rect(g.left, g.top, g.right - g.left, a.edge_width, c(a.edge, 1.));
        if let Some(st) = &a.stripes {
            let mut x = g.left + st.x;
            while x < g.right {
                cv.fill_rect(x, g.top + st.y, st.w.min(g.right - x), st.h, c(st.color, st.alpha));
                x += st.every.max(1.);
            }
        }
    }
    let [off, h, alpha] = a.platform_shadow_band;
    for p in &s.platforms {
        let (x1, x2, y) = p.position(frame);
        cv.fill_rect(x1, y + off, x2 - x1, h, c(a.platform_shadow, alpha));
        let pts = cv.round_rect_pts(x1, y, x2 - x1, a.platform_thickness, a.platform_radius);
        cv.fill_convex(&pts, c(a.platform, 1.));
    }
}
