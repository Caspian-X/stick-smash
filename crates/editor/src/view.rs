//! Viewport geometry: pan/zoom camera, fighter-local ↔ stage transforms, hit-testing handles, and the overlay
//! (hurtboxes, hitboxes, launch arrows, joint handles) drawn through `render::Scene::overlay`.

use crate::overlay::dashed_circle;
use crate::style::{OVERLAY_GOLD as GOLD, OVERLAY_INK};

use render::canvas::{Canvas, P, Rgba};
use render::debug::{hit_type_color, launch_arrow, launch_tip};
use render::pose::Skel;
use sim::data::visual::Pose;
use sim::data::{HitType, Hitbox, Pt};
use sim::world::Fighter;

/// Pan/zoom: the stage point at the viewport center and screen points per stage pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam {
    pub center: P,
    pub zoom: f64,
}

/// The viewport rectangle in screen points and the window size, for mapping.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Port {
    /// `[left, top, right, bottom]` screen points.
    pub rect: [f64; 4],
    pub window: [f64; 2],
}

impl Port {
    pub fn mid(&self) -> P {
        [(self.rect[0] + self.rect[2]) / 2., (self.rect[1] + self.rect[3]) / 2.]
    }
}

impl Cam {
    pub fn to_stage(&self, port: &Port, s: P) -> P {
        let m = port.mid();
        [self.center[0] + (s[0] - m[0]) / self.zoom, self.center[1] + (s[1] - m[1]) / self.zoom]
    }
    pub fn to_screen(&self, port: &Port, p: P) -> P {
        let m = port.mid();
        [m[0] + (p[0] - self.center[0]) * self.zoom, m[1] + (p[1] - self.center[1]) * self.zoom]
    }
    /// The whole-window render camera that puts this view in the viewport rectangle.
    pub fn window_view(&self, port: &Port) -> render::fx::Cam {
        let m = port.mid();
        let [w, h] = port.window;
        render::fx::Cam {
            x: self.center[0] - (m[0] - w / 2.) / self.zoom,
            y: self.center[1] - (m[1] - h / 2.) / self.zoom,
            h: h / self.zoom,
        }
    }
    /// Zoom by `k` keeping the stage point under screen point `at` fixed, within `zoom`'s limits.
    pub fn zoom_at(&mut self, port: &Port, at: P, k: f64, zoom: Zoom) {
        let before = self.to_stage(port, at);
        self.zoom = zoom.clamp(self.zoom * k);
        let after = self.to_stage(port, at);
        self.center = [self.center[0] + before[0] - after[0], self.center[1] + before[1] - after[1]];
    }
}

/// Zoom limits (screen points per stage pixel) for a pane; the two modes frame and zoom differently.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zoom {
    pub min: f64,
    pub max: f64,
}

impl Zoom {
    /// Fighter pane: reframing keeps the figure readable, the wheel may go a little wider.
    pub const FIGHTER_FRAME: Zoom = Zoom { min: 0.5, max: 8. };
    pub const FIGHTER_WHEEL: Zoom = Zoom { min: 0.4, max: 12. };
    /// Stage pane: large stages down to small ones.
    pub const STAGE: Zoom = Zoom { min: 0.1, max: 12. };

    pub fn clamp(self, zoom: f64) -> f64 {
        zoom.clamp(self.min, self.max)
    }
}

/// `v` rounded to the nearest multiple of `step` (the reciprocal form keeps `0.3`, not `0.30000000000000004`).
pub fn round_to(v: f64, step: f64) -> f64 {
    (v * (1. / step)).round() / (1. / step)
}

/// Hitbox-local (x forward, y up from the feet) to stage pixels.
pub fn local_to_stage(f: &Fighter, l: Pt) -> P {
    [f.x + l[0] * f.face, f.y - l[1]]
}
pub fn stage_to_local(f: &Fighter, s: P) -> Pt {
    [(s[0] - f.x) * f.face, f.y - s[1]]
}

fn rot(p: P, deg: f64) -> P {
    let (s, c) = deg.to_radians().sin_cos();
    [c * p[0] - s * p[1], s * p[0] + c * p[1]]
}

