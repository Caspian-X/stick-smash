//! Debug overlay geometry (hitboxes by type, hurtboxes, ledge zones, blast zone) and per-fighter info text.

use std::f64::consts::PI;

use sim::data::{HitType, LaunchDir};
use sim::world::{Fighter, State};
use sim::{GameData, World};

use crate::canvas::{Canvas, Rgba};

pub fn hit_type_color(t: HitType) -> u32 {
    match t {
        HitType::Normal => 0xff3355,
        HitType::Spike => 0xff8a1f,
        HitType::Trip => 0x2ad4c0,
        HitType::Sakurai => 0xff5dc8,
        HitType::Autolink => 0x3fb6ff,
        HitType::Windbox => 0xa8c4ff,
        HitType::Electric => 0xffe14a,
        HitType::Paralyze => 0xffee65,
        HitType::Bury => 0xc89666,
        HitType::Grab => 0xb070ff,
    }
}

/// `pos[i]` = where fighter i is drawn (interpolated), so boxes stick to the figure between ticks.
pub fn draw_debug(cv: &mut Canvas, d: &GameData, w: &World, _pos: &[(f64, f64)]) {
    let s = &d.stages[w.stage];
    let b = &s.blast;
    cv.stroke_closed(
        &[[b.left, b.top], [b.right, b.top], [b.right, b.bottom], [b.left, b.bottom]],
        3.,
        Rgba::hex(0xff3355, 0.6),
    );
    let l = &s.ledge;
    for b in &s.blocks {
        for side in [-1., 1.] {
            if !(if side < 0. { b.left_ledge } else { b.right_ledge }) {
                continue;
            }
            let cx = if side < 0. { b.left } else { b.right };
            let c = [cx, b.top + l.cy];
            let a0 = if side < 0. { PI / 2. } else { -PI / 2. };
            cv.fill_sector(c, 0., l.r, a0, a0 + PI, Rgba::hex(0x66ffcc, 0.08));
            let n = 24;
            let pts: Vec<_> = (0..=n)
                .map(|k| a0 + PI * f64::from(k) / f64::from(n))
                .map(|a| [c[0] + l.r * a.cos(), c[1] + l.r * a.sin()])
                .collect();
            cv.stroke_closed(&pts, 1., Rgba::hex(0x66ffcc, 0.45));
        }
    }
    for p in &w.projs {
        cv.stroke_circle([p.x, p.y], p.r, 2., Rgba::hex(0xff3355, 1.));
    }
    for f in &w.fighters {
        if matches!(f.state, State::Out | State::Dead) {
            continue;
        }
        let (x, y) = (f.x, f.y);
        let (dx, dy) = (x - f.x, y - f.y);
        let h = sim::combat::hurt(d, f);
        let hc = if f.inv > 0 || f.blink > 0 {
            0x66ddff
        } else if f.state == State::Hitstun {
            0xff9a3c
        } else {
            0xffee55
        };
        let r = [[h.x1 + dx, h.y1 + dy], [h.x2 + dx, h.y1 + dy], [h.x2 + dx, h.y2 + dy], [h.x1 + dx, h.y2 + dy]];
        if let Some(regions) = sim::combat::hurt_shapes(d, f, w.frame) {
            for shape in regions {
                combat_shape(cv, shape, 1.5, Rgba::hex(hc, 0.9));
            }
        } else {
            cv.fill_convex(&r, Rgba::hex(hc, 0.12));
            cv.stroke_closed(&r, 1.5, Rgba::hex(hc, 0.9));
        }
        cv.fill_circle([x, y], 2.5, Rgba::hex(0xffffff, 1.));
        cv.fill_circle([x, y - l.hand_y], 2.5, Rgba::hex(0x66ffcc, 1.));
        for bx in sim::combat::active_boxes_at(d, f, w.frame) {
            let (bx_, by, hb) = (bx.x + dx, bx.y + dy, &bx.hit);
            let c = hit_type_color(hb.kind);
            combat_shape(cv, bx.shape, 2., Rgba::hex(c, 1.));
            launch_arrow(cv, [bx_, by], hb, f.face, 1.);
        }
    }
}

