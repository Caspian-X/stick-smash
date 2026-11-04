//! The inspector: structured fields for the selection (hitbox, phase, move, throw, pose, fighter, and schema forms
//! for the fighter-wide sections), with problems shown next to the fields they're about. Widgets edit the fighter
//! in place; `ui::run` turns changes into undo steps.

use std::sync::Arc;

use bevy_egui::egui::{self, DragValue, RichText};
use sim::GameData;
use sim::data::hitbox::{HitType, Hitbox, LaunchDir};
use sim::data::moves::{
    AirControl, Button, Charge, Cond, Counter, END, Event, HELPLESS, LANDING, Move, Phase, Transition, Trigger, Vel,
};
use sim::data::visual::{AnimKey, Pose, PoseRef};

use sim::data::ContentError;

use crate::preview::{Section, Subject};
use crate::style::{self, DIM, FAINT, GOLD, RED, STAR, caps, faint, heading};
use crate::ui::at_playhead;
use crate::{Dialog, Editor, doc, form, throw_mut};

const TRIGGERS: [(Trigger, &str); 17] = [
    (Trigger::Timeout, "timeout"),
    (Trigger::Land, "land"),
    (Trigger::Grounded, "grounded"),
    (Trigger::Airborne, "airborne"),
    (Trigger::Falling, "falling"),
    (Trigger::Release, "release"),
    (Trigger::ReleaseToward, "releaseToward"),
    (Trigger::ReleaseAway, "releaseAway"),
    (Trigger::AttackHeld, "attackHeld"),
    (Trigger::AttackPress, "attackPress"),
    (Trigger::SpecialPress, "specialPress"),
    (Trigger::UpHeld, "upHeld"),
    (Trigger::TargetNear, "targetNear"),
    (Trigger::ShieldPress, "shieldPress"),
    (Trigger::ArticleRelease, "articleRelease"),
    (Trigger::Counter, "counter"),
    (Trigger::Hit, "hit"),
];
const CONDS: [(Option<Cond>, &str); 4] = [
    (None, "always"),
    (Some(Cond::Grounded), "grounded"),
    (Some(Cond::Airborne), "airborne"),
    (Some(Cond::Countered), "countered"),
];
const KINDS: [(HitType, &str); 10] = [
    (HitType::Normal, "normal"),
    (HitType::Spike, "spike"),
    (HitType::Trip, "trip"),
    (HitType::Sakurai, "sakurai"),
    (HitType::Autolink, "autolink"),
    (HitType::Windbox, "windbox"),
    (HitType::Electric, "electric"),
    (HitType::Paralyze, "paralyze"),
    (HitType::Bury, "bury"),
    (HitType::Grab, "grab"),
];
const DIRS: [(LaunchDir, &str); 3] =
    [(LaunchDir::Forward, "forward"), (LaunchDir::Away, "away"), (LaunchDir::Back, "back")];
const AIRS: [(AirControl, &str); 3] =
    [(AirControl::Attack, "attack"), (AirControl::Aerial, "aerial"), (AirControl::UpSpecial, "upSpecial")];

pub(crate) fn combo<T: PartialEq + Clone>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    v: &mut T,
    opts: &[(T, &str)],
) {
    let cur = opts.iter().find(|(o, _)| o == v).map_or("?", |(_, l)| l);
    egui::ComboBox::from_id_salt(id).selected_text(cur).width(110.).show_ui(ui, |ui| {
        for (o, l) in opts {
            ui.selectable_value(v, o.clone(), *l);
        }
    });
}
pub(crate) fn row(ui: &mut egui::Ui, label: &str, tip: &str, add: impl FnOnce(&mut egui::Ui)) {
    let r = ui.add(egui::Label::new(RichText::new(label).color(DIM)).truncate());
    if !tip.is_empty() {
        r.on_hover_text(tip);
    }
    add(ui);
    ui.end_row();
}
/// A numeric field at the inspector's standard width.
pub(crate) fn drag(ui: &mut egui::Ui, v: DragValue) -> egui::Response {
    ui.add_sized([style::NUM_W, ui.spacing().interact_size.y], v)
}
pub(crate) fn num(ui: &mut egui::Ui, label: &str, tip: &str, v: &mut f64, speed: f64) {
    row(ui, label, tip, |ui| drop(drag(ui, DragValue::new(v).speed(speed).max_decimals(3))));
}
pub(crate) fn int(ui: &mut egui::Ui, label: &str, tip: &str, v: &mut u32) {
    row(ui, label, tip, |ui| drop(drag(ui, DragValue::new(v).speed(0.2))));
}
/// A small frameless icon or text button, always with a tooltip.
pub(crate) fn mini(ui: &mut egui::Ui, text: &str, color: egui::Color32, tip: &str) -> bool {
    ui.add(egui::Button::new(RichText::new(text).color(color).size(12.)).frame(false)).on_hover_text(tip).clicked()
}
/// An optional value: when set, its editor and a ✖ to clear it; when unset, a dim "— `unset`" and "+ set".
pub(crate) fn optional<T>(
    ui: &mut egui::Ui,
    v: &mut Option<T>,
    unset: &str,
    default: impl FnOnce() -> T,
    edit: impl FnOnce(&mut egui::Ui, &mut T),
) {
    optional_with(ui, v, unset, || Some(default()), edit, Optional::default());
}

pub(crate) struct Optional<'a> {
    pub inline: bool,
    pub add: &'a str,
    pub remove: String,
    pub add_tip: &'a str,
    pub remove_tip: String,
}

impl Default for Optional<'_> {
    fn default() -> Self {
        Self {
            inline: true,
            add: "+ set",
            remove: style::ICON_DEL.into(),
            add_tip: "Give it its own value",
            remove_tip: String::new(),
        }
    }
}

pub(crate) fn optional_with<T>(
    ui: &mut egui::Ui,
    v: &mut Option<T>,
    unset: &str,
    default: impl FnOnce() -> Option<T>,
    edit: impl FnOnce(&mut egui::Ui, &mut T),
    buttons: Optional<'_>,
) {
    match v {
        Some(x) => {
            let content = |ui: &mut egui::Ui| {
                edit(ui, x);
                if buttons.inline {
                    let tip =
                        if buttons.remove_tip.is_empty() { format!("Clear ({unset})") } else { buttons.remove_tip };
                    mini(ui, &buttons.remove, FAINT, &tip)
                } else {
                    ui.small_button(&buttons.remove).clicked()
                }
            };
            let clear = if buttons.inline { ui.horizontal(content).inner } else { content(ui) };
            if clear {
                *v = None;
            }
        }
        None => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("— {unset}")).color(FAINT));
                if mini(ui, buttons.add, GOLD, buttons.add_tip) {
                    *v = default();
                }
            });
        }
    }
}

pub(crate) fn opt_num(ui: &mut egui::Ui, label: &str, tip: &str, v: &mut Option<f64>, default: f64, unset: &str) {
    row(ui, label, tip, |ui| {
        optional(ui, v, unset, || default, |ui, x| drop(drag(ui, DragValue::new(x).speed(0.1).max_decimals(3))));
    });
}
fn opt_int(ui: &mut egui::Ui, label: &str, tip: &str, v: &mut Option<u32>, default: u32, unset: &str) {
    row(ui, label, tip, |ui| optional(ui, v, unset, || default, |ui, x| drop(drag(ui, DragValue::new(x).speed(0.2)))));
}
pub(crate) fn check(ui: &mut egui::Ui, label: &str, tip: &str, v: &mut bool) {
    row(ui, label, tip, |ui| drop(ui.checkbox(v, "")));
}
pub(crate) fn grid(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, add: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id).num_columns(2).spacing([10., 5.]).min_col_width(style::LABEL_W).show(ui, add);
}
pub(crate) fn section<R>(
    ui: &mut egui::Ui,
    title: &str,
    open: bool,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    ui.add_space(2.);
    egui::CollapsingHeader::new(RichText::new(title).family(style::label()).size(16.).color(STAR))
        .default_open(open)
        .show(ui, add)
        .body_returned
}

/// Problems whose path is `prefix` or inside it, shown in place.
pub(crate) fn errs(ui: &mut egui::Ui, errors: &[ContentError], prefix: &str) {
    for e in errors.iter().filter(|e| under(&e.path, prefix)) {
        let rest = e.path.strip_prefix(prefix).unwrap_or("").trim_start_matches('.');
        let at = if rest.is_empty() { String::new() } else { format!("{rest}: ") };
        egui::Frame::new().fill(style::ERR_BG).corner_radius(5).inner_margin(egui::Margin::symmetric(8, 4)).show(
            ui,
            |ui| {
                ui.label(RichText::new(format!("⚠ {at}{}", e.msg)).color(style::ERR_TEXT));
            },
        );
    }
}
pub(crate) fn under(path: &str, prefix: &str) -> bool {
    path == prefix || path.strip_prefix(prefix).is_some_and(|r| r.starts_with('.') || r.starts_with('['))
}
pub(crate) fn has_err(errors: &[ContentError], prefix: &str) -> bool {
    errors.iter().any(|e| under(&e.path, prefix))
}
fn d(def: &str, key: &str) -> String {
    form::doc(def, key)
}

