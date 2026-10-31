//! Pose resolution and animation sampling: which animation a fighter shows, at what time, and the resulting
//! skeleton in fighter-local space (x forward, y up, feet at the origin).

use sim::GameData;
use sim::data::Fighter as FighterData;
use sim::data::visual::{Anim, AnimKey, Clock, Pose, PoseRef};
use sim::world::{Fighter, State};

use crate::canvas::P;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Skel {
    pub head: P,
    pub shoulder: P,
    pub hip: P,
    pub elbow_f: P,
    pub hand_f: P,
    pub elbow_b: P,
    pub hand_b: P,
    pub knee_f: P,
    pub foot_f: P,
    pub knee_b: P,
    pub foot_b: P,
    /// Degrees, positive pitches forward, around `(0, pivot)`.
    pub rot: f64,
    pub pivot: f64,
    pub offset_y: f64,
}

impl Skel {
    fn joints_mut(&mut self) -> [&mut P; 11] {
        [
            &mut self.head,
            &mut self.shoulder,
            &mut self.hip,
            &mut self.elbow_f,
            &mut self.hand_f,
            &mut self.elbow_b,
            &mut self.hand_b,
            &mut self.knee_f,
            &mut self.foot_f,
            &mut self.knee_b,
            &mut self.foot_b,
        ]
    }
    fn apply(&mut self, p: &Pose) {
        let src = [
            p.head, p.shoulder, p.hip, p.elbow_f, p.hand_f, p.elbow_b, p.hand_b, p.knee_f, p.foot_f, p.knee_b, p.foot_b,
        ];
        for (dst, s) in self.joints_mut().into_iter().zip(src) {
            if let Some(s) = s {
                *dst = s;
            }
        }
        if let Some(v) = p.rot {
            self.rot = v;
        }
        if let Some(v) = p.pivot {
            self.pivot = v;
        }
        if let Some(v) = p.offset_y {
            self.offset_y = v;
        }
    }
    pub fn lerp(&self, o: &Skel, t: f64) -> Skel {
        let mut out = *self;
        let mut b = *o;
        for (a, b) in out.joints_mut().into_iter().zip(b.joints_mut()) {
            *a = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        }
        out.rot = self.rot + (o.rot - self.rot) * t;
        out.pivot = self.pivot + (o.pivot - self.pivot) * t;
        out.offset_y = self.offset_y + (o.offset_y - self.offset_y) * t;
        out
    }
    /// Blend between ticks, taking the short way around for rotation (loops wrap 360 → 0).
    pub fn blend(&self, o: &Skel, t: f64) -> Skel {
        let mut target = *o;
        let d = (o.rot - self.rot).rem_euclid(360.);
        target.rot = self.rot + if d > 180. { d - 360. } else { d };
        self.lerp(&target, t)
    }
}

/// Full skeleton for a pose: `stand`, then the `from` chain (outermost first), then the pose itself.
pub fn resolve(fd: &FighterData, p: &Pose) -> Skel {
    let mut chain = vec![p];
    let mut cur = p;
    while let Some(next) = cur.from.as_deref().and_then(|n| fd.poses.get(n)) {
        if chain.iter().any(|c| std::ptr::eq(*c, next)) {
            break; // a `from` cycle: stop at the first repeat
        }
        chain.push(next);
        cur = next;
    }
    let mut s = Skel::default();
    if let Some(stand) = fd.poses.get("stand") {
        s.apply(stand);
    }
    for p in chain.iter().rev() {
        s.apply(p);
    }
    s
}

fn resolve_ref(fd: &FighterData, r: &PoseRef) -> Skel {
    match r {
        PoseRef::Named(n) => fd.poses.get(n).map(|p| resolve(fd, p)).unwrap_or_else(|| resolve(fd, &Pose::default())),
        PoseRef::Inline(p) => resolve(fd, p),
    }
}

/// Samples keys at time `t`: the last key at or before `t` holds, unless the next key lerps toward itself.
pub fn sample(fd: &FighterData, keys: &[AnimKey], t: f64, period: Option<f64>, f: &Fighter) -> Skel {
    let t = match period {
        Some(p) if p > 0. => t.rem_euclid(p),
        _ => t,
    };
    let live: Vec<&AnimKey> = keys.iter().filter(|k| sim::moves::cond(f, k.cond)).collect();
    let Some(first) = live.first() else {
        return resolve(fd, &Pose::default());
    };
    let i = live.iter().rposition(|k| k.at <= t).unwrap_or(0);
    let cur = if t < first.at { first } else { live[i] };
    let a = resolve_ref(fd, &cur.pose);
    match live.get(i + 1) {
        Some(next) if next.lerp && t >= cur.at && next.at > cur.at => {
            a.lerp(&resolve_ref(fd, &next.pose), ((t - cur.at) / (next.at - cur.at)).clamp(0., 1.))
        }
        _ => a,
    }
}

/// State animation name and its clock value, per `docs/FORMAT.md`.
pub fn state_anim(d: &GameData, f: &Fighter) -> (String, Option<f64>) {
    let g = &d.global;
    let name: String = match f.state {
        State::Idle if f.vx.abs() > g.render.overlays.run_speed => "run".into(),
        State::Air if f.flip_t > 0 => {
            return ("air.flip".into(), Some(f64::from(g.sim.flip_frames.saturating_sub(f.flip_t))));
        }
        State::Air => if f.vy < 0. { "air.rise" } else { "air.fall" }.into(),
        State::Landing if f.sd.kind == "shield" => "shieldDrop".into(),
        State::Hitstun if f.tumble => "tumble".into(),
        State::Getup if f.sd.kind == "atk" => "getupAttack".into(),
        State::Climb => match f.sd.kind.as_str() {
            "attack" => "ledgeAttack",
            "roll" => "ledgeRoll",
            "jump" => "ledgeJump",
            _ => "climb",
        }
        .into(),
        State::Hold if f.sd.pt > 0 => {
            return ("pummel".into(), Some(f64::from(g.grab.pummel_frames.saturating_sub(f.sd.pt))));
        }
        State::Throw => format!("throw.{}", f.sd.kind),
        s => s.name().into(),
    };
    (name, None)
}

fn clock_value(a: &Anim, f: &Fighter, frame: u64) -> f64 {
    match a.clock {
        Clock::State => f64::from(f.t),
        Clock::World => frame as f64,
        Clock::Run => f.run_ph.to_degrees(),
        Clock::Spin => f.spin.to_degrees(),
    }
}

/// The fighter's skeleton this tick (before attachment `hold` posing).
pub fn fighter_pose(d: &GameData, f: &Fighter, frame: u64) -> Skel {
    let fd = &d.fighters[f.data];
    if f.state == State::Attack && f.mov.is_some() {
        let ph = sim::moves::phase(d, f);
        let t = f64::from(f.mov.as_ref().map_or(0, |m| m.t));
        return sample(fd, &ph.anim, t, None, f);
    }
    let (name, clock) = state_anim(d, f);
    match fd.anims.get(&name) {
        Some(a) => sample(fd, &a.keys, clock.unwrap_or_else(|| clock_value(a, f, frame)), a.period, f),
        None => resolve(fd, &Pose::default()),
    }
}
