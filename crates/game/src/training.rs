//! Training panel (egui): current move and frame, last hit's frame advantage and combo, dummy behavior and
//! damage, speed, freeze/step, hitboxes. Gamepad players get the dummy controls from the pause menu too.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use sim::Input;
use sim::actions::{Accepted, Action};
use sim::data::render::CPU_LEVELS;
use sim::world::State as FState;

use crate::input::Devices;
use crate::play::{self, Match, Mode};
use crate::settings::{DummyMode, Saved};
use crate::ui::{self, DIM, GOLD, INK, LINE, STAR};
use crate::{Data, Screen};

const SPEEDS: [f64; 3] = [1., 0.5, 0.25];
/// Input display rows kept and shown.
const LOG_KEEP: usize = 64;
const LOG_SHOW: usize = 14;

/// P1's controls (each change of stick direction, attack stick or buttons) and the actions the game accepted,
/// newest first. Lives in the match, not the sim snapshot.
#[derive(Default, Clone, Debug)]
pub struct InputLog {
    pub rows: VecDeque<(u64, Entry)>,
    last: Option<(u8, u8, u8)>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Raw(Input),
    Act(Action, Option<u64>),
}
impl InputLog {
    pub fn record(&mut self, tick: u64, input: Input, acts: &[Accepted]) {
        let key = (zone(input.stick, 18), zone(input.attack_stick, 64), input.buttons);
        if self.last != Some(key) {
            if self.last.is_some() || key != (0, 0, 0) {
                self.push(tick, Entry::Raw(input));
            }
            self.last = Some(key);
        }
        for a in acts.iter().filter(|a| a.player == 0) {
            self.push(a.tick, Entry::Act(a.action.clone(), a.pressed));
        }
    }
    fn push(&mut self, tick: u64, e: Entry) {
        self.rows.push_front((tick, e));
        self.rows.truncate(LOG_KEEP);
    }
}
/// Eight-way direction (1 = right, counterclockwise), 0 inside `dead`.
fn zone((x, y): (i8, i8), dead: i16) -> u8 {
    let (x, y) = (i16::from(x), i16::from(y));
    if x.abs().max(y.abs()) < dead {
        return 0;
    }
    let a = (f64::from(y)).atan2(f64::from(x)).to_degrees().rem_euclid(360.);
    ((a + 22.5) / 45.) as u8 % 8 + 1
}
const ARROWS: [&str; 9] = ["", "→", "↗", "↑", "↖", "←", "↙", "↓", "↘"];
fn raw_text(i: Input) -> String {
    let mut parts = vec![];
    let z = zone(i.stick, 18);
    if z != 0 {
        let m = (f64::from(i.stick.0).hypot(f64::from(i.stick.1)) / 127. * 100.).min(100.);
        parts.push(if m < 95. { format!("{} {m:.0}%", ARROWS[z as usize]) } else { ARROWS[z as usize].into() });
    }
    let r = zone(i.attack_stick, 64);
    if r != 0 {
        parts.push(format!("R{}", ARROWS[r as usize]));
    }
    let b = i.buttons;
    for (bit, name) in [(1, "Jump"), (32, "Jump"), (2, "Attack"), (4, "Special"), (8, "Grab"), (16, "Shield")] {
        if b & bit != 0 {
            parts.push(name.into());
        }
    }
    if parts.is_empty() { "Released".into() } else { parts.join("  ") }
}
fn act_text(d: &sim::GameData, fighter: usize, a: &Action) -> String {
    let t = |s: &str| s.to_string();
    match a {
        Action::Move(slot) => d.fighters[fighter].moves.get(slot).map_or_else(|| slot.clone(), |m| m.name.clone()),
        Action::Followup { slot, phase } => {
            let name = d.fighters[fighter].moves.get(slot).map_or(slot.as_str(), |m| m.name.as_str());
            format!("{name} → {phase}")
        }
        Action::Jump => t("Jump"),
        Action::ShortHop => t("Short hop"),
        Action::FullHop => t("Full hop"),
        Action::AirJump => t("Double jump"),
        Action::Dash => t("Dash"),
        Action::Turn => t("Turn around"),
        Action::Shield => t("Shield"),
        Action::ShieldDrop => t("Drop shield"),
        Action::Parry => t("Parry!"),
        Action::Roll => t("Roll"),
        Action::SpotDodge => t("Spot dodge"),
        Action::AirDodge => t("Air dodge"),
        Action::FastFall => t("Fast fall"),
        Action::Float => t("Float"),
        Action::PlatformDrop => t("Drop through"),
        Action::LedgeGrab => t("Ledge grab"),
        Action::LedgeTrump => t("Ledge trump"),
        Action::Trumped => t("Knocked off ledge"),
        Action::Ledge(k) => match k.as_str() {
            "attack" => t("Ledge attack"),
            "roll" => t("Ledge roll"),
            "jump" => t("Ledge jump"),
            _ => t("Ledge climb"),
        },
        Action::LedgeDrop => t("Let go of ledge"),
        Action::Getup => t("Get up"),
        Action::GetupAttack => t("Getup attack"),
        Action::Tech => t("Tech"),
        Action::TechRoll => t("Tech roll"),
        Action::Pummel => t("Pummel"),
        Action::Throw(k) => format!(
            "{} throw",
            match k.as_str() {
                "b" => "Back",
                "u" => "Up",
                "d" => "Down",
                _ => "Forward",
            }
        ),
        Action::Influence(deg) => format!("Influence {deg:+}°"),
    }
}

