//! Stage editor geometry, independent of egui: the selectable parts of a stage, generic handles (a rect's sides and
//! corners, a segment's ends, a point), hit-testing, drags with snapping and symmetry (mirrored around the stage's
//! centre x), extra validation notes, and the overlay drawn through `render::Scene::overlay`.
//!
//! Everything goes through [`Shape`]: a part reads as a rect, a horizontal segment or a point, and drags edit the
//! shape. New kinds of geometry (more solid blocks, platforms on paths) add a [`Part`] variant and its `shape` /
//! `set_shape` arms; hit-testing, dragging, mirroring and drawing come for free.

use std::f64::consts::PI;

use render::canvas::{Canvas, P, Rgba};
use sim::data::ContentError;
use sim::data::stage::{Platform, RespawnPoint, Spawn, Stage};

/// A selectable piece of the stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Ground,
    Platform(usize),
    Spawn(usize),
    Respawn(usize),
    Ledge,
    Blast,
    Camera,
}

impl Part {
    /// The JSON path its fields live under (for problems).
    pub fn path(self) -> String {
        match self {
            Part::Ground => "ground".into(),
            Part::Platform(i) => format!("platforms[{i}]"),
            Part::Spawn(i) => format!("spawns[{i}]"),
            Part::Respawn(i) => format!("respawns[{i}]"),
            Part::Ledge => "ledge".into(),
            Part::Blast => "blast".into(),
            Part::Camera => "camera".into(),
        }
    }
    pub fn label(self) -> String {
        match self {
            Part::Ground => "Ground".into(),
            Part::Platform(i) => format!("Platform {}", i + 1),
            Part::Spawn(i) => format!("Spawn P{}", i + 1),
            Part::Respawn(i) => format!("Respawn P{}", i + 1),
            Part::Ledge => "Ledge zone".into(),
            Part::Blast => "Blast zone".into(),
            Part::Camera => "Camera bounds".into(),
        }
    }
}

/// Geometry handles work on. `Rect` = `[left, top, right, bottom]`; `Seg` = `[x1, x2, y]` (horizontal); `Point` =
/// `[x, y]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Rect([f64; 4]),
    Seg([f64; 3]),
    Point(P),
}

/// What part of a shape is grabbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    /// The whole shape.
    Move,
    /// Rect sides `[left, top, right, bottom]` (one = an edge, two = a corner).
    Sides([bool; 4]),
    /// A segment end (0 = x1, 1 = x2).
    End(usize),
    /// A respawn's start height.
    Start,
    /// A spawn's facing arrow (click flips).
    Face,
    /// The ledge zone's radius.
    Rim,
}

/// Something under the mouse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    pub part: Part,
    pub grip: Grip,
}

/// Feet height of a spawn: its own `y`, or the ground top.
pub fn spawn_y(s: &Stage, sp: &Spawn) -> f64 {
    sp.y.unwrap_or(s.ground.top)
}

pub fn shape(s: &Stage, p: Part) -> Option<Shape> {
    let r = |b: &sim::data::stage::Bounds| Shape::Rect([b.left, b.top, b.right, b.bottom]);
    Some(match p {
        Part::Ground => Shape::Rect([s.ground.left, s.ground.top, s.ground.right, s.ground.bottom]),
        Part::Platform(i) => s.platforms.get(i).map(|q| Shape::Seg([q.x1, q.x2, q.y]))?,
        Part::Spawn(i) => s.spawns.get(i).map(|q| Shape::Point([q.x, spawn_y(s, q)]))?,
        Part::Respawn(i) => s.respawns.get(i).map(|q| Shape::Point([q.x, q.end_y]))?,
        Part::Blast => r(&s.blast),
        Part::Camera => r(&s.camera),
        Part::Ledge => return None,
    })
}

pub fn set_shape(s: &mut Stage, p: Part, sh: Shape) {
    let top = s.ground.top;
    let set_b = |b: &mut sim::data::stage::Bounds, [l, t, r, bt]: [f64; 4]| {
        (b.left, b.top, b.right, b.bottom) = (l, t, r, bt);
    };
    match (p, sh) {
        (Part::Ground, Shape::Rect([l, t, r, b])) => {
            let g = &mut s.ground;
            (g.left, g.top, g.right, g.bottom) = (l, t, r, b);
        }
        (Part::Platform(i), Shape::Seg([x1, x2, y])) => {
            if let Some(q) = s.platforms.get_mut(i) {
                (q.x1, q.x2, q.y) = (x1, x2, y);
            }
        }
        (Part::Spawn(i), Shape::Point([x, y])) => {
            if let Some(q) = s.spawns.get_mut(i) {
                q.x = x;
                // Spawns on the ground top stay ground-relative.
                q.y = if q.y.is_none() && (y - top).abs() < 1e-9 { None } else { Some(y) };
            }
        }
        (Part::Respawn(i), Shape::Point([x, y])) => {
            if let Some(q) = s.respawns.get_mut(i) {
                q.start_y += y - q.end_y;
                (q.x, q.end_y) = (x, y);
            }
        }
        (Part::Blast, Shape::Rect(r)) => set_b(&mut s.blast, r),
        (Part::Camera, Shape::Rect(r)) => set_b(&mut s.camera, r),
        _ => {}
    }
}

