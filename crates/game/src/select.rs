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

use std::sync::Arc;

use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use render::portrait::{Portrait, Portraits};
use sim::GameData;
use sim::data::render::CPU_LEVELS;

use crate::grid::{self, Area, Chip};
use crate::hints::{self, Ctl, HintClock};
use crate::input::{Act, Dev, Devices, Nav};
use crate::play::{self, Match, Mode, Rules, Slot};
use crate::pointer::{self, Grid, Look, Pointer, Pointers};
use crate::settings::{self, PlayerPref, Saved};
use crate::showcase;
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
pub(crate) const LAYOUT: u32 = 15;
const TEAM: u32 = 13;
const N_TEAMS: u8 = 4;

/// Room for the roster grid, between the title and the rules panel.
const ROSTER: Area = Area { w: 700., h: 330., aspect: Portraits::ASPECT, extra: 0., gap: 10., max: 116. };
const ROSTER_TOP: f32 = 18.;

pub struct SelectPlugin;

impl Plugin for SelectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Select), enter).add_systems(OnExit(Screen::Select), exit).add_systems(
            Update,
            (
                (input, (hint_clock, moments, portraits, stage).chain().run_if(ui::staying))
                    .chain()
                    .in_set(UiSystems::Input),
                draw.in_set(UiSystems::Draw),
                (animate, hints::fade).in_set(UiSystems::Animate),
            )
                .run_if(in_state(Screen::Select).and_then(resource_exists::<Select>)),
        );
    }
}

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

