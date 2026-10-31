//! Title menu, settings and the controls (remap) screen. Single cursor, driven by every device; the attract match
//! plays behind them.

use bevy::prelude::*;
use bevy::text::LineHeight;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};

use crate::input::{self, ACTS, Dev, Devices};
use crate::play::Mode;
use crate::select::SelectInit;
use crate::settings::{self, Saved};
use crate::ui::{self, *};
use crate::{ContentErrors, Data, Screen};

#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct MenuState {
    pub cursor: Cursor,
    /// Controls screen: the profile being edited and the action waiting for a control.
    pub profile: usize,
    pub capture: Option<usize>,
}

const VERSUS: u32 = 1;
const TRAINING: u32 = 2;
const CONTROLS: u32 = 3;
const SETTINGS: u32 = 4;
const QUIT: u32 = 5;
const KEYBOARD: u32 = 10;
const HITBOXES: u32 = 11;
const WINDOW: u32 = 12;
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
    v.into_iter().map(|d| (d, input::profile(d, &devs.names))).collect()
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    screen: Res<State<Screen>>,
    mut st: ResMut<MenuState>,
    devs: Res<Devices>,
    q: FocusQuery,
    mut saved: ResMut<Saved>,
    mut next: ResMut<NextState<Screen>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut win: Query<&mut Window, With<PrimaryWindow>>,
) {
    let spots = ui::spots(&q);
    let mut s = st.clone();
    let profs = profiles(&saved, &devs);
    s.profile = s.profile.min(profs.len().saturating_sub(1));
    if let Some(a) = s.capture {
        let (pd, prof) = &profs[s.profile];
        let kb = matches!(pd, Dev::Keys | Dev::KeysL | Dev::KeysR);
        let raw = devs
            .map
            .iter()
            .filter(|(d, _)| if kb { matches!(d, Dev::Keys | Dev::KeysL | Dev::KeysR) } else { *d == pd })
            .find_map(|(_, s)| s.raw.clone());
        match raw {
            Some(r) if kb && r == "Escape" => s.capture = None,
            Some(r) => {
                let mut b = input::binds(&saved, prof);
                b[a] = vec![r];
                saved
                    .bindings
                    .insert(prof.clone(), ACTS.iter().zip(b).map(|(act, v)| (act.id().to_string(), v)).collect());
                s.capture = None;
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
            (Screen::Title, Action::Activate(CONTROLS)) => next.set(Screen::Controls),
            (Screen::Title, Action::Activate(SETTINGS)) => next.set(Screen::Settings),
            (Screen::Title, Action::Activate(QUIT)) => {
                exit.write(AppExit::Success);
            }
            (Screen::Title, _) => {}
            (_, Action::Back | Action::Activate(BACK)) => next.set(Screen::Title),
            (Screen::Settings, Action::Spin(KEYBOARD, _)) => saved.split_keyboard = !saved.split_keyboard,
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

#[derive(Component)]
pub struct MenuRoot(String);

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    st: Res<MenuState>,
    saved: Res<Saved>,
    devs: Res<Devices>,
    roots: Query<(Entity, &MenuRoot)>,
    fonts: Res<Fonts>,
    data: Res<Data>,
    errors: Res<ContentErrors>,
) {
    let scr = *screen.get();
    let profs = profiles(&saved, &devs);
    let names: Vec<String> = devs.map.keys().filter(|d| !matches!(d, Dev::Bot(_))).map(|d| devs.name(*d)).collect();
    let sig = format!("{scr:?}{:?}{:?}{names:?}{profs:?}{}", *st, *saved, errors.0.len());
    if roots.iter().any(|(_, r)| r.0 == sig) {
        return;
    }
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let f = &*fonts;
    let d = &data.0;
    let focus = st.cursor.key;
    commands
        .spawn((
            root(),
            MenuRoot(sig),
            DespawnOnExit(scr),
            GlobalZIndex(5),
            BackgroundGradient::from(LinearGradient::to_right(vec![
                ColorStop::new(alpha(NIGHT, 0.95), Val::Percent(0.)),
                ColorStop::new(alpha(NIGHT, 0.9), Val::Percent(if scr == Screen::Title { 30. } else { 55. })),
                ColorStop::new(alpha(NIGHT, 0.2), Val::Percent(if scr == Screen::Title { 70. } else { 90. })),
            ])),
        ))
        .with_children(|p| {
            p.spawn(node(|n| {
                n.flex_direction = FlexDirection::Column;
                n.justify_content = JustifyContent::Center;
                n.padding = UiRect::left(Val::Px(84.));
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
                        Item::button(CONTROLS, "Controls"),
                        Item::button(SETTINGS, "Settings"),
                        Item::button(QUIT, "Quit"),
                    ];
                    menu_list(p, f, &items, focus, 380., 32.);
                }
                Screen::Settings => {
                    heading(p, f, d, "Settings");
                    let onoff = |b: bool| if b { "On" } else { "Off" };
                    let items = [
                        Item::spin(
                            KEYBOARD,
                            "Keyboard",
                            if saved.split_keyboard { "Two players (split)" } else { "One player" },
                        ),
                        Item::spin(HITBOXES, "Hitboxes in versus", onoff(saved.versus_hitboxes)),
                        Item::spin(WINDOW, "Window", if saved.fullscreen { "Fullscreen" } else { "Windowed" }),
                        Item::button(BACK, "Back"),
                    ];
                    menu_list(p, f, &items, focus, 560., 28.);
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
                    let Some((_, prof)) = profs.get(st.profile) else { return };
                    let pname = profs.get(st.profile).map_or_else(String::new, |(dv, _)| devs.name(*dv));
                    let b = input::binds(&saved, prof);
                    let mut items = vec![Item::spin(PROFILE, "Device", pname)];
                    for (i, act) in ACTS.iter().enumerate() {
                        let v = if st.capture == Some(i) {
                            "Press a control…  (Esc cancels)".to_string()
                        } else {
                            b[i].iter().map(|c| input::control_label(c)).collect::<Vec<_>>().join("  or  ")
                        };
                        items.push(Item { key: ACT0 + i as u32, label: act.label().into(), value: Some(v) });
                    }
                    items.push(Item::button(RESET, "Reset to defaults"));
                    items.push(Item::button(BACK, "Back"));
                    p.spawn(node(|n| n.column_gap = Val::Px(40.))).with_children(|p| {
                        controls_list(p, f, &items, focus, st.capture.map(|c| ACT0 + c as u32));
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
                for n in &names {
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

/// Two-column list for bindings: action, then its controls.
fn controls_list(p: &mut ChildSpawnerCommands, f: &Fonts, items: &[Item], focus: u32, capture: Option<u32>) {
    p.spawn(node(|n| {
        n.flex_direction = FlexDirection::Column;
        n.width = Val::Px(560.);
    }))
    .with_children(|p| {
        for it in items {
            let on = it.key == focus;
            let cap = capture == Some(it.key);
            p.spawn((
                node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.align_items = AlignItems::Center;
                    n.height = Val::Px(33.);
                    n.padding = UiRect::horizontal(Val::Px(14.));
                    n.column_gap = Val::Px(12.);
                }),
                BackgroundColor(if on { Color::srgba(1., 0.83, 0.35, 0.08) } else { Color::NONE }),
                Focusable { key: it.key, owner: None, spin: it.key == PROFILE },
            ))
            .with_children(|p| {
                p.spawn((
                    node(|n| {
                        n.width = Val::Px(4.);
                        n.height = Val::Px(20.);
                    }),
                    BackgroundColor(if on { GOLD } else { Color::NONE }),
                    UiTransform::from_rotation(Rot2::degrees(14.)),
                ));
                p.spawn((
                    txt(&f.label, 21., if on { GOLD } else { STAR }, it.label.clone()),
                    node(|n| n.width = Val::Px(170.)),
                ));
                if let Some(v) = &it.value {
                    let v = if on && it.key == PROFILE { format!("‹  {v}  ›") } else { v.clone() };
                    p.spawn(txt(
                        &f.label,
                        20.,
                        if cap {
                            GOLD
                        } else if on {
                            STAR
                        } else {
                            DIM
                        },
                        v,
                    ));
                }
            });
        }
    });
}