pub fn inspector(ui: &mut egui::Ui, ed: &mut Editor) {
    let data = ed.run.as_ref().map(|r| r.data.clone());
    let errors = crate::ui::problems(ed);
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.add_space(6.);
        match ed.subject.clone() {
            Subject::Move(slot) => move_panel(ui, ed, &slot, data, &errors),
            Subject::Throw(k) => {
                ui.label(heading(format!(
                    "{} throw",
                    match k.as_str() {
                        "b" => "Back",
                        "u" => "Up",
                        "d" => "Down",
                        _ => "Forward",
                    }
                )));
                inherited(ui, ed, &format!("throws.{k}"));
                errs(ui, &errors, &format!("throws.{k}"));
                let w = ed_weight(&data, &ed.cur);
                let t = throw_mut(&mut ed.fighter().throws, &k);
                grid(ui, "throw", |ui| {
                    int(ui, "release", &d("Throw", "release"), &mut t.release);
                    int(ui, "total", "Throw length in frames", &mut t.total);
                });
                section(ui, "Hit", true, |ui| hit_fields(ui, &mut t.hit, false, data.as_deref(), w));
                section(ui, "Pre-release damage", false, |ui| {
                    form::edit(
                        ui,
                        egui::Id::new("throw-ticks"),
                        &form::def("Throw")["properties"]["ticks"],
                        &mut t.ticks,
                    );
                });
            }
            Subject::Pose(name) => pose_panel(ui, ed, &name, &errors),
            Subject::Fighter => fighter_panel(ui, ed, &errors),
            Subject::Data(s) => data_panel(ui, ed, s, &errors),
        }
        if let Some(path) = crate::ui::problem_panel(ui, &errors) {
            goto(ed, &path);
        }
    });
}

fn ed_weight(data: &Option<Arc<GameData>>, id: &str) -> f64 {
    data.as_ref().and_then(|d| d.fighter(id)).map_or(1., |f| f.stats.weight)
}

/// "Inherited from base" note when the package's own file doesn't set this path.
fn inherited_note(ed: &Editor, path: &str) -> Option<String> {
    let doc = ed.ws.docs.get(&ed.cur)?;
    let parent = doc.raw.get("extends").and_then(|v| v.as_str())?;
    let mut v = Some(&doc.raw);
    for k in path.split('.') {
        v = v.and_then(|v| v.get(k));
    }
    v.is_none().then(|| format!("↳ Inherited from {parent} · edits save as an override in {}", ed.cur))
}
fn inherited(ui: &mut egui::Ui, ed: &Editor, path: &str) {
    if let Some(n) = inherited_note(ed, path) {
        ui.label(faint(n));
    }
}

struct PhaseSelection {
    phase: usize,
    hitbox: Option<usize>,
    jump: Option<usize>,
    changed: bool,
}

fn phase_list(
    ui: &mut egui::Ui,
    phases: &mut Vec<sim::data::Phase>,
    mut ph: usize,
    mut hb: Option<usize>,
    base: &str,
    errors: &[ContentError],
) -> PhaseSelection {
    let phase_n = phases.len().max(1);
    ph = ph.min(phase_n - 1);
    let mut jump: Option<usize> = None;
    let mut phases_changed = false;
    section(ui, "Phases", true, |ui| {
        let mut op: Option<(usize, i32)> = None;
        #[allow(clippy::needless_range_loop)]
        for i in 0..phases.len() {
            ui.horizontal(|ui| {
                let p = &phases[i];
                let (r, _) = ui.allocate_exact_size(egui::vec2(6., 18.), egui::Sense::hover());
                ui.painter().rect_filled(r, 2, style::PHASE_COLORS[i % 6]);
                let len = p.frames.map_or("loops".into(), |n| format!("{n}f"));
                let bad = has_err(errors, &format!("{base}.phases[{i}]"));
                let mut job = egui::text::LayoutJob::default();
                let c = if i == ph { GOLD } else { STAR };
                job.append(&p.name, 0., egui::TextFormat::simple(egui::FontId::new(15., style::label()), c));
                job.append(&format!("   {len}"), 0., egui::TextFormat::simple(style::text(12.), DIM));
                if !p.hitboxes.is_empty() {
                    job.append(
                        &format!(" · {} box", p.hitboxes.len()),
                        0.,
                        egui::TextFormat::simple(style::text(12.), DIM),
                    );
                }
                if bad {
                    job.append("  ⚠", 0., egui::TextFormat::simple(style::text(12.), RED));
                }
                if ui
                    .add(egui::Button::selectable(i == ph, job).min_size(egui::vec2(170., 0.)))
                    .on_hover_text("Select and jump to it  [ ]")
                    .clicked()
                {
                    ph = i;
                    hb = None;
                    jump = Some(i);
                }
                if ui
                    .add_enabled(i > 0, egui::Button::new(style::ICON_UP).small())
                    .on_hover_text("Move earlier (the first phase starts the move)")
                    .clicked()
                {
                    op = Some((i, -1));
                }
                if ui
                    .add_enabled(i + 1 < phase_n, egui::Button::new(style::ICON_DOWN).small())
                    .on_hover_text("Move later")
                    .clicked()
                {
                    op = Some((i, 1));
                }
                if ui
                    .add_enabled(phase_n > 1, egui::Button::new(style::ICON_DUP).small())
                    .on_hover_text("Duplicate phase")
                    .clicked()
                {
                    op = Some((i, 2));
                }
                if ui
                    .add_enabled(phase_n > 1, egui::Button::new(style::ICON_DEL).small())
                    .on_hover_text("Delete phase (undoable)")
                    .clicked()
                {
                    op = Some((i, 0));
                }
            });
        }
        if ui.button("+ Phase").on_hover_text("Add a phase after the selected one").clicked() {
            let name = doc::unique_name("phase", |n| phases.iter().any(|p| p.name == n));
            phases.insert(ph + 1, Phase { name, frames: Some(10), air_accel_mult: 1., ..Default::default() });
            ph += 1;
            phases_changed = true;
        }
        match op {
            Some((i, 0)) => {
                phases.remove(i);
                ph = ph.min(phases.len() - 1);
                phases_changed = true;
            }
            Some((i, 2)) => {
                let mut c = phases[i].clone();
                c.name = doc::unique_name(&c.name, |n| phases.iter().any(|p| p.name == n));
                phases.insert(i + 1, c);
                ph = i + 1;
                phases_changed = true;
            }
            Some((i, d)) => {
                let j = (i as i32 + d) as usize;
                phases.swap(i, j);
                ph = j;
                phases_changed = true;
            }
            None => {}
        }
    });
    PhaseSelection { phase: ph, hitbox: hb, jump, changed: phases_changed }
}

