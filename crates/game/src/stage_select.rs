//! Stage select (versus), after everyone locks in on character select and presses Start. Every human moves a chip
//! over the stage grid (live thumbnails; the big preview follows the last chip moved): attack votes, special takes
//! the vote back, or with no vote returns to character select with picks kept. Once every human has voted, or anyone
//! presses Start after a vote, a roulette hops over the voted stages (duplicate votes weigh more), slows down and
//! lands on the winner (`render.select.roulette`); after a pause the match starts. Stocks are set here.

use bevy::prelude::*;
use render::thumb::StageThumbs;
use sim::GameData;

use crate::grid::{self, Area, Chip, key, kind};
use crate::input::{Dev, Devices, Nav};
use crate::play::{self, Match, Mode, Rules};
use crate::select::{self, FromStages, STOCKS, Select};
use crate::settings::Saved;
use crate::ui::*;
use crate::{Data, Screen};

const TILE: u32 = 1;
const GRID: Area = Area { w: 740., h: 500., aspect: StageThumbs::ASPECT, extra: NAME, gap: 10., max: 236. };
/// Name band under a stage tile's thumbnail.
const NAME: f32 = 20.;
const GRID_TOP: f32 = 128.;
const PANEL_W: f32 = 404.;

#[derive(Clone, Debug, PartialEq)]
pub struct Voter {
    /// Character select slot.
    pub slot: usize,
    pub dev: Dev,
    pub cursor: Cursor,
    /// Voted tile.
    pub vote: Option<usize>,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Vote {
    pub voters: Vec<Voter>,
    /// Tile in the big preview.
    pub preview: usize,
    pub roll: u32,
    /// Roulette: the stage under the highlight at each hop, the last one wins. Empty until it starts.
    pub hops: Vec<usize>,
    pub hop: usize,
    pub landed: bool,
}

/// Seconds since the roulette started.
#[derive(Resource, Default)]
pub struct Clock(Option<f32>);

impl Clock {
    pub fn secs(&self) -> Option<f32> {
        self.0
    }
}

fn next(roll: &mut u32) -> u32 {
    *roll ^= *roll << 13;
    *roll ^= *roll >> 17;
    *roll ^= *roll << 5;
    *roll
}

/// Stages that fit `players`.
fn fitting(d: &GameData, players: usize) -> Vec<usize> {
    (0..d.stages.len()).filter(|&i| d.stages[i].max_players() >= players).collect()
}

/// Each vote's stage: a vote past the last stage (the random tile) draws one of `fit`.
pub fn resolve(votes: &[usize], stages: usize, fit: &[usize], roll: &mut u32) -> Vec<usize> {
    votes.iter().map(|&t| if t < stages { t } else { fit[next(roll) as usize % fit.len()] }).collect()
}

/// The roulette: a winner drawn from `pool` (each vote one ticket), and `hops + 1` stops cycling through the distinct
/// voted stages in order, ending on it. A single distinct stage just lands.
pub fn roulette(pool: &[usize], hops: u32, roll: &mut u32) -> Vec<usize> {
    let winner = pool[next(roll) as usize % pool.len()];
    let mut distinct = pool.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() == 1 {
        return vec![winner];
    }
    let w = distinct.iter().position(|s| *s == winner).unwrap_or(0) as i64;
    let len = distinct.len() as i64;
    (0..=i64::from(hops)).map(|k| distinct[(w + k - i64::from(hops)).rem_euclid(len) as usize]).collect()
}

/// The hop shown `t` seconds in, of `n` hops (hop `k` starts at `secs * (k / n)^curve`).
pub fn hop_at(t: f32, n: usize, r: &sim::data::render::Roulette) -> usize {
    if n == 0 {
        return 0;
    }
    let x = (t / r.secs).clamp(0., 1.).powf(1. / r.curve.max(0.01));
    ((x * n as f32).floor() as usize).min(n)
}

impl Vote {
    pub fn new(sel: &Select) -> Self {
        let voters = sel
            .humans()
            .enumerate()
            .filter_map(|(v, (slot, p))| {
                Some(Voter {
                    slot,
                    dev: p.dev?,
                    cursor: Cursor { key: key(TILE, sel.stage), owner: Some(v) },
                    vote: None,
                })
            })
            .collect();
        Vote { voters, preview: sel.stage, roll: sel.roll | 1, hops: vec![], hop: 0, landed: false }
    }
    /// One frame of input. Returns (roulette started, back to character select).
    fn frame(&mut self, sel: &mut Select, devs: &Devices, spots: &[Spot], d: &GameData) -> (bool, bool) {
        let n = d.stages.len() + 1;
        let cols = GRID.layout(n).cols;
        let fit = fitting(d, sel.slots.len());
        let votable = |t: usize| fit.contains(&t) || t == d.stages.len() && !fit.is_empty();
        // A dropped controller sends everyone back to character select, which holds its slot for reconnecting.
        if self.voters.iter().any(|v| !devs.map.contains_key(&v.dev)) {
            return (false, true);
        }
        if !self.hops.is_empty() {
            return (false, false);
        }
        let (mut go, mut leave) = (false, false);
        for (dev, nav) in devs.navs() {
            let Some(v) = self.voters.iter().position(|v| v.dev == dev) else { continue };
            if self.voters[v].vote.is_some() {
                match nav {
                    Nav::Back => self.voters[v].vote = None,
                    Nav::Start => go = true,
                    _ => {}
                }
                continue;
            }
            if nav == Nav::Start {
                go = true;
                continue;
            }
            let mut c = self.voters[v].cursor;
            let before = c.key;
            match grid::drive(&mut c, nav, spots, TILE, n, cols) {
                Some(Action::Activate(k)) if kind(k).0 == TILE && votable(kind(k).1) => {
                    self.voters[v].vote = Some(kind(k).1);
                    self.preview = kind(k).1;
                }
                Some(Action::Spin(k, by)) => sel.spin(k, by, d),
                Some(Action::Back) => leave = true,
                _ => {}
            }
            if c.key != before
                && let (TILE, t) = kind(c.key)
            {
                self.preview = t;
            }
            self.voters[v].cursor = c;
        }
        let votes: Vec<usize> = self.voters.iter().filter_map(|v| v.vote).collect();
        if leave || votes.is_empty() || !go && votes.len() < self.voters.len() {
            return (false, leave);
        }
        let pool = resolve(&votes, d.stages.len(), &fit, &mut self.roll);
        self.hops = roulette(&pool, d.global.render.select.roulette.hops, &mut self.roll);
        self.preview = self.hops[0];
        (true, false)
    }
    fn winner(&self) -> Option<usize> {
        self.hops.last().copied()
    }
}

pub fn enter(mut commands: Commands, sel: Option<Res<Select>>) {
    if let Some(sel) = sel {
        commands.insert_resource(Vote::new(&sel));
    }
    commands.insert_resource(Clock::default());
}

pub fn exit(mut commands: Commands, mut thumbs: ResMut<StageThumbs>) {
    thumbs.cells.clear();
    commands.remove_resource::<Vote>();
}

/// A left click acts as the keyboard voter's confirm on the control under the pointer.
fn click(v: &mut Vote, devs: &mut Devices, q: &FocusQuery, mouse: &Mouse) {
    if !mouse.clicked() || !v.hops.is_empty() {
        return;
    }
    let hit = mouse.hit(q);
    let Some(dev) = keyboard(devs, |d| v.voters.iter().any(|x| x.dev == d)) else { return };
    let Some(voter) = v.voters.iter_mut().find(|x| x.dev == dev && x.vote.is_none()) else { return };
    let Some(f) = hit.filter(|f| voter.cursor.allowed(f)) else { return };
    voter.cursor.key = f.key;
    if let (TILE, t) = kind(f.key) {
        v.preview = t;
    }
    if let Some(st) = devs.map.get_mut(&dev) {
        st.nav.push(Nav::Confirm);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    (vote, sel): (Option<ResMut<Vote>>, Option<ResMut<Select>>),
    mut devs: ResMut<Devices>,
    q: FocusQuery,
    mouse: Mouse,
    (time, mut clock): (Res<Time>, ResMut<Clock>),
    mut saved: ResMut<Saved>,
    data: Res<Data>,
    mut next: ResMut<NextState<Screen>>,
    mut scene: ResMut<render::Scene>,
    (mut m, mut thumbs): (Option<ResMut<Match>>, ResMut<StageThumbs>),
) {
    let (Some(mut vote), Some(mut sel)) = (vote, sel) else { return };
    let d = &data.0;
    let spots = spots(&q);
    let (mut v, mut s) = (vote.clone(), sel.clone());
    click(&mut v, &mut devs, &q, &mouse);
    let (spin, leave) = v.frame(&mut s, &devs, &spots, d);
    // Successive votes keep drawing from the same sequence.
    s.roll = v.roll;
    if leave {
        *sel = s;
        clock.0 = None;
        commands.insert_resource(FromStages);
        next.set(Screen::Select);
        return;
    }
    if spin {
        clock.0 = Some(0.);
    }
    let r = &d.global.render.select.roulette;
    if let Some(t) = clock.0 {
        // Frame hitches don't skip hops.
        let t = t + time.delta_secs().min(0.05);
        clock.0 = Some(t);
        let n = v.hops.len() - 1;
        v.hop = hop_at(t, n, r);
        v.preview = v.hops[v.hop];
        let land = if n == 0 { 0. } else { r.secs };
        v.landed = t >= land;
        if t >= land + r.pause
            && let Some(w) = v.winner()
        {
            s.stage = w;
            *sel = s;
            select::remember(&sel, &mut saved, d);
            let rules = Rules { stocks: sel.stocks, stage: d.stages[w].id.clone(), friendly_fire: sel.ff };
            if let Ok(mut m) = Match::new(&data.0, Mode::Versus, sel.match_slots(), rules, play::seed()) {
                m.debug = saved.versus_hitboxes;
                play::install(&mut commands, &mut scene, m);
                next.set(Screen::Match);
            }
            clock.0 = None;
            return;
        }
    }
    if v != *vote {
        *vote = v;
    }
    if s != *sel {
        *sel = s;
    }
    if !thumbs.data.as_ref().is_some_and(|p| std::sync::Arc::ptr_eq(p, &data.0)) {
        thumbs.data = Some(data.0.clone());
    }
    let cells: Vec<usize> = (0..d.stages.len()).collect();
    if thumbs.cells != cells {
        thumbs.cells = cells;
    }
    thumbs.preview = (vote.preview < d.stages.len()).then_some(vote.preview);
    thumbs.colors = (0..sel.slots.len()).map(|i| sel.color(i)).collect();
    // The lineup stands on the previewed stage behind the screen, everyone in their win pose (it stays put while the
    // roulette runs).
    if !vote.hops.is_empty() {
        return;
    }
    let stage = if fitting(d, sel.slots.len()).contains(&vote.preview) { vote.preview } else { sel.stage };
    let rules = Rules { stocks: sel.stocks, stage: d.stages[stage].id.clone(), friendly_fire: sel.ff };
    if let Some(m) = select::showcase(&mut commands, &mut scene, m.as_deref_mut(), &data.0, &sel.match_slots(), &rules)
    {
        for f in &mut m.world.fighters {
            if f.state == sim::world::State::Idle {
                f.set_state(sim::world::State::Win);
            }
        }
    }
}

#[derive(Component)]
pub struct VoteRoot(Vote, i32);

/// The landed tile's bounce.
#[derive(Component)]
pub struct Landed;

pub fn animate(
    clock: Res<Clock>,
    data: Res<Data>,
    vote: Option<Res<Vote>>,
    mut q: Query<&mut UiTransform, With<Landed>>,
) {
    let (Some(t), Some(v)) = (clock.0, vote) else { return };
    let r = &data.0.global.render.select.roulette;
    let land = if v.hops.len() > 1 { r.secs } else { 0. };
    let k = ((t - land) / r.pause.max(0.01)).clamp(0., 1.);
    for mut tf in &mut q {
        let s = r.land_scale + r.land_bounce * (1. - k) * (k * r.land_wobble).sin().abs();
        if tf.scale.x != s {
            tf.scale = Vec2::splat(s);
        }
    }
}

/// "Small · 3 platforms (2 moving) · up to 8 players".
fn info(d: &GameData, i: usize) -> String {
    let s = &d.stages[i];
    let look = &d.global.render.select;
    let c = &d.global.render.camera;
    let [l, t, r, b] = sim::data::camera::play_area(s);
    let size = ((r - l) / c.reference_play_width).max((b - t) / c.reference_play_height);
    let label = if size < look.size_small {
        "Small"
    } else if size > look.size_large {
        "Large"
    } else {
        "Medium"
    };
    let moving = s.platforms.iter().filter(|p| p.motion.is_some()).count();
    let mut out = format!("{label} · {} platform{}", s.platforms.len(), if s.platforms.len() == 1 { "" } else { "s" });
    if moving > 0 {
        out += &format!(" ({moving} moving)");
    }
    out + &format!(" · up to {} players", s.max_players())
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut commands: Commands,
    (vote, sel): (Option<Res<Vote>>, Option<Res<Select>>),
    roots: Query<(Entity, &VoteRoot)>,
    fonts: Res<Fonts>,
    data: Res<Data>,
    thumbs: Res<StageThumbs>,
) {
    let (Some(v), Some(s)) = (vote, sel) else { return };
    if roots.iter().any(|(_, r)| r.0 == *v && r.1 == s.stocks) {
        return;
    }
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let (d, f, v, s) = (&data.0, &*fonts, &*v, &*s);
    let fit = fitting(d, s.slots.len());
    let color = |voter: &Voter| pal(d, s.color(voter.slot));
    let chip = |voter: &Voter| {
        let (c, _, light) = color(voter);
        Chip { label: format!("P{}", voter.slot + 1), color: c, light, locked: voter.vote.is_some() }
    };
    commands
        .spawn((
            root(),
            VoteRoot(v.clone(), s.stocks),
            DespawnOnExit(Screen::Stages),
            GlobalZIndex(5),
            BackgroundColor(alpha(NIGHT, 0.8)),
        ))
        .with_children(|p| {
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
                    p.spawn(txt(&f.display, 58., STAR, "Stage"));
                    palette_strip(p, d, 22., 4.);
                });
                let k = key(STOCKS, 0);
                let on = v.voters.iter().find(|x| x.cursor.key == k && x.vote.is_none());
                p.spawn((
                    node(|n| {
                        n.flex_direction = FlexDirection::Row;
                        n.align_items = AlignItems::Center;
                        n.justify_content = JustifyContent::SpaceBetween;
                        n.width = Val::Px(PANEL_W);
                        n.padding = UiRect::axes(Val::Px(14.), Val::Px(6.));
                        n.border_radius = BorderRadius::all(Val::Px(10.));
                    }),
                    BackgroundColor(on.map_or(INK.with_alpha(1.), |x| color(x).0.mix(&NIGHT, 0.75))),
                    Focusable { key: k, owner: None, spin: true },
                ))
                .with_children(|p| {
                    p.spawn(row()).with_children(|p| {
                        p.spawn((txt(&f.label, 20., DIM, "Stocks"), node(|n| n.margin.right = Val::Px(8.))));
                        if let Some(x) = on {
                            let c = chip(x);
                            p.spawn((
                                node(|n| {
                                    n.padding = UiRect::axes(Val::Px(5.), Val::Px(0.));
                                    n.border_radius = BorderRadius::all(Val::Px(4.));
                                }),
                                BackgroundColor(c.color),
                            ))
                            .with_child(txt(&f.display, 14., NIGHT, c.label));
                        }
                    });
                    let value = if on.is_some() { format!("‹ {} ›", s.stocks) } else { s.stocks.to_string() };
                    p.spawn(txt(&f.display, 26., STAR, value));
                });
            });
            // The stage grid.
            let n = d.stages.len() + 1;
            let l = GRID.layout(n);
            let place = |n: &mut Node| {
                n.top = Val::Px(GRID_TOP);
                n.left = Val::Px(32.);
            };
            let roll = v.hops.get(v.hop).copied().filter(|_| !v.hops.is_empty());
            grid::panel(p, l, GRID.gap, place, (), |p| {
                for t in 0..n {
                    let k = key(TILE, t);
                    let chips = v.voters.iter().filter(|x| x.cursor.key == k).map(chip).collect();
                    let random = t == d.stages.len();
                    let name = if random { "Random" } else { &d.stages[t].name };
                    let off = (!random && !fit.contains(&t)).then(|| format!("Up to {}", d.stages[t].max_players()));
                    let highlight = (roll == Some(t)).then_some(GOLD);
                    let tile = grid::Tile { key: k, w: l.w, h: l.h, name, chips, highlight, off };
                    let lead = tile.chips.first().map(|c| c.color);
                    let mut wrap = p.spawn(node(|n| {
                        n.width = Val::Px(l.w);
                        n.height = Val::Px(l.h);
                    }));
                    if v.landed && roll == Some(t) {
                        wrap.insert((UiTransform::default(), Landed, ZIndex(2)));
                    } else if highlight.is_some() || lead.is_some() {
                        wrap.insert(ZIndex(1));
                    }
                    wrap.with_children(|p| {
                        grid::tile(p, f, tile, |p| {
                            if random {
                                p.spawn(node(|n| {
                                    grid::fill(n);
                                    n.justify_content = JustifyContent::Center;
                                    n.align_items = AlignItems::Center;
                                    n.padding.bottom = Val::Px(l.h * 0.12);
                                }))
                                .with_child(txt(
                                    &f.display,
                                    l.h * 0.6,
                                    lead.unwrap_or(GOLD),
                                    "?",
                                ));
                            } else {
                                p.spawn((
                                    node(|n| {
                                        grid::fill(n);
                                        n.bottom = Val::Px(NAME);
                                    }),
                                    ImageNode {
                                        rect: Some(StageThumbs::rect(t)),
                                        ..ImageNode::new(thumbs.image.clone())
                                    },
                                ));
                            }
                        });
                    });
                }
            });
            preview(p, f, d, s, v, &thumbs.preview_image);
            p.spawn(node(|n| {
                n.position_type = PositionType::Absolute;
                n.bottom = Val::Px(14.);
                n.left = Val::Px(0.);
                n.right = Val::Px(0.);
                n.flex_direction = FlexDirection::Row;
                n.justify_content = JustifyContent::Center;
                n.column_gap = Val::Px(22.);
            }))
            .with_children(|p| {
                hint(p, f, "Attack / Enter", "vote");
                hint(p, f, "Special / Esc", "take back · back to fighters");
                hint(p, f, "Start", "roll the votes");
            });
        });
}

