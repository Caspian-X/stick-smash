//! In-match HUD: player panels along the bottom (percent with damage color and hit shake, stocks), name tags,
//! offscreen bubbles at the screen edge, the countdown / GO! / GAME! text, and the pause menu.

use bevy::prelude::*;
use bevy::text::LineHeight;
use bevy::window::PrimaryWindow;
use render::Framing;
use sim::world::State as FState;

use crate::input::{Dev, Devices, Nav};
use crate::play::{self, Match, Mode};
use crate::settings::{DummyMode, Saved};
use crate::ui::{self, *};
use crate::{Data, Screen};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        let in_match = in_state(Screen::Match);
        app.init_resource::<PauseMenu>()
            .add_systems(OnEnter(Screen::Match), spawn)
            .add_systems(OnEnter(Screen::Playback), spawn)
            .add_systems(
                Update,
                (
                    (play::match_flow, pause).chain().in_set(UiSystems::Input).run_if(in_match.clone()),
                    draw_pause.in_set(UiSystems::Draw).run_if(in_match.clone()),
                    (percents, stocks, tags, bubbles, big)
                        .in_set(UiSystems::Animate)
                        .run_if(in_match.or_else(in_state(Screen::Playback))),
                ),
            );
    }
}

#[derive(Component)]
pub struct Pct(usize);
#[derive(Component)]
pub struct PctSign(usize);
#[derive(Component)]
pub struct Panel(usize);
#[derive(Component)]
pub struct Stock(usize, i32);
#[derive(Component)]
pub struct Tag(usize);
#[derive(Component)]
pub struct Bubble(usize);
#[derive(Component)]
pub struct BubblePct(usize);
#[derive(Component)]
pub struct BubbleNub(usize);
#[derive(Component)]
pub struct Big;
#[derive(Component)]
pub struct HudRoot;

const MAX_STOCK_ICONS: i32 = 5;
const BUBBLE: f32 = 54.;

