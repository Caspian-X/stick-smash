//! Verdant (SSBU Byleth) reference behavior, per-move weapon attachment, tether recovery and raw-input coverage.
mod common;

use sim::data::HitType;
use sim::data::hitbox::Situation;
use sim::data::visual::{Joint, Shape};
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
    let mut w = sandbox(d, &["verdant", "brawler"]);
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

#[track_caller]
fn phase_name(d: &sim::GameData, w: &World) -> Option<String> {
    let m = w.fighters[0].mov.as_ref()?;
    Some(moves::timeline(d, &w.fighters[0])[m.phase].name.clone())
}

/// Total frames of the grounded continuation chain from the first phase (follows a `grounded` branch before
/// the phase timeout; airborne/landing branches are not part of the grounded total).
fn chain_frames(d: &sim::GameData, slot: &str) -> u32 {
    let m = d.fighter("verdant").unwrap().moves.get(slot).unwrap();
    let mut total = 0;
    let mut at = m.phases[0].name.clone();
    for _ in 0..16 {
        let p = m.phases.iter().find(|p| p.name == at).unwrap();
        total += p.frames.unwrap();
        let next = p
            .transitions
            .iter()
            .find(|t| t.on == sim::data::Trigger::Grounded)
            .or_else(|| p.transitions.iter().find(|t| t.on == sim::data::Trigger::Timeout && t.min_charge == 0));
        match next {
            Some(t) if !matches!(t.to.as_str(), "end" | "helpless" | "landing") => at = t.to.clone(),
            _ => break,
        }
    }
    total
}

#[test]
fn package_identity_and_slots() {
    let d = common::data();
    let f = d.fighter("verdant").expect("verdant package");
    assert_eq!(f.id, "verdant");
    assert_eq!(f.name, "Verdant");
    assert_eq!(f.extends.as_deref(), Some("base"));
    assert!(!f.is_abstract);
    for slot in [
        "jab", "ftilt", "utilt", "dtilt", "dashatk", "fsmash", "usmash", "dsmash", "nair", "fair", "bair", "uair",
        "dair", "grab", "dashgrab", "nspec", "sspec", "uspec", "dspec",
    ] {
        assert!(f.moves.get(slot).is_some(), "missing {slot}");
    }
    // All four throws are authored (release inside the total, positive damage).
    for (dir, name) in [("f", "forward"), ("b", "back"), ("u", "up"), ("d", "down")] {
        let t = match dir {
            "f" => &f.throws.f,
            "b" => &f.throws.b,
            "u" => &f.throws.u,
            _ => &f.throws.d,
        };
        assert!(t.release > 0 && t.release <= t.total, "{name} throw release");
        assert!(t.hit.damage > 0., "{name} throw damage");
    }
    // Reference stats: weight 97, and the four relics are drawn from hidden hand attachments.
    assert!((f.stats.weight - 100. / 97.).abs() < 1e-8);
    assert_eq!(f.attachments.len(), 4);
}

