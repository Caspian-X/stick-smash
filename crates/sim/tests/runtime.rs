mod common;
use sim::{
    GameData, HitRecord, MatchConfig, PlayerConfig, World, combat,
    data::{Num, moves::Moves},
    input::Intent,
    moves, states, step_world,
    world::State,
};

fn world(d: &GameData) -> World {
    World::new(
        MatchConfig {
            training: true,
            players: vec![PlayerConfig::human("brawler"), PlayerConfig::human("brawler")],
            ..Default::default()
        },
        d,
    )
    .unwrap()
}

#[test]
fn pummels_and_hits_cap_damage_but_credit_nominal_damage() {
    let d = common::data();
    for pummel in [true, false] {
        let damage = if pummel { d.global.grab.pummel_damage } else { 5. };
        for initial in [d.global.sim.max_damage, d.global.sim.max_damage - damage / 2.] {
            let mut w = world(&d);
            w.fighters[1].dmg = initial;
            if pummel {
                w.fighters[0].set_state(State::Hold);
                w.fighters[0].sd.victim = Some(1);
                w.fighters[0].sd.timer = 100.;
                w.fighters[0].sd.pt = d.global.grab.pummel_hit_frame + 1;
                w.fighters[1].set_state(State::Grabbed);
                w.fighters[1].sd.by = Some(0);
                states::update_fighter(&mut w, &d, 0, Intent::default());
                assert_eq!(w.fighters[1].state, State::Grabbed);
                assert_eq!(w.fighters[1].lag, d.global.grab.pummel_hitlag);
            } else {
                let mut b = d.fighter("brawler").unwrap().moves.jab.phases[0].hitboxes[0].clone();
                b.damage = damage;
                combat::apply_hit(
                    &mut w,
                    &d,
                    1,
                    &b,
                    combat::HitSource {
                        attacker: 0,
                        direction: 1.,
                        point: (0., 0.),
                        projectile: false,
                        label: Some("test".into()),
                    },
                );
            }
            assert_eq!(w.fighters[1].dmg, d.global.sim.max_damage);
            assert_eq!(w.fighters[1].last_hit_by, Some(0));
            assert_eq!(w.hits, [HitRecord { attacker: Some(0), victim: 1, damage, shielded: false }]);
        }
    }
}

#[test]
fn effect_ranges_and_charge_particles_use_the_selected_rollback_rng() {
    let mut d = common::data();
    let id = d.fighters.iter().position(|f| f.id == "brawler").unwrap();
    let p = &mut d.fighters[id].moves.fsmash.phases[0];
    p.charge =
        Some(sim::data::moves::Charge { frame: 0, max: 60, hold_after_max: 0, button: Default::default(), jet: true });
    let fx = [sim::data::moves::Fx::Part {
        x: Num::Range([1., 2.]),
        y: Num::Range([3., 4.]),
        vx: Num::Range([5., 6.]),
        vy: Num::Range([7., 8.]),
        life: Num::Range([9., 10.]),
        size: Num::Range([11., 12.]),
        color: sim::data::Color::Player,
        gravity: 0.,
        inherit_vel: 0.,
    }];
    for separate in [true, false] {
        d.global.sim.separate_cosmetic_rng = separate;
        let mut w = world(&d);
        let mut f = w.fighters[0].clone();
        moves::start_move(&mut w, &d, 0, &mut f, "fsmash");
        f.mov.as_mut().unwrap().charge = 2;
        let before = w.clone();
        let run = |w: &mut World, f: &mut sim::world::Fighter| {
            moves::effects(w, &fx, (0., 0.), 1., (0., 0.), 0);
            moves::st_attack(w, &d, 0, f, Intent { atk_held: true, ..Default::default() }, 0.);
        };
        let mut restored = before.clone();
        let mut restored_f = f.clone();
        run(&mut w, &mut f);
        run(&mut restored, &mut restored_f);
        assert_eq!(w, restored);
        assert_eq!(f, restored_f);
        let mut expected = before.cosmetic_rng.unwrap_or(before.rng);
        for _ in 0..13 {
            expected.next_f64();
        }
        if separate {
            assert_eq!(w.rng, before.rng);
            assert_eq!(w.cosmetic_rng, Some(expected));
        } else {
            assert_eq!(w.rng, expected);
            assert_eq!(w.cosmetic_rng, None);
        }
    }
}

