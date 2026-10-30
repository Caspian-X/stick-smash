use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use sim::data::{Source, load};
use sim::{GameData, Input, MatchConfig, TICK_HZ, World as SimWorld, step_world};

/// The running match. The sim world is plain data; Bevy only owns it as a resource.
#[derive(Resource)]
struct Match {
    world: SimWorld,
    data: GameData,
}

fn main() {
    let data = load_data(&[PathBuf::from("assets")]);
    let world = SimWorld::new(MatchConfig::default(), &data);
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Stick Smash".into(), resolution: (1280, 720).into(), ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_HZ)))
        .insert_resource(Match { world, data })
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(FixedUpdate, tick)
        .add_systems(EguiPrimaryContextPass, debug_panel)
        .run();
}

fn tick(mut m: ResMut<Match>) {
    let Match { world, data } = &mut *m;
    step_world(world, data, &[Input::default(); 2]);
    world.fx.clear();
}

fn debug_panel(mut contexts: EguiContexts, m: Res<Match>) -> Result {
    egui::Window::new("Debug").show(contexts.ctx_mut()?, |ui| {
        ui.label(format!("frame {}", m.world.frame));
        ui.label(format!("fighters: {}", m.data.fighters.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")));
        ui.label(format!("content hash {:016x}", m.data.hash));
    });
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
        Err(errors) => panic!("content failed to load:\n{}", errors.iter().map(|e| format!("  {e}\n")).collect::<String>()),
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
                Ok(text) => out.push(Source { path: p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/"), text }),
                Err(e) => error!("{}: {e}", p.display()),
            }
        }
    }
}
