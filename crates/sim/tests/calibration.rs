//! Calibration against Smash Ultimate for the import calibration set (King, Duchess, Stratus).
//!
//! Ignored; `just calibrate [fighter]` runs it in release. Launches: each smash, tilt, aerial (short hop against a
//! standing dummy, and from the full hop apex against a dummy held at the attacker's height) and throw hits a
//! Mario dummy (`fixtures/calibration.json`: weight, gravity, fall speeds at 5 px/unit on brawler's body) on Arena's
//! main block. The spacing / timing that launches hardest at 100% is kept; at 0/50/100/150% the strongest hit
//! reports hitstun, launch speed and the distance at hitstun end, and a binary search finds the launch-KO percent
//! (the feel report's rule). Each move is one JSON line in `CALIBRATE_OUT` (default `target/calibration.jsonl`)
//! with the hitbox that landed, its start position and the victim, which `tools/reference_ko.py --compare` puts
//! next to Ultimate's numbers. Movement (speeds, accelerations, jumps, airtimes, landing lag, autocancels) prints
//! as a table against the source × 5 here.
mod common;
use serde_json::{Value, json};
use sim::{
    GameData, Input, PlayerConfig, World, combat,
    data::{HitType, Hitbox},
    input::{ATTACK, GRAB, JUMP},
    world::{Fighter, State},
};

const CENTRE: f64 = 640.;
const DUMMY: &str = "brawler";
const FIGHTERS: [&str; 3] = ["king", "duchess", "stratus"];
const PERCENTS: [f64; 4] = [0., 50., 100., 150.];
const KO_CAP: u32 = 300;
const SCALE: f64 = 5.;
const AERIALS: [(&str, (i8, i8)); 5] =
    [("nair", (0, 0)), ("fair", (1, 0)), ("bair", (-1, 0)), ("uair", (0, 1)), ("dair", (0, -1))];
const IDLE: Input = Input { stick: (0, 0), buttons: 0, attack_stick: (0, 0) };

fn refs() -> Value {
    serde_json::from_str(include_str!("fixtures/calibration.json")).unwrap()
}
fn inp(x: i8, y: i8, buttons: u8) -> Input {
    Input { stick: (x, y), buttons, ..Default::default() }
}
fn astick(x: i8, y: i8) -> Input {
    Input { attack_stick: (x, y), ..Default::default() }
}
fn tick(w: &mut World, d: &GameData, i: Input) {
    common::step_draining(w, d, &[i]);
}
fn r2(v: f64) -> f64 {
    (v * 100.).round() / 100.
}
fn r3(v: f64) -> f64 {
    (v * 1000.).round() / 1000.
}

/// Arena without platforms; the dummy has Mario's weight, gravity and fall speeds (brawler's body).
fn data() -> GameData {
    let mut d = common::data();
    d.stages.iter_mut().filter(|s| s.id == "arena").for_each(|s| s.platforms.clear());
    let v = &refs()["victim"];
    let k = |key: &str| v[key].as_f64().unwrap();
    let s = &mut d.fighters.iter_mut().find(|f| f.id == DUMMY).unwrap().stats;
    s.weight = 100. / k("weight");
    s.gravity = k("gravity") * SCALE;
    s.fall_speed = k("fall") * SCALE;
    s.fast_fall_speed = k("fastFall") * SCALE;
    d
}
fn world(d: &GameData, ids: &[&str]) -> World {
    let mut w = common::sandbox(d, "arena", ids.iter().map(|id| PlayerConfig::human(id)).collect());
    for f in &mut w.fighters {
        (f.vx, f.vy) = (0., 0.);
    }
    (w.fighters[0].x, w.fighters[0].face) = (CENTRE, 1.);
    w
}

