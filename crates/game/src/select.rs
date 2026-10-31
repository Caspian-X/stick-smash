//! Join + character select (versus and training). Any free device presses a button to claim the next slot; each
//! player then has their own cursor: their card (fighter, team, tap jump), CPU cards and the rules bar. Attack on
//! your card locks in; Start (or attack while locked in) starts once everyone is ready. The picked fighters stand
//! on the stage behind the cards.

use bevy::prelude::*;
use sim::world::State as FState;

use crate::input::{Dev, Devices, Nav};
use crate::play::{self, Match, Mode, Rules, Slot};
use crate::settings::{DummyMode, PlayerPref, Saved};
use crate::ui::{self, *};
use crate::{Data, Screen};

const FIGHTER: u32 = 1;
const LEVEL: u32 = 2;
const TEAM: u32 = 3;
const TAP: u32 = 4;
const REMOVE: u32 = 5;
const ADD: u32 = 6;
const STOCKS: u32 = 7;
const STAGE: u32 = 8;
const TEAMS: u32 = 9;
const FF: u32 = 10;
/// Card footers: lock in (your card), dummy behavior (training).
const LOCK: u32 = 11;
const BEHAVIOR: u32 = 12;
const N_TEAMS: u8 = 4;
const LEVELS: [&str; 3] = ["Easy", "Normal", "Hard"];