/// Launch direction marker for a hitbox centered at `c` (stage pixels): an arrow along the launch angle sized by
/// bkb, a ring for `away`/sakurai, nothing for grabs and autolink. `px` scales line widths (1 = stage pixels).
pub fn launch_arrow(cv: &mut Canvas, c: [f64; 2], hb: &sim::data::Hitbox, face: f64, px: f64) {
    if matches!(hb.kind, HitType::Grab | HitType::Autolink) {
        return;
    }
    if hb.kind == HitType::Sakurai || hb.dir == LaunchDir::Away {
        cv.stroke_circle(c, 4. * px, 2. * px, Rgba::hex(0xffffff, 0.9));
        return;
    }
    let e = launch_tip(c, hb, face);
    cv.line(c, e, 2.5 * px, Rgba::hex(0xffffff, 0.95));
    cv.fill_circle(e, 3. * px, Rgba::hex(0xffffff, 1.));
}

/// Where [`launch_arrow`] ends: along the launch angle (spikes point down), 16 + 3 × bkb pixels long.
pub fn launch_tip(c: [f64; 2], hb: &sim::data::Hitbox, face: f64) -> [f64; 2] {
    let sgn = if hb.dir == LaunchDir::Back { -face } else { face };
    let a = (if hb.kind == HitType::Spike { -hb.angle.abs() } else { hb.angle }).to_radians();
    let len = 16. + hb.bkb * 3.;
    [c[0] + a.cos() * len * sgn, c[1] - a.sin() * len]
}

/// State / move / frame lines shown above a fighter.
pub fn info(d: &GameData, f: &Fighter) -> String {
    let mut lines = vec![format!("{} {}", f.state.name(), f.t)];
    if f.state == State::Attack
        && let Some(m) = &f.mov
    {
        let def = sim::moves::definition(d, f);
        let ph = &def.phases[m.phase];
        let total = ph.frames.map(|n| format!("/{n}")).unwrap_or_default();
        let charge = if m.charge > 0 { format!(" +{}ch", m.charge) } else { String::new() };
        let phase = if def.phases.len() > 1 { format!(" [{}]", ph.name) } else { String::new() };
        lines.push(format!("{}{phase} {}{total}{charge}", def.name, m.t));
    }
    let mut extra = vec![];
    if f.lag > 0 {
        extra.push(format!("hitlag {}", f.lag));
    }
    if f.state == State::Hitstun {
        extra.push(format!("stun {}", f.hitstun));
    }
    if f.state == State::Shield && f.sd.stun > 0 {
        extra.push(format!("shieldstun {}", f.sd.stun));
    }
    if f.state == State::Landing {
        extra.push(format!("lag {}", f.sd.n - f64::from(f.t)));
    }
    if f.inv > 0 || f.blink > 0 {
        extra.push(format!("inv {}", f.inv.max(f.blink)));
    }
    if !extra.is_empty() {
        lines.push(extra.join(" · "));
    }
    lines.join("\n")
}

/// Exact closed circle/capsule outline at the simulation tick.
pub fn combat_shape(cv: &mut Canvas, shape: sim::geometry::Shape, width: f64, color: Rgba) {
    match shape {
        sim::geometry::Shape::Circle { center, radius } => cv.stroke_circle(center, radius, width, color),
        sim::geometry::Shape::Capsule { a, b, radius } => {
            if a == b {
                cv.stroke_circle(a, radius, width, color);
                return;
            }
            let angle = (b[1] - a[1]).atan2(b[0] - a[0]);
            let mut points = Vec::new();
            for (center, start) in [(b, angle - PI / 2.), (a, angle + PI / 2.)] {
                for i in 0..=24 {
                    let t = start + PI * f64::from(i) / 24.;
                    points.push([center[0] + radius * t.cos(), center[1] + radius * t.sin()]);
                }
            }
            cv.stroke_closed(&points, width, color);
        }
    }
}
