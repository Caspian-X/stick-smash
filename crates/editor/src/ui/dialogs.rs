//! Modal content and actions.

use super::*;

#[derive(Clone, Copy)]
struct Keys {
    esc: bool,
    enter: bool,
}

fn leave(ui: &mut egui::Ui, ed: &mut Editor, d: &mut Dialog, keys: Keys, out: &mut Outbox) -> bool {
    let Dialog::Leave { quit } = d else { return false };
    let Keys { esc, enter } = keys;
    let mut close = false;
    let names = ed.unsaved();
    ui.label(style::heading("Unsaved changes"));
    ui.add_space(4.);
    ui.label(format!("{} {} unsaved edits.", names.join(", "), if names.len() == 1 { "has" } else { "have" }));
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

    close
}

fn new_fighter(ui: &mut egui::Ui, ed: &mut Editor, d: &mut Dialog, keys: Keys) -> bool {
    let Dialog::NewFighter { id, name, extends, error } = d else { return false };
    let Keys { esc, enter } = keys;
    let mut close = false;
    ui.label(style::heading("New fighter"));
    ui.label(faint("Starts as a copy of its parent; your edits save as overrides on top of it."));
    ui.add_space(6.);
    let packages = doc::package_ids(&ed.ws.files);
    egui::Grid::new("nf").num_columns(2).spacing([10., 6.]).show(ui, |ui| {
        ui.label(RichText::new("name").color(DIM));
        let r = ui.add(egui::TextEdit::singleline(name).hint_text("Ninja"));
        if r.changed() {
            *id = doc::slug(name);
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

    close
}

fn rename_pose(ui: &mut egui::Ui, ed: &mut Editor, d: &mut Dialog, keys: Keys) -> bool {
    let Dialog::RenamePose { old, new } = d else { return false };
    let Keys { esc, enter } = keys;
    let mut close = false;
    ui.label(style::heading(format!("Rename pose {old}")));
    let uses = doc::pose_uses(&ed.ws.docs[&ed.cur].item, old);
    ui.label(faint(format!(
        "Updates {uses} reference{} in moves, animations and poses.",
        if uses == 1 { "" } else { "s" }
    )));
    ui.add_space(6.);
    ui.add(egui::TextEdit::singleline(new)).request_focus_once(ui);
    let taken = new != old && ed.ws.docs[&ed.cur].item.poses.contains_key(new.as_str());
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
                ed.pane.stale = true;
                ed.say(format!("Renamed to {n}"), false);
            }
            close = true;
        }
        if ui.button("Cancel").clicked() || esc {
            close = true;
        }
    });

    close
}

fn copy_move(ui: &mut egui::Ui, ed: &mut Editor, d: &mut Dialog, keys: Keys) -> bool {
    let Dialog::CopyMove { from, to } = d else { return false };
    let Keys { esc, .. } = keys;
    let mut close = false;
    ui.label(style::heading(format!("Copy {from} to…")));
    ui.label(faint("Replaces the target move with a copy of this one (undoable)."));
    ui.add_space(6.);
    let f = &ed.ws.docs[&ed.cur].item;
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
        if ui.add_enabled(to.is_some(), egui::Button::new(RichText::new("Copy").color(NIGHT)).fill(GOLD)).clicked()
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

    close
}

