//! The egui layer's frame, shared by the fighter and stage editors: layout, top bar (mode switch, picker, undo,
//! saving, test, status), keyboard shortcuts, dialogs, toasts and the shortcut sheet. Panels live in `browser`,
//! `viewport`, `timeline` and `inspect` (fighters) and `stage_panels` and `stage_port` (stages).

mod dialogs;

use bevy_egui::egui::{self, Align2, Color32, Key, LayerId, RichText, UiBuilder};
use render::pose::{Skel, resolve};
use sim::data::visual::{Pose, PoseRef};
use sim::world::Fighter;

use crate::preview::Subject;
use crate::stage_ed::{self, SANDBOX_MAX};
use crate::stage_view;
use crate::style::{self, DIM, FAINT, GOLD, INK2, NIGHT, RED, STAR, faint, key};
use crate::{
    Dialog, Editor, Mode, Outbox, Request, browser, doc, inspect, stage_panels, stage_port, timeline, viewport,
};

/// A pose being edited in the viewport: a named pose, or an inline pose in a move's anim key.
#[derive(Clone, Debug, PartialEq)]
pub enum PoseTarget {
    Named(String),
    Inline {
        slot: String,
        phase: usize,
        key: usize,
        /// Phase timeline the key lives in (0 = the move's default `phases`).
        variant: u8,
    },
}

pub fn pose_mut<'a>(f: &'a mut sim::data::Fighter, t: &PoseTarget) -> Option<&'a mut Pose> {
    match t {
        PoseTarget::Named(n) => f.poses.get_mut(n),
        PoseTarget::Inline { slot, phase, key, variant } => {
            match &mut f.moves.get_mut(slot)?.timeline_mut(*variant).get_mut(*phase)?.anim.get_mut(*key)?.pose {
                PoseRef::Inline(p) => Some(p),
                PoseRef::Named(_) => None,
            }
        }
    }
}

/// The fighter on the playhead frame, and its phase/phase frame if it's in the move.
pub(crate) struct Playhead {
    pub fighter: Fighter,
    pub phase: Option<(usize, u32)>,
    /// Timeline the simulated move ran (see `preview::Mark::variant`).
    pub variant: u8,
}

pub(crate) fn at_playhead(ed: &Editor) -> Option<Playhead> {
    let r = ed.run.as_ref()?;
    let k = ed.frame.min(r.frames.len() - 1);
    Some(Playhead { fighter: r.frames[k].fighters[0].clone(), phase: r.marks[k].phase, variant: r.marks[k].variant })
}

/// The pose shown on the playhead, if it's exactly a key (not mid-blend), and its skeleton.
pub(crate) fn pose_target(ed: &Editor) -> Option<(PoseTarget, Skel)> {
    let fd = &ed.ws.docs.get(&ed.cur)?.item;
    match &ed.subject {
        Subject::Pose(n) => fd.poses.get(n).map(|p| (PoseTarget::Named(n.clone()), resolve(fd, p))),
        Subject::Move(slot) => {
            let Playhead { fighter: f, phase: Some((p, t)), variant } = at_playhead(ed)? else { return None };
            let anim = &fd.moves.get(slot)?.timeline(variant).get(p)?.anim;
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
                    Some((PoseTarget::Inline { slot: slot.clone(), phase: p, key, variant }, resolve(fd, pose)))
                }
            }
        }
        _ => None,
    }
}

/// Live problems of the current fighter or stage plus the last refused save.
pub(crate) fn problems(ed: &Editor) -> Vec<sim::data::ContentError> {
    let mut list = match ed.mode {
        Mode::Fighters => ed.ws.docs.get(&ed.cur).map(|d| d.errors.clone()),
        Mode::Stages => ed.ws.stages.get(&ed.st.cur).map(|d| d.errors.clone()),
    }
    .unwrap_or_default();
    let extra: Vec<_> = ed.problems.iter().filter(|p| !list.contains(p)).cloned().collect();
    list.extend(extra);
    list
}

pub(crate) fn problem_panel(ui: &mut egui::Ui, list: &[sim::data::ContentError]) -> Option<String> {
    if list.is_empty() {
        return None;
    }
    ui.add_space(10.);
    let mut go = None;
    egui::Frame::new().fill(style::ERR_BG).corner_radius(8).inner_margin(10).show(ui, |ui| {
        ui.label(RichText::new("Problems (fix before saving)").color(RED).strong());
        for e in list {
            let txt = format!("{}: {}", if e.path.is_empty() { &e.file } else { &e.path }, e.msg);
            if ui.link(RichText::new(txt).color(style::ERR_TEXT)).on_hover_text("Go there").clicked() {
                go = Some(e.path.clone());
            }
        }
    });
    go
}