pub struct TrainingPlugin;

impl Plugin for TrainingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(bevy_egui::EguiPrimaryContextPass, panel.run_if(in_state(Screen::Match)));
    }
}

fn panel(
    mut ctx: EguiContexts,
    mut m: ResMut<Match>,
    mut devs: ResMut<Devices>,
    data: Res<Data>,
    mut saved: ResMut<Saved>,
    mut scene: ResMut<render::Scene>,
) -> Result {
    if m.mode != Mode::Training || m.paused.is_some() {
        return Ok(());
    }
    let ctx = ctx.ctx_mut()?;
    let (gold, dim, star) = (ui::egui(GOLD), ui::egui(DIM), ui::egui(STAR));
    let frame = egui::Frame::new()
        .fill(ui::egui(INK))
        .stroke(egui::Stroke::new(1., ui::egui(LINE)))
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
            ui.style_mut().visuals.override_text_color = Some(star);
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
                m.set_dummy_mode(mode, &mut devs, &mut saved);
            }
            if mode == DummyMode::Cpu {
                let mut lv = m.dummy.level;
                ui.horizontal(|ui| {
                    for (i, n) in CPU_LEVELS.iter().enumerate() {
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
                    m.clear_input(&mut devs);
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
            ui.separator();
            egui::CollapsingHeader::new(egui::RichText::new("Inputs").strong()).default_open(true).show(ui, |ui| {
                ui.label(egui::RichText::new("Your controls, and what your fighter did").color(dim).size(11.));
                let fighter = m.world.fighters[0].data;
                for (tick, e) in m.log.rows.iter().take(LOG_SHOW) {
                    let (text, col) = match e {
                        Entry::Raw(i) => (raw_text(*i), dim),
                        Entry::Act(a, pressed) => {
                            let late = pressed.map_or(String::new(), |p| format!("  (buffered {}f)", tick - p));
                            (format!("Did: {}{late}", act_text(&m.data, fighter, a)), gold)
                        }
                    };
                    ui.label(
                        egui::RichText::new(format!("{:>5}  {text}", tick % 100_000)).monospace().size(12.).color(col),
                    );
                }
                if m.log.rows.is_empty() {
                    ui.label(egui::RichText::new("Press something.").color(dim));
                }
            });
            ui.label(egui::RichText::new("F1 hitboxes · Start / Esc pause").color(dim).size(11.));
        });
    if restart {
        play::restart(&mut m, &data.0, &mut scene, &mut devs);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_log_is_bounded_dedups_holds_and_keeps_buffered_press_ticks() {
        let mut log = InputLog::default();
        let right = Input { stick: (127, 0), ..Default::default() };
        for t in 0..200 {
            log.record(t, if t % 2 == 0 { right } else { Input::default() }, &[]);
        }
        assert_eq!(log.rows.len(), LOG_KEEP);
        let mut log = InputLog::default();
        log.record(1, right, &[]);
        log.record(2, right, &[]);
        let act = Accepted { player: 0, action: Action::Move("jab".into()), tick: 3, pressed: Some(1) };
        log.record(3, right, &[act, Accepted { player: 1, action: Action::Dash, tick: 3, pressed: None }]);
        assert_eq!(log.rows.len(), 2);
        assert_eq!(log.rows[0], (3, Entry::Act(Action::Move("jab".into()), Some(1))));
        log.record(4, Input::default(), &[]);
        assert_eq!(log.rows[0], (4, Entry::Raw(Input::default())));
        assert_eq!(raw_text(Input::default()), "Released");
        log.record(5, Input::default(), &[]);
        assert_eq!(log.rows.len(), 3);
        assert_eq!(raw_text(Input { stick: (-127, 127), buttons: 3, ..Default::default() }), "↖  Jump  Attack");
    }
}
