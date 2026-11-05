//! Stratus (SSBU Cloud) reference behavior, Limit resource and raw-input coverage.
mod common;

use sim::world::State;
use sim::{Input, PlayerConfig, World, moves, step_world};

fn sandbox(d: &sim::GameData, fighters: &[&str]) -> World {
    let players = fighters.iter().copied().map(PlayerConfig::human).collect();
    common::sandbox(d, "arena", players)
}

fn tick(w: &mut World, d: &sim::GameData, input: Input) {
    step_world(w, d, &[input]);
    w.fx.clear();
    w.hits.clear();
    w.acts.clear();
}

/// Step only the fighter's move: no opponent AI, hitlag or incidental hits can blur frame boundaries.
fn step_move(w: &mut World, d: &sim::GameData, input: sim::input::Intent) {
    let mut f = w.fighters[0].clone();
    moves::st_attack(w, d, 0, &mut f, input, input.dir());
    w.fighters[0] = f;
    w.frame += 1;
}

fn start(d: &sim::GameData, slot: &str) -> World {
    let mut w = sandbox(d, &["stratus", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].face = 1.;
    w.fighters[0].charges[0] = 0;
    w.fighters[1].x = 1300.;
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, d, 0, &mut f, slot);
    w.fighters[0] = f;
    w
}

/// Start a move while keeping the Limit state `start_move` left behind.
fn begin(w: &mut World, d: &sim::GameData, slot: &str) {
    let mut f = w.fighters[0].clone();
    f.set_state(State::Idle);
    moves::start_move(w, d, 0, &mut f, slot);
    w.fighters[0] = f;
}

#[track_caller]
fn phase_name(d: &sim::GameData, w: &World) -> Option<String> {
    let m = w.fighters[0].mov.as_ref()?;
    Some(moves::timeline(d, &w.fighters[0])[m.phase].name.clone())
}

#[track_caller]
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn len(phases: &[sim::data::Phase]) -> u32 {
    phases.iter().map(|p| p.frames.unwrap_or(0)).sum()
}

#[test]
fn package_identity_and_slots() {
    let d = common::data();
    let f = d.fighter("stratus").expect("stratus package");
    assert_eq!(f.id, "stratus");
    assert_eq!(f.name, "Stratus");
    assert_eq!(f.extends.as_deref(), Some("base"));
    for slot in [
        "jab", "ftilt", "utilt", "dtilt", "dashatk", "fsmash", "usmash", "dsmash", "nair", "fair", "bair", "uair",
        "dair", "grab", "dashgrab", "nspec", "sspec", "uspec", "dspec",
    ] {
        assert!(f.moves.get(slot).is_some(), "missing {slot}");
    }
    assert!(f.moves.zair.is_none(), "Cloud has no tether grab");
    for slot in ["f", "b", "u", "d"] {
        let t = match slot {
            "f" => &f.throws.f,
            "b" => &f.throws.b,
            "u" => &f.throws.u,
            _ => &f.throws.d,
        };
        assert!(t.release < t.total, "{slot} throw release inside its total");
    }
}

#[test]
fn limit_is_a_one_use_pool_with_no_recharge() {
    let d = common::data();
    let f = d.fighter("stratus").unwrap();
    assert_eq!(f.charges.len(), 1);
    let limit = &f.charges[0];
    assert_eq!(limit.id, "limit");
    assert_eq!((limit.uses, limit.recharge), (1, 0), "spent until charged or respawned");
    assert_eq!(sim::charges::ticks_per_use(limit), 1);
}

/// Absolute hit windows, damage and launch fields come from the pinned scripts; the engine phase frame is
/// source frame minus one. Groups distinguish separate hits while continuous swings preserve contact across phase splits.
fn windows(phases: &[sim::data::Phase]) -> Vec<(u32, u32, f64)> {
    let mut t = 0;
    let mut out = vec![];
    for p in phases {
        for h in &p.hitboxes {
            out.push((t + h.start, t + h.end.unwrap_or(p.frames.unwrap_or(0)), h.damage));
        }
        t += p.frames.unwrap_or(0);
    }
    out
}

