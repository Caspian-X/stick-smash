use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use render::{RenderPlugin, RenderSystems, Scene};
use sim::data::{Source, load};
use sim::{GameData, TICK_HZ};

mod audio;
mod editing;
mod hud;
mod input;
mod menu;
mod play;
mod replay;
mod results;
mod select;
mod settings;
mod shots;
mod training;
mod ui;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Title,
    Settings,
    Controls,
    Select,
    Match,
    Results,
    Replays,
    Playback,
    Editor,
}

/// Loaded content (built-in plus user content).
#[derive(Resource, Clone)]
pub struct Data(pub Arc<GameData>);

/// Content problems found at startup, shown as a toast.
#[derive(Resource, Default)]
pub struct ContentErrors(pub Vec<String>);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--verify-replays") {
        let result = args
            .get(i + 1)
            .ok_or("missing replay directory".to_string())
            .and_then(|dir| replay::verify_directory(Path::new(dir)));
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let shot_dir = args
        .iter()
        .position(|a| a == "--screenshot")
        .map(|i| PathBuf::from(args.get(i + 1).map_or("screenshots", |s| s.as_str())));
    let filter = args
        .iter()
        .position(|a| a == "--shots")
        .map(|i| {
            if shot_dir.is_none() {
                eprintln!("--shots requires --screenshot <dir>");
                std::process::exit(1);
            }
            args.get(i + 1)
                .unwrap_or_else(|| {
                    eprintln!("--shots requires comma-separated section names: {}", shots::section_names());
                    std::process::exit(1);
                })
                .as_str()
        })
        .filter(|s| !s.is_empty());
    let script = shot_dir.as_ref().map(|dir| {
        shots::Script::tour(dir.clone(), filter).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1);
        })
    });
    let assets = PathBuf::from("assets");
    let assets_root = assets.clone();
    let mut roots = vec![assets.clone()];
    if shot_dir.is_none() {
        roots.extend(settings::user_content_dir());
    }
    let (data, errors) = load_data(&roots);
    let data = Arc::new(data);
    let persist = settings::Persist(if shot_dir.is_some() { None } else { settings::config_file() });
    let saved = settings::load(persist.0.as_ref());
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(bevy::render::RenderPlugin { synchronous_pipeline_compilation: shot_dir.is_some(), ..default() })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Stick Smash".into(),
                    resolution: (1280, 720).into(),
                    resizable: shot_dir.is_none(),
                    mode: menu::window_mode(saved.fullscreen && shot_dir.is_none()),
                    ..default()
                }),
                // `editing::close_requested` quits, after the editor's unsaved-changes prompt.
                close_when_requested: false,
                ..default()
            }),
    )
    .add_plugins((EguiPlugin::default(), RenderPlugin, audio::SoundPlugin))
    .init_state::<Screen>()
    .insert_resource(ClearColor(ui::NIGHT))
    .insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_HZ)))
    .insert_resource(Scene::new(data.clone()))
    .insert_resource(Data(data))
    .insert_resource(ContentErrors(errors))
    .insert_resource(saved)
    .insert_resource(persist)
    .init_resource::<input::Devices>()
    .init_resource::<menu::MenuState>()
    .init_resource::<hud::PauseMenu>()
    .init_resource::<results::ResultsMenu>()
    .init_resource::<replay::Library>()
    .add_systems(Startup, move |mut commands: Commands, mut fonts: ResMut<Assets<Font>>| {
        commands.insert_resource(ui::Fonts::load(&assets, &mut fonts));
    })
    .add_systems(PreUpdate, input::poll.after(InputSystems))
    .add_systems(PostUpdate, hud::safe_area.after(bevy::ui::UiSystems::Layout))
    .add_systems(FixedUpdate, play::tick.run_if(resource_exists::<play::Match>))
    .add_systems(OnEnter(Screen::Title), menu::enter_title)
    .add_systems(OnEnter(Screen::Settings), menu::enter_settings)
    .add_systems(OnEnter(Screen::Controls), menu::enter_controls)
    .add_systems(OnEnter(Screen::Select), select::enter)
    .add_systems(OnEnter(Screen::Match), (input::clear, hud::spawn))
    .add_systems(OnEnter(Screen::Select), input::clear)
    .add_systems(OnEnter(Screen::Title), input::clear)
    .add_systems(OnEnter(Screen::Results), results::enter)
    .add_systems(OnEnter(Screen::Replays), replay::enter)
    .add_systems(OnEnter(Screen::Playback), hud::spawn)
    .add_systems(
        Update,
        (
            (play::attract, (menu::update, menu::draw).chain().run_if(resource_exists::<ui::Fonts>))
                .chain()
                .run_if(|s: Res<State<Screen>>| matches!(s.get(), Screen::Title | Screen::Settings | Screen::Controls)),
            (select::update, select::draw).chain().run_if(in_state(Screen::Select)),
            (play::match_flow, hud::pause, hud::draw_pause, hud::update).chain().run_if(in_state(Screen::Match)),
            (results::update, results::draw).chain().run_if(in_state(Screen::Results)),
            replay::library.run_if(in_state(Screen::Replays)),
            (replay::controls, hud::update).chain().run_if(in_state(Screen::Playback)),
            play::interpolate.run_if(resource_exists::<play::Match>),
            ui_scale,
            settings::save,
        )
            .before(RenderSystems),
    )
    .add_systems(EguiPrimaryContextPass, (training::panel.run_if(in_state(Screen::Match)), toast));
    editing::setup(&mut app, assets_root, if shot_dir.is_some() { None } else { settings::user_content_dir() });
    if let Some(dir) = shot_dir {
        fs::create_dir_all(&dir).expect("create screenshot dir");
        play::FIXED_SEED.store(7, std::sync::atomic::Ordering::Relaxed);
        app.insert_resource(script.expect("screenshot script")).add_systems(
            Update,
            shots::run.after(play::attract).before(menu::update).before(select::update).before(hud::pause),
        );
    }
    if !app.run().is_success() {
        std::process::exit(1);
    }
}

