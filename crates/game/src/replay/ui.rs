//! Bevy replay library, playback controls and match transitions.
use super::{model::Replay, player::Playback, storage::*};
use crate::{
    Screen,
    input::Devices,
    play::{self, Match, Mode, Rules, Slot},
    ui::{self, *},
};
use bevy::prelude::*;
use std::{path::PathBuf, sync::Arc};

pub struct ReplayPlugin;

impl Plugin for ReplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Library>()
            .init_resource::<PlaybackUi>()
            .add_systems(OnEnter(Screen::Replays), enter)
            .add_systems(OnEnter(Screen::Playback), enter_playback)
            .add_systems(
                Update,
                (
                    library.in_set(UiSystems::Input).run_if(in_state(Screen::Replays)),
                    controls.in_set(UiSystems::Input).run_if(in_state(Screen::Playback)),
                ),
            );
    }
}

#[derive(Resource)]
pub struct ReturnTo(pub Match);

pub(super) fn watching(mut r: Replay) -> Result<Match, String> {
    r.refresh_saved_path();
    let p = Playback::new(r)?;
    let data = Arc::new(p.replay.data.clone());
    let cfg = &p.replay.config;
    let slots = cfg
        .players
        .iter()
        .enumerate()
        .map(|(i, c)| Slot {
            dev: None,
            fighter: data.fighters.iter().position(|f| f.id == c.fighter).unwrap(),
            cpu: c.cpu,
            team: c.team,
            tap_jump: c.tap_jump,
            tag: format!("P{}", i + 1),
            color: i,
        })
        .collect();
    let mut m = Match::new(
        &data,
        Mode::Replay,
        slots,
        Rules { stocks: cfg.stocks, stage: cfg.stage.clone(), friendly_fire: cfg.friendly_fire },
        cfg.seed,
    )?;
    m.data = data;
    m.world = p.snapshots[0].1.clone();
    m.playback = Some(p);
    Ok(m)
}
fn start(
    r: Replay,
    commands: &mut Commands,
    scene: &mut render::Scene,
    next: &mut NextState<Screen>,
) -> Result<(), String> {
    play::install(commands, scene, watching(r)?);
    next.set(Screen::Playback);
    Ok(())
}
pub fn watch_results(
    m: &mut Match,
    commands: &mut Commands,
    scene: &mut render::Scene,
    next: &mut NextState<Screen>,
) -> Result<(), String> {
    let replay = m.recording.clone().ok_or("No recorded match")?;
    let watching = watching(replay)?;
    let old = std::mem::replace(m, watching);
    commands.insert_resource(ReturnTo(old));
    scene.data = m.data.clone();
    scene.reset(&m.world);
    scene.debug = false;
    next.set(Screen::Playback);
    Ok(())
}
pub(super) fn restore_results(m: &mut Match, ret: &mut ReturnTo, scene: &mut render::Scene) {
    if let Some(r) = &mut ret.0.recording {
        r.saved_path = m.playback.as_ref().and_then(|p| p.replay.saved_path.clone());
    }
    std::mem::swap(m, &mut ret.0);
    scene.data = m.data.clone();
    scene.reset(&m.world);
    scene.debug = m.debug;
}
#[derive(Resource, Default)]
struct Library {
    cursor: Cursor,
    entries: Vec<(PathBuf, String)>,
    warning: String,
    page: usize,
    selected: Option<usize>,
}
#[derive(Clone, PartialEq)]
struct LibraryKey {
    cursor: u32,
    page: usize,
    warning: String,
    selected: Option<usize>,
    entries: usize,
}
fn enter(mut lib: ResMut<Library>) {
    lib.cursor = Cursor { key: 1, owner: None };
    lib.warning.clear();
    lib.page = 0;
    lib.selected = None;
    lib.entries = entries();
}
fn library(
    mut commands: Commands,
    mut lib: ResMut<Library>,
    devs: Res<Devices>,
    q: FocusQuery,
    fonts: Res<Fonts>,
    roots: Roots<LibraryKey>,
    mut scene: ResMut<render::Scene>,
    mut next: ResMut<NextState<Screen>>,
) {
    for (_, nav) in devs.navs() {
        let spots = ui::spots(&q);
        match lib.cursor.drive(nav, &spots) {
            Some(Action::Back) if lib.selected.is_some() => lib.selected = None,
            Some(Action::Back | Action::Activate(0)) => next.set(Screen::Title),
            Some(Action::Activate(k)) if k == u32::MAX - 2 => {
                if let Some(i) = lib.selected
                    && let Err(e) =
                        Replay::read(&lib.entries[i].0).and_then(|r| start(r, &mut commands, &mut scene, &mut next))
                {
                    lib.warning = e;
                }
            }
            Some(Action::Activate(k)) if k == u32::MAX - 3 => {
                if let Some(i) = lib.selected {
                    match delete(&lib.entries[i].0) {
                        Ok(()) => {
                            lib.entries.remove(i);
                            lib.selected = None;
                            lib.page = lib.page.min(lib.entries.len().saturating_sub(1) / 8);
                            lib.cursor.key = 0;
                        }
                        Err(e) => lib.warning = e,
                    }
                }
            }
            Some(Action::Activate(k)) if k == u32::MAX - 4 => lib.selected = None,
            Some(Action::Activate(u32::MAX)) => {
                lib.page = lib.page.saturating_sub(1);
                lib.cursor.key = 0;
            }
            Some(Action::Activate(k)) if k == u32::MAX - 1 => {
                lib.page += 1;
                lib.cursor.key = 0;
            }
            Some(Action::Activate(k)) if (k as usize) <= lib.entries.len() => {
                lib.selected = Some(k as usize - 1);
                lib.cursor.key = u32::MAX - 2;
            }
            _ => {}
        }
    }
    let key = LibraryKey {
        cursor: lib.cursor.key,
        page: lib.page,
        warning: lib.warning.clone(),
        selected: lib.selected,
        entries: lib.entries.len(),
    };
    let Some(mut root) = ui::rebuild(&mut commands, &roots, &key, Screen::Replays, ui::z::SCREEN) else { return };
    root.insert(BackgroundColor(ui::NIGHT)).with_children(|p| {
        p.spawn(node(|n| {
            n.flex_direction = FlexDirection::Column;
            n.padding = UiRect::all(Val::Px(40.));
            n.overflow = Overflow::scroll_y();
            n.height = Val::Percent(100.);
        }))
        .with_children(|p| {
            p.spawn(txt(&fonts.display, 64., STAR, "Replays"));
            p.spawn(txt(&fonts.body, 18., DIM, if lib.entries.is_empty() { "No saved replays" } else { &lib.warning }));
            if let Some(i) = lib.selected {
                p.spawn(txt(&fonts.body, 22., STAR, &lib.entries[i].1));
                menu_list(
                    p,
                    &fonts,
                    &[
                        Item::button(u32::MAX - 2, "Watch"),
                        Item::button(u32::MAX - 3, "Delete replay"),
                        Item::button(u32::MAX - 4, "Cancel"),
                    ],
                    lib.cursor.key,
                    List::new(1150., 24.),
                );
                return;
            }
            let mut items: Vec<_> = lib
                .entries
                .iter()
                .enumerate()
                .skip(lib.page * 8)
                .take(8)
                .map(|(i, (_, label))| Item::button(i as u32 + 1, label))
                .collect();
            if lib.page > 0 {
                items.push(Item::button(u32::MAX, "Previous page"));
            }
            if (lib.page + 1) * 8 < lib.entries.len() {
                items.push(Item::button(u32::MAX - 1, "Next page"));
            }
            items.push(Item::button(0, "Back"));
            menu_list(p, &fonts, &items, lib.cursor.key, List::new(1150., 22.));
        });
    });
}
pub(super) fn seek_tick(x: f32, ticks: u64) -> u64 {
    (f64::from((x + 0.5).clamp(0., 1.)) * ticks as f64).round() as u64
}

