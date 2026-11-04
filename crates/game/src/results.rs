//! Results: winner, placements and per-player stats over the frozen final moment. Rematch, character select or
//! main menu, from any device.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::text::LineHeight;

use crate::input::{Dev, Devices, Nav};
use crate::play::{self, Match};
use crate::select::team_name;
use crate::ui::{self, *};
use crate::{Data, Screen};

pub struct ResultsPlugin;

impl Plugin for ResultsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ResultsMenu>().add_systems(OnEnter(Screen::Results), enter).add_systems(
            Update,
            (update.in_set(UiSystems::Input), draw.in_set(UiSystems::Draw), animate.in_set(UiSystems::Animate))
                .run_if(in_state(Screen::Results)),
        );
    }
}

const REMATCH: u32 = 1;
const SELECT: u32 = 2;
const MENU: u32 = 3;
const SAVE: u32 = 4;
const WATCH: u32 = 5;

#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub struct ResultsMenu {
    pub cursor: Cursor,
    pub message: String,
    hold: BTreeMap<Dev, Hold>,
    /// `Time::elapsed_secs` when the screen opened; the entrance animation counts from here.
    entered: f32,
}

/// The results backdrop, dimming the match behind it as the screen enters.
#[derive(Component)]
struct Backdrop;

/// A results element that rises and scales into place `delay` real seconds after the screen opened.
#[derive(Component)]
struct Enter {
    delay: f32,
}

/// One device's hold on the focused button: it fires once, when the hold is long enough.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Hold {
    key: u32,
    secs: f32,
    fired: bool,
}

impl Hold {
    /// Steps the hold on `key` by `dt` seconds while `held`; true on the frame it fires.
    fn step(&mut self, key: u32, held: bool, dt: f32, need: f32) -> bool {
        if self.key != key {
            *self = Hold { key, ..default() };
        }
        if !held {
            (self.secs, self.fired) = (0., false);
            return false;
        }
        if self.fired {
            return false;
        }
        self.secs += dt;
        self.fired = self.secs >= need;
        self.fired
    }
}

fn enter(mut rm: ResMut<ResultsMenu>, mut m: ResMut<Match>, time: Res<Time>) {
    rm.hold.clear();
    rm.entered = time.elapsed_secs();
    if m.results_shown {
        return;
    }
    m.results_shown = true;
    // Nothing is focused until a player steers, so rematch is not a default a stray press can take.
    rm.cursor = Cursor { key: NONE, owner: None };
    rm.message.clear();
}

/// Stops the recording with the match and auto-saves it, returning the message to show.
fn finish(m: &mut Match, rm: &mut ResultsMenu, shots: bool) {
    m.recording_done = true;
    rm.message = m.recording.as_mut().filter(|r| !shots && r.data.global.replay.auto_save_count > 0).map_or_else(
        String::new,
        |r| match r.save(true) {
            Ok(_) => "Replay auto-saved".into(),
            Err(e) => format!("Replay save failed: {e}"),
        },
    );
}

