//! Join + character select (versus and training). Any free device presses a button to claim the next slot. Each
//! player moves a colored chip over the roster grid (hovering previews the fighter, attack locks it in, the "?"
//! tile rolls a random one), CPU cards, and the rules panel. Grab / Shield opens a player's card options (team,
//! up to jump). Start (or attack while locked in) goes on once everyone is ready: to stage select in versus, straight
//! into the match in training. The picked fighters stand on the stage in a strip between the roster and the cards,
//! and a locked-in fighter plays `render.select.flourish` before its win pose.

use bevy::prelude::*;
use render::portrait::{Portrait, Portraits};
use sim::world::State as FState;

use crate::grid::{self, Area, Chip, key, kind};
use crate::input::{self, Act, Dev, Devices, Nav};
use crate::play::{self, Match, Mode, Rules, Slot};
use crate::settings::{DummyMode, PlayerPref, Saved};
use crate::ui::{self, *};
use crate::{Data, Screen};

/// Roster tile `t` (a fighter index; one past the last is the random tile).
const TILE: u32 = 1;
/// CPU / dummy card controls.
const FIGHTER: u32 = 2;
const LEVEL: u32 = 3;
const REMOVE: u32 = 5;
const BEHAVIOR: u32 = 12;
const ADD: u32 = 6;
/// Rules panel.
pub(crate) const STOCKS: u32 = 7;
const STAGE: u32 = 8;
const TEAMS: u32 = 9;
const FF: u32 = 10;
/// A human's options popover rows (not focusable; the popover takes over that player's presses). `TEAM` is also
/// a CPU card row.
const TEAM: u32 = 13;
const TAP: u32 = 14;
const N_TEAMS: u8 = 4;
const LEVELS: [&str; 3] = ["Easy", "Normal", "Hard"];

/// Room for the roster grid, between the title and the rules panel.
const ROSTER: Area = Area { w: 700., h: 330., aspect: Portraits::ASPECT, extra: 0., gap: 10., max: 116. };
const ROSTER_TOP: f32 = 18.;

/// Set by stage select when backing out, so everyone stays locked in.
#[derive(Resource)]
pub struct FromStages;

#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    /// `None`: CPU (or the training dummy).
    pub dev: Option<Dev>,
    pub disconnected: bool,
    pub fighter: usize,
    /// Hovering the random tile; a fighter is rolled on lock-in.
    pub random: bool,
    pub level: u8,
    pub team: u8,
    pub tap_jump: bool,
    pub ready: bool,
    pub cursor: Cursor,
    /// Options popover open, at this row.
    pub menu: Option<usize>,
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
    /// Random tile state.
    pub roll: u32,
}

/// Set before entering the select screen to start fresh; `dev` joins right away.
#[derive(Resource)]
pub struct SelectInit {
    pub mode: Mode,
    pub dev: Option<Dev>,
}

/// When things happened, for the lock-in flip, the ready banner and the stage flourish.
#[derive(Resource, Default)]
pub struct Moments {
    /// Device → seconds when it locked in.
    locked: Vec<(Dev, f32)>,
    banner: Option<f32>,
    /// Slots whose fighter should play the flourish on the stage.
    flourish: Vec<usize>,
}

