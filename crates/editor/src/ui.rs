//! The egui layer: top bar, browser, viewport, timeline and shortcuts. The inspector lives in `inspect`.

use bevy_egui::egui::{self, Align2, Color32, FontId, Key, LayerId, PointerButton, Rect, Sense, Stroke, UiBuilder};
use render::pose::{Skel, resolve};
use sim::data::Hitbox;
use sim::data::visual::{Pose, PoseRef};
use sim::world::{Fighter, State};

use crate::preview::Subject;
use crate::style::{self, DIM, FAINT, GOLD, INK, LINE, RED, STAR, caps};
use crate::view::{self, Hit};
use crate::{Drag, Editor, Outbox, Request, inspect, move_mut, move_ref};

/// A pose being edited in the viewport: a named pose, or an inline pose in a move's anim key.
#[derive(Clone, Debug, PartialEq)]
pub enum PoseTarget {
    Named(String),
    Inline { slot: String, phase: usize, key: usize },
}

pub fn pose_mut<'a>(f: &'a mut sim::data::Fighter, t: &PoseTarget) -> Option<&'a mut Pose> {
    match t {
        PoseTarget::Named(n) => f.poses.get_mut(n),
        PoseTarget::Inline { slot, phase, key } => {
            match &mut move_mut(&mut f.moves, slot)?.phases.get_mut(*phase)?.anim.get_mut(*key)?.pose {
                PoseRef::Inline(p) => Some(p),
                PoseRef::Named(_) => None,
            }
        }
    }
}

pub(crate) fn fighter(ed: &mut Editor) -> &mut sim::data::Fighter {
    &mut ed.ws.docs.get_mut(&ed.cur).expect("current fighter is open").fighter
}

/// The fighter on the playhead frame, and its phase/phase frame if it's in the move.
pub(crate) fn at_playhead(ed: &Editor) -> Option<(Fighter, Option<(usize, u32)>)> {
    let r = ed.run.as_ref()?;
    let k = ed.frame.min(r.frames.len() - 1);
    Some((r.frames[k].fighters[0].clone(), r.marks[k].phase))
}

/// The pose shown on the playhead, if it's exactly a key (not mid-blend), and its skeleton.
pub(crate) fn pose_target(ed: &Editor) -> Option<(PoseTarget, Skel)> {
    let fd = &ed.ws.docs.get(&ed.cur)?.fighter;
    match &ed.subject {
        Subject::Pose(n) => fd.poses.get(n).map(|p| (PoseTarget::Named(n.clone()), resolve(fd, p))),
        Subject::Move(slot) => {
            let (f, Some((p, t))) = at_playhead(ed)? else { return None };
            let anim = &move_ref(&fd.moves, slot)?.phases.get(p)?.anim;
            let live: Vec<usize> = (0..anim.len()).filter(|&i| sim::moves::cond(&f, anim[i].cond)).collect();
            let t = f64::from(t);
            let li = live.iter().rposition(|&i| anim[i].at <= t).unwrap_or(0);
            let key = *live.get(li)?;
            if let Some(&n) = live.get(li + 1)
                && anim[n].lerp
                && t > anim[key].at
                && anim[n].at > anim[key].at
            {
                return None;
            }
            match &anim[key].pose {
                PoseRef::Named(n) => fd.poses.get(n).map(|p| (PoseTarget::Named(n.clone()), resolve(fd, p))),
                PoseRef::Inline(pose) => {
                    Some((PoseTarget::Inline { slot: slot.clone(), phase: p, key }, resolve(fd, pose)))
                }
            }
        }
        _ => None,
    }
}
pub(crate) fn run(ctx: &egui::Context, ed: &mut Editor, scene: &mut render::Scene, dt: f32, out: &mut Outbox) {
    shortcuts(ctx, ed, out);
    let before = ed.doc().fighter.clone();
    let setup = ed.setup.clone();
    let mut root = egui::Ui::new(
        ctx.clone(),
        "editor".into(),
        UiBuilder::new().layer_id(LayerId::background()).max_rect(ctx.viewport_rect()),
    );
    top_bar(&mut root, ed, out);
    egui::Panel::left("ed-browser").resizable(false).exact_size(184.).show(&mut root, |ui| browser(ui, ed));
    egui::Panel::right("ed-inspector")
        .default_size(340.)
        .size_range(280.0..=520.)
        .show(&mut root, |ui| inspect::inspector(ui, ed));
    egui::Panel::bottom("ed-timeline").resizable(false).show(&mut root, |ui| timeline(ui, ed));
    egui::CentralPanel::no_frame().show(&mut root, |ui| viewport(ui, ed));
    if ed.help {
        help(ctx, ed);
    }
    if let Some((_, _, t)) = &mut ed.status {
        *t -= dt;
        if *t <= 0. {
            ed.status = None;
        }
    }
    let ongoing = ctx.input(|i| i.pointer.any_down()) || ctx.egui_wants_keyboard_input();
    let doc = ed.doc();
    if doc.fighter != before {
        doc.changed(before, ongoing);
        ed.stale = true;
    } else if !ongoing {
        doc.close_step();
    }
    if ed.setup != setup {
        ed.stale = true;
    }
    ed.refresh();
    sync_scene(ed, scene, dt);
    overlay(ctx, ed, scene);
}

