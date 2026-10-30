//! Turning content files (already read into strings by the caller) into [`GameData`].
//!
//! Files are addressed by their path relative to a content root: `global.json`, `fighters/<id>/fighter.json`,
//! `stages/<id>/stage.json`. Later files replace earlier ones with the same path, so passing the built-in
//! files first and the user content dir second lets users override packages.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::fighter::Fighter;
use super::global::Global;
use super::json::{content_hash, merge};
use super::moves::{END, HELPLESS, LANDING, Move, Phase, Trigger};
use super::stage::Stage;
use super::visual::{Anim, PoseRef};
use super::{GameData, SCHEMA_VERSION};

/// A problem in one content file. The package it belongs to was skipped.
#[derive(Clone, Debug, PartialEq)]
pub struct ContentError {
    pub file: String,
    /// JSON path inside the file (`moves.dspec.phases[1].transitions[0].to`), empty for whole-file errors.
    pub path: String,
    pub msg: String,
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}: {}", self.file, self.msg)
        } else {
            write!(f, "{}: {}: {}", self.file, self.path, self.msg)
        }
    }
}

/// One content file.
pub struct Source {
    pub path: String,
    pub text: String,
}

/// Loaded content plus errors for packages that were skipped. Fails only if `global.json` is unusable or no
/// playable fighter or stage loaded.
pub fn load(files: impl IntoIterator<Item = Source>) -> Result<(GameData, Vec<ContentError>), Vec<ContentError>> {
    let mut by_path: BTreeMap<String, String> = BTreeMap::new();
    for f in files {
        by_path.insert(f.path.replace('\\', "/"), f.text);
    }
    let mut errors = Vec::new();
    let mut raw_fighters: BTreeMap<String, (String, Value)> = BTreeMap::new();
    let mut raw_stages: Vec<(String, String, Value)> = Vec::new();
    let mut raw_global = None;
    for (path, text) in &by_path {
        let parts: Vec<&str> = path.split('/').collect();
        let kind = match parts.as_slice() {
            ["global.json"] => "global",
            ["fighters", _, "fighter.json"] => "fighter",
            ["stages", _, "stage.json"] => "stage",
            _ => continue,
        };
        let v = match parse(path, text) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        match kind {
            "global" => raw_global = Some(v),
            "fighter" => drop(raw_fighters.insert(parts[1].to_string(), (path.clone(), v))),
            _ => raw_stages.push((parts[1].to_string(), path.clone(), v)),
        }
    }

    let Some(gv) = raw_global else {
        errors.push(err("global.json", "", "missing"));
        return Err(errors);
    };
    let global: Global = match typed("global.json", &gv) {
        Ok(g) => g,
        Err(e) => {
            errors.push(e);
            return Err(errors);
        }
    };
    if global.input.stick_threshold <= 0 {
        errors.push(err("global.json", "input.stickThreshold", "must be between 1 and 127"));
        return Err(errors);
    }
    if global.palette.is_empty() {
        errors.push(err("global.json", "palette", "needs at least one color set"));
        return Err(errors);
    }
    let mut hashed = vec![("global".to_string(), gv)];

    let mut stages = Vec::new();
    for (id, path, v) in raw_stages {
        match typed::<Stage>(&path, &v).and_then(|s| check_stage(&path, &s).map(|()| s)) {
            Ok(mut s) => {
                s.id = id.clone();
                hashed.push((format!("stage:{id}"), v));
                stages.push(s);
            }
            Err(e) => errors.push(e),
        }
    }

    let mut fighters = Vec::new();
    for (id, (path, _)) in &raw_fighters {
        let merged = match resolve(id, &raw_fighters, &mut Vec::new()) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        if merged.get("abstract").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        match typed::<Fighter>(path, &merged).and_then(|f| check_fighter(path, &f).map(|()| f)) {
            Ok(mut f) => {
                f.id = id.clone();
                hashed.push((format!("fighter:{id}"), merged));
                fighters.push(f);
            }
            Err(e) => errors.push(e),
        }
    }

    if fighters.is_empty() || stages.is_empty() {
        errors.push(err("", "", if fighters.is_empty() { "no playable fighter loaded" } else { "no stage loaded" }));
        return Err(errors);
    }
    let all = Value::Array(hashed.into_iter().map(|(k, v)| Value::Array(vec![Value::String(k), v])).collect());
    Ok((GameData { global, stages, fighters, hash: content_hash(&all) }, errors))
}

fn err(file: &str, path: impl Into<String>, msg: impl Into<String>) -> ContentError {
    ContentError { file: file.into(), path: path.into(), msg: msg.into() }
}