impl Select {
    pub(crate) fn humans(&self) -> impl Iterator<Item = (usize, &Pick)> {
        self.slots.iter().enumerate().filter(|(_, p)| p.dev.is_some())
    }
    fn max(&self) -> usize {
        if self.mode == Mode::Training { 2 } else { 8 }
    }
    pub fn ready(&self) -> bool {
        let mut h = self.humans().peekable();
        h.peek().is_some() && h.all(|(_, p)| p.ready) && self.slots.len() >= 2 && self.start_reason().is_none()
    }
    fn start_reason(&self) -> Option<&'static str> {
        let cfg = sim::MatchConfig {
            players: self
                .slots
                .iter()
                .map(|p| sim::PlayerConfig { team: self.teams.then_some(p.team), ..sim::PlayerConfig::human("") })
                .collect(),
            ..default()
        };
        if self.mode == Mode::Versus && self.slots.len() >= 2 && cfg.opposing_sides() < 2 {
            Some("Choose at least two opposing teams")
        } else {
            None
        }
    }
    fn fit_stage(&mut self, d: &sim::GameData) {
        if d.stages.get(self.stage).is_none_or(|s| s.max_players() < self.slots.len())
            && let Some(i) = d.stages.iter().position(|s| s.max_players() >= self.slots.len())
        {
            self.stage = i;
        }
    }
    pub(crate) fn color(&self, i: usize) -> usize {
        if self.teams { self.slots[i].team as usize } else { i }
    }
    fn tag(&self, i: usize) -> String {
        match (self.slots[i].dev.is_some() || self.slots[i].disconnected, self.mode) {
            (true, _) => format!("P{}", i + 1),
            (false, Mode::Training) => "Dummy".into(),
            (false, _) => "CPU".into(),
        }
    }
    /// The roster tile a player's chip rests on.
    fn home(p: &Pick, d: &sim::GameData) -> u32 {
        key(TILE, if p.random { d.fighters.len() } else { p.fighter })
    }
    /// Options popover rows for slot `i`.
    fn options(&self) -> Vec<u32> {
        if self.teams { vec![TEAM, TAP] } else { vec![TAP] }
    }
    fn join(&mut self, dev: Dev, saved: &Saved, d: &sim::GameData) -> bool {
        if let Some(p) = self.slots.iter_mut().find(|p| p.disconnected) {
            p.dev = Some(dev);
            p.disconnected = false;
            p.ready = false;
            return true;
        }
        if self.slots.len() >= self.max() || self.mode == Mode::Training && self.humans().count() > 0 {
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
                disconnected: false,
                fighter,
                random: false,
                level: 1,
                team: if self.teams { (at % 2) as u8 } else { pref.team },
                tap_jump: pref.tap_jump,
                ready: false,
                cursor: Cursor { key: key(TILE, fighter), owner: Some(at) },
                menu: None,
            },
        );
        true
    }
    fn add_cpu(&mut self, saved: &Saved, d: &sim::GameData) {
        if self.slots.len() >= self.max() || self.mode == Mode::Training {
            return;
        }
        let n = self.slots.iter().filter(|p| p.dev.is_none()).count();
        let pref = saved.versus.cpus.get(n).cloned().unwrap_or_default();
        let i = self.slots.len();
        self.slots.push(Pick {
            dev: None,
            disconnected: false,
            fighter: d.fighters.iter().position(|f| f.id == pref.fighter).unwrap_or((i + 1) % d.fighters.len()),
            random: false,
            level: pref.level.min(2),
            team: (i % 2) as u8,
            tap_jump: true,
            ready: true,
            cursor: Cursor::default(),
            menu: None,
        });
    }
    fn lock(&mut self, i: usize, d: &sim::GameData) {
        let p = &mut self.slots[i];
        if p.random {
            self.roll ^= self.roll << 13;
            self.roll ^= self.roll >> 17;
            self.roll ^= self.roll << 5;
            p.fighter = self.roll as usize % d.fighters.len();
            p.random = false;
        }
        p.ready = true;
        p.menu = None;
        p.cursor.key = Self::home(p, d);
    }
    pub(crate) fn spin(&mut self, k: u32, by: i32, d: &sim::GameData) {
        let (kd, s) = kind(k);
        let cyc = |v: i32, n: i32| (v + by).rem_euclid(n);
        let p = self.slots.get_mut(s);
        match kd {
            FIGHTER => {
                if let Some(p) = p {
                    p.fighter = cyc(p.fighter as i32, d.fighters.len() as i32) as usize;
                }
            }
            LEVEL => {
                if let Some(p) = p {
                    p.level = cyc(i32::from(p.level), 3) as u8;
                }
            }
            TEAM => {
                if let Some(p) = p {
                    p.team = cyc(i32::from(p.team), i32::from(N_TEAMS)) as u8;
                }
            }
            TAP => {
                if let Some(p) = p {
                    p.tap_jump = !p.tap_jump;
                }
            }
            STOCKS => self.stocks = cyc(self.stocks - 1, 9) + 1,
            STAGE => {
                let stages: Vec<usize> = d
                    .stages
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.max_players() >= self.slots.len())
                    .map(|(i, _)| i)
                    .collect();
                if !stages.is_empty() {
                    let at = stages.iter().position(|i| *i == self.stage).unwrap_or(0);
                    self.stage = stages[cyc(at as i32, stages.len() as i32) as usize];
                }
            }
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
    /// Fix cursor owners and keys after slots moved. `removed` (sorted, old indices) shifts keys that point at a
    /// later card (a CPU row being edited); a cursor on a removed card or a gone control goes home.
    fn renumber(&mut self, removed: &[usize], d: &sim::GameData) {
        let (n, add) = (self.slots.len(), self.mode == Mode::Versus && self.slots.len() < 8);
        for (i, p) in self.slots.iter_mut().enumerate() {
            p.cursor.owner = Some(i);
            let home = Self::home(p, d);
            let (kd, si) = kind(p.cursor.key);
            p.cursor.key = match kd {
                TILE | STOCKS | STAGE | TEAMS | FF => continue,
                ADD if add => continue,
                ADD => home,
                _ if removed.contains(&si) => home,
                _ => match si - removed.iter().filter(|&&r| r < si).count() {
                    s if s < n => key(kd, s),
                    _ => home,
                },
            };
        }
    }
    fn frame(&mut self, devs: &Devices, spots: &[Spot], saved: &Saved, d: &sim::GameData) -> (bool, bool) {
        let s = self;
        let mut start = false;
        let mut remove = Vec::new();
        let mut adds = 0;
        for (i, p) in s.slots.iter_mut().enumerate() {
            match p.dev.map(|dev| devs.map.get(&dev)) {
                Some(None) if p.ready => {
                    p.dev = None;
                    p.disconnected = true;
                }
                Some(None) => remove.push(i),
                // Grab / Shield toggles the options popover.
                Some(Some(st)) if !p.ready && (st.pressed[Act::Grab as usize] || st.pressed[Act::Shield as usize]) => {
                    p.menu = if p.menu.is_some() { None } else { Some(0) };
                }
                _ => {}
            }
        }
        let navs: Vec<(Dev, Nav)> = devs.navs().collect();
        for (dev, nav) in navs {
            let Some(i) = s.slots.iter().position(|p| p.dev == Some(dev)) else {
                continue;
            };
            if remove.contains(&i) {
                continue;
            }
            if let Some(r) = s.slots[i].menu {
                let rows = s.options();
                let r = r.min(rows.len() - 1);
                let by = |n: i32| (r as i32 + n).rem_euclid(rows.len() as i32) as usize;
                match nav {
                    Nav::Up => s.slots[i].menu = Some(by(-1)),
                    Nav::Down => s.slots[i].menu = Some(by(1)),
                    Nav::Left => s.spin(key(rows[r], i), -1, d),
                    Nav::Right | Nav::Confirm => s.spin(key(rows[r], i), 1, d),
                    Nav::Back | Nav::Start => s.slots[i].menu = None,
                }
                continue;
            }
            if s.slots[i].ready {
                match nav {
                    Nav::Back => s.slots[i].ready = false,
                    Nav::Start | Nav::Confirm if s.ready() => start = true,
                    _ => {}
                }
                continue;
            }
            if nav == Nav::Start {
                if s.ready() {
                    start = true;
                } else {
                    s.lock(i, d);
                }
                continue;
            }
            let mut c = s.slots[i].cursor;
            let before = c.key;
            let n = d.fighters.len() + 1;
            match grid::drive(&mut c, nav, spots, TILE, n, ROSTER.layout(n).cols) {
                Some(Action::Spin(k, by)) => s.spin(k, by, d),
                Some(Action::Activate(k)) => match kind(k) {
                    (TILE, _) => {
                        s.slots[i].cursor = c;
                        s.lock(i, d);
                        continue;
                    }
                    (ADD, _) => adds += 1,
                    (REMOVE, si) if s.slots.get(si).is_some_and(|p| p.dev.is_none()) => {
                        remove.push(si);
                        c.key = Select::home(&s.slots[i], d);
                    }
                    _ => {}
                },
                Some(Action::Back) if kind(c.key).0 == TILE => {
                    remove.push(i);
                    continue;
                }
                Some(Action::Back) => c.key = Select::home(&s.slots[i], d),
                _ => {}
            }
            // Hovering a tile previews that fighter.
            if let (TILE, t) = kind(c.key)
                && c.key != before
            {
                let p = &mut s.slots[i];
                p.random = t >= d.fighters.len();
                if !p.random {
                    p.fighter = t;
                }
            }
            s.slots[i].cursor = c;
        }
        let mut changed = !remove.is_empty() || adds > 0;
        remove.sort_unstable();
        remove.dedup();
        for &i in remove.iter().rev() {
            s.slots.remove(i);
        }
        for _ in 0..adds {
            s.add_cpu(saved, d);
        }
        let mut leave = false;
        for (dev, st) in &devs.map {
            if s.slots.iter().any(|p| p.dev == Some(*dev)) {
                continue;
            }
            // A device leaving this frame cannot immediately join again.
            if devs.navs().any(|(d, n)| d == *dev && n == Nav::Back) {
                leave |= s.humans().count() == 0;
                continue;
            }
            if st.any_press() && s.join(*dev, saved, d) {
                changed = true;
            }
        }
        if changed {
            s.renumber(&remove, d);
        }
        s.fit_stage(d);
        (start && s.ready() && d.stages[s.stage].max_players() >= s.slots.len(), leave && s.humans().count() == 0)
    }
    pub fn match_slots(&self) -> Vec<Slot> {
        (0..self.slots.len())
            .map(|i| {
                let p = &self.slots[i];
                Slot {
                    dev: p.dev,
                    fighter: p.fighter,
                    cpu: (p.dev.is_none() && !p.disconnected && self.mode == Mode::Versus).then_some(p.level),
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
    back: Option<Res<FromStages>>,
) {
    let d = &data.0;
    commands.insert_resource(Moments::default());
    if let (None, Some(mut sel)) = (&init, sel) {
        if back.is_none() {
            for p in sel.slots.iter_mut().filter(|p| p.dev.is_some()) {
                p.ready = false;
            }
        }
        commands.remove_resource::<FromStages>();
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
        roll: play::seed() | 1,
    };
    if init.mode == Mode::Training {
        let t = &saved.training;
        sel.slots.push(Pick {
            dev: None,
            disconnected: false,
            fighter: d.fighters.iter().position(|f| f.id == t.dummy_fighter).unwrap_or(1 % d.fighters.len()),
            random: false,
            level: t.level,
            team: 1,
            tap_jump: true,
            ready: true,
            cursor: Cursor::default(),
            menu: None,
        });
    } else {
        for _ in 0..v.cpus.len().min(sel.max().saturating_sub(1)) {
            sel.add_cpu(&saved, d);
        }
        for (i, p) in sel.slots.iter_mut().enumerate() {
            p.team = if sel.teams { v.cpus.get(i).map_or((i % 2) as u8, |c| c.team) } else { 0 };
        }
    }
    if let Some(dev) = init.dev {
        sel.join(dev, &saved, d);
    }
    sel.renumber(&[], d);
    sel.fit_stage(d);
    commands.insert_resource(sel);
}

pub fn exit(mut portraits: ResMut<Portraits>) {
    portraits.cells.clear();
}

/// A left click acts as the keyboard's confirm press on the control under the pointer.
fn click(sel: &mut Select, devs: &mut Devices, q: &FocusQuery, pointer: &Mouse, fighters: usize) {
    if !pointer.clicked() {
        return;
    }
    let hit = pointer.hit(q);
    let Some(dev) = keyboard(devs, |d| sel.slots.iter().any(|p| p.dev == Some(d))) else { return };
    if let Some(p) = sel.slots.iter_mut().find(|p| p.dev == Some(dev)) {
        let Some(f) = hit.filter(|f| p.cursor.allowed(f)) else { return };
        if p.ready || p.menu.is_some() {
            return;
        }
        p.cursor.key = f.key;
        if let (TILE, t) = kind(f.key) {
            p.random = t >= fighters;
            p.fighter = if p.random { p.fighter } else { t };
        }
    } else if hit.is_none() {
        return;
    }
    if let Some(st) = devs.map.get_mut(&dev) {
        st.nav.push(Nav::Confirm);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    sel: Option<ResMut<Select>>,
    mut devs: ResMut<Devices>,
    q: FocusQuery,
    pointer: Mouse,
    time: Res<Time>,
    mut saved: ResMut<Saved>,
    data: Res<Data>,
    mut next: ResMut<NextState<Screen>>,
    mut scene: ResMut<render::Scene>,
    (mut m, mut moments, mut portraits): (Option<ResMut<Match>>, ResMut<Moments>, ResMut<Portraits>),
) {
    let Some(mut sel) = sel else { return };
    let d = &data.0;
    let spots = ui::spots(&q);
    let mut s = sel.clone();
    click(&mut s, &mut devs, &q, &pointer, d.fighters.len());
    let (start, leave) = s.frame(&devs, &spots, &saved, d);
    if leave {
        next.set(Screen::Title);
        return;
    }
    if s.slots.iter().map(|p| p.dev).ne(sel.slots.iter().map(|p| p.dev)) {
        devs.clear();
    }
    let now = time.elapsed_secs();
    for (i, p) in s.slots.iter().enumerate() {
        if let Some(dev) = p.dev.filter(|_| p.ready)
            && !sel.slots.iter().any(|o| o.dev == Some(dev) && o.ready)
        {
            moments.locked.retain(|(d, _)| *d != dev);
            moments.locked.push((dev, now));
            moments.flourish.push(i);
        }
    }
    moments.locked.retain(|(dev, _)| s.slots.iter().any(|p| p.dev == Some(*dev) && p.ready));
    moments.flourish.retain(|i| s.slots.get(*i).is_some_and(|p| p.ready && p.dev.is_some()));
    moments.banner = s.ready().then(|| moments.banner.unwrap_or(now));
    if s != *sel {
        *sel = s;
    }
    if !portraits.data.as_ref().is_some_and(|p| std::sync::Arc::ptr_eq(p, &data.0)) {
        portraits.data = Some(data.0.clone());
    }
    portraits.cells = (0..d.fighters.len())
        .map(|t| {
            let on = sel.humans().find(|(_, p)| p.cursor.key == key(TILE, t));
            Portrait {
                fighter: t,
                color: on.map(|(i, _)| sel.color(i)),
                win: sel.humans().any(|(_, p)| p.cursor.key == key(TILE, t) && p.ready),
            }
        })
        .collect();
    let mslots = sel.match_slots();
    let rules = Rules { stocks: sel.stocks, stage: d.stages[sel.stage].id.clone(), friendly_fire: sel.ff };
    if start && sel.mode == Mode::Versus {
        next.set(Screen::Stages);
        return;
    }
    if start {
        remember(&sel, &mut saved, d);
        let Ok(mut m) = Match::new(&data.0, sel.mode, mslots, rules, play::seed()) else { return };
        let t = &saved.training;
        m.dummy.mode = sel.dummy;
        m.dummy.level = t.level;
        m.apply_dummy();
        play::install(&mut commands, &mut scene, m);
        next.set(Screen::Match);
        return;
    }
    let Some(m) = showcase(&mut commands, &mut scene, m.as_deref_mut(), &data.0, &mslots, &rules) else { return };
    let slot = &m.data.global.render.select.flourish;
    for i in 0..m.world.fighters.len() {
        let ready = sel.slots.get(i).is_some_and(|p| p.ready && p.dev.is_some());
        let f = &mut m.world.fighters[i];
        let has = m.data.fighters[f.data].moves.iter().any(|(s, _)| s == slot.as_str());
        if ready && f.state == FState::Idle && moments.flourish.contains(&i) {
            moments.flourish.retain(|j| *j != i);
            if has && f.ground {
                let mut f = f.clone();
                sim::moves::start_move(&mut m.world, &m.data, i, &mut f, slot);
                m.world.fighters[i] = f;
            } else {
                f.set_state(FState::Win);
            }
        } else if ready && f.state == FState::Idle {
            f.set_state(FState::Win);
        } else if !ready && f.state == FState::Win {
            f.set_state(FState::Idle);
        }
    }
}

/// Keeps the showcase (the lineup standing on `rules.stage` behind the screen) in step: rebuilt when the lineup's
/// size, colors or stage change, standing left to right in card order; otherwise a changed pick is swapped in place
/// so nobody else moves. Everyone is on one team so flourishes never hit anybody. `None` while it is being rebuilt
/// (or nothing fits).
pub(crate) fn showcase<'a>(
    commands: &mut Commands,
    scene: &mut render::Scene,
    m: Option<&'a mut Match>,
    data: &std::sync::Arc<sim::GameData>,
    mslots: &[Slot],
    rules: &Rules,
) -> Option<&'a mut Match> {
    if mslots.is_empty() || data.stage(&rules.stage).is_none_or(|s| s.max_players() < mslots.len()) {
        return None;
    }
    let m = m.filter(|m| {
        m.mode == Mode::Showcase
            && m.slots.iter().map(|s| s.color).eq(mslots.iter().map(|s| s.color))
            && m.rules.stage == rules.stage
    });
    let Some(m) = m else {
        let mut slots = mslots.to_vec();
        for s in &mut slots {
            s.cpu = None;
            s.dev = None;
            s.team = Some(0);
        }
        let rules = Rules { friendly_fire: false, ..rules.clone() };
        if let Ok(mut m) = Match::new(data, Mode::Showcase, slots, rules, 1) {
            let mut spots: Vec<_> = m.world.fighters.iter().map(|f| (f.x, f.y, f.surf, f.ground, f.face)).collect();
            spots.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (f, (x, y, surf, ground, face)) in m.world.fighters.iter_mut().zip(spots) {
                (f.x, f.y, f.surf, f.ground, f.face) = (x, y, surf, ground, face);
            }
            play::install(commands, scene, m);
        }
        return None;
    };
    for (i, s) in mslots.iter().enumerate() {
        if m.slots[i].fighter != s.fighter {
            swap_fighter(m, i, s.fighter);
        }
    }
    Some(m)
}

/// Put another fighter in showcase slot `i`, where it stands.
fn swap_fighter(m: &mut Match, i: usize, fighter: usize) {
    let f = &mut m.world.fighters[i];
    f.data = fighter;
    f.mov = None;
    f.jumps = m.data.fighters[fighter].stats.air_jumps;
    f.set_state(if f.ground { FState::Idle } else { FState::Air });
    m.world.projs.retain(|p| p.owner != i);
    m.world.cfg.players[i].fighter = m.data.fighters[fighter].id.clone();
    m.slots[i].fighter = fighter;
}

pub(crate) fn remember(sel: &Select, saved: &mut Saved, d: &sim::GameData) {
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
    v.humans = sel.slots.iter().filter(|p| p.dev.is_some() || p.disconnected).map(pref).collect();
    v.cpus = sel.slots.iter().filter(|p| p.dev.is_none() && !p.disconnected).map(pref).collect();
}

#[derive(Component)]
pub struct SelectRoot(Select);

/// Animated by [`animate`]: a card flipping to READY, the sliding ready banner.
#[derive(Component)]
pub enum Motion {
    Flip(Dev),
    Banner,
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0., 1.) - 1.;
    1. + 2.7 * t * t * t + 1.7 * t * t
}

pub fn animate(
    time: Res<Time>,
    moments: Option<Res<Moments>>,
    data: Res<Data>,
    mut q: Query<(&Motion, &mut UiTransform)>,
) {
    let Some(mo) = moments else { return };
    let look = &data.0.global.render.select;
    let now = time.elapsed_secs();
    for (m, mut tf) in &mut q {
        match m {
            Motion::Flip(dev) => {
                let t = mo.locked.iter().find(|(d, _)| d == dev).map_or(1., |(_, at)| (now - at) / look.flip_secs);
                let sx = ease(t).max(0.02);
                if tf.scale.x != sx {
                    tf.scale.x = sx;
                }
            }
            Motion::Banner => {
                let t = mo.banner.map_or(1., |at| (now - at) / look.banner_secs);
                let x = Val::Vw((1. - ease(t)) * -110.);
                if tf.translation.x != x {
                    tf.translation.x = x;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut commands: Commands,
    sel: Option<Res<Select>>,
    roots: Query<(Entity, &SelectRoot)>,
    fonts: Res<Fonts>,
    data: Res<Data>,
    devs: Res<Devices>,
    saved: Res<Saved>,
    portraits: Res<Portraits>,
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
                    n.align_items = AlignItems::FlexStart;
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
                        n.width = Val::Px(220.);
                        n.padding = UiRect::all(Val::Px(8.));
                        n.border_radius = BorderRadius::all(Val::Px(10.));
                    }),
                    BackgroundColor(NIGHT),
                ))
                .with_children(|p| {
                    // Stocks and the stage are chosen on stage select in versus.
                    let onoff = |b: bool| if b { "On" } else { "Off" };
                    let mut rules = vec![];
                    if s.mode == Mode::Versus {
                        rules.push((TEAMS, "Teams", onoff(s.teams).into()));
                        if s.teams {
                            rules.push((FF, "Friendly fire", onoff(s.ff).into()));
                        }
                    } else {
                        rules.push((STAGE, "Stage", d.stages[s.stage].name.clone()));
                    }
                    for (k, label, value) in rules {
                        spin_row(p, f, d, s, key(k, 0), label, value, None, 17.);
                    }
                });
            });
            let bottom = roster(p, f, d, s, &portraits);
            if s.ready() {
                // Centered in the showcase strip between the roster and the cards.
                ready_banner(p, f, (bottom + 545.) / 2. - 40.);
            }
            // Bottom: cards and hints.
            p.spawn((
                node(|n| {
                    n.position_type = PositionType::Absolute;
                    n.bottom = Val::Px(14.);
                    n.left = Val::Px(0.);
                    n.right = Val::Px(0.);
                    n.flex_direction = FlexDirection::Column;
                    n.align_items = AlignItems::Center;
                    n.row_gap = Val::Px(10.);
                }),
                Safe::Bottom,
            ))
            .with_children(|p| {
                if !d.stages.iter().any(|stage| stage.max_players() >= s.slots.len()) {
                    p.spawn(txt(&f.heavy, 22., GOLD, "No stage fits this lineup. Remove a player."));
                } else if let Some(reason) = s.start_reason() {
                    p.spawn(txt(&f.heavy, 22., GOLD, reason));
                }
                let free = s.slots.len() < s.max() && !(s.mode == Mode::Training && s.humans().count() > 0);
                let n = s.slots.len() + usize::from(free);
                let w = (1240. / n as f32 - 10.).min(230.);
                p.spawn(node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.column_gap = Val::Px(10.);
                    n.align_items = AlignItems::Stretch;
                }))
                .with_children(|p| {
                    for i in 0..s.slots.len() {
                        card(p, f, d, s, i, w, &devs, &saved);
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
                    hint(p, f, "Attack / Enter", "pick & lock in");
                    hint(p, f, "Special / Esc", "back out");
                    hint(p, f, "Grab / Shield", "card options");
                    hint(p, f, "Start", "fight when everyone's ready");
                });
            });
        },
    );
}