fn spawn(mut commands: Commands, m: Res<Match>, fonts: Res<Fonts>, screen: Res<State<Screen>>) {
    let f = &*fonts;
    let d = &m.data;
    let n = m.slots.len();
    let w = card_width(n, 236.);
    let training = m.mode == Mode::Training;
    commands.spawn((root(), HudRoot, DespawnOnExit(*screen.get()), GlobalZIndex(z::HUD))).with_children(|p| {
        for (i, s) in m.slots.iter().enumerate() {
            let (main, _, _) = pal(d, i);
            p.spawn((
                node(|n| {
                    n.position_type = PositionType::Absolute;
                    n.flex_direction = FlexDirection::Column;
                    n.align_items = AlignItems::Center;
                }),
                Tag(i),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                p.spawn((
                    txt(&f.heavy, 17., main, s.tag.clone()),
                    TextShadow { offset: Vec2::new(0., 1.5), color: NIGHT.with_alpha(0.9) },
                ));
                p.spawn((
                    node(|n| {
                        n.width = Val::Px(0.);
                        n.height = Val::Px(0.);
                        n.border = UiRect::new(Val::Px(5.), Val::Px(5.), Val::Px(6.), Val::Px(0.));
                    }),
                    BorderColor { top: main, left: Color::NONE, right: Color::NONE, bottom: Color::NONE },
                ));
            });
            p.spawn((
                node(|n| {
                    n.position_type = PositionType::Absolute;
                    n.width = Val::Px(BUBBLE);
                    n.height = Val::Px(BUBBLE);
                    n.border = UiRect::all(Val::Px(3.));
                    n.border_radius = BorderRadius::MAX;
                    n.flex_direction = FlexDirection::Column;
                    n.align_items = AlignItems::Center;
                    n.justify_content = JustifyContent::Center;
                }),
                BackgroundColor(INK),
                BorderColor::all(main),
                BoxShadow::new(main.with_alpha(0.5), Val::Px(0.), Val::Px(0.), Val::Px(0.), Val::Px(12.)),
                Bubble(i),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                p.spawn((
                    node(|n| {
                        n.position_type = PositionType::Absolute;
                        n.width = Val::Px(12.);
                        n.height = Val::Px(12.);
                    }),
                    BackgroundColor(main),
                    BubbleNub(i),
                ));
                p.spawn((txt(&f.heavy, 15., main, s.tag.clone()), LineHeight::RelativeToFont(0.9)));
                p.spawn((txt(&f.display, 15., STAR, "0%"), LineHeight::RelativeToFont(0.9), BubblePct(i)));
            });
        }
        // Panels.
        p.spawn((
            Safe::Bottom,
            node(|n| {
                n.position_type = PositionType::Absolute;
                n.bottom = Val::Px(14.);
                n.left = Val::Px(0.);
                n.right = Val::Px(0.);
                n.flex_direction = FlexDirection::Row;
                n.justify_content = JustifyContent::Center;
                n.column_gap = Val::Px(CARD_GAP);
            }),
        ))
        .with_children(|p| {
            for (i, s) in m.slots.iter().enumerate() {
                let (main, dark, _) = pal(d, i);
                let fighter = &d.fighters[s.fighter];
                let tint = s.team.map_or(INK, |_| dark.mix(&NIGHT, 0.55).with_alpha(0.92));
                p.spawn((
                    node(|n| {
                        n.width = Val::Px(w);
                        n.flex_direction = FlexDirection::Row;
                        n.border_radius = BorderRadius::all(Val::Px(10.));
                        n.overflow = Overflow::clip();
                    }),
                    BackgroundGradient::from(LinearGradient::to_right(vec![
                        ColorStop::auto(tint),
                        ColorStop::auto(INK),
                    ])),
                    Panel(i),
                ))
                .with_children(|p| {
                    p.spawn((
                        node(|n| {
                            n.width = Val::Px(6.);
                            n.height = Val::Percent(100.);
                        }),
                        BackgroundColor(main),
                    ));
                    p.spawn(node(|n| {
                        n.flex_direction = FlexDirection::Column;
                        n.flex_grow = 1.;
                        n.padding = UiRect::new(Val::Px(10.), Val::Px(12.), Val::Px(6.), Val::Px(8.));
                    }))
                    .with_children(|p| {
                        p.spawn(node(|n| {
                            n.flex_direction = FlexDirection::Row;
                            n.justify_content = JustifyContent::SpaceBetween;
                            n.align_items = AlignItems::Baseline;
                        }))
                        .with_children(|p| {
                            p.spawn(txt(&f.heavy, 22., main, s.tag.clone()));
                            p.spawn(txt(&f.label, 15., DIM, fighter.name.clone()));
                        });
                        p.spawn(node(|n| {
                            n.flex_direction = FlexDirection::Row;
                            n.justify_content = JustifyContent::SpaceBetween;
                            n.align_items = AlignItems::FlexEnd;
                        }))
                        .with_children(|p| {
                            p.spawn(node(|n| {
                                n.flex_direction = FlexDirection::Row;
                                n.column_gap = Val::Px(5.);
                                n.margin.bottom = Val::Px(9.);
                                n.align_items = AlignItems::Center;
                            }))
                            .with_children(|p| {
                                if training {
                                    p.spawn(txt(&f.label, 15., FAINT, if i == 0 { "Training" } else { "Dummy" }));
                                } else if m.rules.stocks > MAX_STOCK_ICONS {
                                    stock_icon(p, main, i, 1);
                                    p.spawn((txt(&f.heavy, 18., STAR, ""), Stock(i, 0)));
                                } else {
                                    for k in 1..=m.rules.stocks {
                                        stock_icon(p, main, i, k);
                                    }
                                }
                            });
                            p.spawn((
                                txt(&f.display, 62., STAR, "0"),
                                LineHeight::RelativeToFont(0.95),
                                Pct(i),
                                UiTransform::default(),
                            ))
                            .with_child((
                                TextSpan::new("%"),
                                TextFont { font: f.display.clone().into(), font_size: FontSize::Px(30.), ..default() },
                                PctSign(i),
                            ));
                        });
                    });
                });
            }
        });
        p.spawn((
            node(|n| {
                n.position_type = PositionType::Absolute;
                n.width = Val::Percent(100.);
                n.top = Val::Percent(30.);
                n.justify_content = JustifyContent::Center;
            }),
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                txt(&f.display, 180., STAR, ""),
                TextShadow { offset: Vec2::new(0., 6.), color: NIGHT.with_alpha(0.7) },
                Big,
                UiTransform::default(),
            ));
        });
    });
}

fn stock_icon(p: &mut ChildSpawnerCommands, c: Color, i: usize, k: i32) {
    p.spawn((
        node(|n| {
            n.width = Val::Px(13.);
            n.height = Val::Px(13.);
            n.border = UiRect::all(Val::Px(2.));
            n.border_radius = BorderRadius::MAX;
        }),
        BackgroundColor(c),
        BorderColor::all(STAR.with_alpha(0.8)),
        Stock(i, k),
    ));
}

