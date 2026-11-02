//! Controlled projectiles: construction/travel, release, then an independent attack timeline.
use crate::{
    GameData, World, combat,
    input::Intent,
    moves,
    world::{State, sign},
};

pub fn control(w: &mut World, d: &GameData, owner: usize, inp: Intent) -> bool {
    let mut consumed = false;
    for pi in 0..w.projs.len() {
        let p = &mut w.projs[pi];
        let def = &d.fighters[p.fighter_data].projectiles[&p.id];
        let Some(a) = &def.article else { continue };
        if p.owner != owner || p.released.is_some() || p.life <= 0 {
            continue;
        }
        let f = &w.fighters[owner];
        let controlling = f.mov.as_ref().is_some_and(|m| m.slot == a.slot);
        if controlling && f.lag == 0 && a.steer != 0. {
            p.vy = -inp.y * a.steer;
        }
        let manual = if a.release_on_up { !inp.sp_held || !controlling } else { inp.sp };
        if p.age < a.min_release || !manual || f.lag > 0 {
            continue;
        }
        p.stage = a.stages.iter().rposition(|s| s.from <= p.age).unwrap_or(0);
        p.released = Some(p.age);
        p.vx = 0.;
        p.vy = 0.;
        // Losing control detonates a guided shot without accepting or erasing the victim's next command.
        if !controlling && a.release_on_up {
            continue;
        }
        let mut f = w.fighters[owner].clone();
        if !controlling && !a.release_on_up {
            if !f.actionable(&d.global) {
                p.released = None;
                continue;
            }
            moves::start_move(w, d, owner, &mut f, &a.slot);
        }
        moves::trigger(w, d, owner, &mut f, crate::data::Trigger::ArticleRelease);
        f.buffer.accept();
        w.fighters[owner] = f;
        crate::actions::accept(w, owner, crate::actions::Action::Move(a.slot.clone()));
        consumed |= inp.sp;
    }
    consumed
}

