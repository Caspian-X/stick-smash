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
    if let Err(e) = check_global("global.json", &global) {
        errors.push(e);
        return Err(errors);
    }
    let mut hashed = vec![("global".to_string(), gv)];

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

    let mut stages = Vec::new();
    for (id, path, v) in raw_stages {
        match typed::<Stage>(&path, &v)
            .and_then(|s| check_stage(&path, &s, &fighters, global.sim.platform_pad).map(|()| s))
        {
            Ok(mut s) => {
                s.id = id.clone();
                hashed.push((format!("stage:{id}"), v));
                stages.push(s);
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
pub fn migrate(v: &mut Value) -> Result<(), String> {
    let ver = v.get("schemaVersion").and_then(Value::as_u64).ok_or("missing or not a number")?;
    if ver == 0 || ver > u64::from(SCHEMA_VERSION) {
        return Err(format!("version {ver} isn't supported (this build reads 1 to {SCHEMA_VERSION})"));
    }
    if ver == 1
        && let Some(obj) = v.as_object_mut()
        && let Some(mut ground) = obj.shift_remove("ground")
    {
        if let Some(g) = ground.as_object_mut() {
            g.insert("leftLedge".into(), Value::Bool(true));
            g.insert("rightLedge".into(), Value::Bool(true));
        }
        if let (Some(left), Some(right), Some(top), Some(spawns)) = (
            ground.get("left").and_then(Value::as_f64),
            ground.get("right").and_then(Value::as_f64),
            ground.get("top").cloned(),
            obj.get_mut("spawns").and_then(Value::as_array_mut),
        ) {
            for spawn in spawns {
                if spawn.get("y").is_none()
                    && spawn.get("x").and_then(Value::as_f64).is_some_and(|x| x < left || x > right)
                    && let Some(spawn) = spawn.as_object_mut()
                {
                    spawn.insert("y".into(), top.clone());
                }
            }
        }
        obj.insert("blocks".into(), Value::Array(vec![ground]));
        obj.insert("schemaVersion".into(), Value::from(2));
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
    let Some(parent) =
        v.get("extends").map(|p| p.as_str().ok_or_else(|| err(path, "extends", "must be a string"))).transpose()?
    else {
        return Ok(v.clone());
    };
    if !raw.contains_key(parent) {
        return Err(err(path, "extends", format!("no fighter package {parent:?}")));
    }
    chain.push(id.to_string());
    let mut base =
        resolve(parent, raw, chain).map_err(|e| err(path, "extends", format!("parent {parent:?} failed: {e}")))?;
    chain.pop();
    if let Some(m) = base.as_object_mut() {
        m.shift_remove("abstract");
    }
    merge(&mut base, v);
    Ok(base)
}

/// Checks serde can't do on a stage (clearance needs the fighters' bodies). [`load`] runs these; the editor runs
/// them on every edit.
pub fn check_stage(file: &str, s: &Stage, fighters: &[Fighter], platform_pad: f64) -> Result<(), ContentError> {
    positive(file, "width", s.width)?;
    positive(file, "height", s.height)?;
    positive(file, "ledge.r", s.ledge.r)?;
    for (name, b) in [("blast", &s.blast), ("camera", &s.camera)] {
        if b.left >= b.right || b.top >= b.bottom {
            return Err(err(file, name, "needs left < right and top < bottom"));
        }
    }
    if let Some(z) = &s.zoom {
        if let Some(n) = z.min_height {
            positive(file, "zoom.minHeight", n)?;
        }
        if let Some(n) = z.max_height {
            positive(file, "zoom.maxHeight", n)?;
        }
        if let (Some(min), Some(max)) = (z.min_height, z.max_height)
            && min > max
        {
            return Err(err(file, "zoom", "needs minHeight ≤ maxHeight"));
        }
    }
    for (i, sp) in s.spawns.iter().enumerate() {
        if ![-1, 1].contains(&sp.face) {
            return Err(err(file, format!("spawns[{i}].face"), "must be -1 or 1"));
        }
    }
    if s.blocks.is_empty() {
        return Err(err(file, "blocks", "needs at least one solid block"));
    }
    for (i, g) in s.blocks.iter().enumerate() {
        let path = format!("blocks[{i}]");
        if g.left >= g.right || g.top >= g.bottom {
            return Err(err(file, &path, "needs left < right and top < bottom"));
        }
        positive(file, &format!("{path}.underDepth"), g.under_depth)?;
        positive(file, &format!("{path}.underInset"), g.under_inset)?;
        if g.under_inset > (g.right - g.left) / 2. {
            return Err(err(file, format!("{path}.underInset"), "must not exceed half the block width"));
        }
        for (j, b) in s.blocks[..i].iter().enumerate() {
            if g.left < b.right
                && g.right > b.left
                && g.top < b.bottom + b.under_depth
                && g.bottom + g.under_depth > b.top
            {
                return Err(err(file, &path, format!("solid bounds overlap blocks[{j}] (including underside)")));
            }
        }
    }
    for (i, g) in s.blocks.iter().enumerate() {
        for (field, enabled, x) in [("leftLedge", g.left_ledge, g.left), ("rightLedge", g.right_ledge, g.right)] {
            if !enabled {
                continue;
            }
            for (j, b) in s.blocks.iter().enumerate() {
                let bottom = b.bottom + ((x - b.left).min(b.right - x) / b.under_inset).clamp(0., 1.) * b.under_depth;
                if i != j && x >= b.left && x <= b.right && g.top >= b.top && g.top <= bottom {
                    return Err(err(
                        file,
                        format!("blocks[{i}].{field}"),
                        format!("corner is covered by blocks[{j}]; disable this ledge"),
                    ));
                }
                if i != j
                    && fighters.iter().any(|f| {
                        b.intersects_body(
                            x + if field == "leftLedge" { -s.ledge.hang_x } else { s.ledge.hang_x },
                            g.top + s.ledge.hang_y,
                            f.body.half_width,
                            f.body.height,
                        )
                    })
                {
                    return Err(err(
                        file,
                        format!("blocks[{i}].{field}"),
                        format!("hanging body intersects blocks[{j}]"),
                    ));
                }
            }
        }
    }
    for (i, sp) in s.spawns.iter().enumerate() {
        let y =
            sp.y.or_else(|| s.spawn_y(sp.x))
                .ok_or_else(|| err(file, format!("spawns[{i}].y"), "required when x is not over a block"))?;
        for fighter in fighters {
            let body = &fighter.body;
            for (j, b) in s.blocks.iter().enumerate() {
                if b.intersects_body(sp.x, y, body.half_width, body.height) {
                    return Err(err(
                        file,
                        format!("spawns[{i}]"),
                        format!("{} body intersects blocks[{j}]", fighter.id),
                    ));
                }
            }
        }
    }
    for (i, p) in s.platforms.iter().enumerate() {
        if let Some(m) = &p.motion {
            let path = format!("platforms[{i}].motion.waypoints");
            if m.waypoints.len() < 2 {
                return Err(err(file, &path, "needs at least two waypoints"));
            }
            for (j, point) in m.waypoints.iter().enumerate() {
                if point.travel_ticks == 0
                    && !(m.mode == super::stage::PathMode::PingPong && j + 1 == m.waypoints.len())
                {
                    return Err(err(file, format!("{path}[{j}].travelTicks"), "must be positive"));
                }
            }
        }
    }
    for (i, p) in s.platforms.iter().enumerate() {
        let points: Vec<_> =
            p.motion.as_ref().map_or_else(|| vec![(0., 0.)], |m| m.waypoints.iter().map(|p| (p.x, p.y)).collect());
        let segments = if p.motion.as_ref().is_some_and(|m| m.mode == super::stage::PathMode::PingPong) {
            points.len() - 1
        } else {
            points.len()
        };
        for k in 0..segments {
            let (a, b) = (points[k], points[(k + 1) % points.len()]);
            for (j, block) in s.blocks.iter().enumerate() {
                for f in fighters {
                    let rect = [
                        p.x1 + a.0 - f.body.half_width - platform_pad,
                        p.y + a.1 - f.body.height,
                        p.x2 + a.0 + f.body.half_width + platform_pad,
                        p.y + a.1,
                    ];
                    if block.intersects_sweep(rect, [b.0 - a.0, b.1 - a.1]) {
                        return Err(err(
                            file,
                            format!("platforms[{i}]"),
                            format!("path lacks body clearance from blocks[{j}] for {}", f.id),
                        ));
                    }
                }
            }
        }
    }
    for (i, point) in s.respawns.iter().enumerate() {
        for (j, block) in s.blocks.iter().enumerate() {
            for f in fighters {
                if block.intersects_sweep(
                    [
                        point.x - f.body.half_width,
                        point.start_y - f.body.height,
                        point.x + f.body.half_width,
                        point.start_y,
                    ],
                    [0., point.end_y - point.start_y],
                ) {
                    return Err(err(
                        file,
                        format!("respawns[{i}]"),
                        format!("descending body intersects blocks[{j}] for {}", f.id),
                    ));
                }
            }
        }
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

/// Cross-reference checks serde can't do, on a fully merged fighter. [`load`] runs these; the editor runs them on
/// every edit before simulating.
pub fn check_fighter(file: &str, f: &Fighter) -> Result<(), ContentError> {
    let e = |path: String, msg: String| err(file, path, msg);
    let stats = serde_json::to_value(&f.stats).unwrap();
    for (name, value) in stats.as_object().unwrap() {
        let n = value.as_f64().unwrap();
        if matches!(
            name.as_str(),
            "weight"
                | "runSpeed"
                | "jumpSpeed"
                | "shortHopSpeed"
                | "airJumpSpeed"
                | "airSpeed"
                | "gravity"
                | "fallSpeed"
                | "fastFallSpeed"
        ) {
            positive(file, &format!("stats.{name}"), n)?;
        } else {
            nonnegative(file, &format!("stats.{name}"), n)?;
        }
    }
    positive(file, "body.halfWidth", f.body.half_width)?;
    positive(file, "body.height", f.body.height)?;
    for (name, r) in [("default", f.hurtboxes.default), ("knockdown", f.hurtboxes.knockdown)] {
        if r.x1 >= r.x2 || r.y1 >= r.y2 {
            return Err(err(file, format!("hurtboxes.{name}"), "needs x1 < x2 and y1 < y2"));
        }
    }
    nonnegative(file, "ai.reach", f.ai.reach)?;
    for (name, t) in [("f", &f.throws.f), ("b", &f.throws.b), ("u", &f.throws.u), ("d", &f.throws.d)] {
        if t.release == 0 || t.total < t.release {
            return Err(err(file, format!("throws.{name}"), "needs 0 < release <= total"));
        }
        check_hitbox(file, &format!("throws.{name}.hit"), &t.hit, false)?;
    }
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
        check_move(file, &format!("moves.{slot}"), m, f, &attach_ids, &e, &check_anim)?;
    }
    for (id, p) in &f.projectiles {
        let path = format!("projectiles.{id}");
        positive(file, &format!("{path}.r"), p.r)?;
        nonnegative(file, &format!("{path}.gravity"), p.gravity)?;
        if let Some(n) = p.max_fall {
            positive(file, &format!("{path}.maxFall"), n)?;
        }
        for (name, n) in [("life", p.life), ("max", p.max)] {
            positive(file, &format!("{path}.{name}"), f64::from(n))?;
            if n > i32::MAX as u32 {
                return Err(err(file, format!("{path}.{name}"), "exceeds i32::MAX"));
            }
        }
        if let Some(b) = &p.bat {
            positive(file, &format!("{path}.bat.speed"), b.speed)?;
            check_life(file, &format!("{path}.bat.life"), b.life)?;
        }
        if let Some(b) = &p.bounce {
            for (name, n) in [
                ("restitution", b.restitution),
                ("minSpeed", b.min_speed),
                ("friction", b.friction),
                ("wallRestitution", b.wall_restitution),
            ] {
                nonnegative(file, &format!("{path}.bounce.{name}"), n)?;
            }
            check_life(file, &format!("{path}.bounce.fizzleLife"), b.fizzle_life)?;
        }
        check_hitbox(file, &format!("{path}.hit"), &p.hit, false)?;
        for (i, ev) in p.trail.iter().enumerate() {
            check_event(file, &format!("{path}.trail[{i}]"), ev)?;
            if ev.at.is_some() && (ev.from.is_some() || ev.to.is_some()) {
                return Err(e(format!("projectiles.{id}.trail[{i}]"), "use either `at` or `from`/`to`".into()));
            }
        }
    }
    Ok(())
}

fn check_move(
    file: &str,
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
        for (i, h) in p.hitboxes.iter().enumerate() {
            check_hitbox(file, &format!("{pp}.hitboxes[{i}]"), h, true)?;
        }
        if let Some(n) = p.fall_cap {
            positive(file, &format!("{pp}.fallCap"), n)?;
        }
        nonnegative(file, &format!("{pp}.airAccelMult"), p.air_accel_mult)?;
        if let Some(c) = &p.counter {
            for (name, n) in
                [("damageMult", c.damage_mult), ("minDamage", c.min_damage), ("reflectSpeed", c.reflect_speed)]
            {
                nonnegative(file, &format!("{pp}.counter.{name}"), n)?;
            }
            check_life(file, &format!("{pp}.counter.reflectLife"), c.reflect_life)?;
        }
        check_phase(file, &pp, p, &names, f, attach_ids, e)?;
        check_anim(format!("{pp}.anim"), &p.anim)?;
    }
    Ok(())
}

fn check_phase(
    file: &str,
    pp: &str,
    p: &Phase,
    names: &BTreeSet<&str>,
    f: &Fighter,
    attach_ids: &BTreeSet<&str>,
    e: &impl Fn(String, String) -> ContentError,
) -> Result<(), ContentError> {
    for (i, ev) in p.events.iter().enumerate() {
        check_event(file, &format!("{pp}.events[{i}]"), ev)?;
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

fn positive(file: &str, path: &str, n: f64) -> Result<(), ContentError> {
    if n.is_finite() && n > 0. { Ok(()) } else { Err(err(file, path, "must be finite and positive")) }
}
fn nonnegative(file: &str, path: &str, n: f64) -> Result<(), ContentError> {
    if n.is_finite() && n >= 0. { Ok(()) } else { Err(err(file, path, "must be finite and non-negative")) }
}
fn check_life(file: &str, path: &str, n: u32) -> Result<(), ContentError> {
    if n > 0 && n <= i32::MAX as u32 { Ok(()) } else { Err(err(file, path, "must be between 1 and i32::MAX")) }
}
fn check_hitbox(file: &str, path: &str, h: &super::Hitbox, geometry: bool) -> Result<(), ContentError> {
    if geometry {
        positive(file, &format!("{path}.r"), h.r)?;
    }
    for (name, n) in [
        ("damage", h.damage),
        ("bkb", h.bkb),
        ("kbg", h.kbg),
        ("hitstunMult", h.hitstun_mult),
        ("hitlagMult", h.hitlag_mult),
        ("shieldDmgMult", h.shield_dmg_mult),
        ("shieldStunMult", h.shield_stun_mult),
    ] {
        nonnegative(file, &format!("{path}.{name}"), n)?;
    }
    if h.end.is_some_and(|n| n < h.start) {
        return Err(err(file, path, "end is before start"));
    }
    if h.rehit == Some(0) {
        return Err(err(file, format!("{path}.rehit"), "must be positive"));
    }
    for (i, frame) in h.frames.iter().enumerate() {
        nonnegative(file, &format!("{path}.frames[{i}][0]"), frame[0])?;
        if geometry {
            positive(file, &format!("{path}.frames[{i}][3]"), frame[3])?;
        }
    }
    Ok(())
}
fn check_event(file: &str, path: &str, ev: &super::Event) -> Result<(), ContentError> {
    if ev.every == Some(0) {
        return Err(err(file, format!("{path}.every"), "must be positive"));
    }
    if ev.from.zip(ev.to).is_some_and(|(a, b)| a > b) {
        return Err(err(file, path, "from is after to"));
    }
    if let Some(n) = ev.cap_fall {
        positive(file, &format!("{path}.capFall"), n)?;
    }
    Ok(())
}
fn check_global(file: &str, g: &Global) -> Result<(), ContentError> {
    check_audio(file, &g.audio)?;
    if g.input.stick_threshold <= 0 {
        return Err(err(file, "input.stickThreshold", "must be between 1 and 127"));
    }
    if g.palette.is_empty() {
        return Err(err(file, "palette", "needs at least one color set"));
    }
    let c = &g.render.camera;
    for (name, n) in [
        ("minHeight", c.min_height),
        ("maxHeight", c.max_height),
        ("referencePlayWidth", c.reference_play_width),
        ("referencePlayHeight", c.reference_play_height),
    ] {
        positive(file, &format!("render.camera.{name}"), n)?;
    }
    if c.min_height > c.max_height {
        return Err(err(file, "render.camera", "needs minHeight ≤ maxHeight"));
    }
    for (name, n) in [("minScaleStart", c.min_scale_start), ("minScaleFloor", c.min_scale_floor)] {
        positive(file, &format!("render.camera.{name}"), n)?;
        if n > 1. {
            return Err(err(file, format!("render.camera.{name}"), "must be ≤ 1"));
        }
    }
    positive(file, "knockback.chargeDamageDivisor", g.knockback.charge_damage_divisor)?;
    positive(file, "hitTypes.windbox.maxSpeed", g.hit_types.windbox.max_speed)?;
    positive(file, "shield.max", g.shield.max)?;
    positive(file, "hitstun.gravity", g.hitstun.gravity)?;
    positive(file, "hitstun.maxFall", g.hitstun.max_fall)?;
    positive(file, "shield.breakGravity", g.shield.break_gravity)?;
    positive(file, "shield.breakMaxFall", g.shield.break_max_fall)?;
    positive(file, "respawn.descendFrames", f64::from(g.respawn.descend_frames))?;
    positive(file, "match.endFrames", f64::from(g.match_.end_frames))?;
    for (name, c) in
        [("climb", &g.ledge.climb), ("attack", &g.ledge.attack), ("roll", &g.ledge.roll), ("jump", &g.ledge.jump)]
    {
        if c.a == 0 || c.b < c.a || c.end < c.b || name != "jump" && c.b == c.a {
            return Err(err(file, format!("ledge.{name}"), "needs 0 < a < b <= end (jump may have a == b)"));
        }
        nonnegative(file, &format!("ledge.{name}.dist"), c.dist)?;
    }
    // These sections contain magnitudes/timings; signed velocities and angles are excluded.
    let value = serde_json::to_value(g).unwrap();
    for section in [
        "movement",
        "knockback",
        "hitstun",
        "hitlag",
        "shield",
        "dodge",
        "landing",
        "tech",
        "grab",
        "respawn",
        "match",
        "sim",
    ] {
        for (name, n) in value[section].as_object().unwrap() {
            if section == "movement" && name == "fastFallMinVy" {
                continue;
            }
            if let Some(n) = n.as_f64() {
                nonnegative(file, &format!("{section}.{name}"), n)?;
            }
        }
    }
    for (name, boxes) in [("knockdown.attack", &g.knockdown.attack), ("ledge.attackHit", &g.ledge.attack_hit)] {
        for (i, h) in boxes.iter().enumerate() {
            check_hitbox(file, &format!("{name}[{i}]"), h, true)?;
        }
    }
    for (name, [a, b]) in
        [("knockdown.attackActive", g.knockdown.attack_active), ("ledge.attackActive", g.ledge.attack_active)]
    {
        if a > b {
            return Err(err(file, name, "start is after end"));
        }
    }
    if g.grab.pummel_hit_frame >= g.grab.pummel_frames {
        return Err(err(file, "grab.pummelHitFrame", "must be less than pummelFrames"));
    }
    if g.dodge.roll_move_start > g.dodge.roll_move_end || g.dodge.roll_move_end > g.dodge.roll_frames {
        return Err(err(file, "dodge.rollMoveEnd", "needs rollMoveStart <= rollMoveEnd <= rollFrames"));
    }
    for name in ["react", "charge", "aggression", "mash", "tech", "shieldResponse", "defend", "projectileDefend"] {
        for (i, n) in value["ai"][name].as_array().unwrap().iter().enumerate() {
            let n = n.as_f64().unwrap();
            nonnegative(file, &format!("ai.{name}[{i}]"), n)?;
            if !["react", "charge"].contains(&name) && n > 1. {
                return Err(err(file, format!("ai.{name}[{i}]"), "must be between 0 and 1"));
            }
        }
    }
    let v = &g.ai.thresholds;
    for (name, a, b) in [
        ("ledgeWait", v.ledge_wait_min, v.ledge_wait_max),
        ("hold", v.hold_min, v.hold_max),
        ("shieldHold", v.shield_hold_min, v.shield_hold_max),
    ] {
        if a < 0. || b < a {
            return Err(err(file, format!("ai.thresholds.{name}Max"), "needs 0 <= min <= max"));
        }
    }
    nonnegative(file, "ai.thresholds.targetPad", v.target_pad)?;
    Ok(())
}

fn check_audio(file: &str, a: &super::audio::Audio) -> Result<(), ContentError> {
    let range = |path: &str, n: f64, lo: f64, hi: f64| {
        if n.is_finite() && (lo..=hi).contains(&n) {
            Ok(())
        } else {
            Err(err(file, path, format!("must be between {lo} and {hi}")))
        }
    };
    range("audio.sampleRate", f64::from(a.sample_rate), 8000., 96000.)?;
    range("audio.strengthLevels", a.strength_levels as f64, 1., 16.)?;
    range("audio.maxVoices", a.max_voices as f64, 1., 128.)?;
    range("audio.maxPerFrame", a.max_per_frame as f64, 1., 32.)?;
    range("audio.pan", a.pan, 0., 4.)?;
    range("audio.depth", a.depth, 0.1, 4.)?;
    range("audio.earGap", a.ear_gap, 0.1, 4.)?;
    range("audio.pitchVariation", a.pitch_variation, 0., 0.5)?;
    range("audio.volumeStep", a.volume_step, 0.001, 1.)?;
    range("audio.cooldown", a.cooldown, 0., 10.)?;
    positive(file, "audio.strengthMax", a.strength_max)?;
    for (n, v) in [("master", a.defaults.master), ("sfx", a.defaults.sfx), ("music", a.defaults.music)] {
        range(&format!("audio.defaults.{n}"), v, 0., 1.)?;
    }
    if a.sounds.len() > 128 {
        return Err(err(file, "audio.sounds", "at most 128 sounds"));
    }
    for (name, s) in &a.sounds {
        let p = format!("audio.sounds.{name}");
        range(&format!("{p}.volume"), s.volume, 0., 1.)?;
        range(&format!("{p}.cooldown"), s.cooldown, 0., 10.)?;
        range(&format!("{p}.strengthGain"), s.strength_gain, 0., 1.)?;
        range(&format!("{p}.strengthDuration"), s.strength_duration, 0.1, 4.)?;
        range(&format!("{p}.layers"), s.layers.len() as f64, 1., 8.)?;
        for (i, l) in s.layers.iter().enumerate() {
            let p = format!("{p}.layers[{i}]");
            for (n, v) in [("from", l.from), ("to", l.to)] {
                range(&format!("{p}.{n}"), v, 1., f64::from(a.sample_rate) / 2.)?;
            }
            range(&format!("{p}.duration"), l.duration, 0.001, 3.)?;
            range(&format!("{p}.attack"), l.attack, 0., l.duration)?;
            range(&format!("{p}.decay"), l.decay, 0., 30.)?;
            range(&format!("{p}.gain"), l.gain, 0., 1.)?;
        }
    }
    Ok(())
}
