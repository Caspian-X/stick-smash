//! Join + character select (versus and training). Any free device presses a button to claim the next slot. Each
//! player steers a pointer ([`crate::pointer`]) over the roster grid, CPU cards and the rules panel, holding their
//! chip: hovering a tile previews the fighter, attack drops the chip there (locked in; the "?" tile rolls a random
//! fighter) and frees the pointer. Confirming on another tile switches, on your own chip picks it back up; special
//! picks it up too (and, unlocked, on the roster leaves). Confirming on a CPU card's fighter carries that CPU's chip
//! to a tile. Grab / Shield opens a player's card options (team, up to jump). Start (or attack on your chip or on
//! nothing) goes on once everyone is ready: to stage select in versus, straight into the match in training. The
//! picked fighters stand on the stage in a strip between the roster and the cards, and a locked-in fighter plays
//! `render.select.flourish` before its win pose. Each card shows a hint or two for its player's context in that
//! device's controls, quiet unless the player just joined, idles, points at their card or everyone is ready.

use bevy::prelude::*;
use render::portrait::{Portrait, Portraits};
use sim::world::State as FState;

use std::collections::BTreeMap;

use bevy::ui::UiGlobalTransform;

use crate::grid::{self, Area, Chip, key, kind};
use crate::input::{self, Act, Dev, Devices};
use crate::play::{self, Match, Mode, Rules, Slot};
use crate::pointer::{self, Grid, Look, NONE, Pointer, Pointers};
use crate::settings::{DummyMode, PlayerPref, Saved};
use crate::ui::{self, *};
use crate::{Data, Screen};

/// Roster tile `t` (a fighter index; one past the last is the random tile).
pub(crate) const TILE: u32 = 1;
/// CPU / dummy card controls.
pub(crate) const FIGHTER: u32 = 2;
const LEVEL: u32 = 3;
const REMOVE: u32 = 5;
const BEHAVIOR: u32 = 12;
const ADD: u32 = 6;
/// Rules panel.
pub(crate) const STOCKS: u32 = 7;
const STAGE: u32 = 8;
const TEAMS: u32 = 9;
const FF: u32 = 10;
/// A human's options popover rows (only that player's pointer reaches them, and only them while it is open). `TEAM`
/// is also a CPU card row.
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
    /// What the player's pointer is on.
    pub cursor: Cursor,
    /// Options popover open; the key to return to.
    pub menu: Option<u32>,
    /// Carrying a CPU's chip: its slot and the fighter it had.
    pub carry: Option<(usize, usize)>,
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

