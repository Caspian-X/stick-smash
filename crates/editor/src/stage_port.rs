//! The stage viewport: pan/zoom over the real rendered stage, direct manipulation of its parts (snapping and
//! symmetry), slot labels, the overlay, and the floating toolbars (view, add, sandbox).

use bevy_egui::egui::{self, Align2, Color32, PointerButton, Rect, RichText, Sense, UiBuilder};
use sim::data::render::CPU_LEVELS;

use crate::stage_ed::{self, Drag, GRIDS, SANDBOX_MAX};
use crate::stage_view::{self, Grip, Layers, Part};
use crate::style::{self, DIM, FAINT, GOLD, INK, NIGHT, STAR, caps};
use crate::ui::hint;
use crate::{Editor, view};

fn layers(ed: &Editor) -> Layers {
    Layers { bounds: ed.st.show_bounds, spawns: ed.st.show_spawns }
}

pub(crate) fn overlay(ctx: &egui::Context, ed: &mut Editor, scene: &mut render::Scene) {
    let ppp = f64::from(ctx.pixels_per_point());
    let px = 1. / ed.st.cam.zoom;
    let Some(data) = ed.st.data.clone() else { return };
    scene.overlay.clear(ed.st.cam.zoom * ppp, data.global.render.alpha_gamma);
    let Some(doc) = ed.ws.stages.get(&ed.st.cur) else { return };
    let [l, t, r, b] = ed.port.rect;
    let a = ed.st.cam.to_stage(&ed.port, [l, t]);
    let z = ed.st.cam.to_stage(&ed.port, [r, b]);
    let colors = stage_ed::colors(&data);
    let o = stage_view::Overlay {
        s: &doc.item,
        sel: ed.st.sel,
        hover: ed.st.hover,
        layers: layers(ed),
        grid: ed.st.grid.then_some(ed.st.grid_size),
        view: [a[0], a[1], z[0], z[1]],
        symmetry: ed.st.symmetry,
        colors: &colors,
        zoom_view: Some(
            render::fx::max_zoom_view(&doc.item, &data.global.render.camera, style::PREVIEW_ASPECT)
                .view(style::PREVIEW_ASPECT),
        ),
    };
    stage_view::draw(&mut scene.overlay, &o, px);
}

