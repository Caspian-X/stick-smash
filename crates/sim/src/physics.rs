use crate::data::Trigger;
use crate::{
    GameData, World,
    input::Intent,
    world::{Fighter, State},
};

pub fn surface(data: &GameData, w: &World, id: usize) -> (f64, f64, f64) {
    let s = &data.stages[w.stage];
    if id == 0 {
        (s.ground.left, s.ground.right, s.ground.top)
    } else {
        let p = &s.platforms[id - 1];
        (p.x1, p.x2, p.y)
    }
}
pub fn integrate(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, edge_stop: bool) {
    let (was_g, vy, py, surf) = (f.ground, f.vy, f.y, f.surf);
    let g = &d.global.sim;
    let s = &d.stages[w.stage];
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
        let pad = if surf == 0 { g.main_pad } else { g.platform_pad };
        if f.x >= x1 - pad && f.x <= x2 + pad && !(f.drop > 0 && surf != 0) {
            f.y = y;
            f.vy = 0.;
            return;
        }
    }
    f.y += f.vy;
    f.ground = false;
    f.surf = None;
    if vy >= 0. && f.drop == 0 {
        for (n, p) in s.platforms.iter().enumerate() {
            if py <= p.y + g.surface_tolerance && f.y >= p.y && f.x >= p.x1 - g.platform_pad && f.x <= p.x2 + g.platform_pad {
                f.y = p.y;
                f.vy = 0.;
                f.ground = true;
                f.surf = Some(n + 1);
                break;
            }
        }
    }
    if !f.ground {
        let b = &d.fighters[f.data].body;
        let s = &s.ground;
        let (hx1, hx2, hy1, hy2) = (f.x - b.half_width, f.x + b.half_width, f.y - b.height, f.y);
        let xs = (if f.x < (s.left + s.right) / 2. { hx2 } else { hx1 }).clamp(s.left, s.right);
        let sb = s.bottom + ((xs - s.left).min(s.right - xs) / s.under_inset).min(1.) * s.under_depth;
        if hx2 > s.left && hx1 < s.right && hy2 > s.top && hy1 < sb {
            let over = (hy1 - s.bottom).max(0.) * s.under_inset / s.under_depth;
            let (up, dn, lf, rt) = (hy2 - s.top, sb - hy1, hx2 - s.left - over, s.right - hx1 - over);
            let mn = up.min(dn).min(lf).min(rt);
            if mn == up {
                f.y = s.top;
                f.vy = 0.;
                f.ground = true;
                f.surf = Some(0);
            } else if mn == dn {
                f.y = sb + b.height;
                if f.vy < 0. {
                    f.vy = 0.;
                }
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
    if f.ground && !was_g {
        on_land(w, d, i, f, vy);
    }
}
pub fn fall(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, edge: bool, cap: Option<f64>) {
    let s = &d.fighters[f.data].stats;
    f.vy = (f.vy + s.gravity).min(cap.unwrap_or(s.fall_speed));
    integrate(w, d, i, f, edge);
}
pub fn fast_fall(d: &GameData, f: &mut Fighter, inp: Intent) {
    let m = &d.global.movement;
    if inp.down_p && f.vy > m.fast_fall_min_vy {
        f.ff = true;
        f.vy = f.vy.max(m.fast_fall_kick);
    }
}
pub fn air_drift(d: &GameData, f: &mut Fighter, dir: f64, acc: f64, drag: f64) {
    let max = d.fighters[f.data].stats.air_speed;
    if dir != 0. && acc != 0. && f.vx * dir < max {
        f.vx = (f.vx * dir + acc).min(max) * dir;
    } else {
        f.vx *= drag;
    }
}
pub fn air_phys(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, dir: f64, acc: f64) {
    let m = &d.global.movement;
    let s = &d.fighters[f.data].stats;
    if dir != 0. && acc != 0. && f.vx * dir < s.air_speed {
        air_drift(d, f, dir, acc, 1.);
    } else {
        f.vx *= if dir != 0. { m.air_drag_held } else { m.air_drag };
    }
    fast_fall(d, f, inp);
    fall(w, d, i, f, false, Some(if f.ff { s.fast_fall_speed } else { s.fall_speed }));
}
pub fn landing(f: &mut Fighter, n: u32) {
    f.set_state(State::Landing);
    f.sd.n = f64::from(n);
}
pub fn on_land(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, vy: f64) {
    let g = &d.global;
    f.jumps = d.fighters[f.data].stats.air_jumps;
    f.up_b = false;
    f.ad_used = false;
    f.ff = false;
    f.flip_t = 0;
    match f.state {
        State::Air => {
            if vy > g.landing.soft_land_speed {
                landing(f, g.landing.hard);
            } else {
                f.set_state(State::Idle);
            }
        }
        State::Helpless => landing(f, g.landing.helpless),
        State::Airdodge => landing(f, if f.sd.dirl { g.dodge.dir_air_dodge_landing_lag } else { g.dodge.air_dodge_landing_lag }),
        State::Attack => {
            crate::moves::trigger(w, d, i, f, Trigger::Land);
        }
        State::Hitstun if f.tumble && vy > g.sim.tech_min_speed => {
            f.hitstun = 0;
            f.tumble = false;
            if w.frame as i64 - f.last_sp <= i64::from(g.tech.window) {
                if f.in_dir != 0. {
                    crate::states::roll(w, d, f, f.in_dir, true, false);
                } else {
                    f.set_state(State::Tech);
                    f.inv = g.tech.invuln;
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
pub fn try_ledge(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, rising: bool) {
    if f.ground || f.ledge_cd > 0 || f.vy < 0. && !rising || inp.down {
        return;
    }
    let s = &d.stages[w.stage];
    let l = &s.ledge;
    for side in [-1., 1.] {
        let ex = if side < 0. { s.ground.left } else { s.ground.right };
        let dx = f.x - ex;
        let dy = f.y - l.hand_y - (s.ground.top + l.cy);
        if dx * side < -l.reach_in
            || dx * dx + dy * dy > l.r * l.r
            || w.fighters.iter().enumerate().any(|(j, g)| j != i && matches!(g.state, State::Ledge | State::Climb) && g.sd.side == side)
        {
            continue;
        }
        f.set_state(State::Ledge);
        f.sd.side = side;
        f.jumps = d.fighters[f.data].stats.air_jumps;
        f.up_b = false;
        f.ad_used = false;
        f.ff = false;
        f.vx = 0.;
        f.vy = 0.;
        f.x = ex + side * l.hang_x;
        f.y = s.ground.top + l.hang_y;
        f.face = -side;
        f.inv = f.inv.max(d.global.ledge.invuln);
        w.sfx("land");
        return;
    }
}
