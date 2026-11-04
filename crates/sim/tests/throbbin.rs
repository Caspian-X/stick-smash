//! Throbbin (SSBU Robin) reference behavior, durability resources and raw-input coverage.
mod common;

use sim::data::HitType;
use sim::world::State;
use sim::{Input, PlayerConfig, World, combat, moves, step_world};

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
    let mut w = sandbox(d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].face = 1.;
    w.fighters[1].x = 1300.;
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, d, 0, &mut f, slot);
    w.fighters[0] = f;
    w
}

/// Start a move on player 0 while keeping the charge resources `start_move` spent.
fn begin(w: &mut World, d: &sim::GameData, slot: &str) {
    let mut f = w.fighters[0].clone();
    f.set_state(State::Idle);
    moves::start_move(w, d, 0, &mut f, slot);
    w.fighters[0] = f;
}

fn active_frames(d: &sim::GameData, slot: &str) -> Vec<u32> {
    let mut w = start(d, slot);
    let mut seen = vec![];
    for _ in 0..40 {
        let t = w.fighters[0].mov.as_ref().unwrap().t;
        let boxes = combat::active_boxes_at(d, &w.fighters[0], w.frame);
        if !boxes.is_empty() && !seen.contains(&t) {
            seen.push(t);
        }
        step_move(&mut w, d, sim::input::Intent::default());
        if w.fighters[0].mov.is_none() {
            break;
        }
    }
    seen
}

#[track_caller]
/// Current phase name of player 0's move, or None when no move is running.
fn phase_name(d: &sim::GameData, w: &World) -> Option<String> {
    let m = w.fighters[0].mov.as_ref()?;
    Some(moves::timeline(d, &w.fighters[0])[m.phase].name.clone())
}

#[track_caller]
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

#[test]
fn package_identity_and_slots() {
    let d = common::data();
    let f = d.fighter("throbbin").expect("throbbin package");
    assert_eq!(f.id, "throbbin");
    assert_eq!(f.name, "Throbbin");
    assert_eq!(f.extends.as_deref(), Some("base"));
    for slot in [
        "jab", "ftilt", "utilt", "dtilt", "dashatk", "fsmash", "usmash", "dsmash", "nair", "fair", "bair", "uair",
        "dair", "grab", "dashgrab", "nspec", "sspec", "uspec", "dspec",
    ] {
        assert!(f.moves.get(slot).is_some(), "missing {slot}");
    }
    let ids: Vec<_> = f.charges.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["levin", "thunder", "fire", "wind", "dark"]);
    let levin = &f.charges[0];
    assert_eq!((levin.uses, levin.recharge), (8, 600));
    assert_eq!(f.charges[1].uses, 20);
    assert_eq!(f.charges[2].uses, 6);
    assert_eq!(f.charges[3].uses, 10);
    assert_eq!(f.charges[4].uses, 3);
}

#[test]
fn levin_variants_and_bronze_fallback() {
    let d = common::data();
    let f = d.fighter("throbbin").unwrap();
    for slot in ["fsmash", "usmash", "dsmash", "nair", "fair", "bair", "uair", "dair"] {
        let m = f.moves.get(slot).unwrap();
        assert_eq!(m.variants.len(), 1, "{slot} needs a Levin variant");
        assert_eq!(m.variants[0].needs.charge, "levin");
        assert_eq!(m.variants[0].phases[0].cost.as_ref().map(|c| c.charge.as_str()), Some("levin"), "{slot} cost");
    }
    // Bronze is the default timeline: less damage, shorter reach, no electricity.
    let levin = &f.moves.fsmash.variants[0].phases[0].hitboxes[0];
    let bronze = &f.moves.fsmash.phases[0].hitboxes[0];
    close(levin.damage, 16.0);
    close(bronze.damage, 9.6);
    assert_eq!(levin.kind, HitType::Sakurai, "the source 361 angle keeps its angle behavior");
    assert_eq!(bronze.kind, HitType::Sakurai);
    assert_eq!(
        f.moves.fair.variants[0].phases.iter().flat_map(|p| &p.hitboxes).next().unwrap().kind,
        HitType::Electric
    );
    assert!(levin.r > bronze.r);
    let nair = f.moves.nair.variants[0].phases.iter().flat_map(|p| &p.hitboxes).map(|h| h.damage).fold(0., f64::max);
    close(nair, 11.5);
    // Tilts and the jab always use the Bronze sword.
    assert!(f.moves.ftilt.variants.is_empty());
    assert!(f.moves.jab.phases.iter().all(|p| p.cost.as_ref().map(|c| c.charge.as_str()) != Some("levin")));
}

