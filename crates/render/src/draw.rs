//! Fighters (stick figure, attachments, procedural overlays) and projectiles.

use std::f64::consts::TAU;

use sim::GameData;
use sim::data::Color;
use sim::data::visual::{Attachment, GlowWhen, Joint, Shape, ShapeWhen};
use sim::world::{Fighter, Projectile, State};

use crate::Rand;
use crate::canvas::{Canvas, P, Rgba, add};
use crate::pose::Skel;

pub fn color(d: &GameData, c: Color, player: usize, a: f64) -> Rgba {
    let pal = &d.global.palette;
    let slot = |pick: fn(&sim::data::global::PlayerColors) -> Color| match pal.get(player % pal.len().max(1)).map(pick)
    {
        Some(Color::Rgb(v)) => v,
        _ => 0xffffff,
    };
    let rgb = match c {
        Color::Rgb(v) => v,
        Color::Player => slot(|p| p.main),
        Color::PlayerDark => slot(|p| p.dark),
        Color::PlayerLight => slot(|p| p.light),
    };
    Rgba::hex(rgb, a)
}

fn dir(deg: f64) -> P {
    let (s, c) = deg.to_radians().sin_cos();
    [c, s]
}

/// Linear `[frame, value]` keys: clamps outside the keyed range.
fn keyed(keys: &[[f64; 2]], t: f64) -> f64 {
    for k in 1..keys.len() {
        if t > keys[k][0] {
            continue;
        }
        let ([t0, a0], [t1, a1]) = (keys[k - 1], keys[k]);
        return if t1 > t0 { a0 + (a1 - a0) * ((t - t0) / (t1 - t0)).max(0.) } else { a1 };
    }
    keys.last().map_or(0., |k| k[1])
}
fn keyed_xy(keys: &[[f64; 3]], t: f64) -> P {
    let x: Vec<[f64; 2]> = keys.iter().map(|k| [k[0], k[1]]).collect();
    let y: Vec<[f64; 2]> = keys.iter().map(|k| [k[0], k[2]]).collect();
    [keyed(&x, t), keyed(&y, t)]
}

fn joint(s: &Skel, j: Joint) -> P {
    match j {
        Joint::Root => [0., 0.],
        Joint::Head => s.head,
        Joint::Shoulder => s.shoulder,
        Joint::Hip => s.hip,
        Joint::ElbowF => s.elbow_f,
        Joint::HandF => s.hand_f,
        Joint::ElbowB => s.elbow_b,
        Joint::HandB => s.hand_b,
        Joint::FootF => s.foot_f,
        Joint::FootB => s.foot_b,
    }
}

/// Everything about a fighter's move that drawing cares about.
struct MoveCtx<'a> {
    phase: Option<&'a sim::data::Phase>,
    t: f64,
    charging: bool,
    jet: bool,
}

fn move_ctx<'a>(d: &'a GameData, f: &Fighter) -> MoveCtx<'a> {
    match (&f.mov, f.state) {
        (Some(m), State::Attack) => {
            let ph = sim::moves::phase(d, f);
            let charging = ph.charge.as_ref().is_some_and(|c| m.t == c.frame && m.charge > 0);
            MoveCtx {
                phase: Some(ph),
                t: f64::from(m.t),
                charging,
                jet: charging && ph.charge.as_ref().is_some_and(|c| c.jet),
            }
        }
        _ => MoveCtx { phase: None, t: 0., charging: false, jet: false },
    }
}

pub fn dodging(f: &Fighter) -> bool {
    f.inv > 0 && matches!(f.state, State::Roll | State::Spot | State::Airdodge | State::Tech)
}

/// Resolved attachment for this tick.
struct Placed<'a> {
    a: &'a Attachment,
    at: P,
    angle: f64,
    scale: f64,
    glow: bool,
}

/// A fighter as drawn this frame: interpolated position and skeleton over the current tick's state.
pub struct FighterView<'a> {
    pub i: usize,
    pub f: &'a Fighter,
    pub x: f64,
    pub y: f64,
    pub skel: Skel,
    /// Interpolated named-joint pose, or None to sample the current tick.
    pub rig: Option<sim::data::rig::RigPose>,
}

