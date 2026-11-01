//! Schema-driven forms for data without a hand-made inspector (projectiles, attachments, effects, style, AI,
//! animations, a phase's remaining fields; stage art and background). Field help, enum choices, optional fields and
//! new-item templates come from the JSON schemas; values round-trip through serde, so an edit that doesn't parse
//! stays a draft (shown in red) instead of reaching the content.

use std::sync::LazyLock;

use bevy_egui::egui::{self, DragValue, Id, RichText};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::style::{self, DIM, FAINT, GOLD, RED, STAR};

/// The fighter schema, with the stage schema's definitions added as `stage.<Name>` (and the stage root as
/// `stage.Stage`), so one root resolves every `$ref`.
static SCHEMA: LazyLock<Value> = LazyLock::new(|| {
    fn rewrite(v: &mut Value) {
        match v {
            Value::Object(m) => {
                if let Some(Value::String(r)) = m.get_mut("$ref")
                    && let Some(n) = r.strip_prefix("#/$defs/")
                {
                    *r = format!("#/$defs/stage.{n}");
                }
                m.values_mut().for_each(rewrite);
            }
            Value::Array(a) => a.iter_mut().for_each(rewrite),
            _ => {}
        }
    }
    let mut f: Value = serde_json::from_str(include_str!("../../../assets/schema/fighter.schema.json"))
        .expect("fighter schema parses");
    let mut st: Value =
        serde_json::from_str(include_str!("../../../assets/schema/stage.schema.json")).expect("stage schema parses");
    rewrite(&mut st);
    let st_defs = st.as_object_mut().and_then(|m| m.shift_remove("$defs")).unwrap_or_default();
    let defs = f.get_mut("$defs").and_then(Value::as_object_mut).expect("fighter schema has $defs");
    for (k, v) in st_defs.as_object().into_iter().flatten() {
        defs.insert(format!("stage.{k}"), v.clone());
    }
    defs.insert("stage.Stage".into(), st);
    f
});
static NULL: Value = Value::Null;

/// A definition by name (`Fighter` is the root).
pub fn def(name: &str) -> &'static Value {
    if name == "Fighter" { &SCHEMA } else { SCHEMA.pointer(&format!("/$defs/{name}")).unwrap_or(&NULL) }
}

/// `{ "type": "array", "items": <def> }`.
pub fn array_of(name: &str) -> Value {
    serde_json::json!({ "type": "array", "items": { "$ref": format!("#/$defs/{name}") } })
}

fn deref(s: &Value) -> &Value {
    let mut s = s;
    for _ in 0..8 {
        match s.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix('#')) {
            Some(p) => s = SCHEMA.pointer(p).unwrap_or(&NULL),
            None => break,
        }
    }
    s
}

/// A property's own schema (with its description) and the dereferenced one.
fn prop<'a>(s: &'a Value, key: &str) -> Option<&'a Value> {
    s.get("properties")?.get(key)
}

/// Help text for `key` of definition `def`.
pub fn doc(def_name: &str, key: &str) -> String {
    let s = deref(def(def_name));
    prop(s, key).map(describe).unwrap_or_default()
}

pub(crate) fn describe(s: &Value) -> String {
    let d = s.get("description").or_else(|| deref(s).get("description"));
    d.and_then(Value::as_str).unwrap_or_default().replace('`', "")
}

fn types(s: &Value) -> Vec<&str> {
    match s.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    }
}

fn alternatives(s: &Value) -> Option<&Vec<Value>> {
    s.get("anyOf").or_else(|| s.get("oneOf")).and_then(Value::as_array)
}

/// Strings with fixed choices: `enum`, or `oneOf` of `const`s. Value and help.
fn choices(s: &Value) -> Option<Vec<(String, String)>> {
    let s = deref(s);
    if let Some(e) = s.get("enum").and_then(Value::as_array) {
        return Some(e.iter().filter_map(|v| Some((v.as_str()?.to_string(), String::new()))).collect());
    }
    let alts = alternatives(s)?;
    alts.iter().map(|a| Some((a.get("const")?.as_str()?.to_string(), describe(a)))).collect()
}