#[derive(Clone, Copy)]
enum Setup {
    /// Attack-stick input (with stick tilts on for tilts).
    Ground(Input, bool),
    /// Aerial direction relative to facing, pressed `delay` ticks after a one-tick jump press.
    ShortHop((i8, i8)),
    /// Aerial pressed on the full hop's apex tick; the dummy is held at the attacker's height until hit.
    FullHop((i8, i8)),
    /// Grab, then this stick direction relative to facing.
    Throw((i8, i8)),
}
struct Move {
    name: String,
    slot: &'static str,
    setup: Setup,
}
fn moves() -> Vec<Move> {
    let m = |name: &str, slot, setup| Move { name: name.into(), slot, setup };
    let mut v = vec![
        m("fsmash", "fsmash", Setup::Ground(astick(127, 0), false)),
        m("usmash", "usmash", Setup::Ground(astick(0, 127), false)),
        m("dsmash", "dsmash", Setup::Ground(astick(0, -127), false)),
        m("ftilt", "ftilt", Setup::Ground(astick(127, 0), true)),
        m("utilt", "utilt", Setup::Ground(astick(0, 127), true)),
        m("dtilt", "dtilt", Setup::Ground(astick(0, -127), true)),
    ];
    for (slot, dir) in AERIALS {
        v.push(m(&format!("{slot} SH"), slot, Setup::ShortHop(dir)));
        v.push(m(&format!("{slot} FH"), slot, Setup::FullHop(dir)));
    }
    for (name, dir) in [("fthrow", (1, 0)), ("bthrow", (-1, 0)), ("uthrow", (0, 1)), ("dthrow", (0, -1))] {
        v.push(m(name, "grab", Setup::Throw(dir)));
    }
    v
}
fn aerial_input((x, y): (i8, i8), face: f64) -> Input {
    if (x, y) == (0, 0) { inp(0, 0, ATTACK) } else { astick((f64::from(x) * 127. * face) as i8, y * 127) }
}

#[derive(Clone)]
struct Hit {
    /// Damage this move dealt before the hit.
    prior: f64,
    speed: f64,
    hitstun: i32,
    hitlag: u32,
    grounded: bool,
    x0: f64,
    y0: f64,
    dir: f64,
    hitbox: Option<Hitbox>,
    dx: Option<f64>,
}
#[derive(Clone, Copy, PartialEq)]
enum Out {
    Stay,
    Ring,
    Ko,
}
struct Trial {
    hits: Vec<Hit>,
    out: Out,
}
impl Trial {
    fn strongest(&self) -> Option<&Hit> {
        // Identified (non-autolink) hits first, then the fastest launch.
        self.hits
            .iter()
            .max_by(|a, b| (a.hitbox.is_some(), a.speed).partial_cmp(&(b.hitbox.is_some(), b.speed)).unwrap())
    }
}

/// The hitbox of the attacker's move (or throw) that produced this launch.
fn identify(d: &GameData, a: &Fighter, slot: &str, v: &Fighter, damage: f64, grounded: bool) -> Option<Hitbox> {
    let fd = &d.fighters[a.data];
    let candidates: Vec<&Hitbox> = if a.state == State::Throw {
        let t = &fd.throws;
        vec![
            &match a.sd.kind.as_str() {
                "b" => &t.b,
                "u" => &t.u,
                "d" => &t.d,
                _ => &t.f,
            }
            .hit,
        ]
    } else {
        let m = fd.moves.get(a.mov.as_ref().map_or(slot, |m| m.slot.as_str()))?;
        m.phases.iter().flat_map(|p| &p.hitboxes).collect()
    };
    // Damage and hitstun pick the hitbox; the launch direction breaks ties (the victim may already have moved one
    // tick when its hitlag is zero).
    let weight = d.fighters[v.data].stats.weight;
    let fits = |b: &&Hitbox| {
        let kb = combat::knockback(d, b, v.dmg, weight);
        (b.damage - damage).abs() < 1e-6
            && b.kind != HitType::Autolink
            && (combat::hitstun_of(d, b, kb) - v.hitstun).abs() <= 1
    };
    let along = |b: &&Hitbox| {
        let kb = combat::knockback(d, b, v.dmg, weight);
        let a = combat::launch_angle(d, b, kb, grounded).to_radians();
        let (vx, mut vy) = (libm::cos(a), -libm::sin(a));
        if grounded && vy > 0. {
            vy = -vy * d.global.knockback.ground_bounce;
        }
        let (lx, ly) = v.launch;
        (vx.abs() * lx.abs() + vy * ly) / (vx.hypot(vy) * lx.hypot(ly)).max(1e-9)
    };
    let fit: Vec<&Hitbox> = candidates.into_iter().filter(fits).collect();
    fit.iter().copied().max_by(|a, b| along(a).total_cmp(&along(b))).cloned()
}

/// The feel report's launch-KO rule: side or top before touching ground or a ledge since the hit, or the bottom in
/// hitstun.
fn launch_ko(d: &GameData, w: &World, f: &Fighter, prev: State, untouched: bool) -> bool {
    if f.y > d.stages[w.stage].blast.bottom { prev == State::Hitstun } else { untouched }
}