#[test]
fn reference_active_windows_match_the_ledger() {
    let d = common::data();
    // Engine phase frame = source frame - 1, so UFD "16-18" is phase 15-17.
    let w = start(&d, "fsmash");
    let hitboxes = moves::phase(&d, &w.fighters[0]).hitboxes.clone();
    let spans: Vec<_> = hitboxes.iter().map(|h| (h.start, h.end.unwrap())).collect();
    assert_eq!(spans[0], (15, 17), "Levin clean hit 16-18 -> phase 15-17");
    assert!(spans.contains(&(18, 26)), "Levin late hit 19-27 -> phase 18-26");
    assert!(!spans.contains(&(19, 26)));
    assert_eq!(active_frames(&d, "ftilt"), vec![8, 9]);
    assert_eq!(active_frames(&d, "dtilt"), vec![6, 7]);
}

#[test]
fn levin_uses_are_spent_then_bronze_is_used_until_recharge() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    assert_eq!(w.fighters[0].charges[0], 8);
    for n in 1..=8 {
        let mut f = w.fighters[0].clone();
        f.set_state(State::Idle);
        moves::start_move(&mut w, &d, 0, &mut f, "fsmash");
        assert_eq!(f.mov.as_ref().unwrap().variant, 1, "use {n} stays Levin");
        assert_eq!(f.charges[0], 8 - n);
        w.fighters[0] = f;
    }
    // Empty: the same smash now runs the Bronze timeline and starts the 10 s recharge.
    begin(&mut w, &d, "fsmash");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().variant, 0);
    assert_eq!(w.fighters[0].charge_wait[0], 75, "one use every ceil(600/8) ticks");
    // Recharge restores uses gradually and finishes full after 10 s.
    w.fighters[0].set_state(State::Idle);
    for _ in 0..300 {
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert_eq!(w.fighters[0].charges[0], 4, "four uses after 300 of 600 ticks");
    for _ in 0..300 {
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert_eq!(w.fighters[0].charges[0], 8);
    assert_eq!(w.fighters[0].charge_wait[0], 0);
}

#[test]
fn thunder_charge_tiers_pick_their_spell_and_cost() {
    let d = common::data();
    for (held, spell, cost) in [(0, "thunder", 1), (26, "elthunder", 3), (66, "arcthunder", 5), (126, "thoron", 8)] {
        let mut w = start(&d, "nspec");
        let before = w.fighters[0].charges[1];
        // The charge window opens on phase frame 7; only then do held ticks build charge.
        for _ in 0..200 {
            let m = w.fighters[0].mov.as_ref().unwrap();
            if m.t == 7 && m.charge == held {
                break;
            }
            step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
        }
        let m = w.fighters[0].mov.as_ref().unwrap();
        assert_eq!((m.t, m.charge), (7, held), "{spell} charge ticks");
        step_move(&mut w, &d, sim::input::Intent::default());
        let m = w.fighters[0].mov.as_ref().unwrap();
        let name = &moves::timeline(&d, &w.fighters[0])[m.phase].name;
        assert_eq!(name, spell);
        assert_eq!(w.fighters[0].charges[1], before - cost, "{spell} cost");
    }
    // A spent tome casts the empty animation with no projectile.
    let mut w = start(&d, "nspec");
    w.fighters[0].charges[1] = 0;
    begin(&mut w, &d, "nspec");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().variant, 0);
    assert!(moves::timeline(&d, &w.fighters[0]).iter().all(|p| p.hitboxes.is_empty()));
}

