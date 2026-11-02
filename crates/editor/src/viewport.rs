//! The viewport: playback into the render scene, pan/zoom, direct manipulation of hitboxes and joints (with snapping
//! modifiers), the overlay with onion skins, and the floating toolbars.

use bevy_egui::egui::{self, Color32, Rect, RichText, Sense, UiBuilder};
use render::canvas::Rgba;
use render::pose::resolve;
use sim::data::Hitbox;
use sim::world::{Fighter, State};

use crate::preview::Subject;
use crate::style::{self, FAINT, INK, STAR, caps};
use crate::ui::{at_playhead, hint, pose_mut, pose_target};
use crate::view::{self, Hit};
use crate::{Drag, Editor};

pub(crate) fn sync_scene(ed: &mut Editor, scene: &mut render::Scene, dt: f32) {
    let Some(run) = &ed.run else { return };
    let n = run.frames.len();
    let run_gen = ed.run_gen;
    if ed.playing && n > 1 {
        ed.pane.acc += f64::from(dt) * 60. * ed.speed;
        while ed.pane.acc >= 1. {
            ed.pane.acc -= 1.;
            if ed.frame + 1 >= n && !ed.looping && !ed.subject.idle() {
                ed.playing = false;
                ed.pane.acc = 0.;
                break;
            }
            ed.frame += 1;
            if ed.frame >= n {
                ed.frame = 0;
                ed.pane.shown = None;
            }
            if ed.pane.shown.is_some_and(|(k, g)| g == run_gen && k + 1 == ed.frame) {
                scene.push(&run.frames[ed.frame]);
                ed.pane.shown = Some((ed.frame, run_gen));
            }
        }
        scene.alpha = ed.pane.acc;
    } else {
        ed.pane.acc = 0.;
        scene.alpha = 1.;
    }
    ed.frame = ed.frame.min(n - 1);
    if ed.pane.shown != Some((ed.frame, run_gen)) {
        scene.data = run.data.clone();
        scene.reset(&run.frames[ed.frame]);
        ed.pane.shown = Some((ed.frame, run_gen));
    }
    crate::pane::sync_scene(scene, ed.pane.cam, &ed.port);
    scene.pose_override = match &ed.subject {
        Subject::Pose(n) => ed.ws.docs.get(&ed.cur).and_then(|d| d.item.poses.get(n).map(|p| (0, resolve(&d.item, p)))),
        _ => None,
    };
    if ed.frame != ed.followed {
        ed.followed = ed.frame;
        if let Some((p, _)) = run.marks[ed.frame].phase
            && p != ed.phase
        {
            ed.phase = p;
            ed.hitbox = None;
        }
    }
}

/// The current phase's hitboxes (from the edited data) on the playhead.
pub(crate) struct PlayheadBoxes {
    pub f: Fighter,
    pub phase: usize,
    pub t: u32,
    pub boxes: Vec<Hitbox>,
}

pub(crate) fn playhead_boxes(ed: &Editor) -> Option<PlayheadBoxes> {
    let Subject::Move(slot) = &ed.subject else { return None };
    let crate::ui::Playhead { fighter: f, phase: Some((p, t)) } = at_playhead(ed)? else { return None };
    let boxes = ed.ws.docs.get(&ed.cur)?.item.moves.get(slot)?.phases.get(p)?.hitboxes.clone();
    Some(PlayheadBoxes { f, phase: p, t, boxes })
}