/// Plays the move with the attacker `dx` px from a dummy at centre at `pct`; `follow` keeps going until the dummy
/// dies or settles, otherwise it stops when the attacker can act again.
fn trial(d: &GameData, id: &str, mv: &Move, (dx, delay): (f64, u32), pct: f64, follow: bool) -> Trial {
    let mut w = world(d, &[id, DUMMY]);
    let face = match mv.setup {
        Setup::ShortHop((-1, 0)) | Setup::FullHop((-1, 0)) => -1.,
        _ => 1.,
    };
    (w.fighters[0].x, w.fighters[0].face) = (CENTRE - dx, face);
    (w.fighters[1].x, w.fighters[1].face, w.fighters[1].dmg) = (CENTRE, -1., pct);
    w.cfg.players[0].controls.stick_tilts = matches!(mv.setup, Setup::Ground(_, true));
    let min_hold = d.global.grab.min_hold;
    let mut out = Trial { hits: vec![], out: Out::Stay };
    let (mut pressed, mut pinned, mut touched, mut last) = (None, false, false, pct);
    for t in 0..900u32 {
        let a = &w.fighters[0];
        let i = match mv.setup {
            Setup::Ground(i, _) if t == 0 => i,
            Setup::ShortHop(_) if t == 0 => inp(0, 0, JUMP),
            Setup::ShortHop(dir) if t == delay => aerial_input(dir, face),
            Setup::FullHop(_) if pressed.is_none() && (a.ground || a.vy < 0.) && t < 60 => inp(0, 0, JUMP),
            Setup::FullHop(dir) if pressed.is_none() && t < 60 => {
                pressed = Some(t);
                pinned = true;
                aerial_input(dir, face)
            }
            Setup::Throw(_) if t == 0 => inp(0, 0, GRAB),
            Setup::Throw((x, y)) if a.state == State::Hold && a.t > min_hold => {
                inp((f64::from(x) * 127. * face) as i8, y * 127, 0)
            }
            _ => IDLE,
        };
        if pinned {
            let y = w.fighters[0].y;
            let v = &mut w.fighters[1];
            if v.state != State::Air {
                v.set_state(State::Air);
            }
            (v.y, v.vx, v.vy, v.ground, v.surf) = (y, 0., 0., false, None);
        }
        let (prev, was_ground) = (w.fighters[1].state, w.fighters[1].ground);
        tick(&mut w, d, i);
        let press_t = match mv.setup {
            Setup::Ground(..) | Setup::Throw(_) => Some(0),
            Setup::ShortHop(_) => Some(delay),
            Setup::FullHop(_) => pressed,
        };
        if press_t == Some(t)
            && w.fighters[0].mov.as_ref().is_none_or(|m| m.slot != mv.slot)
            && w.fighters[0].state != State::Jumpsquat
        {
            return out;
        }
        let (a, v) = (&w.fighters[0], &w.fighters[1]);
        if v.dmg > last + 1e-9 && v.state == State::Hitstun {
            pinned = false;
            touched = false;
            let grounded = was_ground && a.state != State::Throw;
            let hitbox = identify(d, a, mv.slot, v, v.dmg - last, grounded);
            // The hit's direction sign (`HitSource::direction`): the launch's x sign over the angle's cosine.
            let dir = hitbox.as_ref().map_or(1., |b| {
                let kb = combat::knockback(d, b, v.dmg, d.fighters[v.data].stats.weight);
                let c = libm::cos(combat::launch_angle(d, b, kb, grounded).to_radians());
                if c.abs() < 1e-9 || v.launch.0 == 0. { 1. } else { (v.launch.0 * c).signum() }
            });
            // A throw without hitlag launches during the thrower's update, so the victim (a later index) has
            // already moved one tick: undo it (position, hitstun tick, one launch decay).
            let stepped = a.state == State::Throw && v.lag == 0 && v.t > 0;
            let (mut x, mut y, mut speed, mut hitstun) = (v.x, v.y, v.vx.hypot(v.vy), v.hitstun);
            if stepped {
                (x, y, hitstun) = (x - v.vx, y - v.vy, hitstun + 1);
                speed = v.launch.0.hypot(v.launch.1) + sim::physics::decay_amount(d);
            }
            out.hits.push(Hit {
                prior: last - pct,
                speed,
                hitstun,
                hitlag: v.lag,
                grounded,
                x0: x - CENTRE,
                y0: d.stages[w.stage].blocks[0].top - y,
                dir,
                hitbox,
                dx: None,
            });
        }
        last = last.max(v.dmg);
        if let Some(h) = out.hits.last_mut()
            && h.dx.is_none()
            && v.state != State::Hitstun
            && v.state != State::Dead
        {
            h.dx = Some(v.x - CENTRE - h.x0);
        }
        if v.state == State::Dead {
            let ko = !out.hits.is_empty() && launch_ko(d, &w, v, prev, !touched);
            out.out = if ko { Out::Ko } else { Out::Ring };
            return out;
        }
        if out.hits.is_empty() {
            if t > 150 || press_t.is_some_and(|p| t > p + 5) && a.actionable(&d.global) {
                return out;
            }
            continue;
        }
        if !follow && a.actionable(&d.global) {
            return out;
        }
        touched |= v.ground;
        if v.state == State::Ledge {
            out.out = Out::Ring;
            return out;
        }
        if v.ground && v.actionable(&d.global) && a.actionable(&d.global) {
            return out;
        }
    }
    out
}