#[derive(Resource)]
struct PlaybackUi {
    cursor: Cursor,
    hide_hud: bool,
    message: String,
    pointer_tick: Option<u64>,
}
impl Default for PlaybackUi {
    fn default() -> Self {
        Self { cursor: Cursor { key: 1, owner: None }, hide_hud: false, message: String::new(), pointer_tick: None }
    }
}
fn enter_playback(mut state: ResMut<PlaybackUi>) {
    *state = default();
}
#[derive(Clone, PartialEq)]
struct ControlsKey {
    cursor: u32,
    frozen: bool,
    speed: f64,
    hide_hud: bool,
    warning: Option<String>,
    message: String,
    saved: bool,
    returning: bool,
}
#[derive(Component)]
struct SeekBar;
#[derive(Component)]
struct SeekFill;
#[derive(Component)]
struct PositionText;

fn controls(
    mut commands: Commands,
    mut m: ResMut<Match>,
    mut scene: ResMut<render::Scene>,
    mut next: ResMut<NextState<Screen>>,
    devs: Res<Devices>,
    q: FocusQuery,
    fonts: Res<Fonts>,
    roots: Roots<ControlsKey>,
    mut hud: Query<&mut Node, With<crate::hud::HudRoot>>,
    bars: Query<&bevy::ui::RelativeCursorPosition, With<SeekBar>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut fills: Query<&mut Node, (With<SeekFill>, Without<crate::hud::HudRoot>)>,
    mut positions: Query<&mut Text, With<PositionText>>,
    mut return_to: Option<ResMut<ReturnTo>>,
    mut state: ResMut<PlaybackUi>,
) {
    let m = &mut *m;
    let returning = return_to.is_some();
    let Some(p) = m.playback.as_mut() else { return };
    let mut seek = None;
    let spots = ui::spots(&q);
    for (_, nav) in devs.navs() {
        match state.cursor.drive(nav, &spots) {
            Some(Action::Activate(1) | Action::Start) => m.frozen = !m.frozen,
            Some(Action::Activate(2)) => {
                m.frozen = true;
                m.step = true;
            }
            Some(Action::Spin(3, by)) => {
                let speeds = [0.25, 0.5, 1., 2.];
                let i = speeds.iter().position(|s| *s == m.speed).unwrap_or(2);
                m.speed = speeds[(i as i32 + by).rem_euclid(4) as usize];
            }
            Some(Action::Activate(4)) => state.hide_hud = !state.hide_hud,
            Some(Action::Back | Action::Activate(5)) => {
                if let Some(ret) = return_to.as_mut() {
                    restore_results(m, ret, &mut scene);
                    commands.remove_resource::<ReturnTo>();
                    next.set(Screen::Results);
                } else {
                    next.set(Screen::Replays);
                }
                return;
            }
            Some(Action::Activate(7)) => {
                state.message = match p.replay.save(false) {
                    Ok(_) => "Replay saved".into(),
                    Err(e) => format!("Save failed: {e}"),
                };
            }
            Some(Action::Spin(6, by)) => {
                seek =
                    Some(p.position.saturating_add_signed(i64::from(by) * i64::from(sim::TICK_HZ)).min(p.replay.ticks))
            }
            _ => {}
        }
    }
    for cursor in &bars {
        if mouse.pressed(MouseButton::Left)
            && cursor.cursor_over
            && let Some(pos) = cursor.normalized
        {
            let target = seek_tick(pos.x, p.replay.ticks);
            if state.pointer_tick != Some(target) {
                seek = Some(target);
                state.pointer_tick = Some(target);
            }
        }
    }
    if !mouse.pressed(MouseButton::Left) {
        state.pointer_tick = None;
    }
    if let Some(tick) = seek
        && p.request_seek(tick, &mut m.world)
    {
        m.sounds.clear();
    }
    if p.advance_seek(&mut m.world, sim::TICK_HZ * 4) {
        scene.reset(&m.world);
    }
    for mut node in &mut hud {
        node.display = if state.hide_hud { Display::None } else { Display::Flex };
    }
    for mut fill in &mut fills {
        fill.width = Val::Percent(100. * p.position as f32 / p.replay.ticks.max(1) as f32);
    }
    for mut text in &mut positions {
        text.0 = format!(
            "Tick {} / {}{} · seek: left / right or drag bar",
            p.position,
            p.replay.ticks,
            if p.seeking() { " · Seeking…" } else { "" }
        );
    }
    let key = ControlsKey {
        cursor: state.cursor.key,
        frozen: m.frozen,
        speed: m.speed,
        hide_hud: state.hide_hud,
        warning: p.warning.clone(),
        message: state.message.clone(),
        saved: p.replay.saved_path.is_some(),
        returning,
    };
    let Some(mut root) = ui::rebuild(&mut commands, &roots, &key, Screen::Playback, ui::z::PAUSE) else { return };
    root.insert((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.top = Val::Px(12.);
            n.left = Val::Percent(3.);
            n.width = Val::Percent(94.);
            n.flex_direction = FlexDirection::Column;
            n.padding = UiRect::all(Val::Px(12.));
            n.border_radius = BorderRadius::all(Val::Px(8.));
        }),
        BackgroundColor(ui::INK),
    ))
    .with_children(|parent| {
        parent.spawn(row()).with_children(|row| {
            for item in [
                Item::button(1, if m.frozen { "Play" } else { "Pause" }),
                Item::button(2, "Frame step"),
                Item::spin(3, "Speed", format!("{}×", m.speed)),
                Item::button(4, if state.hide_hud { "Show HUD" } else { "Hide HUD" }),
                Item::button(5, if returning { "Results" } else { "Replays" }),
            ] {
                menu_list(row, &fonts, &[item], state.cursor.key, List::new(182., 21.));
            }
            if p.replay.saved_path.is_none() {
                menu_list(row, &fonts, &[Item::button(7, "Save replay")], state.cursor.key, List::new(182., 21.));
            }
        });
        if !state.message.is_empty() {
            parent.spawn(txt(&fonts.body, 16., STAR, &state.message));
        }
        parent.spawn((
            txt(
                &fonts.body,
                16.,
                DIM,
                format!(
                    "Tick {} / {}{} · seek: left / right or drag bar",
                    p.position,
                    p.replay.ticks,
                    if p.seeking() { " · Seeking…" } else { "" }
                ),
            ),
            PositionText,
        ));
        parent
            .spawn((
                node(|n| {
                    n.width = Val::Percent(100.);
                    n.height = Val::Px(20.);
                    n.margin.top = Val::Px(6.);
                }),
                SeekBar,
                bevy::ui::RelativeCursorPosition::default(),
                BackgroundColor(ui::LINE),
                Focusable { key: 6, owner: None, spin: true },
            ))
            .with_children(|bar| {
                bar.spawn((
                    node(|n| {
                        n.width = Val::Percent(100. * p.position as f32 / p.replay.ticks.max(1) as f32);
                        n.height = Val::Percent(100.);
                    }),
                    SeekFill,
                    BackgroundColor(if state.cursor.key == 6 { ui::GOLD } else { ui::DIM }),
                ));
            });
        if let Some(e) = &p.warning {
            parent.spawn(txt(&fonts.heavy, 20., Color::srgb(1., 0.3, 0.3), e));
        }
    });
}