pub(crate) fn run(ctx: &egui::Context, ed: &mut Editor, scene: &mut render::Scene, dt: f32, out: &mut Outbox) {
    if ed.dialog.is_none() {
        shortcuts(ctx, ed, out);
    }
    let setup = ed.setup.clone();
    let mut root = egui::Ui::new(
        ctx.clone(),
        "editor".into(),
        UiBuilder::new().layer_id(LayerId::background()).max_rect(ctx.viewport_rect()),
    );
    top_bar(&mut root, ed, out);
    let before = Snapshot::take(ed);
    let browser_panel = egui::Panel::left("ed-browser")
        .default_size(style::BROWSER_W)
        .size_range(style::BROWSER_MIN..=style::BROWSER_MAX);
    let inspector_panel = egui::Panel::right("ed-inspector").default_size(style::INSPECTOR_W).size_range(300.0..=560.);
    match ed.mode {
        Mode::Fighters => {
            browser_panel.show(&mut root, |ui| browser::browser(ui, ed));
            inspector_panel.show(&mut root, |ui| inspect::inspector(ui, ed));
            egui::Panel::bottom("ed-timeline").resizable(false).show(&mut root, |ui| timeline::timeline(ui, ed));
            egui::CentralPanel::no_frame().show(&mut root, |ui| viewport::viewport(ui, ed));
        }
        Mode::Stages => {
            browser_panel.show(&mut root, |ui| stage_panels::browser(ui, ed));
            inspector_panel.show(&mut root, |ui| stage_panels::inspector(ui, ed));
            egui::CentralPanel::no_frame().show(&mut root, |ui| stage_port::viewport(ui, ed));
        }
    }
    if ed.help {
        help(ctx, ed);
    }
    dialogs::show(ctx, ed, out);
    toasts(ctx, ed, dt);
    let ongoing = ctx.input(|i| i.pointer.any_down()) || ctx.egui_wants_keyboard_input();
    record(ed, before, ongoing);
    if ed.setup != setup {
        ed.pane.stale = true;
    }
    match ed.mode {
        Mode::Fighters => {
            ed.refresh();
            viewport::sync_scene(ed, scene, dt);
            viewport::overlay(ctx, ed, scene);
        }
        Mode::Stages => {
            stage_ed::sync_scene(ed, scene, dt);
            stage_port::overlay(ctx, ed, scene);
        }
    }
}

/// Records this frame's active-document edits (`ongoing`: a drag or typing that later frames join).
/// A snapshot of a document switched away from mid-frame doesn't apply.
pub(crate) enum Snapshot {
    Fighter(String, Box<sim::data::Fighter>),
    Stage(String, Box<sim::data::Stage>),
}

impl Snapshot {
    pub(crate) fn take(ed: &mut Editor) -> Self {
        match ed.mode {
            Mode::Fighters => Self::Fighter(ed.cur.clone(), Box::new(ed.doc().item.clone())),
            Mode::Stages => Self::Stage(ed.st.cur.clone(), Box::new(ed.stage_doc().item.clone())),
        }
    }
}

pub(crate) fn record(ed: &mut Editor, before: Snapshot, ongoing: bool) {
    fn record_doc<T: doc::Content>(d: &mut doc::Doc<T>, before: T, ongoing: bool) -> bool {
        if d.item != before {
            d.changed(before, ongoing);
            true
        } else {
            if !ongoing {
                d.close_step();
            }
            false
        }
    }
    if std::mem::take(&mut ed.moved_history) {
        ed.invalidate();
        return;
    }
    match before {
        Snapshot::Fighter(id, before) if ed.cur == id => {
            if record_doc(ed.doc(), *before, ongoing) {
                ed.pane.stale = true;
            }
        }
        Snapshot::Stage(id, before) if ed.st.cur == id => {
            if record_doc(ed.stage_doc(), *before, ongoing) {
                ed.st.pane.stale = true;
            }
        }
        _ => {}
    }
}

pub(crate) fn toggle_play(ed: &mut Editor) {
    ed.playing = !ed.playing;
    if ed.playing && ed.frame + 1 >= ed.frames() {
        ed.frame = 0;
    }
}

