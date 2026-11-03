//! Named rigs are deterministic presentation data until bound collision is implemented.
mod common;

use sim::data::{
    rig::{JointPose, Rig},
    visual::{Pose, PoseRef, Shape},
};
use sim::{Input, PlayerConfig};

const HUMANOID: &str = include_str!("../../../assets/templates/rigs/humanoid.json");
const QUADRUPED: &str = include_str!("../../../assets/templates/rigs/quadruped.json");

#[test]
fn both_templates_validate_and_round_trip_as_fighter_content() {
    let d = common::data();
    for json in [HUMANOID, QUADRUPED] {
        let mut f = d.fighter("brawler").unwrap().clone();
        f.rig = Some(serde_json::from_str(json).unwrap());
        sim::data::load::check_fighter("example", &f).unwrap();
        let encoded = serde_json::to_string(&f).unwrap();
        let restored: sim::data::Fighter = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored.rig, f.rig);
        let pose = sim::pose::resolve_rig_pose(&f, &Pose::default());
        assert_eq!(
            sim::rig::resolve(f.rig.as_ref().unwrap(), &pose),
            sim::rig::resolve(restored.rig.as_ref().unwrap(), &pose)
        );
    }
}

#[test]
fn rig_errors_include_content_paths_for_named_and_inline_poses() {
    let d = common::data();
    let mut f = d.fighter("brawler").unwrap().clone();
    let unknown = [("unknown".into(), JointPose { angle: Some(20.), ..Default::default() })].into();
    f.poses.get_mut("stand").unwrap().joints = unknown;
    assert_eq!(sim::data::load::check_fighter("example", &f).unwrap_err().path, "poses.stand.joints");
    f.rig = Some(serde_json::from_str(QUADRUPED).unwrap());
    assert_eq!(sim::data::load::check_fighter("example", &f).unwrap_err().path, "poses.stand.joints.unknown");
    f.poses.get_mut("stand").unwrap().joints.clear();
    f.moves.jab.phases[0].anim[0].pose = PoseRef::Inline(Box::new(Pose {
        joints: [("head".into(), JointPose { angle: Some(f64::NAN), ..Default::default() })].into(),
        ..Default::default()
    }));
    assert_eq!(
        sim::data::load::check_fighter("example", &f).unwrap_err().path,
        "moves.jab.phases[0].anim[0].pose.joints.head.angle"
    );
    f.moves.jab.phases[0].anim[0].pose = PoseRef::Named("stand".into());
    let rig = f.rig.as_mut().unwrap();
    rig.joints[0].parent = Some("missing".into());
    assert_eq!(sim::data::load::check_fighter("example", &f).unwrap_err().path, "rig.joints[0].parent");
}

#[test]
fn invalid_rig_appearance_is_rejected_before_drawing() {
    let d = common::data();
    let mut f = d.fighter("brawler").unwrap().clone();
    let mut rig: Rig = serde_json::from_str(HUMANOID).unwrap();
    let index = rig.joints.iter().position(|j| j.id == "head").unwrap();
    if let Shape::Circle { r, .. } = &mut rig.joints[index].shapes[0] {
        *r = -1.;
    }
    f.rig = Some(rig);
    assert_eq!(
        sim::data::load::check_fighter("example", &f).unwrap_err().path,
        format!("rig.joints[{index}].shapes[0].r")
    );
}

#[test]
fn changing_body_art_does_not_change_gameplay_or_snapshot_resume() {
    let d = common::data();
    let mut custom = d.clone();
    let f = custom.fighters.iter_mut().find(|f| f.id == "brawler").unwrap();
    f.rig = Some(serde_json::from_str(QUADRUPED).unwrap());
    f.poses
        .get_mut("stand")
        .unwrap()
        .joints
        .insert("tailBase".into(), JointPose { angle: Some(45.), ..Default::default() });
    let players =
        ["brawler", "king"].into_iter().map(|id| PlayerConfig { cpu: Some(2), ..PlayerConfig::human(id) }).collect();
    let mut a = common::sandbox(&d, "arena", players);
    let mut b = a.clone();
    for _ in 0..180 {
        common::step_draining(&mut a, &d, &[Input::default(); 2]);
        common::step_draining(&mut b, &custom, &[Input::default(); 2]);
        assert_eq!(a, b);
    }
    let mut resumed = b.clone();
    for _ in 0..180 {
        common::step_draining(&mut a, &d, &[Input::default(); 2]);
        common::step_draining(&mut b, &custom, &[Input::default(); 2]);
        common::step_draining(&mut resumed, &custom, &[Input::default(); 2]);
        assert_eq!(a, b);
        assert_eq!(b, resumed);
    }
}
