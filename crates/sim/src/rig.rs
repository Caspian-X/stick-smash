//! Deterministic local rig transforms, shared by collision, rendering, and authoring tools.

use std::collections::{BTreeMap, VecDeque};

use crate::data::{
    Pt,
    rig::{Rig, RigPose},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RigError {
    /// Relative path within the rig or pose being evaluated.
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for RigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}
impl std::error::Error for RigError {}

fn error(path: impl Into<String>, message: impl Into<String>) -> RigError {
    RigError { path: path.into(), message: message.into() }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transform {
    pub translation: Pt,
    /// Accumulated counterclockwise rotation, in degrees.
    pub angle: f64,
}

impl Transform {
    pub fn point(self, point: Pt) -> Pt {
        let radians = self.angle.to_radians();
        let (s, c) = (libm::sin(radians), libm::cos(radians));
        [self.translation[0] + c * point[0] - s * point[1], self.translation[1] + s * point[0] + c * point[1]]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedJoint {
    pub id: String,
    pub transform: Transform,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRig {
    /// Same order as the authored joints, regardless of parent declaration order.
    pub joints: Vec<ResolvedJoint>,
}

impl ResolvedRig {
    pub fn get(&self, id: &str) -> Option<Transform> {
        self.joints.iter().find(|j| j.id == id).map(|j| j.transform)
    }
}

struct Layout {
    ids: BTreeMap<String, usize>,
    parents: Vec<Option<usize>>,
    order: Vec<usize>,
}

fn finite(path: &str, translation: Pt, angle: f64) -> Result<(), RigError> {
    if !translation.into_iter().all(f64::is_finite) {
        return Err(error(format!("{path}.translation"), "must contain finite coordinates"));
    }
    if !angle.is_finite() {
        return Err(error(format!("{path}.angle"), "must be finite"));
    }
    Ok(())
}

fn layout(rig: &Rig) -> Result<Layout, RigError> {
    if rig.joints.is_empty() {
        return Err(error("joints", "needs at least one joint"));
    }
    let mut ids = BTreeMap::new();
    for (i, j) in rig.joints.iter().enumerate() {
        if j.id.trim().is_empty() {
            return Err(error(format!("joints[{i}].id"), "must not be blank"));
        }
        if ids.insert(j.id.clone(), i).is_some() {
            return Err(error(format!("joints[{i}].id"), format!("duplicate joint {:?}", j.id)));
        }
        finite(&format!("joints[{i}]"), j.translation, j.angle)?;
    }
    for (i, bone) in rig.bones.iter().enumerate() {
        for (field, id) in [("from", &bone.from), ("to", &bone.to)] {
            if !ids.contains_key(id) {
                return Err(error(format!("bones[{i}].{field}"), format!("unknown joint {id:?}")));
            }
        }
        if !bone.width.is_finite() || bone.width <= 0. {
            return Err(error(format!("bones[{i}].width"), "must be finite and positive"));
        }
    }
    let mut parents = vec![None; rig.joints.len()];
    let mut children = vec![Vec::new(); rig.joints.len()];
    let mut pending = VecDeque::new();
    for (i, j) in rig.joints.iter().enumerate() {
        if let Some(parent) = &j.parent {
            let p = *ids
                .get(parent)
                .ok_or_else(|| error(format!("joints[{i}].parent"), format!("unknown joint {parent:?}")))?;
            parents[i] = Some(p);
            children[p].push(i);
        } else {
            pending.push_back(i);
        }
    }
    let mut order = Vec::with_capacity(rig.joints.len());
    while let Some(i) = pending.pop_front() {
        order.push(i);
        pending.extend(children[i].iter().copied());
    }
    if order.len() != rig.joints.len() {
        let mut reached = vec![false; rig.joints.len()];
        for &i in &order {
            reached[i] = true;
        }
        let i = reached.iter().position(|&v| !v).unwrap_or(0);
        return Err(error(format!("joints[{i}].parent"), "parent chain contains a cycle"));
    }
    Ok(Layout { ids, parents, order })
}

pub fn validate(rig: &Rig) -> Result<(), RigError> {
    layout(rig).map(|_| ())
}

/// Evaluate local overrides into fighter-local transforms. Facing and world placement are separate.
pub fn resolve(rig: &Rig, pose: &RigPose) -> Result<ResolvedRig, RigError> {
    let layout = layout(rig)?;
    for (id, p) in pose {
        if !layout.ids.contains_key(id) {
            return Err(error(format!("pose.{id}"), "unknown joint"));
        }
        finite(&format!("pose.{id}"), p.translation.unwrap_or_default(), p.angle.unwrap_or_default())?;
    }
    let mut transforms = vec![Transform::default(); rig.joints.len()];
    for i in layout.order {
        let j = &rig.joints[i];
        let p = pose.get(&j.id).copied().unwrap_or_default();
        let local =
            Transform { translation: p.translation.unwrap_or(j.translation), angle: p.angle.unwrap_or(j.angle) };
        let t = if let Some(parent) = layout.parents[i] {
            let parent = transforms[parent];
            Transform { translation: parent.point(local.translation), angle: parent.angle + local.angle }
        } else {
            local
        };
        // Finite input can still overflow when large transforms are composed.
        finite(&format!("joints[{i}]"), t.translation, t.angle)?;
        transforms[i] = t;
    }
    Ok(ResolvedRig {
        joints: rig
            .joints
            .iter()
            .zip(transforms)
            .map(|(j, transform)| ResolvedJoint { id: j.id.clone(), transform })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{
        Color,
        rig::{JointPose, RigBone, RigJoint},
    };

    fn joint(id: &str, parent: Option<&str>, translation: Pt, angle: f64) -> RigJoint {
        RigJoint { id: id.into(), parent: parent.map(str::to_owned), translation, angle, shapes: vec![] }
    }
    fn near(actual: Pt, expected: Pt) {
        for (a, b) in actual.into_iter().zip(expected) {
            assert!((a - b).abs() < 1e-10, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn local_rotation_propagates_with_later_declared_parents() {
        let rig = Rig {
            joints: vec![
                joint("tip", Some("arm"), [3., 0.], 20.),
                joint("arm", Some("body"), [2., 0.], 90.),
                joint("body", None, [10., 4.], 90.),
            ],
            bones: vec![],
        };
        let out = resolve(&rig, &RigPose::new()).unwrap();
        assert_eq!(out.joints.iter().map(|j| j.id.as_str()).collect::<Vec<_>>(), ["tip", "arm", "body"]);
        near(out.get("arm").unwrap().translation, [10., 6.]);
        near(out.get("tip").unwrap().translation, [7., 6.]);
        assert_eq!(out.get("tip").unwrap().angle, 200.);
        let pose = RigPose::from([("arm".into(), JointPose { angle: Some(0.), translation: None })]);
        let out = resolve(&rig, &pose).unwrap();
        near(out.get("tip").unwrap().translation, [10., 9.]);
        near(out.get("arm").unwrap().point([1., 0.]), [10., 7.]);
    }

    #[test]
    fn quadruped_has_no_required_humanoid_names_or_limb_count() {
        let mut joints = vec![joint("spine", None, [0., 20.], 0.)];
        for (name, x) in [("foreNear", 12.), ("foreFar", 10.), ("hindNear", -12.), ("hindFar", -10.)] {
            joints.push(joint(name, Some("spine"), [x, -20.], 0.));
        }
        joints.push(joint("tail", Some("spine"), [-18., 2.], 45.));
        let rig = Rig { joints, bones: vec![] };
        let json = serde_json::to_string(&rig).unwrap();
        let restored: Rig = serde_json::from_str(&json).unwrap();
        let a = resolve(&rig, &RigPose::new()).unwrap();
        assert_eq!(a, resolve(&restored, &RigPose::new()).unwrap());
        near(a.get("foreNear").unwrap().translation, [12., 0.]);
        near(a.get("hindFar").unwrap().translation, [-10., 0.]);
        near(a.get("tail").unwrap().translation, [-18., 22.]);
        assert!(a.get("handF").is_none());
    }

    #[test]
    fn malformed_graphs_and_overrides_return_paths() {
        let base = joint("root", None, [0., 0.], 0.);
        let invalid = [
            (vec![], "joints"),
            (vec![joint(" ", None, [0., 0.], 0.)], "joints[0].id"),
            (vec![base.clone(), base.clone()], "joints[1].id"),
            (vec![joint("a", Some("missing"), [0., 0.], 0.)], "joints[0].parent"),
            (vec![joint("a", Some("a"), [0., 0.], 0.)], "joints[0].parent"),
            (vec![joint("a", Some("b"), [0., 0.], 0.), joint("b", Some("a"), [0., 0.], 0.)], "joints[0].parent"),
            (vec![joint("a", None, [f64::NAN, 0.], 0.)], "joints[0].translation"),
            (vec![joint("a", None, [0., 0.], f64::INFINITY)], "joints[0].angle"),
        ];
        for (joints, path) in invalid {
            assert_eq!(resolve(&Rig { joints, bones: vec![] }, &RigPose::new()).unwrap_err().path, path);
        }
        let rig = Rig { joints: vec![base], bones: vec![] };
        let mut pose = RigPose::from([("missing".into(), JointPose::default())]);
        assert_eq!(resolve(&rig, &pose).unwrap_err().path, "pose.missing");
        pose = RigPose::from([("root".into(), JointPose { angle: Some(f64::NAN), translation: None })]);
        assert_eq!(resolve(&rig, &pose).unwrap_err().path, "pose.root.angle");
    }

    #[test]
    fn deep_chains_use_no_recursion_and_reject_composition_overflow() {
        let mut joints = vec![joint("0", None, [1., 0.], 0.)];
        for i in 1..4096 {
            joints.push(joint(&i.to_string(), Some(&(i - 1).to_string()), [1., 0.], 0.));
        }
        let mut rig = Rig { joints, bones: vec![] };
        near(resolve(&rig, &RigPose::new()).unwrap().get("4095").unwrap().translation, [4096., 0.]);
        rig.joints[0].translation[0] = f64::MAX;
        rig.joints[1].translation[0] = f64::MAX;
        assert_eq!(resolve(&rig, &RigPose::new()).unwrap_err().path, "joints[1].translation");
    }

    #[test]
    fn bone_bindings_and_widths_are_validated() {
        let mut rig = Rig {
            joints: vec![joint("root", None, [0., 0.], 0.), joint("tip", Some("root"), [10., 0.], 0.)],
            bones: vec![RigBone { from: "root".into(), to: "tip".into(), width: 2., color: Color::Player }],
        };
        assert!(validate(&rig).is_ok());
        rig.bones[0].to = "missing".into();
        assert_eq!(validate(&rig).unwrap_err().path, "bones[0].to");
        rig.bones[0].to = "tip".into();
        for width in [0., -1., f64::NAN, f64::INFINITY] {
            rig.bones[0].width = width;
            assert_eq!(validate(&rig).unwrap_err().path, "bones[0].width");
        }
    }
}