fn hitbox_panel(
    ui: &mut egui::Ui,
    p: &mut Phase,
    mut hb: Option<usize>,
    pbase: &str,
    playhead_t: Option<u32>,
    data: Option<&GameData>,
    w: f64,
    errors: &[ContentError],
) -> Option<usize> {
    if hb.is_some_and(|h| h >= p.hitboxes.len()) {
        hb = None;
    }
    section(ui, &format!("Hitboxes · {}", p.name), true, |ui| {
        for (i, b) in p.hitboxes.iter().enumerate() {
            let win = format!("{}–{}", b.start, b.end.map_or("end".into(), |e| e.to_string()));
            let mut job = egui::text::LayoutJob::default();
            let on = hb == Some(i);
            let c = style::rgb(render::debug::hit_type_color(b.kind));
            job.append("● ", 0., egui::TextFormat::simple(style::text(12.), c));
            job.append(
                &format!("{i}"),
                0.,
                egui::TextFormat::simple(egui::FontId::new(15., style::label()), if on { GOLD } else { STAR }),
            );
            job.append(
                &format!("   {}%  {}°  f{win}", b.damage, b.angle),
                0.,
                egui::TextFormat::simple(style::text(12.5), if on { GOLD } else { DIM }),
            );
            if !matches!(b.kind, HitType::Normal) {
                job.append(
                    &format!("  {:?}", b.kind).to_lowercase(),
                    0.,
                    egui::TextFormat::simple(style::text(12.), FAINT),
                );
            }
            if has_err(errors, &format!("{pbase}.hitboxes[{i}]")) {
                job.append("  ⚠", 0., egui::TextFormat::simple(style::text(12.), RED));
            }
            if ui.add(egui::Button::selectable(on, job)).on_hover_text("Select  Tab").clicked() {
                hb = if on { None } else { Some(i) };
            }
        }
        if p.hitboxes.is_empty() {
            ui.label(faint("No hitboxes in this phase."));
        }
        ui.horizontal(|ui| {
            if ui.button("+ Hitbox").on_hover_text("New hitbox active from the playhead  N").clicked() {
                p.hitboxes.push(new_hitbox(playhead_t.unwrap_or(0)));
                hb = Some(p.hitboxes.len() - 1);
            }
            if ui.add_enabled(hb.is_some(), egui::Button::new("Duplicate")).on_hover_text("⌘D").clicked()
                && let Some(h) = hb
            {
                let mut c = p.hitboxes[h].clone();
                c.x += 6.;
                p.hitboxes.insert(h + 1, c);
                hb = Some(h + 1);
            }
            if ui.add_enabled(hb.is_some(), egui::Button::new("Delete")).on_hover_text("Del · undoable").clicked()
                && let Some(h) = hb
            {
                p.hitboxes.remove(h);
                hb = None;
            }
        });
        if let Some(h) = hb {
            ui.add_space(4.);
            let card = egui::Frame::new().fill(style::CARD).stroke(egui::Stroke::new(1., style::LINE));
            card.corner_radius(8).inner_margin(10).show(ui, |ui| {
                ui.label(RichText::new(format!("Hitbox {h}")).family(style::label()).size(16.).color(GOLD));
                errs(ui, errors, &format!("{pbase}.hitboxes[{h}]"));
                let b = &mut p.hitboxes[h];
                hit_fields(ui, b, true, data, w);
                if b.geometry.is_none() {
                    key_frames(ui, b, playhead_t);
                }
            });
        }
    });
    hb
}

fn move_panel(ui: &mut egui::Ui, ed: &mut Editor, slot: &str, data: Option<Arc<GameData>>, errors: &[ContentError]) {
    let playhead_t = at_playhead(ed).and_then(|p| p.phase).filter(|(p, _)| *p == ed.phase).map(|(_, t)| t);
    let w = ed_weight(&data, &ed.cur);
    let rigged = ed.fighter().rig.is_some();
    let poses: Vec<String> = ed.fighter().poses.keys().cloned().collect();
    let projectiles: Vec<String> = ed.fighter().projectiles.keys().cloned().collect();
    let attachments: Vec<String> = ed.fighter().attachments.iter().map(|a| a.id.clone()).collect();
    let note = inherited_note(ed, &format!("moves.{slot}"));
    let (mut ph, mut hb) = (ed.phase, ed.hitbox);
    let mut copy = false;
    let mut variant = ed.variant;
    let mut variant_changed = false;
    let Some(mv) = ed.fighter().moves.get_mut(slot) else { return };
    variant = variant.min(mv.variants.len() as u8);
    let base = format!("moves.{slot}");
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut mv.name)
                .font(egui::FontId::new(20., style::display()))
                .frame(egui::Frame::NONE)
                .desired_width(200.),
        )
        .on_hover_text("Move name (training panel, debug overlay). Click to rename.");
        ui.label(caps(slot));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            copy = ui.small_button("Copy to…").on_hover_text("Copy this move over another slot").clicked();
        });
    });
    if let Some(n) = note {
        ui.label(faint(n));
    }
    if !mv.variants.is_empty() {
        let labels: Vec<String> = mv
            .variants
            .iter()
            .enumerate()
            .map(|(i, v)| format!("Variant {i}: needs {} x{}", v.needs.charge, v.needs.uses))
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(faint("Timeline"));
            let mut want = variant;
            ui.selectable_value(&mut want, 0, "Base");
            for (i, label) in labels.iter().enumerate() {
                let v = (i + 1) as u8;
                ui.selectable_value(&mut want, v, label);
            }
            if want != variant {
                variant = want;
                variant_changed = true;
            }
        });
    }
    let tbase = if variant == 0 {
        format!("{base}.phases")
    } else {
        format!("{base}.variants[{}].phases", usize::from(variant) - 1)
    };
    let move_errs: Vec<ContentError> =
        errors.iter().filter(|e| under(&e.path, &base) && !e.path.contains("phases[")).cloned().collect();
    errs(ui, &move_errs, &base);
    let phases = mv.timeline_mut(variant);
    let selection = phase_list(ui, phases, ph, hb, &tbase, errors);
    ph = selection.phase;
    hb = selection.hitbox;
    let (jump, phases_changed) = (selection.jump, selection.changed);
    let names: Vec<String> = mv.timeline(variant).iter().map(|p| p.name.clone()).collect();
    let pbase = format!("{tbase}[{ph}]");
    let p = &mut mv.timeline_mut(variant)[ph];
    hb = hitbox_panel(ui, p, hb, &pbase, playhead_t, data.as_deref(), w, errors);
    let rename = section(ui, &format!("Phase · {}", p.name), true, |ui| {
        let mut rename = None;
        let own: Vec<ContentError> = errors
            .iter()
            .filter(|e| {
                under(&e.path, &pbase)
                    && !["hitboxes", "transitions", "events", "anim"]
                        .iter()
                        .any(|k| e.path.starts_with(&format!("{pbase}.{k}")))
            })
            .cloned()
            .collect();
        errs(ui, &own, &pbase);
        grid(ui, "phase", |ui| {
            row(ui, "name", &d("Phase", "name"), |ui| {
                let old = p.name.clone();
                if ui.text_edit_singleline(&mut p.name).changed() {
                    // Renaming keeps transitions pointing here (fixed up below).
                    rename = Some((old, p.name.clone()));
                }
            });
            row(ui, "frames", &d("Phase", "frames"), |ui| {
                optional(
                    ui,
                    &mut p.frames,
                    "loops until a transition",
                    || 10,
                    |ui, n| {
                        drop(drag(ui, DragValue::new(n).range(1..=600).speed(0.2).suffix(" f")));
                    },
                );
            });
        });
        form::edit(
            ui,
            egui::Id::new(("hurt-windows", ph)),
            &form::def("Phase")["properties"]["hurtRegions"],
            &mut p.hurt_regions,
        );
        ui.label(faint("Region windows override state/default on inclusive phase ticks (not elapsed move time)."));
        sub(ui, "Transitions", &d("Phase", "transitions"));
        errs(ui, errors, &format!("{pbase}.transitions"));
        transitions(ui, p, &names);
        sub(ui, "Events", &d("Phase", "events"));
        errs(ui, errors, &format!("{pbase}.events"));
        events(ui, p, &projectiles, playhead_t);
        sub(ui, "Pose keys", &d("Phase", "anim"));
        errs(ui, errors, &format!("{pbase}.anim"));
        anim_keys(ui, p, &poses, playhead_t, rigged);
        section(ui, "Movement, charge & counter", false, |ui| phase_more(ui, p));
        section(ui, "Attachment animation", false, |ui| {
            ui.label(faint(d("Phase", "attach")));
            let item = form::def("AttachAnim");
            if form::map(ui, egui::Id::new(("attach", ph)), item, &mut p.attach, || Some(Default::default())) {
                ui.ctx().request_repaint();
            }
            if !attachments.is_empty() {
                ui.label(faint(format!("Attachments: {}", attachments.join(", "))));
            }
        });
        rename
    })
    .flatten();
    if let Some((old, new)) = rename {
        for q in mv.timeline_mut(variant).iter_mut() {
            for t in &mut q.transitions {
                if t.to == old {
                    t.to = new.clone();
                }
            }
        }
    }
    section(ui, "Move options", false, |ui| {
        grid(ui, "move", |ui| {
            row(ui, "air control", &d("Move", "air"), |ui| combo(ui, "air", &mut mv.air, &AIRS));
            check(ui, "b-reverse", &d("Move", "bReverse"), &mut mv.b_reverse);
            check(ui, "once per airtime", &d("Move", "oncePerAirtime"), &mut mv.once_per_airtime);
            opt_num(ui, "charge damage", &d("Move", "chargeDamage"), &mut mv.charge_damage, 0.01, "global");
        });
    });
    if variant_changed {
        ed.variant = variant;
        ed.setup.variant = Some(variant);
        ed.phase = 0;
        ed.hitbox = None;
        ed.pane.stale = true;
    } else {
        ed.phase = ph;
        ed.hitbox = hb;
    }
    if phases_changed {
        ed.followed = ed.frame;
    }
    if copy {
        ed.dialog = Some(Dialog::CopyMove { from: slot.into(), to: None });
    }
    if let Some(i) = jump
        && let Some(k) = ed.run.as_ref().and_then(|r| r.marks.iter().position(|m| m.phase.is_some_and(|(p, _)| p == i)))
    {
        ed.set_frame(k);
        ed.followed = k;
    }
}

