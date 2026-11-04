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
    let mut radius = p.r;
    let mut life = p.life;
    if let Some(c) = &p.charge_scale {
        hit.bkb *= 1. + charge * c.bkb;
        radius *= 1. + charge * c.radius;
        life = libm::round(f64::from(life) * (1. + charge * c.life)) as u32;
        if let Some(rehit) = hit.rehit {
            hit.rehit = Some(libm::round((f64::from(rehit) + charge * c.rehit).max(1.)) as u32);
        }
        if c.angle_to.is_some() || c.ground_angle_to.is_some() {
            let max =
                crate::moves::timeline(d, f).iter().filter_map(|p| p.charge.as_ref()).map(|c| c.max).max().unwrap_or(1);
            let fraction = (charge / f64::from(max.max(1))).min(1.);
            if let Some(angle) = c.angle_to {
                hit.angle += (angle - hit.angle) * fraction;
            }
            if let Some(angle) = c.ground_angle_to {
                let start = hit.ground_angle.unwrap_or(hit.angle);
                hit.ground_angle = Some(start + (angle - start) * fraction);
            }
        }
    }
    w.projs.push(Projectile {
        owner: i,
        fighter_data: f.data,
        id: id.into(),
        x: f.x + f.face * p.spawn[0],
        y: f.y - p.spawn[1],
        vx: f.face * p.vel[0] * speed,
        vy: -p.vel[1] * speed,
        life: life as i32,
        r: radius,
        hit,
        bounces: 0,
        cooldown: 0,
        age: 0,
        born: w.frame,
        surf: None,
        released: None,
        automatic: false,
        stage: 0,
        face: f.face,
        damage: 0.,
        hits: vec![],
        damage_hits: vec![],
        power: 1.,
        speed: 1.,
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
            !matches!(b.hit.kind, HitType::Grab | HitType::Windbox)
                && if b.hit.geometry.is_none() {
                    libm::hypot(b.x - p.x, b.y - p.y) < b.hit.r + p.r
                } else {
                    b.shape.overlaps(crate::geometry::Shape::Circle { center: [p.x, p.y], radius: p.r })
                }
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
        p.surf = None;
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
        if d.fighters[w.projs[pi].fighter_data].projectiles[&w.projs[pi].id].article.is_some() {
            crate::articles::step(w, d, pi);
            continue;
        }
        let mut p = w.projs[pi].clone();
        let def = &d.fighters[p.fighter_data].projectiles[&p.id];
        if def.explosion.is_some() && p.life <= 0 {
            continue;
        }
        if let Some(id) = p.surf {
            let (x1, x2, y) = crate::physics::surface(d, w, id);
            if let Some(platform) = id.checked_sub(s.blocks.len()).and_then(|i| s.platforms.get(i)) {
                let (old_x, _, old_y) = platform.position(w.frame.saturating_sub(1) as f64);
                p.x += x1 - old_x;
                p.y += y - old_y;
            }
            if p.x < x1 || p.x > x2 || p.vx != 0. || p.vy != 0. {
                p.surf = None;
            }
        }
        p.age = p.age.saturating_add(1);
        p.x += p.vx;
        if !def.explosion.as_ref().is_some_and(|e| e.manual) {
            p.life -= 1;
        }
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
                    if p.vx == 0. && p.vy == 0. {
                        p.surf = Some(si);
                    }
                    // A settled gadget is still supported; gravity contact is not another bounce.
                    if impact {
                        p.bounces += 1;
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
            !e.manual
                && (p.life <= 0
                    || p.age >= e.arm_ticks
                        && e.proximity.is_some_and(|r| {
                            w.fighters.iter().enumerate().any(|(v, f)| {
                                w.opponents(p.owner, v)
                                    && f.hittable()
                                    && combat::circle_hurt(d, f, w.frame, p.x, p.y, r)
                            })
                        }))
        });
        contacts(w, d, &mut p, detonate);
        for qi in 0..w.projs.len() {
            let q = &w.projs[qi];
            if qi != pi
                && w.opponents(p.owner, q.owner)
                && q.life > 0
                && d.fighters[q.fighter_data].projectiles[&q.id].article.is_none()
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

/// Immediately blast live owned gadgets in projectile order. No movement, aging or fuse tick.
pub fn detonate(w: &mut World, d: &GameData, owner: usize, ids: &[String]) {
    for pi in 0..w.projs.len() {
        let mut p = w.projs[pi].clone();
        if p.owner == owner
            && p.life > 0
            && (ids.is_empty() || ids.contains(&p.id))
            && d.fighters[p.fighter_data].projectiles[&p.id].explosion.is_some()
        {
            contacts(w, d, &mut p, true);
            w.projs[pi] = p;
        }
    }
    w.projs.retain(|p| p.life > 0);
}

fn contacts(w: &mut World, d: &GameData, p: &mut Projectile, detonate: bool) {
    let def = &d.fighters[p.fighter_data].projectiles[&p.id];
    if let Some(e) = &def.explosion
        && detonate
    {
        p.r = e.radius;
        crate::moves::effects(w, &e.fx, (p.x, p.y), sign(p.vx), (p.vx, p.vy), p.owner);
        if let Some(s) = &e.sfx {
            w.sfx(s.clone());
        }
    }
    if detonate || def.explosion.is_none() && p.life > 0 {
        crate::articles::projectile_contacts(w, d, p, detonate);
    }
    for v in 0..w.fighters.len() {
        if !detonate
            && w.opponents(p.owner, v)
            && let Some(c) = combat::reflecting(d, &w.fighters[v])
            && libm::hypot(p.x - w.fighters[v].x, p.y - (w.fighters[v].y - c.height)) < p.r + c.radius
        {
            p.owner = v;
            p.face = -p.face;
            p.vx = -p.vx * c.speed;
            p.hit.damage *= c.damage;
            continue;
        }
        if def.explosion.is_some() && !detonate
            || !w.opponents(p.owner, v)
            || !w.fighters[v].hittable()
            || !p.hit.targets(w.fighters[v].ground)
            || p.life <= 0 && !detonate
            || p.hit.rehit.is_some_and(|n| p.hits.iter().any(|&(i, at)| i == v && p.age.saturating_sub(at) < n))
            || !combat::circle_hurt(d, &w.fighters[v], w.frame, p.x, p.y, p.r)
        {
            continue;
        }
        if !p.hit.unblockable && detonate && combat::countering(d, &w.fighters[v]).is_some() {
            combat::counter(w, d, v, None, 0.);
            continue;
        }
        if !p.hit.unblockable
            && let Some(c) = combat::countering(d, &w.fighters[v])
        {
            p.owner = v;
            p.surf = None;
            p.vx = -p.vx * c.reflect_speed;
            p.life = c.reflect_life as i32;
            if let Some(vy) = def.reflect_vy {
                p.vy = -vy;
            }
            combat::counter(w, d, v, None, 0.);
            continue;
        }
        let direction = if detonate {
            let away = sign(w.fighters[v].x - p.x);
            if away != 0. {
                away
            } else if p.vx != 0. {
                sign(p.vx)
            } else {
                w.fighters[p.owner].face
            }
        } else {
            sign(p.vx)
        };
        let label = Some(def.name.clone());
        if p.hit.unblockable {
            // Command-grab hits ignore guards, parries and counters, exactly like the melee path, but they
            // still take part in the usual contact bookkeeping below (rehit intervals, single-hit expiry).
            combat::apply_hit(
                w,
                d,
                v,
                &p.hit,
                combat::HitSource { attacker: p.owner, direction, point: (p.x, p.y), projectile: true, label },
            );
        } else if combat::parrying(d, &w.fighters[v]) {
            combat::parry(w, d, v, None);
        } else if w.fighters[v].state == State::Shield {
            combat::shield_hit(
                w,
                d,
                v,
                &p.hit,
                combat::HitSource { attacker: p.owner, direction, point: (p.x, p.y), projectile: true, label },
            );
        } else {
            combat::apply_hit(
                w,
                d,
                v,
                &p.hit,
                combat::HitSource { attacker: p.owner, direction, point: (p.x, p.y), projectile: true, label },
            );
        }
        // Rehitting hazards stay in play for their next contact window; single hits are consumed.
        if !detonate {
            p.hits.push((v, p.age));
            if p.hit.rehit.is_none() {
                p.life = 0;
            }
        }
    }
    if detonate {
        p.life = 0;
    }
}