fn sync_scene(ed: &mut Editor, scene: &mut render::Scene, dt: f32) {
    let Some(run) = &ed.run else { return };
    let n = run.frames.len();
    let run_gen = ed.run_gen;
    if ed.playing && n > 1 {
        ed.acc += f64::from(dt) * 60. * ed.speed;
        while ed.acc >= 1. {
            ed.acc -= 1.;
            ed.frame += 1;
            if ed.frame >= n {
                ed.frame = 0;
                ed.shown = None;
            }
            if ed.shown.is_some_and(|(k, g)| g == run_gen && k + 1 == ed.frame) {
                scene.push(&run.frames[ed.frame]);
                ed.shown = Some((ed.frame, run_gen));
            }
        }
        scene.alpha = ed.acc;
    } else {
        ed.acc = 0.;
        scene.alpha = 1.;
    }
    ed.frame = ed.frame.min(n - 1);
    if ed.shown != Some((ed.frame, run_gen)) {
        scene.data = run.data.clone();
        scene.reset(&run.frames[ed.frame]);
        ed.shown = Some((ed.frame, run_gen));
    }
    scene.debug = false;
    scene.bottom_safe_area = 0.;
    scene.fixed_view = Some(ed.cam.window_view(&ed.port));
    scene.pose_override = match &ed.subject {
        Subject::Pose(n) => {
            ed.ws.docs.get(&ed.cur).and_then(|d| d.fighter.poses.get(n).map(|p| (0, resolve(&d.fighter, p))))
        }
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
fn playhead_boxes(ed: &Editor) -> Option<(Fighter, usize, u32, Vec<Hitbox>)> {
    let Subject::Move(slot) = &ed.subject else { return None };
    let (f, Some((p, t))) = at_playhead(ed)? else { return None };
    let boxes = move_ref(&ed.ws.docs.get(&ed.cur)?.fighter.moves, slot)?.phases.get(p)?.hitboxes.clone();
    Some((f, p, t, boxes))
}

fn overlay(ctx: &egui::Context, ed: &mut Editor, scene: &mut render::Scene) {
    let ppp = f64::from(ctx.pixels_per_point());
    let px = 1. / ed.cam.zoom;
    let Some(run) = &ed.run else { return };
    let gamma = run.data.global.render.alpha_gamma;
    scene.overlay.clear(ed.cam.zoom * ppp, gamma);
    let Some((f, _)) = at_playhead(ed) else { return };
    let fd = &ed.ws.docs[&ed.cur].fighter;
    let hurt = if f.state == State::Knockdown { fd.hurtboxes.knockdown } else { fd.hurtboxes.default };
    let (boxes, shown, t) = match playhead_boxes(ed) {
        Some((f, _, t, b)) => {
            let cd = f.mov.as_ref().map_or(0., |m| m.counter_damage);
            let s = view::shown(&f, &b, t, cd);
            (b, s, t)
        }
        None => (vec![], vec![], 0),
    };
    let _ = t;
    let joints = joint_points(ed, &f);
    let trail: Vec<[f64; 2]> = run.frames.iter().filter_map(|w| w.fighters.get(1)).map(|d| [d.x, d.y - 30.]).collect();
    let counter = sim::combat::countering(&run.data, &f).is_some();
    let o = view::Overlay {
        f: &f,
        hurt,
        boxes: &boxes,
        shown: &shown,
        sel: ed.hitbox,
        hover: ed.hover,
        joints: &joints,
        show_hurt: ed.show_hurt,
        show_inactive: ed.show_inactive,
        trail: if trail.len() > 1 { &trail } else { &[] },
        counter,
    };
    view::draw(&mut scene.overlay, &o, px);
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

fn shortcuts(ctx: &egui::Context, ed: &mut Editor, out: &mut Outbox) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let (cmd, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
    let k = |key: Key| ctx.input(|i| i.key_pressed(key));
    let n = ed.frames();
    if cmd && k(Key::Z) {
        if shift {
            ed.doc().redo()
        } else {
            ed.doc().undo()
        }
        ed.stale = true;
        return;
    }
    if cmd && k(Key::Y) {
        ed.doc().redo();
        ed.stale = true;
        return;
    }
    if cmd && k(Key::S) {
        ed.save();
        return;
    }
    if cmd && k(Key::D) {
        inspect::duplicate_hitbox(ed);
        return;
    }
    if cmd {
        return;
    }
    if k(Key::Space) {
        ed.playing = !ed.playing;
        if ed.playing && ed.frame + 1 >= n {
            ed.frame = 0;
        }
    }
    if k(Key::ArrowLeft) || k(Key::Comma) {
        ed.set_frame(ed.frame.saturating_sub(1));
    }
    if k(Key::ArrowRight) || k(Key::Period) {
        ed.set_frame(ed.frame + 1);
    }
    if k(Key::Home) {
        ed.set_frame(0);
    }
    if k(Key::End) {
        ed.set_frame(n);
    }
    if k(Key::OpenBracket) || k(Key::CloseBracket) {
        let fwd = k(Key::CloseBracket);
        if let Some(r) = &ed.run {
            let starts: Vec<usize> = (0..r.marks.len())
                .filter(|&i| i == 0 || r.marks[i].phase.map(|p| p.0) != r.marks[i - 1].phase.map(|p| p.0))
                .collect();
            let to = if fwd {
                starts.iter().find(|&&s| s > ed.frame).copied()
            } else {
                starts.iter().rev().find(|&&s| s < ed.frame).copied()
            };
            if let Some(s) = to {
                ed.set_frame(s);
            }
        }
    }
    if k(Key::N) {
        inspect::add_hitbox(ed);
    }
    if k(Key::Delete) || k(Key::Backspace) {
        inspect::delete_hitbox(ed);
    }
    if k(Key::Tab)
        && let Some((_, _, _, b)) = playhead_boxes(ed)
        && !b.is_empty()
    {
        ed.hitbox = Some(ed.hitbox.map_or(0, |h| (h + if shift { b.len() - 1 } else { 1 }) % b.len()));
    }
    if k(Key::F) {
        ed.reframe = true;
    }
    if k(Key::Escape) {
        if ed.hitbox.is_some() {
            ed.hitbox = None;
        } else {
            out.request = Some(Request::Back);
        }
    }
}

fn top_bar(root: &mut egui::Ui, ed: &mut Editor, out: &mut Outbox) {
    let frame = egui::Frame::new().fill(style::NIGHT).inner_margin(egui::Margin::symmetric(12, 7));
    egui::Panel::top("ed-top").frame(frame).show(root, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("STICK SMASH").family(style::display()).size(20.).color(STAR));
            ui.label(egui::RichText::new("EDITOR").family(style::display()).size(20.).color(GOLD));
            ui.add_space(14.);
            let ids: Vec<(String, String, bool)> = ed
                .ws
                .data
                .fighters
                .iter()
                .map(|f| (f.id.clone(), f.name.clone(), ed.ws.docs.get(&f.id).is_some_and(|d| d.dirty())))
                .collect();
            let label =
                |(_, name, dirty): &(String, String, bool)| if *dirty { format!("{name} •") } else { name.clone() };
            let cur = ids.iter().find(|x| x.0 == ed.cur).map(label).unwrap_or_default();
            let mut pick = None;
            egui::ComboBox::from_id_salt("ed-fighter").selected_text(cur).width(150.).show_ui(ui, |ui| {
                for x in &ids {
                    if ui.selectable_label(x.0 == ed.cur, label(x)).clicked() {
                        pick = Some(x.0.clone());
                    }
                }
            });
            if let Some(id) = pick {
                ed.open(&id);
            }
            ui.add_space(8.);
            let (can_undo, can_redo, dirty, errors) = {
                let d = ed.doc();
                (d.can_undo(), d.can_redo(), d.dirty(), d.errors.len())
            };
            if ui.add_enabled(can_undo, egui::Button::new("⟲ Undo")).on_hover_text("⌘/Ctrl Z").clicked() {
                ed.doc().undo();
                ed.stale = true;
            }
            if ui.add_enabled(can_redo, egui::Button::new("⟳ Redo")).on_hover_text("⌘/Ctrl Shift Z").clicked() {
                ed.doc().redo();
                ed.stale = true;
            }
            let target = if ed.to_assets { Some(ed.config.assets.clone()) } else { ed.config.user_dir.clone() }
                .map_or("saving disabled".into(), |d| d.join(crate::doc::fighter_path(&ed.cur)).display().to_string());
            let save = egui::Button::new(egui::RichText::new(if dirty { "Save •" } else { "Save" }).color(if dirty {
                style::NIGHT
            } else {
                STAR
            }))
            .fill(if dirty { GOLD } else { style::INK2 });
            if ui.add_enabled(dirty, save).on_hover_text(format!("⌘/Ctrl S → {target}")).clicked() {
                ed.save();
            }
            if ui
                .add_enabled(dirty, egui::Button::new("Revert"))
                .on_hover_text("Back to the saved fighter (undoable)")
                .clicked()
            {
                ed.doc().revert();
                ed.stale = true;
            }
            if ed.config.dev {
                ui.checkbox(&mut ed.to_assets, "into assets/").on_hover_text("Dev: save into the built-in assets");
            }
            ui.add_space(6.);
            let test =
                egui::Button::new(egui::RichText::new("▶ Test").color(STAR)).fill(Color32::from_rgb(40, 110, 80));
            if ui
                .add_enabled(errors == 0, test)
                .on_hover_text("Training with these (unsaved) edits; pause → Back to editor")
                .clicked()
            {
                out.request =
                    Some(Request::Test { data: std::sync::Arc::new(ed.ws.live_data()), fighter: ed.cur.clone() });
            }
            ui.add_space(10.);
            if errors > 0 {
                ui.label(egui::RichText::new(format!("⚠ {errors} problem")).color(RED));
            } else {
                ui.label(egui::RichText::new("✔ valid").color(style::GREEN));
            }
            if let Some((msg, err, _)) = &ed.status {
                ui.label(egui::RichText::new(msg).color(if *err { RED } else { DIM }).small());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Back to menu").on_hover_text("Esc · edits stay open until you quit the game").clicked() {
                    out.request = Some(Request::Back);
                }
                ui.toggle_value(&mut ed.help, "?").on_hover_text("Shortcuts");
            });
        });
    });
}