/// Cast a charged spell by holding special for `held` extra ticks, then release, and return the projectile
/// that appeared plus its hit data.
fn cast_thunder(held: u32) -> (sim::world::Projectile, World) {
    let d = common::data();
    let mut w = start(&d, "nspec");
    for _ in 0..400 {
        let m = w.fighters[0].mov.as_ref().unwrap();
        if m.t == 7 && m.charge == held {
            break;
        }
        step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
    }
    step_move(&mut w, &d, sim::input::Intent::default());
    for _ in 0..40 {
        if let Some(p) = w.projs.first() {
            return (p.clone(), w);
        }
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    panic!("no projectile for {held} held ticks");
}

#[test]
fn thunder_tiers_deal_their_source_damage() {
    // Source: Thunder 5.5, Elthunder 11.0, Arcthunder 8.0 blast (vortex approximated), Thoron 2.6 per 4 ticks.
    for (held, damage) in [(0, 5.5), (26, 11.0), (66, 8.0), (126, 2.6)] {
        let (p, _) = cast_thunder(held);
        close(p.hit.damage, damage);
    }
    // Thoron keeps its rehit so the beam hits repeatedly as it passes.
    let (thoron, _) = cast_thunder(126);
    assert_eq!(thoron.hit.rehit, Some(4));
}

/// Every pool restores one use per `ceil(recharge / uses)` ticks, and spending during a cycle never hands the
/// use straight back. Includes a pool whose recharge does not divide evenly.
#[test]
fn recharge_schedule_is_exact_for_every_pool() {
    let mut d = common::data();
    let at = d.fighters.iter().position(|f| f.id == "throbbin").unwrap();
    d.fighters[at].charges.push(sim::data::ChargePool {
        id: "test".into(),
        uses: 7,
        recharge: 100,
        discard_delay: 0,
        empty_sfx: None,
        refill_sfx: None,
    });
    let pumps: Vec<(u32, u32)> = d.fighters[at].charges.iter().map(|c| (c.uses, c.recharge)).collect();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].charges = vec![0; pumps.len()];
    for (i, (uses, recharge)) in pumps.iter().enumerate() {
        let step = recharge.div_ceil(*uses).max(1);
        assert_eq!(sim::charges::ticks_per_use(&d.fighters[at].charges[i]), step);
        // Empty and recharging: exactly one use per step, then full.
        w.fighters[0].charges[i] = 0;
        w.fighters[0].charge_wait[i] = step;
        for k in 1..=*uses {
            for _ in 0..step - 1 {
                step_world(&mut w, &d, &[]);
                w.fx.clear();
                w.hits.clear();
                w.acts.clear();
            }
            assert_eq!(w.fighters[0].charges[i], k - 1, "pool {i} before use {k} of {uses}");
            step_world(&mut w, &d, &[]);
            assert_eq!(w.fighters[0].charges[i], k, "pool {i} restores use {k} of {uses} after {step} ticks");
        }
        assert_eq!(w.fighters[0].charge_wait[i], 0, "pool {i} stops recharging when full");
        // Spend a use at the start of a fresh step: it is not refunded on the next tick.
        w.fighters[0].charges[i] = uses - 1;
        w.fighters[0].charge_wait[i] = step;
        for _ in 0..step - 1 {
            step_world(&mut w, &d, &[]);
            w.fx.clear();
            w.hits.clear();
            w.acts.clear();
            assert_eq!(w.fighters[0].charges[i], uses - 1, "pool {i} does not refund inside the step");
        }
        w.fighters[0].charges[i] = *uses;
        w.fighters[0].charge_wait[i] = 0;
    }
}

#[test]
fn recharge_does_not_refund_uses_spent_while_recharging() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].charges[0] = 0;
    w.fighters[0].charge_wait[0] = sim::charges::ticks_per_use(&d.fighters[w.fighters[0].data].charges[0]);
    for _ in 0..300 {
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    let some = w.fighters[0].charges[0];
    assert!(some >= 3, "recharge returns uses gradually ({some})");
    // Spend one of the recharged uses: the next tick must not hand it back.
    w.fighters[0].charges[0] -= 1;
    let spent = w.fighters[0].charges[0];
    step_world(&mut w, &d, &[]);
    assert_eq!(w.fighters[0].charges[0], spent, "spending during recharge is not refunded");
    for _ in 0..400 {
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert_eq!(w.fighters[0].charges[0], 8, "the pool still completes");
    assert_eq!(w.fighters[0].charge_wait[0], 0);
}

#[test]
fn empty_pools_skip_the_fire_burst_and_the_rapid_loop() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].charges = vec![0; 5];
    begin(&mut w, &d, "jab");
    // Hold attack straight through the chain.
    for _ in 0..80 {
        if w.fighters[0].mov.is_none() {
            break;
        }
        let mut f = w.fighters[0].clone();
        moves::st_attack(
            &mut w,
            &d,
            0,
            &mut f,
            sim::input::Intent { atk: true, atk_held: true, ..Default::default() },
            0.,
        );
        w.fighters[0] = f;
        w.frame += 1;
        if let Some(phase) = phase_name(&d, &w) {
            assert!(
                !matches!(phase.as_str(), "jab3" | "rapid" | "rapidEnd"),
                "empty pools must not run the Fire burst or the Wind loop (got {phase})"
            );
        }
    }
}

