use crate::actions::{Action, accept};
use crate::data::{Trigger, global::UltimateMovement};
use crate::{
    GameData, World,
    input::Intent,
    world::{Fighter, State, sign},
};

pub fn surface(data: &GameData, w: &World, id: usize) -> (f64, f64, f64) {
    let s = &data.stages[w.stage];
    if id < s.blocks.len() {
        let b = &s.blocks[id];
        (b.left, b.right, b.top)
    } else {
        s.platforms[id - s.blocks.len()].position(w.frame as f64)
    }
}
pub fn integrate(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, edge_stop: bool) {
    let (was_g, vy, py, surf) = (f.ground, f.vy, f.y, f.surf);
    let g = &d.global.sim;
    let s = &d.stages[w.stage];
    let px = f.x;
    let mut supported = false;
    f.x += f.vx;
    if was_g
        && let Some(surf) = surf
        && f.vy >= 0.
    {
        let (x1, x2, y) = surface(d, w, surf);
        if edge_stop {
            if f.x < x1 {
                f.x = x1;
                f.vx = 0.;
            }
            if f.x > x2 {
                f.x = x2;
                f.vx = 0.;
            }
        }
        let pad = if surf < s.blocks.len() { g.main_pad } else { g.platform_pad };
        if f.x >= x1 - pad && f.x <= x2 + pad && !(f.drop > 0 && surf >= s.blocks.len()) {
            f.y = y;
            f.vy = 0.;
            supported = true;
        }
    }
    if !supported {
        f.y += f.vy;
        f.ground = false;
        f.surf = None;
        let passing = d.global.movement.platform_pass
            && f.in_down
            && matches!(f.state, State::Air | State::Helpless)
            && !f.tumble;
        if f.drop == 0 && !passing {
            // Choose the first surface crossed in relative motion, rather than array order.
            let mut landing: Option<(usize, f64, f64)> = None;
            for (n, p) in s.platforms.iter().enumerate() {
                let (x1, x2, y) = p.position(w.frame as f64);
                let (_, _, old_y) = p.position(w.frame.saturating_sub(1) as f64);
                let relative = f.y - py - (y - old_y);
                if relative >= 0.
                    && py <= old_y + g.surface_tolerance
                    && f.y >= y
                    && f.x >= x1 - g.platform_pad
                    && f.x <= x2 + g.platform_pad
                {
                    let t = if relative == 0. { 0. } else { ((old_y - py) / relative).max(0.) };
                    if landing.is_none_or(|(_, _, best)| t < best) {
                        landing = Some((n + s.blocks.len(), y, t));
                    }
                }
            }
            if let Some((id, y, _)) = landing {
                f.y = y;
                f.vy = 0.;
                f.ground = true;
                f.surf = Some(id);
            }
        }
    }
    resolve_solids(d, s, f, px, py);
    if f.ground && !was_g {
        on_land(w, d, i, f, vy);
    }
}
/// Resolve solid faces after velocity integration or platform carry.
pub fn resolve_solids(d: &GameData, s: &crate::data::Stage, f: &mut Fighter, px: f64, py: f64) {
    let b = &d.fighters[f.data].body;
    let stage = s;
    for (block, s) in stage.blocks.iter().enumerate() {
        // Swept faces prevent fast movement from crossing thin walls or tops.
        if py < s.top && f.y > s.top && !s.intersects_body(f.x, f.y, b.half_width, b.height) {
            let t = (s.top - py) / (f.y - py);
            let x = px + (f.x - px) * t;
            if x + b.half_width > s.left && x - b.half_width < s.right {
                f.y = s.top;
                f.vy = 0.;
                f.ground = true;
                f.surf = Some(block);
            }
        }
        let dx = f.x - px;
        for (face, toward) in [(s.left - b.half_width, 1.), (s.right + b.half_width, -1.)] {
            if dx * toward > 0. && (px - face) * toward <= 0. && (f.x - face) * toward > 0. {
                let t = (face - px) / dx;
                let y = py + (f.y - py) * t;
                if y > s.top
                    && y - b.height < s.bottom
                    && !stage
                        .blocks
                        .iter()
                        .enumerate()
                        .any(|(i, other)| i != block && other.intersects_body(face, f.y, b.half_width, b.height))
                {
                    f.x = face;
                    f.vx = 0.;
                }
            }
        }
        let (hx1, hx2, hy1, hy2) = (f.x - b.half_width, f.x + b.half_width, f.y - b.height, f.y);
        let xs = (if f.x < (s.left + s.right) / 2. { hx2 } else { hx1 }).clamp(s.left, s.right);
        let sb = s.bottom + ((xs - s.left).min(s.right - xs) / s.under_inset).min(1.) * s.under_depth;
        if hx2 > s.left && hx1 < s.right && py - b.height >= sb && hy1 < sb && py - f.y > s.under_depth {
            f.y = sb + b.height;
            f.vy = 0.;
            f.ground = false;
            f.surf = None;
        } else if hx2 > s.left && hx1 < s.right && hy2 > s.top && hy1 < sb {
            let over = (hy1 - s.bottom).max(0.) * s.under_inset / s.under_depth;
            let (up, dn, lf, rt) = (hy2 - s.top, sb - hy1, hx2 - s.left - over, s.right - hx1 - over);
            let mn = up.min(dn).min(lf).min(rt);
            if mn == up {
                f.y = s.top;
                f.vy = 0.;
                f.ground = true;
                f.surf = Some(block);
            } else if mn == dn {
                f.y = sb + b.height;
                f.ground = false;
                f.surf = None;
                if f.vy < 0. {
                    f.vy = 0.;
                }
            } else if stage.blocks.iter().enumerate().any(|(i, other)| {
                let x = if mn == lf { f.x - lf } else { f.x + rt };
                i != block && other.intersects_body(x, f.y, b.half_width, b.height)
            }) {
                // A seam or narrow gap cannot eject sideways into its neighbour.
                let exit = |down: bool| {
                    let mut y = f.y;
                    for _ in 0..=stage.blocks.len() {
                        let old = y;
                        for other in &stage.blocks {
                            if other.intersects_body(f.x, y, b.half_width, b.height) {
                                y = if down { other.body_bottom(f.x, b.half_width) + b.height } else { other.top };
                            }
                        }
                        if old == y {
                            break;
                        }
                    }
                    y
                };
                let (up_y, down_y) = (exit(false), exit(true));
                f.y = if f.vy < 0. || down_y - f.y < f.y - up_y { down_y } else { up_y };
                f.vy = 0.;
                f.surf = stage
                    .blocks
                    .iter()
                    .position(|g| f.y == g.top && f.x + b.half_width > g.left && f.x - b.half_width < g.right);
                f.ground = f.surf.is_some();
            } else if mn == lf {
                f.x -= lf;
                if f.vx > 0. {
                    f.vx = 0.;
                }
            } else {
                f.x += rt;
                if f.vx < 0. {
                    f.vx = 0.;
                }
            }
        }
    }
}
pub fn fall(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, edge: bool, cap: Option<f64>) {
    if scripted_rise(d, f) {
        integrate(w, d, i, f, edge);
        return;
    }
    let s = &d.fighters[f.data].stats;
    let (mut g, mut cap) = (s.gravity, cap.unwrap_or(s.fall_speed));
    let stun = f.state == State::Hitstun;
    let top = top_launch(d, f);
    if let Some(t) = top
        && stun
    {
        (g, cap) = (t.gravity + t.gravity_per_weight * 100. / s.weight, t.fall_speed);
    }
    // Gravity and the fall cap act on the fighter's own velocity; the launch decays on top of it.
    let l = f.launch;
    let next = if l == (0., 0.) { l } else { decay_launch(d, l) };
    let own = f.vy - l.1;
    let own = if top.is_some() && !stun && own > cap { (own - g).max(cap) } else { (own + g).min(cap) };
    if top.is_some() && !stun && own <= cap {
        f.top_launch = false;
    }
    f.vy = own + next.1;
    f.vx += next.0 - l.0;
    f.launch = next;
    let v = (f.vx, f.vy);
    integrate(w, d, i, f, edge);
    if l == (0., 0.) {
        return;
    }
    // Landing ends the launch; a wall or ceiling stops its component.
    if f.ground {
        f.launch = (0., 0.);
    } else {
        if f.vx != v.0 {
            f.launch.0 = 0.;
        }
        if f.vy != v.1 {
            f.launch.1 = 0.;
        }
    }
}
/// `hitstun.ultimate.topLaunch` when the fighter's current launch uses it.
fn top_launch<'a>(d: &'a GameData, f: &Fighter) -> Option<&'a crate::data::global::TopLaunch> {
    if !f.top_launch {
        return None;
    }
    d.global.hitstun.ultimate.as_ref().and_then(|u| u.top_launch.as_ref())
}
/// Vertical speed set by the jump's initial rise or a float instead of gravity this tick. True when it applied.
pub fn scripted_rise(d: &GameData, f: &mut Fighter) -> bool {
    let s = &d.fighters[f.data].stats;
    if f.rise > 0 {
        f.rise -= 1;
        f.vy = if f.rise > 0 { -s.jump_initial_height.unwrap_or(0.) / 4. } else { -s.jump_speed };
        return true;
    }
    if f.floating > 0
        && let Some(fl) = &s.float
    {
        f.floating -= 1;
        f.vy = fl.speed;
        return true;
    }
    false
}
/// Once per airtime: a float starts holding the vertical speed at `stats.float.speed` (see `data::Float`).
pub fn start_float(d: &GameData, f: &mut Fighter) -> bool {
    let Some(fl) = &d.fighters[f.data].stats.float else {
        return false;
    };
    if f.float_used || f.ground || f.rise > 0 {
        return false;
    }
    f.float_used = true;
    f.floating = fl.frames;
    f.launch = (0., 0.);
    f.ff = false;
    true
}
/// Launch speed lost per tick in pixels (`hitstun.ultimate.launchDecay` × launch speed).
pub fn decay_amount(d: &GameData) -> f64 {
    d.global.hitstun.ultimate.as_ref().map_or(0., |u| u.launch_decay * crate::combat::reference_scale(d))
}
/// Shrink a launch vector's magnitude by `hitstun.ultimate.launchDecay` (reference units → pixels), never past zero.
pub fn decay_launch(d: &GameData, (x, y): (f64, f64)) -> (f64, f64) {
    let k = decay_amount(d);
    let speed = libm::hypot(x, y);
    if speed <= k {
        (0., 0.)
    } else {
        let r = (speed - k) / speed;
        (x * r, y * r)
    }
}
/// Fast fall on a fresh down press (or one within `fastFallBuffer` ticks, still held) once falling. True when it
/// starts.
pub fn fast_fall(d: &GameData, f: &mut Fighter, inp: Intent) -> bool {
    let m = &d.global.movement;
    if !f.tumble && (inp.down_p || inp.down_recent) && f.vy > m.fast_fall_min_vy {
        let started = !f.ff;
        f.ff = true;
        let kick = if m.ultimate.is_some() { d.fighters[f.data].stats.fast_fall_speed } else { m.fast_fall_kick };
        f.vy = f.vy.max(kick);
        return started;
    }
    false
}
/// `v` moved toward zero by `by`, stopping at magnitude `floor` (`|v| >= floor`).
fn slow(v: f64, by: f64, floor: f64) -> f64 {
    sign(v) * (v.abs() - by).max(floor)
}
/// The fighter's traction under `movement.ultimate`.
pub fn traction(d: &GameData, f: &Fighter) -> Option<f64> {
    let u = d.global.movement.ultimate.as_ref()?;
    Some(d.fighters[f.data].stats.traction.unwrap_or(u.traction))
}
/// Grounded slowdown: traction × `scale` (× `fastTraction` above walk speed) toward zero under `movement.ultimate`,
/// otherwise vx ×= `legacy`.
pub fn skid(d: &GameData, f: &mut Fighter, legacy: f64, scale: f64) {
    match (traction(d, f), &d.global.movement.ultimate) {
        (Some(k), Some(u)) => {
            let fast = if f.vx.abs() > crate::gait::walk_speed(d, f) { u.fast_traction } else { 1. };
            f.vx = slow(f.vx, k * scale * fast, 0.);
        }
        _ => f.vx *= legacy,
    }
}
/// Air acceleration for a control kind: the fighter's airAccel × `mult` under `movement.ultimate`, else `legacy`.
pub fn air_accel(d: &GameData, f: &Fighter, legacy: f64, mult: fn(&UltimateMovement) -> f64) -> f64 {
    d.global.movement.ultimate.as_ref().map_or(legacy, |u| d.fighters[f.data].stats.air_accel * mult(u))
}
/// Analog drift: a partial tilt accelerates and tops out proportionally. Never changes facing. Otherwise
/// `movement.ultimate` slows by the fighter's air friction (toward the held cap when above it, never clamping),
/// and the legacy model multiplies by `drag`.
pub fn air_drift(d: &GameData, f: &mut Fighter, x: f64, acc: f64, drag: f64) {
    let (dir, k) = (sign(x), x.abs().min(1.));
    let s = &d.fighters[f.data].stats;
    let max = s.air_speed * k;
    let control = dir != 0. && acc != 0.;
    if control && f.vx * dir < max {
        f.vx = (f.vx * dir + acc * k).min(max) * dir;
    } else if let Some(u) = &d.global.movement.ultimate {
        f.vx = slow(f.vx, s.air_friction.unwrap_or(u.air_friction), if control { max } else { 0. });
    } else {
        f.vx *= drag;
    }
}
pub fn air_phys(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, x: f64, acc: f64) {
    let m = &d.global.movement;
    let s = &d.fighters[f.data].stats;
    // Drift, drag and fast fall act on the fighter's own velocity, not on what remains of a launch.
    let l = f.launch;
    if l != (0., 0.) {
        f.vx -= l.0;
        f.vy -= l.1;
    }
    air_drift(d, f, x, acc, if x != 0. { m.air_drag_held } else { m.air_drag });
    if f.floating == 0 && fast_fall(d, f, inp) {
        accept(w, i, Action::FastFall);
    }
    let cap = if f.ff { s.fast_fall_speed } else { s.fall_speed };
    if l != (0., 0.) {
        f.vx += l.0;
        f.vy += l.1;
    }
    fall(w, d, i, f, false, Some(cap));
}
pub fn landing(f: &mut Fighter, n: u32) {
    f.set_state(State::Landing);
    f.sd.n = f64::from(n);
}
pub fn on_land(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, vy: f64) {
    let g = &d.global;
    (f.rise, f.top_launch, f.floating, f.float_used) = (0, false, 0, false);
    f.jumps = d.fighters[f.data].stats.air_jumps;
    f.up_b = false;
    f.ad_used = false;
    f.ff = false;
    f.flip_t = 0;
    f.ledge_inv = true;
    f.used_air_moves.clear();
    f.paralyze_used = false;
    match f.state {
        State::Air => {
            if vy > g.landing.soft_land_speed {
                landing(f, g.landing.hard);
            } else {
                f.set_state(State::Idle);
            }
        }
        State::Helpless => landing(f, g.landing.helpless),
        State::Airdodge => landing(f, if f.sd.dirl { g.dodge.dir_air.landing_lag } else { g.dodge.air.landing_lag }),
        State::Attack => {
            crate::moves::trigger(w, d, i, f, Trigger::Land);
        }
        State::Hitstun if f.tumble && vy > g.sim.tech_min_speed => {
            f.hitstun = 0;
            f.tumble = false;
            if w.frame as i64 - f.last_sp <= i64::from(g.tech.window) {
                f.tech_lock = 0;
                if f.in_dir != 0. {
                    crate::states::roll(w, d, i, f, f.in_dir);
                } else {
                    accept(w, i, Action::Tech);
                    f.set_state(State::Tech);
                    let len = crate::data::global::span(g.tech.invuln_start, g.tech.invuln_end);
                    crate::states::intangible(f, g.tech.invuln_start, len, false);
                }
                w.ring(f.x, f.y - 20., 10., 4., 12);
                w.sfx("tech");
            } else {
                f.set_state(State::Knockdown);
                f.vx *= g.sim.knockdown_drag;
                w.dust(f.x, f.y, 10);
                w.fx.push(crate::Fx::Shake { k: 3. });
            }
        }
        State::Shieldbreak => {
            f.set_state(State::Dizzy);
            f.sd.n = g.shield.dizzy_min.max(g.shield.dizzy_base - f.dmg * g.shield.dizzy_per_percent);
        }
        _ => {}
    }
    if vy > 3. {
        w.dust(f.x, f.y, 5);
    }
    if vy > 8. {
        w.sfx("land");
    }
}
/// Whoever holds or is climbing a ledge, and whether the grabber may knock them off (only a holder who has hung
/// for `trump.minHang` frames).
fn occupant(w: &World, d: &GameData, i: usize, block: usize, side: f64) -> Option<(usize, bool)> {
    let trump = d.global.ledge.trump.as_ref();
    w.fighters.iter().enumerate().find_map(|(j, g)| {
        (j != i && matches!(g.state, State::Ledge | State::Climb) && g.sd.side == side && g.sd.ledge_block == block)
            .then(|| {
                let ok =
                    g.state == State::Ledge && trump.is_some_and(|t| w.frame >= g.sd.entered + u64::from(t.min_hang));
                (j, ok)
            })
    })
}
pub fn try_ledge(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, rising: bool) {
    if f.ground || f.ledge_cd > 0 || f.vy < 0. && !rising || inp.down {
        return;
    }
    let s = &d.stages[w.stage];
    let l = &s.ledge;
    let g = &d.global.ledge;
    for (block, b) in s.blocks.iter().enumerate() {
        for side in [-1., 1.] {
            if !(if side < 0. { b.left_ledge } else { b.right_ledge }) {
                continue;
            }
            let ex = if side < 0. { b.left } else { b.right };
            let dx = f.x - ex;
            let catch = crate::interactions::ledge(&d.fighters[f.data]).and_then(|l| l.catch_point);
            let point = catch.map(|p| crate::interactions::forward_point(f, p));
            let catch_dx = point.map_or(dx, |p| p[0] - ex);
            let dy = point.map_or_else(|| f.y - l.hand_y - (b.top + l.cy), |p| p[1] - (b.top + l.cy));
            let tether = if f.state == State::Attack { crate::moves::phase(d, f).tether } else { None };
            let tethered = tether.is_some_and(|r| {
                let x = (ex - f.x) * f.face;
                let y = f.y - b.top;
                x >= r.x1 && x <= r.x2 && y >= r.y1 && y <= r.y2 && dx * side >= 0.
            });
            if !tethered && (dx * side < -l.reach_in || catch_dx * catch_dx + dy * dy > l.r * l.r) {
                continue;
            }
            let held = occupant(w, d, i, block, side);
            if held.is_some_and(|(_, ok)| !ok) {
                continue;
            }
            if let (Some((j, _)), Some(t)) = (held, &g.trump) {
                // The holder is pushed off (keeping its jumps) and loses its ledge invulnerability.
                let o = &mut w.fighters[j];
                o.set_state(State::Air);
                o.x += side * d.global.sim.ledge_drop_offset;
                o.vx = side * t.push_x;
                o.vy = -t.push_y;
                o.inv = 0;
                o.ledge_cd = t.regrab_cooldown;
                accept(w, j, Action::Trumped);
                accept(w, i, Action::LedgeTrump);
            } else {
                accept(w, i, Action::LedgeGrab);
            }
            f.set_state(State::Ledge);
            f.sd.side = side;
            f.sd.ledge_block = block;
            f.sd.entered = w.frame;
            f.jumps = d.fighters[f.data].stats.air_jumps;
            f.up_b = false;
            f.ad_used = false;
            f.paralyze_used = false;
            f.ff = false;
            f.vx = 0.;
            f.vy = 0.;
            f.face = -side;
            let root = crate::interactions::ledge_hang_root(&d.fighters[f.data], s, ex, b.top, side);
            f.x = root[0];
            f.y = root[1];
            if f.ledge_inv || !g.fresh_invuln {
                f.inv = f.inv.max(g.invuln);
            }
            f.ledge_inv = !g.fresh_invuln;
            w.sfx("land");
            return;
        }
    }
}