#[test]
fn each_move_wields_the_weapon_the_source_gives_it() {
    let d = common::data();
    let f = d.fighter("verdant").unwrap();
    // Source mapping: the article/bone each move's attack rows use in the pinned 13.0.1 scripts.
    let expected: [(&str, Option<&str>); 19] = [
        ("jab", Some("sword")),
        ("ftilt", Some("sword")),
        ("utilt", Some("sword")),
        ("dtilt", Some("sword")), // master_sword article
        ("dashatk", Some("sword")),
        ("fsmash", Some("lance")), // master_spear (Areadbhar)
        ("usmash", Some("sword")), // master_sword article
        ("dsmash", Some("axe")),   // master_axe (Aymr)
        ("nair", Some("bow")),     // master_bow (Failnaught)
        ("fair", Some("lance")),
        ("bair", Some("lance")),
        ("uair", Some("sword")),
        ("dair", Some("axe")),
        ("grab", None),
        ("dashgrab", None),
        ("nspec", Some("bow")),
        ("sspec", Some("lance")), // master_spear
        ("uspec", Some("sword")), // master_sword article
        ("dspec", Some("axe")),   // master_axe
    ];
    for (slot, weapon) in expected {
        let m = f.moves.get(slot).unwrap();
        for p in &m.phases {
            let keys: Vec<&str> = p.attach.keys().map(String::as_str).collect();
            let want = weapon.map_or(vec![], |w| vec![w]);
            assert_eq!(keys, want, "{slot}.{} attaches {keys:?}", p.name);
            if let Some(weapon) = weapon {
                let anim = &p.attach[weapon];
                assert!(anim.show.is_some(), "{slot}.{} hides {weapon}", p.name);
                if !p.hitboxes.is_empty() {
                    assert!(!anim.angle.is_empty(), "{slot}.{} has no weapon angle", p.name);
                }
            }
        }
    }
    for a in &f.attachments {
        assert!(a.hidden, "{} must be hidden outside its moves", a.id);
        assert_eq!(a.joint, Joint::HandF, "{} is held", a.id);
        assert!(!a.shapes.is_empty(), "{} has no geometry", a.id);
        let id = a.id.as_str();
        assert!(["sword", "lance", "axe", "bow"].contains(&id));
    }
    // Every weapon is used by at least one move.
    for weapon in ["sword", "lance", "axe", "bow"] {
        assert!(expected.iter().any(|(_, w)| *w == Some(weapon)), "{weapon} unused");
    }
    // The whip-sword stretches for the long-reach swings.
    assert!(f.moves.dtilt.phases[0].attach["sword"].scale[0][1] > 1.8);
    assert!(f.moves.usmash.phases[0].attach["sword"].scale[0][1] > 1.8);
    // Areadbhar and Aymr are longer/heavier than the sword's resting blade.
    let len = |id: &str| -> f64 {
        f.attachments
            .iter()
            .find(|a| a.id == id)
            .unwrap()
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Line { from, to, .. } => Some((to[0] - from[0]).hypot(to[1] - from[1])),
                _ => None,
            })
            .fold(0., f64::max)
    };
    assert!(len("lance") > len("sword"));
    assert!(len("axe") > len("bow"));
}

#[test]
fn reference_active_windows_follow_the_pinned_ufd_rows() {
    // UFD gameplay frames; engine phase frame = source frame - 1.
    for (slot, total, windows) in [
        ("jab", 22, vec![(3, 4, 1.5)]),
        ("ftilt", 34, vec![(7, 9, 11.0)]),
        ("utilt", 34, vec![(8, 14, 10.0)]),
        ("dtilt", 35, vec![(12, 14, 8.0)]),
        ("dashatk", 44, vec![(8, 10, 9.0), (8, 10, 13.0)]),
        ("fsmash", 63, vec![(22, 24, 18.0), (22, 24, 12.0)]),
        ("usmash", 56, vec![(12, 15, 2.0), (16, 26, 2.0), (27, 28, 10.0)]),
        ("dsmash", 74, vec![(18, 20, 23.0), (28, 30, 23.0)]),
        ("nair", 51, vec![(5, 25, 2.5), (27, 27, 3.0)]),
        ("fair", 39, vec![(11, 12, 12.75), (11, 12, 8.5)]),
        ("bair", 46, vec![(12, 13, 15.0), (12, 13, 10.0), (14, 16, 7.5), (14, 16, 5.0)]),
        ("uair", 48, vec![(9, 18, 3.0), (19, 22, 5.5)]),
        ("dair", 59, vec![(21, 23, 19.0)]),
        ("grab", 39, vec![(5, 6, 0.0)]),
        ("dashgrab", 48, vec![(9, 10, 0.0)]),
    ] {
        let d = common::data();
        let mut w = start(&d, slot);
        for t in 0..total {
            let bs = combat::active_boxes_at(&d, &w.fighters[0], w.frame);
            let expected = windows.iter().find(|(a, b, _)| (*a..=*b).contains(&t));
            assert_eq!(!bs.is_empty(), expected.is_some(), "{slot} frame {t}: {bs:?}");
            if let Some((_, _, damage)) = expected {
                assert!(bs.iter().any(|b| (b.hit.damage - damage).abs() < 1e-9), "{slot} frame {t}: {bs:?}");
            }
            step_move(&mut w, &d, sim::input::Intent::default());
        }
        assert!(w.fighters[0].mov.is_none(), "{slot}: total {total}");
    }
}