pub(crate) fn viewport(ui: &mut egui::Ui, ed: &mut Editor) {
    let rect = ui.max_rect();
    let win = ui.ctx().viewport_rect();
    ed.port = view::Port {
        rect: [rect.left(), rect.top(), rect.right(), rect.bottom()].map(f64::from),
        window: [f64::from(win.width()), f64::from(win.height())],
    };
    if ed.st.reframe {
        ed.st.reframe = false;
        stage_ed::frame_stage(ed);
    }
    // Picking a bound (from the browser or Tab) zooms out to show it whole.
    if ed.st.sel != ed.st.seen_sel {
        ed.st.seen_sel = ed.st.sel;
        if let Some(p @ (Part::Blast | Part::Camera)) = ed.st.sel
            && ed.st.drag.is_none()
            && let Some(stage_view::Shape::Rect(r)) =
                ed.ws.stages.get(&ed.st.cur).and_then(|d| stage_view::shape(&d.item, p))
        {
            stage_ed::frame_rect(ed, r);
        }
    }
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let to_stage = |ed: &Editor, p: egui::Pos2| ed.st.cam.to_stage(&ed.port, [f64::from(p.x), f64::from(p.y)]);
    let px = 1. / ed.st.cam.zoom;
    let (coarse, fine) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
    let sn = ed.st.snap(coarse, fine);
    let Some(stage) = ed.ws.stages.get(&ed.st.cur).map(|d| d.item.clone()) else { return };
    let mouse = resp.hover_pos().or(resp.interact_pointer_pos());
    if ed.st.drag.is_none() {
        ed.st.hover = mouse
            .filter(|_| resp.hovered())
            .and_then(|m| stage_view::hit_test(&stage, ed.st.sel, to_stage(ed, m), px, layers(ed)));
    }
    if resp.hovered() {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let k = f64::from(pinch) * (f64::from(scroll) * 0.0025).exp();
        if (k - 1.).abs() > 1e-4
            && let Some(m) = mouse
        {
            let port = ed.port;
            ed.st.cam.zoom_at(&port, [f64::from(m.x), f64::from(m.y)], k);
            ed.st.cam.zoom = ed.st.cam.zoom.max(0.1);
        }
    }
    if resp.drag_started_by(PointerButton::Secondary) || resp.drag_started_by(PointerButton::Middle) {
        ed.st.drag = Some(Drag::Pan);
    } else if resp.drag_started_by(PointerButton::Primary) {
        let grab = resp.interact_pointer_pos().map(|p| to_stage(ed, p)).unwrap_or_default();
        ed.st.drag = Some(match ed.st.hover {
            Some(h) => {
                ed.st.select(Some(h.part));
                let partner = if ed.st.symmetry { stage_view::partner(&stage, h.part) } else { None };
                Drag::Part { h, orig: Box::new(stage.clone()), grab, partner }
            }
            None => Drag::Pan,
        });
    }
    if resp.dragged()
        && let Some(drag) = ed.st.drag.clone()
    {
        match &drag {
            Drag::Pan => {
                let d = resp.drag_delta();
                ed.st.cam.center[0] -= f64::from(d.x) * px;
                ed.st.cam.center[1] -= f64::from(d.y) * px;
            }
            Drag::Part { .. } => {
                if let Some(m) = resp.interact_pointer_pos().map(|p| to_stage(ed, p))
                    && stage_ed::drag_to(ed, &drag, m, sn)
                {
                    ed.st.stale = true;
                }
            }
        }
    }
    if resp.drag_stopped() {
        ed.st.drag = None;
    }
    if resp.clicked() {
        match ed.st.hover {
            Some(h) => {
                ed.st.select(Some(h.part));
                match (h.grip, h.part) {
                    (Grip::Face, Part::Spawn(i)) => stage_ed::flip(ed, i),
                    (Grip::Ledge(right), Part::Block(i)) => stage_ed::toggle_ledge(ed, i, right),
                    _ => {}
                }
            }
            None => ed.st.sel = None,
        }
    }
    if resp.double_clicked() && ed.st.hover.is_none() {
        ed.st.reframe = true;
    }
    let cursor = match (&ed.st.drag, ed.st.hover.map(|h| h.grip)) {
        (Some(_), _) => egui::CursorIcon::Grabbing,
        (_, Some(Grip::Sides([l, t, r, b]))) => match (l || r, t || b) {
            (true, true) if l == t => egui::CursorIcon::ResizeNwSe,
            (true, true) => egui::CursorIcon::ResizeNeSw,
            (true, false) => egui::CursorIcon::ResizeHorizontal,
            _ => egui::CursorIcon::ResizeVertical,
        },
        (_, Some(Grip::End(_))) => egui::CursorIcon::ResizeHorizontal,
        (_, Some(Grip::Start)) => egui::CursorIcon::ResizeVertical,
        (_, Some(Grip::Face | Grip::Ledge(_))) => egui::CursorIcon::PointingHand,
        (_, Some(Grip::Rim)) => egui::CursorIcon::ResizeHorizontal,
        (_, Some(Grip::Move)) => egui::CursorIcon::Move,
        _ => egui::CursorIcon::Default,
    };
    if resp.hovered() || ed.st.drag.is_some() {
        ui.ctx().set_cursor_icon(cursor);
    }
    labels(ui, ed, &stage, rect);
    chrome(ui, ed, rect, &stage);
}

/// Slot numbers on spawns (`1`…) and respawns (`R1`…), in the player's color.
fn labels(ui: &egui::Ui, ed: &Editor, s: &sim::data::Stage, rect: Rect) {
    if !ed.st.show_spawns {
        return;
    }
    let px = 1. / ed.st.cam.zoom;
    let colors = ed.st.data.as_deref().map(stage_ed::colors).unwrap_or_default();
    let painter = ui.painter_at(rect);
    let parts = (0..s.spawns.len()).map(Part::Spawn).chain((0..s.respawns.len()).map(Part::Respawn));
    for p in parts {
        let Some(at) = stage_view::label_at(s, p, px) else { continue };
        let [x, y] = ed.st.cam.to_screen(&ed.port, at);
        let (i, text) = match p {
            Part::Spawn(i) => (i, format!("{}", i + 1)),
            Part::Respawn(i) => (i, format!("R{}", i + 1)),
            _ => continue,
        };
        let col = style::rgb(colors.get(i).copied().unwrap_or(0xffffff));
        let on = ed.st.sel == Some(p);
        let font = egui::FontId::new(if on { 15. } else { 13. }, style::label());
        let pos = egui::pos2(x as f32, y as f32);
        painter.text(pos + egui::vec2(0., 1.), Align2::CENTER_CENTER, &text, font.clone(), NIGHT);
        painter.text(pos, Align2::CENTER_CENTER, text, font, if on { GOLD } else { col });
    }
}

fn pill(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(INK.gamma_multiply(0.88))
        .stroke(egui::Stroke::new(1., style::LINE.gamma_multiply(0.6)))
        .corner_radius(7)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| ui.horizontal(add));
}

