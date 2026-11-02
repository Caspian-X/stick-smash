//! Loads and validates every file in `assets/`, and keeps schemas and formatting up to date.
//! `BLESS=1 cargo test -p sim --test content` rewrites `assets/schema/` and reformats every asset in place.

use std::fs;
use std::path::{Path, PathBuf};

use sim::MatchConfig;
use sim::data::json::{merge, to_compact_string};
use sim::data::schema::schemas;
use sim::data::{Source, load};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn bless() -> bool {
    std::env::var_os("BLESS").is_some()
}

fn sources() -> Vec<Source> {
    let (sources, errors) = sim::data::sources(assets());
    assert!(errors.is_empty(), "{errors:?}");
    sources
}

#[test]
fn all_assets_load() {
    let (data, errors) = load(sources()).unwrap_or_else(|e| panic!("content failed to load:\n{}", fmt_errors(&e)));
    assert!(errors.is_empty(), "content errors:\n{}", fmt_errors(&errors));
    let ids: Vec<&str> = data.fighters.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "archer",
            "brawler",
            "dancer",
            "golem",
            "king",
            "lancer",
            "mike",
            "ninja",
            "swordfighter",
            "tinker",
            "wizard",
            "wrangler",
            "yoyo"
        ]
    );
    let ids: Vec<&str> = data.stages.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["arena", "citadel", "foundry", "isles"]);
    assert!(data.stages.iter().all(|s| s.max_players() >= 8));
    assert!(data.global.palette.len() >= 8);
}

#[test]
fn schemas_up_to_date() {
    let dir = assets().join("schema");
    for (name, schema) in schemas() {
        let text = to_compact_string(&schema);
        let path = dir.join(name);
        if bless() {
            fs::create_dir_all(&dir).unwrap();
            fs::write(&path, &text).unwrap();
        } else {
            assert_eq!(
                fs::read_to_string(&path).unwrap_or_default(),
                text,
                "{name} is stale: run BLESS=1 cargo test -p sim --test content"
            );
        }
    }
}

#[test]
fn assets_canonically_formatted() {
    let root = assets();
    for source in sources() {
        let p = root.join(source.path);
        let text = source.text;
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let canon = to_compact_string(&v);
        if bless() {
            fs::write(&p, canon).unwrap();
        } else {
            assert_eq!(
                text,
                canon,
                "{} isn't canonically formatted: run BLESS=1 cargo test -p sim --test content",
                p.display()
            );
        }
    }
}

#[test]
fn hash_ignores_key_order_and_formatting() {
    let a = load(sources()).unwrap().0.hash;
    let reordered: Vec<Source> = sources()
        .into_iter()
        .map(|s| {
            let v: serde_json::Value = serde_json::from_str(&s.text).unwrap();
            let mut m: Vec<_> = v.as_object().unwrap().clone().into_iter().collect();
            m.reverse();
            Source {
                path: s.path,
                text: serde_json::to_string(&serde_json::Value::Object(m.into_iter().collect())).unwrap(),
            }
        })
        .collect();
    assert_eq!(a, load(reordered).unwrap().0.hash);
}

#[test]
fn broken_package_is_skipped_with_path() {
    let mut src = sources();
    let f = src.iter_mut().find(|s| s.path == "fighters/king/fighter.json").unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&f.text).unwrap();
    merge(
        &mut v,
        &serde_json::json!({ "moves": { "jab": { "phases": [{ "name": "a", "frames": 5, "transitions": [{ "on": "timeout", "to": "nope" }] }] } } }),
    );
    f.text = v.to_string();
    let (data, errors) = load(src).unwrap();
    assert!(data.fighter("king").is_none() && data.fighter("brawler").is_some());
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].file, "fighters/king/fighter.json");
    assert_eq!(errors[0].path, "moves.jab.phases[0].transitions[0].to");
}

#[test]
fn type_errors_name_the_json_path() {
    let mut src = sources();
    let f = src.iter_mut().find(|s| s.path == "fighters/brawler/fighter.json").unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&f.text).unwrap();
    merge(&mut v, &serde_json::json!({ "stats": { "weight": "heavy" } }));
    f.text = v.to_string();
    let (_, errors) = load(src).unwrap();
    assert_eq!(errors[0].path, "stats.weight");
}