fn key(kind: u32, slot: usize) -> u32 {
    kind * 16 + slot as u32
}
fn kind(key: u32) -> (u32, usize) {
    (key / 16, (key % 16) as usize)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    /// `None`: CPU (or the training dummy).
    pub dev: Option<Dev>,
    pub fighter: usize,
    pub level: u8,
    pub team: u8,
    pub tap_jump: bool,
    pub ready: bool,
    pub cursor: Cursor,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Select {
    pub mode: Mode,
    pub slots: Vec<Pick>,
    pub stocks: i32,
    pub stage: usize,
    pub teams: bool,
    pub ff: bool,
    pub dummy: DummyMode,
}

/// Set before entering the select screen to start fresh; `dev` joins right away.
#[derive(Resource)]
pub struct SelectInit {
    pub mode: Mode,
    pub dev: Option<Dev>,
}

impl Select {
    fn humans(&self) -> impl Iterator<Item = (usize, &Pick)> {
        self.slots.iter().enumerate().filter(|(_, p)| p.dev.is_some())
    }
    fn max(&self, d: &sim::GameData) -> usize {
        let spawns = d.stages.iter().map(|s| s.spawns.len().min(s.respawns.len())).min().unwrap_or(2);
        if self.mode == Mode::Training { 2 } else { 8.min(spawns).min(d.global.palette.len()) }
    }
    pub fn ready(&self) -> bool {
        let mut h = self.humans().peekable();
        h.peek().is_some() && h.all(|(_, p)| p.ready) && self.slots.len() >= 2
    }
    fn color(&self, i: usize) -> usize {
        if self.teams { self.slots[i].team as usize } else { i }
    }
    fn tag(&self, i: usize) -> String {
        match (self.slots[i].dev, self.mode) {
            (Some(_), _) => format!("P{}", i + 1),
            (None, Mode::Training) => "Dummy".into(),
            (None, _) => "CPU".into(),
        }
    }
    fn join(&mut self, dev: Dev, saved: &Saved, d: &sim::GameData) -> bool {
        if self.slots.len() >= self.max(d) || self.mode == Mode::Training && self.humans().count() > 0 {
            return false;
        }
        let n = self.humans().count();
        let at = self.slots.iter().position(|p| p.dev.is_none()).unwrap_or(self.slots.len());
        let pref = if self.mode == Mode::Training {
            PlayerPref { fighter: saved.training.fighter.clone(), ..default() }
        } else {
            saved.versus.humans.get(n).cloned().unwrap_or_default()
        };
        let fighter = d.fighters.iter().position(|f| f.id == pref.fighter).unwrap_or(n % d.fighters.len());
        self.slots.insert(
            at,
            Pick {
                dev: Some(dev),
                fighter,
                level: 1,
                team: if self.teams { (at % 2) as u8 } else { pref.team },
                tap_jump: pref.tap_jump,
                ready: false,
                cursor: Cursor { key: key(FIGHTER, at), owner: Some(at) },
            },
        );
        true
    }
    fn add_cpu(&mut self, saved: &Saved, d: &sim::GameData) {
        if self.slots.len() >= self.max(d) || self.mode == Mode::Training {
            return;
        }
        let n = self.slots.iter().filter(|p| p.dev.is_none()).count();
        let pref = saved.versus.cpus.get(n).cloned().unwrap_or_default();
        let i = self.slots.len();
        self.slots.push(Pick {
            dev: None,
            fighter: d.fighters.iter().position(|f| f.id == pref.fighter).unwrap_or((i + 1) % d.fighters.len()),
            level: pref.level.min(2),
            team: (i % 2) as u8,
            tap_jump: true,
            ready: true,
            cursor: Cursor::default(),
        });
    }
    fn spin(&mut self, k: u32, by: i32, d: &sim::GameData) {
        let (kd, s) = kind(k);
        let cyc = |v: i32, n: i32| (v + by).rem_euclid(n);
        match kd {
            FIGHTER => self.slots[s].fighter = cyc(self.slots[s].fighter as i32, d.fighters.len() as i32) as usize,
            LEVEL => self.slots[s].level = cyc(i32::from(self.slots[s].level), 3) as u8,
            TEAM => self.slots[s].team = cyc(i32::from(self.slots[s].team), i32::from(N_TEAMS)) as u8,
            TAP => self.slots[s].tap_jump = !self.slots[s].tap_jump,
            STOCKS => self.stocks = cyc(self.stocks - 1, 9) + 1,
            STAGE => self.stage = cyc(self.stage as i32, d.stages.len() as i32) as usize,
            TEAMS => {
                self.teams = !self.teams;
                for (i, p) in self.slots.iter_mut().enumerate() {
                    p.team = (i % 2) as u8;
                }
            }
            FF => self.ff = !self.ff,
            BEHAVIOR => {
                let all = DummyMode::ALL;
                let i = all.iter().position(|m| *m == self.dummy).unwrap_or(0) as i32;
                self.dummy = all[cyc(i, all.len() as i32) as usize];
            }
            _ => {}
        }
    }
    /// Fix cursor owners and keys after slots moved.
    fn renumber(&mut self) {
        for (i, p) in self.slots.iter_mut().enumerate() {
            p.cursor.owner = Some(i);
            let (kd, s) = kind(p.cursor.key);
            if matches!(kd, FIGHTER | TEAM | TAP | LOCK) && s != i {
                p.cursor.key = key(FIGHTER, i);
            }
        }
    }
    pub fn match_slots(&self) -> Vec<Slot> {
        (0..self.slots.len())
            .map(|i| {
                let p = &self.slots[i];
                Slot {
                    dev: p.dev,
                    fighter: p.fighter,
                    cpu: (p.dev.is_none() && self.mode == Mode::Versus).then_some(p.level),
                    team: self.teams.then_some(p.team),
                    tap_jump: p.tap_jump,
                    tag: self.tag(i),
                    color: self.color(i),
                }
            })
            .collect()
    }
}

pub fn enter(
    mut commands: Commands,
    init: Option<Res<SelectInit>>,
    sel: Option<ResMut<Select>>,
    saved: Res<Saved>,
    data: Res<Data>,
) {
    let d = &data.0;
    if let (None, Some(mut sel)) = (&init, sel) {
        for p in sel.slots.iter_mut().filter(|p| p.dev.is_some()) {
            p.ready = false;
        }
        return;
    }
    let Some(init) = init else { return };
    commands.remove_resource::<SelectInit>();
    let v = &saved.versus;
    let stage_id = if init.mode == Mode::Training { &saved.training.stage } else { &v.stage };
    let mut sel = Select {
        mode: init.mode,
        slots: vec![],
        stocks: v.stocks.clamp(1, 9),
        stage: d.stages.iter().position(|s| &s.id == stage_id).unwrap_or(0),
        teams: v.teams && init.mode == Mode::Versus,
        ff: v.friendly_fire,
        dummy: saved.training.mode,
    };
    if init.mode == Mode::Training {
        let t = &saved.training;
        sel.slots.push(Pick {
            dev: None,
            fighter: d.fighters.iter().position(|f| f.id == t.dummy_fighter).unwrap_or(1 % d.fighters.len()),
            level: t.level,
            team: 1,
            tap_jump: true,
            ready: true,
            cursor: Cursor::default(),
        });
    } else {
        for _ in 0..v.cpus.len().min(sel.max(d).saturating_sub(1)) {
            sel.add_cpu(&saved, d);
        }
        for (i, p) in sel.slots.iter_mut().enumerate() {
            p.team = if sel.teams { v.cpus.get(i).map_or((i % 2) as u8, |c| c.team) } else { 0 };
        }
    }
    if let Some(dev) = init.dev {
        sel.join(dev, &saved, d);
    }
    sel.renumber();
    commands.insert_resource(sel);
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    sel: Option<ResMut<Select>>,
    devs: Res<Devices>,
    q: FocusQuery,
    mut saved: ResMut<Saved>,
    data: Res<Data>,
    mut next: ResMut<NextState<Screen>>,
    mut scene: ResMut<render::Scene>,
    m: Option<ResMut<Match>>,
) {
    let Some(mut sel) = sel else { return };
    let d = &data.0;
    let spots = ui::spots(&q);
    let mut s = sel.clone();
    let mut start = false;
    for (dev, st) in &devs.map {
        if s.slots.iter().any(|p| p.dev == Some(*dev)) {
            continue;
        }
        if st.nav.contains(&Nav::Back) && s.humans().count() == 0 {
            next.set(Screen::Title);
            return;
        }
        if st.any_press() && !st.nav.contains(&Nav::Back) && s.join(*dev, &saved, d) {
            s.renumber();
        }
    }
    let navs: Vec<(Dev, Nav)> = devs.navs().collect();
    for (dev, nav) in navs {
        let Some(i) = s.slots.iter().position(|p| p.dev == Some(dev)) else {
            continue;
        };
        if sel.slots.iter().all(|p| p.dev != Some(dev)) {
            continue; // joined this frame
        }
        let own = |k: u32| {
            let (kd, si) = kind(k);
            matches!(kd, FIGHTER | TEAM | TAP | LOCK) && si == i
        };
        if s.slots[i].ready {
            match nav {
                Nav::Back => s.slots[i].ready = false,
                Nav::Start | Nav::Confirm if s.ready() => start = true,
                _ => {}
            }
            continue;
        }
        if nav == Nav::Confirm && own(s.slots[i].cursor.key) {
            s.slots[i].ready = true;
            continue;
        }
        if nav == Nav::Start {
            if s.ready() {
                start = true;
            } else {
                s.slots[i].ready = true;
            }
            continue;
        }
        let mut c = s.slots[i].cursor;
        match c.drive(nav, &spots) {
            Some(Action::Spin(k, by)) => s.spin(k, by, d),
            Some(Action::Activate(k)) => match kind(k) {
                (LOCK, _) => s.slots[i].ready = true,
                (ADD, _) => s.add_cpu(&saved, d),
                (REMOVE, si) if s.slots.get(si).is_some_and(|p| p.dev.is_none()) => {
                    s.slots.remove(si);
                    c.key = key(FIGHTER, i);
                }
                _ => {}
            },
            Some(Action::Back) if own(c.key) => {
                s.slots.remove(i);
                s.renumber();
                continue;
            }
            Some(Action::Back) => c.key = key(FIGHTER, i),
            _ => {}
        }
        s.slots[i].cursor = c;
        s.renumber();
    }
    if s != *sel {
        *sel = s;
    }
    let mslots = sel.match_slots();
    let rules = Rules { stocks: sel.stocks, stage: d.stages[sel.stage].id.clone(), friendly_fire: sel.ff };
    if start {
        remember(&sel, &mut saved, d);
        let mut m = Match::new(&data.0, sel.mode, mslots, rules, play::seed());
        if sel.mode == Mode::Training {
            let t = &saved.training;
            m.dummy.mode = sel.dummy;
            m.dummy.level = t.level;
            m.apply_dummy();
        } else {
            m.debug = saved.versus_hitboxes;
        }
        play::install(&mut commands, &mut scene, m);
        next.set(Screen::Match);
        return;
    }
    if mslots.is_empty() {
        return;
    }
    // Showcase: rebuild when the lineup changes; ready fighters strike their win pose.
    let showcase = m.as_ref().is_some_and(|m| {
        m.mode == Mode::Showcase
            && m.slots.iter().map(|s| (s.fighter, s.color)).eq(mslots.iter().map(|s| (s.fighter, s.color)))
            && m.rules.stage == rules.stage
    });
    if !showcase {
        let mut slots = mslots;
        for s in &mut slots {
            s.cpu = None;
            s.dev = None;
        }
        play::install(&mut commands, &mut scene, Match::new(&data.0, Mode::Showcase, slots, rules, 1));
    } else if let Some(mut m) = m {
        for (i, f) in m.world.fighters.iter_mut().enumerate() {
            let ready = sel.slots.get(i).is_some_and(|p| p.ready && p.dev.is_some());
            if ready && f.state != FState::Win {
                f.set_state(FState::Win);
            } else if !ready && f.state == FState::Win {
                f.set_state(FState::Idle);
            }
        }
    }
}

fn remember(sel: &Select, saved: &mut Saved, d: &sim::GameData) {
    let pref = |p: &Pick| PlayerPref {
        fighter: d.fighters[p.fighter].id.clone(),
        level: p.level,
        team: p.team,
        tap_jump: p.tap_jump,
    };
    if sel.mode == Mode::Training {
        let t = &mut saved.training;
        if let Some(h) = sel.slots.iter().find(|p| p.dev.is_some()) {
            t.fighter = d.fighters[h.fighter].id.clone();
        }
        if let Some(c) = sel.slots.iter().find(|p| p.dev.is_none()) {
            t.dummy_fighter = d.fighters[c.fighter].id.clone();
        }
        t.stage = d.stages[sel.stage].id.clone();
        t.mode = sel.dummy;
        return;
    }
    let v = &mut saved.versus;
    v.stocks = sel.stocks;
    v.stage = d.stages[sel.stage].id.clone();
    v.teams = sel.teams;
    v.friendly_fire = sel.ff;
    v.humans = sel.slots.iter().filter(|p| p.dev.is_some()).map(pref).collect();
    v.cpus = sel.slots.iter().filter(|p| p.dev.is_none()).map(pref).collect();
}

#[derive(Component)]
pub struct SelectRoot(Select);

pub fn draw(
    mut commands: Commands,
    sel: Option<Res<Select>>,
    roots: Query<(Entity, &SelectRoot)>,
    fonts: Res<Fonts>,
    data: Res<Data>,
    devs: Res<Devices>,
) {
    let Some(sel) = sel else { return };
    if roots.iter().any(|(_, r)| r.0 == *sel) {
        return;
    }
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let d = &data.0;
    let f = &*fonts;
    let s = &*sel;
    commands.spawn((root(), SelectRoot(s.clone()), DespawnOnExit(Screen::Select), GlobalZIndex(5))).with_children(
        |p| {
            // Top: title and rules.
            p.spawn((
                node(|n| {
                    n.position_type = PositionType::Absolute;
                    n.top = Val::Px(0.);
                    n.left = Val::Px(0.);
                    n.right = Val::Px(0.);
                    n.padding = UiRect::new(Val::Px(40.), Val::Px(40.), Val::Px(26.), Val::Px(30.));
                    n.flex_direction = FlexDirection::Row;
                    n.align_items = AlignItems::FlexEnd;
                    n.justify_content = JustifyContent::SpaceBetween;
                }),
                BackgroundGradient::from(LinearGradient::to_bottom(vec![
                    ColorStop::auto(alpha(NIGHT, 0.92)),
                    ColorStop::auto(alpha(NIGHT, 0.0)),
                ])),
            ))
            .with_children(|p| {
                p.spawn(col()).with_children(|p| {
                    let title = if s.mode == Mode::Training { "Training" } else { "Versus" };
                    p.spawn(txt(&f.display, 58., STAR, title));
                    palette_strip(p, d, 22., 4.);
                });
                p.spawn((
                    node(|n| {
                        n.flex_direction = FlexDirection::Column;
                        n.width = Val::Px(300.);
                        n.padding = UiRect::all(Val::Px(8.));
                        n.border_radius = BorderRadius::all(Val::Px(10.));
                    }),
                    BackgroundColor(INK_SOFT),
                ))
                .with_children(|p| {
                    let onoff = |b: bool| if b { "On" } else { "Off" };
                    let mut rules = vec![(STAGE, "Stage", d.stages[s.stage].name.clone())];
                    if s.mode == Mode::Versus {
                        rules.insert(0, (STOCKS, "Stocks", s.stocks.to_string()));
                        rules.push((TEAMS, "Teams", onoff(s.teams).into()));
                        if s.teams {
                            rules.push((FF, "Friendly fire", onoff(s.ff).into()));
                        }
                    }
                    for (k, label, value) in rules {
                        spin_row(p, f, d, s, key(k, 0), None, label, value, None, 17.);
                    }
                });
            });
            // Bottom: cards and hints.
            p.spawn(node(|n| {
                n.position_type = PositionType::Absolute;
                n.bottom = Val::Px(18.);
                n.left = Val::Px(0.);
                n.right = Val::Px(0.);
                n.flex_direction = FlexDirection::Column;
                n.align_items = AlignItems::Center;
                n.row_gap = Val::Px(14.);
            }))
            .with_children(|p| {
                if s.ready() {
                    ready_banner(p, f);
                }
                let free = s.slots.len() < s.max(d) && !(s.mode == Mode::Training && s.humans().count() > 0);
                let n = s.slots.len() + usize::from(free);
                let w = (1240. / n as f32 - 12.).min(272.);
                p.spawn(node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.column_gap = Val::Px(12.);
                    n.align_items = AlignItems::Stretch;
                }))
                .with_children(|p| {
                    for i in 0..s.slots.len() {
                        card(p, f, d, s, i, w, &devs);
                    }
                    if free {
                        join_card(p, f, d, s, w);
                    }
                });
                p.spawn(node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.column_gap = Val::Px(22.);
                }))
                .with_children(|p| {
                    hint(p, f, "Attack / Enter", "lock in");
                    hint(p, f, "Special / Esc", "back out");
                    hint(p, f, "Start", "fight when everyone's ready");
                });
            });
        },
    );
}

