//! Render-side effects from the sim's `Fx` events (particles, rings, KO beams, shake) and the follow camera.
//! Both advance once per sim tick and keep their previous state so frames between ticks can interpolate.

use std::f64::consts::TAU;

use sim::data::Color;
use sim::world::State;
use sim::{Fx, GameData, World};

use crate::Rand;
use crate::canvas::{Canvas, Rgba};
use crate::draw::color;

struct Particle {
    x: f64,
    y: f64,
    px: f64,
    py: f64,
    vx: f64,
    vy: f64,
    life: f64,
    max: f64,
    col: Rgba,
    size: f64,
    g: f64,
}
struct Ring {
    x: f64,
    y: f64,
    r: f64,
    pr: f64,
    vr: f64,
    life: u32,
    max: u32,
    col: Rgba,
}
struct Beam {
    x: f64,
    y: f64,
    ang: f64,
    life: u32,
    max: u32,
    col: Rgba,
}

/// Visible stage rectangle `[x0, y0, x1, y1]`.
pub type View = [f64; 4];

#[derive(Default)]
pub struct FxLayer {
    parts: Vec<Particle>,
    rings: Vec<Ring>,
    beams: Vec<Beam>,
    pub shake: f64,
    rng: Rand,
}

impl FxLayer {
    pub fn clear(&mut self) {
        self.parts.clear();
        self.rings.clear();
        self.beams.clear();
        self.shake = 0.;
    }
    fn part(&mut self, x: f64, y: f64, vx: f64, vy: f64, life: f64, col: Rgba, size: f64, g: f64) {
        self.parts.push(Particle { x, y, px: x, py: y, vx, vy, life, max: life.max(1.), col, size, g });
    }
    fn burst(&mut self, x: f64, y: f64, col: Rgba, n: f64, speed: f64, life: f64, size: f64, g: f64) {
        for _ in 0..n as u32 {
            let a = self.rng.unit() * TAU;
            let s = self.rng.unit() * speed;
            let sz = size * (0.5 + self.rng.unit());
            self.part(x, y, a.cos() * s, a.sin() * s, life, col, sz, g);
        }
    }
    fn ring(&mut self, x: f64, y: f64, r: f64, vr: f64, life: u32, col: Rgba) {
        self.rings.push(Ring { x, y, r, pr: r, vr, life, max: life.max(1), col });
    }

    pub fn consume(&mut self, d: &GameData, w: &World, fx: &[Fx], view: View) {
        let look = &d.global.render.fx;
        for e in fx {
            match *e {
                Fx::Sfx { .. } => {}
                Fx::Burst { x, y, color: c, player, n, speed, life, size, gravity } => {
                    self.burst(x, y, color(d, c, player, 1.), n, speed, life, size, gravity)
                }
                Fx::Dust { x, y, n } => {
                    let u = &look.dust;
                    for _ in 0..n {
                        let r = &mut self.rng;
                        let (dx, vx, vy, sz) = (
                            r.range(-u.spread, u.spread),
                            r.range(-u.vx, u.vx),
                            r.range(u.vy[0], u.vy[1]),
                            r.range(u.size[0], u.size[1]),
                        );
                        self.part(x + dx, y, vx, vy, u.life, color(d, u.color, 0, 1.), sz, 0.);
                    }
                }
                Fx::Ring { x, y, r, grow, life, color: c, player } => {
                    self.ring(x, y, r, grow, life, color(d, c, player, 1.))
                }
                Fx::Part { x, y, vx, vy, life, color: c, player, size, gravity } => {
                    self.part(x, y, vx, vy, life, color(d, c, player, 1.), size, gravity)
                }
                Fx::Shake { k } => self.shake = self.shake.max(k),
                Fx::Beam { x, y, player } => {
                    let b = &look.beam;
                    let x = x.clamp(view[0] + b.inset, view[2] - b.inset);
                    let y = y.clamp(view[1] + b.inset, view[3] - b.inset);
                    for s in &b.bursts {
                        self.burst(x, y, color(d, s.color, player, 1.), f64::from(s.n), s.speed, s.life, s.size, 0.);
                    }
                    let st = &d.stages[w.stage];
                    let ang = (st.height / 2. - y).atan2(st.width / 2. - x);
                    self.beams.push(Beam {
                        x,
                        y,
                        ang,
                        life: b.life,
                        max: b.life.max(1),
                        col: color(d, Color::Player, player, 1.),
                    });
                    self.ring(x, y, b.ring.r, b.ring.grow, b.ring.life, color(d, b.ring.color, player, 1.));
                }
            }
        }
    }