/// The fighter tiles and the random tile, with every player's chip on the tile it points at. Returns the grid's
/// bottom edge.
fn roster(p: &mut ChildSpawnerCommands, f: &Fonts, d: &sim::GameData, s: &Select, portraits: &Portraits) -> f32 {
    let n = d.fighters.len() + 1;
    let l = ROSTER.layout(n);
    let left = 640. - (l.cols as f32 * (l.w + ROSTER.gap) - ROSTER.gap) / 2. - 8.;
    let place = |n: &mut Node| {
        n.top = Val::Px(ROSTER_TOP);
        n.left = Val::Px(left);
    };
    grid::panel(p, l, ROSTER.gap, place, Safe::Top, |p| {
        for t in 0..n {
            let k = key(TILE, t);
            let chips = s
                .humans()
                .filter(|(_, p)| p.cursor.key == k)
                .map(|(i, p)| {
                    let (color, _, light) = pal(d, s.color(i));
                    Chip { label: format!("P{}", i + 1), color, light, locked: p.ready }
                })
                .collect();
            let random = t >= d.fighters.len();
            let name = if random { "Random" } else { &d.fighters[t].name };
            let tile = grid::Tile { key: k, w: l.w, h: l.h, name, chips, highlight: None, off: None };
            let lead = tile.chips.first().map(|c| c.color);
            grid::tile(p, f, tile, |p| {
                if random {
                    p.spawn(node(|n| {
                        grid::fill(n);
                        n.justify_content = JustifyContent::Center;
                        n.align_items = AlignItems::Center;
                        n.padding.bottom = Val::Px(l.h * 0.1);
                    }))
                    .with_child(txt(&f.display, l.w * 0.62, lead.unwrap_or(GOLD), "?"));
                } else {
                    p.spawn((
                        node(grid::fill),
                        ImageNode { rect: Some(Portraits::rect(t)), ..ImageNode::new(portraits.image.clone()) },
                    ));
                }
            });
        }
    });
    ROSTER_TOP + 16. + l.rows as f32 * (l.h + ROSTER.gap) - ROSTER.gap
}