/// Percents: the damage number in its ramp color, shaking while hit; "Out" once out of stocks.
fn percents(
    m: Res<Match>,
    time: Res<Time>,
    mut pcts: Query<(&Pct, &mut Text, &mut TextColor, &mut UiTransform)>,
    mut signs: Query<(&PctSign, &mut TextSpan)>,
) {
    let (w, d) = (&m.world, &m.data);
    let look = &d.global.render.hud;
    let t = time.elapsed_secs();
    for (p, mut text, mut color, mut tf) in &mut pcts {
        let f = &w.fighters[p.0];
        let out = f.state == FState::Out;
        text.0 = if out { "Out".into() } else { format!("{}", f.dmg.floor() as i64) };
        color.0 = if out { FAINT } else { ui::damage_color(d, f.dmg) };
        let k = look.shake as f32 * f.hud_shake.min(12) as f32 / 12.;
        let j = |s: f32| (t * look.look.shake_speed + s + p.0 as f32 * 7.).sin() * k;
        tf.translation = Val2::px(j(0.), j(1.7));
    }
    for (p, mut span) in &mut signs {
        span.0 = if w.fighters[p.0].state == FState::Out { String::new() } else { "%".into() };
    }
}

/// Panels fade once their player is out; stock icons (or the ×N count) follow the stocks left.
fn stocks(
    m: Res<Match>,
    mut panels: Query<(&Panel, &mut BackgroundGradient)>,
    mut icons: Query<(&Stock, &mut Visibility), Without<Text>>,
    mut counts: Query<(&Stock, &mut Text)>,
) {
    let w = &m.world;
    for (p, mut bg) in &mut panels {
        let out = w.fighters[p.0].state == FState::Out;
        let a = if out { 0.45 } else { 0.92 };
        if let Some(Gradient::Linear(g)) = bg.0.first_mut() {
            for s in &mut g.stops {
                s.color.set_alpha(a);
            }
        }
    }
    for (s, mut vis) in &mut icons {
        *vis = if w.fighters[s.0].stocks >= s.1 { Visibility::Inherited } else { Visibility::Hidden };
    }
    for (s, mut text) in &mut counts {
        text.0 = format!("×{}", w.fighters[s.0].stocks.max(0));
    }
}

/// Stage pixel (y down) → UI logical pixel.
fn to_ui(fr: &Framing, size: Vec2, x: f64, y: f64) -> Vec2 {
    let [x0, y0, x1, y1] = fr.view;
    Vec2::new(((x - x0) / (x1 - x0)) as f32 * size.x, ((y - y0) / (y1 - y0)) as f32 * size.y)
}

/// Where fighter `i` is drawn this frame.
fn drawn(m: &Match, fr: &Framing, i: usize) -> (f64, f64) {
    fr.fighters.get(i).copied().unwrap_or((m.world.fighters[i].x, m.world.fighters[i].y))
}

fn gone(s: FState) -> bool {
    matches!(s, FState::Out | FState::Dead)
}

/// Name tags over fighters on screen, before the countdown ends.
fn tags(
    m: Res<Match>,
    fr: Res<Framing>,
    win: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    mut tags: Query<(&Tag, &mut Node, &mut Visibility)>,
) {
    let w = &m.world;
    let look = &m.data.global.render.hud.look;
    let size = screen_size(win.single().ok(), &scale);
    let [x0, y0, x1, y1] = fr.view;
    let (top, bottom) = (y0 + (y1 - y0) * fr.top_safe_area, y1 - (y1 - y0) * fr.bottom_safe_area);
    for (tag, mut node, mut vis) in &mut tags {
        let f = &w.fighters[tag.0];
        let (x, y) = drawn(&m, &fr, tag.0);
        let p = to_ui(&fr, size, x, y - look.tag_height);
        let at = y - look.anchor_height;
        let show = !gone(f.state)
            && f.state != FState::Win
            && m.mode != Mode::Showcase
            && w.count == 0
            && x > x0
            && x < x1
            && at > top
            && at < bottom;
        *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
        node.left = Val::Px(p.x - 20.);
        node.width = Val::Px(40.);
        node.top = Val::Px(p.y - 26.);
    }
}