pub(crate) fn undo(ed: &mut Editor, redo: bool) {
    ed.moved_history = true;
    let can = match ed.mode {
        Mode::Fighters => {
            let d = ed.doc();
            let can = if redo { d.can_redo() } else { d.can_undo() };
            if redo {
                d.redo()
            } else {
                d.undo()
            }
            ed.pane.stale = true;
            can
        }
        Mode::Stages => {
            let d = ed.stage_doc();
            let can = if redo { d.can_redo() } else { d.can_undo() };
            if redo {
                d.redo()
            } else {
                d.undo()
            }
            ed.st.pane.stale = true;
            ed.st.pane.drag = None;
            can
        }
    };
    if can {
        ed.say(if redo { "Redid the edit · ⌘Z undoes it" } else { "Undid the last edit · ⇧⌘Z redoes it" }, false);
    }
}

/// Plays the (unsaved) content: training with the current fighter, or training / a match on the current stage.
pub(crate) fn test(ed: &mut Editor, out: &mut Outbox, versus: bool) {
    let ok = match ed.mode {
        Mode::Fighters => ed.ws.docs.get(&ed.cur).is_some_and(|d| d.errors.is_empty()),
        Mode::Stages => ed.ws.stages.get(&ed.st.cur).is_some_and(|d| d.errors.is_empty()),
    };
    if !ok {
        ed.say("Fix the problems before testing", true);
        return;
    }
    let data = std::sync::Arc::new(ed.ws.live_data());
    out.request = Some(match ed.mode {
        Mode::Fighters => Request::Test { data, fighter: Some(ed.cur.clone()), stage: None, versus: false },
        Mode::Stages => Request::Test { data, fighter: None, stage: Some(ed.st.cur.clone()), versus },
    });
}

/// The next/previous browser entry (moves, throws, the fighter, sections), for ↑ / ↓.
fn step_subject(ed: &mut Editor, by: isize) {
    let list = browser::visible(ed);
    if list.is_empty() {
        return;
    }
    let i = list.iter().position(|s| *s == ed.subject).map_or(0, |i| i as isize + by);
    let i = i.clamp(0, list.len() as isize - 1) as usize;
    ed.select(list[i].clone());
}

fn shortcuts(ctx: &egui::Context, ed: &mut Editor, out: &mut Outbox) {
    let (cmd, shift, alt) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));
    let k = |key: Key| ctx.input(|i| i.key_pressed(key));
    if cmd && k(Key::S) {
        if shift {
            ed.save_all()
        } else {
            ed.save()
        };
        return;
    }
    if cmd && k(Key::F) && ed.mode == Mode::Fighters {
        ed.focus_search = true;
        return;
    }
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    if cmd {
        if k(Key::Z) {
            undo(ed, shift);
        } else if k(Key::Y) {
            undo(ed, true);
        } else if k(Key::D) {
            match ed.mode {
                Mode::Fighters => inspect::duplicate_hitbox(ed),
                Mode::Stages => stage_ed::duplicate_selected(ed),
            }
        } else if k(Key::Enter) {
            test(ed, out, ed.mode == Mode::Stages && shift);
        } else if k(Key::Num1) {
            ed.set_mode(Mode::Fighters);
        } else if k(Key::Num2) {
            ed.set_mode(Mode::Stages);
        }
        return;
    }
    let selected = match ed.mode {
        Mode::Fighters => ed.hitbox.is_some(),
        Mode::Stages => ed.st.sel.is_some(),
    };
    if alt && selected {
        let (step, y) = match ed.mode {
            Mode::Fighters => (if shift { 5. } else { 0.5 }, -1.),
            Mode::Stages => (if shift { 10. } else { 1. }, 1.),
        };
        for (key, [dx, dy]) in [
            (Key::ArrowLeft, [-step, 0.]),
            (Key::ArrowRight, [step, 0.]),
            (Key::ArrowUp, [0., -step * y]),
            (Key::ArrowDown, [0., step * y]),
        ] {
            if k(key) {
                match ed.mode {
                    Mode::Fighters => inspect::nudge_hitbox(ed, dx, dy),
                    Mode::Stages => stage_ed::nudge(ed, [dx, dy]),
                }
            }
        }
        return;
    }
    match ed.mode {
        Mode::Fighters => fighter_shortcuts(ed, shift, &k),
        Mode::Stages => stage_shortcuts(ed, shift, &k),
    }
    if k(Key::Slash) || k(Key::Questionmark) || k(Key::F1) {
        ed.help = !ed.help;
    }
    let selected = match ed.mode {
        Mode::Fighters => ed.hitbox.is_some(),
        Mode::Stages => ed.st.sel.is_some(),
    };
    if k(Key::Escape) {
        if ed.help {
            ed.help = false;
        } else if selected {
            match ed.mode {
                Mode::Fighters => ed.hitbox = None,
                Mode::Stages => ed.st.sel = None,
            }
        } else {
            ed.leave(false, out);
        }
    }
}

