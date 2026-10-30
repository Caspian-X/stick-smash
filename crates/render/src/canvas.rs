//! Immediate-mode vector tessellation into one vertex-colored triangle list (painter's order = append order).
//! Coordinates go through a 2D transform stack into stage pixels (y down); vertices come out in Bevy's y-up space.

use std::f64::consts::{PI, TAU};

pub type P = [f64; 2];

/// Straight (non-premultiplied) linear RGBA.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub [f32; 4]);

impl Rgba {
    pub fn hex(rgb: u32, a: f64) -> Self {
        let c = |s: u32| {
            let v = f64::from((rgb >> s) & 0xff) / 255.;
            (if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }) as f32
        };
        Rgba([c(16), c(8), c(0), a.clamp(0., 1.) as f32])
    }
    pub fn alpha(self, a: f64) -> Self {
        let [r, g, b, a0] = self.0;
        Rgba([r, g, b, (f64::from(a0) * a).clamp(0., 1.) as f32])
    }
    pub fn a(self) -> f64 {
        f64::from(self.0[3])
    }
    pub fn lerp(self, o: Rgba, t: f64) -> Self {
        let t = t as f32;
        Rgba(std::array::from_fn(|i| {
            self.0[i] + (o.0[i] - self.0[i]) * t
        }))
    }
}

/// `[a, b, c, d, e, f]`: x' = a x + c y + e, y' = b x + d y + f.
#[derive(Clone, Copy, Debug)]
struct Affine([f64; 6]);

impl Affine {
    const ID: Self = Affine([1., 0., 0., 1., 0., 0.]);
    fn mul(self, o: Affine) -> Affine {
        let [a, b, c, d, e, f] = self.0;
        let [a2, b2, c2, d2, e2, f2] = o.0;
        Affine([
            a * a2 + c * b2,
            b * a2 + d * b2,
            a * c2 + c * d2,
            b * c2 + d * d2,
            a * e2 + c * f2 + e,
            b * e2 + d * f2 + f,
        ])
    }
    fn apply(self, [x, y]: P) -> P {
        let [a, b, c, d, e, f] = self.0;
        [a * x + c * y + e, b * x + d * y + f]
    }
    /// Average linear scale (for tessellation detail).
    fn scale(self) -> f64 {
        let [a, b, c, d, ..] = self.0;
        ((a * d - b * c).abs()).sqrt()
    }
}

#[derive(Default)]
pub struct Canvas {
    pub pos: Vec<[f32; 3]>,
    pub col: Vec<[f32; 4]>,
    pub idx: Vec<u32>,
    xf: Vec<Affine>,
    /// Screen pixels per stage pixel; more detail on curves when zoomed in.
    pub detail: f64,
    /// Alpha is raised to this power on output (see `global.render.alphaGamma`).
    pub alpha_gamma: f64,
}

impl Canvas {
    pub fn new(detail: f64) -> Self {
        Canvas {
            detail,
            alpha_gamma: 1.,
            ..Default::default()
        }
    }
    pub fn clear(&mut self, detail: f64, alpha_gamma: f64) {
        self.alpha_gamma = alpha_gamma;
        self.pos.clear();
        self.col.clear();
        self.idx.clear();
        self.xf.clear();
        self.detail = detail;
    }
    fn cur(&self) -> Affine {
        self.xf.last().copied().unwrap_or(Affine::ID)
    }
    pub fn save(&mut self) {
        self.xf.push(self.cur());
    }
    pub fn restore(&mut self) {
        self.xf.pop();
    }
    fn push_xf(&mut self, m: Affine) {
        let c = self.cur().mul(m);
        match self.xf.last_mut() {
            Some(t) => *t = c,
            None => self.xf.push(c),
        }
    }
    pub fn translate(&mut self, x: f64, y: f64) {
        self.push_xf(Affine([1., 0., 0., 1., x, y]));
    }
    pub fn scale(&mut self, sx: f64, sy: f64) {
        self.push_xf(Affine([sx, 0., 0., sy, 0., 0.]));
    }
    /// Counter-clockwise in the current space's math orientation (radians).
    pub fn rotate(&mut self, a: f64) {
        let (s, c) = a.sin_cos();
        self.push_xf(Affine([c, s, -s, c, 0., 0.]));
    }
    pub fn to_world(&self, p: P) -> P {
        self.cur().apply(p)
    }

