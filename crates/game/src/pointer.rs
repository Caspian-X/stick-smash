//! Player pointers on the select screens. An analog stick glides the pointer (with a little acceleration); as it
//! slows it is pulled sideways onto the nearest target, and once released it eases onto it, so it never rests
//! between two things. Digital presses hop to the neighbouring target (grid moves from [`grid::drive`], otherwise
//! [`Cursor::drive`]) and slide there. The mouse moves its pointer directly, with no magnet. The pointer's cursor key
//! is whatever target is under it (`NONE`: nothing).
//!
//! The look: an arrow drawn like a fighter's limbs (three thick rounded strokes with a dark outline and a neon glow in
//! the player's colour). Whatever chip the player holds rides on its tail: their own until they drop it, or a CPU's
//! while carrying it. An empty arrow shows only a small player number. It squashes and bounces when it lands on a
//! target or drops a chip, leans into and stretches along its travel, the chip trails on a spring, and a pointer left
//! still breathes. Holding Back to leave a select screen fills a ring of dashes around the arrow ([`Ring`]) in the
//! player's colour, with a spark riding its filled end; letting go drains it away. No text is involved.

use std::collections::BTreeMap;
use std::ops::{Add, Mul, Sub};

use bevy::prelude::*;
use sim::data::render::{BackRing, PointerLook, PointerMotion, SelectLook};

use crate::grid;
use crate::input::{Dev, DevState, Devices, Nav};
use crate::ui::*;

pub struct PointerPlugin;

impl Plugin for PointerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pointers>()
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((root(), PointerLayer, GlobalZIndex(z::POINTERS), Pickable::IGNORE));
            })
            .add_systems(Update, draw.after(UiSystems::Animate).before(render::RenderSystems));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Drive {
    #[default]
    Digital,
    Analog,
    Mouse,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Look {
    pub color: Color,
    /// The player's number, small near the tail when the arrow doesn't carry their own chip.
    pub num: String,
    /// The chip riding on the arrow: its label and colour.
    pub chip: Option<(String, Color)>,
    /// That chip is the player's own.
    pub own: bool,
}

impl Look {
    /// Player `slot`'s pointer: holding their own chip (`held`), else carrying someone else's (`carry`) or empty.
    pub fn new(color: Color, slot: usize, held: bool, carry: Option<(String, Color)>) -> Self {
        let own = held && carry.is_none();
        let chip = carry.or_else(|| held.then(|| (format!("P{}", slot + 1), color)));
        Look { color, num: (slot + 1).to_string(), chip, own }
    }
}

/// Motion-driven animation, stepped with real time by [`Pointer::animate`]: springs as (value, velocity).
#[derive(Clone, Debug, Default)]
pub struct Anim {
    last: Option<Vec2>,
    pub lean: (f32, f32),
    pub stretch: (f32, f32),
    /// Last direction of travel.
    dir: Vec2,
    /// Where the chip is (it trails the tip).
    pub chip: (Vec2, Vec2),
    /// Seconds still, and of animation.
    still: f32,
    clock: f32,
}

/// How to draw a pointer this frame: the arrow's lean (radians, clockwise), scale and bob, and the chip's offset
/// and swing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub lean: f32,
    pub scale: Vec2,
    pub bob: f32,
    pub chip: Vec2,
    pub swing: f32,
}

/// One semi-implicit step of a damped spring toward `to`: `[stiffness, damping ratio]`.
fn spring<T>(x: &mut T, v: &mut T, to: T, [k, z]: [f32; 2], h: f32)
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<f32, Output = T>,
{
    *v = *v + ((to - *x) * k - *v * (2. * z * k.sqrt())) * h;
    *x = *x + *v * h;
}

#[derive(Clone, Debug, Default)]
pub struct Pointer {
    pub pos: Vec2,
    pub vel: Vec2,
    pub drive: Drive,
    /// Digital hop in progress: where it started and seconds into the slide.
    slide: Option<(Vec2, f32)>,
    /// Settled on its target (lands once).
    rest: bool,
    /// Seconds since the last bounce, and its amplitude.
    pub bounce: (f32, f32),
    /// Fan-out index, so pointers on one target don't cover each other.
    pub fan: usize,
    pub look: Look,
    pub anim: Anim,
}

#[derive(Resource, Default)]
pub struct Pointers {
    pub back: BTreeMap<Dev, BackHold>,
    /// The ring each device's hold fills ([`Ring`]). It outlives the hold, so a released one can drain away.
    pub ring: BTreeMap<Dev, Ring>,
    pub map: BTreeMap<Dev, Pointer>,
    /// Last mouse position; only motion moves the keyboard player's pointer.
    pub mouse: Option<Vec2>,
    /// Screen size in logical UI pixels.
    pub screen: Vec2,
}

#[derive(Default)]
pub struct BackHold {
    elapsed: f32,
    consumed: bool,
}

/// A device's hold-to-go-back ring: the filled part chases the hold and swings as it fills, and a released hold
/// drains it and shrinks it away.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ring {
    /// Filled fraction and its velocity.
    shown: (f32, f32),
    /// Group scale and its velocity.
    pop: (f32, f32),
    /// Seconds of wobble.
    clock: f32,
    /// Where the pointer carrying it is (the screen centre for a device without one).
    pub at: Vec2,
    pub color: Color,
}

