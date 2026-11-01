//! Shared bevy_ui look (colors, fonts, text and menu rows) and focus navigation for gamepad-driven screens.
//!
//! Screens are rebuilt from a plain model whenever it changes. Focus is a stable `key` per focusable node, so it
//! survives rebuilds. Several cursors can be live at once (character select gives each player one); each moves
//! geometrically between the nodes it may focus, using last frame's layout.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::{FontSource, GenericFontFamily};
use bevy::ui::UiGlobalTransform;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;
use sim::GameData;
use sim::data::Color as DataColor;

use crate::Screen;
use crate::input::{Dev, Devices, Nav};

pub const NIGHT: Color = Color::srgb(0.043, 0.051, 0.165);
pub const INK: Color = Color::srgba(0.055, 0.067, 0.2, 0.9);
pub const INK_SOFT: Color = Color::srgba(0.055, 0.067, 0.2, 0.6);
pub const LINE: Color = Color::srgb(0.2, 0.23, 0.48);
pub const STAR: Color = Color::srgb(0.93, 0.94, 1.0);
pub const DIM: Color = Color::srgb(0.55, 0.58, 0.78);
pub const FAINT: Color = Color::srgb(0.36, 0.39, 0.6);
pub const GOLD: Color = Color::srgb(1.0, 0.83, 0.35);

/// Each screen's frame: read input and update its model, rebuild what changed, then animate. Before the renderer.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum UiSystems {
    Input,
    Draw,
    Animate,
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (UiSystems::Input, UiSystems::Draw, UiSystems::Animate).chain().before(render::RenderSystems),
        )
        .add_systems(Update, (crate::input::clear.run_if(state_changed::<Screen>).before(UiSystems::Input), ui_scale))
        .add_systems(PostUpdate, safe_area.after(bevy::ui::UiSystems::Layout));
    }
}

/// No screen change is pending (systems after a screen's input skip the frame it leaves).
pub fn staying(next: Res<NextState<Screen>>) -> bool {
    matches!(*next, NextState::Unchanged)
}

/// UI is laid out for a 720-pixel-high window and scaled to the real one.
fn ui_scale(win: Query<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    if let Ok(w) = win.single() {
        let s = (w.height() / 720.).max(0.3);
        if (scale.0 - s).abs() > 1e-3 {
            scale.0 = s;
        }
    }
}

/// Stacking of full-window layers.
pub mod z {
    pub const HUD: i32 = 2;
    pub const SCREEN: i32 = 5;
    /// A select card's options popover.
    pub const POPOVER: i32 = 8;
    /// The pause menu and replay controls, over the HUD.
    pub const PAUSE: i32 = 10;
    pub const POINTERS: i32 = 30;
}

/// The bottom row of player cards (select, HUD, results) spans this width.
const CARDS_W: f32 = 1240.;
pub const CARD_GAP: f32 = 10.;

/// Width of each of `n` player cards in a row, at most `max`.
pub fn card_width(n: usize, max: f32) -> f32 {
    (CARDS_W / n.max(1) as f32 - CARD_GAP).min(max)
}

/// Back-out ease: overshoots by about `overshoot / 10` before settling at 1.
pub fn ease_out_back(t: f32, overshoot: f32) -> f32 {
    let t = t.clamp(0., 1.) - 1.;
    1. + (overshoot + 1.) * t * t * t + overshoot * t * t
}

pub fn on_off(b: bool) -> &'static str {
    if b { "On" } else { "Off" }
}

/// The same color for egui panels.
pub fn egui(c: Color) -> egui::Color32 {
    let [r, g, b, a] = c.to_srgba().to_u8_array();
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

#[derive(Resource, Clone)]
pub struct Fonts {
    /// Barlow Condensed Black Italic: wordmark, percents, big moments.
    pub display: Handle<Font>,
    /// Barlow Condensed Bold Italic: headings, names.
    pub heavy: Handle<Font>,
    /// Barlow Condensed SemiBold: menu rows, labels.
    pub label: Handle<Font>,
    /// Barlow Medium / SemiBold: help text.
    pub body: Handle<Font>,
    pub body_bold: Handle<Font>,
}

impl Fonts {
    pub fn load(root: &std::path::Path, fonts: &mut Assets<Font>) -> Self {
        let mut f = |name: &str| match std::fs::read(root.join("fonts").join(name)) {
            Ok(b) => fonts.add(Font::from_bytes(b)),
            Err(e) => {
                error!("font {name}: {e}");
                Handle::default()
            }
        };
        Fonts {
            display: f("BarlowCondensed-BlackItalic.ttf"),
            heavy: f("BarlowCondensed-BoldItalic.ttf"),
            label: f("BarlowCondensed-SemiBold.ttf"),
            body: f("Barlow-Medium.ttf"),
            body_bold: f("Barlow-SemiBold.ttf"),
        }
    }
}

/// The font, falling back to a system sans-serif for glyphs it lacks (arrows in fighter blurbs).
pub fn font_list(font: &Handle<Font>) -> FontSource {
    FontSource::List(vec![font.clone().into(), FontSource::Generic(GenericFontFamily::SansSerif)])
}

pub fn txt(font: &Handle<Font>, size: f32, color: Color, s: impl Into<String>) -> impl Bundle {
    (Text::new(s), TextFont { font: font_list(font), font_size: FontSize::Px(size), ..default() }, TextColor(color))
}

pub fn rgb(c: DataColor) -> Color {
    match c {
        DataColor::Rgb(v) => Color::srgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8),
        _ => STAR,
    }
}