pub(crate) fn overlay(ctx: &egui::Context, ed: &mut Editor, scene: &mut render::Scene) {
    let ppp = f64::from(ctx.pixels_per_point());
    let px = 1. / ed.pane.cam.zoom;
    let Some(run) = &ed.run else { return };
    let gamma = run.data.global.render.alpha_gamma;
    scene.overlay.clear(ed.pane.cam.zoom * ppp, gamma);
    let Some(crate::ui::Playhead { fighter: f, .. }) = at_playhead(ed) else { return };
    if ed.onion && !ed.playing && matches!(ed.subject, Subject::Move(_) | Subject::Throw(_)) {
        onion(ed, &mut scene.overlay, px);
    }
    let fd = &ed.ws.docs[&ed.cur].item;
    let hurt = if f.state == State::Knockdown { fd.hurtboxes.knockdown } else { fd.hurtboxes.default };
    let (boxes, shown) = match playhead_boxes(ed) {
        Some(PlayheadBoxes { f, t, boxes: b, .. }) => {
            let cd = f.mov.as_ref().map_or(0., |m| m.counter_damage);
            let s = view::shown(&f, &b, t, cd);
            (b, s)
        }
        None => (vec![], vec![]),
    };
    let joints = joint_points(ed, &f);
    let trail: Vec<[f64; 2]> = run.frames.iter().filter_map(|w| w.fighters.get(1)).map(|d| [d.x, d.y - 30.]).collect();
    let counter = sim::combat::countering(&run.data, &f).is_some();
    let o = view::Overlay {
        f: &f,
        hurt,
        boxes: &boxes,
        shown: &shown,
        sel: ed.hitbox,
        hover: ed.pane.hover,
        joints: &joints,
        show_hurt: ed.show_hurt,
        show_inactive: ed.show_inactive,
        trail: if trail.len() > 1 { &trail } else { &[] },
        counter,
    };
    view::draw(&mut scene.overlay, &o, px);
}

/// Ghost stick figures of the frames around the playhead (earlier ones warm, later ones cool).
fn onion(ed: &Editor, cv: &mut render::canvas::Canvas, px: f64) {
    let Some(run) = &ed.run else { return };
    let d = &run.data;
    let head_r = d.fighter(&ed.cur).map_or(10., |f| f.style.head_radius);
    for k in 1..=style::ONION_FRAMES {
        let a = 0.5 * (1. - (k as f64 - 1.) / style::ONION_FRAMES as f64);
        for (idx, col) in [
            (ed.frame.checked_sub(k), style::ONION_PAST),
            (Some(ed.frame + k).filter(|&i| i < run.frames.len()), style::ONION_NEXT),
        ] {
            let Some(w) = idx.map(|i| &run.frames[i]) else { continue };
            let Some(f) = w.fighters.first() else { continue };
            let sk = render::pose::fighter_pose(d, f, w.frame);
            let p = |j: usize| view::joint_to_stage(f, &sk, view::skel_joint(&sk, j));
            let c = Rgba::hex(col, a);
            for (x, y) in [(1, 2), (1, 3), (3, 4), (1, 5), (5, 6), (2, 7), (7, 8), (2, 9), (9, 10)] {
                cv.line(p(x), p(y), 1.5 * px, c);
            }
            cv.stroke_circle(p(0), head_r, 1.5 * px, c);
        }
    }
}

fn joint_points(ed: &Editor, f: &Fighter) -> Vec<[f64; 2]> {
    if ed.playing {
        return vec![];
    }
    match pose_target(ed) {
        Some((_, sk)) => (0..11).map(|j| view::joint_to_stage(f, &sk, view::skel_joint(&sk, j))).collect(),
        None => vec![],
    }
}

pub(crate) fn viewport(ui: &mut egui::Ui, ed: &mut Editor) {
    let rect = ui.max_rect();
    ed.port = crate::pane::port(ui);
    if ed.pane.reframe
        && let Some(f) = ed.run.as_ref().map(|r| r.frames[0].fighters[0].clone())
    {
        ed.pane.reframe = false;
        ed.pane.cam.zoom = view::Zoom::FIGHTER_FRAME.clamp(f64::from(rect.height()) / 240.);
        ed.pane.cam.center = [f.x + 30., f.y - 60.];
    }
    crate::pane::interact(ui, ed, FighterInteraction { boxes: playhead_boxes(ed) });
    chrome(ui, ed, rect);
}

struct FighterInteraction {
    boxes: Option<PlayheadBoxes>,
}

