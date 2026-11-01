use super::{
    model::{interval, *},
    player::*,
    storage::*,
    ui::*,
};
use crate::play::{Match, Mode};
use bevy::prelude::*;
use sim::{Input, MatchConfig, World, step_world};
use std::{fs, io::Read, sync::Arc};
fn recorded() -> (Replay, World) {
    let data = crate::test_data();
    let cfg = MatchConfig {
        stocks: 1,
        seed: 7,
        players: vec![
            sim::PlayerConfig { cpu: Some(2), ..sim::PlayerConfig::human("brawler") },
            sim::PlayerConfig { cpu: Some(2), ..sim::PlayerConfig::human("king") },
        ],
        ..default()
    };
    let mut w = World::new(cfg, &data).unwrap();
    let mut r = Replay::new(&data, &w);
    while !w.over && w.frame < 36000 {
        let inputs = vec![Input::default(); 2];
        step_world(&mut w, &data, &inputs);
        drain(&mut w);
        r.record(inputs, &w);
    }
    assert!(w.over);
    (r, w)
}
#[test]
fn replay_payload_rejects_invalid_camera_globals() {
    let (mut r, _) = recorded();
    r.data.global.render.camera.min_scale_floor = 2.;
    let encoded = serde_json::to_vec(&r).unwrap();
    let error = serde_json::from_slice::<Replay>(&encoded).err().expect("invalid replay globals rejected");
    assert!(error.to_string().contains("render.camera.minScaleFloor"));
}

