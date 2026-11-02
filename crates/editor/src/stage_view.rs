//! Stage editor geometry, independent of egui: the selectable parts of a stage, generic handles (a rect's sides and
//! corners, a segment's ends, a point), hit-testing, drags with snapping and symmetry (mirrored around the stage's
//! centre x), extra validation notes, and the overlay drawn through `render::Scene::overlay`.
//!
//! Everything goes through [`Shape`]: a part reads as a rect (blocks, bounds), a horizontal segment (platforms) or
//! a point (waypoints, spawns, respawns), and drags edit the shape. New kinds of geometry add a [`Part`] variant
//! and its `shape` / `set_shape` arms; hit-testing, dragging, mirroring and drawing come for free.

use crate::overlay::{dashed, dashed_rect, knob, square};
use crate::style::{
    OVERLAY_BLAST as BLAST, OVERLAY_CAMERA as CAMERA, OVERLAY_GOLD as SEL, OVERLAY_INK as INK, OVERLAY_PATH as PATH,
    OVERLAY_TEAL as TEAL,
};

use std::f64::consts::PI;

use render::canvas::{Canvas, P, Rgba};
use sim::data::ContentError;
use sim::data::stage::{Ground, Motion, PathMode, Platform, RespawnPoint, Spawn, Stage, Waypoint};

/// A selectable piece of the stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Block(usize),
    Platform(usize),
    /// Waypoint `.1` of platform `.0`'s motion.
    Waypoint(usize, usize),
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
            Part::Block(i) => format!("blocks[{i}]"),
            Part::Platform(i) => format!("platforms[{i}]"),
            Part::Waypoint(i, j) => format!("platforms[{i}].motion.waypoints[{j}]"),
            Part::Spawn(i) => format!("spawns[{i}]"),
            Part::Respawn(i) => format!("respawns[{i}]"),
            Part::Ledge => "ledge".into(),
            Part::Blast => "blast".into(),
            Part::Camera => "camera".into(),
        }
    }
    pub fn label(self) -> String {
        match self {
            Part::Block(i) => format!("Block {}", i + 1),
            Part::Platform(i) => format!("Platform {}", i + 1),
            Part::Waypoint(i, j) => format!("Platform {} · waypoint {}", i + 1, j + 1),
            Part::Spawn(i) => format!("Spawn P{}", i + 1),
            Part::Respawn(i) => format!("Respawn P{}", i + 1),
            Part::Ledge => "Ledge zone".into(),
            Part::Blast => "Blast zone".into(),
            Part::Camera => "Camera bounds".into(),
        }
    }
    /// The platform a part belongs to (the platform itself or one of its waypoints).
    pub fn platform(self) -> Option<usize> {
        match self {
            Part::Platform(i) | Part::Waypoint(i, _) => Some(i),
            _ => None,
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
    /// A block's corner ledge toggle (click flips; `true` = right corner).
    Ledge(bool),
}

/// Something under the mouse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    pub part: Part,
    pub grip: Grip,
}

/// Feet height of a spawn: its own `y`, or the highest block top below it.
pub fn spawn_y(s: &Stage, sp: &Spawn) -> f64 {
    sp.y.or_else(|| s.spawn_y(sp.x)).unwrap_or_else(|| s.blocks.first().map_or(0., |b| b.top))
}

fn rect_of(b: &Ground) -> [f64; 4] {
    [b.left, b.top, b.right, b.bottom]
}

/// Where a waypoint puts the platform's centre.
fn waypoint_at(p: &Platform, w: &Waypoint) -> P {
    [(p.x1 + p.x2) / 2. + w.x, p.y + w.y]
}

pub fn shape(s: &Stage, p: Part) -> Option<Shape> {
    let r = |b: &sim::data::stage::Bounds| Shape::Rect([b.left, b.top, b.right, b.bottom]);
    Some(match p {
        Part::Block(i) => Shape::Rect(rect_of(s.blocks.get(i)?)),
        Part::Platform(i) => s.platforms.get(i).map(|q| Shape::Seg([q.x1, q.x2, q.y]))?,
        Part::Waypoint(i, j) => {
            let q = s.platforms.get(i)?;
            Shape::Point(waypoint_at(q, q.motion.as_ref()?.waypoints.get(j)?))
        }
        Part::Spawn(i) => s.spawns.get(i).map(|q| Shape::Point([q.x, spawn_y(s, q)]))?,
        Part::Respawn(i) => s.respawns.get(i).map(|q| Shape::Point([q.x, q.end_y]))?,
        Part::Blast => r(&s.blast),
        Part::Camera => r(&s.camera),
        Part::Ledge => return None,
    })
}