/// Snapping from the modifiers and grid: fine (Alt) = 0.1 px, grid = multiples of `grid`, else whole pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    pub grid: Option<f64>,
    pub fine: bool,
}

impl Snap {
    pub fn v(self, v: f64) -> f64 {
        match (self.fine, self.grid) {
            (true, _) => (v * 10.).round() / 10.,
            (false, Some(g)) if g > 0. => (v / g).round() * g,
            _ => v.round(),
        }
    }
}

/// Smallest size a rect or segment can be dragged to.
const MIN_SIZE: f64 = 4.;

/// `orig` with `grip` dragged by `d`; the moved coordinates land on the snap.
pub fn drag_shape(orig: Shape, grip: Grip, d: P, sn: Snap) -> Shape {
    match (orig, grip) {
        (Shape::Rect([l, t, r, b]), Grip::Move) => {
            let (nl, nt) = (sn.v(l + d[0]), sn.v(t + d[1]));
            Shape::Rect([nl, nt, nl + r - l, nt + b - t])
        }
        (Shape::Rect([l, t, r, b]), Grip::Sides([sl, st, sr, sb])) => {
            let mut v = [l, t, r, b];
            if sl {
                v[0] = sn.v(l + d[0]).min(v[2] - MIN_SIZE);
            }
            if sr {
                v[2] = sn.v(r + d[0]).max(v[0] + MIN_SIZE);
            }
            if st {
                v[1] = sn.v(t + d[1]).min(v[3] - MIN_SIZE);
            }
            if sb {
                v[3] = sn.v(b + d[1]).max(v[1] + MIN_SIZE);
            }
            Shape::Rect(v)
        }
        (Shape::Seg([x1, x2, y]), Grip::Move) => {
            let nx = sn.v(x1 + d[0]);
            Shape::Seg([nx, nx + x2 - x1, sn.v(y + d[1])])
        }
        (Shape::Seg([x1, x2, y]), Grip::End(0)) => Shape::Seg([sn.v(x1 + d[0]).min(x2 - MIN_SIZE), x2, y]),
        (Shape::Seg([x1, x2, y]), Grip::End(_)) => Shape::Seg([x1, sn.v(x2 + d[0]).max(x1 + MIN_SIZE), y]),
        (Shape::Point([x, y]), Grip::Move) => Shape::Point([sn.v(x + d[0]), sn.v(y + d[1])]),
        (s, _) => s,
    }
}

/// The stage's mirror axis.
pub fn centre(s: &Stage) -> f64 {
    s.width / 2.
}

pub fn mirror_shape(sh: Shape, c: f64) -> Shape {
    match sh {
        Shape::Rect([l, t, r, b]) => Shape::Rect([2. * c - r, t, 2. * c - l, b]),
        Shape::Seg([x1, x2, y]) => Shape::Seg([2. * c - x2, 2. * c - x1, y]),
        Shape::Point([x, y]) => Shape::Point([2. * c - x, y]),
    }
}

/// Makes a shape symmetric around `c`, keeping the side the grip moved (a move keeps it centred).
fn symmetrize(sh: Shape, grip: Grip, c: f64) -> Shape {
    match (sh, grip) {
        (Shape::Rect([_, t, r, b]), Grip::Sides([false, _, true, _])) => Shape::Rect([2. * c - r, t, r, b]),
        (Shape::Rect([l, t, _, b]), Grip::Sides([true, _, false, _])) => Shape::Rect([l, t, 2. * c - l, b]),
        (Shape::Rect([l, t, r, b]), _) => {
            let h = (r - l) / 2.;
            Shape::Rect([c - h, t, c + h, b])
        }
        (Shape::Seg([_, x2, y]), Grip::End(1)) => Shape::Seg([2. * c - x2, x2, y]),
        (Shape::Seg([x1, _, y]), Grip::End(_)) => Shape::Seg([x1, 2. * c - x1, y]),
        (Shape::Seg([x1, x2, y]), _) => {
            let h = (x2 - x1) / 2.;
            Shape::Seg([c - h, c + h, y])
        }
        (Shape::Point([_, y]), _) => Shape::Point([c, y]),
    }
}

const MIRROR_TOL: f64 = 2.;

fn near(a: Shape, b: Shape) -> bool {
    let v = |s: Shape| match s {
        Shape::Rect(r) => r.to_vec(),
        Shape::Seg(r) => r.to_vec(),
        Shape::Point(p) => p.to_vec(),
    };
    let (a, b) = (v(a), v(b));
    a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= MIRROR_TOL)
}