impl Ring {
    /// A ring just starting to show: it pops in from the configured scale.
    fn start(look: &BackRing) -> Self {
        Self { pop: (look.pop, 0.), ..Self::default() }
    }
    /// Steps toward a hold of `target` progress (`0..1`) by `dt` real seconds, in slices short enough that any frame
    /// rate moves alike.
    fn step(&mut self, target: f32, look: &BackRing, dt: f32) {
        let dt = dt.clamp(0., 0.05);
        let n = (dt * 240.).ceil().max(1.);
        let h = dt / n;
        // A released hold (0) drains the ring and shrinks it away.
        let (follow, grow) = (look.follow, look.grow);
        for _ in 0..n as usize {
            spring(&mut self.shown.0, &mut self.shown.1, target, follow, h);
            spring(&mut self.pop.0, &mut self.pop.1, if target > 0. { 1. } else { 0. }, grow, h);
        }
        self.shown.0 = self.shown.0.clamp(0., 1.2);
        self.clock += dt;
    }
    /// Grown away: nothing to draw.
    fn dead(&self) -> bool {
        self.pop.0 < 0.02 && self.shown.0 < 0.02
    }
    /// How much of the ring is filled.
    fn shown(&self) -> f32 {
        self.shown.0.clamp(0., 1.)
    }
    /// Scale of the whole ring this frame, wobbling as it fills.
    fn scale(&self, look: &BackRing) -> f32 {
        let w = (self.clock * std::f32::consts::TAU / look.wobble_secs.max(0.05)).sin();
        (self.pop.0 * (1. + w * look.wobble * 0.5 * self.shown())).max(0.)
    }
    /// Its lean this frame, in radians.
    fn lean(&self, look: &BackRing) -> f32 {
        let w = (self.clock * std::f32::consts::TAU / look.wobble_secs.max(0.05)).sin();
        w * look.wobble * self.shown()
    }
}

impl Pointers {
    pub fn hold_back(&mut self, dev: Dev, st: &crate::input::DevState, contextual: bool, dt: f32, secs: f32) -> bool {
        if !st.back_held() {
            self.back.remove(&dev);
            return false;
        }
        let h = self.back.entry(dev).or_default();
        if st.nav.contains(&Nav::Back) && contextual {
            h.consumed = true;
        }
        if h.consumed {
            return false;
        }
        h.elapsed += dt.clamp(0., 0.05);
        if h.elapsed + f32::EPSILON * 8. >= secs {
            h.consumed = true;
            return true;
        }
        false
    }
    /// Nobody points at anything (leaving a select screen).
    pub fn reset(&mut self) {
        self.map.clear();
        self.back.clear();
        self.ring.clear();
        self.mouse = None;
    }
    /// `dev`'s pointer, made on cursor `c`'s target (else mid-screen) the first time; `fan`: its place among
    /// pointers resting on one target.
    pub fn of(&mut self, dev: Dev, fan: usize, c: &Cursor, spots: &[Spot]) -> &mut Pointer {
        let at = spots.iter().find(|s| s.f.key == c.key).map_or(self.screen / 2., |s| s.center);
        let p = self.map.entry(dev).or_insert_with(|| Pointer::new(at, fan));
        p.fan = fan;
        p
    }
}

/// A tile grid for digital hops.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub tile: u32,
    pub n: usize,
    pub cols: usize,
}

/// The target nearest `pos` (by distance to its box, then to its centre).
pub fn nearest(pos: Vec2, spots: &[Spot]) -> Option<&Spot> {
    spots.iter().min_by(|a, b| {
        (a.dist(pos), a.center.distance(pos)).partial_cmp(&(b.dist(pos), b.center.distance(pos))).unwrap()
    })
}

/// The target under `pos`, or the nearest within `reach` of its edge.
pub fn under(pos: Vec2, spots: &[Spot], reach: f32) -> Option<&Spot> {
    nearest(pos, spots).filter(|s| s.dist(pos) <= reach)
}

impl Pointer {
    pub fn new(pos: Vec2, fan: usize) -> Self {
        Pointer { pos, fan, rest: true, ..default() }
    }
    pub fn bump(&mut self, amp: f32) {
        self.bounce = (0., amp);
    }
    /// Put it where the mouse is.
    pub fn point(&mut self, at: Vec2) {
        (self.pos, self.drive, self.vel, self.slide, self.rest) = (at, Drive::Mouse, Vec2::ZERO, None, true);
    }
    /// Let go at `at`, as if a stick just stopped there: the magnet pulls it onto the nearest target.
    pub fn release(&mut self, at: Vec2) {
        (self.pos, self.drive, self.vel, self.slide, self.rest) = (at, Drive::Analog, Vec2::ZERO, None, false);
    }
    /// Slide to the cursor's target (which just changed).
    pub fn hop(&mut self) {
        self.drive = Drive::Digital;
        self.slide = Some((self.pos, 0.));
        self.vel = Vec2::ZERO;
        self.rest = false;
    }
    /// Horizontal and vertical scale of the squash and bounce.
    pub fn squash(&self, cfg: &PointerLook) -> Vec2 {
        let (t, a) = self.bounce;
        let s = a * (1. - t / cfg.bounce_secs.max(0.01)).max(0.).powi(2) * (t * cfg.bounce_wobble).cos();
        Vec2::new(1. + s, 1. - s)
    }
    /// Where the tip rests on a target: a little above its centre, fanned out per player (closer on small targets).
    pub fn snap(&self, s: &Spot, cfg: &PointerLook) -> Vec2 {
        // A stair up to the right, so every chip (down-right of its tip) shows at least its number.
        // The whole fan (x from -1 to 2 steps, y from -1.5 to 0.5) fits inside; it slides down rather than squash.
        let (i, k) = (self.fan % 8, (self.fan % 4) as f32);
        let lim = s.size * 0.42;
        let f = cfg.fan.min(s.size.x * 0.2).min(lim.y);
        let off = Vec2::new(k - 1., -0.5 * k + if i >= 4 { 0.5 } else { 0. }) * f;
        let y = (s.center.y - s.size.y * cfg.rest).max(s.center.y - lim.y + 1.5 * f).min(s.center.y + lim.y - 0.5 * f);
        Vec2::new(s.center.x, y) + off
    }
    /// Steps the motion springs by `dt` real seconds, in slices short enough that any frame rate moves alike.
    pub fn animate(&mut self, m: &PointerMotion, dt: f32) {
        let a = &mut self.anim;
        let dt = dt.min(0.1);
        if dt <= 0. {
            return;
        }
        let from = a.last.unwrap_or_else(|| {
            a.chip = (self.pos, Vec2::ZERO);
            self.pos
        });
        a.last = Some(self.pos);
        let vel = (self.pos - from) / dt;
        let speed = vel.length();
        if speed > m.still_speed {
            (a.dir, a.still) = (vel / speed, 0.);
        } else {
            a.still += dt;
        }
        a.clock += dt;
        let lean = (vel.x * m.lean).clamp(-m.lean_max, m.lean_max);
        let stretch = (speed * m.stretch).min(m.stretch_max);
        let n = (dt * m.substeps.max(1.)).ceil();
        let h = dt / n;
        for i in 1..=n as usize {
            spring(&mut a.lean.0, &mut a.lean.1, lean, m.lean_spring, h);
            spring(&mut a.stretch.0, &mut a.stretch.1, stretch, m.stretch_spring, h);
            spring(&mut a.chip.0, &mut a.chip.1, from.lerp(self.pos, i as f32 / n), m.chip_spring, h);
        }
        // A jump (the mouse, a moved layout) never flings the chip far.
        let off = a.chip.0 - self.pos;
        if off.length() > m.chip_lag * 4. {
            a.chip.0 = self.pos + off.normalize() * m.chip_lag * 4.;
        }
    }
    pub fn pose(&self, cfg: &PointerLook) -> Pose {
        let (a, m) = (&self.anim, &cfg.motion);
        // Stretch along the last travel, thinner across it.
        let s = a.stretch.0;
        let along = Vec2::ONE + s * (a.dir * a.dir * 1.5 - Vec2::splat(0.5));
        let fade = (a.still - m.breath_after).clamp(0., 1.);
        let w = (a.clock * std::f32::consts::TAU / m.breath_secs.max(0.1) + self.fan as f32 * 2.4).sin() * fade;
        let off = a.chip.0 - self.pos;
        let len = off.length();
        let chip = if len > 0.01 { off / len * m.chip_lag * (len / m.chip_lag.max(0.1)).tanh() } else { Vec2::ZERO };
        Pose {
            lean: a.lean.0,
            scale: self.squash(cfg) * along * (1. + w * m.breath_scale),
            bob: w * m.breath_bob,
            chip,
            swing: -chip.x * m.chip_swing,
        }
    }
}

