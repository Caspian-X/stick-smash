//! The stage editor's browser (sections, platforms, spawns, with dirty / problem / note markers) and inspector
//! (hand-made fields for the geometry, schema forms for art and the backdrop, problems and notes in place).

use bevy_egui::egui::{self, DragValue, RichText};
use serde_json::Value;
use sim::data::ContentError;
use sim::data::stage::Bounds;

use crate::browser::{group_label, item};
use crate::inspect::{errs, grid, mini, num, row, section, under};
use crate::stage_ed::{self, Section};
use crate::stage_view::Part;
use crate::style::{self, DIM, FAINT, GOLD, RED, STAR, faint, heading};
use crate::{Dialog, Editor, form};

fn tip(key: &str) -> String {
    form::doc("stage.Stage", key)
}

/// Saved-vs-edited per top-level key, for dirty markers.
fn dirty_keys(ed: &Editor) -> Vec<String> {
    let Some(d) = ed.ws.stages.get(&ed.st.cur) else { return vec![] };
    let (a, b) =
        (serde_json::to_value(&d.item).unwrap_or_default(), serde_json::to_value(&d.saved).unwrap_or_default());
    let mut keys: Vec<String> = a
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, v)| *k != "art" && b.get(k.as_str()) != Some(*v))
        .map(|(k, _)| k.clone())
        .collect();
    // Art and its backdrop show on their own rows.
    let art = |v: &Value| {
        let mut a = v.get("art").cloned().unwrap_or_default();
        a.as_object_mut().map(|m| m.shift_remove("background"));
        a
    };
    if art(&a) != art(&b) {
        keys.push("art".into());
    }
    if a.pointer("/art/background") != b.pointer("/art/background") {
        keys.push("art.background".into());
    }
    keys
}

fn section_dirty(keys: &[String], s: Section) -> bool {
    s.keys().iter().any(|k| keys.iter().any(|d| d == k))
}

fn bad(list: &[ContentError], s: Section) -> bool {
    s.keys().iter().any(|k| list.iter().any(|e| under(&e.path, k)))
}