#[test]
fn charge_tier_thresholds_are_inclusive_and_the_tick_below_stays_lower() {
    // Elthunder at 26 held ticks, Arcthunder at 66, Thoron at 126; one tick less stays in the lower tier.
    for (held, expected) in [
        (25, "thunder"),
        (26, "elthunder"),
        (65, "elthunder"),
        (66, "arcthunder"),
        (125, "arcthunder"),
        (126, "thoron"),
    ] {
        let d = common::data();
        let mut w = start(&d, "nspec");
        for _ in 0..400 {
            let m = w.fighters[0].mov.as_ref().unwrap();
            if m.t == 7 && m.charge == held {
                break;
            }
            step_move(&mut w, &d, sim::input::Intent { sp_held: true, ..Default::default() });
        }
        step_move(&mut w, &d, sim::input::Intent::default());
        assert_eq!(phase_name(&d, &w).as_deref(), Some(expected), "{held} held ticks");
    }
}

#[test]
fn an_interrupted_move_still_spends_its_use() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    begin(&mut w, &d, "fsmash");
    assert_eq!(w.fighters[0].charges[0], 7, "Levin use spent at move start");
    // Cancelled before any hitbox (as if interrupted): the use stays spent and is not paid again.
    w.fighters[0].set_state(State::Hitstun);
    assert_eq!(w.fighters[0].charges[0], 7);
    step_world(&mut w, &d, &[]);
    assert_eq!(w.fighters[0].charges[0], 7);
}

#[test]
fn respawn_refills_every_pool() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].charges = vec![0, 4, 0, 2, 1];
    w.fighters[0].charge_wait = vec![300, 120, 0, 50, 900];
    let mut f = w.fighters[0].clone();
    sim::states::respawn(&w, &d, 0, &mut f);
    w.fighters[0] = f;
    assert_eq!(w.fighters[0].charges, vec![8, 20, 6, 10, 3]);
    assert_eq!(w.fighters[0].charge_wait, vec![0; 5]);
}

#[test]
fn landing_during_the_ending_autocancel_has_no_landing_lag() {
    let d = common::data();
    let mut w = start(&d, "fair");
    let f = &mut w.fighters[0];
    f.ground = false;
    f.surf = None;
    f.y = 300.;
    w.fighters[0] = f.clone();
    // Run into the ending autocancel window (source 28+, engine 27+) before landing.
    for _ in 0..29 {
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    let f = &mut w.fighters[0];
    f.y = 516.;
    f.vy = 6.;
    f.ground = false;
    f.surf = None;
    w.fighters[0] = f.clone();
    step_move(&mut w, &d, sim::input::Intent::default());
    let state = w.fighters[0].state;
    assert!(
        state != State::Landing || w.fighters[0].sd.n == 0.,
        "no landing lag inside the ending autocancel ({state:?})"
    );
}

/// The rapid loop spends about one Wind use per second and stops when the tome empties.
#[test]
fn rapid_jab_drains_wind_and_exits_when_empty() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].charges[4 /* dark */] = 3;
    w.fighters[0].charges[3 /* wind */] = 3;
    w.fighters[0].charges[2 /* fire */] = 1;
    begin(&mut w, &d, "jab");
    let mut saw_rapid = false;
    for tick in 0..400 {
        if w.fighters[0].mov.is_none() {
            break;
        }
        if let Some(name) = phase_name(&d, &w) {
            if name == "rapid" {
                saw_rapid = true;
                // One use is spent per 60-tick loop.
                let expected = 3 - (tick / 60).min(3) as u32;
                assert!(w.fighters[0].charges[3] <= expected, "wind {} at tick {tick}", w.fighters[0].charges[3]);
            }
            if saw_rapid && name == "rapidEnd" {
                assert_eq!(w.fighters[0].charges[3], 0, "the loop runs until the tome empties");
                return;
            }
        }
        let mut f = w.fighters[0].clone();
        moves::st_attack(
            &mut w,
            &d,
            0,
            &mut f,
            sim::input::Intent { atk: true, atk_held: true, ..Default::default() },
            0.,
        );
        w.fighters[0] = f;
        w.frame += 1;
    }
    panic!("rapid jab never ended (saw rapid: {saw_rapid})");
}

