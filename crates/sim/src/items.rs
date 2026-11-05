//! Held items, Smash-style: holding, throwing (the `toss` state), picking up, catching and loose items.
//!
//! Items are content (`Fighter.items`); nothing here knows a fighter or item id. A fighter holds at most one
//! (`Fighter::item`), granted by a move event's `item`. Attacks and aerials throw it: after `toss.release` ticks the
//! item's projectile is launched, tagged with the item (`Projectile::item`). A thrown item that lands, connects or
//! runs out of life becomes a [`LooseItem`]: it falls, bounces, rests on surfaces and expires after `life` ticks.
//! Attack or grab picks up a landed item on the ground; attack, grab or air dodge catches one in flight.
use crate::actions::{Action, accept};
use crate::physics::*;
use crate::{
    GameData, World,
    data::{Item, Pt, Rect, global::Toss},
    input::{Intent, Strike},
    world::{Fighter, Held, LooseItem, Projectile, State, sign},
};

pub fn def<'a>(d: &'a GameData, h: &Held) -> &'a Item {
    &d.fighters[h.data].items[&h.id]
}

/// The item's projectile for a throw (`smash`: `smashProjectile`).
pub fn projectile<'a>(d: &'a GameData, h: &Held, smash: bool) -> &'a str {
    let it = def(d, h);
    if smash { it.smash_projectile.as_deref().unwrap_or(&it.projectile) } else { &it.projectile }
}

fn timing(d: &GameData, h: &Held) -> Toss {
    def(d, h).toss.unwrap_or(d.global.items.toss)
}

/// Put item `id` of the fighter's own package in its hand (a move event's `item`); nothing if it holds one.
pub fn give(f: &mut Fighter, id: &str) {
    if f.item.is_none() {
        f.item = Some(Held { data: f.data, id: id.into() });
    }
}

/// The throw an attack press asks for: (`f` / `b` / `u` / `d`, smash). The attack stick and attack + special throw
/// toward their direction (smash per the stick setting, always for the combo); the button smash-throws on a
/// recent flick, tilt-throws toward a held direction and forward when neutral (a dash throws forward hard).
pub fn aim(d: &GameData, f: &Fighter, inp: &Intent) -> (&'static str, bool) {
    let side = |x: f64| if x != 0. && x == -f.face { "b" } else { "f" };
    match inp.strike {
        Strike::Stick { x, y, smash } => (
            if y > 0 {
                "u"
            } else if y < 0 {
                "d"
            } else {
                side(f64::from(x))
            },
            smash,
        ),
        Strike::Combo => (
            if inp.up {
                "u"
            } else if inp.down {
                "d"
            } else {
                side(inp.dir())
            },
            true,
        ),
        Strike::Button if inp.tap_r || inp.tap_l => (side(if inp.tap_r { 1. } else { -1. }), true),
        Strike::Button if inp.tap_u => ("u", true),
        Strike::Button if inp.tap_d => ("d", true),
        Strike::Button if inp.up => ("u", false),
        Strike::Button if inp.down => ("d", false),
        Strike::Button => (side(inp.dir()), f.ground && crate::gait::dashing(d, f)),
    }
}

/// The throw replacing a short-hop aerial picked during jumpsquat (`nair`, `fair`… slot names).
pub fn aerial_kind(slot: &str) -> &'static str {
    match slot {
        "bair" => "b",
        "uair" => "u",
        "dair" => "d",
        _ => "f",
    }
}

/// Start throwing the held item. A back throw turns the fighter around first.
pub fn toss(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, kind: &str, smash: bool) {
    let Some(h) = f.item.clone() else { return };
    accept(w, i, Action::Toss(kind.into()));
    f.set_state(State::Toss);
    if kind == "b" {
        f.face = -f.face;
    }
    f.sd.kind = kind.into();
    f.sd.n = if smash { 1. } else { 0. };
    // The item is gone by the end of the throw, so its length is kept on the state.
    f.sd.end = timing(d, &h).frames;
}

