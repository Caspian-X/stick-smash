//! The inspector: structured fields for the selection (hitbox, phase, move, throw, pose, fighter), plus the
//! problems list. Widgets edit the fighter in place; `ui::run` turns changes into undo steps.

use std::sync::Arc;

use bevy_egui::egui::{self, DragValue, RichText};
use sim::GameData;
use sim::data::hitbox::{HitType, Hitbox, LaunchDir};
use sim::data::moves::{
    AirControl, Button, Charge, Cond, Counter, END, Event, HELPLESS, LANDING, Move, Phase, Transition, Trigger, Vel,
};
use sim::data::visual::{AnimKey, Pose, PoseRef};

use crate::preview::Subject;
use crate::style::{self, DIM, FAINT, GOLD, RED, STAR, caps, heading};
use crate::ui::{at_playhead, fighter};
use crate::{Editor, move_mut, throw_mut};

const TRIGGERS: [(Trigger, &str); 9] = [
    (Trigger::Timeout, "timeout"),
    (Trigger::Land, "land"),
    (Trigger::Grounded, "grounded"),
    (Trigger::Airborne, "airborne"),
    (Trigger::Falling, "falling"),
    (Trigger::Release, "release"),
    (Trigger::ShieldPress, "shieldPress"),
    (Trigger::Counter, "counter"),
    (Trigger::Hit, "hit"),
];
const CONDS: [(Option<Cond>, &str); 4] = [
    (None, "always"),
    (Some(Cond::Grounded), "grounded"),
    (Some(Cond::Airborne), "airborne"),
    (Some(Cond::Countered), "countered"),
];
const KINDS: [(HitType, &str); 8] = [
    (HitType::Normal, "normal"),
    (HitType::Spike, "spike"),
    (HitType::Trip, "trip"),
    (HitType::Sakurai, "sakurai"),
    (HitType::Autolink, "autolink"),
    (HitType::Windbox, "windbox"),
    (HitType::Electric, "electric"),
    (HitType::Grab, "grab"),
];
const DIRS: [(LaunchDir, &str); 3] =
    [(LaunchDir::Forward, "forward"), (LaunchDir::Away, "away"), (LaunchDir::Back, "back")];
const AIRS: [(AirControl, &str); 3] =
    [(AirControl::Attack, "attack"), (AirControl::Aerial, "aerial"), (AirControl::UpSpecial, "upSpecial")];

fn combo<T: PartialEq + Clone>(
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
fn row(ui: &mut egui::Ui, label: &str, tip: &str, add: impl FnOnce(&mut egui::Ui)) {
    let r = ui.label(RichText::new(label).color(DIM));
    if !tip.is_empty() {
        r.on_hover_text(tip);
    }
    add(ui);
    ui.end_row();
}
fn num(ui: &mut egui::Ui, label: &str, tip: &str, v: &mut f64, speed: f64) {
    row(ui, label, tip, |ui| {
        ui.add(DragValue::new(v).speed(speed).max_decimals(3));
    });
}
fn int(ui: &mut egui::Ui, label: &str, tip: &str, v: &mut u32) {
    row(ui, label, tip, |ui| {
        ui.add(DragValue::new(v).speed(0.2));
    });
}
/// Optional number: a checkbox turns it on with `default`.
fn opt_num(ui: &mut egui::Ui, label: &str, tip: &str, v: &mut Option<f64>, default: f64) {
    row(ui, label, tip, |ui| {
        ui.horizontal(|ui| {
            let mut on = v.is_some();
            if ui.checkbox(&mut on, "").changed() {
                *v = on.then_some(v.unwrap_or(default));
            }
            if let Some(x) = v {
                ui.add(DragValue::new(x).speed(0.1).max_decimals(3));
            }
        });
    });
}
fn opt_int(ui: &mut egui::Ui, label: &str, tip: &str, v: &mut Option<u32>, default: u32) {
    row(ui, label, tip, |ui| {
        ui.horizontal(|ui| {
            let mut on = v.is_some();
            if ui.checkbox(&mut on, "").changed() {
                *v = on.then_some(v.unwrap_or(default));
            }
            if let Some(x) = v {
                ui.add(DragValue::new(x).speed(0.2));
            }
        });
    });
}
fn grid(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, add: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id).num_columns(2).spacing([10., 5.]).min_col_width(92.).show(ui, add);
}
fn section(ui: &mut egui::Ui, title: &str, open: bool, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).family(style::label()).size(15.).color(STAR))
        .default_open(open)
        .show(ui, add);
}

