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

/// A problem in a content source or resolved package.
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

/// Reads JSON sources in path order, skipping `schema` directories and symbolic links. A missing root is empty.
/// Read failures name the root and affected path; callers can log them before loading the remaining sources.
#[cfg(feature = "fs")]
pub fn sources(root: impl AsRef<std::path::Path>) -> (Vec<Source>, Vec<ContentError>) {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<Source>, errors: &mut Vec<ContentError>) {
        let path = |p: &std::path::Path| p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/");
        let shown = |p: &std::path::Path| p.to_string_lossy().replace('\\', "/");
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if dir == root && e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                errors.push(err(&shown(dir), "", e.to_string()));
                return;
            }
        };
        let mut entries: Vec<_> = entries
            .filter_map(|e| match e {
                Ok(e) => Some(e),
                Err(e) => {
                    errors.push(err(&shown(dir), "", e.to_string()));
                    None
                }
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let p = entry.path();
            match entry.file_type() {
                Ok(t) if t.is_dir() && entry.file_name() != "schema" => walk(root, &p, out, errors),
                Ok(t) if t.is_file() && p.extension().is_some_and(|e| e == "json") => {
                    match std::fs::read_to_string(&p) {
                        Ok(text) => out.push(Source { path: path(&p), text }),
                        Err(e) => errors.push(err(&shown(&p), "", e.to_string())),
                    }
                }
                Err(e) => errors.push(err(&shown(&p), "", e.to_string())),
                _ => {}
            }
        }
    }
    let (mut out, mut errors) = (Vec::new(), Vec::new());
    walk(root.as_ref(), root.as_ref(), &mut out, &mut errors);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    errors.sort_by(|a, b| a.file.cmp(&b.file));
    (out, errors)
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
            ["global.json"] => ContentKind::Global,
            ["fighters", _, "fighter.json"] => ContentKind::Fighter,
            ["stages", _, "stage.json"] => ContentKind::Stage,
            _ => continue,
        };
        let v = match parse(path, text, kind) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        match kind {
            ContentKind::Global => raw_global = Some(v),
            ContentKind::Fighter => drop(raw_fighters.insert(parts[1].to_string(), (path.clone(), v))),
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
        match typed::<Fighter>(path, &merged) {
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
        match typed::<Stage>(&path, &v) {
            Ok(mut s) => {
                s.id = id.clone();
                hashed.push((format!("stage:{id}"), v));
                stages.push(s);
            }
            Err(e) => errors.push(e),
        }
    }

    let mut data = GameData { global, stages, fighters, hash: 0 };
    let invalid = validate(&data);
    // `validate` runs `check_global`; nothing loads without a valid global.
    if invalid.iter().any(|e| e.file == "global.json") {
        return Err(invalid);
    }
    let skipped: BTreeSet<_> = invalid.iter().map(|e| e.file.as_str()).collect();
    data.fighters.retain(|f| !skipped.contains(format!("fighters/{}/fighter.json", f.id).as_str()));
    data.stages.retain(|s| !skipped.contains(format!("stages/{}/stage.json", s.id).as_str()));
    errors.extend(invalid);
    if data.fighters.is_empty() || data.stages.is_empty() {
        errors.push(err(
            "",
            "",
            if data.fighters.is_empty() { "no playable fighter loaded" } else { "no stage loaded" },
        ));
        return Err(errors);
    }
    hashed.retain(|(key, _)| match key.split_once(':') {
        Some(("fighter", id)) => data.fighter(id).is_some(),
        Some(("stage", id)) => data.stage(id).is_some(),
        _ => true,
    });
    let all = Value::Array(hashed.into_iter().map(|(k, v)| Value::Array(vec![Value::String(k), v])).collect());
    data.hash = content_hash(&all);
    Ok((data, errors))
}

/// Validates resolved content (including embedded replay data). Reports the first problem per package,
/// plus missing packages and duplicate ids. Paths use the same file/JSON coordinates as [`load`].
pub fn validate(data: &GameData) -> Vec<ContentError> {
    let mut errors = Vec::new();
    errors.extend(check_global("global.json", &data.global).err());
    let mut ids = BTreeSet::new();
    for f in &data.fighters {
        let file = format!("fighters/{}/fighter.json", f.id);
        if !ids.insert(&f.id) {
            errors.push(err(&file, "id", "duplicate fighter package id"));
        }
        errors.extend(check_fighter(&file, f).err());
        for (kind, configured, name) in [
            (super::HitType::Paralyze, data.global.hit_types.paralyze.is_some(), "paralyze"),
            (super::HitType::Bury, data.global.hit_types.bury.is_some(), "bury"),
        ] {
            if configured {
                continue;
            }
            let uses = f
                .moves
                .iter()
                .flat_map(|(_, m)| &m.phases)
                .flat_map(|p| &p.hitboxes)
                .chain(f.projectiles.values().map(|p| &p.hit))
                .chain(
                    f.projectiles
                        .values()
                        .filter_map(|p| p.article.as_ref())
                        .flat_map(|a| &a.stages)
                        .flat_map(|s| &s.hitboxes),
                )
                .chain([&f.throws.f.hit, &f.throws.b.hit, &f.throws.u.hit, &f.throws.d.hit])
                .any(|h| h.kind == kind);
            if uses {
                errors.push(err(&file, "moves", format!("{name} hits require global hitTypes.{name}")));
            }
        }
    }
    let fighters = if errors.iter().any(|e| e.file.starts_with("fighters/")) {
        std::borrow::Cow::Owned(
            data.fighters
                .iter()
                .filter(|f| !errors.iter().any(|e| e.file == format!("fighters/{}/fighter.json", f.id)))
                .cloned()
                .collect::<Vec<_>>(),
        )
    } else {
        std::borrow::Cow::Borrowed(data.fighters.as_slice())
    };
    ids.clear();
    for s in &data.stages {
        let file = format!("stages/{}/stage.json", s.id);
        if !ids.insert(&s.id) {
            errors.push(err(&file, "id", "duplicate stage package id"));
        }
        errors.extend(check_stage(&file, s, &fighters, data.global.sim.platform_pad).err());
    }
    if data.fighters.is_empty() {
        errors.push(err("", "fighters", "needs at least one playable fighter"));
    }
    if data.stages.is_empty() {
        errors.push(err("", "stages", "needs at least one stage"));
    }
    errors
}

fn err(file: &str, path: impl Into<String>, msg: impl Into<String>) -> ContentError {
    ContentError { file: file.into(), path: path.into(), msg: msg.into() }
}

fn parse(path: &str, text: &str, kind: ContentKind) -> Result<Value, ContentError> {
    let mut v: Value = serde_json::from_str(text).map_err(|e| err(path, "", e.to_string()))?;
    migrate_kind(kind, &mut v).map_err(|m| err(path, "schemaVersion", m))?;
    Ok(v)
}

/// Content formats have independent version histories.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentKind {
    Global,
    Fighter,
    Stage,
}