    fn segs(&self, r: f64) -> usize {
        let px = r.abs() * self.detail * self.cur().scale();
        ((4.5 * px.max(0.).sqrt()).ceil() as usize).clamp(8, 96)
    }
    fn vert(&mut self, p: P, c: Rgba) -> u32 {
        let [x, y] = self.cur().apply(p);
        self.pos.push([x as f32, -y as f32, 0.]);
        let [r, g, b, a] = c.0;
        self.col.push([
            r,
            g,
            b,
            if a < 1. {
                a.powf(self.alpha_gamma as f32)
            } else {
                a
            },
        ]);
        (self.pos.len() - 1) as u32
    }
    fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.idx.extend([a, b, c]);
    }
    fn visible(c: Rgba) -> bool {
        c.0[3] > 0.002
    }

    /// Convex polygon (triangle fan).
    pub fn fill_convex(&mut self, pts: &[P], c: Rgba) {
        if pts.len() < 3 || !Self::visible(c) {
            return;
        }
        let i0 = self.vert(pts[0], c);
        let mut prev = self.vert(pts[1], c);
        for p in &pts[2..] {
            let i = self.vert(*p, c);
            self.tri(i0, prev, i);
            prev = i;
        }
    }
    /// Any simple polygon (ear clipping).
    pub fn fill_poly(&mut self, pts: &[P], c: Rgba) {
        if pts.len() < 3 || !Self::visible(c) {
            return;
        }
        let area: f64 = (0..pts.len())
            .map(|i| cross(pts[i], pts[(i + 1) % pts.len()]))
            .sum();
        let mut ring: Vec<usize> = (0..pts.len()).collect();
        if area < 0. {
            ring.reverse();
        }
        let base: Vec<u32> = pts.iter().map(|p| self.vert(*p, c)).collect();
        let mut guard = 0;
        while ring.len() > 3 && guard < pts.len() * pts.len() {
            guard += 1;
            let n = ring.len();
            let ear = (0..n).find(|&k| {
                let (a, b, cc) = (
                    pts[ring[(k + n - 1) % n]],
                    pts[ring[k]],
                    pts[ring[(k + 1) % n]],
                );
                cross(sub(b, a), sub(cc, b)) > 1e-9
                    && !ring.iter().any(|&j| {
                        let p = pts[j];
                        j != ring[(k + n - 1) % n]
                            && j != ring[k]
                            && j != ring[(k + 1) % n]
                            && in_tri(p, a, b, cc)
                    })
            });
            let k = ear.unwrap_or(0);
            self.tri(
                base[ring[(k + n - 1) % n]],
                base[ring[k]],
                base[ring[(k + 1) % n]],
            );
            ring.remove(k);
        }
        if ring.len() == 3 {
            self.tri(base[ring[0]], base[ring[1]], base[ring[2]]);
        }
    }
    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, c: Rgba) {
        self.fill_convex(&[[x, y], [x + w, y], [x + w, y + h], [x, y + h]], c);
    }
    /// Vertical gradient from `top` (at y) to `bottom` (at y + h).
    pub fn fill_rect_v(&mut self, x: f64, y: f64, w: f64, h: f64, top: Rgba, bottom: Rgba) {
        let a = self.vert([x, y], top);
        let b = self.vert([x + w, y], top);
        let c = self.vert([x + w, y + h], bottom);
        let d = self.vert([x, y + h], bottom);
        self.tri(a, b, c);
        self.tri(a, c, d);
    }
    /// Quad with a color per corner.
    pub fn fill_quad(&mut self, pts: [P; 4], cols: [Rgba; 4]) {
        let i: Vec<u32> = pts
            .iter()
            .zip(cols)
            .map(|(p, c)| self.vert(*p, c))
            .collect();
        self.tri(i[0], i[1], i[2]);
        self.tri(i[0], i[2], i[3]);
    }
    pub fn round_rect_pts(&self, x: f64, y: f64, w: f64, h: f64, r: f64) -> Vec<P> {
        let r = r.min(w.abs() / 2.).min(h.abs() / 2.).max(0.);
        if r <= 0. {
            return vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
        }
        let n = (self.segs(r) / 4).max(2);
        let mut out = Vec::with_capacity(4 * (n + 1));
        for (cx, cy, a0) in [
            (x + w - r, y + h - r, 0.),
            (x + r, y + h - r, PI / 2.),
            (x + r, y + r, PI),
            (x + w - r, y + r, 1.5 * PI),
        ] {
            for k in 0..=n {
                let a = a0 + PI / 2. * k as f64 / n as f64;
                out.push([cx + r * a.cos(), cy + r * a.sin()]);
            }
        }
        out
    }
    pub fn ellipse_pts(&self, c: P, rx: f64, ry: f64) -> Vec<P> {
        let n = self.segs(rx.max(ry));
        (0..n)
            .map(|k| k as f64 * TAU / n as f64)
            .map(|a| [c[0] + rx * a.cos(), c[1] + ry * a.sin()])
            .collect()
    }
    pub fn fill_circle(&mut self, c: P, r: f64, col: Rgba) {
        if r > 0. {
            let pts = self.ellipse_pts(c, r, r);
            self.fill_convex(&pts, col);
        }
    }
    pub fn fill_ellipse(&mut self, c: P, rx: f64, ry: f64, col: Rgba) {
        let pts = self.ellipse_pts(c, rx, ry);
        self.fill_convex(&pts, col);
    }
    pub fn stroke_circle(&mut self, c: P, r: f64, w: f64, col: Rgba) {
        let pts = self.ellipse_pts(c, r, r);
        self.stroke_closed(&pts, w, col);
    }
    /// Ring sector between radii `ri` and `ro`, from angle `a0` to `a1` (radians, any direction).
    pub fn fill_sector(&mut self, c: P, ri: f64, ro: f64, a0: f64, a1: f64, col: Rgba) {
        if !Self::visible(col) {
            return;
        }
        let n = ((self.segs(ro) as f64 * (a1 - a0).abs() / TAU).ceil() as usize).max(2);
        let mut prev: Option<(u32, u32)> = None;
        for k in 0..=n {
            let a = a0 + (a1 - a0) * k as f64 / n as f64;
            let (s, co) = a.sin_cos();
            let i = self.vert([c[0] + ri * co, c[1] + ri * s], col);
            let o = self.vert([c[0] + ro * co, c[1] + ro * s], col);
            if let Some((pi, po)) = prev {
                self.tri(pi, po, o);
                self.tri(pi, o, i);
            }
            prev = Some((i, o));
        }
    }
    pub fn line(&mut self, a: P, b: P, w: f64, c: Rgba) {
        self.stroke_open(&[a, b], w, c, false);
    }
    /// Open polyline. With `round`, joints and ends get round caps (only when opaque, like the TS renderer, so
    /// translucent strokes don't double up); translucent strokes get mitered joints instead.
    pub fn stroke_open(&mut self, pts: &[P], w: f64, c: Rgba, round: bool) {
        if pts.len() < 2 || !Self::visible(c) || w <= 0. {
            return;
        }
        let opaque = c.a() >= 0.99;
        if opaque || pts.len() == 2 {
            for s in pts.windows(2) {
                self.seg_quad(s[0], s[1], w, c);
            }
            if round && opaque {
                for p in pts {
                    self.fill_circle(*p, w / 2., c);
                }
            }
        } else {
            self.strip(pts, w, c, false);
        }
    }
    pub fn stroke_closed(&mut self, pts: &[P], w: f64, c: Rgba) {
        if pts.len() >= 3 && Self::visible(c) && w > 0. {
            self.strip(pts, w, c, true);
        }
    }
    fn seg_quad(&mut self, a: P, b: P, w: f64, c: Rgba) {
        let d = sub(b, a);
        let l = len(d);
        if l < 1e-9 {
            return;
        }
        let n = [-d[1] / l * w / 2., d[0] / l * w / 2.];
        let i = [
            self.vert(add(a, n), c),
            self.vert(add(b, n), c),
            self.vert(sub(b, n), c),
            self.vert(sub(a, n), c),
        ];
        self.tri(i[0], i[1], i[2]);
        self.tri(i[0], i[2], i[3]);
    }
    fn strip(&mut self, pts: &[P], w: f64, c: Rgba, closed: bool) {
        let n = pts.len();
        let normal = |a: P, b: P| {
            let d = sub(b, a);
            let l = len(d).max(1e-9);
            [-d[1] / l, d[0] / l]
        };
        let mut prev: Option<(u32, u32)> = None;
        let mut first: Option<(u32, u32)> = None;
        for k in 0..n {
            let p = pts[k];
            let has_prev = closed || k > 0;
            let has_next = closed || k + 1 < n;
            let n0 = has_prev.then(|| normal(pts[(k + n - 1) % n], p));
            let n1 = has_next.then(|| normal(p, pts[(k + 1) % n]));
            let m = match (n0, n1) {
                (Some(a), Some(b)) => {
                    let s = add(a, b);
                    let l = len(s);
                    if l < 1e-6 {
                        a
                    } else {
                        let m = [s[0] / l, s[1] / l];
                        let d = (m[0] * a[0] + m[1] * a[1]).max(0.5);
                        [m[0] / d, m[1] / d]
                    }
                }
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => return,
            };
            let off = [m[0] * w / 2., m[1] * w / 2.];
            let l = self.vert(add(p, off), c);
            let r = self.vert(sub(p, off), c);
            if let Some((pl, pr)) = prev {
                self.tri(pl, l, r);
                self.tri(pl, r, pr);
            } else {
                first = Some((l, r));
            }
            prev = Some((l, r));
        }
        if closed && let (Some((pl, pr)), Some((l, r))) = (prev, first) {
            self.tri(pl, l, r);
            self.tri(pl, r, pr);
        }
    }
    /// Four-pointed sparkle (two thin diamonds), like the TS `star`.
    pub fn star(&mut self, c: P, r: f64, col: Rgba) {
        let [x, y] = c;
        let k = r * 0.3;
        self.fill_convex(&[[x - r, y], [x, y - k], [x + r, y], [x, y + k]], col);
        self.fill_convex(&[[x, y - r], [x + k, y], [x, y + r], [x - k, y]], col);
    }
}