#[test]
fn user_file_overrides_builtin() {
    let mut src = sources();
    let brawler = src.iter().find(|s| s.path == "fighters/brawler/fighter.json").unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&brawler.text).unwrap();
    merge(&mut v, &serde_json::json!({ "name": "Modded" }));
    src.push(Source { path: "fighters/brawler/fighter.json".into(), text: v.to_string() });
    assert_eq!(load(src).unwrap().0.fighter("brawler").unwrap().name, "Modded");
}

fn fmt_errors(e: &[sim::data::ContentError]) -> String {
    e.iter().map(|e| format!("  {e}\n")).collect()
}

fn patched(file: &str, patch: serde_json::Value) -> Vec<Source> {
    let mut src = sources();
    let s = src.iter_mut().find(|s| s.path == file).unwrap();
    let mut v = serde_json::from_str(&s.text).unwrap();
    merge(&mut v, &patch);
    s.text = v.to_string();
    src
}

#[test]
fn invalid_fighter_numbers_skip_package_with_path() {
    use serde_json::json;
    let file = "fighters/king/fighter.json";
    for (patch, path) in [
        (json!({"stats":{"runSpeed":-1}}), "stats.runSpeed"),
        (
            json!({"moves":{"jab":{"phases":[{"name":"hit","frames":5,"armorThreshold":-1}]}}}),
            "moves.jab.phases[0].armorThreshold",
        ),
        (json!({"stats":{"weight":0}}), "stats.weight"),
        (json!({"moves":{"jab":{"chargeDamage":-1}}}), "moves.jab.chargeDamage"),
        (
            json!({"moves":{"jab":{"phases":[{"name":"main","frames":5,"events":[{"at":0,"chargeVelocity":-1}]}]}}}),
            "moves.jab.phases[0].events[0].chargeVelocity",
        ),
        (
            json!({"moves":{"jab":{"phases":[{"name":"main","frames":5,"transitions":[{"on":"targetNear","to":"end"}]}]}}}),
            "moves.jab.phases[0].transitions[0].on",
        ),
        (
            json!({"moves":{"jab":{"phases":[{"name":"main","frames":5,"detection":{"x1":5,"x2":0,"y1":0,"y2":80}}]}}}),
            "moves.jab.phases[0].detection",
        ),
        (
            json!({"projectiles":{"gordo":{"chargeScale":{"speed":-1,"damage":0}}}}),
            "projectiles.gordo.chargeScale.speed",
        ),
        (
            json!({"projectiles":{"gordo":{"explosion":{"armTicks":0,"radius":0}}}}),
            "projectiles.gordo.explosion.radius",
        ),
        (
            json!({"projectiles":{"gordo":{"explosion":{"armTicks":0,"radius":20,"proximity":-1}}}}),
            "projectiles.gordo.explosion.proximity",
        ),
        (json!({"stats":{"gravity":0}}), "stats.gravity"),
        (json!({"stats":{"fallSpeed":-1}}), "stats.fallSpeed"),
        (json!({"hurtboxes":{"default":{"x1":16,"x2":-16}}}), "hurtboxes.default"),
        (json!({"throws":{"f":{"release":0}}}), "throws.f"),
        (json!({"moves":{"jab":{"phases":[{"name":"hit","frames":-1}]}}}), "moves.jab.phases[0].frames"),
        (
            json!({"moves":{"jab":{"phases":[{"name":"hit","frames":5,"hitboxes":[{"x":0,"y":0,"r":0,"damage":1,"bkb":1,"kbg":0,"angle":0}]}]}}}),
            "moves.jab.phases[0].hitboxes[0].r",
        ),
        (
            json!({"moves":{"jab":{"phases":[{"name":"hit","frames":5,"events":[{"every":0}]}]}}}),
            "moves.jab.phases[0].events[0].every",
        ),
    ] {
        let (data, errors) = load(patched(file, patch)).unwrap();
        assert!(data.fighter("king").is_none());
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].file, file);
        assert_eq!(errors[0].path, path);
    }
}