pub(crate) fn browser(ui: &mut egui::Ui, ed: &mut Editor) {
    let Some(doc) = ed.ws.stages.get(&ed.st.cur) else { return };
    let s = doc.item.clone();
    let saved = doc.saved.clone();
    let errors = doc.errors.clone();
    let notes = stage_ed::notes(ed);
    let dirty = dirty_keys(ed);
    let mut pick_section = None;
    let mut pick_part: Option<Part> = None;
    ui.add_space(6.);
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            group_label(ui, "Stage");
            let sec = Section::Overview;
            let r = item(ui, ed.st.section == sec, sec.name(), &s.name, section_dirty(&dirty, sec), bad(&errors, sec));
            if r.clicked() {
                pick_section = Some(sec);
            }
            group_label(ui, "Geometry");
            let geo = [
                (Section::Blocks, format!("{}", s.blocks.len())),
                (Section::Platforms, format!("{}", s.platforms.len())),
                (Section::Ledge, format!("r {:.0}", s.ledge.r)),
                (Section::Bounds, String::new()),
            ];
            for (sec, info) in geo {
                let note = bad(&notes, sec) && !bad(&errors, sec);
                let name = if note { format!("{info}  ℹ") } else { info };
                if item(ui, ed.st.section == sec, sec.name(), &name, section_dirty(&dirty, sec), bad(&errors, sec))
                    .clicked()
                {
                    pick_section = Some(sec);
                }
                let rows: Vec<(Part, String, bool)> = match sec {
                    Section::Blocks => s
                        .blocks
                        .iter()
                        .enumerate()
                        .map(|(i, b)| {
                            let ledges = match (b.left_ledge, b.right_ledge) {
                                (true, true) => "",
                                (true, false) => " · ledge L",
                                (false, true) => " · ledge R",
                                (false, false) => " · no ledges",
                            };
                            let txt = format!("{:.0} × {:.0}{ledges}", b.right - b.left, b.bottom - b.top);
                            (Part::Block(i), txt, saved.blocks.get(i) != Some(b))
                        })
                        .collect(),
                    Section::Platforms => s
                        .platforms
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let path = if p.motion.is_some() { "  ⟳ moves" } else { "" };
                            let txt = format!("{:.0}–{:.0} · y {:.0}{path}", p.x1, p.x2, p.y);
                            (Part::Platform(i), txt, saved.platforms.get(i) != Some(p))
                        })
                        .collect(),
                    _ => vec![],
                };
                for (part, txt, ch) in rows {
                    let bad_p = errors.iter().any(|e| under(&e.path, &part.path()));
                    let on = ed
                        .st
                        .sel
                        .is_some_and(|q| q == part || q.platform().is_some() && q.platform() == part.platform());
                    ui.indent(("st-row", part.path()), |ui| {
                        let n = match part {
                            Part::Block(i) | Part::Platform(i) => i + 1,
                            _ => 0,
                        };
                        if item(ui, on, &format!("{n}"), &txt, ch, bad_p).clicked() {
                            pick_part = Some(part);
                        }
                    });
                }
            }
            group_label(ui, "Players");
            for (sec, n) in [(Section::Spawns, s.spawns.len()), (Section::Respawns, s.respawns.len())] {
                let warn = bad(&notes, sec);
                let info = if warn { format!("{n} of 8  ℹ") } else { format!("{n}") };
                if item(ui, ed.st.section == sec, sec.name(), &info, section_dirty(&dirty, sec), bad(&errors, sec))
                    .clicked()
                {
                    pick_section = Some(sec);
                }
            }
            group_label(ui, "Look");
            for sec in [Section::Art, Section::Background] {
                let info = match sec {
                    Section::Background if s.art.background.is_none() => "none",
                    Section::Background => "sky · moon · ridges · stars",
                    _ => "colors · stripes",
                };
                if item(ui, ed.st.section == sec, sec.name(), info, section_dirty(&dirty, sec), bad(&errors, sec))
                    .clicked()
                {
                    pick_section = Some(sec);
                }
            }
            ui.add_space(8.);
        });
    });
    if let Some(p) = pick_part {
        ed.st.select(Some(p));
    }
    if let Some(sec) = pick_section {
        ed.st.section = sec;
        if ed.st.sel.is_some_and(|p| Section::of(p) != sec) {
            ed.st.sel = None;
        }
    }
}

/// Notes (legal but suspicious), gold, in place.
fn note_list(ui: &mut egui::Ui, list: &[ContentError], prefix: &str) {
    for e in list.iter().filter(|e| prefix.is_empty() || under(&e.path, prefix)) {
        egui::Frame::new()
            .fill(GOLD.gamma_multiply(0.12))
            .corner_radius(5)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                let at = if prefix.is_empty() { format!("{}: ", e.path) } else { String::new() };
                ui.add(egui::Label::new(RichText::new(format!("ℹ {at}{}", e.msg)).color(GOLD)).wrap());
            });
    }
}

fn num_cell(ui: &mut egui::Ui, v: &mut f64, tip: &str) {
    ui.add_sized([style::NUM_W, ui.spacing().interact_size.y], DragValue::new(v).speed(1.).max_decimals(1))
        .on_hover_text(tip);
}

/// A card per list item: header with select / duplicate / delete, highlighted when selected.
fn card(
    ui: &mut egui::Ui,
    on: bool,
    title: RichText,
    add: impl FnOnce(&mut egui::Ui),
    act: &mut Option<(Part, &'static str)>,
    part: Part,
) {
    let stroke = if on { egui::Stroke::new(1.5, GOLD) } else { egui::Stroke::new(1., style::LINE) };
    egui::Frame::new()
        .fill(style::CARD)
        .stroke(stroke)
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add(egui::Label::new(title).sense(egui::Sense::click())).on_hover_text("Select").clicked() {
                    *act = Some((part, "sel"));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if mini(ui, style::ICON_DEL, FAINT, "Delete  Del") {
                        *act = Some((part, "del"));
                    }
                    if mini(ui, style::ICON_DUP, FAINT, "Duplicate  ⌘D") {
                        *act = Some((part, "dup"));
                    }
                });
            });
            add(ui);
        });
    ui.add_space(3.);
}