pub fn draw_fighter(cv: &mut Canvas, d: &GameData, frame: u64, v: &FighterView, rng: &mut Rand) {
    let (f, i) = (v.f, v.i);
    if matches!(f.state, State::Out | State::Dead) {
        return;
    }
    let fd = &d.fighters[f.data];
    let o = &d.global.render.overlays;
    let st = &fd.style;
    let mc = move_ctx(d, f);
    if mc.phase.is_some_and(|p| p.hidden) {
        return;
    }
    let dodge = dodging(f);
    let mut p = v.skel;
    let fr = frame as f64;

    // Attachments: phase keys, hold IK (moves the front arm), resting angles.
    let active = !sim::combat::active_boxes_at(d, f, frame).is_empty();
    let countering = sim::combat::countering(d, f).is_some();
    let mut placed = Vec::new();
    let mut trails = Vec::new();
    for a in fd.attachments.iter().filter(|_| fd.rig.is_none()) {
        let anim = mc.phase.and_then(|ph| ph.attach.get(&a.id));
        let show = anim.and_then(|an| an.show).is_some_and(|[s, e]| mc.t >= f64::from(s) && mc.t <= f64::from(e));
        if a.hidden && !show {
            continue;
        }
        let keyed_angle = anim.filter(|an| !an.angle.is_empty()).map(|an| keyed(&an.angle, mc.t));
        if let (Some(ang), Some(h)) = (keyed_angle, &a.hold) {
            p.elbow_f = add(p.shoulder, dir(ang + h.bend).map(|c| c * h.elbow));
            p.hand_f = add(p.shoulder, dir(ang).map(|c| c * h.hand));
        }
        if let (Some(an), Some(tr), Some(ph)) = (anim, &a.trail, mc.phase)
            && !an.angle.is_empty()
        {
            let total = f64::from(ph.frames.unwrap_or(u32::MAX));
            let cd = f.mov.as_ref().map_or(0., |m| m.counter_damage);
            let win = ph
                .hitboxes
                .iter()
                .filter(|b| !b.counter_damage || cd > 0.)
                .map(|b| (f64::from(b.start), b.end.map_or(total, f64::from)))
                .rfind(|&(s, e)| mc.t >= s && mc.t <= e + f64::from(o.trail.fade));
            if let Some((s, e)) = win {
                let a1 = keyed(&an.angle, mc.t.min(e));
                let a0 = keyed(&an.angle, s.max(mc.t - f64::from(o.trail.frames)));
                let fade = if mc.t > e { 1. - (mc.t - e) / (f64::from(o.trail.fade) + 1.) } else { 1. };
                if (a1 - a0).abs() > 3. {
                    trails.push((tr.inner, tr.outer, a0, a1, fade));
                }
            }
        }
        placed.push((a, anim, keyed_angle));
    }
    let placed: Vec<Placed> = placed
        .into_iter()
        .map(|(a, anim, keyed_angle)| {
            let base = joint(&p, a.joint);
            let rest = match a.joint {
                Joint::HandF => vec_angle(p.elbow_f, p.hand_f) + a.angle,
                Joint::HandB => vec_angle(p.elbow_b, p.hand_b) + a.angle,
                _ => a.angle,
            };
            let sway = a.sway.as_ref().map_or(0., |s| s.deg * (TAU * fr / s.period.max(1.)).sin());
            let off = anim.filter(|an| !an.offset.is_empty()).map_or([0., 0.], |an| keyed_xy(&an.offset, mc.t));
            let scale = anim.filter(|an| !an.scale.is_empty()).map_or(1., |an| keyed(&an.scale, mc.t));
            let glow = match a.glow {
                Some(GlowWhen::Countering) => countering,
                Some(GlowWhen::JetCharging) => mc.jet,
                Some(GlowWhen::Active) => active,
                None => false,
            };
            Placed { a, at: add(base, off), angle: keyed_angle.unwrap_or(rest) + sway, scale, glow }
        })
        .collect();

    // Overlay colors and alpha.
    let pal = |c: Color, a: f64| color(d, c, i, a);
    if dodge && fd.rig.is_none() {
        let n = o.dodge.ghosts;
        for k in (1..=n).rev() {
            let s = f64::from(k) * o.dodge.spacing;
            let gy = if f.ground { 0. } else { f.vy * s };
            let c = pal(Color::PlayerLight, o.dodge.ghost_alpha * f64::from(n + 1 - k));
            ghost(cv, f, &p, [v.x - f.vx * s, v.y - gy], c, (o.dodge.ghost_width, st.head_radius));
        }
    }
    let jitter = if f.lag > 0 && f.flash > 0 {
        o.hit_jitter
    } else if mc.charging {
        o.charge.jitter
    } else {
        0.
    };
    let jx = if jitter > 0. { rng.range(-jitter, jitter) } else { 0. };
    cv.save();
    cv.translate(v.x + jx, v.y);
    if f.state == State::Respawn {
        let r = &o.respawn;
        let a = r.alpha + r.pulse * (fr * r.speed).to_radians().sin();
        cv.fill_ellipse([0., r.y], r.w / 2., r.h / 2., pal(Color::Player, a));
    }
    let blink = f.blink > 0 && (frame / u64::from(o.blink_period.max(1))) % 2 == 1;
    let alpha = if blink {
        o.blink_alpha
    } else if dodge {
        o.dodge.alpha
    } else {
        1.
    };
    apply_body_transform(cv, sim::pose::BodyTransform::new(&p, [0., 0.], f.face));
    let helpless = f.state == State::Helpless;
    let charge_blink = mc.charging && (frame / u64::from(o.charge.blink.max(1))) % 2 == 1;
    let body = if f.flash > 0 {
        color(d, o.flash, i, alpha)
    } else if charge_blink {
        color(d, o.charge.color, i, alpha)
    } else if helpless {
        color(d, o.helpless, i, alpha)
    } else if dodge {
        pal(Color::PlayerLight, alpha)
    } else {
        pal(Color::Player, alpha)
    };
    let back = if helpless {
        color(d, o.helpless_back, i, alpha)
    } else if dodge {
        pal(Color::PlayerLight, alpha)
    } else {
        pal(Color::PlayerDark, alpha)
    };
    let glow = if mc.charging {
        color(d, o.charge.color, i, o.charge.glow_alpha * alpha)
    } else if dodge {
        color(d, o.dodge.glow, i, o.dodge.glow_alpha * alpha)
    } else {
        pal(Color::Player, o.glow_alpha * alpha)
    };

    for (ri, ro, a0, a1, fade) in trails {
        cv.fill_sector(
            p.shoulder,
            ri,
            ro,
            a0.to_radians(),
            a1.to_radians(),
            color(d, o.trail.color, i, o.trail.alpha * fade * alpha),
        );
    }
    if let Some(rig) = &fd.rig {
        let sampled;
        let pose = match &v.rig {
            Some(pose) => pose,
            None => {
                sampled = sim::pose::fighter_rig_pose(d, f, frame);
                &sampled
            }
        };
        draw_rig(
            cv,
            d,
            i,
            rig,
            pose,
            alpha,
            glow,
            st.glow_width,
            (body, back),
            active || mc.charging || countering,
            frame,
        );
    } else {
        let neck = [p.head[0] * 0.5 + p.shoulder[0] * 0.5, p.head[1] - st.head_radius];
        let torso = [neck, p.shoulder, p.hip];
        let limbs = [
            [p.hip, p.knee_b, p.foot_b],
            [p.shoulder, p.elbow_b, p.hand_b],
            [p.hip, p.knee_f, p.foot_f],
            [p.shoulder, p.elbow_f, p.hand_f],
        ];
        for l in &limbs {
            cv.stroke_open(l, st.glow_width, glow, true);
        }
        cv.stroke_open(&torso, st.torso_glow_width, glow, true);
        cv.fill_circle(p.head, st.head_glow_radius, glow);

        for pl in placed.iter().filter(|pl| pl.a.behind) {
            draw_attachment(cv, d, i, pl, alpha, frame);
        }
        cv.stroke_open(&limbs[0], st.back_limb_width, back, true);
        cv.stroke_open(&limbs[1], st.back_limb_width, back, true);
        cv.stroke_open(&torso, st.torso_width, body, true);
        cv.stroke_open(&limbs[2], st.front_limb_width, body, true);
        cv.stroke_open(&limbs[3], st.front_limb_width, body, true);
        cv.fill_circle(p.head, st.head_radius, color(d, st.head_fill, i, alpha));
        cv.stroke_circle(p.head, st.head_radius, st.head_ring, body);
        let [ex, ey, ew, eh] = st.eye;
        let h = p.head;
        if matches!(f.state, State::Dizzy | State::Knockdown) {
            let (x0, x1, y0, y1) = (h[0] + ex - 1., h[0] + ex + ew, h[1] + ey, h[1] + ey + eh);
            cv.line([x0, y1], [x1, y0], o.x_eye_width, body);
            cv.line([x1, y1], [x0, y0], o.x_eye_width, body);
        } else {
            cv.fill_rect(h[0] + ex, h[1] + ey, ew, eh, body);
        }
    }
    for pl in placed.iter().filter(|pl| !pl.a.behind) {
        draw_attachment(cv, d, i, pl, alpha, frame);
    }
    cv.restore();

    if f.state == State::Grabbed
        && let Some(g) = &o.grabbed
    {
        for [y, rx, ry] in &g.bands {
            let height = fd.body.height;
            let pts = cv.ellipse_pts([v.x, v.y - y * height], rx * height, ry * height);
            cv.stroke_closed(&pts, g.width, pal(g.color, g.alpha));
        }
    }
    if f.state == State::Shield {
        let s = &o.shield;
        let stun = f.sd.stun > 0;
        let r = s.min_r + s.grow_r * f.sh.max(0.) / d.global.shield.max;
        let c = [v.x, v.y - s.y];
        cv.fill_circle(c, r, pal(Color::Player, if stun { s.stun_alpha } else { s.alpha }));
        let stroke = if stun { Rgba::hex(0xffffff, s.stroke_alpha) } else { pal(Color::Player, s.stroke_alpha) };
        cv.stroke_circle(c, r, s.width, stroke);
    }
    if f.state == State::Dizzy {
        let z = &o.dizzy;
        for k in 0..z.stars {
            let a = (fr * z.speed).to_radians() + TAU * f64::from(k) / f64::from(z.stars.max(1));
            cv.star([v.x + a.cos() * z.rx, v.y - z.y + a.sin() * z.ry], z.r, color(d, z.color, i, 1.));
        }
    }
}