#[test]
fn invalid_global_numbers_are_fatal() {
    use serde_json::json;
    for (patch, path) in [
        (json!({"audio":{"sampleRate":0}}), "audio.sampleRate"),
        (json!({"audio":{"strengthLevels":0}}), "audio.strengthLevels"),
        (json!({"audio":{"maxVoices":10000}}), "audio.maxVoices"),
        (json!({"audio":{"sounds":{"hit":{"layers":[]}}}}), "audio.sounds.hit.layers"),
        (json!({"knockback":{"chargeDamageDivisor":0}}), "knockback.chargeDamageDivisor"),
        (json!({"respawn":{"descendFrames":0}}), "respawn.descendFrames"),
        (json!({"hitTypes":{"windbox":{"maxSpeed":-1}}}), "hitTypes.windbox.maxSpeed"),
        (json!({"ledge":{"climb":{"a":0}}}), "ledge.climb"),
        (json!({"ledge":{"roll":{"a":10,"b":10}}}), "ledge.roll"),
    ] {
        let errors = load(patched("global.json", patch)).unwrap_err();
        assert_eq!(errors[0].file, "global.json");
        assert_eq!(errors[0].path, path);
    }
}

#[test]
fn presentation_values_that_disable_an_effect_still_load() {
    use serde_json::json;
    for patch in [
        json!({"render":{"select":{"dropScale":0}}}),
        json!({"render":{"hud":{"look":{"tagHeight":0,"anchorHeight":-10}}}}),
        json!({"render":{"select":{"pointer":{"slideSecs":0,"bounceSecs":0}}}}),
    ] {
        assert!(load(patched("global.json", patch.clone())).is_ok(), "{patch}");
    }
}

#[test]
fn invalid_stage_numbers_skip_package_with_path() {
    use serde_json::json;
    let file = "stages/arena/stage.json";
    for (patch, path) in [
        (json!({"blast":{"left":1600}}), "blast"),
        (json!({"camera":{"bottom":-1000}}), "camera"),
        (
            json!({"blocks":[{"left":300,"right":980,"top":520,"bottom":580,"underDepth":100,"underInset":0,"leftLedge":true,"rightLedge":true}]}),
            "blocks[0].underInset",
        ),
        (
            json!({"blocks":[{"left":300,"right":980,"top":520,"bottom":580,"underDepth":0,"underInset":60,"leftLedge":true,"rightLedge":true}]}),
            "blocks[0].underDepth",
        ),
        (json!({"ledge":{"r":0}}), "ledge.r"),
        (json!({"spawns":[{"x":500,"face":0}]}), "spawns[0].face"),
    ] {
        let mut src = patched(file, patch);
        let mut backup = sources().into_iter().find(|s| s.path == file).unwrap();
        backup.path = "stages/backup/stage.json".into();
        src.push(backup);
        let (data, errors) = load(src).unwrap();
        assert!(data.stage("arena").is_none() && data.stage("backup").is_some());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file, file);
        assert_eq!(errors[0].path, path);
    }
}

#[test]
fn legacy_stage_migrates_to_identical_geometry() {
    let file = "stages/arena/stage.json";
    let original = load(sources()).unwrap().0;
    let mut src = sources();
    let stage = src.iter_mut().find(|s| s.path == file).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&stage.text).unwrap();
    let mut ground = v["blocks"][0].clone();
    ground.as_object_mut().unwrap().shift_remove("leftLedge");
    ground.as_object_mut().unwrap().shift_remove("rightLedge");
    v.as_object_mut().unwrap().shift_remove("blocks");
    v["ground"] = ground;
    v["schemaVersion"] = 1.into();
    stage.text = v.to_string();
    let (migrated, errors) = load(src).unwrap();
    assert!(errors.is_empty());
    assert_eq!(original.stages, migrated.stages);
    assert_eq!(original.hash, migrated.hash);
}