fn chrome(ui: &mut egui::Ui, ed: &mut Editor, rect: Rect, stage: &sim::data::Stage) {
    let bar = Rect::from_min_max(rect.min + egui::vec2(10., 10.), rect.max - egui::vec2(10., 10.));
    // The view pill, then the add pill beside it or (narrow viewports) below; the sandbox sits above the hint line.
    let wide = bar.width() > 680.;
    let used = ui.scope_builder(UiBuilder::new().max_rect(bar), |ui| {
        ui.horizontal(|ui| {
            pill(ui, |ui| {
                let st = &mut ed.st;
                ui.toggle_value(&mut st.show_bounds, "Bounds").on_hover_text("Blast zone and camera bounds  B");
                ui.toggle_value(&mut st.show_spawns, "Spawns").on_hover_text("Spawn and respawn points  P");
                ui.separator();
                ui.toggle_value(&mut st.grid, "Grid").on_hover_text("Show the grid and snap to it  G (⇧ snaps anyway)");
                egui::ComboBox::from_id_salt("st-grid")
                    .selected_text(format!("{:.0}", st.grid_size))
                    .width(38.)
                    .show_ui(ui, |ui| {
                        for g in GRIDS {
                            ui.selectable_value(&mut st.grid_size, g, format!("{g:.0} px"));
                        }
                    })
                    .response
                    .on_hover_text("Grid size");
                ui.toggle_value(&mut st.symmetry, "⇋ Mirror")
                    .on_hover_text("Symmetry: edits mirror around the stage's centre x  M");
                ui.separator();
                if ui.button("⛶").on_hover_text("Frame the stage  F · double-click").clicked() {
                    st.reframe = true;
                }
                ui.label(RichText::new(format!("{:.0}%", st.cam.zoom * 100.)).color(FAINT).small());
            });
            if wide {
                add_pill(ui, ed);
            }
        });
        if !wide {
            add_pill(ui, ed);
        }
    });
    let sand = Rect::from_min_size(rect.left_bottom() + egui::vec2(10., -68.), egui::vec2(rect.width() - 20., 30.));
    ui.scope_builder(UiBuilder::new().max_rect(sand), |ui| {
        ui.horizontal(|ui| {
            pill(ui, |ui| sandbox_bar(ui, ed));
            if stage.platforms.iter().any(|p| p.motion.is_some()) {
                pill(ui, |ui| clock_bar(ui, ed, stage));
            }
        });
    });
    let errors = ed.ws.stages.get(&ed.st.cur).map(|d| d.errors.clone()).unwrap_or_default();
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
                    let txt = format!("⚠ Preview paused on the last valid stage · {}: {}", e.path, e.msg);
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
            crate::stage_panels::goto(ed, &e.path);
        }
    }
    let shown = ed.st.drag.as_ref().and_then(|d| match d {
        Drag::Part { h, .. } => Some(h.part),
        Drag::Pan => None,
    });
    let hints: &[(&str, &str)] = match (ed.st.drag.is_some(), ed.st.hover.map(|h| h.grip)) {
        (true, _) if shown.is_some() => &[("⇧", "snap to grid"), ("Alt", "fine")],
        (true, _) => &[],
        (_, Some(Grip::Face)) => &[("click", "flip facing")],
        (_, Some(Grip::Start)) => &[("drag", "where the respawn starts")],
        (_, Some(Grip::Rim)) => &[("drag", "grab radius")],
        (_, Some(Grip::Move)) => &[("click", "select"), ("drag", "move")],
        (_, Some(_)) => &[("drag", "resize"), ("⇧", "snap"), ("Alt", "fine")],
        _ if matches!(
            ed.st.sel,
            Some(Part::Block(_) | Part::Platform(_) | Part::Waypoint(..) | Part::Spawn(_) | Part::Respawn(_))
        ) =>
        {
            &[("Alt ←→↑↓", "nudge"), ("⌘D", "duplicate"), ("Del", "delete")]
        }
        _ if ed.st.sel.is_some() => &[("drag", "edges and corners"), ("Alt ←→↑↓", "nudge")],
        _ => &[
            ("click", "select"),
            ("N", "new platform"),
            ("wheel", "zoom"),
            ("right-drag", "pan"),
            ("?", "all shortcuts"),
        ],
    };
    let hint_rect =
        Rect::from_min_size(rect.left_bottom() + egui::vec2(12., -28.), egui::vec2(rect.width() - 24., 22.));
    let part = shown.or(ed.st.hover.map(|h| h.part)).or(ed.st.sel);
    ui.scope_builder(UiBuilder::new().max_rect(hint_rect), |ui| {
        ui.set_clip_rect(hint_rect.intersect(ui.clip_rect()));
        ui.horizontal(|ui| {
            for (k, v) in hints {
                hint(ui, k, v);
                ui.add_space(6.);
            }
            if let Some(p) = part {
                ui.label(RichText::new(p.label()).color(STAR));
                ui.label(RichText::new(stage_ed::readout(stage, p)).color(DIM));
                if ed.st.symmetry
                    && let Some(q) = stage_view::partner(stage, p)
                {
                    let t = if q == p { "centred".to_string() } else { format!("mirrors {}", q.label()) };
                    ui.label(RichText::new(format!("⇋ {t}")).color(GOLD));
                }
            }
        });
    });
}