impl Pick {
    /// A player joining as slot `at`, their pointer on their fighter's tile.
    fn human(dev: Dev, at: usize, fighter: usize, team: u8, tap_jump: bool) -> Self {
        Pick {
            dev: Some(dev),
            disconnected: false,
            fighter,
            random: false,
            level: 1,
            team,
            tap_jump,
            ready: false,
            cursor: Cursor { key: key(TILE, fighter), owner: Some(at) },
            menu: None,
            carry: None,
        }
    }
    /// A CPU (or the training dummy), always ready.
    fn cpu(fighter: usize, level: u8, team: u8) -> Self {
        Pick {
            dev: None,
            disconnected: false,
            fighter,
            random: false,
            level,
            team,
            tap_jump: true,
            ready: true,
            cursor: Cursor::default(),
            menu: None,
            carry: None,
        }
    }
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Select {
    pub mode: Mode,
    pub slots: Vec<Pick>,
    pub stocks: i32,
    pub stage: usize,
    pub teams: bool,
    pub ff: bool,
    pub dummy: crate::settings::DummyMode,
    /// The select screens' random state (the random tile, random stage votes, the roulette): see [`roll`].
    pub roll: u32,
}

/// Next of the select screens' random numbers (a xorshift of `state`).
pub(crate) fn roll(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

/// What a frame of select input leads to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Stay,
    /// On to the next screen.
    Start,
    /// Back to the previous screen.
    Leave,
}

/// Set before entering the select screen to start fresh; `dev` joins right away.
#[derive(Resource)]
pub struct SelectInit {
    pub mode: Mode,
    pub dev: Option<Dev>,
}

/// When things happened, for the lock-in flip, the chip drop, the ready banner and the stage flourish.
#[derive(Resource, Default)]
pub struct Moments {
    /// Device → seconds when it locked in.
    locked: Vec<(Dev, f32)>,
    banner: Option<f32>,
    /// Slots whose fighter should play the flourish on the stage.
    flourish: Vec<usize>,
    /// Locked-in (device, fighter) picks as of the last frame.
    picks: Vec<(Dev, usize)>,
    /// The device whose pointer the mouse moved this frame.
    mouse: Option<Dev>,
}

impl Moments {
    fn new(s: &Select) -> Self {
        Moments { picks: s.picks(), ..default() }
    }
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
    /// Locked-in players' (device, fighter).
    fn picks(&self) -> Vec<(Dev, usize)> {
        self.slots.iter().filter(|p| p.ready).filter_map(|p| Some((p.dev?, p.fighter))).collect()
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
    /// Whether stage `i` has room for this lineup.
    pub(crate) fn fits(&self, d: &GameData, i: usize) -> bool {
        d.stages.get(i).is_some_and(|s| s.max_players() >= self.slots.len())
    }
    /// The stages with room for this lineup.
    pub(crate) fn fitting(&self, d: &GameData) -> Vec<usize> {
        (0..d.stages.len()).filter(|&i| self.fits(d, i)).collect()
    }
    fn fit_stage(&mut self, d: &GameData) {
        if !self.fits(d, self.stage)
            && let Some(&i) = self.fitting(d).first()
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
    fn home(p: &Pick, d: &GameData) -> u32 {
        key(TILE, if p.random { d.fighters.len() } else { p.fighter })
    }
    /// Rows of a player's options popover.
    fn options(&self) -> Vec<u32> {
        let mut v = vec![LAYOUT];
        if self.teams {
            v.push(TEAM);
        }
        v
    }
    fn join(&mut self, dev: Dev, saved: &mut Saved, devs: &Devices, d: &GameData) -> bool {
        let profile = devs.profile(dev);
        settings::migrate_layout(saved, &profile, self.humans().count());
        if let Some(p) = self.slots.iter_mut().find(|p| p.disconnected) {
            p.dev = Some(dev);
            p.disconnected = false;
            p.ready = false;
            p.tap_jump = settings::control_prefs(saved, &profile).tap_jump;
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
        let team = if self.teams { (at % 2) as u8 } else { pref.team };
        let tap_jump = settings::control_prefs(saved, &profile).tap_jump;
        self.slots.insert(at, Pick::human(dev, at, fighter, team, tap_jump));
        self.inserted(at);
        true
    }
    /// A card was inserted at `at`: references to later cards (CPU rows being edited, carried chips) move along.
    fn inserted(&mut self, at: usize) {
        let up = |k: u32| match kind(k) {
            (0 | TILE | STOCKS | STAGE | TEAMS | FF | ADD, _) => k,
            (kd, si) if si >= at => key(kd, si + 1),
            _ => k,
        };
        for (i, p) in self.slots.iter_mut().enumerate() {
            if i != at {
                p.cursor.key = up(p.cursor.key);
                p.menu = p.menu.map(up);
                p.carry = p.carry.map(|(c, f)| (c + usize::from(c >= at), f));
            }
            p.cursor.owner = Some(i);
        }
    }
    fn add_cpu(&mut self, saved: &Saved, d: &GameData) {
        if self.slots.len() >= self.max() || self.mode == Mode::Training {
            return;
        }
        let n = self.slots.iter().filter(|p| p.dev.is_none()).count();
        let pref = saved.versus.cpus.get(n).cloned().unwrap_or_default();
        let i = self.slots.len();
        let fighter = d.fighters.iter().position(|f| f.id == pref.fighter).unwrap_or((i + 1) % d.fighters.len());
        self.slots.push(Pick::cpu(fighter, pref.level.min(CPU_LEVELS.len() as u8 - 1), (i % 2) as u8));
    }
    fn roll_random(&mut self, i: usize, d: &GameData) {
        let p = &mut self.slots[i];
        if p.random {
            p.fighter = roll(&mut self.roll) as usize % d.fighters.len();
            p.random = false;
        }
    }
    /// Drop player `i`'s chip on its fighter (the pointer stays free).
    fn lock(&mut self, i: usize, d: &GameData) {
        self.roll_random(i, d);
        let p = &mut self.slots[i];
        p.ready = true;
        if let Some(k) = p.menu.take() {
            p.cursor.key = k;
        }
    }
    /// Player `i`'s pointer moved onto tile `t`: preview it for the chip in hand (theirs or a carried CPU's).
    fn hover(&mut self, i: usize, t: usize, d: &GameData) {
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
    fn drop_carry(&mut self, i: usize, keep: bool, d: &GameData) {
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
    pub(crate) fn spin(&mut self, k: u32, by: i32, d: &GameData) {
        self.spin_rows(k, by, d, None);
    }
    pub(crate) fn spin_with(&mut self, k: u32, by: i32, d: &GameData, saved: &mut Saved, devs: &Devices) {
        self.spin_rows(k, by, d, Some((saved, devs)));
    }
    fn spin_rows(&mut self, k: u32, by: i32, d: &GameData, ctx: Option<(&mut Saved, &Devices)>) {
        let (kd, s) = kind(k);
        let cyc = |v: i32, n: i32| (v + by).rem_euclid(n);
        let p = self.slots.get_mut(s);
        match kd {
            LAYOUT => {
                if let (Some(dev), Some((saved, devs))) = (p.and_then(|p| p.dev), ctx) {
                    let profile = devs.profile(dev);
                    settings::cycle_layout(saved, &profile, by);
                    if let Some(p) = self.slots.get_mut(s) {
                        p.tap_jump = settings::control_prefs(saved, &profile).tap_jump;
                    }
                }
            }
            FIGHTER => {
                if let Some(p) = p {
                    p.fighter = cyc(p.fighter as i32, d.fighters.len() as i32) as usize;
                }
            }
            LEVEL => {
                if let Some(p) = p {
                    p.level = cyc(i32::from(p.level), CPU_LEVELS.len() as i32) as u8;
                }
            }
            TEAM => {
                if let Some(p) = p {
                    p.team = cyc(i32::from(p.team), i32::from(N_TEAMS)) as u8;
                }
            }
            STOCKS => self.stocks = cyc(self.stocks - 1, 9) + 1,
            STAGE => {
                let stages = self.fitting(d);
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
            BEHAVIOR => self.dummy = self.dummy.cycle(by),
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
    /// One frame of input: moves every human's pointer, acts on their presses, joins new devices. Starts once
    /// everyone is ready, leaves when the last player does.
    fn frame(
        &mut self,
        devs: &Devices,
        spots: &[Spot],
        saved: &mut Saved,
        d: &GameData,
        ptrs: &mut Pointers,
        dt: f32,
    ) -> Step {
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
            let ptr = ptrs.of(dev, i, &s.slots[i].cursor, spots);
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
            let grid = (!menu).then_some(g);
            let (acts, moved) = pointer::drive(ptr, &mut s.slots[i].cursor, st, &mine, grid, cfg, dt, screen);
            if let Some((TILE, t)) = moved.map(kind) {
                s.hover(i, t, d);
            }
            for a in acts {
                start |= s.act(i, a, d, saved, devs, ptr, &mut remove, &mut adds);
            }
        }
        let changed = !remove.is_empty() || adds > 0;
        remove.sort_unstable();
        remove.dedup();
        for &i in &remove {
            s.drop_carry(i, false, d);
        }
        for &i in remove.iter().rev() {
            s.slots.remove(i);
        }
        for _ in 0..adds {
            s.add_cpu(saved, d);
        }
        // Removals first, in old indices; each join then shifts what it displaces.
        if changed {
            s.renumber(&remove);
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
            if st.any_press() {
                s.join(*dev, saved, devs, d);
            }
        }
        s.fit_stage(d);
        ptrs.map.retain(|dev, _| s.slots.iter().any(|p| p.dev == Some(*dev)));
        for (i, p) in s.slots.iter().enumerate() {
            if let Some(ptr) = p.dev.and_then(|dev| ptrs.map.get_mut(&dev)) {
                let carry = p.carry.map(|(c, _)| (s.tag(c), pal(d, s.color(c)).0));
                ptr.look = Look::new(pal(d, s.color(i)).0, i, !p.ready, carry);
            }
        }
        if leave && s.humans().count() == 0 {
            Step::Leave
        } else if start && s.ready() && s.fits(d, s.stage) {
            Step::Start
        } else {
            Step::Stay
        }
    }
    /// Player `i` acts on what their pointer is on. Returns whether to start.
    fn act(
        &mut self,
        i: usize,
        a: Action,
        d: &GameData,
        saved: &mut Saved,
        devs: &Devices,
        ptr: &mut Pointer,
        remove: &mut Vec<usize>,
        adds: &mut usize,
    ) -> bool {
        let cfg = &d.global.render.select.pointer;
        let p = &self.slots[i];
        let (home, ready, carry, k) = (Self::home(p, d), p.ready, p.carry, p.cursor.key);
        if p.menu.is_some() {
            match a {
                Action::Spin(k, by) => self.spin_with(k, by, d, saved, devs),
                Action::Activate(k) if k != NONE => self.spin_with(k, 1, d, saved, devs),
                Action::Back | Action::Start => self.toggle_menu(i, ptr),
                Action::Activate(_) => {}
            }
            return false;
        }
        match a {
            Action::Spin(k, by) => self.spin_with(k, by, d, saved, devs),
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
                (LEVEL | TEAM | BEHAVIOR | STAGE | TEAMS | FF | LAYOUT, _) => self.spin_with(k, 1, d, saved, devs),
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
                    controls: Default::default(),
                    tag: self.tag(i),
                    color: self.color(i),
                }
            })
            .collect()
    }
    /// The match slots with each device's effective layout controls filled in.
    pub(crate) fn match_slots_with(&self, saved: &Saved, devs: &Devices) -> Vec<Slot> {
        let mut slots = self.match_slots();
        for s in &mut slots {
            if let Some(dv) = s.dev {
                let prefs = settings::control_prefs(saved, &devs.profile(dv));
                s.controls = prefs.controls();
                s.tap_jump = prefs.tap_jump;
            }
        }
        slots
    }
    /// The rules for a match on stage `stage`.
    pub(crate) fn rules(&self, d: &GameData, stage: usize) -> Rules {
        Rules { stocks: self.stocks, stage: d.stages[stage].id.clone(), friendly_fire: self.ff }
    }
    /// Remembers everyone's picks and starts the match on [`Self::stage`].
    pub(crate) fn start(
        &self,
        data: &Arc<GameData>,
        saved: &mut Saved,
        commands: &mut Commands,
        scene: &mut render::Scene,
        next: &mut NextState<Screen>,
        devs: &Devices,
    ) {
        remember(self, saved, data);
        let rules = self.rules(data, self.stage);
        let slots = self.match_slots_with(saved, devs);
        let Ok(mut m) = Match::new(data, self.mode, slots, rules, play::seed()) else { return };
        if self.mode == Mode::Training {
            m.dummy.mode = self.dummy;
            m.dummy.level = saved.training.level;
            m.apply_dummy();
        } else {
            m.debug = saved.versus_hitboxes;
        }
        play::install(commands, scene, m);
        next.set(Screen::Match);
    }
}

pub fn enter(
    mut commands: Commands,
    init: Option<Res<SelectInit>>,
    sel: Option<ResMut<Select>>,
    mut saved: ResMut<Saved>,
    data: Res<Data>,
    devs: Res<Devices>,
    back: Option<Res<FromStages>>,
    mut portraits: ResMut<Portraits>,
) {
    let d = &data.0;
    portraits.data = Some(d.clone());
    commands.insert_resource(HintClock::default());
    if let (None, Some(mut sel)) = (&init, sel) {
        if back.is_none() {
            for p in sel.slots.iter_mut().filter(|p| p.dev.is_some()) {
                p.ready = false;
            }
        }
        commands.insert_resource(Moments::new(&sel));
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
        let fighter = d.fighters.iter().position(|f| f.id == t.dummy_fighter).unwrap_or(1 % d.fighters.len());
        sel.slots.push(Pick::cpu(fighter, t.level, 1));
    } else {
        for _ in 0..v.cpus.len().min(sel.max().saturating_sub(1)) {
            sel.add_cpu(&saved, d);
        }
        for (i, p) in sel.slots.iter_mut().enumerate() {
            p.team = if sel.teams { v.cpus.get(i).map_or((i % 2) as u8, |c| c.team) } else { 0 };
        }
    }
    if let Some(dev) = init.dev {
        sel.join(dev, &mut saved, &devs, d);
    }
    sel.renumber(&[]);
    sel.fit_stage(d);
    commands.insert_resource(Moments::new(&sel));
    commands.insert_resource(sel);
}

pub fn exit(mut commands: Commands, mut portraits: ResMut<Portraits>, mut ptrs: ResMut<Pointers>) {
    portraits.cells.clear();
    ptrs.reset();
    commands.remove_resource::<Moments>();
    commands.remove_resource::<HintClock>();
}

/// Moves pointers and acts on everyone's input; on to stage select (versus) or the match (training) once everyone
/// is ready, back to the title when the last player leaves.
fn input(
    mut commands: Commands,
    mut sel: ResMut<Select>,
    mut devs: ResMut<Devices>,
    q: FocusQuery,
    (mouse, mut ptrs, mut moments): (Mouse, ResMut<Pointers>, ResMut<Moments>),
    time: Res<Time>,
    mut saved: ResMut<Saved>,
    data: Res<Data>,
    mut next: ResMut<NextState<Screen>>,
    mut scene: ResMut<render::Scene>,
) {
    let d = &data.0;
    let spots = ui::spots(&q);
    let mut s = sel.clone();
    ptrs.screen = mouse.screen();
    moments.mouse =
        pointer::mouse(&mut devs, &mut ptrs, &spots, &mouse, |dv| s.slots.iter().any(|p| p.dev == Some(dv)));
    let layouts = saved.layout_choices.clone();
    let step = s.frame(&devs, &spots, &mut saved, d, &mut ptrs, time.delta_secs());
    if s.slots.iter().map(|p| p.dev).ne(sel.slots.iter().map(|p| p.dev)) || saved.layout_choices != layouts {
        devs.clear();
    }
    sel.set_if_neq(s);
    match step {
        Step::Leave => next.set(Screen::Title),
        Step::Start if sel.mode == Mode::Versus => next.set(Screen::Stages),
        Step::Start => sel.start(d, &mut saved, &mut commands, &mut scene, &mut next, &devs),
        Step::Stay => {}
    }
}

/// A player's card, for showing their hints while they point at it.
#[derive(Component)]
pub struct CardArea(usize);

/// Shows each player's hints for a while after joining, after idling, while pointing at their own card, and once
/// everyone is ready.
fn hint_clock(
    sel: Res<Select>,
    devs: Res<Devices>,
    ptrs: Res<Pointers>,
    cards: Query<(&CardArea, &ComputedNode, &UiGlobalTransform)>,
    moments: Res<Moments>,
    mut clock: ResMut<HintClock>,
    time: Res<Time>,
    data: Res<Data>,
) {
    let look = &data.0.global.render.select;
    clock.retain(|dev| sel.slots.iter().any(|p| p.dev == Some(*dev)));
    for (i, p) in sel.humans() {
        let Some(dev) = p.dev else { continue };
        let active = moments.mouse == Some(dev)
            || devs.map.get(&dev).is_some_and(|st| {
                !st.nav.is_empty() || st.held.iter().any(|h| *h) || st.stick.length() > look.pointer.deadzone
            });
        let on_card = ptrs.map.get(&dev).is_some_and(|ptr| {
            cards.iter().any(|(c, n, t)| {
                let (at, size) = logical_box(n, t);
                c.0 == i && ((ptr.pos - at).abs() * 2.).cmple(size).all()
            })
        });
        let want = on_card || p.ready && sel.ready();
        clock.step(dev, active, want, (time.elapsed_secs(), time.delta_secs()), &look.hints);
    }
}

/// Lock-ins (or switching fighters while locked in) drop the chip and queue the flourish; the banner shows while
/// everyone is ready.
fn moments(sel: Res<Select>, mut mo: ResMut<Moments>, time: Res<Time>) {
    let now = time.elapsed_secs();
    for (i, p) in sel.slots.iter().enumerate() {
        if let Some(dev) = p.dev.filter(|_| p.ready)
            && !mo.picks.contains(&(dev, p.fighter))
        {
            mo.locked.retain(|(d, _)| *d != dev);
            mo.locked.push((dev, now));
            mo.flourish.push(i);
        }
    }
    mo.picks = sel.picks();
    mo.locked.retain(|(dev, _)| sel.slots.iter().any(|p| p.dev == Some(*dev) && p.ready));
    mo.flourish.retain(|i| sel.slots.get(*i).is_some_and(|p| p.ready && p.dev.is_some()));
    mo.banner = sel.ready().then(|| mo.banner.unwrap_or(now));
}

/// Roster tile figures: lit in the color of a player pointing at them, in the win pose under a locked-in chip.
fn portraits(sel: Res<Select>, data: Res<Data>, mut portraits: ResMut<Portraits>) {
    let d = &data.0;
    portraits.cells = (0..d.fighters.len())
        .map(|t| {
            let on = sel.humans().find(|(_, p)| p.cursor.key == key(TILE, t));
            let mine = sel.humans().find(|(_, p)| p.ready && Select::home(p, d) == key(TILE, t));
            Portrait { fighter: t, color: on.or(mine).map(|(i, _)| sel.color(i)), win: mine.is_some() }
        })
        .collect();
}

/// The lineup on the stage between the roster and the cards; a locked-in fighter plays the flourish, then its win
/// pose.
fn stage(
    mut commands: Commands,
    sel: Res<Select>,
    mut mo: ResMut<Moments>,
    data: Res<Data>,
    mut scene: ResMut<render::Scene>,
    mut m: Option<ResMut<Match>>,
) {
    let d = &data.0;
    if !sel.fits(d, sel.stage) {
        return;
    }
    let rules = sel.rules(d, sel.stage);
    let Some(m) = showcase::showcase(&mut commands, &mut scene, m.as_deref_mut(), d, &sel.match_slots(), &rules) else {
        return;
    };
    showcase::pose(m, |i| sel.slots.get(i).is_some_and(|p| p.ready && p.dev.is_some()), &mut mo.flourish);
}

pub(crate) fn remember(sel: &Select, saved: &mut Saved, d: &GameData) {
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

/// Animated by [`animate`]: a card flipping to READY, the sliding ready banner.
#[derive(Component)]
pub enum Motion {
    Flip(Dev),
    Banner,
}

fn animate(
    time: Res<Time>,
    mo: Res<Moments>,
    data: Res<Data>,
    mut q: Query<(&Motion, &mut UiTransform), Without<grid::DropIn>>,
    mut drops: Query<(&grid::DropIn, &mut UiTransform), Without<Motion>>,
) {
    let look = &data.0.global.render.select;
    let ease = |t: f32| ease_out_back(t, look.overshoot);
    let now = time.elapsed_secs();
    let since_lock =
        |dev: &Dev| mo.locked.iter().find(|(d, _)| d == dev).map_or(1., |(_, at)| (now - at) / look.flip_secs);
    // A dropped chip falls onto its tile from above, big, and settles.
    for (dr, mut tf) in &mut drops {
        let k = 1. - ease(since_lock(&dr.0));
        let (s, y) = (1. + k * look.drop_scale, Val::Px(-k * look.drop_height));
        if tf.scale.x != s || tf.translation.y != y {
            tf.scale = Vec2::splat(s);
            tf.translation.y = y;
        }
    }
    for (m, mut tf) in &mut q {
        match m {
            Motion::Flip(dev) => {
                let sx = ease(since_lock(dev)).max(0.02);
                if tf.scale.x != sx {
                    tf.scale.x = sx;
                }
            }
            Motion::Banner => {
                let t = mo.banner.map_or(1., |at| (now - at) / look.banner_secs);
                let x = Val::Vw((1. - ease(t)) * -look.banner_from);
                if tf.translation.x != x {
                    tf.translation.x = x;
                }
            }
        }
    }
}

fn draw(
    mut commands: Commands,
    sel: Res<Select>,
    roots: Roots<Select>,
    fonts: Res<Fonts>,
    data: Res<Data>,
    devs: Res<Devices>,
    saved: Res<Saved>,
    portraits: Res<Portraits>,
) {
    let Some(mut root) = ui::rebuild(&mut commands, &roots, &*sel, Screen::Select, z::SCREEN) else { return };
    let (d, f, s) = (&data.0, &*fonts, &*sel);
    root.with_children(|p| {
        let title = if s.mode == Mode::Training { "Training" } else { "Versus" };
        title_band(p, f, d, title, |p| {
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
                let mut rules = vec![];
                if s.mode == Mode::Versus {
                    rules.push((TEAMS, "Teams", on_off(s.teams).into()));
                    if s.teams {
                        rules.push((FF, "Friendly fire", on_off(s.ff).into()));
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
            ready_banner(p, f, d, bottom + 6.);
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
            if s.fitting(d).is_empty() {
                p.spawn(txt(&f.heavy, 22., GOLD, "No stage fits this lineup. Remove a player."));
            } else if let Some(reason) = s.start_reason() {
                p.spawn(txt(&f.heavy, 22., GOLD, reason));
            }
            let free = s.slots.len() < s.max() && !(s.mode == Mode::Training && s.humans().count() > 0);
            let n = s.slots.len() + usize::from(free);
            let w = card_width(n, 230.);
            p.spawn(node(|n| {
                n.flex_direction = FlexDirection::Row;
                n.column_gap = Val::Px(CARD_GAP);
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
    });
}

/// The fighter tiles and the random tile, with every player's chip on the tile it points at. Returns the grid's
/// bottom edge.
fn roster(p: &mut ChildSpawnerCommands, f: &Fonts, d: &GameData, s: &Select, portraits: &Portraits) -> f32 {
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
                    grid::random_face(p, f, l.w * 0.62, l.h * 0.1, lead.unwrap_or(GOLD));
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

fn ready_banner(p: &mut ChildSpawnerCommands, f: &Fonts, d: &GameData, top: f32) {
    p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.top = Val::Px(top);
            n.left = Val::Px(0.);
            n.right = Val::Px(0.);
            n.justify_content = JustifyContent::Center;
        }),
        UiTransform::from_translation(Val2::new(Val::Vw(-d.global.render.select.banner_from), Val::Px(0.))),
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
                ColorStop::auto(GOLD.with_alpha(0.)),
                ColorStop::auto(GOLD.with_alpha(0.96)),
                ColorStop::auto(GOLD.with_alpha(0.96)),
                ColorStop::auto(GOLD.with_alpha(0.)),
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

fn card(
    p: &mut ChildSpawnerCommands,
    f: &Fonts,
    d: &GameData,
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
        BorderColor::all(if ready { main } else { main.with_alpha(0.35) }),
        BoxShadow::new(
            main.with_alpha(if ready { 0.5 } else { 0. }),
            Val::Px(0.),
            Val::Px(0.),
            Val::Px(2.),
            Val::Px(18.),
        ),
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
                Some(dv) => device_glyph(p, f, dv, NIGHT.with_alpha(0.85)),
                None if pk.disconnected => {
                    p.spawn(txt(&f.body_bold, 11., NIGHT.with_alpha(0.8), "Disconnected"));
                }
                None if s.mode == Mode::Versus => {
                    p.spawn(txt(&f.body_bold, 12., NIGHT.with_alpha(0.8), level_name(pk.level)));
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
                    BackgroundColor(tint.map_or(Color::NONE, |c| c.with_alpha(0.16))),
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
            if let (Some(dev), true) = (pk.dev, human) {
                let choice = settings::layout_choice(saved, &devs.profile(dev));
                p.spawn(txt(&f.body_bold, 11., GOLD.with_alpha(0.85), format!("Layout: {}", choice.label())));
            }
            if !human && s.mode == Mode::Versus {
                spin_row(p, f, d, s, key(LEVEL, i), "Level", level_name(pk.level).into(), None, 16.);
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
                    BackgroundColor(on.map_or(Color::NONE, |j| pal(d, s.color(j)).0.with_alpha(0.16))),
                    Focusable { key: k, owner: None, spin: false },
                ))
                .with_child(txt(&f.label, 15., if on.is_some() { STAR } else { FAINT }, "Remove"));
            }
            if let (Some(dev), true) = (pk.dev, human) {
                hints::hint_row(p, f, dev, &hints(s, i, d), if ready { STAR } else { DIM }, true, (saved, devs));
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
            options(p, f, d, s, i, saved, devs);
        }
    });
}

/// What player `i` can do right now, at most two things.
fn hints(s: &Select, i: usize, d: &GameData) -> Vec<(Ctl, &'static str)> {
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

/// A player's options popover, above their card. Only their pointer reaches its rows.
fn options(p: &mut ChildSpawnerCommands, f: &Fonts, d: &GameData, s: &Select, i: usize, saved: &Saved, devs: &Devices) {
    let pk = &s.slots[i];
    let (main, _, _) = pal(d, s.color(i));
    p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.bottom = Val::Percent(100.);
            n.left = Val::Px(-2.);
            n.min_width = Val::Px(240.);
            n.margin.bottom = Val::Px(8.);
            n.flex_direction = FlexDirection::Column;
            n.padding = UiRect::all(Val::Px(8.));
            n.row_gap = Val::Px(2.);
            n.border = UiRect::all(Val::Px(2.));
            n.border_radius = BorderRadius::all(Val::Px(10.));
        }),
        BackgroundColor(NIGHT.with_alpha(0.97)),
        BorderColor::all(main),
        BoxShadow::new(NIGHT.with_alpha(0.7), Val::Px(0.), Val::Px(4.), Val::Px(0.), Val::Px(14.)),
        GlobalZIndex(z::POPOVER),
    ))
    .with_children(|p| {
        p.spawn(txt(&f.heavy, 15., main, format!("P{} options", i + 1)));
        for kd in s.options() {
            let k = key(kd, i);
            let on = pk.cursor.key == k;
            let (label, value, color) = match kd {
                LAYOUT => {
                    let dev = pk.dev.expect("a human card");
                    ("Layout", settings::layout_choice(saved, &devs.profile(dev)).label(), None)
                }
                _ => ("Team", team_name(pk.team).to_string(), Some(pal(d, pk.team as usize).0)),
            };
            p.spawn((
                node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.justify_content = JustifyContent::SpaceBetween;
                    n.column_gap = Val::Px(14.);
                    n.padding = UiRect::axes(Val::Px(8.), Val::Px(2.));
                    n.border_radius = BorderRadius::all(Val::Px(5.));
                }),
                BackgroundColor(if on { main.with_alpha(0.2) } else { Color::NONE }),
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

fn spin_row(
    p: &mut ChildSpawnerCommands,
    f: &Fonts,
    d: &GameData,
    s: &Select,
    k: u32,
    label: &str,
    value: String,
    color: Option<Color>,
    size: f32,
) {
    let on = hovered_by(s, k);
    let tint = on.map_or(Color::NONE, |i| pal(d, s.color(i)).0.with_alpha(0.16));
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

fn join_card(p: &mut ChildSpawnerCommands, f: &Fonts, d: &GameData, s: &Select, w: f32) {
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
        BorderColor::all(FAINT.with_alpha(0.6)),
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
                BackgroundColor(on.map_or(GOLD.with_alpha(0.08), |c| c.with_alpha(0.22))),
                BorderColor::all(on.unwrap_or(GOLD.with_alpha(0.7))),
                BoxShadow::new(
                    on.unwrap_or(GOLD).with_alpha(0.35),
                    Val::Px(0.),
                    Val::Px(0.),
                    Val::Px(0.),
                    Val::Px(12.),
                ),
                Focusable { key: k, owner: None, spin: false },
            ))
            .with_child(txt(&f.display, 22., if on.is_some() { STAR } else { GOLD }, "+ Add CPU"));
        }
    });
}

pub fn team_name(t: u8) -> &'static str {
    ["Red team", "Blue team", "Green team", "Yellow team"][t as usize % 4]
}

/// The name of a saved CPU level, clamped to [`CPU_LEVELS`] in case settings were edited by hand.
fn level_name(level: u8) -> &'static str {
    CPU_LEVELS[(level as usize).min(CPU_LEVELS.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ControlPrefs, DummyMode, Family, Layout};

    struct T {
        s: Select,
        devs: Devices,
        d: GameData,
        saved: Saved,
        ptrs: Pointers,
    }
    impl T {
        fn new() -> Self {
            let data = (*crate::test_data()).clone();
            let mut saved = Saved::default();
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
            let mut devs = Devices::default();
            devs.map.insert(Dev::Bot(0), default());
            devs.map.insert(Dev::Bot(1), default());
            s.join(Dev::Bot(0), &mut saved, &devs, &data);
            s.join(Dev::Bot(1), &mut saved, &devs, &data);
            s.add_cpu(&saved, &data);
            let ptrs = Pointers { screen: Vec2::new(1280., 720.), ..default() };
            T { s, devs, d: data, saved, ptrs }
        }
        fn navs(&mut self, b: [Vec<Nav>; 2]) -> &mut Self {
            for (i, n) in b.into_iter().enumerate() {
                self.devs.map.get_mut(&Dev::Bot(i as u8)).unwrap().nav = n;
            }
            self
        }
        fn navs_at(&mut self, b: [(Dev, Vec<Nav>); 2]) -> &mut Self {
            for (d, n) in b {
                self.devs.map.get_mut(&d).unwrap().nav = n;
            }
            self
        }
        fn frame(&mut self, spots: &[Spot]) -> Step {
            let r = self.s.frame(&self.devs, spots, &mut self.saved, &self.d, &mut self.ptrs, 1. / 60.);
            for st in self.devs.map.values_mut() {
                st.nav.clear();
            }
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
        assert_ne!(t.frame(&[]), Step::Start);
        assert!(!t.s.slots[1].ready, "back unlocks");
        assert_eq!(t.s.slots.len(), 3);
    }
    #[test]
    fn simultaneous_leave_and_join_keeps_new_player() {
        let mut t = T::new();
        t.s.slots.truncate(1);
        t.s.slots[0].cursor.key = key(TILE, 0);
        t.navs([vec![Nav::Back], vec![Nav::Confirm]]);
        assert_ne!(t.frame(&[spot(key(TILE, 0), false)]), Step::Leave);
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
        t.s.frame(&t.devs, &[], &mut t.saved, &t.d, &mut t.ptrs, 0.016);
        assert_eq!(t.s.slots.len(), 2);
        assert!(t.s.slots[0].disconnected);
        assert_eq!(t.s.slots[0].dev, None);
        assert!(t.ptrs.map.is_empty(), "no pointers for the gone");
        assert!(t.s.join(Dev::Bot(2), &mut t.saved, &t.devs, &t.d));
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
        t.s.frame(&t.devs, &[spot(key(REMOVE, 0), false)], &mut t.saved, &t.d, &mut t.ptrs, 0.016);
        assert_eq!(t.s.slots.len(), 2);
        assert_eq!(t.s.slots[0].dev, Some(Dev::Bot(1)));
        assert!(!t.s.slots.iter().any(|p| p.disconnected));
    }
    #[test]
    fn hand_edited_cpu_levels_are_clamped() {
        let T { mut s, mut saved, d, .. } = T::new();
        saved.versus.cpus = vec![PlayerPref { level: 3, ..default() }, PlayerPref { level: 250, ..default() }];
        s.slots.retain(|p| p.dev.is_some());
        s.add_cpu(&saved, &d);
        s.add_cpu(&saved, &d);
        let levels: Vec<u8> = s.slots.iter().filter(|p| p.dev.is_none()).map(|p| p.level).collect();
        assert_eq!(levels, vec![CPU_LEVELS.len() as u8 - 1; 2]);
        assert_eq!(level_name(250), CPU_LEVELS[CPU_LEVELS.len() - 1]);
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
        assert_eq!(t.ptrs.map[&Dev::Bot(0)].look.chip, None, "the chip is on the tile, not the pointer");
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
        assert!(!t.s.slots[0].ready && t.ptrs.map[&Dev::Bot(0)].look.own);
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
        assert_eq!(t.navs([vec![], vec![Nav::Confirm]]).frame(&spots), Step::Start);
    }
    #[test]
    fn a_cpu_chip_is_carried_to_a_tile_or_put_back() {
        let mut t = T::new();
        let spots = t.spots(&[(key(FIGHTER, 2), true)]);
        let f = t.s.slots[2].fighter;
        t.s.slots[0].cursor.key = key(FIGHTER, 2);
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        assert_eq!(t.s.slots[0].carry, Some((2, f)));
        let look = &t.ptrs.map[&Dev::Bot(0)].look;
        assert_eq!((look.chip.as_ref().map(|c| c.0.as_str()), look.own), (Some("CPU"), false), "it rides the arrow");
        // Nobody else can grab it meanwhile.
        t.s.slots[1].cursor.key = key(FIGHTER, 2);
        t.navs([vec![], vec![Nav::Confirm]]).frame(&spots);
        assert_eq!(t.s.slots[1].carry, None);
        // Hovering previews the CPU's fighter (not P1's), confirm drops it there.
        let p1 = t.s.slots[0].fighter;
        t.s.slots[0].cursor.key = key(TILE, 2);
        t.navs([vec![Nav::Right], vec![]]).frame(&spots);
        assert_eq!((t.s.slots[2].fighter, t.s.slots[0].fighter), (3, p1));
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        assert_eq!((t.s.slots[2].fighter, t.s.slots[0].carry), (3, None));
        // Back puts a carried chip back where it was.
        t.s.slots[0].cursor.key = key(FIGHTER, 2);
        t.navs([vec![Nav::Confirm], vec![]]).frame(&spots);
        t.s.slots[0].cursor.key = key(TILE, 0);
        t.navs([vec![Nav::Right], vec![]]).frame(&spots);
        t.navs([vec![Nav::Back], vec![]]).frame(&spots);
        assert_eq!((t.s.slots[2].fighter, t.s.slots[0].carry, t.s.humans().count()), (3, None, 2));
    }
    #[test]
    fn a_carried_cpu_chip_follows_its_card_when_someone_joins_or_returns_when_the_carrier_leaves() {
        let mut t = T::new();
        let spots = t.spots(&[(key(FIGHTER, 2), true)]);
        let f = t.s.slots[2].fighter;
        t.s.slots[1].cursor.key = key(FIGHTER, 2);
        t.navs([vec![], vec![Nav::Confirm]]).frame(&spots);
        assert_eq!(t.s.slots[1].carry, Some((2, f)));
        // A new human takes the CPU's place in line; the chip stays the CPU's.
        assert!(t.s.join(Dev::Bot(2), &mut t.saved, &t.devs, &t.d));
        assert_eq!((t.s.slots[2].dev, t.s.slots[1].carry), (Some(Dev::Bot(2)), Some((3, f))));
        let p3 = t.s.slots[2].fighter;
        t.s.hover(1, (f + 1) % t.d.fighters.len(), &t.d);
        assert_eq!(t.s.slots[2].fighter, p3);
        assert_ne!(t.s.slots[3].fighter, f);
        // The unlocked carrier unplugs: the CPU gets its own fighter back.
        t.devs.map.remove(&Dev::Bot(1));
        t.devs.map.insert(Dev::Bot(2), default());
        t.s.frame(&t.devs, &spots, &mut t.saved, &t.d, &mut t.ptrs, 1. / 60.);
        assert_eq!((t.s.slots.len(), t.s.slots[2].dev, t.s.slots[2].fighter), (3, None, f));
    }
    #[test]
    fn options_popover_takes_the_pointer_until_closed() {
        let mut t = T::new();
        t.s.teams = true;
        let tap = t.s.slots[0].tap_jump;
        let team = t.s.slots[0].team;
        let home = t.s.slots[0].cursor.key;
        let rows = |o: usize| {
            [LAYOUT, TEAM].map(|kd| Spot {
                f: Focusable { key: key(kd, o), owner: Some(o), spin: true },
                center: Vec2::new(0., if kd == LAYOUT { -130. } else { -100. }),
                size: Vec2::new(150., 20.),
            })
        };
        let mut spots = t.spots(&[]);
        spots.extend(rows(0));
        spots.extend(rows(1));
        t.devs.map.get_mut(&Dev::Bot(0)).unwrap().pressed[Act::Grab as usize] = true;
        t.navs([vec![Nav::Down, Nav::Right], vec![]]).frame(&spots);
        assert_eq!(
            (t.s.slots[0].menu, t.s.slots[0].cursor.key, t.s.slots[0].team, t.s.slots[0].tap_jump),
            (Some(home), key(TEAM, 0), (team + 1) % N_TEAMS, tap)
        );
        t.devs.map.get_mut(&Dev::Bot(0)).unwrap().pressed = [false; 10];
        // Only its own rows: down from the last wraps to the first, and the layout row cycles the device's profile.
        t.navs([vec![Nav::Down], vec![]]).frame(&spots);
        assert_eq!(t.s.slots[0].cursor.key, key(LAYOUT, 0), "down past the last row wraps");
        t.navs([vec![Nav::Right], vec![]]).frame(&spots);
        assert_eq!(t.saved.layout_choices.get("bot").map(String::as_str), Some("tilt"));
        assert!(t.s.slots[0].tap_jump, "Tiltstick's up-to-jump option follows the profile");
        t.navs([vec![Nav::Down], vec![]]).frame(&spots);
        assert_eq!(t.s.slots[0].cursor.key, key(TEAM, 0));
        t.navs([vec![Nav::Back], vec![]]).frame(&spots);
        assert_eq!((t.s.slots.len(), t.s.slots[0].menu, t.s.slots[0].cursor.key), (3, None, home));
    }
    #[test]
    fn legacy_join_order_options_become_independent_device_layouts() {
        let mut t = T::new();
        t.s.slots.clear();
        t.saved = serde_json::from_str(r#"{"versus":{"humans":[{"tapJump":false},{"tapJump":true}]}}"#).unwrap();
        t.s.join(Dev::KeysL, &mut t.saved, &t.devs, &t.d);
        t.s.join(Dev::KeysR, &mut t.saved, &t.devs, &t.d);
        assert!(!t.s.slots[0].tap_jump);
        assert!(t.s.slots[1].tap_jump);
        assert!(!settings::control_prefs(&t.saved, "keyboard-left").tap_jump);
        assert!(settings::control_prefs(&t.saved, "keyboard-right").tap_jump);
        t.s.slots.clear();
        t.s.join(Dev::KeysR, &mut t.saved, &t.devs, &t.d);
        t.s.join(Dev::KeysL, &mut t.saved, &t.devs, &t.d);
        assert!(t.s.slots[0].tap_jump);
        assert!(!t.s.slots[1].tap_jump);
    }

    #[test]
    fn layouts_are_chosen_per_device_and_can_be_shared() {
        let data = (*crate::test_data()).clone();
        let mut saved = Saved { split_keyboard: true, ..default() };
        saved.layouts.insert(
            "Split A".into(),
            Layout {
                family: Family::Keyboard,
                bindings: [("attack".into(), vec!["KeyU".into()])].into(),
                options: ControlPrefs { stick_tilts: true, stick_range: 0.7, ..default() },
            },
        );
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
        let mut devs = Devices::default();
        devs.map.insert(Dev::Keys, default());
        devs.map.insert(Dev::KeysL, default());
        s.join(Dev::Keys, &mut saved, &devs, &data);
        s.join(Dev::KeysL, &mut saved, &devs, &data);
        let mut t = T { s, devs, d: data, saved, ptrs: Pointers { screen: Vec2::new(1280., 720.), ..default() } };
        let rows = |o: usize| Spot {
            f: Focusable { key: key(LAYOUT, o), owner: Some(o), spin: true },
            center: Vec2::new(o as f32 * 100., 400.),
            size: Vec2::new(150., 20.),
        };
        let spots = [rows(0), rows(1)];
        for p in &mut t.s.slots {
            p.menu = Some(key(TILE, p.fighter));
            p.cursor.key = key(LAYOUT, p.cursor.owner.unwrap());
        }
        // Both pick the shared named layout independently (Smashstick, Tiltstick, Custom, then the named one).
        for _ in 0..3 {
            t.navs_at([(Dev::Keys, vec![Nav::Right]), (Dev::KeysL, vec![Nav::Right])]).frame(&spots);
        }
        assert_eq!(t.saved.layout_choices["keyboard"], "Split A");
        assert_eq!(t.saved.layout_choices["keyboard-left"], "Split A");
        let slots = t.s.match_slots_with(&t.saved, &t.devs);
        assert!(slots[0].controls.stick_tilts && slots[1].controls.stick_tilts);
        assert_eq!(crate::input::binds(&t.saved, "keyboard")[5], ["KeyU"], "the shared layout's binding is live");
        // One device can move on without disturbing the other, and its choice survives fighter changes.
        t.navs_at([(Dev::Keys, vec![Nav::Right]), (Dev::KeysL, vec![])]).frame(&spots);
        assert_eq!(t.saved.layout_choices["keyboard"], "smash");
        assert_eq!(t.saved.layout_choices["keyboard-left"], "Split A");
        t.navs_at([(Dev::Keys, vec![Nav::Back]), (Dev::KeysL, vec![])]).frame(&spots);
        t.s.slots[0].cursor.key = key(TILE, 3);
        t.navs_at([(Dev::Keys, vec![Nav::Confirm]), (Dev::KeysL, vec![])]);
        t.frame(&t.spots(&[]));
        assert!(t.s.slots[0].ready);
        assert_eq!(t.saved.layout_choices["keyboard"], "smash");
    }
    #[test]
    fn match_slots_use_each_devices_effective_layout_controls() {
        let mut t = T::new();
        t.saved.controls.insert("bot".into(), ControlPrefs { stick_range: 0.8, ..default() });
        t.saved.layout_choices.insert("bot".into(), settings::TILT.into());
        let slots = t.s.match_slots_with(&t.saved, &t.devs);
        assert_eq!(slots.len(), 3);
        assert!(slots[0].controls.stick_tilts && slots[1].controls.stick_tilts, "both bots share the preset");
        assert!(slots[2].controls == Default::default(), "the CPU has no device layout");
        t.saved.layout_choices.insert("bot".into(), settings::SMASH.into());
        assert!(!t.s.match_slots_with(&t.saved, &t.devs)[0].controls.stick_tilts);
    }
    #[test]
    fn selected_up_to_jump_reaches_the_match_config() {
        let mut t = T::new();
        t.saved.layouts.insert(
            "No up jump".into(),
            Layout {
                family: Family::Gamepad,
                bindings: default(),
                options: ControlPrefs { tap_jump: false, ..default() },
            },
        );
        t.saved.layout_choices.insert("bot".into(), "No up jump".into());
        let slots = t.s.match_slots_with(&t.saved, &t.devs);
        assert!(!slots[0].tap_jump && !slots[1].tap_jump);
        assert!(slots[2].tap_jump, "the CPU keeps its own preference");
        let rules = Rules { stocks: 1, stage: "arena".into(), friendly_fire: false };
        let data = std::sync::Arc::new(t.d.clone());
        let m = Match::new(&data, Mode::Versus, slots, rules, 7).unwrap();
        assert!(!m.world.cfg.players[0].tap_jump);
        assert!(!m.world.cfg.players[1].tap_jump);
        assert_eq!(m.world.cfg.players[0].controls, t.saved.layouts["No up jump"].options.controls());
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
    fn hints_follow_the_context() {
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
    }
}
