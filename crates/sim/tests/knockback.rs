mod common;
use sim::{combat, data::Hitbox};

#[test]
fn roster_estimates_preserve_uncharged_middleweight_launches() {
    let d = common::data();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut checked = 0;
    for (file, hits) in common::knockback_baseline() {
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(&file)).unwrap()).unwrap();
        for (path, [_, base, growth, damage]) in hits {
            let mut b: Hitbox = serde_json::from_value(json.pointer(&path).unwrap().clone()).unwrap();
            b.damage = damage;
            for before in [0., 50., 100., 150., 250.] {
                let after = before + damage;
                let expected = if b.set_kb { base } else { base + after * growth * 1.4 };
                let actual = combat::knockback(&d, &b, after, 1.);
                assert!((actual - expected).abs() < 1e-7, "{file}{path} at {before}: {actual} vs {expected}");
            }
            checked += 1;
        }
    }
    assert!(checked > 300);
}

#[test]
fn shared_formula_handles_damage_charge_weight_and_fixed_hits() {
    let d = common::data();
    for fighter in &d.fighters {
        let mut b = fighter.moves.jab.phases[0].hitboxes[0].clone();
        b.set_kb = false;
        let normal = combat::knockback(&d, &b, 100. + b.damage, 1.);
        let light = combat::knockback(&d, &b, 100. + b.damage, 1.25);
        let heavy = combat::knockback(&d, &b, 100. + b.damage, 0.8);
        assert!(light > normal && normal > heavy, "{}", fighter.id);
        b.damage *= 1.4;
        assert!(combat::knockback(&d, &b, 100. + b.damage, 1.) > normal, "{}", fighter.id);
        b.set_kb = true;
        assert_eq!(combat::knockback(&d, &b, 0., 1.), combat::knockback(&d, &b, 250., 1.));
    }
}