/// Player palette entry `i`: (main, dark, light).
pub fn pal(d: &GameData, i: usize) -> (Color, Color, Color) {
    let p = &d.global.palette[i % d.global.palette.len()];
    (rgb(p.main), rgb(p.dark), rgb(p.light))
}

pub fn node(f: impl FnOnce(&mut Node)) -> Node {
    let mut n = Node::default();
    f(&mut n);
    n
}

pub fn row() -> Node {
    node(|n| {
        n.flex_direction = FlexDirection::Row;
        n.align_items = AlignItems::Center;
    })
}

pub fn col() -> Node {
    node(|n| n.flex_direction = FlexDirection::Column)
}

/// Full-window root for a screen.
pub fn root() -> Node {
    node(|n| {
        n.position_type = PositionType::Absolute;
        n.width = Val::Percent(100.);
        n.height = Val::Percent(100.);
    })
}

/// A screen root drawn from `K` (the model, or a small key struct of what it shows); rebuilt when that changes.
#[derive(Component)]
pub struct Rebuild<K: Send + Sync + 'static>(pub K);

pub type Roots<'w, 's, K> = Query<'w, 's, (Entity, &'static Rebuild<K>), ()>;

/// Keeps one full-window root for `key` on `screen` at `z`: when no root shows an equal key, despawns the old ones and
/// returns a fresh one to fill. `None` (nothing to show) just despawns them.
pub fn rebuild<'a, 'k, K: PartialEq + Clone + Send + Sync + 'static>(
    commands: &'a mut Commands,
    roots: &Roots<K>,
    key: impl Into<Option<&'k K>>,
    screen: Screen,
    z: i32,
) -> Option<EntityCommands<'a>> {
    let key = key.into();
    if key.map_or(roots.is_empty(), |k| roots.iter().any(|(_, r)| r.0 == *k)) {
        return None;
    }
    for (e, _) in roots {
        commands.entity(e).despawn();
    }
    Some(commands.spawn((root(), Rebuild(key?.clone()), DespawnOnExit(screen), GlobalZIndex(z))))
}

/// The top band of a select screen: `title` over the palette strip on the left, `right` on the right.
pub fn title_band(
    p: &mut ChildSpawnerCommands,
    f: &Fonts,
    d: &GameData,
    title: &str,
    right: impl FnOnce(&mut ChildSpawnerCommands),
) {
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
            ColorStop::auto(NIGHT.with_alpha(0.92)),
            ColorStop::auto(NIGHT.with_alpha(0.)),
        ])),
    ))
    .with_children(|p| {
        p.spawn(col()).with_children(|p| {
            p.spawn(txt(&f.display, 58., STAR, title));
            palette_strip(p, d, 22., 4.);
        });
        right(p);
    });
}

/// Focus key for control `kind` of card / slot `slot`.
pub fn key(kind: u32, slot: usize) -> u32 {
    kind << 8 | slot as u32
}
pub fn kind(key: u32) -> (u32, usize) {
    (key >> 8, (key & 255) as usize)
}

/// Cursor key for "over nothing".
pub const NONE: u32 = 0;

#[derive(Component, Clone, Copy, Debug)]
pub struct Focusable {
    pub key: u32,
    /// Only this cursor owner may focus it (`None`: anyone).
    pub owner: Option<usize>,
    /// Left/right change its value instead of moving focus.
    pub spin: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub key: u32,
    pub owner: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Activate(u32),
    Spin(u32, i32),
    Back,
    Start,
}

/// A focusable node's box in logical UI pixels (the 720-high layout).
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub f: Focusable,
    pub center: Vec2,
    pub size: Vec2,
}