#[test]
fn single_wind_use_still_gives_the_first_elwind_blade() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].charges[3] = 1;
    begin(&mut w, &d, "uspec");
    assert_eq!(w.fighters[0].mov.as_ref().unwrap().variant, 2, "one use picks the single-blade timeline");
    let phases: Vec<_> = moves::timeline(&d, &w.fighters[0]).iter().map(|p| p.name.clone()).collect();
    assert!(phases.contains(&"rise1".to_string()));
    // Total length matches the source's 85 frames.
    let total: u32 = moves::timeline(&d, &w.fighters[0]).iter().map(|p| p.frames.unwrap_or(0)).sum();
    assert_eq!(total, 85);
}

#[test]
fn nosferatu_capture_window_and_whiff_recovery() {
    let d = common::data();
    let hit_frames = |slot: &str| -> (Vec<u32>, u32) {
        let mut w = start(&d, slot);
        w.fighters[0].x = 640.;
        w.fighters[0].y = 520.;
        w.fighters[1].x = 1300.;
        begin(&mut w, &d, slot);
        let phase = |w: &World| {
            let m = w.fighters[0].mov.as_ref().unwrap();
            moves::timeline(&d, &w.fighters[0])[m.phase].name.clone()
        };
        let mut active = vec![];
        let mut seen = None;
        for _ in 0..70 {
            if phase(&w) == "capture" {
                let m = w.fighters[0].mov.as_ref().unwrap();
                seen.get_or_insert(m.elapsed);
                if !combat::active_boxes_at(&d, &w.fighters[0], w.frame).is_empty() {
                    active.push(m.t);
                }
            }
            step_move(&mut w, &d, sim::input::Intent::default());
            if w.fighters[0].mov.is_none() {
                break;
            }
        }
        (active, seen.expect("capture phase"))
    };
    let (active, first) = hit_frames("dspec");
    assert_eq!(active, vec![0, 1, 2, 3], "grabboxes run the source's four frames (15-18)");
    assert_eq!(first, 14, "capture starts on engine frame 14");
    // The whiff branch lasts the source's 49 frames; catching on the first capture tick also totals 49 (a later
    // capture tick runs up to three ticks longer, because the drain length is fixed).
    let mut whiff = start(&d, "dspec");
    whiff.fighters[1].x = 1300.;
    let mut ticks = 0;
    while whiff.fighters[0].mov.is_some() && ticks < 200 {
        step_move(&mut whiff, &d, sim::input::Intent::default());
        ticks += 1;
    }
    assert_eq!(ticks, 49, "whiffed Nosferatu ends after the source's 49 frames");
    // A real catch goes through collision, so it needs the normal step: the drain branch starts on the capture
    // tick and the whole move lasts 49 frames when caught on the first window tick (up to 52 on the last).
    let catch = |dx: f64, enter: u32| -> (u32, bool) {
        let mut w = start(&d, "dspec");
        w.fighters[1].x = 640. + dx;
        w.fighters[1].y = 520.;
        let mut ticks = 0;
        let mut drained = false;
        while w.fighters[0].mov.is_some() && ticks < 200 {
            // Out of reach until `enter`: the villain steps into the grabbox on that tick.
            w.fighters[1].x = 640. + if ticks >= enter { dx } else { dx + 300. };
            w.fighters[1].y = 520.;
            w.fighters[1].vx = 0.;
            w.fighters[1].vy = 0.;
            step_world(&mut w, &d, &[]);
            w.fx.clear();
            w.hits.clear();
            w.acts.clear();
            if phase_name(&d, &w).as_deref() == Some("drain") {
                drained = true;
            }
            ticks += 1;
        }
        (ticks, drained)
    };
    let (early, drained) = catch(50., 0);
    assert!(drained, "a catch enters the drain branch");
    assert_eq!(early, 49, "caught on the first capture tick totals the source's 49 frames");
    let (late, drained_late) = catch(50., 16);
    assert!(drained_late, "a late catch still drains");
    assert!((50..=52).contains(&late), "a later catch runs 50-52 ticks (got {late})");
}

