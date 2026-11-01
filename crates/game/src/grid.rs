//! Auto-sizing tile grids with dropped player chips, shared by the fighter roster and stage select. Tiles fill an area
//! row by row (the last row may be shorter) at the largest size that fits the count. Digital pointer hops move in 2D:
//! rows wrap sideways, moving down into the ragged last row lands on its last tile, and up / down off the grid leave
//! it geometrically (or wrap to the other edge when nothing is there).

use bevy::prelude::*;

use crate::input::{Dev, Nav};
use crate::ui::*;

/// Room for a grid, in logical pixels. A tile is `w` wide and `w * aspect + extra` tall, at most `max` wide.
#[derive(Clone, Copy, Debug)]
pub struct Area {
    pub w: f32,
    pub h: f32,
    pub aspect: f32,
    pub extra: f32,
    pub gap: f32,
    pub max: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub cols: usize,
    pub rows: usize,
    pub w: f32,
    pub h: f32,
}

impl Area {
    /// Biggest tiles for `n`; among equal sizes, fewer rows, then fewer columns.
    pub fn layout(&self, n: usize) -> Layout {
        let n = n.max(1);
        let mut best = Layout { cols: n, rows: 1, w: 0., h: 0. };
        for cols in 1..=n {
            let rows = n.div_ceil(cols);
            let by_w = (self.w - self.gap * (cols - 1) as f32) / cols as f32;
            let by_h = ((self.h - self.gap * (rows - 1) as f32) / rows as f32 - self.extra) / self.aspect;
            let w = by_w.min(by_h).min(self.max).floor();
            if w > best.w || w == best.w && rows < best.rows {
                best = Layout { cols, rows, w, h: (w * self.aspect + self.extra).floor() };
            }
        }
        best
    }
}

/// Grid move from tile `i` of `n` in `cols` columns. `None`: off the top or bottom edge.
pub fn step(i: usize, n: usize, cols: usize, nav: Nav) -> Option<usize> {
    let start = i / cols * cols;
    let len = cols.min(n - start);
    match nav {
        Nav::Left => Some(start + (i - start + len - 1) % len),
        Nav::Right => Some(start + (i - start + 1) % len),
        Nav::Up if i >= cols => Some(i - cols),
        Nav::Down if start + cols < n => Some((i + cols).min(n - 1)),
        _ => None,
    }
}

/// The tile in the same column at the opposite edge (clamped into the ragged last row).
pub fn wrap(i: usize, n: usize, cols: usize, nav: Nav) -> usize {
    match nav {
        Nav::Up => ((n - 1) / cols * cols + i % cols).min(n - 1),
        _ => i % cols,
    }
}

/// [`Cursor::drive`] that moves on the grid while the cursor is on one of its `n` tiles (keys of kind `tile`).
pub fn drive(c: &mut Cursor, nav: Nav, spots: &[Spot], tile: u32, n: usize, cols: usize) -> Option<Action> {
    let (k, t) = kind(c.key);
    if k != tile || t >= n || !matches!(nav, Nav::Up | Nav::Down | Nav::Left | Nav::Right) {
        return c.drive(nav, spots);
    }
    if let Some(j) = step(t, n, cols, nav) {
        c.key = key(tile, j);
        return None;
    }
    let at = |k: u32| spots.iter().find(|s| s.f.key == k).map(|s| s.center.y);
    let from = at(c.key);
    let a = c.drive(nav, spots);
    // Only leave for something actually above / below (the geometric fallback wraps around the whole screen).
    let ahead = match (from, at(c.key)) {
        (Some(y0), Some(y1)) => (y1 - y0) * if nav == Nav::Up { -1. } else { 1. } > 2.,
        _ => false,
    };
    if !ahead || kind(c.key).0 == tile {
        c.key = key(tile, wrap(t, n, cols, nav));
    }
    a
}

/// A chip dropped on a tile (a pick or a vote).
pub struct Chip {
    pub label: String,
    pub color: Color,
    pub light: Color,
    /// A CPU's: smaller, and it doesn't light the tile.
    pub cpu: bool,
    /// Drops in with [`DropIn`] (when this device locked in).
    pub drop: Option<Dev>,
}

/// A freshly dropped chip's fall, animated by the screen.
#[derive(Component)]
pub struct DropIn(pub Dev);