pub fn inspector(ui: &mut egui::Ui, ed: &mut Editor) {
    let data = ed.run.as_ref().map(|r| r.data.clone());
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.add_space(6.);
        match ed.subject.clone() {
            Subject::Move(slot) => move_panel(ui, ed, &slot, data),
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
                let w = ed_weight(&data, &ed.cur);
                let t = throw_mut(&mut fighter(ed).throws, &k);
                grid(ui, "throw", |ui| {
                    int(ui, "release", "Frame the victim is released and hit", &mut t.release);
                    int(ui, "total", "Throw length", &mut t.total);
                });
                section(ui, "Hit", true, |ui| hit_fields(ui, &mut t.hit, false, data.as_deref(), w));
            }
            Subject::Pose(name) => pose_panel(ui, ed, &name),
            Subject::Fighter => fighter_panel(ui, ed),
        }
        problems(ui, ed);
    });
}

fn ed_weight(data: &Option<Arc<GameData>>, id: &str) -> f64 {
    data.as_ref().and_then(|d| d.fighter(id)).map_or(1., |f| f.stats.weight)
}

/// "Inherited from base" note when the package's own file doesn't set this path.
fn inherited(ui: &mut egui::Ui, ed: &Editor, path: &str) {
    let Some(doc) = ed.ws.docs.get(&ed.cur) else { return };
    let Some(parent) = doc.raw.get("extends").and_then(|v| v.as_str()) else { return };
    let mut v = Some(&doc.raw);
    for k in path.split('.') {
        v = v.and_then(|v| v.get(k));
    }
    if v.is_none() {
        ui.label(
            RichText::new(format!("Inherited from {parent} · edits save as an override in {}", ed.cur))
                .color(FAINT)
                .small(),
        );
    }
}