/// Same-kind parts of the stage.
fn siblings(s: &Stage, p: Part) -> Vec<Part> {
    match p {
        Part::Platform(_) => (0..s.platforms.len()).map(Part::Platform).collect(),
        Part::Spawn(_) => (0..s.spawns.len()).map(Part::Spawn).collect(),
        Part::Respawn(_) => (0..s.respawns.len()).map(Part::Respawn).collect(),
        p => vec![p],
    }
}

/// The part that mirrors `p` around the centre: another part (a mirrored pair) or `p` itself (one centred, or
/// one of a kind like the ground, which symmetry keeps centred). `None`: nothing mirrors it.
pub fn partner(s: &Stage, p: Part) -> Option<Part> {
    let c = centre(s);
    let sh = shape(s, p)?;
    let m = mirror_shape(sh, c);
    let list = siblings(s, p);
    if list.len() == 1 {
        return Some(p);
    }
    let same_kind_extra = |q: Part| match (p, q) {
        (Part::Spawn(i), Part::Spawn(j)) => s.spawns[i].face == -s.spawns[j].face,
        (Part::Respawn(i), Part::Respawn(j)) => (s.respawns[i].start_y - s.respawns[j].start_y).abs() <= MIRROR_TOL,
        _ => true,
    };
    if let Some(q) =
        list.iter().copied().find(|&q| q != p && same_kind_extra(q) && shape(s, q).is_some_and(|o| near(o, m)))
    {
        return Some(q);
    }
    near(sh, m).then_some(p)
}

/// After `p` was dragged by `grip`: makes its partner the mirror image (or `p` itself symmetric).
pub fn apply_mirror(s: &mut Stage, p: Part, partner: Part, grip: Grip) {
    let c = centre(s);
    let Some(sh) = shape(s, p) else { return };
    if partner == p {
        set_shape(s, p, symmetrize(sh, grip, c));
        return;
    }
    set_shape(s, partner, mirror_shape(sh, c));
    match (p, partner) {
        (Part::Spawn(i), Part::Spawn(j)) => s.spawns[j].face = -s.spawns[i].face,
        (Part::Respawn(i), Part::Respawn(j)) => s.respawns[j].start_y = s.respawns[i].start_y,
        _ => {}
    }
}

/// Adds a platform (with its mirror image when `sym` and it's off-centre); returns the new part.
pub fn add_platform(s: &mut Stage, at: P, sym: bool, sn: Snap) -> Part {
    let half = 90.;
    let (x, y) = (sn.v(at[0]), sn.v(at[1]));
    s.platforms.push(Platform { x1: x - half, x2: x + half, y });
    let p = Part::Platform(s.platforms.len() - 1);
    if sym && (x - centre(s)).abs() > half {
        let c = centre(s);
        s.platforms.push(Platform { x1: 2. * c - x - half, x2: 2. * c - x + half, y });
    }
    p
}

pub fn add_spawn(s: &mut Stage, x: f64) -> Part {
    let face = if x <= centre(s) { 1 } else { -1 };
    s.spawns.push(Spawn { x, y: None, face });
    Part::Spawn(s.spawns.len() - 1)
}

pub fn add_respawn(s: &mut Stage, x: f64) -> Part {
    let (start_y, end_y) = s.respawns.last().map_or((-80., 150.), |r| (r.start_y, r.end_y));
    s.respawns.push(RespawnPoint { x, start_y, end_y });
    Part::Respawn(s.respawns.len() - 1)
}

/// Removes a list part (and its mirror partner too when given). Returns whether anything was removed.
pub fn delete(s: &mut Stage, p: Part, partner: Option<Part>) -> bool {
    let mut idx: Vec<Part> = std::iter::once(p).chain(partner.filter(|q| *q != p)).collect();
    // Higher indices first, so the lower one stays valid.
    idx.sort_by_key(|q| std::cmp::Reverse(index(*q)));
    let mut any = false;
    for q in idx {
        // Spawns and respawns keep at least one (the loader needs it).
        match q {
            Part::Platform(i) if i < s.platforms.len() => drop(s.platforms.remove(i)),
            Part::Spawn(i) if i < s.spawns.len() && s.spawns.len() > 1 => drop(s.spawns.remove(i)),
            Part::Respawn(i) if i < s.respawns.len() && s.respawns.len() > 1 => drop(s.respawns.remove(i)),
            _ => continue,
        }
        any = true;
    }
    any
}

fn index(p: Part) -> usize {
    match p {
        Part::Platform(i) | Part::Spawn(i) | Part::Respawn(i) => i,
        _ => 0,
    }
}