impl Spot {
    /// Distance from `p` to the box (0 inside).
    pub fn dist(&self, p: Vec2) -> f32 {
        ((p - self.center).abs() - self.size / 2.).max(Vec2::ZERO).length()
    }
}

pub type FocusQuery<'w, 's> = Query<'w, 's, (&'static Focusable, &'static ComputedNode, &'static UiGlobalTransform)>;

/// A laid-out node's center and size in logical UI pixels.
pub fn logical_box(n: &ComputedNode, t: &UiGlobalTransform) -> (Vec2, Vec2) {
    let k = n.inverse_scale_factor();
    (t.affine().translation * k, n.size() * k)
}

pub fn spots(q: &FocusQuery) -> Vec<Spot> {
    let mut v: Vec<Spot> = q
        .iter()
        .filter(|(_, n, _)| n.size().x > 0.)
        .map(|(f, n, t)| {
            let (center, size) = logical_box(n, t);
            Spot { f: *f, center, size }
        })
        .collect();
    v.sort_by_key(|s| s.f.key);
    v
}

impl Cursor {
    pub fn allowed(&self, f: &Focusable) -> bool {
        f.owner.is_none() || f.owner == self.owner
    }
    /// Keep focus on something that exists (first allowed node in reading order otherwise).
    pub fn settle(&mut self, spots: &[Spot]) {
        if spots.is_empty() || spots.iter().any(|s| s.f.key == self.key && self.allowed(&s.f)) {
            return;
        }
        if let Some(s) = spots
            .iter()
            .filter(|s| self.allowed(&s.f))
            .min_by(|a, b| (a.center.y, a.center.x).partial_cmp(&(b.center.y, b.center.x)).unwrap())
        {
            self.key = s.f.key;
        }
    }
    pub fn drive(&mut self, nav: Nav, spots: &[Spot]) -> Option<Action> {
        self.settle(spots);
        let cur = spots.iter().find(|s| s.f.key == self.key)?;
        let dir = match nav {
            Nav::Confirm => {
                return Some(if cur.f.spin { Action::Spin(self.key, 1) } else { Action::Activate(self.key) });
            }
            Nav::Back => return Some(Action::Back),
            Nav::Start => return Some(Action::Start),
            Nav::Left if cur.f.spin => return Some(Action::Spin(self.key, -1)),
            Nav::Right if cur.f.spin => return Some(Action::Spin(self.key, 1)),
            Nav::Left => Vec2::NEG_X,
            Nav::Right => Vec2::X,
            Nav::Up => Vec2::NEG_Y,
            Nav::Down => Vec2::Y,
        };
        let cand = spots.iter().filter(|s| s.f.key != self.key && self.allowed(&s.f)).map(|s| {
            let d = s.center - cur.center;
            (s, d.dot(dir), (d - dir * d.dot(dir)).length())
        });
        let best = cand
            .clone()
            // Sideways moves stay within 45° so they don't jump to a different row of the screen.
            .filter(|(_, p, o)| *p > 2. && (dir.x == 0. || *o <= *p))
            .min_by(|a, b| (a.1 + a.2 * 3.).total_cmp(&(b.1 + b.2 * 3.)))
            // Vertical lists wrap around.
            .or_else(|| {
                (dir.y != 0.)
                    .then(|| {
                        cand.filter(|(_, p, _)| *p < -2.).min_by(|a, b| (a.2 * 3. + a.1).total_cmp(&(b.2 * 3. + b.1)))
                    })
                    .flatten()
            });
        if let Some((s, _, _)) = best {
            self.key = s.f.key;
        }
        None
    }
}

/// Marks UI that covers the top / bottom of the screen; the gameplay view is framed between them (character select,
/// the HUD's player panels).
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Safe {
    Top,
    Bottom,
}

/// Frames gameplay between the [`Safe`] UI. Layout reports physical pixels, so this ratio also handles window / UI
/// scaling.
pub fn safe_area(
    screen: Res<State<Screen>>,
    edges: Query<(&ComputedNode, &UiGlobalTransform, &Safe)>,
    win: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    mut scene: ResMut<render::Scene>,
) {
    let Ok(w) = win.single() else { return };
    let h = w.physical_height().max(1) as f32;
    let (bottom, top) = match screen.get() {
        // In a match: above the panels' height (they may be hidden in replays) plus a margin.
        Screen::Match | Screen::Playback => {
            let margin = 28. * scale.0 * w.scale_factor();
            let height =
                edges.iter().filter(|(.., e)| **e == Safe::Bottom).map(|(n, ..)| n.size().y).fold(0., f32::max);
            (f64::from((height + margin) / h).clamp(0., 0.5), 0.)
        }
        // Character select: the showcase is framed in the gap between the roster and the cards (the camera's padding
        // keeps whole fighters inside it).
        Screen::Select => edges.iter().fold((0f64, 0f64), |(b, t), (n, tf, e)| {
            let y = tf.affine().translation.y;
            match e {
                Safe::Bottom => (b.max(f64::from((h - (y - n.size().y / 2.)) / h)), t),
                Safe::Top => (b, t.max(f64::from((y + n.size().y / 2.) / h))),
            }
        }),
        _ => (0., 0.),
    };
    if scene.bottom_safe_area != bottom {
        scene.bottom_safe_area = bottom;
    }
    if scene.top_safe_area != top {
        scene.top_safe_area = top;
    }
}

/// The window's size in logical UI pixels (1280 × 720 without one).
pub fn screen_size(win: Option<&Window>, scale: &UiScale) -> Vec2 {
    win.map_or(Vec2::new(1280., 720.), |w| w.size() / scale.0.max(0.01))
}

/// The mouse, in logical UI pixels like [`Spot`]s.
#[derive(SystemParam)]
pub struct Mouse<'w, 's> {
    win: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    scale: Res<'w, UiScale>,
}

impl Mouse<'_, '_> {
    pub fn clicked(&self) -> bool {
        self.mouse.just_pressed(MouseButton::Left)
    }
    pub fn pos(&self) -> Option<Vec2> {
        Some(self.win.single().ok()?.cursor_position()? / self.scale.0)
    }
    /// The screen's size.
    pub fn screen(&self) -> Vec2 {
        screen_size(self.win.single().ok(), &self.scale)
    }
}

/// The keyboard (half) clicks act for: one already `playing`, else the first connected.
pub fn keyboard(devs: &Devices, playing: impl Fn(Dev) -> bool) -> Option<Dev> {
    let keys = [Dev::Keys, Dev::KeysL, Dev::KeysR];
    keys.into_iter().find(|d| playing(*d)).or_else(|| keys.into_iter().find(|d| devs.map.contains_key(d)))
}

/// A menu row: label, optional value (a spinner's, or just shown), focused look.
pub struct Item {
    pub key: u32,
    pub label: String,
    pub value: Option<String>,
    /// Left / right change the value.
    pub spin: bool,
    /// The value is waiting on the player (gold).
    pub hot: bool,
}

impl Item {
    pub fn button(key: u32, label: impl Into<String>) -> Self {
        Item { key, label: label.into(), value: None, spin: false, hot: false }
    }
    pub fn spin(key: u32, label: impl Into<String>, value: impl Into<String>) -> Self {
        Item { key, label: label.into(), value: Some(value.into()), spin: true, hot: false }
    }
    /// A row showing a value it doesn't spin.
    pub fn info(key: u32, label: impl Into<String>, value: impl Into<String>) -> Self {
        Item { key, label: label.into(), value: Some(value.into()), spin: false, hot: false }
    }
}

/// A menu list's size: its width, the label font size, the gap between rows and a fixed label column (else the
/// label takes what the value leaves).
#[derive(Clone, Copy)]
pub struct List {
    pub width: f32,
    pub size: f32,
    pub gap: f32,
    pub label_width: Option<f32>,
}

impl List {
    pub fn new(width: f32, size: f32) -> Self {
        List { width, size, gap: 2., label_width: None }
    }
}

/// Vertical menu list. The focused row gets a gold slash marker and gold text.
pub fn menu_list(p: &mut ChildSpawnerCommands, fonts: &Fonts, items: &[Item], focus: u32, l: List) {
    let size = l.size;
    p.spawn(node(|n| {
        n.flex_direction = FlexDirection::Column;
        n.width = Val::Px(l.width);
        n.row_gap = Val::Px(l.gap);
    }))
    .with_children(|p| {
        for it in items {
            let on = it.key == focus;
            p.spawn((
                node(|n| {
                    n.flex_direction = FlexDirection::Row;
                    n.align_items = AlignItems::Center;
                    n.height = Val::Px(size * 1.55);
                    n.padding = UiRect::horizontal(Val::Px(14.));
                    n.column_gap = Val::Px(12.);
                }),
                BackgroundColor(if on { GOLD.with_alpha(0.08) } else { Color::NONE }),
                Focusable { key: it.key, owner: None, spin: it.spin },
            ))
            .with_children(|p| {
                p.spawn((
                    node(|n| {
                        n.width = Val::Px(5.);
                        n.height = Val::Px(size * 0.95);
                    }),
                    BackgroundColor(if on { GOLD } else { Color::NONE }),
                    UiTransform::from_rotation(Rot2::degrees(14.)),
                ));
                p.spawn((
                    txt(&fonts.label, size, if on { GOLD } else { STAR }, it.label.clone()),
                    node(|n| match l.label_width {
                        Some(w) => n.width = Val::Px(w),
                        None => n.flex_grow = 1.,
                    }),
                ));
                if let Some(v) = &it.value {
                    let s = if on && it.spin { format!("‹  {v}  ›") } else { v.clone() };
                    let color = if it.hot {
                        GOLD
                    } else if on {
                        STAR
                    } else {
                        DIM
                    };
                    p.spawn(txt(&fonts.label, size * 0.92, color, s));
                }
            });
        }
    });
}

/// The 8-color palette strip used under the wordmark and as screen accents.
pub fn palette_strip(p: &mut ChildSpawnerCommands, d: &GameData, w: f32, h: f32) {
    p.spawn(node(|n| {
        n.flex_direction = FlexDirection::Row;
        n.column_gap = Val::Px(3.);
    }))
    .with_children(|p| {
        for i in 0..d.global.palette.len() {
            p.spawn((
                node(|n| {
                    n.width = Val::Px(w);
                    n.height = Val::Px(h);
                }),
                BackgroundColor(pal(d, i).0),
            ));
        }
    });
}

pub fn ordinal(n: usize) -> String {
    let suf = match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    };
    format!("{n}{suf}")
}

