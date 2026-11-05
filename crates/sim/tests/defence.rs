//! Authored dodge / tech / getup intangibility windows, dodge staling, shield drop lag and the legacy data shape.
//! Frames in test names are Ultimate-style (1 = the tick the state starts); the data uses the state clock (0).
mod common;
use sim::{
    GameData, Input, World, combat,
    input::{ATTACK, JUMP, SHIELD},
    moves, physics,
    world::State,
};

fn world(d: &GameData) -> World {
    let mut w = common::fighters(d, "brawler", &["brawler"]);
    w.fighters[1].x = w.fighters[0].x + 600.;
    w
}
fn step(w: &mut World, d: &GameData, stick: (i8, i8), buttons: u8) {
    common::step_draining(w, d, &[Input { stick, buttons, ..Default::default() }]);
}
/// Player 1 jabs player 0 where it stands right now (state frozen): does the hit land?
fn connects(w: &World, d: &GameData) -> bool {
    let mut w = w.clone();
    let mut a = w.fighters[1].clone();
    a.set_state(State::Idle);
    moves::start_move(&mut w, d, 1, &mut a, "jab");
    w.fighters[1] = a;
    let v = combat::hurt(d, &w.fighters[0]);
    for _ in 0..30 {
        if let Some(b) = combat::active_boxes_at(d, &w.fighters[1], w.frame).first() {
            let a = &mut w.fighters[1];
            a.x += (v.x1 + v.x2) / 2. - b.x;
            a.y += (v.y1 + v.y2) / 2. - b.y;
            combat::hit_pass(&mut w, d);
            return w.hits.iter().any(|h| h.victim == 0 && !h.shielded);
        }
        let mut a = w.fighters[1].clone();
        moves::st_attack(&mut w, d, 1, &mut a, Default::default(), 0.);
        w.fighters[1] = a;
        w.frame += 1;
    }
    panic!("jab never became active");
}
/// Steps (neutral input) until player 0 is on `frame` of `state`.
fn at(w: &World, d: &GameData, state: State, frame: u32) -> World {
    let mut w = w.clone();
    while w.fighters[0].t + 1 < frame {
        assert_eq!(w.fighters[0].state, state);
        step(&mut w, d, (0, 0), 0);
    }
    assert_eq!((w.fighters[0].state, w.fighters[0].t + 1), (state, frame));
    w
}
fn shield_then(d: &GameData, stick: (i8, i8)) -> World {
    let mut w = world(d);
    step(&mut w, d, (0, 0), SHIELD);
    assert_eq!(w.fighters[0].state, State::Shield);
    step(&mut w, d, stick, SHIELD);
    w
}
fn airborne(d: &GameData, stick: (i8, i8)) -> World {
    let mut w = world(d);
    let f = &mut w.fighters[0];
    f.set_state(State::Air);
    f.ground = false;
    f.surf = None;
    f.y -= 600.;
    step(&mut w, d, stick, SHIELD);
    assert_eq!(w.fighters[0].state, State::Airdodge);
    w
}
/// Which state frames (1-based) of the current state are intangible, until it ends.
fn intangible(w: &World, d: &GameData) -> (Vec<u32>, u32) {
    let mut w = w.clone();
    let state = w.fighters[0].state;
    let mut out = vec![];
    while w.fighters[0].state == state {
        if !w.fighters[0].hittable() {
            out.push(w.fighters[0].t + 1);
        }
        let last = w.fighters[0].t + 1;
        step(&mut w, d, (0, 0), 0);
        if w.fighters[0].state != state {
            return (out, last);
        }
    }
    unreachable!()
}

#[test]
fn jab_probe_hits_a_standing_fighter() {
    let d = common::data();
    assert!(connects(&world(&d), &d));
}

