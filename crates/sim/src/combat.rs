use crate::moves;
use crate::{
    Fx, GameData, World,
    data::{Color, HitType, Hitbox, LaunchDir, Rect},
    world::{Fighter, State, Trace, sign},
};
#[derive(Clone, Debug)]
pub struct ActiveBox {
    pub hit: Hitbox,
    pub x: f64,
    pub y: f64,
}
pub fn hurt(d: &GameData, f: &Fighter) -> Rect {
    let h = &d.fighters[f.data].hurtboxes;
    let r = if f.state == State::Knockdown { h.knockdown } else { h.default };
    let (x1, x2) = (f.x + r.x1 * f.face, f.x + r.x2 * f.face);
    Rect { x1: x1.min(x2), x2: x1.max(x2), y1: f.y - r.y2, y2: f.y - r.y1 }
}
pub fn circ_rect(x: f64, y: f64, r: f64, b: Rect) -> bool {
    let dx = x - x.clamp(b.x1, b.x2);
    let dy = y - y.clamp(b.y1, b.y2);
    dx * dx + dy * dy <= r * r
}
pub fn active_boxes(d: &GameData, f: &Fighter) -> Vec<ActiveBox> {
    let g = &d.global;
    let (boxes, t, cd, charge) = if f.state == State::Attack
        && let Some(m) = &f.mov
    {
        (&moves::phase(d, f).hitboxes, m.t, m.counter_damage, m.charge)
    } else if f.state == State::Getup
        && f.sd.kind == "atk"
        && f.t >= g.knockdown.attack_active[0]
        && f.t <= g.knockdown.attack_active[1]
    {
        (&g.knockdown.attack, f.t, 0., 0)
    } else if f.state == State::Climb
        && f.sd.kind == "attack"
        && f.t >= g.ledge.attack_active[0]
        && f.t <= g.ledge.attack_active[1]
    {
        (&g.ledge.attack_hit, f.t, 0., 0)
    } else {
        return vec![];
    };
    let scale = (f.state == State::Attack && f.mov.is_some()).then(|| moves::definition(d, f).charge_damage).flatten();
    let mult = 1.
        + scale
            .map_or_else(|| f64::from(charge) / g.knockback.charge_damage_divisor, |scale| f64::from(charge) * scale);
    boxes
        .iter()
        .filter(|b| b.active(t) && charge >= b.min_charge && (!b.counter_damage || cd > 0.))
        .map(|b| {
            let (x, y, r) = b.geom_at(t);
            let mut hit = b.clone();
            hit.x = x;
            hit.y = y;
            hit.r = r;
            hit.damage = (if b.counter_damage { cd } else { b.damage }) * mult;
            hit.bkb *= 1. + (mult - 1.) * g.knockback.charge_kb_ratio;
            ActiveBox { hit, x: f.x + x * f.face, y: f.y - y }
        })
        .collect()
}
pub fn countering<'a>(d: &'a GameData, f: &Fighter) -> Option<&'a crate::data::moves::Counter> {
    if f.state != State::Attack {
        return None;
    }
    let m = f.mov.as_ref()?;
    moves::phase(d, f).counter.as_ref().filter(|c| m.t >= c.from && m.t <= c.to)
}
pub fn attack_label(d: &GameData, f: &Fighter) -> String {
    match f.state {
        State::Attack => moves::definition(d, f).name.clone(),
        State::Throw => format!("Throw ({})", f.sd.kind),
        State::Getup => "Getup Attack".into(),
        State::Climb => "Ledge Attack".into(),
        _ => f.state.name().into(),
    }
}
fn trace(w: &mut World, a: usize, v: usize, label: String, on_shield: bool, dmg: f64, armored: bool) {
    let p = w.trace.as_ref();
    let chained =
        p.is_some_and(|p| !armored && !p.armored && !on_shield && !p.on_shield && p.defender == v && p.d_free < 0);
    let (combo, combo_dmg) = if chained {
        let p = p.unwrap();
        (p.combo + 1, p.combo_dmg + dmg)
    } else {
        (1, dmg)
    };
    w.trace = Some(Trace {
        attacker: a,
        defender: v,
        label,
        on_shield,
        armored,
        hit_frame: w.frame,
        a_free: -1,
        d_free: -1,
        done: false,
        combo,
        combo_dmg,
    });
}
pub fn release_grab(w: &mut World, d: &GameData, i: usize) {
    let f = &w.fighters[i];
    let face = f.face;
    let r = &d.global.sim;
    if matches!(f.state, State::Hold | State::Throw)
        && let Some(v) = f.sd.victim
        && w.fighters[v].state == State::Grabbed
    {
        let v = &mut w.fighters[v];
        v.set_state(State::Air);
        v.vy = -r.release_vy;
        v.vx = face * r.release_vx;
        v.ground = false;
        v.surf = None;
    }
    if w.fighters[i].state == State::Grabbed
        && let Some(by) = w.fighters[i].sd.by
        && matches!(w.fighters[by].state, State::Hold | State::Throw)
    {
        w.fighters[by].set_state(State::Idle);
    }
}
pub fn knockback(d: &GameData, b: &Hitbox, pct: f64, weight: f64) -> f64 {
    (if b.set_kb { b.bkb } else { b.bkb + pct * b.kbg * d.global.knockback.kb_percent_scale }) * weight
}
pub fn hitstun_of(d: &GameData, b: &Hitbox, kb: f64) -> i32 {
    let h = &d.global.hitstun;
    libm::floor((libm::floor(kb * h.per_knockback) + h.base) * b.hitstun_mult + b.hitstun_add) as i32
}
pub fn hitlag_of(d: &GameData, b: &Hitbox, kb: f64) -> u32 {
    let h = &d.global.hitlag;
    libm::floor(h.max.min(libm::floor(h.base + b.damage * h.per_damage + kb * h.per_knockback)) * b.hitlag_mult) as u32
}
pub fn launch_angle(d: &GameData, b: &Hitbox, kb: f64, ground: bool) -> f64 {
    match b.kind {
        HitType::Spike => -b.angle.abs(),
        HitType::Sakurai => {
            let s = &d.global.hit_types.sakurai;
            if !ground {
                s.air
            } else if kb > d.global.knockback.tumble_threshold {
                s.ground_high
            } else {
                s.ground_low
            }
        }
        _ => b.angle,
    }
}
pub struct HitSource {
    pub attacker: usize,
    pub direction: f64,
    pub point: (f64, f64),
    pub projectile: bool,
    pub label: Option<String>,
}
fn armors_hit(d: &GameData, f: &Fighter, damage: f64) -> bool {
    f.state == State::Attack && f.mov.is_some() && moves::phase(d, f).armor_threshold.is_some_and(|n| damage <= n)
}
pub(crate) fn credit_damage(w: &mut World, d: &GameData, attacker: usize, victim: usize, damage: f64) {
    let f = &mut w.fighters[victim];
    f.dmg = d.global.sim.max_damage.min(f.dmg + damage);
    f.last_hit_by = Some(attacker);
    w.hits.push(crate::HitRecord { attacker: Some(attacker), victim, damage, shielded: false });
}
pub fn apply_hit(w: &mut World, d: &GameData, v: usize, b: &Hitbox, source: HitSource) {
    let HitSource { attacker, direction: sgn, point, projectile: proj, label } = source;
    let g = &d.global;
    let ht = &g.hit_types;
    let grounded = w.fighters[v].ground;
    let armored = armors_hit(d, &w.fighters[v], b.damage);
    release_grab(w, d, v);
    let label = label.unwrap_or_else(|| attack_label(d, &w.fighters[attacker]));
    let (avx, avy) = (w.fighters[attacker].vx, w.fighters[attacker].vy);
    credit_damage(w, d, attacker, v, b.damage);
    let vic = &mut w.fighters[v];
    let kb = knockback(d, b, vic.dmg, d.fighters[vic.data].stats.weight);
    let hl = hitlag_of(d, b, kb);
    vic.lag = if b.kind == HitType::Electric {
        libm::floor(f64::from(hl) * ht.electric.victim_hitlag_mult) as u32
    } else {
        hl
    };
    vic.flash = vic.lag.saturating_add(2);
    vic.hud_shake = 12;
    if !armored {
        vic.ff = false;
        vic.up_b = false;
        vic.ad_used = false;
    }
    if !proj {
        w.fighters[attacker].lag = hl;
    }
    trace(w, attacker, v, label, false, b.damage, armored);
    let vic = &mut w.fighters[v];
    let (px, py) = point;
    let hx = (px + vic.x) / 2.;
    let hy = (py + vic.y - 40.) / 2.;
    if armored {
        w.ring(hx, hy, 12., 2., 10);
        w.fx.push(Fx::Sfx { name: "hit".into(), arg: Some(kb), x: Some(hx) });
        return;
    }
    if b.kind == HitType::Trip && grounded {
        vic.set_state(State::Knockdown);
        vic.vx = sgn * ht.trip.slide;
        vic.vy = 0.;
        vic.hitstun = 0;
        vic.tumble = false;
        let (x, y) = (vic.x, vic.y);
        w.fx.push(Fx::Burst {
            x: hx,
            y: hy,
            color: Color::Rgb(0xffffff),
            player: 0,
            n: 8.,
            speed: 3.,
            life: 14.,
            size: 3.,
            gravity: 0.,
        });
        w.dust(x, y, 10);
        w.fx.push(Fx::Shake { k: 3. });
        w.fx.push(Fx::Sfx { name: "hit".into(), arg: Some(kb), x: Some(hx) });
        return;
    }
    if b.kind == HitType::Autolink {
        let h = d.fighters[vic.data].hurtboxes.default;
        let cx = vic.x + (h.x1 + h.x2) / 2. * vic.face;
        let cy = vic.y - (h.y1 + h.y2) / 2.;
        vic.vx = (px - cx) * ht.autolink.pull + avx * ht.autolink.momentum;
        vic.vy = (py - cy) * ht.autolink.pull + avy * ht.autolink.momentum;
    } else {
        let angle = launch_angle(d, b, kb, grounded) * std::f64::consts::PI / 180.;
        vic.vx = libm::cos(angle) * kb * sgn;
        vic.vy = -libm::sin(angle) * kb;
        if grounded && vic.vy > 0. {
            vic.vy = -vic.vy * g.knockback.ground_bounce;
        }
    }
    vic.ground = false;
    vic.surf = None;
    vic.set_state(State::Hitstun);
    vic.hitstun = hitstun_of(d, b, kb);
    vic.tumble = b.kind != HitType::Autolink && kb > g.knockback.tumble_threshold;
    let (x, y, lag) = (vic.x, vic.y, vic.lag);
    w.fx.push(Fx::Burst {
        x: hx,
        y: hy,
        color: Color::Rgb(0xffffff),
        player: 0,
        n: 8. + kb * 0.6,
        speed: 4. + kb * 0.25,
        life: 18.,
        size: 4.,
        gravity: 0.,
    });
    w.fx.push(Fx::Burst {
        x: hx,
        y: hy,
        color: if b.kind == HitType::Electric { Color::Rgb(0xffe14a) } else { Color::Player },
        player: attacker,
        n: 6. + kb * 0.4,
        speed: 3. + kb * 0.2,
        life: 24.,
        size: 3.,
        gravity: 0.,
    });
    if b.kind == HitType::Electric {
        for k in 0..10 {
            w.part(
                (x, y - 40.),
                (f64::from(k % 5 - 2) * 2.2, f64::from(k % 3 - 1) * 2.5),
                f64::from(lag) + 6.,
                Color::Rgb(0xfff27a),
                3.,
                0.,
            );
        }
    }
    if b.kind == HitType::Spike {
        for k in 0..8 {
            w.part(
                (x + (f64::from(k) - 3.5) * 4., y - 40.),
                (0., f64::from(6 + k % 3)),
                18.,
                Color::Rgb(0xffb35a),
                4.,
                0.,
            );
        }
        w.fx.push(Fx::Ring { x: hx, y: hy, r: 10., grow: 5., life: 16, color: Color::Rgb(0xffb35a), player: 0 });
    }
    w.ring(hx, hy, 6., 3. + kb * 0.15, 14);
    w.fx.push(Fx::Shake { k: kb * 0.45 });
    w.fx.push(Fx::Sfx { name: "hit".into(), arg: Some(kb), x: Some(hx) });
}
pub fn shield_break(w: &mut World, d: &GameData, i: usize) {
    let f = &mut w.fighters[i];
    f.sh = 0.;
    f.set_state(State::Shieldbreak);
    f.vy = -d.global.shield.break_launch;
    f.vx = 0.;
    f.ground = false;
    f.surf = None;
    let (x, y) = (f.x, f.y);
    w.fx.push(Fx::Burst {
        x,
        y: y - 40.,
        color: Color::Rgb(0xccddff),
        player: i,
        n: 30.,
        speed: 8.,
        life: 30.,
        size: 4.,
        gravity: 0.1,
    });
    w.fx.push(Fx::Ring { x, y: y - 38., r: 30., grow: 6., life: 18, color: Color::Player, player: i });
    w.fx.push(Fx::Shake { k: 10. });
    w.fx.push(Fx::Sfx { name: "shieldBreak".into(), arg: None, x: Some(x) });
}
pub fn shield_hit(w: &mut World, d: &GameData, v: usize, b: &Hitbox, source: HitSource) {
    let HitSource { attacker: a, direction: sgn, projectile: proj, label, .. } = source;
    let s = &d.global.shield;
    let label = label.unwrap_or_else(|| attack_label(d, &w.fighters[a]));
    let f = &mut w.fighters[v];
    f.sh -= b.damage * s.damage_mult * b.shield_dmg_mult;
    f.sd.stun = libm::floor((b.damage * s.stun_per_damage + s.stun_base) * b.shield_stun_mult) as i32;
    f.vx = sgn * (b.damage * s.push_per_damage + s.push_base);
    let hl = libm::floor(s.hitlag_base + b.damage * s.hitlag_per_damage) as u32;
    f.lag = hl;
    if !proj {
        let a = &mut w.fighters[a];
        a.lag = hl;
        if a.ground {
            a.vx = -a.face * s.attacker_pushback;
        }
    }
    w.hits.push(crate::HitRecord { attacker: Some(a), victim: v, damage: b.damage, shielded: true });
    trace(w, a, v, label, true, b.damage, false);
    let f = &w.fighters[v];
    w.ring(f.x, f.y - 38., 20., 2.5, 10);
    w.fx.push(Fx::Sfx { name: "shieldHit".into(), arg: Some(b.damage), x: Some(w.fighters[v].x) });
    if w.fighters[v].sh <= 0. {
        shield_break(w, d, v);
    }
}
pub fn start_hold(w: &mut World, d: &GameData, a: usize, v: usize) {
    release_grab(w, d, v);
    let timer = d.global.grab.hold_base + w.fighters[v].dmg * d.global.grab.hold_per_percent;
    let f = &mut w.fighters[a];
    f.set_state(State::Hold);
    f.sd.victim = Some(v);
    f.sd.timer = timer;
    f.vx = 0.;
    let (x, y, face, ground) = (f.x, f.y, f.face, f.ground);
    let v = &mut w.fighters[v];
    v.set_state(State::Grabbed);
    v.sd.by = Some(a);
    v.vx = 0.;
    v.vy = 0.;
    v.face = -face;
    v.hitstun = 0;
    v.tumble = false;
    v.y = y;
    v.x = x + face * d.global.sim.grab_offset;
    v.ground = ground;
    w.sfx("grab");
}
pub fn counter(w: &mut World, d: &GameData, v: usize, a: Option<usize>, damage: f64) {
    let Some(c) = countering(d, &w.fighters[v]) else {
        return;
    };
    let c = c.clone();
    let mut f = w.fighters[v].clone();
    f.mov.as_mut().unwrap().counter_damage = if a.is_some() { c.min_damage.max(damage * c.damage_mult) } else { 0. };
    if let Some(a) = a {
        let dx = w.fighters[a].x - f.x;
        if dx != 0. {
            f.face = sign(dx);
        }
        w.fighters[a].lag = c.attacker_freeze;
        w.fighters[a].flash = 4;
    }
    f.inv = f.inv.max(c.invuln);
    f.lag = 0;
    moves::trigger(w, d, v, &mut f, crate::data::Trigger::Counter);
    w.fighters[v] = f;
}
pub fn direction(f: &Fighter, x: f64, dir: LaunchDir) -> f64 {
    match dir {
        LaunchDir::Away => {
            let s = sign(x - f.x);
            if s == 0. { f.face } else { s }
        }
        LaunchDir::Back => -f.face,
        LaunchDir::Forward => f.face,
    }
}
pub fn hit_pass(w: &mut World, d: &GameData) {
    for a in 0..w.fighters.len() {
        if w.fighters[a].lag > 0 || !matches!(w.fighters[a].state, State::Attack | State::Getup | State::Climb) {
            continue;
        }
        let bx = active_boxes(d, &w.fighters[a]);
        if bx.is_empty() {
            continue;
        }
        crate::projectiles::bat(w, d, a, &bx);
        for v in 0..w.fighters.len() {
            if !w.opponents(a, v) || !w.fighters[v].hittable() {
                continue;
            }
            let bx = active_boxes(d, &w.fighters[a]);
            let t = w.fighters[a].mov.as_ref().map_or(w.fighters[a].t, |m| m.t);
            let hb = hurt(d, &w.fighters[v]);
            let mut wind = None;
            let mut hit = false;
            for b in &bx {
                if !b.hit.targets(w.fighters[v].ground) || !circ_rect(b.x, b.y, b.hit.r, hb) {
                    continue;
                }
                if b.hit.kind == HitType::Windbox {
                    if wind.is_none() {
                        wind = Some(b);
                    }
                    continue;
                }
                let f = &mut w.fighters[a];
                if !f.sd.can_hit(v, t, b.hit.rehit) {
                    continue;
                }
                f.sd.record_hit(v, t);
                hit = true;
                let sgn = direction(&w.fighters[a], w.fighters[v].x, b.hit.dir);
                if b.hit.kind == HitType::Grab {
                    if !matches!(w.fighters[v].state, State::Ledge | State::Climb | State::Knockdown)
                        && (w.fighters[v].y - w.fighters[a].y).abs() < d.global.grab.height_tolerance
                    {
                        start_hold(w, d, a, v);
                    }
                } else if countering(d, &w.fighters[v]).is_some() {
                    counter(w, d, v, Some(a), b.hit.damage);
                } else if w.fighters[v].state == State::Shield {
                    shield_hit(
                        w,
                        d,
                        v,
                        &b.hit,
                        HitSource { attacker: a, direction: sgn, point: (b.x, b.y), projectile: false, label: None },
                    );
                } else {
                    apply_hit(
                        w,
                        d,
                        v,
                        &b.hit,
                        HitSource { attacker: a, direction: sgn, point: (b.x, b.y), projectile: false, label: None },
                    );
                }
                let mut f = w.fighters[a].clone();
                if f.mov.is_some() {
                    moves::trigger(w, d, a, &mut f, crate::data::Trigger::Hit);
                    w.fighters[a] = f;
                }
                break;
            }
            if !hit
                && let Some(b) = wind
                && !armors_hit(d, &w.fighters[v], 0.)
            {
                let sgn = direction(&w.fighters[a], w.fighters[v].x, b.hit.dir);
                let k = &d.global.hit_types.windbox;
                let angle = b.hit.angle * std::f64::consts::PI / 180.;
                let p = b.hit.bkb * k.push * d.fighters[w.fighters[v].data].stats.weight;
                let f = &mut w.fighters[v];
                f.vx = (f.vx + libm::cos(angle) * p * sgn).clamp(-k.max_speed, k.max_speed);
                let dy = -libm::sin(angle) * p;
                if dy < 0. || !f.ground {
                    f.vy = (f.vy + dy).clamp(-k.max_speed, k.max_speed);
                }
            }
        }
    }
}
