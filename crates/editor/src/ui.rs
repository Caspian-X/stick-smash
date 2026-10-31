//! The egui layer's frame: layout, top bar, keyboard shortcuts, dialogs, toasts and the shortcut sheet. Panels
//! live in `browser`, `viewport`, `timeline` and `inspect`.

use bevy_egui::egui::{self, Align2, Color32, Key, LayerId, RichText, UiBuilder};
use render::pose::{Skel, resolve};
use sim::data::visual::{Pose, PoseRef};
use sim::world::Fighter;

use crate::preview::Subject;
use crate::style::{self, DIM, FAINT, GOLD, INK2, NIGHT, RED, STAR, faint, key};
use crate::{Dialog, Editor, Outbox, Request, browser, doc, inspect, move_mut, move_ref, timeline, viewport};

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

/// Live problems of the current fighter plus the last refused save.
pub(crate) fn problems(ed: &Editor) -> Vec<sim::data::ContentError> {
    let mut list = ed.ws.docs.get(&ed.cur).map(|d| d.errors.clone()).unwrap_or_default();
    let extra: Vec<_> = ed.problems.iter().filter(|p| !list.contains(p)).cloned().collect();
    list.extend(extra);
    list
}

pub(crate) fn run(ctx: &egui::Context, ed: &mut Editor, scene: &mut render::Scene, dt: f32, out: &mut Outbox) {
    if ed.dialog.is_none() {
        shortcuts(ctx, ed, out);
    }
    let before = ed.doc().fighter.clone();
    let setup = ed.setup.clone();
    let mut root = egui::Ui::new(
        ctx.clone(),
        "editor".into(),
        UiBuilder::new().layer_id(LayerId::background()).max_rect(ctx.viewport_rect()),
    );
    top_bar(&mut root, ed, out);
    egui::Panel::left("ed-browser")
        .default_size(style::BROWSER_W)
        .size_range(style::BROWSER_MIN..=style::BROWSER_MAX)
        .show(&mut root, |ui| browser::browser(ui, ed));
    egui::Panel::right("ed-inspector")
        .default_size(style::INSPECTOR_W)
        .size_range(300.0..=560.)
        .show(&mut root, |ui| inspect::inspector(ui, ed));
    egui::Panel::bottom("ed-timeline").resizable(false).show(&mut root, |ui| timeline::timeline(ui, ed));
    egui::CentralPanel::no_frame().show(&mut root, |ui| viewport::viewport(ui, ed));
    if ed.help {
        help(ctx, ed);
    }
    dialogs(ctx, ed, out);
    toasts(ctx, ed, dt);
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
    viewport::sync_scene(ed, scene, dt);
    viewport::overlay(ctx, ed, scene);
}

pub(crate) fn toggle_play(ed: &mut Editor) {
    ed.playing = !ed.playing;
    if ed.playing && ed.frame + 1 >= ed.frames() {
        ed.frame = 0;
    }
}

pub(crate) fn undo(ed: &mut Editor, redo: bool) {
    let d = ed.doc();
    let can = if redo { d.can_redo() } else { d.can_undo() };
    if redo {
        d.redo()
    } else {
        d.undo()
    }
    ed.stale = true;
    if can {
        ed.say(if redo { "Redid the edit · ⌘Z undoes it" } else { "Undid the last edit · ⇧⌘Z redoes it" }, false);
    }
}