#[test]
fn dodges_are_hittable_before_and_after_their_intangibility() {
    let d = common::data();
    let cases = [
        (State::Roll, shield_then(&d, (127, 0)), [2, 8, 25]),
        (State::Spot, shield_then(&d, (0, -127)), [2, 8, 25]),
        (State::Airdodge, airborne(&d, (0, 0)), [2, 8, 35]),
        (State::Airdodge, airborne(&d, (127, 0)), [2, 8, 30]),
    ];
    for (state, w, [early, mid, late]) in cases {
        assert_eq!(w.fighters[0].state, state);
        assert!(connects(&at(&w, &d, state, early), &d), "{state:?} frame {early}");
        assert!(!connects(&at(&w, &d, state, mid), &d), "{state:?} frame {mid}");
        assert!(connects(&at(&w, &d, state, late), &d), "{state:?} frame {late}");
    }
}

#[test]
fn dodge_windows_follow_the_data() {
    let d = common::data();
    let g = &d.global.dodge;
    for (w, win) in [
        (shield_then(&d, (127, 0)), &g.roll),
        (shield_then(&d, (0, -127)), &g.spot),
        (airborne(&d, (0, 0)), &g.air),
        (airborne(&d, (127, 0)), &g.dir_air),
    ] {
        let (inv, last) = intangible(&w, &d);
        assert_eq!(inv, (win.invuln_start + 1..=win.invuln_end + 1).collect::<Vec<_>>());
        assert_eq!(last, win.frames, "the state's last frame is clock frames − 1");
    }
}

#[test]
fn repeated_dodges_stale_and_recover_after_the_window() {
    let d = common::data();
    let s = &d.global.dodge.staling;
    assert!(!s.steps.is_empty());
    let mut w = world(&d);
    let spot = |w: &mut World| {
        step(w, &d, (0, 0), SHIELD);
        step(w, &d, (0, -127), SHIELD);
        assert_eq!(w.fighters[0].state, State::Spot);
        let (inv, last) = intangible(w, &d);
        while w.fighters[0].state == State::Spot {
            step(w, &d, (0, 0), 0);
        }
        (inv.len(), last)
    };
    let fresh = spot(&mut w);
    let first = spot(&mut w);
    let second = spot(&mut w);
    assert!(first.0 < fresh.0 && first.1 > fresh.1, "{fresh:?} → {first:?}");
    assert!(second.0 < first.0 && second.1 > first.1, "{first:?} → {second:?}");
    let full = s.steps.last().unwrap();
    for _ in 0..s.steps.len() {
        spot(&mut w);
    }
    let capped = spot(&mut w);
    let win = &d.global.dodge.spot;
    assert_eq!(capped.0 as f64, (f64::from(win.invuln_len()) * full.invuln).round());
    assert_eq!(capped.1 as f64, (f64::from(win.frames) * full.frames).round());
    common::run_draining(&mut w, &d, s.window as usize + 1, &[Input::default()]);
    assert_eq!(spot(&mut w), fresh, "a dodge after the window is fresh");
    // Air dodges share the counter; getup rolls neither stale nor count.
    let mut w = world(&d);
    spot(&mut w);
    let f = &mut w.fighters[0];
    f.set_state(State::Air);
    f.ground = false;
    f.surf = None;
    f.y -= 600.;
    step(&mut w, &d, (0, 0), SHIELD);
    let air = &d.global.dodge.air;
    assert!(intangible(&w, &d).0.len() < air.invuln_len() as usize);
    assert_eq!(w.fighters[0].dodge_stale, 1);
}

#[test]
fn shield_drop_has_its_lag() {
    let d = common::data();
    assert_eq!(d.global.shield.drop_lag, 11);
    let mut w = world(&d);
    step(&mut w, &d, (0, 0), SHIELD);
    step(&mut w, &d, (0, 0), 0);
    assert_eq!((w.fighters[0].state, w.fighters[0].sd.kind.as_str()), (State::Landing, "shield"));
    let mut ticks = 0;
    while !w.fighters[0].actionable(&d.global) {
        step(&mut w, &d, (0, 0), 0);
        ticks += 1;
    }
    assert_eq!(ticks, d.global.shield.drop_lag);
}