/// When things happened, for the lock-in flip, the chip drop, the ready banner, the stage flourish and the hints.
#[derive(Resource, Default)]
pub struct Moments {
    /// Device → seconds when it locked in.
    locked: Vec<(Dev, f32)>,
    banner: Option<f32>,
    /// Slots whose fighter should play the flourish on the stage.
    flourish: Vec<usize>,
    /// Device → (seconds it joined, seconds of its last input).
    seen: BTreeMap<Dev, (f32, f32)>,
    /// Device → hint opacity.
    hints: BTreeMap<Dev, f32>,
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
                carry: None,
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
            carry: None,
        });
    }
    fn roll_random(&mut self, i: usize, d: &sim::GameData) {
        let p = &mut self.slots[i];
        if p.random {
            self.roll ^= self.roll << 13;
            self.roll ^= self.roll >> 17;
            self.roll ^= self.roll << 5;
            p.fighter = self.roll as usize % d.fighters.len();
            p.random = false;
        }
    }
    /// Drop player `i`'s chip on its fighter (the pointer stays free).
    fn lock(&mut self, i: usize, d: &sim::GameData) {
        self.roll_random(i, d);
        let p = &mut self.slots[i];
        p.ready = true;
        if let Some(k) = p.menu.take() {
            p.cursor.key = k;
        }
    }
    /// Player `i`'s pointer moved onto tile `t`: preview it for the chip in hand (theirs or a carried CPU's).
    fn hover(&mut self, i: usize, t: usize, d: &sim::GameData) {
        let j = match self.slots[i].carry {
            Some((c, _)) => c,
            None if !self.slots[i].ready => i,
            None => return,
        };
        let p = &mut self.slots[j];
        p.random = t >= d.fighters.len();
        if !p.random {
            p.fighter = t;
        }
    }
    /// Put down a carried CPU chip: where it is (`keep`), or back where it came from.
    fn drop_carry(&mut self, i: usize, keep: bool, d: &sim::GameData) {
        let Some((c, f)) = self.slots[i].carry.take() else { return };
        if keep {
            self.roll_random(c, d);
        } else if let Some(p) = self.slots.get_mut(c) {
            (p.fighter, p.random) = (f, false);
        }
    }
    fn toggle_menu(&mut self, i: usize, ptr: &mut Pointer) {
        let first = self.options()[0];
        let p = &mut self.slots[i];
        match p.menu.take() {
            Some(k) => p.cursor.key = k,
            None => {
                p.menu = Some(p.cursor.key);
                p.cursor.key = key(first, i);
            }
        }
        ptr.hop();
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
    /// Fix cursor owners, keys and carried chips after slots moved. `removed` (sorted, old indices) shifts keys that
    /// point at a later card (a CPU row being edited); a cursor on a removed card or a gone control points at
    /// nothing (its pointer settles on whatever is nearest), and a removed CPU's chip is gone.
    fn renumber(&mut self, removed: &[usize]) {
        let (n, add) = (self.slots.len(), self.mode == Mode::Versus && self.slots.len() < 8);
        let shift = |k: u32| {
            let (kd, si) = kind(k);
            match kd {
                0 | TILE | STOCKS | STAGE | TEAMS | FF => k,
                ADD if add => k,
                ADD => NONE,
                _ if removed.contains(&si) => NONE,
                _ => match si - removed.iter().filter(|&&r| r < si).count() {
                    s if s < n => key(kd, s),
                    _ => NONE,
                },
            }
        };
        for (i, p) in self.slots.iter_mut().enumerate() {
            p.cursor.owner = Some(i);
            p.cursor.key = shift(p.cursor.key);
            p.menu = p.menu.map(shift);
            p.carry = p.carry.filter(|(c, _)| !removed.contains(c)).map(|(c, f)| (kind(shift(key(FIGHTER, c))).1, f));
        }
    }
    /// One frame of input: moves every human's pointer, acts on their presses, joins new devices. Returns (start,
    /// everyone left).
    fn frame(
        &mut self,
        devs: &Devices,
        spots: &[Spot],
        saved: &Saved,
        d: &sim::GameData,
        ptrs: &mut Pointers,
        dt: f32,
    ) -> (bool, bool) {
        let s = self;
        let cfg = &d.global.render.select.pointer;
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
                _ => {}
            }
        }
        for i in 0..s.slots.len() {
            if s.slots[i].dev.is_none() {
                s.drop_carry(i, false, d);
            }
        }
        let n = d.fighters.len() + 1;
        let g = Grid { tile: TILE, n, cols: ROSTER.layout(n).cols };
        let screen = ptrs.screen;
        let humans: Vec<(usize, Dev)> = s.slots.iter().enumerate().filter_map(|(i, p)| Some((i, p.dev?))).collect();
        for (i, dev) in humans {
            let Some(st) = devs.map.get(&dev).filter(|_| !remove.contains(&i)) else { continue };
            let at = spots.iter().find(|sp| sp.f.key == s.slots[i].cursor.key).map_or(screen / 2., |sp| sp.center);
            let ptr = ptrs.map.entry(dev).or_insert_with(|| Pointer::new(at, i));
            ptr.fan = i;
            // Grab / Shield toggles the options popover.
            if st.pressed[Act::Grab as usize] || st.pressed[Act::Shield as usize] {
                s.toggle_menu(i, ptr);
            }
            let menu = s.slots[i].menu.is_some();
            let mine: Vec<Spot> = spots
                .iter()
                .filter(|sp| if menu { sp.f.owner == Some(i) } else { sp.f.owner.is_none_or(|o| o == i) })
                .copied()
                .collect();
            let mut c = s.slots[i].cursor;
            let before = c.key;
            let acts = pointer::steer(ptr, &mut c, &st.nav, st.stick, &mine, (!menu).then_some(g), cfg, dt, screen);
            s.slots[i].cursor = c;
            if c.key != before
                && let (TILE, t) = kind(c.key)
            {
                s.hover(i, t, d);
            }
            for a in acts {
                start |= s.act(i, a, d, ptr, &mut remove, &mut adds);
            }
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
            if devs.navs().any(|(d, n)| d == *dev && n == input::Nav::Back) {
                leave |= s.humans().count() == 0;
                continue;
            }
            if st.any_press() && s.join(*dev, saved, d) {
                changed = true;
            }
        }
        if changed {
            s.renumber(&remove);
        }
        s.fit_stage(d);
        ptrs.map.retain(|dev, _| s.slots.iter().any(|p| p.dev == Some(*dev)));
        for (i, p) in s.slots.iter().enumerate() {
            if let Some(ptr) = p.dev.and_then(|dev| ptrs.map.get_mut(&dev)) {
                ptr.look = Look {
                    color: pal(d, s.color(i)).0,
                    tag: format!("P{}", i + 1),
                    held: !p.ready,
                    carry: p.carry.map(|(c, _)| (s.tag(c), pal(d, s.color(c)).0)),
                };
            }
        }
        (start && s.ready() && d.stages[s.stage].max_players() >= s.slots.len(), leave && s.humans().count() == 0)
    }
    /// Player `i` acts on what their pointer is on. Returns whether to start.
    fn act(
        &mut self,
        i: usize,
        a: Action,
        d: &sim::GameData,
        ptr: &mut Pointer,
        remove: &mut Vec<usize>,
        adds: &mut usize,
    ) -> bool {
        let cfg = &d.global.render.select.pointer;
        let p = &self.slots[i];
        let (home, ready, carry, k) = (Self::home(p, d), p.ready, p.carry, p.cursor.key);
        if p.menu.is_some() {
            match a {
                Action::Spin(k, by) => self.spin(k, by, d),
                Action::Activate(k) if k != NONE => self.spin(k, 1, d),
                Action::Back | Action::Start => self.toggle_menu(i, ptr),
                Action::Activate(_) => {}
            }
            return false;
        }
        match a {
            Action::Spin(k, by) => self.spin(k, by, d),
            Action::Back if carry.is_some() => self.drop_carry(i, false, d),
            // Pick the chip back up: the pointer goes to get it.
            Action::Back if ready => {
                self.slots[i].ready = false;
                self.slots[i].cursor.key = home;
                ptr.hop();
            }
            Action::Back if kind(k).0 == TILE => remove.push(i),
            Action::Back => {
                self.slots[i].cursor.key = home;
                ptr.hop();
            }
            Action::Start if self.ready() => return true,
            Action::Start if !ready => {
                self.lock(i, d);
                ptr.bump(cfg.drop);
            }
            Action::Start => {}
            Action::Activate(k) => match kind(k) {
                (TILE, t) if carry.is_some() => {
                    self.hover(i, t, d);
                    self.drop_carry(i, true, d);
                    ptr.bump(cfg.drop);
                }
                (TILE, _) if ready && k == home && self.ready() => return true,
                (TILE, _) if ready && k == home => {
                    self.slots[i].ready = false;
                    ptr.bump(cfg.land);
                }
                (TILE, t) => {
                    self.slots[i].ready = false;
                    self.hover(i, t, d);
                    self.lock(i, d);
                    ptr.bump(cfg.drop);
                }
                (FIGHTER, c) if carry.is_some_and(|(x, _)| x == c) => self.drop_carry(i, false, d),
                (FIGHTER, c)
                    if self.slots.get(c).is_some_and(|p| p.dev.is_none() && !p.disconnected)
                        && !self.slots.iter().any(|o| o.carry.is_some_and(|(x, _)| x == c)) =>
                {
                    self.drop_carry(i, false, d);
                    self.slots[i].carry = Some((c, self.slots[c].fighter));
                    ptr.bump(cfg.land);
                }
                (ADD, _) => *adds += 1,
                (REMOVE, si) if self.slots.get(si).is_some_and(|p| p.dev.is_none()) => {
                    remove.push(si);
                    if let Some(j) = self.slots.iter().position(|o| o.carry.is_some_and(|(x, _)| x == si)) {
                        self.drop_carry(j, false, d);
                    }
                }
                (LEVEL | TEAM | BEHAVIOR | STAGE | TEAMS | FF | TAP, _) => self.spin(k, 1, d),
                _ if ready && self.ready() => return true,
                _ => {}
            },
        }
        false
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
            carry: None,
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
    sel.renumber(&[]);
    sel.fit_stage(d);
    commands.insert_resource(sel);
}