fn sub(ui: &mut egui::Ui, title: &str, tip: &str) {
    ui.add_space(6.);
    let r = ui.label(caps(title));
    if !tip.is_empty() {
        r.on_hover_text(tip);
    }
}

pub fn new_hitbox(t: u32) -> Hitbox {
    Hitbox {
        start: t,
        end: Some(t + 3),
        x: 30.,
        y: 40.,
        r: 14.,
        damage: 6.,
        bkb: 35.,
        kbg: 1.,
        angle: 40.,
        hitstun_mult: 1.,
        hitlag_mult: 1.,
        shield_dmg_mult: 1.,
        shield_stun_mult: 1.,
        ..Default::default()
    }
}

fn geometry_fields(ui: &mut egui::Ui, id: egui::Id, g: &mut sim::data::hitbox::CombatGeometry) {
    use sim::data::hitbox::CombatGeometry;
    ui.push_id(id, |ui| {
        let capsule = matches!(g, CombatGeometry::Capsule { .. });
        ui.horizontal(|ui| {
            for (label, want) in [("Circle", false), ("Capsule", true)] {
                if ui.selectable_label(capsule == want, label).clicked() && capsule != want {
                    let point = g.points()[0].clone();
                    let radius = g.radius();
                    *g = if want {
                        CombatGeometry::Capsule { a: point.clone(), b: point, radius }
                    } else {
                        CombatGeometry::Circle { center: point, radius }
                    };
                }
            }
        });
        form::edit(ui, id, form::def("CombatGeometry"), g);
    });
}

fn hit_fields(ui: &mut egui::Ui, b: &mut Hitbox, geometry: bool, data: Option<&GameData>, weight: f64) {
    if geometry {
        if b.geometry.is_none() && ui.button("+ Attached geometry").clicked() {
            b.geometry = Some(sim::data::hitbox::CombatGeometry::Circle {
                center: sim::data::hitbox::BoundPoint { joint: None, offset: [b.x, b.y] },
                radius: b.r,
            });
        }
        if let Some(g) = &mut b.geometry {
            geometry_fields(ui, egui::Id::new("combat-geometry"), g);
            ui.label(faint(
                "Bound geometry: edit through forms; viewport handles are disabled. Static x/y/r are ignored.",
            ));
            if !b.frames.is_empty() && ui.button("Clear incompatible legacy geometry tracks").clicked() {
                b.frames.clear();
            }
            if ui.small_button("Remove attached geometry").clicked() {
                b.geometry = None;
            }
        }
    }
    grid(ui, "hit", |ui| {
        if geometry {
            int(ui, "start", &d("Hitbox", "start"), &mut b.start);
            let start = b.start;
            row(ui, "end", &d("Hitbox", "end"), |ui| {
                optional(
                    ui,
                    &mut b.end,
                    "phase end",
                    || start + 3,
                    |ui, e| drop(drag(ui, DragValue::new(e).speed(0.2))),
                );
            });
            row(
                ui,
                "x · y · r",
                "Offset forward and up from the feet, and radius. Drag the box or its rim in the viewport; Alt + arrows nudge.",
                |ui| {
                    ui.horizontal(|ui| {
                        drag(ui, DragValue::new(&mut b.x).speed(0.5).max_decimals(1));
                        drag(ui, DragValue::new(&mut b.y).speed(0.5).max_decimals(1));
                        drag(ui, DragValue::new(&mut b.r).speed(0.25).range(0.5..=400.).max_decimals(1));
                    });
                },
            );
        }
        num(ui, "damage", &d("Hitbox", "damage"), &mut b.damage, 0.1);
        num(ui, "bkb", &d("Hitbox", "bkb"), &mut b.bkb, 0.1);
        num(ui, "kbg", &d("Hitbox", "kbg"), &mut b.kbg, 0.005);
        num(ui, "angle", &format!("{} Drag the gold dot in the viewport.", d("Hitbox", "angle")), &mut b.angle, 1.);
        opt_num(ui, "ground angle", &d("Hitbox", "groundAngle"), &mut b.ground_angle, 0., "same as air");
        row(
            ui,
            "direction",
            "Which way the launch angle faces: the attacker's forward, away from the attacker, or back",
            |ui| combo(ui, "dir", &mut b.dir, &DIRS),
        );
        row(
            ui,
            "type",
            "Hit behavior: spike, trip, sakurai, autolink, windbox, electric, paralysis, bury or grab",
            |ui| combo(ui, "kind", &mut b.kind, &KINDS),
        );
        check(ui, "fixed kb", &d("Hitbox", "setKb"), &mut b.set_kb);
        opt_int(ui, "rehit", &d("Hitbox", "rehit"), &mut b.rehit, 4, "hits once");
        row(ui, "victims", &d("Hitbox", "situation"), |ui| {
            use sim::data::hitbox::Situation;
            combo(
                ui,
                "situation",
                &mut b.situation,
                &[(Situation::Both, "both"), (Situation::Ground, "ground"), (Situation::Air, "air")],
            );
        });
        if geometry {
            int(ui, "min charge", &d("Hitbox", "minCharge"), &mut b.min_charge);
        }
    });
    section(ui, "Multipliers", false, |ui| {
        grid(ui, "mult", |ui| {
            num(ui, "hitstun ×", &d("Hitbox", "hitstunMult"), &mut b.hitstun_mult, 0.01);
            num(ui, "hitstun +", &d("Hitbox", "hitstunAdd"), &mut b.hitstun_add, 0.1);
            num(ui, "hitlag ×", &d("Hitbox", "hitlagMult"), &mut b.hitlag_mult, 0.01);
            num(ui, "shield dmg ×", &d("Hitbox", "shieldDmgMult"), &mut b.shield_dmg_mult, 0.01);
            num(ui, "shield stun ×", &d("Hitbox", "shieldStunMult"), &mut b.shield_stun_mult, 0.01);
            check(ui, "counter dmg", &d("Hitbox", "counterDamage"), &mut b.counter_damage);
        });
    });
    if let Some(gd) = data
        && !matches!(b.kind, HitType::Grab | HitType::Windbox)
    {
        let kb = |pct: f64| sim::combat::knockback(gd, b, pct + b.damage, weight);
        ui.add_space(4.);
        ui.label(caps("Knockback vs. this fighter"))
            .on_hover_text("Knockback and hitstun at a few pre-hit victim percents, including this hit's damage, with this fighter's weight");
        egui::Grid::new("kb").num_columns(5).spacing([12., 2.]).show(ui, |ui| {
            ui.label(faint("victim %"));
            for p in [0., 50., 100., 150.] {
                ui.label(faint(format!("{p:.0}%")));
            }
            ui.end_row();
            ui.label(faint("knockback"));
            for p in [0., 50., 100., 150.] {
                ui.label(RichText::new(format!("{:.1}", kb(p))).color(STAR).small());
            }
            ui.end_row();
            ui.label(faint("hitstun"));
            for p in [0., 50., 100., 150.] {
                ui.label(RichText::new(format!("{}f", sim::combat::hitstun_of(gd, b, kb(p)))).color(STAR).small());
            }
            ui.end_row();
        });
    }
}

/// Per-frame geometry keys.
fn key_frames(ui: &mut egui::Ui, b: &mut Hitbox, t: Option<u32>) {
    ui.horizontal(|ui| {
        ui.label(caps("Per-frame geometry"));
        if let Some(t) = t
            && !b.frames.iter().any(|e| e[0] == f64::from(t))
            && ui.small_button(format!("+ key frame {t}")).on_hover_text("Move/resize from this frame on").clicked()
        {
            let (x, y, r) = b.geom_at(t);
            b.frames.push([f64::from(t), x, y, r]);
            b.frames.sort_by(|a, c| a[0].total_cmp(&c[0]));
        }
    });
    let mut del = None;
    for (i, e) in b.frames.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(RichText::new("f").color(FAINT));
            drag(ui, DragValue::new(&mut e[0]).speed(0.2).range(0.0..=999.).max_decimals(0));
            for v in &mut e[1..] {
                drag(ui, DragValue::new(v).speed(0.5).max_decimals(1));
            }
            if mini(ui, style::ICON_DEL, FAINT, "Delete this key frame") {
                del = Some(i);
            }
        });
    }
    if let Some(i) = del {
        b.frames.remove(i);
    }
}