#[test]
fn tech_and_getups_follow_their_windows() {
    let d = common::data();
    // Tech in place: a tumble landing with a fresh shield press.
    let mut w = world(&d);
    let mut f = w.fighters[0].clone();
    f.set_state(State::Hitstun);
    f.tumble = true;
    f.last_sp = w.frame as i64;
    physics::on_land(&mut w, &d, 0, &mut f, 20.);
    w.fighters[0] = f;
    assert_eq!(w.fighters[0].state, State::Tech);
    let t = &d.global.tech;
    assert!(connects(&w, &d), "tech frame 1");
    let (inv, last) = intangible(&w, &d);
    assert_eq!(inv, (t.invuln_start + 1..=t.invuln_end + 1).collect::<Vec<_>>());
    assert_eq!(last, t.frames);
    // Neutral getup and getup attack out of knockdown.
    let k = &d.global.knockdown;
    for (buttons, win) in [(JUMP, &k.getup), (ATTACK, &k.getup_attack)] {
        let mut w = world(&d);
        let f = &mut w.fighters[0];
        f.set_state(State::Knockdown);
        f.t = k.min_frames + 1;
        step(&mut w, &d, (0, 0), buttons);
        assert_eq!(w.fighters[0].state, State::Getup);
        assert!(connects(&w, &d), "getup frame 1");
        let (inv, _) = intangible(&w, &d);
        assert_eq!(inv, (win.invuln_start + 1..=win.invuln_end + 1).collect::<Vec<_>>());
    }
}

/// The pre-window global shape still loads, as invulnerability from the entry tick, frame for frame.
#[test]
fn legacy_flat_timings_load_frame_exact() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let (mut sources, errors) = sim::data::sources(root);
    assert!(errors.is_empty(), "{errors:?}");
    let s = sources.iter_mut().find(|s| s.path == "global.json").unwrap();
    let mut g: serde_json::Value = serde_json::from_str(&s.text).unwrap();
    g["dodge"] = serde_json::json!({
        "rollInvuln": 31, "rollFrames": 30, "rollSpeed": 7, "rollMoveStart": 3, "rollMoveEnd": 18,
        "spotInvuln": 16, "spotFrames": 26, "airDodgeInvuln": 28, "airDodgeFrames": 30, "airDodgeLandingLag": 4,
        "dirAirDodgeInvuln": 24, "dirAirDodgeFrames": 34, "dirAirDodgeSpeed": 10, "dirAirDodgeDecay": 0.88,
        "dirAirDodgeDecayFrames": 18, "dirAirDodgeLandingLag": 10
    });
    g["tech"] = serde_json::json!({ "window": 12, "invuln": 20, "frames": 24, "lockout": 40 });
    let k = g["knockdown"].as_object_mut().unwrap();
    k.remove("getup");
    k.remove("getupAttack");
    for (key, n) in [("getupFrames", 24), ("getupInvuln", 22), ("attackFrames", 32), ("attackInvuln", 14)] {
        k.insert(key.into(), n.into());
    }
    for key in ["climb", "attack", "roll", "jump"] {
        let c = g["ledge"][key].as_object_mut().unwrap();
        let end = c.remove("invulnStart").and(c.remove("invulnEnd")).unwrap().as_u64().unwrap();
        c.insert("inv".into(), (end + 1).into());
    }
    s.text = g.to_string();
    let (legacy, errors) = sim::data::load(sources).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    let mut pinned = common::data();
    common::reference_defence(&mut pinned);
    assert_eq!(legacy.global.dodge, pinned.global.dodge);
    assert_eq!(legacy.global.tech, pinned.global.tech);
    assert_eq!(legacy.global.knockdown, pinned.global.knockdown);
    assert_eq!(legacy.global.ledge, pinned.global.ledge);
    // Rolls were invulnerable from the entry tick, one tick past their end.
    let mut w = shield_then(&legacy, (127, 0));
    assert_eq!(w.fighters[0].state, State::Roll);
    for t in 0..=31 {
        assert_eq!(w.fighters[0].hittable(), t == 31, "tick {t}");
        step(&mut w, &legacy, (0, 0), 0);
    }
}