fn lowest(yes: impl Fn(u32) -> bool) -> Option<u32> {
    if !yes(KO_CAP) {
        return None;
    }
    let (mut lo, mut hi) = (0, KO_CAP);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if yes(mid) { hi = mid } else { lo = mid + 1 }
    }
    Some(lo)
}

/// Spacing (and short-hop timing) whose strongest hit launches hardest at 100%.
fn best_setup(d: &GameData, id: &str, mv: &Move) -> Option<(f64, u32)> {
    let (delays, step) = match mv.setup {
        Setup::ShortHop(_) => (1..=25, 4),
        _ => (0..=0, 2),
    };
    let mut best: Option<((f64, u32), f64)> = None;
    for delay in delays {
        for dx in (0..=300 / step).rev().map(|k| f64::from(k * step)) {
            let t = trial(d, id, mv, (dx, delay), 100., false);
            if let Some(h) = t.strongest()
                && best.is_none_or(|(_, s)| h.speed > s + 1e-9)
            {
                best = Some(((dx, delay), h.speed));
                if matches!(mv.setup, Setup::Throw(_)) {
                    return best.map(|b| b.0);
                }
            }
        }
    }
    best.map(|b| b.0)
}

fn launches(d: &GameData, id: &str, mv: &Move) -> Value {
    let mut v = json!({ "fighter": id, "move": mv.name, "victim": refs()["victim"] });
    let Some(setup) = best_setup(d, id, mv) else {
        v["reason"] = json!("no hit");
        return v;
    };
    let at = |p: f64| trial(d, id, mv, setup, p, true);
    let rows: Vec<Trial> = PERCENTS.iter().map(|p| at(*p)).collect();
    let main = rows[2].strongest().cloned().unwrap();
    let pick = |f: fn(&Hit) -> Value| rows.iter().map(|t| t.strongest().map_or(Value::Null, f)).collect::<Vec<_>>();
    let ko = lowest(|p| at(f64::from(p)).out == Out::Ko);
    let o = v.as_object_mut().unwrap();
    o.insert("setup".into(), json!({ "dx": setup.0, "delay": setup.1 }));
    o.insert("hitbox".into(), main.hitbox.as_ref().map_or(Value::Null, |h| serde_json::to_value(h).unwrap()));
    if main.hitbox.is_none() {
        o.insert("reason".into(), json!("hitbox not identified"));
    }
    o.insert("grounded".into(), json!(main.grounded));
    o.insert("x0".into(), json!(r2(main.x0)));
    o.insert("y0".into(), json!(r2(main.y0)));
    o.insert("direction".into(), json!(main.dir));
    o.insert("prior".into(), json!(main.prior));
    o.insert("ko".into(), json!(ko));
    o.insert("hitstun".into(), json!(pick(|h| json!(h.hitstun))));
    o.insert("hitlag".into(), json!(pick(|h| json!(h.hitlag))));
    o.insert("speed".into(), json!(pick(|h| json!(r2(h.speed)))));
    o.insert("dx100".into(), json!(main.dx.map(|x| (x * 10.).round() / 10.)));
    v
}

