//! Screen-sized editor handles and dashed outlines.

use crate::style::{OVERLAY_GOLD as SEL, OVERLAY_INK as INK};
use render::canvas::{Canvas, P, Rgba};

pub fn dashed(cv: &mut Canvas, a: P, b: P, w: f64, col: Rgba, dash: f64) {
    let len = (a[0] - b[0]).hypot(a[1] - b[1]);
    let n = ((len / dash).ceil() as usize).max(1);
    for k in (0..n).step_by(2) {
        let t0 = k as f64 / n as f64;
        let t1 = ((k + 1) as f64 / n as f64).min(1.);
        cv.line(
            [a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0],
            [a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1],
            w,
            col,
        );
    }
}

pub(crate) fn dashed_rect(cv: &mut Canvas, [l, t, r, b]: [f64; 4], w: f64, col: Rgba, dash: f64) {
    for (a, z) in [([l, t], [r, t]), ([r, t], [r, b]), ([r, b], [l, b]), ([l, b], [l, t])] {
        dashed(cv, a, z, w, col, dash);
    }
}

pub(crate) fn knob(cv: &mut Canvas, p: P, px: f64, hot: bool, col: u32) {
    let r = if hot { 6. } else { 4.5 } * px;
    cv.fill_circle(p, r, Rgba::hex(INK, 0.9));
    cv.stroke_circle(p, r, 2. * px, Rgba::hex(if hot { SEL } else { col }, 1.));
}

pub(crate) fn square(cv: &mut Canvas, p: P, px: f64, hot: bool, col: u32) {
    let r = if hot { 5.5 } else { 4. } * px;
    let pts = [[p[0] - r, p[1] - r], [p[0] + r, p[1] - r], [p[0] + r, p[1] + r], [p[0] - r, p[1] + r]];
    cv.fill_convex(&pts, Rgba::hex(INK, 0.9));
    cv.stroke_closed(&pts, 2. * px, Rgba::hex(if hot { SEL } else { col }, 1.));
}

pub(crate) fn dashed_circle(cv: &mut Canvas, c: P, r: f64, w: f64, col: Rgba, px: f64) {
    let n = ((std::f64::consts::TAU * r / (6. * px)).ceil() as usize).clamp(8, 96) & !1;
    for k in (0..n).step_by(2) {
        let a0 = std::f64::consts::TAU * k as f64 / n as f64;
        let a1 = std::f64::consts::TAU * (k + 1) as f64 / n as f64;
        cv.line([c[0] + r * a0.cos(), c[1] + r * a0.sin()], [c[0] + r * a1.cos(), c[1] + r * a1.sin()], w, col);
    }
}
