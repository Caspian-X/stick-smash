//! Stage select (versus), after everyone locks in on character select and presses Start. Every human steers their
//! pointer ([`crate::pointer`]) over the stage grid (live thumbnails; the big preview follows the last pointer moved)
//! and Stocks: attack drops a vote chip on the stage (again elsewhere moves it), special takes the vote back, or with
//! no vote returns to character select with picks kept. Once every human has voted, or anyone
//! presses Start after a vote, a roulette hops over the voted stages (duplicate votes weigh more), slows down and
//! lands on the winner (`render.select.roulette`); after a pause the match starts. Stocks are set here.

use bevy::prelude::*;
use render::thumb::StageThumbs;
use sim::GameData;

use crate::grid::{self, Area, Chip};
use crate::hints::{self, Ctl};
use crate::input::{Dev, Devices};
use crate::play::Match;
use crate::pointer::{self, Grid, Look, Pointers};
use crate::select::{self, FromStages, STOCKS, Select, Step};
use crate::settings::Saved;
use crate::showcase;
use crate::ui::{self, *};
use crate::{Data, Screen};

const TILE: u32 = 1;
const GRID: Area = Area { w: 740., h: 500., aspect: StageThumbs::ASPECT, extra: NAME, gap: 10., max: 236. };
/// Name band under a stage tile's thumbnail.
const NAME: f32 = 20.;
const GRID_TOP: f32 = 128.;
const PANEL_W: f32 = 404.;

pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Stages), enter).add_systems(OnExit(Screen::Stages), exit).add_systems(
            Update,
            (
                (input, (roulette_clock, thumbs, stage).chain().run_if(ui::staying)).chain().in_set(UiSystems::Input),
                draw.in_set(UiSystems::Draw),
                animate.in_set(UiSystems::Animate),
            )
                .run_if(
                    in_state(Screen::Stages)
                        .and_then(resource_exists::<Vote>)
                        .and_then(resource_exists::<Select>)
                        .and_then(resource_exists::<Clock>),
                ),
        );
    }
}

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

/// Each vote's stage: a vote past the last stage (the random tile) draws one of `fit`.
pub fn resolve(votes: &[usize], stages: usize, fit: &[usize], roll: &mut u32) -> Vec<usize> {
    votes.iter().map(|&t| if t < stages { t } else { fit[select::roll(roll) as usize % fit.len()] }).collect()
}

/// The roulette: a winner drawn from `pool` (each vote one ticket), and `hops + 1` stops cycling through the distinct
/// voted stages in order, ending on it. A single distinct stage just lands.
pub fn roulette(pool: &[usize], hops: u32, roll: &mut u32) -> Vec<usize> {
    let winner = pool[select::roll(roll) as usize % pool.len()];
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
    /// One frame of input: [`Step::Start`] when the roulette starts, [`Step::Leave`] back to character select.
    fn frame(
        &mut self,
        sel: &mut Select,
        devs: &Devices,
        spots: &[Spot],
        d: &GameData,
        ptrs: &mut Pointers,
        dt: f32,
    ) -> Step {
        let n = d.stages.len() + 1;
        let g = Grid { tile: TILE, n, cols: GRID.layout(n).cols };
        let cfg = &d.global.render.select.pointer;
        let fit = sel.fitting(d);
        let votable = |t: usize| fit.contains(&t) || t == d.stages.len() && !fit.is_empty();
        // A dropped controller sends everyone back to character select, which holds its slot for reconnecting.
        if self.voters.iter().any(|v| !devs.map.contains_key(&v.dev)) {
            return Step::Leave;
        }
        if !self.hops.is_empty() {
            return Step::Stay;
        }
        let (mut go, mut leave) = (false, false);
        let screen = ptrs.screen;
        for (v, x) in self.voters.iter_mut().enumerate() {
            let Some(st) = devs.map.get(&x.dev) else { continue };
            let ptr = ptrs.of(x.dev, v, &x.cursor, spots);
            let (acts, moved) = pointer::drive(ptr, &mut x.cursor, st, spots, Some(g), cfg, dt, screen);
            for a in acts {
                match a {
                    // Confirming your own vote again rolls, like Start.
                    Action::Activate(k) if kind(k).0 == TILE && x.vote == Some(kind(k).1) => go = true,
                    Action::Activate(k) if kind(k).0 == TILE && votable(kind(k).1) => {
                        x.vote = Some(kind(k).1);
                        self.preview = kind(k).1;
                        ptr.bump(cfg.drop);
                    }
                    Action::Activate(k) | Action::Spin(k, _) if kind(k).0 != STOCKS => {}
                    Action::Activate(k) => sel.spin(k, 1, d),
                    Action::Spin(k, by) => sel.spin(k, by, d),
                    Action::Back if x.vote.is_some() => {
                        x.vote = None;
                        ptr.bump(cfg.land);
                    }
                    Action::Back => leave = true,
                    Action::Start => go = true,
                }
            }
            if let Some((TILE, t)) = moved.map(kind) {
                self.preview = t;
            }
            ptr.look = Look::new(pal(d, sel.color(x.slot)).0, x.slot, x.vote.is_none(), None);
        }
        let votes: Vec<usize> = self.voters.iter().filter_map(|v| v.vote).collect();
        if leave {
            return Step::Leave;
        }
        if votes.is_empty() || !go && votes.len() < self.voters.len() {
            return Step::Stay;
        }
        let pool = resolve(&votes, d.stages.len(), &fit, &mut self.roll);
        self.hops = roulette(&pool, d.global.render.select.roulette.hops, &mut self.roll);
        self.preview = self.hops[0];
        Step::Start
    }
    /// What voter `x` can do right now.
    fn hints(&self, x: &Voter) -> Vec<(Ctl, &'static str)> {
        let on = kind(x.cursor.key);
        let mut hints = vec![];
        if on.0 == TILE {
            hints.push((Ctl::Confirm, if x.vote == Some(on.1) { "roll" } else { "vote" }));
        } else if on.0 == STOCKS {
            hints.push((Ctl::Confirm, "change"));
        }
        hints.push(match x.vote {
            Some(_) => (Ctl::Back, "take back"),
            None => (Ctl::Back, "to fighters"),
        });
        hints
    }
    fn winner(&self) -> Option<usize> {
        self.hops.last().copied()
    }
}