// Movement.

fn f0(w: &World) -> &Fighter {
    &w.fighters[0]
}
/// Holds `first` for one tick, then `rest`, until airborne; returns the takeoff height.
fn takeoff(w: &mut World, d: &GameData, first: Input, rest: Input) -> f64 {
    let y0 = f0(w).y;
    tick(w, d, first);
    for _ in 0..20 {
        if !f0(w).ground {
            break;
        }
        tick(w, d, rest);
    }
    y0
}
/// Height, and airborne frames before landing (frame 1 = first airborne tick; the landing tick is not counted, as
/// in UFD's air times).
fn flight(w: &mut World, d: &GameData, y0: f64) -> (f64, u32) {
    let (mut top, mut t) = (f0(w).y, 1);
    while !f0(w).ground && t < 600 {
        tick(w, d, IDLE);
        t += 1;
        top = top.min(f0(w).y);
    }
    (y0 - top, t - 1)
}
fn to_apex(w: &mut World, d: &GameData) {
    for _ in 0..200 {
        if f0(w).vy >= 0. || f0(w).ground {
            break;
        }
        tick(w, d, IDLE);
    }
}

/// Lag after landing on move frame `k` (0 = the tick the aerial starts): ticks until actionable.
fn landing_on(d: &GameData, id: &str, input: Input, k: u32) -> Option<u32> {
    let mut w = world(d, &[id]);
    let floor = f0(&w).y;
    let hold = |w: &mut World, y: f64| {
        let f = &mut w.fighters[0];
        (f.y, f.vy, f.vx, f.ground, f.surf) = (y, 0., 0., false, None);
    };
    w.fighters[0].set_state(State::Air);
    hold(&mut w, floor - 200.);
    tick(&mut w, d, input);
    f0(&w).mov.as_ref()?;
    while f0(&w).mov.as_ref().is_some_and(|m| m.elapsed + 1 < k) {
        hold(&mut w, floor - 200.);
        tick(&mut w, d, IDLE);
    }
    hold(&mut w, floor - 0.01);
    tick(&mut w, d, IDLE);
    if !f0(&w).ground {
        return None;
    }
    let g = &d.global;
    if f0(&w).actionable(g) {
        return Some(0);
    }
    (1..=120).find(|_| {
        tick(&mut w, d, IDLE);
        f0(&w).actionable(g)
    })
}
/// Source frames (1-based) whose landing lag is below the aerial's full landing lag, as ranges.
fn autocancels(d: &GameData, id: &str, slot: &str, input: Input) -> (Option<u32>, Vec<[u32; 2]>) {
    let total: u32 = d.fighter(id).unwrap().moves.get(slot).unwrap().phases.iter().filter_map(|p| p.frames).sum();
    let lags: Vec<(u32, Option<u32>)> = (0..total).map(|k| (k + 1, landing_on(d, id, input, k))).collect();
    let full = lags.iter().filter_map(|l| l.1).max();
    let mut out: Vec<[u32; 2]> = vec![];
    for (frame, lag) in lags {
        if lag.is_some_and(|l| Some(l) < full) {
            match out.last_mut() {
                Some(r) if r[1] + 1 == frame => r[1] = frame,
                _ => out.push([frame, frame]),
            }
        }
    }
    (full, out)
}