const GROUPS: [(&str, &[&str]); 5] = [
    ("Ground", &["jab", "ftilt", "utilt", "dtilt", "dashatk"]),
    ("Smash", &["fsmash", "usmash", "dsmash"]),
    ("Air", &["nair", "fair", "bair", "uair", "dair"]),
    ("Special", &["nspec", "sspec", "uspec", "dspec"]),
    ("Grab", &["grab", "dashgrab"]),
];

fn browser(ui: &mut egui::Ui, ed: &mut Editor) {
    let mut pick: Option<Subject> = None;
    let mut add_sspec = false;
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            ui.add_space(4.);
            let doc = ed.ws.docs.get(&ed.cur).expect("open");
            let (f, saved) = (&doc.fighter, &doc.saved);
            let item = |ui: &mut egui::Ui, on: bool, key: &str, name: &str, dirty: bool| {
                let mut t = egui::text::LayoutJob::default();
                let c = if on { GOLD } else { STAR };
                t.append(key, 0., egui::TextFormat::simple(FontId::new(15., style::label()), c));
                t.append(
                    &format!("  {name}"),
                    0.,
                    egui::TextFormat::simple(FontId::proportional(12.), if on { GOLD } else { DIM }),
                );
                if dirty {
                    t.append("  •", 0., egui::TextFormat::simple(FontId::proportional(12.), GOLD));
                }
                ui.add(egui::Button::selectable(on, t).min_size(egui::vec2(0., 20.))).clicked()
            };
            for (group, slots) in GROUPS {
                ui.label(caps(group));
                for &slot in slots {
                    match move_ref(&f.moves, slot) {
                        Some(m) => {
                            let dirty = move_ref(&saved.moves, slot) != Some(m);
                            if item(ui, ed.subject == Subject::Move(slot.into()), slot, &m.name, dirty) {
                                pick = Some(Subject::Move(slot.into()));
                            }
                        }
                        None => {
                            if ui.small_button("+ side special").on_hover_text("Start from a copy of nspec").clicked() {
                                add_sspec = true;
                            }
                        }
                    }
                }
                ui.add_space(4.);
            }
            ui.label(caps("Throws"));
            for (k, name) in [("f", "Forward"), ("b", "Back"), ("u", "Up"), ("d", "Down")] {
                let dirty =
                    crate::throw_mut(&mut saved.throws.clone(), k) != crate::throw_mut(&mut f.throws.clone(), k);
                if item(ui, ed.subject == Subject::Throw(k.into()), &format!("throw.{k}"), name, dirty) {
                    pick = Some(Subject::Throw(k.into()));
                }
            }
            ui.add_space(4.);
            ui.label(caps("Fighter"));
            let dirty = f.stats != saved.stats
                || f.body != saved.body
                || f.hurtboxes != saved.hurtboxes
                || f.name != saved.name;
            if item(ui, ed.subject == Subject::Fighter, "stats", "& body", dirty) {
                pick = Some(Subject::Fighter);
            }
            ui.add_space(4.);
            egui::CollapsingHeader::new(caps(&format!("Poses ({})", f.poses.len())))
                .default_open(matches!(ed.subject, Subject::Pose(_)))
                .show(ui, |ui| {
                    for (name, p) in &f.poses {
                        let dirty = saved.poses.get(name) != Some(p);
                        if item(ui, ed.subject == Subject::Pose(name.clone()), name, "", dirty) {
                            pick = Some(Subject::Pose(name.clone()));
                        }
                    }
                });
        });
    });
    if add_sspec {
        let f = fighter(ed);
        let mut m = f.moves.nspec.clone();
        m.name = format!("{} (side)", m.name);
        f.moves.sspec = Some(m);
        pick = Some(Subject::Move("sspec".into()));
    }
    if let Some(s) = pick {
        ed.select(s);
    }
}