/// Skeleton joint (pose space) to stage pixels, matching `render::draw::draw_fighter`.
pub fn joint_to_stage(f: &Fighter, sk: &Skel, p: P) -> P {
    let r = rot([p[0], p[1] - sk.pivot], -sk.rot);
    [f.x + f.face * r[0], f.y - sk.offset_y - (r[1] + sk.pivot)]
}
pub fn stage_to_joint(f: &Fighter, sk: &Skel, s: P) -> P {
    let r = [(s[0] - f.x) * f.face, f.y - sk.offset_y - s[1] - sk.pivot];
    let p = rot(r, sk.rot);
    [p[0], p[1] + sk.pivot]
}

pub const JOINTS: [&str; 11] =
    ["head", "shoulder", "hip", "elbowF", "handF", "elbowB", "handB", "kneeF", "footF", "kneeB", "footB"];

pub fn skel_joint(s: &Skel, i: usize) -> P {
    [s.head, s.shoulder, s.hip, s.elbow_f, s.hand_f, s.elbow_b, s.hand_b, s.knee_f, s.foot_f, s.knee_b, s.foot_b][i]
}
pub fn pose_joint(p: &mut Pose, i: usize) -> &mut Option<Pt> {
    match i {
        0 => &mut p.head,
        1 => &mut p.shoulder,
        2 => &mut p.hip,
        3 => &mut p.elbow_f,
        4 => &mut p.hand_f,
        5 => &mut p.elbow_b,
        6 => &mut p.hand_b,
        7 => &mut p.knee_f,
        8 => &mut p.foot_f,
        9 => &mut p.knee_b,
        _ => &mut p.foot_b,
    }
}

/// Hitbox geometry shown on phase frame `t`, and which `frames` entry it comes from (`None`: the base x/y/r).
pub fn geom_source(h: &Hitbox, t: u32) -> ((f64, f64, f64), Option<usize>) {
    let tf = f64::from(t);
    let mut best: Option<usize> = None;
    for (i, e) in h.frames.iter().enumerate() {
        if e[0] <= tf && best.is_none_or(|b| e[0] >= h.frames[b][0]) {
            best = Some(i);
        }
    }
    (h.geom_at(t), best)
}

/// Launch direction for a stage-space drag from the hitbox center: degrees, 0 forward, 90 up.
pub fn drag_angle(f: &Fighter, h: &Hitbox, c: P, m: P) -> f64 {
    let face = if h.dir == sim::data::LaunchDir::Back { -f.face } else { f.face };
    let a = (-(m[1] - c[1])).atan2((m[0] - c[0]) * face).to_degrees();
    if h.kind == HitType::Spike { a.abs() } else { a }
}

/// Something under the mouse in the viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Angle(usize),
    Rim(usize),
    Body(usize),
    Joint(usize),
}

/// One drawn hitbox: index in the phase, center (stage), radius, active.
#[derive(Clone, Copy, Debug)]
pub struct Shown {
    pub i: usize,
    pub c: P,
    pub r: f64,
    pub active: bool,
}

/// The current phase's hitboxes as drawn on phase frame `t`.
pub fn shown(f: &Fighter, boxes: &[Hitbox], t: u32, cd: f64) -> Vec<Shown> {
    boxes
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let (x, y, r) = h.geom_at(t);
            Shown { i, c: local_to_stage(f, [x, y]), r, active: h.active(t) && (!h.counter_damage || cd > 0.) }
        })
        .collect()
}