/// A copy of a list part, offset a little; returns it.
pub fn duplicate(s: &mut Stage, p: Part) -> Option<Part> {
    Some(match p {
        Part::Platform(i) => {
            let mut q = s.platforms.get(i)?.clone();
            (q.x1, q.x2, q.y) = (q.x1 + 20., q.x2 + 20., q.y - 20.);
            s.platforms.push(q);
            Part::Platform(s.platforms.len() - 1)
        }
        Part::Spawn(i) => {
            let mut q = s.spawns.get(i)?.clone();
            q.x += 20.;
            s.spawns.push(q);
            Part::Spawn(s.spawns.len() - 1)
        }
        Part::Respawn(i) => {
            let mut q = s.respawns.get(i)?.clone();
            q.x += 20.;
            s.respawns.push(q);
            Part::Respawn(s.respawns.len() - 1)
        }
        _ => return None,
    })
}

/// Ledge grab zones: centre and which way the half circle opens (−1 left corner, 1 right corner).
pub fn ledges(s: &Stage) -> [(P, f64); 2] {
    let y = s.ground.top + s.ledge.cy;
    [([s.ground.left, y], -1.), ([s.ground.right, y], 1.)]
}

fn dist(a: P, b: P) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Spawn facing-arrow tip.
pub fn face_tip(s: &Stage, sp: &Spawn, px: f64) -> P {
    [sp.x + f64::from(sp.face) * 26. * px, spawn_y(s, sp) - 44. * px]
}

/// Rect handle positions: corners then edge midpoints, with their grips.
pub fn rect_handles([l, t, r, b]: [f64; 4]) -> [(P, Grip); 8] {
    let (mx, my) = ((l + r) / 2., (t + b) / 2.);
    [
        ([l, t], Grip::Sides([true, true, false, false])),
        ([r, t], Grip::Sides([false, true, true, false])),
        ([r, b], Grip::Sides([false, false, true, true])),
        ([l, b], Grip::Sides([true, false, false, true])),
        ([l, my], Grip::Sides([true, false, false, false])),
        ([mx, t], Grip::Sides([false, true, false, false])),
        ([r, my], Grip::Sides([false, false, true, false])),
        ([mx, b], Grip::Sides([false, false, false, true])),
    ]
}

/// A rect's corner, edge or (with `body`) inside under `m`; `px` = stage pixels per screen point.
pub fn rect_hit(r: [f64; 4], m: P, px: f64, body: bool) -> Option<Grip> {
    let [l, t, rt, b] = r;
    let tol = 6. * px;
    let inside_x = m[0] > l - tol && m[0] < rt + tol;
    let inside_y = m[1] > t - tol && m[1] < b + tol;
    if !(inside_x && inside_y) {
        return None;
    }
    let sides = [(m[0] - l).abs() < tol, (m[1] - t).abs() < tol, (m[0] - rt).abs() < tol, (m[1] - b).abs() < tol];
    if sides.iter().any(|s| *s) {
        return Some(Grip::Sides(sides));
    }
    (body && m[0] > l && m[0] < rt && m[1] > t && m[1] < b).then_some(Grip::Move)
}

/// A segment's end or body under `m`.
pub fn seg_hit([x1, x2, y]: [f64; 3], thick: f64, m: P, px: f64) -> Option<Grip> {
    let tol = 6. * px;
    if dist(m, [x1, y + thick / 2.]) < tol * 1.3 {
        return Some(Grip::End(0));
    }
    if dist(m, [x2, y + thick / 2.]) < tol * 1.3 {
        return Some(Grip::End(1));
    }
    (m[0] > x1 && m[0] < x2 && m[1] > y - tol && m[1] < y + thick + tol).then_some(Grip::Move)
}

/// Visibility of overlay layers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layers {
    pub bounds: bool,
    pub spawns: bool,
}