pub(crate) fn inspector(ui: &mut egui::Ui, ed: &mut Editor) {
    let errors = crate::ui::problems(ed);
    let notes = stage_ed::notes(ed);
    let colors = ed.st.data.as_deref().map(stage_ed::colors).unwrap_or_default();
    let mut act: Option<(Part, &'static str)> = None;
    let mut dialog = None;
    let camera = ed.st.data.as_ref().map(|d| d.global.render.camera.clone());
    let before = ed.stage_doc().item.clone();
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.add_space(6.);
        let sec = ed.st.section;
        let sel = ed.st.sel;
        let (id, fresh, path) = {
            let d = ed.stage_doc();
            (d.id.clone(), d.fresh, d.path.clone())
        };
        let s = stage_ed::stage_mut(ed);
        match sec {
            Section::Overview => {
                ui.label(heading(&s.name));
                ui.label(faint(format!("{path}{}", if fresh { " · new, not saved yet" } else { "" })));
                ui.horizontal(|ui| {
                    if ui.button("Rename…").clicked() {
                        dialog = Some(Dialog::RenameStage {
                            id: id.clone(),
                            new_id: id.clone(),
                            name: s.name.clone(),
                            error: None,
                        });
                    }
                    if ui.button(format!("{} Duplicate…", style::ICON_DUP)).clicked() {
                        let name = format!("{} copy", s.name);
                        dialog =
                            Some(Dialog::NewStage { id: crate::doc::slug(&name), name, from: id.clone(), error: None });
                    }
                });
                for k in ["name", "width", "height"] {
                    errs(ui, &errors, k);
                }
                section(ui, "Stage", true, |ui| {
                    grid(ui, "st-ov", |ui| {
                        row(ui, "name", "Shown on the stage select", |ui| drop(ui.text_edit_singleline(&mut s.name)));
                        num(ui, "width", &tip("width"), &mut s.width, 1.);
                        num(ui, "height", &tip("height"), &mut s.height, 1.);
                        row(ui, "centre x", "Symmetry mirrors around width / 2", |ui| {
                            ui.label(RichText::new(format!("{:.1}", s.width / 2.)).color(FAINT));
                        });
                    });
                });
                section(ui, "Players", true, |ui| {
                    ui.label(format!(
                        "Up to {} players: {} spawns, {} respawn points.",
                        s.max_players(),
                        s.spawns.len(),
                        s.respawns.len()
                    ));
                });
                if !notes.is_empty() {
                    ui.add_space(4.);
                    ui.label(style::caps("Notes"));
                    note_list(ui, &notes, "");
                }
            }
            Section::Blocks => {
                ui.horizontal(|ui| {
                    ui.label(heading("Blocks"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+ Add").on_hover_text("A solid block in the middle of the view  B").clicked() {
                            act = Some((Part::Block(usize::MAX), "add"));
                        }
                    });
                });
                ui.label(faint(
                    "Solid ground. Blocks can't overlap (undersides included); touching ones make walls and raised \
                     centres. Each top corner can be a grabbable ledge.",
                ));
                errs(ui, &errors, "blocks");
                note_list(ui, &notes, "blocks");
                let n = s.blocks.len();
                for i in 0..n {
                    let part = Part::Block(i);
                    let on = sel == Some(part);
                    let title = RichText::new(format!("Block {}", i + 1)).color(if on { GOLD } else { STAR });
                    let g = &mut s.blocks[i];
                    let mut lact = None;
                    card(
                        ui,
                        on,
                        title,
                        |ui| {
                            ui.horizontal(|ui| {
                                for (l, v, t) in [
                                    ("l", &mut g.left, "Left wall x"),
                                    ("r", &mut g.right, "Right wall x"),
                                    ("t", &mut g.top, "Top (floor) height; y grows down"),
                                    ("b", &mut g.bottom, "Bottom of the vertical walls"),
                                ] {
                                    ui.label(RichText::new(l).color(DIM));
                                    num_cell(ui, v, t);
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("under").color(DIM));
                                num_cell(ui, &mut g.under_depth, &form::doc("stage.Ground", "underDepth"));
                                num_cell(ui, &mut g.under_inset, &form::doc("stage.Ground", "underInset"));
                                ui.add_space(6.);
                                ui.label(RichText::new("ledges").color(DIM));
                                // Through `toggle_ledge`, so Mirror updates the partner's corner too.
                                if ui.selectable_label(g.left_ledge, "◧ L").on_hover_text("Top-left corner can be grabbed").clicked() {
                                    lact = Some((part, "ledgeL"));
                                }
                                if ui.selectable_label(g.right_ledge, "R ◨").on_hover_text("Top-right corner can be grabbed").clicked() {
                                    lact = Some((part, "ledgeR"));
                                }
                            });
                            errs(ui, &errors, &part.path());
                            note_list(ui, &notes, &part.path());
                        },
                        &mut act,
                        part,
                    );
                    act = lact.or(act);
                }
                ui.add_space(4.);
                ui.label(faint("Drag edges or corners in the viewport; click the teal dots above a selected block's corners to toggle its ledges."));
            }
            Section::Platforms => {
                ui.horizontal(|ui| {
                    ui.label(heading("Platforms"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+ Add").on_hover_text("A platform in the middle of the view  N").clicked() {
                            act = Some((Part::Platform(usize::MAX), "add"));
                        }
                    });
                });
                ui.label(faint("Pass-through: land from above, drop through with down."));
                errs(ui, &errors, "platforms");
                for (i, p) in s.platforms.iter_mut().enumerate() {
                    let part = Part::Platform(i);
                    let on = sel.and_then(Part::platform) == Some(i);
                    let title = RichText::new(format!("Platform {}", i + 1)).color(if on { GOLD } else { STAR });
                    let mut mact = None;
                    card(
                        ui,
                        on,
                        title,
                        |ui| {
                            ui.horizontal(|ui| {
                                for (l, v, t) in [
                                    ("x1", &mut p.x1, "Left end"),
                                    ("x2", &mut p.x2, "Right end"),
                                    ("y", &mut p.y, "Top surface height"),
                                ] {
                                    ui.label(RichText::new(l).color(DIM));
                                    num_cell(ui, v, t);
                                }
                            });
                            motion(ui, i, p, sel, &mut mact);
                            errs(ui, &errors, &part.path());
                            note_list(ui, &notes, &part.path());
                        },
                        &mut act,
                        part,
                    );
                    act = mact.or(act);
                }
                if s.platforms.is_empty() {
                    ui.label(RichText::new("No platforms").color(FAINT));
                }
            }
            Section::Ledge => {
                ui.label(heading("Ledge zone"));
                ui.label(faint(tip_text("LedgeZone")));
                errs(ui, &errors, "ledge");
                let l = &mut s.ledge;
                grid(ui, "st-ledge", |ui| {
                    num(ui, "radius", "Grab radius around each corner", &mut l.r, 0.5);
                    num(ui, "centre below", "The zone's centre sits this far below the corner", &mut l.cy, 0.5);
                    num(ui, "hand height", "The grab point: this far above the feet", &mut l.hand_y, 0.5);
                    num(ui, "reach in", "How far the flat side reaches over the stage", &mut l.reach_in, 0.5);
                    num(ui, "hang x", "Feet while hanging: out from the wall", &mut l.hang_x, 0.5);
                    num(ui, "hang y", "Feet while hanging: below the floor", &mut l.hang_y, 0.5);
                });
                ui.add_space(4.);
                ui.label(faint("Drag the teal rim in the viewport to change the radius."));
            }
            Section::Bounds => {
                ui.label(heading("Blast & camera"));
                for (part, name, b, help) in [
                    (Part::Blast, "Blast zone", &mut s.blast, tip("blast")),
                    (Part::Camera, "Camera bounds", &mut s.camera, tip("camera")),
                ] {
                    let on = sel == Some(part);
                    section(ui, name, true, |ui| {
                        ui.label(faint(help));
                        errs(ui, &errors, &part.path());
                        note_list(ui, &notes, &part.path());
                        bounds(ui, part, b, on);
                    });
                }
                if let Some(c) = &camera {
                    let auto = render::camera::automatic_range(s, c, style::PREVIEW_ASPECT);
                    let effective = render::camera::zoom_range(s, c, style::PREVIEW_ASPECT);
                    ui.label(faint(format!("Automatic (16:9): {:.0}–{:.0} px", auto.min, auto.max)));
                    ui.label(faint(format!("Effective (16:9): {:.0}–{:.0} px", effective.min, effective.max)));
                    ui.label(faint(tip("zoom")));
                    errs(ui, &errors, "zoom");
                    let mut z = s.zoom.clone().unwrap_or_default();
                    let before_zoom = z.clone();
                    grid(ui, "st-zoom", |ui| {
                        optional_zoom(ui, "min height", &mut z.min_height, auto.min, form::doc("stage.Zoom", "minHeight"));
                        optional_zoom(ui, "max height", &mut z.max_height, auto.max, form::doc("stage.Zoom", "maxHeight"));
                    });
                    if z != before_zoom {
                        s.zoom = (z.min_height.is_some() || z.max_height.is_some()).then_some(z);
                    }
                }
            }
            Section::Spawns => {
                ui.horizontal(|ui| {
                    ui.label(heading("Spawns"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+ Add").clicked() {
                            act = Some((Part::Spawn(usize::MAX), "add"));
                        }
                    });
                });
                ui.label(faint(tip("spawns")));
                errs(ui, &errors, "spawns");
                note_list(ui, &notes, "spawns");
                let n = s.spawns.len();
                let floor: Vec<f64> = s.spawns.iter().map(|sp| crate::stage_view::spawn_y(s, sp)).collect();
                for (i, &floor) in floor.iter().enumerate().take(n) {
                    let part = Part::Spawn(i);
                    let col = style::rgb(colors.get(i).copied().unwrap_or(0xffffff));
                    let on = sel == Some(part);
                    let title = RichText::new(format!("P{}", i + 1)).family(style::label()).size(15.).color(if on {
                        GOLD
                    } else {
                        col
                    });
                    let sp = &mut s.spawns[i];
                    card(
                        ui,
                        on,
                        title,
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("x").color(DIM));
                                num_cell(ui, &mut sp.x, "Feet x");
                                ui.label(RichText::new("y").color(DIM));
                                match &mut sp.y {
                                    Some(y) => {
                                        num_cell(ui, y, "Feet height");
                                        if mini(ui, style::ICON_DEL, FAINT, "Stand on the ground instead") {
                                            sp.y = None;
                                        }
                                    }
                                    None => {
                                        ui.label(RichText::new("ground").color(FAINT));
                                        if mini(ui, "+ set", GOLD, "Start at a height of its own (e.g. on a platform)")
                                        {
                                            sp.y = Some(floor);
                                        }
                                    }
                                }
                                let arrow = if sp.face > 0 { "→ right" } else { "← left" };
                                if ui.button(arrow).on_hover_text("Facing (click to flip)").clicked() {
                                    sp.face = -sp.face;
                                }
                            });
                            errs(ui, &errors, &part.path());
                            note_list(ui, &notes, &part.path());
                        },
                        &mut act,
                        part,
                    );
                }
            }
            Section::Respawns => {
                ui.horizontal(|ui| {
                    ui.label(heading("Respawns"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+ Add").clicked() {
                            act = Some((Part::Respawn(usize::MAX), "add"));
                        }
                    });
                });
                ui.label(faint(tip("respawns")));
                ui.label(faint(tip_text("RespawnPoint")));
                errs(ui, &errors, "respawns");
                note_list(ui, &notes, "respawns");
                for (i, r) in s.respawns.iter_mut().enumerate() {
                    let part = Part::Respawn(i);
                    let col = style::rgb(colors.get(i).copied().unwrap_or(0xffffff));
                    let on = sel == Some(part);
                    let title = RichText::new(format!("R{}", i + 1)).family(style::label()).size(15.).color(if on {
                        GOLD
                    } else {
                        col
                    });
                    card(
                        ui,
                        on,
                        title,
                        |ui| {
                            ui.horizontal(|ui| {
                                for (l, v, t) in [
                                    ("x", &mut r.x, "Column x"),
                                    ("from", &mut r.start_y, "Starts descending here"),
                                    ("to", &mut r.end_y, "Waits on the respawn platform here"),
                                ] {
                                    ui.label(RichText::new(l).color(DIM));
                                    num_cell(ui, v, t);
                                }
                            });
                            errs(ui, &errors, &part.path());
                            note_list(ui, &notes, &part.path());
                        },
                        &mut act,
                        part,
                    );
                }
            }
            Section::Art => {
                ui.label(heading("Art"));
                ui.label(faint("Colors of the ground and platforms. Hover a field for help."));
                errs(ui, &errors, "art");
                let mut def = form::def("stage.StageArt").clone();
                if let Some(p) = def.get_mut("properties").and_then(Value::as_object_mut) {
                    p.shift_remove("background");
                    p.shift_remove("stripes");
                }
                let (bg, stripes) = (s.art.background.take(), s.art.stripes.take());
                section(ui, "Ground & platforms", true, |ui| {
                    form::edit(ui, egui::Id::new(("st-art", &id)), &def, &mut s.art);
                });
                (s.art.background, s.art.stripes) = (bg, stripes);
                section(ui, "Stripes", true, |ui| {
                    ui.label(faint(tip_text("Stripes")));
                    optional_form(ui, egui::Id::new(("st-stripes", &id)), "Stripes", &mut s.art.stripes, "no stripes");
                });
            }
            Section::Background => {
                ui.label(heading("Background"));
                ui.label(faint("Procedural backdrop filling the camera bounds: sky gradient, moon, ridges, stars."));
                errs(ui, &errors, "art.background");
                match &mut s.art.background {
                    Some(bg) => {
                        let bid = egui::Id::new(("st-bg", &id));
                        let bdef = form::def("stage.Background");
                        let prop = |k: &str| bdef.pointer(&format!("/properties/{k}")).cloned().unwrap_or_default();
                        section(ui, "Sky", true, |ui| {
                            ui.label(faint(form::doc("stage.Background", "sky")));
                            form::edit(ui, bid.with("sky"), &prop("sky"), &mut bg.sky);
                        });
                        section(ui, "Moon", true, |ui| {
                            optional_form(ui, bid.with("moon"), "Moon", &mut bg.moon, "no moon")
                        });
                        section(ui, "Ridges", true, |ui| {
                            ui.label(faint(tip_text("Ridge")));
                            form::edit(ui, bid.with("ridges"), &form::array_of("stage.Ridge"), &mut bg.ridges);
                        });
                        section(ui, "Stars", true, |ui| {
                            ui.label(faint(tip_text("Starfield")));
                            optional_form(ui, bid.with("stars"), "Starfield", &mut bg.stars, "no stars");
                        });
                        section(ui, "Parallax", true, |ui| {
                            ui.label(faint(form::doc("stage.Background", "parallax")));
                            ui.add(egui::Slider::new(&mut bg.parallax, 0.0..=1.0));
                        });
                        ui.add_space(6.);
                        if ui.button(RichText::new("Remove backdrop").color(RED)).clicked() {
                            s.art.background = None;
                        }
                    }
                    None => {
                        ui.label(RichText::new("No backdrop: the stage floats on the clear color.").color(FAINT));
                        if ui.button("+ Add backdrop").clicked() {
                            s.art.background =
                                serde_json::from_value(form::template(form::def("stage.Background"))).ok();
                        }
                    }
                }
            }
        }
        problems(ui, ed, &errors);
    });
    if let Some(d) = dialog {
        ed.dialog = Some(d);
    }
    // With Mirror on, a path edited here (mode, phase, waypoints) carries over to the partner platform.
    if ed.st.symmetry {
        let s = stage_ed::stage_mut(ed);
        for i in 0..s.platforms.len().min(before.platforms.len()) {
            if s.platforms[i].motion != before.platforms[i].motion
                && let Some(q @ Part::Platform(k)) = crate::stage_view::partner(&before, Part::Platform(i))
                && k != i
            {
                crate::stage_view::apply_mirror(s, Part::Platform(i), q, crate::stage_view::Grip::Move);
            }
        }
    }
    match act {
        Some((p, "sel")) => ed.st.select(Some(p)),
        Some((p, "del")) => {
            ed.st.select(Some(p));
            stage_ed::delete_selected(ed);
        }
        Some((p, "dup")) => {
            ed.st.select(Some(p));
            stage_ed::duplicate_selected(ed);
        }
        Some((Part::Platform(_), "add")) => stage_ed::add_platform(ed),
        Some((Part::Block(_), "add")) => stage_ed::add_block(ed),
        Some((Part::Block(i), "ledgeL")) => stage_ed::toggle_ledge(ed, i, false),
        Some((Part::Block(i), "ledgeR")) => stage_ed::toggle_ledge(ed, i, true),
        Some((Part::Platform(i), "path")) => {
            ed.st.select(Some(Part::Platform(i)));
            stage_ed::add_waypoint(ed);
        }
        Some((Part::Spawn(_), "add")) => stage_ed::add_spawn(ed, false),
        Some((Part::Respawn(_), "add")) => stage_ed::add_spawn(ed, true),
        _ => {}
    }
}