#[test]
fn reference_damage_and_launch_values() {
    let d = common::data();
    let f = d.fighter("verdant").unwrap();
    let first = |slot: &str| f.moves.get(slot).unwrap().phases.iter().flat_map(|p| &p.hitboxes).next().unwrap().clone();
    // FSmash: 18% clean Areadbhar tip (36°, BKB 60, KBG 0.90) over a 12% Sakurai body hit.
    let fs = f.moves.fsmash.phases[0].hitboxes.clone();
    assert!(fs.iter().any(|b| (b.damage - 18.0).abs() < 1e-9
        && (b.angle - 36.).abs() < 1e-9
        && (b.bkb - 60.).abs() < 1e-9
        && (b.kbg - 0.9).abs() < 1e-9
        && (b.hitlag_mult - 1.45).abs() < 1e-9));
    assert!(fs.iter().any(|b| (b.damage - 12.0).abs() < 1e-9 && b.kind == HitType::Sakurai));
    // Jab 1 is a 1.5% Sakurai poke; jab 3 hits 4.5%.
    assert_eq!(first("jab").damage, 1.5);
    assert_eq!(first("jab").kind, HitType::Sakurai);
    assert!(f.moves.jab.phases[2].hitboxes.iter().all(|b| (b.damage - 4.5).abs() < 1e-9));
    // DSweep: 23% Aymr both sides, 8 shield damage, angle 63.
    let ds = &f.moves.dsmash.phases[0].hitboxes;
    assert!(ds.iter().all(|b| (b.damage - 23.0).abs() < 1e-9 && (b.angle - 63.).abs() < 1e-9));
    assert!(ds.iter().all(|b| (b.shield_dmg_mult - (23. + 8.) / 23.).abs() < 1e-3));
    // DAir: 19% Aymr spike in the air, 72° on the ground.
    let da = &f.moves.dair.phases[1].hitboxes;
    assert!(da.iter().any(|b| b.kind == HitType::Spike && b.situation == Situation::Air));
    assert!(da.iter().any(|b| (b.angle - 72.).abs() < 1e-9 && b.situation == Situation::Ground));
    // USmash: 2% autolink multihits then a 10% launch in its own phase (so it can hit an already-hit victim).
    let us: Vec<_> = f.moves.usmash.phases.iter().flat_map(|p| &p.hitboxes).collect();
    assert!(us.iter().any(|b| (b.damage - 10.0).abs() < 1e-9 && (b.angle - 79.).abs() < 1e-9));
    assert!(us.iter().filter(|b| (b.damage - 2.0).abs() < 1e-9).all(|b| b.kind == HitType::Autolink));
    // NSpec carries the two source arrows: 12% uncharged / 29% at full charge.
    assert_eq!(f.projectiles["arrow"].hit.damage, 12.0);
    assert_eq!(f.projectiles["arrowMax"].hit.damage, 29.0);
    assert_eq!(f.projectiles["arrowMax"].hit.kind, HitType::Sakurai);
    // DSpec main hit is grounded 30% / aerial 25% with the source's 36% damage armour.
    let g = f.moves.dspec.phases.iter().find(|p| p.name == "impact").unwrap();
    assert_eq!(g.hitboxes[0].damage, 30.0);
    let a = f.moves.dspec.phases.iter().find(|p| p.name == "impactAir").unwrap();
    assert_eq!(a.hitboxes[0].damage, 25.0);
    assert_eq!(f.moves.dspec.phases.iter().find(|p| p.name == "slam").unwrap().armor_threshold, Some(36.));
    // Throws: 6 / 7 / 9 / 5.
    assert_eq!(
        (f.throws.f.hit.damage, f.throws.b.hit.damage, f.throws.u.hit.damage, f.throws.d.hit.damage),
        (6., 7., 9., 5.)
    );
    // Movement/weight conversions.
    assert!((f.stats.run_speed - 1.43 * 3.5).abs() < 1e-9);
    assert!((f.stats.gravity - 0.089 * 6.).abs() < 1e-9);
}

