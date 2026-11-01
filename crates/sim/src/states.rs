use crate::{
    Fx, GameData, World,
    data::Color,
    input::Intent,
    world::{Fighter, State},
};
use crate::{combat, moves, physics::*};
pub fn roll(w: &mut World, d: &GameData, f: &mut Fighter, dir: f64) {
    f.set_state(State::Roll);
    f.sd.dir = dir;
    f.face = dir;
    f.inv = d.global.dodge.roll_invuln;
    w.sfx("roll");
}
fn airdodge(w: &mut World, d: &GameData, f: &mut Fighter, inp: Intent, dir: f64) {
    let g = &d.global.dodge;
    let dy = f64::from(inp.down as i8 - inp.up as i8);
    f.ad_used = true;
    f.inv = if dir != 0. || dy != 0. { g.dir_air_dodge_invuln } else { g.air_dodge_invuln };
    f.ff = false;
    w.sfx("roll");
    f.set_state(State::Airdodge);
    if dir != 0. || dy != 0. {
        let l = libm::hypot(dir, dy);
        f.sd.dirl = true;
        f.sd.dx = dir / l;
        f.sd.dy = dy / l;
        f.vx = f.sd.dx * g.dir_air_dodge_speed;
        f.vy = f.sd.dy * g.dir_air_dodge_speed;
    }
}
pub fn update_fighter(w: &mut World, d: &GameData, i: usize, mut inp: Intent) {
    let mut f = w.fighters[i].clone();
    if w.cfg.players[i].cpu.is_none() {
        let actionable = f.lag == 0
            && match f.state {
                State::Idle | State::Air => true,
                State::Shield => f.sd.stun <= 0,
                State::Ledge => f.t.saturating_add(1) >= d.global.ledge.min_hang,
                State::Knockdown => f.t >= d.global.knockdown.min_frames,
                _ => false,
            };
        f.buffer.deliver(&mut inp, w.frame, d.global.input.buffer_frames, actionable && w.count == 0 && w.end_t == 0);
    }
    if matches!(f.state, State::Out | State::Win) {
        w.fighters[i] = f;
        return;
    }
    f.flash = f.flash.saturating_sub(1);
    f.hud_shake = f.hud_shake.saturating_sub(1);
    if f.state == State::Dead {
        f.sd.n -= 1.;
        if f.sd.n <= 0. {
            respawn(w, d, i, &mut f);
        }
        w.fighters[i] = f;
        return;
    }
    if f.lag > 0 {
        f.lag -= 1;
        w.fighters[i] = f;
        return;
    }
    let g = &d.global;
    let s = &d.fighters[f.data].stats;
    let r = &g.sim;
    if inp.shield_p {
        f.last_sp = w.frame as i64;
    }
    f.inv = f.inv.saturating_sub(1);
    f.blink = f.blink.saturating_sub(1);
    f.ledge_cd = f.ledge_cd.saturating_sub(1);
    f.drop = f.drop.saturating_sub(1);
    f.flip_t = f.flip_t.saturating_sub(1);
    if f.state != State::Shield {
        f.sh = g.shield.max.min(f.sh + g.shield.regen);
    }
    f.t += 1;
    let dir = inp.dir();
    f.in_dir = dir;
    let sd = f.sd.clone();
    match f.state {
        State::Respawn => {
            let rp = &d.stages[w.stage].respawns[i];
            let rt = &g.respawn;
            if f.t <= rt.descend_frames {
                f.y = rp.start_y + (rp.end_y - rp.start_y) * ease(f64::from(f.t) / f64::from(rt.descend_frames));
            } else if f.t > rt.max_wait
                || inp.left
                || inp.right
                || inp.down_p
                || inp.jump
                || inp.atk
                || inp.sp
                || inp.shield_p
                || inp.grab
            {
                f.set_state(State::Air);
                f.blink = rt.invuln;
            }
        }
        State::Idle => {
            if !moves::ground_actions(w, d, i, &mut f, inp, dir) {
                if dir != 0. {
                    if dir != f.face && f.vx.abs() > 3. {
                        w.dust(f.x, f.y, 3);
                    }
                    f.face = dir;
                    f.vx = (f.vx + dir * s.run_accel).clamp(-s.run_speed, s.run_speed);
                    f.run_ph += f.vx.abs() * 0.07;
                } else {
                    f.vx *= s.ground_friction;
                }
                fall(w, d, i, &mut f, false, None);
                if !f.ground {
                    f.set_state(State::Air);
                }
            }
        }
        State::Jumpsquat => {
            if inp.atk {
                moves::start_move(w, d, i, &mut f, "usmash");
            } else if inp.sp && inp.up && !f.up_b {
                moves::up_special(w, d, i, &mut f, dir);
            } else if inp.grab {
                moves::start_move(w, d, i, &mut f, "grab");
            } else {
                f.vx *= r.jumpsquat_friction;
                if f.t >= g.movement.jumpsquat {
                    f.vy = if inp.jump_held { -s.jump_speed } else { -s.short_hop_speed };
                    f.ground = false;
                    f.surf = None;
                    f.ff = false;
                    f.set_state(State::Air);
                    w.dust(f.x, f.y, 4);
                    w.sfx("jump");
                    integrate(w, d, i, &mut f, false);
                } else {
                    fall(w, d, i, &mut f, true, None);
                }
            }
        }
        State::Air => {
            if inp.shield_p && !f.ad_used {
                airdodge(w, d, &mut f, inp, dir);
                integrate(w, d, i, &mut f, false);
            } else {
                if inp.sp {
                    moves::special(w, d, i, &mut f, inp, dir);
                } else if inp.atk {
                    let slot = if inp.up {
                        "uair"
                    } else if inp.down {
                        "dair"
                    } else if dir != 0. && dir == -f.face {
                        "bair"
                    } else if dir != 0. {
                        "fair"
                    } else {
                        "nair"
                    };
                    moves::start_move(w, d, i, &mut f, slot);
                } else if inp.jump && f.jumps > 0 {
                    f.jumps -= 1;
                    f.vy = -s.air_jump_speed;
                    if dir != 0. {
                        f.vx = dir * s.air_jump_drift;
                    }
                    f.ff = false;
                    f.flip_t = r.flip_frames;
                    w.sfx("djump");
                    moves::effects(w, &d.fighters[f.data].fx.air_jump, (f.x, f.y), f.face, (f.vx, f.vy), i);
                }
                air_phys(w, d, i, &mut f, inp, dir, s.air_accel);
                if f.state == State::Air {
                    try_ledge(w, d, i, &mut f, inp, false);
                }
            }
        }
        State::Attack => moves::st_attack(w, d, i, &mut f, inp, dir),
        State::Helpless => {
            air_phys(w, d, i, &mut f, inp, dir, g.movement.helpless_accel);
            if f.state == State::Helpless {
                try_ledge(w, d, i, &mut f, inp, false);
            }
        }
        State::Landing => {
            f.vx *= r.landing_friction;
            fall(w, d, i, &mut f, true, None);
            if !f.ground {
                f.set_state(State::Air);
            } else if f64::from(f.t) >= sd.n {
                f.set_state(State::Idle);
            }
        }
        State::Shield => {
            f.sh -= g.shield.decay;
            f.vx *= r.shield_friction;
            fall(w, d, i, &mut f, true, None);
            if !f.ground {
                f.set_state(State::Air);
            } else if f.sh <= 0. {
                w.fighters[i] = f;
                combat::shield_break(w, d, i);
                return;
            } else if sd.stun > 0 {
                f.sd.stun -= 1;
            } else if inp.jump {
                f.set_state(State::Jumpsquat);
            } else if inp.grab || inp.atk {
                moves::start_move(w, d, i, &mut f, "grab");
            } else if inp.sp && inp.up && !f.up_b {
                moves::up_special(w, d, i, &mut f, dir);
            } else if inp.tap_r || inp.tap_l {
                roll(w, d, &mut f, if inp.tap_r { 1. } else { -1. });
            } else if inp.tap_d {
                f.set_state(State::Spot);
                f.inv = g.dodge.spot_invuln;
                w.sfx("roll");
            } else if !inp.shield {
                landing(&mut f, g.shield.drop_lag);
                f.sd.kind = "shield".into();
            }
        }
        State::Roll => {
            let dd = &g.dodge;
            f.vx = if f.t >= dd.roll_move_start && f.t <= dd.roll_move_end {
                sd.dir * dd.roll_speed
            } else {
                f.vx * r.dodge_friction
            };
            fall(w, d, i, &mut f, true, None);
            if !f.ground {
                f.set_state(State::Air);
            } else if f.t >= dd.roll_frames {
                f.face = -sd.dir;
                f.set_state(State::Idle);
            }
        }
        State::Spot => {
            f.vx *= r.dodge_friction;
            fall(w, d, i, &mut f, true, None);
            if f.t >= g.dodge.spot_frames {
                f.set_state(State::Idle);
            }
        }
        State::Airdodge => {
            let dd = &g.dodge;
            if sd.dirl && f.t < dd.dir_air_dodge_decay_frames {
                f.vx *= dd.dir_air_dodge_decay;
                f.vy *= dd.dir_air_dodge_decay;
                integrate(w, d, i, &mut f, false);
            } else {
                air_phys(w, d, i, &mut f, inp, dir, g.movement.helpless_accel);
            }
            if f.state == State::Airdodge {
                try_ledge(w, d, i, &mut f, inp, false);
                if f.state == State::Airdodge
                    && f.t >= if sd.dirl { dd.dir_air_dodge_frames } else { dd.air_dodge_frames }
                {
                    f.set_state(State::Air);
                }
            }
        }
        State::Hitstun => {
            let h = &g.hitstun;
            f.hitstun = f.hitstun.saturating_sub(1);
            f.vx *= if f.ground { h.ground_drag } else { h.air_drag };
            f.vx += dir * h.drift_accel;
            if f.tumble {
                f.spin += 0.3 * if f.vx >= 0. { 1. } else { -1. };
            }
            if libm::hypot(f.vx, f.vy) > r.hitstun_particle_speed && w.frame.is_multiple_of(2) {
                w.part((f.x, f.y - 40.), (0., 0.), 26., Color::Rgb(0xbbbbcc), 7., -0.05);
            }
            f.vy = (f.vy + h.gravity).min(h.max_fall);
            integrate(w, d, i, &mut f, false);
            if f.state == State::Hitstun && f.hitstun <= 0 {
                f.tumble = false;
                f.set_state(if f.ground { State::Idle } else { State::Air });
            }
        }
        State::Knockdown => {
            let k = &g.knockdown;
            f.vx *= r.knockdown_friction;
            fall(w, d, i, &mut f, true, None);
            if !f.ground {
                f.set_state(State::Air);
            } else {
                if f.t > k.min_frames {
                    if inp.atk {
                        f.set_state(State::Getup);
                        f.sd.kind = "atk".into();
                        f.inv = k.attack_invuln;
                    } else if dir != 0. {
                        roll(w, d, &mut f, dir);
                    } else if inp.jump || inp.up || inp.shield_p || inp.sp || inp.grab {
                        f.set_state(State::Getup);
                        f.sd.kind = "stand".into();
                        f.inv = k.getup_invuln;
                    }
                }
                if f.state == State::Knockdown && f.t >= k.auto_getup {
                    f.set_state(State::Getup);
                    f.sd.kind = "stand".into();
                    f.inv = k.getup_invuln;
                }
            }
        }
        State::Getup => {
            let k = &g.knockdown;
            f.vx *= r.knockdown_friction;
            fall(w, d, i, &mut f, true, None);
            if sd.kind == "atk" && f.t == k.attack_active[0] {
                w.sfx("swing");
            }
            if f.t >= if sd.kind == "atk" { k.attack_frames } else { k.getup_frames } {
                f.set_state(State::Idle);
            }
        }
        State::Tech => {
            f.vx *= r.dodge_friction;
            fall(w, d, i, &mut f, true, None);
            if f.t >= g.tech.frames {
                f.set_state(State::Idle);
            }
        }
        State::Ledge => st_ledge(w, d, &mut f, inp, dir),
        State::Climb => st_climb(w, d, &mut f),
        State::Hold => st_hold(w, d, i, &mut f, inp, dir),
        State::Throw => st_throw(w, d, i, &mut f),
        State::Grabbed => {
            if let Some(by) = sd.by
                && matches!(w.fighters[by].state, State::Hold | State::Throw)
            {
                if inp.any_p && w.fighters[by].state == State::Hold {
                    w.fighters[by].sd.timer -= g.grab.mash_per_press;
                }
            } else {
                f.set_state(State::Air);
            }
        }
        State::Shieldbreak => {
            f.vx *= r.shieldbreak_friction;
            f.vy = (f.vy + g.shield.break_gravity).min(g.shield.break_max_fall);
            integrate(w, d, i, &mut f, false);
        }
        State::Dizzy => {
            f.vx *= r.landing_friction;
            fall(w, d, i, &mut f, true, None);
            if inp.any_p {
                f.sd.n -= g.shield.dizzy_mash;
            }
            if f64::from(f.t) >= f.sd.n {
                f.sh = g.shield.dizzy_recover_hp;
                f.set_state(State::Idle);
            }
        }
        _ => {}
    }
    w.fighters[i] = f;
}
fn ease(k: f64) -> f64 {
    1. - (1. - k) * (1. - k)
}
fn st_ledge(w: &World, d: &GameData, f: &mut Fighter, inp: Intent, dir: f64) {
    let g = &d.global.ledge;
    let s = &d.stages[w.stage];
    let block = &s.blocks[f.sd.ledge_block];
    let side = f.sd.side;
    let ex = if side < 0. { block.left } else { block.right };
    f.vx = 0.;
    f.vy = 0.;
    f.x = ex + side * s.ledge.hang_x;
    f.y = block.top + s.ledge.hang_y;
    if f.t < g.min_hang {
        return;
    }
    let kind = if inp.jump {
        Some(("jump", g.jump.inv))
    } else if inp.atk || inp.sp {
        Some(("attack", g.attack.inv))
    } else if inp.shield_p {
        Some(("roll", g.roll.inv))
    } else if dir == -side || inp.up {
        Some(("climb", g.climb.inv))
    } else {
        None
    };
    if let Some((kind, inv)) = kind {
        let block = f.sd.ledge_block;
        f.set_state(State::Climb);
        f.sd.ledge_block = block;
        f.sd.side = side;
        f.sd.kind = kind.into();
        f.inv = f.inv.max(inv);
    } else if inp.down || dir == side || f.t > g.max_hang {
        f.set_state(State::Air);
        f.ledge_cd = g.regrab_cooldown;
        f.vy = d.global.sim.ledge_drop_speed;
        f.x += side * d.global.sim.ledge_drop_offset;
    }
}
fn st_climb(w: &mut World, d: &GameData, f: &mut Fighter) {
    let g = &d.global.ledge;
    let r = &d.global.sim;
    let kind = f.sd.kind.clone();
    let c = match kind.as_str() {
        "jump" => &g.jump,
        "roll" => &g.roll,
        "attack" => &g.attack,
        _ => &g.climb,
    };
    let side = f.sd.side;
    let s = &d.stages[w.stage];
    let block = &s.blocks[f.sd.ledge_block];
    let ex = if side < 0. { block.left } else { block.right };
    let fx = -side;
    f.vx = 0.;
    f.vy = 0.;
    if f.t <= c.a {
        let k = ease(f64::from(f.t) / f64::from(c.a));
        let x0 = ex + side * s.ledge.hang_x;
        f.x = x0 + (ex + fx * r.ledge_pull_in - x0) * k;
        let y0 = block.top + s.ledge.hang_y;
        f.y = y0 + (block.top - y0) * k;
        f.ground = false;
        f.surf = None;
        return;
    }
    if kind == "jump" {
        f.set_state(State::Air);
        f.y = block.top - r.ledge_jump_offset;
        f.vy = -g.jump_speed;
        f.vx = fx * r.ledge_jump_drift;
        f.flip_t = r.flip_frames;
        f.ledge_cd = r.ledge_jump_cooldown;
        w.sfx("jump");
        return;
    }
    f.ground = true;
    f.surf = Some(f.sd.ledge_block);
    f.y = block.top;
    if f.t <= c.b {
        f.x = ex
            + fx * r.ledge_pull_in
            + fx * (c.dist - r.ledge_pull_in) * ease(f64::from(f.t - c.a) / f64::from(c.b - c.a));
    }
    if kind == "attack" && f.t == g.attack_active[0] {
        w.sfx("swing");
    }
    if kind == "roll" && f.t == c.a + 1 {
        w.sfx("roll");
    }
    if f.t >= c.end {
        f.face = fx;
        f.set_state(State::Idle);
    }
}
fn st_hold(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, dir: f64) {
    let g = &d.global.grab;
    let v = f.sd.victim;
    f.vx *= d.global.sim.landing_friction;
    fall(w, d, i, f, true, None);
    let Some(v) = v.filter(|v| w.fighters[*v].state == State::Grabbed) else {
        f.set_state(State::Idle);
        return;
    };
    let vic = &mut w.fighters[v];
    vic.x = f.x + f.face * d.global.sim.grab_offset;
    vic.y = f.y;
    vic.face = -f.face;
    vic.ground = f.ground;
    f.sd.timer -= 1.;
    if f.sd.pt > 0 {
        f.sd.pt -= 1;
        if f.sd.pt == g.pummel_hit_frame {
            combat::credit_damage(w, d, i, v, g.pummel_damage);
            let vic = &mut w.fighters[v];
            vic.flash = 5;
            vic.hud_shake = 6;
            f.lag = g.pummel_hitlag;
            vic.lag = g.pummel_hitlag;
            let (x, y) = (vic.x, vic.y);
            w.fx.push(Fx::Burst {
                x,
                y: y - 44.,
                color: Color::Rgb(0xffffff),
                player: 0,
                n: 6.,
                speed: 3.,
                life: 12.,
                size: 3.,
                gravity: 0.,
            });
            w.sfx("pummel");
        }
        return;
    }
    if f.sd.timer <= 0. {
        landing(f, g.release_lag);
        f.vx = -f.face * d.global.sim.escape_attacker_vx;
        let v = &mut w.fighters[v];
        v.set_state(State::Air);
        v.vx = f.face * d.global.sim.escape_vx;
        v.vy = -d.global.sim.escape_vy;
        v.ground = false;
        v.surf = None;
        return;
    }
    if f.t < g.min_hold {
        return;
    }
    if inp.atk {
        f.sd.pt = g.pummel_frames;
        return;
    }
    let kind = if inp.up {
        Some("u")
    } else if inp.down {
        Some("d")
    } else if dir == f.face {
        Some("f")
    } else if dir == -f.face {
        Some("b")
    } else {
        None
    };
    if let Some(kind) = kind {
        f.set_state(State::Throw);
        f.sd.victim = Some(v);
        f.sd.kind = kind.into();
        w.sfx("throw");
    }
}
fn st_throw(w: &mut World, d: &GameData, i: usize, f: &mut Fighter) {
    let Some(v) = f.sd.victim else {
        f.set_state(State::Idle);
        return;
    };
    let k = f.sd.kind.clone();
    let ts = &d.fighters[f.data].throws;
    let tt = match k.as_str() {
        "b" => &ts.b,
        "u" => &ts.u,
        "d" => &ts.d,
        _ => &ts.f,
    };
    let r = &d.global.sim;
    f.vx *= r.landing_friction;
    fall(w, d, i, f, true, None);
    let vic = &mut w.fighters[v];
    if vic.state == State::Grabbed {
        let p = (f64::from(f.t) / f64::from(tt.release)).clamp(0., 1.);
        match k.as_str() {
            "f" => {
                vic.x = f.x + f.face * (r.grab_offset + p * r.throw_forward);
                vic.y = f.y;
            }
            "b" => {
                let a = p * std::f64::consts::PI;
                vic.x = f.x + f.face * r.grab_offset * libm::cos(a);
                vic.y = f.y - r.throw_back_height * libm::sin(a);
            }
            "u" => {
                vic.x = f.x + f.face * r.grab_offset * (1. - p);
                vic.y = f.y - r.throw_up_height * p;
            }
            _ => {
                vic.x = f.x + f.face * r.throw_down_offset;
                vic.y = f.y;
            }
        }
        if f.t == tt.release {
            vic.set_state(State::Air);
            vic.ground = false;
            vic.surf = None;
            let (x, y) = (vic.x, vic.y);
            if k == "d" {
                w.dust(x, f.y, 12);
                w.fx.push(Fx::Shake { k: 5. });
            }
            w.fighters[i] = f.clone();
            combat::apply_hit(
                w,
                d,
                v,
                &tt.hit,
                combat::HitSource {
                    attacker: i,
                    direction: if k == "b" { -f.face } else { f.face },
                    point: (x, y - 40.),
                    projectile: false,
                    label: None,
                },
            );
            f.lag = w.fighters[i].lag;
        }
    }
    if f.t >= tt.total {
        f.set_state(State::Idle);
    }
}
pub fn respawn(w: &World, d: &GameData, i: usize, f: &mut Fighter) {
    let rp = &d.stages[w.stage].respawns[i];
    f.x = rp.x;
    f.y = rp.start_y;
    f.vx = 0.;
    f.vy = 0.;
    f.dmg = 0.;
    f.ground = false;
    f.surf = None;
    f.hitstun = 0;
    f.up_b = false;
    f.ad_used = false;
    f.jumps = d.fighters[f.data].stats.air_jumps;
    f.tumble = false;
    f.ff = false;
    f.face = 1.;
    f.sh = d.global.shield.max;
    f.inv = 0;
    f.blink = 0;
    f.set_state(State::Respawn);
}
pub fn ko(w: &mut World, d: &GameData, i: usize) {
    combat::release_grab(w, d, i);
    let f = &mut w.fighters[i];
    f.stocks -= 1;
    w.fx.push(Fx::Beam { x: f.x, y: f.y - 40., player: i });
    w.fx.push(Fx::Shake { k: 20. });
    f.vx = 0.;
    f.vy = 0.;
    f.hitstun = 0;
    f.lag = 0;
    f.tumble = false;
    if w.cfg.training {
        f.stocks = f.stocks.max(1);
        f.set_state(State::Dead);
        f.sd.n = f64::from(d.global.respawn.dead_frames);
    } else if f.stocks > 0 {
        f.set_state(State::Dead);
        f.sd.n = f64::from(d.global.respawn.dead_frames);
    } else {
        f.set_state(State::Out);
    }
    w.fx.push(Fx::Sfx { name: "ko".into(), arg: None, x: Some(f.x) });
}