pub fn exit(mut portraits: ResMut<Portraits>, mut ptrs: ResMut<Pointers>) {
    portraits.cells.clear();
    ptrs.map.clear();
    ptrs.mouse = None;
}

/// Keeps each player's hint opacity: shown for a while after joining, after idling, while pointing at their own
/// card, and once everyone is ready.
#[allow(clippy::too_many_arguments)]
fn hint_clock(
    s: &Select,
    devs: &Devices,
    ptrs: &Pointers,
    cards: &Query<(&CardArea, &ComputedNode, &UiGlobalTransform)>,
    moved: Option<Dev>,
    (now, dt): (f32, f32),
    mo: &mut Moments,
    d: &sim::GameData,
) {
    let h = &d.global.render.select.hints;
    let dead = d.global.render.select.pointer.deadzone;
    let live = |dev: &Dev| s.slots.iter().any(|p| p.dev == Some(*dev));
    mo.seen.retain(|dev, _| live(dev));
    mo.hints.retain(|dev, _| live(dev));
    for (i, p) in s.humans() {
        let Some(dev) = p.dev else { continue };
        let seen = mo.seen.entry(dev).or_insert((now, now));
        if moved == Some(dev)
            || devs
                .map
                .get(&dev)
                .is_some_and(|st| !st.nav.is_empty() || st.held.iter().any(|h| *h) || st.stick.length() > dead)
        {
            seen.1 = now;
        }
        let on_card = ptrs.map.get(&dev).is_some_and(|ptr| {
            cards.iter().any(|(c, n, t)| {
                let k = n.inverse_scale_factor();
                c.0 == i && ((ptr.pos - t.translation * k).abs() * 2.).cmple(n.size() * k).all()
            })
        });
        let want = now - seen.0 < h.show_secs || now - seen.1 > h.idle_secs || on_card || p.ready && s.ready();
        let a = mo.hints.entry(dev).or_insert(0.);
        *a = (*a + if want { 1. } else { -1. } * dt / h.fade_secs.max(0.01)).clamp(0., 1.);
    }
}