/// Attack (or grab) while holding an item, in the air: a throw by aerial direction, or a drop for grab.
pub fn air_toss(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: &Intent) {
    let (kind, smash) = if inp.grab { ("drop", false) } else { aim(d, f, inp) };
    toss(w, d, i, f, kind, smash);
}

/// One tick of the `toss` state: attack movement, the release, then back to idle / air.
pub fn st_toss(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, inp: Intent) {
    let g = &d.global;
    if f.ground {
        skid(d, f, g.sim.landing_friction, 1.);
        fall(w, d, i, f, true, None);
    } else {
        let acc = air_accel(d, f, g.movement.attack_air_accel, |u| u.attack_air_accel_mult);
        air_phys(w, d, i, f, inp, inp.x, acc);
    }
    if let Some(h) = f.item.clone()
        && f.t >= timing(d, &h).release
    {
        release(w, d, i, f, &h);
    }
    if f.state == State::Toss && f.t >= f.sd.end {
        f.set_state(if f.ground { State::Idle } else { State::Air });
    }
}

fn velocity(it: &Item, kind: &str, smash: bool) -> Pt {
    let s = match kind {
        "b" => &it.throws.back,
        "u" => &it.throws.up,
        "d" => &it.throws.down,
        "drop" => return it.throws.drop,
        _ => &it.throws.forward,
    };
    if smash { s.smash } else { s.tilt }
}

/// Launch the held item's projectile from its `hold` point at the throw's velocity.
fn release(w: &mut World, d: &GameData, i: usize, f: &mut Fighter, h: &Held) {
    let it = def(d, h);
    let (kind, smash) = (f.sd.kind.clone(), f.sd.n > 0.);
    let id = projectile(d, h, smash);
    let pd = &d.fighters[h.data].projectiles[id];
    let v = velocity(it, &kind, smash);
    let mut p = crate::projectiles::make(w, d, i, f, h.data, id);
    (p.x, p.y) = (f.x + f.face * it.hold[0], f.y - it.hold[1]);
    (p.vx, p.vy) = (f.face * v[0], -v[1]);
    if it.speed_damage != 0. {
        let faster = libm::hypot(v[0], v[1]) - libm::hypot(pd.vel[0], pd.vel[1]);
        p.hit.damage = (p.hit.damage + it.speed_damage * faster).max(0.);
    }
    p.item = Some(h.id.clone());
    w.projs.push(p);
    if let Some(s) = &pd.spawn_sfx {
        w.sfx(s.clone());
    }
    f.item = None;
}

/// A thrown item whose flight ended becomes loose where it is: with its own velocity, or bouncing off (`rebound`)
/// when its hit `connected` (without `rebound` the hit uses it up). Clears the projectile's item tag.
pub fn loosen(w: &mut World, d: &GameData, p: &mut Projectile, connected: bool) {
    let Some(id) = p.item.take() else { return };
    let it = &d.fighters[p.fighter_data].items[&id];
    let (vx, vy) = match (connected, it.rebound) {
        (false, _) => (p.vx, p.vy),
        (true, Some(r)) => (-sign(p.vx) * r[0], -r[1]),
        (true, None) => return,
    };
    w.items.push(LooseItem {
        item: Held { data: p.fighter_data, id },
        owner: p.owner,
        x: p.x,
        y: p.y,
        vx,
        vy,
        surf: None,
        life: it.life,
        age: 0,
    });
}

fn inside(f: &Fighter, r: &Rect, x: f64, y: f64) -> bool {
    let (lx, ly) = ((x - f.x) * f.face, f.y - y);
    lx >= r.x1 && lx <= r.x2 && ly >= r.y1 && ly <= r.y2
}