/// The tag key of an internally tagged enum (`type`, `shape`), if `s` is one.
fn tag(s: &Value) -> Option<String> {
    let first = alternatives(s)?.first()?;
    first.get("properties")?.as_object()?.iter().find(|(_, p)| p.get("const").is_some()).map(|(k, _)| k.clone())
}

/// The schema that applies to `v` among `anyOf` / `oneOf` alternatives (skipping `null`).
fn resolve<'a>(s: &'a Value, v: &Value) -> &'a Value {
    let s = deref(s);
    let Some(alts) = alternatives(s) else { return s };
    if choices(s).is_some() {
        return s;
    }
    if let Some(t) = tag(s) {
        let want = v.get(&t);
        return alts.iter().find(|a| prop(a, &t).and_then(|p| p.get("const")) == want).unwrap_or(&alts[0]);
    }
    let kind = |a: &Value| -> bool {
        let a = deref(a);
        let ts = types(a);
        match v {
            Value::Null => ts.contains(&"null"),
            Value::Bool(_) => ts.contains(&"boolean"),
            Value::Number(_) => ts.contains(&"number") || ts.contains(&"integer"),
            Value::String(_) => ts.contains(&"string") || a.get("enum").is_some() || alternatives(a).is_some(),
            Value::Array(_) => ts.contains(&"array"),
            Value::Object(_) => ts.contains(&"object") || alternatives(a).is_some(),
        }
    };
    alts.iter().find(|a| kind(a)).or_else(|| alts.iter().find(|a| !types(a).contains(&"null"))).map_or(s, deref)
}

fn nullable(s: &Value) -> bool {
    types(s).contains(&"null") || alternatives(s).is_some_and(|a| a.iter().any(|a| types(a) == ["null"]))
}

/// A value that satisfies `s`, for new fields and items.
pub fn template(s: &Value) -> Value {
    let s = deref(s);
    if let Some(c) = choices(s) {
        return Value::String(c.first().map(|c| c.0.clone()).unwrap_or_default());
    }
    if let Some(alts) = alternatives(s) {
        let a = alts.iter().find(|a| !types(a).contains(&"null") || types(a).len() > 1).unwrap_or(&alts[0]);
        return template(a);
    }
    if s.get("pattern").and_then(Value::as_str).is_some_and(|p| p.contains("#[0-9a-fA-F]")) {
        return Value::String("#ffffff".into());
    }
    let ts = types(s);
    match ts.iter().find(|t| **t != "null").copied().unwrap_or("object") {
        "number" | "integer" => {
            s.get("minimum").cloned().filter(|m| m.as_f64().is_some_and(|m| m > 0.)).unwrap_or(0.into())
        }
        "boolean" => false.into(),
        "string" => s.get("const").cloned().unwrap_or_else(|| "".into()),
        "array" => {
            let n = s.get("minItems").and_then(Value::as_u64).unwrap_or(0) as usize;
            Value::Array((0..n).map(|_| s.get("items").map_or(0.into(), template)).collect())
        }
        _ => {
            let mut m = Map::new();
            for r in s.get("required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                m.insert(r.into(), prop(s, r).map_or(Value::Null, template));
            }
            Value::Object(m)
        }
    }
}

/// Edits `v` through its JSON form against schema `s`. Returns whether `v` changed. Unparseable edits are kept as
/// a draft until they parse or `v` changes elsewhere (undo).
pub fn edit<T: Serialize + DeserializeOwned>(ui: &mut egui::Ui, id: Id, s: &Value, v: &mut T) -> bool {
    let cur = serde_json::to_value(&*v).expect("content serializes");
    let draft_id = id.with("draft");
    let mut work = match ui.data(|d| d.get_temp::<(Value, Value, String)>(draft_id)) {
        Some((base, draft, _)) if base == cur => draft,
        _ => cur.clone(),
    };
    let before = work.clone();
    value(ui, id, s, &mut work, 0);
    if work == before {
        if let Some((_, _, err)) =
            ui.data(|d| d.get_temp::<(Value, Value, String)>(draft_id)).filter(|(b, ..)| *b == cur)
        {
            ui.label(RichText::new(format!("Not applied: {err}")).color(RED).small());
        }
        return false;
    }
    match serde_json::from_value::<T>(work.clone()) {
        Ok(n) => {
            ui.data_mut(|d| d.remove::<(Value, Value, String)>(draft_id));
            *v = n;
            true
        }
        Err(e) => {
            ui.data_mut(|d| d.insert_temp(draft_id, (cur, work, e.to_string())));
            false
        }
    }
}

