//! Feel report: per-fighter jump, gait, air, landing, launch and KO numbers to tune physics against.
//!
//! Opt-in and ignored so it never runs in `just test`; `just feel [fighter]` runs it. Each fighter prints one
//! `FEEL <fighter> {json}` line, then two markdown tables. `FEEL_OUT=<dir>` also writes `baseline.jsonl` and
//! `baseline.md` there (`just feel-baseline`). Metrics are defined in `docs/balance/feel/README.md`.
//!
//! Measurement only: raw `Input`s through `step_world`, plus `combat::apply_hit` for the synthetic launches. Every
//! scenario runs on Arena's main block with its pass-through platforms removed, so tall jumps and launches are not
//! caught early.
mod common;
use serde_json::{Value, json};
use sim::{
    GameData, Input, PlayerConfig, World, combat,
    data::Hitbox,
    input::{ATTACK, GRAB, JUMP},
    world::{Fighter, Gait, State},
};

const CENTRE: f64 = 640.;
const DUMMY: &str = "brawler";
const KO_CAP: u32 = 300;
const PERCENTS: [f64; 4] = [0., 50., 100., 150.];
const AERIALS: [(&str, (i8, i8)); 5] =
    [("nair", (0, 0)), ("fair", (1, 0)), ("bair", (-1, 0)), ("uair", (0, 1)), ("dair", (0, -1))];

fn inp(x: i8, y: i8, buttons: u8) -> Input {
    Input { stick: (x, y), buttons, ..Default::default() }
}
fn astick(x: i8, y: i8) -> Input {
    Input { attack_stick: (x, y), ..Default::default() }
}
const IDLE: Input = Input { stick: (0, 0), buttons: 0, attack_stick: (0, 0) };
fn tick(w: &mut World, d: &GameData, i: Input) {
    common::step_draining(w, d, &[i]);
}
fn r2(v: f64) -> Value {
    json!((v * 100.).round() / 100.)
}
fn skip(reason: &str) -> Value {
    json!({ "value": null, "reason": reason })
}
fn f0(w: &World) -> &Fighter {
    &w.fighters[0]
}

/// Arena without its pass-through platforms.
fn data() -> GameData {
    let mut d = common::data();
    d.stages.iter_mut().filter(|s| s.id == "arena").for_each(|s| s.platforms.clear());
    d
}
fn world(d: &GameData, ids: &[&str]) -> World {
    let mut w = common::sandbox(d, "arena", ids.iter().map(|id| PlayerConfig::human(id)).collect());
    for f in &mut w.fighters {
        (f.vx, f.vy) = (0., 0.);
    }
    w.fighters[0].x = CENTRE;
    w.fighters[0].face = 1.;
    w
}

struct Flight {
    height: f64,
    apex: u32,
    air: Option<u32>,
}
impl Flight {
    fn json(&self) -> Value {
        json!({ "height": r2(self.height), "apex": self.apex, "air": self.air })
    }
}
/// Steps through jumpsquat until airborne; that tick is flight frame 1.
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
fn fly(w: &mut World, d: &GameData, y0: f64, mut t: u32, input: impl Fn(u32) -> Input) -> Flight {
    let (mut top, mut apex) = (f0(w).y, t);
    while !f0(w).ground && t < 600 {
        tick(w, d, input(t));
        t += 1;
        if f0(w).y < top {
            (top, apex) = (f0(w).y, t);
        }
    }
    Flight { height: y0 - top, apex, air: f0(w).ground.then_some(t) }
}
fn full_hop(w: &mut World, d: &GameData, x: i8) -> f64 {
    takeoff(w, d, inp(x, 0, JUMP), inp(x, 0, JUMP))
}
/// Holds `hold` until the fighter stops rising.
fn to_apex(w: &mut World, d: &GameData, hold: Input) {
    for _ in 0..200 {
        if f0(w).vy >= 0. || f0(w).ground {
            break;
        }
        tick(w, d, hold);
    }
}
/// Ticks until `done`, up to `cap`; the 1-based tick count it took.
fn until(w: &mut World, d: &GameData, cap: u32, i: Input, done: impl Fn(&Fighter) -> bool) -> Option<u32> {
    (1..=cap).find(|_| {
        tick(w, d, i);
        done(f0(w))
    })
}

