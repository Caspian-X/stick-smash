//! Stable feet-relative interaction geometry, independent of animated poses.
use crate::{
    GameData,
    data::{self, Pt},
    world::Fighter,
};
use data::fighter::{GrabInteraction, LedgeInteraction};
pub fn grab(f: &data::Fighter) -> Option<&GrabInteraction> {
    f.interactions.as_ref()?.grab.as_ref()
}
pub fn ledge(f: &data::Fighter) -> Option<&LedgeInteraction> {
    f.interactions.as_ref()?.ledge.as_ref()
}
pub fn forward_point(f: &Fighter, p: Pt) -> Pt {
    [f.x + f.face * p[0], f.y - p[1]]
}
pub fn inverse_anchor(target: Pt, face: f64, anchor: Pt) -> Pt {
    [target[0] - face * anchor[0], target[1] + anchor[1]]
}
pub fn hold_point(d: &GameData, f: &Fighter) -> Pt {
    grab(&d.fighters[f.data]).and_then(|g| g.hold_point).unwrap_or([d.global.sim.grab_offset, 0.])
}
pub fn held_anchor(d: &GameData, f: &Fighter) -> Pt {
    grab(&d.fighters[f.data]).and_then(|g| g.held_anchor).unwrap_or([0., 0.])
}
pub fn grab_overridden(d: &GameData, a: &Fighter, v: &Fighter) -> bool {
    grab(&d.fighters[a.data]).is_some() || grab(&d.fighters[v.data]).is_some()
}
pub fn place_held(d: &GameData, holder: &Fighter, victim: &mut Fighter) {
    victim.face = -holder.face;
    if grab_overridden(d, holder, victim) {
        let p = inverse_anchor(forward_point(holder, hold_point(d, holder)), victim.face, held_anchor(d, victim));
        victim.x = p[0];
        victim.y = p[1];
        victim.surf = None;
    } else {
        victim.x = holder.x + holder.face * d.global.sim.grab_offset;
        victim.y = holder.y;
    }
    victim.ground = holder.ground;
}
pub fn grab_vertical_distance(d: &GameData, a: &Fighter, v: &Fighter) -> f64 {
    if !grab_overridden(d, a, v) {
        return (v.y - a.y).abs();
    }
    let catch = grab(&d.fighters[a.data]).and_then(|g| g.catch_point).unwrap_or([0., 0.]);
    (forward_point(v, held_anchor(d, v))[1] - forward_point(a, catch)[1]).abs()
}
pub fn ledge_hang_root(f: &data::Fighter, s: &data::Stage, edge: f64, top: f64, side: f64) -> Pt {
    if let Some(anchor) = ledge(f).and_then(|l| l.hang_anchor) {
        inverse_anchor([edge, top], -side, anchor)
    } else {
        [edge + side * s.ledge.hang_x, top + s.ledge.hang_y]
    }
}
