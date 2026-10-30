use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use render::{RenderPlugin, RenderSystems, Scene};
use sim::data::{Color as DataColor, Source, load};
use sim::{GameData, Input, MatchConfig, PlayerConfig, TICK_HZ, World as SimWorld, step_world};

mod shots;

/// The running match. The sim world is plain data; Bevy only owns it as a resource.
#[derive(Resource)]
struct Match {
    world: SimWorld,
    data: Arc<GameData>,
    /// P1's fighter (index into `data.fighters`).
    p1: usize,
}

impl Match {
    fn config(&self, seed: u32) -> MatchConfig {
        let id = |i: usize| self.data.fighters[i % self.data.fighters.len()].id.clone();
        let cpu = self
            .data
            .fighters
            .iter()
            .position(|f| f.id == "king")
            .unwrap_or(0);
        MatchConfig {
            seed,
            players: vec![
                PlayerConfig::human(&id(self.p1)),
                PlayerConfig {
                    cpu: Some(2),
                    ..PlayerConfig::human(&id(cpu))
                },
            ],
            ..default()
        }
    }
}

/// Keyboard state for P1. Presses seen between ticks are latched so a tap shorter than a tick still counts.
#[derive(Resource, Default)]
struct Keys {
    latched: Input,
}

// Matches the TS CONTROLS[0] layout.
const LEFT: KeyCode = KeyCode::KeyA;
const RIGHT: KeyCode = KeyCode::KeyD;
const UP: KeyCode = KeyCode::KeyW;
const DOWN: KeyCode = KeyCode::KeyS;
const BUTTONS: [(KeyCode, u8); 5] = [
    (KeyCode::Space, 1),
    (KeyCode::KeyJ, 2),
    (KeyCode::KeyK, 4),
    (KeyCode::KeyI, 8),
    (KeyCode::KeyL, 16),
];

fn read_input(
    k: &ButtonInput<KeyCode>,
    pressed: impl Fn(&ButtonInput<KeyCode>, KeyCode) -> bool,
) -> Input {
    let axis = |neg, pos| (i8::from(pressed(k, pos)) - i8::from(pressed(k, neg))) * 127;
    let buttons = BUTTONS
        .iter()
        .filter(|(c, _)| pressed(k, *c))
        .fold(0, |b, (_, bit)| b | bit);
    Input {
        stick: (axis(LEFT, RIGHT), axis(DOWN, UP)),
        buttons,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let shot_dir = args
        .iter()
        .position(|a| a == "--screenshot")
        .map(|i| PathBuf::from(args.get(i + 1).map_or("screenshots", |s| s.as_str())));
    let data = Arc::new(load_data(&[PathBuf::from("assets")]));
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Stick Smash".into(),
            resolution: (1280, 720).into(),
            resizable: shot_dir.is_none(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins((EguiPlugin::default(), RenderPlugin))
    .insert_resource(ClearColor(Color::srgb_u8(0x0b, 0x0d, 0x2a)))
    .insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_HZ)))
    .insert_resource(Scene::new(data.clone()))
    .add_systems(EguiPrimaryContextPass, readout);
    if let Some(dir) = shot_dir {
        let (world, list) = shots::plan(&data);
        info!(
            "screenshot plan: {}",
            list.iter()
                .map(|s| format!("{}@{}", s.name, s.tick))
                .collect::<Vec<_>>()
                .join(", ")
        );
        fs::create_dir_all(&dir).expect("create screenshot dir");
        app.insert_resource(Match { world, data, p1: 0 })
            .insert_resource(shots::Shots {
                dir,
                list,
                next: 0,
                wait: 0,
                step: shots::Step::Advance,
            })
            .add_systems(Startup, start)
            .add_systems(Update, shots::run.before(RenderSystems));
    } else {
        let p1 = data
            .fighters
            .iter()
            .position(|f| f.id == "brawler")
            .unwrap_or(0);
        let mut m = Match {
            world: SimWorld::new(MatchConfig::default(), &data),
            data,
            p1,
        };
        m.world = SimWorld::new(m.config(seed()), &m.data);
        app.insert_resource(m)
            .init_resource::<Keys>()
            .add_systems(Startup, start)
            .add_systems(PreUpdate, latch.after(InputSystems))
            .add_systems(FixedUpdate, tick)
            .add_systems(Update, (hotkeys, interpolate).chain().before(RenderSystems));
    }
    app.run();
}

fn seed() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.subsec_nanos() ^ d.as_secs() as u32)
}

fn start(m: Res<Match>, mut scene: ResMut<Scene>) {
    scene.reset(&m.world);
}

fn latch(k: Res<ButtonInput<KeyCode>>, mut keys: ResMut<Keys>) {
    let p = read_input(&k, |k, c| k.just_pressed(c));
    let l = &mut keys.latched;
    l.buttons |= p.buttons;
    if p.stick.0 != 0 {
        l.stick.0 = p.stick.0;
    }
    if p.stick.1 != 0 {
        l.stick.1 = p.stick.1;
    }
}