fn fighter_shortcuts(ed: &mut Editor, shift: bool, k: &impl Fn(Key) -> bool) {
    let n = ed.frames();
    if k(Key::Space) {
        toggle_play(ed);
    }
    let step = if shift { 10 } else { 1 };
    if k(Key::ArrowLeft) || k(Key::Comma) {
        ed.set_frame(ed.frame.saturating_sub(step));
    }
    if k(Key::ArrowRight) || k(Key::Period) {
        ed.set_frame(ed.frame + step);
    }
    if k(Key::ArrowUp) {
        step_subject(ed, -1);
    }
    if k(Key::ArrowDown) {
        step_subject(ed, 1);
    }
    if k(Key::Home) {
        ed.set_frame(0);
    }
    if k(Key::End) {
        ed.set_frame(n);
    }
    if k(Key::OpenBracket) || k(Key::CloseBracket) {
        timeline::jump_phase(ed, k(Key::CloseBracket));
    }
    if k(Key::N) {
        inspect::add_hitbox(ed);
    }
    if k(Key::K) {
        inspect::add_pose_key(ed);
    }
    if k(Key::Delete) || k(Key::Backspace) {
        inspect::delete_hitbox(ed);
    }
    if k(Key::Tab)
        && let Some(viewport::PlayheadBoxes { boxes: b, .. }) = viewport::playhead_boxes(ed)
        && !b.is_empty()
    {
        ed.hitbox = Some(ed.hitbox.map_or(0, |h| (h + if shift { b.len() - 1 } else { 1 }) % b.len()));
    }
    if k(Key::F) {
        ed.pane.reframe = true;
    }
    if k(Key::H) {
        ed.show_hurt = !ed.show_hurt;
    }
    if k(Key::I) {
        ed.show_inactive = !ed.show_inactive;
    }
    if k(Key::O) {
        ed.onion = !ed.onion;
    }
    if k(Key::L) {
        ed.looping = !ed.looping;
    }
}

fn stage_shortcuts(ed: &mut Editor, shift: bool, k: &impl Fn(Key) -> bool) {
    let st = &mut ed.st;
    if k(Key::G) {
        st.grid = !st.grid;
    }
    if k(Key::M) {
        st.symmetry = !st.symmetry;
    }
    if k(Key::B) {
        st.show_bounds = !st.show_bounds;
    }
    if k(Key::P) {
        st.show_spawns = !st.show_spawns;
    }
    if k(Key::F) {
        st.pane.reframe = true;
    }
    if k(Key::C) {
        st.set_sandbox((st.sandbox + 1) % (SANDBOX_MAX + 1));
    }
    if k(Key::Space) {
        if st.sandbox == 0 {
            st.set_sandbox(2);
            st.paused = false;
        } else {
            st.paused = !st.paused;
        }
    }
    if k(Key::Tab) {
        let s = &ed.ws.stages[&ed.st.cur].item;
        let mut parts: Vec<stage_view::Part> = (0..s.blocks.len()).map(stage_view::Part::Block).collect();
        parts.extend((0..s.platforms.len()).map(stage_view::Part::Platform));
        parts.extend((0..s.spawns.len()).map(stage_view::Part::Spawn));
        parts.extend((0..s.respawns.len()).map(stage_view::Part::Respawn));
        parts.extend([stage_view::Part::Ledge, stage_view::Part::Blast, stage_view::Part::Camera]);
        let n = parts.len();
        let i = ed
            .st
            .sel
            .and_then(|p| parts.iter().position(|q| *q == p))
            .map_or(0, |i| (i + if shift { n - 1 } else { 1 }) % n);
        ed.st.select(Some(parts[i]));
    }
    if k(Key::N) {
        stage_ed::add_platform(ed);
    }
    if k(Key::W) && ed.st.sel.and_then(stage_view::Part::platform).is_some() {
        stage_ed::add_waypoint(ed);
    }
    if k(Key::T) && ed.st.sandbox == 0 {
        ed.st.clock_play = !ed.st.clock_play;
    }
    if k(Key::Delete) || k(Key::Backspace) {
        stage_ed::delete_selected(ed);
    }
}

