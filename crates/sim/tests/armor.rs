mod common;
use sim::{MatchConfig, PlayerConfig, World, combat, data::HitType, moves, world::State};

#[test]
fn phase_armor_takes_damage_but_preserves_motion_and_recovery() {
    let mut d = common::data();
    let id = d.fighters.iter().position(|f| f.id == "brawler").unwrap();
    d.fighters[id].moves.jab.phases[0].armor_threshold = Some(6.);
    for projectile in [false, true] {
        for (damage, armored) in [(5., true), (6., true), (6.1, false)] {
            let mut w = World::new(
                MatchConfig {
                    training: true,
                    players: vec![PlayerConfig::human("brawler"), PlayerConfig::human("brawler")],
                    ..Default::default()
                },
                &d,
            )
            .unwrap();
            let mut f = w.fighters[1].clone();
            moves::start_move(&mut w, &d, 1, &mut f, "jab");
            f.vx = 3.;
            f.vy = -2.;
            f.up_b = true;
            f.ad_used = true;
            w.fighters[1] = f;
            let mut b = d.fighters[id].moves.jab.phases[0].hitboxes[0].clone();
            b.damage = damage;
            b.kind = HitType::Trip;
            combat::apply_hit(
                &mut w,
                &d,
                1,
                &b,
                combat::HitSource { attacker: 0, direction: 1., point: (0., 0.), projectile, label: None },
            );
            let f = &w.fighters[1];
            assert_eq!(f.dmg, damage);
            assert_eq!(f.last_hit_by, Some(0));
            assert!(f.lag > 0);
            assert_eq!(f.state == State::Attack, armored);
            assert_eq!(f.up_b, armored);
            assert_eq!(f.ad_used, armored);
            if armored {
                assert_eq!((f.vx, f.vy), (3., -2.));
                assert!(f.ground && f.mov.is_some());
            } else {
                assert_eq!(f.state, State::Knockdown);
            }
            assert_eq!(w.hits.len(), 1);
        }
    }
}

#[test]
fn grabs_bypass_phase_armor() {
    let mut d = common::data();
    let id = d.fighters.iter().position(|f| f.id == "brawler").unwrap();
    d.fighters[id].moves.jab.phases[0].armor_threshold = Some(100.);
    let mut w = World::new(
        MatchConfig {
            training: true,
            players: vec![PlayerConfig::human("brawler"), PlayerConfig::human("brawler")],
            ..Default::default()
        },
        &d,
    )
    .unwrap();
    w.fighters[1].x = w.fighters[0].x + 34.;
    for (i, slot) in [(0, "grab"), (1, "jab")] {
        let mut f = w.fighters[i].clone();
        moves::start_move(&mut w, &d, i, &mut f, slot);
        f.mov.as_mut().unwrap().t = if i == 0 { 6 } else { 0 };
        w.fighters[i] = f;
    }
    combat::hit_pass(&mut w, &d);
    assert_eq!(w.fighters[1].state, State::Grabbed);
}
