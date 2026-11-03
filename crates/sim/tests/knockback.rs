mod common;
use sim::{combat, data::Hitbox};

/// The recorded TS-era launch values are the linear-compatibility contract: TS traces restore these fields
/// (`common::linear_data`) and disable the Ultimate equation. Current roster growth has since been rebalanced
/// (see `docs/BALANCE_ORIGINAL_ROSTER.md`), so this checks the restored values still reproduce the recorded
/// launches through the fallback formula rather than asserting the live content still equals the migration fit.
#[test]
fn linear_compatibility_restores_recorded_launches() {
    let d = common::linear_data();
    assert!(d.global.knockback.ultimate.is_none());
    let mut checked = 0;
    for (file, hits) in common::knockback_baseline() {
        let targets = if file == "global.json" {
            vec![serde_json::to_value(&d.global).unwrap()]
        } else if file == "fighters/base/fighter.json" {
            // These fighters must inherit the restored base throws, not their new growth overrides.
            ["archer", "brawler", "dancer", "golem", "lancer", "swordfighter", "tinker"]
                .iter()
                .map(|id| serde_json::to_value(d.fighter(id).unwrap()).unwrap())
                .collect()
        } else {
            let id = file.strip_prefix("fighters/").unwrap().strip_suffix("/fighter.json").unwrap();
            vec![serde_json::to_value(d.fighter(id).unwrap()).unwrap()]
        };
        for json in targets {
            for (path, [damage, base, growth, _]) in &hits {
                let node = json.pointer(path).unwrap_or_else(|| panic!("missing restored {file}{path}"));
                let b: Hitbox = serde_json::from_value(node.clone()).unwrap();
                assert_eq!([b.damage, b.bkb, b.kbg], [*damage, *base, *growth], "{file}{path}");
                for before in [0., 50., 100., 150., 250.] {
                    let after = before + damage;
                    let expected = if b.set_kb { *base } else { base + after * growth * 1.4 };
                    let actual = combat::knockback(&d, &b, after, 1.);
                    assert!((actual - expected).abs() < 1e-7, "{file}{path} at {before}: {actual} vs {expected}");
                }
                checked += 1;
            }
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