fn move_panel(ui: &mut egui::Ui, ed: &mut Editor, slot: &str, data: Option<Arc<GameData>>) {
    let playhead_t = at_playhead(ed).and_then(|(_, m)| m).filter(|(p, _)| *p == ed.phase).map(|(_, t)| t);
    let w = ed_weight(&data, &ed.cur);
    let poses: Vec<String> = fighter(ed).poses.keys().cloned().collect();
    let projectiles: Vec<String> = fighter(ed).projectiles.keys().cloned().collect();
    let (mut ph, mut hb) = (ed.phase, ed.hitbox);
    let Some(mv) = move_mut(&mut fighter(ed).moves, slot) else { return };
    ui.horizontal(|ui| {
        ui.label(heading(&mv.name));
        ui.label(caps(slot));
    });
    let phase_n = mv.phases.len();
    ph = ph.min(phase_n - 1);
    let mut jump: Option<usize> = None;
    // Inherited note (needs ed immutably, so after the borrow ends below).
    let mut phases_changed = false;
    section(ui, "Phases", true, |ui| {
        let mut op: Option<(usize, i32)> = None;
        for i in 0..mv.phases.len() {
            ui.horizontal(|ui| {
                let p = &mv.phases[i];
                let len = p.frames.map_or("loop".into(), |n| format!("{n}f"));
                let txt = RichText::new(format!("{}  {}", p.name, len)).color(if i == ph { GOLD } else { STAR });
                if ui.add(egui::Button::selectable(i == ph, txt).min_size(egui::vec2(150., 0.))).clicked() {
                    ph = i;
                    hb = None;
                    jump = Some(i);
                }
                if ui
                    .add_enabled(i > 0, egui::Button::new("⏶").small())
                    .on_hover_text("Move earlier (the first phase starts the move)")
                    .clicked()
                {
                    op = Some((i, -1));
                }
                if ui.add_enabled(i + 1 < phase_n, egui::Button::new("⏷").small()).clicked() {
                    op = Some((i, 1));
                }
                if ui.add_enabled(phase_n > 1, egui::Button::new("✖").small()).on_hover_text("Delete phase").clicked()
                {
                    op = Some((i, 0));
                }
            });
        }
        if ui.button("+ Phase").on_hover_text("Add a phase after the selected one").clicked() {
            let mut n = mv.phases.len();
            while mv.phases.iter().any(|p| p.name == format!("phase{n}")) {
                n += 1;
            }
            mv.phases.insert(
                ph + 1,
                Phase { name: format!("phase{n}"), frames: Some(10), air_accel_mult: 1., ..Default::default() },
            );
            ph += 1;
            phases_changed = true;
        }
        match op {
            Some((i, 0)) => {
                mv.phases.remove(i);
                ph = ph.min(mv.phases.len() - 1);
                phases_changed = true;
            }
            Some((i, d)) => {
                let j = (i as i32 + d) as usize;
                mv.phases.swap(i, j);
                ph = j;
                phases_changed = true;
            }
            None => {}
        }
    });
    let names: Vec<String> = mv.phases.iter().map(|p| p.name.clone()).collect();
    if let Some(h) = hb.filter(|&h| h < mv.phases[ph].hitboxes.len()) {
        section(ui, &format!("Hitbox {h}"), true, |ui| {
            let p = &mut mv.phases[ph];
            let b = &mut p.hitboxes[h];
            hit_fields(ui, b, true, data.as_deref(), w);
            key_frames(ui, b, playhead_t);
        });
    } else {
        hb = None;
    }
    let p = &mut mv.phases[ph];
    section(ui, &format!("Phase · {}", p.name), true, |ui| {
        grid(ui, "phase", |ui| {
            row(ui, "name", "Unique in the move; transitions refer to it", |ui| {
                let old = p.name.clone();
                if ui.text_edit_singleline(&mut p.name).changed() {
                    let new = p.name.clone();
                    // Renaming keeps transitions pointing here (fixed up below).
                    ui.data_mut(|d| d.insert_temp(egui::Id::new("ed-rename"), (old, new)));
                }
            });
            row(ui, "frames", "Phase length; off = loops until a transition fires", |ui| {
                ui.horizontal(|ui| {
                    let mut fixed = p.frames.is_some();
                    if ui.checkbox(&mut fixed, "").changed() {
                        p.frames = fixed.then_some(p.frames.unwrap_or(10));
                    }
                    match &mut p.frames {
                        Some(n) => {
                            ui.add(DragValue::new(n).range(1..=600).speed(0.2));
                        }
                        None => {
                            ui.label(RichText::new("loops").color(FAINT));
                        }
                    }
                });
            });
        });
        ui.add_space(4.);
        ui.label(caps("Hitboxes"));
        for (i, b) in p.hitboxes.iter().enumerate() {
            let win = format!("{}–{}", b.start, b.end.map_or("end".into(), |e| e.to_string()));
            let txt = format!("{i}  {:?}  {}%  {}°  f{win}", b.kind, b.damage, b.angle).to_lowercase();
            if ui
                .selectable_label(hb == Some(i), RichText::new(txt).color(if hb == Some(i) { GOLD } else { STAR }))
                .clicked()
            {
                hb = Some(i);
            }
        }
        ui.horizontal(|ui| {
            if ui.button("+ Hitbox").on_hover_text("N").clicked() {
                p.hitboxes.push(new_hitbox(playhead_t.unwrap_or(0)));
                hb = Some(p.hitboxes.len() - 1);
            }
            if ui.add_enabled(hb.is_some(), egui::Button::new("Duplicate")).on_hover_text("⌘/Ctrl D").clicked()
                && let Some(h) = hb
            {
                let mut c = p.hitboxes[h].clone();
                c.x += 6.;
                p.hitboxes.insert(h + 1, c);
                hb = Some(h + 1);
            }
            if ui.add_enabled(hb.is_some(), egui::Button::new("Delete")).on_hover_text("Delete / Backspace").clicked()
                && let Some(h) = hb
            {
                p.hitboxes.remove(h);
                hb = None;
            }
        });
        ui.add_space(4.);
        ui.label(caps("Transitions"));
        transitions(ui, p, &names);
        ui.add_space(4.);
        ui.label(caps("Events"));
        events(ui, p, &projectiles, playhead_t);
        ui.add_space(4.);
        ui.label(caps("Pose keys"));
        anim_keys(ui, p, &poses, playhead_t);
        ui.add_space(4.);
        section(ui, "More phase options", false, |ui| phase_more(ui, p));
    });
    if let Some((old, new)) = ui.data_mut(|d| d.remove_temp::<(String, String)>(egui::Id::new("ed-rename"))) {
        for q in &mut mv.phases {
            for t in &mut q.transitions {
                if t.to == old {
                    t.to = new.clone();
                }
            }
        }
    }
    section(ui, "Move", false, |ui| {
        grid(ui, "move", |ui| {
            row(ui, "name", "Shown in the training panel", |ui| {
                ui.text_edit_singleline(&mut mv.name);
            });
            row(ui, "air control", "Air acceleration while airborne; aerial also allows fast-fall", |ui| {
                combo(ui, "air", &mut mv.air, &AIRS)
            });
            row(ui, "b-reverse", "Turn around by holding back early in the move", |ui| {
                ui.checkbox(&mut mv.b_reverse, "");
            });
        });
    });
    inherited(ui, ed, &format!("moves.{slot}"));
    ed.phase = ph;
    ed.hitbox = hb;
    if phases_changed {
        ed.followed = ed.frame;
    }
    if let Some(i) = jump
        && let Some(k) = ed.run.as_ref().and_then(|r| r.marks.iter().position(|m| m.phase.is_some_and(|(p, _)| p == i)))
    {
        ed.set_frame(k);
        ed.followed = k;
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
        bkb: 5.,
        kbg: 0.1,
        angle: 40.,
        hitstun_mult: 1.,
        hitlag_mult: 1.,
        shield_dmg_mult: 1.,
        shield_stun_mult: 1.,
        ..Default::default()
    }
}