/// Topmost handle under `m`. The selected part's handles win, then spawns and respawns, platforms, the ground, the
/// ledge rims, and the bounds' edges.
pub fn hit_test(s: &Stage, sel: Option<Part>, m: P, px: f64, layers: Layers) -> Option<Handle> {
    let h = |part, grip| Some(Handle { part, grip });
    let order = |p: Part| match p {
        Part::Spawn(_) | Part::Respawn(_) => 0,
        Part::Platform(_) => 1,
        Part::Ground => 2,
        Part::Ledge => 3,
        Part::Blast | Part::Camera => 4,
    };
    let mut parts: Vec<Part> = vec![];
    if layers.spawns {
        parts.extend((0..s.spawns.len()).map(Part::Spawn));
        parts.extend((0..s.respawns.len()).map(Part::Respawn));
    }
    parts.extend((0..s.platforms.len()).rev().map(Part::Platform));
    parts.extend([Part::Ground, Part::Ledge]);
    if layers.bounds {
        parts.extend([Part::Blast, Part::Camera]);
    }
    parts.sort_by_key(|p| (Some(*p) != sel, order(*p)));
    for p in parts {
        let hit = match p {
            Part::Spawn(i) => {
                let sp = &s.spawns[i];
                let at = [sp.x, spawn_y(s, sp)];
                if dist(m, face_tip(s, sp, px)) < 8. * px {
                    h(p, Grip::Face)
                } else if dist(m, [at[0], at[1] - 18. * px]) < 12. * px {
                    h(p, Grip::Move)
                } else {
                    None
                }
            }
            Part::Respawn(i) => {
                let r = &s.respawns[i];
                if dist(m, [r.x, r.start_y]) < 7. * px {
                    h(p, Grip::Start)
                } else if (m[0] - r.x).abs() < 22. * px && (m[1] - r.end_y).abs() < 7. * px {
                    h(p, Grip::Move)
                } else {
                    None
                }
            }
            Part::Platform(i) => {
                let q = &s.platforms[i];
                seg_hit([q.x1, q.x2, q.y], s.art.platform_thickness, m, px).and_then(|g| h(p, g))
            }
            Part::Ground => {
                let g = &s.ground;
                rect_hit([g.left, g.top, g.right, g.bottom], m, px, true).and_then(|g| h(p, g))
            }
            Part::Ledge => ledges(s)
                .iter()
                .find(|(c, side)| {
                    (m[0] - c[0]) * side >= -s.ledge.reach_in && (dist(m, *c) - s.ledge.r).abs() < 5. * px
                })
                .and_then(|_| h(p, Grip::Rim)),
            Part::Blast | Part::Camera => match shape(s, p) {
                Some(Shape::Rect(r)) => rect_hit(r, m, px, false).and_then(|g| h(p, g)),
                _ => None,
            },
        };
        if hit.is_some() {
            return hit;
        }
    }
    None
}

/// Extra notes beyond the loader's checks: legal, but probably not what you want. Same shape as problems.
pub fn notes(s: &Stage, path: &str) -> Vec<ContentError> {
    let n = |p: String, msg: String| ContentError { file: path.into(), path: p, msg };
    let mut v = vec![];
    let full = 8;
    for (key, have, what) in [("spawns", s.spawns.len(), "spawns"), ("respawns", s.respawns.len(), "respawn points")] {
        if have < full {
            v.push(n(
                key.into(),
                format!(
                    "needs {full} {what} for {full} players (has {have}, so matches here allow {})",
                    s.max_players()
                ),
            ));
        }
    }
    let g = &s.ground;
    for (i, sp) in s.spawns.iter().enumerate() {
        let on_platform = s.platforms.iter().any(|p| sp.y == Some(p.y) && sp.x >= p.x1 && sp.x <= p.x2);
        if sp.y.is_none() && (sp.x < g.left || sp.x > g.right) || sp.y.is_some() && !on_platform && sp.y != Some(g.top)
        {
            v.push(n(format!("spawns[{i}]"), "isn't standing on the ground or a platform (falls at the start)".into()));
        }
    }
    let inside = |b: &sim::data::stage::Bounds, r: [f64; 4]| {
        r[0] >= b.left && r[1] >= b.top && r[2] <= b.right && r[3] <= b.bottom
    };
    if !inside(&s.blast, [g.left, g.top, g.right, g.bottom + g.under_depth]) {
        v.push(n("blast".into(), "the ground reaches past the blast zone".into()));
    }
    for (i, p) in s.platforms.iter().enumerate() {
        if !inside(&s.blast, [p.x1, p.y, p.x2, p.y]) {
            v.push(n(format!("platforms[{i}]"), "reaches past the blast zone".into()));
        }
    }
    let b = &s.blast;
    if !inside(&s.camera, [b.left, b.top, b.right, b.bottom]) {
        v.push(n("camera".into(), "should contain the blast zone, or KOs happen off camera".into()));
    }
    for (i, r) in s.respawns.iter().enumerate() {
        if r.start_y > r.end_y {
            v.push(n(format!("respawns[{i}]"), "starts below where it ends (startY > endY)".into()));
        }
    }
    v
}

// --- Overlay ---

const TEAL: u32 = 0x66ffcc;
const BLAST: u32 = 0xff5577;
const CAMERA: u32 = 0x7fb2ff;
const SEL: u32 = 0xffd35a;
const INK: u32 = 0x0b0d2a;

pub struct Overlay<'a> {
    pub s: &'a Stage,
    pub sel: Option<Part>,
    pub hover: Option<Handle>,
    pub layers: Layers,
    /// Grid spacing to draw (stage pixels) and the visible rect `[l, t, r, b]`.
    pub grid: Option<f64>,
    pub view: [f64; 4],
    pub symmetry: bool,
    /// Player colors, for spawn markers.
    pub colors: &'a [u32],
}