impl crate::pane::Interaction for FighterInteraction {
    type Hit = Hit;
    type Shown = (usize, u32);
    fn pane(ed: &mut Editor) -> &mut crate::Pane<Hit, Self::Shown> {
        &mut ed.pane
    }
    fn zoom() -> view::Zoom {
        view::Zoom::FIGHTER_WHEEL
    }
    fn hit(&self, ed: &Editor, m: [f64; 2], px: f64) -> Option<Hit> {
        let (f, shown, b) = match &self.boxes {
            Some(PlayheadBoxes { f, t, boxes: b, .. }) => {
                (f.clone(), view::shown(f, b, *t, f.mov.as_ref().map_or(0., |m| m.counter_damage)), b.clone())
            }
            None => (at_playhead(ed)?.fighter, vec![], vec![]),
        };
        let joints = joint_points(ed, &f);
        view::hit_test(&f, &b, &shown, ed.hitbox, &joints, m, px)
    }
    fn start(&self, ed: &mut Editor, grab: [f64; 2]) -> Option<Drag> {
        match (ed.pane.hover, &self.boxes) {
            (
                Some(h @ (Hit::Body(i) | Hit::Rim(i) | Hit::Angle(i))),
                Some(PlayheadBoxes { f, phase: p, t, boxes: b }),
            ) => {
                ed.hitbox = Some(i);
                let (g, _) = view::geom_source(&b[i], *t);
                Some(Drag::Box { kind: h, phase: *p, t: *t, f: Box::new(f.clone()), grab, orig: g })
            }
            (Some(Hit::Joint(j)), _) => match (pose_target(ed), at_playhead(ed)) {
                (Some((target, skel)), Some(crate::ui::Playhead { fighter: f, .. })) => {
                    Some(Drag::Joint { j, target, f: Box::new(f), skel })
                }
                _ => None,
            },
            _ => Some(Drag::Pan),
        }
    }
    fn drag(&self, ed: &mut Editor, drag: &Drag, m: [f64; 2], (coarse, fine): (bool, bool)) {
        match drag {
            Drag::Box { kind, phase, t, f, grab, orig } => {
                drag_box(ed, (*kind, *phase, *t), f, *grab, *orig, m, (coarse, fine));
            }
            Drag::Joint { j, target, f, skel } => {
                let p = view::stage_to_joint(f, skel, m).map(|v| crate::stage_view::Snap::fighter(coarse, fine).v(v));
                if let Some(pose) = pose_mut(ed.fighter(), target) {
                    *view::pose_joint(pose, *j) = Some(p);
                }
            }
            _ => {}
        }
    }
    fn click(&self, ed: &mut Editor) {
        match ed.pane.hover {
            Some(Hit::Body(i) | Hit::Rim(i) | Hit::Angle(i)) => ed.hitbox = Some(i),
            Some(Hit::Joint(_)) => {}
            None => ed.hitbox = None,
        }
    }
    fn cursor(&self, ed: &Editor) -> egui::CursorIcon {
        match (&ed.pane.drag, ed.pane.hover) {
            (Some(Drag::Pan), _) => egui::CursorIcon::Grabbing,
            (Some(Drag::Joint { .. }), _) => egui::CursorIcon::Grabbing,
            (_, Some(Hit::Rim(_))) => egui::CursorIcon::ResizeNeSw,
            (_, Some(Hit::Angle(_))) => egui::CursorIcon::Crosshair,
            (_, Some(Hit::Joint(_))) => egui::CursorIcon::Grab,
            (_, Some(_)) => egui::CursorIcon::Move,
            _ => egui::CursorIcon::Default,
        }
    }
}