pub fn enter(mut commands: Commands, sel: Option<Res<Select>>, data: Res<Data>, mut thumbs: ResMut<StageThumbs>) {
    if let Some(sel) = sel {
        commands.insert_resource(Vote::new(&sel));
    }
    commands.insert_resource(Clock::default());
    thumbs.data = Some(data.0.clone());
    thumbs.cells = (0..data.0.stages.len()).collect();
}

pub fn exit(mut commands: Commands, mut thumbs: ResMut<StageThumbs>, mut ptrs: ResMut<Pointers>) {
    thumbs.cells.clear();
    commands.remove_resource::<Vote>();
    commands.remove_resource::<Clock>();
    ptrs.reset();
}

/// Moves pointers and takes votes (the mouse too, until the roulette starts); back to character select keeping
/// everyone locked in.
fn input(
    mut commands: Commands,
    (mut vote, mut sel): (ResMut<Vote>, ResMut<Select>),
    mut devs: ResMut<Devices>,
    q: FocusQuery,
    (mouse, mut ptrs): (Mouse, ResMut<Pointers>),
    (time, mut clock): (Res<Time>, ResMut<Clock>),
    data: Res<Data>,
    mut next: ResMut<NextState<Screen>>,
) {
    let spots = ui::spots(&q);
    let (mut v, mut s) = (vote.clone(), sel.clone());
    ptrs.screen = mouse.screen();
    if v.hops.is_empty() {
        pointer::mouse(&mut devs, &mut ptrs, &spots, &mouse, |dv| v.voters.iter().any(|x| x.dev == dv));
    }
    let step = v.frame(&mut s, &devs, &spots, &data.0, &mut ptrs, time.delta_secs());
    // Successive votes keep drawing from the same sequence.
    s.roll = v.roll;
    vote.set_if_neq(v);
    sel.set_if_neq(s);
    match step {
        Step::Leave => {
            clock.0 = None;
            commands.insert_resource(FromStages);
            next.set(Screen::Select);
        }
        Step::Start => clock.0 = Some(0.),
        Step::Stay => {}
    }
}

/// Steps the roulette's hops; after the landing and a pause, the match starts on the winner.
fn roulette_clock(
    mut commands: Commands,
    (mut vote, mut sel): (ResMut<Vote>, ResMut<Select>),
    (time, mut clock): (Res<Time>, ResMut<Clock>),
    mut saved: ResMut<Saved>,
    data: Res<Data>,
    mut next: ResMut<NextState<Screen>>,
    mut scene: ResMut<render::Scene>,
) {
    let Some(t) = clock.0 else { return };
    let r = &data.0.global.render.select.roulette;
    // Frame hitches don't skip hops.
    let t = t + time.delta_secs().min(0.05);
    clock.0 = Some(t);
    let n = vote.hops.len() - 1;
    let hop = hop_at(t, n, r);
    let land = if n == 0 { 0. } else { r.secs };
    let landed = t >= land;
    if (vote.hop, vote.landed) != (hop, landed) {
        vote.hop = hop;
        vote.preview = vote.hops[hop];
        vote.landed = landed;
    }
    if t >= land + r.pause
        && let Some(w) = vote.winner()
    {
        sel.stage = w;
        sel.start(&data.0, &mut saved, &mut commands, &mut scene, &mut next);
        clock.0 = None;
    }
}