fn new_stage(ui: &mut egui::Ui, ed: &mut Editor, d: &mut Dialog, keys: Keys) -> bool {
    let Dialog::NewStage { id, name, from, error } = d else { return false };
    let Keys { esc, enter } = keys;
    let mut close = false;
    let dup = !name.is_empty() && name.ends_with(" copy");
    ui.label(style::heading(if dup { "Duplicate stage" } else { "New stage" }));
    ui.label(faint("A copy of the template stage (with its unsaved edits) to reshape. Saved as a new package."));
    ui.add_space(6.);
    let list = ed.ws.stage_list();
    egui::Grid::new("ns").num_columns(2).spacing([10., 6.]).show(ui, |ui| {
        ui.label(RichText::new("name").color(DIM));
        let r = ui.add(egui::TextEdit::singleline(name).hint_text("Sky Fortress"));
        if r.changed() {
            *id = doc::slug(name);
        }
        r.request_focus_once(ui);
        ui.end_row();
        ui.label(RichText::new("id").color(DIM)).on_hover_text("Folder name: stages/<id>/");
        ui.add(egui::TextEdit::singleline(id).hint_text("sky-fortress"));
        ui.end_row();
        ui.label(RichText::new("template").color(DIM)).on_hover_text("The stage it starts as a copy of");
        let shown = list.iter().find(|x| x.0 == *from).map_or(from.clone(), |x| x.1.clone());
        egui::ComboBox::from_id_salt("ns-from").selected_text(shown).show_ui(ui, |ui| {
            for (sid, sname, _) in &list {
                ui.selectable_value(from, sid.clone(), sname);
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
            match ed.ws.new_stage(id, name, from) {
                Ok(()) => {
                    let id = id.clone();
                    ed.open_stage(&id);
                    ed.st.section = stage_ed::Section::Overview;
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

    close
}

fn rename_stage(ui: &mut egui::Ui, ed: &mut Editor, d: &mut Dialog, keys: Keys) -> bool {
    let Dialog::RenameStage { id, new_id, name, error } = d else { return false };
    let Keys { esc, enter } = keys;
    let mut close = false;
    ui.label(style::heading("Rename stage"));
    let fresh = ed.ws.stages.get(id.as_str()).is_some_and(|d| d.fresh);
    ui.label(faint(if fresh {
        "Not saved yet, so the id (its folder) can change too."
    } else {
        "The id is its folder and stays; duplicate the stage for a new one."
    }));
    ui.add_space(6.);
    egui::Grid::new("rs").num_columns(2).spacing([10., 6.]).show(ui, |ui| {
        ui.label(RichText::new("name").color(DIM));
        let r = ui.add(egui::TextEdit::singleline(name));
        if r.changed() && fresh {
            *new_id = doc::slug(name);
        }
        r.request_focus_once(ui);
        ui.end_row();
        ui.label(RichText::new("id").color(DIM));
        ui.add_enabled(fresh, egui::TextEdit::singleline(new_id));
        ui.end_row();
    });
    if let Some(e) = error {
        ui.label(RichText::new(e.as_str()).color(RED));
    }
    ui.add_space(8.);
    ui.horizontal(|ui| {
        let ok = !name.is_empty() && !new_id.is_empty();
        if ui.add_enabled(ok, egui::Button::new(RichText::new("Rename").color(NIGHT)).fill(GOLD)).clicked()
            || (enter && ok)
        {
            match ed.ws.rename_stage(id, new_id, name) {
                Ok(()) => {
                    let nid = new_id.clone();
                    ed.st.cur = nid.clone();
                    ed.st.pane.stale = true;
                    ed.st.world = None;
                    ed.say(format!("Renamed to {name} · ⌘Z to undo the name"), false);
                    close = true;
                }
                Err(e) => *error = Some(e),
            }
        }
        if ui.button("Cancel").clicked() || esc {
            close = true;
        }
    });

    close
}

pub(super) fn show(ctx: &egui::Context, ed: &mut Editor, out: &mut Outbox) {
    let Some(mut d) = ed.dialog.clone() else { return };
    let mut close = false;
    let keys = Keys { esc: ctx.input(|i| i.key_pressed(Key::Escape)), enter: ctx.input(|i| i.key_pressed(Key::Enter)) };
    modal("ed-dialog").show(ctx, |ui| {
        ui.set_width(380.);
        match &mut d {
            Dialog::Leave { .. } => close = leave(ui, ed, &mut d, keys, out),
            Dialog::NewFighter { .. } => close = new_fighter(ui, ed, &mut d, keys),
            Dialog::RenamePose { .. } => close = rename_pose(ui, ed, &mut d, keys),
            Dialog::CopyMove { .. } => close = copy_move(ui, ed, &mut d, keys),
            Dialog::NewStage { .. } => close = new_stage(ui, ed, &mut d, keys),
            Dialog::RenameStage { .. } => close = rename_stage(ui, ed, &mut d, keys),
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