/// The mouse drives the keyboard player's pointer (`playing`: devices in the game): motion moves it and a click
/// confirms on what is under it, or joins as the keyboard player when it is over something. Returns the keyboard
/// device when the mouse moved its pointer.
pub fn mouse(
    devs: &mut Devices,
    ptrs: &mut Pointers,
    spots: &[Spot],
    m: &Mouse,
    playing: impl Fn(Dev) -> bool,
) -> Option<Dev> {
    let pos = m.pos();
    let moved = pos.is_some_and(|p| ptrs.mouse.is_some_and(|q| q.distance(p) > 0.5));
    ptrs.mouse = pos.or(ptrs.mouse);
    let dev = keyboard(devs, &playing)?;
    let joined = playing(dev);
    if joined
        && (moved || m.clicked())
        && let (Some(at), Some(p)) = (pos, ptrs.map.get_mut(&dev))
    {
        p.point(at);
    }
    if m.clicked()
        && (joined || pos.is_some_and(|at| under(at, spots, 0.).is_some()))
        && let Some(st) = devs.map.get_mut(&dev)
    {
        st.nav.push(Nav::Confirm);
    }
    (joined && moved).then_some(dev)
}

/// One frame of a pointer and its cursor: mouse and stick motion, then `navs` (digital moves hop; Left / Right on a
/// spinner spin it), then the slide. `stick` is analog, y up; while it is tilted, the direction presses it made are
/// ignored. `spots` are the targets this pointer may use (none: it stays put). Confirm activates the target under
/// the pointer (`NONE` over nothing); the caller decides what that means.
pub fn steer(
    p: &mut Pointer,
    c: &mut Cursor,
    navs: &[Nav],
    stick: Vec2,
    spots: &[Spot],
    grid: Option<Grid>,
    cfg: &PointerLook,
    dt: f32,
    screen: Vec2,
) -> Vec<Action> {
    let mut out = vec![];
    p.bounce.0 += dt;
    let tilt = ((stick.length() - cfg.deadzone) / (1. - cfg.deadzone).max(0.01)).clamp(0., 1.);
    let analog = tilt > 0.;
    if analog && !spots.is_empty() {
        p.drive = Drive::Analog;
        p.slide = None;
        p.rest = false;
    }
    // A digital pointer whose target went away (a card removed) settles on whatever is nearest.
    if p.drive == Drive::Digital && !spots.is_empty() && !spots.iter().any(|s| s.f.key == c.key) {
        p.drive = Drive::Analog;
        p.slide = None;
    }
    match p.drive {
        _ if spots.is_empty() => {}
        Drive::Analog => {
            let dir = Vec2::new(stick.x, -stick.y).normalize_or_zero();
            let want = dir * tilt.powf(cfg.curve) * cfg.speed;
            p.vel += (want - p.vel) * (1. - (-cfg.accel * dt).exp());
            p.pos = (p.pos + p.vel * dt).clamp(Vec2::ZERO, screen.max(Vec2::ONE));
            // The target nearest to where it would coast to.
            if let Some(t) = nearest(p.pos + p.vel / cfg.accel.max(0.1), spots) {
                let to = p.snap(t, cfg);
                let mut pull = to - p.pos;
                let engage = if analog {
                    // Only sideways while steering, so it never fights the stick.
                    pull -= dir * pull.dot(dir);
                    let slow = (1. - p.vel.length() / (cfg.speed * cfg.magnet_speed).max(1.)).clamp(0., 1.);
                    if to.distance(p.pos) <= cfg.magnet_radius { slow } else { 0. }
                } else {
                    1.
                };
                p.pos += pull * (1. - (-cfg.magnet_strength * engage * dt).exp());
                if !analog && !p.rest && p.pos.distance(to) < 1.5 {
                    p.rest = true;
                    p.vel = Vec2::ZERO;
                    p.bump(cfg.land);
                }
            }
            c.key = under(p.pos, spots, cfg.reach).map_or(NONE, |s| s.f.key);
        }
        Drive::Mouse => c.key = under(p.pos, spots, 0.).map_or(NONE, |s| s.f.key),
        Drive::Digital => {}
    }
    for &nav in navs {
        match nav {
            Nav::Up | Nav::Down | Nav::Left | Nav::Right if analog || spots.is_empty() => {}
            Nav::Up | Nav::Down | Nav::Left | Nav::Right => {
                if !spots.iter().any(|s| s.f.key == c.key)
                    && let Some(s) = nearest(p.pos, spots)
                {
                    c.key = s.f.key;
                }
                let before = c.key;
                let a = match grid {
                    Some(g) => grid::drive(c, nav, spots, g.tile, g.n, g.cols),
                    None => c.drive(nav, spots),
                };
                out.extend(a);
                if c.key != before || p.drive != Drive::Digital {
                    p.hop();
                }
            }
            Nav::Confirm => out.push(Action::Activate(c.key)),
            Nav::Back => out.push(Action::Back),
            Nav::Start => out.push(Action::Start),
        }
    }
    if p.drive == Drive::Digital
        && let Some(s) = spots.iter().find(|s| s.f.key == c.key)
    {
        let to = p.snap(s, cfg);
        p.pos = match p.slide {
            Some((from, t)) => {
                let t = t + dt;
                let k = t / cfg.slide_secs.max(0.01);
                p.slide = (k < 1.).then_some((from, t));
                if k >= 1. {
                    p.rest = true;
                    p.bump(cfg.land);
                }
                from.lerp(to, ease_out_back(k, cfg.slide_overshoot))
            }
            // Resting: follow the target if the layout moves.
            None => to,
        };
    }
    out
}