#[test]
fn reference_windows_and_damage_match_the_ledger() {
    let d = common::data();
    let f = d.fighter("stratus").unwrap();

    let fsmash = windows(&f.moves.fsmash.phases);
    assert_eq!(
        fsmash,
        vec![
            (18, 18, 3.0),
            (18, 18, 3.0),
            (18, 18, 3.0),
            (18, 18, 3.0),
            (23, 23, 3.0),
            (23, 23, 2.0),
            (23, 23, 4.0),
            (27, 27, 13.0),
            (27, 27, 13.0),
            (27, 27, 12.0),
            (27, 27, 12.0),
            (27, 27, 12.0),
        ],
        "source 19/24/28 -> phase 18/23/27"
    );
    assert_eq!(f.moves.fsmash.phases[0].charge.as_ref().unwrap().frame, 4, "UFD charge hold 5");

    let ftilt = &f.moves.ftilt.phases[0].hitboxes[0];
    close(ftilt.damage, 11.0);
    assert_eq!(ftilt.kind, sim::data::HitType::Sakurai);
    assert_eq!((ftilt.start, ftilt.end), (8, Some(9)));

    let dtilt = windows(&f.moves.dtilt.phases);
    assert!(dtilt.iter().any(|&(s, e, d)| (s, e, d) == (6, 17, 7.0)), "7.0 knee box runs source 7-17");
    assert!(dtilt.iter().any(|&(s, e, d)| (s, e, d) == (6, 7, 6.0)), "6.0 root box clears early");

    let bair = windows(&f.moves.bair.phases);
    assert_eq!(bair, vec![(10, 10, 13.0), (11, 11, 13.0)]);
    assert_eq!(f.moves.bair.phases[2].hitboxes[0].dir, sim::data::LaunchDir::Back);

    let fair = windows(&f.moves.fair.phases);
    assert!(fair.contains(&(17, 18, 14.0)));
    assert!(fair.iter().any(|&(_, _, d)| d == 13.0), "the 13.0 arc box is the long one");
    assert!(fair.contains(&(19, 24, 11.0)), "11.0 late hit source 20-25");
    assert!(
        f.moves
            .fair
            .phases
            .iter()
            .flat_map(|p| &p.hitboxes)
            .any(|h| h.kind == sim::data::HitType::Spike && h.damage == 14.0)
    );

    let dair = windows(&f.moves.dair.phases);
    assert_eq!(dair, vec![(10, 12, 15.0), (10, 12, 13.0), (13, 38, 8.0)]);

    let uair = windows(&f.moves.uair.phases);
    assert!(uair.iter().any(|&(_, e, d)| (e, d) == (8, 11.0)));
    assert!(uair.iter().any(|&(s, e, d)| (s, e, d) == (9, 22, 8.0)), "8.0 tier source 10-23");

    let usmash = windows(&f.moves.usmash.phases);
    assert!(usmash.iter().any(|&(s, _, d)| (s, d) == (11, 13.0)));
    assert!(
        usmash.iter().any(|&(s, e, d)| (s, e, d) == (11, 13, 13.0))
            && usmash.iter().any(|&(s, e, d)| (s, e, d) == (14, 14, 13.0)),
        "the 13.0 rows persist into the 12.0/11.0 window: {usmash:?}"
    );
    assert!(usmash.iter().any(|&(s, _, d)| (s, d) == (14, 8.0)));

    let dsmash = windows(&f.moves.dsmash.phases);
    assert!(dsmash.iter().any(|&(s, e, d)| (s, e, d) == (7, 9, 3.0)));
    assert!(dsmash.iter().any(|&(s, e, d)| (s, e, d) == (20, 23, 11.0)));

    let nair = &f.moves.nair.phases[0].hitboxes;
    close(nair[0].damage, 8.0);
    assert!(nair.iter().any(|h| h.x < 0.), "the neutral-air sweep covers behind as well");

    let climb = &f.moves.uspec.phases;
    assert_eq!((climb[0].hitboxes[0].start, climb[0].hitboxes[0].end), (6, Some(6)));
    assert_eq!(windows(climb).iter().filter(|&&(_, _, d)| d == 3.0).count(), 3);
}