fn jumps(d: &GameData, id: &str) -> Value {
    let mut w = world(d, &[id]);
    let y0 = full_hop(&mut w, d, 0);
    let full = fly(&mut w, d, y0, 1, |_| IDLE);
    let mut w = world(d, &[id]);
    let y0 = takeoff(&mut w, d, inp(0, 0, JUMP), IDLE);
    let short = fly(&mut w, d, y0, 1, |_| IDLE);
    let mut w = world(d, &[id]);
    full_hop(&mut w, d, 0);
    to_apex(&mut w, d, IDLE);
    let air = if f0(&w).jumps == 0 {
        skip("no air jumps")
    } else {
        let y0 = f0(&w).y;
        tick(&mut w, d, inp(0, 0, JUMP));
        fly(&mut w, d, y0, 1, |_| IDLE).json()
    };
    json!({ "full": full.json(), "short": short.json(), "air": air, "shortRatio": r2(short.height / full.height) })
}

fn ground(d: &GameData, id: &str) -> Value {
    let s = &d.fighter(id).unwrap().stats;
    let mut w = world(d, &[id]);
    tick(&mut w, d, inp(127, 0, 0));
    for _ in 1..20 {
        tick(&mut w, d, IDLE);
    }
    let dash = f0(&w).x - CENTRE;
    let mut w = world(d, &[id]);
    let run95 = until(&mut w, d, 120, inp(127, 0, 0), |f| f.vx >= 0.95 * s.run_speed);
    let running = || {
        let mut w = world(d, &[id]);
        w.fighters[0].x = 340.;
        for _ in 0..40 {
            tick(&mut w, d, inp(127, 0, 0));
        }
        assert_eq!(f0(&w).gait, Gait::Run, "{id}: not running");
        w
    };
    let mut w = running();
    let x0 = f0(&w).x;
    let stop = until(&mut w, d, 200, IDLE, |f| f.vx.abs() < 0.1);
    let stop = json!({ "frames": stop, "distance": r2(f0(&w).x - x0) });
    let mut w = running();
    let face = until(&mut w, d, 60, inp(-127, 0, 0), |f| f.face < 0.);
    let mut w = running();
    let x0 = f0(&w).x;
    let cross = until(&mut w, d, 60, inp(-127, 0, 0), |f| f.vx <= 0.);
    let slide = f0(&w).x - x0;
    let full = until(&mut w, d, 120, inp(-127, 0, 0), |f| f.vx <= -0.95 * s.run_speed).zip(cross).map(|(a, b)| a + b);
    let turn = json!({ "face": face, "cross": cross, "full": full, "slide": r2(slide) });
    // Dash-back: a flick right held 3 ticks, then a one-tick flick left and the stick neutral for 19 more.
    let mut w = world(d, &[id]);
    for _ in 0..3 {
        tick(&mut w, d, inp(127, 0, 0));
    }
    let x0 = f0(&w).x;
    tick(&mut w, d, inp(-127, 0, 0));
    let mut window = u32::from(f0(&w).gait == Gait::DashTurn);
    for _ in 1..20 {
        tick(&mut w, d, IDLE);
        window += u32::from(f0(&w).gait == Gait::DashTurn);
    }
    let back = json!({ "distance": r2(x0 - f0(&w).x), "turn": window });
    json!({ "dash20": r2(dash), "run95": run95, "stop": stop, "turn": turn, "dashBack": back })
}

fn air(d: &GameData, id: &str) -> Value {
    let s = &d.fighter(id).unwrap().stats;
    let carry = |apex: Input| {
        let mut w = world(d, &[id]);
        w.fighters[0].x = 340.;
        full_hop(&mut w, d, 127);
        to_apex(&mut w, d, inp(127, 0, 0));
        let vx = f0(&w).vx;
        tick(&mut w, d, apex);
        for _ in 1..15 {
            tick(&mut w, d, IDLE);
        }
        json!({ "apexVx": r2(vx / s.air_speed), "retained15": r2(f0(&w).vx / s.air_speed) })
    };
    let mut w = world(d, &[id]);
    full_hop(&mut w, d, 0);
    to_apex(&mut w, d, IDLE);
    let ff = until(&mut w, d, 60, inp(0, -127, 0), |f| f.vy >= 0.95 * s.fast_fall_speed);
    json!({ "drift": carry(IDLE), "nair": carry(inp(0, 0, ATTACK)), "fastFall": ff })
}

