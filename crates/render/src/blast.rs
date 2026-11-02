//! Gameplay KO boundary, clipped to the current view and drawn at a constant screen size.
use sim::{GameData, data::Stage};

use crate::{
    canvas::{Canvas, P},
    draw::color,
};

pub fn draw(cv: &mut Canvas, d: &GameData, s: &Stage, view: [f64; 4]) {
    let Some(look) = &d.global.render.blast_zone else { return };
    if look.alpha == 0. || cv.detail <= 0. {
        return;
    }
    let b = &s.blast;
    let [l, t, r, bottom] = view;
    let dash = look.dash / cv.detail;
    let stride = (look.dash + look.gap) / cv.detail;
    let width = look.width / cv.detail;
    let col = color(d, look.color, 0, look.alpha);
    let mut line = |lo: f64, hi: f64, point: &dyn Fn(f64) -> P| {
        let mut at = (lo / stride).floor() * stride;
        while at < hi {
            let end = (at + dash).min(hi);
            if end > lo {
                cv.line(point(at.max(lo)), point(end), width, col);
            }
            at += stride;
        }
    };
    for x in [b.left, b.right] {
        if x >= l && x <= r {
            line(t.max(b.top), bottom.min(b.bottom), &|y| [x, y]);
        }
    }
    for y in [b.top, b.bottom] {
        if y >= t && y <= bottom {
            line(l.max(b.left), r.min(b.right), &|x| [x, y]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_only_draw_where_visible_and_can_be_disabled() {
        let mut d = GameData {
            global: serde_json::from_str(include_str!("../../../assets/global.json")).unwrap(),
            stages: vec![],
            fighters: vec![],
            hash: 0,
        };
        let s: Stage = serde_json::from_str(include_str!("../../../assets/stages/arena/stage.json")).unwrap();
        let mut cv = Canvas::new(1.);
        draw(&mut cv, &d, &s, [300., 100., 980., 700.]);
        assert!(cv.idx.is_empty());
        let view = [s.blast.left - 20., 0., s.blast.left + 20., 100.];
        draw(&mut cv, &d, &s, view);
        assert!(!cv.idx.is_empty());
        assert!(cv.pos.iter().all(|p| (f64::from(p[0]) - s.blast.left).abs() <= 1. && p[1] >= -100. && p[1] <= 0.));
        let count = cv.pos.len();
        cv.clear(2., 1.);
        draw(&mut cv, &d, &s, view);
        assert!(cv.pos.len() > count);
        assert!(cv.pos.iter().all(|p| (f64::from(p[0]) - s.blast.left).abs() <= 0.5));
        cv.clear(1., 1.);
        d.global.render.blast_zone.as_mut().unwrap().alpha = 0.;
        draw(&mut cv, &d, &s, view);
        assert!(cv.idx.is_empty());
        d.global.render.blast_zone = None;
        draw(&mut cv, &d, &s, view);
        assert!(cv.idx.is_empty());
    }
}