fn hit_fields(ui: &mut egui::Ui, b: &mut Hitbox, geometry: bool, data: Option<&GameData>, weight: f64) {
    grid(ui, "hit", |ui| {
        if geometry {
            int(ui, "start", "First active phase frame", &mut b.start);
            row(ui, "end", "Last active phase frame (inclusive); off = until the phase ends", |ui| {
                ui.horizontal(|ui| {
                    let mut on = b.end.is_some();
                    if ui.checkbox(&mut on, "").changed() {
                        b.end = on.then_some(b.end.unwrap_or(b.start + 3));
                    }
                    match &mut b.end {
                        Some(e) => drop(ui.add(DragValue::new(e).speed(0.2))),
                        None => drop(ui.label(RichText::new("phase end").color(FAINT))),
                    }
                });
            });
            row(ui, "x · y · r", "Offset forward, up from the feet, and radius (drag in the viewport)", |ui| {
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut b.x).speed(0.5).max_decimals(1));
                    ui.add(DragValue::new(&mut b.y).speed(0.5).max_decimals(1));
                    ui.add(DragValue::new(&mut b.r).speed(0.25).range(0.5..=400.).max_decimals(1));
                });
            });
        }
        num(ui, "damage", "Percent dealt", &mut b.damage, 0.1);
        num(ui, "bkb", "Base knockback", &mut b.bkb, 0.1);
        num(ui, "kbg", "Knockback growth per victim percent", &mut b.kbg, 0.005);
        num(ui, "angle", "Launch angle: 0 forward, 90 up, negative down (drag the gold dot)", &mut b.angle, 1.);
        row(ui, "direction", "Launch direction rule", |ui| combo(ui, "dir", &mut b.dir, &DIRS));
        row(ui, "type", "Hit behavior", |ui| combo(ui, "kind", &mut b.kind, &KINDS));
        row(ui, "fixed kb", "Ignore the victim's percent (knockback = bkb × weight)", |ui| {
            ui.checkbox(&mut b.set_kb, "");
        });
        opt_int(ui, "rehit", "Multi-hit: hit the same victim again every N frames", &mut b.rehit, 4);
    });
    section(ui, "Multipliers", false, |ui| {
        grid(ui, "mult", |ui| {
            num(ui, "hitstun ×", "", &mut b.hitstun_mult, 0.01);
            num(ui, "hitstun +", "Flat frames", &mut b.hitstun_add, 0.1);
            num(ui, "hitlag ×", "", &mut b.hitlag_mult, 0.01);
            num(ui, "shield dmg ×", "", &mut b.shield_dmg_mult, 0.01);
            num(ui, "shield stun ×", "", &mut b.shield_stun_mult, 0.01);
            row(ui, "counter dmg", "Damage comes from the countered hit", |ui| {
                ui.checkbox(&mut b.counter_damage, "");
            });
        });
    });
    if let Some(d) = data
        && !matches!(b.kind, HitType::Grab | HitType::Windbox)
    {
        let kb = |pct: f64| sim::combat::knockback(d, b, pct, weight);
        let line = [0., 50., 100., 150.]
            .map(|p| format!("{p:.0}%: {:.1} kb · {}f", kb(p), sim::combat::hitstun_of(d, b, kb(p))))
            .join("   ");
        ui.label(RichText::new(format!("vs. this fighter's weight  {line}")).color(FAINT).small());
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
            ui.add(DragValue::new(&mut e[0]).speed(0.2).range(0.0..=999.).max_decimals(0));
            for v in &mut e[1..] {
                ui.add(DragValue::new(v).speed(0.5).max_decimals(1));
            }
            if ui.small_button("✖").clicked() {
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
        ui.horizontal_wrapped(|ui| {
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
            let mut from = t.from.is_some();
            if ui.checkbox(&mut from, "from").on_hover_text("Only from this phase frame on").changed() {
                t.from = from.then_some(1);
            }
            if let Some(f) = &mut t.from {
                ui.add(DragValue::new(f).speed(0.2));
            }
            if i > 0 && ui.small_button("⏶").on_hover_text("Checked in order; first match wins").clicked() {
                up = Some(i);
            }
            if ui.small_button("✖").clicked() {
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
        p.transitions.push(Transition { on, cond: None, from: None, to: END.into(), lag: None });
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
                opt_num(ui, "capFall", "Cap downward speed", &mut e.cap_fall, 2.);
                row(ui, "leaveGround", "Needed for upward launches from the ground", |ui| {
                    ui.checkbox(&mut e.leave_ground, "");
                });
                opt_int(ui, "invuln", "Invulnerable frames", &mut e.invuln, 5);
                opt_int(ui, "every", "Only on world frames divisible by this", &mut e.every, 2);
                row(ui, "spawn", "Projectile", |ui| {
                    let mut opts: Vec<(Option<String>, &str)> = vec![(None, "none")];
                    opts.extend(projectiles.iter().map(|p| (Some(p.clone()), p.as_str())));
                    combo(ui, ("ev-sp", i), &mut e.spawn, &opts);
                });
                row(ui, "sfx", "Sound name", |ui| {
                    let mut s = e.sfx.clone().unwrap_or_default();
                    if ui.text_edit_singleline(&mut s).changed() {
                        e.sfx = (!s.is_empty()).then_some(s);
                    }
                });
            });
            if !e.fx.is_empty() {
                ui.label(
                    RichText::new(format!("{} effect(s) — edit fx in the JSON file", e.fx.len())).color(FAINT).small(),
                );
            }
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

fn anim_keys(ui: &mut egui::Ui, p: &mut Phase, poses: &[String], t: Option<u32>) {
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
            if ui.small_button("✖").clicked() {
                del = Some(i);
            }
        });
    }
    if let Some(i) = del {
        p.anim.remove(i);
    }
    let t = t.unwrap_or(0);
    if !p.anim.iter().any(|k| k.at == f64::from(t))
        && ui.small_button(format!("+ Pose key at frame {t}")).on_hover_text("Copies the pose shown now").clicked()
    {
        let prev = p.anim.iter().rfind(|k| k.at <= f64::from(t)).map(|k| k.pose.clone());
        let pose = match prev {
            Some(PoseRef::Inline(pz)) => PoseRef::Inline(pz),
            Some(PoseRef::Named(n)) => PoseRef::Inline(Box::new(Pose { from: Some(n), ..Default::default() })),
            None => PoseRef::Inline(Box::new(Pose { from: Some("stand".into()), ..Default::default() })),
        };
        p.anim.push(AnimKey { at: f64::from(t), pose, lerp: false, cond: None });
        p.anim.sort_by(|a, b| a.at.total_cmp(&b.at));
    }
    ui.label(RichText::new("Scrub onto a ♦ key to drag its joints in the viewport.").color(FAINT).small());
}