fn ready_banner(p: &mut ChildSpawnerCommands, f: &Fonts, top: f32) {
    p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.top = Val::Px(top);
            n.left = Val::Px(0.);
            n.right = Val::Px(0.);
            n.justify_content = JustifyContent::Center;
        }),
        UiTransform::from_translation(Val2::new(Val::Vw(-110.), Val::Px(0.))),
        Motion::Banner,
    ))
    .with_children(|p| {
        p.spawn((
            node(|n| {
                n.width = Val::Percent(100.);
                n.padding = UiRect::axes(Val::Px(60.), Val::Px(4.));
                n.flex_direction = FlexDirection::Row;
                n.justify_content = JustifyContent::Center;
                n.align_items = AlignItems::Center;
                n.column_gap = Val::Px(24.);
            }),
            BackgroundGradient::from(LinearGradient::to_right(vec![
                ColorStop::auto(alpha(GOLD, 0.)),
                ColorStop::auto(alpha(GOLD, 0.96)),
                ColorStop::auto(alpha(GOLD, 0.96)),
                ColorStop::auto(alpha(GOLD, 0.)),
            ])),
            UiTransform::from_rotation(Rot2::degrees(-2.)),
        ))
        .with_children(|p| {
            p.spawn(txt(&f.display, 54., NIGHT, "Ready to fight"));
            p.spawn((
                node(|n| {
                    n.padding = UiRect::axes(Val::Px(10.), Val::Px(1.));
                    n.border_radius = BorderRadius::all(Val::Px(6.));
                }),
                BackgroundColor(NIGHT),
            ))
            .with_child(txt(&f.heavy, 22., GOLD, "press Start"));
        });
    });
}