/// One frame of a player's pointer and cursor from their device `st` ([`steer`]). Returns their actions and the
/// cursor's new target when it moved onto another.
pub fn drive(
    p: &mut Pointer,
    c: &mut Cursor,
    st: &DevState,
    spots: &[Spot],
    grid: Option<Grid>,
    cfg: &PointerLook,
    dt: f32,
    screen: Vec2,
) -> (Vec<Action>, Option<u32>) {
    let before = c.key;
    let acts = steer(p, c, &st.nav, st.stick, spots, grid, cfg, dt, screen);
    (acts, (c.key != before).then_some(c.key))
}

#[derive(Component)]
pub struct PointerLayer;

#[derive(Component)]
pub struct PointerNode(Dev, Look);

/// A device's back-hold ring, drawn at its centre (zero-sized, so scaling and leaning it turn the whole gauge).
#[derive(Component)]
pub struct BackRingNode(Dev);

/// One dash of a back-hold ring, lit as the hold fills.
#[derive(Component)]
pub struct BackDash(Dev, u32);

/// The pivot carrying a back-hold ring's spark round the filled end.
#[derive(Component)]
pub struct BackHead(Dev);

/// The chip riding on a pointer's arrow; pivots where it hangs.
#[derive(Component)]
pub struct PointerChip(Dev);

/// Keeps one arrow per pointer, above all select UI: rebuilt when its look changes, animated every frame. A device
/// holding Back wears a ring around its arrow that fills as the hold runs, in the player's colour.
pub fn draw(
    mut commands: Commands,
    mut ptrs: ResMut<Pointers>,
    layer: Single<Entity, With<PointerLayer>>,
    mut nodes: Query<
        (Entity, &PointerNode, &mut Node, &mut UiTransform),
        (Without<PointerChip>, Without<BackRingNode>, Without<BackHead>, Without<BackDash>),
    >,
    mut chips: Query<(&PointerChip, &mut UiTransform), (Without<PointerNode>, Without<BackHead>)>,
    mut rings: Query<
        (Entity, &BackRingNode, &mut Node, &mut UiTransform),
        (Without<PointerNode>, Without<BackHead>, Without<PointerChip>),
    >,
    mut heads: Query<(&BackHead, &mut UiTransform), Without<PointerNode>>,
    mut dashes: Query<(&BackDash, &mut BackgroundColor)>,
    f: Res<Fonts>,
    data: Res<crate::Data>,
    time: Res<Time<Real>>,
) {
    let look = &data.0.global.render.select;
    let cfg = &look.pointer;
    let ring = &look.back_ring;
    let dt = time.delta_secs();
    for p in ptrs.map.values_mut() {
        p.animate(&cfg.motion, dt);
    }
    step_rings(&mut ptrs, look, dt);
    let poses: BTreeMap<Dev, Pose> = ptrs.map.iter().map(|(d, p)| (*d, p.pose(cfg))).collect();
    let mut have = vec![];
    for (e, pn, mut n, mut tf) in &mut nodes {
        match ptrs.map.get(&pn.0) {
            Some(p) if p.look == pn.1 => {
                have.push(pn.0);
                let pose = poses[&pn.0];
                n.left = Val::Px(p.pos.x);
                n.top = Val::Px(p.pos.y + pose.bob);
                tf.set_if_neq(UiTransform { scale: pose.scale, rotation: Rot2::radians(pose.lean), ..default() });
            }
            _ => commands.entity(e).despawn(),
        }
    }
    for (pc, mut tf) in &mut chips {
        if let Some(pose) = poses.get(&pc.0) {
            tf.set_if_neq(UiTransform {
                translation: Val2::px(pose.chip.x, pose.chip.y),
                rotation: Rot2::radians(pose.swing),
                ..default()
            });
        }
    }
    for (dev, p) in &ptrs.map {
        if have.contains(dev) {
            continue;
        }
        commands.entity(*layer).with_children(|c| {
            c.spawn((
                node(|n| {
                    n.position_type = PositionType::Absolute;
                    n.left = Val::Px(p.pos.x);
                    n.top = Val::Px(p.pos.y);
                }),
                UiTransform::from_scale(poses[dev].scale),
                // Later players draw on top; keep P1 above the rest.
                ZIndex(-(p.fan as i32)),
                PointerNode(*dev, p.look.clone()),
            ))
            .with_children(|c| arrow(c, &f, *dev, &p.look, cfg));
        });
    }
    // The gauge draws over every arrow: the chip on a pointer's tail overlaps it, and the gauge must win.
    let mut showing = vec![];
    for (e, b, mut n, mut tf) in &mut rings {
        let Some(r) = ptrs.ring.get(&b.0) else {
            commands.entity(e).despawn();
            continue;
        };
        showing.push(b.0);
        n.left = Val::Px(r.at.x);
        n.top = Val::Px(r.at.y);
        tf.set_if_neq(ring_form(r, ring));
    }
    for (h, mut tf) in &mut heads {
        if let Some(r) = ptrs.ring.get(&h.0) {
            tf.set_if_neq(UiTransform::from_rotation(Rot2::radians(r.shown() * std::f32::consts::TAU)));
        }
    }
    for (d, mut bg) in &mut dashes {
        let Some(r) = ptrs.ring.get(&d.0) else { continue };
        let c = dash_color(r, d.1, ring);
        if bg.0 != c {
            bg.0 = c;
        }
    }
    for (dev, r) in &ptrs.ring {
        if showing.contains(dev) {
            continue;
        }
        commands.entity(*layer).with_children(|p| back_ring(p, *dev, r, ring));
    }
}