#[test]
fn extra_cosmetic_effects_do_not_change_cpu_gameplay() {
    let d = common::data();
    assert!(d.global.sim.separate_cosmetic_rng);
    let mut a = World::new(MatchConfig { training: true, ..Default::default() }, &d).unwrap();
    let mut b = a.clone();
    let fx = [sim::data::moves::Fx::Dust { x: Num::Range([-5., 5.]), y: Num::Range([0., 10.]), n: 2 }];
    for _ in 0..240 {
        moves::effects(&mut b, &fx, (0., 0.), 1., (0., 0.), 0);
        step_world(&mut a, &d, &[]);
        step_world(&mut b, &d, &[]);
        a.fx.clear();
        b.fx.clear();
        let mut gameplay = b.clone();
        gameplay.cosmetic_rng = a.cosmetic_rng;
        assert_eq!(a, gameplay);
    }
    assert_ne!(a.rng.0, a.cfg.seed);
    assert_ne!(a.cosmetic_rng, b.cosmetic_rng);
}

#[test]
fn move_slots_handle_optional_and_unknown_moves_explicitly() {
    let d = common::data();
    let mut moves = d.fighter("brawler").unwrap().moves.clone();
    moves.sspec = Some(moves.nspec.clone());
    moves.zair = Some(moves.nair.clone());
    assert_eq!(moves.iter().map(|(slot, _)| slot).collect::<Vec<_>>(), Moves::SLOTS);
    for slot in Moves::SLOTS {
        let name = format!("changed {slot}");
        moves.get_mut(slot).unwrap().name.clone_from(&name);
        assert_eq!(moves.get(slot).unwrap().name, name);
    }
    moves.sspec = None;
    moves.zair = None;
    for slot in ["sspec", "zair", "unknown"] {
        assert!(moves.get(slot).is_none());
        assert!(moves.get_mut(slot).is_none());
        assert!(!moves.iter().any(|(s, _)| s == slot));
    }
    let mut w = world(&d);
    let mut f = w.fighters[0].clone();
    moves::special(&mut w, &d, 0, &mut f, Intent::default(), 1.);
    assert_eq!(f.mov.as_ref().unwrap().slot, "nspec");
}

#[test]
#[should_panic(expected = "known, present move slot")]
fn invalid_runtime_move_does_not_fall_back_to_neutral_special() {
    let d = common::data();
    let mut w = world(&d);
    let mut f = w.fighters[0].clone();
    moves::start_move(&mut w, &d, 0, &mut f, "unknown");
}

#[test]
fn hit_history_tracks_each_victim_rehit_tick_and_resets_on_phase_entry() {
    let mut d = common::data();
    let id = d.fighters.iter().position(|f| f.id == "brawler").unwrap();
    for (rehit, preserve_hits) in [(None, false), (Some(3), false), (None, true), (Some(3), true)] {
        let p = &mut d.fighters[id].moves.jab.phases[0];
        p.frames = Some(100);
        p.hitboxes.truncate(1);
        p.hitboxes[0].start = 0;
        p.hitboxes[0].end = None;
        p.hitboxes[0].r = 1000.;
        p.hitboxes[0].frames.clear();
        p.hitboxes[0].rehit = rehit;
        p.transitions = vec![sim::data::moves::Transition {
            on: sim::data::Trigger::Release,
            cond: None,
            from: None,
            min_charge: 0,
            needs: None,
            to: "again".into(),
            lag: None,
            preserve_hits,
        }];
        let mut again = p.clone();
        again.name = "again".into();
        again.transitions.clear();
        d.fighters[id].moves.jab.phases = vec![p.clone(), again];
        let mut w = World::new(
            MatchConfig { training: true, players: vec![PlayerConfig::human("brawler"); 3], ..Default::default() },
            &d,
        )
        .unwrap();
        for f in &mut w.fighters {
            f.x = 0.;
            f.y = 0.;
            f.set_state(State::Shield);
        }
        let mut f = w.fighters[0].clone();
        moves::start_move(&mut w, &d, 0, &mut f, "jab");
        w.fighters[0] = f;
        for (tick, contacts) in [(0, 2), (2, 2), (3, 4), (5, 4), (6, 6)] {
            w.fighters[0].lag = 0;
            w.fighters[0].mov.as_mut().unwrap().t = tick;
            w.fighters[0].mov.as_mut().unwrap().elapsed = tick;
            combat::hit_pass(&mut w, &d);
            assert_eq!(w.hits.len(), if rehit.is_some() { contacts } else { 2 });
            assert!(w.hits.iter().all(|h| h.shielded));
            assert_eq!(w.hits.iter().map(|h| h.victim).collect::<Vec<_>>(), [1, 2].repeat(w.hits.len() / 2));
        }
        let before = w.hits.len();
        let mut f = w.fighters[0].clone();
        assert!(moves::trigger(&mut w, &d, 0, &mut f, sim::data::Trigger::Release));
        f.lag = 0;
        w.fighters[0] = f;
        combat::hit_pass(&mut w, &d);
        assert_eq!(w.hits.len(), before + if preserve_hits { 0 } else { 2 });
        if preserve_hits && rehit.is_some() {
            w.fighters[0].lag = 0;
            let m = w.fighters[0].mov.as_mut().unwrap();
            m.t = 3;
            m.elapsed = 9;
            combat::hit_pass(&mut w, &d);
            assert_eq!(w.hits.len(), before + 2);
        }
    }
}