fn add_pill(ui: &mut egui::Ui, ed: &mut Editor) {
    pill(ui, |ui| {
        ui.label(caps("Add"));
        if ui.button("Block").on_hover_text("A solid block in the middle of the view  B").clicked() {
            stage_ed::add_block(ed);
        }
        if ui.button("Platform").on_hover_text("A platform in the middle of the view  N").clicked() {
            stage_ed::add_platform(ed);
        }
        let plat = ed.st.sel.and_then(Part::platform).is_some();
        if ui
            .add_enabled(plat, egui::Button::new("Waypoint"))
            .on_hover_text("Make the selected platform move, or add a stop to its path  W")
            .on_disabled_hover_text("Select a platform to give it a path")
            .clicked()
        {
            stage_ed::add_waypoint(ed);
        }
        if ui.button("Spawn").on_hover_text("Another start position (one per player)").clicked() {
            stage_ed::add_spawn(ed, false);
        }
        if ui.button("Respawn").on_hover_text("Another respawn point (one per player)").clicked() {
            stage_ed::add_spawn(ed, true);
        }
    });
}

/// Play / scrub the moving platforms' clock (shown when the stage has any).
fn clock_bar(ui: &mut egui::Ui, ed: &mut Editor, stage: &sim::data::Stage) {
    let cycle = stage.platforms.iter().filter_map(|p| p.motion.as_ref()).map(stage_view::cycle).max().unwrap_or(0);
    let Some(frame) = ed.st.world.as_ref().map(|w| w.frame) else { return };
    ui.label(caps("Motion"));
    let running = ed.st.sandbox > 0 && !ed.st.paused;
    let icon = if ed.st.clock_play || running { "⏸" } else { "▶" };
    if ui
        .add_enabled(ed.st.sandbox == 0, egui::Button::new(icon))
        .on_hover_text("Play the moving platforms  T")
        .clicked()
    {
        ed.st.clock_play = !ed.st.clock_play;
    }
    let mut t = frame % cycle.max(1);
    let r = ui.add_enabled(
        cycle > 0 && ed.st.sandbox == 0,
        egui::Slider::new(&mut t, 0..=cycle.saturating_sub(1)).suffix(" f"),
    );
    // With CPUs on, the clock belongs to the sim: scrubbing would move platforms out from under riders.
    let r = r.on_disabled_hover_text("Turn the sandbox off to scrub");
    if r.on_hover_text("Scrub the platforms' cycle (world frame)").changed() {
        ed.st.scrub = Some(frame - frame % cycle.max(1) + t);
    }
}

fn sandbox_bar(ui: &mut egui::Ui, ed: &mut Editor) {
    ui.label(caps("Sandbox"));
    let mut n = ed.st.sandbox;
    ui.selectable_value(&mut n, 0, "Off").on_hover_text("Just the stage");
    for k in 1..=SANDBOX_MAX {
        ui.selectable_value(&mut n, k, format!("{k}"))
            .on_hover_text(format!("{k} CPU{} playing on the stage (real sim)", if k == 1 { "" } else { "s" }));
    }
    ed.st.set_sandbox(n);
    if n > 0 {
        let mut lv = ed.st.level;
        egui::ComboBox::from_id_salt("st-level")
            .selected_text(CPU_LEVELS[lv as usize % CPU_LEVELS.len()])
            .width(64.)
            .show_ui(ui, |ui| {
                for (i, l) in CPU_LEVELS.iter().enumerate() {
                    ui.selectable_value(&mut lv, i as u8, *l);
                }
            });
        if lv != ed.st.level {
            ed.st.level = lv;
            ed.st.world = None;
            ed.st.stale = true;
        }
        let icon = if ed.st.paused { "▶" } else { "⏸" };
        ui.toggle_value(&mut ed.st.paused, icon).on_hover_text("Pause the CPUs  Space");
        if ui.button("⟲").on_hover_text("Restart the sandbox").clicked() {
            ed.st.world = None;
            ed.st.stale = true;
        }
    }
}