const NUMERIC_TUPLE: usize = 4;

fn value(ui: &mut egui::Ui, id: Id, s: &Value, v: &mut Value, depth: usize) {
    let r = resolve(s, v);
    if let Some(c) = choices(r) {
        let cur = v.as_str().unwrap_or("").to_string();
        egui::ComboBox::from_id_salt(id).selected_text(&cur).width(120.).show_ui(ui, |ui| {
            for (o, help) in c {
                let resp = ui.selectable_label(cur == o, &o);
                let resp = if help.is_empty() { resp } else { resp.on_hover_text(help) };
                if resp.clicked() {
                    *v = Value::String(o);
                }
            }
        });
        return;
    }
    match v {
        Value::Null => {
            ui.label(RichText::new("none").color(FAINT));
        }
        Value::Bool(b) => {
            ui.checkbox(b, "");
        }
        Value::Number(n) => {
            let int = types(r).contains(&"integer");
            let mut x = n.as_f64().unwrap_or(0.);
            let min = r.get("minimum").and_then(Value::as_f64).unwrap_or(f64::NEG_INFINITY);
            let dv = DragValue::new(&mut x).range(min..=f64::INFINITY).speed(if int { 0.2 } else { 0.1 });
            if ui.add(if int { dv.max_decimals(0) } else { dv.max_decimals(3) }).changed() {
                *v = if int {
                    Value::from(x.round() as i64)
                } else {
                    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
                };
            }
        }
        Value::String(t) => {
            let color = r.get("pattern").and_then(Value::as_str).is_some_and(|p| p.contains("#[0-9a-fA-F]"));
            ui.horizontal(|ui| {
                if color
                    && let Some(mut c) = hex(t)
                    && ui.color_edit_button_srgb(&mut c).changed()
                {
                    *t = format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
                }
                ui.add(egui::TextEdit::singleline(t).desired_width(if color { 90. } else { 150. }));
            });
        }
        Value::Array(a) => array(ui, id, r, a, depth),
        Value::Object(m) => object(ui, id, r, m, depth),
    }
}

/// A collapsible entry with buttons on its header line.
pub fn collapsing<'u>(
    ui: &'u mut egui::Ui,
    id: Id,
    title: &str,
    buttons: impl FnOnce(&mut egui::Ui),
) -> egui::collapsing_header::HeaderResponse<'u, ()> {
    let state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id.with("open"), false);
    state.show_header(ui, |ui| {
        ui.label(RichText::new(title).color(STAR));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), buttons);
    })
}

fn hex(t: &str) -> Option<[u8; 3]> {
    let h = t.strip_prefix('#').filter(|h| h.len() == 6)?;
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([b(0)?, b(2)?, b(4)?])
}

fn array(ui: &mut egui::Ui, id: Id, s: &Value, a: &mut Vec<Value>, depth: usize) {
    let items = s.get("items").unwrap_or(&NULL);
    let fixed = s.get("minItems").is_some() && s.get("minItems") == s.get("maxItems");
    // Short tuples (points, ranges, `[frame, value]` keys, `[y, color]` stops) edit inline.
    if fixed && a.len() <= NUMERIC_TUPLE && a.iter().all(|x| x.is_number() || x.is_string()) {
        let prefix = s.get("prefixItems").and_then(Value::as_array);
        ui.horizontal(|ui| {
            for (i, x) in a.iter_mut().enumerate() {
                value(ui, id.with(i), prefix.and_then(|p| p.get(i)).unwrap_or(items), x, depth + 1);
            }
        });
        return;
    }
    let mut op: Option<(usize, i8)> = None;
    ui.vertical(|ui| {
        for (i, x) in a.iter_mut().enumerate() {
            let simple = !matches!(x, Value::Object(_)) && !matches!(x, Value::Array(v) if v.len() > NUMERIC_TUPLE);
            let row = |ui: &mut egui::Ui, op: &mut Option<(usize, i8)>| {
                if ui.small_button("✖").on_hover_text("Remove").clicked() {
                    *op = Some((i, 0));
                }
                if i > 0 && ui.small_button(style::ICON_UP).on_hover_text("Move up").clicked() {
                    *op = Some((i, -1));
                }
            };
            if simple {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{i}")).color(FAINT));
                    value(ui, id.with(i), items, x, depth + 1);
                    row(ui, &mut op);
                });
            } else {
                let title = summary(x, resolve(items, x));
                collapsing(ui, id.with(i), &format!("{i}  {title}"), |ui| {
                    row(ui, &mut op);
                    if ui.small_button(style::ICON_DUP).on_hover_text("Duplicate").clicked() {
                        op = Some((i, 2));
                    }
                })
                .body(|ui| value(ui, id.with(i), items, x, depth + 1));
            }
        }
        if !fixed {
            add_item(ui, id, items, a);
        }
    });
    match op {
        Some((i, 0)) => drop(a.remove(i)),
        Some((i, 2)) => a.insert(i + 1, a[i].clone()),
        Some((i, _)) => a.swap(i, i - 1),
        None => {}
    }
}