/// Golden determinism: a fixed-seed CPU match with `separateCosmeticRng` off and effect-range moves injected
/// (so cosmetic draws advance the shared gameplay RNG) must always end on the same World hash. The hash is
/// FNV-1a over the world's Debug text; update the constant only when a deliberate change is intended.
#[test]
fn cpu_match_golden_world_hash() {
    fn fnv1a(bytes: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            h = (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3);
        }
        h
    }

    let mut d = common::data();
    d.global.sim.separate_cosmetic_rng = false;
    for f in &mut d.fighters {
        for slot in ["jab", "fsmash"] {
            let m = f.moves.get_mut(slot).unwrap();
            m.phases[0].events.push(sim::data::moves::Event {
                at: Some(0),
                fx: vec![sim::data::moves::Fx::Part {
                    x: Num::Range([-6., 6.]),
                    y: Num::Range([0., 8.]),
                    vx: Num::Range([-3., 3.]),
                    vy: Num::Range([-2., 6.]),
                    life: Num::Range([10., 24.]),
                    size: Num::Range([2., 7.]),
                    color: sim::data::Color::Player,
                    gravity: 0.,
                    inherit_vel: 0.,
                }],
                ..Default::default()
            });
        }
    }
    let mut w = World::new(
        MatchConfig {
            seed: 0x5eed_5eed,
            stocks: 1,
            players: vec![
                PlayerConfig { cpu: Some(2), ..PlayerConfig::human("brawler") },
                PlayerConfig { cpu: Some(2), ..PlayerConfig::human("ninja") },
            ],
            ..Default::default()
        },
        &d,
    )
    .unwrap();
    for _ in 0..900 {
        step_world(&mut w, &d, &[]);
        w.fx.clear();
        w.hits.clear();
        w.acts.clear();
    }
    // The decaying launch model (`hitstun.ultimate`), Ultimate hitlag and tech lockout change this scenario's
    // trajectories and add `launch` / `tech_lock` to the Debug snapshot. Authored dodge / tech / getup windows and
    // dodge staling add `dodge_stale` / `dodge_at` and state `end` / `inv_at` / `inv_len`; subtractive momentum (`movement.ultimate`) changes movement too; replay SIM_VERSION -sim-31. The 5 px/unit
    // content unification (speeds, launch 0.15, Battlefield-sized Arena) changes it again (content only). The
    // Ultimate calibration pass (-sim-33: jump initial height, upward-launch gravity, Sakurai ramp, hitlag rounding,
    // `fkb`, float, per-fighter traction) adds `rise` / `top_launch` / `floating` / `float_used` and moves it again.
    // -sim-34 (ground float, Duchess's braking Bomber) leaves this scenario unchanged.
    assert_eq!(fnv1a(format!("{w:?}").as_bytes()), 0x516185616a1deed4);
}