/// Falls to the ground with the stick neutral, then counts ticks until actionable.
fn land_lag(w: &mut World, d: &GameData) -> Value {
    if until(w, d, 400, IDLE, |f| f.ground).is_none() {
        return skip("never landed");
    }
    let g = &d.global;
    if f0(w).actionable(g) {
        return json!(0);
    }
    until(w, d, 200, IDLE, |f| f.actionable(g)).map_or_else(|| skip("not actionable within 200 frames"), |n| json!(n))
}
fn landing(d: &GameData, id: &str) -> Value {
    let dropped = |state: State, height: f64| {
        let mut w = world(d, &[id]);
        let f = &mut w.fighters[0];
        f.set_state(state);
        (f.ground, f.surf, f.y) = (false, None, f.y - height);
        land_lag(&mut w, d)
    };
    let mut out = json!({ "soft": dropped(State::Air, 1.), "helpless": dropped(State::Helpless, 100.) });
    let mut w = world(d, &[id]);
    full_hop(&mut w, d, 0);
    out["hard"] = land_lag(&mut w, d);
    for (slot, (x, y)) in AERIALS {
        let mut w = world(d, &[id]);
        full_hop(&mut w, d, 0);
        to_apex(&mut w, d, IDLE);
        tick(&mut w, d, if (x, y) == (0, 0) { inp(0, 0, ATTACK) } else { astick(x * 127, y * 127) });
        out[slot] = if f0(&w).mov.as_ref().map(|m| m.slot.as_str()) == Some(slot) {
            land_lag(&mut w, d)
        } else {
            skip("aerial did not start")
        };
    }
    out
}

fn hitbox(angle: f64) -> Hitbox {
    serde_json::from_value(json!({ "x": 0, "y": 0, "r": 10, "damage": 12, "bkb": 40, "kbg": 1.0, "angle": angle }))
        .unwrap()
}
fn launch(d: &GameData, id: &str, angle: f64, pct: f64) -> Value {
    let mut w = world(d, &[id, DUMMY]);
    w.fighters[1].x = 360.;
    w.fighters[0].dmg = pct;
    let (x0, y0) = (f0(&w).x, f0(&w).y);
    let src = combat::HitSource {
        attacker: 1,
        direction: 1.,
        point: (x0 - 20., y0 - 40.),
        projectile: true,
        label: Some("feel".into()),
    };
    combat::apply_hit(&mut w, d, 0, &hitbox(angle), src);
    let f = f0(&w);
    let (hitlag, hitstun, speed, tumble) = (f.lag, f.hitstun, f.vx.hypot(f.vy), f.tumble);
    let (mut top, mut dx, mut died, mut landed) = (y0, None, false, None);
    for t in 1..=900 {
        let prev = f0(&w).state;
        tick(&mut w, d, IDLE);
        let f = f0(&w);
        if f.state == State::Dead {
            // Same launch-KO rule as the KO percents.
            died = launch_ko(d, &w, f, prev, landed.is_none());
            break;
        }
        top = top.min(f.y);
        if dx.is_none() && f.state != State::Hitstun {
            dx = Some(r2(f.x - x0));
        }
        if landed.is_none() && t > 1 && f.ground && f.surf == Some(0) {
            landed = Some(t);
        }
        if landed.is_some() && dx.is_some() {
            break;
        }
    }
    json!({
        "pct": pct as u32, "hitlag": hitlag, "hitstun": hitstun, "speed": r2(speed), "tumble": tumble, "dx": dx,
        "apex": r2(y0 - top), "died": died, "landed": landed,
    })
}