/// Keyboard (L / R marks a split half) or gamepad outline for a card header.
fn device_glyph(p: &mut ChildSpawnerCommands, f: &Fonts, dev: Dev, ink: Color) {
    let (pad, half) = match dev {
        Dev::Pad(_) => (true, None),
        Dev::Keys => (false, None),
        Dev::KeysL => (false, Some("L")),
        Dev::KeysR => (false, Some("R")),
        // Scripted bots stand in for a gamepad and the right keyboard half.
        Dev::Bot(n) => (n % 2 == 0, (n % 2 == 1).then_some("R")),
    };
    let rect = |p: &mut ChildSpawnerCommands, x: f32, y: f32, w: f32, h: f32, r: f32, c: Color| {
        p.spawn((
            node(|n| {
                n.position_type = PositionType::Absolute;
                n.left = Val::Px(x);
                n.top = Val::Px(y);
                n.width = Val::Px(w);
                n.height = Val::Px(h);
                n.border_radius = BorderRadius::all(Val::Px(r));
            }),
            BackgroundColor(c),
        ));
    };
    p.spawn(row()).with_children(|p| {
        if let Some(h) = half {
            p.spawn((txt(&f.display, 17., ink, h), node(|n| n.margin.right = Val::Px(4.))));
        }
        p.spawn(node(|n| {
            n.width = Val::Px(30.);
            n.height = Val::Px(19.);
        }))
        .with_children(|p| {
            if pad {
                rect(p, 0., 1., 30., 17., 8.5, ink);
                rect(p, 5., 7.5, 8., 2.5, 0.5, Color::WHITE.with_alpha(0.9));
                rect(p, 7.75, 4.75, 2.5, 8., 0.5, Color::WHITE.with_alpha(0.9));
                rect(p, 19., 5., 3.5, 3.5, 2., Color::WHITE.with_alpha(0.9));
                rect(p, 22.5, 9., 3.5, 3.5, 2., Color::WHITE.with_alpha(0.9));
            } else {
                rect(p, 0., 1., 30., 17., 3., ink);
                for (r, n) in [(0, 6), (1, 5)] {
                    for c in 0..n {
                        let x = 2.5 + c as f32 * 4.3 + r as f32 * 2.;
                        rect(p, x, 3.5 + r as f32 * 4.5, 3., 3., 0.5, Color::WHITE.with_alpha(0.9));
                    }
                }
                rect(p, 8., 12.5, 14., 3., 0.5, Color::WHITE.with_alpha(0.9));
            }
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn card(
    p: &mut ChildSpawnerCommands,
    f: &Fonts,
    d: &sim::GameData,
    s: &Select,
    i: usize,
    w: f32,
    devs: &Devices,
    saved: &Saved,
) {
    let pk = &s.slots[i];
    let (main, dark, _) = pal(d, s.color(i));
    let human = pk.dev.is_some() || pk.disconnected;
    let ready = pk.ready && pk.dev.is_some();
    let narrow = w < 170.;
    let fighter = &d.fighters[pk.fighter];
    let mut card = p.spawn((
        node(|n| {
            n.width = Val::Px(w);
            n.min_height = Val::Px(118.);
            n.flex_direction = FlexDirection::Column;
            n.border = UiRect::all(Val::Px(2.));
            n.border_radius = BorderRadius::all(Val::Px(10.));
        }),
        BackgroundColor(if ready { dark.mix(&NIGHT, 0.55) } else { INK }),
        BorderColor::all(if ready { main } else { alpha(main, 0.35) }),
        BoxShadow::new(alpha(main, if ready { 0.5 } else { 0. }), Val::Px(0.), Val::Px(0.), Val::Px(2.), Val::Px(18.)),
    ));
    if let Some(dev) = pk.dev.filter(|_| ready) {
        card.insert((UiTransform::default(), Motion::Flip(dev)));
    }
    card.with_children(|p| {
        // Header band: tag and device (or CPU level).
        p.spawn((
            node(|n| {
                n.flex_direction = FlexDirection::Row;
                n.align_items = AlignItems::Center;
                n.justify_content = JustifyContent::SpaceBetween;
                n.padding = UiRect::axes(Val::Px(10.), Val::Px(1.));
                n.border_radius = BorderRadius::top(Val::Px(8.));
            }),
            BackgroundGradient::from(LinearGradient::to_right(vec![ColorStop::auto(main), ColorStop::auto(dark)])),
        ))
        .with_children(|p| {
            p.spawn(txt(&f.display, 24., NIGHT, s.tag(i)));
            match pk.dev {
                Some(dv) => device_glyph(p, f, dv, alpha(NIGHT, 0.85)),
                None if pk.disconnected => {
                    p.spawn(txt(&f.body_bold, 11., alpha(NIGHT, 0.8), "Disconnected"));
                }
                None if s.mode == Mode::Versus => {
                    p.spawn(txt(&f.body_bold, 12., alpha(NIGHT, 0.8), LEVELS[pk.level as usize]));
                }
                None => {}
            }
        });
        p.spawn(node(|n| {
            n.flex_direction = FlexDirection::Column;
            n.padding = UiRect::new(Val::Px(10.), Val::Px(10.), Val::Px(4.), Val::Px(6.));
            n.flex_grow = 1.;
        }))
        .with_children(|p| {
            let name_size = if narrow { 22. } else { 28. };
            let (name, line) = if pk.random && !ready {
                ("Random".to_string(), "Rolled when you lock in".to_string())
            } else {
                (fighter.name.clone(), fighter.tagline.clone())
            };
            if human {
                p.spawn((txt(&f.display, name_size, STAR, name), TextLayout::new(Justify::Left, LineBreak::NoWrap)));
            } else {
                let k = key(FIGHTER, i);
                let on = focused_by(s, k);
                p.spawn((
                    node(|n| {
                        n.flex_direction = FlexDirection::Row;
                        n.align_items = AlignItems::Center;
                        n.column_gap = Val::Px(4.);
                        n.padding = UiRect::horizontal(Val::Px(4.));
                        n.margin = UiRect::horizontal(Val::Px(-4.));
                        n.border_radius = BorderRadius::all(Val::Px(5.));
                    }),
                    BackgroundColor(on.map_or(Color::NONE, |j| alpha(pal(d, s.color(j)).0, 0.16))),
                    Focusable { key: k, owner: None, spin: true },
                ))
                .with_children(|p| {
                    if on.is_some() {
                        p.spawn(txt(&f.display, name_size * 0.8, GOLD, "‹"));
                    }
                    p.spawn((
                        txt(&f.display, name_size, STAR, name),
                        TextLayout::new(Justify::Left, LineBreak::NoWrap),
                    ));
                    if on.is_some() {
                        p.spawn(txt(&f.display, name_size * 0.8, GOLD, "›"));
                    }
                    cursor_chips(p, f, d, s, k);
                });
            }
            // Narrow cards wrap the tagline to two lines; anything longer is cut.
            p.spawn(node(|n| {
                n.height = Val::Px(if narrow { 28. } else { 15. });
                n.overflow = Overflow::clip();
                n.margin.bottom = Val::Px(4.);
            }))
            .with_child((txt(&f.body, 11., DIM, line), TextLayout::new(Justify::Left, LineBreak::WordBoundary)));
            if !human && s.mode == Mode::Versus {
                spin_row(p, f, d, s, key(LEVEL, i), "Level", LEVELS[pk.level as usize].into(), None, 16.);
            }
            if !human && s.teams {
                let short = team_name(pk.team).trim_end_matches(" team").into();
                spin_row(p, f, d, s, key(TEAM, i), "Team", short, Some(pal(d, pk.team as usize).0), 16.);
            }
            if s.mode == Mode::Training && !human {
                spin_row(p, f, d, s, key(BEHAVIOR, i), "Behavior", s.dummy.label().into(), None, 16.);
            }
            if pk.disconnected || !human && s.mode == Mode::Versus {
                let k = key(REMOVE, i);
                let on = focused_by(s, k);
                p.spawn((
                    node(|n| {
                        n.flex_direction = FlexDirection::Row;
                        n.column_gap = Val::Px(6.);
                        n.margin.top = Val::Auto;
                        n.padding = UiRect::axes(Val::Px(6.), Val::Px(1.));
                        n.border_radius = BorderRadius::all(Val::Px(5.));
                    }),
                    BackgroundColor(on.map_or(Color::NONE, |j| alpha(pal(d, s.color(j)).0, 0.16))),
                    Focusable { key: k, owner: None, spin: false },
                ))
                .with_children(|p| {
                    p.spawn(txt(&f.label, 15., if on.is_some() { STAR } else { FAINT }, "Remove"));
                    cursor_chips(p, f, d, s, k);
                });
            }
            if let (Some(dev), true) = (pk.dev, human) {
                if ready {
                    p.spawn((
                        node(|n| {
                            n.margin = UiRect::new(Val::Px(-10.), Val::Px(-10.), Val::Auto, Val::Px(-6.));
                            n.justify_content = JustifyContent::Center;
                            n.border_radius = BorderRadius::bottom(Val::Px(8.));
                        }),
                        BackgroundColor(main),
                    ))
                    .with_child(txt(&f.display, 28., NIGHT, "READY!"));
                } else {
                    // Which of this device's controls opens the options.
                    let b = input::binds(saved, &devs.profile(dev));
                    let grab = b[Act::Grab as usize].first().map_or("Grab".into(), |c| input::control_label(c));
                    p.spawn(node(|n| {
                        n.margin.top = Val::Auto;
                        n.flex_direction = FlexDirection::Row;
                        n.align_items = AlignItems::Center;
                        n.column_gap = Val::Px(5.);
                    }))
                    .with_children(|p| {
                        p.spawn((
                            node(|n| {
                                n.padding = UiRect::axes(Val::Px(5.), Val::Px(0.));
                                n.border = UiRect::all(Val::Px(1.));
                                n.border_radius = BorderRadius::all(Val::Px(4.));
                            }),
                            BorderColor::all(FAINT),
                        ))
                        .with_child(txt(&f.body_bold, 11., DIM, grab));
                        p.spawn(txt(&f.body, 11., FAINT, "options"));
                    });
                }
            }
        });
        if let Some(r) = pk.menu {
            options(p, f, d, s, i, r);
        }
    });
}

/// A player's options popover, above their card.
fn options(p: &mut ChildSpawnerCommands, f: &Fonts, d: &sim::GameData, s: &Select, i: usize, r: usize) {
    let pk = &s.slots[i];
    let (main, _, _) = pal(d, s.color(i));
    p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.bottom = Val::Percent(100.);
            n.left = Val::Px(-2.);
            n.min_width = Val::Px(190.);
            n.margin.bottom = Val::Px(8.);
            n.flex_direction = FlexDirection::Column;
            n.padding = UiRect::all(Val::Px(8.));
            n.row_gap = Val::Px(2.);
            n.border = UiRect::all(Val::Px(2.));
            n.border_radius = BorderRadius::all(Val::Px(10.));
        }),
        BackgroundColor(NIGHT.with_alpha(0.97)),
        BorderColor::all(main),
        BoxShadow::new(alpha(NIGHT, 0.7), Val::Px(0.), Val::Px(4.), Val::Px(0.), Val::Px(14.)),
        GlobalZIndex(8),
    ))
    .with_children(|p| {
        p.spawn(txt(&f.heavy, 15., main, format!("P{} options", i + 1)));
        for (n, kd) in s.options().into_iter().enumerate() {
            let on = n == r;
            let (label, value, color) = match kd {
                TEAM => ("Team", team_name(pk.team).to_string(), Some(pal(d, pk.team as usize).0)),
                _ => ("Up to jump", if pk.tap_jump { "On" } else { "Off" }.to_string(), None),
            };
            p.spawn((
                node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.justify_content = JustifyContent::SpaceBetween;
                    n.column_gap = Val::Px(14.);
                    n.padding = UiRect::axes(Val::Px(8.), Val::Px(2.));
                    n.border_radius = BorderRadius::all(Val::Px(5.));
                }),
                BackgroundColor(if on { alpha(main, 0.2) } else { Color::NONE }),
            ))
            .with_children(|p| {
                p.spawn(txt(&f.label, 17., if on { STAR } else { DIM }, label));
                let v = if on { format!("‹ {value} ›") } else { value };
                p.spawn(txt(&f.label, 18., color.unwrap_or(STAR), v));
            });
        }
        p.spawn((
            txt(&f.body, 11., FAINT, "↑↓ choose · ←→ change · Special close"),
            node(|n| n.margin.top = Val::Px(4.)),
        ));
    });
}