#[test]
fn stage_motion_and_spawn_errors_have_paths() {
    use serde_json::json;
    let file = "stages/arena/stage.json";
    for (patch, path) in [
        (json!({"blocks":[]}), "blocks"),
        (json!({"spawns":[{"x":500,"y":550,"face":1}]}), "spawns[0]"),
        // Feet outside the block, but the fighter's body intersects the solid.
        (json!({"spawns":[{"x":290,"y":550,"face":1}]}), "spawns[0]"),
        (json!({"spawns":[{"x":100,"face":1}]}), "spawns[0].y"),
        (
            json!({"platforms":[{"x1":400,"x2":600,"y":300,"motion":{"mode":"loop","waypoints":[]}}]}),
            "platforms[0].motion.waypoints",
        ),
        (
            json!({"platforms":[{"x1":400,"x2":600,"y":300,"motion":{"mode":"loop","waypoints":[{"x":0,"y":0,"travelTicks":10},{"x":20,"y":0,"travelTicks":0}]}}]}),
            "platforms[0].motion.waypoints[1].travelTicks",
        ),
        (
            json!({"platforms":[{"x1":400,"x2":600,"y":300,"motion":{"mode":"loop","waypoints":[{"x":"bad","y":0,"travelTicks":10},{"x":20,"y":0,"travelTicks":10}]}}]}),
            "platforms[0].motion.waypoints[0].x",
        ),
    ] {
        let mut src = patched(file, patch);
        let mut backup = sources().into_iter().find(|s| s.path == file).unwrap();
        backup.path = "stages/backup/stage.json".into();
        src.push(backup);
        let (_, errors) = load(src).unwrap();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].file, file);
        assert_eq!(errors[0].path, path);
    }
}

#[test]
fn ledges_cannot_have_arbitrary_positions() {
    let file = "stages/arena/stage.json";
    let mut src = sources();
    let stage = src.iter_mut().find(|s| s.path == file).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&stage.text).unwrap();
    v["blocks"][0]["ledges"] = serde_json::json!([{"x":640,"y":520}]);
    stage.text = v.to_string();
    let mut backup = sources().into_iter().find(|s| s.path == file).unwrap();
    backup.path = "stages/backup/stage.json".into();
    src.push(backup);
    let (_, errors) = load(src).unwrap();
    assert_eq!(errors[0].path, "blocks[0].ledges");
    assert!(errors[0].msg.contains("unknown field `ledges`"));
}

#[test]
fn adjacent_blocks_require_exposed_ledges() {
    let file = "stages/arena/stage.json";
    // Only arena, so its rejection leaves no stages and aborts the load.
    let only_arena = |src: Vec<Source>| -> Vec<Source> {
        src.into_iter().filter(|s| !s.path.starts_with("stages/") || s.path == file).collect()
    };
    let mut src = only_arena(sources());
    let stage = src.iter_mut().find(|s| s.path == file).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&stage.text).unwrap();
    let mut left = v["blocks"][0].clone();
    let mut right = left.clone();
    left["right"] = 600.into();
    right["left"] = 600.into();
    v["blocks"] = serde_json::json!([left, right]);
    stage.text = v.to_string();
    let Err(errors) = load(src) else { panic!("invalid stage accepted") };
    assert_eq!(errors[0].path, "blocks[0].rightLedge");
    v["blocks"][0]["rightLedge"] = false.into();
    v["blocks"][1]["leftLedge"] = false.into();
    assert!(load(only_arena(patched(file, v.clone()))).unwrap().1.is_empty());
    v["blocks"][1]["left"] = 590.into();
    let Err(errors) = load(only_arena(patched(file, v))) else { panic!("invalid stage accepted") };
    assert_eq!(errors[0].path, "blocks[1]");
}

fn stage_error(patch: serde_json::Value) -> sim::data::ContentError {
    let file = "stages/arena/stage.json";
    let mut src = patched(file, patch);
    let mut backup = sources().into_iter().find(|s| s.path == file).unwrap();
    backup.path = "stages/backup/stage.json".into();
    src.push(backup);
    let (_, mut errors) = load(src).unwrap();
    assert_eq!(errors.len(), 1, "{errors:?}");
    errors.remove(0)
}