#[test]
fn duration_ledger_matches_the_pinned_totals() {
    let d = common::data();
    for (slot, total) in [
        ("jab", 22),
        ("ftilt", 34),
        ("utilt", 34),
        ("dtilt", 35),
        ("dashatk", 44),
        ("fsmash", 63),
        ("usmash", 56),
        ("dsmash", 74),
        ("nair", 51),
        ("fair", 39),
        ("bair", 46),
        ("uair", 48),
        ("dair", 59),
        ("grab", 39),
        ("dashgrab", 48),
        ("nspec", 80),
        ("sspec", 65),
        ("uspec", 46),
        ("dspec", 133),
    ] {
        assert_eq!(chain_frames(&d, slot), total, "{slot} total");
    }
    let f = d.fighter("verdant").unwrap();
    assert_eq!(f.moves.jab.phases[1].frames, Some(24), "jab 2");
    assert_eq!(f.moves.jab.phases[2].frames, Some(34), "jab 3");
    assert_eq!(f.moves.jab.phases[5].frames, Some(50), "rapid finisher");
    // Aerial landing lag runs before the shared landing state.
    assert_eq!(f.moves.nair.phases[1].transitions[0].lag, Some(11));
    for (slot, lag) in [("fair", 11), ("bair", 13), ("uair", 14), ("dair", 28)] {
        let m = f.moves.get(slot).unwrap();
        let l = m.phases.iter().flat_map(|p| &p.transitions).find(|t| t.lag.is_some()).unwrap();
        assert_eq!(l.lag, Some(lag), "{slot} landing lag");
    }
}