/// The live thumbnails: the preview and the lineup's spawn colors.
fn thumbs(vote: Res<Vote>, sel: Res<Select>, data: Res<Data>, mut thumbs: ResMut<StageThumbs>) {
    let preview = (vote.preview < data.0.stages.len()).then_some(vote.preview);
    let colors: Vec<usize> = (0..sel.slots.len()).map(|i| sel.color(i)).collect();
    if thumbs.preview != preview || thumbs.colors != colors {
        (thumbs.preview, thumbs.colors) = (preview, colors);
    }
}

/// The lineup stands on the previewed stage behind the screen, everyone in their win pose (it stays put while the
/// roulette runs).
fn stage(
    mut commands: Commands,
    (vote, sel): (Res<Vote>, Res<Select>),
    data: Res<Data>,
    mut scene: ResMut<render::Scene>,
    mut m: Option<ResMut<Match>>,
) {
    let d = &data.0;
    if !vote.hops.is_empty() {
        return;
    }
    let stage = if sel.fits(d, vote.preview) { vote.preview } else { sel.stage };
    if !sel.fits(d, stage) {
        return;
    }
    let (slots, rules) = (sel.match_slots(), sel.rules(d, stage));
    if let Some(m) = showcase::showcase(&mut commands, &mut scene, m.as_deref_mut(), d, &slots, &rules) {
        showcase::pose(m, |_| true, &mut vec![]);
    }
}

#[derive(Clone, PartialEq)]
pub struct VoteKey {
    vote: Vote,
    stocks: i32,
}

/// The landed tile's bounce.
#[derive(Component)]
pub struct Landed;

fn animate(clock: Res<Clock>, data: Res<Data>, v: Res<Vote>, mut q: Query<&mut UiTransform, With<Landed>>) {
    let Some(t) = clock.0 else { return };
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
    let [l, t, r, b] = render::camera::play_area(s);
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

fn draw(
    mut commands: Commands,
    (vote, sel): (Res<Vote>, Res<Select>),
    roots: Roots<VoteKey>,
    fonts: Res<Fonts>,
    data: Res<Data>,
    thumbs: Res<StageThumbs>,
    (devs, saved): (Res<Devices>, Res<Saved>),
) {
    let shown = VoteKey { vote: vote.clone(), stocks: sel.stocks };
    let Some(mut root) = ui::rebuild(&mut commands, &roots, &shown, Screen::Stages, z::SCREEN) else { return };
    let (d, f, v, s) = (&data.0, &*fonts, &*vote, &*sel);
    let fit = s.fitting(d);
    let color = |voter: &Voter| pal(d, s.color(voter.slot));
    let chip = |voter: &Voter| {
        let (c, _, light) = color(voter);
        Chip { label: format!("P{}", voter.slot + 1), color: c, light, cpu: false, drop: None }
    };
    root.insert(BackgroundColor(NIGHT.with_alpha(0.8))).with_children(|p| {
        title_band(p, f, d, "Stage", |p| {
            let k = key(STOCKS, 0);
            let on = v.voters.iter().find(|x| x.cursor.key == k);
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
                p.spawn(txt(&f.label, 20., DIM, "Stocks"));
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
                let chips = v.voters.iter().filter(|x| x.vote == Some(t)).map(chip).collect();
                let hover = v.voters.iter().find(|x| x.cursor.key == k).map(|x| color(x).0);
                let random = t == d.stages.len();
                let name = if random { "Random" } else { &d.stages[t].name };
                let off = (!random && !fit.contains(&t)).then(|| format!("Up to {}", d.stages[t].max_players()));
                let highlight = (roll == Some(t)).then_some(GOLD);
                let tile = grid::Tile { key: k, w: l.w, h: l.h, name, chips, hover, highlight, off };
                let lead = tile.hover.or(tile.chips.first().map(|c| c.color));
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
                            grid::random_face(p, f, l.h * 0.6, l.h * 0.12, lead.unwrap_or(GOLD));
                        } else {
                            p.spawn((
                                node(|n| {
                                    grid::fill(n);
                                    n.bottom = Val::Px(NAME);
                                }),
                                ImageNode { rect: Some(StageThumbs::rect(t)), ..ImageNode::new(thumbs.image.clone()) },
                            ));
                        }
                    });
                });
            }
        });
        preview(p, f, d, s, v, &thumbs.preview_image, &devs, &saved);
    });
}