/// Topmost handle at stage point `m` (`px`: stage pixels per screen point). Handles of the selected box win, then
/// joints, then the smallest box containing the point (active boxes before inactive ones).
pub fn hit_test(
    f: &Fighter,
    boxes: &[Hitbox],
    shown: &[Shown],
    sel: Option<usize>,
    joints: &[P],
    m: P,
    px: f64,
) -> Option<Hit> {
    let dist = |a: P, b: P| (a[0] - b[0]).hypot(a[1] - b[1]);
    if let Some(s) = sel.and_then(|i| shown.iter().find(|b| b.i == i)) {
        if !matches!(boxes[s.i].kind, HitType::Grab | HitType::Autolink | HitType::Sakurai)
            && boxes[s.i].dir != sim::data::LaunchDir::Away
            && dist(launch_tip(s.c, &boxes[s.i], f.face), m) < 8. * px
        {
            return Some(Hit::Angle(s.i));
        }
        if (dist(s.c, m) - s.r).abs() < 5. * px {
            return Some(Hit::Rim(s.i));
        }
    }
    if let Some(j) = (0..joints.len())
        .filter(|&j| dist(joints[j], m) < 7. * px)
        .min_by(|&a, &b| dist(joints[a], m).total_cmp(&dist(joints[b], m)))
    {
        return Some(Hit::Joint(j));
    }
    shown
        .iter()
        .filter(|b| dist(b.c, m) <= b.r.max(4. * px))
        .min_by(|a, b| (!a.active, a.r).partial_cmp(&(!b.active, b.r)).unwrap_or(std::cmp::Ordering::Equal))
        .map(|b| Hit::Body(b.i))
}

pub struct Overlay<'a> {
    pub f: &'a Fighter,
    pub hurt: sim::data::Rect,
    pub boxes: &'a [Hitbox],
    pub shown: &'a [Shown],
    pub sel: Option<usize>,
    pub hover: Option<Hit>,
    pub joints: &'a [P],
    pub show_hurt: bool,
    pub show_inactive: bool,
    pub trail: &'a [P],
    pub counter: bool,
}