fn transitions(ui: &mut egui::Ui, p: &mut Phase, names: &[String]) {
    let mut targets: Vec<(String, &str)> = names.iter().map(|n| (n.clone(), n.as_str())).collect();
    targets.extend([(END.to_string(), "end"), (HELPLESS.to_string(), "helpless"), (LANDING.to_string(), "landing")]);
    let mut del = None;
    let mut up = None;
    for (i, t) in p.transitions.iter_mut().enumerate() {
        // Fields wrap in a fixed-width block with the row buttons after it, so ✖ never reads as part of a field.
        ui.horizontal(|ui| {
            let wrap = egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true);
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width() - 44., 0.), wrap, |ui| {
                ui.label(RichText::new("on").color(FAINT));
                combo(ui, ("tr-on", i), &mut t.on, &TRIGGERS);
                combo(ui, ("tr-if", i), &mut t.cond, &CONDS);
                ui.label(RichText::new("to").color(FAINT));
                let cur = t.to.clone();
                egui::ComboBox::from_id_salt(("tr-to", i)).selected_text(&cur).width(90.).show_ui(ui, |ui| {
                    for (v, l) in &targets {
                        ui.selectable_value(&mut t.to, v.clone(), *l);
                    }
                });
                if t.to == LANDING {
                    let lag = t.lag.get_or_insert(10);
                    ui.add(DragValue::new(lag).speed(0.2).prefix("lag "));
                } else {
                    t.lag = None;
                }
                ui.checkbox(&mut t.preserve_hits, "preserve hits")
                    .on_hover_text("Keep victim contact history across this phase transition");
                let mut from = t.from.is_some();
                if ui.checkbox(&mut from, "from").on_hover_text("Only from this phase frame on").changed() {
                    t.from = from.then_some(1);
                }
                if let Some(f) = &mut t.from {
                    drag(ui, DragValue::new(f).speed(0.2));
                }
            });
            if i > 0 && mini(ui, style::ICON_UP, DIM, "Check earlier (checked in order; first match wins)") {
                up = Some(i);
            }
            if mini(ui, style::ICON_DEL, FAINT, "Delete this transition") {
                del = Some(i);
            }
        });
    }
    if let Some(i) = up {
        p.transitions.swap(i, i - 1);
    }
    if let Some(i) = del {
        p.transitions.remove(i);
    }
    if ui.small_button("+ Transition").clicked() {
        let on = if p.frames.is_some() { Trigger::Timeout } else { Trigger::Land };
        p.transitions.push(Transition {
            on,
            cond: None,
            from: None,
            min_charge: 0,
            needs: None,
            to: END.into(),
            lag: None,
            preserve_hits: false,
        });
    }
}

fn vel(ui: &mut egui::Ui, label: &str, v: &mut Option<Vel>) {
    ui.horizontal(|ui| {
        let mut on = v.is_some();
        if ui.checkbox(&mut on, label).changed() {
            *v = on.then(|| v.unwrap_or(Vel { x: Some(0.), y: None }));
        }
        if let Some(v) = v {
            for (c, a) in [("x", &mut v.x), ("y", &mut v.y)] {
                let mut has = a.is_some();
                if ui.checkbox(&mut has, c).changed() {
                    *a = has.then_some(0.);
                }
                if let Some(x) = a {
                    ui.add(DragValue::new(x).speed(0.1).max_decimals(2));
                }
            }
        }
    });
}

fn summary(e: &Event) -> String {
    let when = match (e.at, e.from, e.to) {
        (Some(a), _, _) => format!("@{a}"),
        (None, f, t) => format!("{}–{}", f.unwrap_or(1), t.map_or("…".into(), |t| t.to_string())),
    };
    let mut what = vec![];
    if e.set_vel.is_some() {
        what.push("setVel");
    }
    if e.add_vel.is_some() {
        what.push("addVel");
    }
    if e.mul_vel.is_some() {
        what.push("mulVel");
    }
    if e.leave_ground {
        what.push("leaveGround");
    }
    if e.invuln.is_some() {
        what.push("invuln");
    }
    if e.spawn.is_some() {
        what.push("spawn");
    }
    if e.sfx.is_some() {
        what.push("sfx");
    }
    if !e.fx.is_empty() {
        what.push("fx");
    }
    format!("{when}  {}", what.join(" · "))
}

fn events(ui: &mut egui::Ui, p: &mut Phase, projectiles: &[String], t: Option<u32>) {
    let mut del = None;
    for (i, e) in p.events.iter_mut().enumerate() {
        egui::CollapsingHeader::new(RichText::new(summary(e)).color(STAR)).id_salt(("ev", i)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut range = e.at.is_none();
                if ui.checkbox(&mut range, "range").on_hover_text("One frame (`at`) or a range (`from`–`to`)").changed()
                {
                    if range {
                        e.from = e.at.or(Some(1));
                        e.at = None;
                    } else {
                        e.at = e.from.or(Some(0));
                        (e.from, e.to) = (None, None);
                    }
                }
                match &mut e.at {
                    Some(a) => drop(ui.add(DragValue::new(a).speed(0.2).prefix("at "))),
                    None => {
                        let f = e.from.get_or_insert(1);
                        ui.add(DragValue::new(f).speed(0.2).prefix("from "));
                        let mut to = e.to.is_some();
                        if ui.checkbox(&mut to, "to").changed() {
                            e.to = to.then_some(e.from.unwrap_or(1) + 5);
                        }
                        if let Some(x) = &mut e.to {
                            ui.add(DragValue::new(x).speed(0.2));
                        }
                    }
                }
                combo(ui, ("ev-if", i), &mut e.cond, &CONDS);
            });
            vel(ui, "setVel", &mut e.set_vel);
            vel(ui, "addVel", &mut e.add_vel);
            vel(ui, "mulVel", &mut e.mul_vel);
            grid(ui, ("ev-g", i), |ui| {
                opt_num(ui, "capFall", "Cap downward speed", &mut e.cap_fall, 2., "no cap");
                num(ui, "charge velocity", &d("Event", "chargeVelocity"), &mut e.charge_velocity, 0.001);
                int(ui, "min charge", &d("Event", "minCharge"), &mut e.min_charge);
                check(ui, "leaveGround", "Needed for upward launches from the ground", &mut e.leave_ground);
                check(ui, "turn", &d("Event", "turn"), &mut e.turn);
                opt_int(ui, "invuln", "Invulnerable frames", &mut e.invuln, 5, "none");
                opt_int(ui, "every", "Only on world frames divisible by this", &mut e.every, 2, "every frame");
                row(ui, "spawn", "Projectile", |ui| {
                    let mut opts: Vec<(Option<String>, &str)> = vec![(None, "none")];
                    opts.extend(projectiles.iter().map(|p| (Some(p.clone()), p.as_str())));
                    combo(ui, ("ev-sp", i), &mut e.spawn, &opts);
                });
                row(ui, "sfx", &format!("Sound name (from global.json audio). {}", d("Event", "sfx")), |ui| {
                    let mut s = e.sfx.clone().unwrap_or_default();
                    if ui.text_edit_singleline(&mut s).changed() {
                        e.sfx = (!s.is_empty()).then_some(s);
                    }
                });
            });
            ui.label(caps("Effects")).on_hover_text(d("Event", "fx"));
            form::edit(ui, egui::Id::new(("ev-fx", i)), &form::array_of("Fx"), &mut e.fx);
            if ui.small_button("Delete event").clicked() {
                del = Some(i);
            }
        });
    }
    if let Some(i) = del {
        p.events.remove(i);
    }
    if ui.small_button(format!("+ Event at frame {}", t.unwrap_or(0))).clicked() {
        p.events.push(Event { at: Some(t.unwrap_or(0)), ..Default::default() });
    }
}

fn anim_keys(ui: &mut egui::Ui, p: &mut Phase, poses: &[String], t: Option<u32>, rigged: bool) {
    let mut del = None;
    for (i, k) in p.anim.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let on = t.is_some_and(|t| f64::from(t) == k.at);
            ui.label(RichText::new("♦").color(if on { GOLD } else { FAINT }));
            ui.add(DragValue::new(&mut k.at).speed(0.2).range(0.0..=999.).max_decimals(1).prefix("at "));
            let label = match &k.pose {
                PoseRef::Named(n) => n.clone(),
                PoseRef::Inline(_) => "inline".into(),
            };
            let mut pick = None;
            egui::ComboBox::from_id_salt(("key-pose", i)).selected_text(label).width(96.).height(320.).show_ui(
                ui,
                |ui| {
                    if ui.selectable_label(matches!(k.pose, PoseRef::Inline(_)), "inline (own copy)").clicked() {
                        pick = Some(None);
                    }
                    for n in poses {
                        if ui.selectable_label(matches!(&k.pose, PoseRef::Named(x) if x == n), n).clicked() {
                            pick = Some(Some(n.clone()));
                        }
                    }
                },
            );
            match pick {
                Some(Some(n)) => k.pose = PoseRef::Named(n),
                Some(None) => {
                    if let PoseRef::Named(n) = &k.pose {
                        k.pose = PoseRef::Inline(Box::new(Pose { from: Some(n.clone()), ..Default::default() }));
                    }
                }
                None => {}
            }
            ui.checkbox(&mut k.lerp, "lerp").on_hover_text("Blend from the previous key");
            combo(ui, ("key-if", i), &mut k.cond, &CONDS);
            if mini(ui, style::ICON_DEL, FAINT, "Delete this pose key") {
                del = Some(i);
            }
        });
    }
    if let Some(i) = del {
        p.anim.remove(i);
    }
    let t = t.unwrap_or(0);
    if !p.anim.iter().any(|k| k.at == f64::from(t))
        && ui.small_button(format!("+ Pose key at frame {t}")).on_hover_text("Copies the pose shown now  K").clicked()
    {
        insert_key(p, t);
    }
    ui.label(faint(if rigged {
        "Edit named joint transforms in the pose inspector; inline keys can be edited below."
    } else {
        "Scrub onto a ♦ key to drag its joints in the viewport."
    }));
    if rigged {
        for (i, key) in p.anim.iter_mut().enumerate() {
            if let PoseRef::Inline(pose) = &mut key.pose {
                section(ui, &format!("Key {} · named joints", i + 1), false, |ui| {
                    rig_pose(ui, egui::Id::new(("inline-rig-pose", i)), pose);
                });
            }
        }
    }
}

