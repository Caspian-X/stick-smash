//! Hooks the fighter editor (`editor` crate) into the game: its screen, leaving it, and the "Test" round trip
//! through training with the edited, unsaved content.

use std::path::PathBuf;

use bevy::prelude::*;
use editor::{EditorConfig, EditorPlugin, Outbox, Request};

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
        .add_systems(Update, requests.run_if(in_state(Screen::Editor)));
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
) {
    match out.request.take() {
        Some(Request::Back) => {
            if let Some(d) = out.reload.take() {
                data.0 = d;
            }
            next.set(Screen::Title);
        }
        Some(Request::Test { data: d, fighter }) => {
            let Some(fi) = d.fighters.iter().position(|f| f.id == fighter) else { return };
            let t = &saved.training;
            let dummy = d.fighters.iter().position(|f| f.id == t.dummy_fighter).unwrap_or(fi);
            let slot = |dev, fighter, tag: &str, color| Slot {
                dev,
                fighter,
                cpu: None,
                team: None,
                tap_jump: true,
                tag: tag.into(),
                color,
            };
            let p1 = if saved.split_keyboard { Dev::KeysL } else { Dev::Keys };
            let stage = d.stage(&t.stage).map_or_else(|| d.stages[0].id.clone(), |s| s.id.clone());
            let rules = Rules { stocks: 1, stage, friendly_fire: false };
            match Match::new(
                &d,
                Mode::Training,
                vec![slot(Some(p1), fi, "P1", 0), slot(None, dummy, "Dummy", 1)],
                rules,
                play::seed(),
            ) {
                Ok(mut m) => {
                    m.dummy.mode = t.mode;
                    m.dummy.level = t.level;
                    m.apply_dummy();
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