/// An optional object edited through its schema form, with add / remove.
fn optional_form<T: serde::Serialize + serde::de::DeserializeOwned>(
    ui: &mut egui::Ui,
    id: egui::Id,
    def: &str,
    v: &mut Option<T>,
    unset: &str,
) {
    let def = form::def(&format!("stage.{def}"));
    match v {
        Some(x) => {
            form::edit(ui, id, def, x);
            if ui.small_button(format!("{} Remove", style::ICON_DEL)).clicked() {
                *v = None;
            }
        }
        None => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("— {unset}")).color(FAINT));
                if mini(ui, "+ add", GOLD, "Add one, then tune it") {
                    *v = serde_json::from_value(form::template(def)).ok();
                }
            });
        }
    }
}

/// A platform's path: mode, phase and its waypoints (offset, travel, wait), or a button to give it one.
fn motion(
    ui: &mut egui::Ui,
    i: usize,
    p: &mut sim::data::stage::Platform,
    sel: Option<Part>,
    act: &mut Option<(Part, &'static str)>,
) {
    use sim::data::stage::PathMode;
    let Some(m) = &mut p.motion else {
        if mini(ui, "+ make it move", GOLD, "Give it a waypoint path (moving platform)") {
            *act = Some((Part::Platform(i), "path"));
        }
        return;
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new("path").color(DIM));
        egui::ComboBox::from_id_salt(("st-mode", i))
            .selected_text(if m.mode == PathMode::Loop { "loop" } else { "ping-pong" })
            .width(84.)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut m.mode, PathMode::PingPong, "ping-pong")
                    .on_hover_text("Back and forth, reversing at the ends");
                ui.selectable_value(&mut m.mode, PathMode::Loop, "loop")
                    .on_hover_text("The last waypoint travels back to the first");
            });
        ui.label(RichText::new("phase").color(DIM));
        ui.add(DragValue::new(&mut m.phase).speed(1.).suffix(" f")).on_hover_text(form::doc("stage.Motion", "phase"));
        ui.label(faint(format!("cycle {} f", crate::stage_view::cycle(m))));
    });
    let n = m.waypoints.len();
    let mut del = None;
    egui::Grid::new(("st-wps", i)).num_columns(6).spacing([6., 3.]).show(ui, |ui| {
        for t in ["", "x", "y", "travel", "wait", ""] {
            ui.label(RichText::new(t).color(FAINT).small());
        }
        ui.end_row();
        for (j, w) in m.waypoints.iter_mut().enumerate() {
            let on = sel == Some(Part::Waypoint(i, j));
            if ui
                .add(
                    egui::Label::new(RichText::new(format!("{}", j + 1)).color(if on { GOLD } else { DIM }))
                        .sense(egui::Sense::click()),
                )
                .on_hover_text("Select this waypoint")
                .clicked()
            {
                *act = Some((Part::Waypoint(i, j), "sel"));
            }
            num_cell(ui, &mut w.x, "Offset from the platform's base x");
            num_cell(ui, &mut w.y, "Offset from the platform's base y (positive is down)");
            ui.add_sized([style::NUM_W, 20.], DragValue::new(&mut w.travel_ticks).speed(1.).suffix(" f"))
                .on_hover_text(form::doc("stage.Waypoint", "travelTicks"));
            ui.add_sized([style::NUM_W, 20.], DragValue::new(&mut w.wait_ticks).speed(1.).suffix(" f"))
                .on_hover_text(form::doc("stage.Waypoint", "waitTicks"));
            if n > 2 && mini(ui, style::ICON_DEL, FAINT, "Remove this waypoint") {
                del = Some(j);
            }
            ui.end_row();
        }
    });
    if let Some(j) = del {
        m.waypoints.remove(j);
    }
    ui.horizontal(|ui| {
        if mini(ui, "+ waypoint", GOLD, "Another stop, after the last one") {
            *act = Some((Part::Platform(i), "path"));
        }
        if mini(ui, "stop moving", FAINT, "Remove the path (the platform stays at its base)") {
            p.motion = None;
        }
    });
}