/// Chips of the cursors (other than the owner's own card focus) sitting on `k`.
fn cursor_chips(p: &mut ChildSpawnerCommands, f: &Fonts, d: &sim::GameData, s: &Select, k: u32) {
    for (i, pk) in s.humans() {
        if pk.cursor.key == k && !pk.ready {
            p.spawn((
                node(|n| {
                    n.padding = UiRect::axes(Val::Px(5.), Val::Px(0.));
                    n.border_radius = BorderRadius::all(Val::Px(3.));
                }),
                BackgroundColor(pal(d, s.color(i)).0),
            ))
            .with_child(txt(&f.heavy, 14., NIGHT, format!("P{}", i + 1)));
        }
    }
}

fn focused_by(s: &Select, k: u32) -> Option<usize> {
    s.humans().find(|(_, p)| p.cursor.key == k && !p.ready).map(|(i, _)| i)
}

fn ready_banner(p: &mut ChildSpawnerCommands, f: &Fonts) {
    p.spawn((
        node(|n| {
            n.padding = UiRect::axes(Val::Px(60.), Val::Px(2.));
            n.flex_direction = FlexDirection::Row;
            n.align_items = AlignItems::Center;
            n.column_gap = Val::Px(24.);
        }),
        BackgroundGradient::from(LinearGradient::to_right(vec![
            ColorStop::auto(Color::NONE),
            ColorStop::auto(alpha(GOLD, 0.95)),
            ColorStop::auto(alpha(GOLD, 0.95)),
            ColorStop::auto(Color::NONE),
        ])),
        UiTransform::from_rotation(Rot2::degrees(-2.)),
    ))
    .with_children(|p| {
        p.spawn(txt(&f.display, 46., NIGHT, "Ready to fight"));
        p.spawn(txt(&f.heavy, 22., alpha(NIGHT, 0.75), "press start"));
    });
}