/// The big preview, its info line, and everyone's votes (or the roulette's result).
fn preview(
    p: &mut ChildSpawnerCommands,
    f: &Fonts,
    d: &GameData,
    s: &Select,
    v: &Vote,
    img: &Handle<Image>,
    devs: &Devices,
    saved: &Saved,
) {
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
        BackgroundColor(NIGHT.with_alpha(0.94)),
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
                GOLD.with_alpha(if v.landed { 0.6 } else { 0. }),
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
                p.spawn((txt(&f.label, 18., col, label), node(|n| n.flex_grow = 1.)));
                if v.hops.is_empty() {
                    hints::hint_row(p, f, x.dev, &v.hints(x), DIM, false, (saved, devs));
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Nav;
    use crate::play::Mode;
    use crate::select::SelectInit;

    fn data() -> std::sync::Arc<GameData> {
        crate::test_data()
    }

    /// Character select with bots 0 and 1 joined (and the default CPUs), everyone locked in.
    fn lobby(d: &std::sync::Arc<GameData>) -> App {
        let mut app = App::new();
        app.insert_resource(Data(d.clone()))
            .init_resource::<render::portrait::Portraits>()
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
                size: Vec2::splat(60.),
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
        assert_eq!(
            v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![]]), &spots, &d, &mut Pointers::default(), 0.016),
            Step::Stay
        );
        assert_eq!(v.voters[0].vote, Some(0));
        // Voting again elsewhere moves the chip.
        v.voters[0].cursor.key = key(TILE, 1);
        v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![]]), &spots, &d, &mut Pointers::default(), 0.016);
        assert_eq!(v.voters[0].vote, Some(1));
        // Special with a vote takes it back; without one it leaves.
        assert_eq!(
            v.frame(&mut sel, &devices([vec![Nav::Back], vec![]]), &spots, &d, &mut Pointers::default(), 0.016),
            Step::Stay
        );
        assert_eq!(v.voters[0].vote, None);
        assert_eq!(
            v.frame(&mut sel, &devices([vec![Nav::Back], vec![]]), &spots, &d, &mut Pointers::default(), 0.016),
            Step::Leave
        );
        // Start after one vote rolls; all votes in rolls too.
        v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![]]), &spots, &d, &mut Pointers::default(), 0.016);
        assert_eq!(
            v.frame(&mut sel, &devices([vec![], vec![Nav::Start]]), &spots, &d, &mut Pointers::default(), 0.016),
            Step::Start
        );
        assert_eq!(v.hops, [1]);
        // Confirming your own vote again rolls too (keyboards have no Start here).
        let mut v = Vote::new(&sel);
        v.voters[0].cursor.key = key(TILE, 0);
        v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![]]), &spots, &d, &mut Pointers::default(), 0.016);
        assert_eq!(
            v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![]]), &spots, &d, &mut Pointers::default(), 0.016),
            Step::Start
        );
        let mut v = Vote::new(&sel);
        v.voters[0].cursor.key = key(TILE, 0);
        v.voters[1].cursor.key = key(TILE, 1);
        let r = v.frame(
            &mut sel,
            &devices([vec![Nav::Confirm], vec![Nav::Confirm]]),
            &spots,
            &d,
            &mut Pointers::default(),
            0.016,
        );
        assert_eq!(r, Step::Start);
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
        v.frame(&mut sel, &devices([vec![Nav::Confirm], vec![]]), &spots, &d, &mut Pointers::default(), 0.016);
        let mut gone = devices([vec![], vec![]]);
        gone.map.remove(&Dev::Bot(1));
        assert_eq!(
            v.frame(&mut sel, &gone, &spots, &d, &mut Pointers::default(), 0.016),
            Step::Leave,
            "not counted as everyone voted"
        );
        v.hops = vec![0, 1];
        assert_eq!(v.frame(&mut sel, &gone, &spots, &d, &mut Pointers::default(), 0.016), Step::Leave);
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
            let step = v.frame(
                &mut sel,
                &devices([vec![Nav::Confirm], vec![Nav::Confirm]]),
                &spots,
                &d,
                &mut Pointers::default(),
                0.016,
            );
            assert_eq!(step, Step::Start);
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