/// Durations come from the pinned UFD totals (the number of frames in the move). Hit windows shift by one
/// because the move's first tick is source frame 1; the length does not.
#[test]
fn durations_match_the_source_ledger() {
    let d = common::data();
    let len = |phases: &[sim::data::Phase]| -> u32 { phases.iter().map(|p| p.frames.unwrap_or(0)).sum() };
    let f = d.fighter("throbbin").unwrap();
    for (slot, total) in [
        ("ftilt", 32),
        ("utilt", 32),
        ("dtilt", 21),
        ("dashatk", 41),
        ("fsmash", 57),
        ("usmash", 52),
        ("dsmash", 56),
        ("nair", 47),
        ("fair", 33),
        ("bair", 39),
        ("uair", 45),
        ("dair", 59),
        ("grab", 39),
        ("dashgrab", 47),
    ] {
        let m = f.moves.get(slot).unwrap();
        assert_eq!(len(&m.phases), total, "{slot} base timeline");
        for (i, v) in m.variants.iter().enumerate() {
            assert_eq!(len(&v.phases), total, "{slot} variant {}", i + 1);
        }
    }
    // Specials: cast and empty-tome durations from UFD.
    let phase_len = |slot: &str, variant: usize, phase: &str| -> u32 {
        let m = f.moves.get(slot).unwrap();
        let phases = if variant == 0 { &m.phases } else { &m.variants[variant - 1].phases };
        phases.iter().find(|p| p.name == phase).and_then(|p| p.frames).unwrap()
    };
    assert_eq!(phase_len("nspec", 1, "thunder"), 32);
    assert_eq!(phase_len("nspec", 1, "elthunder"), 38);
    assert_eq!(phase_len("nspec", 1, "arcthunder"), 38);
    assert_eq!(phase_len("nspec", 1, "thoron"), 74);
    assert_eq!(phase_len("nspec", 0, "empty"), 39);
    assert_eq!(phase_len("sspec", 1, "main"), 63);
    assert_eq!(phase_len("uspec", 0, "empty"), 51);
    assert_eq!(len(&f.moves.uspec.variants[0].phases), 85);
    let rapid_end = f.moves.jab.phases.iter().find(|p| p.name == "rapidEnd").unwrap();
    assert_eq!(rapid_end.frames, Some(47), "source 47-frame rapid finisher");
}

/// Directional coverage: the first neutral-air swing reaches in front, the second sweeps behind (UFD frames).
#[test]
fn neutral_air_swings_front_then_back() {
    let d = common::data();
    // Run the move with a dummy parked at `dx` and report which swings connected.
    let hits = |dx: f64| -> (bool, bool) {
        let mut w = sandbox(&d, &["throbbin", "brawler"]);
        w.count = 0;
        w.fighters[0].x = 640.;
        w.fighters[0].y = 460.;
        w.fighters[0].ground = false;
        w.fighters[0].surf = None;
        w.fighters[0].state = State::Air;
        w.fighters[0].face = 1.;
        w.fighters[1].x = 640. + dx;
        w.fighters[1].y = 500.;
        begin(&mut w, &d, "nair");
        let pin = |w: &mut World| {
            w.fighters[0].x = 640.;
            w.fighters[0].y = 460.;
            w.fighters[0].vx = 0.;
            w.fighters[0].vy = 0.;
            w.fighters[1].x = 640. + dx;
            w.fighters[1].y = 500.;
            w.fighters[1].vx = 0.;
            w.fighters[1].vy = 0.;
        };
        let (mut first, mut second, mut seen) = (false, false, 0.);
        for _ in 0..40 {
            pin(&mut w);
            step_world(&mut w, &d, &[]);
            w.fx.clear();
            w.hits.clear();
            w.acts.clear();
            let Some(m) = w.fighters[0].mov.as_ref() else { break };
            let phase = moves::timeline(&d, &w.fighters[0])[m.phase].name.clone();
            if w.fighters[1].dmg > seen + 1e-9 {
                seen = w.fighters[1].dmg;
                match phase.as_str() {
                    "swing1" => first = true,
                    "swing2" => second = true,
                    _ => {}
                }
            }
        }
        (first, second)
    };
    let (front_first, front_second) = hits(50.);
    assert!(front_first, "the first swing reaches in front");
    assert!(!front_second, "the second swing does not reach in front");
    let (back_first, back_second) = hits(-50.);
    assert!(!back_first, "the first swing does not reach behind");
    assert!(back_second, "the second swing sweeps behind");
}

