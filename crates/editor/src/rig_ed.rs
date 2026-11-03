//! Mouse authoring for arbitrary parent-local rigs.
use bevy_egui::egui;
use sim::data::{
    Fighter,
    rig::{Rig, RigJoint, RigPose},
};

#[derive(Default)]
pub(crate) struct State {
    pub selected: Option<String>,
    pub add: bool,
    rename: String,
    message: String,
}

pub fn local(parent: sim::rig::Transform, p: [f64; 2]) -> [f64; 2] {
    sim::rig::Transform { translation: [0., 0.], angle: -parent.angle }
        .point([p[0] - parent.translation[0], p[1] - parent.translation[1]])
}

pub(crate) fn panel(ui: &mut egui::Ui, f: &mut Fighter, state: &mut State) {
    ui.label("Rest geometry · drag joints; select a parent, enable Add, then click to connect. Poses do not affect this view.");
    if f.rig.is_none() && ui.button("Add blank rig").clicked() {
        f.rig = Some(Rig {
            joints: vec![RigJoint {
                id: "root".into(),
                parent: None,
                translation: [0., 0.],
                angle: 0.,
                shapes: vec![],
            }],
            bones: vec![],
        });
    }
    let Some(rig) = &mut f.rig else { return };
    if !rig.joints.iter().any(|j| Some(&j.id) == state.selected.as_ref()) {
        state.selected = None;
    }
    ui.checkbox(&mut state.add, "Add connected joint (click viewport)");
    for j in &rig.joints {
        if ui.selectable_label(state.selected.as_ref() == Some(&j.id), &j.id).clicked() {
            state.selected = Some(j.id.clone());
            state.rename = j.id.clone();
        }
    }
    let Some(id) = state.selected.clone() else { return };
    if let Some(j) = rig.joints.iter_mut().find(|j| j.id == id) {
        ui.label(format!("Selected: {id} · parent: {}", j.parent.as_deref().unwrap_or("fighter root")));
        ui.horizontal(|ui| {
            ui.label("Rest angle");
            ui.add(egui::DragValue::new(&mut j.angle));
        });
        let _ =
            crate::form::edit(ui, egui::Id::new(("rig-shapes", &id)), &crate::form::array_of("Shape"), &mut j.shapes);
    }
    ui.text_edit_singleline(&mut state.rename);
    if ui.button("Rename joint & references").clicked() {
        match crate::doc::rename_joint(f, &id, &state.rename) {
            Ok(()) => {
                state.selected = Some(state.rename.clone());
                state.message.clear();
            }
            Err(e) => state.message = e,
        }
    }
    if ui.button("Delete leaf & connected strokes").clicked() {
        match crate::doc::delete_joint(f, &id) {
            Ok(()) => {
                state.selected = None;
                state.message.clear();
            }
            Err(e) => state.message = e,
        }
    }
    ui.label(&state.message);
}

pub fn add(f: &mut Fighter, parent: Option<String>, point: [f64; 2]) -> Option<String> {
    let rig = f.rig.as_mut()?;
    let resolved = sim::rig::resolve(rig, &RigPose::new()).ok()?;
    let transform = match &parent {
        Some(id) => resolved.get(id)?,
        None => Default::default(),
    };
    let id = (1..).map(|n| format!("joint{n}")).find(|id| !rig.joints.iter().any(|j| &j.id == id))?;
    if let Some(parent) = &parent {
        // Reuse the selected chain's stroke; otherwise the template's first stroke.
        let style =
            rig.bones.iter().find(|b| &b.to == parent || &b.from == parent).or(rig.bones.first()).cloned().or_else(
                || {
                    serde_json::from_str::<Rig>(include_str!("../../../assets/templates/rigs/humanoid.json"))
                        .ok()?
                        .bones
                        .into_iter()
                        .next()
                },
            );
        if let Some(mut bone) = style {
            bone.from = parent.clone();
            bone.to = id.clone();
            rig.bones.push(bone);
        }
    }
    rig.joints.push(RigJoint {
        id: id.clone(),
        parent,
        translation: local(transform, point),
        angle: 0.,
        shapes: vec![],
    });
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deep_rotated_chain_inverse_including_facing_and_body_transform() {
        let mut rig: Rig = serde_json::from_str(include_str!("../../../assets/templates/rigs/quadruped.json")).unwrap();
        for j in &mut rig.joints {
            j.angle = 37.;
        }
        for i in 0..128 {
            let parent = rig.joints.last().unwrap().id.clone();
            rig.joints.push(RigJoint {
                id: format!("segment{i}"),
                parent: Some(parent),
                translation: [3., 2.],
                angle: 7.,
                shapes: vec![],
            });
        }
        let resolved = sim::rig::resolve(&rig, &RigPose::new()).unwrap();
        let parent = resolved.joints.last().unwrap().transform;
        let point = parent.point([13., -7.]);
        let mut f = crate::tests_fighter();
        f.x = 123.;
        f.y = 88.;
        f.face = -1.;
        let sk = render::pose::Skel { rot: 71., pivot: 18., offset_y: 9., ..Default::default() };
        let stage = crate::view::joint_to_stage(&f, &sk, point);
        let p = local(parent, crate::view::stage_to_joint(&f, &sk, stage));
        assert!((p[0] - 13.).abs() < 1e-9);
        assert!((p[1] + 7.).abs() < 1e-9);
    }
}
