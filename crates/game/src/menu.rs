//! Title menu, settings and the controls (remap) screen. Single cursor, driven by every device; the attract match
//! plays behind them.

use bevy::prelude::*;
use bevy::text::LineHeight;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};

use crate::hints::hint;
use crate::input::{self, ACTS, Dev, Devices};
use crate::play::{self, Mode};
use crate::select::SelectInit;
use crate::settings::{self, Saved};
use crate::ui::{self, *};
use crate::{ContentErrors, Data, Screen};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        let menus = in_state(Screen::Title).or_else(in_state(Screen::Settings)).or_else(in_state(Screen::Controls));
        app.init_resource::<MenuState>()
            .add_systems(OnEnter(Screen::Title), enter_title)
            .add_systems(OnEnter(Screen::Settings), enter_settings)
            .add_systems(OnEnter(Screen::Controls), enter_controls)
            .add_systems(
                Update,
                (play::attract.before(UiSystems::Input), update.in_set(UiSystems::Input), draw.in_set(UiSystems::Draw))
                    .run_if(menus),
            );
    }
}

#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct MenuState {
    pub cursor: Cursor,
    /// Controls screen: the profile being edited and the action waiting for a control.
    pub profile: usize,
    pub capture: Option<usize>,
    pub warning: Option<String>,
}

const VERSUS: u32 = 1;
const TRAINING: u32 = 2;
const CONTROLS: u32 = 3;
const SETTINGS: u32 = 4;
const QUIT: u32 = 5;
const EDITOR: u32 = 6;
const REPLAYS: u32 = 7;
const KEYBOARD: u32 = 10;
const HITBOXES: u32 = 11;
const WINDOW: u32 = 12;
const MASTER: u32 = 13;
const SFX: u32 = 14;
const MUSIC: u32 = 15;
const BACK: u32 = 19;
const PROFILE: u32 = 20;
const ACT0: u32 = 30;
const RESET: u32 = 50;

pub fn enter_title(mut st: ResMut<MenuState>) {
    *st = MenuState { cursor: Cursor { key: VERSUS, owner: None }, ..default() };
}
pub fn enter_settings(mut st: ResMut<MenuState>) {
    *st = MenuState { cursor: Cursor { key: KEYBOARD, owner: None }, ..default() };
}
pub fn enter_controls(mut st: ResMut<MenuState>) {
    *st = MenuState { cursor: Cursor { key: PROFILE, owner: None }, ..default() };
}

/// Control profiles that can be edited right now: the keyboard (or its halves) and each connected gamepad.
fn profiles(saved: &Saved, devs: &Devices) -> Vec<(Dev, String)> {
    let mut v: Vec<Dev> = if saved.split_keyboard { vec![Dev::KeysL, Dev::KeysR] } else { vec![Dev::Keys] };
    v.extend(devs.map.keys().filter(|d| matches!(d, Dev::Pad(_))));
    v.into_iter().map(|d| (d, devs.profile(d))).collect()
}