/// Neutral air has two windows; taking both on the same victim proves the phase entry clears the already-hit
/// list. The swings sweep opposite ways, so the dummy is moved from the front to the back between them.
/// Unblockable projectile hits bypass guards and counters like melee ones, but still record contact history so
/// rehit intervals and single-hit expiry apply. Uses Thunder with the flag turned on in memory.
#[test]
fn unblockable_projectile_contact_follows_melee_rules() {
    let mut d = common::data();
    let at = d.fighters.iter().position(|f| f.id == "throbbin").unwrap();
    {
        let thunder = d.fighters[at].projectiles.get_mut("thunder").unwrap();
        thunder.hit.unblockable = true;
        thunder.hit.rehit = Some(8);
        thunder.life = 60;
        thunder.r = 20.;
        thunder.vel = [0.; 2];
    }
    // Park the bolt on the victim so the rehit window is the only thing that changes between ticks.
    let run = |d: &sim::GameData, shielded: bool, ticks: u32| -> (f64, usize, World) {
        let mut w = sandbox(d, &["throbbin", "brawler"]);
        w.count = 0;
        w.fighters[0].x = 640.;
        w.fighters[0].y = 520.;
        w.fighters[1].x = 640.;
        w.fighters[1].y = 520.;
        let shooter = w.fighters[0].clone();
        sim::projectiles::spawn(&mut w, d, 0, &shooter, "thunder");
        // Sit the bolt on the victim's body.
        w.projs[0].x = 640.;
        w.projs[0].y = 480.;
        if shielded {
            w.fighters[1].set_state(State::Shield);
        }
        // Hold shield for the whole window so the victim stays guarding (state alone drops it).
        let hold = vec![Input::default(), Input { buttons: 16, ..Default::default() }];
        let mut hits = 0;
        for _ in 0..ticks {
            let before = w.fighters[1].dmg;
            step_world(&mut w, d, if shielded { &hold } else { &[] });
            w.fx.clear();
            w.hits.clear();
            w.acts.clear();
            w.fighters[1].x = 640.;
            w.fighters[1].y = 520.;
            if let Some(p) = w.projs.first_mut() {
                (p.x, p.y) = (640., 480.);
            }
            if w.fighters[1].dmg > before {
                hits += 1;
            }
        }
        (w.fighters[1].dmg, hits, w)
    };
    // Guarding does not stop the contact, and the 8-tick rehit window limits it to one hit in 7 ticks.
    let (dmg, hits, _) = run(&d, true, 7);
    assert_eq!((dmg, hits), (5.5, 1), "shielded victim takes exactly one hit inside the rehit window");
    // By the ninth tick a second contact is allowed again.
    let (dmg, hits, _) = run(&d, true, 9);
    assert_eq!((dmg, hits), (11.0, 2), "the rehit window reopens after 8 ticks");
    // Without rehit the projectile is consumed by its first contact.
    let mut single = d.clone();
    if let Some(t) = single.fighters[at].projectiles.get_mut("thunder") {
        t.hit.rehit = None;
    }
    let (dmg, hits, w) = run(&single, false, 12);
    assert_eq!((dmg, hits), (5.5, 1), "a single-hit projectile hits once");
    assert!(w.projs.is_empty(), "and is consumed");
}

#[test]
fn neutral_air_swings_twice() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 470.;
    w.fighters[0].ground = false;
    w.fighters[0].surf = None;
    w.fighters[0].state = State::Air;
    w.fighters[0].face = 1.;
    begin(&mut w, &d, "nair");
    for tick in 0..34 {
        w.fighters[0].x = 640.;
        w.fighters[0].y = 470.;
        w.fighters[0].vx = 0.;
        w.fighters[0].vy = 0.;
        w.fighters[1].x = if tick < 14 { 690. } else { 590. };
        w.fighters[1].y = 470.;
        w.fighters[1].vx = 0.;
        w.fighters[1].vy = 0.;
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert!(w.fighters[1].dmg >= 22.9, "both Levin swings connect on the same victim ({})", w.fighters[1].dmg);
}

#[test]
fn nosferatu_ignores_shields_and_heals_the_drained_amount() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].dmg = 60.;
    w.fighters[1].x = 700.;
    w.fighters[1].y = 520.;
    // Hold shield: a command grab must still connect.
    w.fighters[1].set_state(State::Shield);
    begin(&mut w, &d, "dspec");
    let start = (w.fighters[1].dmg, w.fighters[0].dmg);
    for _ in 0..50 {
        w.fighters[1].x = 700.;
        w.fighters[1].y = 520.;
        w.fighters[1].vx = 0.;
        w.fighters[1].vy = 0.;
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    let dealt = w.fighters[1].dmg - start.0;
    let healed = start.1 - w.fighters[0].dmg;
    assert!(dealt > 4., "shielding victim still takes the drain ({dealt})");
    // The 1.0 capture tick deals damage without healing; the drain ticks heal 1.5x what they deal.
    close(healed, (dealt - 1.0) * 1.5);
}