fn movement(d: &GameData, id: &str) -> Vec<(String, Value, Value)> {
    let r = &refs()["fighters"][id];
    let src = |k: &str| r[k].as_f64().unwrap() * SCALE;
    let mut rows: Vec<(String, Value, Value)> = vec![];
    let mut row = |name: &str, eng: Value, src: Value| rows.push((name.into(), eng, src));
    // Run: held from standstill; dash: the tick after a flick.
    let mut w = world(d, &[id]);
    w.fighters[0].x = 260.;
    tick(&mut w, d, inp(127, 0, 0));
    row("dash", json!(r2(f0(&w).vx)), json!(r2(src("dash"))));
    for _ in 0..50 {
        tick(&mut w, d, inp(127, 0, 0));
    }
    row("run", json!(r2(f0(&w).vx)), json!(r2(src("run"))));
    row("walk (stat)", json!(r2(sim::gait::walk_speed(d, f0(&w)))), json!(r2(src("walk"))));
    // Gravity: one rising tick of a short hop (a full hop's first ticks are its initial rise).
    let mut w = world(d, &[id]);
    takeoff(&mut w, d, inp(0, 0, JUMP), IDLE);
    let v1 = f0(&w).vy;
    tick(&mut w, d, IDLE);
    row("gravity", json!(r3(f0(&w).vy - v1)), json!(r3(src("gravity"))));
    // Air: full hop holding forward to the apex; air accel is the first drift tick from rest.
    let mut w = world(d, &[id]);
    takeoff(&mut w, d, inp(0, 0, JUMP), inp(0, 0, JUMP));
    tick(&mut w, d, inp(127, 0, JUMP));
    row("air accel", json!(r3(f0(&w).vx)), json!(r3(src("airAccel"))));
    for _ in 0..40 {
        tick(&mut w, d, inp(127, 0, 0));
    }
    row("air speed", json!(r2(f0(&w).vx)), json!(r2(src("air"))));
    let mut top = 0f64;
    while !f0(&w).ground {
        tick(&mut w, d, IDLE);
        top = top.max(f0(&w).vy);
    }
    row("fall", json!(r2(top)), json!(r2(src("fall"))));
    let mut w = world(d, &[id]);
    takeoff(&mut w, d, inp(0, 0, JUMP), inp(0, 0, JUMP));
    to_apex(&mut w, d);
    tick(&mut w, d, inp(0, -127, 0));
    row("fast fall", json!(r2(f0(&w).vy)), json!(r2(src("fastFall"))));
    // Jumps: heights against source × 5, airtimes against UFD.
    let j = &r["jumps"];
    let mut w = world(d, &[id]);
    let y0 = takeoff(&mut w, d, inp(0, 0, JUMP), inp(0, 0, JUMP));
    let (fh, fh_air) = flight(&mut w, d, y0);
    let mut w = world(d, &[id]);
    let y0 = takeoff(&mut w, d, inp(0, 0, JUMP), IDLE);
    let (sh, sh_air) = flight(&mut w, d, y0);
    let mut w = world(d, &[id]);
    takeoff(&mut w, d, inp(0, 0, JUMP), inp(0, 0, JUMP));
    to_apex(&mut w, d);
    let y0 = f0(&w).y;
    tick(&mut w, d, inp(0, 0, JUMP));
    let (aj, _) = flight(&mut w, d, y0);
    row("full hop px", json!(r2(fh)), json!(r2(j["full"].as_f64().unwrap() * SCALE)));
    row("short hop px", json!(r2(sh)), json!(r2(j["short"].as_f64().unwrap() * SCALE)));
    row("air jump px", json!(r2(aj)), json!(r2(j["air"].as_f64().unwrap() * SCALE)));
    row("full hop airtime", json!(fh_air), r["airtime"]["full"].clone());
    row("short hop airtime", json!(sh_air), r["airtime"]["short"].clone());
    for (slot, dir) in AERIALS {
        let input = aerial_input(dir, 1.);
        let (lag, ac) = autocancels(d, id, slot, input);
        row(&format!("{slot} landing lag"), json!(lag), r["landing"][slot].clone());
        row(&format!("{slot} autocancel"), json!(ac), r["autocancel"][slot].clone());
    }
    rows
}

#[test]
#[ignore]
fn calibrate() {
    let d = data();
    let only = std::env::var("CALIBRATE_FIGHTER").unwrap_or_default();
    let ids: Vec<&str> = FIGHTERS.into_iter().filter(|f| only.is_empty() || only == *f).collect();
    assert!(!ids.is_empty(), "calibration fighters are {FIGHTERS:?}");
    let mut lines = String::new();
    for id in &ids {
        println!("\n### {id}: movement, engine vs source × {SCALE} (airtime: UFD frames)\n");
        println!("| Metric | Engine | Source | Δ |\n|---|---|---|---|");
        for (name, eng, src) in movement(&d, id) {
            let delta = eng.as_f64().zip(src.as_f64()).map_or(String::new(), |(a, b)| format!("{}", r2(a - b)));
            println!("| {name} | {eng} | {src} | {delta} |");
        }
        for mv in moves() {
            let v = launches(&d, id, &mv);
            lines += &format!("{v}\n");
        }
    }
    let path = std::env::var("CALIBRATE_OUT").unwrap_or_else(|_| "target/calibration.jsonl".into());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
    std::fs::write(&path, lines).unwrap();
    println!("\nlaunch rows: {}", path.display());
}