pub fn step(w: &mut World, d: &GameData, pi: usize) {
    let mut p = w.projs[pi].clone();
    let a = d.fighters[p.fighter_data].projectiles[&p.id].article.as_ref().unwrap();
    if p.life <= 0 || p.age == 0 && p.born == w.frame {
        return;
    }
    if matches!(w.fighters[p.owner].state, State::Dead | State::Respawn | State::Out) {
        w.projs[pi].life = 0;
        return;
    }
    p.age += 1;
    p.life -= 1;
    if p.released.is_none() {
        p.stage = a.stages.iter().rposition(|s| s.from <= p.age).unwrap_or(0);
        if p.age >= a.auto_release {
            p.automatic = true;
            p.released = Some(p.age);
            p.vx = 0.;
            p.vy = 0.;
            let mut f = w.fighters[p.owner].clone();
            if f.mov.as_ref().is_some_and(|m| m.slot == a.slot) {
                moves::trigger(w, d, p.owner, &mut f, crate::data::Trigger::ArticleRelease);
                w.fighters[p.owner] = f;
            }
        }
    }
    let stage = &a.stages[p.stage];
    let t = p.released.map(|age| {
        p.age - age + if p.automatic { stage.startup - stage.auto_startup.unwrap_or(stage.startup) } else { 0 }
    });
    if let Some(t) = t {
        p.vx = p.face * stage.vel[0] * p.speed;
        p.vy = -stage.vel[1];
        if t >= stage.frames {
            p.life = 0;
        }
    }
    p.x += p.vx;
    p.y += p.vy;
    if let Some(r) = a.hurtbox {
        let rect = crate::data::Rect { x1: p.x + r.x1, x2: p.x + r.x2, y1: p.y - r.y2, y2: p.y - r.y1 };
        for i in 0..w.fighters.len() {
            if !w.opponents(p.owner, i) {
                continue;
            }
            let fighter = &w.fighters[i];
            let (start, phase) = fighter
                .mov
                .as_ref()
                .map_or((w.frame.saturating_sub(u64::from(fighter.t)), 0), |m| (m.started, m.phase));
            for b in combat::active_boxes(d, fighter) {
                if b.hit.kind == crate::data::HitType::Windbox
                    || b.hit.kind == crate::data::HitType::Grab
                    || p.damage_hits.iter().any(|&(v, attack, part, age)| {
                        v == i && attack == start && part == phase && p.age - age < b.hit.rehit.unwrap_or(u32::MAX)
                    })
                    || !combat::circ_rect(b.x, b.y, b.hit.r, rect)
                {
                    continue;
                }
                p.damage += b.hit.damage;
                p.damage_hits.retain(|&(v, _, _, _)| v != i);
                p.damage_hits.push((i, start, phase, p.age));
                break;
            }
        }
        if stage.health > 0. && p.damage >= stage.health {
            p.life = 0;
        }
    }
    if let Some(t) = t.filter(|_| p.life > 0) {
        for v in 0..w.fighters.len() {
            let mut wind_applied = false;
            if !w.opponents(p.owner, v) {
                continue;
            }
            for b in &stage.hitboxes {
                if !b.active(t) || !b.targets(w.fighters[v].ground) {
                    continue;
                }
                let (x, y, r) = b.geom_at(t);
                let (x, y) = (p.x + p.face * x, p.y - y);
                if let Some(c) = combat::reflecting(d, &w.fighters[v])
                    && b.kind != crate::data::HitType::Windbox
                    && libm::hypot(x - w.fighters[v].x, y - (w.fighters[v].y - c.height)) < r + c.radius
                {
                    p.owner = v;
                    p.face = -p.face;
                    p.vx = -p.vx * c.speed;
                    p.damage = 0.;
                    p.power *= c.damage;
                    p.speed *= c.speed;
                    p.hits.clear();
                    break;
                }
                if !combat::circ_rect(x, y, r, combat::hurt(d, &w.fighters[v]))
                    || !w.fighters[v].hittable()
                    || p.hits.iter().any(|&(i, age)| i == v && p.age - age < b.rehit.unwrap_or(u32::MAX))
                {
                    continue;
                }
                let direction = match b.dir {
                    crate::data::LaunchDir::Forward => p.face,
                    crate::data::LaunchDir::Back => -p.face,
                    crate::data::LaunchDir::Away => {
                        let s = sign(w.fighters[v].x - p.x);
                        if s == 0. { p.face } else { s }
                    }
                };
                let mut hit = b.clone();
                hit.damage *= p.power;
                let source = combat::HitSource {
                    attacker: p.owner,
                    direction,
                    point: (x, y),
                    projectile: true,
                    label: Some(d.fighters[p.fighter_data].projectiles[&p.id].name.clone()),
                };
                if b.kind == crate::data::HitType::Windbox {
                    if wind_applied {
                        continue;
                    }
                    wind_applied = true;
                    let angle = b.angle.to_radians();
                    let k = &d.global.hit_types.windbox;
                    let force = b.bkb * k.push * d.fighters[w.fighters[v].data].stats.weight;
                    let f = &mut w.fighters[v];
                    f.vx = (f.vx + direction * libm::cos(angle) * force).clamp(-k.max_speed, k.max_speed);
                    let dy = -libm::sin(angle) * force;
                    if dy < 0. || !f.ground {
                        f.vy = (f.vy + dy).clamp(-k.max_speed, k.max_speed);
                    }
                    continue;
                }
                if let Some(c) = combat::countering(d, &w.fighters[v]) {
                    p.owner = v;
                    p.face = -p.face;
                    p.speed *= c.reflect_speed;
                    p.hits.clear();
                    combat::counter(w, d, v, None, 0.);
                } else if combat::parrying(d, &w.fighters[v]) {
                    combat::parry(w, d, v, None);
                } else if w.fighters[v].state == State::Shield {
                    combat::shield_hit(w, d, v, &hit, source);
                } else {
                    combat::apply_hit(w, d, v, &hit, source);
                }
                p.hits.push((v, p.age));
                break;
            }
        }
    }
    w.projs[pi] = p;
}

/// Damage the independent bodies touched by a traveling shot or an explosion.
pub fn projectile_contacts(w: &mut World, d: &GameData, p: &mut crate::world::Projectile, explosion: bool) {
    for qi in 0..w.projs.len() {
        let q = &w.projs[qi];
        let def = &d.fighters[q.fighter_data].projectiles[&q.id];
        let Some(a) = &def.article else { continue };
        let Some(r) = a.hurtbox else { continue };
        if q.life <= 0 || !w.opponents(p.owner, q.owner) {
            continue;
        }
        let rect = crate::data::Rect { x1: q.x + r.x1, x2: q.x + r.x2, y1: q.y - r.y2, y2: q.y - r.y1 };
        if !combat::circ_rect(p.x, p.y, p.r, rect) {
            continue;
        }
        let health = a.stages[q.stage].health;
        let q = &mut w.projs[qi];
        q.damage += p.hit.damage;
        if health > 0. && q.damage >= health {
            q.life = 0;
        }
        if !explosion {
            p.life = 0;
            break;
        }
    }
}