pub fn set_shape(s: &mut Stage, p: Part, sh: Shape) {
    let set_b = |b: &mut sim::data::stage::Bounds, [l, t, r, bt]: [f64; 4]| {
        (b.left, b.top, b.right, b.bottom) = (l, t, r, bt);
    };
    match (p, sh) {
        (Part::Block(i), Shape::Rect([l, t, r, b])) => {
            if let Some(g) = s.blocks.get_mut(i) {
                (g.left, g.top, g.right, g.bottom) = (l, t, r, b);
            }
        }
        (Part::Platform(i), Shape::Seg([x1, x2, y])) => {
            if let Some(q) = s.platforms.get_mut(i) {
                (q.x1, q.x2, q.y) = (x1, x2, y);
            }
        }
        (Part::Waypoint(i, j), Shape::Point([x, y])) => {
            if let Some(q) = s.platforms.get_mut(i) {
                let (mid, base) = ((q.x1 + q.x2) / 2., q.y);
                if let Some(w) = q.motion.as_mut().and_then(|m| m.waypoints.get_mut(j)) {
                    (w.x, w.y) = (x - mid, y - base);
                }
            }
        }
        (Part::Spawn(i), Shape::Point([x, y])) => {
            // Spawns standing on the block below stay block-relative.
            let floor = s.spawn_y(x);
            if let Some(q) = s.spawns.get_mut(i) {
                q.x = x;
                q.y = if q.y.is_none() && floor.is_some_and(|f| (y - f).abs() < 1e-9) { None } else { Some(y) };
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
    pub(crate) fn fighter(coarse: bool, fine: bool) -> Self {
        Self { grid: Some(if coarse { 1. } else { 0.5 }), fine }
    }
    pub fn v(self, v: f64) -> f64 {
        match (self.fine, self.grid) {
            (true, _) => crate::view::round_to(v, crate::style::FINE_STEP),
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

/// Makes a shape symmetric around `c`, keeping the side the grip moved (a move keeps it centred). The moved side
/// can't cross the axis, so the result keeps at least the minimum size.
fn symmetrize(sh: Shape, grip: Grip, c: f64) -> Shape {
    let h = MIN_SIZE / 2.;
    match (sh, grip) {
        (Shape::Rect([_, t, r, b]), Grip::Sides([false, _, true, _])) => {
            let r = r.max(c + h);
            Shape::Rect([2. * c - r, t, r, b])
        }
        (Shape::Rect([l, t, _, b]), Grip::Sides([true, _, false, _])) => {
            let l = l.min(c - h);
            Shape::Rect([l, t, 2. * c - l, b])
        }
        (Shape::Rect([l, t, r, b]), _) => {
            let w = ((r - l) / 2.).max(h);
            Shape::Rect([c - w, t, c + w, b])
        }
        (Shape::Seg([_, x2, y]), Grip::End(1)) => {
            let x2 = x2.max(c + h);
            Shape::Seg([2. * c - x2, x2, y])
        }
        (Shape::Seg([x1, _, y]), Grip::End(_)) => {
            let x1 = x1.min(c - h);
            Shape::Seg([x1, 2. * c - x1, y])
        }
        (Shape::Seg([x1, x2, y]), _) => {
            let w = ((x2 - x1) / 2.).max(h);
            Shape::Seg([c - w, c + w, y])
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

/// Same-kind parts of the stage (list elements); empty for structural singletons.
fn siblings(s: &Stage, p: Part) -> Vec<Part> {
    match p {
        Part::Block(_) => (0..s.blocks.len()).map(Part::Block).collect(),
        Part::Platform(_) => (0..s.platforms.len()).map(Part::Platform).collect(),
        Part::Spawn(_) => (0..s.spawns.len()).map(Part::Spawn).collect(),
        Part::Respawn(_) => (0..s.respawns.len()).map(Part::Respawn).collect(),
        _ => vec![],
    }
}

/// The part that mirrors `p` around the centre: another part (a mirrored pair), or `p` itself when it is its own
/// reflection (a centred list element) or a structural singleton (bounds), which symmetry keeps centred. `None`:
/// nothing mirrors it, so symmetric edits leave it alone.
pub fn partner(s: &Stage, p: Part) -> Option<Part> {
    let c = centre(s);
    if let Part::Waypoint(i, j) = p {
        return match partner(s, Part::Platform(i))? {
            Part::Platform(k) if k != i => {
                let n = |x: usize| s.platforms[x].motion.as_ref().map(|m| m.waypoints.len());
                (n(k) == n(i) && j < n(k).unwrap_or(0)).then_some(Part::Waypoint(k, j))
            }
            _ => None,
        };
    }
    let sh = shape(s, p)?;
    let m = mirror_shape(sh, c);
    let list = siblings(s, p);
    if list.is_empty() {
        return Some(p);
    }
    let matches = |q: Part| {
        let extra = match (p, q) {
            (Part::Spawn(i), Part::Spawn(j)) => i == j || s.spawns[i].face == -s.spawns[j].face,
            (Part::Respawn(i), Part::Respawn(j)) => (s.respawns[i].start_y - s.respawns[j].start_y).abs() <= MIRROR_TOL,
            (Part::Block(i), Part::Block(j)) => {
                let (a, b) = (&s.blocks[i], &s.blocks[j]);
                (a.under_depth - b.under_depth).abs() <= MIRROR_TOL
                    && (a.under_inset - b.under_inset).abs() <= MIRROR_TOL
            }
            _ => true,
        };
        extra && shape(s, q).is_some_and(|o| near(o, m))
    };
    list.iter().copied().find(|&q| q != p && matches(q)).or_else(|| matches(p).then_some(p))
}

/// Ticks in one full cycle of a path (as `Motion::offset` walks it).
pub fn cycle(m: &Motion) -> u64 {
    let n = m.waypoints.len();
    if n < 2 {
        return 0;
    }
    let count = if m.mode == PathMode::Loop { n } else { 2 * (n - 1) };
    let index = |k: usize| if k < n { k } else { 2 * (n - 1) - k };
    (0..count)
        .map(|k| {
            let (a, b) = (index(k), index((k + 1) % count));
            let travel = m.waypoints[if m.mode == PathMode::PingPong { a.min(b) } else { a }].travel_ticks;
            u64::from(m.waypoints[a].wait_ticks) + u64::from(travel)
        })
        .sum()
}

/// `m` with every waypoint reflected (x offsets negated).
fn mirror_motion(m: &Motion) -> Motion {
    let mut m = m.clone();
    m.waypoints.iter_mut().for_each(|w| w.x = -w.x);
    m
}

/// After `p` was dragged by `grip`: makes its partner the mirror image (or `p` itself symmetric).
pub fn apply_mirror(s: &mut Stage, p: Part, partner: Part, grip: Grip) {
    let c = centre(s);
    let Some(sh) = shape(s, p) else { return };
    if partner == p {
        if !matches!(p, Part::Waypoint(..)) {
            set_shape(s, p, symmetrize(sh, grip, c));
        }
        return;
    }
    set_shape(s, partner, mirror_shape(sh, c));
    match (p, partner) {
        (Part::Spawn(i), Part::Spawn(j)) => s.spawns[j].face = -s.spawns[i].face,
        (Part::Respawn(i), Part::Respawn(j)) => s.respawns[j].start_y = s.respawns[i].start_y,
        (Part::Platform(i), Part::Platform(j)) => {
            s.platforms[j].motion = s.platforms[i].motion.as_ref().map(mirror_motion)
        }
        (Part::Block(i), Part::Block(j)) => mirror_block_extras(s, i, j),
        _ => {}
    }
}

/// Underside and ledges of block `j` from its mirror image `i` (left and right swap).
fn mirror_block_extras(s: &mut Stage, i: usize, j: usize) {
    let a = s.blocks[i].clone();
    let b = &mut s.blocks[j];
    (b.under_depth, b.under_inset) = (a.under_depth, a.under_inset);
    (b.left_ledge, b.right_ledge) = (a.right_ledge, a.left_ledge);
}

/// Flips a block corner's ledge; with symmetry, its mirror corner follows.
pub fn toggle_ledge(s: &mut Stage, i: usize, right: bool, sym: bool) {
    let partner = if sym { partner(s, Part::Block(i)) } else { None };
    let Some(b) = s.blocks.get_mut(i) else { return };
    let v = if right { &mut b.right_ledge } else { &mut b.left_ledge };
    *v = !*v;
    let on = *v;
    match partner {
        Some(Part::Block(j)) if j == i => (b.left_ledge, b.right_ledge) = (on, on),
        Some(Part::Block(j)) => mirror_block_extras(s, i, j),
        _ => {}
    }
}

/// Adds a platform at `at`. With symmetry, one that would cross the axis is centred on it; otherwise its mirror
/// image is added too. Returns the new part.
pub fn add_platform(s: &mut Stage, at: P, sym: bool, sn: Snap) -> Part {
    let half = 90.;
    let c = centre(s);
    let (mut x, y) = (sn.v(at[0]), sn.v(at[1]));
    if sym && (x - c).abs() < half {
        x = c;
    }
    s.platforms.push(Platform { x1: x - half, x2: x + half, y, motion: None });
    let p = Part::Platform(s.platforms.len() - 1);
    if sym && x != c {
        s.platforms.push(Platform { x1: 2. * c - x - half, x2: 2. * c - x + half, y, motion: None });
    }
    p
}

/// Adds a solid block at `at`, the same way as [`add_platform`].
pub fn add_block(s: &mut Stage, at: P, sym: bool, sn: Snap) -> Part {
    let (half, h) = (100., 40.);
    let c = centre(s);
    let (mut x, y) = (sn.v(at[0]), sn.v(at[1]));
    if sym && (x - c).abs() < half {
        x = c;
    }
    let block = |x: f64| Ground {
        left: x - half,
        right: x + half,
        top: y,
        bottom: y + h,
        under_depth: 30.,
        under_inset: 30.,
        left_ledge: true,
        right_ledge: true,
    };
    s.blocks.push(block(x));
    let p = Part::Block(s.blocks.len() - 1);
    if sym && x != c {
        s.blocks.push(block(2. * c - x));
    }
    p
}

pub fn add_spawn(s: &mut Stage, x: f64) -> Part {
    let face = if x <= centre(s) { 1 } else { -1 };
    let y = if s.spawn_y(x).is_some() { None } else { s.blocks.first().map(|b| b.top) };
    s.spawns.push(Spawn { x, y, face });
    Part::Spawn(s.spawns.len() - 1)
}

pub fn add_respawn(s: &mut Stage, x: f64) -> Part {
    let (start_y, end_y) = s.respawns.last().map_or((-80., 150.), |r| (r.start_y, r.end_y));
    s.respawns.push(RespawnPoint { x, start_y, end_y });
    Part::Respawn(s.respawns.len() - 1)
}

/// Gives platform `i` a path (two waypoints, ping-pong) or adds a waypoint past its last one. Returns the new
/// waypoint.
pub fn add_waypoint(s: &mut Stage, i: usize) -> Option<Part> {
    let p = s.platforms.get_mut(i)?;
    let w = |x: f64, y: f64| Waypoint { x, y, travel_ticks: 120, wait_ticks: 30 };
    match &mut p.motion {
        None => {
            p.motion = Some(Motion { waypoints: vec![w(0., 0.), w(0., -120.)], mode: PathMode::PingPong, phase: 0 });
            Some(Part::Waypoint(i, 1))
        }
        Some(m) => {
            let last = m.waypoints.last().map_or((0., 0.), |l| (l.x, l.y));
            m.waypoints.push(w(last.0 + 80., last.1));
            Some(Part::Waypoint(i, m.waypoints.len() - 1))
        }
    }
}

/// Removes a list part, and its mirror partner too when given. The whole removal is checked first and refused if
/// it would leave the stage without a block, spawn or respawn point, or a path with fewer than two waypoints.
pub fn delete(s: &mut Stage, p: Part, partner: Option<Part>) -> Result<(), String> {
    let mut set: Vec<Part> = std::iter::once(p).chain(partner.filter(|q| *q != p)).collect();
    let count = |f: fn(&Part) -> bool| set.iter().filter(|q| f(q)).count();
    let left = |have: usize, gone: usize| have.saturating_sub(gone);
    if !set.iter().all(|q| shape(s, *q).is_some()) {
        return Err("Nothing to delete".into());
    }
    if left(s.blocks.len(), count(|q| matches!(q, Part::Block(_)))) == 0 {
        return Err("A stage needs at least one solid block".into());
    }
    if left(s.spawns.len(), count(|q| matches!(q, Part::Spawn(_)))) == 0 {
        return Err("A stage needs at least one spawn".into());
    }
    if left(s.respawns.len(), count(|q| matches!(q, Part::Respawn(_)))) == 0 {
        return Err("A stage needs at least one respawn point".into());
    }
    for q in &set {
        if let Part::Waypoint(i, _) = q {
            let n = s.platforms[*i].motion.as_ref().map_or(0, |m| m.waypoints.len());
            if n <= 2 {
                return Err("A path needs two waypoints · remove the motion instead".into());
            }
        }
    }
    if matches!(p, Part::Ledge | Part::Blast | Part::Camera) {
        return Err(format!("{} can't be deleted", p.label()));
    }
    // Higher indices first, so the lower one stays valid.
    set.sort_by_key(|q| std::cmp::Reverse(index(*q)));
    for q in set {
        match q {
            Part::Block(i) => drop(s.blocks.remove(i)),
            Part::Platform(i) => drop(s.platforms.remove(i)),
            Part::Spawn(i) => drop(s.spawns.remove(i)),
            Part::Respawn(i) => drop(s.respawns.remove(i)),
            Part::Waypoint(i, j) => {
                if let Some(m) = s.platforms[i].motion.as_mut() {
                    m.waypoints.remove(j);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn index(p: Part) -> usize {
    match p {
        Part::Block(i) | Part::Platform(i) | Part::Spawn(i) | Part::Respawn(i) | Part::Waypoint(_, i) => i,
        _ => 0,
    }
}

/// A copy of a list part, offset a little; returns it.
pub fn duplicate(s: &mut Stage, p: Part) -> Option<Part> {
    Some(match p {
        Part::Block(i) => {
            let mut q = s.blocks.get(i)?.clone();
            let w = q.right - q.left + 20.;
            (q.left, q.right) = (q.left + w, q.right + w);
            s.blocks.push(q);
            Part::Block(s.blocks.len() - 1)
        }
        Part::Platform(i) => {
            let mut q = s.platforms.get(i)?.clone();
            (q.x1, q.x2, q.y) = (q.x1 + 20., q.x2 + 20., q.y - 20.);
            s.platforms.push(q);
            Part::Platform(s.platforms.len() - 1)
        }
        Part::Waypoint(i, j) => {
            let m = s.platforms.get_mut(i)?.motion.as_mut()?;
            let mut w = m.waypoints.get(j)?.clone();
            w.x += 40.;
            m.waypoints.insert(j + 1, w);
            Part::Waypoint(i, j + 1)
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

/// Enabled ledge grab zones: centre, which way the half circle opens (−1 left corner, 1 right corner), block.
pub fn ledges(s: &Stage) -> Vec<(P, f64, usize)> {
    let mut v = vec![];
    for (i, b) in s.blocks.iter().enumerate() {
        let y = b.top + s.ledge.cy;
        if b.left_ledge {
            v.push(([b.left, y], -1., i));
        }
        if b.right_ledge {
            v.push(([b.right, y], 1., i));
        }
    }
    v
}

/// A block corner's ledge toggle, a little above the corner.
pub fn ledge_toggle(b: &Ground, right: bool, px: f64) -> P {
    [if right { b.right } else { b.left }, b.top - 20. * px]
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

/// Topmost handle under `m`. The selected part's handles win, then the selected platform's waypoints, spawns and
/// respawns, platforms, blocks (with the selected block's ledge toggles), the ledge rims, and the bounds' edges.
pub fn hit_test(s: &Stage, sel: Option<Part>, m: P, px: f64, layers: Layers) -> Option<Handle> {
    let h = |part, grip| Some(Handle { part, grip });
    let order = |p: Part| match p {
        Part::Waypoint(..) => 0,
        Part::Spawn(_) | Part::Respawn(_) => 1,
        Part::Platform(_) => 2,
        Part::Block(_) => 3,
        Part::Ledge => 4,
        Part::Blast | Part::Camera => 5,
    };
    let mut parts: Vec<Part> = vec![];
    if let Some(i) = sel.and_then(Part::platform)
        && let Some(m) = s.platforms.get(i).and_then(|p| p.motion.as_ref())
    {
        parts.extend((0..m.waypoints.len()).map(|j| Part::Waypoint(i, j)));
    }
    if layers.spawns {
        parts.extend((0..s.spawns.len()).map(Part::Spawn));
        parts.extend((0..s.respawns.len()).map(Part::Respawn));
    }
    parts.extend((0..s.platforms.len()).rev().map(Part::Platform));
    parts.extend((0..s.blocks.len()).rev().map(Part::Block));
    parts.push(Part::Ledge);
    if layers.bounds {
        parts.extend([Part::Blast, Part::Camera]);
    }
    parts.sort_by_key(|p| (Some(*p) != sel, order(*p)));
    for p in parts {
        let hit = match p {
            Part::Waypoint(..) => match shape(s, p) {
                Some(Shape::Point(at)) if dist(m, at) < 7. * px => h(p, Grip::Move),
                _ => None,
            },
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
            Part::Block(i) => {
                let b = &s.blocks[i];
                let toggle =
                    [false, true].into_iter().find(|r| Some(p) == sel && dist(m, ledge_toggle(b, *r, px)) < 8. * px);
                match toggle {
                    Some(r) => h(p, Grip::Ledge(r)),
                    None => rect_hit(rect_of(b), m, px, true).and_then(|g| h(p, g)),
                }
            }
            Part::Ledge => ledges(s)
                .iter()
                .find(|(c, side, _)| {
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
    for (i, sp) in s.spawns.iter().enumerate() {
        let y = spawn_y(s, sp);
        let on_block = s.blocks.iter().any(|b| (b.top - y).abs() < 1e-6 && sp.x >= b.left && sp.x <= b.right);
        let on_platform = s.platforms.iter().any(|p| {
            let (x1, x2, py) = p.position(0.);
            (py - y).abs() < 1e-6 && sp.x >= x1 && sp.x <= x2
        });
        if !on_block && !on_platform {
            v.push(n(format!("spawns[{i}]"), "isn't standing on a block or platform (falls at the start)".into()));
        }
    }
    let inside = |b: &sim::data::stage::Bounds, r: [f64; 4]| {
        r[0] >= b.left && r[1] >= b.top && r[2] <= b.right && r[3] <= b.bottom
    };
    for (i, g) in s.blocks.iter().enumerate() {
        if !inside(&s.blast, [g.left, g.top, g.right, g.bottom + g.under_depth]) {
            v.push(n(format!("blocks[{i}]"), "reaches past the blast zone".into()));
        }
    }
    for (i, p) in s.platforms.iter().enumerate() {
        let path: Vec<(f64, f64)> =
            p.motion.as_ref().map_or_else(|| vec![(0., 0.)], |m| m.waypoints.iter().map(|w| (w.x, w.y)).collect());
        if path.iter().any(|(dx, dy)| !inside(&s.blast, [p.x1 + dx, p.y + dy, p.x2 + dx, p.y + dy])) {
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
    if s.blocks.iter().all(|b| !b.left_ledge && !b.right_ledge) {
        v.push(n("blocks".into(), "no corner has a ledge, so recovering means landing on top".into()));
    }
    v
}

// --- Overlay ---

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
    pub zoom_view: Option<[f64; 4]>,
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
        if let Some(r) = o.zoom_view {
            dashed_rect(cv, r, px, Rgba::hex(CAMERA, 0.8), 10. * px);
        }
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
    draw_ledges(cv, o, px);
    draw_blocks(cv, o, px);
    draw_platforms(cv, o, px);
    if o.layers.spawns {
        draw_respawns(cv, o, px);
        draw_spawns(cv, o, px);
    }
}

fn draw_ledges(cv: &mut Canvas, o: &Overlay, px: f64) {
    let s = o.s;
    let hov = |p: Part| o.hover.is_some_and(|h| h.part == p);
    let hot = |p: Part, g: Grip| o.hover == Some(Handle { part: p, grip: g });
    // Ledge grab zones, only on enabled corners.
    let l = &s.ledge;
    let lsel = o.sel == Some(Part::Ledge) || hov(Part::Ledge);
    for (c, side, i) in ledges(s) {
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
        let hang = [c[0] + side * l.hang_x, s.blocks[i].top + l.hang_y];
        cv.line([hang[0] - 4. * px, hang[1]], [hang[0] + 4. * px, hang[1]], 1.5 * px, Rgba::hex(TEAL, 0.7));
        cv.line([c[0], c[1] - l.cy], [c[0], c[1]], px, Rgba::hex(TEAL, 0.5));
        if lsel {
            knob(cv, [c[0] + side * l.r, c[1]], px, hot(Part::Ledge, Grip::Rim), TEAL);
        }
    }
}

fn draw_blocks(cv: &mut Canvas, o: &Overlay, px: f64) {
    let s = o.s;
    let hov = |p: Part| o.hover.is_some_and(|h| h.part == p);
    let hot = |p: Part, g: Grip| o.hover == Some(Handle { part: p, grip: g });
    // Block outlines, handles and the selected block's ledge toggles.
    for (i, b) in s.blocks.iter().enumerate() {
        let p = Part::Block(i);
        let on = o.sel == Some(p);
        if !(on || hov(p)) {
            continue;
        }
        let r = rect_of(b);
        let pts = [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]];
        cv.stroke_closed(&pts, if on { 2.5 } else { 1.5 } * px, Rgba::hex(if on { SEL } else { 0xffffff }, 0.85));
        for (q, gp) in rect_handles(r) {
            square(cv, q, px, hot(p, gp), 0xe8ecff);
        }
        if on {
            for right in [false, true] {
                let at = ledge_toggle(b, right, px);
                let enabled = if right { b.right_ledge } else { b.left_ledge };
                let hotl = hot(p, Grip::Ledge(right));
                let rad = if hotl { 7. } else { 6. } * px;
                cv.fill_circle(at, rad, Rgba::hex(if enabled { TEAL } else { INK }, if enabled { 0.9 } else { 0.85 }));
                cv.stroke_circle(at, rad, 1.5 * px, Rgba::hex(if hotl { SEL } else { TEAL }, 1.));
                let corner = [if right { b.right } else { b.left }, b.top];
                dashed(cv, [at[0], at[1] + rad], corner, px, Rgba::hex(TEAL, 0.6), 3. * px);
            }
        }
    }
}

fn draw_platforms(cv: &mut Canvas, o: &Overlay, px: f64) {
    let s = o.s;
    let hov = |p: Part| o.hover.is_some_and(|h| h.part == p);
    let hot = |p: Part, g: Grip| o.hover == Some(Handle { part: p, grip: g });
    // Platforms, with paths for moving ones.
    let th = s.art.platform_thickness;
    for (i, q) in s.platforms.iter().enumerate() {
        let p = Part::Platform(i);
        let mine = o.sel.and_then(Part::platform) == Some(i);
        let on = o.sel == Some(p);
        if let Some(m) = &q.motion {
            let pts: Vec<P> = m.waypoints.iter().map(|w| waypoint_at(q, w)).collect();
            let a = if mine || hov(p) { 0.9 } else { 0.4 };
            for k in 0..pts.len() {
                if k + 1 < pts.len() || m.mode == PathMode::Loop && pts.len() > 2 {
                    dashed(cv, pts[k], pts[(k + 1) % pts.len()], 1.5 * px, Rgba::hex(PATH, a), 7. * px);
                }
            }
            if mine {
                let half = (q.x2 - q.x1) / 2.;
                for (j, w) in pts.iter().enumerate() {
                    let ghost =
                        [[w[0] - half, w[1]], [w[0] + half, w[1]], [w[0] + half, w[1] + th], [w[0] - half, w[1] + th]];
                    dashed_rect(
                        cv,
                        [ghost[0][0], ghost[0][1], ghost[2][0], ghost[2][1]],
                        px,
                        Rgba::hex(PATH, 0.5),
                        5. * px,
                    );
                    let wp = Part::Waypoint(i, j);
                    let sel = o.sel == Some(wp);
                    knob(cv, *w, px, hot(wp, Grip::Move) || sel, if sel { SEL } else { PATH });
                }
            }
        }
        if on || hov(p) {
            let pts = [[q.x1, q.y], [q.x2, q.y], [q.x2, q.y + th], [q.x1, q.y + th]];
            cv.stroke_closed(&pts, if on { 2.5 } else { 1.5 } * px, Rgba::hex(if on { SEL } else { 0xffffff }, 0.9));
            for (e, x) in [(0, q.x1), (1, q.x2)] {
                knob(cv, [x, q.y + th / 2.], px, hot(p, Grip::End(e)), 0xe8ecff);
            }
        }
    }
}

fn draw_respawns(cv: &mut Canvas, o: &Overlay, px: f64) {
    let s = o.s;
    let hov = |p: Part| o.hover.is_some_and(|h| h.part == p);
    let hot = |p: Part, g: Grip| o.hover == Some(Handle { part: p, grip: g });
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
}

fn draw_spawns(cv: &mut Canvas, o: &Overlay, px: f64) {
    let s = o.s;
    let hov = |p: Part| o.hover.is_some_and(|h| h.part == p);
    let hot = |p: Part, g: Grip| o.hover == Some(Handle { part: p, grip: g });
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
    const FREE: Snap = Snap { grid: None, fine: false };

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
            drag_shape(r, Grip::Sides([false, false, true, false]), [-500., 0.], FREE),
            Shape::Rect([100., 200., 104., 260.])
        );
        let s = Shape::Seg([0., 100., 50.]);
        assert_eq!(drag_shape(s, Grip::End(1), [24.6, 9.], FREE), Shape::Seg([0., 125., 50.]));
        assert_eq!(drag_shape(s, Grip::Move, [0.04, 0.], Snap { grid: None, fine: true }), Shape::Seg([0., 100., 50.]));
    }

    #[test]
    fn symmetry_finds_partners_and_mirrors_edits() {
        let mut s = arena();
        // Platforms 0 and 1 mirror each other; platform 2 is centred.
        assert_eq!(partner(&s, Part::Platform(0)), Some(Part::Platform(1)));
        assert_eq!(partner(&s, Part::Platform(2)), Some(Part::Platform(2)));
        assert_eq!(partner(&s, Part::Spawn(0)), Some(Part::Spawn(1)));
        assert_eq!(partner(&s, Part::Block(0)), Some(Part::Block(0)));
        assert_eq!(partner(&s, Part::Blast), Some(Part::Blast));
        set_shape(&mut s, Part::Platform(0), Shape::Seg([380., 570., 390.]));
        apply_mirror(&mut s, Part::Platform(0), Part::Platform(1), Grip::Move);
        assert_eq!((s.platforms[1].x1, s.platforms[1].x2, s.platforms[1].y), (710., 900., 390.));
        set_shape(&mut s, Part::Block(0), Shape::Rect([250., 520., 980., 580.]));
        apply_mirror(&mut s, Part::Block(0), Part::Block(0), Grip::Sides([true, false, false, false]));
        assert_eq!((s.blocks[0].left, s.blocks[0].right), (250., 1030.));
        let top = s.blocks[0].top;
        set_shape(&mut s, Part::Spawn(0), Shape::Point([400., top]));
        apply_mirror(&mut s, Part::Spawn(0), Part::Spawn(1), Grip::Move);
        assert_eq!((s.spawns[1].x, s.spawns[1].face, s.spawns[1].y), (880., -1, None));
    }

    #[test]
    fn mirrored_resizes_never_invert_centred_geometry() {
        let mut s = arena();
        // Drag the centred platform's x1 far past the axis.
        set_shape(&mut s, Part::Platform(2), Shape::Seg([900., 730., 285.]));
        apply_mirror(&mut s, Part::Platform(2), Part::Platform(2), Grip::End(0));
        let p = &s.platforms[2];
        assert!(p.x1 < p.x2 && (p.x1 + p.x2) / 2. == 640., "{p:?}");
        set_shape(&mut s, Part::Block(0), Shape::Rect([300., 520., 100., 580.]));
        apply_mirror(&mut s, Part::Block(0), Part::Block(0), Grip::Sides([false, false, true, false]));
        assert!(s.blocks[0].left < s.blocks[0].right);
        set_shape(&mut s, Part::Blast, Shape::Rect([900., -380., 1500., 940.]));
        apply_mirror(&mut s, Part::Blast, Part::Blast, Grip::Sides([true, false, false, false]));
        assert!(s.blast.left < s.blast.right);
    }

    #[test]
    fn lone_off_centre_parts_have_no_partner() {
        let mut s = arena();
        s.platforms.truncate(1);
        assert_eq!(partner(&s, Part::Platform(0)), None, "a single off-centre platform isn't its own mirror");
        s.spawns.truncate(1);
        assert_eq!(partner(&s, Part::Spawn(0)), None);
        // A block that isn't centred, alone, isn't either.
        set_shape(&mut s, Part::Block(0), Shape::Rect([100., 520., 500., 580.]));
        assert_eq!(partner(&s, Part::Block(0)), None);
    }

    #[test]
    fn mirror_blocks_swap_ledges_and_carry_paths() {
        let mut s = arena();
        let a = add_block(&mut s, [200., 300.], true, FREE);
        assert_eq!(s.blocks.len(), 3);
        let Part::Block(i) = a else { panic!() };
        assert_eq!(partner(&s, a), Some(Part::Block(i + 1)));
        toggle_ledge(&mut s, i, false, true);
        assert_eq!((s.blocks[i].left_ledge, s.blocks[i + 1].right_ledge), (false, false));
        assert!(s.blocks[i + 1].left_ledge);
        // Centred blocks toggle both corners together.
        toggle_ledge(&mut s, 0, true, true);
        assert_eq!((s.blocks[0].left_ledge, s.blocks[0].right_ledge), (false, false));
        // Platform paths mirror with their platform.
        let w = add_waypoint(&mut s, 0).unwrap();
        set_shape(&mut s, w, Shape::Point([560., 300.]));
        apply_mirror(&mut s, Part::Platform(0), Part::Platform(1), Grip::Move);
        let (m0, m1) = (s.platforms[0].motion.as_ref().unwrap(), s.platforms[1].motion.as_ref().unwrap());
        assert_eq!(m1.waypoints[1].x, -m0.waypoints[1].x);
        assert_eq!(partner(&s, w), Some(Part::Waypoint(1, 1)));
    }

    #[test]
    fn adding_near_the_axis_centres_and_elsewhere_pairs() {
        let mut s = arena();
        let n = s.platforms.len();
        add_platform(&mut s, [600., 200.], true, FREE);
        assert_eq!(s.platforms.len(), n + 1);
        assert_eq!(partner(&s, Part::Platform(n)), Some(Part::Platform(n)));
        let p = add_platform(&mut s, [400., 200.], true, FREE);
        assert_eq!(s.platforms.len(), n + 3);
        let q = partner(&s, p);
        assert_eq!(q, Some(Part::Platform(n + 2)));
        delete(&mut s, p, q).unwrap();
        assert_eq!(s.platforms.len(), n + 1);
    }

    #[test]
    fn deletions_are_checked_as_a_whole() {
        let mut s = arena();
        s.spawns.truncate(2);
        // The last mirrored pair: refused, and nothing removed.
        assert!(delete(&mut s, Part::Spawn(0), Some(Part::Spawn(1))).is_err());
        assert_eq!(s.spawns.len(), 2);
        assert!(delete(&mut s, Part::Spawn(0), None).is_ok());
        assert!(delete(&mut s, Part::Block(0), None).is_err());
        add_waypoint(&mut s, 0);
        assert!(delete(&mut s, Part::Waypoint(0, 1), None).is_err());
    }

    #[test]
    fn handles_hit_in_priority_order() {
        let s = arena();
        let all = Layers { bounds: true, spawns: true };
        let px = 1.;
        let b = &s.blocks[0];
        assert_eq!(
            hit_test(&s, None, [b.left, b.top], px, all),
            Some(Handle { part: Part::Block(0), grip: Grip::Sides([true, true, false, false]) })
        );
        assert_eq!(hit_test(&s, None, [640., 560.], px, all), Some(Handle { part: Part::Block(0), grip: Grip::Move }));
        assert_eq!(
            hit_test(&s, Some(Part::Block(0)), ledge_toggle(b, true, px), px, all),
            Some(Handle { part: Part::Block(0), grip: Grip::Ledge(true) })
        );
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
    fn notes_flag_missing_spawns() {
        let mut s = arena();
        assert!(notes(&s, "").is_empty(), "{:?}", notes(&s, ""));
        s.spawns.truncate(4);
        assert!(notes(&s, "").iter().any(|e| e.path == "spawns" && e.msg.contains("needs 8 spawns")));
        s.blocks[0].left_ledge = false;
        assert_eq!(ledges(&s).len(), 1);
    }
}
