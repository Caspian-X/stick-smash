//! Pose resolution and animation sampling: which animation a fighter shows, at what time, and the resulting
//! skeleton in fighter-local space (x forward, y up, feet at the origin).

use crate::GameData;
use crate::data::Fighter as FighterData;
use crate::data::visual::{Anim, AnimKey, Clock, Pose, PoseRef};
use crate::world::{Fighter, Gait, State};

use crate::data::Pt as P;

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

/// Full point transform from fighter-local (x forward, y up) to stage world (y down).
/// Body space includes whole-pose rotation and vertical offset, before facing/placement.
/// Positive pose rotation pitches forward: the local rotation is **negative** degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyTransform {
    pub root: P,
    /// Facing must be +1 or -1.
    pub facing: f64,
    pub rotation: f64,
    pub pivot: f64,
    pub offset_y: f64,
}

impl BodyTransform {
    pub fn new(skel: &Skel, root: P, facing: f64) -> Self {
        Self { root, facing, rotation: skel.rot, pivot: skel.pivot, offset_y: skel.offset_y }
    }
    pub fn local_to_body(self, point: P) -> P {
        let (s, c) = (libm::sin(self.rotation.to_radians()), libm::cos(self.rotation.to_radians()));
        let y = point[1] - self.pivot;
        [c * point[0] + s * y, -s * point[0] + c * y + self.pivot + self.offset_y]
    }
    pub fn body_to_local(self, point: P) -> P {
        let (s, c) = (libm::sin(self.rotation.to_radians()), libm::cos(self.rotation.to_radians()));
        let y = point[1] - self.pivot - self.offset_y;
        [c * point[0] - s * y, s * point[0] + c * y + self.pivot]
    }
    pub fn body_to_world(self, point: P) -> P {
        [self.root[0] + self.facing * point[0], self.root[1] - point[1]]
    }
    pub fn world_to_body(self, point: P) -> P {
        [(point[0] - self.root[0]) / self.facing, self.root[1] - point[1]]
    }
    pub fn local_to_world(self, point: P) -> P {
        self.body_to_world(self.local_to_body(point))
    }
    pub fn world_to_local(self, point: P) -> P {
        self.body_to_local(self.world_to_body(point))
    }
}

/// Tick-resolved named joints plus whole-body placement. No interpolation or cosmetic effects.
/// Keep accumulated joint orientation in local space; reflection cannot be represented by an angle alone.
#[derive(Clone, Debug, PartialEq)]
pub struct FighterRigTransform {
    pub body: BodyTransform,
    pub joints: crate::rig::ResolvedRig,
}

impl FighterRigTransform {
    /// Transform a joint-relative point, including all parent rotations and whole-body placement.
    pub fn joint_to_world(&self, id: &str, point: P) -> Option<P> {
        Some(self.body.local_to_world(self.joints.get(id)?.point(point)))
    }
    pub fn world_to_joint(&self, id: &str, point: P) -> Option<P> {
        let joint = self.joints.get(id)?;
        let local = self.body.world_to_local(point);
        let (s, c) = (libm::sin(joint.angle.to_radians()), libm::cos(joint.angle.to_radians()));
        let (x, y) = (local[0] - joint.translation[0], local[1] - joint.translation[1]);
        Some([c * x + s * y, -s * x + c * y])
    }
}