/// Offscreen fighters: a bubble at the screen edge with their percent, its nub pointing at them, smaller further out.
fn bubbles(
    m: Res<Match>,
    fr: Res<Framing>,
    win: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    mut bubbles: Query<(&Bubble, &mut Node, &mut Visibility, &mut UiTransform)>,
    mut nubs: Query<(&BubbleNub, &mut Node), Without<Bubble>>,
    mut pcts: Query<(&BubblePct, &mut Text)>,
) {
    let w = &m.world;
    let look = &m.data.global.render.hud.look;
    let size = screen_size(win.single().ok(), &scale);
    let (top, bottom) = (size.y * fr.top_safe_area as f32, size.y * (1. - fr.bottom_safe_area as f32));
    let mut nub_pos = vec![None; w.fighters.len()];
    for (b, mut node, mut vis, mut tf) in &mut bubbles {
        let f = &w.fighters[b.0];
        let (x, y) = drawn(&m, &fr, b.0);
        let p = to_ui(&fr, size, x, y - look.anchor_height);
        let m_ = BUBBLE / 2. + 10.;
        let c = Vec2::new(p.x.clamp(m_, size.x - m_), p.y.clamp(top + m_, bottom - m_));
        let off = p - c;
        let show = !gone(f.state) && f.state != FState::Respawn && off.length() > 1.;
        *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
        node.left = Val::Px(c.x - BUBBLE / 2.);
        node.top = Val::Px(c.y - BUBBLE / 2.);
        tf.scale = Vec2::splat((1. - off.length() / look.bubble_shrink).clamp(look.bubble_min_scale, 1.));
        let dir = off.normalize_or_zero();
        nub_pos[b.0] = Some(dir * (BUBBLE / 2. + 3.) + Vec2::splat(BUBBLE / 2. - 3. - 6.));
    }
    for (n, mut node) in &mut nubs {
        if let Some(p) = nub_pos[n.0] {
            node.left = Val::Px(p.x);
            node.top = Val::Px(p.y);
        }
    }
    for (b, mut text) in &mut pcts {
        text.0 = format!("{}%", w.fighters[b.0].dmg.floor() as i64);
    }
}

/// The countdown, GO! and GAME!, popping in.
fn big(m: Res<Match>, mut q: Query<(&mut Text, &mut TextColor, &mut UiTransform, &mut Visibility), With<Big>>) {
    let w = &m.world;
    let g = &m.data.global.match_;
    let look = &m.data.global.render.hud.look;
    let hz = sim::TICK_HZ;
    let (s, color, pop): (String, Color, f32) = if m.mode == Mode::Training || m.mode == Mode::Showcase {
        (String::new(), STAR, 0.)
    } else if w.count > 0 {
        let into = 1. - (w.count % hz) as f32 / hz as f32;
        (w.count.div_ceil(hz).to_string(), STAR, if w.count.is_multiple_of(hz) { 0. } else { into })
    } else if w.go_t > 0 {
        ("GO!".into(), GOLD, 1. - w.go_t as f32 / g.go_frames as f32)
    } else if w.end_t > 0 && !w.over {
        ("GAME!".into(), STAR, 1. - w.end_t as f32 / g.end_frames as f32)
    } else {
        (String::new(), STAR, 0.)
    };
    for (mut text, mut tc, mut tf, mut vis) in &mut q {
        *vis = if s.is_empty() { Visibility::Hidden } else { Visibility::Inherited };
        text.0.clone_from(&s);
        let fade = if s == "GO!" { (1. - pop) * 2. } else { 1. };
        tc.0 = color.with_alpha(fade.clamp(0., 1.));
        let e = (pop * look.pop_speed).min(1.);
        tf.scale = Vec2::splat(look.pop_scale - (look.pop_scale - 1.) * (1. - (1. - e).powi(3)));
        tf.rotation = Rot2::degrees(if s == "GAME!" { -4. } else { 0. });
    }
}

#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub struct PauseMenu {
    pub cursor: Cursor,
    open: bool,
}

const RESUME: u32 = 1;
const RESTART: u32 = 2;
const DUMMY: u32 = 3;
const DUMMY_PCT: u32 = 4;
const RESET: u32 = 5;
const HITBOXES: u32 = 6;
const TO_SELECT: u32 = 7;
const TO_MENU: u32 = 8;