fn drag_box(
    ed: &mut Editor,
    (kind, phase, t): (Hit, usize, u32),
    f: &Fighter,
    grab: [f64; 2],
    orig: (f64, f64, f64),
    m: [f64; 2],
    (coarse, fine): (bool, bool),
) {
    let Subject::Move(slot) = ed.subject.clone() else { return };
    let s = |v: f64| crate::stage_view::Snap::fighter(coarse, fine).v(v);
    let Some(h) = ed.fighter().moves.get_mut(&slot).and_then(|mv| mv.phases.get_mut(phase)).and_then(|p| match kind {
        Hit::Body(i) | Hit::Rim(i) | Hit::Angle(i) => p.hitboxes.get_mut(i),
        Hit::Joint(_) => None,
    }) else {
        return;
    };
    let (x, y, r) = orig;
    let (nx, ny, nr) = match kind {
        Hit::Body(_) => {
            let (a, b) = (view::stage_to_local(f, grab), view::stage_to_local(f, m));
            (s(x + b[0] - a[0]), s(y + b[1] - a[1]), r)
        }
        Hit::Rim(_) => {
            let l = view::stage_to_local(f, m);
            (x, y, s((l[0] - x).hypot(l[1] - y)).max(1.))
        }
        Hit::Angle(_) => {
            let a = view::drag_angle(f, h, view::local_to_stage(f, [x, y]), m);
            h.angle = if coarse {
                (a / 15.).round() * 15.
            } else if fine {
                view::round_to(a, style::FINE_STEP)
            } else {
                a.round()
            };
            return;
        }
        Hit::Joint(_) => return,
    };
    match view::geom_source(h, t).1 {
        Some(j) => {
            let e = &mut h.frames[j];
            (e[1], e[2], e[3]) = (nx, ny, nr);
        }
        None => (h.x, h.y, h.r) = (nx, ny, nr),
    }
}

fn pill(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(INK.gamma_multiply(0.88))
        .stroke(egui::Stroke::new(1., style::LINE.gamma_multiply(0.6)))
        .corner_radius(7)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| ui.horizontal_wrapped(add));
}