/// UI is laid out for a 720-pixel-high window and scaled to the real one.
fn ui_scale(win: Query<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    if let Ok(w) = win.single() {
        let s = (w.height() / 720.).max(0.3);
        if (scale.0 - s).abs() > 1e-3 {
            scale.0 = s;
        }
    }
}

fn toast(mut ctx: EguiContexts, errors: Res<ContentErrors>, time: Res<Time>) -> Result {
    if errors.0.is_empty() || time.elapsed_secs() > 15. {
        return Ok(());
    }
    let ctx = ctx.ctx_mut()?;
    egui::Area::new(egui::Id::new("content-errors")).anchor(egui::Align2::RIGHT_BOTTOM, [-16., -16.]).show(ctx, |ui| {
        egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(60, 14, 24, 235))
            .corner_radius(8)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.set_max_width(460.);
                ui.label(
                    egui::RichText::new("Some content didn't load and was skipped")
                        .strong()
                        .color(egui::Color32::WHITE),
                );
                for e in errors.0.iter().take(5) {
                    ui.label(egui::RichText::new(e).size(12.).color(egui::Color32::from_rgb(255, 200, 200)));
                }
                if errors.0.len() > 5 {
                    ui.label(format!("…and {} more (see the log)", errors.0.len() - 5));
                }
            });
    });
    Ok(())
}

/// Loads content from each root in order (built-in first, then the user content dir; later files override).
/// Broken packages are logged and skipped. Panics if nothing playable loads.
fn load_data(roots: &[PathBuf]) -> (GameData, Vec<String>) {
    let mut files = Vec::new();
    for root in roots {
        collect(root, root, &mut files);
    }
    match load(files) {
        Ok((data, errors)) => {
            errors.iter().for_each(|e| error!("{e}"));
            (data, errors.iter().map(ToString::to_string).collect())
        }
        Err(errors) => {
            panic!("content failed to load:\n{}", errors.iter().map(|e| format!("  {e}\n")).collect::<String>())
        }
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
                    path: p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/"),
                    text,
                }),
                Err(e) => error!("{}: {e}", p.display()),
            }
        }
    }
}
