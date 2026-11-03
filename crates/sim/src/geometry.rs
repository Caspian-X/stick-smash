//! World-space collision geometry, independent of combat routing.
//!
//! Callers must supply finite coordinates, finite positive radii, and ordered rectangle bounds
//! (`x1 <= x2`, `y1 <= y2`). Extreme magnitudes are normalized before distance arithmetic.
//! Shapes are closed: tangency counts as contact. A zero-length capsule is a circle.

use crate::data::{Pt, Rect};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Circle { center: Pt, radius: f64 },
    Capsule { a: Pt, b: Pt, radius: f64 },
}

impl Shape {
    fn segment(self) -> (Pt, Pt, f64) {
        match self {
            Self::Circle { center, radius } => (center, center, radius),
            Self::Capsule { a, b, radius } => (a, b, radius),
        }
    }

    /// Circle center or capsule segment midpoint.
    pub fn center(self) -> Pt {
        let (a, b, _) = self.segment();
        [midpoint(a[0], b[0]), midpoint(a[1], b[1])]
    }

    pub fn bounds(self) -> Rect {
        let (a, b, r) = self.segment();
        Rect { x1: a[0].min(b[0]) - r, x2: a[0].max(b[0]) + r, y1: a[1].min(b[1]) - r, y2: a[1].max(b[1]) + r }
    }

    fn scaled(self, scale: f64) -> Self {
        let (a, b, r) = self.segment();
        Self::Capsule { a: a.map(|v| v / scale), b: b.map(|v| v / scale), radius: r / scale }
    }

    pub fn overlaps(self, other: Self) -> bool {
        let (a, b, r) = self.segment();
        let (c, d, s) = other.segment();
        let scale = magnitude(&[a, b, c, d], r.max(s));
        if scale > 1e100 {
            return self.scaled(scale).overlaps(other.scaled(scale));
        }
        segment_distance_squared(a, b, c, d) <= (r + s) * (r + s)
    }

    pub fn overlaps_rect(self, rect: Rect) -> bool {
        let (a, b, r) = self.segment();
        let scale = magnitude(&[a, b, [rect.x1, rect.y1], [rect.x2, rect.y2]], r);
        if scale > 1e100 {
            return self.scaled(scale).overlaps_rect(Rect {
                x1: rect.x1 / scale,
                x2: rect.x2 / scale,
                y1: rect.y1 / scale,
                y2: rect.y2 / scale,
            });
        }
        let inside = |p: Pt| p[0] >= rect.x1 && p[0] <= rect.x2 && p[1] >= rect.y1 && p[1] <= rect.y2;
        if inside(a) || inside(b) {
            return true;
        }
        let corners = [[rect.x1, rect.y1], [rect.x2, rect.y1], [rect.x2, rect.y2], [rect.x1, rect.y2]];
        (0..4).any(|i| segment_distance_squared(a, b, corners[i], corners[(i + 1) % 4]) <= r * r)
    }
}

fn midpoint(a: f64, b: f64) -> f64 {
    if a.is_sign_positive() != b.is_sign_positive() { a * 0.5 + b * 0.5 } else { a + (b - a) * 0.5 }
}
fn magnitude(points: &[Pt], radius: f64) -> f64 {
    points.iter().flatten().fold(radius, |m, v| m.max(v.abs()))
}

