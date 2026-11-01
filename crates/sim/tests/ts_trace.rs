mod common;
use serde_json::{Value, json};
use sim::{Input, MatchConfig, PlayerConfig, World, step_world, world::State};
use std::{fs, path::Path};
fn snapshot(w: &World) -> Value {
    json!({"rng":w.rng.0,"f":w.fighters.iter().map(|f|json!([f.x,f.y,f.vx,f.vy,f.state.name(),f.dmg,f.stocks,f.lag,f.hitstun,f.face,f.mov.as_ref().map(|m|m.slot.as_str()),f.mov.as_ref().map_or(0,|m|m.charge)])).collect::<Vec<_>>(),"p":w.projs.iter().map(|p|json!([p.owner,p.x,p.y,p.vx,p.vy,p.life])).collect::<Vec<_>>()})
}
fn mismatch(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            let exact = [".stocks", ".lag", ".hitstun", ".face", ".charge", ".owner", ".life"]
                .iter()
                .any(|field| path.ends_with(field));
            if if exact { a != b } else { (a - b).abs() > 1e-6 } {
                Some(format!("{path}: Rust {a}, TS {b}"))
            } else {
                None
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Some(format!("{path} count: Rust {}, TS {}", a.len(), b.len()));
            }
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                let field = if path.starts_with("f[") && !path.contains('.') {
                    ["x", "y", "vx", "vy", "state", "dmg", "stocks", "lag", "hitstun", "face", "move", "charge"]
                        .get(i)
                        .copied()
                        .unwrap_or("?")
                        .to_string()
                } else if path.starts_with("p[") && !path.contains('.') {
                    ["owner", "x", "y", "vx", "vy", "life"][i].to_string()
                } else {
                    format!("[{i}]")
                };
                let path = if field.starts_with('[') { format!("{path}{field}") } else { format!("{path}.{field}") };
                if let Some(e) = mismatch(a, b, &path) {
                    return Some(e);
                }
            }
            None
        }
        _ => {
            if a == b {
                None
            } else {
                Some(format!("{path}: Rust {a}, TS {b}"))
            }
        }
    }
}
#[test]
fn matches_typescript_traces() {
    let mut d = common::data();
    d.global.input.buffer_frames = 0;
    d.global.sim.separate_cosmetic_rng = false;
    d.global.ai.thresholds.surface_chase_ticks = 0;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ts");
    let mut paths: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        panic!("TS trace fixtures absent; run just record-traces");
    }
    let filter = std::env::var("TRACE_FILTER").unwrap_or_default();
    let mut failures = vec![];
    let mut passed = 0;
    for path in paths {
        let fixture: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let name = fixture["name"].as_str().unwrap();
        if !name.contains(&filter) {
            continue;
        }
        let cfg = &fixture["config"];
        let players = cfg["players"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| PlayerConfig {
                fighter: p["fighter"].as_str().unwrap().into(),
                cpu: if p["cpu"] == true { Some(cfg["cpuLevel"].as_u64().unwrap_or(2) as u8) } else { None },
                team: None,
                tap_jump: p["tapJump"].as_bool().unwrap_or(false),
            })
            .collect();
        let mut w = World::new(
            MatchConfig {
                seed: cfg["seed"].as_u64().unwrap() as u32,
                stage: cfg["stage"].as_str().unwrap().into(),
                stocks: cfg["stocks"].as_i64().unwrap() as i32,
                training: cfg["training"].as_bool().unwrap_or(false),
                players,
                friendly_fire: false,
            },
            &d,
        )
        .unwrap();
        if let Some(setup) = cfg["setup"].as_array() {
            for setup in setup {
                let i = setup["player"].as_u64().unwrap() as usize;
                let f = &mut w.fighters[i];
                for (k, v) in setup.as_object().unwrap() {
                    match k.as_str() {
                        "player" => {}
                        "x" => f.x = v.as_f64().unwrap(),
                        "y" => f.y = v.as_f64().unwrap(),
                        "vx" => f.vx = v.as_f64().unwrap(),
                        "vy" => f.vy = v.as_f64().unwrap(),
                        "face" => f.face = v.as_f64().unwrap(),
                        "dmg" => f.dmg = v.as_f64().unwrap(),
                        "ground" => f.ground = v.as_bool().unwrap(),
                        "surf" => {
                            assert!(v.is_null());
                            f.surf = None;
                        }
                        "state" => {
                            f.state = serde_json::from_value::<State>(v.clone()).unwrap();
                        }
                        _ => panic!("unsupported setup {k}"),
                    }
                }
            }
        }
        let frames = fixture["frames"].as_array().unwrap();
        assert_eq!(frames.len(), fixture["inputs"].as_array().unwrap().len(), "{name}: frame/input lengths differ");
        let mut context = vec![];
        let mut failed = false;
        for (tick, raw) in fixture["inputs"].as_array().unwrap().iter().enumerate() {
            let ins: Vec<_> = raw
                .as_array()
                .unwrap()
                .iter()
                .map(|v| Input {
                    stick: (v[0].as_i64().unwrap() as i8, v[1].as_i64().unwrap() as i8),
                    buttons: v[2].as_u64().unwrap() as u8,
                })
                .collect();
            step_world(&mut w, &d, &ins);
            w.fx.clear();
            let actual = snapshot(&w);
            let expected = &frames[tick];
            let err = if actual["rng"] != expected["rng"] {
                Some(format!("rng: Rust {}, TS {}", actual["rng"], expected["rng"]))
            } else {
                mismatch(&actual["f"], &expected["f"], "f").or_else(|| mismatch(&actual["p"], &expected["p"], "p"))
            };
            if let Some(err) = err {
                failures.push(format!(
                    "{name}, tick {}: {err}\ncontext:\n{}\nRust {actual}\nTS   {expected}",
                    tick + 1,
                    context.join("\n")
                ));
                failed = true;
                break;
            }
            context.push(format!("tick {}: {actual}", tick + 1));
            if context.len() > 3 {
                context.remove(0);
            }
        }
        if !failed {
            passed += 1;
        }
    }
    assert!(passed + failures.len() > 0, "TRACE_FILTER matched no fixtures");
    eprintln!("TS fixtures: {passed} passed, {} failed", failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