fn update(
    screen: Res<State<Screen>>,
    data: Res<Data>,
    mut st: ResMut<MenuState>,
    mut devs: ResMut<Devices>,
    q: FocusQuery,
    mut saved: ResMut<Saved>,
    mut next: ResMut<NextState<Screen>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut win: Query<&mut Window, With<PrimaryWindow>>,
    mut ed: Option<ResMut<editor::Editor>>,
) {
    let spots = ui::spots(&q);
    let mut s = st.clone();
    let profs = profiles(&saved, &devs);
    s.profile = s.profile.min(profs.len().saturating_sub(1));
    if let Some(a) = s.capture {
        let (pd, prof) = &profs[s.profile];
        let kb = pd.is_keyboard();
        let raw = devs
            .map
            .iter()
            .filter(|(d, _)| if kb { d.is_keyboard() } else { *d == pd })
            .find_map(|(_, s)| s.raw.clone());
        let cancel = devs.map.iter().any(|(d, s)| d.is_keyboard() && s.raw.as_deref() == Some("Escape"))
            || devs.map.get(pd).is_some_and(|s| s.raw.as_deref() == Some("Select") || s.raw.as_deref() == Some("East"));
        match raw {
            _ if cancel => s.capture = None,
            Some(r) if input::keyboard_conflict(&saved, prof, &r) => {
                s.warning = Some(format!(
                    "{} is used by the other keyboard player. Choose another key.",
                    input::control_label(&r)
                ));
            }
            Some(r) => {
                let mut b = input::binds(&saved, prof);
                b[a] = vec![r];
                saved
                    .bindings
                    .insert(prof.clone(), ACTS.iter().zip(b).map(|(act, v)| (act.id().to_string(), v)).collect());
                s.capture = None;
                s.warning = None;
                devs.clear();
            }
            None => {}
        }
        if s != *st {
            *st = s;
        }
        return;
    }
    for (dev, nav) in devs.navs().collect::<Vec<_>>() {
        let Some(a) = s.cursor.drive(nav, &spots) else {
            continue;
        };
        match (screen.get(), a) {
            (Screen::Title, Action::Activate(VERSUS | TRAINING)) => {
                let mode = if s.cursor.key == VERSUS { Mode::Versus } else { Mode::Training };
                commands.insert_resource(SelectInit { mode, dev: Some(dev) });
                next.set(Screen::Select);
            }
            (Screen::Title, Action::Activate(REPLAYS)) => next.set(Screen::Replays),
            (Screen::Title, Action::Activate(EDITOR)) => next.set(Screen::Editor),
            (Screen::Title, Action::Activate(CONTROLS)) => next.set(Screen::Controls),
            (Screen::Title, Action::Activate(SETTINGS)) => next.set(Screen::Settings),
            (Screen::Title, Action::Activate(QUIT)) => {
                if !crate::editing::ask_quit(ed.as_deref_mut(), &mut next) {
                    exit.write(AppExit::Success);
                }
            }
            (Screen::Title, _) => {}
            (_, Action::Back | Action::Activate(BACK)) => next.set(Screen::Title),
            (Screen::Settings, Action::Spin(KEYBOARD, _)) => {
                saved.split_keyboard = !saved.split_keyboard;
                devs.clear();
            }
            (Screen::Settings, Action::Spin(k @ (MASTER | SFX | MUSIC), by)) => {
                let a = &data.0.global.audio;
                let v = saved.volume.get_or_insert_with(|| a.defaults.clone());
                let value = match k {
                    MASTER => &mut v.master,
                    SFX => &mut v.sfx,
                    _ => &mut v.music,
                };
                *value = (*value + f64::from(by) * a.volume_step).clamp(0., 1.);
            }
            (Screen::Settings, Action::Spin(HITBOXES, _)) => saved.versus_hitboxes = !saved.versus_hitboxes,
            (Screen::Settings, Action::Spin(WINDOW, _)) => {
                saved.fullscreen = !saved.fullscreen;
                if let Ok(mut w) = win.single_mut() {
                    w.mode = window_mode(saved.fullscreen);
                }
            }
            (Screen::Controls, Action::Spin(PROFILE, by)) => {
                s.profile = (s.profile as i32 + by).rem_euclid(profs.len().max(1) as i32) as usize;
            }
            (Screen::Controls, Action::Activate(k)) if (ACT0..ACT0 + 10).contains(&k) => {
                s.capture = Some((k - ACT0) as usize);
                s.warning = None;
            }
            (Screen::Controls, Action::Activate(RESET)) => {
                if let Some((_, prof)) = profs.get(s.profile) {
                    saved.bindings.remove(prof);
                }
            }
            _ => {}
        }
    }
    if s != *st {
        *st = s;
    }
}

pub fn window_mode(full: bool) -> WindowMode {
    if full { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed }
}

/// What a menu screen shows.
#[derive(Clone, PartialEq)]
pub struct MenuKey {
    screen: Screen,
    st: MenuState,
    saved: Saved,
    devices: Vec<String>,
    profiles: Vec<(Dev, String)>,
    errors: usize,
}