    pub fn tick(&mut self, d: &GameData) {
        let look = &d.global.render;
        for p in &mut self.parts {
            (p.px, p.py) = (p.x, p.y);
            p.x += p.vx;
            p.y += p.vy;
            p.vy += p.g;
            p.vx *= look.fx.drag;
            p.life -= 1.;
        }
        self.parts.retain(|p| p.life > 0.);
        for r in &mut self.rings {
            r.pr = r.r;
            r.r += r.vr;
            r.life = r.life.saturating_sub(1);
        }
        self.rings.retain(|r| r.life > 0);
        for b in &mut self.beams {
            b.life = b.life.saturating_sub(1);
        }
        self.beams.retain(|b| b.life > 0);
        self.shake *= look.camera.shake_decay;
        if self.shake < look.camera.shake_min {
            self.shake = 0.;
        }
    }

    pub fn shake_offset(&mut self) -> (f64, f64) {
        if self.shake <= 0. {
            return (0., 0.);
        }
        let s = self.shake;
        (self.rng.range(-s, s), self.rng.range(-s, s))
    }

    /// `t` interpolates positions from the previous tick (0) to the current one (1).
    pub fn draw(&self, cv: &mut Canvas, d: &GameData, t: f64) {
        let look = &d.global.render.fx;
        for b in &self.beams {
            let a = f64::from(b.life) / f64::from(b.max);
            let w = look.beam.width + (1. - a) * look.beam.grow;
            let len = look.beam.length;
            let n = look.beam.bands.max(1);
            cv.save();
            cv.translate(b.x, b.y);
            cv.rotate(b.ang);
            // Smooth gradient: white at the KO point, the player's color fading out along the beam.
            let edge = |k: u32| {
                let col = if k == 0 { Rgba::hex(0xffffff, 1.) } else { b.col };
                (len * f64::from(k) / f64::from(n), col.alpha(a * look.beam.alpha * (1. - f64::from(k) / f64::from(n))))
            };
            for k in 0..n {
                let ((x0, c0), (x1, c1)) = (edge(k), edge(k + 1));
                cv.fill_quad(
                    [[x0, -w * x0 / len], [x1, -w * x1 / len], [x1, w * x1 / len], [x0, w * x0 / len]],
                    [c0, c1, c1, c0],
                );
            }
            cv.restore();
        }
        for p in &self.parts {
            let k = p.life / p.max;
            let (x, y) = (p.px + (p.x - p.px) * t, p.py + (p.y - p.py) * t);
            cv.fill_circle([x, y], p.size * (look.shrink + (1. - look.shrink) * k), p.col.alpha(k));
        }
        for r in &self.rings {
            let rad = r.pr + (r.r - r.pr) * t;
            cv.stroke_circle(
                [r.x, r.y],
                rad.max(0.1),
                look.ring_width,
                r.col.alpha(f64::from(r.life) / f64::from(r.max)),
            );
        }
    }
}

/// Camera framing: center and visible height, in stage pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam {
    pub x: f64,
    pub y: f64,
    pub h: f64,
}

impl Cam {
    pub fn lerp(self, o: Cam, t: f64) -> Cam {
        Cam { x: self.x + (o.x - self.x) * t, y: self.y + (o.y - self.y) * t, h: self.h + (o.h - self.h) * t }
    }
    pub fn view(self, aspect: f64) -> View {
        let (hw, hh) = (self.h * aspect / 2., self.h / 2.);
        [self.x - hw, self.y - hh, self.x + hw, self.y + hh]
    }
}

