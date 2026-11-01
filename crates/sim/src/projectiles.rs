use crate::combat::{self, ActiveBox};
use crate::{
    Fx, GameData, World,
    data::HitType,
    world::{Fighter, Projectile, State, sign},
};
pub fn spawn(w: &mut World, d: &GameData, i: usize, f: &Fighter, id: &str) {
    let p = &d.fighters[f.data].projectiles[id];
    if w.projs.iter().filter(|p| p.owner == i && p.id == id).count() >= p.max as usize {
        if let Some(s) = &p.full_sfx {
            w.sfx(s.clone());
        }
        return;
    }
    let charge = f.mov.as_ref().map_or(0., |m| f64::from(m.charge));
    let speed = p.charge_scale.as_ref().map_or(1., |c| 1. + charge * c.speed);
    let mut hit = p.hit.clone();
    hit.damage *= p.charge_scale.as_ref().map_or(1., |c| 1. + charge * c.damage);
    w.projs.push(Projectile {
        owner: i,
        fighter_data: f.data,
        id: id.into(),
        x: f.x + f.face * p.spawn[0],
        y: f.y - p.spawn[1],
        vx: f.face * p.vel[0] * speed,
        vy: -p.vel[1] * speed,
        life: p.life as i32,
        r: p.r,
        hit,
        bounces: 0,
        cooldown: 0,
    });
    if let Some(s) = &p.spawn_sfx {
        w.sfx(s.clone());
    }
}
pub fn bat(w: &mut World, d: &GameData, a: usize, bx: &[ActiveBox]) {
    for pi in 0..w.projs.len() {
        let p = &w.projs[pi];
        let Some(bat) = d.fighters[p.fighter_data].projectiles[&p.id].bat.as_ref() else {
            continue;
        };
        if p.cooldown > 0 || p.life <= 0 || p.owner != a && !w.opponents(a, p.owner) {
            continue;
        }
        let Some(b) = bx.iter().find(|b| {
            !matches!(b.hit.kind, HitType::Grab | HitType::Windbox) && libm::hypot(b.x - p.x, b.y - p.y) < b.hit.r + p.r
        }) else {
            continue;
        };
        let sgn = combat::direction(&w.fighters[a], p.x, b.hit.dir);
        let p = &mut w.projs[pi];
        p.owner = a;
        p.vx = sgn * bat.speed;
        p.vy = -bat.vy;
        p.life = bat.life as i32;
        p.bounces = 0;
        p.cooldown = bat.cooldown;
        w.fighters[a].lag = w.fighters[a].lag.max(bat.hitlag);
        let (x, y) = (p.x, p.y);
        w.fx.push(Fx::Burst {
            x,
            y,
            color: crate::data::Color::Rgb(0xffffff),
            player: 0,
            n: 10.,
            speed: 4.,
            life: 14.,
            size: 3.,
            gravity: 0.,
        });
        w.sfx("bonk");
        w.sfx("whack");
    }
}
pub fn step_projectiles(w: &mut World, d: &GameData) {
    let s = &d.stages[w.stage];
    let g = &d.global.sim;
    for pi in 0..w.projs.len() {
        let mut p = w.projs[pi].clone();
        let def = &d.fighters[p.fighter_data].projectiles[&p.id];
        if def.explosion.is_some() && p.life <= 0 {
            continue;
        }
        p.x += p.vx;
        p.life -= 1;
        p.cooldown = p.cooldown.saturating_sub(1);
        if let Some(b) = &def.bounce {
            let py = p.y;
            p.vy = (p.vy + def.gravity).min(def.max_fall.unwrap_or(f64::INFINITY));
            p.y += p.vy;
            if p.vy > 0. {
                for si in 0..s.blocks.len() + s.platforms.len() {
                    let (x1, x2, y) = crate::physics::surface(d, w, si);
                    if py + p.r > y + g.surface_tolerance
                        || p.y + p.r < y
                        || p.x < x1 - g.projectile_pad
                        || p.x > x2 + g.projectile_pad
                    {
                        continue;
                    }
                    let impact = p.vy > def.gravity || b.min_speed > 0.;
                    p.y = y - p.r;
                    p.vy = -b.min_speed.max(p.vy * b.restitution);
                    p.vx *= b.friction;
                    p.bounces += 1;
                    if impact {
                        w.dust(p.x, y, 3);
                        if let Some(s) = &b.sfx {
                            w.sfx(s.clone());
                        }
                    }
                    if p.bounces > b.max_bounces {
                        p.life = p.life.min(b.fizzle_life as i32);
                    }
                    break;
                }
            }
            for s in &s.blocks {
                if p.y + p.r > s.top + g.projectile_wall_top
                    && p.y - p.r < s.bottom
                    && p.x > s.left - p.r
                    && p.x < s.right + p.r
                {
                    p.vx = -p.vx * b.wall_restitution;
                    p.x = if p.x < (s.left + s.right) / 2. { s.left - p.r } else { s.right + p.r };
                }
            }
        } else {
            p.vy = (p.vy + def.gravity).min(def.max_fall.unwrap_or(f64::INFINITY));
            p.y += p.vy;
        }
        for e in &def.trail {
            if e.fires(1, w.frame) {
                crate::moves::effects(w, &e.fx, (p.x, p.y), sign(p.vx), (p.vx, p.vy), p.owner);
            }
        }
        let detonate = def.explosion.as_ref().is_some_and(|e| {
            p.life <= 0
                || def.life.saturating_sub(p.life.max(0) as u32) >= e.arm_ticks
                    && e.proximity.is_some_and(|r| {
                        w.fighters.iter().enumerate().any(|(v, f)| {
                            w.opponents(p.owner, v)
                                && f.hittable()
                                && combat::circ_rect(p.x, p.y, r, combat::hurt(d, f))
                        })
                    })
        });
        if let Some(e) = &def.explosion
            && detonate
        {
            p.r = e.radius;
            crate::moves::effects(w, &e.fx, (p.x, p.y), sign(p.vx), (p.vx, p.vy), p.owner);
            if let Some(s) = &e.sfx {
                w.sfx(s.clone());
            }
        }
        for v in 0..w.fighters.len() {
            if def.explosion.is_some() && !detonate
                || !w.opponents(p.owner, v)
                || !w.fighters[v].hittable()
                || p.life <= 0 && !detonate
                || !combat::circ_rect(p.x, p.y, p.r, combat::hurt(d, &w.fighters[v]))
            {
                continue;
            }
            if let Some(c) = combat::countering(d, &w.fighters[v]).filter(|_| !detonate) {
                p.owner = v;
                p.vx = -p.vx * c.reflect_speed;
                p.life = c.reflect_life as i32;
                if let Some(vy) = def.reflect_vy {
                    p.vy = -vy;
                }
                combat::counter(w, d, v, None, 0.);
                continue;
            }
            let label = Some(def.name.clone());
            if w.fighters[v].state == State::Shield {
                combat::shield_hit(
                    w,
                    d,
                    v,
                    &p.hit,
                    combat::HitSource {
                        attacker: p.owner,
                        direction: sign(p.vx),
                        point: (p.x, p.y),
                        projectile: true,
                        label,
                    },
                );
            } else {
                combat::apply_hit(
                    w,
                    d,
                    v,
                    &p.hit,
                    combat::HitSource {
                        attacker: p.owner,
                        direction: sign(p.vx),
                        point: (p.x, p.y),
                        projectile: true,
                        label,
                    },
                );
            }
            if !detonate {
                p.life = 0;
            }
        }
        if detonate {
            p.life = 0;
        }
        for qi in 0..w.projs.len() {
            let q = &w.projs[qi];
            if qi != pi
                && w.opponents(p.owner, q.owner)
                && q.life > 0
                && p.life > 0
                && libm::hypot(q.x - p.x, q.y - p.y) < p.r + q.r
            {
                let x = (p.x + q.x) / 2.;
                w.fx.push(Fx::Burst {
                    x,
                    y: p.y,
                    color: crate::data::Color::Rgb(0xffffff),
                    player: 0,
                    n: 14.,
                    speed: 5.,
                    life: 18.,
                    size: 3.,
                    gravity: 0.,
                });
                p.life = 0;
                w.projs[qi].life = 0;
            }
        }
        w.projs[pi] = p;
    }
    let b = &s.blast;
    w.projs.retain(|p| p.life > 0 && p.x > b.left && p.x < b.right && p.y < b.bottom);
}