/// Fighters | Stages.
fn mode_switch(ui: &mut egui::Ui, ed: &mut Editor) {
    egui::Frame::new().fill(NIGHT).stroke(egui::Stroke::new(1., style::LINE)).corner_radius(7).inner_margin(2).show(
        ui,
        |ui| {
            ui.spacing_mut().item_spacing.x = 2.;
            for (m, label, tip) in
                [(Mode::Fighters, "Fighters", "Edit fighters  ⌘1"), (Mode::Stages, "Stages", "Edit stages  ⌘2")]
            {
                let on = ed.mode == m;
                let dirty = match m {
                    Mode::Fighters => ed.ws.docs.values().any(|d| d.dirty()),
                    Mode::Stages => ed.ws.stages.values().any(|d| d.dirty()),
                };
                let txt = RichText::new(if dirty { format!("{label} ●") } else { label.to_string() })
                    .family(style::label())
                    .size(15.)
                    .color(if on { NIGHT } else { DIM });
                let b = egui::Button::new(txt).fill(if on { GOLD } else { Color32::TRANSPARENT }).corner_radius(5);
                if ui.add(b).on_hover_text(tip).clicked() {
                    ed.set_mode(m);
                }
            }
        },
    );
}

fn top_bar(root: &mut egui::Ui, ed: &mut Editor, out: &mut Outbox) {
    let frame = egui::Frame::new().fill(NIGHT).inner_margin(egui::Margin::symmetric(12, 7));
    egui::Panel::top("ed-top").frame(frame).show(root, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("STICK SMASH").family(style::display()).size(20.).color(STAR));
            ui.label(RichText::new("EDITOR").family(style::display()).size(20.).color(GOLD));
            ui.add_space(14.);
            mode_switch(ui, ed);
            ui.add_space(4.);
            match ed.mode {
                Mode::Fighters => fighter_picker(ui, ed),
                Mode::Stages => stage_picker(ui, ed),
            }
            ui.separator();
            let (can_undo, can_redo, dirty) = match ed.mode {
                Mode::Fighters => {
                    let d = ed.doc();
                    (d.can_undo(), d.can_redo(), d.dirty())
                }
                Mode::Stages => {
                    let d = ed.stage_doc();
                    (d.can_undo(), d.can_redo(), d.dirty())
                }
            };
            let (kind, cur) = ed.current();
            if ui.add_enabled(can_undo, egui::Button::new("⟲")).on_hover_text("Undo  ⌘Z").clicked() {
                undo(ed, false);
            }
            if ui.add_enabled(can_redo, egui::Button::new("⟳")).on_hover_text("Redo  ⇧⌘Z").clicked() {
                undo(ed, true);
            }
            ui.separator();
            let target =
                ed.save_dir().map_or("saving disabled".into(), |d| d.join(kind.path(&cur)).display().to_string());
            let save = egui::Button::new(RichText::new(if dirty { "Save ●" } else { "Saved" }).color(if dirty {
                NIGHT
            } else {
                DIM
            }))
            .fill(if dirty { GOLD } else { INK2 });
            if ui
                .add_enabled(dirty, save)
                .on_hover_text(format!("⌘S → {target}"))
                .on_disabled_hover_text("No unsaved changes")
                .clicked()
            {
                ed.save();
            }
            let others = ed.ws.dirty().iter().filter(|(k, id)| (*k, id) != (kind, &cur)).count();
            if others > 0
                && ui
                    .button(format!("Save all ({})", others + usize::from(dirty)))
                    .on_hover_text(format!("⇧⌘S · {}", ed.unsaved().join(", ")))
                    .clicked()
            {
                ed.save_all();
            }
            let noun = kind.noun();
            if ui
                .add_enabled(dirty, egui::Button::new("Revert"))
                .on_hover_text(format!("Back to the saved {noun} (undoable)"))
                .clicked()
            {
                match ed.mode {
                    Mode::Fighters => {
                        ed.doc().revert();
                        ed.pane.stale = true;
                    }
                    Mode::Stages => {
                        ed.stage_doc().revert();
                        ed.st.pane.stale = true;
                    }
                }
                ed.say(format!("Reverted to the saved {noun} · ⌘Z to undo"), false);
            }
            if ed.config.dev {
                ui.checkbox(&mut ed.to_assets, "into assets/").on_hover_text("Dev: save into the built-in assets");
            }
            ui.add_space(6.);
            let errors = problems(ed);
            let test_btn = |t: &str| egui::Button::new(RichText::new(t).color(STAR)).fill(style::TEST);
            let (label, tip) = match ed.mode {
                Mode::Fighters => ("▶ Test", "Play training with these (unsaved) edits · ⌘↵\nPause → Back to editor"),
                Mode::Stages => {
                    ("▶ Training", "Training on this stage with these (unsaved) edits · ⌘↵\nPause → Back to editor")
                }
            };
            if ui
                .add_enabled(errors.is_empty(), test_btn(label))
                .on_hover_text(tip)
                .on_disabled_hover_text("Fix the problems first")
                .clicked()
            {
                test(ed, out, false);
            }
            if ed.mode == Mode::Stages
                && ui
                    .add_enabled(errors.is_empty(), test_btn("▶ Match"))
                    .on_hover_text("A 3-stock match against a CPU on this stage · ⇧⌘↵\nPause → Back to editor")
                    .on_disabled_hover_text("Fix the problems first")
                    .clicked()
            {
                test(ed, out, true);
            }
            ui.add_space(8.);
            match errors.first() {
                Some(e) => {
                    let n = errors.len();
                    let txt = RichText::new(format!("⚠ {n} problem{}", if n == 1 { "" } else { "s" })).color(RED);
                    if ui.link(txt).on_hover_text(format!("{}: {}\nClick to go there", e.path, e.msg)).clicked() {
                        match ed.mode {
                            Mode::Fighters => inspect::goto(ed, &e.path.clone()),
                            Mode::Stages => stage_panels::goto(ed, &e.path.clone()),
                        }
                    }
                }
                None => drop(
                    ui.label(RichText::new("✔ valid").color(style::GREEN))
                        .on_hover_text("Passes the game's content checks"),
                ),
            }
            if ed.mode == Mode::Stages {
                let notes = stage_ed::notes(ed);
                if let Some(e) = notes.first() {
                    let n = notes.len();
                    let txt = RichText::new(format!("ℹ {n} note{}", if n == 1 { "" } else { "s" })).color(GOLD);
                    let all = notes.iter().map(|e| format!("{}: {}", e.path, e.msg)).collect::<Vec<_>>().join("\n");
                    if ui.link(txt).on_hover_text(format!("Saves fine, but:\n{all}")).clicked() {
                        stage_panels::goto(ed, &e.path.clone());
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Back to menu").on_hover_text("Esc").clicked() {
                    ed.leave(false, out);
                }
                ui.toggle_value(&mut ed.help, RichText::new(" ? ").strong()).on_hover_text("Keyboard shortcuts  ?");
            });
        });
    });
}