pub(crate) fn test(ed: &mut Editor, out: &mut Outbox) {
    if ed.ws.docs.get(&ed.cur).is_some_and(|d| d.errors.is_empty()) {
        out.request = Some(Request::Test { data: std::sync::Arc::new(ed.ws.live_data()), fighter: ed.cur.clone() });
    } else {
        ed.say("Fix the problems before testing", true);
    }
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
    // These work even while typing in a field.
    if cmd && k(Key::S) {
        if shift {
            ed.save_all()
        } else {
            ed.save()
        };
        return;
    }
    if cmd && k(Key::F) {
        ed.focus_search = true;
        return;
    }
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let n = ed.frames();
    if cmd && k(Key::Z) {
        undo(ed, shift);
        return;
    }
    if cmd && k(Key::Y) {
        undo(ed, true);
        return;
    }
    if cmd && k(Key::D) {
        inspect::duplicate_hitbox(ed);
        return;
    }
    if cmd && k(Key::Enter) {
        test(ed, out);
        return;
    }
    if cmd {
        return;
    }
    if alt && ed.hitbox.is_some() {
        // Nudge the selected hitbox (Shift: ×10).
        let s = if shift { 5. } else { 0.5 };
        let d = [
            (Key::ArrowLeft, [-s, 0.]),
            (Key::ArrowRight, [s, 0.]),
            (Key::ArrowUp, [0., s]),
            (Key::ArrowDown, [0., -s]),
        ];
        for (key, [dx, dy]) in d {
            if k(key) {
                inspect::nudge_hitbox(ed, dx, dy);
            }
        }
        return;
    }
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
        && let Some((_, _, _, b)) = viewport::playhead_boxes(ed)
        && !b.is_empty()
    {
        ed.hitbox = Some(ed.hitbox.map_or(0, |h| (h + if shift { b.len() - 1 } else { 1 }) % b.len()));
    }
    if k(Key::F) {
        ed.reframe = true;
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
    if k(Key::Slash) || k(Key::Questionmark) || k(Key::F1) {
        ed.help = !ed.help;
    }
    if k(Key::Escape) {
        if ed.help {
            ed.help = false;
        } else if ed.hitbox.is_some() {
            ed.hitbox = None;
        } else {
            ed.leave(false, out);
        }
    }
}

fn top_bar(root: &mut egui::Ui, ed: &mut Editor, out: &mut Outbox) {
    let frame = egui::Frame::new().fill(NIGHT).inner_margin(egui::Margin::symmetric(12, 7));
    egui::Panel::top("ed-top").frame(frame).show(root, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("STICK SMASH").family(style::display()).size(20.).color(STAR));
            ui.label(RichText::new("EDITOR").family(style::display()).size(20.).color(GOLD));
            ui.add_space(14.);
            fighter_picker(ui, ed);
            ui.separator();
            let (can_undo, can_redo, dirty) = {
                let d = ed.doc();
                (d.can_undo(), d.can_redo(), d.dirty())
            };
            if ui.add_enabled(can_undo, egui::Button::new("⟲")).on_hover_text("Undo  ⌘Z").clicked() {
                undo(ed, false);
            }
            if ui.add_enabled(can_redo, egui::Button::new("⟳")).on_hover_text("Redo  ⇧⌘Z").clicked() {
                undo(ed, true);
            }
            ui.separator();
            let target = if ed.to_assets { Some(ed.config.assets.clone()) } else { ed.config.user_dir.clone() }
                .map_or("saving disabled".into(), |d| d.join(doc::fighter_path(&ed.cur)).display().to_string());
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
            let others = ed.ws.docs.values().filter(|d| d.dirty() && d.id != ed.cur).count();
            if others > 0
                && ui
                    .button(format!("Save all ({})", others + usize::from(dirty)))
                    .on_hover_text(format!("⇧⌘S · {}", ed.unsaved().join(", ")))
                    .clicked()
            {
                ed.save_all();
            }
            if ui
                .add_enabled(dirty, egui::Button::new("Revert"))
                .on_hover_text("Back to the saved fighter (undoable)")
                .clicked()
            {
                ed.doc().revert();
                ed.stale = true;
                ed.say("Reverted to the saved fighter · ⌘Z to undo", false);
            }
            if ed.config.dev {
                ui.checkbox(&mut ed.to_assets, "into assets/").on_hover_text("Dev: save into the built-in assets");
            }
            ui.add_space(6.);
            let errors = problems(ed);
            let test_btn = egui::Button::new(RichText::new("▶ Test").color(STAR)).fill(style::TEST);
            if ui
                .add_enabled(errors.is_empty(), test_btn)
                .on_hover_text("Play training with these (unsaved) edits · ⌘↵\nPause → Back to editor")
                .on_disabled_hover_text("Fix the problems first")
                .clicked()
            {
                test(ed, out);
            }
            ui.add_space(8.);
            match errors.first() {
                Some(e) => {
                    let n = errors.len();
                    let txt = RichText::new(format!("⚠ {n} problem{}", if n == 1 { "" } else { "s" })).color(RED);
                    if ui.link(txt).on_hover_text(format!("{}: {}\nClick to go there", e.path, e.msg)).clicked() {
                        inspect::goto(ed, &e.path.clone());
                    }
                }
                None => drop(
                    ui.label(RichText::new("✔ valid").color(style::GREEN))
                        .on_hover_text("Passes the game's content checks"),
                ),
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

fn dialogs(ctx: &egui::Context, ed: &mut Editor, out: &mut Outbox) {
    let Some(mut d) = ed.dialog.clone() else { return };
    let mut close = false;
    let esc = ctx.input(|i| i.key_pressed(Key::Escape));
    let enter = ctx.input(|i| i.key_pressed(Key::Enter));
    modal("ed-dialog").show(ctx, |ui| {
        ui.set_width(380.);
        match &mut d {
            Dialog::Leave { quit } => {
                let names = ed.unsaved();
                ui.label(style::heading("Unsaved changes"));
                ui.add_space(4.);
                ui.label(format!(
                    "{} {} unsaved edits.",
                    names.join(", "),
                    if names.len() == 1 { "has" } else { "have" }
                ));
                ui.label(faint(if *quit {
                    "Quitting discards them."
                } else {
                    "They stay open in the editor until you quit, but matches play the saved versions."
                }));
                ui.add_space(10.);
                ui.horizontal(|ui| {
                    let save = egui::Button::new(RichText::new("Save all").color(NIGHT)).fill(GOLD);
                    if ui.add(save).clicked() || enter {
                        if ed.save_all() {
                            out.request = Some(if *quit { Request::Quit } else { Request::Back });
                        }
                        close = true;
                    }
                    let go = if *quit { "Quit without saving" } else { "Leave (keep edits open)" };
                    if ui.button(go).clicked() {
                        out.request = Some(if *quit { Request::Quit } else { Request::Back });
                        close = true;
                    }
                    if ui.button("Cancel").clicked() || esc {
                        close = true;
                    }
                });
            }
            Dialog::NewFighter { id, name, extends, error } => {
                ui.label(style::heading("New fighter"));
                ui.label(faint("Starts as a copy of its parent; your edits save as overrides on top of it."));
                ui.add_space(6.);
                let packages = doc::package_ids(&ed.ws.files);
                egui::Grid::new("nf").num_columns(2).spacing([10., 6.]).show(ui, |ui| {
                    ui.label(RichText::new("name").color(DIM));
                    let r = ui.add(egui::TextEdit::singleline(name).hint_text("Ninja"));
                    if r.changed() {
                        *id = name
                            .to_lowercase()
                            .chars()
                            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
                            .collect::<String>()
                            .replace(' ', "-");
                    }
                    r.request_focus_once(ui);
                    ui.end_row();
                    ui.label(RichText::new("id").color(DIM)).on_hover_text("Folder name: fighters/<id>/");
                    ui.add(egui::TextEdit::singleline(id).hint_text("ninja"));
                    ui.end_row();
                    ui.label(RichText::new("extends").color(DIM))
                        .on_hover_text("Everything not overridden comes from this fighter");
                    egui::ComboBox::from_id_salt("nf-ext").selected_text(extends.as_str()).show_ui(ui, |ui| {
                        for p in &packages {
                            ui.selectable_value(extends, p.clone(), p);
                        }
                    });
                    ui.end_row();
                });
                if let Some(e) = error {
                    ui.label(RichText::new(e.as_str()).color(RED));
                }
                ui.add_space(8.);
                ui.horizontal(|ui| {
                    let create = egui::Button::new(RichText::new("Create").color(NIGHT)).fill(GOLD);
                    if ui.add_enabled(!id.is_empty(), create).clicked() || (enter && !id.is_empty()) {
                        match ed.ws.new_fighter(id, name, extends) {
                            Ok(()) => {
                                let id = id.clone();
                                ed.open(&id);
                                ed.select(Subject::Fighter);
                                ed.say(format!("Created {id} (unsaved) · ⌘S to write it"), false);
                                close = true;
                            }
                            Err(e) => *error = Some(e),
                        }
                    }
                    if ui.button("Cancel").clicked() || esc {
                        close = true;
                    }
                });
            }
            Dialog::RenamePose { old, new } => {
                ui.label(style::heading(format!("Rename pose {old}")));
                let uses = doc::pose_uses(&ed.ws.docs[&ed.cur].fighter, old);
                ui.label(faint(format!(
                    "Updates {uses} reference{} in moves, animations and poses.",
                    if uses == 1 { "" } else { "s" }
                )));
                ui.add_space(6.);
                ui.add(egui::TextEdit::singleline(new)).request_focus_once(ui);
                let taken = new != old && ed.ws.docs[&ed.cur].fighter.poses.contains_key(new.as_str());
                if taken {
                    ui.label(RichText::new("That name is taken").color(RED));
                }
                ui.add_space(8.);
                ui.horizontal(|ui| {
                    let ok = !new.is_empty() && !taken;
                    if ui.add_enabled(ok, egui::Button::new(RichText::new("Rename").color(NIGHT)).fill(GOLD)).clicked()
                        || (enter && ok)
                    {
                        let (o, n) = (old.clone(), new.clone());
                        if o != n {
                            ed.doc().edit(|f| {
                                doc::rename_pose(f, &o, &n);
                            });
                            if ed.subject == Subject::Pose(o) {
                                ed.subject = Subject::Pose(n.clone());
                            }
                            ed.stale = true;
                            ed.say(format!("Renamed to {n}"), false);
                        }
                        close = true;
                    }
                    if ui.button("Cancel").clicked() || esc {
                        close = true;
                    }
                });
            }
            Dialog::CopyMove { from, to } => {
                ui.label(style::heading(format!("Copy {from} to…")));
                ui.label(faint("Replaces the target move with a copy of this one (undoable)."));
                ui.add_space(6.);
                let f = &ed.ws.docs[&ed.cur].fighter;
                ui.horizontal_wrapped(|ui| {
                    for slot in crate::SLOTS.iter().filter(|s| **s != from.as_str()) {
                        let exists = move_ref(&f.moves, slot).is_some();
                        let txt = if exists { slot.to_string() } else { format!("{slot} (new)") };
                        if ui.selectable_label(to.as_deref() == Some(slot), txt).clicked() {
                            *to = Some(slot.to_string());
                        }
                    }
                });
                ui.add_space(8.);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(to.is_some(), egui::Button::new(RichText::new("Copy").color(NIGHT)).fill(GOLD))
                        .clicked()
                        && let Some(t) = to.clone()
                    {
                        let src = from.clone();
                        ed.doc().edit(|f| {
                            doc::copy_move(f, &src, &t);
                        });
                        ed.select(Subject::Move(t.clone()));
                        ed.say(format!("Copied {src} → {t} · ⌘Z to undo"), false);
                        close = true;
                    }
                    if ui.button("Cancel").clicked() || esc {
                        close = true;
                    }
                });
            }
        }
    });
    ed.dialog = if close { None } else { ed.dialog.as_ref().map(|_| d) };
}

trait FocusOnce {
    fn request_focus_once(&self, ui: &egui::Ui);
}
impl FocusOnce for egui::Response {
    /// Focuses the field when its dialog opens.
    fn request_focus_once(&self, ui: &egui::Ui) {
        let id = self.id.with("focused");
        if !ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false)) {
            self.request_focus();
            ui.data_mut(|d| d.insert_temp(id, true));
        }
    }
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
            ("?", "this sheet"),
            ("Esc", "deselect, then leave"),
        ],
    ),
];

fn help(ctx: &egui::Context, ed: &mut Editor) {
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
                for (title, keys) in SHORTCUTS {
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