// Keep the canvas matrix for strokes and joint-attached shapes (including reflection).
// This is the matrix form of BodyTransform, tested against its deterministic point math.
fn apply_body_transform(cv: &mut Canvas, t: sim::pose::BodyTransform) {
    cv.translate(t.root[0], t.root[1] - t.offset_y);
    cv.scale(t.facing, -1.);
    if t.rotation != 0. {
        cv.translate(0., t.pivot);
        cv.rotate(-t.rotation.to_radians());
        cv.translate(0., -t.pivot);
    }
}

/// Named figures use the same deterministic local transforms as authoring tools.
fn draw_rig(
    cv: &mut Canvas,
    d: &GameData,
    player: usize,
    rig: &sim::data::rig::Rig,
    pose: &sim::data::rig::RigPose,
    alpha: f64,
    glow: Rgba,
    glow_width: f64,
    palette: (Rgba, Rgba),
    lit: bool,
    frame: u64,
) {
    let Ok(resolved) = sim::rig::resolve(rig, pose) else { return };
    for bone in &rig.bones {
        let (Some(a), Some(b)) = (resolved.get(&bone.from), resolved.get(&bone.to)) else { continue };
        cv.line(a.translation, b.translation, bone.width + glow_width, glow);
        cv.line(a.translation, b.translation, bone.width, rig_color(d, bone.color, player, alpha, 1., Some(palette)));
    }
    for (joint, resolved) in rig.joints.iter().zip(&resolved.joints) {
        cv.save();
        cv.translate(resolved.transform.translation[0], resolved.transform.translation[1]);
        cv.rotate(resolved.transform.angle.to_radians());
        for shape in &joint.shapes {
            draw_shape_palette(cv, d, player, shape, alpha, lit, frame, Some(palette));
        }
        cv.restore();
    }
}

