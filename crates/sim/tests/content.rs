//! Loads and validates every file in `assets/`, and keeps schemas and formatting up to date.
//! `BLESS=1 cargo test -p sim --test content` rewrites `assets/schema/` and reformats every asset in place.

use std::fs;
use std::path::{Path, PathBuf};

use sim::data::json::{merge, to_compact_string};
use sim::data::schema::schemas;
use sim::data::{Source, load};

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn bless() -> bool {
    std::env::var_os("BLESS").is_some()
}

fn json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "schema") {
                json_files(&p, out);
            }
        } else if p.extension().is_some_and(|e| e == "json") {
            out.push(p);
        }
    }
}

fn sources() -> Vec<Source> {
    let root = assets();
    let mut files = Vec::new();
    json_files(&root, &mut files);
    files
        .into_iter()
        .map(|p| Source {
            path: p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"),
            text: fs::read_to_string(&p).unwrap(),
        })
        .collect()
}

#[test]
fn all_assets_load() {
    let (data, errors) = load(sources()).unwrap_or_else(|e| panic!("content failed to load:\n{}", fmt_errors(&e)));
    assert!(errors.is_empty(), "content errors:\n{}", fmt_errors(&errors));
    let ids: Vec<&str> = data.fighters.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, ["brawler", "king", "swordfighter"]);
    assert!(data.stages.len() == 1 && data.stages[0].spawns.len() >= 8 && data.stages[0].respawns.len() >= 8);
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
    let mut files = Vec::new();
    json_files(&root, &mut files);
    for p in files {
        let text = fs::read_to_string(&p).unwrap();
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
        (json!({"stats":{"weight":0}}), "stats.weight"),
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
fn invalid_stage_numbers_skip_package_with_path() {
    use serde_json::json;
    let file = "stages/arena/stage.json";
    for (patch, path) in [
        (json!({"blast":{"left":1600}}), "blast"),
        (json!({"camera":{"bottom":-1000}}), "camera"),
        (json!({"ground":{"underInset":0}}), "ground.underInset"),
        (json!({"ground":{"underDepth":0}}), "ground.underDepth"),
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