#[test]
fn nosferatu_drains_and_heals() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].dmg = 40.;
    w.fighters[1].x = 700.;
    w.fighters[1].y = 520.;
    begin(&mut w, &d, "dspec");
    let start_dmg = w.fighters[1].dmg;
    for _ in 0..60 {
        w.fighters[1].x = 700.;
        w.fighters[1].y = 520.;
        w.fighters[1].vx = 0.;
        w.fighters[1].vy = 0.;
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    assert!(w.fighters[1].dmg > start_dmg + 5., "drained {}", w.fighters[1].dmg - start_dmg);
    assert!(w.fighters[0].dmg < 40., "healed to {}", w.fighters[0].dmg);
    assert_eq!(w.fighters[0].charges[4], 2, "one dark use");
}

#[test]
fn empty_aerial_uses_do_not_break_movement() {
    let d = common::data();
    let mut w = sandbox(&d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].charges = vec![0; 5];
    for slot in ["nair", "fair", "bair", "uair", "dair", "fsmash", "usmash", "dsmash"] {
        begin(&mut w, &d, slot);
        assert_eq!(w.fighters[0].mov.as_ref().unwrap().variant, 0, "{slot} falls back");
        for _ in 0..80 {
            step_move(&mut w, &d, sim::input::Intent::default());
            if w.fighters[0].mov.is_none() {
                break;
            }
        }
        assert!(w.fighters[0].mov.is_none(), "{slot} completes");
    }
}

#[test]
fn aerial_landing_lag_and_autocancel() {
    let d = common::data();
    // f-air lands during its active window -> 11 frames of landing lag.
    let mut w = start(&d, "fair");
    let f = &mut w.fighters[0];
    f.ground = false;
    f.surf = None;
    f.y = 300.;
    w.fighters[0] = f.clone();
    for _ in 0..12 {
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    let f = &mut w.fighters[0];
    f.y = 516.;
    f.vy = 6.;
    f.ground = false;
    f.surf = None;
    w.fighters[0] = f.clone();
    step_move(&mut w, &d, sim::input::Intent::default());
    assert_eq!(w.fighters[0].state, State::Landing);
    assert_eq!(w.fighters[0].sd.n, 11.);
}

/// Contact coverage: each attack connects at close range from the direction it swings, and nothing reaches far
/// past its fitted blade. The dummy stands still, so this isolates geometry from CPU spacing.
#[test]
fn move_reach_matches_fitted_geometry() {
    let d = common::data();
    // (slot, dummy side, dummy above the attacker, distances that must connect)
    let cases: [(&str, f64, bool, &[f64]); 11] = [
        ("jab", 1., false, &[40., 55.]),
        ("ftilt", 1., false, &[55., 70.]),
        ("utilt", 1., false, &[55., 70.]),
        ("dtilt", 1., false, &[40., 55.]),
        ("fsmash", 1., false, &[70.]),
        ("nair", 1., false, &[40., 55.]),
        ("fair", 1., false, &[55., 70.]),
        ("bair", -1., false, &[55., 70.]),
        ("uair", 1., true, &[40.]),
        ("dair", 1., true, &[40.]),
        ("dspec", 1., false, &[40., 55.]),
    ];
    for (slot, side, above, must) in cases {
        for want in must {
            assert!(connects(&d, slot, side * want, above), "{slot} must connect at {want} px ({side:+} side)");
        }
        assert!(!connects(&d, slot, side * 170., above), "{slot} must not connect 170 px away");
    }
}

fn connects(d: &sim::GameData, slot: &str, dx: f64, above: bool) -> bool {
    let mut w = sandbox(d, &["throbbin", "brawler"]);
    w.count = 0;
    w.fighters[0].x = 640.;
    w.fighters[0].y = 520.;
    w.fighters[0].face = 1.;
    let aerial = slot.ends_with("air") || matches!(slot, "nair" | "fair" | "bair" | "uair" | "dair");
    if aerial {
        let f = &mut w.fighters[0];
        // Up air sweeps above the feet, so its attacker hangs below the dummy instead.
        f.y = match slot {
            "uair" => 590.,
            // Down air strikes underneath, so its attacker hangs a little above the dummy.
            "dair" => 470.,
            _ => 480.,
        };
        f.ground = false;
        f.surf = None;
        f.state = State::Air;
    }
    w.fighters[1].x = 640. + dx;
    w.fighters[1].y = if above { 460. } else { 520. };
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, d, 0, &mut f, slot);
    w.fighters[0] = f;
    for t in 0..40 {
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
            PlayerConfig { tap_jump: false, ..PlayerConfig::human("throbbin") },
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