fn parse(path: &str, text: &str) -> Result<Value, ContentError> {
    let mut v: Value = serde_json::from_str(text).map_err(|e| err(path, "", e.to_string()))?;
    migrate(&mut v).map_err(|m| err(path, "schemaVersion", m))?;
    Ok(v)
}

/// Upgrades older files in place. Each future format change adds a step here.
fn migrate(v: &mut Value) -> Result<(), String> {
    let ver = v.get("schemaVersion").and_then(Value::as_u64).ok_or("missing or not a number")?;
    if ver == 0 || ver > u64::from(SCHEMA_VERSION) {
        return Err(format!("version {ver} isn't supported (this build reads 1 to {SCHEMA_VERSION})"));
    }
    Ok(())
}

fn typed<T: DeserializeOwned>(file: &str, v: &Value) -> Result<T, ContentError> {
    serde_path_to_error::deserialize(v).map_err(|e| {
        let path = e.path().to_string();
        err(file, if path == "." { String::new() } else { path }, e.into_inner().to_string())
    })
}

/// The fighter's JSON with its `extends` chain merged in.
fn resolve(id: &str, raw: &BTreeMap<String, (String, Value)>, chain: &mut Vec<String>) -> Result<Value, ContentError> {
    let (path, v) = &raw[id];
    if chain.iter().any(|c| c == id) {
        return Err(err(path, "extends", format!("inheritance cycle: {} → {id}", chain.join(" → "))));
    }
    let Some(parent) = v.get("extends").map(|p| p.as_str().ok_or_else(|| err(path, "extends", "must be a string"))).transpose()? else {
        return Ok(v.clone());
    };
    if !raw.contains_key(parent) {
        return Err(err(path, "extends", format!("no fighter package {parent:?}")));
    }
    chain.push(id.to_string());
    let mut base = resolve(parent, raw, chain).map_err(|e| err(path, "extends", format!("parent {parent:?} failed: {e}")))?;
    chain.pop();
    if let Some(m) = base.as_object_mut() {
        m.shift_remove("abstract");
    }
    merge(&mut base, v);
    Ok(base)
}

fn check_stage(file: &str, s: &Stage) -> Result<(), ContentError> {
    let g = &s.ground;
    if g.left >= g.right || g.top >= g.bottom {
        return Err(err(file, "ground", "needs left < right and top < bottom"));
    }
    if s.spawns.is_empty() {
        return Err(err(file, "spawns", "needs at least one spawn"));
    }
    if s.respawns.is_empty() {
        return Err(err(file, "respawns", "needs at least one respawn point"));
    }
    if let Some(i) = s.platforms.iter().position(|p| p.x1 >= p.x2) {
        return Err(err(file, format!("platforms[{i}]"), "needs x1 < x2"));
    }
    Ok(())
}

/// Cross-reference checks serde can't do.
fn check_fighter(file: &str, f: &Fighter) -> Result<(), ContentError> {
    let e = |path: String, msg: String| err(file, path, msg);
    if !(0.0..=1.0).contains(&f.ai.counter_chance) {
        return Err(err(file, "ai.counterChance", "must be between 0 and 1"));
    }
    if let Some(name) = &f.ai.recovery_shield_cancel
        && !f.moves.uspec.phases.iter().any(|p| &p.name == name)
    {
        return Err(err(file, "ai.recoveryShieldCancel", "must name a phase in uspec"));
    }
    for (i, t) in f.ai.specials.iter().enumerate() {
        if !(0.0..=1.0).contains(&t.chance)
            || t.min_distance < 0.0
            || t.max_distance.is_some_and(|n| n <= t.min_distance)
            || t.height <= 0.0
            || t.hold.is_some_and(|[a, b]| a < 0.0 || b < a)
        {
            return Err(err(file, format!("ai.specials[{i}]"), "invalid distance, height, chance, or hold range"));
        }
    }
    let pose_ok = |r: &PoseRef| match r {
        PoseRef::Named(n) => f.poses.contains_key(n),
        PoseRef::Inline(p) => p.from.as_ref().is_none_or(|n| f.poses.contains_key(n)),
    };
    for (name, p) in &f.poses {
        let mut seen = BTreeSet::from([name.as_str()]);
        let mut cur = p;
        while let Some(from) = &cur.from {
            let Some(next) = f.poses.get(from) else {
                return Err(e(format!("poses.{name}.from"), format!("no pose {from:?}")));
            };
            if !seen.insert(from) {
                return Err(e(format!("poses.{name}.from"), "cycle".into()));
            }
            cur = next;
        }
    }
    if !f.poses.contains_key("stand") {
        return Err(e("poses".into(), "needs a \"stand\" pose (the default for missing joints)".into()));
    }
    let check_anim = |path: String, keys: &[super::visual::AnimKey]| -> Result<(), ContentError> {
        match keys.iter().position(|k| !pose_ok(&k.pose)) {
            Some(i) => Err(e(format!("{path}[{i}].pose"), "unknown pose".into())),
            None => Ok(()),
        }
    };
    for (name, Anim { keys, .. }) in &f.anims {
        if keys.is_empty() {
            return Err(e(format!("anims.{name}.keys"), "needs at least one key".into()));
        }
        check_anim(format!("anims.{name}.keys"), keys)?;
    }
    let attach_ids: BTreeSet<&str> = f.attachments.iter().map(|a| a.id.as_str()).collect();
    if attach_ids.len() != f.attachments.len() {
        return Err(e("attachments".into(), "ids must be unique".into()));
    }
    for (slot, m) in f.moves.iter() {
        check_move(&format!("moves.{slot}"), m, f, &attach_ids, &e, &check_anim)?;
    }
    for (id, p) in &f.projectiles {
        for (i, ev) in p.trail.iter().enumerate() {
            if ev.at.is_some() && (ev.from.is_some() || ev.to.is_some()) {
                return Err(e(format!("projectiles.{id}.trail[{i}]"), "use either `at` or `from`/`to`".into()));
            }
        }
    }
    Ok(())
}