fn sub(a: Pt, b: Pt) -> Pt {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot(a: Pt, b: Pt) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: Pt, b: Pt) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn point_distance_squared(p: Pt, a: Pt, b: Pt) -> f64 {
    let ab = sub(b, a);
    let length = dot(ab, ab);
    let t = if length == 0. { 0. } else { (dot(sub(p, a), ab) / length).clamp(0., 1.) };
    let delta = sub(p, [a[0] + t * ab[0], a[1] + t * ab[1]]);
    dot(delta, delta)
}
fn segment_distance_squared(a: Pt, b: Pt, c: Pt, d: Pt) -> f64 {
    let ab = sub(b, a);
    let cd = sub(d, c);
    let sides = |x: f64, y: f64| (x <= 0. && y >= 0.) || (x >= 0. && y <= 0.);
    // The bounds check distinguishes disjoint collinear (including degenerate) segments.
    let bounds_overlap = (0..2).all(|i| a[i].min(b[i]) <= c[i].max(d[i]) && c[i].min(d[i]) <= a[i].max(b[i]));
    if bounds_overlap
        && sides(cross(ab, sub(c, a)), cross(ab, sub(d, a)))
        && sides(cross(cd, sub(a, c)), cross(cd, sub(b, c)))
    {
        return 0.;
    }
    point_distance_squared(a, c, d)
        .min(point_distance_squared(b, c, d))
        .min(point_distance_squared(c, a, b))
        .min(point_distance_squared(d, a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(x: f64, y: f64, radius: f64) -> Shape {
        Shape::Circle { center: [x, y], radius }
    }
    fn capsule(a: Pt, b: Pt, radius: f64) -> Shape {
        Shape::Capsule { a, b, radius }
    }
    fn contact(a: Shape, b: Shape, expected: bool) {
        for a in [a, reverse(a)] {
            for b in [b, reverse(b)] {
                assert_eq!(a.overlaps(b), expected, "{a:?}, {b:?}");
                assert_eq!(b.overlaps(a), expected, "{b:?}, {a:?}");
            }
        }
    }
    fn reverse(s: Shape) -> Shape {
        match s {
            Shape::Capsule { a, b, radius } => capsule(b, a, radius),
            _ => s,
        }
    }

    #[test]
    fn finite_extreme_geometry_does_not_overflow() {
        let far = circle(1e200, 0., 1e190);
        contact(far, circle(0., 0., 1.), false);
        assert!(!far.overlaps_rect(Rect { x1: -1., x2: 1., y1: -1., y2: 1. }));
        let span = capsule([-1e308, 0.], [1e308, 0.], 1e190);
        assert_eq!(span.center(), [0., 0.]);
        contact(span, circle(0., 0., 1.), true);
        contact(span, circle(0., 1e200, 1.), false);
    }

    #[test]
    fn circles_and_degenerate_capsules() {
        for a in [circle(0., 0., 2.), capsule([0., 0.], [0., 0.], 2.)] {
            contact(a, circle(3., 4., 3.), true);
            contact(a, capsule([3., 4.], [3., 4.], 2.9), false);
            contact(a, circle(0., 0., 0.1), true);
        }
    }

    #[test]
    fn circle_capsule_side_and_end_contacts() {
        let a = capsule([0., 0.], [10., 0.], 1.);
        contact(a, circle(5., 3., 2.), true);
        contact(a, circle(5., 3.01, 2.), false);
        contact(a, circle(13., 4., 4.), true);
        contact(a, circle(13., 4.01, 4.), false);
    }

    #[test]
    fn crossing_parallel_and_collinear_segments() {
        let a = capsule([-10., 0.], [10., 0.], 0.25);
        contact(a, capsule([0., -10.], [0., 10.], 0.25), true);
        contact(a, capsule([-5., 0.5], [5., 0.5], 0.25), true);
        contact(a, capsule([-5., 0.51], [5., 0.51], 0.25), false);
        contact(a, capsule([5., 0.], [15., 0.], 0.25), true);
        contact(a, capsule([10.5, 0.], [15., 0.], 0.25), true);
        contact(a, capsule([10.51, 0.], [15., 0.], 0.25), false);
        contact(a, capsule([0., 0.], [0., 0.], 0.25), true);
        contact(a, capsule([0., 1.], [0., 1.], 0.25), false);
    }

    #[test]
    fn rectangles_use_edges_and_rounded_corners() {
        let r = Rect { x1: 0., x2: 10., y1: 0., y2: 10. };
        for (s, expected) in [
            (capsule([-5., 5.], [15., 5.], 0.1), true),
            (capsule([2., 2.], [8., 8.], 0.1), true),
            (capsule([-5., -1.], [15., -1.], 1.), true),
            (capsule([-5., -1.01], [15., -1.01], 1.), false),
            (circle(-3., -4., 5.), true),
            (circle(-0.8, -0.8, 1.), false),
            (capsule([-2., -0.8], [-0.8, -0.8], 1.), false),
            (capsule([-3., -4.], [-3., -4.], 5.), true),
        ] {
            assert_eq!(s.overlaps_rect(r), expected, "{s:?}");
            assert_eq!(reverse(s).overlaps_rect(r), expected);
        }
        let s = capsule([-2., -0.8], [-0.8, -0.8], 1.);
        assert!(s.bounds().x2 > r.x1 && s.bounds().y2 > r.y1);
    }

    #[test]
    fn scales_and_queries() {
        for scale in [0.001, 1., 10000.] {
            let p = |x, y| [x * scale, y * scale];
            let a = capsule(p(-10., 0.), p(10., 0.), scale);
            contact(a, capsule(p(0., -10.), p(0., 10.), scale), true);
            contact(a, capsule(p(-5., 3.), p(5., 3.), scale), false);
            assert_eq!(a.center(), [0., 0.]);
            assert_eq!(a.bounds(), Rect { x1: -11. * scale, x2: 11. * scale, y1: -scale, y2: scale });
        }
        assert_eq!(circle(3., 4., 2.).center(), [3., 4.]);
    }
}
