//! Geometry-derived presentation limits shared by the follow camera and stage editor.
use super::{Stage, render::Camera};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomRange {
    pub min: f64,
    pub max: f64,
}

/// Union of solid bounds (including undersides) and platform paths. Linear motion has no interior extrema.
pub fn play_area(s: &Stage) -> [f64; 4] {
    let mut r = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    let mut add = |l: f64, t: f64, right: f64, b: f64| {
        r[0] = r[0].min(l);
        r[1] = r[1].min(t);
        r[2] = r[2].max(right);
        r[3] = r[3].max(b);
    };
    for b in &s.blocks {
        add(b.left, b.top, b.right, b.bottom + b.under_depth);
    }
    for p in &s.platforms {
        if let Some(m) = &p.motion
            && !m.waypoints.is_empty()
        {
            for w in &m.waypoints {
                add(p.x1 + w.x, p.y + w.y, p.x2 + w.x, p.y + w.y);
            }
        } else {
            add(p.x1, p.y, p.x2, p.y);
        }
    }
    if r.iter().all(|v| v.is_finite()) { r } else { [s.width / 2., s.height / 2., s.width / 2., s.height / 2.] }
}

fn cap(s: &Stage, aspect: f64, mut r: ZoomRange) -> ZoomRange {
    let limit = (s.camera.bottom - s.camera.top).min((s.camera.right - s.camera.left) / aspect).max(f64::EPSILON);
    r.max = r.max.min(limit).max(f64::EPSILON);
    r.min = r.min.min(r.max).max(f64::EPSILON);
    r
}

pub fn automatic_range(s: &Stage, c: &Camera, aspect: f64) -> ZoomRange {
    let aspect = aspect.max(f64::EPSILON);
    let [l, t, r, b] = play_area(s);
    let (w, h) = ((r - l).max(0.), (b - t).max(0.));
    let fit = |w: f64, h: f64| ((w + 2. * c.pad_x) / aspect).max(h + c.pad_top + c.pad_bottom);
    let ratio = fit(w, h) / fit(c.reference_play_width, c.reference_play_height);
    let size = (w / c.reference_play_width).max(h / c.reference_play_height);
    let min = c.min_height * (size / c.min_scale_start).max(c.min_scale_floor).min(1.);
    let cx = (l + r) / 2.;
    let cy = (t - c.pad_top + b + c.pad_bottom) / 2.;
    let bias_x = (s.width / 2. - cx).abs() * c.stage_bias;
    let bias_y = (s.height / 2. - cy).abs() * c.stage_bias;
    let max = (c.max_height * ratio.max(1.)).max(fit(w + 2. * bias_x, h + 2. * bias_y));
    cap(s, aspect, ZoomRange { min, max })
}

/// Explicit bounds win over automatic bounds; camera bounds always constrain the final visible height.
pub fn zoom_range(s: &Stage, c: &Camera, aspect: f64) -> ZoomRange {
    let mut r = automatic_range(s, c, aspect);
    if let Some(z) = &s.zoom {
        if let Some(min) = z.min_height {
            r.min = min;
            r.max = r.max.max(min);
        }
        if let Some(max) = z.max_height {
            r.max = max;
            r.min = r.min.min(max);
        }
    }
    cap(s, aspect.max(f64::EPSILON), r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> (Stage, Camera) {
        let s = serde_json::from_str(include_str!("../../../../assets/stages/arena/stage.json")).unwrap();
        let g: crate::data::Global = serde_json::from_str(include_str!("../../../../assets/global.json")).unwrap();
        (s, g.render.camera)
    }

    #[test]
    fn arena_keeps_reference_range_at_landscape_aspects() {
        let (s, c) = data();
        for aspect in [1.2, 4. / 3., 16. / 9., 21. / 9., 3.] {
            assert_eq!(automatic_range(&s, &c, aspect), ZoomRange { min: c.min_height, max: c.max_height });
        }
    }

    #[test]
    fn portrait_max_is_continuous_at_reference_size() {
        let (s, c) = data();
        for aspect in [0.5, 0.75, 16. / 9.] {
            let baseline = automatic_range(&s, &c, aspect).max;
            for delta in [-1., -0.001, 0.001, 1.] {
                let mut resized = s.clone();
                resized.blocks[0].right += delta;
                let changed = automatic_range(&resized, &c, aspect).max;
                assert!((changed - baseline).abs() <= delta.abs() * 4., "{aspect}: {baseline} → {changed}");
            }
        }
        assert!(automatic_range(&s, &c, 0.5).max > c.max_height);
    }

    #[test]
    fn wide_small_and_empty_stages() {
        let (mut s, c) = data();
        let aspect = 16. / 9.;
        s.blocks[0].left -= c.reference_play_width / 2.;
        s.blocks[0].right += c.reference_play_width / 2.;
        assert!(automatic_range(&s, &c, aspect).max > c.max_height);
        s.platforms.clear();
        s.blocks[0].right = s.blocks[0].left + c.reference_play_width / 4.;
        s.blocks[0].bottom = s.blocks[0].top;
        s.blocks[0].under_depth = 0.;
        assert!(automatic_range(&s, &c, aspect).min < c.min_height);
        s.blocks.clear();
        let r = zoom_range(&s, &c, aspect);
        assert!(r.min > 0. && r.max.is_finite() && r.min <= r.max);
    }

    #[test]
    fn overrides_win_and_mixed_bounds_stay_ordered() {
        let (mut s, c) = data();
        s.zoom = Some(crate::data::stage::Zoom { min_height: Some(600.), max_height: Some(1300.) });
        assert_eq!(zoom_range(&s, &c, 16. / 9.), ZoomRange { min: 600., max: 1300. });
        s.zoom.as_mut().unwrap().min_height = None;
        s.zoom.as_mut().unwrap().max_height = Some(300.);
        assert_eq!(zoom_range(&s, &c, 16. / 9.), ZoomRange { min: 300., max: 300. });
        s.zoom = Some(crate::data::stage::Zoom { min_height: Some(1500.), max_height: None });
        assert_eq!(zoom_range(&s, &c, 16. / 9.), ZoomRange { min: 1500., max: 1500. });
        s.camera.bottom = s.camera.top + 700.;
        assert_eq!(zoom_range(&s, &c, 16. / 9.), ZoomRange { min: 700., max: 700. });
    }

    #[test]
    fn motion_extrema_and_camera_cap() {
        let (mut s, c) = data();
        s.platforms[0].motion = Some(crate::data::stage::Motion {
            mode: crate::data::stage::PathMode::PingPong,
            phase: 0,
            waypoints: vec![
                crate::data::stage::Waypoint { x: 0., y: 0., travel_ticks: 1, wait_ticks: 0 },
                crate::data::stage::Waypoint { x: 2000., y: -1000., travel_ticks: 1, wait_ticks: 0 },
            ],
        });
        let area = play_area(&s);
        assert_eq!(area[2], 2570.);
        assert_eq!(area[1], -600.);
        let r = automatic_range(&s, &c, 16. / 9.);
        assert!(r.max > c.max_height);
        assert!(r.max <= (s.camera.right - s.camera.left) / (16. / 9.));
    }
}
