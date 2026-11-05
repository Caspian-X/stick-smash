//! Title menu, settings and the controls (remap) screen. Single cursor, driven by every device; the attract match
//! plays behind them.

use bevy::input::ButtonState;
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
    /// Controls screen: a layout name being typed before saving.
    pub naming: Option<String>,
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
const LOCK60: u32 = 16;
const BACK: u32 = 19;
const TAB0: u32 = 100;
const ACT0: u32 = 30;
const RESET: u32 = 50;
const RIGHT_STICK: u32 = 51;
const AB_SMASH: u32 = 52;
const RANGE: u32 = 53;
pub(crate) const LAYOUT: u32 = 54;
pub(crate) const SAVE_LAYOUT: u32 = 55;
pub(crate) const DELETE_LAYOUT: u32 = 56;
const TAP_JUMP: u32 = 57;

pub fn enter_title(mut st: ResMut<MenuState>) {
    *st = MenuState { cursor: Cursor { key: VERSUS, owner: None }, ..default() };
}
pub fn enter_settings(mut st: ResMut<MenuState>) {
    *st = MenuState { cursor: Cursor { key: KEYBOARD, owner: None }, ..default() };
}
pub fn enter_controls(mut st: ResMut<MenuState>) {
    *st = MenuState { cursor: Cursor { key: TAB0, owner: None }, ..default() };
}

/// A fresh name so a controller-only player can save without typing.
fn new_layout_name(saved: &Saved) -> String {
    (1..).map(|n| format!("Layout {n}")).find(|n| !saved.layouts.contains_key(n)).unwrap()
}

/// Save a profile's effective bindings and options under `name` and make that profile use it.
fn save_layout(saved: &mut Saved, profile: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the layout a name first.".into());
    }
    if settings::reserved_name(name) {
        return Err(format!("\"{name}\" is a built-in profile name; choose another name."));
    }
    if saved.layouts.contains_key(name) {
        return Err(format!("A layout named \"{name}\" already exists. Choose another name."));
    }
    let bindings = ACTS.iter().zip(input::binds(saved, profile)).map(|(a, b)| (a.id().to_string(), b)).collect();
    let layout = settings::Layout {
        family: settings::Family::of(profile),
        bindings,
        options: settings::control_prefs(saved, profile),
    };
    saved.layouts.insert(name.into(), layout);
    saved.layout_choices.insert(profile.into(), name.into());
    Ok(())
}

/// Control profiles that can be edited right now: the keyboard (or its halves) and each connected gamepad.
fn profiles(saved: &Saved, devs: &Devices) -> Vec<(Dev, String)> {
    let mut v: Vec<Dev> = if saved.split_keyboard { vec![Dev::KeysL, Dev::KeysR] } else { vec![Dev::Keys] };
    v.extend(devs.map.keys().filter(|d| matches!(d, Dev::Pad(_))));
    v.into_iter().map(|d| (d, devs.profile(d))).collect()
}