fn tip_text(def: &str) -> String {
    let d = form::def(&format!("stage.{def}"));
    d.get("description").and_then(Value::as_str).unwrap_or_default().replace('`', "")
}

fn bounds(ui: &mut egui::Ui, part: Part, b: &mut Bounds, on: bool) {
    let frame = if on { egui::Stroke::new(1.5, GOLD) } else { egui::Stroke::NONE };
    egui::Frame::new().stroke(frame).corner_radius(6).inner_margin(egui::Margin::symmetric(4, 3)).show(ui, |ui| {
        grid(ui, ("st-b", part.path()), |ui| {
            num(ui, "left", "", &mut b.left, 1.);
            num(ui, "right", "", &mut b.right, 1.);
            num(ui, "top", "", &mut b.top, 1.);
            num(ui, "bottom", "", &mut b.bottom, 1.);
        });
        ui.label(faint(format!("{:.0} × {:.0}", b.right - b.left, b.bottom - b.top)));
    });
}

fn problems(ui: &mut egui::Ui, ed: &mut Editor, list: &[ContentError]) {
    if list.is_empty() {
        return;
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
    if let Some(p) = go {
        goto(ed, &p);
    }
}

/// Selects what a JSON path like `platforms[1].x1` points at.
pub(crate) fn goto(ed: &mut Editor, path: &str) {
    let idx = |key: &str| {
        path.strip_prefix(key)
            .and_then(|r| r.strip_prefix('['))
            .and_then(|r| r.split(']').next())
            .and_then(|n| n.parse::<usize>().ok())
    };
    let part = match path.split(['.', '[']).next().unwrap_or("") {
        "blocks" => idx("blocks").map(Part::Block),
        "platforms" => idx("platforms").map(|i| {
            let wp = path.split("waypoints[").nth(1).and_then(|r| r.split(']').next()).and_then(|n| n.parse().ok());
            wp.map_or(Part::Platform(i), |j| Part::Waypoint(i, j))
        }),
        "spawns" => idx("spawns").map(Part::Spawn),
        "respawns" => idx("respawns").map(Part::Respawn),
        "ledge" => Some(Part::Ledge),
        "blast" => Some(Part::Blast),
        "camera" => Some(Part::Camera),
        _ => None,
    };
    match part {
        Some(p) => ed.st.select(Some(p)),
        None => {
            ed.st.sel = None;
            ed.st.section = match path {
                p if p.starts_with("art.background") => Section::Background,
                p if p.starts_with("art") => Section::Art,
                p if p.starts_with("spawns") => Section::Spawns,
                p if p.starts_with("respawns") => Section::Respawns,
                p if p.starts_with("platforms") => Section::Platforms,
                p if p.starts_with("blocks") => Section::Blocks,
                p if p.starts_with("zoom") => Section::Bounds,
                _ => Section::Overview,
            };
        }
    }
}

fn optional_zoom(ui: &mut egui::Ui, label: &str, v: &mut Option<f64>, auto: f64, help: String) {
    ui.label(label).on_hover_text(&help);
    ui.horizontal(|ui| match v {
        Some(n) => {
            num_cell(ui, n, &help);
            if mini(ui, style::ICON_DEL, FAINT, "Use automatic height") {
                *v = None;
            }
        }
        None => {
            ui.label(RichText::new("— auto").color(FAINT));
            if mini(ui, "+ set", GOLD, &help) {
                *v = Some(auto);
            }
        }
    });
    ui.end_row();
}