fn draw(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    st: Res<MenuState>,
    saved: Res<Saved>,
    devs: Res<Devices>,
    roots: Roots<MenuKey>,
    fonts: Res<Fonts>,
    data: Res<Data>,
    errors: Res<ContentErrors>,
) {
    let scr = *screen.get();
    let key = MenuKey {
        screen: scr,
        st: st.clone(),
        saved: saved.clone(),
        devices: devs.map.keys().filter(|d| !matches!(d, Dev::Bot(_))).map(|d| devs.name(*d)).collect(),
        profiles: profiles(&saved, &devs),
        errors: errors.0.len(),
    };
    let Some(mut root) = ui::rebuild(&mut commands, &roots, &key, scr, z::SCREEN) else { return };
    let (names, profs) = (&key.devices, &key.profiles);
    let f = &*fonts;
    let d = &data.0;
    let focus = st.cursor.key;
    root.insert(BackgroundGradient::from(LinearGradient::to_right(vec![
        ColorStop::new(NIGHT.with_alpha(0.95), Val::Percent(0.)),
        ColorStop::new(NIGHT.with_alpha(0.9), Val::Percent(if scr == Screen::Title { 30. } else { 55. })),
        ColorStop::new(NIGHT.with_alpha(0.2), Val::Percent(if scr == Screen::Title { 70. } else { 90. })),
    ])))
    .with_children(|p| {
        p.spawn(node(|n| {
            n.flex_direction = FlexDirection::Column;
            n.justify_content = JustifyContent::Center;
            // The bottom padding keeps centred menus clear of the device strip.
            n.padding = UiRect::new(Val::Px(84.), Val::Px(0.), Val::Px(16.), Val::Px(60.));
            n.height = Val::Percent(100.);
            n.row_gap = Val::Px(8.);
        }))
        .with_children(|p| match scr {
            Screen::Title => {
                p.spawn((
                    txt(&f.display, 132., STAR, "Stick\nSmash"),
                    TextLayout::new(Justify::Left, LineBreak::NoWrap),
                    LineHeight::RelativeToFont(0.82),
                ));
                p.spawn(node(|n| n.margin = UiRect::new(Val::Px(6.), Val::Px(0.), Val::Px(10.), Val::Px(6.))))
                    .with_children(|p| palette_strip(p, d, 36., 6.));
                p.spawn((
                    txt(&f.body, 18., DIM, "A tiny platform fighter for up to eight friends."),
                    node(|n| n.margin.bottom = Val::Px(28.)),
                ));
                let items = [
                    Item::button(VERSUS, "Versus"),
                    Item::button(TRAINING, "Training"),
                    Item::button(EDITOR, "Editor"),
                    Item::button(CONTROLS, "Controls"),
                    Item::button(SETTINGS, "Settings"),
                    Item::button(REPLAYS, "Replays"),
                    Item::button(QUIT, "Quit"),
                ];
                menu_list(p, f, &items, focus, List::new(380., 32.));
            }
            Screen::Settings => {
                heading(p, f, d, "Settings");
                let volume = saved.volume.as_ref().unwrap_or(&d.global.audio.defaults);
                let meter = |v: f64| {
                    let filled = (v.clamp(0., 1.) * 10.).round() as usize;
                    format!("{}{} {:3.0}%", "━".repeat(filled), "─".repeat(10 - filled), v * 100.)
                };
                let items = [
                    Item::spin(MASTER, "Master volume", meter(volume.master)),
                    Item::spin(SFX, "SFX volume", meter(volume.sfx)),
                    Item::spin(MUSIC, "Music (reserved)", meter(volume.music)),
                    Item::spin(
                        KEYBOARD,
                        "Keyboard",
                        if saved.split_keyboard { "Two players (split)" } else { "One player" },
                    ),
                    Item::spin(HITBOXES, "Hitboxes in versus", on_off(saved.versus_hitboxes)),
                    Item::spin(WINDOW, "Window", if saved.fullscreen { "Fullscreen" } else { "Windowed" }),
                    Item::button(BACK, "Back"),
                ];
                menu_list(p, f, &items, focus, List::new(560., 28.));
                p.spawn(node(|n| {
                    n.flex_direction = FlexDirection::Column;
                    n.margin.top = Val::Px(30.);
                    n.row_gap = Val::Px(6.);
                    n.max_width = Val::Px(620.);
                }))
                .with_children(|p| {
                    let path =
                        |o: Option<std::path::PathBuf>| o.map_or("unavailable".into(), |p| p.display().to_string());
                    p.spawn(txt(&f.body_bold, 14., DIM, "Settings are saved to"));
                    p.spawn(txt(&f.body, 14., FAINT, path(settings::config_file())));
                    p.spawn(txt(&f.body_bold, 14., DIM, "Your fighters and stages go in (same layout as assets/)"));
                    p.spawn(txt(&f.body, 14., FAINT, path(settings::user_content_dir())));
                    if !errors.0.is_empty() {
                        p.spawn(txt(
                            &f.body_bold,
                            14.,
                            Color::srgb(1., 0.55, 0.5),
                            format!("{} content problem(s) at startup; see the log", errors.0.len()),
                        ));
                    }
                });
            }
            Screen::Controls => {
                heading(p, f, d, "Controls");
                if let Some(warning) = &st.warning {
                    p.spawn(txt(&f.body, 16., GOLD, warning));
                }
                let Some((_, prof)) = profs.get(st.profile) else { return };
                let pname = profs.get(st.profile).map_or_else(String::new, |(dv, _)| devs.name(*dv));
                let b = input::binds(&saved, prof);
                let mut items = vec![Item::spin(PROFILE, "Device", pname)];
                for (i, act) in ACTS.iter().enumerate() {
                    let hot = st.capture == Some(i);
                    let v = if hot {
                        "Press a control…  (Esc / B / Back cancels)".to_string()
                    } else {
                        b[i].iter().map(|c| input::control_label(c)).collect::<Vec<_>>().join("  or  ")
                    };
                    items.push(Item { hot, ..Item::info(ACT0 + i as u32, act.label(), v) });
                }
                items.push(Item::button(RESET, "Reset to defaults"));
                items.push(Item::button(BACK, "Back"));
                p.spawn(node(|n| n.column_gap = Val::Px(40.))).with_children(|p| {
                    // Two columns: action, then its controls.
                    let list = List { gap: 0., label_width: Some(170.), ..List::new(560., 21.) };
                    menu_list(p, f, &items, focus, list);
                    p.spawn(node(|n| {
                        n.flex_direction = FlexDirection::Column;
                        n.row_gap = Val::Px(8.);
                        n.width = Val::Px(300.);
                        n.margin.top = Val::Px(8.);
                    }))
                    .with_children(|p| {
                        for line in [
                            "Tap a direction with attack for a smash attack; hold attack to charge it.",
                            "Shield + left/right rolls, shield + down dodges, shield in the air air-dodges.",
                            "Grab, then attack to pummel or a direction to throw.",
                            "Gamepad sticks always move; Esc or a gamepad's B backs out of menus.",
                        ] {
                            p.spawn(txt(&f.body, 14., DIM, line));
                        }
                    });
                });
            }
            _ => {}
        });
        // Connected devices, bottom left.
        p.spawn(node(|n| {
            n.position_type = PositionType::Absolute;
            n.left = Val::Px(84.);
            n.bottom = Val::Px(28.);
            n.flex_direction = FlexDirection::Row;
            n.column_gap = Val::Px(8.);
            n.align_items = AlignItems::Center;
        }))
        .with_children(|p| {
            p.spawn(txt(&f.body, 13., FAINT, "Connected"));
            for n in names {
                p.spawn((
                    node(|n| {
                        n.padding = UiRect::axes(Val::Px(8.), Val::Px(1.));
                        n.border_radius = BorderRadius::all(Val::Px(10.));
                    }),
                    BackgroundColor(INK),
                ))
                .with_child(txt(&f.body_bold, 13., DIM, n.clone()));
            }
        });
        p.spawn(node(|n| {
            n.position_type = PositionType::Absolute;
            n.right = Val::Px(40.);
            n.bottom = Val::Px(28.);
            n.flex_direction = FlexDirection::Row;
            n.column_gap = Val::Px(20.);
        }))
        .with_children(|p| {
            if scr == Screen::Title && focus == EDITOR {
                hint(p, f, "Keyboard + mouse", "fighter & stage editor");
            }
            hint(p, f, "Attack / Enter", "select");
            if scr != Screen::Title {
                hint(p, f, "Special / Esc", "back");
            }
        });
    });
}

fn heading(p: &mut ChildSpawnerCommands, f: &Fonts, d: &sim::GameData, s: &str) {
    p.spawn(txt(&f.display, 64., STAR, s));
    p.spawn(node(|n| n.margin = UiRect::new(Val::Px(4.), Val::Px(0.), Val::Px(4.), Val::Px(22.))))
        .with_children(|p| palette_strip(p, d, 22., 4.));
}