fn update(
    testing: Option<Res<crate::editing::Testing>>,
    mut m: ResMut<Match>,
    mut rm: ResMut<ResultsMenu>,
    mut devs: ResMut<Devices>,
    q: FocusQuery,
    data: Res<Data>,
    time: Res<Time>,
    shots: Option<Res<crate::shots::Script>>,
    mut next: ResMut<NextState<Screen>>,
    mut scene: ResMut<render::Scene>,
    mut commands: Commands,
) {
    // The screen opens during the end freeze, before the match is over: it keeps ticking (and recording) behind
    // the results, so the replay is finished and auto-saved once the end arrives.
    if !m.recording_done && m.world.over {
        finish(&mut m, &mut rm, shots.is_some());
    }
    let spots = ui::spots(&q);
    let mut c = rm.cursor;
    let mut back = false;
    for (_, nav) in devs.navs().collect::<Vec<_>>() {
        match nav {
            Nav::Up | Nav::Down | Nav::Left | Nav::Right => {
                // Steering picks the first button to move from.
                if c.key == NONE {
                    c.key = REMATCH;
                }
                c.drive(nav, &spots);
            }
            Nav::Back => back = true,
            _ => {}
        }
    }
    if rm.cursor != c {
        rm.cursor = c;
    }
    // A held control fires the focused button, so a press buffered from the match cannot pick one.
    let need = m.data.global.render.hud.results_hold_secs.max(1e-3);
    let dt = time.delta_secs().clamp(0., 0.05);
    rm.hold.retain(|dev, _| devs.map.contains_key(dev));
    let mut fire = false;
    if c.key != NONE {
        for (dev, st) in &devs.map {
            let held = st.confirm_held() || st.start_held();
            let h = rm.hold.entry(*dev).or_default();
            fire |= h.step(c.key, held, dt, need);
        }
    }
    match fire.then_some(c.key) {
        Some(REMATCH) => {
            play::restart(&mut m, &data.0, &mut scene, &mut devs);
            next.set(Screen::Match);
        }
        Some(SAVE) if m.recording_done => {
            if let Some(r) = &mut m.recording {
                rm.message = match r.save(false) {
                    Ok(p) => format!("Saved {}", p.display()),
                    Err(e) => format!("Replay save failed: {e}"),
                };
            }
        }
        Some(WATCH) if m.recording_done => {
            if let Err(e) = crate::replay::watch_results(&mut m, &mut commands, &mut scene, &mut next) {
                rm.message = e;
            }
        }
        Some(MENU) if testing.is_some() => next.set(Screen::Editor),
        Some(MENU) => next.set(Screen::Title),
        Some(SELECT) => next.set(Screen::Select),
        _ => {}
    }
    if back {
        // A test match from the editor goes back there.
        next.set(if testing.is_some() { Screen::Editor } else { Screen::Select });
    }
}

/// What the results screen shows besides the frozen match.
#[derive(Clone, PartialEq)]
pub struct ResultsKey {
    cursor: u32,
    message: String,
}

fn draw(
    testing: Option<Res<crate::editing::Testing>>,
    mut commands: Commands,
    m: Res<Match>,
    rm: Res<ResultsMenu>,
    roots: Roots<ResultsKey>,
    fonts: Res<Fonts>,
) {
    let key = ResultsKey { cursor: rm.cursor.key, message: rm.message.clone() };
    let Some(mut root) = ui::rebuild(&mut commands, &roots, &key, Screen::Results, z::SCREEN) else { return };
    let f = &*fonts;
    let d = &m.data;
    let w = &m.world;
    let place = m.placements();
    let mut order: Vec<usize> = (0..w.fighters.len()).collect();
    order.sort_by_key(|&i| (place[i], i));
    let (title, color) = match w.winner {
        Some(i) => match m.slots[i].team {
            Some(t) => (format!("{} wins", team_name(t)), pal(d, i).0),
            None => (format!("{} wins", m.slots[i].tag), pal(d, i).0),
        },
        None => ("Draw".into(), STAR),
    };
    let sub = w.winner.map(|i| d.fighters[w.fighters[i].data].name.clone());
    let n = order.len();
    let cw = card_width(n, 230.);
    let stagger = m.data.global.render.hud.results_enter.stagger;
    root.insert(Backdrop).insert(background(0.)).with_children(|p| {
        p.spawn((
            node(|n| {
                n.flex_direction = FlexDirection::Column;
                n.padding = UiRect::new(Val::Px(70.), Val::Px(70.), Val::Px(50.), Val::Px(30.));
                n.height = Val::Percent(100.);
            }),
            Enter { delay: 0. },
            UiTransform::IDENTITY,
        ))
        .with_children(|p| {
            p.spawn((
                txt(&f.display, 110., color, title),
                TextShadow { offset: Vec2::new(0., 5.), color: NIGHT.with_alpha(0.8) },
                LineHeight::RelativeToFont(0.9),
            ));
            if let Some(s) = sub {
                p.spawn(txt(&f.heavy, 26., DIM, s));
            }
            p.spawn(txt(&f.body, 16., DIM, &rm.message));
            p.spawn(node(|n| n.flex_grow = 1.));
            p.spawn(node(|n| {
                n.flex_direction = FlexDirection::Row;
                n.column_gap = Val::Px(CARD_GAP);
                n.align_items = AlignItems::FlexEnd;
                n.margin.bottom = Val::Px(30.);
            }))
            .with_children(|p| {
                for (k, i) in order.iter().copied().enumerate() {
                    let delay = stagger * (k + 1) as f32;
                    column(p, f, &m, i, place[i], cw, delay);
                }
            });
            p.spawn((
                node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.column_gap = Val::Px(14.);
                }),
                Enter { delay: stagger * (n + 1) as f32 },
                UiTransform::IDENTITY,
            ))
            .with_children(|p| {
                // Editor tests go back to the editor and leave no replay.
                let items: &[(u32, &str)] = if testing.is_some() {
                    &[(REMATCH, "Rematch"), (MENU, "Back to editor")]
                } else {
                    &[
                        (REMATCH, "Rematch"),
                        (SELECT, "Character select"),
                        (MENU, "Main menu"),
                        (SAVE, "Save replay"),
                        (WATCH, "Watch replay"),
                    ]
                };
                for &(k, label) in items {
                    let on = rm.cursor.key == k;
                    p.spawn((
                        node(|n| {
                            n.padding = UiRect::axes(Val::Px(22.), Val::Px(6.));
                            n.border = UiRect::all(Val::Px(2.));
                            n.border_radius = BorderRadius::all(Val::Px(8.));
                        }),
                        BackgroundColor(if on { GOLD.with_alpha(0.14) } else { INK_SOFT }),
                        BorderColor::all(if on { GOLD } else { LINE.with_alpha(0.8) }),
                        Focusable { key: k, owner: None, spin: false },
                    ))
                    .with_child(txt(&f.label, 21., if on { GOLD } else { STAR }, label));
                }
            });
        });
    });
}