/// "+ Add" for an array: a template per tagged variant, or one template.
fn add_item(ui: &mut egui::Ui, id: Id, items: &Value, a: &mut Vec<Value>) {
    let it = deref(items);
    match (tag(it), alternatives(it)) {
        (Some(t), Some(alts)) => {
            egui::ComboBox::from_id_salt(id.with("add")).selected_text("+ Add…").width(90.).show_ui(ui, |ui| {
                for alt in alts {
                    let name = prop(alt, &t).and_then(|p| p.get("const")).and_then(Value::as_str).unwrap_or("?");
                    if ui.selectable_label(false, name).on_hover_text(describe(alt)).clicked() {
                        a.push(template(alt));
                    }
                }
            });
        }
        _ => {
            if ui.small_button("+ Add").clicked() {
                a.push(a.last().cloned().unwrap_or_else(|| template(items)));
            }
        }
    }
}

/// One-line label for a collapsed item: its tag, id or name, and a few scalar fields.
fn summary(v: &Value, s: &Value) -> String {
    let Value::Object(m) = v else { return String::new() };
    let t =
        tag(s).or_else(|| ["type", "shape", "id", "name"].iter().find(|k| m.contains_key(**k)).map(|k| k.to_string()));
    let head = t.and_then(|t| m.get(&t)).and_then(Value::as_str).unwrap_or("").to_string();
    let rest: Vec<String> = m
        .iter()
        .filter(|(_, x)| x.is_number())
        .take(3)
        .map(|(k, x)| format!("{k} {}", x.as_f64().map_or(String::new(), |f| format!("{f}"))))
        .collect();
    format!("{head}  {}", rest.join(" · ")).trim().to_string()
}

fn object(ui: &mut egui::Ui, id: Id, s: &Value, m: &mut Map<String, Value>, depth: usize) {
    let tag = m.keys().find(|k| prop(s, k).is_some_and(|p| p.get("const").is_some())).cloned();
    let required: Vec<&str> =
        s.get("required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    let mut remove = None;
    egui::Grid::new(id.with("grid")).num_columns(2).spacing([10., 5.]).min_col_width(92.).show(ui, |ui| {
        let keys: Vec<String> = m.keys().cloned().collect();
        for k in keys {
            if Some(&k) == tag.as_ref() {
                continue;
            }
            let ps = prop(s, &k).unwrap_or(&NULL);
            let help = describe(ps);
            let lbl = ui.label(RichText::new(&k).color(DIM));
            let lbl = if help.is_empty() { lbl } else { lbl.on_hover_text(&help) };
            let optional = !required.contains(&k.as_str());
            lbl.context_menu(|ui| {
                if optional && ui.button("Remove field").clicked() {
                    remove = Some(k.clone());
                }
            });
            ui.horizontal(|ui| {
                let x = m.get_mut(&k).expect("key");
                let nested = matches!(x, Value::Object(_))
                    || matches!(x, Value::Array(a) if !a.iter().all(|x| x.is_number() || x.is_string()) || a.len() > NUMERIC_TUPLE);
                if nested && depth > 0 {
                    egui::CollapsingHeader::new(RichText::new(summary(x, resolve(ps, x))).color(FAINT))
                        .id_salt(id.with(&k))
                        .show(ui, |ui| value(ui, id.with(&k), ps, x, depth + 1));
                } else {
                    value(ui, id.with(&k), ps, x, depth + 1);
                }
                if optional && ui.small_button("✖").on_hover_text("Remove (use the default)").clicked() {
                    remove = Some(k.clone());
                }
            });
            ui.end_row();
        }
    });
    if let Some(k) = remove {
        m.shift_remove(&k);
    }
    let missing: Vec<(String, &Value)> = s
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(k, _)| !m.contains_key(*k))
        .map(|(k, p)| (k.clone(), p))
        .collect();
    if !missing.is_empty() {
        egui::ComboBox::from_id_salt(id.with("add-field"))
            .selected_text(RichText::new("+ Field…").color(GOLD))
            .width(110.)
            .show_ui(ui, |ui| {
                for (k, p) in missing {
                    if ui.selectable_label(false, &k).on_hover_text(describe(p)).clicked() {
                        let t = if nullable(p) {
                            template(p)
                        } else {
                            p.get("default").cloned().unwrap_or_else(|| template(p))
                        };
                        m.insert(k, t);
                    }
                }
            });
    }
}