pub fn dashed(cv: &mut Canvas, a: P, b: P, w: f64, col: Rgba, dash: f64) {
    let len = dist(a, b);
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

fn dashed_rect(cv: &mut Canvas, [l, t, r, b]: [f64; 4], w: f64, col: Rgba, dash: f64) {
    for (a, z) in [([l, t], [r, t]), ([r, t], [r, b]), ([r, b], [l, b]), ([l, b], [l, t])] {
        dashed(cv, a, z, w, col, dash);
    }
}

fn knob(cv: &mut Canvas, p: P, px: f64, hot: bool, col: u32) {
    let r = if hot { 6. } else { 4.5 } * px;
    cv.fill_circle(p, r, Rgba::hex(INK, 0.9));
    cv.stroke_circle(p, r, 2. * px, Rgba::hex(if hot { SEL } else { col }, 1.));
}

fn square(cv: &mut Canvas, p: P, px: f64, hot: bool, col: u32) {
    let r = if hot { 5.5 } else { 4. } * px;
    let pts = [[p[0] - r, p[1] - r], [p[0] + r, p[1] - r], [p[0] + r, p[1] + r], [p[0] - r, p[1] + r]];
    cv.fill_convex(&pts, Rgba::hex(INK, 0.9));
    cv.stroke_closed(&pts, 2. * px, Rgba::hex(if hot { SEL } else { col }, 1.));
}

/// Draws the overlay; `px` = stage pixels per screen point, so strokes keep a constant screen width.
pub fn draw(cv: &mut Canvas, o: &Overlay, px: f64) {
    let s = o.s;
    let hov = |p: Part| o.hover.is_some_and(|h| h.part == p);
    let hot = |p: Part, g: Grip| o.hover == Some(Handle { part: p, grip: g });
    let [vl, vt, vr, vb] = o.view;
    if let Some(mut g) = o.grid {
        // Coarser lines when zoomed out, so the grid never turns into a haze.
        while g / px < 7. {
            g *= 2.;
        }
        let major = g * 5.;
        let mut x = (vl / g).floor() * g;
        while x <= vr {
            let m = (x / major).round() * major == x;
            cv.line([x, vt], [x, vb], px, Rgba::hex(0xffffff, if m { 0.12 } else { 0.05 }));
            x += g;
        }
        let mut y = (vt / g).floor() * g;
        while y <= vb {
            let m = (y / major).round() * major == y;
            cv.line([vl, y], [vr, y], px, Rgba::hex(0xffffff, if m { 0.12 } else { 0.05 }));
            y += g;
        }
    }
    if o.symmetry {
        let c = centre(s);
        dashed(cv, [c, vt], [c, vb], 1.5 * px, Rgba::hex(SEL, 0.55), 10. * px);
    }
    if o.layers.bounds {
        for (p, col) in [(Part::Camera, CAMERA), (Part::Blast, BLAST)] {
            let Some(Shape::Rect(r)) = shape(s, p) else { continue };
            let on = o.sel == Some(p);
            let a = if on || hov(p) { 1. } else { 0.6 };
            dashed_rect(cv, r, if on { 2.5 } else { 1.5 } * px, Rgba::hex(if on { SEL } else { col }, a), 12. * px);
            if on || hov(p) {
                for (q, g) in rect_handles(r) {
                    square(cv, q, px, hot(p, g), col);
                }
            }
        }
    }
    // Ledge grab zones.
    let l = &s.ledge;
    let lsel = o.sel == Some(Part::Ledge) || hov(Part::Ledge);
    for (c, side) in ledges(s) {
        let a0 = if side < 0. { PI / 2. } else { -PI / 2. };
        cv.fill_sector(c, 0., l.r, a0, a0 + PI, Rgba::hex(TEAL, if lsel { 0.2 } else { 0.1 }));
        let pts: Vec<P> = (0..=24)
            .map(|k| a0 + PI * f64::from(k) / 24.)
            .map(|a| [c[0] + l.r * a.cos(), c[1] + l.r * a.sin()])
            .collect();
        cv.stroke_open(
            &pts,
            if lsel { 2. } else { 1.2 } * px,
            Rgba::hex(if o.sel == Some(Part::Ledge) { SEL } else { TEAL }, 0.8),
            false,
        );
        // Where hanging feet go.
        let hang = [c[0] + side * l.hang_x, s.ground.top + l.hang_y];
        cv.line([hang[0] - 4. * px, hang[1]], [hang[0] + 4. * px, hang[1]], 1.5 * px, Rgba::hex(TEAL, 0.7));
        cv.line([c[0], c[1] - l.cy], [c[0], c[1]], 1. * px, Rgba::hex(TEAL, 0.5));
        if lsel {
            knob(cv, [c[0] + side * l.r, c[1]], px, hot(Part::Ledge, Grip::Rim), TEAL);
        }
    }
    // Ground outline and handles.
    let g = &s.ground;
    let gr = [g.left, g.top, g.right, g.bottom];
    let gsel = o.sel == Some(Part::Ground);
    if gsel || hov(Part::Ground) {
        let pts = [[gr[0], gr[1]], [gr[2], gr[1]], [gr[2], gr[3]], [gr[0], gr[3]]];
        cv.stroke_closed(&pts, if gsel { 2.5 } else { 1.5 } * px, Rgba::hex(if gsel { SEL } else { 0xffffff }, 0.85));
        for (q, gp) in rect_handles(gr) {
            square(cv, q, px, hot(Part::Ground, gp), 0xe8ecff);
        }
    }
    // Platforms.
    let th = s.art.platform_thickness;
    for (i, q) in s.platforms.iter().enumerate() {
        let p = Part::Platform(i);
        let on = o.sel == Some(p);
        if on || hov(p) {
            let pts = [[q.x1, q.y], [q.x2, q.y], [q.x2, q.y + th], [q.x1, q.y + th]];
            cv.stroke_closed(&pts, if on { 2.5 } else { 1.5 } * px, Rgba::hex(if on { SEL } else { 0xffffff }, 0.9));
            for (e, x) in [(0, q.x1), (1, q.x2)] {
                knob(cv, [x, q.y + th / 2.], px, hot(p, Grip::End(e)), 0xe8ecff);
            }
        }
    }
    if !o.layers.spawns {
        return;
    }
    // Respawn points: the descent and the platform it ends on.
    for (i, r) in s.respawns.iter().enumerate() {
        let p = Part::Respawn(i);
        let col = o.colors.get(i).copied().unwrap_or(0xffffff);
        let on = o.sel == Some(p);
        let a = if on || hov(p) { 1. } else { 0.55 };
        dashed(cv, [r.x, r.start_y], [r.x, r.end_y], 1.5 * px, Rgba::hex(col, a * 0.7), 6. * px);
        let w = 20. * px;
        cv.fill_convex(
            &[
                [r.x - w, r.end_y],
                [r.x + w, r.end_y],
                [r.x + w * 0.7, r.end_y + 5. * px],
                [r.x - w * 0.7, r.end_y + 5. * px],
            ],
            Rgba::hex(col, a * 0.8),
        );
        if on {
            cv.stroke_closed(
                &[
                    [r.x - w - 2. * px, r.end_y - 2. * px],
                    [r.x + w + 2. * px, r.end_y - 2. * px],
                    [r.x + w + 2. * px, r.end_y + 7. * px],
                    [r.x - w - 2. * px, r.end_y + 7. * px],
                ],
                2. * px,
                Rgba::hex(SEL, 1.),
            );
        }
        if on || hov(p) {
            knob(cv, [r.x, r.start_y], px, hot(p, Grip::Start), col);
        }
    }
    // Spawns: a stand-in figure outline, feet mark and facing arrow.
    for (i, sp) in s.spawns.iter().enumerate() {
        let p = Part::Spawn(i);
        let col = o.colors.get(i).copied().unwrap_or(0xffffff);
        let on = o.sel == Some(p);
        let y = spawn_y(s, sp);
        let a = if on || hov(p) { 1. } else { 0.75 };
        let body =
            [[sp.x - 9. * px, y - 36. * px], [sp.x + 9. * px, y - 36. * px], [sp.x + 9. * px, y], [sp.x - 9. * px, y]];
        cv.fill_convex(&body, Rgba::hex(col, 0.22 * a));
        cv.stroke_closed(&body, if on { 2.5 } else { 1.5 } * px, Rgba::hex(if on { SEL } else { col }, a));
        cv.line([sp.x - 12. * px, y], [sp.x + 12. * px, y], 2. * px, Rgba::hex(col, a));
        let tip = face_tip(s, sp, px);
        let base = [sp.x + f64::from(sp.face) * 8. * px, tip[1]];
        let hotf = hot(p, Grip::Face);
        cv.line(base, tip, if hotf { 3. } else { 2. } * px, Rgba::hex(if hotf { SEL } else { col }, a));
        let f = f64::from(sp.face);
        cv.fill_convex(
            &[
                [tip[0] + f * 3. * px, tip[1]],
                [tip[0] - f * 5. * px, tip[1] - 5. * px],
                [tip[0] - f * 5. * px, tip[1] + 5. * px],
            ],
            Rgba::hex(if hotf { SEL } else { col }, a),
        );
    }
}

/// Where to draw a part's label (slot number), in stage pixels.
pub fn label_at(s: &Stage, p: Part, px: f64) -> Option<P> {
    match p {
        Part::Spawn(i) => s.spawns.get(i).map(|sp| [sp.x, spawn_y(s, sp) - 26. * px]),
        Part::Respawn(i) => s.respawns.get(i).map(|r| [r.x, r.end_y + 14. * px]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arena() -> Stage {
        let files = crate::doc::tests_files();
        let (d, _) = sim::data::load(files.sources()).unwrap();
        d.stage("arena").unwrap().clone()
    }

    #[test]
    fn rect_and_segment_drags_snap_and_keep_a_minimum_size() {
        let sn = Snap { grid: Some(10.), fine: false };
        let r = Shape::Rect([100., 200., 300., 260.]);
        assert_eq!(
            drag_shape(r, Grip::Sides([true, false, false, false]), [13., 0.], sn),
            Shape::Rect([110., 200., 300., 260.])
        );
        assert_eq!(drag_shape(r, Grip::Move, [3., 7.], sn), Shape::Rect([100., 210., 300., 270.]));
        assert_eq!(
            drag_shape(r, Grip::Sides([false, false, true, false]), [-500., 0.], Snap { grid: None, fine: false }),
            Shape::Rect([100., 200., 104., 260.])
        );
        let s = Shape::Seg([0., 100., 50.]);
        assert_eq!(
            drag_shape(s, Grip::End(1), [24.6, 9.], Snap { grid: None, fine: false }),
            Shape::Seg([0., 125., 50.])
        );
        assert_eq!(drag_shape(s, Grip::Move, [0.04, 0.], Snap { grid: None, fine: true }), Shape::Seg([0., 100., 50.]));
    }

    #[test]
    fn symmetry_finds_partners_and_mirrors_edits() {
        let mut s = arena();
        // Platforms 0 and 1 mirror each other; platform 2 is centred.
        assert_eq!(partner(&s, Part::Platform(0)), Some(Part::Platform(1)));
        assert_eq!(partner(&s, Part::Platform(2)), Some(Part::Platform(2)));
        assert_eq!(partner(&s, Part::Spawn(0)), Some(Part::Spawn(1)));
        assert_eq!(partner(&s, Part::Ground), Some(Part::Ground));
        set_shape(&mut s, Part::Platform(0), Shape::Seg([380., 570., 390.]));
        apply_mirror(&mut s, Part::Platform(0), Part::Platform(1), Grip::Move);
        assert_eq!((s.platforms[1].x1, s.platforms[1].x2, s.platforms[1].y), (710., 900., 390.));
        set_shape(&mut s, Part::Ground, Shape::Rect([250., 520., 980., 580.]));
        apply_mirror(&mut s, Part::Ground, Part::Ground, Grip::Sides([true, false, false, false]));
        assert_eq!((s.ground.left, s.ground.right), (250., 1030.));
        let top = s.ground.top;
        set_shape(&mut s, Part::Spawn(0), Shape::Point([400., top]));
        apply_mirror(&mut s, Part::Spawn(0), Part::Spawn(1), Grip::Move);
        assert_eq!((s.spawns[1].x, s.spawns[1].face, s.spawns[1].y), (880., -1, None));
    }

    #[test]
    fn handles_hit_in_priority_order() {
        let s = arena();
        let all = Layers { bounds: true, spawns: true };
        let px = 1.;
        assert_eq!(
            hit_test(&s, None, [s.ground.left, s.ground.top], px, all),
            Some(Handle { part: Part::Ground, grip: Grip::Sides([true, true, false, false]) })
        );
        assert_eq!(hit_test(&s, None, [640., 560.], px, all), Some(Handle { part: Part::Ground, grip: Grip::Move }));
        let p = &s.platforms[0];
        assert_eq!(
            hit_test(&s, None, [p.x2, p.y + 3.], px, all),
            Some(Handle { part: Part::Platform(0), grip: Grip::End(1) })
        );
        let sp = &s.spawns[0];
        assert_eq!(
            hit_test(&s, None, face_tip(&s, sp, px), px, all),
            Some(Handle { part: Part::Spawn(0), grip: Grip::Face })
        );
        assert_eq!(hit_test(&s, None, [s.blast.left, 0.], px, all).map(|h| h.part), Some(Part::Blast));
        assert_eq!(hit_test(&s, None, [s.blast.left, 0.], px, Layers { bounds: false, spawns: true }), None);
    }

    #[test]
    fn adding_and_deleting_keeps_mirrored_pairs() {
        let mut s = arena();
        let sn = Snap { grid: None, fine: false };
        let n = s.platforms.len();
        let p = add_platform(&mut s, [400., 200.], true, sn);
        assert_eq!(s.platforms.len(), n + 2);
        let q = partner(&s, p);
        assert_eq!(q, Some(Part::Platform(n + 1)));
        assert!(delete(&mut s, p, q));
        assert_eq!(s.platforms.len(), n);
        assert!(notes(&s, "").is_empty(), "{:?}", notes(&s, ""));
        s.spawns.truncate(4);
        assert!(notes(&s, "").iter().any(|e| e.path == "spawns" && e.msg.contains("needs 8 spawns")));
    }
}