fn column(p: &mut ChildSpawnerCommands, f: &Fonts, m: &Match, i: usize, place: usize, w: f32, delay: f32) {
    let d = &m.data;
    let (main, dark, _) = pal(d, i);
    let s = &m.stats[i];
    let first = place == 1;
    p.spawn((
        node(|n| {
            n.width = Val::Px(w);
            n.flex_direction = FlexDirection::Column;
            n.border_radius = BorderRadius::all(Val::Px(10.));
            n.overflow = Overflow::clip();
            n.border = UiRect::all(Val::Px(2.));
        }),
        BackgroundColor(INK),
        BorderColor::all(if first { main } else { main.with_alpha(0.3) }),
        BoxShadow::new(
            main.with_alpha(if first { 0.45 } else { 0. }),
            Val::Px(0.),
            Val::Px(0.),
            Val::Px(2.),
            Val::Px(22.),
        ),
        Enter { delay },
        UiTransform::IDENTITY,
    ))
    .with_children(|p| {
        p.spawn((
            node(|n| {
                n.flex_direction = FlexDirection::Row;
                n.align_items = AlignItems::Baseline;
                n.justify_content = JustifyContent::SpaceBetween;
                n.padding = UiRect::new(Val::Px(14.), Val::Px(14.), Val::Px(2.), Val::Px(0.));
                n.min_height = Val::Px(if first { 92. } else { 70. });
            }),
            BackgroundGradient::from(LinearGradient::to_right(vec![ColorStop::auto(main), ColorStop::auto(dark)])),
        ))
        .with_children(|p| {
            p.spawn(txt(&f.display, if first { 72. } else { 52. }, NIGHT, ordinal(place)));
            p.spawn(txt(&f.display, 26., NIGHT.with_alpha(0.85), m.slots[i].tag.clone()));
        });
        p.spawn(node(|n| {
            n.flex_direction = FlexDirection::Column;
            n.padding = UiRect::all(Val::Px(14.));
            n.row_gap = Val::Px(4.);
        }))
        .with_children(|p| {
            p.spawn((
                txt(&f.heavy, 26., STAR, d.fighters[m.slots[i].fighter].name.clone()),
                node(|n| n.margin.bottom = Val::Px(6.)),
            ));
            let mut lines = vec![
                ("KOs", s.kos.to_string()),
                ("Falls", s.falls.to_string()),
                ("Damage dealt", format!("{:.0}%", s.dealt)),
                ("Damage taken", format!("{:.0}%", s.taken)),
            ];
            if s.sds > 0 {
                lines.push(("Self-destructs", s.sds.to_string()));
            }
            for (k, v) in lines {
                p.spawn(node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.justify_content = JustifyContent::SpaceBetween;
                }))
                .with_children(|p| {
                    p.spawn(txt(&f.label, 17., DIM, k));
                    p.spawn(txt(&f.heavy, 19., STAR, v));
                });
            }
        });
    });
}