#[test]
fn ledge_hanging_body_needs_clearance_from_neighbours() {
    let error = stage_error(serde_json::json!({"blocks":[
        {"left":300,"right":640,"top":520,"bottom":580,"underInset":60,"underDepth":100,"leftLedge":true,"rightLedge":false},
        {"left":640,"right":980,"top":470,"bottom":580,"underInset":60,"underDepth":100,"leftLedge":true,"rightLedge":true}
    ]}));
    assert_eq!(error.path, "blocks[1].leftLedge");
    assert!(error.msg.contains("hanging body"));
}

#[test]
fn platforms_need_body_clearance_along_entire_motion_segments() {
    use serde_json::json;
    assert_eq!(stage_error(json!({"platforms":[{"x1":500,"x2":600,"y":700}]})).path, "platforms[0]");
    // Standing feet may extend past the visible platform by global.sim.platformPad.
    assert_eq!(stage_error(json!({"platforms":[{"x1":100,"x2":284,"y":600}]})).path, "platforms[0]");
    // Both endpoints are clear; even a one-tick traversal must not cross the solid.
    let moving = json!({"x1":100,"x2":150,"y":600,"motion":{"mode":"pingPong","waypoints":[
        {"x":0,"y":0,"travelTicks":1},{"x":1000,"y":0,"travelTicks":0}
    ]}});
    assert_eq!(stage_error(json!({"platforms":[moving]})).path, "platforms[0]");
    let (_, errors) =
        load(patched("stages/arena/stage.json", json!({"platforms":[{"x1":500,"x2":600,"y":754}]}))).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn respawn_descent_and_waiting_body_must_clear_solids() {
    use serde_json::json;
    for (start, end) in [(400, 550), (750, 400), (600, 600)] {
        assert_eq!(stage_error(json!({"respawns":[{"x":500,"startY":start,"endY":end}]})).path, "respawns[0]");
    }
}

#[test]
fn version_one_offstage_spawn_keeps_original_ground_height() {
    let file = "stages/arena/stage.json";
    let mut src = sources();
    let stage = src.iter_mut().find(|s| s.path == file).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&stage.text).unwrap();
    let mut ground = v["blocks"][0].clone();
    ground.as_object_mut().unwrap().shift_remove("leftLedge");
    ground.as_object_mut().unwrap().shift_remove("rightLedge");
    v.as_object_mut().unwrap().shift_remove("blocks");
    v["ground"] = ground;
    v["schemaVersion"] = 1.into();
    v["spawns"][0]["x"] = 100.into();
    stage.text = v.to_string();
    let (data, errors) = load(src).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    let w = sim::World::new(
        sim::MatchConfig::sandbox("arena", vec![sim::PlayerConfig::human("brawler")], MatchConfig::default().seed),
        &data,
    )
    .unwrap();
    assert_eq!(w.fighters[0].y, 520.);
    assert!(!w.fighters[0].ground);
}

#[test]
fn moving_platform_spawns_use_phase_and_validate_carried_clearance() {
    use serde_json::json;
    let platform = json!({"x1":400,"x2":600,"y":300,"motion":{"mode":"pingPong","phase":10,"waypoints":[
        {"x":0,"y":0,"travelTicks":20},{"x":100,"y":-40,"travelTicks":0}
    ]}});
    let (data, errors) =
        load(patched("stages/arena/stage.json", json!({"platforms":[platform],"spawns":[{"x":500,"y":280,"face":1}]})))
            .unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    let mut w = sim::World::new(
        sim::MatchConfig::sandbox("arena", vec![sim::PlayerConfig::human("brawler")], MatchConfig::default().seed),
        &data,
    )
    .unwrap();
    assert_eq!(w.fighters[0].surf, Some(1));
    sim::step_world(&mut w, &data, &[]);
    assert_eq!((w.fighters[0].x, w.fighters[0].y), (505., 278.));
    let error = stage_error(json!({"platforms":[{"x1":400,"x2":600,"y":300,"motion":{"mode":"loop","waypoints":[
        {"x":0,"y":0,"travelTicks":10},{"x":0,"y":320,"travelTicks":10}
    ]}}],"spawns":[{"x":500,"y":300,"face":1}]}));
    assert_eq!(error.path, "platforms[0]");
    assert!(error.msg.contains("clearance"));
}