/// Where the camera wants to be for this world state.
pub fn camera_target(d: &GameData, w: &World, aspect: f64) -> Cam {
    let s = &d.stages[w.stage];
    let c = &d.global.render.camera;
    let (sx, sy) = (s.width / 2., s.height / 2.);
    let m = c.blast_margin;
    let pts: Vec<(f64, f64)> = w
        .fighters
        .iter()
        .filter(|f| !matches!(f.state, State::Dead | State::Out))
        .map(|f| (f.x.clamp(s.blast.left - m, s.blast.right + m), f.y.clamp(s.blast.top - m, s.blast.bottom + m)))
        .collect();
    let mut cam = if pts.is_empty() {
        Cam { x: sx, y: sy, h: s.height }
    } else {
        let x0 = pts.iter().map(|p| p.0).fold(f64::MAX, f64::min) - c.pad_x;
        let x1 = pts.iter().map(|p| p.0).fold(f64::MIN, f64::max) + c.pad_x;
        let y0 = pts.iter().map(|p| p.1).fold(f64::MAX, f64::min) - c.pad_top;
        let y1 = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max) + c.pad_bottom;
        let cx = (x0 + x1) / 2. + (sx - (x0 + x1) / 2.) * c.stage_bias;
        let cy = (y0 + y1) / 2. + (sy - (y0 + y1) / 2.) * c.stage_bias;
        let hw = (cx - x0).max(x1 - cx);
        let hh = (cy - y0).max(y1 - cy);
        Cam { x: cx, y: cy, h: (2. * hh).max(2. * hw / aspect) }
    };
    let range = crate::camera::zoom_range(s, c, aspect);
    cam.h = cam.h.clamp(range.min, range.max);
    clamp_to(cam, &s.camera, aspect)
}

/// Maximum stage view, using the same padded centre, bias and bounds clamp as fighter framing.
pub fn max_zoom_view(s: &sim::data::Stage, c: &sim::data::render::Camera, aspect: f64) -> Cam {
    let [l, t, r, b] = crate::camera::play_area(s);
    let cx = (l + r) / 2.;
    let cy = (t - c.pad_top + b + c.pad_bottom) / 2.;
    let cam = Cam {
        x: cx + (s.width / 2. - cx) * c.stage_bias,
        y: cy + (s.height / 2. - cy) * c.stage_bias,
        h: crate::camera::zoom_range(s, c, aspect).max,
    };
    clamp_to(cam, &s.camera, aspect)
}

fn clamp_to(mut cam: Cam, b: &sim::data::stage::Bounds, aspect: f64) -> Cam {
    cam.h = cam.h.min(b.bottom - b.top).min((b.right - b.left) / aspect);
    let (hw, hh) = (cam.h * aspect / 2., cam.h / 2.);
    cam.x = cam.x.clamp(b.left + hw, b.right - hw);
    cam.y = cam.y.clamp(b.top + hh, b.bottom - hh);
    cam
}

#[derive(Default)]
pub struct CameraRig {
    pub prev: Option<Cam>,
    pub cur: Option<Cam>,
}

impl CameraRig {
    pub fn tick(&mut self, d: &GameData, w: &World, aspect: f64) {
        let target = camera_target(d, w, aspect);
        let c = &d.global.render.camera;
        let next = match self.cur {
            None => target,
            Some(cur) => {
                let k = Cam {
                    x: cur.x + (target.x - cur.x) * c.follow,
                    y: cur.y + (target.y - cur.y) * c.follow,
                    h: cur.h + (target.h - cur.h) * c.zoom_follow,
                };
                clamp_to(k, &d.stages[w.stage].camera, aspect)
            }
        };
        self.prev = Some(self.cur.unwrap_or(next));
        self.cur = Some(next);
    }
    pub fn at(&self, t: f64) -> Option<Cam> {
        Some(self.prev?.lerp(self.cur?, t))
    }
}

#[cfg(test)]
mod camera_tests {
    use super::*;