#[test]
fn replay_payload_rejects_invalid_fighter_content() {
    let (mut r, _) = recorded();
    let id = r.data.fighters[0].id.clone();
    r.data.fighters[0].moves.jab.phases.clear();
    let error = serde_json::from_value::<Replay>(serde_json::to_value(&r).unwrap()).err().unwrap().to_string();
    assert!(error.contains("invalid replay content"), "{error}");
    assert!(error.contains(&id) && error.contains("moves.jab.phases"), "{error}");
}
#[test]
fn replay_payload_validates_restored_package_ids() {
    let (r, _) = recorded();
    let mut v = serde_json::to_value(&r).unwrap();
    v["data"][1][1] = v["data"][1][0].clone();
    let error = serde_json::from_value::<Replay>(v).err().unwrap().to_string();
    assert!(error.contains("duplicate") && error.contains("invalid replay content"), "{error}");
}
#[test]
fn older_presentation_fields_are_filled_without_replacing_recorded_tuning() {
    let (r, _) = recorded();
    let mut v = serde_json::to_value(&r).unwrap();
    let render = v.pointer_mut("/data/0/global/render").unwrap();
    render.as_object_mut().unwrap().remove("interpolate");
    render["hud"].as_object_mut().unwrap().remove("look");
    for field in ["referencePlayWidth", "referencePlayHeight", "minScaleStart", "minScaleFloor"] {
        render["camera"].as_object_mut().unwrap().remove(field);
    }
    render["camera"]["padTop"] = serde_json::json!(123.);
    let back: Replay = serde_json::from_value(v).unwrap();
    let mut expected = r.data.global.render.clone();
    expected.camera.pad_top = 123.;
    assert_eq!(back.data.global.render, expected);
}
#[test]
fn recordings_with_other_select_looks_still_load() {
    let (r, _) = recorded();
    let mut v = serde_json::to_value(&r).unwrap();
    let select = v.pointer_mut("/data/0/global/render/select").expect("select look in payload");
    select.as_object_mut().unwrap().remove("pointer");
    let back: Replay = serde_json::from_value(v).unwrap();
    assert_eq!(back.data.global.render.select, r.data.global.render.select);
}
#[test]
fn cpu_match_round_trip_checksums_outcome_and_seek() {
    let (r, expected) = recorded();
    let encoded = serde_json::to_vec(&r).unwrap();
    let dir = std::env::temp_dir().join(format!("stick-smash-replay-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("cpu.replay.json");
    let round_trip: Replay = serde_json::from_slice(&encoded).unwrap();
    round_trip.write(&file).unwrap();
    let r = Replay::read(&file).unwrap();
    verify_directory(&dir).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(r.runs.len(), 1, "CPU inputs compress to one run");
    let mut p = Playback::new(r).unwrap();
    let mut w = p.snapshots[0].1.clone();
    p.seek(p.replay.ticks, &mut w);
    assert_eq!(p.warning, None);
    assert_eq!(w, expected);
    p.seek(450, &mut w);
    p.seek(1200.min(p.replay.ticks), &mut w);
    p.seek(p.replay.ticks, &mut w);
    assert_eq!(w, expected);
    assert_eq!(p.warning, None);
}
#[test]
fn detects_changed_simulation_and_unsupported_format() {
    let (mut r, _) = recorded();
    for f in &mut r.data.fighters {
        f.stats.run_speed *= 0.5;
    }
    let mut p = Playback::new(r.clone()).unwrap();
    let mut w = p.snapshots[0].1.clone();
    p.seek(p.replay.ticks, &mut w);
    assert!(p.warning.is_some());
    assert!(!p.warning.as_ref().unwrap().contains("tick 0:"));
    r.version = 99;
    assert!(Playback::new(r).is_err());
}
#[test]
fn seek_bar_uses_centered_coordinates() {
    assert_eq!(seek_tick(-0.5, 1000), 0);
    assert_eq!(seek_tick(0., 1000), 500);
    assert_eq!(seek_tick(0.5, 1000), 1000);
    assert_eq!(seek_tick(2., 1000), 1000);
}
#[test]
fn seeks_are_bounded_and_identical_requests_do_nothing() {
    let (r, expected) = recorded();
    let mut p = Playback::new(r).unwrap();
    let mut w = p.snapshots[0].1.clone();
    assert!(!p.request_seek(0, &mut w));
    assert!(p.request_seek(p.replay.ticks, &mut w));
    p.advance_seek(&mut w, 10);
    assert_eq!(p.position, 10);
    assert!(p.seeking());
    assert!(!p.request_seek(p.replay.ticks, &mut w));
    assert_eq!(p.position, 10);
    while p.seeking() {
        p.advance_seek(&mut w, 240);
    }
    assert_eq!(w, expected);
    assert_eq!(p.warning, None);
}
#[test]
fn input_resets_round_trip_and_survive_snapshot_seeks() {
    let mut data = (*crate::test_data()).clone();
    data.global.replay.checkpoint_ticks = 1;
    assert_eq!(interval(&data), u64::from(sim::TICK_HZ));
    let cfg = MatchConfig { training: true, ..default() };
    let mut w = World::new(cfg, &data).unwrap();
    let mut r = Replay::new(&data, &w);
    for tick in 0..1200 {
        if [250, 510, 600].contains(&tick) {
            r.clear_inputs();
            r.clear_inputs();
            clear_world_inputs(&mut w);
        }
        let input =
            Input { stick: (if tick % 20 < 10 { 127 } else { -127 }, 0), buttons: if tick % 9 == 0 { 2 } else { 0 } };
        let inputs = vec![input, Input::default()];
        step_world(&mut w, &data, &inputs);
        drain(&mut w);
        r.record(inputs, &w);
    }
    r.final_hash = checksum(&w);
    let r: Replay = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
    assert_eq!(r.clears, [250, 510, 600]);
    let mut p = Playback::new(r).unwrap();
    let mut actual = p.snapshots[0].1.clone();
    p.seek(1200, &mut actual);
    assert_eq!(actual, w);
    assert_eq!(p.warning, None);
    p.seek(600, &mut actual);
    p.seek(1200, &mut actual);
    assert_eq!(actual, w);
    assert_eq!(p.warning, None);
}
#[test]
fn library_header_does_not_parse_payload_and_versions_are_explicit() {
    let (mut r, _) = recorded();
    let mut bytes = serde_json::to_vec(&r.header()).unwrap();
    bytes.extend_from_slice(b"\nnot a valid replay payload");
    let mut reader = std::io::Cursor::new(bytes);
    let h = Header::read(&mut reader).unwrap();
    h.compatible().unwrap();
    assert!(h.label().contains("brawler"));
    let mut remaining = String::new();
    reader.read_to_string(&mut remaining).unwrap();
    assert_eq!(remaining, "not a valid replay payload");
    r.sim_version = "another-sim".into();
    assert!(Playback::new(r).err().unwrap().contains("recorded with a different version"));
}
#[test]
fn palette_validation_and_cleanup_failures() {
    let (mut r, _) = recorded();
    let dir = std::env::temp_dir().join(format!("stick-smash-cleanup-test-{}", std::process::id()));
    fs::create_dir_all(dir.join("auto-0-broken.replay.json")).unwrap();
    r.data.global.replay.auto_save_count = 1;
    let saved = r.save_in(&dir, true).unwrap();
    assert!(saved.is_file());
    assert!(Replay::read(&saved).is_ok());
    fs::remove_dir_all(dir).unwrap();
    r.data.global.palette.clear();
    assert!(r.validate().unwrap_err().contains("palette"));
    let data = Arc::new(r.data.clone());
    assert!(Match::attract(&data, 1).err().unwrap().contains("palette"));
}
#[test]
fn watching_results_preserves_match_and_allows_saving_without_autosave() {
    let (mut r, expected) = recorded();
    r.data.global.replay.auto_save_count = 0;
    let mut m = watching(r.clone()).unwrap();
    m.mode = Mode::Versus;
    m.playback = None;
    m.world = expected.clone();
    m.recording = Some(r);
    m.recording_done = true;
    m.stats[0].kos = 7;
    let places = m.placements();
    let mut scene = render::Scene::new(m.data.clone());
    let mut world = bevy::prelude::World::new();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut next = NextState::default();
    watch_results(&mut m, &mut Commands::new(&mut queue, &world), &mut scene, &mut next).unwrap();
    queue.apply(&mut world);
    assert!(m.playback.as_ref().unwrap().replay.saved_path.is_none());
    let dir = std::env::temp_dir().join(format!("stick-smash-return-test-{}", std::process::id()));
    let path = m.playback.as_mut().unwrap().replay.save_in(&dir, false).unwrap();
    let mut ret = world.remove_resource::<ReturnTo>().unwrap();
    restore_results(&mut m, &mut ret, &mut scene);
    assert_eq!(m.world, expected);
    assert_eq!(m.stats[0].kos, 7);
    assert_eq!(m.placements(), places);
    assert_eq!(m.mode, Mode::Versus);
    assert!(m.recording_done);
    assert_eq!(m.recording.as_ref().unwrap().saved_path.as_ref(), Some(&path));
    fs::remove_dir_all(dir).unwrap();
}