fn phase_more(ui: &mut egui::Ui, p: &mut Phase) {
    grid(ui, "phase-more", |ui| {
        opt_num(ui, "armor threshold", &d("Phase", "armorThreshold"), &mut p.armor_threshold, 6., "none");
        check(ui, "hover", "Airborne: vertical speed held at 0", &mut p.hover);
        opt_num(ui, "fallCap", "Max fall speed in this phase", &mut p.fall_cap, 4., "fighter's");
        opt_num(ui, "travel speed", &d("Phase", "travelSpeed"), &mut p.travel_speed, 20., "none");
        check(ui, "hidden", &d("Phase", "hidden"), &mut p.hidden);
        check(ui, "pass platforms", &d("Phase", "passPlatforms"), &mut p.pass_platforms);
        num(ui, "air accel ×", "0 = no drift control", &mut p.air_accel_mult, 0.01);
        num(ui, "charge frames", &d("Phase", "chargeFrames"), &mut p.charge_frames, 0.01);
        int(ui, "slide", "Grounded: frames below this use slide friction", &mut p.slide);
        opt_int(ui, "ledgeGrab", "Can grab ledges from this phase frame on", &mut p.ledge_grab, 1, "never");
        row(ui, "charge", "Hold to charge on a frame", |ui| {
            let mut on = p.charge.is_some();
            if ui.checkbox(&mut on, "").changed() {
                p.charge =
                    on.then_some(Charge { frame: 1, max: 60, hold_after_max: 0, button: Button::Attack, jet: false });
            }
        });
    });
    ui.label(caps("Target detection"));
    form::edit(ui, egui::Id::new("phase-detection"), &form::def("Phase")["properties"]["detection"], &mut p.detection);
    ui.label(caps("Tether ledge reach"));
    form::edit(ui, egui::Id::new("phase-tether"), &form::def("Phase")["properties"]["tether"], &mut p.tether);
    ui.label(caps("Reflector"));
    form::edit(ui, egui::Id::new("phase-reflector"), &form::def("Phase")["properties"]["reflector"], &mut p.reflector);
    if let Some(c) = &mut p.charge {
        grid(ui, "charge", |ui| {
            int(ui, "  frame", "Phase frame that freezes while held", &mut c.frame);
            int(ui, "  max", "Max charge frames", &mut c.max);
            int(ui, "  hold after max", &d("Charge", "holdAfterMax"), &mut c.hold_after_max);
            row(ui, "  button", "", |ui| {
                combo(ui, "btn", &mut c.button, &[(Button::Attack, "attack"), (Button::Special, "special")])
            });
            check(ui, "  jet", "Jet flames, sound and glowing attachments", &mut c.jet);
        });
    }
    grid(ui, "counter-on", |ui| {
        row(ui, "counter", "Counter window", |ui| {
            let mut on = p.counter.is_some();
            if ui.checkbox(&mut on, "").changed() {
                p.counter = on.then_some(Counter {
                    from: 1,
                    to: 10,
                    damage_mult: 1.2,
                    min_damage: 5.,
                    attacker_freeze: 20,
                    invuln: 10,
                    reflect_speed: 1.5,
                    reflect_life: 60,
                });
            }
        });
    });
    if let Some(c) = &mut p.counter {
        grid(ui, "counter", |ui| {
            int(ui, "  from", "", &mut c.from);
            int(ui, "  to", "", &mut c.to);
            num(ui, "  damage ×", "", &mut c.damage_mult, 0.01);
            num(ui, "  min damage", "", &mut c.min_damage, 0.1);
            int(ui, "  freeze", "Attacker freeze frames", &mut c.attacker_freeze);
            int(ui, "  invuln", "", &mut c.invuln);
            num(ui, "  reflect speed ×", "", &mut c.reflect_speed, 0.01);
            int(ui, "  reflect life", "", &mut c.reflect_life);
        });
    }
}

fn pose_panel(ui: &mut egui::Ui, ed: &mut Editor, name: &str, errors: &[ContentError]) {
    let mut rename = false;
    ui.horizontal(|ui| {
        ui.label(heading(name));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            rename = ui
                .add_enabled(name != "stand", egui::Button::new("Rename…").small())
                .on_hover_text("Renames every reference too")
                .on_disabled_hover_text("The game needs a pose called stand")
                .clicked();
        });
    });
    let uses = doc::pose_users(ed.fighter(), name);
    let what = if uses > 0 { " · edits show everywhere it's used" } else { "" };
    let rigged = ed.fighter().rig.is_some();
    let hint = if rigged { "Edit named joint transforms below." } else { "Drag joints in the viewport." };
    ui.label(faint(format!("{}{what}. {hint}", doc::uses_text(name, uses))));
    inherited(ui, ed, &format!("poses.{name}"));
    errs(ui, errors, &format!("poses.{name}"));
    if rename {
        ed.dialog = Some(Dialog::RenamePose { old: name.into(), new: name.into() });
    }
    let names: Vec<String> = ed.fighter().poses.keys().filter(|n| *n != name).cloned().collect();
    let parent =
        |f: &sim::data::Fighter| f.poses.get(name).map(|p| Pose { from: p.from.clone(), ..Default::default() });
    let Some(inherit) = parent(ed.fighter()).map(|p| render::pose::resolve(ed.fighter(), &p)) else { return };
    let base = if name == "stand" { "default" } else { "inherited" };
    let Some(p) = ed.fighter().poses.get_mut(name) else { return };
    grid(ui, "pose", |ui| {
        row(ui, "from", &d("Pose", "from"), |ui| {
            let mut opts: Vec<(Option<String>, &str)> = vec![(None, "stand")];
            opts.extend(names.iter().map(|n| (Some(n.clone()), n.as_str())));
            combo(ui, "from", &mut p.from, &opts);
        });
        opt_num(ui, "rot", &d("Pose", "rot"), &mut p.rot, 0., base);
        opt_num(ui, "pivot", &d("Pose", "pivot"), &mut p.pivot, 30., base);
        opt_num(ui, "offsetY", &d("Pose", "offsetY"), &mut p.offset_y, 0., base);
    });
    if rigged {
        rig_pose(ui, egui::Id::new(("rig-pose", name)), p);
    } else {
        pose_joints(ui, p, &inherit, base);
    }
}

fn rig_pose(ui: &mut egui::Ui, id: egui::Id, p: &mut Pose) {
    ui.label(caps("Named joint transforms"));
    ui.label(faint("Use joint IDs from Rig. Unset components inherit the parent pose or rig rest transform."));
    let _ = form::map(ui, id, form::def("JointPose"), &mut p.joints, || Some(Default::default()));
}

/// Joint fields; unset joints come from `from` / `stand` (`inherit`), which is where "+ set" starts them.
fn pose_joints(ui: &mut egui::Ui, p: &mut Pose, inherit: &render::pose::Skel, base: &str) {
    ui.add_space(4.);
    ui.label(caps("Joints (x forward, y up)"));
    grid(ui, "joints", |ui| {
        for (j, name) in crate::view::JOINTS.iter().enumerate() {
            let v = crate::view::pose_joint(p, j);
            let from = crate::view::skel_joint(inherit, j);
            row(ui, name, &d("Pose", name), |ui| {
                optional(
                    ui,
                    v,
                    base,
                    || from,
                    |ui, pt| {
                        drag(ui, DragValue::new(&mut pt[0]).speed(0.5).max_decimals(1));
                        drag(ui, DragValue::new(&mut pt[1]).speed(0.5).max_decimals(1));
                    },
                );
            });
        }
    });
}

