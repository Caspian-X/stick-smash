//! The browser: searchable list of the fighter's moves, throws, poses and fighter-wide sections, with dirty and
//! problem markers and context menus (copy a move, rename / duplicate / delete a pose).

use bevy_egui::egui::{self, FontId, Key, RichText};

use crate::preview::{Section, Subject};
use crate::style::{self, DIM, FAINT, GOLD, RED, STAR, caps};
use crate::ui::fighter;
use crate::{Dialog, Editor, doc, move_ref};

const GROUPS: [(&str, &[&str]); 5] = [
    ("Ground", &["jab", "ftilt", "utilt", "dtilt", "dashatk"]),
    ("Smash", &["fsmash", "usmash", "dsmash"]),
    ("Air", &["nair", "fair", "bair", "uair", "dair"]),
    ("Special", &["nspec", "sspec", "uspec", "dspec"]),
    ("Grab", &["grab", "dashgrab"]),
];
const THROWS: [(&str, &str); 4] = [("f", "Forward"), ("b", "Back"), ("u", "Up"), ("d", "Down")];

fn matches(q: &str, texts: &[&str]) -> bool {
    q.is_empty() || texts.iter().any(|t| t.to_lowercase().contains(q))
}

/// Subjects shown with the current search, in order (for ↑ / ↓).
pub fn visible(ed: &Editor) -> Vec<Subject> {
    let q = ed.search.trim().to_lowercase();
    let Some(d) = ed.ws.docs.get(&ed.cur) else { return vec![] };
    let f = &d.item;
    let mut v = vec![];
    for (_, slots) in GROUPS {
        for &slot in slots {
            if let Some(m) = move_ref(&f.moves, slot)
                && matches(&q, &[slot, &m.name])
            {
                v.push(Subject::Move(slot.into()));
            }
        }
    }
    v.extend(
        THROWS
            .iter()
            .filter(|(k, n)| matches(&q, &[&format!("throw {k}"), n]))
            .map(|(k, _)| Subject::Throw(k.to_string())),
    );
    if matches(&q, &["stats", "body", "hurtbox", "fighter"]) {
        v.push(Subject::Fighter);
    }
    v.extend(Section::ALL.into_iter().filter(|s| matches(&q, &[s.name(), s.key()])).map(Subject::Data));
    v.extend(f.poses.keys().filter(|n| matches(&q, &[n])).map(|n| Subject::Pose(n.clone())));
    v
}

/// A list row: key in the label face, name dimmer, markers after. Truncated to the panel width (full text on hover)
/// so long names never widen the panel.
pub(crate) fn item(ui: &mut egui::Ui, on: bool, key: &str, name: &str, dirty: bool, bad: bool) -> egui::Response {
    let w = ui.available_width() - 2. * ui.spacing().button_padding.x;
    let mut t = egui::text::LayoutJob { wrap: egui::text::TextWrapping::truncate_at_width(w), ..Default::default() };
    let c = if on { GOLD } else { STAR };
    t.append(key, 0., egui::TextFormat::simple(FontId::new(15., style::label()), c));
    t.append(&format!("  {name}"), 0., egui::TextFormat::simple(style::text(12.), if on { GOLD } else { DIM }));
    if bad {
        t.append("  ⚠", 0., egui::TextFormat::simple(style::text(12.), RED));
    }
    if dirty {
        t.append("  ●", 0., egui::TextFormat::simple(style::text(9.), GOLD));
    }
    let full = if name.is_empty() { key.to_string() } else { format!("{key}  {name}") };
    let r = ui.add(egui::Button::selectable(on, t).min_size(egui::vec2(0., 21.))).on_hover_text(full);
    if on && ui.ctx().input(|i| i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowDown)) {
        r.scroll_to_me(None);
    }
    r
}

pub(crate) fn group_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.);
    ui.label(caps(text));
}