/// Chips of the cursors sitting on the control `k`.
fn cursor_chips(p: &mut ChildSpawnerCommands, f: &Fonts, d: &sim::GameData, s: &Select, k: u32) {
    for (i, pk) in s.humans() {
        if pk.cursor.key == k && !pk.ready {
            p.spawn((
                node(|n| {
                    n.padding = UiRect::axes(Val::Px(5.), Val::Px(0.));
                    n.border_radius = BorderRadius::all(Val::Px(4.));
                }),
                BackgroundColor(pal(d, s.color(i)).0),
            ))
            .with_child(txt(&f.display, 13., NIGHT, format!("P{}", i + 1)));
        }
    }
}

fn focused_by(s: &Select, k: u32) -> Option<usize> {
    s.humans().find(|(_, p)| p.cursor.key == k && !p.ready).map(|(i, _)| i)
}

#[allow(clippy::too_many_arguments)]
fn spin_row(
    p: &mut ChildSpawnerCommands,
    f: &Fonts,
    d: &sim::GameData,
    s: &Select,
    k: u32,
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
            n.padding = UiRect::axes(Val::Px(6.), Val::Px(2.));
            n.margin = UiRect::horizontal(Val::Px(-4.));
            n.border_radius = BorderRadius::all(Val::Px(5.));
        }),
        BackgroundColor(tint),
        Focusable { key: k, owner: None, spin: true },
    ))
    .with_children(|p| {
        p.spawn(row()).with_children(|p| {
            p.spawn((txt(&f.label, size, DIM, label), node(|n| n.margin.right = Val::Px(6.))));
            cursor_chips(p, f, d, s, k);
        });
        let v = if on.is_some() { format!("‹ {value} ›") } else { value };
        p.spawn(txt(&f.label, size + 1., color.unwrap_or(STAR), v));
    });
}