/// A player's card, for showing their hints while they point at it.
#[derive(Component)]
pub struct CardArea(usize);

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    sel: Option<ResMut<Select>>,
    mut devs: ResMut<Devices>,
    (q, cards): (FocusQuery, Query<(&CardArea, &ComputedNode, &UiGlobalTransform)>),
    (mouse, mut ptrs): (Mouse, ResMut<Pointers>),
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
    ptrs.screen = mouse.screen();
    let moved = pointer::mouse(&mut devs, &mut ptrs, &spots, &mouse, |dv| s.slots.iter().any(|p| p.dev == Some(dv)));
    let (start, leave) = s.frame(&devs, &spots, &saved, d, &mut ptrs, time.delta_secs());
    if leave {
        next.set(Screen::Title);
        return;
    }
    let now = time.elapsed_secs();
    hint_clock(&s, &devs, &ptrs, &cards, moved, (now, time.delta_secs()), &mut moments, d);
    if s.slots.iter().map(|p| p.dev).ne(sel.slots.iter().map(|p| p.dev)) {
        devs.clear();
    }
    for (i, p) in s.slots.iter().enumerate() {
        // Locking in, or switching fighters while locked in, drops the chip and plays the flourish.
        if let Some(dev) = p.dev.filter(|_| p.ready)
            && !sel.slots.iter().any(|o| o.dev == Some(dev) && o.ready && o.fighter == p.fighter)
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
            let mine = sel.humans().find(|(_, p)| p.ready && Select::home(p, d) == key(TILE, t));
            Portrait { fighter: t, color: on.or(mine).map(|(i, _)| sel.color(i)), win: mine.is_some() }
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

#[allow(clippy::type_complexity)]
pub fn animate(
    time: Res<Time>,
    moments: Option<Res<Moments>>,
    data: Res<Data>,
    mut q: Query<(&Motion, &mut UiTransform), Without<grid::DropIn>>,
    mut drops: Query<(&grid::DropIn, &mut UiTransform), Without<Motion>>,
    mut fades: Query<(&Fade, Option<&mut TextColor>, Option<&mut BackgroundColor>, Option<&mut BorderColor>)>,
) {
    let Some(mo) = moments else { return };
    let look = &data.0.global.render.select;
    let now = time.elapsed_secs();
    // A dropped chip falls onto its tile from above, big, and settles.
    for (dr, mut tf) in &mut drops {
        let t = mo.locked.iter().find(|(d, _)| *d == dr.0).map_or(1., |(_, at)| (now - at) / look.flip_secs);
        let (s, y) = (1. + (1. - ease(t)) * 0.9, Val::Px((1. - ease(t)) * -26.));
        if tf.scale.x != s || tf.translation.y != y {
            tf.scale = Vec2::splat(s);
            tf.translation.y = y;
        }
    }
    for (fd, text, bg, border) in &mut fades {
        if !fd.2 {
            continue;
        }
        let c = fd.1.with_alpha(fd.1.alpha() * mo.hints.get(&fd.0).copied().unwrap_or(0.));
        if let Some(mut t) = text.filter(|t| fd.3 == 0 && t.0 != c) {
            t.0 = c;
        }
        if let Some(mut b) = bg.filter(|b| fd.3 == 1 && b.0 != c) {
            b.0 = c;
        }
        if let Some(mut b) = border.filter(|b| fd.3 == 2 && b.top != c) {
            *b = BorderColor::all(c);
        }
    }
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
                    BackgroundColor(INK.with_alpha(1.)),
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
                // Just under the roster, above the fighters' flourishes.
                ready_banner(p, f, bottom + 6.);
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
            // Dropped chips: locked-in players', then CPUs' (not while carried).
            let chips = (s.slots.iter().enumerate())
                .filter(|(i, p)| {
                    let carried = s.slots.iter().any(|o| o.carry.is_some_and(|(c, _)| c == *i));
                    Select::home(p, d) == k
                        && (p.ready && p.dev.is_some() || p.dev.is_none() && !p.disconnected)
                        && !carried
                })
                .map(|(i, p)| {
                    let (color, _, light) = pal(d, s.color(i));
                    let cpu = p.dev.is_none();
                    Chip { label: if cpu { s.tag(i) } else { format!("P{}", i + 1) }, color, light, cpu, drop: p.dev }
                })
                .collect();
            let random = t >= d.fighters.len();
            let name = if random { "Random" } else { &d.fighters[t].name };
            let hover = hovered_by(s, k).map(|i| pal(d, s.color(i)).0);
            let tile = grid::Tile { key: k, w: l.w, h: l.h, name, chips, hover, highlight: None, off: None };
            let lead = tile.hover.or(tile.chips.iter().find(|c| !c.cpu).map(|c| c.color));
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
        .with_child(txt(&f.display, 54., NIGHT, "Ready to fight"));
    });
}

/// Keyboard (L / R marks a split half) or gamepad outline for a card header.
fn device_glyph(p: &mut ChildSpawnerCommands, f: &Fonts, dev: Dev, ink: Color) {
    let (pad, half) = match dev {
        Dev::Pad(_) => (true, None),
        Dev::Keys => (false, None),
        Dev::KeysL => (false, Some("L")),
        Dev::KeysR => (false, Some("R")),
        // Scripted bots stand in for gamepads and the keyboard.
        Dev::Bot(n) => (n % 2 == 0, None),
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
        CardArea(i),
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
                // A CPU's fighter: confirm carries its chip to the roster, left / right flip through fighters.
                let k = key(FIGHTER, i);
                let on = hovered_by(s, k);
                let by = s.humans().find(|(_, h)| h.carry.is_some_and(|(c, _)| c == i)).map(|(j, _)| j);
                let tint = on.or(by).map(|j| pal(d, s.color(j)).0);
                p.spawn((
                    node(|n| {
                        n.flex_direction = FlexDirection::Row;
                        n.align_items = AlignItems::Center;
                        n.column_gap = Val::Px(4.);
                        n.padding = UiRect::horizontal(Val::Px(4.));
                        n.margin = UiRect::horizontal(Val::Px(-4.));
                        n.border = UiRect::all(Val::Px(1.5));
                        n.border_radius = BorderRadius::all(Val::Px(5.));
                    }),
                    BackgroundColor(tint.map_or(Color::NONE, |c| alpha(c, 0.16))),
                    BorderColor::all(if by.is_some() { tint.unwrap_or(FAINT) } else { Color::NONE }),
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
                let on = hovered_by(s, k);
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
                .with_child(txt(&f.label, 15., if on.is_some() { STAR } else { FAINT }, "Remove"));
            }
            if let (Some(dev), true) = (pk.dev, human) {
                let caps: Vec<_> =
                    hints(s, i, d).into_iter().map(|(c, what)| (cap(c, dev, saved, devs), what)).collect();
                hint_row(p, f, dev, &caps, ready);
                if ready {
                    p.spawn((
                        node(|n| {
                            n.margin = UiRect::new(Val::Px(-10.), Val::Px(-10.), Val::Px(4.), Val::Px(-6.));
                            n.justify_content = JustifyContent::Center;
                            n.border_radius = BorderRadius::bottom(Val::Px(8.));
                        }),
                        BackgroundColor(main),
                    ))
                    .with_child(txt(&f.display, 28., NIGHT, "READY!"));
                }
            }
        });
        if pk.menu.is_some() {
            options(p, f, d, s, i);
        }
    });
}

/// A control named in a hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ctl {
    Confirm,
    Back,
    Start,
    Options,
    Change,
}