pub fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
pub fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
pub fn len(a: P) -> f64 {
    a[0].hypot(a[1])
}
fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn in_tri(p: P, a: P, b: P, c: P) -> bool {
    let d1 = cross(sub(b, a), sub(p, a));
    let d2 = cross(sub(c, b), sub(p, b));
    let d3 = cross(sub(a, c), sub(p, c));
    !((d1 < 0. || d2 < 0. || d3 < 0.) && (d1 > 0. || d2 > 0. || d3 > 0.))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concave_poly_fills_all_area() {
        let mut c = Canvas::new(1.);
        let crown = [
            [-9., 7.],
            [-10., 19.],
            [-4., 12.],
            [0., 21.],
            [4., 12.],
            [10., 19.],
            [9., 7.],
        ];
        c.fill_poly(&crown, Rgba::hex(0xffffff, 1.));
        assert_eq!(c.idx.len(), 3 * 5);
        let area: f64 = c
            .idx
            .chunks(3)
            .map(|t| {
                let p = |i: u32| {
                    [
                        f64::from(c.pos[i as usize][0]),
                        f64::from(c.pos[i as usize][1]),
                    ]
                };
                cross(sub(p(t[1]), p(t[0])), sub(p(t[2]), p(t[0]))).abs() / 2.
            })
            .sum();
        let poly: f64 = (0..7)
            .map(|i| cross(crown[i], crown[(i + 1) % 7]))
            .sum::<f64>()
            .abs()
            / 2.;
        assert!((area - poly).abs() < 1e-3, "{area} vs {poly}");
    }

    #[test]
    fn transforms_compose() {
        let mut c = Canvas::new(1.);
        c.translate(10., 20.);
        c.scale(-1., -1.);
        c.rotate(PI / 2.);
        let p = c.to_world([1., 0.]);
        assert!(
            (p[0] - 10.).abs() < 1e-9 && (p[1] - 19.).abs() < 1e-9,
            "{p:?}"
        );
    }
}