fn join_card(p: &mut ChildSpawnerCommands, f: &Fonts, d: &sim::GameData, s: &Select, w: f32) {
    p.spawn((
        node(|n| {
            n.width = Val::Px(w);
            n.min_height = Val::Px(118.);
            n.flex_direction = FlexDirection::Column;
            n.justify_content = JustifyContent::Center;
            n.align_items = AlignItems::Center;
            n.row_gap = Val::Px(10.);
            n.border = UiRect::all(Val::Px(2.));
            n.border_radius = BorderRadius::all(Val::Px(10.));
            n.padding = UiRect::all(Val::Px(10.));
        }),
        BackgroundColor(INK_SOFT),
        BorderColor::all(alpha(FAINT, 0.6)),
    ))
    .with_children(|p| {
        p.spawn((
            txt(&f.heavy, 20., DIM, "Press any button to join"),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        ));
        if s.mode == Mode::Versus && s.humans().count() > 0 {
            let k = key(ADD, 0);
            let on = focused_by(s, k).is_some();
            p.spawn((
                node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.column_gap = Val::Px(6.);
                    n.padding = UiRect::axes(Val::Px(12.), Val::Px(3.));
                    n.border = UiRect::all(Val::Px(1.5));
                    n.border_radius = BorderRadius::all(Val::Px(6.));
                }),
                BorderColor::all(if on { GOLD } else { FAINT }),
                Focusable { key: k, owner: None, spin: false },
            ))
            .with_children(|p| {
                p.spawn(txt(&f.label, 17., if on { GOLD } else { DIM }, "+ Add CPU"));
                cursor_chips(p, f, d, s, k);
            });
        }
    });
}