/// The big preview, its info line, and everyone's votes (or the roulette's result).
fn preview(p: &mut ChildSpawnerCommands, f: &Fonts, d: &GameData, s: &Select, v: &Vote, img: &Handle<Image>) {
    let random = v.preview >= d.stages.len();
    p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.top = Val::Px(GRID_TOP);
            n.right = Val::Px(40.);
            n.width = Val::Px(PANEL_W);
            n.flex_direction = FlexDirection::Column;
            n.row_gap = Val::Px(6.);
            n.padding = UiRect::all(Val::Px(10.));
            n.border_radius = BorderRadius::all(Val::Px(14.));
        }),
        BackgroundColor(alpha(NIGHT, 0.94)),
    ))
    .with_children(|p| {
        let h = (PANEL_W - 20.) * StageThumbs::ASPECT;
        p.spawn((
            node(|n| {
                n.width = Val::Percent(100.);
                n.height = Val::Px(h);
                n.border = UiRect::all(Val::Px(if v.landed { 3. } else { 1.5 }));
                n.border_radius = BorderRadius::all(Val::Px(10.));
                n.overflow = Overflow::clip();
                n.justify_content = JustifyContent::Center;
                n.align_items = AlignItems::Center;
            }),
            BackgroundColor(INK),
            BorderColor::all(if v.landed { GOLD } else { LINE }),
            BoxShadow::new(
                alpha(GOLD, if v.landed { 0.6 } else { 0. }),
                Val::Px(0.),
                Val::Px(0.),
                Val::Px(2.),
                Val::Px(20.),
            ),
        ))
        .with_children(|p| {
            if random {
                p.spawn(txt(&f.display, h * 0.6, GOLD, "?"));
            } else {
                p.spawn((
                    node(grid::fill),
                    ImageNode { rect: Some(StageThumbs::preview_rect()), ..ImageNode::new(img.clone()) },
                ));
            }
        });
        let (name, line) = if random {
            ("Random".to_string(), "A stage that fits everyone, drawn at the roulette".to_string())
        } else {
            (d.stages[v.preview].name.clone(), info(d, v.preview))
        };
        p.spawn(txt(&f.display, 34., if v.landed { GOLD } else { STAR }, name));
        p.spawn(txt(&f.body, 13., DIM, line));
        p.spawn((node(|n| n.height = Val::Px(1.)), BackgroundColor(LINE)));
        if v.landed {
            p.spawn(txt(&f.display, 30., GOLD, "Let's go!"));
        } else if !v.hops.is_empty() {
            p.spawn(txt(&f.display, 26., STAR, "Rolling…"));
        }
        for x in &v.voters {
            let (c, _, _) = pal(d, s.color(x.slot));
            p.spawn(node(|n| {
                n.flex_direction = FlexDirection::Row;
                n.align_items = AlignItems::Center;
                n.column_gap = Val::Px(8.);
            }))
            .with_children(|p| {
                p.spawn((
                    node(|n| {
                        n.padding = UiRect::axes(Val::Px(5.), Val::Px(0.));
                        n.border_radius = BorderRadius::all(Val::Px(4.));
                    }),
                    BackgroundColor(c),
                ))
                .with_child(txt(&f.display, 15., NIGHT, format!("P{}", x.slot + 1)));
                let (label, col) = match x.vote {
                    Some(t) if t >= d.stages.len() => ("Random".to_string(), STAR),
                    Some(t) => (d.stages[t].name.clone(), STAR),
                    None => ("choosing…".to_string(), FAINT),
                };
                p.spawn(txt(&f.label, 18., col, label));
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::select::SelectInit;

    fn data() -> std::sync::Arc<GameData> {
        let (data, errors) = crate::load_data(&[std::path::PathBuf::from("../../assets")]);
        assert!(errors.is_empty());
        std::sync::Arc::new(data)
    }

    /// Character select with bots 0 and 1 joined (and the default CPUs), everyone locked in.
    fn lobby(d: &std::sync::Arc<GameData>) -> App {
        let mut app = App::new();
        app.insert_resource(Data(d.clone()))
            .insert_resource(Saved::default())
            .insert_resource(SelectInit { mode: Mode::Versus, dev: Some(Dev::Bot(0)) })
            .add_systems(Update, select::enter);
        app.update();
        let mut sel = app.world_mut().resource_mut::<Select>();
        let at = sel.slots.len();
        let mut p = sel.slots[0].clone();
        p.dev = Some(Dev::Bot(1));
        sel.slots.insert(at, p);
        for (i, p) in sel.slots.iter_mut().enumerate() {
            p.ready = true;
            p.cursor.owner = Some(i);
        }
        app
    }

    fn devices(navs: [Vec<Nav>; 2]) -> Devices {
        let mut devs = Devices::default();
        for (b, n) in navs.into_iter().enumerate() {
            devs.map.entry(Dev::Bot(b as u8)).or_default().nav = n;
        }
        devs
    }

    fn tiles(n: usize) -> Vec<Spot> {
        (0..n)
            .map(|t| Spot {
                f: Focusable { key: key(TILE, t), owner: None, spin: false },
                center: Vec2::new(t as f32 * 100., 0.),
            })
            .collect()
    }

    #[test]
    fn back_to_character_select_keeps_picks_and_locks() {
        let d = data();
        let mut app = lobby(&d);
        app.world_mut().resource_mut::<Select>().slots[0].fighter = 2;
        app.insert_resource(FromStages);
        app.update();
        let sel = app.world().resource::<Select>();
        assert!(sel.slots.iter().all(|p| p.ready) && sel.slots[0].fighter == 2);
        assert!(app.world().get_resource::<FromStages>().is_none());
        // Coming back from a match unlocks everyone as before.
        app.update();
        assert!(app.world().resource::<Select>().humans().all(|(_, p)| !p.ready));
    }

    #[test]
    fn votes_start_the_roulette_and_special_takes_back_or_leaves() {
        let d = data();
        let app = lobby(&d);
        let mut sel = app.world().resource::<Select>().clone();
        let mut v = Vote::new(&sel);
        assert_eq!(v.voters.len(), 2);
        let spots = tiles(d.stages.len() + 1);
        v.voters[0].cursor.key = key(TILE, 0);
        v.voters[1].cursor.key = key(TILE, 0);
        assert_eq!(v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![]]), &spots, &d), (false, false));
        assert_eq!(v.voters[0].vote, Some(0));
        // Special with a vote takes it back; without one it leaves.
        assert_eq!(v.frame(&mut sel, &devices([vec![Nav::Back], vec![]]), &spots, &d), (false, false));
        assert_eq!(v.voters[0].vote, None);
        assert_eq!(v.frame(&mut sel, &devices([vec![Nav::Back], vec![]]), &spots, &d), (false, true));
        // Start after one vote rolls; all votes in rolls too.
        v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![]]), &spots, &d);
        assert_eq!(v.frame(&mut sel, &devices([vec![], vec![Nav::Start]]), &spots, &d), (true, false));
        assert_eq!(v.hops, [0]);
        let mut v = Vote::new(&sel);
        v.voters[0].cursor.key = key(TILE, 0);
        v.voters[1].cursor.key = key(TILE, 1);
        let r = v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![Nav::Confirm]]), &spots, &d);
        assert_eq!(r, (true, false));
        assert_eq!(v.hops.len(), d.global.render.select.roulette.hops as usize + 1);
        assert!([0, 1].contains(v.hops.last().unwrap()));
    }

    #[test]
    fn a_dropped_voter_returns_to_character_select_even_mid_roulette() {
        let d = data();
        let app = lobby(&d);
        let mut sel = app.world().resource::<Select>().clone();
        let spots = tiles(d.stages.len() + 1);
        let mut v = Vote::new(&sel);
        v.voters[0].cursor.key = key(TILE, 0);
        v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![]]), &spots, &d);
        let mut gone = devices([vec![], vec![]]);
        gone.map.remove(&Dev::Bot(1));
        assert_eq!(v.frame(&mut sel, &gone, &spots, &d), (false, true), "not counted as everyone voted");
        v.hops = vec![0, 1];
        assert_eq!(v.frame(&mut sel, &gone, &spots, &d), (false, true));
    }

    #[test]
    fn the_random_state_carries_over_to_the_next_vote() {
        let d = data();
        let app = lobby(&d);
        let mut sel = app.world().resource::<Select>().clone();
        let spots = tiles(d.stages.len() + 1);
        let mut rolls = vec![];
        for _ in 0..3 {
            let mut v = Vote::new(&sel);
            for x in &mut v.voters {
                x.cursor.key = key(TILE, d.stages.len());
            }
            let (spin, _) = v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![Nav::Confirm]]), &spots, &d);
            assert!(spin);
            sel.roll = v.roll;
            rolls.push(v.roll);
        }
        rolls.dedup();
        assert_eq!(rolls.len(), 3);
    }

    #[test]
    fn roulette_weights_votes_resolves_random_and_slows_down() {
        let d = data();
        let fit = [0, 1, 3];
        let mut roll = 7;
        for _ in 0..50 {
            assert!(fit.contains(&resolve(&[4], 4, &fit, &mut roll)[0]), "random votes draw a fitting stage");
        }
        let wins = (0..2000).filter(|_| *roulette(&[2, 2, 2, 5], 10, &mut roll).last().unwrap() == 2).count();
        assert!((1300..1700).contains(&wins), "three votes of four win about 3/4 of the time: {wins}");
        let hops = roulette(&[1, 3, 5], 10, &mut roll);
        assert_eq!(hops.len(), 11);
        assert!(hops.windows(2).all(|w| w[0] != w[1]), "hops move to another voted stage every time");
        assert_eq!(roulette(&[4, 4], 10, &mut roll), [4], "one distinct stage just lands");
        let r = sim::data::render::Roulette { secs: 2., curve: 2., ..d.global.render.select.roulette.clone() };
        let at: Vec<usize> = [0., 0.5, 1., 1.5, 2., 3.].iter().map(|t| hop_at(*t, 10, &r)).collect();
        assert_eq!((at[0], at[4], at[5]), (0, 10, 10));
        assert!(at.windows(2).all(|w| w[0] <= w[1]));
        assert!(at[1] - at[0] > at[4] - at[3], "early hops come faster than late ones");
    }
}