const STAT_TIPS: [(&str, &str); 14] = [
    ("weight", ""),
    ("runSpeed", "Top ground speed (px per frame)"),
    ("runAccel", "Ground speed gained per frame while running"),
    ("groundFriction", "Ground speed kept per frame when not running (lower stops faster)"),
    ("jumpSpeed", ""),
    ("shortHopSpeed", "Short hop launch speed (upward)"),
    ("airJumpSpeed", "Mid-air jump launch speed (upward)"),
    ("airJumpDrift", "Horizontal speed a mid-air jump can redirect to"),
    ("airJumps", ""),
    ("airSpeed", "Top horizontal air speed"),
    ("airAccel", "Horizontal air speed gained per frame of drift"),
    ("gravity", "Downward speed gained per frame"),
    ("fallSpeed", "Max fall speed"),
    ("fastFallSpeed", "Fall speed while fast-falling"),
];
fn stat_tip(key: &str) -> String {
    let doc = d("Stats", key);
    if doc.is_empty() {
        STAT_TIPS.iter().find(|(k, _)| *k == key).map_or(String::new(), |(_, t)| t.to_string())
    } else {
        doc
    }
}

fn region_state_choices() -> Vec<(sim::data::hitbox::HurtState, String)> {
    use sim::data::hitbox::HurtState;
    let mut choices = vec![(HurtState::Crouch, "crouch".into())];
    for value in form::def("State")["enum"].as_array().expect("State enum schema") {
        let state = serde_json::from_value(value.clone()).expect("State schema value");
        choices.push((HurtState::State(state), value.as_str().unwrap().into()));
    }
    choices
}

fn region_states(ui: &mut egui::Ui, states: &mut Vec<sim::data::hitbox::HurtStateOverride>) {
    use sim::data::hitbox::{HurtState, HurtStateOverride};
    let choices = region_state_choices();
    let mut remove = None;
    for (i, entry) in states.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                let options: Vec<_> = choices.iter().map(|(state, label)| (state.clone(), label.as_str())).collect();
                combo(ui, "state", &mut entry.state, &options);
                ui.checkbox(&mut entry.enabled, "Enabled");
                if ui.small_button("Remove override").clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        states.remove(i);
    }
    if ui.small_button("+ State override").clicked() {
        states.push(HurtStateOverride { state: HurtState::Crouch, enabled: false });
    }
}

fn fighter_panel(ui: &mut egui::Ui, ed: &mut Editor, errors: &[ContentError]) {
    let id = ed.cur.clone();
    let fresh = ed.ws.docs.get(&id).is_some_and(|d| d.fresh);
    let f = ed.fighter();
    ui.label(heading(&f.name));
    ui.label(faint(format!(
        "fighters/{id}{}{}",
        f.extends.as_ref().map_or(String::new(), |e| format!(" · extends {e}")),
        if fresh { " · new, not saved yet" } else { "" }
    )));
    for k in ["name", "blurb", "tagline", "stats", "body", "hurtboxes", "hurtRegions", "combatCenter", "interactions"] {
        errs(ui, errors, k);
    }
    section(ui, "Identity", true, |ui| {
        grid(ui, "id", |ui| {
            row(ui, "name", "Shown on the select screen", |ui| {
                ui.text_edit_singleline(&mut f.name);
            });
            row(ui, "blurb", &d("Fighter", "blurb"), |ui| {
                ui.text_edit_singleline(&mut f.blurb);
            });
            row(ui, "tagline", &d("Fighter", "tagline"), |ui| {
                ui.text_edit_singleline(&mut f.tagline);
            });
        });
    });
    section(ui, "Stats", true, |ui| {
        let s = &mut f.stats;
        grid(ui, "stats", |ui| {
            num(ui, "weight", &stat_tip("weight"), &mut s.weight, 0.005);
            opt_num(ui, "pummel damage", &d("Stats", "pummelDamage"), &mut s.pummel_damage, 1.3, "global");
            num(ui, "run speed", &stat_tip("runSpeed"), &mut s.run_speed, 0.05);
            num(ui, "run accel", &stat_tip("runAccel"), &mut s.run_accel, 0.01);
            opt_num(ui, "walk speed", &d("Stats", "walkSpeed"), &mut s.walk_speed, 3., "global ratio");
            opt_num(ui, "dash speed", &d("Stats", "dashSpeed"), &mut s.dash_speed, 6., "global ratio");
            opt_int(ui, "dash frames", &d("Stats", "dashFrames"), &mut s.dash_frames, 16, "global");
            num(ui, "ground friction", &stat_tip("groundFriction"), &mut s.ground_friction, 0.005);
            num(ui, "jump speed", &stat_tip("jumpSpeed"), &mut s.jump_speed, 0.05);
            num(ui, "short hop", &stat_tip("shortHopSpeed"), &mut s.short_hop_speed, 0.05);
            num(ui, "air jump speed", &stat_tip("airJumpSpeed"), &mut s.air_jump_speed, 0.05);
            num(ui, "air jump drift", &stat_tip("airJumpDrift"), &mut s.air_jump_drift, 0.05);
            int(ui, "air jumps", &stat_tip("airJumps"), &mut s.air_jumps);
            num(ui, "air speed", &stat_tip("airSpeed"), &mut s.air_speed, 0.05);
            num(ui, "air accel", &stat_tip("airAccel"), &mut s.air_accel, 0.01);
            num(ui, "gravity", &stat_tip("gravity"), &mut s.gravity, 0.005);
            num(ui, "fall speed", &stat_tip("fallSpeed"), &mut s.fall_speed, 0.05);
            num(ui, "fast fall", &stat_tip("fastFallSpeed"), &mut s.fast_fall_speed, 0.05);
        });
    });
    section(ui, "Body & hurtboxes", true, |ui| {
        grid(ui, "body", |ui| {
            num(ui, "half width", "Stage collision: half the body's width", &mut f.body.half_width, 0.5);
            num(ui, "height", "Stage collision height", &mut f.body.height, 0.5);
            for (n, r) in [("hurtbox", &mut f.hurtboxes.default), ("knockdown", &mut f.hurtboxes.knockdown)] {
                row(ui, n, "x1 x2 y1 y2 (x forward, y up from the feet). Knockdown: while lying down.", |ui| {
                    ui.horizontal(|ui| {
                        for v in [&mut r.x1, &mut r.x2, &mut r.y1, &mut r.y2] {
                            drag(ui, DragValue::new(v).speed(0.5).max_decimals(1));
                        }
                    });
                });
            }
        });
    });
    if f.hurt_regions.is_none() && ui.button("+ Named hurt regions").clicked() {
        let r = f.hurtboxes.default;
        f.combat_center = Some([(r.x1 + r.x2) / 2., (r.y1 + r.y2) / 2.]);
        f.hurt_regions = Some(vec![]);
    }
    let referenced: Vec<String> = f
        .moves
        .iter()
        .flat_map(|(_, m)| m.timelines())
        .flatten()
        .flat_map(|p| &p.hurt_regions)
        .map(|w| w.region.clone())
        .collect();
    let mut region_renames = Vec::new();
    if let Some(regions) = &mut f.hurt_regions {
        let ids: Vec<String> = regions.iter().map(|r| r.id.clone()).collect();
        let mut remove = None;
        for (i, r) in regions.iter_mut().enumerate() {
            ui.push_id(("hurt-region", i), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Region ID");
                    let mut proposed = r.id.clone();
                    if ui.text_edit_singleline(&mut proposed).changed() && !proposed.trim().is_empty() && !ids.iter().any(|id| id == &proposed) {
                        region_renames.push((r.id.clone(), proposed.clone()));
                        r.id = proposed;
                    }
                    if ui.add_enabled(!referenced.contains(&r.id), egui::Button::new("Remove region").small()).on_hover_text("Remove phase window references first").clicked() {
                        remove = Some(i);
                    }
                });
                ui.checkbox(&mut r.enabled, "Enabled by default");
                region_states(ui, &mut r.states);
                ui.label(faint("State overrides default; crouch overrides idle. Phase windows override both. Rename updates phase references. Remove phase references before deleting a region."));
                geometry_fields(ui, egui::Id::new(("region-geometry", i)), &mut r.geometry);
            });
        }
        if let Some(i) = remove {
            regions.remove(i);
        }
        if ui.button("+ Region").clicked() {
            let id = doc::unique_name("region", |id| regions.iter().any(|r| r.id == id));
            regions.push(sim::data::hitbox::HurtRegion {
                enabled: true,
                states: vec![],
                id,
                geometry: sim::data::hitbox::CombatGeometry::Circle {
                    center: sim::data::hitbox::BoundPoint { joint: None, offset: [0., 0.] },
                    radius: 1.,
                },
            });
        }
        form::edit(
            ui,
            egui::Id::new("combat-center"),
            &form::def("Fighter")["properties"]["combatCenter"],
            &mut f.combat_center,
        );
        ui.label(faint("combatCenter: stable forward/up autolink target"));
        if ui.button("Use legacy hurt rectangles").clicked() {
            doc::remove_hurt_regions(f);
        }
    }
    for (old, new) in region_renames {
        for slot in sim::data::moves::Moves::SLOTS {
            if let Some(m) = f.moves.get_mut(slot) {
                for p in m.timelines_mut().flat_map(|phases| phases.iter_mut()) {
                    for w in &mut p.hurt_regions {
                        if w.region == old {
                            w.region = new.clone();
                        }
                    }
                }
            }
        }
    }
    section(ui, "Grab & ledge interactions", false, |ui| {
        form::edit(
            ui,
            egui::Id::new("interactions"),
            &form::def("Fighter")["properties"]["interactions"],
            &mut f.interactions,
        );
    });
    ui.label(faint("The preview loops the idle animation with the hurtbox."));
}