pub fn team_name(t: u8) -> &'static str {
    ["Red team", "Blue team", "Green team", "Yellow team"][t as usize % 4]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (Select, Devices, sim::GameData, Saved) {
        let (data, errors) = crate::load_data(&[std::path::PathBuf::from("../../assets")]);
        assert!(errors.is_empty());
        let saved = Saved::default();
        let mut s = Select {
            mode: Mode::Versus,
            slots: vec![],
            stocks: 3,
            stage: 0,
            teams: false,
            ff: false,
            dummy: DummyMode::Stand,
            roll: 1,
        };
        s.join(Dev::Bot(0), &saved, &data);
        s.join(Dev::Bot(1), &saved, &data);
        s.add_cpu(&saved, &data);
        let mut devices = Devices::default();
        devices.map.insert(Dev::Bot(0), default());
        devices.map.insert(Dev::Bot(1), default());
        (s, devices, data, saved)
    }
    fn spot(k: u32, spin: bool) -> Spot {
        Spot { f: Focusable { key: k, owner: None, spin }, center: Vec2::ZERO }
    }
    #[test]
    fn simultaneous_remove_and_edit_uses_original_lineup() {
        let (mut s, mut devs, d, saved) = setup();
        s.slots[0].cursor.key = key(REMOVE, 2);
        s.slots[1].cursor.key = key(LEVEL, 2);
        devs.map.get_mut(&Dev::Bot(0)).unwrap().nav = vec![Nav::Confirm];
        devs.map.get_mut(&Dev::Bot(1)).unwrap().nav = vec![Nav::Right];
        let spots = [spot(key(REMOVE, 2), false), spot(key(LEVEL, 2), true)];
        s.frame(&devs, &spots, &saved, &d);
        assert_eq!(s.slots.len(), 2);
        for (i, p) in s.slots.iter().enumerate() {
            assert_eq!(p.cursor, Cursor { owner: Some(i), key: key(TILE, p.fighter) });
        }
        s.spin(key(LEVEL, 7), 1, &d); // stale layout is harmless
    }
    #[test]
    fn cpu_row_focus_survives_lineup_changes() {
        let (mut s, mut devs, d, saved) = setup();
        s.add_cpu(&saved, &d);
        s.slots[0].cursor.key = key(TEAM, 3);
        s.slots[1].cursor.key = key(ADD, 0);
        devs.map.get_mut(&Dev::Bot(1)).unwrap().nav = vec![Nav::Confirm];
        s.frame(&devs, &[spot(key(ADD, 0), false)], &saved, &d);
        assert_eq!((s.slots.len(), s.slots[0].cursor.key), (5, key(TEAM, 3)));
        s.slots[1].cursor.key = key(REMOVE, 2);
        s.frame(&devs, &[spot(key(REMOVE, 2), false)], &saved, &d);
        assert_eq!((s.slots.len(), s.slots[0].cursor.key), (4, key(TEAM, 2)));
    }
    #[test]
    fn start_and_back_in_one_frame_does_not_start() {
        let (mut s, mut devs, d, saved) = setup();
        for p in &mut s.slots {
            p.ready = true;
        }
        devs.map.get_mut(&Dev::Bot(0)).unwrap().nav = vec![Nav::Start];
        devs.map.get_mut(&Dev::Bot(1)).unwrap().nav = vec![Nav::Back];
        assert!(!s.frame(&devs, &[], &saved, &d).0);
        assert!(!s.slots[1].ready);
    }
    #[test]
    fn simultaneous_leave_and_join_keeps_new_player() {
        let (mut s, mut devs, d, saved) = setup();
        s.slots.truncate(1);
        s.slots[0].cursor.key = key(TILE, 0);
        devs.map.get_mut(&Dev::Bot(0)).unwrap().nav = vec![Nav::Back];
        devs.map.get_mut(&Dev::Bot(1)).unwrap().nav = vec![Nav::Confirm];
        let (_, leave) = s.frame(&devs, &[spot(key(TILE, 0), false)], &saved, &d);
        assert!(!leave);
        assert_eq!(s.slots.len(), 1);
        assert_eq!(s.slots[0].dev, Some(Dev::Bot(1)));
        assert!(!s.slots[0].ready);
    }
    #[test]
    fn disconnect_releases_unready_slot_and_ready_slot_can_be_reclaimed() {
        let (mut s, mut devs, d, saved) = setup();
        s.slots[0].ready = true;
        devs.map.clear();
        s.frame(&devs, &[], &saved, &d);
        assert_eq!(s.slots.len(), 2);
        assert!(s.slots[0].disconnected);
        assert_eq!(s.slots[0].dev, None);
        assert!(s.join(Dev::Bot(2), &saved, &d));
        assert_eq!(s.slots[0].dev, Some(Dev::Bot(2)));
        assert!(!s.slots[0].ready);
    }
    #[test]
    fn another_device_can_remove_a_ready_disconnected_player() {
        let (mut s, mut devs, d, saved) = setup();
        s.slots[0].ready = true;
        s.slots[1].cursor.key = key(REMOVE, 0);
        devs.map.remove(&Dev::Bot(0));
        devs.map.get_mut(&Dev::Bot(1)).unwrap().nav = vec![Nav::Confirm];
        s.frame(&devs, &[spot(key(REMOVE, 0), false)], &saved, &d);
        assert_eq!(s.slots.len(), 2);
        assert_eq!(s.slots[0].dev, Some(Dev::Bot(1)));
        assert!(!s.slots.iter().any(|p| p.disconnected));
    }
    #[test]
    fn teams_need_opposition_and_stages_fit_lineup() {
        let (mut s, _, mut d, saved) = setup();
        s.teams = true;
        for p in &mut s.slots {
            p.team = 0;
            p.ready = true;
        }
        assert!(!s.ready());
        s.slots[1].team = 1;
        assert!(s.ready());
        d.stages.truncate(1);
        let mut small = d.stages[0].clone();
        small.spawns.truncate(2);
        small.respawns.truncate(2);
        d.stages.insert(0, small);
        s.stage = 0;
        s.fit_stage(&d);
        assert_eq!(s.stage, 1);
        s.spin(key(STAGE, 0), 1, &d);
        assert_eq!(s.stage, 1);
        while s.slots.len() < 8 {
            s.add_cpu(&saved, &d);
        }
        assert_eq!(s.slots.len(), 8);
    }
    #[test]
    fn chips_preview_on_hover_lock_on_attack_and_random_rolls() {
        let (mut s, mut devs, d, saved) = setup();
        let n = d.fighters.len();
        let spots: Vec<Spot> = (0..=n)
            .map(|t| Spot {
                f: Focusable { key: key(TILE, t), owner: None, spin: false },
                center: Vec2::new(t as f32 * 100., 0.),
            })
            .collect();
        s.slots[0].cursor.key = key(TILE, 0);
        devs.map.get_mut(&Dev::Bot(0)).unwrap().nav = vec![Nav::Right];
        s.frame(&devs, &spots, &saved, &d);
        assert_eq!((s.slots[0].fighter, s.slots[0].ready), (1, false));
        devs.map.get_mut(&Dev::Bot(0)).unwrap().nav = vec![Nav::Confirm];
        s.frame(&devs, &spots, &saved, &d);
        assert!(s.slots[0].ready);
        s.slots[1].cursor.key = key(TILE, n - 1);
        devs.map.get_mut(&Dev::Bot(0)).unwrap().nav = vec![];
        devs.map.get_mut(&Dev::Bot(1)).unwrap().nav = vec![Nav::Right];
        s.frame(&devs, &spots, &saved, &d);
        assert!(s.slots[1].random);
        devs.map.get_mut(&Dev::Bot(1)).unwrap().nav = vec![Nav::Confirm];
        s.frame(&devs, &spots, &saved, &d);
        assert!(s.slots[1].ready && !s.slots[1].random && s.slots[1].fighter < n);
    }
    #[test]
    fn options_popover_takes_presses_until_closed() {
        let (mut s, mut devs, d, saved) = setup();
        s.teams = true;
        let tap = s.slots[0].tap_jump;
        let st = devs.map.get_mut(&Dev::Bot(0)).unwrap();
        st.pressed[Act::Grab as usize] = true;
        st.nav = vec![Nav::Down, Nav::Right];
        s.frame(&devs, &[], &saved, &d);
        assert_eq!((s.slots[0].menu, s.slots[0].tap_jump), (Some(1), !tap));
        let st = devs.map.get_mut(&Dev::Bot(0)).unwrap();
        st.pressed = [false; 10];
        st.nav = vec![Nav::Back];
        s.frame(&devs, &[], &saved, &d);
        assert_eq!((s.slots.len(), s.slots[0].menu), (3, None));
    }
    #[test]
    fn cpu_team_row_spins_that_cpu() {
        let (mut s, mut devs, d, saved) = setup();
        s.teams = true;
        s.slots[0].cursor.key = key(TEAM, 2);
        let team = s.slots[2].team;
        devs.map.get_mut(&Dev::Bot(0)).unwrap().nav = vec![Nav::Right];
        s.frame(&devs, &[spot(key(TEAM, 2), true)], &saved, &d);
        assert_eq!(s.slots[2].team, (team + 1) % N_TEAMS);
        assert_eq!(s.slots[0].team, 0);
    }
}