/// One tile of a grid: chips fan out along its top edge, the name sits in a band at the bottom (on small tiles only
/// while it is pointed at, lit or has a chip).
pub struct Tile<'a> {
    pub key: u32,
    pub w: f32,
    pub h: f32,
    pub name: &'a str,
    pub chips: Vec<Chip>,
    /// The colour of a pointer on it.
    pub hover: Option<Color>,
    /// Border / glow color overriding the others (the roulette).
    pub highlight: Option<Color>,
    /// Not pickable: dimmed, with this note instead of the name.
    pub off: Option<String>,
}

/// The backing panel holding a grid's tiles, at `top`, horizontally centered in `[left, left + width]`.
pub fn panel(
    p: &mut ChildSpawnerCommands,
    l: Layout,
    gap: f32,
    place: impl FnOnce(&mut Node),
    extra: impl Bundle,
    tiles: impl FnOnce(&mut ChildSpawnerCommands),
) {
    let width = l.cols as f32 * l.w + (l.cols - 1) as f32 * gap;
    p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.flex_direction = FlexDirection::Row;
            n.flex_wrap = FlexWrap::Wrap;
            n.width = Val::Px(width + 16.);
            n.padding = UiRect::all(Val::Px(8.));
            n.row_gap = Val::Px(gap);
            n.column_gap = Val::Px(gap);
            n.border_radius = BorderRadius::all(Val::Px(14.));
            place(n);
        }),
        BackgroundColor(NIGHT),
        extra,
    ))
    .with_children(tiles);
}

pub fn tile(p: &mut ChildSpawnerCommands, f: &Fonts, t: Tile, content: impl FnOnce(&mut ChildSpawnerCommands)) {
    let lead = t.highlight.or(t.hover).or(t.chips.iter().find(|c| !c.cpu).map(|c| c.color));
    let locked = t.highlight.is_some() || t.chips.iter().any(|c| !c.cpu);
    let small = t.w < 84.;
    p.spawn((
        node(|n| {
            n.width = Val::Px(t.w);
            n.height = Val::Px(t.h);
            n.border = UiRect::all(Val::Px(if lead.is_some() { 3. } else { 1.5 }));
            n.border_radius = BorderRadius::all(Val::Px(if small { 7. } else { 10. }));
            n.overflow = Overflow::clip();
        }),
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::auto(lead.unwrap_or(LINE).mix(&NIGHT, if locked { 0.45 } else { 0.75 })),
            ColorStop::auto(NIGHT),
        ])),
        BorderColor::all(lead.unwrap_or(LINE.with_alpha(0.9))),
        BoxShadow::new(lead.unwrap_or(NIGHT).with_alpha(0.55), Val::Px(0.), Val::Px(0.), Val::Px(1.), Val::Px(14.)),
        UiTransform::from_scale(Vec2::splat(if lead.is_some() { 1.06 } else { 1. })),
        ZIndex(i32::from(lead.is_some())),
        Focusable { key: t.key, owner: None, spin: false },
    ))
    .with_children(|p| {
        content(p);
        if t.off.is_some() {
            p.spawn((node(fill), BackgroundColor(NIGHT.with_alpha(0.6))));
        }
        let label = t.off.as_deref().unwrap_or(t.name);
        if !small || lead.is_some() || !t.chips.is_empty() || t.off.is_some() {
            p.spawn((
                node(|n| {
                    n.position_type = PositionType::Absolute;
                    n.left = Val::Px(0.);
                    n.right = Val::Px(0.);
                    n.bottom = Val::Px(0.);
                    n.justify_content = JustifyContent::Center;
                    n.padding =
                        UiRect::new(Val::Px(3.), Val::Px(3.), Val::Px(if small { 6. } else { 10. }), Val::Px(2.));
                }),
                BackgroundGradient::from(LinearGradient::to_bottom(vec![
                    ColorStop::auto(NIGHT.with_alpha(0.)),
                    ColorStop::auto(NIGHT.with_alpha(0.92)),
                ])),
            ))
            .with_child((
                txt(
                    &f.heavy,
                    // Shrinks to fit the tile (condensed glyphs are about 0.44 em wide).
                    (t.w * if small { 0.15 } else { 0.17 })
                        .min((t.w - 6.) / (0.44 * label.chars().count().max(1) as f32))
                        .clamp(8., 19.),
                    if lead.is_some() { STAR } else { DIM },
                    label,
                ),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
            ));
        }
        // Chips fan out; later ones overlap earlier ones but every label start stays visible.
        let size = (t.w * 0.2).clamp(11., 15.);
        let cw = size * 2.;
        let n = t.chips.len();
        let step = if n > 1 { ((t.w - 8. - cw) / (n - 1) as f32).min(cw + 3.) } else { 0. };
        for (k, c) in t.chips.iter().enumerate() {
            chip(p, f, c, 4. + k as f32 * step, size);
        }
    });
}