/// Floating toolbars, the live readout of what's under the cursor, the hint line and the problem banner.
fn chrome(ui: &mut egui::Ui, ed: &mut Editor, rect: Rect) {
    // Pills wrap onto a second row when the viewport is narrow; the banner goes below whatever they took.
    let bar = Rect::from_min_max(rect.min + egui::vec2(10., 10.), rect.max - egui::vec2(10., 10.));
    let used = ui.scope_builder(UiBuilder::new().max_rect(bar), |ui| {
        ui.horizontal_wrapped(|ui| {
            pill(ui, |ui| {
                ui.toggle_value(&mut ed.show_hurt, "Hurtbox").on_hover_text("Show the hurtbox  H");
                ui.toggle_value(&mut ed.show_inactive, "Inactive")
                    .on_hover_text("Show hitboxes outside their active frames  I");
                let move_like = matches!(ed.subject, Subject::Move(_) | Subject::Throw(_));
                ui.add_enabled(move_like, egui::Button::selectable(ed.onion, "Onion"))
                    .on_hover_text("Ghosts of the 3 frames before (red) and after (green)  O")
                    .clicked()
                    .then(|| ed.onion = !ed.onion);
                ui.separator();
                if ui.button("⛶").on_hover_text("Frame the fighter  F · double-click").clicked() {
                    ed.pane.reframe = true;
                }
                ui.label(RichText::new(format!("{:.0}%", ed.pane.cam.zoom * 100. / 3.)).color(FAINT).small());
            });
            if matches!(ed.subject, Subject::Move(_) | Subject::Throw(_)) {
                pill(ui, |ui| setup_bar(ui, ed));
            }
        });
    });
    let errors = ed.ws.docs.get(&ed.cur).map(|d| d.errors.clone()).unwrap_or_default();
    if let Some(e) = errors.first() {
        let top = used.response.rect.bottom() + 8.;
        let r = Rect::from_min_max(egui::pos2(bar.left(), top), egui::pos2(bar.right(), top + 40.));
        let mut go = false;
        ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
            egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(90, 20, 30, 235))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 4))
                .show(ui, |ui| {
                    let more = if errors.len() > 1 { format!(" (+{} more)", errors.len() - 1) } else { String::new() };
                    let txt = format!("⚠ Preview paused on the last valid data · {}: {}{more}", e.path, e.msg);
                    let l =
                        egui::Label::new(RichText::new(txt).color(STAR).size(12.5)).truncate().sense(Sense::click());
                    go = ui
                        .add(l)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Click to go there")
                        .clicked();
                });
        });
        if go {
            crate::inspect::goto(ed, &e.path);
        }
    }
    let hints: &[(&str, &str)] = match (&ed.subject, ed.pane.hover, ed.playing, ed.pane.drag.is_some()) {
        (_, _, true, _) => &[("Space", "pause"), ("L", if ed.looping { "loop on" } else { "loop off" })],
        (_, _, _, true) => &[("⇧", "snap"), ("Alt", "fine")],
        (_, Some(Hit::Angle(_)), _, _) => &[("drag", "aim the launch angle"), ("⇧", "15°")],
        (_, Some(Hit::Rim(_)), _, _) => &[("drag", "resize"), ("⇧", "snap"), ("Alt", "fine")],
        (_, Some(Hit::Body(_)), _, _) => &[("click", "select"), ("drag", "move")],
        (_, Some(Hit::Joint(_)), _, _) => &[("drag", "pose the joint"), ("⇧", "snap"), ("Alt", "fine")],
        (Subject::Move(_), _, _, _) if ed.hitbox.is_some() => &[
            ("drag", "box, rim or gold dot"),
            ("Alt ←→↑↓", "nudge"),
            ("⌘D", "duplicate"),
            ("Del", "delete"),
            ("Tab", "next"),
        ],
        (Subject::Move(_), _, _, _) => {
            &[("click", "a hitbox"), ("N", "new hitbox"), ("K", "pose key"), ("◆", "keys pose by dragging joints")]
        }
        (Subject::Pose(_), _, _, _) => &[("drag", "joints to pose"), ("wheel", "zoom"), ("right-drag", "pan")],
        _ => &[("wheel", "zoom"), ("right-drag", "pan"), ("?", "all shortcuts")],
    };
    let hint_rect = Rect::from_min_size(
        rect.left_bottom() + egui::vec2(12., -style::HINT_BOTTOM),
        egui::vec2(rect.width() - 24., 22.),
    );
    ui.scope_builder(UiBuilder::new().max_rect(hint_rect), |ui| {
        ui.horizontal(|ui| {
            for (k, v) in hints {
                hint(ui, k, v);
                ui.add_space(6.);
            }
            if let Some(Hit::Joint(j)) = ed.pane.hover {
                ui.label(RichText::new(view::JOINTS[j]).color(STAR));
            }
        });
    });
}

fn setup_bar(ui: &mut egui::Ui, ed: &mut Editor) {
    let throw = matches!(ed.subject, Subject::Throw(_));
    let s = &mut ed.setup;
    ui.label(caps("Sandbox"));
    let mut air = s.air.is_some();
    if ui
        .add_enabled(!throw, egui::Checkbox::new(&mut air, "Airborne"))
        .on_hover_text("Start the move in the air")
        .changed()
    {
        s.air = air.then_some(120.);
    }
    if let Some(h) = &mut s.air {
        ui.add(egui::DragValue::new(h).range(1.0..=400.0).speed(1.).suffix(" px")).on_hover_text("Start height");
    }
    ui.separator();
    ui.add_enabled(!throw, egui::Checkbox::new(&mut s.dummy, "Dummy"))
        .on_hover_text("A standing dummy to see real hits and knockback");
    if s.dummy || throw {
        ui.add(egui::DragValue::new(&mut s.dummy_dist).range(0.0..=400.0).speed(1.).suffix(" px"))
            .on_hover_text("Distance in front");
        ui.add(egui::DragValue::new(&mut s.dummy_pct).range(0.0..=300.0).speed(1.).suffix("%"))
            .on_hover_text("Dummy damage");
    }
    ui.separator();
    ui.label(RichText::new("Hold").color(style::DIM));
    ui.add(egui::DragValue::new(&mut s.hold).range(0..=240).suffix(" f"))
        .on_hover_text("Hold the move's button this many frames (charge, release)");
}