#[test]
fn arrow_charge_tiers_pick_the_source_damage() {
    let d = common::data();
    let f = d.fighter("verdant").unwrap();
    let draw = &f.moves.nspec.phases[0];
    assert_eq!(draw.charge.as_ref().unwrap().frame, 43, "charge holds on the last draw frame");
    assert_eq!(draw.charge.as_ref().unwrap().max, 70, "70 held ticks reach the 114-frame full draw");
    // Hold from the start to the tier the charge picks.
    for (held, projectile, damage) in [(0, "arrow", 12.0), (130, "arrowMax", 29.0)] {
        let mut w = start(&d, "nspec");
        let mut intent = sim::input::Intent { sp: true, sp_held: true, ..Default::default() };
        for n in 0..held + 60 {
            if n == held {
                intent.sp_held = false;
            }
            step_move(&mut w, &d, intent);
            if !w.projs.is_empty() {
                break;
            }
        }
        let p = w.projs.first().unwrap_or_else(|| panic!("{projectile} never spawned"));
        assert_eq!(p.id, projectile, "held {held}");
        assert_eq!(p.hit.damage, damage);
    }
    // Releasing before the hold frame is reached still looses at the minimum draw.
    let mut w = start(&d, "nspec");
    for _ in 0..3 {
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    assert!(w.projs.is_empty());
    for _ in 0..50 {
        step_move(&mut w, &d, sim::input::Intent::default());
    }
    assert_eq!(w.projs.first().map(|p| p.hit.damage), Some(12.0));
}

#[test]
fn up_special_tethers_a_distant_ledge() {
    for (distance, frame, catches) in
        [(100., 2, false), (100., 7, true), (100., 9, true), (100., 16, true), (215., 9, false), (100., 20, false)]
    {
        let d = common::data();
        let mut w = start(&d, "uspec");
        for _ in 0..frame - 1 {
            step_move(&mut w, &d, sim::input::Intent::default());
        }
        let block = d.stages[w.stage].blocks[0].clone();
        {
            let f = &mut w.fighters[0];
            f.x = block.left - distance;
            f.y = block.top + 40.;
            f.vx = 0.;
            f.vy = -d.fighter("verdant").unwrap().stats.gravity;
            f.face = 1.;
            f.ledge_cd = 0;
            f.ground = false;
            f.surf = None;
        }
        step_move(&mut w, &d, sim::input::Intent::default());
        assert_eq!(w.fighters[0].state == State::Ledge, catches, "distance {distance}, frame {frame}");
    }
}

#[test]
fn reach_matches_the_fitted_weapon_geometry() {
    let d = common::data();
    let f = d.fighter("verdant").unwrap();
    // Furthest hitbox edge over the move's timelines. Long-reach weapon swings stay inside the authored
    // weapon length (hand offset + blade × maximum phase scale) and short pokes stay short.
    let edge = |slot: &str| -> f64 {
        let m = f.moves.get(slot).unwrap();
        m.timelines()
            .flat_map(|p| p.iter())
            .flat_map(|p| &p.hitboxes)
            .map(|b| b.frames.iter().map(|k| k[1].hypot(k[2]) + k[3]).fold(b.x.hypot(b.y) + b.r, f64::max))
            .fold(0., f64::max)
    };
    for (slot, min, max) in [
        ("jab", 60., 135.),
        ("ftilt", 70., 120.),
        ("utilt", 100., 140.),
        ("dtilt", 120., 175.), // whip
        ("dashatk", 90., 130.),
        ("fsmash", 130., 180.), // Areadbhar
        ("usmash", 140., 175.), // stretched whip
        ("dsmash", 80., 115.),  // Aymr
        ("nair", 50., 100.),
        ("fair", 120., 175.), // Areadbhar
        ("bair", 110., 160.),
        ("uair", 140., 175.),
        ("dair", 50., 90.),
        ("grab", 45., 80.),
        ("sspec", 130., 180.), // Areadbhar lunge
        ("uspec", 160., 225.), // tether whip
    ] {
        let e = edge(slot);
        assert!(e >= min && e <= max, "{slot} reach {e} outside {min}..{max}");
    }
    // The lance outranges the axe, and the whip stretches past the resting sword.
    assert!(edge("fsmash") > edge("dsmash"));
    assert!(edge("dtilt") > edge("ftilt"));

    // Contact: the Areadbhar fsmash connects at 120 px and misses at 200.
    for (slot, frame, hit_at, miss_at) in [("fsmash", 22, 120., 200.), ("dtilt", 12, 100., 160.)] {
        for (distance, connects) in [(hit_at, true), (miss_at, false)] {
            let mut w = start(&d, slot);
            w.fighters[1].x = w.fighters[0].x + distance;
            for _ in 0..frame {
                step_move(&mut w, &d, sim::input::Intent::default());
            }
            let mut hit = false;
            for _ in 0..8 {
                combat::hit_pass(&mut w, &d);
                hit |= w.fighters[1].dmg > 0.;
                step_move(&mut w, &d, sim::input::Intent::default());
            }
            assert_eq!(hit, connects, "{slot} at {distance}px");
        }
    }
}

#[test]
fn every_move_completes_and_is_actionable() {
    let d = common::data();
    for slot in sim::data::Moves::SLOTS {
        let m = d.fighter("verdant").unwrap().moves.get(slot);
        let Some(_) = m else { continue };
        let mut w = start(&d, slot);
        for _ in 0..200 {
            step_move(&mut w, &d, sim::input::Intent::default());
            if w.fighters[0].mov.is_none() {
                break;
            }
        }
        assert!(w.fighters[0].mov.is_none(), "{slot} never ended (phase {:?})", phase_name(&d, &w));
    }
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
        (Input { buttons: 4, stick: (0, 127), ..Default::default() }, "uspec"),
        (Input { buttons: 4, stick: (0, -127), ..Default::default() }, "dspec"),
        (Input { buttons: 8, ..Default::default() }, "grab"),
        (Input { buttons: 8, stick: (127, 0), ..Default::default() }, "dashgrab"),
    ] {
        let players = vec![
            PlayerConfig { tap_jump: false, ..PlayerConfig::human("verdant") },
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
    // Jump + attack takes the short-hop aerial route, and the attack stick picks an aerial.
    let players = vec![
        PlayerConfig { tap_jump: true, ..PlayerConfig::human("verdant") },
        PlayerConfig { tap_jump: false, ..PlayerConfig::human("brawler") },
    ];
    let mut w = common::sandbox(&d, "arena", players);
    w.count = 0;
    tick(&mut w, &d, Input { buttons: 1 | 2, ..Default::default() });
    for _ in 0..8 {
        tick(&mut w, &d, Input { buttons: 1, ..Default::default() });
    }
    let m = w.fighters[0].mov.as_ref().expect("jump + attack aerial");
    assert_eq!(m.slot, "nair", "neutral short-hop aerial");
}

/// Step a move against a pinned victim and list `(tick, damage)` for each connecting hit. Pinning isolates the
/// move's contact bookkeeping from knockback: a phase that clears contacts lets the same victim be hit again.
fn pinned_hits(d: &sim::GameData, slot: &str, at: impl Fn(usize) -> (f64, f64, bool)) -> Vec<(u64, f64)> {
    let mut w = start(d, slot);
    let mut seen = vec![];
    for tick in 0..120 {
        // A still attacker: clear its own hitlag so every hit window can be re-tested.
        w.fighters[0].lag = 0;
        combat::hit_pass(&mut w, d);
        for hit in w.hits.drain(..) {
            seen.push((tick, hit.damage));
        }
        if w.fighters[0].mov.is_none() {
            break;
        }
        let (dx, dy, airborne) = at(seen.len());
        let (ax, ay) = (w.fighters[0].x, w.fighters[0].y);
        {
            let v = &mut w.fighters[1];
            v.x = ax + dx;
            v.y = ay - dy;
            v.ground = !airborne;
            v.vx = 0.;
            v.vy = 0.;
            v.lag = 0;
            v.inv = 0;
            v.blink = 0;
            v.flash = 0;
            v.set_state(State::Idle);
        }
        step_move(&mut w, d, sim::input::Intent::default());
    }
    seen
}

#[test]
fn multihits_chain_into_the_finisher_on_one_victim() {
    let d = common::data();
    // Neutral air: multihit body circles then the 3.0 finisher, all inside one move.
    let nair = pinned_hits(&d, "nair", |_| (30., 30., false));
    assert!(nair.len() >= 3, "nair hits {nair:?}");
    assert!((nair.last().unwrap().1 - 3.0).abs() < 1e-9, "nair must end on the 3% finisher: {nair:?}");
    // Up air: 3.0 multihits above the head then the 5.5 finisher.
    let uair = pinned_hits(&d, "uair", |_| (10., 120., true));
    assert!(uair.len() >= 2, "uair hits {uair:?}");
    assert!((uair.last().unwrap().1 - 5.5).abs() < 1e-9, "uair must end on the 5.5% finisher: {uair:?}");
    // Up smash: 2.0 autolink rehits then the 10.0 finisher.
    let usmash = pinned_hits(&d, "usmash", |_| (10., 100., true));
    assert!(usmash.len() >= 3, "usmash hits {usmash:?}");
    assert!((usmash.last().unwrap().1 - 10.0).abs() < 1e-9, "usmash must end on the 10% finisher: {usmash:?}");
}

#[test]
fn down_smash_hits_the_same_victim_with_both_swings() {
    let d = common::data();
    // The front swing hits a victim in front, the back swing the same victim once it is behind.
    let hits = pinned_hits(&d, "dsmash", |n| if n == 0 { (52., 0., false) } else { (-50., 0., false) });
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert!(hits.iter().all(|(_, dmg)| (dmg - 23.0).abs() < 1e-9), "{hits:?}");
    // Separate phases, so the second swing clears the first swing's contact history.
    let f = d.fighter("verdant").unwrap();
    let front = f.moves.dsmash.phases.iter().find(|p| p.name == "front").unwrap();
    let back = f.moves.dsmash.phases.iter().find(|p| p.name == "back").unwrap();
    assert_eq!((front.frames, back.frames), (Some(28), Some(46)));
}

#[test]
fn grab_connects_at_close_range_only() {
    let d = common::data();
    for (distance, grabs) in [(40., true), (55., true), (90., false)] {
        let mut w = start(&d, "grab");
        w.fighters[1].x = w.fighters[0].x + distance;
        for _ in 0..10 {
            w.fighters[0].lag = 0;
            combat::hit_pass(&mut w, &d);
            if w.fighters[1].state == State::Grabbed {
                break;
            }
            step_move(&mut w, &d, sim::input::Intent::default());
        }
        assert_eq!(w.fighters[1].state == State::Grabbed, grabs, "grab at {distance}px");
    }
}

#[test]
fn saturated_charge_still_looses_the_charged_arrow() {
    let d = common::data();
    let mut w = start(&d, "nspec");
    let held = sim::input::Intent { sp: true, sp_held: true, ..Default::default() };
    // Charge 70 ticks, then hold through the 46 extra hold frames; the phase ends on its own timeout and must
    // still pick the full-charge tier (the source's arrow is already fully drawn).
    for _ in 0..220 {
        step_move(&mut w, &d, held);
        if !w.projs.is_empty() {
            break;
        }
    }
    let p = w.projs.first().expect("a held full charge must still loose");
    assert_eq!(p.id, "arrowMax");
    assert_eq!(p.hit.damage, 29.0);
    // Releasing before the hold frame still looses on the source's 45th move frame (the entry tick counts).
    let mut w = start(&d, "nspec");
    let mut steps = 0;
    for n in 0..60 {
        step_move(&mut w, &d, if n < 2 { held } else { sim::input::Intent::default() });
        steps += 1;
        if !w.projs.is_empty() {
            break;
        }
    }
    assert_eq!(steps + 1, 45, "uncharged arrow leaves on source frame 45");
    assert_eq!(w.projs[0].id, "arrow");
}

#[test]
fn side_special_air_branch_and_sweetspot_delay() {
    let d = common::data();
    let f = d.fighter("verdant").unwrap();
    let m = f.moves.sspec.as_ref().unwrap();
    let ground = m.phases.iter().find(|p| p.name == "thrust").unwrap();
    let air = m.phases.iter().find(|p| p.name == "airThrust").unwrap();
    // Ground sweetspot 17.25 spawns one frame after its 11.5 sour boxes; the aerial branch is weaker.
    assert!(ground.hitboxes.iter().any(|b| (b.damage - 17.25).abs() < 1e-9 && b.start == 1));
    assert!(ground.hitboxes.iter().any(|b| (b.damage - 11.5).abs() < 1e-9 && b.start == 0));
    assert!(air.hitboxes.iter().any(|b| (b.damage - 14.25).abs() < 1e-9));
    assert!(air.hitboxes.iter().any(|b| (b.damage - 9.5).abs() < 1e-9));
    assert_eq!(ground.frames, Some(5));
    assert_eq!(air.frames, Some(5));
    // Grounded total 65, aerial total 61 (source): wind 19 + thrust 5 + recovery.
    let frames = |name: &str| m.phases.iter().find(|p| p.name == name).unwrap().frames.unwrap();
    assert_eq!(frames("wind") + frames("thrust") + frames("recover"), 65);
    assert_eq!(frames("wind") + frames("airThrust") + frames("airRecover"), 61);
}

#[test]
fn down_special_impact_timing_and_armour() {
    let d = common::data();
    let f = d.fighter("verdant").unwrap();
    let slam = f.moves.dspec.phases.iter().find(|p| p.name == "slam").unwrap();
    let impact = f.moves.dspec.phases.iter().find(|p| p.name == "impact").unwrap();
    let raise = f.moves.dspec.phases.iter().find(|p| p.name == "raise").unwrap();
    // Source gameplay frames 1-61 are the wind-up, so the impact window starts on frame 62 (`start: 0` -> 62).
    assert_eq!(raise.frames.unwrap() + slam.frames.unwrap(), 61);
    assert_eq!(raise.frames, Some(33));
    assert_eq!(slam.armor_threshold, Some(36.));
    assert_eq!(slam.frames, Some(28));
    assert_eq!(impact.armor_threshold, Some(36.));
    assert_eq!(impact.hitboxes[0].damage, 30.0);
    assert_eq!((impact.hitboxes[0].start, impact.hitboxes[0].end), (0, Some(5)));
    assert_eq!(chain_frames(&d, "dspec"), 133);
}

#[test]
fn separated_bow_windows_connect_in_order() {
    assert_eq!(common::contact_sequence(&common::data(), "verdant", "nair"), [2.5, 2.5, 2.5, 3.]);
}

#[test]
fn loop_seam_finisher_has_an_independent_identity() {
    let d = common::data();
    assert_eq!(common::contact_sequence(&d, "verdant", "uair"), [3., 3., 5.5]);
    assert_eq!(common::contact_sequence(&d, "verdant", "usmash"), [2., 2., 2., 2., 10.]);
}