/// Damage color from `render.hud.damageRamp`.
pub fn damage_color(d: &GameData, pct: f64) -> Color {
    let ramp = &d.global.render.hud.damage_ramp;
    let Some(first) = ramp.first() else {
        return STAR;
    };
    if pct <= first.0 {
        return rgb(first.1);
    }
    for w in ramp.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if pct <= b {
            let t = ((pct - a) / (b - a).max(1e-6)) as f32;
            return rgb(ca).mix(&rgb(cb), t);
        }
    }
    rgb(ramp[ramp.len() - 1].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spot(key: u32, x: f32, y: f32, spin: bool, owner: Option<usize>) -> Spot {
        Spot { f: Focusable { key, owner, spin }, center: Vec2::new(x, y), size: Vec2::splat(20.) }
    }

    #[test]
    fn vertical_lists_move_and_wrap() {
        let s = [spot(1, 0., 0., false, None), spot(2, 0., 40., false, None), spot(3, 0., 80., false, None)];
        let mut c = Cursor { key: 1, owner: None };
        c.drive(Nav::Down, &s);
        assert_eq!(c.key, 2);
        c.drive(Nav::Down, &s);
        c.drive(Nav::Down, &s);
        assert_eq!(c.key, 1, "wraps to the top");
        c.drive(Nav::Up, &s);
        assert_eq!(c.key, 3, "wraps to the bottom");
    }

    #[test]
    fn spinners_take_left_right_and_confirm() {
        let s = [spot(1, 0., 0., true, None), spot(2, 100., 0., false, None)];
        let mut c = Cursor { key: 1, owner: None };
        assert_eq!(c.drive(Nav::Right, &s), Some(Action::Spin(1, 1)));
        assert_eq!(c.drive(Nav::Left, &s), Some(Action::Spin(1, -1)));
        assert_eq!(c.drive(Nav::Confirm, &s), Some(Action::Spin(1, 1)));
        assert_eq!(c.key, 1);
    }

    #[test]
    fn sideways_moves_stay_in_their_row_and_respect_owners() {
        let s = [
            spot(1, 0., 100., false, Some(0)),
            spot(2, 200., 100., false, Some(1)),
            spot(3, 400., 110., false, None),
            spot(4, 250., -300., false, None),
        ];
        let mut c = Cursor { key: 1, owner: Some(0) };
        c.drive(Nav::Right, &s);
        assert_eq!(c.key, 3, "skips another player's node, ignores the far-up one");
        c.drive(Nav::Right, &s);
        assert_eq!(c.key, 3, "nothing further right");
    }

    #[test]
    fn lost_focus_settles_on_the_first_allowed_node() {
        let s = [spot(5, 0., 50., false, Some(1)), spot(6, 0., 60., false, None)];
        let mut c = Cursor { key: 99, owner: Some(0) };
        c.settle(&s);
        assert_eq!(c.key, 6);
    }

    #[test]
    fn ordinals() {
        let v: Vec<String> = [1, 2, 3, 4, 11, 12, 21].iter().map(|n| ordinal(*n)).collect();
        assert_eq!(v, ["1st", "2nd", "3rd", "4th", "11th", "12th", "21st"]);
    }
}