/// A stationary victim parked in range takes every window: the source re-creates the drag boxes, and each
/// engine phase entry clears contact history so the clean hit is not blocked.
#[test]
fn forward_smash_drags_into_the_clean_hit() {
    let d = common::data();
    let mut w = start(&d, "fsmash");
    let dx = 70.;
    let mut hits = vec![];
    let mut last = 0.;
    for _ in 0..70 {
        w.fighters[1].x = 640. + dx;
        w.fighters[1].y = 520.;
        w.fighters[1].vx = 0.;
        w.fighters[1].vy = 0.;
        step_world(&mut w, &d, &[Input::default(), Input::default()]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        if w.fighters[1].dmg > last + 1e-9 {
            hits.push(w.fighters[1].dmg - last);
            last = w.fighters[1].dmg;
        }
        if w.fighters[0].mov.is_none() {
            break;
        }
    }
    assert_eq!(hits.len(), 3, "drag, second drag and clean hit ({hits:?})");
    assert!(hits[0] >= 3.0 && hits[1] >= 2.0 && hits[2] >= 12.0, "{hits:?}");
    assert!(last >= 18.0, "total {last}");
}

#[test]
fn limit_cross_slash_connects_all_five_windows() {
    let d = common::data();
    let mut w = sandbox(&d, &["stratus", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].charges[0] = 1;
    w.fighters[1].x = 700.;
    w.fighters[1].y = 520.;
    begin(&mut w, &d, "sspec");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().variant, 1);
    let mut hits = vec![];
    let mut last = 0.;
    for _ in 0..70 {
        w.fighters[1].x = 700.;
        w.fighters[1].y = 520.;
        w.fighters[1].vx = 0.;
        w.fighters[1].vy = 0.;
        step_world(&mut w, &d, &[Input::default(), Input::default()]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        if w.fighters[1].dmg > last + 1e-9 {
            hits.push(w.fighters[1].dmg - last);
            last = w.fighters[1].dmg;
        }
        if w.fighters[0].mov.is_none() {
            break;
        }
    }
    assert!(hits.len() >= 5, "five slashes must connect ({hits:?})");
    assert!(last >= 25.9, "27% total (got {last})");
}

#[test]
fn finishing_touch_covers_the_front_and_the_rear() {
    let d = common::data();
    let run = |dx: f64| -> f64 {
        let mut w = sandbox(&d, &["stratus", "brawler"]);
        w.count = 0;
        w.fighters[0].x = 640.;
        w.fighters[0].y = 520.;
        w.fighters[0].charges[0] = 1;
        w.fighters[1].x = 640. + dx;
        w.fighters[1].y = 520.;
        begin(&mut w, &d, "dspec");
        for _ in 0..40 {
            w.fighters[1].x = 640. + dx;
            w.fighters[1].y = 520.;
            w.fighters[1].vx = 0.;
            w.fighters[1].vy = 0.;
            step_world(&mut w, &d, &[Input::default(), Input::default()]);
            w.fx.clear();
            w.hits.clear();
            w.acts.clear();
            if w.fighters[0].mov.is_none() {
                break;
            }
        }
        w.fighters[1].dmg
    };
    close(run(50.), 1.0);
    close(run(-50.), 1.0);
}

/// The rising slash's hitboxes are re-based inside its phase: the first is live on move frame 9 (source 10),
/// the follow-up window opens at move frame 19 (source 20), and the plunge keeps its hitbox until landing.
#[test]
fn climhazzard_move_frame_windows_and_lingering_plunge() {
    let d = common::data();
    let mut w = start(&d, "uspec");
    w.fighters[0].y = 300.;
    w.fighters[0].ground = false;
    w.fighters[0].surf = None;
    let len = sim::moves::timeline(&d, &w.fighters[0])[0].frames.unwrap();
    assert_eq!(len, 9, "thrust runs move frames 0-8 so the rise starts on source frame 10");
    let mut first_rise_hit = None;
    let mut follow = None;
    for _ in 0..80 {
        let Some(m) = w.fighters[0].mov.as_ref() else { break };
        let (elapsed, phase) = (m.elapsed, m.phase);
        if phase >= 1
            && first_rise_hit.is_none()
            && sim::combat::active_boxes_at(&d, &w.fighters[0], w.frame).iter().any(|b| b.hit.damage == 4.0)
        {
            first_rise_hit = Some(elapsed);
        }
        if phase >= 1 && follow.is_none() && elapsed >= 19 {
            step_move(&mut w, &d, sim::input::Intent { sp: true, ..Default::default() });
            if phase_name(&d, &w).as_deref() == Some("plunge") {
                follow = Some(elapsed);
            }
            continue;
        }
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    assert_eq!(first_rise_hit, Some(9), "first rising hit on move frame 9");
    assert!(follow.is_some_and(|f| f >= 19), "follow-up opens from source 20");

    // A fresh plunge keeps its attack live for more than one tick and lands into the 26-frame landing phase.
    // Clear of the Arena's platforms, so the plunge falls the whole way to the floor.
    let mut w = start(&d, "uspec");
    w.fighters[0].x = 1000.;
    w.fighters[0].y = 300.;
    w.fighters[0].ground = false;
    w.fighters[0].surf = None;
    for _ in 0..40 {
        if w.fighters[0].mov.is_none() {
            break;
        }
        if w.fighters[0].mov.as_ref().is_some_and(|m| m.elapsed >= 19) {
            break;
        }
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    step_move(&mut w, &d, sim::input::Intent { sp: true, ..Default::default() });
    assert_eq!(phase_name(&d, &w).as_deref(), Some("plunge"));
    for _ in 0..12 {
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    assert!(
        !sim::combat::active_boxes_at(&d, &w.fighters[0], w.frame).is_empty(),
        "the plunge hitbox persists while falling"
    );
    for _ in 0..8 {
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    let f = &mut w.fighters[0];
    f.y = 516.;
    f.vy = 6.;
    f.ground = false;
    f.surf = None;
    w.fighters[0] = f.clone();
    step_move(&mut w, &d, sim::input::Intent::default());
    assert_eq!(phase_name(&d, &w).as_deref(), Some("landHit"));
    let landing = d.fighter("stratus").unwrap().moves.uspec.phases.iter().find(|p| p.name == "landHit").unwrap();
    assert_eq!(landing.frames, Some(26), "UFD reports 26 frames of follow-up landing lag");
}

#[test]
fn blade_beam_spawns_on_phase_17() {
    let d = common::data();
    let mut w = start(&d, "nspec");
    for _ in 0..16 {
        step_move(&mut w, &d, sim::input::Intent::default());
        assert!(w.projs.is_empty(), "no wave before phase 17");
    }
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().t, 16);
    step_move(&mut w, &d, sim::input::Intent::default());
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().t, 17);
    assert_eq!(w.projs.len(), 1, "the wave spawns on source frame 18");
}

#[test]
fn durations_match_the_source_ledger() {
    let d = common::data();
    let f = d.fighter("stratus").unwrap();
    for (slot, total) in [
        ("ftilt", 35),
        ("utilt", 32),
        ("dtilt", 40),
        ("dashatk", 45),
        ("fsmash", 66),
        ("usmash", 45),
        ("dsmash", 49),
        ("nair", 38),
        ("fair", 53),
        ("bair", 42),
        ("uair", 44),
        ("dair", 64),
        ("grab", 36),
        ("dashgrab", 44),
    ] {
        let m = f.moves.get(slot).unwrap();
        assert_eq!(len(&m.phases), total, "{slot}");
        for (i, v) in m.variants.iter().enumerate() {
            assert_eq!(len(&v.phases), total, "{slot} variant {}", i + 1);
        }
    }
    assert_eq!(len(&f.moves.jab.phases[0..1]), 29, "jab 1");
    assert_eq!(len(&f.moves.jab.phases[1..2]), 32, "jab 2");
    assert_eq!(len(&f.moves.jab.phases[2..3]), 37, "jab 3");
    // Blade Beam: 53 frames on the ground, 60 in the air (UFD "53(60)"); Limit Blade Beam is 60.
    let nspec = &f.moves.nspec;
    assert_eq!(len(&nspec.phases[0..2]), 53, "grounded Blade Beam");
    assert_eq!(nspec.phases[0].frames.unwrap() + nspec.phases[2].frames.unwrap(), 60, "air Blade Beam");
    assert_eq!(len(&nspec.variants[0].phases), 60, "Limit Blade Beam");
    // Cross Slash chains at the earliest allowed input, so the animation tail of the first two slashes is
    // skipped: 12 + 14 + the third slash's 55 frames. Limit Cross Slash is one 60-frame sequence.
    assert_eq!(len(&f.moves.sspec.as_ref().unwrap().phases), 12 + 14 + 55);
    assert_eq!(len(&f.moves.sspec.as_ref().unwrap().variants[0].phases), 60);
    // Climhazzard: 10-frame thrust and a 24-frame rise (Limit: 10 + 36); the plunge adds 26 + 10.
    assert_eq!(f.moves.uspec.phases[0].frames, Some(9));
    assert_eq!(len(&f.moves.uspec.phases[1..5]), 24, "normal rise totals 24 frames");
    assert_eq!(len(&f.moves.uspec.variants[0].phases[1..5]), 36, "Limit rise totals 36 frames");
    assert_eq!(f.moves.uspec.phases[5].frames, Some(26), "plunge");
    assert_eq!(f.moves.uspec.phases[6].frames, Some(26), "follow-up landing lag");
    // Limit Charge is a 13-frame animation with a 7-frame tail after the charge frame; Finishing Touch is 84.
    assert_eq!(f.moves.dspec.phases[0].frames, Some(13));
    let finishing: u32 = f.moves.dspec.variants[0].phases.iter().map(|p| p.frames.unwrap_or(0)).sum();
    assert_eq!(finishing, 84);
    for (t, total) in [(&f.throws.f, 35), (&f.throws.b, 31), (&f.throws.u, 50), (&f.throws.d, 43)] {
        assert_eq!(t.total, total);
        assert!(t.release < total);
    }
}

/// Hold the down special to the source's 100-unit rate (0.3 units/frame since 7.0.0, 334 frames) and release.
#[test]
fn limit_charge_fills_at_the_source_rate() {
    let d = common::data();
    assert_eq!(d.fighter("stratus").unwrap().moves.dspec.phases[0].charge.as_ref().unwrap().max, 334);
    let hold = |ticks: u32| -> World {
        let mut w = start(&d, "dspec");
        for _ in 0..ticks + 20 {
            let held = w.fighters[0].mov.as_ref().is_some_and(|m| m.charge < ticks);
            if !held {
                break;
            }
            step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
        }
        for _ in 0..30 {
            if w.fighters[0].mov.is_none() {
                break;
            }
            step_move(&mut w, &d, sim::input::Intent::default());
        }
        w
    };
    let w = hold(334);
    assert_eq!(w.fighters[0].charges[0], 1, "a full hold fills Limit");
    assert!(w.fighters[0].mov.is_none(), "and the release animation completes");
    // One tick short of the source rate does not fill it.
    let w = hold(333);
    assert_eq!(w.fighters[0].charges[0], 0, "a release below 334 held ticks leaves the gauge empty");
    // A short tap is animation only.
    let w = hold(10);
    assert_eq!(w.fighters[0].charges[0], 0);
}

#[test]
fn limit_specials_pick_their_limit_timeline_and_spend_the_pool() {
    let d = common::data();
    for slot in ["nspec", "sspec", "uspec", "dspec"] {
        let mut w = sandbox(&d, &["stratus", "brawler"]);
        w.count = 0;
        w.fighters[0].charges[0] = 1;
        begin(&mut w, &d, slot);
        let m = w.fighters[0].mov.as_ref().unwrap();
        assert_eq!(m.variant, 1, "{slot} runs its Limit variant");
        assert_eq!(w.fighters[0].charges[0], 0, "{slot} spends the Limit use");
        // The normal timeline returns once the gauge is empty.
        w.fighters[0].set_state(State::Idle);
        begin(&mut w, &d, slot);
        assert_eq!(w.fighters[0].mov.as_ref().unwrap().variant, 0, "{slot} falls back");
    }
}

#[test]
fn empty_meter_uses_the_normal_specials() {
    let d = common::data();
    let mut w = sandbox(&d, &["stratus", "brawler"]);
    w.count = 0;
    w.fighters[0].charges[0] = 0;
    let f = d.fighter("stratus").unwrap();
    for slot in ["nspec", "sspec", "uspec", "dspec"] {
        begin(&mut w, &d, slot);
        let m = w.fighters[0].mov.as_ref().unwrap();
        assert_eq!(m.variant, 0, "{slot}");
        assert_eq!(moves::timeline(&d, &w.fighters[0]), &f.moves.get(slot).unwrap().phases[..]);
    }
    // The default down special is Limit Charge: no hitboxes and a charge window.
    let charge = &f.moves.dspec.phases[0];
    assert!(charge.hitboxes.is_empty());
    assert_eq!(charge.charge.as_ref().unwrap().frame, 6, "the charge hold sits on phase frame 6");
    // The Limit down special is Finishing Touch: one 1.0 hit with the source's extreme growth.
    let finisher = f.moves.dspec.variants[0]
        .phases
        .iter()
        .flat_map(|p| &p.hitboxes)
        .find(|h| h.damage == 1.0)
        .expect("Finishing Touch 1.0 hit");
    close(finisher.kbg, 4.31);
    close(finisher.bkb, 60.0);
    assert!(
        f.moves.dspec.variants[0]
            .phases
            .iter()
            .flat_map(|p| &p.hitboxes)
            .any(|h| h.kind == sim::data::HitType::Windbox)
    );
}

#[test]
fn limit_blade_beam_uses_the_limit_damage_and_speed() {
    let d = common::data();
    let f = d.fighter("stratus").unwrap();
    // Normal: 8.0 grounded, 6.4 airborne (UFD's ground/air early tiers).
    close(f.projectiles["bladeBeam"].hit.damage, 8.0);
    close(f.projectiles["bladeBeamAir"].hit.damage, 6.4);
    assert!(f.projectiles["limitBlade"].vel[0] > f.projectiles["bladeBeam"].vel[0], "the Limit wave is faster");
    // Limit: 19% total on the first target, carried in one contact hit with a fitted launch that matches the
    // source's final hit at 100% (see the record for the fit).
    close(f.projectiles["limitBlade"].hit.damage, 19.0);
    close(f.projectiles["limitBladeAir"].hit.damage, 15.0);
    // Casting with Limit spawns the Limit wave instead of the normal one.
    let mut w = sandbox(&d, &["stratus", "brawler"]);
    w.count = 0;
    w.fighters[0].charges[0] = 1;
    begin(&mut w, &d, "nspec");
    for _ in 0..40 {
        step_move(&mut w, &d, sim::input::Intent::default());
        if let Some(p) = w.projs.first() {
            assert_eq!(p.id, "limitBlade");
            return;
        }
    }
    panic!("Limit Blade Beam never spawned its wave");
}

#[test]
fn cross_slash_chains_its_three_slashes() {
    let d = common::data();
    let mut w = start(&d, "sspec");
    let mut seen: Vec<String> = vec![];
    for _ in 0..120 {
        if let Some(name) = phase_name(&d, &w)
            && seen.last() != Some(&name)
        {
            seen.push(name);
        }
        step_move(&mut w, &d, sim::input::Intent::default());
        if w.fighters[0].mov.is_none() {
            break;
        }
    }
    assert_eq!(
        &seen[..3],
        ["slash1", "slash2", "windup"],
        "the natural combo chains: slash 1, slash 2, then the third slash's timeline"
    );
}

#[test]
fn climhazzard_rises_and_offers_the_downward_followup() {
    let d = common::data();
    let mut w = start(&d, "uspec");
    w.fighters[0].ground = false;
    w.fighters[0].surf = None;
    w.fighters[0].y = 300.;
    let mut saw_rise = false;
    let mut followup = false;
    for _ in 0..80 {
        let Some(m) = w.fighters[0].mov.as_ref() else { break };
        let (phase, elapsed) = (phase_name(&d, &w), m.elapsed);
        if phase.as_deref() != Some("thrust") {
            saw_rise = true;
        }
        let press = saw_rise && !followup && elapsed >= 19;
        step_move(&mut w, &d, sim::input::Intent { sp: press, ..Default::default() });
        if press && phase_name(&d, &w).as_deref() == Some("plunge") {
            followup = true;
            break;
        }
    }
    assert!(saw_rise, "Climhazzard reaches its rise phases");
    assert!(followup, "a fresh special press from move frame 19 starts the plunge");
}

#[test]
fn aerial_autocancel_and_landing_lag_follow_the_ufd_rows() {
    let d = common::data();
    let lag = |slot: &str, enter_ticks: u32| -> Option<f64> {
        let mut w = start(&d, slot);
        let f = &mut w.fighters[0];
        f.ground = false;
        f.surf = None;
        f.y = 300.;
        w.fighters[0] = f.clone();
        for _ in 0..enter_ticks {
            if w.fighters[0].mov.is_none() {
                break;
            }
            // Hold the fighter up so the test lands on the tick it chooses.
            w.fighters[0].y = 300.;
            w.fighters[0].vy = 0.;
            step_move(&mut w, &d, sim::input::Intent::default());
        }
        let f = &mut w.fighters[0];
        f.y = 516.;
        f.vy = 6.;
        f.ground = false;
        f.surf = None;
        w.fighters[0] = f.clone();
        if w.fighters[0].mov.is_some() {
            step_move(&mut w, &d, sim::input::Intent::default());
        }
        (w.fighters[0].state == State::Landing).then_some(w.fighters[0].sd.n)
    };
    // Forward air lands during its active window -> 11 frames; the ending autocancel has none.
    close(lag("fair", 20).expect("fair lands"), 11.0);
    assert_eq!(lag("fair", 48), None, "landing on frame 49 is inside the ending autocancel");
    close(lag("nair", 10).expect("nair lands"), 9.0);
    assert_eq!(lag("nair", 33), None);
    close(lag("dair", 20).expect("dair lands"), 16.0);
    assert_eq!(lag("dair", 50), None);
}

/// Contact coverage: each attack connects at close range from the direction it swings and nothing reaches far
/// past the fitted blade (the Buster Sword is long, so the cut-off is 220 px).
#[test]
fn move_reach_matches_fitted_geometry() {
    let d = common::data();
    // (slot, dummy side, attacker feet height, dummy feet height, distances that must connect)
    let cases: [(&str, f64, f64, f64, &[f64]); 9] = [
        ("jab", 1., 520., 520., &[45., 60.]),
        ("ftilt", 1., 520., 520., &[60., 80.]),
        ("fsmash", 1., 520., 520., &[70., 85.]),
        ("dtilt", 1., 520., 520., &[45., 60.]),
        ("nair", 1., 500., 520., &[40., 55.]),
        ("fair", 1., 480., 520., &[60., 80.]),
        ("bair", -1., 480., 520., &[60., 80.]),
        ("uair", 1., 470., 400., &[35.]),
        ("dair", 1., 470., 520., &[35.]),
    ];
    for (slot, side, ay, dy, must) in cases {
        for want in must {
            assert!(connects(&d, slot, side * want, ay, dy), "{slot} must connect at {want} px ({side:+} side)");
        }
        assert!(!connects(&d, slot, side * 220., ay, dy), "{slot} must not connect 220 px away");
    }
}

fn connects(d: &sim::GameData, slot: &str, dx: f64, ay: f64, dy: f64) -> bool {
    let mut w = sandbox(d, &["stratus", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = ay;
    w.fighters[0].face = 1.;
    let aerial = matches!(slot, "nair" | "fair" | "bair" | "uair" | "dair");
    if aerial {
        let f = &mut w.fighters[0];
        f.ground = false;
        f.surf = None;
        f.state = State::Air;
    }
    w.fighters[1].x = 640. + dx;
    w.fighters[1].y = dy;
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, d, 0, &mut f, slot);
    w.fighters[0] = f;
    for t in 0..70 {
        // Pin both fighters so the test isolates hit geometry from travel and gravity.
        w.fighters[0].x = 640.;
        w.fighters[0].y = ay;
        w.fighters[0].vx = 0.;
        w.fighters[0].vy = 0.;
        w.fighters[1].x = 640. + dx;
        w.fighters[1].y = dy;
        w.fighters[1].vx = 0.;
        w.fighters[1].vy = 0.;
        let before = (w.fighters[1].dmg, w.fighters[1].sh, w.fighters[1].state);
        // Release the button after the press so smash attacks stop charging instead of looping.
        let buttons = if t == 0 { 2 } else { 0 };
        let inputs = vec![Input { buttons, ..Default::default() }, Input::default()];
        step_world(&mut w, d, &inputs);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
        if w.fighters[0].mov.is_none() && t > 2 {
            break;
        }
        if (w.fighters[1].dmg, w.fighters[1].sh, w.fighters[1].state) != before {
            return true;
        }
    }
    false
}
#[test]
fn raw_input_routing_starts_the_expected_moves() {
    let d = common::data();
    for (input, slot) in [
        (Input { buttons: 2, ..Default::default() }, "jab"),
        (Input { buttons: 2, stick: (0, 127), ..Default::default() }, "utilt"),
        (Input { buttons: 2, stick: (0, -127), ..Default::default() }, "dtilt"),
        (Input { buttons: 4, ..Default::default() }, "nspec"),
        (Input { buttons: 4, stick: (127, 0), ..Default::default() }, "sspec"),
        (Input { buttons: 4, stick: (0, -127), ..Default::default() }, "dspec"),
        (Input { buttons: 8, ..Default::default() }, "grab"),
    ] {
        let players = vec![
            PlayerConfig { tap_jump: false, ..PlayerConfig::human("stratus") },
            PlayerConfig { tap_jump: false, ..PlayerConfig::human("brawler") },
        ];
        let mut w = common::sandbox(&d, "arena", players);
        w.count = 0;
        if input.stick != (0, 0) {
            // Hold the direction until it is no longer a fresh flick, which would select a smash attack.
            for _ in 0..8 {
                tick(&mut w, &d, Input { stick: input.stick, ..Default::default() });
            }
        }
        tick(&mut w, &d, input);
        let m = w.fighters[0].mov.as_ref().unwrap_or_else(|| panic!("{slot} did not start"));
        assert_eq!(m.slot, slot);
    }
}

#[test]
fn respawn_refills_limit() {
    let d = common::data();
    let mut w = sandbox(&d, &["stratus", "brawler"]);
    w.count = 0;
    w.fighters[0].charges[0] = 0;
    w.fighters[0].charge_wait[0] = 0;
    let mut f = w.fighters[0].clone();
    sim::states::respawn(&w, &d, 0, &mut f);
    w.fighters[0] = f;
    assert_eq!(w.fighters[0].charges[0], 1);
}

/// The Limit wave's fitted launch should KO noticeably earlier than the normal wave. This is a sanity check on
/// the fitted KBG, not an SSBU KO-percent claim.
#[test]
fn limit_blade_beam_launches_harder_than_the_normal_wave() {
    let d = common::data();
    let hit_power = |id: &str| -> f64 {
        let p = &d.fighter("stratus").unwrap().projectiles[id];
        p.hit.damage * p.hit.kbg
    };
    assert!(hit_power("limitBlade") > hit_power("bladeBeam") * 2.5);
}

#[test]
fn separated_smash_windows_connect_in_order() {
    assert_eq!(common::contact_sequence(&common::data(), "stratus", "fsmash"), [3., 3., 13.]);
}

#[test]
fn continuous_swings_contact_once_across_phase_splits() {
    let d = common::data();
    for (slot, damage) in [("bair", 13.), ("usmash", 13.), ("fair", 14.), ("uair", 11.), ("dair", 15.)] {
        assert_eq!(common::contact_sequence(&d, "stratus", slot), [damage], "{slot}");
    }
}

#[test]
fn late_windows_stop_at_the_pinned_inclusive_endpoints() {
    let d = common::data();
    let ledger: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/imports/sources/stratus/ufd.json")).unwrap();
    for (slot, row, active) in [
        ("usmash", "Up Smash", "12(13—14/15—16)"),
        ("uair", "Up Air", "8—9(10—23)"),
        ("dair", "Down Air", "11—13(14—39)"),
    ] {
        let source = ledger["rows"][row]["active"].as_str().unwrap();
        assert_eq!(source, active);
        let numbers: Vec<u32> =
            source.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
        let (start, end) = (numbers[numbers.len() - 2] - 1, numbers[numbers.len() - 1] - 1);
        let late: Vec<_> = windows(&d.fighter("stratus").unwrap().moves.get(slot).unwrap().phases)
            .into_iter()
            .filter(|&(_, _, damage)| damage == 8.)
            .collect();
        assert!(!late.is_empty());
        assert!(late.iter().all(|&(s, e, _)| s == start && e == end), "{slot}: {late:?}");
    }
}

#[test]
fn continuous_tiers_hit_once() {
    let d = common::data();
    let run = |slot: &str, dx: f64, y: f64| -> f64 {
        let mut w = start(&d, slot);
        w.fighters[1].x = 640. + dx;
        w.fighters[1].y = y;
        for _ in 0..70 {
            w.fighters[1].x = 640. + dx;
            w.fighters[1].y = y;
            w.fighters[1].vx = 0.;
            w.fighters[1].vy = 0.;
            step_world(&mut w, &d, &[Input::default(), Input::default()]);
            w.fx.clear();
            w.hits.clear();
            w.acts.clear();
            if w.fighters[0].mov.is_none() {
                break;
            }
        }
        w.fighters[1].dmg
    };
    // Back air sits behind; the dummy is parked there.
    let bair = run("bair", -70., 520.);
    close(bair, 13.0);
    let dash = run("dashatk", 60., 520.);
    assert!(dash <= 11.0 + 1e-6, "dash attack must not add its 8% late tier to the same victim ({dash})");
    let usmash = run("usmash", 35., 520.);
    assert!(usmash <= 13.0 + 1e-6, "up smash must not add 12%/8% after the 13% tier ({usmash})");
}