#[test]
fn stage_zoom_requires_positive_ordered_overrides() {
    for (zoom, path) in [
        (serde_json::json!({}), "zoom"),
        (serde_json::json!({"minHeight": 0}), "zoom.minHeight"),
        (serde_json::json!({"maxHeight": -1}), "zoom.maxHeight"),
        (serde_json::json!({"minHeight": 700, "maxHeight": 600}), "zoom"),
    ] {
        assert_eq!(stage_error(serde_json::json!({"zoom": zoom})).path, path);
    }
    for zoom in [serde_json::json!({"minHeight": 400}), serde_json::json!({"maxHeight": 1200})] {
        assert!(load(patched("stages/arena/stage.json", serde_json::json!({"zoom": zoom}))).unwrap().1.is_empty());
    }
}

#[test]
fn camera_globals_validate_zoom_scaling_and_limits() {
    let mut g = load(sources()).unwrap().0.global;
    for floor in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        g.render.camera.min_scale_floor = floor;
        assert_eq!(sim::data::load::check_global("global", &g).unwrap_err().path, "render.camera.minScaleFloor");
    }
    for floor in [0., 1.] {
        g.render.camera.min_scale_floor = floor;
        assert!(sim::data::load::check_global("global", &g).is_ok());
    }
    g.render.camera.min_height = g.render.camera.max_height + 1.;
    assert_eq!(sim::data::load::check_global("global", &g).unwrap_err().path, "render.camera");
}

#[test]
fn resolved_validation_covers_packages_and_duplicate_ids() {
    let mut data = load(sources()).unwrap().0;
    assert!(sim::data::validate(&data).is_empty());
    data.fighters[0].moves.jab.phases.clear();
    data.stages[0].blocks.clear();
    data.fighters.push(data.fighters[1].clone());
    data.stages.push(data.stages[1].clone());
    let errors = sim::data::validate(&data);
    for (file, path) in [
        ("fighters/archer/fighter.json", "moves.jab.phases"),
        ("stages/arena/stage.json", "blocks"),
        ("fighters/brawler/fighter.json", "id"),
        ("stages/citadel/stage.json", "id"),
    ] {
        assert!(errors.iter().any(|e| e.file == file && e.path == path), "{errors:?}");
    }
    data.fighters.clear();
    data.stages.clear();
    let errors = sim::data::validate(&data);
    assert!(errors.iter().any(|e| e.path == "fighters"));
    assert!(errors.iter().any(|e| e.path == "stages"));
}

#[test]
fn ai_scalar_probabilities_ranges_and_cutoffs_are_checked() {
    use serde_json::json;
    for (patch, path) in [
        (json!({"initialCool": -1}), "ai.initialCool"),
        (json!({"thresholds": {"jumpChance": 2}}), "ai.thresholds.jumpChance"),
        (json!({"thresholds": {"centerChance": -0.1}}), "ai.thresholds.centerChance"),
        (json!({"thresholds": {"projectileShieldChance": 2}}), "ai.thresholds.projectileShieldChance"),
        (json!({"thresholds": {"defendHeight": -1}}), "ai.thresholds.defendHeight"),
        (json!({"thresholds": {"holdMax": 5}}), "ai.thresholds.holdMax"),
        (json!({"thresholds": {"knockdownCenter": 0.2}}), "ai.thresholds.knockdownCenter"),
        (json!({"thresholds": {"ledgeAttack": 0.2}}), "ai.thresholds.ledgeAttack"),
        (json!({"thresholds": {"ledgeShield": 0.4}}), "ai.thresholds.ledgeShield"),
        (json!({"thresholds": {"throwUp": 0.2}}), "ai.thresholds.throwUp"),
        (json!({"thresholds": {"throwForward": 0.5}}), "ai.thresholds.throwForward"),
        (json!({"thresholds": {"shieldSpecial": 0.4}}), "ai.thresholds.shieldSpecial"),
        (json!({"thresholds": {"smashDown": 0.4}}), "ai.thresholds.smashDown"),
        (json!({"thresholds": {"grabChance": 0.1}}), "ai.thresholds.grabChance"),
        (json!({"thresholds": {"tiltChance": 0.3}}), "ai.thresholds.tiltChance"),
        (json!({"thresholds": {"downTiltChance": 0.5}}), "ai.thresholds.downTiltChance"),
        (json!({"thresholds": {"jabChance": 0.7}}), "ai.thresholds.jabChance"),
    ] {
        let errors = load(patched("global.json", json!({"ai": patch}))).unwrap_err();
        assert_eq!(errors[0].path, path, "{errors:?}");
    }
    let errors = load(patched("global.json", json!({"hitTypes": {"autolink": {"pull": -1}}}))).unwrap_err();
    assert_eq!(errors[0].path, "hitTypes.autolink.pull");
}