fn viewport(ui: &mut egui::Ui, ed: &mut Editor) {
    let rect = ui.max_rect();
    let win = ui.ctx().viewport_rect();
    ed.port = view::Port {
        rect: [rect.left(), rect.top(), rect.right(), rect.bottom()].map(f64::from),
        window: [f64::from(win.width()), f64::from(win.height())],
    };
    if ed.reframe
        && let Some((f, _)) = ed.run.as_ref().map(|r| (r.frames[0].fighters[0].clone(), ()))
    {
        ed.reframe = false;
        ed.cam.zoom = (f64::from(rect.height()) / 240.).clamp(0.5, 8.);
        ed.cam.center = [f.x + 30., f.y - 60.];
    }
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let to_stage = |ed: &Editor, p: egui::Pos2| ed.cam.to_stage(&ed.port, [f64::from(p.x), f64::from(p.y)]);
    let px = 1. / ed.cam.zoom;
    let shift = ui.input(|i| i.modifiers.shift);
    let boxes = playhead_boxes(ed);
    let mouse = resp.hover_pos().or(resp.interact_pointer_pos());
    // Hover.
    if ed.drag.is_none() {
        ed.hover = mouse.filter(|_| resp.hovered()).and_then(|m| {
            let m = to_stage(ed, m);
            let (f, shown, b) = match &boxes {
                Some((f, _, t, b)) => {
                    (f.clone(), view::shown(f, b, *t, f.mov.as_ref().map_or(0., |m| m.counter_damage)), b.clone())
                }
                None => (at_playhead(ed)?.0, vec![], vec![]),
            };
            let joints = joint_points(ed, &f);
            view::hit_test(&f, &b, &shown, ed.hitbox, &joints, m, px)
        });
    }
    // Zoom.
    if resp.hovered() {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let k = f64::from(pinch) * (f64::from(scroll) * 0.0025).exp();
        if (k - 1.).abs() > 1e-4
            && let Some(m) = mouse
        {
            let port = ed.port;
            ed.cam.zoom_at(&port, [f64::from(m.x), f64::from(m.y)], k);
        }
    }
    // Drag start.
    if resp.drag_started_by(PointerButton::Secondary) || resp.drag_started_by(PointerButton::Middle) {
        ed.drag = Some(Drag::Pan);
    } else if resp.drag_started_by(PointerButton::Primary) {
        let grab = resp.interact_pointer_pos().map(|p| to_stage(ed, p)).unwrap_or_default();
        ed.drag = match (ed.hover, &boxes) {
            (Some(h @ (Hit::Body(i) | Hit::Rim(i) | Hit::Angle(i))), Some((f, p, t, b))) => {
                ed.hitbox = Some(i);
                let (g, _) = view::geom_source(&b[i], *t);
                Some(Drag::Box { kind: h, phase: *p, t: *t, f: Box::new(f.clone()), grab, orig: g })
            }
            (Some(Hit::Joint(j)), _) => match (pose_target(ed), at_playhead(ed)) {
                (Some((target, skel)), Some((f, _))) => Some(Drag::Joint { j, target, f: Box::new(f), skel }),
                _ => None,
            },
            _ => Some(Drag::Pan),
        };
    }
    if resp.dragged()
        && let Some(drag) = ed.drag.clone()
    {
        match drag {
            Drag::Pan => {
                let d = resp.drag_delta();
                ed.cam.center[0] -= f64::from(d.x) * px;
                ed.cam.center[1] -= f64::from(d.y) * px;
            }
            Drag::Box { kind, phase, t, f, grab, orig } => {
                if let Some(m) = resp.interact_pointer_pos().map(|p| to_stage(ed, p)) {
                    drag_box(ed, kind, phase, t, &f, grab, orig, m, shift);
                }
            }
            Drag::Joint { j, target, f, skel } => {
                if let Some(m) = resp.interact_pointer_pos().map(|p| to_stage(ed, p)) {
                    let p = view::stage_to_joint(&f, &skel, m).map(|v| (v * 2.).round() / 2.);
                    if let Some(pose) = pose_mut(fighter(ed), &target) {
                        *view::pose_joint(pose, j) = Some(p);
                    }
                }
            }
        }
    }
    if resp.drag_stopped() {
        ed.drag = None;
    }
    if resp.clicked() {
        match ed.hover {
            Some(Hit::Body(i) | Hit::Rim(i) | Hit::Angle(i)) => ed.hitbox = Some(i),
            Some(Hit::Joint(_)) => {}
            None => ed.hitbox = None,
        }
    }
    let cursor = match (&ed.drag, ed.hover) {
        (Some(Drag::Pan), _) => egui::CursorIcon::Grabbing,
        (_, Some(Hit::Rim(_))) => egui::CursorIcon::ResizeNeSw,
        (_, Some(Hit::Angle(_))) => egui::CursorIcon::Crosshair,
        (_, Some(_)) => egui::CursorIcon::Move,
        _ => egui::CursorIcon::Default,
    };
    if resp.hovered() || ed.drag.is_some() {
        ui.ctx().set_cursor_icon(cursor);
    }
    viewport_chrome(ui, ed, rect);
}