/// What player `i` can do right now, at most two things.
fn hints(s: &Select, i: usize, d: &sim::GameData) -> Vec<(Ctl, &'static str)> {
    let p = &s.slots[i];
    let k = p.cursor.key;
    let kd = kind(k).0;
    if p.menu.is_some() {
        return vec![(Ctl::Change, "change"), (Ctl::Back, "close")];
    }
    if p.carry.is_some() {
        return vec![(Ctl::Confirm, if kd == TILE { "drop here" } else { "pick a fighter" }), (Ctl::Back, "put back")];
    }
    let fight = p.ready && s.ready();
    let first = match kd {
        TILE if p.ready && k == Select::home(p, d) => {
            Some(if fight { (Ctl::Start, "fight!") } else { (Ctl::Confirm, "pick up") })
        }
        TILE if p.ready => Some((Ctl::Confirm, "switch")),
        TILE => Some((Ctl::Confirm, "lock in")),
        ADD => Some((Ctl::Confirm, "add CPU")),
        REMOVE => Some((Ctl::Confirm, "remove")),
        FIGHTER => Some((Ctl::Confirm, "move chip")),
        LEVEL | TEAM | BEHAVIOR | STAGE | TEAMS | FF => Some((Ctl::Confirm, "change")),
        _ if fight => Some((Ctl::Start, "fight!")),
        _ => None,
    };
    let second = if first.is_some_and(|(_, w)| w == "pick up") {
        (Ctl::Options, "options")
    } else if p.ready {
        (Ctl::Back, "unlock")
    } else if kd == TILE {
        (Ctl::Back, "leave")
    } else if first.is_some() {
        (Ctl::Back, "back")
    } else {
        (Ctl::Options, "options")
    };
    first.into_iter().chain([second]).collect()
}

/// How `dev` presses `c` in menus: the label, and whether it is a gamepad button (round) or a key.
pub(crate) fn cap(c: Ctl, dev: Dev, saved: &Saved, devs: &Devices) -> (String, bool) {
    // Scripted bots stand in for gamepads and the keyboard.
    let prof = match dev {
        Dev::Bot(n) if n % 2 == 1 => "keyboard".to_string(),
        _ => devs.profile(dev),
    };
    let pad = !prof.starts_with("keyboard");
    let b = input::binds(saved, &prof);
    let first = |a: Act| {
        let l = b[a as usize].first().map_or(a.label().to_string(), |c| input::control_label(c));
        l.split(" / ").next().unwrap_or_default().to_string()
    };
    // Enter, Esc and Backspace always work on keyboards (Enter and Esc not on the other half).
    let half = |l: Dev| matches!(dev, Dev::KeysL | Dev::KeysR) && dev != l;
    let s = match c {
        Ctl::Confirm | Ctl::Start if !pad && !half(Dev::KeysR) => "Enter".into(),
        Ctl::Start if pad => first(Act::Start),
        Ctl::Confirm | Ctl::Start => first(Act::Attack),
        Ctl::Back if !pad && !half(Dev::KeysL) => "Esc".into(),
        Ctl::Back if !pad => "Backspace".into(),
        Ctl::Back => first(Act::Special),
        Ctl::Options => first(Act::Grab),
        Ctl::Change if pad => "D-pad".into(),
        Ctl::Change if matches!(dev, Dev::KeysL) => format!("{} {}", first(Act::Left), first(Act::Right)),
        Ctl::Change => "← →".into(),
    };
    (s, pad)
}

/// A hint's text (0), fill (1) or border (2) color, faded with its player's hint opacity by [`animate`] (when the
/// flag is set).
#[derive(Component)]
pub struct Fade(Dev, Color, bool, u8);

/// A row of `[control] what` hints for `dev`, faded in and out.
fn hint_row(p: &mut ChildSpawnerCommands, f: &Fonts, dev: Dev, caps: &[((String, bool), &str)], ready: bool) {
    p.spawn(node(|n| {
        n.margin.top = Val::Auto;
        n.flex_direction = FlexDirection::Row;
        n.flex_wrap = FlexWrap::Wrap;
        n.align_items = AlignItems::Center;
        n.column_gap = Val::Px(8.);
        n.row_gap = Val::Px(2.);
    }))
    .with_children(|p| {
        for ((label, pad), what) in caps {
            key_cap(p, f, label, *pad, what, Some(dev), if ready { STAR } else { DIM });
        }
    });
}

/// One `[control] what` hint: a round gamepad button or a keycap. `fade`: faded with that device's hint opacity.
pub(crate) fn key_cap(
    p: &mut ChildSpawnerCommands,
    f: &Fonts,
    label: &str,
    pad: bool,
    what: &str,
    fade: Option<Dev>,
    ink: Color,
) {
    let fade = |c: Color, part: u8| Fade(fade.unwrap_or(Dev::Keys), c, fade.is_some(), part);
    p.spawn(node(|n| {
        n.flex_direction = FlexDirection::Row;
        n.align_items = AlignItems::Center;
        n.column_gap = Val::Px(4.);
    }))
    .with_children(|p| {
        let mut c = p.spawn(node(|n| {
            n.padding = UiRect::axes(Val::Px(if pad { 6. } else { 5. }), Val::Px(0.));
            n.min_width = Val::Px(17.);
            n.justify_content = JustifyContent::Center;
            n.border = UiRect::all(Val::Px(1.));
            n.border_radius = BorderRadius::all(Val::Px(if pad { 9. } else { 4. }));
        }));
        if pad {
            c.insert((BackgroundColor(ink), fade(ink, 1)));
        } else {
            c.insert((BorderColor::all(ink), fade(ink, 2)));
        }
        let text = if pad { NIGHT } else { STAR };
        c.with_child((txt(&f.body_bold, 11., text, label), fade(text, 0)));
        p.spawn((txt(&f.body, 11., ink, what), fade(ink, 0)));
    });
}