#[test]
fn resolved_nonfinite_values_report_json_paths_without_serializing() {
    let data = load(sources()).unwrap().0;
    for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut d = data.clone();
        d.global.ai.thresholds.jump_chance = n;
        assert_eq!(sim::data::validate(&d)[0].path, "ai.thresholds.jumpChance");
        let mut d = data.clone();
        d.global.ai.thresholds.hold_max = n;
        assert_eq!(sim::data::validate(&d)[0].path, "ai.thresholds.holdMax");
        let mut d = data.clone();
        d.global.movement.fast_fall_min_vy = n;
        assert_eq!(sim::data::validate(&d)[0].path, "movement.fastFallMinVy");
        let mut d = data.clone();
        d.fighters[0].stats.weight = n;
        assert_eq!(sim::data::validate(&d)[0].path, "stats.weight");
        let mut d = data.clone();
        d.stages[0].blocks[0].left = n;
        assert_eq!(sim::data::validate(&d)[0].path, "blocks[0].left");
    }
}

#[test]
fn migration_is_scoped_to_content_kind_and_accepts_legacy_aliases() {
    use serde_json::json;
    use sim::data::{ContentKind, migrate_kind};
    for kind in [ContentKind::Fighter, ContentKind::Global] {
        for version in [1, 2] {
            let mut v = json!({"schemaVersion": version, "ground": {"left": 0, "right": 10, "top": 5}});
            let ground = v["ground"].clone();
            migrate_kind(kind, &mut v).unwrap();
            assert_eq!(v["schemaVersion"], 1);
            assert_eq!(v["ground"], ground);
            assert!(v.get("blocks").is_none());
        }
    }
    for kind in [ContentKind::Fighter, ContentKind::Global, ContentKind::Stage] {
        for version in [0, 3] {
            assert!(migrate_kind(kind, &mut json!({"schemaVersion": version})).is_err());
        }
    }
    let data = load(sources()).unwrap().0;
    let aliased = load(patched("fighters/archer/fighter.json", json!({"schemaVersion": 2}))).unwrap().0;
    assert_eq!(data.fighters, aliased.fighters);
    assert_eq!(data.hash, aliased.hash);
}

#[test]
fn filesystem_sources_are_sorted_relative_and_skip_schemas() {
    let root = std::env::temp_dir().join(format!("stick-smash-sources-{}", std::process::id()));
    fs::create_dir_all(root.join("nested/schema")).unwrap();
    fs::create_dir_all(root.join("schema")).unwrap();
    for name in ["z.json", "a.json", "nested/c.json", "schema/skip.json", "nested/schema/skip.json", "ignore.txt"] {
        fs::write(root.join(name), "{}").unwrap();
    }
    let (files, errors) = sim::data::sources(&root);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(files.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(), ["a.json", "nested/c.json", "z.json"]);
    assert!(files.iter().all(|s| s.text == "{}"));
    fs::remove_dir_all(&root).unwrap();
    let (files, errors) = sim::data::sources(&root);
    assert!(files.is_empty() && errors.is_empty(), "a missing root is empty: {errors:?}");
}