#[allow(clippy::too_many_arguments)]
fn drag_box(
    ed: &mut Editor,
    kind: Hit,
    phase: usize,
    t: u32,
    f: &Fighter,
    grab: [f64; 2],
    orig: (f64, f64, f64),
    m: [f64; 2],
    shift: bool,
) {
    let Subject::Move(slot) = ed.subject.clone() else { return };
    let half = |v: f64| (v * 2.).round() / 2.;
    let Some(h) =
        move_mut(&mut fighter(ed).moves, &slot).and_then(|mv| mv.phases.get_mut(phase)).and_then(|p| match kind {
            Hit::Body(i) | Hit::Rim(i) | Hit::Angle(i) => p.hitboxes.get_mut(i),
            Hit::Joint(_) => None,
        })
    else {
        return;
    };
    let (x, y, r) = orig;
    let (nx, ny, nr) = match kind {
        Hit::Body(_) => {
            let (a, b) = (view::stage_to_local(f, grab), view::stage_to_local(f, m));
            (half(x + b[0] - a[0]), half(y + b[1] - a[1]), r)
        }
        Hit::Rim(_) => {
            let l = view::stage_to_local(f, m);
            (x, y, half((l[0] - x).hypot(l[1] - y)).max(1.))
        }
        Hit::Angle(_) => {
            let a = view::drag_angle(f, h, view::local_to_stage(f, [x, y]), m);
            h.angle = if shift { (a / 15.).round() * 15. } else { a.round() };
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

/// Toggles, zoom and hints drawn over the viewport.
fn viewport_chrome(ui: &mut egui::Ui, ed: &mut Editor, rect: Rect) {
    let bar = Rect::from_min_size(rect.min + egui::vec2(10., 10.), egui::vec2(rect.width() - 20., 26.));
    ui.scope_builder(UiBuilder::new().max_rect(bar), |ui| {
        ui.horizontal(|ui| {
            egui::Frame::new()
                .fill(INK.gamma_multiply(0.85))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.toggle_value(&mut ed.show_hurt, "Hurtbox");
                        ui.toggle_value(&mut ed.show_inactive, "Inactive boxes");
                        if ui.button("Frame (F)").clicked() {
                            ed.reframe = true;
                        }
                        ui.label(egui::RichText::new(format!("{:.0}%", ed.cam.zoom * 100. / 3.)).color(FAINT).small());
                    });
                });
        });
    });
    let sandbox = matches!(ed.subject, Subject::Move(_) | Subject::Throw(_));
    let bar2 = Rect::from_min_size(rect.min + egui::vec2(10., 44.), egui::vec2(rect.width() - 20., 26.));
    if sandbox {
        ui.scope_builder(UiBuilder::new().max_rect(bar2), |ui| {
            ui.horizontal(|ui| {
                egui::Frame::new()
                    .fill(INK.gamma_multiply(0.85))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(8, 3))
                    .show(ui, |ui| ui.horizontal(|ui| setup_bar(ui, ed)));
            });
        });
    }
    let painter = ui.painter_at(rect);
    let hint = match (&ed.subject, ed.hover, ed.playing) {
        (_, _, true) => "Space pauses",
        (_, Some(Hit::Angle(_)), _) => "Drag to aim the launch angle · Shift snaps to 15°",
        (_, Some(Hit::Rim(_)), _) => "Drag to resize",
        (_, Some(Hit::Body(_)), _) => "Drag to move · click to select",
        (_, Some(Hit::Joint(j)), _) => view::JOINTS[j],
        (Subject::Move(_), _, _) => {
            if ed.hitbox.is_some() {
                "Drag the box, its rim or the gold dot · N new · ⌘D duplicate · Del delete · Tab next"
            } else {
                "Click a hitbox · N new · drag joints on pose keys · wheel zoom · right-drag pan"
            }
        }
        (Subject::Pose(_), _, _) => "Drag joints to pose · wheel zoom · right-drag pan",
        _ => "Wheel zoom · right-drag pan",
    };
    painter.text(
        rect.left_bottom() + egui::vec2(12., -10.),
        Align2::LEFT_BOTTOM,
        hint,
        FontId::proportional(12.5),
        DIM,
    );
    let errors = ed.ws.docs.get(&ed.cur).map(|d| d.errors.clone()).unwrap_or_default();
    if let Some(e) = errors.first() {
        let r = Rect::from_min_size(rect.left_top() + egui::vec2(10., 78.), egui::vec2(rect.width() - 20., 26.));
        painter.rect_filled(r, 6, Color32::from_rgba_unmultiplied(90, 20, 30, 230));
        painter.text(
            r.left_center() + egui::vec2(10., 0.),
            Align2::LEFT_CENTER,
            format!("Preview paused · {}: {}", e.path, e.msg),
            FontId::proportional(12.5),
            STAR,
        );
    }
}