fn fighter_picker(ui: &mut egui::Ui, ed: &mut Editor) {
    let ids = ed.ws.fighters();
    let label = |(_, name, dirty): &(String, String, bool)| if *dirty { format!("{name}  ●") } else { name.clone() };
    let cur = ids.iter().find(|x| x.0 == ed.cur).map(label).unwrap_or_default();
    let mut pick = None;
    let mut new = false;
    egui::ComboBox::from_id_salt("ed-fighter")
        .selected_text(RichText::new(cur).family(style::label()).size(16.).color(STAR))
        .width(170.)
        .show_ui(ui, |ui| {
            for x in &ids {
                if ui.selectable_label(x.0 == ed.cur, label(x)).on_hover_text(doc::fighter_path(&x.0)).clicked() {
                    pick = Some(x.0.clone());
                }
            }
            ui.separator();
            if ui.selectable_label(false, RichText::new("+ New fighter…").color(GOLD)).clicked() {
                new = true;
            }
        })
        .response
        .on_hover_text("Fighter being edited (● unsaved)");
    if let Some(id) = pick {
        ed.open(&id);
    }
    if new {
        let extends = ed.cur.clone();
        ed.dialog = Some(Dialog::NewFighter { id: String::new(), name: String::new(), extends, error: None });
    }
}

fn stage_picker(ui: &mut egui::Ui, ed: &mut Editor) {
    let list = ed.ws.stage_list();
    let label = |(_, name, dirty): &(String, String, bool)| if *dirty { format!("{name}  ●") } else { name.clone() };
    let cur = list.iter().find(|x| x.0 == ed.st.cur).map(label).unwrap_or_default();
    let mut pick = None;
    let mut dialog = None;
    let (id, name) = (ed.st.cur.clone(), ed.ws.stages.get(&ed.st.cur).map(|d| d.item.name.clone()).unwrap_or_default());
    egui::ComboBox::from_id_salt("ed-stage")
        .selected_text(RichText::new(cur).family(style::label()).size(16.).color(STAR))
        .width(170.)
        .show_ui(ui, |ui| {
            for x in &list {
                if ui.selectable_label(x.0 == ed.st.cur, label(x)).on_hover_text(doc::stage_path(&x.0)).clicked() {
                    pick = Some(x.0.clone());
                }
            }
            ui.separator();
            if ui.selectable_label(false, RichText::new("+ New stage…").color(GOLD)).clicked() {
                dialog =
                    Some(Dialog::NewStage { id: String::new(), name: String::new(), from: id.clone(), error: None });
            }
            if ui.selectable_label(false, format!("{} Duplicate {name}…", style::ICON_DUP)).clicked() {
                let n = format!("{name} copy");
                dialog = Some(Dialog::NewStage { id: doc::slug(&n), name: n, from: id.clone(), error: None });
            }
            if ui.selectable_label(false, format!("Rename {name}…")).clicked() {
                dialog =
                    Some(Dialog::RenameStage { id: id.clone(), new_id: id.clone(), name: name.clone(), error: None });
            }
        })
        .response
        .on_hover_text("Stage being edited (● unsaved)");
    if let Some(id) = pick {
        ed.open_stage(&id);
    }
    if dialog.is_some() {
        ed.dialog = dialog;
    }
}

