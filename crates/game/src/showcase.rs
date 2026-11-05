//! The select screens' showcase: the lineup standing on a stage behind the UI, in card order, swapping fighters in
//! place as picks change, and posing (idle, the lock-in flourish, the win pose).

use std::sync::Arc;

use bevy::prelude::*;
use sim::GameData;
use sim::world::State as FState;

use crate::play::{self, Match, Mode, Rules, Slot};

/// Keeps the showcase (the lineup standing on `rules.stage`, which must fit it) in step: rebuilt when the lineup's
/// size, colors or stage change, standing left to right in card order; otherwise a changed pick is swapped in place
/// so nobody else moves. Everyone is on one team so flourishes never hit anybody. `None` while it is being rebuilt.
pub fn showcase<'a>(
    commands: &mut Commands,
    scene: &mut render::Scene,
    m: Option<&'a mut Match>,
    data: &Arc<GameData>,
    mslots: &[Slot],
    rules: &Rules,
) -> Option<&'a mut Match> {
    if mslots.is_empty() {
        return None;
    }
    if !data.stage(&rules.stage).is_some_and(|s| s.max_players() >= mslots.len()) {
        return None;
    }
    let m = m.filter(|m| {
        m.mode == Mode::Showcase
            && m.slots.iter().map(|s| s.color).eq(mslots.iter().map(|s| s.color))
            && m.rules.stage == rules.stage
    });
    let Some(m) = m else {
        let mut slots = mslots.to_vec();
        for s in &mut slots {
            s.cpu = None;
            s.dev = None;
            s.team = Some(0);
        }
        let rules = Rules { friendly_fire: false, ..rules.clone() };
        if let Ok(mut m) = Match::new(data, Mode::Showcase, slots, rules, 1) {
            let mut spots: Vec<_> = m.world.fighters.iter().map(|f| (f.x, f.y, f.surf, f.ground, f.face)).collect();
            spots.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (f, (x, y, surf, ground, face)) in m.world.fighters.iter_mut().zip(spots) {
                (f.x, f.y, f.surf, f.ground, f.face) = (x, y, surf, ground, face);
            }
            play::install(commands, scene, m);
        }
        return None;
    };
    for (i, s) in mslots.iter().enumerate() {
        if m.slots[i].fighter != s.fighter {
            swap_fighter(m, i, s.fighter);
        }
    }
    Some(m)
}

/// Put another fighter in showcase slot `i`, where it stands.
fn swap_fighter(m: &mut Match, i: usize, fighter: usize) {
    let f = &mut m.world.fighters[i];
    f.data = fighter;
    f.mov = None;
    f.item = None;
    f.jumps = m.data.fighters[fighter].stats.air_jumps;
    f.set_state(if f.ground { FState::Idle } else { FState::Air });
    m.world.projs.retain(|p| p.owner != i);
    m.world.items.retain(|it| it.owner != i);
    m.world.cfg.players[i].fighter = m.data.fighters[fighter].id.clone();
    m.slots[i].fighter = fighter;
}

/// Ready fighters stand in their win pose, the rest idle. A ready fighter listed in `flourish` first plays
/// `render.select.flourish` (once idle; on the ground, if it has that move) and leaves the list.
pub fn pose(m: &mut Match, ready: impl Fn(usize) -> bool, flourish: &mut Vec<usize>) {
    let slot = &m.data.global.render.select.flourish;
    for i in 0..m.world.fighters.len() {
        let ready = ready(i);
        let f = &mut m.world.fighters[i];
        if ready && f.state == FState::Idle && flourish.contains(&i) {
            flourish.retain(|j| *j != i);
            if f.ground && m.data.fighters[f.data].moves.iter().any(|(s, _)| s == slot.as_str()) {
                let mut f = f.clone();
                sim::moves::start_move(&mut m.world, &m.data, i, &mut f, slot);
                m.world.fighters[i] = f;
            } else {
                f.set_state(FState::Win);
            }
        } else if ready && f.state == FState::Idle {
            f.set_state(FState::Win);
        } else if !ready && f.state == FState::Win {
            f.set_state(FState::Idle);
        }
    }
}
