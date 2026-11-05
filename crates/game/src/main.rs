use std::fs;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::app::RunFixedMainLoopSystems;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::time::TimeSystems;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use render::{RenderPlugin, RenderSystems, Scene};
use sim::data::load;
use sim::{GameData, TICK_HZ};
use ui::UiSystems;

mod audio;
mod editing;
mod grid;
mod hints;
mod hud;
mod input;
mod layout_shots;
mod menu;
mod play;
mod pointer;
mod replay;
mod results;
mod select;
mod settings;
mod shots;
mod showcase;
mod stage_select;
mod training;
mod ui;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Title,
    Settings,
    Controls,
    Select,
    /// Versus stage vote, after character select.
    Stages,
    Match,
    Results,
    Replays,
    Playback,
    Editor,
}

impl Screen {
    const ALL: [Screen; 10] = [
        Screen::Title,
        Screen::Settings,
        Screen::Controls,
        Screen::Select,
        Screen::Stages,
        Screen::Match,
        Screen::Results,
        Screen::Replays,
        Screen::Playback,
        Screen::Editor,
    ];
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
    let layout_dir = args
        .iter()
        .position(|a| a == "--layout-shots")
        .map(|i| PathBuf::from(args.get(i + 1).map_or("screenshots", |s| s.as_str())));
    if layout_dir.is_some() && shot_dir.is_some() {
        eprintln!("--layout-shots and --screenshot are separate runs");
        std::process::exit(1);
    }
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
    let persist =
        settings::Persist(if shot_dir.is_some() || layout_dir.is_some() { None } else { settings::config_file() });
    let saved = if layout_dir.is_some() { layout_shots::saved() } else { settings::load(persist.0.as_ref()) };
    let mut pace = play::Pace::default();
    pace.allowed = shot_dir.is_none() && layout_dir.is_none();
    let mut app = App::new();
    // Pipelined rendering and a two-frame swapchain queue each add a frame of input latency; a fighting game wants
    // the newest tick on screen as soon as possible, so render on the main thread with one frame in flight (vsync
    // stays on).
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<PipelinedRenderingPlugin>()
            .set(bevy::render::RenderPlugin { synchronous_pipeline_compilation: shot_dir.is_some(), ..default() })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Stick Smash".into(),
                    resolution: (1280, 720).into(),
                    resizable: shot_dir.is_none(),
                    mode: menu::window_mode(saved.fullscreen && shot_dir.is_none()),
                    desired_maximum_frame_latency: NonZero::new(1),
                    ..default()
                }),
                // `editing::close_requested` quits, after the editor's unsaved-changes prompt.
                close_when_requested: false,
                ..default()
            }),
    )
    .add_plugins((EguiPlugin::default(), RenderPlugin, audio::SoundPlugin, pads::PadsPlugin))
    .init_state::<Screen>()
    .add_plugins((
        ui::UiPlugin,
        pointer::PointerPlugin,
        menu::MenuPlugin,
        select::SelectPlugin,
        stage_select::StagePlugin,
        hud::HudPlugin,
        results::ResultsPlugin,
        replay::ReplayPlugin,
        training::TrainingPlugin,
    ))
    .insert_resource(ClearColor(ui::NIGHT))
    .insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_HZ)))
    .insert_resource(Scene::new(data.clone()))
    .insert_resource(Data(data))
    .insert_resource(ContentErrors(errors))
    .insert_resource(saved)
    .insert_resource(persist)
    .init_resource::<input::Devices>()
    .add_systems(Startup, move |mut commands: Commands, mut fonts: ResMut<Assets<Font>>| {
        commands.insert_resource(ui::Fonts::load(&assets, &mut fonts));
    })
    .add_systems(PreUpdate, input::poll.after(InputSystems))
    .insert_resource(pace)
    .add_systems(First, play::pace.after(TimeSystems))
    .add_systems(FixedUpdate, play::tick.run_if(resource_exists::<play::Match>).run_if(not(play::locked)))
    // Locked to a 60 Hz display: one step per frame, where the fixed step would run.
    .add_systems(
        RunFixedMainLoop,
        play::tick
            .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop)
            .run_if(resource_exists::<play::Match>)
            .run_if(play::locked),
    )
    .add_systems(
        Update,
        (play::interpolate.before(RenderSystems).run_if(resource_exists::<play::Match>), settings::save),
    )
    .add_systems(EguiPrimaryContextPass, toast);
    editing::setup(&mut app, assets_root, if shot_dir.is_some() { None } else { settings::user_content_dir() });
    if let Some(dir) = shot_dir {
        fs::create_dir_all(&dir).expect("create screenshot dir");
        play::FIXED_SEED.store(7, std::sync::atomic::Ordering::Relaxed);
        app.insert_resource(script.expect("screenshot script"))
            .add_systems(Update, shots::run.after(play::attract).before(UiSystems::Input));
    }
    if let Some(dir) = layout_dir {
        fs::create_dir_all(&dir).expect("create layout screenshot dir");
        play::FIXED_SEED.store(7, std::sync::atomic::Ordering::Relaxed);
        app.insert_resource(layout_shots::Plan::new(dir))
            .add_systems(Update, layout_shots::run.before(UiSystems::Input));
    }
    if !app.run().is_success() {
        std::process::exit(1);
    }
}