/// A `name → item` map (projectiles, state animations, attachment animations): one collapsible entry per key, with
/// add, rename, duplicate and delete. Returns whether anything changed.
pub fn map<T: Serialize + DeserializeOwned + Clone>(
    ui: &mut egui::Ui,
    id: Id,
    item: &Value,
    m: &mut std::collections::BTreeMap<String, T>,
    new_item: impl Fn() -> Option<T>,
) -> bool {
    let mut changed = false;
    let mut op: Option<(String, &str)> = None;
    let mut renames: Vec<(String, String)> = vec![];
    let taken: Vec<String> = m.keys().cloned().collect();
    let keys_has = |n: &str| taken.iter().any(|k| k == n);
    let keys: Vec<String> = m.keys().cloned().collect();
    for k in keys {
        let v = m.get_mut(&k).expect("key");
        collapsing(ui, id.with(&k), &k, |ui| {
            if ui.small_button("✖").on_hover_text("Delete").clicked() {
                op = Some((k.clone(), "del"));
            }
            if ui.small_button(style::ICON_DUP).on_hover_text("Duplicate").clicked() {
                op = Some((k.clone(), "dup"));
            }
        })
        .body(|ui| {
            let rid = id.with(&k).with("rename");
            let mut name = ui.data(|d| d.get_temp::<String>(rid)).unwrap_or_else(|| k.clone());
            ui.horizontal(|ui| {
                ui.label(RichText::new("name").color(DIM));
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.));
                if r.lost_focus() && name != k && !name.is_empty() && !keys_has(&name) {
                    op = Some((name.clone(), "ren"));
                }
            });
            ui.data_mut(|d| d.insert_temp(rid, name));
            changed |= edit(ui, id.with(&k), item, v);
        });
        if let Some((n, "ren")) = &op {
            renames.push((k.clone(), n.clone()));
            op = None;
        }
    }
    for (old, new) in renames {
        if !m.contains_key(&new)
            && let Some(v) = m.remove(&old)
        {
            m.insert(new, v);
            changed = true;
        }
    }
    let name_id = id.with("new-name");
    let mut name = ui.data(|d| d.get_temp::<String>(name_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut name).hint_text("new name").desired_width(120.));
        let free = !name.is_empty() && !m.contains_key(&name);
        if ui
            .add_enabled(free, egui::Button::new("+ Add"))
            .on_hover_text("Starts as a copy of the first entry")
            .clicked()
            && let Some(v) = m.values().next().cloned().or_else(&new_item)
        {
            m.insert(std::mem::take(&mut name), v);
            changed = true;
        }
    });
    ui.data_mut(|d| d.insert_temp(name_id, name));
    match op {
        Some((k, "dup")) => {
            let n = crate::doc::unique_name(&format!("{k}_copy"), |n| m.contains_key(n));
            let v = m[&k].clone();
            m.insert(n, v);
            changed = true;
        }
        Some((k, _)) => {
            m.remove(&k);
            changed = true;
        }
        None => {}
    }
    changed
}