/// A player's options popover, above their card. Only their pointer reaches its rows.
fn options(p: &mut ChildSpawnerCommands, f: &Fonts, d: &sim::GameData, s: &Select, i: usize) {
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
        for kd in s.options() {
            let k = key(kd, i);
            let on = pk.cursor.key == k;
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
                Focusable { key: k, owner: Some(i), spin: true },
            ))
            .with_children(|p| {
                p.spawn(txt(&f.label, 17., if on { STAR } else { DIM }, label));
                let v = if on { format!("‹ {value} ›") } else { value };
                p.spawn(txt(&f.label, 18., color.unwrap_or(STAR), v));
            });
        }
    });
}

/// The human whose pointer is on control `k`.
fn hovered_by(s: &Select, k: u32) -> Option<usize> {
    s.humans().find(|(_, p)| p.cursor.key == k).map(|(i, _)| i)
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
    let on = hovered_by(s, k);
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
        p.spawn((txt(&f.label, size, DIM, label), node(|n| n.margin.right = Val::Px(6.))));
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
            n.row_gap = Val::Px(12.);
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
            let on = hovered_by(s, k).map(|i| pal(d, s.color(i)).0);
            p.spawn((
                node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.align_items = AlignItems::Center;
                    n.column_gap = Val::Px(6.);
                    n.padding = UiRect::axes(Val::Px(14.), Val::Px(4.));
                    n.border = UiRect::all(Val::Px(2.));
                    n.border_radius = BorderRadius::all(Val::Px(8.));
                }),
                BackgroundColor(on.map_or(alpha(GOLD, 0.08), |c| alpha(c, 0.22))),
                BorderColor::all(on.unwrap_or(alpha(GOLD, 0.7))),
                BoxShadow::new(alpha(on.unwrap_or(GOLD), 0.35), Val::Px(0.), Val::Px(0.), Val::Px(0.), Val::Px(12.)),
                Focusable { key: k, owner: None, spin: false },
            ))
            .with_child(txt(&f.display, 22., if on.is_some() { STAR } else { GOLD }, "+ Add CPU"));
        }
    });
}