/// Take an item within reach into an empty hand: a landed one on the ground (with `items.pickupLag`), else one in
/// flight (loose and falling, or thrown) that can be caught. True when one was taken (the press is used).
pub fn take(w: &mut World, d: &GameData, i: usize, f: &mut Fighter) -> bool {
    let g = &d.global.items;
    if f.item.is_some() || w.items.is_empty() && !w.projs.iter().any(|p| p.item.is_some()) {
        return false;
    }
    let landed = w.items.iter().position(|it| it.surf.is_some() && f.ground && inside(f, &g.pickup, it.x, it.y));
    if let Some(k) = landed {
        f.item = Some(w.items.remove(k).item);
        accept(w, i, Action::Pickup);
        landing(f, g.pickup_lag);
        f.sd.kind = "pickup".into();
        return true;
    }
    let own = |owner: usize, age: u32| owner == i && age < g.regrab;
    let falling = w.items.iter().position(|it| {
        it.surf.is_none() && def(d, &it.item).catch && !own(it.owner, it.age) && inside(f, &g.catch, it.x, it.y)
    });
    let held = if let Some(k) = falling {
        w.items.remove(k).item
    } else {
        let Some(p) = w.projs.iter_mut().find(|p| {
            p.life > 0
                && p.item.as_ref().is_some_and(|id| d.fighters[p.fighter_data].items[id].catch)
                && !own(p.owner, p.age)
                && inside(f, &g.catch, p.x, p.y)
        }) else {
            return false;
        };
        p.life = 0;
        Held { data: p.fighter_data, id: p.item.take().unwrap() }
    };
    f.item = Some(held);
    accept(w, i, Action::Catch);
    true
}

/// Move loose items: gravity and fall cap from their projectile, bounces (`bounce.restitution` / `friction`) until
/// they settle below `items.settle`, stage walls, riding moving platforms; expired or out-of-bounds ones vanish.
pub fn step(w: &mut World, d: &GameData) {
    if w.items.is_empty() {
        return;
    }
    let s = &d.stages[w.stage];
    let g = &d.global;
    for k in 0..w.items.len() {
        let mut it = w.items[k].clone();
        let pd = &d.fighters[it.item.data].projectiles[&def(d, &it.item).projectile];
        let b = pd.bounce.as_ref();
        it.age = it.age.saturating_add(1);
        it.life = it.life.saturating_sub(1);
        if let Some(id) = it.surf {
            let (x1, x2, y) = surface(d, w, id);
            if let Some(platform) = id.checked_sub(s.blocks.len()).and_then(|i| s.platforms.get(i)) {
                let (old_x, _, old_y) = platform.position(w.frame.saturating_sub(1) as f64);
                it.x += x1 - old_x;
                it.y += y - old_y;
            }
            if it.x >= x1 && it.x <= x2 {
                w.items[k] = it;
                continue;
            }
            it.surf = None;
        }
        let py = it.y;
        it.vy = (it.vy + pd.gravity).min(pd.max_fall.unwrap_or(f64::INFINITY));
        it.x += it.vx;
        it.y += it.vy;
        if it.vy > 0. {
            for si in 0..s.blocks.len() + s.platforms.len() {
                let (x1, x2, y) = surface(d, w, si);
                if py + pd.r > y + g.sim.surface_tolerance
                    || it.y + pd.r < y
                    || it.x < x1 - g.sim.projectile_pad
                    || it.x > x2 + g.sim.projectile_pad
                {
                    continue;
                }
                it.y = y - pd.r;
                let up = it.vy * b.map_or(0., |b| b.restitution);
                if up > g.items.settle {
                    it.vy = -up;
                    it.vx *= b.map_or(0., |b| b.friction);
                } else {
                    (it.vx, it.vy, it.surf) = (0., 0., Some(si));
                }
                break;
            }
        }
        for s in &s.blocks {
            if it.y + pd.r > s.top + g.sim.projectile_wall_top
                && it.y - pd.r < s.bottom
                && it.x > s.left - pd.r
                && it.x < s.right + pd.r
            {
                it.vx = -it.vx * b.map_or(0., |b| b.wall_restitution);
                it.x = if it.x < (s.left + s.right) / 2. { s.left - pd.r } else { s.right + pd.r };
            }
        }
        w.items[k] = it;
    }
    let b = &s.blast;
    w.items.retain(|it| it.life > 0 && it.x > b.left && it.x < b.right && it.y < b.bottom);
}
