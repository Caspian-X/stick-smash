//! Training panel (egui): current move and frame, last hit's frame advantage and combo, dummy behavior and
//! damage, speed, freeze/step, hitboxes. Gamepad players get the dummy controls from the pause menu too.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use sim::world::State as FState;

use crate::Data;
use crate::input::Devices;
use crate::play::{self, Match, Mode};
use crate::settings::{DummyMode, Saved};

const SPEEDS: [f64; 3] = [1., 0.5, 0.25];

pub fn panel(
    mut ctx: EguiContexts,
    mut m: ResMut<Match>,
    devs: Res<Devices>,
    data: Res<Data>,
    mut saved: ResMut<Saved>,
    mut scene: ResMut<render::Scene>,
) -> Result {
    if m.mode != Mode::Training || m.paused.is_some() {
        return Ok(());
    }
    let ctx = ctx.ctx_mut()?;
    let gold = egui::Color32::from_rgb(255, 211, 90);
    let dim = egui::Color32::from_rgb(140, 148, 200);
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(14, 17, 51, 230))
        .stroke(egui::Stroke::new(1., egui::Color32::from_rgb(51, 59, 122)))
        .corner_radius(10)
        .inner_margin(12);
    let mut restart = false;
    egui::Window::new("Training")
        .anchor(egui::Align2::RIGHT_TOP, [-14., 14.])
        .resizable(false)
        .collapsible(true)
        .frame(frame)
        .default_width(270.)
        .show(ctx, |ui| {
            ui.style_mut().visuals.override_text_color = Some(egui::Color32::from_rgb(232, 236, 255));
            let w = &m.world;
            let d = &m.data;
            let p1 = &w.fighters[0];
            let mono = |s: String| egui::RichText::new(s).monospace().size(13.);
            match (&p1.mov, p1.state) {
                (Some(mv), FState::Attack) => {
                    let def = sim::moves::definition(d, p1);
                    ui.label(mono(format!("P1 {}  ·  phase {}  frame {}", def.name, mv.phase, mv.elapsed)));
                }
                _ => {
                    ui.label(mono(format!("P1 {}", p1.state.name())));
                }
            }
            ui.add_space(4.);
            match &w.trace {
                Some(tr) => {
                    let adv = if tr.done { tr.d_free - tr.a_free } else { 0 };
                    let a = if tr.done { format!("{adv:+}") } else { "…".into() };
                    let col = if !tr.done {
                        dim
                    } else if adv >= 0 {
                        egui::Color32::from_rgb(110, 230, 140)
                    } else {
                        egui::Color32::from_rgb(255, 120, 110)
                    };
                    ui.label(mono(format!("Last: {} {}", tr.label, if tr.on_shield { "on shield" } else { "on hit" })));
                    ui.horizontal(|ui| {
                        ui.label(mono("Frame advantage".into()));
                        ui.label(egui::RichText::new(a).monospace().size(18.).strong().color(col));
                    });
                    if tr.combo > 1 {
                        ui.label(
                            egui::RichText::new(format!("Combo  {} hits  {:.0}%", tr.combo, tr.combo_dmg))
                                .color(gold)
                                .strong(),
                        );
                    }
                }
                None => {
                    ui.label(egui::RichText::new("Hit the dummy to see frame advantage.").color(dim));
                }
            }
            ui.separator();
            ui.label(egui::RichText::new("Dummy").strong());
            let mut mode = m.dummy.mode;
            ui.horizontal_wrapped(|ui| {
                for x in DummyMode::ALL {
                    ui.selectable_value(&mut mode, x, x.label());
                }
            });
            if mode != m.dummy.mode {
                m.dummy.mode = mode;
                m.dummy.dev = None;
                saved.training.mode = mode;
                m.apply_dummy();
            }
            if mode == DummyMode::Cpu {
                let mut lv = m.dummy.level;
                ui.horizontal(|ui| {
                    for (i, n) in ["Easy", "Normal", "Hard"].iter().enumerate() {
                        ui.selectable_value(&mut lv, i as u8, *n);
                    }
                });
                if lv != m.dummy.level {
                    m.dummy.level = lv;
                    saved.training.level = lv;
                    m.apply_dummy();
                }
            }
            if mode == DummyMode::Human {
                let who = m
                    .dummy
                    .dev
                    .map_or("Press a button on another device".into(), |d| format!("Controlled by {}", devs.name(d)));
                ui.label(egui::RichText::new(who).color(dim));
            }
            let mut pct = m.world.fighters[1].dmg;
            ui.horizontal(|ui| {
                ui.label("Damage");
                ui.add(egui::Slider::new(&mut pct, 0.0..=300.0).step_by(1.).suffix("%"));
            });
            if pct != m.world.fighters[1].dmg {
                m.world.fighters[1].dmg = pct;
                m.dummy.reset_pct = pct;
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Speed");
                for s in SPEEDS {
                    let mut sp = m.speed;
                    if ui.selectable_value(&mut sp, s, format!("{s}×")).changed() {
                        m.speed = sp;
                    }
                }
            });
            ui.horizontal(|ui| {
                let label = if m.frozen { "Resume" } else { "Freeze" };
                if ui.button(label).clicked() {
                    m.frozen = !m.frozen;
                }
                if ui.add_enabled(m.frozen, egui::Button::new("Step")).clicked() {
                    m.step = true;
                }
                let mut dbg = m.debug;
                if ui.checkbox(&mut dbg, "Hitboxes").changed() {
                    m.debug = dbg;
                }
            });
            if ui.button("Reset positions").clicked() {
                restart = true;
            }
            ui.label(egui::RichText::new("F1 hitboxes · Start / Esc pause").color(dim).size(11.));
        });
    if restart {
        m.restart(&data.0, play::seed());
        scene.reset(&m.world);
    }
    Ok(())
}