/// Fighter-wide data through schema forms.
fn data_panel(ui: &mut egui::Ui, ed: &mut Editor, s: Section, errors: &[ContentError]) {
    ui.label(heading(s.name()));
    let help = match s {
        Section::Rig => {
            "Named joints, local rest transforms, bones, and joint shapes. Named poses animate these joints. Stage and combat geometry remain separate."
        }
        Section::Style => "How the legacy stick figure is drawn: limb widths, glow, head.",
        Section::Attachments => {
            "Vector parts hung on joints: weapons, hats, props. Phases animate them under Attachment animation."
        }
        Section::Projectiles => "Spawned by a move event's spawn field.",
        Section::Effects => "Visual effects for fighter-wide moments.",
        Section::Anims => "State animations (idle, run, air…): pose keys over a clock.",
        Section::Ai => "How CPU players use this fighter.",
    };
    ui.label(faint(help));
    inherited(ui, ed, s.key());
    errs(ui, errors, s.key());
    ui.label(faint("Hover a field for help · ✖ removes an optional field · + Field adds one."));
    ui.add_space(4.);
    let id = egui::Id::new(("data", s.key()));
    if s == Section::Rig {
        let mut state = std::mem::take(&mut ed.rig_edit);
        crate::rig_ed::panel(ui, ed.fighter(), &mut state);
        ed.rig_edit = state;
    }
    let f = ed.fighter();
    match s {
        Section::Rig => {
            if f.rig.is_some() {
                if ui
                    .small_button("Remove rig, joint keys & bound collision")
                    .on_hover_text("Return to legacy: remove named pose keys, joint-bound attack boxes, and any region set with joint bindings. Undo restores all.")
                    .clicked()
                {
                    doc::remove_rig(f);
                }
            } else {
                for (label, text) in [
                    ("Add humanoid rig", include_str!("../../../assets/templates/rigs/humanoid.json")),
                    ("Add quadruped rig", include_str!("../../../assets/templates/rigs/quadruped.json")),
                ] {
                    if ui.button(label).clicked() {
                        f.rig = Some(serde_json::from_str(text).expect("bundled rig template"));
                    }
                }
                ui.label(faint("Templates replace the drawn body. Legacy poses and combat geometry stay unchanged; animate named joints in Poses."));
            }
        }
        Section::Style => drop(form::edit(ui, id, form::def("Style"), &mut f.style)),
        Section::Attachments => drop(form::edit(ui, id, &form::array_of("Attachment"), &mut f.attachments)),
        Section::Projectiles => drop(form::map(ui, id, form::def("Projectile"), &mut f.projectiles, || None)),
        Section::Effects => drop(form::edit(ui, id, form::def("FighterFx"), &mut f.fx)),
        Section::Anims => drop(form::map(ui, id, form::def("Anim"), &mut f.anims, || None)),
        Section::Ai => drop(form::edit(ui, id, form::def("Ai"), &mut f.ai)),
    }
}

/// Selects what a JSON path like `moves.jab.phases[1].hitboxes[0].r` points at.
pub fn goto(ed: &mut Editor, path: &str) {
    let parts: Vec<&str> = path.split('.').collect();
    match parts.as_slice() {
        ["moves", slot, rest @ ..] => {
            ed.select(Subject::Move(slot.to_string()));
            if let Some(p) = rest.first().and_then(|s| crate::section::index_of(s, "phases")) {
                ed.phase = p;
                ed.hitbox = rest.get(1).and_then(|s| crate::section::index_of(s, "hitboxes"));
                ed.followed = ed.frame;
            }
        }
        ["throws", k, ..] => ed.select(Subject::Throw(k.to_string())),
        ["poses", n, ..] => ed.select(Subject::Pose(n.to_string())),
        [k, ..] if let Some(s) = crate::section::of_path::<Section>(k) => ed.select(Subject::Data(s)),
        _ => ed.select(Subject::Fighter),
    }
}

/// Current phase on the playhead, for keyboard hitbox operations.
fn cur_phase(ed: &mut Editor) -> Option<(&mut Phase, Option<u32>)> {
    let Subject::Move(slot) = ed.subject.clone() else { return None };
    let t = at_playhead(ed).and_then(|p| p.phase).filter(|(p, _)| *p == ed.phase).map(|(_, t)| t);
    let ph = ed.phase;
    let variant = ed.variant;
    let mv: &mut Move = ed.fighter().moves.get_mut(&slot)?;
    Some((mv.timeline_mut(variant).get_mut(ph)?, t))
}

pub fn add_hitbox(ed: &mut Editor) {
    let Some((p, t)) = cur_phase(ed) else { return };
    p.hitboxes.push(new_hitbox(t.unwrap_or(0)));
    let n = p.hitboxes.len() - 1;
    ed.hitbox = Some(n);
}
pub fn duplicate_hitbox(ed: &mut Editor) {
    let Some(h) = ed.hitbox else { return };
    let Some((p, _)) = cur_phase(ed) else { return };
    let Some(mut c) = p.hitboxes.get(h).cloned() else { return };
    c.x += 6.;
    p.hitboxes.insert(h + 1, c);
    ed.hitbox = Some(h + 1);
}
pub fn delete_hitbox(ed: &mut Editor) {
    let Some(h) = ed.hitbox else { return };
    let Some((p, _)) = cur_phase(ed) else { return };
    if h < p.hitboxes.len() {
        p.hitboxes.remove(h);
    }
    ed.hitbox = None;
}

pub fn nudge_hitbox(ed: &mut Editor, dx: f64, dy: f64) {
    let Some(h) = ed.hitbox else { return };
    let t = at_playhead(ed).and_then(|p| p.phase).map_or(0, |(_, t)| t);
    let Some((p, _)) = cur_phase(ed) else { return };
    let Some(b) = p.hitboxes.get_mut(h) else { return };
    if b.geometry.is_some() {
        return;
    }
    match crate::view::geom_source(b, t).1 {
        Some(j) => (b.frames[j][1], b.frames[j][2]) = (b.frames[j][1] + dx, b.frames[j][2] + dy),
        None => (b.x, b.y) = (b.x + dx, b.y + dy),
    }
}

/// A pose key on the playhead frame, copying the pose shown (like "+ Pose key").
pub fn add_pose_key(ed: &mut Editor) {
    let Some((p, Some(t))) = cur_phase(ed) else { return };
    insert_key(p, t);
}

fn insert_key(p: &mut Phase, t: u32) {
    if p.anim.iter().any(|k| k.at == f64::from(t)) {
        return;
    }
    let prev = p.anim.iter().rfind(|k| k.at <= f64::from(t)).map(|k| k.pose.clone());
    let pose = match prev {
        Some(PoseRef::Inline(pz)) => PoseRef::Inline(pz),
        Some(PoseRef::Named(n)) => PoseRef::Inline(Box::new(Pose { from: Some(n), ..Default::default() })),
        None => PoseRef::Inline(Box::new(Pose { from: Some("stand".into()), ..Default::default() })),
    };
    p.anim.push(AnimKey { at: f64::from(t), pose, lerp: false, cond: None, min_charge: 0 });
    p.anim.sort_by(|a, b| a.at.total_cmp(&b.at));
}

#[cfg(test)]
mod region_selector_tests {
    #[test]
    fn selector_offers_crouch_and_every_ordinary_state() {
        use sim::data::hitbox::HurtState;
        let choices = super::region_state_choices();
        assert!(choices.iter().any(|(s, _)| *s == HurtState::Crouch));
        assert!(choices.iter().any(|(s, _)| *s == HurtState::State(sim::world::State::Idle)));
        assert_eq!(choices.len(), super::form::def("State")["enum"].as_array().unwrap().len() + 1);
    }
}