    fn old_target(d: &GameData, w: &World, aspect: f64) -> Cam {
        let s = &d.stages[w.stage];
        let c = &d.global.render.camera;
        let (sx, sy) = (s.width / 2., s.height / 2.);
        let m = c.blast_margin;
        let pts: Vec<(f64, f64)> = w
            .fighters
            .iter()
            .filter(|f| !matches!(f.state, State::Dead | State::Out))
            .map(|f| (f.x.clamp(s.blast.left - m, s.blast.right + m), f.y.clamp(s.blast.top - m, s.blast.bottom + m)))
            .collect();
        let mut cam = if pts.is_empty() {
            Cam { x: sx, y: sy, h: s.height }
        } else {
            let x0 = pts.iter().map(|p| p.0).fold(f64::MAX, f64::min) - c.pad_x;
            let x1 = pts.iter().map(|p| p.0).fold(f64::MIN, f64::max) + c.pad_x;
            let y0 = pts.iter().map(|p| p.1).fold(f64::MAX, f64::min) - c.pad_top;
            let y1 = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max) + c.pad_bottom;
            let cx = (x0 + x1) / 2. + (sx - (x0 + x1) / 2.) * c.stage_bias;
            let cy = (y0 + y1) / 2. + (sy - (y0 + y1) / 2.) * c.stage_bias;
            let hw = (cx - x0).max(x1 - cx);
            let hh = (cy - y0).max(y1 - cy);
            Cam { x: cx, y: cy, h: (2. * hh).max(2. * hw / aspect) }
        };
        cam.h = cam.h.clamp(c.min_height, c.max_height);
        clamp_to(cam, &s.camera, aspect)
    }

    #[test]
    fn arena_cpu_rig_matches_legacy_camera_frame_by_frame() {
        let sources = [
            ("global.json", include_str!("../../../assets/global.json")),
            ("stages/arena/stage.json", include_str!("../../../assets/stages/arena/stage.json")),
            ("fighters/base/fighter.json", include_str!("../../../assets/fighters/base/fighter.json")),
            ("fighters/brawler/fighter.json", include_str!("../../../assets/fighters/brawler/fighter.json")),
        ]
        .map(|(path, text)| sim::data::Source { path: path.into(), text: text.into() });
        let (d, errors) = sim::data::load(sources).unwrap();
        assert!(errors.is_empty());
        let cfg = sim::MatchConfig {
            training: true,
            players: vec![sim::PlayerConfig { cpu: Some(2), ..sim::PlayerConfig::human("brawler") }; 2],
            ..Default::default()
        };
        let mut w = World::new(cfg, &d).unwrap();
        let mut rig = CameraRig::default();
        let mut legacy = None::<Cam>;
        let c = &d.global.render.camera;
        let aspect = 16. / 9.;
        for frame in 0..900 {
            let target = old_target(&d, &w, aspect);
            let next = match legacy {
                None => target,
                Some(cur) => clamp_to(
                    Cam {
                        x: cur.x + (target.x - cur.x) * c.follow,
                        y: cur.y + (target.y - cur.y) * c.follow,
                        h: cur.h + (target.h - cur.h) * c.zoom_follow,
                    },
                    &d.stages[w.stage].camera,
                    aspect,
                ),
            };
            rig.tick(&d, &w, aspect);
            assert_eq!(rig.prev, Some(legacy.unwrap_or(next)), "previous frame {frame}");
            assert_eq!(rig.cur, Some(next), "frame {frame}");
            legacy = Some(next);
            sim::step_world(&mut w, &d, &[sim::Input::default(); 2]);
            w.fx.clear();
            w.hits.clear();
        }
    }

    #[test]
    fn max_view_uses_padded_bias_and_camera_bounds() {
        let s: sim::data::Stage =
            serde_json::from_str(include_str!("../../../assets/stages/arena/stage.json")).unwrap();
        let g: sim::data::Global = serde_json::from_str(include_str!("../../../assets/global.json")).unwrap();
        let [l, t, r, b] = crate::camera::play_area(&s);
        let c = &g.render.camera;
        let cy = (t - c.pad_top + b + c.pad_bottom) / 2.;
        let expected = clamp_to(
            Cam {
                x: (l + r) / 2. + (s.width / 2. - (l + r) / 2.) * c.stage_bias,
                y: cy + (s.height / 2. - cy) * c.stage_bias,
                h: crate::camera::zoom_range(&s, c, 16. / 9.).max,
            },
            &s.camera,
            16. / 9.,
        );
        assert_eq!(max_zoom_view(&s, c, 16. / 9.), expected);
    }
}