/// The results backdrop at entrance `k` (`0`: the match shows through, `1`: fully dimmed).
fn background(k: f32) -> BackgroundGradient {
    BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::new(NIGHT.with_alpha(0.5 * k), Val::Percent(0.)),
        ColorStop::new(NIGHT.with_alpha(0.82 * k), Val::Percent(45.)),
        ColorStop::new(NIGHT.with_alpha(0.95 * k), Val::Percent(100.)),
    ]))
}

/// How far an entrance `t` seconds in has settled (`0..1`), eased out.
fn settled(t: f32, secs: f32) -> f32 {
    1. - (1. - (t / secs.max(1e-3)).clamp(0., 1.)).powi(3)
}

/// Real-time entrance: the content rises and scales in, the cards a beat apart, while the backdrop darkens.
fn animate(
    time: Res<Time>,
    m: Res<Match>,
    rm: Res<ResultsMenu>,
    mut enters: Query<(&Enter, &mut UiTransform)>,
    mut backdrops: Query<&mut BackgroundGradient, With<Backdrop>>,
) {
    let look = &m.data.global.render.hud.results_enter;
    let now = time.elapsed_secs();
    for (e, mut tf) in &mut enters {
        let k = settled(now - rm.entered - e.delay, look.secs);
        let (y, s) = (Val::Px((1. - k) * look.slide), 0.98 + 0.02 * k);
        if tf.translation.y != y || tf.scale.x != s {
            tf.translation.y = y;
            tf.scale = Vec2::splat(s);
        }
    }
    let want = background(settled(now - rm.entered, look.secs));
    for mut g in &mut backdrops {
        if *g != want {
            *g = want.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entrance_settles_in_real_seconds() {
        assert_eq!(settled(0., 0.3), 0.);
        assert_eq!(settled(-1., 0.3), 0., "a delayed element stays back");
        assert_eq!(settled(0.3, 0.3), 1.);
        assert_eq!(settled(9., 0.3), 1.);
        let mid = settled(0.1, 0.3);
        assert!(mid > 0. && mid < 1., "between the ends: {mid}");
        assert!(settled(0.1, 0.3) < settled(0.2, 0.3), "and monotonic");
    }

    #[test]
    fn the_entrance_lifts_its_content_into_place() {
        use std::time::Duration;
        let data = crate::test_data();
        let slot = |i: usize| crate::play::Slot {
            dev: None,
            fighter: i,
            cpu: Some(1),
            team: None,
            tap_jump: true,
            controls: Default::default(),
            tag: "CPU".into(),
            color: i,
        };
        let rules = crate::play::Rules { stocks: 1, stage: "arena".into(), friendly_fire: false, time_limit: None };
        let m = Match::new(&data, crate::play::Mode::Versus, (0..2).map(slot).collect(), rules, 7).unwrap();
        let slide = m.data.global.render.hud.results_enter.slide;
        let secs = m.data.global.render.hud.results_enter.secs;
        let mut app = App::new();
        app.insert_resource(m)
            .insert_resource(ResultsMenu::default())
            .init_resource::<Time>()
            .add_systems(Update, animate);
        let e = app.world_mut().spawn((Enter { delay: 0. }, UiTransform::IDENTITY)).id();
        app.update();
        let tf = *app.world().get::<UiTransform>(e).unwrap();
        assert_eq!(tf.translation.y, Val::Px(slide), "starts below its place");
        assert!(tf.scale.x < 1.);
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_secs_f32(secs + 0.5));
        app.update();
        let tf = *app.world().get::<UiTransform>(e).unwrap();
        assert_eq!(tf.translation.y, Val::Px(0.), "settled");
        assert_eq!(tf.scale, Vec2::ONE);
    }

    #[test]
    fn a_button_fires_once_after_a_full_hold() {
        let mut h = Hold::default();
        assert!(!h.step(REMATCH, true, 0.1, 0.25));
        assert!(!h.step(REMATCH, true, 0.1, 0.25));
        assert!(h.step(REMATCH, true, 0.1, 0.25), "fires after a quarter second");
        assert!(!h.step(REMATCH, true, 0.1, 0.25), "only once per hold");
        assert!(!h.step(REMATCH, false, 0.1, 0.25));
        assert!(!h.step(REMATCH, true, 0.1, 0.25), "releasing arms it again");
        assert!(!h.step(SELECT, true, 0.1, 0.25), "another button needs its own hold");
        assert!(h.step(SELECT, true, 0.2, 0.25));
    }
}