fn controls_navs(devs: &Devices) -> Vec<(Dev, input::Nav)> {
    devs.map
        .iter()
        .flat_map(|(dev, s)| {
            let pad = matches!(dev, Dev::Pad(_));
            let mut nav = s.nav.clone();
            if pad {
                nav.retain(|n| matches!(n, input::Nav::Left | input::Nav::Right | input::Nav::Up | input::Nav::Down));
                nav.extend(match s.raw.as_deref() {
                    Some("South") => Some(input::Nav::Confirm),
                    Some("East") => Some(input::Nav::Back),
                    Some("Start") => Some(input::Nav::Start),
                    _ => None,
                });
            }
            nav.into_iter().map(move |n| (*dev, n))
        })
        .collect()
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
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: MessageReader<bevy::input::keyboard::KeyboardInput>,
) {
    let spots = ui::spots(&q);
    let mut s = st.clone();
    let profs = profiles(&saved, &devs);
    s.profile = s.profile.min(profs.len().saturating_sub(1));
    // Reading every frame keeps old keystrokes from appearing when a name prompt opens.
    let typed: Vec<_> = typed.read().collect();
    if let Some(mut name) = s.naming.take() {
        let navs: Vec<input::Nav> =
            controls_navs(&devs).into_iter().filter(|(dev, _)| !dev.is_keyboard()).map(|(_, n)| n).collect();
        let (mut cancel, mut confirm) = (
            keys.just_pressed(KeyCode::Escape) || navs.contains(&input::Nav::Back),
            navs.iter().any(|n| matches!(n, input::Nav::Confirm | input::Nav::Start)),
        );
        for e in typed.iter().filter(|e| e.state == ButtonState::Pressed && !e.repeat) {
            match e.key_code {
                KeyCode::Escape => cancel = true,
                KeyCode::Enter | KeyCode::NumpadEnter => confirm = true,
                KeyCode::Backspace => {
                    name.pop();
                }
                _ => {
                    for c in e.text.iter().flat_map(|t| t.chars()) {
                        if !c.is_control() && name.chars().count() < 24 {
                            name.push(c);
                        }
                    }
                }
            }
        }
        if confirm && let Some((_, prof)) = profs.get(s.profile) {
            match save_layout(&mut saved, prof, &name) {
                Ok(()) => s.warning = None,
                Err(e) => {
                    s.warning = Some(e);
                    s.naming = Some(name);
                }
            }
        } else if !cancel {
            s.naming = Some(name);
        }
        if s.naming.is_none() {
            devs.clear();
        }
        *st = s;
        return;
    }
    if let Some(a) = s.capture {
        let (pd, prof) = &profs[s.profile];
        let kb = pd.is_keyboard();
        let raw = devs
            .map
            .iter()
            .filter(|(d, _)| if kb { d.is_keyboard() } else { *d == pd })
            .find_map(|(_, s)| s.raw.clone());
        let cancel = devs.map.iter().any(|(d, s)| d.is_keyboard() && s.raw.as_deref() == Some("Escape"))
            || devs.map.get(pd).is_some_and(|s| s.raw.as_deref() == Some("Select"));
        match raw {
            _ if cancel => {
                s.capture = None;
                devs.clear();
            }
            Some(r) if input::keyboard_conflict(&saved, prof, &r) => {
                s.warning = Some(format!(
                    "{} is used by the other keyboard player. Choose another key.",
                    input::control_label(&r)
                ));
            }
            Some(r) => {
                settings::customize_layout(&mut saved, prof);
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
    if *screen.get() == Screen::Controls {
        let cycle =
            devs.map.iter().filter(|(d, _)| matches!(d, Dev::Pad(_))).find_map(|(_, s)| match s.raw.as_deref() {
                Some("LeftTrigger") => Some(-1),
                Some("RightTrigger") => Some(1),
                _ => None,
            });
        if let Some(by) = cycle {
            s.profile = (s.profile as i32 + by).rem_euclid(profs.len() as i32) as usize;
            if (TAB0..TAB0 + profs.len() as u32).contains(&s.cursor.key) {
                s.cursor.key = TAB0 + s.profile as u32;
            }
            s.warning = None;
            devs.clear();
            *st = s;
            return;
        }
    }
    let navs = if *screen.get() == Screen::Controls { controls_navs(&devs) } else { devs.navs().collect() };
    for (dev, nav) in navs {
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
            (Screen::Settings, Action::Spin(LOCK60, _)) => saved.lock_60 = !saved.lock_60,
            (Screen::Settings, Action::Spin(WINDOW, _)) => {
                saved.fullscreen = !saved.fullscreen;
                if let Ok(mut w) = win.single_mut() {
                    w.mode = window_mode(saved.fullscreen);
                }
            }
            (Screen::Controls, Action::Activate(k)) if (TAB0..TAB0 + profs.len() as u32).contains(&k) => {
                s.profile = (k - TAB0) as usize;
                s.warning = None;
            }
            (Screen::Controls, Action::Activate(k)) if (ACT0..ACT0 + 10).contains(&k) => {
                s.capture = Some((k - ACT0) as usize);
                s.warning = None;
                devs.clear();
                break;
            }
            (Screen::Controls, Action::Activate(RESET)) => {
                if let Some((_, prof)) = profs.get(s.profile) {
                    saved.bindings.remove(prof);
                    saved.controls.remove(prof);
                    saved.layout_choices.insert(prof.clone(), String::new());
                    s.warning = None;
                }
            }
            (Screen::Controls, Action::Spin(LAYOUT, by)) => {
                if let Some((_, prof)) = profs.get(s.profile) {
                    settings::cycle_layout(&mut saved, prof, by);
                    s.warning = None;
                }
            }
            (Screen::Controls, Action::Activate(SAVE_LAYOUT)) => {
                if profs.get(s.profile).is_some() {
                    s.naming = Some(new_layout_name(&saved));
                    s.warning = None;
                    devs.clear();
                    break;
                }
            }
            (Screen::Controls, Action::Activate(DELETE_LAYOUT)) => {
                if let Some((_, prof)) = profs.get(s.profile)
                    && let settings::Choice::Named(name) = settings::layout_choice(&saved, prof)
                {
                    settings::delete_layout(&mut saved, &name);
                    s.warning =
                        Some(format!("Deleted layout \"{name}\". Its devices now use their own Custom profiles."));
                }
            }
            (Screen::Controls, Action::Spin(k @ (RIGHT_STICK | AB_SMASH | RANGE | TAP_JUMP), by)) => {
                if let Some((_, prof)) = profs.get(s.profile) {
                    // Options belong to the device's Custom profile; built-ins and named layouts stay shared.
                    settings::customize_layout(&mut saved, prof);
                    let mut c = settings::control_prefs(&saved, prof);
                    match k {
                        RIGHT_STICK => c.stick_tilts = !c.stick_tilts,
                        AB_SMASH => c.ab_smash = !c.ab_smash,
                        TAP_JUMP => c.tap_jump = !c.tap_jump,
                        _ => c.stick_range = (c.stick_range - by as f32 * 0.05).clamp(0.5, 1.),
                    }
                    saved.controls.insert(prof.clone(), c);
                    s.warning = None;
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
                    Item::spin(LOCK60, "Lock sim to 60 Hz display", on_off(saved.lock_60)),
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
                if let Some(name) = &st.naming {
                    p.spawn(txt(&f.label, 22., STAR, format!("Layout name: {name}_")));
                    p.spawn(txt(
                        &f.body,
                        14.,
                        DIM,
                        "Type a name  •  Enter saves  •  Esc cancels  •  the layout keeps this device's bindings and options",
                    ));
                }
                let Some((_, prof)) = profs.get(st.profile) else { return };
                p.spawn(node(|n| {
                    n.column_gap = Val::Px(8.);
                    n.row_gap = Val::Px(8.);
                    n.flex_wrap = FlexWrap::Wrap;
                    n.max_width = Val::Px(960.);
                    n.margin.bottom = Val::Px(4.);
                }))
                .with_children(|p| {
                    for (i, (dev, _)) in profs.iter().enumerate() {
                        let selected = st.profile == i;
                        let key = TAB0 + i as u32;
                        p.spawn((
                            node(|n| {
                                n.padding = UiRect::axes(Val::Px(16.), Val::Px(8.));
                                n.border.bottom = Val::Px(3.);
                            }),
                            BackgroundColor(if focus == key { GOLD.with_alpha(0.16) } else { INK }),
                            BorderColor::all(if selected { GOLD } else { LINE }),
                            Focusable { key, owner: None, spin: false },
                        ))
                        .with_child(txt(&f.label, 18., if selected { GOLD } else { DIM }, devs.name(*dev)));
                    }
                });
                p.spawn(txt(&f.body, 14., DIM, "LB / RB: switch device   •   A / Enter: select   •   B / Esc: back"));
                let b = input::binds(&saved, prof);
                let mut items = vec![];
                for (i, act) in ACTS.iter().enumerate() {
                    let hot = st.capture == Some(i);
                    let v = if hot {
                        "Press a control…  (Esc / Back / View cancels)".to_string()
                    } else {
                        b[i].iter().map(|c| input::control_label(c)).collect::<Vec<_>>().join("  or  ")
                    };
                    items.push(Item { hot, ..Item::info(ACT0 + i as u32, act.label(), v) });
                }
                let c = settings::control_prefs(&saved, prof);
                let choice = settings::layout_choice(&saved, prof);
                if !prof.starts_with("keyboard") {
                    let tilts = if c.stick_tilts { "Tilt attacks" } else { "Smash attacks" };
                    items.push(Item::spin(RIGHT_STICK, "Right stick", tilts));
                    let sens = format!("{:.0}%  (full tilt at {:.0}% travel)", 100. / c.stick_range, c.stick_range * 100.);
                    items.push(Item::spin(RANGE, "Stick sensitivity", sens));
                }
                items.push(Item::spin(AB_SMASH, "Attack + Special", if c.ab_smash { "Smash attack" } else { "Off" }));
                items.push(Item::spin(TAP_JUMP, "Up to jump", on_off(c.tap_jump)));
                items.push(Item::button(RESET, "Reset to defaults"));
                items.push(Item::button(BACK, "Back"));
                let mut layouts = vec![Item::spin(
                    LAYOUT,
                    "Layout",
                    format!("{} ({})", choice.label(), settings::Family::of(prof).label()),
                )];
                layouts.push(Item::button(SAVE_LAYOUT, "Save layout…"));
                if matches!(choice, settings::Choice::Named(_)) {
                    layouts.push(Item::button(DELETE_LAYOUT, "Delete layout"));
                }
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
                        menu_list(p, f, &layouts, focus, List::new(300., 19.));
                        p.spawn(node(|n| n.height = Val::Px(4.)));
                        for line in [
                            "Smashstick is the default profile; Tiltstick is the same with a tilt right stick. Both keep this device's bindings.",
                            "A named layout saves bindings and options (right stick, stick range, attack + special, up to jump) for either family.",
                            "Selecting a profile applies it here and at character select. Changing bindings or options returns the device to Custom.",
                            "Hold attack to charge a smash. Press both jump buttons (or jump twice) to short hop.",
                            "Shield + up attack is an up smash; let go of shield just before a hit to parry.",
                            "On this screen, physical A selects and B goes back, regardless of gameplay bindings.",
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
            hint(p, f, if scr == Screen::Controls { "A / Enter" } else { "Attack / Enter" }, "select");
            if scr != Screen::Title {
                hint(p, f, if scr == Screen::Controls { "B / Esc" } else { "Special / Esc" }, "back");
            }
        });
    });
}

fn heading(p: &mut ChildSpawnerCommands, f: &Fonts, d: &sim::GameData, s: &str) {
    p.spawn(txt(&f.display, 64., STAR, s));
    p.spawn(node(|n| n.margin = UiRect::new(Val::Px(4.), Val::Px(0.), Val::Px(4.), Val::Px(22.))))
        .with_children(|p| palette_strip(p, d, 22., 4.));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::{
        InputPlugin, InputSystems,
        gamepad::{Gamepad, GamepadButton, RawGamepadButtonChangedEvent, RawGamepadEvent},
    };

    fn controls() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin))
            .insert_resource(State::new(Screen::Controls))
            .init_resource::<NextState<Screen>>()
            .insert_resource(Data(crate::test_data()))
            .init_resource::<Saved>()
            .init_resource::<Devices>()
            .insert_resource(MenuState { profile: 1, ..default() })
            .add_message::<AppExit>()
            .add_systems(PreUpdate, input::poll.after(InputSystems))
            .add_systems(Update, update);
        let pad = app.world_mut().spawn((Gamepad::default(), Name::new("Xbox"), pads::Native)).id();
        app.update();
        (app, pad)
    }

    fn button(app: &mut App, pad: Entity, button: GamepadButton, value: f32) {
        app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(pad, button, value)));
        app.update();
    }

    fn focus(app: &mut App, key: u32, spin: bool) {
        app.world_mut().spawn((
            Focusable { key, owner: None, spin },
            ComputedNode { size: Vec2::splat(30.), ..default() },
            UiGlobalTransform::default(),
        ));
        app.world_mut().resource_mut::<MenuState>().cursor.key = key;
    }

    fn type_key(app: &mut App, key: KeyCode, text: Option<&str>) {
        app.world_mut().write_message(bevy::input::keyboard::KeyboardInput {
            key_code: key,
            logical_key: bevy::input::keyboard::Key::Character(text.unwrap_or_default().into()),
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }

    #[test]
    fn layout_rows_cycle_save_and_delete_through_normal_input() {
        let (mut app, pad) = controls();
        let profile = app.world().resource::<Devices>().profile(Dev::Pad(pad));
        focus(&mut app, LAYOUT, true);
        button(&mut app, pad, GamepadButton::DPadRight, 1.);
        assert_eq!(app.world().resource::<Saved>().layout_choices[&profile], settings::TILT);
        button(&mut app, pad, GamepadButton::DPadRight, 0.);

        // Save the current device layout under a typed name.
        focus(&mut app, SAVE_LAYOUT, false);
        button(&mut app, pad, GamepadButton::South, 1.);
        assert_eq!(app.world().resource::<MenuState>().naming.as_deref(), Some("Layout 1"));
        button(&mut app, pad, GamepadButton::South, 0.);
        type_key(&mut app, KeyCode::KeyA, Some("A"));
        type_key(&mut app, KeyCode::Enter, None);
        let saved = app.world().resource::<Saved>().clone();
        assert!(saved.layouts.contains_key("Layout 1A"));
        assert_eq!(saved.layouts["Layout 1A"].family, settings::Family::Gamepad);
        assert_eq!(saved.layout_choices[&profile], "Layout 1A");
        assert!(saved.layouts["Layout 1A"].options.stick_tilts, "the built-in preset was captured");
        assert_eq!(app.world().resource::<MenuState>().naming, None);

        // Delete it: this device falls back to its own layout.
        focus(&mut app, DELETE_LAYOUT, false);
        button(&mut app, pad, GamepadButton::South, 1.);
        let saved = app.world().resource::<Saved>();
        assert!(!saved.layouts.contains_key("Layout 1A"));
        assert_eq!(saved.layout_choices[&profile], "", "deletion falls back to this device's Custom profile");
        button(&mut app, pad, GamepadButton::South, 0.);
        assert_eq!(app.world().resource::<MenuState>().profile, 1);
    }

    #[test]
    fn profile_editor_toggles_up_to_jump_and_saves_it_for_reuse() {
        let (mut app, pad) = controls();
        let profile = app.world().resource::<Devices>().profile(Dev::Pad(pad));
        focus(&mut app, TAP_JUMP, true);
        button(&mut app, pad, GamepadButton::DPadRight, 1.);
        let saved = app.world().resource::<Saved>();
        assert!(!saved.controls[&profile].tap_jump);
        assert_eq!(settings::layout_choice(saved, &profile), settings::Choice::Device);
        button(&mut app, pad, GamepadButton::DPadRight, 0.);

        focus(&mut app, SAVE_LAYOUT, false);
        button(&mut app, pad, GamepadButton::South, 1.);
        button(&mut app, pad, GamepadButton::South, 0.);
        type_key(&mut app, KeyCode::Enter, None);
        let saved = app.world().resource::<Saved>();
        assert!(!saved.layouts["Layout 1"].options.tap_jump);
        assert_eq!(saved.layout_choices[&profile], "Layout 1");
    }

    #[test]
    fn layout_name_ignores_keyboard_gameplay_actions() {
        let (mut app, _) = controls();
        app.world_mut().resource_mut::<MenuState>().naming = Some(String::new());
        for (key, text, expected) in
            [(KeyCode::KeyJ, Some("J"), "J"), (KeyCode::KeyK, Some("K"), "JK"), (KeyCode::Backspace, None, "J")]
        {
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
            type_key(&mut app, key, text);
            assert!(app.world().resource::<ButtonInput<KeyCode>>().pressed(key));
            assert_eq!(app.world().resource::<MenuState>().naming.as_deref(), Some(expected));
            assert!(app.world().resource::<Saved>().layouts.is_empty());
        }
        type_key(&mut app, KeyCode::Enter, None);
        assert!(app.world().resource::<Saved>().layouts.contains_key("J"));
    }

    #[test]
    fn editing_layout_preserves_the_other_effective_settings() {
        let (mut app, pad) = controls();
        let profile = app.world().resource::<Devices>().profile(Dev::Pad(pad));
        app.world_mut().resource_mut::<Saved>().layout_choices.insert(profile.clone(), settings::TILT.into());
        focus(&mut app, TAP_JUMP, true);
        button(&mut app, pad, GamepadButton::DPadRight, 1.);
        let prefs = settings::control_prefs(app.world().resource::<Saved>(), &profile);
        assert!(prefs.stick_tilts);
        assert!(!prefs.tap_jump);
        button(&mut app, pad, GamepadButton::DPadRight, 0.);
        save_layout(&mut app.world_mut().resource_mut::<Saved>(), &profile, "Tilt no jump").unwrap();
        app.world_mut().resource_mut::<MenuState>().capture = Some(input::Act::Attack as usize);
        let before = input::binds(app.world().resource::<Saved>(), &profile);
        button(&mut app, pad, GamepadButton::North, 1.);
        let saved = app.world().resource::<Saved>();
        assert_eq!(settings::control_prefs(saved, &profile), prefs);
        let mut expected = before;
        expected[input::Act::Attack as usize] = vec!["North".into()];
        assert_eq!(input::binds(saved, &profile), expected);
        assert_eq!(saved.layouts["Tilt no jump"].options, prefs);
    }

    #[test]
    fn rebinding_can_swap_face_buttons_despite_their_mapped_menu_actions() {
        let (mut app, pad) = controls();
        let profile = app.world().resource::<Devices>().profile(Dev::Pad(pad));
        // B is initially Special/Back. Binding it to Attack must capture B rather than cancel.
        app.world_mut().resource_mut::<MenuState>().capture = Some(input::Act::Attack as usize);
        button(&mut app, pad, GamepadButton::East, 1.);
        assert_eq!(input::binds(app.world().resource::<Saved>(), &profile)[5], ["East"]);
        assert_eq!(app.world().resource::<MenuState>().capture, None);
        assert!(app.world().resource::<Devices>().map[&Dev::Pad(pad)].nav.is_empty());
        button(&mut app, pad, GamepadButton::East, 0.);

        app.world_mut().resource_mut::<MenuState>().capture = Some(input::Act::Special as usize);
        button(&mut app, pad, GamepadButton::South, 1.);
        assert_eq!(input::binds(app.world().resource::<Saved>(), &profile)[6], ["South"]);
        assert_eq!(app.world().resource::<MenuState>().capture, None);
        button(&mut app, pad, GamepadButton::South, 0.);

        // After the swap, A means Back and B means Confirm; capture still uses physical buttons.
        for (button_id, raw) in [(GamepadButton::South, "South"), (GamepadButton::East, "East")] {
            app.world_mut().resource_mut::<MenuState>().capture = Some(input::Act::Jump as usize);
            button(&mut app, pad, button_id, 1.);
            assert_eq!(input::binds(app.world().resource::<Saved>(), &profile)[4], [raw]);
            button(&mut app, pad, button_id, 0.);
        }
    }

    #[test]
    fn capture_cancel_uses_back_view_or_escape_and_clears_menu_input() {
        let (mut app, pad) = controls();
        let saved = app.world().resource::<Saved>().clone();
        app.world_mut().resource_mut::<MenuState>().capture = Some(input::Act::Attack as usize);
        button(&mut app, pad, GamepadButton::Select, 1.);
        assert_eq!(app.world().resource::<MenuState>().capture, None);
        assert_eq!(*app.world().resource::<Saved>(), saved);
        assert!(app.world().resource::<Devices>().map[&Dev::Pad(pad)].raw.is_none());
        button(&mut app, pad, GamepadButton::Select, 0.);

        app.world_mut().resource_mut::<MenuState>().capture = Some(input::Act::Attack as usize);
        app.world_mut().write_message(bevy::input::keyboard::KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key: bevy::input::keyboard::Key::Escape,
            state: bevy::input::ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        assert_eq!(app.world().resource::<MenuState>().capture, None);
        assert_eq!(*app.world().resource::<Saved>(), saved);
        assert!(app.world().resource::<Devices>().map[&Dev::Keys].nav.is_empty());
    }

    #[test]
    fn starting_capture_stops_other_navigation_from_the_same_button() {
        let (mut app, pad) = controls();
        let profile = app.world().resource::<Devices>().profile(Dev::Pad(pad));
        // A temporary duplicate binding is normal while swapping Attack and Special.
        app.world_mut()
            .resource_mut::<Saved>()
            .bindings
            .insert(profile, [("special".into(), vec!["South".into()])].into());
        let key = ACT0 + input::Act::Attack as u32;
        app.world_mut().resource_mut::<MenuState>().cursor.key = key;
        app.world_mut().spawn((
            Focusable { key, owner: None, spin: false },
            ComputedNode { size: Vec2::splat(20.), ..default() },
            UiGlobalTransform::default(),
        ));
        button(&mut app, pad, GamepadButton::South, 1.);
        assert_eq!(app.world().resource::<MenuState>().capture, Some(input::Act::Attack as usize));
        assert!(matches!(app.world().resource::<NextState<Screen>>(), NextState::Unchanged));
        assert!(app.world().resource::<Devices>().map[&Dev::Pad(pad)].nav.is_empty());
        app.update();
        assert_eq!(app.world().resource::<MenuState>().capture, Some(input::Act::Attack as usize));
    }

    #[test]
    fn controls_navigation_uses_physical_face_buttons_after_a_swap() {
        let pad = Dev::Pad(Entity::PLACEHOLDER);
        let mut devs = Devices::default();
        let s = devs.map.entry(pad).or_default();
        s.raw = Some("South".into());
        s.nav = vec![input::Nav::Back];
        assert_eq!(controls_navs(&devs), [(pad, input::Nav::Confirm)]);
        let s = devs.map.get_mut(&pad).unwrap();
        s.raw = Some("East".into());
        s.nav = vec![input::Nav::Confirm];
        assert_eq!(controls_navs(&devs), [(pad, input::Nav::Back)]);
    }

    #[test]
    fn bumpers_cycle_and_wrap_tabs_but_are_bindable_during_capture() {
        let (mut app, pad) = controls();
        let profile = app.world().resource::<Devices>().profile(Dev::Pad(pad));
        app.world_mut().resource_mut::<Saved>().bindings.insert(
            profile.clone(),
            [("attack".into(), vec!["RightTrigger".into()]), ("special".into(), vec!["LeftTrigger".into()])].into(),
        );
        app.world_mut().resource_mut::<MenuState>().cursor.key = TAB0 + 1;
        button(&mut app, pad, GamepadButton::RightTrigger, 1.);
        assert_eq!(app.world().resource::<MenuState>().profile, 0);
        assert_eq!(app.world().resource::<MenuState>().cursor.key, TAB0);
        assert_eq!(app.world().resource::<MenuState>().capture, None);
        button(&mut app, pad, GamepadButton::RightTrigger, 0.);
        button(&mut app, pad, GamepadButton::LeftTrigger, 1.);
        assert_eq!(app.world().resource::<MenuState>().profile, 1);
        assert_eq!(app.world().resource::<MenuState>().cursor.key, TAB0 + 1);
        button(&mut app, pad, GamepadButton::LeftTrigger, 0.);

        app.world_mut().resource_mut::<MenuState>().capture = Some(input::Act::Shield as usize);
        button(&mut app, pad, GamepadButton::RightTrigger, 1.);
        assert_eq!(app.world().resource::<MenuState>().profile, 1);
        assert_eq!(app.world().resource::<MenuState>().capture, None);
        assert_eq!(input::binds(app.world().resource::<Saved>(), &profile)[8], ["RightTrigger"]);
    }
}