fn dialog_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(style::INK)
        .stroke(egui::Stroke::new(1., style::LINE))
        .corner_radius(10)
        .inner_margin(18)
        .shadow(egui::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(160) })
}

/// An opaque, centered modal over a dimmed backdrop that swallows clicks. No fade-in, so it's readable at once.
fn modal(id: &'static str) -> egui::Modal {
    egui::Modal::new(id.into())
        .area(egui::Modal::default_area(id.into()).fade_in(false))
        .backdrop_color(style::BACKDROP)
        .frame(dialog_frame())
}

/// Toasts stack at the viewport's bottom right, above the hint line, clear of the fighter.
fn toasts(ctx: &egui::Context, ed: &mut Editor, dt: f32) {
    for t in &mut ed.toasts {
        t.left -= dt;
    }
    ed.toasts.retain(|t| t.left > 0.);
    if ed.toasts.is_empty() {
        return;
    }
    let [_, _, r, b] = ed.port.rect.map(|v| v as f32);
    egui::Area::new("ed-toasts".into())
        .order(egui::Order::Tooltip)
        .pivot(Align2::RIGHT_BOTTOM)
        .fixed_pos(egui::pos2(r - 12., b - 40.))
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_max_width(style::TOAST_W);
            for t in &ed.toasts {
                let a = t.left.clamp(0., 0.4) / 0.4;
                let (fill, fg, accent, icon) =
                    if t.error { (style::ERR_BG, style::ERR_TEXT, RED, "⚠") } else { (INK2, STAR, GOLD, "✔") };
                egui::Frame::new()
                    .fill(fill.gamma_multiply(a))
                    .stroke(egui::Stroke::new(1., accent.gamma_multiply(0.6 * a)))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon).color(accent.gamma_multiply(a)));
                            ui.add(egui::Label::new(RichText::new(&t.msg).color(fg.gamma_multiply(a))).wrap());
                        });
                    });
                ui.add_space(4.);
            }
        });
}

pub(crate) const SHORTCUTS: [(&str, &[(&str, &str)]); 4] = [
    (
        "Playback",
        &[
            ("Space", "play / pause"),
            ("← →   , .", "step a frame (⇧ ×10)"),
            ("Home  End", "first / last frame"),
            ("[  ]", "previous / next phase"),
            ("L", "loop playback"),
            ("↑ ↓", "previous / next move"),
        ],
    ),
    (
        "Editing",
        &[
            ("Click  Tab", "select hitbox (⇧Tab back)"),
            ("N", "new hitbox on this frame"),
            ("K", "pose key on this frame"),
            ("⌘D", "duplicate hitbox"),
            ("Del", "delete hitbox"),
            ("Alt ← → ↑ ↓", "nudge hitbox (⇧ ×10)"),
            ("drag + ⇧", "snap: whole px, 15°"),
            ("drag + Alt", "fine: no snapping"),
        ],
    ),
    (
        "View",
        &[
            ("Wheel  pinch", "zoom at cursor"),
            ("Right drag", "pan (or middle drag)"),
            ("F", "frame the fighter"),
            ("H  I  O", "hurtbox, inactive, onion skin"),
        ],
    ),
    (
        "File",
        &[
            ("⌘Z  ⇧⌘Z", "undo, redo"),
            ("⌘S  ⇧⌘S", "save, save all"),
            ("⌘↵", "test in training"),
            ("⌘F", "search the browser"),
            ("⌘1  ⌘2", "fighters, stages"),
            ("?", "this sheet"),
            ("Esc", "deselect, then leave"),
        ],
    ),
];