fn tick(
    mut m: ResMut<Match>,
    k: Res<ButtonInput<KeyCode>>,
    mut keys: ResMut<Keys>,
    mut scene: ResMut<Scene>,
) {
    let held = read_input(&k, |k, c| k.pressed(c));
    let l = std::mem::take(&mut keys.latched);
    let pick = |h: i8, l: i8| if h != 0 { h } else { l };
    let p1 = Input {
        stick: (pick(held.stick.0, l.stick.0), pick(held.stick.1, l.stick.1)),
        buttons: held.buttons | l.buttons,
    };
    let Match { world, data, .. } = &mut *m;
    let mut inputs = vec![Input::default(); world.fighters.len()];
    inputs[0] = p1;
    step_world(world, data, &inputs);
    scene.push(world);
    world.fx.clear();
}

fn hotkeys(k: Res<ButtonInput<KeyCode>>, mut m: ResMut<Match>, mut scene: ResMut<Scene>) {
    if k.just_pressed(KeyCode::F1) {
        scene.debug = !scene.debug;
    }
    let pick = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
        .iter()
        .position(|c| k.just_pressed(*c));
    if let Some(p) = pick.filter(|p| *p < m.data.fighters.len()) {
        m.p1 = p;
    }
    if pick.is_some() || k.just_pressed(KeyCode::KeyR) {
        m.world = SimWorld::new(m.config(seed()), &m.data);
        scene.reset(&m.world);
    }
}

fn interpolate(time: Res<Time<Fixed>>, mut scene: ResMut<Scene>) {
    scene.alpha = time.overstep_fraction_f64();
}

fn player_color(d: &GameData, i: usize) -> egui::Color32 {
    match d.global.palette.get(i).map(|p| p.main) {
        Some(DataColor::Rgb(v)) => {
            egui::Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
        }
        _ => egui::Color32::WHITE,
    }
}

/// Minimal percent/stock readout and match status (the real HUD is milestone 4).
fn readout(mut ctx: EguiContexts, m: Res<Match>, shots: Option<Res<shots::Shots>>) -> Result {
    let ctx = ctx.ctx_mut()?;
    let w = &m.world;
    let d = &m.data;
    egui::Area::new(egui::Id::new("readout"))
        .anchor(egui::Align2::CENTER_BOTTOM, [0., -12.])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, f) in w.fighters.iter().enumerate() {
                    let name = &d.fighters[f.data].name;
                    let who = if w.cfg.players[i].cpu.is_some() {
                        "CPU"
                    } else {
                        "P1"
                    };
                    let txt = format!("{who} {name}  {:.0}%  ×{}", f.dmg, f.stocks.max(0));
                    egui::Frame::new()
                        .fill(egui::Color32::from_black_alpha(150))
                        .inner_margin(8)
                        .corner_radius(6)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(txt)
                                    .size(18.)
                                    .strong()
                                    .color(player_color(d, i)),
                            );
                        });
                }
            });
        });
    let big = if w.count > 0 {
        Some(format!("{}", w.count.div_ceil(TICK_HZ)))
    } else if w.go_t > 0 {
        Some("GO!".into())
    } else if w.over {
        Some(match w.winner {
            Some(i) => format!("{} wins!", d.fighters[w.fighters[i].data].name),
            None => "Draw!".into(),
        })
    } else if w.end_t > 0 {
        Some("GAME!".into())
    } else {
        None
    };
    if let Some(big) = big {
        egui::Area::new(egui::Id::new("big"))
            .anchor(egui::Align2::CENTER_CENTER, [0., -80.])
            .show(ctx, |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(big)
                            .size(72.)
                            .strong()
                            .color(egui::Color32::WHITE),
                    )
                    .extend(),
                );
            });
    }
    if shots.is_none() {
        let names: Vec<String> = d
            .fighters
            .iter()
            .enumerate()
            .map(|(i, f)| format!("{} {}", i + 1, f.name))
            .collect();
        egui::Area::new(egui::Id::new("help")).anchor(egui::Align2::LEFT_TOP, [10., 10.]).show(ctx, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "A/D/W/S move · Space jump · J attack · K special · I grab · L shield\nR restart · {} · F1 debug{}",
                    names.join(" · "),
                    if w.over { "\nMatch over: press R" } else { "" }
                ))
                .size(13.)
                .color(egui::Color32::from_white_alpha(170)),
            );
        });
    }
    Ok(())
}

/// Loads content from each root in order (built-in first, then user dirs; later files override).
/// Broken packages are logged and skipped. Panics if nothing playable loads.
fn load_data(roots: &[PathBuf]) -> GameData {
    let mut files = Vec::new();
    for root in roots {
        collect(root, root, &mut files);
    }
    match load(files) {
        Ok((data, errors)) => {
            errors.iter().for_each(|e| error!("{e}"));
            data
        }
        Err(errors) => panic!(
            "content failed to load:\n{}",
            errors
                .iter()
                .map(|e| format!("  {e}\n"))
                .collect::<String>()
        ),
    }
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<Source>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut paths: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            collect(root, &p, out);
        } else if p.extension().is_some_and(|e| e == "json") {
            match fs::read_to_string(&p) {
                Ok(text) => out.push(Source {
                    path: p
                        .strip_prefix(root)
                        .unwrap_or(&p)
                        .to_string_lossy()
                        .replace('\\', "/"),
                    text,
                }),
                Err(e) => error!("{}: {e}", p.display()),
            }
        }
    }
}