/// A death that counts as a launch KO: a side or top blast line crossed before touching ground or a ledge since
/// the hit, or the bottom line crossed in hitstun (a spike). Anything else is a ring-out.
fn launch_ko(d: &GameData, w: &World, f: &Fighter, prev: State, untouched: bool) -> bool {
    let b = &d.stages[w.stage].blast;
    if f.y > b.bottom { prev == State::Hitstun } else { untouched }
}
#[derive(Clone, Copy)]
enum Ko {
    Fsmash,
    Utilt,
    Bair,
    Fthrow,
}
#[derive(Clone, Copy, PartialEq)]
enum Out {
    Stay,
    /// Off the main block (hanging on a ledge, or dead) without a launch KO.
    Ring,
    /// See [`launch_ko`].
    Ko,
}
struct Trial {
    hit: bool,
    out: Out,
    /// Dummy launch speed on the first hit tick (0 for a grab).
    speed: f64,
}
/// The attacker stands `dx` left of a neutral dummy at centre (backs it for a back air) and plays the move; a back
/// air is a short hop with the aerial pressed `delay` ticks after the jump press (1 = during jumpsquat).
fn trial(d: &GameData, id: &str, mv: Ko, (dx, delay): (f64, u32), pct: f64, stop_on_hit: bool) -> Trial {
    let mut w = world(d, &[id, DUMMY]);
    let face = if matches!(mv, Ko::Bair) { -1. } else { 1. };
    (w.fighters[0].x, w.fighters[0].face) = (CENTRE - dx, face);
    (w.fighters[1].x, w.fighters[1].face, w.fighters[1].dmg) = (CENTRE, -1., pct);
    w.cfg.players[0].controls.stick_tilts = matches!(mv, Ko::Utilt);
    let fwd = (127. * face) as i8;
    let min_hold = d.global.grab.min_hold;
    let miss = Trial { hit: false, out: Out::Stay, speed: 0. };
    let mut touched = false;
    for t in 0..900u32 {
        let (prev, launched) = (w.fighters[1].state, !touched);
        let a = f0(&w);
        let i = match (mv, t) {
            (Ko::Fsmash, 0) => astick(fwd, 0),
            (Ko::Utilt, 0) => astick(0, 127),
            (Ko::Bair, 0) => inp(0, 0, JUMP),
            (Ko::Bair, t) if t == delay => astick(-fwd, 0),
            (Ko::Fthrow, 0) => inp(0, 0, GRAB),
            (Ko::Fthrow, _) if a.state == State::Hold && a.t > min_hold => inp(fwd, 0, 0),
            _ => IDLE,
        };
        tick(&mut w, d, i);
        let slot = match mv {
            Ko::Fsmash => "fsmash",
            Ko::Utilt => "utilt",
            Ko::Bair => "bair",
            Ko::Fthrow => "grab",
        };
        let pressed = if matches!(mv, Ko::Bair) { delay } else { 0 };
        if t == pressed && f0(&w).mov.as_ref().is_none_or(|m| m.slot != slot) && f0(&w).state != State::Jumpsquat {
            // Pressed too late (a back air after landing) or the move didn't start as intended.
            return miss;
        }
        let v = &w.fighters[1];
        if v.state == State::Dead {
            let ko = launch_ko(d, &w, v, prev, launched);
            return Trial { hit: true, out: if ko { Out::Ko } else { Out::Ring }, speed: 0. };
        }
        if v.dmg > pct || v.state == State::Grabbed {
            let speed = v.vx.hypot(v.vy);
            if stop_on_hit {
                return Trial { hit: true, out: Out::Stay, speed };
            }
            if v.state == State::Ledge {
                return Trial { hit: true, out: Out::Ring, speed };
            }
            touched |= v.ground && v.state != State::Grabbed;
            if v.ground && v.actionable(&d.global) && t > 30 {
                return Trial { hit: true, out: Out::Stay, speed };
            }
        } else if t > 120 {
            break;
        }
    }
    Trial { hit: w.fighters[1].dmg > pct, ..miss }
}
/// Lowest percent in `0..=KO_CAP` where `yes` holds, assuming it stays true above.
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
fn ko_at(d: &GameData, id: &str, mv: Ko, setup: (f64, u32)) -> Value {
    let out = |p: u32| trial(d, id, mv, setup, f64::from(p), false).out;
    let (dx, delay) = setup;
    let delay = matches!(mv, Ko::Bair).then_some(delay);
    let ko = lowest(|p| out(p) == Out::Ko);
    // The first percent that takes the dummy off the block counts as a ring-out unless it is already a launch KO.
    let ring = lowest(|p| out(p) != Out::Stay).filter(|p| out(*p) == Out::Ring);
    let mut v = json!({ "percent": ko, "ringOut": ring, "range": dx, "delay": delay });
    if ko.is_none() {
        v["reason"] = json!(format!("no launch KO below {KO_CAP}%"));
    }
    v
}
/// KO percent at the outermost connecting spacing (`first`) and at the spacing / timing that launches hardest at
/// 100% (`best`).
fn ko_percent(d: &GameData, id: &str, mv: Ko) -> Value {
    let delays = if matches!(mv, Ko::Bair) { 1..=40 } else { 0..=0 };
    let (mut first, mut best): (Option<(f64, u32)>, Option<((f64, u32), f64)>) = (None, None);
    for delay in delays {
        for dx in (0..=150).rev().map(|k| f64::from(k) * 2.) {
            let t = trial(d, id, mv, (dx, delay), 100., true);
            if !t.hit {
                continue;
            }
            if first.is_none_or(|(f, _)| dx > f) {
                first = Some((dx, delay));
            }
            if best.is_none_or(|(_, s)| t.speed > s) {
                best = Some(((dx, delay), t.speed));
            }
        }
    }
    let (Some(first), Some((best, _))) = (first, best) else {
        return json!({ "first": { "percent": null, "reason": "never connects within 300px" }, "best": null });
    };
    let first_ko = ko_at(d, id, mv, first);
    let best_ko = if best == first { first_ko.clone() } else { ko_at(d, id, mv, best) };
    json!({ "first": first_ko, "best": best_ko })
}
/// Lowest percent (before the hit) at which the synthetic 45° hit KOs.
fn launch_death(d: &GameData, id: &str) -> Value {
    let dies = |p: u32| launch(d, id, 45., f64::from(p))["died"] == true;
    if !dies(KO_CAP) {
        return json!(null);
    }
    let (mut lo, mut hi) = (0, KO_CAP);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if dies(mid) { hi = mid } else { lo = mid + 1 }
    }
    json!(lo)
}