/// Returns None for legacy figures and propagates invalid rig errors for named figures.
/// Uses gameplay pose tuning, independent of renderer configuration.
pub fn fighter_rig_transform(
    d: &GameData,
    f: &Fighter,
    frame: u64,
) -> Result<Option<FighterRigTransform>, crate::rig::RigError> {
    let Some(rig) = &d.fighters[f.data].rig else { return Ok(None) };
    Ok(Some(FighterRigTransform {
        body: BodyTransform::new(&fighter_pose(d, f, frame), [f.x, f.y], f.face),
        joints: crate::rig::resolve(rig, &fighter_rig_pose(d, f, frame))?,
    }))
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
    let charge = f.mov.as_ref().map_or(0, |m| m.charge);
    let live: Vec<&AnimKey> = keys.iter().filter(|k| k.min_charge <= charge && crate::moves::cond(f, k.cond)).collect();
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

/// Optional gait clip when a package defines it; legacy figures keep the previous selection.
fn has_clip(d: &GameData, f: &Fighter, name: &str) -> bool {
    d.fighters[f.data].anims.contains_key(name)
}

/// State animation name and its clock value, per `docs/FORMAT.md`.
pub fn state_anim(d: &GameData, f: &Fighter) -> (String, Option<f64>) {
    let g = &d.global;
    let name: String = match f.state {
        State::Idle if f.ground && matches!(f.gait, Gait::Crouch | Gait::Rise) => {
            if f.gait == Gait::Crouch && f.vx.abs() > 0.1 && d.fighters[f.data].anims.contains_key("crawl") {
                "crawl".into()
            } else {
                "crouch".into()
            }
        }
        State::Idle if f.ground && f.gait == Gait::Walk && has_clip(d, f, "walk") => "walk".into(),
        // Optional gait clips are one-shot per gait transition: sample them from the gait clock, not from the
        // state clock, so entering them after any amount of idle time starts at their first key.
        State::Idle if f.ground && f.gait == Gait::Turn && has_clip(d, f, "turn") => {
            return ("turn".into(), Some(f64::from(f.gait_t)));
        }
        State::Idle if f.ground && f.gait == Gait::Brake && has_clip(d, f, "brake") => {
            return ("brake".into(), Some(f64::from(f.gait_t)));
        }
        State::Idle if f.vx.abs() > g.gait.pose_run_speed => "run".into(),
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
        let ph = crate::moves::phase(d, f);
        let t = f64::from(f.mov.as_ref().map_or(0, |m| m.t));
        return sample(fd, &ph.anim, t, None, f);
    }
    let (name, clock) = state_anim(d, f);
    let pose = match fd.anims.get(&name) {
        Some(a) => sample(fd, &a.keys, clock.unwrap_or_else(|| clock_value(a, f, frame)), a.period, f),
        None => resolve(fd, &Pose::default()),
    };
    if name == "crouch" {
        let frames = d.global.gait.crouch_frames;
        let t = if frames == 0 { 1. } else { (f64::from(f.gait_t) / f64::from(frames)).min(1.) };
        let amount = if f.gait == Gait::Rise { 1. - t } else { t };
        let idle = fd.anims.get("idle").map_or_else(
            || resolve(fd, &Pose::default()),
            |a| sample(fd, &a.keys, clock_value(a, f, frame), a.period, f),
        );
        idle.lerp(&pose, amount)
    } else {
        pose
    }
}

/// Named rig pose with every joint resolved to a local transform. Missing overrides use the rig rest pose.
pub fn resolve_rig_pose(fd: &FighterData, p: &Pose) -> crate::data::rig::RigPose {
    use crate::data::rig::JointPose;
    let Some(rig) = &fd.rig else { return Default::default() };
    let mut out: crate::data::rig::RigPose = rig
        .joints
        .iter()
        .map(|j| (j.id.clone(), JointPose { translation: Some(j.translation), angle: Some(j.angle) }))
        .collect();
    let mut chain = vec![p];
    let mut cur = p;
    while let Some(next) = cur.from.as_deref().and_then(|n| fd.poses.get(n)) {
        if chain.iter().any(|c| std::ptr::eq(*c, next)) {
            break;
        }
        chain.push(next);
        cur = next;
    }
    for p in fd.poses.get("stand").into_iter().chain(chain.into_iter().rev()) {
        for (id, joint) in &p.joints {
            let Some(dst) = out.get_mut(id) else { continue };
            if joint.translation.is_some() {
                dst.translation = joint.translation;
            }
            if joint.angle.is_some() {
                dst.angle = joint.angle;
            }
        }
    }
    out
}

fn resolve_rig_ref(fd: &FighterData, r: &PoseRef) -> crate::data::rig::RigPose {
    match r {
        PoseRef::Named(n) => {
            fd.poses.get(n).map(|p| resolve_rig_pose(fd, p)).unwrap_or_else(|| resolve_rig_pose(fd, &Pose::default()))
        }
        PoseRef::Inline(p) => resolve_rig_pose(fd, p),
    }
}

fn lerp_rig_value(a: f64, b: f64, t: f64) -> f64 {
    let delta = b - a;
    if delta.is_finite() { a + delta * t } else { a * (1. - t) + b * t }
}

fn lerp_rig(mut a: crate::data::rig::RigPose, b: &crate::data::rig::RigPose, t: f64) -> crate::data::rig::RigPose {
    for (id, a) in &mut a {
        let Some(b) = b.get(id) else { continue };
        if let (Some(x), Some(y)) = (a.translation, b.translation) {
            a.translation = Some([lerp_rig_value(x[0], y[0], t), lerp_rig_value(x[1], y[1], t)]);
        }
        if let (Some(x), Some(y)) = (a.angle, b.angle) {
            a.angle = Some(lerp_rig_value(x, y, t));
        }
    }
    a
}

/// Blend tick poses for rendering, filling omitted joints/components from the rig's resting pose.
/// Like legacy `Skel::blend`, local rotations take the short path; animation keys use authored sweeps.
pub fn blend_rig_pose(
    fd: &FighterData,
    a: &crate::data::rig::RigPose,
    b: &crate::data::rig::RigPose,
    t: f64,
) -> crate::data::rig::RigPose {
    let mut from = resolve_rig_pose(fd, &Pose { joints: a.clone(), ..Default::default() });
    let mut to = resolve_rig_pose(fd, &Pose { joints: b.clone(), ..Default::default() });
    for (id, joint) in &mut to {
        if let (Some(a), Some(b)) = (from.get(id).and_then(|j| j.angle), joint.angle) {
            let difference = b - a;
            let delta = if difference.is_finite() { difference } else { b.rem_euclid(360.) - a.rem_euclid(360.) }
                .rem_euclid(360.);
            joint.angle = Some(a + if delta > 180. { delta - 360. } else { delta });
        }
    }
    from = lerp_rig(from, &to, t);
    from
}

/// Sample named local transforms using the same key timing and conditions as the legacy skeleton.
pub fn sample_rig(
    fd: &FighterData,
    keys: &[AnimKey],
    t: f64,
    period: Option<f64>,
    f: &Fighter,
) -> crate::data::rig::RigPose {
    let t = match period {
        Some(p) if p > 0. => t.rem_euclid(p),
        _ => t,
    };
    let charge = f.mov.as_ref().map_or(0, |m| m.charge);
    let live: Vec<&AnimKey> = keys.iter().filter(|k| k.min_charge <= charge && crate::moves::cond(f, k.cond)).collect();
    let Some(first) = live.first() else { return resolve_rig_pose(fd, &Pose::default()) };
    let i = live.iter().rposition(|k| k.at <= t).unwrap_or(0);
    let cur = if t < first.at { first } else { live[i] };
    let a = resolve_rig_ref(fd, &cur.pose);
    match live.get(i + 1) {
        Some(next) if next.lerp && t >= cur.at && next.at > cur.at => {
            lerp_rig(a, &resolve_rig_ref(fd, &next.pose), ((t - cur.at) / (next.at - cur.at)).clamp(0., 1.))
        }
        _ => a,
    }
}

/// The fighter's named rig-local transforms at a simulation tick, before cosmetic rendering effects.
pub fn fighter_rig_pose(d: &GameData, f: &Fighter, frame: u64) -> crate::data::rig::RigPose {
    let fd = &d.fighters[f.data];
    if f.state == State::Attack && f.mov.is_some() {
        let ph = crate::moves::phase(d, f);
        let t = f64::from(f.mov.as_ref().map_or(0, |m| m.t));
        return sample_rig(fd, &ph.anim, t, None, f);
    }
    let (name, clock) = state_anim(d, f);
    let pose = match fd.anims.get(&name) {
        Some(a) => sample_rig(fd, &a.keys, clock.unwrap_or_else(|| clock_value(a, f, frame)), a.period, f),
        None => resolve_rig_pose(fd, &Pose::default()),
    };
    if name == "crouch" {
        let frames = d.global.gait.crouch_frames;
        let t = if frames == 0 { 1. } else { (f64::from(f.gait_t) / f64::from(frames)).min(1.) };
        let amount = if f.gait == Gait::Rise { 1. - t } else { t };
        let idle = fd.anims.get("idle").map_or_else(
            || resolve_rig_pose(fd, &Pose::default()),
            |a| sample_rig(fd, &a.keys, clock_value(a, f, frame), a.period, f),
        );
        lerp_rig(idle, &pose, amount)
    } else {
        pose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> GameData {
        let sources = [
            ("global.json", include_str!("../../../assets/global.json")),
            ("stages/arena/stage.json", include_str!("../../../assets/stages/arena/stage.json")),
            ("fighters/base/fighter.json", include_str!("../../../assets/fighters/base/fighter.json")),
            ("fighters/brawler/fighter.json", include_str!("../../../assets/fighters/brawler/fighter.json")),
            ("fighters/mike/fighter.json", include_str!("../../../assets/fighters/mike/fighter.json")),
        ]
        .map(|(path, text)| crate::data::Source { path: path.into(), text: text.into() });
        let (d, errors) = crate::data::load(sources).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        d
    }

    fn near(a: P, b: P) {
        for (a, b) in a.into_iter().zip(b) {
            assert!((a - b).abs() < 1e-10, "{a} != {b}");
        }
    }

    #[test]
    fn body_transform_has_independent_numeric_coordinates() {
        for facing in [-1., 1.] {
            let t = BodyTransform::new(
                &Skel { rot: 90., pivot: 3., offset_y: 7., ..Default::default() },
                [100., 200.],
                facing,
            );
            // (2, 8) around (0, 3), clockwise 90 => (5, 1), then offset => (5, 8).
            near(t.local_to_body([2., 8.]), [5., 8.]);
            near(t.local_to_world([2., 8.]), [100. + facing * 5., 192.]);
            near(t.world_to_local([100. + facing * 5., 192.]), [2., 8.]);
            near(t.body_to_local([5., 8.]), [2., 8.]);
            near(t.world_to_body([100. + facing * 5., 192.]), [5., 8.]);
            let t = BodyTransform::new(&Skel::default(), [10., 20.], facing);
            near(t.local_to_world([2., 8.]), [10. + facing * 2., 12.]);
            near(t.world_to_local([10. + facing * 2., 12.]), [2., 8.]);
        }
    }

    #[test]
    fn tick_named_transforms_include_parent_and_body_rotation_and_clone_stably() {
        use crate::data::rig::{Rig, RigJoint};
        let mut d = data();
        let mut w =
            crate::World::new(crate::MatchConfig::sandbox("arena", vec![crate::PlayerConfig::human("brawler")], 1), &d)
                .unwrap();
        let f = &mut w.fighters[0];
        f.x = 100.;
        f.y = 200.;
        f.face = -1.;
        let fd = &mut d.fighters[f.data];
        fd.rig = Some(Rig {
            joints: vec![
                RigJoint { id: "root".into(), parent: None, translation: [2., 3.], angle: 90., shapes: vec![] },
                RigJoint {
                    id: "tip".into(),
                    parent: Some("root".into()),
                    translation: [5., 0.],
                    angle: 90.,
                    shapes: vec![],
                },
            ],
            bones: vec![],
        });
        fd.anims.clear();
        fd.poses
            .insert("stand".into(), Pose { rot: Some(90.), pivot: Some(3.), offset_y: Some(7.), ..Default::default() });
        let before = w.clone();
        let a = fighter_rig_transform(&d, &w.fighters[0], w.frame).unwrap().unwrap();
        near(a.joint_to_world("tip", [0., 0.]).unwrap(), [95., 192.]);
        // Tip accumulates 180 degrees: its (1, 0) is fighter-local (1, 8).
        near(a.joint_to_world("tip", [1., 0.]).unwrap(), [95., 191.]);
        near(a.world_to_joint("tip", [95., 191.]).unwrap(), [1., 0.]);
        assert_eq!(a, fighter_rig_transform(&d, &before.fighters[0], before.frame).unwrap().unwrap());
        assert_eq!(w, before);
        for _ in 0..3 {
            let mut clone = w.clone();
            crate::step_world(&mut w, &d, &[crate::Input::default()]);
            crate::step_world(&mut clone, &d, &[crate::Input::default()]);
            assert_eq!(
                fighter_rig_transform(&d, &w.fighters[0], w.frame),
                fighter_rig_transform(&d, &clone.fighters[0], clone.frame)
            );
            assert_eq!(w, clone);
        }
    }

    #[test]
    fn crouch_blends_in_and_out_and_keeps_feet_planted() {
        let d = data();
        for fd in &d.fighters {
            let w = crate::World::new(
                crate::MatchConfig::sandbox("arena", vec![crate::PlayerConfig::human(&fd.id)], 1),
                &d,
            )
            .unwrap();
            let mut f = w.fighters[0].clone();
            let idle = fighter_pose(&d, &f, 0);
            f.gait = Gait::Crouch;
            assert_eq!(fighter_pose(&d, &f, 0), idle);
            f.gait_t = d.global.gait.crouch_frames;
            let crouch = fighter_pose(&d, &f, 0);
            assert!(crouch.head[1] < idle.head[1]);
            assert_eq!((crouch.foot_f[1], crouch.foot_b[1]), (0., 0.));
            assert_ne!(fighter_pose(&d, &f, 30).head, crouch.head);
            f.gait_t /= 2;
            let halfway = fighter_pose(&d, &f, 0);
            assert!(halfway.head[1] > crouch.head[1] && halfway.head[1] < idle.head[1]);
            f.gait = Gait::Rise;
            assert_eq!(fighter_pose(&d, &f, 0), halfway);
            f.gait_t = d.global.gait.crouch_frames;
            assert_eq!(fighter_pose(&d, &f, 0), idle);
            f.state = State::Air;
            assert_ne!(state_anim(&d, &f).0, "crouch");
        }
    }

    #[test]
    fn legacy_attack_pose_keeps_absolute_coordinates_and_inherited_joints() {
        let d = data();
        let mut w =
            crate::World::new(crate::MatchConfig::sandbox("arena", vec![crate::PlayerConfig::human("brawler")], 1), &d)
                .unwrap();
        let mut f = w.fighters[0].clone();
        crate::moves::start_move(&mut w, &d, 0, &mut f, "jab");
        let windup = fighter_pose(&d, &f, w.frame);
        assert_eq!(windup.hand_f, [8., 50.]);
        assert_eq!(windup.head, [0., 64.]);
        assert_eq!(windup.foot_b, [-9., 0.]);
        f.mov.as_mut().unwrap().t = 3;
        w.fighters[0] = f;
        let before = w.clone();
        let strike = fighter_pose(&d, &w.fighters[0], w.frame);
        assert_eq!(strike.hand_f, [36., 52.]);
        assert_eq!(strike.shoulder, [3., 50.]);
        assert_eq!(strike.hip, [0., 28.]);
        assert_eq!(strike.pivot, 38.);
        assert_eq!(fighter_pose(&d, &before.fighters[0], before.frame), strike);
        assert_eq!(w, before);
    }

    #[test]
    fn animation_rotation_preserves_authored_sweeps_but_tick_blending_wraps() {
        let a = Skel { rot: 350., ..Default::default() };
        let b = Skel { rot: 70., ..Default::default() };
        assert_eq!(a.lerp(&b, 0.5).rot, 210.);
        assert_eq!(a.blend(&b, 0.5).rot, 390.);
    }

    #[test]
    fn named_pose_resolves_inheritance_and_interpolates_against_rest() {
        use crate::data::{
            moves::Cond,
            rig::{JointPose, Rig, RigJoint},
        };
        let mut d = data();
        let f =
            crate::World::new(crate::MatchConfig::sandbox("arena", vec![crate::PlayerConfig::human("brawler")], 1), &d)
                .unwrap()
                .fighters[0]
                .clone();
        let fd = &mut d.fighters[f.data];
        fd.rig = Some(Rig {
            joints: vec![RigJoint {
                id: "tail".into(),
                parent: None,
                translation: [4., 12.],
                angle: 10.,
                shapes: vec![],
            }],
            bones: vec![],
        });
        fd.poses
            .get_mut("stand")
            .unwrap()
            .joints
            .insert("tail".into(), JointPose { angle: Some(20.), translation: None });
        fd.poses.insert(
            "tailBase".into(),
            Pose {
                joints: [("tail".into(), JointPose { translation: Some([8., 16.]), angle: None })].into(),
                ..Default::default()
            },
        );
        let target = Pose {
            from: Some("tailBase".into()),
            joints: [("tail".into(), JointPose { angle: Some(100.), translation: None })].into(),
            ..Default::default()
        };
        let keys = vec![
            AnimKey { at: 0., pose: PoseRef::Inline(Box::default()), lerp: false, cond: None, min_charge: 0 },
            AnimKey {
                at: 5.,
                pose: PoseRef::Inline(Box::default()),
                lerp: false,
                cond: Some(Cond::Airborne),
                min_charge: 0,
            },
            AnimKey { at: 10., pose: PoseRef::Inline(Box::new(target)), lerp: true, cond: None, min_charge: 0 },
        ];
        let sampled = sample_rig(fd, &keys, 5., None, &f);
        assert_eq!(sampled["tail"].translation, Some([6., 14.]));
        assert_eq!(sampled["tail"].angle, Some(60.));
        assert_eq!(sample_rig(fd, &keys, 25., Some(20.), &f), sampled);
        assert_eq!(sample_rig(fd, &keys, 5., None, &f.clone()), sampled);
        fd.poses.get_mut("stand").unwrap().joints.clear();
        assert_eq!(resolve_rig_pose(fd, &Pose::default())["tail"].angle, Some(10.));
    }
}