/// Steps every ring toward its device's hold progress, dropping the ones that finished draining.
fn step_rings(ptrs: &mut Pointers, look: &SelectLook, dt: f32) {
    let ring = &look.back_ring;
    let mut want: Vec<(Dev, f32)> =
        ptrs.back.iter().map(|(dev, h)| (*dev, (h.elapsed / look.back_hold_secs.max(1e-3)).clamp(0., 1.))).collect();
    // Released holds keep their ring until it has drained.
    for dev in ptrs.ring.keys() {
        if !want.iter().any(|(d, _)| d == dev) {
            want.push((*dev, 0.));
        }
    }
    for (dev, target) in want {
        // Only a hold that has started wears one; a ring already draining steps on to nothing.
        if target <= 0. && !ptrs.ring.contains_key(&dev) {
            continue;
        }
        // The pointer carries it, over its arrow; a device without one (nobody has joined) fills one mid-screen.
        let (at, color) = match ptrs.map.get(&dev) {
            Some(p) => {
                let off = Vec2::new(ring.offset[0], ring.offset[1]) * look.pointer.scale;
                (p.pos + off, p.look.color)
            }
            None => (ptrs.screen / 2., GOLD),
        };
        let r = ptrs.ring.entry(dev).or_insert_with(|| Ring::start(ring));
        (r.at, r.color) = (at, color);
        r.step(target, ring, dt);
    }
    ptrs.ring.retain(|_, r| !r.dead());
}

/// A player's gauge: the dashes that fill, and the spark riding the filled end.
fn back_ring(p: &mut ChildSpawnerCommands, dev: Dev, ring: &Ring, look: &BackRing) {
    let (r, w) = (look.radius, look.thickness);
    let spark = look.spark * w;
    // Over every arrow (they stack from 0 down per player) and rimmed, so it reads on any tile.
    p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.left = Val::Px(ring.at.x);
            n.top = Val::Px(ring.at.y);
        }),
        ring_form(ring, look),
        ZIndex(5),
        BackRingNode(dev),
    ))
    .with_children(|p| {
        let len = dash_len(look);
        for i in 0..look.ticks {
            let (at, angle) = dash(i, look.ticks, r);
            p.spawn((
                node(|n| {
                    n.position_type = PositionType::Absolute;
                    n.left = Val::Px(at.x - len / 2.);
                    n.top = Val::Px(at.y - w / 2.);
                    n.width = Val::Px(len);
                    n.height = Val::Px(w);
                    n.border_radius = BorderRadius::all(Val::Px(w / 2.));
                }),
                BackgroundColor(ring.color.with_alpha(look.track)),
                BoxShadow::new(NIGHT.with_alpha(0.85), Val::Px(0.), Val::Px(0.), Val::Px(1.), Val::Px(look.rim)),
                UiTransform::from_rotation(Rot2::radians(angle)),
                BackDash(dev, i),
            ));
        }
        // One pivot for the spark and its two tails: rotating it sweeps them round the filled end.
        p.spawn((node(|n| n.position_type = PositionType::Absolute), BackHead(dev))).with_children(|p| {
            for (lag, size, alpha) in [(0., 1., 1.), (0.03, 0.6, 0.5), (0.06, 0.36, 0.26)] {
                let d = spark * size;
                let a = -std::f32::consts::FRAC_PI_2 - lag * std::f32::consts::TAU;
                let (x, y) = (r * a.cos(), r * a.sin());
                p.spawn((
                    node(|n| {
                        n.position_type = PositionType::Absolute;
                        n.left = Val::Px(x - d / 2.);
                        n.top = Val::Px(y - d / 2.);
                        n.width = Val::Px(d);
                        n.height = Val::Px(d);
                        n.border_radius = BorderRadius::MAX;
                    }),
                    BackgroundColor(Color::WHITE.with_alpha(alpha)),
                    BoxShadow::new(
                        ring.color.with_alpha(0.7),
                        Val::Px(0.),
                        Val::Px(0.),
                        Val::Px(d * 0.5),
                        Val::Px(d * 0.7),
                    ),
                ));
            }
        });
    });
}

/// A dash's length: its even share of the circle, shortened by `dash`.
fn dash_len(look: &BackRing) -> f32 {
    std::f32::consts::TAU * look.radius / look.ticks.max(1) as f32 * look.dash
}

/// Dash `i` of `n` around a ring of radius `r`: where its middle sits, and the angle its long axis lies along (a chord
/// of the circle, from 12 o'clock clockwise).
fn dash(i: u32, n: u32, r: f32) -> (Vec2, f32) {
    let a = i as f32 / n.max(1) as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    (Vec2::new(r * a.cos(), r * a.sin()), a + std::f32::consts::FRAC_PI_2)
}

/// How a ring's nodes are drawn this frame: its pop and wobble about their shared centre.
fn ring_form(r: &Ring, look: &BackRing) -> UiTransform {
    UiTransform { scale: Vec2::splat(r.scale(look)), rotation: Rot2::radians(r.lean(look)), ..default() }
}

/// How lit dash `i` is when the ring has filled `shown` of the way: `(filled, heat)`, the heat whitening the dashes
/// just behind the leading edge like a comet head.
fn lit(i: u32, n: u32, shown: f32, hot: f32) -> (f32, f32) {
    let d = shown * n.max(1) as f32 - i as f32;
    let filled = d.clamp(0., 1.);
    (filled, filled * (1. - d / hot.max(0.01)).clamp(0., 1.))
}

/// A dash's colour this frame: the player's colour, whitened at the leading edge and fading in from the track.
fn dash_color(r: &Ring, i: u32, look: &BackRing) -> Color {
    let (filled, heat) = lit(i, look.ticks, r.shown(), look.hot);
    r.color.mix(&Color::WHITE, heat * 0.55).with_alpha(look.track + (1. - look.track) * filled)
}