fn toast(mut ctx: EguiContexts, errors: Res<ContentErrors>, time: Res<Time>) -> Result {
    if errors.0.is_empty() || time.elapsed_secs() > 15. {
        return Ok(());
    }
    let ctx = ctx.ctx_mut()?;
    let star = ui::egui(ui::STAR);
    egui::Area::new(egui::Id::new("content-errors")).anchor(egui::Align2::RIGHT_BOTTOM, [-16., -16.]).show(ctx, |ui| {
        egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(60, 14, 24, 235))
            .corner_radius(8)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.set_max_width(460.);
                ui.label(egui::RichText::new("Some content didn't load and was skipped").strong().color(star));
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
    let mut errors = Vec::new();
    for root in roots {
        let (sources, failures) = sim::data::sources(root);
        files.extend(sources);
        errors.extend(failures);
    }
    match load(files) {
        Ok((data, failures)) => {
            errors.extend(failures);
            errors.iter().for_each(|e| error!("{e}"));
            (data, errors.iter().map(ToString::to_string).collect())
        }
        Err(failures) => {
            errors.extend(failures);
            panic!("content failed to load:\n{}", errors.iter().map(|e| format!("  {e}\n")).collect::<String>())
        }
    }
}

#[cfg(test)]
pub(crate) fn test_data() -> Arc<GameData> {
    static DATA: std::sync::OnceLock<Arc<GameData>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let (data, errors) = load_data(&[PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")]);
        assert!(errors.is_empty(), "{errors:?}");
        Arc::new(data)
    })
    .clone()
}

#[cfg(test)]
mod screens {
    use super::*;
    use bevy::input::ButtonInput;
    use bevy::input::mouse::MouseButton;
    use bevy::state::app::StatesPlugin;
    use bevy::text::Font;
    use bevy::ui::UiScale;
    use render::portrait::Portraits;
    use render::thumb::StageThumbs;

    /// The app without rendering, audio or windows: enough to drive `Screen` through its plugins' enters and exits.
    fn app() -> App {
        let data = test_data();
        let font = || Handle::<Font>::default();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<Screen>()
            .insert_resource(Data(data.clone()))
            .insert_resource(Scene::new(data))
            .insert_resource(settings::Saved::default())
            .insert_resource(ui::Fonts {
                display: font(),
                heavy: font(),
                label: font(),
                body: font(),
                body_bold: font(),
            })
            .init_resource::<input::Devices>()
            .init_resource::<pointer::Pointers>()
            .init_resource::<Portraits>()
            .init_resource::<StageThumbs>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<UiScale>()
            .add_plugins((select::SelectPlugin, stage_select::StagePlugin));
        app
    }

    fn go(app: &mut App, screen: Screen) {
        app.world_mut().resource_mut::<NextState<Screen>>().set(screen);
        app.update();
    }

    fn has<T: Resource>(app: &App) -> bool {
        app.world().contains_resource::<T>()
    }

    #[test]
    fn select_and_stage_screens_own_their_resources() {
        let mut app = app();
        app.update();
        assert!(!has::<select::Select>(&app) && !has::<stage_select::Vote>(&app), "nothing on the title");

        app.insert_resource(select::SelectInit { mode: play::Mode::Versus, dev: Some(input::Dev::Bot(0)) });
        go(&mut app, Screen::Select);
        assert!(has::<select::Select>(&app), "character select enters with a lobby");
        assert!(has::<select::Moments>(&app) && has::<hints::HintClock>(&app));

        go(&mut app, Screen::Stages);
        assert!(has::<stage_select::Vote>(&app) && has::<stage_select::Clock>(&app), "stage select enters");
        assert!(
            !has::<select::Moments>(&app) && !has::<hints::HintClock>(&app),
            "character select cleaned up after itself"
        );

        go(&mut app, Screen::Title);
        assert!(!has::<stage_select::Vote>(&app) && !has::<stage_select::Clock>(&app), "stage select cleaned up");
    }
}