fn help(ctx: &egui::Context, ed: &mut Editor) {
    egui::Window::new("Shortcuts")
        .open(&mut ed.help)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::RIGHT_TOP, [-360., 56.])
        .show(ctx, |ui| {
            egui::Grid::new("ed-keys").num_columns(2).spacing([16., 4.]).show(ui, |ui| {
                for (k, v) in [
                    ("Space", "play / pause"),
                    ("← →  or  , .", "step a frame"),
                    ("Home / End", "first / last frame"),
                    ("[  ]", "previous / next phase"),
                    ("Click / Tab", "select hitbox"),
                    ("N", "new hitbox on this frame"),
                    ("⌘/Ctrl D", "duplicate hitbox"),
                    ("Delete", "delete hitbox"),
                    ("Shift + drag dot", "snap angle 15°"),
                    ("Wheel / pinch", "zoom at cursor"),
                    ("Right / middle drag", "pan"),
                    ("F", "frame the fighter"),
                    ("⌘/Ctrl Z, Shift Z", "undo, redo"),
                    ("⌘/Ctrl S", "save"),
                    ("Esc", "deselect, then back to menu"),
                ] {
                    ui.label(egui::RichText::new(k).color(GOLD));
                    ui.label(v);
                    ui.end_row();
                }
            });
        });
}