pub fn browser(ui: &mut egui::Ui, ed: &mut Editor) {
    ui.add_space(6.);
    let id = egui::Id::new("ed-search");
    // Right to left: the clear button takes its size first and the field gets exactly the rest, so the row never
    // pushes the panel wider.
    let row = egui::Layout::right_to_left(egui::Align::Center);
    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), ui.spacing().interact_size.y), row, |ui| {
        if !ed.search.is_empty() && ui.small_button(style::ICON_DEL).on_hover_text("Clear the search  Esc").clicked() {
            ed.search.clear();
        }
        let r = ui.add(
            egui::TextEdit::singleline(&mut ed.search)
                .id(id)
                .hint_text("🔍 Search   ⌘F")
                .desired_width(ui.available_width()),
        );
        if ed.focus_search {
            r.request_focus();
            ed.focus_search = false;
        }
        if r.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
            ed.search.clear();
            r.surrender_focus();
        }
        if r.lost_focus()
            && ui.input(|i| i.key_pressed(Key::Enter))
            && let Some(s) = visible(ed).into_iter().next()
        {
            ed.select(s);
        }
    });
    let shown = visible(ed);
    let mut pick: Option<Subject> = None;
    let mut action: Option<Action> = None;
    let errors: Vec<String> =
        ed.ws.docs.get(&ed.cur).map(|d| d.errors.iter().map(|e| e.path.clone()).collect()).unwrap_or_default();
    let bad = |prefix: &str| {
        errors
            .iter()
            .any(|p| p == prefix || p.starts_with(&format!("{prefix}.")) || p.starts_with(&format!("{prefix}[")))
    };
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            let doc = ed.ws.docs.get(&ed.cur).expect("open");
            let (f, saved) = (&doc.item, &doc.saved);
            let searching = !ed.search.trim().is_empty();
            for (group, slots) in GROUPS {
                let any = slots.iter().any(|s| shown.contains(&Subject::Move(s.to_string())));
                let add_sspec = !searching && group == "Special" && f.moves.sspec.is_none();
                if !any && !add_sspec {
                    continue;
                }
                group_label(ui, group);
                for &slot in slots {
                    let subj = Subject::Move(slot.into());
                    match move_ref(&f.moves, slot) {
                        Some(m) if shown.contains(&subj) => {
                            let dirty = move_ref(&saved.moves, slot) != Some(m);
                            let r = item(ui, ed.subject == subj, slot, &m.name, dirty, bad(&format!("moves.{slot}")));
                            if r.clicked() {
                                pick = Some(subj);
                            }
                            r.context_menu(|ui| {
                                if ui.button("Copy to slot…").clicked() {
                                    action = Some(Action::Copy(slot));
                                }
                                if ui.add_enabled(dirty, egui::Button::new("Revert to saved")).clicked() {
                                    action = Some(Action::RevertMove(slot));
                                }
                                if slot == "sspec" && ui.button("Remove side special").clicked() {
                                    action = Some(Action::RemoveSspec);
                                }
                            });
                        }
                        None if add_sspec
                            && ui
                                .add(
                                    egui::Button::new(RichText::new("+ side special").color(FAINT).size(12.5))
                                        .frame(false),
                                )
                                .on_hover_text("Add a side special, starting from a copy of nspec")
                                .clicked() =>
                        {
                            action = Some(Action::AddSspec);
                        }
                        _ => {}
                    }
                }
            }
            if THROWS.iter().any(|(k, _)| shown.contains(&Subject::Throw(k.to_string()))) {
                group_label(ui, "Throws");
            }
            for (k, name) in THROWS {
                let subj = Subject::Throw(k.into());
                if shown.contains(&subj) {
                    let dirty =
                        crate::throw_mut(&mut saved.throws.clone(), k) != crate::throw_mut(&mut f.throws.clone(), k);
                    if item(ui, ed.subject == subj, &format!("throw.{k}"), name, dirty, bad(&format!("throws.{k}")))
                        .clicked()
                    {
                        pick = Some(subj);
                    }
                }
            }
            if shown.iter().any(|s| s.idle()) {
                group_label(ui, "Fighter");
            }
            if shown.contains(&Subject::Fighter) {
                let dirty = f.stats != saved.stats
                    || f.body != saved.body
                    || f.hurtboxes != saved.hurtboxes
                    || f.name != saved.name
                    || f.blurb != saved.blurb
                    || f.tagline != saved.tagline;
                let b = bad("stats") || bad("body") || bad("hurtboxes");
                if item(ui, ed.subject == Subject::Fighter, "stats", "body · hurtboxes", dirty, b).clicked() {
                    pick = Some(Subject::Fighter);
                }
            }
            for s in Section::ALL {
                if shown.contains(&Subject::Data(s)) {
                    let (a, b) = (section_value(f, s), section_value(saved, s));
                    if item(ui, ed.subject == Subject::Data(s), s.name(), &section_count(f, s), a != b, bad(s.key()))
                        .clicked()
                    {
                        pick = Some(Subject::Data(s));
                    }
                }
            }
            let poses: Vec<&String> =
                f.poses.keys().filter(|n| shown.contains(&Subject::Pose(n.to_string()))).collect();
            if !poses.is_empty() || !searching {
                ui.add_space(6.);
                ui.horizontal(|ui| {
                    ui.label(caps(&format!("Poses · {}", f.poses.len())));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("+ New").on_hover_text("A new pose, starting from the one shown").clicked() {
                            action = Some(Action::NewPose);
                        }
                    });
                });
                for name in poses {
                    let p = &f.poses[name];
                    let dirty = saved.poses.get(name) != Some(p);
                    let subj = Subject::Pose(name.clone());
                    let uses = doc::pose_users(f, name);
                    let r = item(
                        ui,
                        ed.subject == subj,
                        name,
                        &if uses > 0 { format!("×{uses}") } else { String::new() },
                        dirty,
                        bad(&format!("poses.{name}")),
                    );
                    if r.clicked() {
                        pick = Some(subj);
                    }
                    r.context_menu(|ui| {
                        if ui.add_enabled(name != "stand", egui::Button::new("Rename…")).clicked() {
                            action = Some(Action::RenamePose(name.clone()));
                        }
                        if ui.button("Duplicate").clicked() {
                            action = Some(Action::DuplicatePose(name.clone()));
                        }
                        let del = ui.add_enabled(uses == 0, egui::Button::new("Delete"));
                        if del.on_disabled_hover_text(doc::uses_text(name, uses)).clicked() {
                            action = Some(Action::DeletePose(name.clone()));
                        }
                    });
                }
            }
            if searching && shown.is_empty() {
                ui.add_space(10.);
                ui.label(RichText::new("Nothing matches").color(FAINT));
            }
            ui.add_space(8.);
        });
    });
    if let Some(a) = action {
        act(ed, a);
    }
    if let Some(s) = pick {
        ed.select(s);
    }
}

