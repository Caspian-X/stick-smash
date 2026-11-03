//! Focused control layout screenshots: `just layout-shots [dir]`.
//! Captures character-select layout selection and Controls profile management.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::Screen;
use crate::input::{self, ACTS, Dev};
use crate::menu::{self, MenuState};
use crate::play::Mode;
use crate::select::{self, SelectInit};
use crate::settings::{self, ControlPrefs, Family, Layout, Saved};

/// Content for the captures: a named keyboard layout, already selected by the keyboard.
pub fn saved() -> Saved {
    let mut s = Saved::default();
    let bindings = ACTS
        .iter()
        .zip(input::default_binds("keyboard"))
        .map(|(a, b)| (a.id().to_string(), b.iter().map(|c| (*c).to_string()).collect()));
    s.layouts.insert(
        "Compact keys".into(),
        Layout {
            family: Family::Keyboard,
            bindings: bindings.collect(),
            options: ControlPrefs { stick_tilts: true, stick_range: 0.8, ..default() },
        },
    );
    s.layout_choices.insert("keyboard".into(), "Compact keys".into());
    s.versus.cpus = vec![settings::PlayerPref::default()];
    s
}

#[derive(Resource)]
pub struct Plan {
    dir: PathBuf,
    frame: u32,
    phase: u8,
}

impl Plan {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, frame: 0, phase: 0 }
    }
    fn next(&mut self, phase: u8) {
        self.phase = phase;
        self.frame = 0;
    }
}

/// Drives the two captures through real screen transitions, then quits.
pub fn run(
    mut commands: Commands,
    mut plan: ResMut<Plan>,
    screen: Res<State<Screen>>,
    mut next: ResMut<NextState<Screen>>,
    sel: Option<ResMut<select::Select>>,
    menu: Option<ResMut<MenuState>>,
    mut exit: MessageWriter<AppExit>,
) {
    plan.frame += 1;
    match plan.phase {
        0 => {
            commands.insert_resource(SelectInit { mode: Mode::Versus, dev: Some(Dev::Keys) });
            next.set(Screen::Select);
            plan.next(1);
        }
        1 if *screen.get() == Screen::Select && plan.frame > 20 => {
            if let Some(mut s) = sel {
                let p = &mut s.slots[0];
                let home = crate::ui::key(select::TILE, p.fighter);
                p.menu = Some(home);
                p.cursor.key = crate::ui::key(select::LAYOUT, 0);
            }
            plan.next(2);
        }
        2 if plan.frame > 20 => {
            let path = plan.dir.join("layout-select.png");
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            info!("layout shots: select captured");
            plan.next(3);
        }
        3 => {
            next.set(Screen::Controls);
            plan.next(4);
        }
        4 if *screen.get() == Screen::Controls && plan.frame > 20 => {
            if let Some(mut m) = menu {
                m.profile = 0;
                m.cursor.key = menu::LAYOUT;
                m.warning = None;
            }
            plan.next(5);
        }
        5 if plan.frame > 20 => {
            let path = plan.dir.join("layout-controls.png");
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            info!("layout shots: controls captured");
            plan.next(6);
        }
        6 if plan.frame > 30 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