/// A rounded stroke from `a` to `b`, `w` thick.
fn stroke(p: &mut ChildSpawnerCommands, a: Vec2, b: Vec2, w: f32, color: Color, glow: Option<Color>) {
    let d = b - a;
    let (len, mid) = (d.length() + w, (a + b) / 2.);
    let mut e = p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.left = Val::Px(mid.x - len / 2.);
            n.top = Val::Px(mid.y - w / 2.);
            n.width = Val::Px(len);
            n.height = Val::Px(w);
            n.border_radius = BorderRadius::all(Val::Px(w / 2.));
        }),
        BackgroundColor(color),
        UiTransform::from_rotation(Rot2::radians(d.y.atan2(d.x))),
    ));
    if let Some(g) = glow {
        e.insert(BoxShadow::new(g, Val::Px(0.), Val::Px(0.), Val::Px(1.), Val::Px(w * 2.2)));
    }
}

fn arrow(p: &mut ChildSpawnerCommands, f: &Fonts, dev: Dev, look: &Look, cfg: &PointerLook) {
    let (k, w) = (cfg.scale, cfg.stroke * cfg.scale);
    // The tip is at the origin: a shaft back to the lower right, two barbs (down a little italic slant, and right).
    let tail = Vec2::new(17., 25.) * k;
    let strokes = [(Vec2::ZERO, tail), (Vec2::ZERO, Vec2::new(1.5, 18.) * k), (Vec2::ZERO, Vec2::new(16., 5.) * k)];
    for (a, b) in strokes {
        stroke(p, a, b, w + 4., NIGHT.with_alpha(0.92), None);
    }
    for (a, b) in strokes {
        stroke(p, a, b, w, look.color, Some(look.color.with_alpha(0.75)));
    }
    // A bright hotspot at the tip.
    p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.left = Val::Px(-w * 0.3);
            n.top = Val::Px(-w * 0.3);
            n.width = Val::Px(w * 0.6);
            n.height = Val::Px(w * 0.6);
            n.border_radius = BorderRadius::MAX;
        }),
        BackgroundColor(Color::WHITE.with_alpha(0.9)),
    ));
    // Not carrying its own chip, a small number says whose arrow it is.
    if !look.own {
        let d = 14. * k;
        let at = tail + Vec2::new(-10., 3.) * k;
        p.spawn((
            node(|n| {
                n.position_type = PositionType::Absolute;
                n.left = Val::Px(at.x - d / 2.);
                n.top = Val::Px(at.y - d / 2.);
                n.width = Val::Px(d);
                n.height = Val::Px(d);
                n.border = UiRect::all(Val::Px(1.5));
                n.border_radius = BorderRadius::MAX;
                n.justify_content = JustifyContent::Center;
                n.align_items = AlignItems::Center;
            }),
            BackgroundColor(NIGHT.with_alpha(0.9)),
            BorderColor::all(look.color),
        ))
        .with_child(txt(&f.display, 10. * k, look.color, &look.num));
    }
    let Some((label, color)) = &look.chip else { return };
    // Hangs from the tail (the far end of the shaft).
    let at = tail + Vec2::new(-2., -1.) * k;
    let size = 16. * k;
    p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.left = Val::Px(at.x);
            n.top = Val::Px(at.y);
        }),
        PointerChip(dev),
    ))
    .with_children(|p| {
        p.spawn((
            node(|n| {
                n.position_type = PositionType::Absolute;
                n.padding = UiRect::axes(Val::Px(size * 0.32), Val::Px(0.));
                n.border = UiRect::all(Val::Px(2.));
                n.border_radius = BorderRadius::all(Val::Px(5.));
            }),
            BackgroundColor(*color),
            BorderColor::all(NIGHT),
            BoxShadow::new(color.with_alpha(0.5), Val::Px(0.), Val::Px(0.), Val::Px(0.), Val::Px(10.)),
            UiTransform::from_rotation(Rot2::degrees(-8.)),
        ))
        .with_child(txt(&f.display, size, NIGHT, label));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> PointerLook {
        let data = crate::test_data();
        data.global.render.select.pointer.clone()
    }
    fn spot(key: u32, x: f32, y: f32) -> Spot {
        Spot { f: Focusable { key, owner: None, spin: false }, center: Vec2::new(x, y), size: Vec2::splat(60.) }
    }
    const SCREEN: Vec2 = Vec2::new(1280., 720.);

    fn run(p: &mut Pointer, c: &mut Cursor, stick: Vec2, spots: &[Spot], frames: usize) {
        for _ in 0..frames {
            steer(p, c, &[], stick, spots, None, &cfg(), 1. / 60., SCREEN);
        }
    }

    #[test]
    fn a_released_stick_eases_onto_the_nearest_target() {
        let spots = [spot(1, 100., 100.), spot(2, 300., 100.)];
        let mut p = Pointer::new(Vec2::new(100., 100.), 1);
        let mut c = Cursor { key: 1, owner: None };
        run(&mut p, &mut c, Vec2::X, &spots, 8);
        assert_eq!(p.drive, Drive::Analog);
        assert!(p.pos.x > 150. && p.pos.x < 250., "glides between the targets: {}", p.pos);
        run(&mut p, &mut c, Vec2::ZERO, &spots, 90);
        let to = p.snap(&spots[1], &cfg());
        assert!(p.pos.distance(to) < 1.5, "settled on the target ahead: {}", p.pos);
        assert_eq!(c.key, 2);
        assert!(p.bounce.1 > 0., "landing bounces");
    }

    #[test]
    fn the_magnet_never_fights_the_stick() {
        let spots = [spot(1, 100., 100.), spot(2, 400., 100.)];
        let mut p = Pointer::new(Vec2::new(100., 100.), 0);
        let mut c = Cursor { key: 1, owner: None };
        // A gentle push leaves the target anyway.
        run(&mut p, &mut c, Vec2::new(0.4, 0.), &spots, 120);
        assert!(p.pos.x > 160., "{}", p.pos);
        // In a gap nothing is hovered.
        let mut p = Pointer::new(Vec2::new(250., 100.), 0);
        p.drive = Drive::Mouse;
        run(&mut p, &mut c, Vec2::ZERO, &spots, 1);
        assert_eq!((c.key, p.pos), (NONE, Vec2::new(250., 100.)), "the mouse has no magnet");
    }

    #[test]
    fn digital_presses_hop_and_slide_with_the_layout() {
        let cfg = cfg();
        let mut spots = [spot(1, 100., 100.), spot(2, 300., 100.), spot(3, 100., 300.)];
        let mut p = Pointer::new(Vec2::new(100., 100.), 0);
        let mut c = Cursor { key: 1, owner: None };
        steer(&mut p, &mut c, &[Nav::Right], Vec2::ZERO, &spots, None, &cfg, 1. / 60., SCREEN);
        assert_eq!(c.key, 2, "the cursor hops at once");
        assert!(p.pos.x < 200., "the pointer slides: {}", p.pos);
        run(&mut p, &mut c, Vec2::ZERO, &spots, 30);
        assert_eq!(p.pos, p.snap(&spots[1], &cfg));
        spots[1].center.y = 140.;
        run(&mut p, &mut c, Vec2::ZERO, &spots, 1);
        assert_eq!(p.pos, p.snap(&spots[1], &cfg), "follows its target");
        // Over nothing (the mouse left it there), a press hops from the nearest target.
        p.drive = Drive::Mouse;
        p.pos = Vec2::new(90., 220.);
        steer(&mut p, &mut c, &[Nav::Up], Vec2::ZERO, &spots, None, &cfg, 1. / 60., SCREEN);
        assert_eq!(c.key, 1);
        // Its target gone, a digital pointer settles on the nearest.
        let rest = [spots[0], spots[2]];
        c.key = 2;
        run(&mut p, &mut c, Vec2::ZERO, &rest, 60);
        assert_eq!(c.key, 1);
    }

    #[test]
    fn stick_presses_are_not_hops_and_confirm_targets_what_is_under() {
        let spots = [spot(1, 100., 100.), spot(2, 300., 100.)];
        let mut p = Pointer::new(Vec2::new(100., 100.), 0);
        let mut c = Cursor { key: 1, owner: None };
        let a = steer(&mut p, &mut c, &[Nav::Right, Nav::Confirm], Vec2::X, &spots, None, &cfg(), 1. / 60., SCREEN);
        assert_eq!(a, [Action::Activate(1)]);
        assert_eq!(c.key, 1);
    }

    #[test]
    fn pointers_on_one_target_fan_out() {
        let cfg = cfg();
        for (w, h) in [(60., 60.), (100., 100.), (200., 240.)] {
            let s = Spot { size: Vec2::new(w, h), ..spot(1, 100., 100.) };
            let at: Vec<Vec2> = (0..8).map(|i| Pointer::new(Vec2::ZERO, i).snap(&s, &cfg)).collect();
            let gap = cfg.fan.min(w * 0.2) * 0.45;
            for a in 0..8 {
                assert_eq!(s.dist(at[a]), 0., "{w}x{h} P{} {at:?}", a + 1);
                for b in a + 1..8 {
                    assert!(at[a].distance(at[b]) >= gap, "{w}x{h}: P{} and P{} {at:?}", a + 1, b + 1);
                }
            }
        }
    }

    #[test]
    fn the_arrow_carries_exactly_the_chip_in_hand() {
        let (red, green) = (Color::srgb(1., 0., 0.), Color::srgb(0., 1., 0.));
        let held = Look::new(red, 0, true, None);
        assert_eq!((held.chip, held.own), (Some(("P1".into(), red)), true));
        let empty = Look::new(red, 1, false, None);
        assert_eq!((empty.chip, empty.own, empty.num.as_str()), (None, false, "2"), "dropped: just the number");
        let cpu = Look::new(red, 0, false, Some(("CPU".into(), green)));
        assert_eq!((cpu.chip, cpu.own), (Some(("CPU".into(), green)), false), "a CPU chip rides where its own would");
        assert_eq!(Look::new(red, 0, true, Some(("CPU".into(), green))).chip.unwrap().0, "CPU");
    }

    /// Glides right for half a second, then stops, animated at `fps`; returns the pose mid-glide and once settled.
    fn glide(fps: f32) -> (Pose, Pose) {
        let cfg = cfg();
        let mut p = Pointer::new(Vec2::new(100., 300.), 1);
        let dt = 1. / fps;
        let mut mid = None;
        let mut t = 0.;
        while t < 3. {
            t += dt;
            p.pos.x = 100. + 800. * t.min(0.5);
            p.animate(&cfg.motion, dt);
            if mid.is_none() && t >= 0.4 {
                mid = Some(p.pose(&cfg));
            }
        }
        (mid.unwrap(), p.pose(&cfg))
    }

    #[test]
    fn motion_springs_lean_stretch_and_settle_alike_at_any_frame_rate() {
        let m = cfg().motion;
        let (slow, fast) = (glide(30.), glide(144.));
        assert!(slow.0.lean > 0.1 && slow.0.scale.x > 1.05 && slow.0.chip.x < -m.chip_lag * 0.5, "{:?}", slow.0);
        let near = |a: Pose, b: Pose, tol: f32| {
            (a.lean - b.lean).abs() < tol
                && a.scale.distance(b.scale) < tol
                && a.chip.distance(b.chip) < tol * m.chip_lag * 4.
                && (a.bob - b.bob).abs() < tol * m.breath_bob.max(1.) * 4.
        };
        assert!(near(slow.0, fast.0, 0.04), "mid-glide {:?} vs {:?}", slow.0, fast.0);
        assert!(near(slow.1, fast.1, 0.04), "settled {:?} vs {:?}", slow.1, fast.1);
        // Settled: upright, unstretched, the chip home; only the breath remains.
        let s = slow.1;
        assert!(s.lean.abs() < 1e-3 && s.chip.length() < 0.05, "{s:?}");
        assert!((s.scale - Vec2::ONE).abs().max_element() <= m.breath_scale + 1e-3, "{s:?}");
    }
}

#[cfg(test)]
mod back_tests {
    use super::*;
    #[test]
    fn threshold_release_consumption_and_independent_devices() {
        let mut p = Pointers::default();
        let mut st = DevState::default();
        st.held[6] = true;
        for _ in 0..29 {
            assert!(!p.hold_back(Dev::Bot(0), &st, false, 0.05, 1.5));
        }
        assert!(!p.hold_back(Dev::Bot(1), &st, false, 0.05, 1.5));
        assert!(p.hold_back(Dev::Bot(0), &st, false, 0.05, 1.5));
        st.held[6] = false;
        p.hold_back(Dev::Bot(0), &st, false, 0.05, 1.5);
        st.held[6] = true;
        st.nav.push(Nav::Back);
        assert!(!p.hold_back(Dev::Bot(0), &st, true, 0.05, 1.5));
        st.nav.clear();
        for _ in 0..40 {
            assert!(!p.hold_back(Dev::Bot(0), &st, false, 0.05, 1.5));
        }
    }

    fn ring() -> BackRing {
        crate::test_data().global.render.select.back_ring.clone()
    }

    /// A ring filling `target` for `frames`, then released.
    fn run(look: &BackRing, frames: usize, target: f32) -> Ring {
        let mut r = Ring::start(look);
        for _ in 0..frames {
            r.step(target, look, 1. / 60.);
        }
        r
    }

    #[test]
    fn the_ring_pops_in_with_a_hold_and_drains_away_once_released() {
        let look = ring();
        let mut r = Ring::start(&look);
        assert_eq!(r.scale(&look), look.pop, "a hold starts at the pop scale");
        r.step(0.5, &look, 1. / 60.);
        assert!(r.pop.0 > look.pop && r.pop.0 < 1., "grows in from the pop scale: {}", r.pop.0);
        r = run(&look, 60, 0.5);
        assert!((r.shown() - 0.5).abs() < 0.1, "chases the hold: {}", r.shown());
        assert!((r.scale(&look) - 1.).abs() < 0.1, "settles at full size: {}", r.scale(&look));
        let released = run(&look, 90, 0.);
        assert!(released.dead(), "drains and shrinks away");
        // Partway through the drain it is still drawn, and less full than it was.
        let mut draining = r;
        for _ in 0..3 {
            draining.step(0., &look, 1. / 60.);
        }
        assert!(draining.shown() > 0. && draining.shown() < 0.5 && !draining.dead(), "{}", draining.shown());
    }

    #[test]
    fn only_a_started_hold_wears_a_ring_and_a_released_one_drains_away() {
        let d = crate::test_data();
        let look = &d.global.render.select;
        let mut p = Pointers { screen: Vec2::new(1280., 720.), ..default() };
        let mut st = DevState::default();
        // Not holding (the screen still asks every frame): nothing to show.
        assert!(!p.hold_back(Dev::Bot(0), &st, false, 0.05, look.back_hold_secs));
        step_rings(&mut p, look, 1. / 60.);
        assert!(p.ring.is_empty(), "no ring before a hold");
        // A contextual Back that got used up is not leaving either.
        st.held[6] = true;
        st.nav.push(Nav::Back);
        assert!(!p.hold_back(Dev::Bot(0), &st, true, 0.05, look.back_hold_secs));
        st.nav.clear();
        step_rings(&mut p, look, 1. / 60.);
        assert!(p.ring.is_empty(), "a consumed Back pops nothing in");
        // A real hold fills one, and letting go drains it away.
        st.held[6] = false;
        p.hold_back(Dev::Bot(0), &st, false, 0.05, look.back_hold_secs);
        st.held[6] = true;
        for _ in 0..12 {
            p.hold_back(Dev::Bot(0), &st, false, 0.05, look.back_hold_secs);
            step_rings(&mut p, look, 1. / 60.);
        }
        let shown = p.ring.get(&Dev::Bot(0)).map(|r| r.shown()).expect("a ring while held");
        assert!(shown > 0.2, "fills as the hold runs: {shown}");
        st.held[6] = false;
        p.hold_back(Dev::Bot(0), &st, false, 0.05, look.back_hold_secs);
        for _ in 0..60 {
            step_rings(&mut p, look, 1. / 60.);
        }
        assert!(p.ring.is_empty(), "a released ring drains away");
    }

    #[test]
    fn any_frame_rate_fills_the_ring_alike() {
        let look = ring();
        let mut slow = Ring::start(&look);
        let mut fast = Ring::start(&look);
        for _ in 0..30 {
            slow.step(0.7, &look, 1. / 30.);
        }
        for _ in 0..120 {
            fast.step(0.7, &look, 1. / 120.);
        }
        assert!((slow.shown() - fast.shown()).abs() < 0.02, "{} vs {}", slow.shown(), fast.shown());
        assert!((slow.scale(&look) - fast.scale(&look)).abs() < 0.02);
    }

    #[test]
    fn dashes_sit_on_the_ring_from_the_top_round_clockwise_and_light_from_the_fill() {
        let (n, r) = (24, 40.);
        let (top, angle) = dash(0, n, r);
        assert!(top.distance(Vec2::new(0., -r)) < 1e-3, "the first dash is at 12 o'clock: {top}");
        assert!(angle.abs() < 1e-5, "and lies flat, along the circle: {angle}");
        let (right, angle) = dash(n / 4, n, r);
        assert!(right.distance(Vec2::new(r, 0.)) < 1e-3, "a quarter turn is on the right: {right}");
        assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 1e-5, "standing upright: {angle}");
        assert_eq!(lit(0, n, 0., 4.), (0., 0.), "an empty ring shows only its track");
        assert_eq!(lit(0, n, 1., 4.), (1., 0.), "a full turn leaves the first dash behind the head");
        assert_eq!(lit(n - 1, n, 1., 4.), (1., 0.75), "the dashes behind the head whiten");
        let (filled, heat) = lit(5, n, 0.25, 4.); // a quarter filled: dash 5 sits just behind the edge
        assert!(filled == 1. && heat > 0. && heat < 1., "{filled} {heat}");
    }

    #[test]
    fn a_full_hold_lights_the_whole_ring() {
        let look = ring();
        let mut r = Ring::start(&look);
        for _ in 0..120 {
            r.step(1., &look, 1. / 60.);
        }
        assert_eq!(r.shown(), 1.);
        let alpha = |i: u32| dash_color(&r, i, &look).alpha();
        assert!((alpha(0) - 1.).abs() < 0.02, "filled dashes are solid: {}", alpha(0));
        assert!(look.track < 1.);
    }
}