fn phase_more(ui: &mut egui::Ui, p: &mut Phase) {
    grid(ui, "phase-more", |ui| {
        row(ui, "hover", "Airborne: vertical speed held at 0", |ui| {
            ui.checkbox(&mut p.hover, "");
        });
        opt_num(ui, "fallCap", "Max fall speed in this phase", &mut p.fall_cap, 4.);
        num(ui, "air accel ×", "0 = no drift control", &mut p.air_accel_mult, 0.01);
        int(ui, "slide", "Grounded: frames below this use slide friction", &mut p.slide);
        opt_int(ui, "ledgeGrab", "Can grab ledges from this phase frame on", &mut p.ledge_grab, 1);
        row(ui, "charge", "Hold to charge on a frame", |ui| {
            let mut on = p.charge.is_some();
            if ui.checkbox(&mut on, "").changed() {
                p.charge = on.then_some(Charge { frame: 1, max: 60, button: Button::Attack, jet: false });
            }
        });
    });
    if let Some(c) = &mut p.charge {
        grid(ui, "charge", |ui| {
            int(ui, "  frame", "Phase frame that freezes while held", &mut c.frame);
            int(ui, "  max", "Max charge frames", &mut c.max);
            row(ui, "  button", "", |ui| {
                combo(ui, "btn", &mut c.button, &[(Button::Attack, "attack"), (Button::Special, "special")])
            });
            row(ui, "  jet", "Jet flames, sound and glowing attachments", |ui| {
                ui.checkbox(&mut c.jet, "");
            });
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

fn pose_panel(ui: &mut egui::Ui, ed: &mut Editor, name: &str) {
    ui.label(heading(name));
    let uses = {
        let f = fighter(ed);
        let named = |k: &AnimKey| matches!(&k.pose, PoseRef::Named(n) if n == name);
        f.moves.iter().flat_map(|(_, m)| m.phases.iter()).flat_map(|p| p.anim.iter()).filter(|k| named(k)).count()
            + f.anims.values().flat_map(|a| a.keys.iter()).filter(|k| named(k)).count()
            + f.poses.values().filter(|p| p.from.as_deref() == Some(name)).count()
    };
    ui.label(
        RichText::new(format!("Shared: used by {uses} key(s) / pose(s). Drag joints in the viewport."))
            .color(FAINT)
            .small(),
    );
    inherited(ui, ed, &format!("poses.{name}"));
    let names: Vec<String> = fighter(ed).poses.keys().filter(|n| *n != name).cloned().collect();
    let Some(p) = fighter(ed).poses.get_mut(name) else { return };
    grid(ui, "pose", |ui| {
        row(ui, "from", "Missing joints come from this pose (else stand)", |ui| {
            let mut opts: Vec<(Option<String>, &str)> = vec![(None, "stand")];
            opts.extend(names.iter().map(|n| (Some(n.clone()), n.as_str())));
            combo(ui, "from", &mut p.from, &opts);
        });
        opt_num(ui, "rot", "Body rotation (degrees, + pitches forward)", &mut p.rot, 0.);
        opt_num(ui, "pivot", "Rotation center height", &mut p.pivot, 30.);
        opt_num(ui, "offsetY", "Lift the whole figure", &mut p.offset_y, 0.);
    });
    pose_joints(ui, p);
}

/// Joint fields; unchecked joints come from `from` / `stand`.
fn pose_joints(ui: &mut egui::Ui, p: &mut Pose) {
    ui.label(caps("Joints (x forward, y up)"));
    grid(ui, "joints", |ui| {
        for (j, name) in crate::view::JOINTS.iter().enumerate() {
            let v = crate::view::pose_joint(p, j);
            row(ui, name, "", |ui| {
                ui.horizontal(|ui| {
                    let mut on = v.is_some();
                    if ui.checkbox(&mut on, "").on_hover_text("Off: inherited").changed() {
                        *v = on.then_some([0., 0.]);
                    }
                    if let Some(pt) = v {
                        ui.add(DragValue::new(&mut pt[0]).speed(0.5).max_decimals(1));
                        ui.add(DragValue::new(&mut pt[1]).speed(0.5).max_decimals(1));
                    }
                });
            });
        }
    });
}

fn fighter_panel(ui: &mut egui::Ui, ed: &mut Editor) {
    let id = ed.cur.clone();
    let f = fighter(ed);
    ui.label(heading(&f.name));
    ui.label(
        RichText::new(format!(
            "fighters/{id}{}",
            f.extends.as_ref().map_or(String::new(), |e| format!(" · extends {e}"))
        ))
        .color(FAINT)
        .small(),
    );
    section(ui, "Identity", true, |ui| {
        grid(ui, "id", |ui| {
            row(ui, "name", "", |ui| {
                ui.text_edit_singleline(&mut f.name);
            });
            row(ui, "blurb", "Menu help line", |ui| {
                ui.text_edit_singleline(&mut f.blurb);
            });
        });
    });
    section(ui, "Stats", true, |ui| {
        let s = &mut f.stats;
        grid(ui, "stats", |ui| {
            num(ui, "weight", "Knockback taken multiplier (lower = heavier)", &mut s.weight, 0.005);
            num(ui, "run speed", "", &mut s.run_speed, 0.05);
            num(ui, "run accel", "", &mut s.run_accel, 0.01);
            num(ui, "ground friction", "", &mut s.ground_friction, 0.005);
            num(ui, "jump speed", "Full hop", &mut s.jump_speed, 0.05);
            num(ui, "short hop", "", &mut s.short_hop_speed, 0.05);
            num(ui, "air jump speed", "", &mut s.air_jump_speed, 0.05);
            num(ui, "air jump drift", "", &mut s.air_jump_drift, 0.05);
            int(ui, "air jumps", "", &mut s.air_jumps);
            num(ui, "air speed", "", &mut s.air_speed, 0.05);
            num(ui, "air accel", "", &mut s.air_accel, 0.01);
            num(ui, "gravity", "", &mut s.gravity, 0.005);
            num(ui, "fall speed", "", &mut s.fall_speed, 0.05);
            num(ui, "fast fall", "", &mut s.fast_fall_speed, 0.05);
        });
    });
    section(ui, "Body & hurtboxes", true, |ui| {
        grid(ui, "body", |ui| {
            num(ui, "half width", "Stage collision", &mut f.body.half_width, 0.5);
            num(ui, "height", "", &mut f.body.height, 0.5);
            for (n, r) in [("hurtbox", &mut f.hurtboxes.default), ("knockdown", &mut f.hurtboxes.knockdown)] {
                row(ui, n, "x1 x2 y1 y2 (x forward, y up)", |ui| {
                    ui.horizontal(|ui| {
                        for v in [&mut r.x1, &mut r.x2, &mut r.y1, &mut r.y2] {
                            ui.add(DragValue::new(v).speed(0.5).max_decimals(1));
                        }
                    });
                });
            }
        });
    });
    ui.label(RichText::new("The preview loops the idle animation with the hurtbox.").color(FAINT).small());
}

/// Live validation errors and the last refused save, each clickable to its place.
fn problems(ui: &mut egui::Ui, ed: &mut Editor) {
    let mut list: Vec<sim::data::ContentError> = ed.ws.docs.get(&ed.cur).map(|d| d.errors.clone()).unwrap_or_default();
    let extra: Vec<_> = ed.problems.iter().filter(|p| !list.contains(p)).cloned().collect();
    list.extend(extra);
    if list.is_empty() {
        return;
    }
    ui.add_space(10.);
    egui::Frame::new().fill(egui::Color32::from_rgb(60, 14, 24)).corner_radius(8).inner_margin(10).show(ui, |ui| {
        ui.label(RichText::new("Problems (fix before saving)").color(RED).strong());
        for e in list {
            let txt = format!("{}: {}", if e.path.is_empty() { &e.file } else { &e.path }, e.msg);
            if ui.link(RichText::new(txt).color(egui::Color32::from_rgb(255, 200, 200))).clicked() {
                goto(ed, &e.path);
            }
        }
    });
}

/// Selects what a JSON path like `moves.jab.phases[1].hitboxes[0].r` points at.
pub fn goto(ed: &mut Editor, path: &str) {
    let parts: Vec<&str> = path.split('.').collect();
    let idx = |s: &str, key: &str| {
        s.strip_prefix(key)
            .and_then(|r| r.strip_prefix('['))
            .and_then(|r| r.split(']').next())
            .and_then(|n| n.parse::<usize>().ok())
    };
    match parts.as_slice() {
        ["moves", slot, rest @ ..] => {
            ed.select(Subject::Move(slot.to_string()));
            if let Some(p) = rest.first().and_then(|s| idx(s, "phases")) {
                ed.phase = p;
                ed.hitbox = rest.get(1).and_then(|s| idx(s, "hitboxes"));
                ed.followed = ed.frame;
            }
        }
        ["throws", k, ..] => ed.select(Subject::Throw(k.to_string())),
        ["poses", n, ..] => ed.select(Subject::Pose(n.to_string())),
        _ => ed.select(Subject::Fighter),
    }
}

/// Current phase on the playhead, for keyboard hitbox operations.
fn cur_phase(ed: &mut Editor) -> Option<(&mut Phase, Option<u32>)> {
    let Subject::Move(slot) = ed.subject.clone() else { return None };
    let t = at_playhead(ed).and_then(|(_, m)| m).filter(|(p, _)| *p == ed.phase).map(|(_, t)| t);
    let ph = ed.phase;
    let mv: &mut Move = move_mut(&mut fighter(ed).moves, &slot)?;
    Some((mv.phases.get_mut(ph)?, t))
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
