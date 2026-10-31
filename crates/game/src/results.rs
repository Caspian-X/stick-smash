//! Results: winner, placements and per-player stats over the frozen final moment. Rematch, character select or
//! main menu, from any device.

use bevy::prelude::*;
use bevy::text::LineHeight;

use crate::input::Devices;
use crate::play::{self, Match};
use crate::select::team_name;
use crate::ui::{self, *};
use crate::{Data, Screen};

const REMATCH: u32 = 1;
const SELECT: u32 = 2;
const MENU: u32 = 3;
const SAVE: u32 = 4;
const WATCH: u32 = 5;

#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub struct ResultsMenu {
    pub cursor: Cursor,
    pub message: String,
}

pub fn enter(mut rm: ResMut<ResultsMenu>, mut m: ResMut<Match>, shots: Option<Res<crate::shots::Script>>) {
    if m.recording_done {
        return;
    }
    m.recording_done = true;
    rm.message = m
        .recording
        .as_mut()
        .filter(|r| shots.is_none() && r.data.global.replay.auto_save_count > 0)
        .map_or_else(String::new, |r| match r.save(true) {
            Ok(_) => "Replay auto-saved".into(),
            Err(e) => format!("Replay save failed: {e}"),
        });
    rm.cursor = Cursor { key: REMATCH, owner: None };
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    testing: Option<Res<crate::editing::Testing>>,
    mut m: ResMut<Match>,
    mut rm: ResMut<ResultsMenu>,
    devs: Res<Devices>,
    q: FocusQuery,
    data: Res<Data>,
    mut next: ResMut<NextState<Screen>>,
    mut scene: ResMut<render::Scene>,
    mut commands: Commands,
) {
    let spots = ui::spots(&q);
    let mut c = rm.cursor;
    for (_, nav) in devs.navs().collect::<Vec<_>>() {
        match c.drive(nav, &spots) {
            Some(Action::Activate(REMATCH) | Action::Start) => {
                m.restart(&data.0, play::seed());
                scene.reset(&m.world);
                next.set(Screen::Match);
            }
            // A test match from the editor goes back there.
            Some(Action::Activate(MENU) | Action::Back) if testing.is_some() => next.set(Screen::Editor),
            Some(Action::Activate(SELECT) | Action::Back) => next.set(Screen::Select),
            Some(Action::Activate(MENU)) => next.set(Screen::Title),
            Some(Action::Activate(SAVE)) => {
                if let Some(r) = &mut m.recording {
                    rm.message = match r.save(false) {
                        Ok(p) => format!("Saved {}", p.display()),
                        Err(e) => format!("Replay save failed: {e}"),
                    };
                }
            }
            Some(Action::Activate(WATCH)) => {
                if let Err(e) = crate::replay::watch_results(&mut m, &mut commands, &mut scene, &mut next) {
                    rm.message = e;
                }
            }
            _ => {}
        }
    }
    if rm.cursor != c {
        rm.cursor = c;
    }
}

#[derive(Component)]
pub struct ResultsRoot(u32, String);

pub fn draw(
    testing: Option<Res<crate::editing::Testing>>,
    mut commands: Commands,
    m: Res<Match>,
    rm: Res<ResultsMenu>,
    roots: Query<(Entity, &ResultsRoot)>,
    fonts: Res<Fonts>,
) {
    if roots.iter().any(|(_, r)| r.0 == rm.cursor.key && r.1 == rm.message) {
        return;
    }
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
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
    let cw = (1180. / n as f32 - 12.).min(230.);
    commands
        .spawn((
            root(),
            ResultsRoot(rm.cursor.key, rm.message.clone()),
            DespawnOnExit(Screen::Results),
            GlobalZIndex(5),
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::new(alpha(NIGHT, 0.5), Val::Percent(0.)),
                ColorStop::new(alpha(NIGHT, 0.82), Val::Percent(45.)),
                ColorStop::new(alpha(NIGHT, 0.95), Val::Percent(100.)),
            ])),
        ))
        .with_children(|p| {
            p.spawn(node(|n| {
                n.flex_direction = FlexDirection::Column;
                n.padding = UiRect::new(Val::Px(70.), Val::Px(70.), Val::Px(50.), Val::Px(30.));
                n.height = Val::Percent(100.);
            }))
            .with_children(|p| {
                p.spawn((
                    txt(&f.display, 110., color, title),
                    TextShadow { offset: Vec2::new(0., 5.), color: alpha(NIGHT, 0.8) },
                    LineHeight::RelativeToFont(0.9),
                ));
                if let Some(s) = sub {
                    p.spawn(txt(&f.heavy, 26., DIM, s));
                }
                p.spawn(txt(&f.body, 16., DIM, &rm.message));
                p.spawn(node(|n| n.flex_grow = 1.));
                p.spawn(node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.column_gap = Val::Px(12.);
                    n.align_items = AlignItems::FlexEnd;
                    n.margin.bottom = Val::Px(30.);
                }))
                .with_children(|p| {
                    for &i in &order {
                        column(p, f, &m, i, place[i], cw);
                    }
                });
                p.spawn(node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.column_gap = Val::Px(14.);
                }))
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
                            BackgroundColor(if on { alpha(GOLD, 0.14) } else { INK_SOFT }),
                            BorderColor::all(if on { GOLD } else { alpha(LINE, 0.8) }),
                            Focusable { key: k, owner: None, spin: false },
                        ))
                        .with_child(txt(&f.label, 21., if on { GOLD } else { STAR }, label));
                    }
                });
            });
        });
}

fn column(p: &mut ChildSpawnerCommands, f: &Fonts, m: &Match, i: usize, place: usize, w: f32) {
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
        BorderColor::all(if first { main } else { alpha(main, 0.3) }),
        BoxShadow::new(alpha(main, if first { 0.45 } else { 0. }), Val::Px(0.), Val::Px(0.), Val::Px(2.), Val::Px(22.)),
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
            p.spawn(txt(&f.display, 26., alpha(NIGHT, 0.85), m.slots[i].tag.clone()));
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
