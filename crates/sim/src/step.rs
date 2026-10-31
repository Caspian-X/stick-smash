use crate::{
    GameData, Input, World,
    input::Intent,
    world::{State, sign},
};
/// Advance one tick, preserving fighter/pair index order and RNG call order.
pub fn step_world(w: &mut World, d: &GameData, inputs: &[Input]) {
    let g = &d.global;
    w.frame += 1;
    if w.count > 0 {
        if w.count.is_multiple_of(crate::TICK_HZ) {
            w.sfx("beep");
        }
        w.count -= 1;
        if w.count == 0 {
            w.go_t = g.match_.go_frames;
            w.sfx("go");
        }
    }
    w.go_t = w.go_t.saturating_sub(1);
    let frozen = w.count > 0 || w.end_t > 0;
    let mut ins = Vec::with_capacity(w.fighters.len());
    for i in 0..w.fighters.len() {
        let p = &w.cfg.players[i];
        let inp = if let Some(level) = p.cpu {
            if frozen { Intent::default() } else { crate::ai::ai_input(w, d, i, level) }
        } else {
            let intent = w.fighters[i].history.derive(
                inputs.get(i).copied().unwrap_or_default(),
                w.frame,
                g.input.stick_threshold,
                g.movement.tap_window,
                p.tap_jump,
            );
            if frozen { Intent::default() } else { intent }
        };
        ins.push(inp);
    }
    for (i, inp) in ins.into_iter().enumerate() {
        crate::states::update_fighter(w, d, i, inp);
    }
    crate::combat::hit_pass(w, d);
    crate::projectiles::step_projectiles(w, d);
    let no_push = |s| {
        matches!(
            s,
            State::Out
                | State::Dead
                | State::Respawn
                | State::Ledge
                | State::Climb
                | State::Grabbed
                | State::Hold
                | State::Throw
                | State::Win
                | State::Roll
                | State::Spot
                | State::Airdodge
        )
    };
    for a in 0..w.fighters.len() {
        for b in a + 1..w.fighters.len() {
            if no_push(w.fighters[a].state) || no_push(w.fighters[b].state) {
                continue;
            }
            let dx = w.fighters[b].x - w.fighters[a].x;
            let r = g.movement.push_radius;
            if dx.abs() < r && (w.fighters[a].y - w.fighters[b].y).abs() < g.sim.push_height {
                let s = if dx == 0. { 1. } else { sign(dx) };
                let push = (r - dx.abs()) * g.movement.push_strength;
                w.fighters[a].x -= s * push;
                w.fighters[b].x += s * push;
            }
        }
    }
    let b = &d.stages[w.stage].blast;
    for i in 0..w.fighters.len() {
        let f = &w.fighters[i];
        if w.end_t > 0
            || w.over
            || matches!(f.state, State::Out | State::Dead | State::Respawn | State::Grabbed | State::Win)
        {
            continue;
        }
        if f.x < b.left || f.x > b.right || f.y > b.bottom || f.y < b.top {
            crate::states::ko(w, d, i);
        }
    }
    if !w.cfg.training && w.end_t == 0 && !w.over {
        let living: Vec<_> = w.fighters.iter().enumerate().filter(|(_, f)| f.stocks > 0).map(|(i, _)| i).collect();
        if living.is_empty()
            || living.iter().all(|&i| {
                i == living[0]
                    || w.cfg.players[i].team.is_some() && w.cfg.players[i].team == w.cfg.players[living[0]].team
            })
        {
            w.winner = living.first().copied();
            w.end_t = g.match_.end_frames;
        }
    }
    if let Some(tr) = &mut w.trace
        && !tr.done
    {
        if tr.a_free < 0 && w.fighters[tr.attacker].actionable() {
            tr.a_free = w.frame as i64;
        }
        if tr.d_free < 0 && w.fighters[tr.defender].actionable() {
            tr.d_free = w.frame as i64;
        }
        if tr.a_free >= 0 && tr.d_free >= 0 {
            tr.done = true;
        }
    }
    if w.end_t > 0 {
        w.end_t -= 1;
        if w.end_t == 0 {
            w.over = true;
            if let Some(win) = w.winner {
                let team = w.cfg.players[win].team;
                for i in 0..w.fighters.len() {
                    if i == win || team.is_some() && w.cfg.players[i].team == team && w.fighters[i].stocks > 0 {
                        crate::combat::release_grab(w, d, i);
                        let f = &mut w.fighters[i];
                        f.set_state(State::Win);
                        f.vx = 0.;
                        f.vy = 0.;
                    }
                }
            }
        }
    }
}
