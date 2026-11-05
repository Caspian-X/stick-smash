use crate::data::moves::AirControl;
use crate::{
    GameData, World,
    input::Intent,
    world::{State, sign},
};
fn center(inp: &mut Intent, x: f64, mid: f64) {
    inp.right = x < mid;
    inp.left = x >= mid;
}
fn toward(inp: &mut Intent, dir: f64) {
    inp.left = dir < 0.;
    inp.right = dir > 0.;
}
/// Nearest living opponent; iteration order resolves distance ties.
pub fn target(w: &World, i: usize) -> Option<usize> {
    let f = &w.fighters[i];
    let mut best = None;
    let mut dist = f64::INFINITY;
    for (j, o) in w.fighters.iter().enumerate() {
        if !w.enemies(i, j) || matches!(o.state, State::Out | State::Dead | State::Respawn) {
            continue;
        }
        let dd = libm::hypot(o.x - f.x, o.y - f.y);
        if dd < dist {
            dist = dd;
            best = Some(j);
        }
    }
    best
}
pub fn ai_input(w: &mut World, d: &GameData, i: usize, level: u8) -> Intent {
    let mut f = w.fighters[i].clone();
    let o = target(w, i).map(|j| w.fighters[j].clone());
    let lv = usize::from(level.min(2));
    let st = f.state;
    let cfg = &d.global.ai;
    let v = &cfg.thresholds;
    let s = &d.stages[w.stage];
    if v.surface_chase_ticks > 0 {
        let separated = o.as_ref().is_some_and(|o| {
            o.surf.is_some()
                && o.surf != f.surf
                && ((o.x - f.x).abs() > d.fighters[f.data].ai.reach || (o.y - f.y).abs() > v.attack_height)
        });
        if !separated || matches!(st, State::Dead | State::Respawn | State::Out) {
            f.ai.pursuit = false;
            f.ai.pursuit_ticks = 0;
        } else if f.ground {
            if (f.x - f.ai.pursuit_x).abs() < v.target_deadzone {
                f.ai.pursuit_ticks = f.ai.pursuit_ticks.saturating_add(1);
                f.ai.pursuit |= f.ai.pursuit_ticks >= v.surface_chase_ticks;
            } else {
                f.ai.pursuit_x = f.x;
                f.ai.pursuit_ticks = 0;
            }
        }
    }
    let support = if matches!(st, State::Ledge | State::Climb) { Some(f.sd.ledge_block) } else { f.surf };
    let support = if !f.ground && matches!(st, State::Air | State::Helpless) {
        let target = o.as_ref().and_then(|o| {
            o.surf.filter(|&id| id < s.blocks.len()).or_else(|| {
                f.ai.pursuit
                    .then(|| {
                        s.blocks
                            .iter()
                            .enumerate()
                            .min_by(|(_, a), (_, b)| {
                                (o.x - o.x.clamp(a.left, a.right))
                                    .abs()
                                    .total_cmp(&(o.x - o.x.clamp(b.left, b.right)).abs())
                            })
                            .map(|(id, _)| id)
                    })
                    .flatten()
            })
        });
        target.or(support)
    } else {
        support
    };
    let s = &s.blocks[support.filter(|&i| i < s.blocks.len()).unwrap_or_else(|| {
        s.blocks
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let distance =
                    |b: &crate::data::stage::Ground| libm::hypot(f.x - f.x.clamp(b.left, b.right), f.y - b.top);
                distance(a).total_cmp(&distance(b))
            })
            .map_or(0, |(i, _)| i)
    })];
    let mid = (s.left + s.right) / 2.;
    let c = &d.fighters[f.data];
    let mut inp = Intent::default();
    let ai = &mut f.ai;
    if ai.charge_t > 0. {
        ai.charge_t -= 1.;
        inp.atk_held = true;
    }
    if ai.sp_t > 0. {
        ai.sp_t -= 1.;
        inp.sp_held = true;
    }
    if ai.up_t > 0. {
        ai.up_t -= 1.;
        inp.jump_held = true;
    }
    if ai.shield_t > 0. {
        ai.shield_t -= 1.;
    }
    // Return through a closure so every path stores the same AI snapshot.
    let result = (|| {
        if st == State::Attack
            && crate::moves::definition(d, &w.fighters[i]).air == AirControl::UpSpecial
            && f.y > s.top - v.recovery_height
        {
            center(&mut inp, f.x, mid);
            // A directional warp aims up from below the stage or the ground, so an out-of-shield one can't leave it.
            if c.moves.uspec.phases.iter().any(|p| p.travel_speed.is_some()) && (f.y > s.top || f.ground) {
                inp.up = true;
            }
            if c.ai
                .recovery_shield_cancel
                .as_ref()
                .is_some_and(|name| crate::moves::phase(d, &w.fighters[i]).name == *name)
                && (f.x < s.left - v.recovery_edge || f.x > s.right + v.recovery_edge)
            {
                inp.shield_p = true;
            }
            return inp;
        }
        if matches!(st, State::Grabbed | State::Dizzy) {
            inp.any_p = w.rng.next_f64() < cfg.mash[lv];
            return inp;
        }
        if st == State::Hitstun {
            if f.tumble && w.rng.next_f64() < cfg.tech[lv] {
                inp.shield_p = true;
            }
            if w.rng.next_f64() < v.center_chance {
                center(&mut inp, f.x, mid);
            }
            return inp;
        }
        if st == State::Knockdown {
            if f64::from(f.t) > v.knockdown_wait + w.rng.next_f64() * v.knockdown_random {
                let r = w.rng.next_f64();
                if r < v.knockdown_attack {
                    inp.atk = true;
                } else if r < v.knockdown_center {
                    center(&mut inp, f.x, mid);
                } else {
                    inp.jump = true;
                }
            }
            return inp;
        }
        if st == State::Ledge {
            if ai.wait == 0. {
                ai.wait = libm::floor(w.rng.range(v.ledge_wait_min, v.ledge_wait_max));
            }
            if f64::from(f.t) > ai.wait {
                ai.wait = 0.;
                let r = w.rng.next_f64();
                if r < v.ledge_jump {
                    inp.jump = true;
                } else if r < v.ledge_attack {
                    inp.atk = true;
                } else if r < v.ledge_shield {
                    inp.shield_p = true;
                } else {
                    center(&mut inp, f.x, mid);
                }
            }
            return inp;
        }
        if st == State::Hold {
            if f.t == 1 {
                ai.hold_t = libm::floor(w.rng.range(v.hold_min, v.hold_max));
                ai.pum = libm::floor(w.rng.range(0., v.pummel_max));
            }
            if f64::from(f.t) > ai.hold_t && f.sd.pt == 0 {
                if ai.pum > 0. && w.rng.next_f64() < v.pummel_chance {
                    ai.pum -= 1.;
                    inp.atk = true;
                } else if o.as_ref().is_some_and(|o| o.dmg > v.throw_kill_percent) {
                    toward(&mut inp, if f.x < mid { -1. } else { 1. });
                } else {
                    let r = w.rng.next_f64();
                    if r < v.throw_down {
                        inp.down = true;
                    } else if r < v.throw_up {
                        inp.up = true;
                    } else if r < v.throw_forward {
                        toward(&mut inp, f.face);
                    } else {
                        toward(&mut inp, -f.face);
                    }
                }
            }
            return inp;
        }
        if st == State::Shield {
            let atk = o.as_ref().is_some_and(|o| o.state == State::Attack);
            let close = o.as_ref().is_some_and(|o| (o.x - f.x).abs() < v.shield_close);
            if !atk && close && w.rng.next_f64() < cfg.shield_response[lv] {
                let r = w.rng.next_f64();
                if r < v.shield_grab {
                    inp.grab = true;
                } else if r < v.shield_special {
                    inp.up = true;
                    inp.sp = true;
                } else {
                    inp.jump = true;
                    ai.up_t = v.jump_hold;
                }
                ai.shield_t = 0.;
                return inp;
            }
            if f.sh > v.shield_min && (ai.shield_t > 0. || atk) {
                inp.shield = true;
            }
            return inp;
        }
        let under = f.x > s.left + s.under_inset - v.under_pad
            && f.x < s.right - s.under_inset + v.under_pad
            && f.y > s.bottom - v.under_bottom;
        let offstage = matches!(st, State::Air | State::Helpless)
            && (f.x < s.left || f.x > s.right || f.y > s.top + v.offstage_pad);
        if offstage {
            let dir = if under {
                if f.x < mid { -1. } else { 1. }
            } else if f.x < mid {
                1.
            } else {
                -1.
            };
            toward(&mut inp, dir);
            inp.up = false;
            if st == State::Helpless {
                return inp;
            }
            let edge = if f.x < mid { s.left } else { s.right };
            if f.vy > v.recover_jump_vy && f.jumps > 0 && f.y > s.top - v.recover_jump_height {
                inp.jump = true;
            } else if f.jumps == 0
                && !f.up_b
                && f.vy > v.recover_special_vy
                && !under
                && f.y > s.top - v.recover_special_height
                && ((f.x - edge).abs() < v.recover_special_distance || f.y > s.top + v.recover_special_depth)
            {
                inp.up = true;
                inp.sp = true;
            }
            return inp;
        }
        let Some(o) = &o else {
            let dx = mid - f.x;
            if dx.abs() > v.center_deadzone {
                toward(&mut inp, dx);
            }
            return inp;
        };
        if ai.pursuit {
            let destination = o.surf.unwrap();
            let (left, right, top) = crate::physics::surface(d, w, destination);
            let tx = o.x.clamp(left, right);
            toward(&mut inp, tx - f.x);
            if f.ground {
                if let Some(id) = f.surf {
                    let (x1, x2, _) = crate::physics::surface(d, w, id);
                    let edge = if tx > f.x { x2 } else { x1 };
                    if (f.x - edge).abs() <= v.target_pad || f.x >= left && f.x <= right {
                        inp.jump = true;
                        inp.jump_held = true;
                        ai.up_t = v.jump_hold;
                    }
                }
            } else if f.vy > v.recover_jump_vy && f.jumps > 0 && f.y > top - v.recover_jump_height {
                inp.jump = true;
                inp.jump_held = true;
                ai.up_t = v.jump_hold;
            }
            return inp;
        }
        let dx = o.x - f.x;
        let dy = o.y - f.y;
        let adx = dx.abs();
        let tdir = if dx == 0. { 1. } else { sign(dx) };
        let stage = &d.stages[w.stage];
        let destination =
            o.surf.filter(|&id| (v.surface_chase_ticks > 0 || id < stage.blocks.len()) && Some(id) != f.surf);
        let (left, right, _) = if v.surface_chase_ticks > 0 {
            o.surf.map_or((s.left, s.right, s.top), |id| crate::physics::surface(d, w, id))
        } else {
            destination.map_or((s.left, s.right, s.top), |id| {
                let b = &stage.blocks[id];
                (b.left, b.right, b.top)
            })
        };
        let pad = v.target_pad.min((right - left) / 2.);
        let tx = o.x.clamp(left + pad, right - pad);
        let edge = if v.surface_chase_ticks > 0 {
            let (left, right, _) = f.surf.map_or((s.left, s.right, s.top), |id| crate::physics::surface(d, w, id));
            if dx > 0. { right } else { left }
        } else if dx > 0. {
            s.right
        } else {
            s.left
        };
        if destination.is_some() && f.ground && (f.x - edge).abs() <= v.target_pad {
            inp.jump = true;
            ai.up_t = v.jump_hold;
        }
        if (tx - f.x).abs() > v.target_deadzone {
            toward(&mut inp, tx - f.x);
        }
        if f.ground && st == State::Idle {
            if o.state == State::Attack
                && adx < v.defend_distance
                && dy.abs() < v.defend_height
                && w.rng.next_f64() < cfg.defend[lv]
            {
                if c.ai.counter_chance > 0. && w.rng.next_f64() < c.ai.counter_chance {
                    return Intent { down: true, sp: true, ..Default::default() };
                }
                ai.shield_t = libm::floor(w.rng.range(v.shield_hold_min, v.shield_hold_max));
            }
            for p in &w.projs {
                if w.opponents(i, p.owner)
                    && sign(p.vx) == sign(f.x - p.x)
                    && (p.x - f.x).abs() < v.projectile_distance
                    && (p.y - (f.y - v.projectile_target_height)).abs() < v.projectile_height
                    && w.rng.next_f64() < cfg.projectile_defend[lv]
                {
                    if w.rng.next_f64() < v.projectile_shield_chance {
                        ai.shield_t = v.projectile_shield_frames;
                    } else {
                        inp.jump = true;
                        ai.up_t = v.jump_hold;
                    }
                }
            }
            if ai.shield_t > 0. {
                return Intent { shield: true, ..Default::default() };
            }
        }
        if dy < -v.jump_target_height && adx < v.jump_target_distance && f.ground && w.rng.next_f64() < v.jump_chance {
            inp.jump = true;
            ai.up_t = v.jump_hold;
        }
        if f.ground
            && f.surf.is_some_and(|s| s >= d.stages[w.stage].blocks.len())
            && dy > v.drop_height
            && adx < v.drop_distance
            && w.rng.next_f64() < v.drop_chance
        {
            inp.down_p = true;
        }
        if f.item.is_none() {
            ai.item_t = 0;
        } else if matches!(st, State::Idle | State::Air) {
            // Carry a held item toward the target and smash-throw it once in range (or after `items.cpuHold`).
            ai.item_t = ai.item_t.saturating_add(1);
            let items = &d.global.items;
            if adx < items.cpu_range && dy.abs() < v.attack_height || ai.item_t >= items.cpu_hold {
                ai.item_t = 0;
                toward(&mut inp, tdir);
                (inp.tap_r, inp.tap_l, inp.up, inp.down, inp.jump, inp.atk) = (tdir > 0., tdir < 0., false, false, false, true);
            }
            return inp;
        }
        if ai.cool > 0. {
            ai.cool -= 1.;
            return inp;
        }
        let punish = matches!(o.state, State::Dizzy | State::Knockdown);
        if !punish && w.rng.next_f64() > cfg.aggression[lv] {
            return inp;
        }
        let r = w.rng.next_f64();
        if adx < c.ai.reach && dy.abs() < v.attack_height {
            inp.jump = false;
            inp.up = false;
            if f.ground && o.state == State::Shield {
                toward(&mut inp, tdir);
                inp.grab = true;
            } else if f.ground && punish {
                toward(&mut inp, tdir);
                inp.tap_r = tdir > 0.;
                inp.tap_l = tdir < 0.;
                inp.atk = true;
                ai.charge_t = if o.state == State::Dizzy { v.punish_dizzy_charge } else { v.punish_charge };
            } else if f.ground && r < v.smash_chance {
                let r = w.rng.next_f64();
                if r < v.smash_forward {
                    toward(&mut inp, tdir);
                    inp.tap_r = tdir > 0.;
                    inp.tap_l = tdir < 0.;
                } else {
                    inp.left = false;
                    inp.right = false;
                    if r < v.smash_down {
                        inp.tap_d = true;
                    } else {
                        inp.tap_u = true;
                    }
                }
                inp.atk = true;
                ai.charge_t = libm::floor(w.rng.range(0., cfg.charge[lv]));
            } else if f.ground && r < v.grab_chance {
                toward(&mut inp, tdir);
                inp.grab = true;
            } else if r < v.tilt_chance || tdir != f.face {
                toward(&mut inp, tdir);
                inp.atk = true;
            } else if r < v.down_tilt_chance {
                inp.left = false;
                inp.right = false;
                inp.down = true;
                inp.atk = true;
            } else if r < v.jab_chance {
                inp.left = false;
                inp.right = false;
                inp.atk = true;
            } else {
                inp.left = false;
                inp.right = false;
                inp.down = true;
                inp.sp = true;
            }
        } else if dy < -v.up_attack_height && adx < v.up_attack_distance {
            inp.up = true;
            inp.atk = true;
        } else if dy > v.down_attack_height && adx < v.down_attack_distance && !f.ground {
            inp.down = true;
            inp.atk = true;
        } else {
            let mut used = false;
            for t in &c.ai.specials {
                if (!t.grounded || f.ground)
                    && adx > t.min_distance
                    && t.max_distance.is_none_or(|max| adx < max)
                    && dy.abs() < t.height
                    && (!t.no_projectile || !w.projs.iter().any(|p| p.owner == i))
                    && (!t.neutral || f.ground && st == State::Idle && f.lag == 0)
                    && w.rng.next_f64() < t.chance
                {
                    toward(&mut inp, tdir);
                    if t.neutral {
                        w.fighters[i].face = tdir;
                        inp.left = false;
                        inp.right = false;
                        inp.up = false;
                        inp.down = false;
                    }
                    inp.down = t.down || inp.down;
                    inp.sp = true;
                    if let Some([a, b]) = t.hold {
                        ai.sp_t = libm::floor(w.rng.range(a, b));
                    }
                    used = true;
                    break;
                }
            }
            if !used {
                return inp;
            }
        }
        let react = cfg.react[lv];
        ai.cool = react + libm::floor(w.rng.range(0., react));
        inp
    })();
    w.fighters[i].ai = f.ai;
    result
}
