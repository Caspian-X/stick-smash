//! Every numeric content field carries an `x-unit` (and spatial units an `x-axis`) in the generated schemas, so
//! `tools/rescale.py` can rescale content by unit. See `sim::data::unit`.

use std::collections::BTreeSet;

use serde_json::{Map, Value};
use sim::data::{schema::schemas, unit};

struct Walk<'a> {
    defs: &'a Map<String, Value>,
    seen: BTreeSet<(String, bool)>,
    errors: Vec<String>,
    leaves: usize,
}

fn types(n: &Map<String, Value>) -> Vec<&str> {
    match n.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    }
}

fn check_tag(unit: &Value, axis: Option<&Value>) -> Result<(), String> {
    let units: Vec<&Value> = unit.as_array().map_or(vec![unit], |a| a.iter().collect());
    let axes: Vec<Option<&Value>> = match axis {
        Some(Value::Array(a)) => a.iter().map(Some).collect(),
        a => vec![a],
    };
    if units.len() > 1 && axes.len() > 1 && units.len() != axes.len() {
        return Err(format!("x-unit {unit} and x-axis {axis:?} lengths differ"));
    }
    for i in 0..units.len().max(axes.len()) {
        let u = units[i.min(units.len() - 1)].as_str().filter(|u| unit::UNITS.contains(u));
        let Some(u) = u else { return Err(format!("unknown x-unit {unit}")) };
        let a = axes[i.min(axes.len() - 1)].filter(|a| !a.is_null());
        match a.map(|a| a.as_str().filter(|a| unit::AXES.contains(a))) {
            Some(None) => return Err(format!("unknown x-axis {axis:?}")),
            None if unit::spatial(u) => return Err(format!("{u} needs an x-axis")),
            Some(Some(_)) if !unit::spatial(u) => return Err(format!("{u} takes no x-axis")),
            _ => {}
        }
    }
    Ok(())
}

impl Walk<'_> {
    /// `tag` is the outermost `x-unit` seen on the way down; it covers every leaf below it.
    fn node(&mut self, n: &Value, at: &str, tagged: bool) {
        let Some(n) = n.as_object() else { return };
        let mut tagged = tagged;
        if let Some(u) = n.get("x-unit") {
            if let Err(e) = check_tag(u, n.get("x-axis")) {
                self.errors.push(format!("{at}: {e}"));
            }
            tagged = true;
        } else if n.contains_key("x-axis") {
            self.errors.push(format!("{at}: x-axis without x-unit"));
        }
        if let Some(Value::String(r)) = n.get("$ref") {
            let name = r.trim_start_matches("#/$defs/");
            if self.seen.insert((name.to_string(), tagged)) {
                let def = self.defs.get(name).unwrap_or_else(|| panic!("{at}: missing {r}"));
                self.node(def, name.trim_end_matches("Partial"), tagged);
            }
        }
        if types(n).iter().any(|t| *t == "number" || *t == "integer") {
            self.leaves += 1;
            if !tagged {
                self.errors.push(format!("{at}: numeric field without x-unit"));
            }
        }
        if let Some(Value::Object(props)) = n.get("properties") {
            for (k, v) in props {
                self.node(v, &format!("{at}.{k}"), tagged);
            }
        }
        for key in ["items", "additionalProperties"] {
            if let Some(v) = n.get(key) {
                self.node(v, &format!("{at}[]"), tagged);
            }
        }
        for key in ["prefixItems", "oneOf", "anyOf", "allOf"] {
            if let Some(Value::Array(a)) = n.get(key) {
                for (i, v) in a.iter().enumerate() {
                    self.node(
                        v,
                        &format!("{at}{}", if key == "prefixItems" { format!("[{i}]") } else { String::new() }),
                        tagged,
                    );
                }
            }
        }
    }
}

/// Problems in one schema, and how many numeric leaves it has.
fn problems(schema: &Value, name: &str) -> (Vec<String>, usize) {
    let empty = Map::new();
    let defs = schema.get("$defs").and_then(Value::as_object).unwrap_or(&empty);
    let mut w = Walk { defs, seen: BTreeSet::new(), errors: vec![], leaves: 0 };
    w.node(schema, name, false);
    w.errors.sort();
    w.errors.dedup();
    (w.errors, w.leaves)
}

#[test]
fn every_numeric_field_has_a_unit() {
    let mut errors = vec![];
    for (name, schema) in schemas() {
        let (e, leaves) = problems(&schema, name.trim_end_matches(".schema.json"));
        assert!(leaves > 20, "{name}: walked only {leaves} numeric leaves");
        errors.extend(e.into_iter().map(|e| format!("{name}: {e}")));
    }
    assert!(
        errors.is_empty(),
        "untagged or badly tagged numeric fields (see sim::data::unit):\n  {}",
        errors.join("\n  ")
    );
}

#[test]
fn walker_finds_untagged_leaves_through_refs_and_overrides() {
    let schema = serde_json::json!({
        "properties": {
            "a": { "type": "number" },
            "b": { "anyOf": [{ "$ref": "#/$defs/V" }, { "type": "null" }] },
            "c": { "$ref": "#/$defs/V", "x-unit": "ratio" },
            "d": { "type": "array", "items": { "type": "array", "prefixItems": [{ "type": "number" }, { "type": "string" }] } },
            "e": { "type": "number", "x-unit": "px" }
        },
        "$defs": { "VPartial": { "properties": {
            "x": { "type": "number", "x-unit": "px/tick", "x-axis": "x" },
            "y": { "type": ["number", "null"] }
        } }, "V": { "properties": { "y": { "type": "integer" } } } }
    });
    let mut schema = schema;
    schema["properties"]["b"]["anyOf"][0]["$ref"] = "#/$defs/VPartial".into();
    let (errors, leaves) = problems(&schema, "t");
    assert_eq!(leaves, 6);
    assert_eq!(
        errors,
        [
            "V.y: numeric field without x-unit",
            "t.a: numeric field without x-unit",
            "t.d[][0]: numeric field without x-unit",
            "t.e: px needs an x-axis"
        ]
    );
}

#[test]
fn tag_checks_catch_mistakes() {
    use serde_json::json;
    assert!(check_tag(&json!("px"), Some(&json!("x"))).is_ok());
    assert!(check_tag(&json!(["tick", "px"]), Some(&json!([null, "y"]))).is_ok());
    assert!(check_tag(&json!("px"), None).is_err());
    assert!(check_tag(&json!("tick"), Some(&json!("x"))).is_err());
    assert!(check_tag(&json!("furlong"), None).is_err());
    assert!(check_tag(&json!("px"), Some(&json!("z"))).is_err());
    assert!(check_tag(&json!(["tick", "px", "px"]), Some(&json!([null, "x"]))).is_err());
}
