use crate::actions::{Action, accept};
use crate::{
    Fx, GameData, World,
    data::{
        Color,
        global::{Window, span},
    },
    input::Intent,
    world::{Fighter, State},
};
use crate::{combat, moves, physics::*};
/// Intangible for `len` frames from state frame `start` of the state just entered. Start 0 replaces the current
/// invulnerability (or keeps the longer with `keep`); a later start is applied when the state clock gets there.
pub fn intangible(f: &mut Fighter, start: u32, len: u32, keep: bool) {
    if start == 0 {
        f.inv = if keep { f.inv.max(len) } else { len };
    } else {
        f.sd.inv_at = start;
        f.sd.inv_len = len;
    }
}
/// Starts a dodge's window: staled by recent dodges when `stales`, else fresh.
fn dodge(w: &World, d: &GameData, f: &mut Fighter, win: &Window, stales: bool) {
    let s = &d.global.dodge.staling;
    let mut step = None;
    if stales && !s.steps.is_empty() {
        let recent = f.dodge_at > 0 && w.frame - f.dodge_at <= u64::from(s.window);
        f.dodge_stale = if recent { (f.dodge_stale + 1).min(s.steps.len() as u8) } else { 0 };
        f.dodge_at = w.frame;
        step = f.dodge_stale.checked_sub(1).map(|n| s.steps[usize::from(n)]);
    }
    let scale = |n: u32, k: f64| libm::round(f64::from(n) * k) as u32;
    f.sd.end = step.map_or(win.frames, |s| scale(win.frames, s.frames));
    intangible(f, win.invuln_start, step.map_or(win.invuln_len(), |s| scale(win.invuln_len(), s.invuln)), false);
}
fn roll_from(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, dir: f64, stales: bool) {
    accept(w, i, Action::Roll);
    f.set_state(State::Roll);
    f.sd.dir = dir;
    f.face = dir;
    dodge(w, d, f, &d.global.dodge.roll, stales);
    w.sfx("roll");
}
/// Getup and tech rolls (no staling).
pub fn roll(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, dir: f64) {
    roll_from(w, d, i, f, dir, false);
}
fn airdodge(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, dir: f64) {
    accept(w, i, Action::AirDodge);
    let g = &d.global.dodge;
    let dy = f64::from(inp.down as i8 - inp.up as i8);
    let directional = dir != 0. || dy != 0.;
    f.ad_used = true;
    f.ff = false;
    w.sfx("roll");
    f.set_state(State::Airdodge);
    dodge(w, d, f, if directional { &g.dir_air } else { &g.air }, true);
    if directional {
        let l = libm::hypot(dir, dy);
        f.sd.dirl = true;
        f.sd.dx = dir / l;
        f.sd.dy = dy / l;
        f.vx = f.sd.dx * g.dir_air.speed;
        f.vy = f.sd.dy * g.dir_air.speed;
    }
}
fn air_jump(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, dir: f64) {
    accept(w, i, Action::AirJump);
    let fighter = &d.fighters[f.data];
    f.jumps -= 1;
    f.launch = (0., 0.);
    f.vy = -fighter.stats.air_jump_speed;
    if dir != 0. {
        f.vx = dir * fighter.stats.air_jump_drift;
    }
    f.ff = false;
    f.flip_t = d.global.sim.flip_frames;
    w.sfx("djump");
    moves::effects(w, &fighter.fx.air_jump, (f.x, f.y), f.face, (f.vx, f.vy), i);
}
/// One fighter's tick: jumpsquat shortcuts, the press buffer, the state machine, then buffer acceptance.
pub fn update_fighter(w: &mut World, d: &GameData, i: usize, mut inp: Intent) {
    let g = &d.global;
    let f = &mut w.fighters[i];
    if f.actionable(g) {
        f.paralyze_used = false;
    }
    if g.input.jump_aerials && f.state == State::Jumpsquat && f.lag == 0 {
        // Presses during jumpsquat shape the jump instead of queueing: attack picks a short-hop aerial, a second
        // jump press short hops.
        if inp.atk {
            f.sd.jump_aerial = Some(moves::aerial_slot(f, inp).into());
            inp.atk = false;
            if inp.jump_attack {
                inp.jump = false;
                inp.jump_attack = false;
            }
        }
        if inp.jump {
            f.sd.short = true;
            inp.jump = false;
            inp.short_hop = false;
        }
    }
    let mut delivered = None;
    if w.cfg.players[i].cpu.is_none() {
        let command = f.buffer.cmd.as_ref().map_or(&inp, |c| &c.intent);
        let actionable =
            (f.actionable(g) || f.lag == 0 && moves::accepts_command(d, f, command)) && w.count == 0 && w.end_t == 0;
        let ab = w.cfg.players[i].controls.ab_smash;
        delivered = f.buffer.deliver(&mut inp, w.frame, &g.input, actionable, ab);
    }
    if !crate::gait::can_cancel(d, &w.fighters[i], &inp) {
        if delivered.is_none() && w.cfg.players[i].cpu.is_none() {
            let ab = w.cfg.players[i].controls.ab_smash;
            w.fighters[i].buffer.deliver(&mut inp, w.frame, &g.input, false, ab);
        }
        inp = inp.movement_only();
    }
    let start = w.acts.len();
    step(w, d, i, inp);
    let mut accepted = false;
    for a in w.acts[start..].iter_mut().filter(|a| a.player == i && a.action.command()) {
        a.pressed = delivered;
        accepted = true;
    }
    if accepted {
        w.fighters[i].buffer.accept();
    }
}
fn step(w: &mut World, d: &GameData, i: usize, inp: Intent) {
    let mut f = w.fighters[i].clone();
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
    if std::mem::take(&mut f.di) && f.state == State::Hitstun {
        combat::influence(w, d, i, &mut f, (inp.x, inp.y));
    }
    let g = &d.global;
    let s = &d.fighters[f.data].stats;
    let r = &g.sim;
    if inp.shield_p && !(f.state == State::Hitstun && w.frame < f.tech_lock) {
        f.last_sp = w.frame as i64;
        if f.state == State::Hitstun && g.tech.lockout > 0 {
            f.tech_lock = w.frame + u64::from(g.tech.lockout);
        }
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
    if f.sd.inv_len > 0 && f.t == f.sd.inv_at {
        f.inv = f.inv.max(f.sd.inv_len);
    }
    let dir = inp.dir();
    f.in_dir = dir;
    f.in_down = inp.down;
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
                if g.gait.enabled {
                    crate::gait::step(w, d, i, &mut f, inp);
                } else if dir != 0. {
                    if dir != f.face && f.vx.abs() > 3. {
                        w.dust(f.x, f.y, 3);
                    }
                    f.face = dir;
                    f.vx = (f.vx + dir * s.run_accel).clamp(-s.run_speed, s.run_speed);
                    f.run_ph += f.vx.abs() * 0.07;
                } else {
                    skid(d, &mut f, s.ground_friction, 1.);
                }
                fall(w, d, i, &mut f, false, None);
                if !f.ground {
                    f.set_state(State::Air);
                }
            }
        }
        State::Jumpsquat => {
            if inp.atk {
                moves::start_attack(w, d, i, &mut f, "usmash", inp);
            } else if inp.sp && inp.up && !f.up_b {
                moves::up_special(w, d, i, &mut f, dir);
            } else if inp.grab {
                moves::start_attack(w, d, i, &mut f, "grab", inp);
            } else {
                skid(d, &mut f, r.jumpsquat_friction, 1.);
                if f.t >= g.movement.jumpsquat {
                    let aerial = f.sd.jump_aerial.take();
                    let full = inp.jump_held && aerial.is_none() && !f.sd.short;
                    f.vy = if full { -s.jump_speed } else { -s.short_hop_speed };
                    accept(w, i, if full { Action::FullHop } else { Action::ShortHop });
                    f.ground = false;
                    f.surf = None;
                    f.ff = false;
                    f.set_state(State::Air);
                    w.dust(f.x, f.y, 4);
                    w.sfx("jump");
                    if let Some(slot) = aerial {
                        accept(w, i, Action::Move(slot.clone()));
                        moves::start_move(w, d, i, &mut f, &slot);
                    }
                    integrate(w, d, i, &mut f, false);
                } else {
                    fall(w, d, i, &mut f, true, None);
                }
            }
        }
        State::Air => {
            if inp.shield_p && !f.ad_used {
                airdodge(w, d, i, &mut f, inp, dir);
                integrate(w, d, i, &mut f, false);
            } else {
                if inp.sp {
                    moves::special(w, d, i, &mut f, inp, dir);
                } else if inp.grab && d.fighters[f.data].moves.zair.is_some() {
                    moves::start_attack(w, d, i, &mut f, "zair", inp);
                } else if inp.atk {
                    if g.input.jump_aerials && inp.jump_attack && f.jumps > 0 {
                        air_jump(w, d, i, &mut f, dir);
                    }
                    let slot = moves::aerial_slot(&f, inp);
                    moves::start_attack(w, d, i, &mut f, slot, inp);
                } else if inp.jump && f.jumps > 0 {
                    air_jump(w, d, i, &mut f, dir);
                }
                air_phys(w, d, i, &mut f, inp, inp.x, s.air_accel);
                if f.state == State::Air {
                    try_ledge(w, d, i, &mut f, inp, false);
                }
            }
        }
        State::Attack => moves::st_attack(w, d, i, &mut f, inp, dir),
        State::Helpless => {
            let acc = air_accel(d, &f, g.movement.helpless_accel, |u| u.helpless_accel_mult);
            air_phys(w, d, i, &mut f, inp, inp.x, acc);
            if f.state == State::Helpless {
                try_ledge(w, d, i, &mut f, inp, false);
            }
        }
        State::Landing => {
            skid(d, &mut f, r.landing_friction, 1.);
            fall(w, d, i, &mut f, true, None);
            if !f.ground {
                f.set_state(State::Air);
            } else if f64::from(f.t) >= sd.n {
                f.set_state(State::Idle);
            }
        }
        State::Shield => {
            f.sh -= g.shield.decay;
            skid(d, &mut f, r.shield_friction, 1.);
            fall(w, d, i, &mut f, true, None);
            if !f.ground {
                f.set_state(State::Air);
            } else if f.sh <= 0. {
                w.fighters[i] = f;
                combat::shield_break(w, d, i);
                return;
            } else if sd.stun > 0 {
                f.sd.stun -= 1;
            } else if inp.jump_attack && g.input.jump_aerials {
                moves::start_jump(w, i, &mut f, inp, true);
            } else if inp.atk && g.shield.up_smash && inp.strike.up(&inp) {
                // Up smash out of shield comes before shield + attack = grab.
                moves::start_attack(w, d, i, &mut f, "usmash", inp);
            } else if inp.jump {
                moves::start_jump(w, i, &mut f, inp, g.input.jump_aerials);
            } else if inp.grab || inp.atk {
                moves::start_attack(w, d, i, &mut f, "grab", inp);
            } else if inp.sp && inp.up && !f.up_b {
                moves::up_special(w, d, i, &mut f, dir);
            } else if inp.tap_r || inp.tap_l {
                roll_from(w, d, i, &mut f, if inp.tap_r { 1. } else { -1. }, true);
            } else if inp.tap_d {
                accept(w, i, Action::SpotDodge);
                f.set_state(State::Spot);
                dodge(w, d, &mut f, &g.dodge.spot, true);
                w.sfx("roll");
            } else if !inp.shield {
                accept(w, i, Action::ShieldDrop);
                landing(&mut f, g.shield.drop_lag);
                f.sd.kind = "shield".into();
            }
        }
        State::Roll => {
            let dd = &g.dodge.roll;
            f.vx = if f.t >= dd.move_start && f.t <= dd.move_end { sd.dir * dd.speed } else { f.vx * r.dodge_friction };
            fall(w, d, i, &mut f, true, None);
            if !f.ground {
                f.set_state(State::Air);
            } else if f.t >= end(&sd, dd) {
                f.face = -sd.dir;
                f.set_state(State::Idle);
            }
        }
        State::Spot => {
            f.vx *= r.dodge_friction;
            fall(w, d, i, &mut f, true, None);
            if f.t >= end(&sd, &g.dodge.spot) {
                f.set_state(State::Idle);
            }
        }
        State::Airdodge => {
            let dd = if sd.dirl { &g.dodge.dir_air } else { &g.dodge.air };
            if sd.dirl && f.t < dd.decay_frames {
                f.vx *= dd.decay;
                f.vy *= dd.decay;
                integrate(w, d, i, &mut f, false);
            } else {
                let acc = air_accel(d, &f, g.movement.helpless_accel, |u| u.helpless_accel_mult);
                air_phys(w, d, i, &mut f, inp, inp.x, acc);
            }
            if f.state == State::Airdodge {
                try_ledge(w, d, i, &mut f, inp, false);
                if f.state == State::Airdodge && f.t >= end(&sd, dd) {
                    f.set_state(State::Air);
                }
            }
        }
        State::Hitstun => {
            let h = &g.hitstun;
            f.hitstun = f.hitstun.saturating_sub(1);
            if let Some(c) = f.hitstun_cancel(g) {
                let dodge = inp.shield_p && !f.ad_used && f.t >= c.air_dodge_after;
                if dodge || inp.atk && f.t >= c.aerial_after {
                    f.hitstun = 0;
                    f.tumble = false;
                    if dodge {
                        airdodge(w, d, i, &mut f, inp, dir);
                        integrate(w, d, i, &mut f, false);
                    } else {
                        let slot = moves::aerial_slot(&f, inp);
                        moves::start_attack(w, d, i, &mut f, slot, inp);
                        air_phys(w, d, i, &mut f, inp, inp.x, s.air_accel);
                    }
                    w.fighters[i] = f;
                    return;
                }
            }
            let launch = h.ultimate.is_some();
            if launch && f.ground {
                f.launch = (0., 0.);
            }
            if launch && !f.ground {
                // Stick drift accelerates the fighter's own velocity up to its air speed, apart from the launch.
                f.vx -= f.launch.0;
                air_drift(d, &mut f, inp.x, h.drift_accel, g.movement.air_drag);
                f.vx += f.launch.0;
            } else {
                f.vx *= if f.ground { h.ground_drag } else { h.air_drag };
                f.vx += inp.x * h.drift_accel;
            }
            if f.tumble {
                f.spin += 0.3 * if f.vx >= 0. { 1. } else { -1. };
            }
            if libm::hypot(f.vx, f.vy) > r.hitstun_particle_speed && w.frame.is_multiple_of(2) {
                w.part((f.x, f.y - 40.), (0., 0.), 26., Color::Rgb(0xbbbbcc), 7., -0.05);
            }
            if launch {
                fall(w, d, i, &mut f, false, None);
            } else {
                f.vy = (f.vy + h.gravity).min(h.max_fall);
                integrate(w, d, i, &mut f, false);
            }
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
                let getup = |f: &mut Fighter, kind: &str, win: &Window| {
                    f.set_state(State::Getup);
                    f.sd.kind = kind.into();
                    intangible(f, win.invuln_start, win.invuln_len(), false);
                };
                if f.t > k.min_frames {
                    if inp.atk {
                        accept(w, i, Action::GetupAttack);
                        getup(&mut f, "atk", &k.getup_attack);
                    } else if dir != 0. {
                        roll(w, d, i, &mut f, dir);
                    } else if inp.jump || inp.up || inp.shield_p || inp.sp || inp.grab {
                        accept(w, i, Action::Getup);
                        getup(&mut f, "stand", &k.getup);
                    }
                }
                if f.state == State::Knockdown && f.t >= k.auto_getup {
                    getup(&mut f, "stand", &k.getup);
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
            if f.t >= if sd.kind == "atk" { k.getup_attack.frames } else { k.getup.frames } {
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
        State::Ledge => st_ledge(w, d, i, &mut f, inp, dir),
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
            skid(d, &mut f, r.landing_friction, 1.);
            fall(w, d, i, &mut f, true, None);
            if inp.any_p {
                f.sd.n -= g.shield.dizzy_mash;
            }
            if f64::from(f.t) >= f.sd.n {
                f.sh = g.shield.dizzy_recover_hp;
                f.set_state(State::Idle);
            }
        }
        State::Buried => {
            f.vx = 0.;
            f.vy = 0.;
            if inp.any_p
                && let Some(b) = &g.hit_types.bury
            {
                f.sd.n -=
                    if inp.atk || inp.sp || inp.jump || inp.grab || inp.shield_p { b.button_mash } else { b.mash };
            }
            fall(w, d, i, &mut f, true, None);
            if f64::from(f.t) >= f.sd.n || !f.ground {
                f.set_state(if f.ground { State::Idle } else { State::Air });
            }
        }
        _ => {}
    }
    w.fighters[i] = f;
}
/// A dodge's last frame after staling.
fn end(sd: &crate::world::StateData, win: &Window) -> u32 {
    if sd.end > 0 { sd.end } else { win.frames }
}
fn ease(k: f64) -> f64 {
    1. - (1. - k) * (1. - k)
}
fn st_ledge(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, dir: f64) {
    let g = &d.global.ledge;
    let s = &d.stages[w.stage];
    let block = &s.blocks[f.sd.ledge_block];
    let side = f.sd.side;
    let ex = if side < 0. { block.left } else { block.right };
    f.vx = 0.;
    f.vy = 0.;
    let root = crate::interactions::ledge_hang_root(&d.fighters[f.data], s, ex, block.top, side);
    f.x = root[0];
    f.y = root[1];
    if f.t < g.min_hang {
        return;
    }
    let kind = if inp.jump {
        Some(("jump", &g.jump))
    } else if inp.atk || inp.sp {
        Some(("attack", &g.attack))
    } else if inp.shield_p {
        Some(("roll", &g.roll))
    } else if dir == -side || inp.up {
        Some(("climb", &g.climb))
    } else {
        None
    };
    if let Some((kind, c)) = kind {
        accept(w, i, Action::Ledge(kind.into()));
        let block = f.sd.ledge_block;
        f.set_state(State::Climb);
        f.sd.ledge_block = block;
        f.sd.side = side;
        f.sd.kind = kind.into();
        intangible(f, c.invuln_start, span(c.invuln_start, c.invuln_end), true);
    } else if inp.down || dir == side || f.t > g.max_hang {
        if f.t <= g.max_hang {
            accept(w, i, Action::LedgeDrop);
        }
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
    let interaction = crate::interactions::ledge(&d.fighters[f.data]);
    let pull_in = interaction.and_then(|l| l.pull_in).unwrap_or(r.ledge_pull_in);
    let dist = interaction
        .and_then(|l| match kind.as_str() {
            "roll" => l.roll_distance,
            "attack" => l.attack_distance,
            "climb" => l.climb_distance,
            _ => None,
        })
        .unwrap_or(c.dist);
    f.vx = 0.;
    f.vy = 0.;
    if f.t <= c.a {
        let k = ease(f64::from(f.t) / f64::from(c.a));
        let root = crate::interactions::ledge_hang_root(&d.fighters[f.data], s, ex, block.top, side);
        let x0 = root[0];
        f.x = x0 + (ex + fx * pull_in - x0) * k;
        let y0 = root[1];
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
    f.used_air_moves.clear();
    f.y = block.top;
    if f.t <= c.b {
        f.x = ex + fx * pull_in + fx * (dist - pull_in) * ease(f64::from(f.t - c.a) / f64::from(c.b - c.a));
    }
    if kind == "attack" && f.t == g.attack_active[0] {
        w.sfx("swing");
    }
    if kind == "roll" && f.t == c.a + 1 {
        w.sfx("roll");
    }
    if f.t >= c.end {
        f.face = fx;
        f.ledge_inv = true;
        f.set_state(State::Idle);
    }
}
fn st_hold(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent, dir: f64) {
    let g = &d.global.grab;
    let v = f.sd.victim;
    skid(d, f, d.global.sim.landing_friction, 1.);
    fall(w, d, i, f, true, None);
    let Some(v) = v.filter(|v| w.fighters[*v].state == State::Grabbed) else {
        f.set_state(State::Idle);
        return;
    };
    let vic = &mut w.fighters[v];
    crate::interactions::place_held(d, f, vic);
    f.sd.timer -= 1.;
    if f.sd.pt > 0 {
        f.sd.pt -= 1;
        if f.sd.pt == g.pummel_hit_frame {
            combat::credit_damage(w, d, i, v, d.fighters[f.data].stats.pummel_damage.unwrap_or(g.pummel_damage));
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
        accept(w, i, Action::Pummel);
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
        accept(w, i, Action::Throw(kind.into()));
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
    skid(d, f, r.landing_friction, 1.);
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
        if crate::interactions::grab_overridden(d, f, vic) {
            let h = crate::interactions::hold_point(d, f);
            let target = match k.as_str() {
                "f" => [h[0] + p * r.throw_forward, h[1]],
                "b" => {
                    let a = p * std::f64::consts::PI;
                    [h[0] * libm::cos(a), h[1] + r.throw_back_height * libm::sin(a)]
                }
                "u" => [h[0] * (1. - p), h[1] + r.throw_up_height * p],
                _ => [r.throw_down_offset, h[1]],
            };
            let root = crate::interactions::inverse_anchor(
                crate::interactions::forward_point(f, target),
                vic.face,
                crate::interactions::held_anchor(d, vic),
            );
            vic.x = root[0];
            vic.y = root[1];
            vic.surf = None;
        }
        for hit in tt.ticks.iter().filter(|h| h.at == f.t) {
            combat::credit_damage(w, d, i, v, hit.damage);
        }
        let vic = &mut w.fighters[v];
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
    f.di = false;
    f.ledge_inv = true;
    f.used_air_moves.clear();
    f.paralyze_used = false;
    crate::charges::refill(d, f);
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
    f.di = false;
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