const PHASE_COLORS: [Color32; 6] = [
    Color32::from_rgb(63, 182, 255),
    Color32::from_rgb(255, 138, 31),
    Color32::from_rgb(176, 112, 255),
    Color32::from_rgb(42, 212, 192),
    Color32::from_rgb(255, 93, 200),
    Color32::from_rgb(168, 196, 255),
];

fn timeline(ui: &mut egui::Ui, ed: &mut Editor) {
    ui.add_space(6.);
    let n = ed.frames();
    ui.horizontal(|ui| {
        if ui.button("⏮").on_hover_text("First frame (Home)").clicked() {
            ed.set_frame(0);
        }
        if ui.button("◀").on_hover_text("Previous frame (← or ,)").clicked() {
            ed.set_frame(ed.frame.saturating_sub(1));
        }
        let play = if ed.playing { "⏸" } else { "▶" };
        if ui
            .add(egui::Button::new(egui::RichText::new(play).color(GOLD)).min_size(egui::vec2(34., 0.)))
            .on_hover_text("Space")
            .clicked()
        {
            ed.playing = !ed.playing;
            if ed.playing && ed.frame + 1 >= n {
                ed.frame = 0;
            }
        }
        if ui.button("▶|").on_hover_text("Next frame (→ or .)").clicked() {
            ed.set_frame(ed.frame + 1);
        }
        if ui.button("⏭").on_hover_text("Last frame (End)").clicked() {
            ed.set_frame(n);
        }
        for (s, l) in [(1., "1×"), (0.5, "½×"), (0.25, "¼×")] {
            ui.selectable_value(&mut ed.speed, s, l);
        }
        ui.add_space(8.);
        let info = match ed.run.as_ref().map(|r| r.marks[ed.frame.min(n - 1)]) {
            Some(m) => match (m.phase, &ed.subject) {
                (Some((p, t)), Subject::Move(slot)) => {
                    let name = move_ref(&ed.ws.docs[&ed.cur].fighter.moves, slot)
                        .and_then(|mv| mv.phases.get(p))
                        .map_or(String::new(), |ph| ph.name.clone());
                    format!("{name} · phase frame {t}")
                }
                _ => m.state.name().to_string(),
            },
            None => String::new(),
        };
        ui.label(egui::RichText::new(format!("Frame {}", ed.frame)).family(style::label()).size(17.).color(STAR));
        ui.label(egui::RichText::new(format!("/ {}   {info}", n - 1)).color(DIM));
    });
    ui.add_space(4.);
    track(ui, ed);
    ui.add_space(6.);
}

