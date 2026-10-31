//! Hooks the editor (`editor` crate) into the game: its screen, leaving it, and the "Test" round trip through
//! training (or a match, for stages) with the edited, unsaved content.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::WindowCloseRequested;
use editor::{Dialog, Editor, EditorConfig, EditorPlugin, Outbox, Request};

use crate::input::Dev;
use crate::play::{self, Match, Mode, Rules, Slot};
use crate::settings::Saved;
use crate::{Data, Screen};

/// Training started from the editor. Holds the game's own content to restore on return.
#[derive(Resource)]
pub struct Testing(pub std::sync::Arc<sim::GameData>);

pub fn setup(app: &mut App, assets: PathBuf, user_dir: Option<PathBuf>) {
    let mut roots = vec![assets.clone()];
    roots.extend(user_dir.clone());
    app.insert_resource(EditorConfig { roots, assets, user_dir, dev: cfg!(feature = "dev") })
        .add_plugins(EditorPlugin { state: Screen::Editor })
        .add_systems(OnEnter(Screen::Editor), enter)
        .add_systems(Update, (requests.run_if(in_state(Screen::Editor)), close_requested));
}

/// Quitting with unsaved editor changes: opens the editor's prompt instead. Returns whether it did.
pub fn ask_quit(ed: Option<&mut Editor>, next: &mut NextState<Screen>) -> bool {
    match ed {
        Some(ed) if !ed.unsaved().is_empty() => {
            ed.dialog = Some(Dialog::Leave { quit: true });
            next.set(Screen::Editor);
            true
        }
        _ => false,
    }
}

/// The window's close button (the window doesn't close by itself, see `main`).
fn close_requested(
    mut closes: MessageReader<WindowCloseRequested>,
    mut ed: Option<ResMut<Editor>>,
    mut next: ResMut<NextState<Screen>>,
    mut exit: MessageWriter<AppExit>,
) {
    if closes.read().count() > 0 && !ask_quit(ed.as_deref_mut(), &mut next) {
        exit.write(AppExit::Success);
    }
}

fn enter(mut commands: Commands, testing: Option<Res<Testing>>, mut data: ResMut<Data>) {
    commands.remove_resource::<Match>();
    if let Some(t) = testing {
        data.0 = t.0.clone();
        commands.remove_resource::<Testing>();
    }
}

fn requests(
    mut commands: Commands,
    mut out: ResMut<Outbox>,
    mut data: ResMut<Data>,
    saved: Res<Saved>,
    mut scene: ResMut<render::Scene>,
    mut next: ResMut<NextState<Screen>>,
    mut exit: MessageWriter<AppExit>,
) {
    match out.request.take() {
        Some(Request::Back) => {
            if let Some(d) = out.reload.take() {
                data.0 = d;
            }
            next.set(Screen::Title);
        }
        Some(Request::Quit) => {
            exit.write(AppExit::Success);
        }
        Some(Request::Test { data: d, fighter, stage, versus }) => {
            let t = &saved.training;
            let fighter = fighter.unwrap_or_else(|| t.fighter.clone());
            let fi = d.fighters.iter().position(|f| f.id == fighter).unwrap_or(0);
            let dummy = d.fighters.iter().position(|f| f.id == t.dummy_fighter).unwrap_or(fi);
            let slot = |dev, fighter, cpu, tag: &str, color| Slot {
                dev,
                fighter,
                cpu,
                team: None,
                tap_jump: true,
                tag: tag.into(),
                color,
            };
            let p1 = if saved.split_keyboard { Dev::KeysL } else { Dev::Keys };
            let stage = stage
                .or_else(|| Some(t.stage.clone()))
                .and_then(|s| d.stage(&s))
                .map_or_else(|| d.stages[0].id.clone(), |s| s.id.clone());
            let (mode, stocks, other) = if versus {
                (Mode::Versus, 3, slot(None, dummy, Some(t.level), "CPU", 1))
            } else {
                (Mode::Training, 1, slot(None, dummy, None, "Dummy", 1))
            };
            let rules = Rules { stocks, stage, friendly_fire: false };
            match Match::new(&d, mode, vec![slot(Some(p1), fi, None, "P1", 0), other], rules, play::seed()) {
                Ok(mut m) => {
                    if !versus {
                        m.dummy.mode = t.mode;
                        m.dummy.level = t.level;
                        m.apply_dummy();
                    }
                    commands.insert_resource(Testing(data.0.clone()));
                    data.0 = d;
                    play::install(&mut commands, &mut scene, m);
                    next.set(Screen::Match);
                }
                Err(e) => error!("editor test: {e}"),
            }
        }
        None => {}
    }
}
