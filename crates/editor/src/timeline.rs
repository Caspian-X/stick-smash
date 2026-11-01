//! The timeline: transport (play, step, loop, speed), the frame readout, and the track with phase bands, hitbox
//! active windows, event ranges and pose keys. Click or drag anywhere on it to scrub.

use bevy_egui::egui::{self, Align2, FontId, Rect, RichText, Sense, Stroke};

use crate::preview::Subject;
use crate::style::{self, DIM, FAINT, GOLD, LINE, PHASE_COLORS, RED, STAR};
use crate::ui::{badge, toggle_play};
use crate::{Editor, move_ref};

pub(crate) fn jump_phase(ed: &mut Editor, fwd: bool) {
    let Some(r) = &ed.run else { return };
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

fn tbutton(ui: &mut egui::Ui, text: &str, tip: &str) -> bool {
    ui.add(egui::Button::new(RichText::new(text).size(14.)).min_size(egui::vec2(30., 24.))).on_hover_text(tip).clicked()
}

pub(crate) fn timeline(ui: &mut egui::Ui, ed: &mut Editor) {
    ui.add_space(6.);
    if let Subject::Pose(name) = &ed.subject {
        let mut job = egui::text::LayoutJob::default();
        job.append(&format!("Pose {name}  "), 0., egui::TextFormat::simple(FontId::new(17., style::label()), STAR));
        job.append(
            "is a single still frame: drag its joints in the viewport, or open a move and key it with K.",
            0.,
            egui::TextFormat::simple(style::text(13.5), DIM),
        );
        ui.add(egui::Label::new(job).wrap());
        ui.add_space(8.);
        return;
    }
    let n = ed.frames();
    ui.horizontal(|ui| {
        if tbutton(ui, "⏮", "First frame  Home") {
            ed.set_frame(0);
        }
        if tbutton(ui, "⏪", "Previous phase  [") {
            jump_phase(ed, false);
        }
        if tbutton(ui, "◀", "Previous frame  ←  (⇧ ×10)") {
            ed.set_frame(ed.frame.saturating_sub(1));
        }
        let play = if ed.playing { "⏸" } else { "▶" };
        let pb = egui::Button::new(RichText::new(play).size(16.).color(style::NIGHT))
            .fill(GOLD)
            .min_size(egui::vec2(40., 24.));
        if ui.add(pb).on_hover_text("Play / pause  Space").clicked() {
            toggle_play(ed);
        }
        if tbutton(ui, "▶", "Next frame  →  (⇧ ×10)") {
            ed.set_frame(ed.frame + 1);
        }
        if tbutton(ui, "⏩", "Next phase  ]") {
            jump_phase(ed, true);
        }
        if tbutton(ui, "⏭", "Last frame  End") {
            ed.set_frame(n);
        }
        ui.separator();
        ui.toggle_value(&mut ed.looping, "🔁").on_hover_text("Loop playback  L");
        for (s, l) in [(1., "1×"), (0.5, "½×"), (0.25, "¼×")] {
            ui.selectable_value(&mut ed.speed, s, l).on_hover_text("Playback speed");
        }
        ui.separator();
        readout(ui, ed, n);
    });
    ui.add_space(4.);
    track(ui, ed);
    ui.add_space(6.);
}

/// Frame number, phase and phase frame, and what's live on this frame.
fn readout(ui: &mut egui::Ui, ed: &Editor, n: usize) {
    ui.label(RichText::new(format!("{}", ed.frame)).family(style::label()).size(22.).color(STAR));
    ui.label(RichText::new(format!("/ {}", n - 1)).color(FAINT));
    let Some(m) = ed.run.as_ref().map(|r| r.marks[ed.frame.min(n - 1)]) else { return };
    match (m.phase, &ed.subject) {
        (Some((p, t)), Subject::Move(slot)) => {
            let mv = move_ref(&ed.ws.docs[&ed.cur].item.moves, slot);
            let ph = mv.and_then(|mv| mv.phases.get(p));
            let name = ph.map_or(String::new(), |ph| ph.name.clone());
            let len = ph.and_then(|ph| ph.frames).map_or("∞".into(), |f| f.to_string());
            ui.add_space(6.);
            ui.label(RichText::new(name).family(style::label()).size(16.).color(PHASE_COLORS[p % 6]));
            ui.label(RichText::new(format!("{t} / {len}")).color(DIM)).on_hover_text("Phase frame / phase length");
            if ph.is_some_and(|ph| ph.hitboxes.iter().any(|h| h.active(t))) {
                badge(ui, "ACTIVE", RED);
            }
            if ph.is_some_and(|ph| ph.anim.iter().any(|k| k.at == f64::from(t))) {
                badge(ui, "◆ KEY", GOLD);
            }
        }
        _ => {
            ui.add_space(6.);
            ui.label(RichText::new(m.state.name()).color(DIM));
        }
    }
}

fn track(ui: &mut egui::Ui, ed: &mut Editor) {
    let Some(run) = &ed.run else { return };
    let n = run.marks.len();
    let doc = &ed.ws.docs[&ed.cur];
    let (boxes, events, keys, phase_names) = match &ed.subject {
        Subject::Move(slot) => match move_ref(&doc.item.moves, slot) {
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
    let is_move = matches!(ed.subject, Subject::Move(_));
    let rows = boxes.len().min(8);
    let row_h = 14.;
    let label_w = 84.;
    let height = 18. + 22. + if is_move { rows as f32 * row_h + 2. * 15. + 4. } else { 0. };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6, style::NIGHT);
    let x0 = rect.left() + label_w;
    let w = ((rect.width() - label_w - 8.) / n as f32).max(1.);
    let fx = |k: usize| x0 + k as f32 * w;
    let small = style::text(10.5);
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
    lbl(y + 2., if is_move { "PHASES" } else { "STATE" });
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
        let r = Rect::from_min_max(egui::pos2(fx(k) + 0.5, y), egui::pos2(fx(e + 1) - 0.5, y + 18.));
        let (fill, name) = match m.phase {
            Some((pi, _)) => (PHASE_COLORS[pi % 6], phase_names.get(pi).cloned().unwrap_or_default()),
            None => (style::INK2, m.state.name().to_string()),
        };
        let sel = m.phase.is_some_and(|(pi, _)| pi == ed.phase);
        p.rect_filled(r, 3, if m.phase.is_some() { fill.gamma_multiply(if sel { 0.9 } else { 0.4 }) } else { fill });
        if sel {
            p.rect_stroke(r, 3, Stroke::new(1., STAR.gamma_multiply(0.6)), egui::StrokeKind::Inside);
        }
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
    y += 22.;
    let in_phase = |k: usize| run.marks[k].phase.filter(|(pi, _)| *pi == ed.phase).map(|(_, t)| t);
    let mut row_hit = None;
    let mut key_hit = None;
    if is_move {
        // Hitbox windows for the selected phase.
        for (i, h) in boxes.iter().take(rows).enumerate() {
            let ry = y + i as f32 * row_h;
            let sel = ed.hitbox == Some(i);
            if sel {
                p.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(rect.left() + 2., ry),
                        egui::pos2(rect.right() - 2., ry + row_h - 1.),
                    ),
                    2,
                    GOLD.gamma_multiply(0.12),
                );
            }
            let c = style::rgb(render::debug::hit_type_color(h.kind));
            p.circle_filled(egui::pos2(rect.left() + 12., ry + row_h / 2.), 3.5, c);
            p.text(
                egui::pos2(rect.left() + 20., ry + row_h / 2.),
                Align2::LEFT_CENTER,
                format!("box {i} · {}%", h.damage),
                small.clone(),
                if sel { GOLD } else { FAINT },
            );
            let mut k = 0;
            while k < n {
                if in_phase(k).is_some_and(|t| h.active(t)) {
                    let mut e = k;
                    while e + 1 < n && in_phase(e + 1).is_some_and(|t| h.active(t)) {
                        e += 1;
                    }
                    let r = Rect::from_min_max(egui::pos2(fx(k), ry + 2.), egui::pos2(fx(e + 1), ry + row_h - 3.));
                    p.rect_filled(r, 2, c.gamma_multiply(if sel { 1. } else { 0.65 }));
                    if r.width() > 26. {
                        let txt = format!("{}–{}", in_phase(k).unwrap_or(0), in_phase(e).unwrap_or(0));
                        p.text(r.center(), Align2::CENTER_CENTER, txt, style::text(9.5), style::NIGHT);
                    }
                    k = e + 1;
                } else {
                    k += 1;
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
        if boxes.len() > rows {
            p.text(
                egui::pos2(rect.right() - 8., y),
                Align2::RIGHT_TOP,
                format!("+{} more", boxes.len() - rows),
                small.clone(),
                FAINT,
            );
        }
        y += rows as f32 * row_h + 2.;
        // Events of the selected phase: ranges as bars, single frames as dots.
        lbl(y, "EVENTS");
        for k in 0..n {
            if let Some(t) = in_phase(k)
                && (k == 0 || run.marks[k].phase != run.marks[k - 1].phase)
                && events.iter().any(|e| e.fires(t, run.frames[k].frame))
            {
                let ranged = events.iter().any(|e| e.at.is_none() && e.fires(t, run.frames[k].frame));
                if ranged {
                    p.rect_filled(
                        Rect::from_min_max(egui::pos2(fx(k), y + 5.), egui::pos2(fx(k + 1), y + 9.)),
                        0,
                        STAR.gamma_multiply(0.7),
                    );
                } else {
                    p.circle_filled(egui::pos2(fx(k) + w / 2., y + 7.), 3.5_f32.min(w / 2. + 1.), STAR);
                }
            }
        }
        y += 15.;
        lbl(y, "POSE KEYS");
        for k in 0..n {
            if let Some(t) = in_phase(k)
                && (k == 0 || run.marks[k].phase != run.marks[k - 1].phase)
                && keys.iter().any(|&a| a == f64::from(t))
            {
                let c = egui::pos2(fx(k) + w / 2., y + 7.);
                let s = 4.5_f32.min(w / 2. + 2.);
                p.add(egui::Shape::convex_polygon(
                    vec![c + egui::vec2(0., -s), c + egui::vec2(s, 0.), c + egui::vec2(0., s), c + egui::vec2(-s, 0.)],
                    GOLD,
                    Stroke::NONE,
                ));
                if let Some(pos) = resp.interact_pointer_pos()
                    && resp.clicked()
                    && (pos.y - c.y).abs() < 8.
                    && (pos.x - c.x).abs() < s.max(4.)
                {
                    key_hit = Some(k);
                }
            }
        }
    }
    // Hover line and readout.
    if let Some(h) = resp.hover_pos().filter(|h| h.x >= x0) {
        let k = (((h.x - x0) / w).floor().max(0.) as usize).min(n - 1);
        let x = fx(k) + w / 2.;
        p.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(1., STAR.gamma_multiply(0.25)),
        );
        let tip = match run.marks[k].phase {
            Some((pi, t)) => format!("frame {k} · {} {t}", phase_names.get(pi).cloned().unwrap_or_default()),
            None => format!("frame {k} · {}", run.marks[k].state.name()),
        };
        resp.clone().on_hover_text_at_pointer(tip);
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
        ed.set_frame(key_hit.unwrap_or(k));
    }
    if resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
}