/// Draws the overlay; `px` = stage pixels per screen point, so strokes keep a constant screen width.
pub fn draw(cv: &mut Canvas, o: &Overlay, px: f64) {
    let f = o.f;
    // Feet origin.
    let c = [f.x, f.y];
    cv.line([c[0] - 6. * px, c[1]], [c[0] + 6. * px, c[1]], 1.5 * px, Rgba::hex(0xffffff, 0.7));
    cv.line([c[0], c[1] - 6. * px], [c[0], c[1] + 6. * px], 1.5 * px, Rgba::hex(0xffffff, 0.7));
    if o.show_hurt {
        let h = o.hurt;
        let pts = [
            local_to_stage(f, [h.x1, h.y1]),
            local_to_stage(f, [h.x2, h.y1]),
            local_to_stage(f, [h.x2, h.y2]),
            local_to_stage(f, [h.x1, h.y2]),
        ];
        let col = if o.counter { 0x66ddff } else { 0xffee55 };
        cv.fill_convex(&pts, Rgba::hex(col, 0.08));
        cv.stroke_closed(&pts, 1.5 * px, Rgba::hex(col, 0.75));
    }
    for (k, p) in o.trail.iter().enumerate() {
        if k % 2 == 0 {
            cv.fill_circle(*p, 2. * px, Rgba::hex(0xffffff, 0.35));
        }
    }
    let mut order: Vec<&Shown> = o.shown.iter().collect();
    order.sort_by_key(|b| (b.active, Some(b.i) == o.sel));
    for b in order {
        let h = &o.boxes[b.i];
        let col = hit_type_color(h.kind);
        let selected = Some(b.i) == o.sel;
        let hovered = matches!(o.hover, Some(Hit::Body(i)) if i == b.i);
        if b.active {
            cv.fill_circle(b.c, b.r, Rgba::hex(col, 0.32));
            cv.stroke_circle(b.c, b.r, 2. * px, Rgba::hex(col, 1.));
            launch_arrow(cv, b.c, h, f.face, px);
        } else if o.show_inactive || selected {
            dashed_circle(cv, b.c, b.r, 1.5 * px, Rgba::hex(col, 0.55), px);
            if selected {
                launch_arrow(cv, b.c, h, f.face, px * 0.8);
            }
        }
        if hovered && !selected {
            cv.stroke_circle(b.c, b.r + 2. * px, 1.5 * px, Rgba::hex(0xffffff, 0.6));
        }
        if selected {
            let rim = matches!(o.hover, Some(Hit::Rim(_)));
            cv.stroke_circle(b.c, b.r + 3. * px, if rim { 3. } else { 2. } * px, Rgba::hex(GOLD, 1.));
            cv.fill_circle(b.c, 3. * px, Rgba::hex(GOLD, 1.));
            if !matches!(h.kind, HitType::Grab | HitType::Autolink | HitType::Sakurai)
                && h.dir != sim::data::LaunchDir::Away
            {
                let tip = launch_tip(b.c, h, f.face);
                let hot = matches!(o.hover, Some(Hit::Angle(_)));
                cv.fill_circle(tip, if hot { 7. } else { 5.5 } * px, Rgba::hex(GOLD, 1.));
                cv.fill_circle(tip, 2.5 * px, Rgba::hex(OVERLAY_INK, 1.));
            }
        }
    }
    for (j, p) in o.joints.iter().enumerate() {
        let hot = o.hover == Some(Hit::Joint(j));
        crate::overlay::knob(cv, *p, px, hot, 0xe8ecff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zooming_a_large_framed_stage_does_not_jump_to_a_higher_minimum() {
        let port = Port { rect: [0., 0., 800., 600.], window: [800., 600.] };
        let mut cam = Cam { center: [640., 400.], zoom: 0.15 };
        let at = [300., 200.];
        let before = cam.to_stage(&port, at);
        cam.zoom_at(&port, at, 1.01, Zoom::STAGE);
        assert!((cam.zoom - 0.1515).abs() < 1e-12);
        let after = cam.to_stage(&port, at);
        assert!((before[0] - after[0]).abs() < 1e-9 && (before[1] - after[1]).abs() < 1e-9);
    }

    #[test]
    fn fine_rounding_keeps_clean_decimals() {
        assert_eq!(round_to(0.30000000000000004, 0.1), 0.3);
        assert_eq!(round_to(0.04, 0.1), 0.);
        assert_eq!(round_to(-1.26, 0.1), -1.3);
        assert!((round_to(12.6, 15.) - 15.).abs() < 1e-9);
    }

    #[test]
    fn each_pane_clamps_its_own_zoom() {
        let port = Port { rect: [0., 0., 800., 600.], window: [800., 600.] };
        let mut cam = Cam { center: [640., 400.], zoom: 1. };
        for _ in 0..100 {
            cam.zoom_at(&port, [300., 200.], 0.9, Zoom::FIGHTER_WHEEL);
        }
        assert!((cam.zoom - 0.4).abs() < 1e-12);
        cam.zoom = 1.;
        for _ in 0..100 {
            cam.zoom_at(&port, [300., 200.], 0.9, Zoom::STAGE);
        }
        assert!((cam.zoom - 0.1).abs() < 1e-12);
        assert_eq!((Zoom::FIGHTER_FRAME.clamp(0.1), Zoom::FIGHTER_FRAME.clamp(99.)), (0.5, 8.));
    }

    #[test]
    fn joint_transforms_round_trip() {
        let f = Fighter { x: 100., y: 300., face: -1., ..crate::tests_fighter() };
        let sk = Skel { rot: 35., pivot: 30., offset_y: 7., ..Default::default() };
        for p in [[0., 0.], [12., 40.], [-8., 64.]] {
            let s = joint_to_stage(&f, &sk, p);
            let back = stage_to_joint(&f, &sk, s);
            assert!((back[0] - p[0]).abs() < 1e-9 && (back[1] - p[1]).abs() < 1e-9, "{p:?} → {back:?}");
        }
        let flat = Skel { pivot: 30., ..Default::default() };
        assert_eq!(joint_to_stage(&f, &flat, [10., 20.]), [90., 280.]);
    }

    #[test]
    fn camera_maps_both_ways() {
        let port = Port { rect: [200., 40., 900., 540.], window: [1280., 720.] };
        let mut cam = Cam { center: [640., 400.], zoom: 2. };
        let s = [700., 300.];
        let p = cam.to_stage(&port, s);
        assert_eq!(cam.to_screen(&port, p), s);
        cam.zoom_at(&port, s, 1.5, Zoom::STAGE);
        let q = cam.to_stage(&port, s);
        assert!((q[0] - p[0]).abs() < 1e-9 && (q[1] - p[1]).abs() < 1e-9);
    }
}