fn card(p: &mut ChildSpawnerCommands, f: &Fonts, d: &sim::GameData, s: &Select, i: usize, w: f32, devs: &Devices) {
    let pk = &s.slots[i];
    let (main, dark, _) = pal(d, s.color(i));
    let human = pk.dev.is_some();
    let narrow = w < 200.;
    let fighter = &d.fighters[pk.fighter];
    p.spawn((
        node(|n| {
            n.width = Val::Px(w);
            n.flex_direction = FlexDirection::Column;
            n.border = UiRect::all(Val::Px(2.));
            n.border_radius = BorderRadius::all(Val::Px(10.));
            n.overflow = Overflow::clip();
        }),
        BackgroundColor(INK),
        BorderColor::all(if pk.ready && human { main } else { alpha(main, 0.35) }),
        BoxShadow::new(
            alpha(main, if pk.ready && human { 0.45 } else { 0. }),
            Val::Px(0.),
            Val::Px(0.),
            Val::Px(2.),
            Val::Px(18.),
        ),
    ))
    .with_children(|p| {
        // Header band.
        p.spawn((
            node(|n| {
                n.flex_direction = FlexDirection::Row;
                n.align_items = AlignItems::Center;
                n.justify_content = JustifyContent::SpaceBetween;
                n.padding = UiRect::axes(Val::Px(12.), Val::Px(2.));
            }),
            BackgroundGradient::from(LinearGradient::to_right(vec![ColorStop::auto(main), ColorStop::auto(dark)])),
        ))
        .with_children(|p| {
            p.spawn(txt(&f.display, 26., NIGHT, s.tag(i)));
            let who = match pk.dev {
                Some(dv) => devs.name(dv),
                None if s.mode == Mode::Training => "Training dummy".into(),
                None => LEVELS[pk.level as usize].into(),
            };
            p.spawn(txt(&f.body_bold, 12., alpha(NIGHT, 0.8), who));
        });
        p.spawn(node(|n| {
            n.flex_direction = FlexDirection::Column;
            n.padding = UiRect::new(Val::Px(12.), Val::Px(12.), Val::Px(8.), Val::Px(10.));
            n.row_gap = Val::Px(2.);
            n.flex_grow = 1.;
        }))
        .with_children(|p| {
            let k = key(FIGHTER, i);
            let on = focused_by(s, k);
            p.spawn((
                node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.align_items = AlignItems::Center;
                    n.column_gap = Val::Px(6.);
                    n.min_height = Val::Px(44.);
                }),
                Focusable { key: k, owner: human.then_some(i), spin: true },
            ))
            .with_children(|p| {
                let arrows = on.is_some();
                if arrows {
                    p.spawn(txt(&f.display, 30., GOLD, "‹"));
                }
                p.spawn(txt(&f.display, if narrow { 25. } else { 38. }, STAR, fighter.name.clone()));
                if arrows {
                    p.spawn(txt(&f.display, 30., GOLD, "›"));
                }
                if !human {
                    cursor_chips(p, f, d, s, k);
                }
            });
            if !narrow {
                p.spawn((
                    txt(&f.body, 12.5, DIM, fighter.blurb.clone()),
                    node(|n| {
                        n.height = Val::Px(52.);
                        n.overflow = Overflow::clip();
                        n.margin.bottom = Val::Px(6.);
                    }),
                ));
            }
            let mut rows: Vec<(u32, &str, String, Option<Color>)> = vec![];
            if !human && s.mode == Mode::Versus {
                rows.push((LEVEL, "Level", LEVELS[pk.level as usize].into(), None));
            }
            if s.teams {
                let tc = pal(d, pk.team as usize).0;
                rows.push((TEAM, "Team", team_name(pk.team).into(), Some(tc)));
            }
            if human {
                rows.push((TAP, "Up to jump", if pk.tap_jump { "On" } else { "Off" }.into(), None));
            }
            for (kd, label, value, color) in rows {
                spin_row(p, f, d, s, key(kd, i), human.then_some(i), label, value, color, 17.);
            }
            if s.mode == Mode::Training && !human {
                spin_row(p, f, d, s, key(BEHAVIOR, i), None, "Behavior", s.dummy.label().into(), None, 17.);
            }
            if !human && s.mode == Mode::Versus {
                let k = key(REMOVE, i);
                let on = focused_by(s, k);
                p.spawn((
                    node(|n| {
                        n.flex_direction = FlexDirection::Row;
                        n.column_gap = Val::Px(6.);
                        n.margin.top = Val::Auto;
                        n.padding = UiRect::axes(Val::Px(8.), Val::Px(3.));
                        n.border_radius = BorderRadius::all(Val::Px(5.));
                    }),
                    BackgroundColor(if on.is_some() { alpha(main, 0.16) } else { Color::NONE }),
                    Focusable { key: k, owner: None, spin: false },
                ))
                .with_children(|p| {
                    p.spawn(txt(&f.label, 17., if on.is_some() { STAR } else { FAINT }, "Remove CPU"));
                    cursor_chips(p, f, d, s, k);
                });
            }
            if human {
                let k = key(LOCK, i);
                let on = focused_by(s, k).is_some();
                let (label, color) = if pk.ready {
                    ("Ready!", main)
                } else if on {
                    ("Lock in", GOLD)
                } else {
                    ("Choosing…", FAINT)
                };
                p.spawn((
                    node(|n| {
                        n.margin.top = Val::Auto;
                        n.justify_content = JustifyContent::Center;
                        n.padding = UiRect::axes(Val::Px(0.), Val::Px(2.));
                        n.border = UiRect::all(Val::Px(1.5));
                        n.border_radius = BorderRadius::all(Val::Px(6.));
                    }),
                    BackgroundColor(if pk.ready { alpha(main, 0.2) } else { Color::NONE }),
                    BorderColor::all(if on && !pk.ready { GOLD } else { Color::NONE }),
                    Focusable { key: k, owner: Some(i), spin: false },
                ))
                .with_child(txt(&f.display, 24., color, label));
            }
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn spin_row(
    p: &mut ChildSpawnerCommands,
    f: &Fonts,
    d: &sim::GameData,
    s: &Select,
    k: u32,
    owner: Option<usize>,
    label: &str,
    value: String,
    color: Option<Color>,
    size: f32,
) {
    let on = focused_by(s, k);
    let tint = on.map_or(Color::NONE, |i| alpha(pal(d, s.color(i)).0, 0.16));
    p.spawn((
        node(|n| {
            n.flex_direction = FlexDirection::Row;
            n.align_items = AlignItems::Center;
            n.justify_content = JustifyContent::SpaceBetween;
            n.padding = UiRect::axes(Val::Px(8.), Val::Px(3.));
            n.border_radius = BorderRadius::all(Val::Px(5.));
        }),
        BackgroundColor(tint),
        Focusable { key: k, owner, spin: true },
    ))
    .with_children(|p| {
        p.spawn(row()).with_children(|p| {
            p.spawn((txt(&f.label, size, DIM, label), node(|n| n.margin.right = Val::Px(6.))));
            if owner.is_none() {
                cursor_chips(p, f, d, s, k);
            }
        });
        let v = if on.is_some() { format!("‹ {value} ›") } else { value };
        p.spawn(txt(&f.label, size + 1., color.unwrap_or(STAR), v));
    });
}

fn join_card(p: &mut ChildSpawnerCommands, f: &Fonts, d: &sim::GameData, s: &Select, w: f32) {
    p.spawn((
        node(|n| {
            n.width = Val::Px(w);
            n.min_height = Val::Px(150.);
            n.flex_direction = FlexDirection::Column;
            n.justify_content = JustifyContent::Center;
            n.align_items = AlignItems::Center;
            n.row_gap = Val::Px(14.);
            n.border = UiRect::all(Val::Px(2.));
            n.border_radius = BorderRadius::all(Val::Px(10.));
            n.padding = UiRect::all(Val::Px(12.));
        }),
        BackgroundColor(INK_SOFT),
        BorderColor::all(alpha(FAINT, 0.6)),
    ))
    .with_children(|p| {
        p.spawn((
            txt(&f.heavy, 22., DIM, "Press any button to join"),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        ));
        if s.mode == Mode::Versus && s.humans().count() > 0 {
            let k = key(ADD, 0);
            let on = focused_by(s, k).is_some();
            p.spawn((
                node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.column_gap = Val::Px(6.);
                    n.padding = UiRect::axes(Val::Px(14.), Val::Px(4.));
                    n.border = UiRect::all(Val::Px(1.5));
                    n.border_radius = BorderRadius::all(Val::Px(6.));
                }),
                BorderColor::all(if on { GOLD } else { FAINT }),
                Focusable { key: k, owner: None, spin: false },
            ))
            .with_children(|p| {
                p.spawn(txt(&f.label, 18., if on { GOLD } else { DIM }, "+ Add CPU"));
                cursor_chips(p, f, d, s, k);
            });
        }
    });
}

pub fn team_name(t: u8) -> &'static str {
    ["Red team", "Blue team", "Green team", "Yellow team"][t as usize % 4]
}
