//! JSON Schemas for content files, generated from the Rust types (doc comments become hover docs).

use schemars::{JsonSchema, generate::SchemaSettings};
use serde_json::{Map, Value};

use super::{Fighter, Global, Stage};

/// `(file name, schema)` for every content type, as saved in `assets/schema/`.
pub fn schemas() -> Vec<(&'static str, Value)> {
    let mut fighter = gen_schema::<Fighter>();
    partial_root(&mut fighter);
    vec![("global.schema.json", gen_schema::<Global>()), ("stage.schema.json", gen_schema::<Stage>()), ("fighter.schema.json", fighter)]
}

fn gen_schema<T: JsonSchema>() -> Value {
    SchemaSettings::draft2020_12().into_generator().into_root_schema_for::<T>().to_value()
}

/// A fighter file with `extends` can inherit any object field, so required fields are dropped along every path
/// the merge reaches (objects, not array items). Those definitions get a `Partial` copy; array items keep the
/// full definition.
fn partial_root(root: &mut Value) {
    let Some(defs) = root.get("$defs").and_then(Value::as_object).cloned() else {
        return;
    };
    let mut out = defs.clone();
    let mut node = root.clone();
    if let Some(m) = node.as_object_mut() {
        m.remove("$defs");
    }
    partialize(&mut node, &defs, &mut out);
    if let Some(m) = node.as_object_mut() {
        m.insert("$defs".into(), Value::Object(out));
    }
    *root = node;
}

fn partialize(node: &mut Value, defs: &Map<String, Value>, out: &mut Map<String, Value>) {
    let Some(m) = node.as_object_mut() else {
        return;
    };
    m.remove("required");
    if let Some(Value::String(r)) = m.get("$ref")
        && let Some(name) = r.strip_prefix("#/$defs/")
    {
        let partial = format!("{name}Partial");
        if !out.contains_key(&partial)
            && let Some(def) = defs.get(name)
        {
            out.insert(partial.clone(), Value::Null);
            let mut d = def.clone();
            partialize(&mut d, defs, out);
            out.insert(partial.clone(), d);
        }
        m.insert("$ref".into(), Value::String(format!("#/$defs/{partial}")));
    }
    for key in ["properties"] {
        if let Some(Value::Object(props)) = m.get_mut(key) {
            for v in props.values_mut() {
                partialize(v, defs, out);
            }
        }
    }
    if let Some(v) = m.get_mut("additionalProperties") {
        partialize(v, defs, out);
    }
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(Value::Array(a)) = m.get_mut(key) {
            for v in a {
                partialize(v, defs, out);
            }
        }
    }
}