fn check_move(
    path: &str,
    m: &Move,
    f: &Fighter,
    attach_ids: &BTreeSet<&str>,
    e: &impl Fn(String, String) -> ContentError,
    check_anim: &impl Fn(String, &[super::visual::AnimKey]) -> Result<(), ContentError>,
) -> Result<(), ContentError> {
    if m.phases.is_empty() {
        return Err(e(format!("{path}.phases"), "needs at least one phase".into()));
    }
    let names: BTreeSet<&str> = m.phases.iter().map(|p| p.name.as_str()).collect();
    if names.len() != m.phases.len() {
        return Err(e(format!("{path}.phases"), "phase names must be unique".into()));
    }
    for (pi, p) in m.phases.iter().enumerate() {
        let pp = format!("{path}.phases[{pi}]");
        if [END, HELPLESS, LANDING].contains(&p.name.as_str()) {
            return Err(e(format!("{pp}.name"), format!("{:?} is reserved", p.name)));
        }
        check_phase(&pp, p, &names, f, attach_ids, e)?;
        check_anim(format!("{pp}.anim"), &p.anim)?;
    }
    Ok(())
}

fn check_phase(
    pp: &str,
    p: &Phase,
    names: &BTreeSet<&str>,
    f: &Fighter,
    attach_ids: &BTreeSet<&str>,
    e: &impl Fn(String, String) -> ContentError,
) -> Result<(), ContentError> {
    for (i, h) in p.hitboxes.iter().enumerate() {
        if h.end.is_some_and(|end| end < h.start) {
            return Err(e(format!("{pp}.hitboxes[{i}]"), "end is before start".into()));
        }
    }
    for (i, ev) in p.events.iter().enumerate() {
        if ev.at.is_some() && (ev.from.is_some() || ev.to.is_some()) {
            return Err(e(format!("{pp}.events[{i}]"), "use either `at` or `from`/`to`".into()));
        }
        if let Some(s) = &ev.spawn
            && !f.projectiles.contains_key(s)
        {
            return Err(e(format!("{pp}.events[{i}].spawn"), format!("no projectile {s:?}")));
        }
    }
    for (i, t) in p.transitions.iter().enumerate() {
        let tp = format!("{pp}.transitions[{i}]");
        let target_ok = names.contains(t.to.as_str()) || [END, HELPLESS, LANDING].contains(&t.to.as_str());
        if !target_ok {
            return Err(e(format!("{tp}.to"), format!("no phase {:?} (or end/helpless/landing)", t.to)));
        }
        if (t.to == LANDING) != t.lag.is_some() {
            return Err(e(format!("{tp}.lag"), "`lag` goes with (and only with) `to: \"landing\"`".into()));
        }
        if t.on == Trigger::Timeout && p.frames.is_none() {
            return Err(e(format!("{tp}.on"), "timeout needs the phase to have `frames`".into()));
        }
        if t.on == Trigger::Counter && p.counter.is_none() {
            return Err(e(format!("{tp}.on"), "counter needs the phase to have a `counter` window".into()));
        }
    }
    if p.frames.is_none() && p.transitions.is_empty() {
        return Err(e(format!("{pp}.frames"), "a phase without `frames` needs a transition to leave it".into()));
    }
    if let Some(c) = &p.counter
        && c.from > c.to
    {
        return Err(e(format!("{pp}.counter"), "from is after to".into()));
    }
    if let Some(id) = p.attach.keys().find(|k| !attach_ids.contains(k.as_str())) {
        return Err(e(format!("{pp}.attach.{id}"), "no attachment with this id".into()));
    }
    Ok(())
}