fn vec_angle(a: P, b: P) -> f64 {
    (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees()
}

fn ghost(cv: &mut Canvas, f: &Fighter, p: &Skel, [x, y]: P, c: Rgba, (w, head): (f64, f64)) {
    cv.save();
    cv.translate(x, y - p.offset_y);
    cv.scale(f.face, -1.);
    if p.rot != 0. {
        cv.translate(0., p.pivot);
        cv.rotate(-p.rot.to_radians());
        cv.translate(0., -p.pivot);
    }
    for l in [
        [p.hip, p.knee_b, p.foot_b],
        [p.shoulder, p.elbow_b, p.hand_b],
        [p.hip, p.knee_f, p.foot_f],
        [p.shoulder, p.elbow_f, p.hand_f],
    ] {
        cv.stroke_open(&l, w, c, true);
    }
    cv.stroke_open(&[p.shoulder, p.hip], w, c, true);
    cv.stroke_circle(p.head, head, w, c);
    cv.restore();
}

fn draw_attachment(cv: &mut Canvas, d: &GameData, player: usize, pl: &Placed, alpha: f64, frame: u64) {
    cv.save();
    cv.translate(pl.at[0], pl.at[1]);
    cv.rotate(pl.angle.to_radians());
    cv.scale(pl.scale, pl.scale);
    for s in &pl.a.shapes {
        draw_shape(cv, d, player, s, alpha, pl.glow, frame);
    }
    cv.restore();
}

/// One vector shape in the current (y-up) space.
pub fn draw_shape(cv: &mut Canvas, d: &GameData, player: usize, s: &Shape, alpha: f64, glow: bool, frame: u64) {
    draw_shape_palette(cv, d, player, s, alpha, glow, frame, None);
}

fn rig_color(
    d: &GameData,
    c: Color,
    player: usize,
    alpha: f64,
    shape_alpha: f64,
    palette: Option<(Rgba, Rgba)>,
) -> Rgba {
    if let Some((body, back)) = palette {
        let base = match c {
            Color::Player | Color::PlayerLight => Some(body),
            Color::PlayerDark => Some(back),
            _ => None,
        };
        if let Some(base) = base {
            return base.alpha(shape_alpha);
        }
    }
    color(d, c, player, alpha * shape_alpha)
}

fn draw_shape_palette(
    cv: &mut Canvas,
    d: &GameData,
    player: usize,
    s: &Shape,
    alpha: f64,
    glow: bool,
    frame: u64,
    palette: Option<(Rgba, Rgba)>,
) {
    let on = |w: ShapeWhen| match w {
        ShapeWhen::Always => true,
        ShapeWhen::Glow => glow,
        ShapeWhen::NoGlow => !glow,
    };
    let col = |c: Color, a: f64| rig_color(d, c, player, alpha, a, palette);
    match s {
        Shape::Line { from, to, width, color: c, alpha: a, when } if on(*when) => {
            cv.line(*from, *to, *width, col(*c, *a))
        }
        Shape::Circle { at, r, fill, stroke, width, alpha: a, when } if on(*when) => {
            if let Some(c) = fill {
                cv.fill_circle(*at, *r, col(*c, *a));
            }
            if let Some(c) = stroke {
                cv.stroke_circle(*at, *r, *width, col(*c, *a));
            }
        }
        Shape::Poly { points, fill, stroke, width, alpha: a, when } if on(*when) => {
            if let Some(c) = fill {
                cv.fill_poly(points, col(*c, *a));
            }
            if let Some(c) = stroke {
                cv.stroke_closed(points, *width, col(*c, *a));
            }
        }
        Shape::Rect { x, y, w, h, radius, fill, stroke, width, alpha: a, when } if on(*when) => {
            let pts = cv.round_rect_pts(*x, *y, *w, *h, *radius);
            if let Some(c) = fill {
                cv.fill_convex(&pts, col(*c, *a));
            }
            if let Some(c) = stroke {
                cv.stroke_closed(&pts, *width, col(*c, *a));
            }
        }
        Shape::Star { at, r, fill, alpha: a, blink, when }
            if on(*when) && (*blink == 0 || (frame / u64::from(*blink)) % 2 == 1) =>
        {
            cv.star(*at, *r, col(*fill, *a))
        }
        _ => {}
    }
}

pub fn draw_projectile(cv: &mut Canvas, d: &GameData, p: &Projectile, x: f64, y: f64, frame: u64) {
    let Some(pd) = d.fighters[p.fighter_data].projectiles.get(&p.id) else {
        return;
    };
    cv.save();
    cv.translate(x, y);
    cv.scale(1., -1.);
    if let Some(a) = &pd.article {
        cv.scale(p.face, 1.);
        if pd.spin != 0. {
            // Article projectiles don't carry a travel-direction ring; spin them on their own age.
            cv.rotate((pd.spin * p.age as f64).to_radians());
        }
        let stage = &a.stages[p.stage];
        let startup = if p.automatic { stage.auto_startup.unwrap_or(stage.startup) } else { stage.startup };
        let attacking = p.released.is_some_and(|age| p.age - age >= startup);
        let shapes = if attacking && !stage.attack_shapes.is_empty() { &stage.attack_shapes } else { &stage.shapes };
        for s in shapes {
            draw_shape(cv, d, p.owner, s, 1., false, frame);
        }
        if let (Some(stroke), Some(age)) = (&stage.stroke, p.released) {
            let t = p.age - age + if p.automatic { stage.startup - startup } else { 0 };
            let mut previous = None;
            for b in stage.hitboxes.iter().filter(|b| b.damage > 0. && !b.frames.is_empty() && b.active(t)) {
                let (x, y, _) = b.geom_at(t);
                if let Some(point) = previous {
                    cv.line(point, [x, y], stroke.width, color(d, stroke.color, p.owner, 1.));
                }
                previous = Some([x, y]);
            }
        }
        cv.restore();
        return;
    }
    cv.save();
    cv.rotate((pd.spin * x).to_radians());
    for s in &pd.shapes {
        draw_shape(cv, d, p.owner, s, 1., false, frame);
    }
    cv.restore();
    for s in &pd.overlay {
        draw_shape(cv, d, p.owner, s, 1., false, frame);
    }
    cv.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mike_up_smash_keeps_swinging_forward_through_zero() {
        let fighter: serde_json::Value =
            serde_json::from_str(include_str!("../../../assets/fighters/mike/fighter.json")).unwrap();
        let anim: sim::data::visual::AttachAnim =
            serde_json::from_value(fighter["moves"]["usmash"]["phases"][0]["attach"]["sword"].clone()).unwrap();
        let mut previous = keyed(&anim.angle, 25.);
        for step in 1..=40 {
            let t = 25. + f64::from(step) / 10.;
            let angle = keyed(&anim.angle, t);
            assert!(angle > previous, "swing reversed at frame {t}: {previous} → {angle}");
            previous = angle;
        }
        assert_eq!(keyed(&anim.angle, 26.5), 390.);
        // Trail endpoints must stay on the same turn, even across several keyframes.
        assert_eq!(keyed(&anim.angle, 28.) - keyed(&anim.angle, 25.), 220.);
        assert_eq!(keyed(&anim.angle, 31.), 540.);
        assert_eq!(keyed(&anim.angle, 62.), 415.);
    }
}

#[cfg(test)]
mod transform_tests {
    use super::*;

    #[test]
    fn canvas_body_and_joint_matrices_match_shared_point_math() {
        for face in [-1., 1.] {
            for rot in [0., 90., -37., 215.] {
                for pivot in [0., 13.] {
                    for offset_y in [0., 7.] {
                        let t = sim::pose::BodyTransform::new(
                            &Skel { rot, pivot, offset_y, ..Default::default() },
                            [101., -53.],
                            face,
                        );
                        let joint = sim::rig::Transform { translation: [2., 8.], angle: 123. };
                        let mut cv = Canvas::new(1.);
                        apply_body_transform(&mut cv, t);
                        cv.translate(joint.translation[0], joint.translation[1]);
                        cv.rotate(joint.angle.to_radians());
                        for p in [[0., 0.], [3., -9.], [-7., 4.]] {
                            let actual = cv.to_world(p);
                            let expected = t.local_to_world(joint.point(p));
                            for (a, b) in actual.into_iter().zip(expected) {
                                assert!((a - b).abs() < 1e-10, "{actual:?} != {expected:?}");
                            }
                        }
                    }
                }
            }
        }
    }
}