fn pause(
    testing: Option<Res<crate::editing::Testing>>,
    mut m: ResMut<Match>,
    mut pm: ResMut<PauseMenu>,
    mut devs: ResMut<Devices>,
    q: FocusQuery,
    data: Res<Data>,
    mut saved: ResMut<Saved>,
    mut next: ResMut<NextState<Screen>>,
    mut scene: ResMut<render::Scene>,
) {
    if m.paused.is_some_and(|d| !devs.map.contains_key(&d)) {
        if let Some(d) = devs.map.keys().next().copied() {
            m.paused = Some(d);
        }
        m.clear_input(&mut devs);
    }
    let Some(by) = m.paused else {
        pm.cursor = Cursor { key: RESUME, owner: None };
        pm.open = false;
        return;
    };
    // The Start press that paused is still in this frame's presses.
    if !pm.open {
        pm.open = true;
        return;
    }
    let spots = ui::spots(&q);
    let mut c = pm.cursor;
    for (_, nav) in devs.navs().filter(|(d, _)| *d == by).collect::<Vec<_>>() {
        if nav == Nav::Start {
            m.paused = None;
            break;
        }
        match c.drive(nav, &spots) {
            Some(Action::Back | Action::Activate(RESUME)) => m.paused = None,
            Some(Action::Activate(RESTART)) => play::restart(&mut m, &data.0, &mut scene, &mut devs),
            Some(Action::Activate(RESET)) => {
                play::restart(&mut m, &data.0, &mut scene, &mut devs);
                m.paused = None;
            }
            Some(Action::Spin(DUMMY, by)) => {
                let mode = m.dummy.mode.cycle(by);
                m.set_dummy_mode(mode, &mut devs, &mut saved);
            }
            Some(Action::Spin(DUMMY_PCT, by)) => {
                let p = (m.world.fighters[1].dmg + 10. * f64::from(by)).clamp(0., 300.);
                m.world.fighters[1].dmg = (p / 10.).round() * 10.;
                m.dummy.reset_pct = m.world.fighters[1].dmg;
            }
            Some(Action::Spin(HITBOXES, _)) => m.debug = !m.debug,
            Some(Action::Activate(TO_SELECT)) => next.set(Screen::Select),
            Some(Action::Activate(TO_MENU)) => {
                next.set(if testing.is_some() { Screen::Editor } else { Screen::Title });
            }
            _ => {}
        }
    }
    if m.paused.is_none() {
        m.clear_input(&mut devs);
    }
    if pm.cursor != c {
        pm.cursor = c;
    }
}

/// What the pause menu shows.
#[derive(Clone, PartialEq)]
pub struct PauseKey {
    by: Dev,
    cursor: Cursor,
    dummy: DummyMode,
    dummy_dmg: f64,
    hitboxes: bool,
}

fn draw_pause(
    testing: Option<Res<crate::editing::Testing>>,
    mut commands: Commands,
    m: Res<Match>,
    pm: Res<PauseMenu>,
    devs: Res<Devices>,
    roots: Roots<PauseKey>,
    fonts: Res<Fonts>,
) {
    let training = m.mode == Mode::Training;
    let key = m.paused.map(|by| PauseKey {
        by,
        cursor: pm.cursor,
        dummy: m.dummy.mode,
        dummy_dmg: m.world.fighters.get(1).map_or(0., |f| f.dmg),
        hitboxes: m.debug,
    });
    let Some(mut root) = ui::rebuild(&mut commands, &roots, key.as_ref(), Screen::Match, z::PAUSE) else { return };
    let Some(by) = m.paused else { return };
    let f = &*fonts;
    let slot = m.slots.iter().position(|s| s.dev == Some(by));
    let who = slot.map_or_else(|| devs.name(by), |i| format!("{} · {}", m.slots[i].tag, devs.name(by)));
    root.insert(BackgroundColor(NIGHT.with_alpha(0.62))).with_children(|p| {
        p.spawn(node(|n| {
            n.flex_direction = FlexDirection::Column;
            n.justify_content = JustifyContent::Center;
            n.padding = UiRect::left(Val::Px(84.));
            n.height = Val::Percent(100.);
            n.row_gap = Val::Px(6.);
        }))
        .with_children(|p| {
            p.spawn(txt(&f.display, 96., STAR, "Paused"));
            p.spawn((
                txt(&f.body, 16., DIM, format!("Paused by {who}. Only this device controls the menu.")),
                node(|n| n.margin.bottom = Val::Px(26.)),
            ));
            let mut items = vec![Item::button(RESUME, "Resume")];
            if training {
                items.push(Item::spin(DUMMY, "Dummy", m.dummy.mode.label()));
                items.push(Item::spin(DUMMY_PCT, "Dummy damage", format!("{}%", m.world.fighters[1].dmg.floor())));
                items.push(Item::button(RESET, "Reset positions"));
                items.push(Item::spin(HITBOXES, "Hitboxes", on_off(m.debug)));
            } else {
                items.push(Item::button(RESTART, "Restart"));
            }
            if testing.is_some() {
                items.push(Item::button(TO_MENU, "Back to editor"));
            } else {
                items.push(Item::button(TO_SELECT, "Character select"));
                items.push(Item::button(TO_MENU, "Quit to menu"));
            }
            menu_list(p, f, &items, pm.cursor.key, List::new(460., 30.));
        });
    });
}