fn measure(d: &GameData, id: &str) -> Value {
    let launches = |angle| PERCENTS.iter().map(|p| launch(d, id, angle, *p)).collect::<Vec<_>>();
    json!({
        "fighter": id,
        "jumps": jumps(d, id),
        "ground": ground(d, id),
        "air": air(d, id),
        "landing": landing(d, id),
        "launch": { "angle45": launches(45.), "spike270": launches(270.), "death45": launch_death(d, id) },
        "ko": {
            "fsmash": ko_percent(d, id, Ko::Fsmash),
            "utilt": ko_percent(d, id, Ko::Utilt),
            "bair": ko_percent(d, id, Ko::Bair),
            "fthrow": ko_percent(d, id, Ko::Fthrow),
        },
    })
}

/// Compact cell text: numbers as-is, nulls (or skipped values) as "–".
fn cell(v: &Value) -> String {
    match v {
        Value::Null => "–".into(),
        Value::Object(o) if o.contains_key("reason") => "–".into(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        v => v.to_string(),
    }
}
fn cells(v: &Value, ptrs: &[&str]) -> String {
    ptrs.iter().map(|p| cell(v.pointer(p).unwrap_or(&Value::Null))).collect::<Vec<_>>().join(" / ")
}
fn tables(rows: &[Value]) -> String {
    let mut s = String::from(
        "### Movement\n\n| Fighter | Full hop h / apex / air | Short hop h / apex / air | SH:FH | Air jump h / apex / air \
         | Dash 20f px | Run 95% f | Stop f / px | Turn face / vx≤0 / full f, slide px | Dash-back 20f px / turn f \
         | Drift / nair kept | Fast fall f |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for r in rows {
        let j = |k: &str| {
            cells(r, &[&format!("/jumps/{k}/height"), &format!("/jumps/{k}/apex"), &format!("/jumps/{k}/air")])
        };
        s += &format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {}, {} | {} | {} | {} |\n",
            cell(&r["fighter"]).trim_matches('"'),
            j("full"),
            j("short"),
            cell(&r["jumps"]["shortRatio"]),
            j("air"),
            cell(&r["ground"]["dash20"]),
            cell(&r["ground"]["run95"]),
            cells(r, &["/ground/stop/frames", "/ground/stop/distance"]),
            cells(r, &["/ground/turn/face", "/ground/turn/cross", "/ground/turn/full"]),
            cell(&r["ground"]["turn"]["slide"]),
            cells(r, &["/ground/dashBack/distance", "/ground/dashBack/turn"]),
            cells(r, &["/air/drift/retained15", "/air/nair/retained15"]),
            cell(&r["air"]["fastFall"]),
        );
    }
    s += "\n### Landing to actionable (frames)\n\n| Fighter | Soft | Hard | Helpless | Nair | Fair | Bair | Uair | Dair |\n\
          |---|---|---|---|---|---|---|---|---|\n";
    for r in rows {
        let l = &r["landing"];
        let v: Vec<_> =
            ["soft", "hard", "helpless", "nair", "fair", "bair", "uair", "dair"].map(|k| cell(&l[k])).into();
        s += &format!("| {} | {} |\n", cell(&r["fighter"]).trim_matches('"'), v.join(" | "));
    }
    s += "\n### Launch (12% / 40 BKB / 1.0 KBG) and KO percents\n\n| Fighter | 45° speed @0/50/100/150 \
          | 45° hitstun @0/50/100/150 | 45° dx at hitstun end @100 | 45° KO% | Spike speed / hitstun @100 \
          | F-smash KO% | U-tilt KO% | SH bair KO% | F-throw KO% | Ring-out % F-smash / U-tilt / bair / F-throw \
          |\n|---|---|---|---|---|---|---|---|---|---|---|\n";
    for r in rows {
        let l = r["launch"]["angle45"].as_array().unwrap();
        let col = |k: &str| l.iter().map(|x| cell(&x[k])).collect::<Vec<_>>().join(" / ");
        let dies = cell(&r["launch"]["death45"]);
        let spike = &r["launch"]["spike270"][2];
        let ko = |k: &str| {
            let one = |v: &Value| match &v["percent"] {
                Value::Null if v["range"].is_null() => "no hit".to_string(),
                Value::Null => "none".into(),
                p => p.to_string(),
            };
            let (first, best) = (one(&r["ko"][k]["first"]), one(&r["ko"][k]["best"]));
            if first == best || r["ko"][k]["best"].is_null() { first } else { format!("{first} / {best}") }
        };
        let ring = ["fsmash", "utilt", "bair", "fthrow"]
            .map(|k| {
                let one = |v: &Value| cell(&v["ringOut"]);
                let (first, best) = (one(&r["ko"][k]["first"]), one(&r["ko"][k]["best"]));
                if first == best || r["ko"][k]["best"].is_null() { first } else { format!("{first}/{best}") }
            })
            .join(" / ");
        s += &format!(
            "| {} | {} | {} | {} | {} | {} / {} | {} | {} | {} | {} | {} |\n",
            cell(&r["fighter"]).trim_matches('"'),
            col("speed"),
            col("hitstun"),
            cell(&l[2]["dx"]),
            dies,
            cell(&spike["speed"]),
            cell(&spike["hitstun"]),
            ko("fsmash"),
            ko("utilt"),
            ko("bair"),
            ko("fthrow"),
            ring,
        );
    }
    s
}

#[test]
#[ignore]
fn feel_report() {
    let d = data();
    let only = std::env::var("FEEL_FIGHTER").unwrap_or_default();
    let rows: Vec<Value> = d
        .fighters
        .iter()
        .map(|f| f.id.as_str())
        .filter(|id| only.is_empty() || only == *id)
        .map(|id| {
            let r = measure(&d, id);
            println!("FEEL {id} {r}");
            r
        })
        .collect();
    assert!(!rows.is_empty(), "unknown fighter {only:?}");
    let table = tables(&rows);
    println!("\n{table}");
    if let Some(dir) = std::env::var_os("FEEL_OUT").filter(|d| !d.is_empty()) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lines: String = rows.iter().map(|r| format!("{r}\n")).collect();
        std::fs::write(dir.join("baseline.jsonl"), lines).unwrap();
        std::fs::write(
            dir.join("baseline.md"),
            format!("# Feel report baseline\n\nGenerated by `just feel-baseline`.\n\n{table}"),
        )
        .unwrap();
    }
}