pub fn team_name(t: u8) -> &'static str {
    ["Red team", "Blue team", "Green team", "Yellow team"][t as usize % 4]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Nav;

    struct T {
        s: Select,
        devs: Devices,
        d: sim::GameData,
        saved: Saved,
        ptrs: Pointers,
    }
    impl T {
        fn new() -> Self {
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
            let mut devs = Devices::default();
            devs.map.insert(Dev::Bot(0), default());
            devs.map.insert(Dev::Bot(1), default());
            let ptrs = Pointers { screen: Vec2::new(1280., 720.), ..default() };
            T { s, devs, d: data, saved, ptrs }
        }
        fn navs(&mut self, b: [Vec<Nav>; 2]) -> &mut Self {
            for (i, n) in b.into_iter().enumerate() {
                self.devs.map.get_mut(&Dev::Bot(i as u8)).unwrap().nav = n;
            }
            self
        }
        fn frame(&mut self, spots: &[Spot]) -> (bool, bool) {
            let r = self.s.frame(&self.devs, spots, &self.saved, &self.d, &mut self.ptrs, 1. / 60.);
            self.navs([vec![], vec![]]);
            r
        }
        /// Every roster tile in a row, then the given controls below.
        fn spots(&self, extra: &[(u32, bool)]) -> Vec<Spot> {
            let n = self.d.fighters.len();
            let mut v: Vec<Spot> = (0..=n).map(|t| at(key(TILE, t), t as f32 * 100., 0., false)).collect();
            v.extend(extra.iter().enumerate().map(|(j, (k, spin))| at(*k, j as f32 * 100., 400., *spin)));
            v
        }
    }
    fn at(k: u32, x: f32, y: f32, spin: bool) -> Spot {
        Spot { f: Focusable { key: k, owner: None, spin }, center: Vec2::new(x, y), size: Vec2::splat(60.) }
    }
    fn spot(k: u32, spin: bool) -> Spot {
        at(k, 0., 0., spin)
    }
    #[test]
    fn simultaneous_remove_and_edit_uses_original_lineup() {
        let mut t = T::new();
        t.s.slots[0].cursor.key = key(REMOVE, 2);
        t.s.slots[1].cursor.key = key(LEVEL, 2);
        t.navs([vec![Nav::Confirm], vec![Nav::Right]]);
        t.frame(&[spot(key(REMOVE, 2), false), at(key(LEVEL, 2), 0., 50., true)]);
        assert_eq!(t.s.slots.len(), 2);
        for (i, p) in t.s.slots.iter().enumerate() {
            assert_eq!(p.cursor, Cursor { owner: Some(i), key: NONE }, "the pointer settles on what's nearest");
        }
        t.s.spin(key(LEVEL, 7), 1, &t.d); // stale layout is harmless
    }
    #[test]
    fn cpu_row_focus_survives_lineup_changes() {
        let mut t = T::new();
        t.s.add_cpu(&t.saved, &t.d);
        t.s.slots[0].cursor.key = key(TEAM, 3);
        t.s.slots[1].cursor.key = key(ADD, 0);
        t.navs([vec![], vec![Nav::Confirm]]);
        t.frame(&[spot(key(ADD, 0), false), at(key(TEAM, 3), 0., 50., true)]);
        assert_eq!((t.s.slots.len(), t.s.slots[0].cursor.key), (5, key(TEAM, 3)));
        t.s.slots[1].cursor.key = key(REMOVE, 2);
        t.navs([vec![], vec![Nav::Confirm]]);
        t.frame(&[spot(key(REMOVE, 2), false), at(key(TEAM, 3), 0., 50., true)]);
        assert_eq!((t.s.slots.len(), t.s.slots[0].cursor.key), (4, key(TEAM, 2)));
    }
    #[test]
    fn start_and_back_in_one_frame_does_not_start() {
        let mut t = T::new();
        for p in &mut t.s.slots {
            p.ready = true;
        }
        t.navs([vec![Nav::Start], vec![Nav::Back]]);
        assert!(!t.frame(&[]).0);
        assert!(!t.s.slots[1].ready, "back unlocks");
        assert_eq!(t.s.slots.len(), 3);
    }
    #[test]
    fn simultaneous_leave_and_join_keeps_new_player() {
        let mut t = T::new();
        t.s.slots.truncate(1);
        t.s.slots[0].cursor.key = key(TILE, 0);
        t.navs([vec![Nav::Back], vec![Nav::Confirm]]);
        let (_, leave) = t.frame(&[spot(key(TILE, 0), false)]);
        assert!(!leave);
        assert_eq!(t.s.slots.len(), 1);
        assert_eq!(t.s.slots[0].dev, Some(Dev::Bot(1)));
        assert!(!t.s.slots[0].ready);
        assert!(!t.ptrs.map.contains_key(&Dev::Bot(0)));
    }
    #[test]
    fn disconnect_releases_unready_slot_and_ready_slot_can_be_reclaimed() {
        let mut t = T::new();
        t.s.slots[0].ready = true;
        t.devs.map.clear();
        t.s.frame(&t.devs, &[], &t.saved, &t.d, &mut t.ptrs, 0.016);
        assert_eq!(t.s.slots.len(), 2);
        assert!(t.s.slots[0].disconnected);
        assert_eq!(t.s.slots[0].dev, None);
        assert!(t.ptrs.map.is_empty(), "no pointers for the gone");
        assert!(t.s.join(Dev::Bot(2), &t.saved, &t.d));
        assert_eq!(t.s.slots[0].dev, Some(Dev::Bot(2)));
        assert!(!t.s.slots[0].ready);
    }
    #[test]
    fn another_device_can_remove_a_ready_disconnected_player() {
        let mut t = T::new();
        t.s.slots[0].ready = true;
        t.s.slots[1].cursor.key = key(REMOVE, 0);
        t.devs.map.remove(&Dev::Bot(0));
        t.devs.map.get_mut(&Dev::Bot(1)).unwrap().nav = vec![Nav::Confirm];
        t.s.frame(&t.devs, &[spot(key(REMOVE, 0), false)], &t.saved, &t.d, &mut t.ptrs, 0.016);
        assert_eq!(t.s.slots.len(), 2);
        assert_eq!(t.s.slots[0].dev, Some(Dev::Bot(1)));
        assert!(!t.s.slots.iter().any(|p| p.disconnected));
    }
    #[test]
    fn teams_need_opposition_and_stages_fit_lineup() {
        let T { mut s, mut d, saved, .. } = T::new();
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
        let mut t = T::new();
        let n = t.d.fighters.len();
        let spots = t.spots(&[]);
        t.s.slots[0].cursor.key = key(TILE, 0);
        t.navs([vec![Nav::Right], vec![]]).frame(&spots);
        assert_eq!((t.s.slots[0].fighter, t.s.slots[0].ready), (1, false));
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        assert!(t.s.slots[0].ready);
        t.s.slots[1].cursor.key = key(TILE, n - 1);
        t.navs([vec![], vec![Nav::Right]]).frame(&spots);
        assert!(t.s.slots[1].random);
        t.navs([vec![], vec![Nav::Confirm]]).frame(&spots);
        assert!(t.s.slots[1].ready && !t.s.slots[1].random && t.s.slots[1].fighter < n);
    }
    #[test]
    fn a_dropped_chip_frees_the_pointer_and_can_be_switched_or_picked_up() {
        let mut t = T::new();
        let spots = t.spots(&[(key(ADD, 0), false)]);
        t.s.slots[0].cursor.key = key(TILE, 2);
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        assert!(t.s.slots[0].ready);
        assert!(!t.ptrs.map[&Dev::Bot(0)].look.held, "the chip is on the tile, not the pointer");
        // The free pointer roams without changing the pick, and still adds CPUs.
        t.navs([vec![Nav::Right], vec![]]).frame(&spots);
        assert_eq!((t.s.slots[0].fighter, t.s.slots[0].cursor.key), (2, key(TILE, 3)));
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        assert_eq!((t.s.slots[0].fighter, t.s.slots[0].ready), (3, true), "confirming elsewhere switches");
        t.s.slots[0].cursor.key = key(ADD, 0);
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        assert_eq!((t.s.slots.len(), t.s.slots[0].ready), (4, true));
        // Confirm on your own chip picks it up (P2 isn't ready, so it doesn't start).
        t.s.slots[0].cursor.key = key(TILE, 3);
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        assert!(!t.s.slots[0].ready && t.ptrs.map[&Dev::Bot(0)].look.held);
        // Back while locked picks it up and brings the pointer to it; back again on the roster leaves.
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        t.s.slots[0].cursor.key = key(ADD, 0);
        t.navs([vec![Nav::Back], vec![]]).frame(&spots);
        assert_eq!((t.s.slots[0].ready, t.s.slots[0].cursor.key), (false, key(TILE, 3)));
        t.navs([vec![Nav::Back], vec![]]).frame(&spots);
        assert_eq!(t.s.humans().count(), 1);
    }
    #[test]
    fn everyone_ready_confirm_on_your_chip_starts() {
        let mut t = T::new();
        let spots = t.spots(&[]);
        for i in 0..2 {
            t.s.slots[i].cursor.key = key(TILE, i);
        }
        t.navs([vec![Nav::Confirm], vec![Nav::Confirm]]).frame(&spots);
        assert!(t.s.ready());
        assert!(t.navs([vec![], vec![Nav::Confirm]]).frame(&spots).0);
    }
    #[test]
    fn a_cpu_chip_is_carried_to_a_tile_or_put_back() {
        let mut t = T::new();
        let spots = t.spots(&[(key(FIGHTER, 2), true)]);
        let f = t.s.slots[2].fighter;
        t.s.slots[0].cursor.key = key(FIGHTER, 2);
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        assert_eq!(t.s.slots[0].carry, Some((2, f)));
        assert_eq!(t.ptrs.map[&Dev::Bot(0)].look.carry.as_ref().map(|c| c.0.as_str()), Some("CPU"));
        // Nobody else can grab it meanwhile.
        t.s.slots[1].cursor.key = key(FIGHTER, 2);
        t.navs([vec![], vec![Nav::Confirm]]).frame(&spots);
        assert_eq!(t.s.slots[1].carry, None);
        // Hovering previews the CPU's fighter (not P1's), confirm drops it there.
        let p1 = t.s.slots[0].fighter;
        t.s.slots[0].cursor.key = key(TILE, 4);
        t.navs([vec![Nav::Right], vec![]]).frame(&spots);
        assert_eq!((t.s.slots[2].fighter, t.s.slots[0].fighter), (5, p1));
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        assert_eq!((t.s.slots[2].fighter, t.s.slots[0].carry), (5, None));
        // Back puts a carried chip back where it was.
        t.s.slots[0].cursor.key = key(FIGHTER, 2);
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        t.s.slots[0].cursor.key = key(TILE, 0);
        t.navs([vec![Nav::Right], vec![]]).frame(&spots);
        t.navs([vec![Nav::Back], vec![]]).frame(&spots);
        assert_eq!((t.s.slots[2].fighter, t.s.slots[0].carry, t.s.humans().count()), (5, None, 2));
    }
    #[test]
    fn options_popover_takes_the_pointer_until_closed() {
        let mut t = T::new();
        t.s.teams = true;
        let tap = t.s.slots[0].tap_jump;
        let home = t.s.slots[0].cursor.key;
        let rows = |o: usize| {
            [TEAM, TAP].map(|kd| Spot {
                f: Focusable { key: key(kd, o), owner: Some(o), spin: true },
                center: Vec2::new(0., if kd == TEAM { -100. } else { -70. }),
                size: Vec2::new(150., 20.),
            })
        };
        let mut spots = t.spots(&[]);
        spots.extend(rows(0));
        spots.extend(rows(1));
        t.devs.map.get_mut(&Dev::Bot(0)).unwrap().pressed[Act::Grab as usize] = true;
        t.navs([vec![Nav::Down, Nav::Right], vec![]]).frame(&spots);
        assert_eq!(
            (t.s.slots[0].menu, t.s.slots[0].cursor.key, t.s.slots[0].tap_jump),
            (Some(home), key(TAP, 0), !tap)
        );
        t.devs.map.get_mut(&Dev::Bot(0)).unwrap().pressed = [false; 10];
        // Only its own rows: down wraps rather than leaving for P2's.
        t.navs([vec![Nav::Down], vec![]]).frame(&spots);
        assert_eq!(t.s.slots[0].cursor.key, key(TEAM, 0));
        t.navs([vec![Nav::Back], vec![]]).frame(&spots);
        assert_eq!((t.s.slots.len(), t.s.slots[0].menu, t.s.slots[0].cursor.key), (3, None, home));
    }
    #[test]
    fn cpu_team_row_spins_that_cpu() {
        let mut t = T::new();
        t.s.teams = true;
        t.s.slots[0].cursor.key = key(TEAM, 2);
        let team = t.s.slots[2].team;
        t.navs([vec![Nav::Right], vec![]]).frame(&[spot(key(TEAM, 2), true)]);
        assert_eq!(t.s.slots[2].team, (team + 1) % N_TEAMS);
        assert_eq!(t.s.slots[0].team, 0);
    }
    #[test]
    fn hints_follow_the_context_in_the_devices_controls() {
        let mut t = T::new();
        t.s.slots[0].cursor.key = key(TILE, 0);
        assert_eq!(hints(&t.s, 0, &t.d), [(Ctl::Confirm, "lock in"), (Ctl::Back, "leave")]);
        t.s.slots[0].cursor.key = key(ADD, 0);
        assert_eq!(hints(&t.s, 0, &t.d)[0], (Ctl::Confirm, "add CPU"));
        for p in &mut t.s.slots {
            p.ready = true;
        }
        t.s.slots[0].cursor.key = NONE;
        assert_eq!(hints(&t.s, 0, &t.d), [(Ctl::Start, "fight!"), (Ctl::Back, "unlock")]);
        let saved = Saved::default();
        assert_eq!(cap(Ctl::Confirm, Dev::Keys, &saved, &t.devs), ("Enter".into(), false));
        assert_eq!(cap(Ctl::Back, Dev::KeysR, &saved, &t.devs), ("Backspace".into(), false));
        assert_eq!(cap(Ctl::Confirm, Dev::KeysL, &saved, &t.devs), ("J".into(), false));
        assert_eq!(cap(Ctl::Back, Dev::Bot(0), &saved, &t.devs), ("B".into(), true));
        assert_eq!(cap(Ctl::Start, Dev::Bot(0), &saved, &t.devs), ("Start".into(), true));
    }
}