/// Upgrades only the selected content format. Fighter/global version 2 is a legacy alias for version 1.
pub fn migrate_kind(kind: ContentKind, v: &mut Value) -> Result<(), String> {
    let ver = v.get("schemaVersion").and_then(Value::as_u64).ok_or("missing or not a number")?;
    let max = match kind {
        ContentKind::Stage => SCHEMA_VERSION,
        ContentKind::Fighter | ContentKind::Global => 2,
    };
    if ver == 0 || ver > u64::from(max) {
        return Err(format!("version {ver} isn't supported (this build reads 1 to {max})"));
    }
    if kind != ContentKind::Stage {
        v["schemaVersion"] = Value::from(1);
        return Ok(());
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
    for (name, n) in [("hangX", s.ledge.hang_x), ("hangY", s.ledge.hang_y)] {
        finite(file, &format!("ledge.{name}"), n)?;
    }
    for (name, b) in [("blast", &s.blast), ("camera", &s.camera)] {
        for (field, n) in [("left", b.left), ("right", b.right), ("top", b.top), ("bottom", b.bottom)] {
            finite(file, &format!("{name}.{field}"), n)?;
        }
        if b.left >= b.right || b.top >= b.bottom {
            return Err(err(file, name, "needs left < right and top < bottom"));
        }
    }
    if let Some(z) = &s.zoom {
        if z.min_height.is_none() && z.max_height.is_none() {
            return Err(err(file, "zoom", "needs at least one height override; omit zoom for automatic limits"));
        }
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
        finite(file, &format!("spawns[{i}].x"), sp.x)?;
        if let Some(y) = sp.y {
            finite(file, &format!("spawns[{i}].y"), y)?;
        }
        if ![-1, 1].contains(&sp.face) {
            return Err(err(file, format!("spawns[{i}].face"), "must be -1 or 1"));
        }
    }
    if s.blocks.is_empty() {
        return Err(err(file, "blocks", "needs at least one solid block"));
    }
    for (i, g) in s.blocks.iter().enumerate() {
        let path = format!("blocks[{i}]");
        for (field, n) in [("left", g.left), ("right", g.right), ("top", g.top), ("bottom", g.bottom)] {
            finite(file, &format!("{path}.{field}"), n)?;
        }
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
            for f in fighters {
                if let Some(l) = crate::interactions::ledge(f) {
                    for (name, distance) in [
                        ("pullIn", l.pull_in),
                        ("climbDistance", l.climb_distance),
                        ("rollDistance", l.roll_distance),
                        ("attackDistance", l.attack_distance),
                    ] {
                        let Some(distance) = distance else { continue };
                        let side = if field == "leftLedge" { -1. } else { 1. };
                        let end = x - side * distance;
                        let path = format!("blocks[{i}].{field}");
                        if !end.is_finite() || end < g.left || end > g.right {
                            return Err(err(
                                file,
                                &path,
                                format!("{} interactions.ledge.{name} endpoint leaves supporting block", f.id),
                            ));
                        }
                        if let Some((j, _)) = s
                            .blocks
                            .iter()
                            .enumerate()
                            .find(|(_, b)| b.intersects_body(end, g.top, f.body.half_width, f.body.height))
                        {
                            return Err(err(
                                file,
                                &path,
                                format!("{} interactions.ledge.{name} endpoint intersects blocks[{j}]", f.id),
                            ));
                        }
                    }
                }
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
                if fighters.iter().any(|f| {
                    let overridden = crate::interactions::ledge(f).is_some_and(|l| l.hang_anchor.is_some());
                    let root = crate::interactions::ledge_hang_root(
                        f,
                        s,
                        x,
                        g.top,
                        if field == "leftLedge" { -1. } else { 1. },
                    );
                    (i != j || overridden) && b.intersects_body(root[0], root[1], f.body.half_width, f.body.height)
                }) {
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
        for (field, n) in [("x1", p.x1), ("x2", p.x2), ("y", p.y)] {
            finite(file, &format!("platforms[{i}].{field}"), n)?;
        }
        if let Some(m) = &p.motion {
            let path = format!("platforms[{i}].motion.waypoints");
            if m.waypoints.len() < 2 {
                return Err(err(file, &path, "needs at least two waypoints"));
            }
            for (j, point) in m.waypoints.iter().enumerate() {
                finite(file, &format!("{path}[{j}].x"), point.x)?;
                finite(file, &format!("{path}[{j}].y"), point.y)?;
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
        for (field, n) in [("x", point.x), ("startY", point.start_y), ("endY", point.end_y)] {
            finite(file, &format!("respawns[{i}].{field}"), n)?;
        }
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

// Leave ample headroom for composition and downstream squared-distance arithmetic.
fn interaction_number(file: &str, path: &str, n: f64) -> Result<(), ContentError> {
    finite(file, path, n)?;
    if n.abs() > 1e100 {
        return Err(err(file, path, "interaction magnitude must not exceed 1e100"));
    }
    Ok(())
}

/// Cross-reference checks serde can't do, on a fully merged fighter. [`load`] runs these; the editor runs them on
/// every edit before simulating.
pub fn check_fighter(file: &str, f: &Fighter) -> Result<(), ContentError> {
    let e = |path: String, msg: String| err(file, path, msg);
    check_stats(file, &f.stats)?;
    if let Some(g) = crate::interactions::grab(f) {
        for (name, p) in [("catchPoint", g.catch_point), ("holdPoint", g.hold_point), ("heldAnchor", g.held_anchor)] {
            if let Some(p) = p {
                for (i, n) in p.into_iter().enumerate() {
                    interaction_number(file, &format!("interactions.grab.{name}[{i}]"), n)?;
                }
            }
        }
    }
    if let Some(l) = crate::interactions::ledge(f) {
        for (name, p) in [("catchPoint", l.catch_point), ("hangAnchor", l.hang_anchor)] {
            if let Some(p) = p {
                for (i, n) in p.into_iter().enumerate() {
                    interaction_number(file, &format!("interactions.ledge.{name}[{i}]"), n)?;
                }
            }
        }
        for (name, n) in [
            ("pullIn", l.pull_in),
            ("climbDistance", l.climb_distance),
            ("rollDistance", l.roll_distance),
            ("attackDistance", l.attack_distance),
        ] {
            if let Some(n) = n {
                nonnegative(file, &format!("interactions.ledge.{name}"), n)?;
                interaction_number(file, &format!("interactions.ledge.{name}"), n)?;
            }
        }
    }
    if let Some(rig) = &f.rig {
        crate::rig::resolve(rig, &Default::default()).map_err(|e| err(file, format!("rig.{}", e.path), e.message))?;
        for (i, joint) in rig.joints.iter().enumerate() {
            for (j, shape) in joint.shapes.iter().enumerate() {
                check_rig_shape(file, &format!("rig.joints[{i}].shapes[{j}]"), shape)?;
            }
        }
    }
    positive(file, "body.halfWidth", f.body.half_width)?;
    positive(file, "body.height", f.body.height)?;
    for (name, r) in [("default", f.hurtboxes.default), ("knockdown", f.hurtboxes.knockdown)] {
        for (field, n) in [("x1", r.x1), ("x2", r.x2), ("y1", r.y1), ("y2", r.y2)] {
            finite(file, &format!("hurtboxes.{name}.{field}"), n)?;
        }
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
        for (i, tick) in t.ticks.iter().enumerate() {
            let p = format!("throws.{name}.ticks[{i}]");
            nonnegative(file, &format!("{p}.damage"), tick.damage)?;
            if tick.at >= t.release {
                return Err(err(file, p, "must precede release"));
            }
        }
    }
    probability(file, "ai.counterChance", f.ai.counter_chance)?;
    if let Some(name) = &f.ai.recovery_shield_cancel
        && !f.moves.uspec.phases.iter().any(|p| &p.name == name)
    {
        return Err(err(file, "ai.recoveryShieldCancel", "must name a phase in uspec"));
    }
    for (i, t) in f.ai.specials.iter().enumerate() {
        let path = format!("ai.specials[{i}]");
        probability(file, &format!("{path}.chance"), t.chance)?;
        nonnegative(file, &format!("{path}.minDistance"), t.min_distance)?;
        positive(file, &format!("{path}.height"), t.height)?;
        if let Some(n) = t.max_distance {
            finite(file, &format!("{path}.maxDistance"), n)?;
            if n <= t.min_distance {
                return Err(err(file, format!("{path}.maxDistance"), "must exceed minDistance"));
            }
        }
        if let Some([a, b]) = t.hold {
            nonnegative_range(file, &format!("{path}.hold"), a, b)?;
        }
    }
    let pose_ok = |r: &PoseRef| match r {
        PoseRef::Named(n) => f.poses.contains_key(n),
        PoseRef::Inline(p) => p.from.as_ref().is_none_or(|n| f.poses.contains_key(n)),
    };
    for (name, p) in &f.poses {
        check_rig_pose(file, &format!("poses.{name}.joints"), f, p)?;
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
    if let Some(rig) = &f.rig {
        for (name, pose) in &f.poses {
            crate::rig::resolve(rig, &crate::pose::resolve_rig_pose(f, pose))
                .map_err(|e| err(file, format!("poses.{name}.joints"), e.to_string()))?;
        }
    }
    if !f.poses.contains_key("stand") {
        return Err(e("poses".into(), "needs a \"stand\" pose (the default for missing joints)".into()));
    }
    let check_anim = |path: String, keys: &[super::visual::AnimKey]| -> Result<(), ContentError> {
        for (i, key) in keys.iter().enumerate() {
            nonnegative(file, &format!("{path}[{i}].at"), key.at)?;
            if let PoseRef::Inline(p) = &key.pose {
                check_rig_pose(file, &format!("{path}[{i}].pose.joints"), f, p)?;
                if let Some(rig) = &f.rig {
                    crate::rig::resolve(rig, &crate::pose::resolve_rig_pose(f, p))
                        .map_err(|e| err(file, format!("{path}[{i}].pose.joints"), e.to_string()))?;
                }
            }
        }
        match keys.iter().position(|k| !pose_ok(&k.pose)) {
            Some(i) => Err(e(format!("{path}[{i}].pose"), "unknown pose".into())),
            None => Ok(()),
        }
    };
    for (name, Anim { keys, period, .. }) in &f.anims {
        if let Some(n) = period {
            positive(file, &format!("anims.{name}.period"), *n)?;
        }
        if keys.is_empty() {
            return Err(e(format!("anims.{name}.keys"), "needs at least one key".into()));
        }
        check_anim(format!("anims.{name}.keys"), keys)?;
    }
    let attach_ids: BTreeSet<&str> = f.attachments.iter().map(|a| a.id.as_str()).collect();
    if attach_ids.len() != f.attachments.len() {
        return Err(e("attachments".into(), "ids must be unique".into()));
    }
    if let Some(center) = f.combat_center {
        for n in center {
            finite(file, "combatCenter", n)?;
        }
    }
    if let Some(regions) = &f.hurt_regions {
        if regions.is_empty() {
            return Err(err(file, "hurtRegions", "must be nonempty"));
        }
        if f.combat_center.is_none() {
            return Err(err(file, "combatCenter", "required with hurtRegions"));
        }
        if !regions.iter().any(|r| r.enabled) {
            return Err(err(file, "hurtRegions", "requires at least one default-enabled region"));
        }
        let mut ids = BTreeSet::new();
        for (i, r) in regions.iter().enumerate() {
            let path = format!("hurtRegions[{i}]");
            if r.id.trim().is_empty() || !ids.insert(&r.id) {
                return Err(err(file, format!("{path}.id"), "requires unique nonblank ID"));
            }
            for (j, value) in r.states.iter().enumerate() {
                if r.states[..j].iter().any(|v| v.state == value.state) {
                    return Err(err(file, format!("{path}.states[{j}].state"), "duplicate state selector"));
                }
            }
            check_combat_geometry(file, &format!("{path}.geometry"), &r.geometry, f)?;
        }
    }
    for (slot, m) in f.moves.iter() {
        for (i, p) in m.phases.iter().enumerate() {
            for (j, window) in p.hurt_regions.iter().enumerate() {
                let path = format!("moves.{slot}.phases[{i}].hurtRegions[{j}]");
                if !f.hurt_regions.as_ref().is_some_and(|rs| rs.iter().any(|r| r.id == window.region)) {
                    return Err(err(
                        file,
                        format!("{path}.region"),
                        "unknown or deleted hurtRegions ID; restore the region or update this reference",
                    ));
                }
                if window.from > window.to || p.frames.is_some_and(|n| window.to >= n) {
                    return Err(err(
                        file,
                        &path,
                        "requires from <= to and to before phase end (phase end enters the next phase)",
                    ));
                }
                if p.hurt_regions[..j]
                    .iter()
                    .any(|v| v.region == window.region && v.from <= window.to && window.from <= v.to)
                {
                    return Err(err(file, &path, "overlapping windows for the same region"));
                }
            }
            for (j, h) in p.hitboxes.iter().enumerate() {
                if let Some(g) = &h.geometry {
                    check_combat_geometry(file, &format!("moves.{slot}.phases[{i}].hitboxes[{j}].geometry"), g, f)?;
                }
            }
        }
    }

    for (slot, m) in f.moves.iter() {
        check_move(file, &format!("moves.{slot}"), m, f, &attach_ids, &e, &check_anim)?;
    }
    for (id, p) in &f.projectiles {
        let path = format!("projectiles.{id}");
        for (field, n) in [
            ("spawn[0]", p.spawn[0]),
            ("spawn[1]", p.spawn[1]),
            ("vel[0]", p.vel[0]),
            ("vel[1]", p.vel[1]),
            ("spin", p.spin),
        ] {
            finite(file, &format!("{path}.{field}"), n)?;
        }
        if let Some(n) = p.reflect_vy {
            finite(file, &format!("{path}.reflectVy"), n)?;
        }
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
        if let Some(c) = &p.charge_scale {
            nonnegative(file, &format!("{path}.chargeScale.speed"), c.speed)?;
            nonnegative(file, &format!("{path}.chargeScale.damage"), c.damage)?;
            for (name, n) in [("bkb", c.bkb), ("radius", c.radius), ("life", c.life)] {
                nonnegative(file, &format!("{path}.chargeScale.{name}"), n)?;
            }
            if let Some(angle) = c.angle_to {
                finite(file, &format!("{path}.chargeScale.angleTo"), angle)?;
            }
            if let Some(angle) = c.ground_angle_to {
                finite(file, &format!("{path}.chargeScale.groundAngleTo"), angle)?;
            }
        }
        if let Some(e) = &p.explosion {
            positive(file, &format!("{path}.explosion.radius"), e.radius)?;
            if let Some(r) = e.proximity {
                positive(file, &format!("{path}.explosion.proximity"), r)?;
            }
        }
        if let Some(b) = &p.bat {
            positive(file, &format!("{path}.bat.speed"), b.speed)?;
            finite(file, &format!("{path}.bat.vy"), b.vy)?;
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
        if let Some(a) = &p.article {
            if p.explosion.is_some() || p.bounce.is_some() || p.bat.is_some() || p.charge_scale.is_some() {
                return Err(err(
                    file,
                    format!("{path}.article"),
                    "article timeline cannot combine with explosion/bounce/bat/chargeScale",
                ));
            }
            if f.moves.get(&a.slot).is_none() {
                return Err(err(file, format!("{path}.article.slot"), "unknown move"));
            }
            if a.stages.is_empty() || a.stages[0].from != 0 || a.stages.windows(2).any(|s| s[0].from >= s[1].from) {
                return Err(err(file, format!("{path}.article.stages"), "needs increasing ages starting at zero"));
            }
            if a.auto_release < a.min_release || a.auto_release >= p.life {
                return Err(err(file, format!("{path}.article.autoRelease"), "needs minRelease <= autoRelease < life"));
            }
            nonnegative(file, &format!("{path}.article.steer"), a.steer)?;
            if let Some(r) = a.hurtbox {
                for n in [r.x1, r.x2, r.y1, r.y2] {
                    finite(file, &format!("{path}.article.hurtbox"), n)?;
                }
                if r.x1 >= r.x2 || r.y1 >= r.y2 {
                    return Err(err(file, format!("{path}.article.hurtbox"), "needs positive extents"));
                }
            }
            for (i, s) in a.stages.iter().enumerate() {
                let sp = format!("{path}.article.stages[{i}]");
                if s.startup >= s.frames {
                    return Err(err(file, &sp, "startup must precede frames"));
                }
                if s.auto_startup.is_some_and(|n| n > s.startup) {
                    return Err(err(file, &sp, "autoStartup must not exceed startup"));
                }
                nonnegative(file, &format!("{sp}.health"), s.health)?;
                if let Some(stroke) = &s.stroke {
                    positive(file, &format!("{sp}.stroke.width"), stroke.width)?;
                }
                for n in s.vel {
                    finite(file, &format!("{sp}.vel"), n)?;
                }
                for (j, b) in s.hitboxes.iter().enumerate() {
                    check_hitbox(file, &format!("{sp}.hitboxes[{j}]"), b, true)?;
                }
            }
        }
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
    if let Some(n) = m.charge_damage {
        nonnegative(file, &format!("{path}.chargeDamage"), n)?;
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
        if let Some(n) = p.armor_threshold {
            nonnegative(file, &format!("{pp}.armorThreshold"), n)?;
        }
        if let Some(n) = p.fall_cap {
            positive(file, &format!("{pp}.fallCap"), n)?;
        }
        if let Some(n) = p.travel_speed {
            positive(file, &format!("{pp}.travelSpeed"), n)?;
        }
        if let Some(r) = &p.reflector {
            if r.from > r.to {
                return Err(err(file, format!("{pp}.reflector"), "from is after to"));
            }
            for (name, n) in [("speed", r.speed), ("damage", r.damage), ("radius", r.radius)] {
                positive(file, &format!("{pp}.reflector.{name}"), n)?;
            }
            finite(file, &format!("{pp}.reflector.height"), r.height)?;
        }
        nonnegative(file, &format!("{pp}.airAccelMult"), p.air_accel_mult)?;
        nonnegative(file, &format!("{pp}.chargeFrames"), p.charge_frames)?;
        if p.charge_frames != 0. && p.frames.is_none() {
            return Err(e(format!("{pp}.chargeFrames"), "needs a finite phase length".into()));
        }
        for (name, r) in [("detection", p.detection), ("tether", p.tether)] {
            let Some(r) = r else { continue };
            for (field, n) in [("x1", r.x1), ("x2", r.x2), ("y1", r.y1), ("y2", r.y2)] {
                finite(file, &format!("{pp}.{name}.{field}"), n)?;
            }
            if r.x1 >= r.x2 || r.y1 >= r.y2 {
                return Err(e(format!("{pp}.{name}"), "needs x1 < x2 and y1 < y2".into()));
            }
        }
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
        if t.on == Trigger::TargetNear && p.detection.is_none() {
            return Err(e(format!("{tp}.on"), "targetNear needs a detection rectangle".into()));
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

fn check_rig_pose(file: &str, path: &str, f: &Fighter, pose: &super::visual::Pose) -> Result<(), ContentError> {
    let base = path.strip_suffix(".joints").unwrap_or(path);
    for (field, value) in [("rot", pose.rot), ("pivot", pose.pivot), ("offsetY", pose.offset_y)] {
        if let Some(n) = value {
            finite(file, &format!("{base}.{field}"), n)?;
        }
    }
    if pose.joints.is_empty() {
        return Ok(());
    }
    let Some(rig) = &f.rig else { return Err(err(file, path, "named joint overrides need a rig")) };
    crate::rig::resolve(rig, &pose.joints).map_err(|e| {
        let suffix = e.path.strip_prefix("pose.").unwrap_or(&e.path);
        err(file, format!("{path}.{suffix}"), e.message)
    })?;
    Ok(())
}

fn check_rig_shape(file: &str, path: &str, shape: &super::visual::Shape) -> Result<(), ContentError> {
    use super::visual::Shape;
    let point = |name: &str, p: [f64; 2]| -> Result<(), ContentError> {
        finite(file, &format!("{path}.{name}[0]"), p[0])?;
        finite(file, &format!("{path}.{name}[1]"), p[1])
    };
    let scalar = |name: &str, n| nonnegative(file, &format!("{path}.{name}"), n);
    let alpha = match shape {
        Shape::Line { from, to, width, alpha, .. } => {
            point("from", *from)?;
            point("to", *to)?;
            positive(file, &format!("{path}.width"), *width)?;
            alpha
        }
        Shape::Circle { at, r, width, alpha, .. } => {
            point("at", *at)?;
            positive(file, &format!("{path}.r"), *r)?;
            scalar("width", *width)?;
            alpha
        }
        Shape::Poly { points, width, alpha, .. } => {
            if points.len() < 3 {
                return Err(err(file, format!("{path}.points"), "needs at least three points"));
            }
            for (i, p) in points.iter().enumerate() {
                point(&format!("points[{i}]"), *p)?;
            }
            scalar("width", *width)?;
            alpha
        }
        Shape::Star { at, r, alpha, .. } => {
            point("at", *at)?;
            positive(file, &format!("{path}.r"), *r)?;
            alpha
        }
        Shape::Rect { x, y, w, h, radius, width, alpha, .. } => {
            finite(file, &format!("{path}.x"), *x)?;
            finite(file, &format!("{path}.y"), *y)?;
            positive(file, &format!("{path}.w"), *w)?;
            positive(file, &format!("{path}.h"), *h)?;
            scalar("radius", *radius)?;
            scalar("width", *width)?;
            alpha
        }
    };
    probability(file, &format!("{path}.alpha"), *alpha)
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
    if let Some(g) = &h.geometry {
        if !geometry || path.contains(".article.") || path.starts_with("knockdown") || path.starts_with("ledge") {
            return Err(err(
                file,
                format!("{path}.geometry"),
                "explicit geometry is supported only on fighter move attacks",
            ));
        }
        if !h.frames.is_empty() {
            return Err(err(file, format!("{path}.frames"), "cannot combine geometry and legacy tracks"));
        }
        positive(file, &format!("{path}.geometry.radius"), g.radius())?;
    }
    if let Some(angle) = h.ground_angle {
        finite(file, &format!("{path}.groundAngle"), angle)?;
    }
    for (name, n) in [("x", h.x), ("y", h.y), ("angle", h.angle), ("hitstunAdd", h.hitstun_add)] {
        finite(file, &format!("{path}.{name}"), n)?;
    }
    if geometry && h.geometry.is_none() {
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
        finite(file, &format!("{path}.frames[{i}][1]"), frame[1])?;
        finite(file, &format!("{path}.frames[{i}][2]"), frame[2])?;
        if geometry {
            positive(file, &format!("{path}.frames[{i}][3]"), frame[3])?;
        }
    }
    Ok(())
}
fn check_event(file: &str, path: &str, ev: &super::Event) -> Result<(), ContentError> {
    nonnegative(file, &format!("{path}.chargeVelocity"), ev.charge_velocity)?;
    for (name, vel) in [("mulVel", &ev.mul_vel), ("setVel", &ev.set_vel), ("addVel", &ev.add_vel)] {
        if let Some(vel) = vel {
            for (axis, n) in [("x", vel.x), ("y", vel.y)] {
                if let Some(n) = n {
                    finite(file, &format!("{path}.{name}.{axis}"), n)?;
                }
            }
        }
    }
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
/// Validates global content, including presentation tuning in embedded replay payloads.
pub fn check_global(file: &str, g: &Global) -> Result<(), ContentError> {
    check_audio(file, &g.audio)?;
    nonnegative(file, "gait.poseRunSpeed", g.gait.pose_run_speed)?;
    range(file, "gait.runThreshold", g.gait.run_threshold, 0., 1.)?;
    range(file, "gait.walkThreshold", g.gait.walk_threshold, 0., 1.)?;
    if g.gait.run_threshold > 0. && g.gait.walk_threshold > g.gait.run_threshold {
        return Err(err(file, "gait.walkThreshold", "must not exceed runThreshold"));
    }
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
    positive(file, "render.camera.minScaleStart", c.min_scale_start)?;
    if c.min_scale_start > 1. {
        return Err(err(file, "render.camera.minScaleStart", "must be ≤ 1"));
    }
    if !c.min_scale_floor.is_finite() || !(0. ..=1.).contains(&c.min_scale_floor) {
        return Err(err(file, "render.camera.minScaleFloor", "must be finite and between 0 and 1"));
    }
    positive(file, "knockback.chargeDamageDivisor", g.knockback.charge_damage_divisor)?;
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
    check_movement(file, &g.movement)?;
    check_knockback(file, &g.knockback)?;
    check_hitstun(file, &g.hitstun)?;
    check_hitlag(file, &g.hitlag)?;
    check_shield(file, &g.shield)?;
    check_dodge(file, &g.dodge)?;
    check_landing(file, &g.landing)?;
    check_grab(file, &g.grab)?;
    check_sim(file, &g.sim)?;
    check_hit_types(file, &g.hit_types)?;
    check_render(file, &g.render)?;
    nonnegative(file, "ledge.jumpSpeed", g.ledge.jump_speed)?;
    check_ai(file, &g.ai)?;
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
    Ok(())
}

fn check_render(file: &str, r: &super::render::Render) -> Result<(), ContentError> {
    if let Some(b) = &r.blast_zone {
        range(file, "render.blastZone.alpha", b.alpha, 0., 1.)?;
        positive(file, "render.blastZone.width", b.width)?;
        positive(file, "render.blastZone.dash", b.dash)?;
        nonnegative(file, "render.blastZone.gap", b.gap)?;
    }
    for (name, n) in [
        ("interpolate.fighterJump", r.interpolate.fighter_jump),
        ("interpolate.projectileJump", r.interpolate.projectile_jump),
        ("select.sizeSmall", r.select.size_small),
        ("select.sizeLarge", r.select.size_large),
    ] {
        positive(file, &format!("render.{name}"), n)?;
    }
    finite(file, "render.hud.look.tagHeight", r.hud.look.tag_height)?;
    finite(file, "render.hud.look.anchorHeight", r.hud.look.anchor_height)?;
    nonnegative(file, "render.hud.look.shakeSpeed", f64::from(r.hud.look.shake_speed))?;
    positive(file, "render.hud.look.bubbleShrink", f64::from(r.hud.look.bubble_shrink))?;
    range(file, "render.hud.look.bubbleMinScale", f64::from(r.hud.look.bubble_min_scale), 0., 1.)?;
    positive(file, "render.hud.look.popScale", f64::from(r.hud.look.pop_scale))?;
    positive(file, "render.hud.look.popSpeed", f64::from(r.hud.look.pop_speed))?;
    if r.select.size_small > r.select.size_large {
        return Err(err(file, "render.select", "needs sizeSmall ≤ sizeLarge"));
    }
    for (i, n) in r.select.thumb_crop.iter().enumerate() {
        positive(file, &format!("render.select.thumbCrop[{i}]"), *n)?;
    }
    let s = &r.select;
    for (name, n) in [
        ("flipSecs", f64::from(s.flip_secs)),
        ("bannerSecs", f64::from(s.banner_secs)),
        ("roulette.secs", f64::from(s.roulette.secs)),
        ("roulette.hops", f64::from(s.roulette.hops)),
        ("roulette.curve", f64::from(s.roulette.curve)),
        ("roulette.landScale", f64::from(s.roulette.land_scale)),
    ] {
        positive(file, &format!("render.select.{name}"), n)?;
    }
    for (name, n) in [
        ("dropScale", s.drop_scale),
        ("overshoot", s.overshoot),
        ("dropHeight", s.drop_height),
        ("bannerFrom", s.banner_from),
        ("roulette.pause", s.roulette.pause),
        ("roulette.landBounce", s.roulette.land_bounce),
        ("roulette.landWobble", s.roulette.land_wobble),
    ] {
        nonnegative(file, &format!("render.select.{name}"), f64::from(n))?;
    }
    check_pointer_look(file, &s.pointer)?;
    positive(file, "render.select.hints.fadeSecs", f64::from(s.hints.fade_secs))?;
    nonnegative(file, "render.select.hints.showSecs", f64::from(s.hints.show_secs))?;
    nonnegative(file, "render.select.hints.idleSecs", f64::from(s.hints.idle_secs))?;
    Ok(())
}

fn check_pointer_look(file: &str, p: &super::render::PointerLook) -> Result<(), ContentError> {
    for (name, n) in [
        ("speed", p.speed),
        ("curve", p.curve),
        ("stroke", p.stroke),
        ("scale", p.scale),
        ("motion.breathSecs", p.motion.breath_secs),
        ("motion.substeps", p.motion.substeps),
    ] {
        positive(file, &format!("render.select.pointer.{name}"), f64::from(n))?;
    }
    for (name, n) in [
        ("slideSecs", p.slide_secs),
        ("bounceSecs", p.bounce_secs),
        ("accel", p.accel),
        ("magnetRadius", p.magnet_radius),
        ("magnetStrength", p.magnet_strength),
        ("reach", p.reach),
        ("slideOvershoot", p.slide_overshoot),
        ("fan", p.fan),
        ("rest", p.rest),
        ("land", p.land),
        ("drop", p.drop),
        ("bounceWobble", p.bounce_wobble),
        ("motion.lean", p.motion.lean),
        ("motion.leanMax", p.motion.lean_max),
        ("motion.stretch", p.motion.stretch),
        ("motion.stretchMax", p.motion.stretch_max),
        ("motion.chipLag", p.motion.chip_lag),
        ("motion.chipSwing", p.motion.chip_swing),
        ("motion.stillSpeed", p.motion.still_speed),
        ("motion.breathAfter", p.motion.breath_after),
        ("motion.breathBob", p.motion.breath_bob),
        ("motion.breathScale", p.motion.breath_scale),
    ] {
        nonnegative(file, &format!("render.select.pointer.{name}"), f64::from(n))?;
    }
    range(file, "render.select.pointer.deadzone", f64::from(p.deadzone), 0., 1.)?;
    range(file, "render.select.pointer.magnetSpeed", f64::from(p.magnet_speed), 0., 1.)?;
    for (name, [stiffness, damping]) in [
        ("motion.leanSpring", p.motion.lean_spring),
        ("motion.stretchSpring", p.motion.stretch_spring),
        ("motion.chipSpring", p.motion.chip_spring),
    ] {
        positive(file, &format!("render.select.pointer.{name}"), f64::from(stiffness))?;
        nonnegative(file, &format!("render.select.pointer.{name}"), f64::from(damping))?;
    }
    Ok(())
}

fn check_audio(file: &str, a: &super::audio::Audio) -> Result<(), ContentError> {
    range(file, "audio.sampleRate", f64::from(a.sample_rate), 8000., 96000.)?;
    range(file, "audio.strengthLevels", a.strength_levels as f64, 1., 16.)?;
    range(file, "audio.maxVoices", a.max_voices as f64, 1., 128.)?;
    range(file, "audio.maxPerFrame", a.max_per_frame as f64, 1., 32.)?;
    range(file, "audio.pan", a.pan, 0., 4.)?;
    range(file, "audio.depth", a.depth, 0.1, 4.)?;
    range(file, "audio.earGap", a.ear_gap, 0.1, 4.)?;
    range(file, "audio.pitchVariation", a.pitch_variation, 0., 0.5)?;
    range(file, "audio.volumeStep", a.volume_step, 0.001, 1.)?;
    range(file, "audio.cooldown", a.cooldown, 0., 10.)?;
    positive(file, "audio.strengthMax", a.strength_max)?;
    for (n, v) in [("master", a.defaults.master), ("sfx", a.defaults.sfx), ("music", a.defaults.music)] {
        range(file, &format!("audio.defaults.{n}"), v, 0., 1.)?;
    }
    if a.sounds.len() > 128 {
        return Err(err(file, "audio.sounds", "at most 128 sounds"));
    }
    for (name, s) in &a.sounds {
        let p = format!("audio.sounds.{name}");
        range(file, &format!("{p}.volume"), s.volume, 0., 1.)?;
        range(file, &format!("{p}.cooldown"), s.cooldown, 0., 10.)?;
        range(file, &format!("{p}.strengthGain"), s.strength_gain, 0., 1.)?;
        range(file, &format!("{p}.strengthDuration"), s.strength_duration, 0.1, 4.)?;
        range(file, &format!("{p}.layers"), s.layers.len() as f64, 1., 8.)?;
        for (i, l) in s.layers.iter().enumerate() {
            let p = format!("{p}.layers[{i}]");
            for (n, v) in [("from", l.from), ("to", l.to)] {
                range(file, &format!("{p}.{n}"), v, 1., f64::from(a.sample_rate) / 2.)?;
            }
            range(file, &format!("{p}.duration"), l.duration, 0.001, 3.)?;
            range(file, &format!("{p}.attack"), l.attack, 0., l.duration)?;
            range(file, &format!("{p}.decay"), l.decay, 0., 30.)?;
            range(file, &format!("{p}.gain"), l.gain, 0., 1.)?;
        }
    }
    Ok(())
}

fn finite(file: &str, path: &str, n: f64) -> Result<(), ContentError> {
    if n.is_finite() { Ok(()) } else { Err(err(file, path, "must be finite")) }
}
fn range(file: &str, path: &str, n: f64, lo: f64, hi: f64) -> Result<(), ContentError> {
    if n.is_finite() && (lo..=hi).contains(&n) {
        Ok(())
    } else {
        Err(err(file, path, format!("must be finite and between {lo} and {hi}")))
    }
}
fn probability(file: &str, path: &str, n: f64) -> Result<(), ContentError> {
    range(file, path, n, 0., 1.)
}
fn nonnegative_range(file: &str, path: &str, a: f64, b: f64) -> Result<(), ContentError> {
    nonnegative(file, path, a)?;
    nonnegative(file, path, b)?;
    if a <= b { Ok(()) } else { Err(err(file, path, "needs 0 <= min <= max")) }
}

fn check_stats(file: &str, v: &super::Stats) -> Result<(), ContentError> {
    for (name, n) in [("walkSpeed", v.walk_speed), ("dashSpeed", v.dash_speed), ("pummelDamage", v.pummel_damage)] {
        if let Some(n) = n {
            positive(file, &format!("stats.{name}"), n)?;
        }
    }
    if v.dash_frames == Some(0) {
        return Err(err(file, "stats.dashFrames", "must be positive"));
    }
    for (name, n) in [
        ("runAccel", v.run_accel),
        ("groundFriction", v.ground_friction),
        ("airJumpDrift", v.air_jump_drift),
        ("airAccel", v.air_accel),
    ] {
        nonnegative(file, &format!("stats.{name}"), n)?;
    }
    for (name, n) in [
        ("weight", v.weight),
        ("runSpeed", v.run_speed),
        ("jumpSpeed", v.jump_speed),
        ("shortHopSpeed", v.short_hop_speed),
        ("airJumpSpeed", v.air_jump_speed),
        ("airSpeed", v.air_speed),
        ("gravity", v.gravity),
        ("fallSpeed", v.fall_speed),
        ("fastFallSpeed", v.fast_fall_speed),
    ] {
        positive(file, &format!("stats.{name}"), n)?;
    }
    Ok(())
}

fn check_movement(file: &str, v: &super::global::Movement) -> Result<(), ContentError> {
    for (name, n) in [
        ("airDrag", v.air_drag),
        ("airDragHeld", v.air_drag_held),
        ("fastFallKick", v.fast_fall_kick),
        ("attackAirAccel", v.attack_air_accel),
        ("aerialAccel", v.aerial_accel),
        ("helplessAccel", v.helpless_accel),
        ("upSpecialAccel", v.up_special_accel),
        ("attackFriction", v.attack_friction),
        ("attackSlideFriction", v.attack_slide_friction),
        ("attackAirDrag", v.attack_air_drag),
        ("pushRadius", v.push_radius),
        ("pushStrength", v.push_strength),
        ("dashAttackSpeed", v.dash_attack_speed),
        ("dashGrabSpeed", v.dash_grab_speed),
    ] {
        nonnegative(file, &format!("movement.{name}"), n)?;
    }
    finite(file, "movement.fastFallMinVy", v.fast_fall_min_vy)?;
    Ok(())
}

fn check_knockback(file: &str, v: &super::global::Knockback) -> Result<(), ContentError> {
    if let Some(k) = &v.ultimate {
        for (name, n) in [
            ("percentScale", k.percent_scale),
            ("damageScale", k.damage_scale),
            ("weightScale", k.weight_scale),
            ("base", k.base),
        ] {
            nonnegative(file, &format!("knockback.ultimate.{name}"), n)?;
        }
        positive(file, "knockback.ultimate.launchSpeed", k.launch_speed)?;
    }
    for (name, n) in [
        ("kbPercentScale", v.kb_percent_scale),
        ("tumbleThreshold", v.tumble_threshold),
        ("groundBounce", v.ground_bounce),
        ("chargeKbRatio", v.charge_kb_ratio),
    ] {
        nonnegative(file, &format!("knockback.{name}"), n)?;
    }
    positive(file, "knockback.chargeDamageDivisor", v.charge_damage_divisor)?;
    Ok(())
}

fn check_hitstun(file: &str, v: &super::global::Hitstun) -> Result<(), ContentError> {
    for (name, n) in [
        ("perKnockback", v.per_knockback),
        ("base", v.base),
        ("airDrag", v.air_drag),
        ("groundDrag", v.ground_drag),
        ("driftAccel", v.drift_accel),
    ] {
        nonnegative(file, &format!("hitstun.{name}"), n)?;
    }
    for (name, n) in [("gravity", v.gravity), ("maxFall", v.max_fall)] {
        positive(file, &format!("hitstun.{name}"), n)?;
    }
    Ok(())
}

fn check_hitlag(file: &str, v: &super::global::Hitlag) -> Result<(), ContentError> {
    for (name, n) in [("base", v.base), ("perDamage", v.per_damage), ("perKnockback", v.per_knockback), ("max", v.max)]
    {
        nonnegative(file, &format!("hitlag.{name}"), n)?;
    }
    Ok(())
}

fn check_shield(file: &str, v: &super::global::Shield) -> Result<(), ContentError> {
    for (name, n) in [
        ("decay", v.decay),
        ("regen", v.regen),
        ("damageMult", v.damage_mult),
        ("stunPerDamage", v.stun_per_damage),
        ("stunBase", v.stun_base),
        ("hitlagBase", v.hitlag_base),
        ("hitlagPerDamage", v.hitlag_per_damage),
        ("pushPerDamage", v.push_per_damage),
        ("pushBase", v.push_base),
        ("attackerPushback", v.attacker_pushback),
        ("breakLaunch", v.break_launch),
        ("dizzyBase", v.dizzy_base),
        ("dizzyMin", v.dizzy_min),
        ("dizzyPerPercent", v.dizzy_per_percent),
        ("dizzyMash", v.dizzy_mash),
        ("dizzyRecoverHP", v.dizzy_recover_hp),
    ] {
        nonnegative(file, &format!("shield.{name}"), n)?;
    }
    for (name, n) in [("max", v.max), ("breakGravity", v.break_gravity), ("breakMaxFall", v.break_max_fall)] {
        positive(file, &format!("shield.{name}"), n)?;
    }
    Ok(())
}

fn check_dodge(file: &str, v: &super::global::Dodge) -> Result<(), ContentError> {
    for (name, n) in [
        ("rollSpeed", v.roll_speed),
        ("dirAirDodgeSpeed", v.dir_air_dodge_speed),
        ("dirAirDodgeDecay", v.dir_air_dodge_decay),
    ] {
        nonnegative(file, &format!("dodge.{name}"), n)?;
    }
    Ok(())
}

fn check_landing(file: &str, v: &super::global::Landing) -> Result<(), ContentError> {
    nonnegative(file, "landing.softLandSpeed", v.soft_land_speed)
}

fn check_grab(file: &str, v: &super::global::Grab) -> Result<(), ContentError> {
    for (name, n) in [
        ("holdBase", v.hold_base),
        ("holdPerPercent", v.hold_per_percent),
        ("mashPerPress", v.mash_per_press),
        ("pummelDamage", v.pummel_damage),
        ("heightTolerance", v.height_tolerance),
    ] {
        nonnegative(file, &format!("grab.{name}"), n)?;
    }
    Ok(())
}

fn check_sim(file: &str, v: &super::global::SimRules) -> Result<(), ContentError> {
    for (name, n) in [
        ("jumpsquatFriction", v.jumpsquat_friction),
        ("landingFriction", v.landing_friction),
        ("shieldFriction", v.shield_friction),
        ("dodgeFriction", v.dodge_friction),
        ("shieldbreakFriction", v.shieldbreak_friction),
        ("knockdownFriction", v.knockdown_friction),
        ("pushHeight", v.push_height),
        ("mainPad", v.main_pad),
        ("platformPad", v.platform_pad),
        ("surfaceTolerance", v.surface_tolerance),
        ("techMinSpeed", v.tech_min_speed),
        ("hitstunParticleSpeed", v.hitstun_particle_speed),
        ("knockdownDrag", v.knockdown_drag),
        ("ledgeDropSpeed", v.ledge_drop_speed),
        ("ledgeDropOffset", v.ledge_drop_offset),
        ("ledgePullIn", v.ledge_pull_in),
        ("ledgeJumpOffset", v.ledge_jump_offset),
        ("ledgeJumpDrift", v.ledge_jump_drift),
        ("grabOffset", v.grab_offset),
        ("releaseVx", v.release_vx),
        ("releaseVy", v.release_vy),
        ("escapeAttackerVx", v.escape_attacker_vx),
        ("escapeVx", v.escape_vx),
        ("escapeVy", v.escape_vy),
        ("throwForward", v.throw_forward),
        ("throwBackHeight", v.throw_back_height),
        ("throwUpHeight", v.throw_up_height),
        ("throwDownOffset", v.throw_down_offset),
        ("platformDropOffset", v.platform_drop_offset),
        ("maxDamage", v.max_damage),
        ("projectilePad", v.projectile_pad),
        ("projectileWallTop", v.projectile_wall_top),
    ] {
        nonnegative(file, &format!("sim.{name}"), n)?;
    }
    Ok(())
}

fn check_hit_types(file: &str, v: &super::global::HitTypes) -> Result<(), ContentError> {
    if let Some(p) = &v.paralyze {
        check_life(file, "hitTypes.paralyze.maxFrames", p.max_frames)?;
        nonnegative(file, "hitTypes.paralyze.extraFrames", p.extra_frames)?;
    }
    if let Some(b) = &v.bury {
        for (name, n) in [
            ("baseFrames", b.base_frames),
            ("perPercent", b.per_percent),
            ("perKnockback", b.per_knockback),
            ("maxFrames", b.max_frames),
            ("mash", b.mash),
            ("buttonMash", b.button_mash),
            ("releaseThreshold", b.release_threshold),
        ] {
            nonnegative(file, &format!("hitTypes.bury.{name}"), n)?;
        }
        if b.max_frames < b.base_frames {
            return Err(err(file, "hitTypes.bury.maxFrames", "must be at least baseFrames"));
        }
    }
    positive(file, "hitTypes.windbox.maxSpeed", v.windbox.max_speed)?;
    for (path, n) in [
        ("hitTypes.trip.slide", v.trip.slide),
        ("hitTypes.autolink.pull", v.autolink.pull),
        ("hitTypes.autolink.momentum", v.autolink.momentum),
        ("hitTypes.windbox.push", v.windbox.push),
        ("hitTypes.electric.victimHitlagMult", v.electric.victim_hitlag_mult),
    ] {
        nonnegative(file, path, n)?;
    }
    for (name, n) in
        [("groundLow", v.sakurai.ground_low), ("groundHigh", v.sakurai.ground_high), ("air", v.sakurai.air)]
    {
        finite(file, &format!("hitTypes.sakurai.{name}"), n)?;
    }
    Ok(())
}

fn check_ai(file: &str, v: &super::global::AiRules) -> Result<(), ContentError> {
    nonnegative(file, "ai.initialCool", v.initial_cool)?;
    for (name, values) in [("react", v.react), ("charge", v.charge)] {
        for (i, n) in values.into_iter().enumerate() {
            nonnegative(file, &format!("ai.{name}[{i}]"), n)?;
        }
    }
    for (name, values) in [
        ("aggression", v.aggression),
        ("mash", v.mash),
        ("tech", v.tech),
        ("shieldResponse", v.shield_response),
        ("defend", v.defend),
        ("projectileDefend", v.projectile_defend),
    ] {
        for (i, n) in values.into_iter().enumerate() {
            probability(file, &format!("ai.{name}[{i}]"), n)?;
        }
    }
    check_ai_thresholds(file, &v.thresholds)
}

fn check_ai_thresholds(file: &str, v: &super::global::AiThresholds) -> Result<(), ContentError> {
    for (name, n) in [
        ("recoveryHeight", v.recovery_height),
        ("recoveryEdge", v.recovery_edge),
        ("knockdownWait", v.knockdown_wait),
        ("knockdownRandom", v.knockdown_random),
        ("ledgeWaitMin", v.ledge_wait_min),
        ("ledgeWaitMax", v.ledge_wait_max),
        ("holdMin", v.hold_min),
        ("holdMax", v.hold_max),
        ("pummelMax", v.pummel_max),
        ("throwKillPercent", v.throw_kill_percent),
        ("shieldClose", v.shield_close),
        ("jumpHold", v.jump_hold),
        ("shieldMin", v.shield_min),
        ("underPad", v.under_pad),
        ("underBottom", v.under_bottom),
        ("offstagePad", v.offstage_pad),
        ("recoverJumpHeight", v.recover_jump_height),
        ("recoverSpecialHeight", v.recover_special_height),
        ("recoverSpecialDistance", v.recover_special_distance),
        ("recoverSpecialDepth", v.recover_special_depth),
        ("centerDeadzone", v.center_deadzone),
        ("targetPad", v.target_pad),
        ("targetDeadzone", v.target_deadzone),
        ("defendDistance", v.defend_distance),
        ("defendHeight", v.defend_height),
        ("shieldHoldMin", v.shield_hold_min),
        ("shieldHoldMax", v.shield_hold_max),
        ("projectileDistance", v.projectile_distance),
        ("projectileHeight", v.projectile_height),
        ("projectileShieldFrames", v.projectile_shield_frames),
        ("jumpTargetHeight", v.jump_target_height),
        ("jumpTargetDistance", v.jump_target_distance),
        ("dropHeight", v.drop_height),
        ("dropDistance", v.drop_distance),
        ("attackHeight", v.attack_height),
        ("punishDizzyCharge", v.punish_dizzy_charge),
        ("punishCharge", v.punish_charge),
        ("upAttackHeight", v.up_attack_height),
        ("upAttackDistance", v.up_attack_distance),
        ("downAttackHeight", v.down_attack_height),
        ("downAttackDistance", v.down_attack_distance),
        ("projectileTargetHeight", v.projectile_target_height),
    ] {
        nonnegative(file, &format!("ai.thresholds.{name}"), n)?;
    }
    for (name, n) in [("recoverJumpVy", v.recover_jump_vy), ("recoverSpecialVy", v.recover_special_vy)] {
        finite(file, &format!("ai.thresholds.{name}"), n)?;
    }
    for (name, n) in [
        ("centerChance", v.center_chance),
        ("knockdownAttack", v.knockdown_attack),
        ("knockdownCenter", v.knockdown_center),
        ("ledgeJump", v.ledge_jump),
        ("ledgeAttack", v.ledge_attack),
        ("ledgeShield", v.ledge_shield),
        ("pummelChance", v.pummel_chance),
        ("throwDown", v.throw_down),
        ("throwUp", v.throw_up),
        ("throwForward", v.throw_forward),
        ("shieldGrab", v.shield_grab),
        ("shieldSpecial", v.shield_special),
        ("projectileShieldChance", v.projectile_shield_chance),
        ("jumpChance", v.jump_chance),
        ("dropChance", v.drop_chance),
        ("smashChance", v.smash_chance),
        ("smashForward", v.smash_forward),
        ("smashDown", v.smash_down),
        ("grabChance", v.grab_chance),
        ("tiltChance", v.tilt_chance),
        ("downTiltChance", v.down_tilt_chance),
        ("jabChance", v.jab_chance),
    ] {
        probability(file, &format!("ai.thresholds.{name}"), n)?;
    }
    for (name, a, b) in [
        ("ledgeWaitMax", v.ledge_wait_min, v.ledge_wait_max),
        ("holdMax", v.hold_min, v.hold_max),
        ("shieldHoldMax", v.shield_hold_min, v.shield_hold_max),
    ] {
        nonnegative_range(file, &format!("ai.thresholds.{name}"), a, b)?;
    }
    if v.knockdown_attack > v.knockdown_center {
        return Err(err(file, "ai.thresholds.knockdownCenter", "cumulative cutoff must not precede knockdownAttack"));
    }
    if v.ledge_jump > v.ledge_attack {
        return Err(err(file, "ai.thresholds.ledgeAttack", "cumulative cutoff must not precede ledgeJump"));
    }
    if v.ledge_attack > v.ledge_shield {
        return Err(err(file, "ai.thresholds.ledgeShield", "cumulative cutoff must not precede ledgeAttack"));
    }
    if v.throw_down > v.throw_up {
        return Err(err(file, "ai.thresholds.throwUp", "cumulative cutoff must not precede throwDown"));
    }
    if v.throw_up > v.throw_forward {
        return Err(err(file, "ai.thresholds.throwForward", "cumulative cutoff must not precede throwUp"));
    }
    if v.shield_grab > v.shield_special {
        return Err(err(file, "ai.thresholds.shieldSpecial", "cumulative cutoff must not precede shieldGrab"));
    }
    if v.smash_forward > v.smash_down {
        return Err(err(file, "ai.thresholds.smashDown", "cumulative cutoff must not precede smashForward"));
    }
    if v.smash_chance > v.grab_chance {
        return Err(err(file, "ai.thresholds.grabChance", "cumulative cutoff must not precede smashChance"));
    }
    if v.grab_chance > v.tilt_chance {
        return Err(err(file, "ai.thresholds.tiltChance", "cumulative cutoff must not precede grabChance"));
    }
    if v.tilt_chance > v.down_tilt_chance {
        return Err(err(file, "ai.thresholds.downTiltChance", "cumulative cutoff must not precede tiltChance"));
    }
    if v.down_tilt_chance > v.jab_chance {
        return Err(err(file, "ai.thresholds.jabChance", "cumulative cutoff must not precede downTiltChance"));
    }
    Ok(())
}

fn check_combat_geometry(
    file: &str,
    path: &str,
    g: &super::hitbox::CombatGeometry,
    f: &Fighter,
) -> Result<(), ContentError> {
    positive(file, &format!("{path}.radius"), g.radius())?;
    use super::hitbox::CombatGeometry;
    let points = match g {
        CombatGeometry::Circle { center, .. } => vec![("center", center)],
        CombatGeometry::Capsule { a, b, .. } => vec![("a", a), ("b", b)],
    };
    for (field, p) in points {
        for n in p.offset {
            finite(file, &format!("{path}.{field}.offset"), n)?;
        }
        if let Some(id) = &p.joint
            && !f.rig.as_ref().is_some_and(|r| r.joints.iter().any(|j| &j.id == id))
        {
            return Err(err(file, format!("{path}.{field}.joint"), "unknown joint (requires rig joint, not bone)"));
        }
        if let Some(id) = &p.joint {
            let rig = f.rig.as_ref().expect("checked joint binding");
            let mut poses: Vec<&super::visual::Pose> = f.poses.values().collect();
            let keys = f
                .anims
                .values()
                .flat_map(|a| &a.keys)
                .chain(f.moves.iter().flat_map(|(_, m)| m.phases.iter().flat_map(|p| &p.anim)));
            for key in keys {
                if let PoseRef::Inline(pose) = &key.pose {
                    poses.push(pose);
                }
            }
            for pose in poses {
                let joints = crate::rig::resolve(rig, &crate::pose::resolve_rig_pose(f, pose))
                    .map_err(|e| err(file, format!("{path}.{field}"), e.to_string()))?;
                let body = crate::pose::BodyTransform::new(&crate::pose::resolve(f, pose), [0., 0.], 1.);
                let point = body.local_to_world(joints.get(id).expect("checked joint binding").point(p.offset));
                for n in point {
                    finite(file, &format!("{path}.{field}.offset (resolved pose)"), n)?;
                }
            }
        }
    }
    Ok(())
}