fn setup_bar(ui: &mut egui::Ui, ed: &mut Editor) {
    let throw = matches!(ed.subject, Subject::Throw(_));
    let s = &mut ed.setup;
    ui.label(caps("Sandbox"));
    let mut air = s.air.is_some();
    if ui.add_enabled(!throw, egui::Checkbox::new(&mut air, "Airborne")).changed() {
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
    ui.label(egui::RichText::new("Hold").color(DIM));
    ui.add(egui::DragValue::new(&mut s.hold).range(0..=240).suffix(" f"))
        .on_hover_text("Hold the move's button this many frames (charge, release)");
}

fn track(ui: &mut egui::Ui, ed: &mut Editor) {
    let Some(run) = &ed.run else { return };
    let n = run.marks.len();
    let doc = &ed.ws.docs[&ed.cur];
    let (boxes, events, keys, phase_names) = match &ed.subject {
        Subject::Move(slot) => match move_ref(&doc.fighter.moves, slot) {
            Some(mv) => {
                let p = mv.phases.get(ed.phase);
                (
                    p.map(|p| p.hitboxes.clone()).unwrap_or_default(),
                    p.map(|p| p.events.clone()).unwrap_or_default(),
                    p.map(|p| p.anim.iter().map(|k| k.at).collect::<Vec<_>>()).unwrap_or_default(),
                    mv.phases.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
                )
            }
            None => Default::default(),
        },
        _ => Default::default(),
    };
    let rows = boxes.len().min(8);
    let row_h = 13.;
    let label_w = 76.;
    let height = 18. + 20. + rows as f32 * row_h + 2. * 14. + 6.;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6, style::NIGHT);
    let x0 = rect.left() + label_w;
    let w = ((rect.width() - label_w - 8.) / n as f32).max(1.);
    let fx = |k: usize| x0 + k as f32 * w;
    let small = FontId::proportional(10.5);
    // Ruler.
    let step = if w >= 9. {
        5
    } else if w >= 3. {
        10
    } else {
        30
    };
    for k in 0..n {
        let x = fx(k);
        let major = k % step == 0;
        if major || w >= 5. {
            p.line_segment(
                [egui::pos2(x, rect.top() + if major { 4. } else { 10. }), egui::pos2(x, rect.top() + 16.)],
                Stroke::new(1., LINE),
            );
        }
        if major {
            p.text(egui::pos2(x + 2., rect.top() + 2.), Align2::LEFT_TOP, k.to_string(), small.clone(), FAINT);
        }
    }
    let mut y = rect.top() + 18.;
    let lbl = |y: f32, s: &str| {
        p.text(egui::pos2(rect.left() + 8., y + 7.), Align2::LEFT_CENTER, s, FontId::new(13., style::label()), DIM)
    };
    // Phase segments (from the simulation).
    lbl(y + 2., "PHASES");
    let mut k = 0;
    while k < n {
        let m = run.marks[k];
        let mut e = k;
        while e + 1 < n
            && run.marks[e + 1].phase.map(|p| p.0) == m.phase.map(|p| p.0)
            && run.marks[e + 1].state == m.state
        {
            e += 1;
        }
        let r = Rect::from_min_max(egui::pos2(fx(k) + 0.5, y), egui::pos2(fx(e + 1) - 0.5, y + 17.));
        let (fill, name) = match m.phase {
            Some((pi, _)) => (PHASE_COLORS[pi % 6], phase_names.get(pi).cloned().unwrap_or_default()),
            None => (style::INK2, m.state.name().to_string()),
        };
        let sel = m.phase.is_some_and(|(pi, _)| pi == ed.phase);
        p.rect_filled(r, 3, if m.phase.is_some() { fill.gamma_multiply(if sel { 0.85 } else { 0.45 }) } else { fill });
        if r.width() > 30. {
            p.text(
                r.left_center() + egui::vec2(5., 0.),
                Align2::LEFT_CENTER,
                name,
                FontId::new(12.5, style::label()),
                if m.phase.is_some() { style::NIGHT } else { FAINT },
            );
        }
        k = e + 1;
    }
    y += 20.;
    let in_phase = |k: usize| run.marks[k].phase.filter(|(pi, _)| *pi == ed.phase).map(|(_, t)| t);
    // Hitbox windows for the selected phase.
    let mut row_hit = None;
    for (i, h) in boxes.iter().take(rows).enumerate() {
        let ry = y + i as f32 * row_h;
        let sel = ed.hitbox == Some(i);
        if sel {
            p.rect_filled(
                Rect::from_min_max(egui::pos2(rect.left() + 2., ry), egui::pos2(rect.right() - 2., ry + row_h - 1.)),
                2,
                GOLD.gamma_multiply(0.12),
            );
        }
        p.text(
            egui::pos2(rect.left() + 8., ry + row_h / 2.),
            Align2::LEFT_CENTER,
            format!("hitbox {i}"),
            small.clone(),
            if sel { GOLD } else { FAINT },
        );
        let c = render::debug::hit_type_color(h.kind);
        let col = Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
        for k in 0..n {
            if in_phase(k).is_some_and(|t| h.active(t)) {
                p.rect_filled(
                    Rect::from_min_max(egui::pos2(fx(k), ry + 2.), egui::pos2(fx(k + 1), ry + row_h - 3.)),
                    0,
                    col.gamma_multiply(if sel { 1. } else { 0.7 }),
                );
            }
        }
        if let Some(pos) = resp.interact_pointer_pos()
            && resp.clicked()
            && pos.y >= ry
            && pos.y < ry + row_h
        {
            row_hit = Some(i);
        }
    }
    y += rows as f32 * row_h;
    // Events and pose keys of the selected phase.
    lbl(y, "EVENTS");
    for k in 0..n {
        if let Some(t) = in_phase(k)
            && events.iter().any(|e| e.fires(t, run.frames[k].frame))
        {
            let c = egui::pos2(fx(k) + w / 2., y + 7.);
            p.circle_filled(c, 3.5_f32.min(w / 2. + 1.), STAR);
        }
    }
    y += 14.;
    lbl(y, "POSE KEYS");
    for k in 0..n {
        if let Some(t) = in_phase(k)
            && keys.iter().any(|&a| a == f64::from(t))
        {
            let c = egui::pos2(fx(k) + w / 2., y + 7.);
            let s = 4.5_f32.min(w / 2. + 2.);
            p.add(egui::Shape::convex_polygon(
                vec![c + egui::vec2(0., -s), c + egui::vec2(s, 0.), c + egui::vec2(0., s), c + egui::vec2(-s, 0.)],
                GOLD,
                Stroke::NONE,
            ));
        }
    }
    // Playhead.
    let x = fx(ed.frame.min(n - 1)) + w / 2.;
    p.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], Stroke::new(2., GOLD));
    p.rect_filled(Rect::from_center_size(egui::pos2(x, rect.top() + 3.), egui::vec2(9., 7.)), 2, GOLD);
    if let Some(i) = row_hit {
        ed.hitbox = Some(i);
    }
    if (resp.dragged() || resp.clicked())
        && let Some(pos) = resp.interact_pointer_pos()
        && pos.x >= x0
    {
        let k = (((pos.x - x0) / w).floor().max(0.) as usize).min(n - 1);
        ed.set_frame(k);
    }
    if resp.hovered() && ui.input(|i| i.pointer.primary_down()) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
}