/// The random tile's face: a big "?" (`size` high, lifted by `lift`) in `color`.
pub fn random_face(p: &mut ChildSpawnerCommands, f: &Fonts, size: f32, lift: f32, color: Color) {
    p.spawn(node(|n| {
        fill(n);
        n.justify_content = JustifyContent::Center;
        n.align_items = AlignItems::Center;
        n.padding.bottom = Val::Px(lift);
    }))
    .with_child(txt(&f.display, size, color, "?"));
}

pub fn fill(n: &mut Node) {
    n.position_type = PositionType::Absolute;
    n.left = Val::Px(0.);
    n.right = Val::Px(0.);
    n.top = Val::Px(0.);
    n.bottom = Val::Px(0.);
}

fn chip(p: &mut ChildSpawnerCommands, f: &Fonts, c: &Chip, left: f32, size: f32) {
    p.spawn((
        node(|n| {
            n.position_type = PositionType::Absolute;
            n.top = Val::Px(4.);
            n.left = Val::Px(left);
            n.padding = UiRect::axes(Val::Px(size * 0.33), Val::Px(0.));
            n.border = UiRect::all(Val::Px(1.5));
            n.border_radius = BorderRadius::all(Val::Px(5.));
        }),
        BackgroundColor(c.color),
        BorderColor::all(if c.cpu { NIGHT } else { c.light }),
        BoxShadow::new(NIGHT.with_alpha(0.6), Val::Px(1.), Val::Px(2.), Val::Px(0.), Val::Px(3.)),
        UiTransform::default(),
    ))
    .insert_if(DropIn(c.drop.unwrap_or(Dev::Keys)), || c.drop.is_some())
    .with_child(txt(&f.display, if c.cpu { size * 0.8 } else { size }, NIGHT, c.label.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_grow_rows_and_shrink_tiles_with_the_count() {
        let a = Area { w: 700., h: 330., aspect: 1.23, extra: 0., gap: 10., max: 116. };
        let l = a.layout(4);
        assert_eq!((l.cols, l.rows, l.w), (4, 1, 116.));
        let l = a.layout(13);
        assert_eq!(l.rows, 2);
        let l = a.layout(31);
        assert!((3..=4).contains(&l.rows) && l.cols * l.rows >= 31 && l.w >= 50., "{l:?}");
        assert!(l.rows as f32 * l.h + (l.rows - 1) as f32 * 10. <= 330.);
    }

    #[test]
    fn rows_wrap_sideways_and_the_ragged_last_row_clamps() {
        // 0 1 2 3
        // 4 5 6 7
        // 8 9
        let (n, c) = (10, 4);
        assert_eq!(step(3, n, c, Nav::Right), Some(0));
        assert_eq!(step(0, n, c, Nav::Left), Some(3));
        assert_eq!(step(9, n, c, Nav::Right), Some(8));
        assert_eq!(step(8, n, c, Nav::Left), Some(9));
        assert_eq!(step(6, n, c, Nav::Down), Some(9), "no tile below: the last one");
        assert_eq!(step(9, n, c, Nav::Up), Some(5));
        assert_eq!((step(9, n, c, Nav::Down), step(2, n, c, Nav::Up)), (None, None));
        assert_eq!(wrap(3, n, c, Nav::Up), 9);
        assert_eq!(wrap(1, n, c, Nav::Up), 9);
        assert_eq!(wrap(0, n, c, Nav::Up), 8);
        assert_eq!(wrap(9, n, c, Nav::Down), 1);
    }

    #[test]
    fn leaving_the_grid_falls_back_to_geometry_or_wraps() {
        let spot = |key, x, y| Spot {
            f: Focusable { key, owner: None, spin: false },
            center: Vec2::new(x, y),
            size: Vec2::splat(50.),
        };
        let below = 99;
        let spots = [spot(key(1, 0), 0., 0.), spot(key(1, 1), 100., 0.), spot(below, 0., 300.)];
        let mut c = Cursor { key: key(1, 1), owner: None };
        drive(&mut c, Nav::Down, &spots, 1, 2, 2);
        assert_eq!(c.key, below);
        let mut c = Cursor { key: key(1, 1), owner: None };
        drive(&mut c, Nav::Up, &spots, 1, 2, 2);
        assert_eq!(c.key, key(1, 1), "nothing above: wraps to the same (only) row");
        drive(&mut c, Nav::Right, &spots, 1, 2, 2);
        assert_eq!(c.key, key(1, 0));
    }
}