fn section_value(f: &sim::data::Fighter, s: Section) -> serde_json::Value {
    let v = match s {
        Section::Style => serde_json::to_value(&f.style),
        Section::Attachments => serde_json::to_value(&f.attachments),
        Section::Projectiles => serde_json::to_value(&f.projectiles),
        Section::Effects => serde_json::to_value(&f.fx),
        Section::Anims => serde_json::to_value(&f.anims),
        Section::Ai => serde_json::to_value(&f.ai),
    };
    v.unwrap_or_default()
}

fn section_count(f: &sim::data::Fighter, s: Section) -> String {
    match s {
        Section::Attachments => f.attachments.len().to_string(),
        Section::Projectiles => f.projectiles.len().to_string(),
        Section::Anims => f.anims.len().to_string(),
        _ => String::new(),
    }
}

enum Action {
    Copy(&'static str),
    RevertMove(&'static str),
    AddSspec,
    RemoveSspec,
    NewPose,
    RenamePose(String),
    DuplicatePose(String),
    DeletePose(String),
}

fn act(ed: &mut Editor, a: Action) {
    match a {
        Action::Copy(slot) => ed.dialog = Some(Dialog::CopyMove { from: slot.into(), to: None }),
        Action::RevertMove(slot) => {
            let saved = ed.doc().saved.clone();
            ed.doc().edit(|f| {
                if slot == "sspec" {
                    f.moves.sspec = saved.moves.sspec.clone();
                } else if let (Some(m), Some(s)) = (crate::move_mut(&mut f.moves, slot), move_ref(&saved.moves, slot)) {
                    *m = s.clone();
                }
            });
            ed.stale = true;
            ed.say(format!("Reverted {slot} · ⌘Z to undo"), false);
        }
        Action::AddSspec => {
            ed.doc().edit(|f| {
                let mut m = f.moves.nspec.clone();
                m.name = format!("{} (side)", m.name);
                f.moves.sspec = Some(m);
            });
            ed.select(Subject::Move("sspec".into()));
        }
        Action::RemoveSspec => {
            ed.doc().edit(|f| f.moves.sspec = None);
            ed.select(Subject::Move("nspec".into()));
            ed.say("Removed the side special · ⌘Z to undo", false);
        }
        Action::NewPose => {
            let shown = crate::ui::pose_target(ed).and_then(|(t, _)| match t {
                crate::ui::PoseTarget::Named(n) => fighter(ed).poses.get(&n).cloned(),
                t => crate::ui::pose_mut(fighter(ed), &t).map(|p| p.clone()),
            });
            let mut name = String::new();
            ed.doc().edit(|f| name = doc::new_pose(f, "pose", shown.as_ref()));
            ed.select(Subject::Pose(name.clone()));
            ed.dialog = Some(Dialog::RenamePose { old: name.clone(), new: name });
        }
        Action::RenamePose(n) => ed.dialog = Some(Dialog::RenamePose { old: n.clone(), new: n }),
        Action::DuplicatePose(n) => {
            let p = fighter(ed).poses.get(&n).cloned();
            let mut name = String::new();
            ed.doc().edit(|f| name = doc::new_pose(f, &format!("{n}_copy"), p.as_ref()));
            ed.select(Subject::Pose(name));
        }
        Action::DeletePose(n) => {
            ed.doc().edit(|f| drop(f.poses.remove(&n)));
            if ed.subject == Subject::Pose(n.clone()) {
                ed.select(Subject::Fighter);
            }
            ed.say(format!("Deleted pose {n} · ⌘Z to undo"), false);
        }
    }
}