pub(crate) const STAGE_SHORTCUTS: [(&str, &[(&str, &str)]); 4] = [
    (
        "Editing",
        &[
            ("Click  Tab", "select a part (⇧Tab back)"),
            ("drag", "move; edges, corners, ends resize"),
            ("drag + ⇧", "snap to the grid"),
            ("drag + Alt", "fine: 0.1 px"),
            ("Alt ← → ↑ ↓", "nudge 1 px (⇧ ×10)"),
            ("N", "new platform"),
            ("W", "path stop on the selected platform"),
            ("⌘D  Del", "duplicate, delete"),
            ("click arrow", "flip a spawn's facing"),
        ],
    ),
    (
        "Layout aids",
        &[
            ("G", "grid (snaps by default)"),
            ("M", "mirror edits around the centre"),
            ("B", "blast zone and camera bounds"),
            ("P", "spawn and respawn points"),
        ],
    ),
    (
        "View & sandbox",
        &[
            ("Wheel  pinch", "zoom at cursor"),
            ("Right drag", "pan (or drag empty space)"),
            ("F", "frame the stage"),
            ("T", "play the moving platforms"),
            ("Space", "CPUs on / pause"),
            ("C", "cycle the CPU count"),
        ],
    ),
    (
        "File",
        &[
            ("⌘Z  ⇧⌘Z", "undo, redo"),
            ("⌘S  ⇧⌘S", "save, save all"),
            ("⌘↵  ⇧⌘↵", "test: training, match"),
            ("⌘1  ⌘2", "fighters, stages"),
            ("?", "this sheet"),
            ("Esc", "deselect, then leave"),
        ],
    ),
];

fn help(ctx: &egui::Context, ed: &mut Editor) {
    let sheet = if ed.mode == Mode::Stages { STAGE_SHORTCUTS } else { SHORTCUTS };
    let r = modal("ed-help").show(ctx, |ui| {
        // Four columns when they fit, else two (or one): never wider than the screen.
        let col = 250.;
        let fit = ((ctx.content_rect().width() - 120.) / (col + 22.)).floor().clamp(1., 4.);
        let cols = if fit >= 4. { 4. } else { fit.min(2.) };
        ui.set_width(cols * col + (cols - 1.) * 22.);
        ui.horizontal(|ui| {
            ui.label(style::heading("Keyboard shortcuts"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(style::ICON_DEL).on_hover_text("Close  Esc").clicked() {
                    ed.help = false;
                }
            });
        });
        ui.add_space(6.);
        egui::ScrollArea::vertical().max_height(ctx.content_rect().height() - 160.).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 22.;
                for (title, keys) in sheet {
                    ui.vertical(|ui| {
                        ui.set_width(col);
                        ui.label(style::caps(title));
                        egui::Grid::new(("ed-keys", title))
                            .num_columns(2)
                            .spacing([10., 3.])
                            .min_col_width(84.)
                            .max_col_width(col - 94.)
                            .show(ui, |ui| {
                                for (k, v) in keys {
                                    ui.label(key(k));
                                    ui.add(egui::Label::new(*v).wrap());
                                    ui.end_row();
                                }
                            });
                        ui.add_space(8.);
                    });
                }
            });
        });
        ui.label(faint(
            "⌘ is Ctrl on Windows and Linux, and Alt is Option on macOS. Hover any field for what it does.",
        ));
    });
    if r.backdrop_response.clicked() {
        ed.help = false;
    }
}

pub(crate) fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.22))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).family(style::label()).size(12.5).color(color));
        });
}

pub(crate) fn hint(ui: &mut egui::Ui, keys: &str, what: &str) {
    ui.label(key(keys));
    ui.label(RichText::new(what).color(FAINT));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_number_selects_either_mode_from_both_modes() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let mut ed =
            Editor::new(crate::EditorConfig { roots: vec![assets.clone()], assets, user_dir: None, dev: false })
                .unwrap();
        let ctx = egui::Context::default();
        let mut out = Outbox::default();
        for (from, key, to) in [
            (Mode::Fighters, Key::Num1, Mode::Fighters),
            (Mode::Fighters, Key::Num2, Mode::Stages),
            (Mode::Stages, Key::Num1, Mode::Fighters),
            (Mode::Stages, Key::Num2, Mode::Stages),
        ] {
            ed.set_mode(from);
            let modifiers = egui::Modifiers { command: true, mac_cmd: true, ..Default::default() };
            ctx.begin_pass(egui::RawInput {
                events: vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
                ],
                ..Default::default()
            });
            shortcuts(&ctx, &mut ed, &mut out);
            ctx.end_pass().textures_delta.clear();
            assert_eq!(ed.mode, to);
            ctx.begin_pass(egui::RawInput {
                events: vec![egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers }],
                ..Default::default()
            });
            ctx.end_pass().textures_delta.clear();
        }
    }
}
