//! JSON helpers: package merging, the compact pretty-printer used for every content file, and a stable hash.

use serde_json::Value;

/// Merge `child` over `parent`: objects key by key (recursively), everything else (arrays included) replaced
/// whole, `null` removes the key.
pub fn merge(parent: &mut Value, child: &Value) {
    match (parent, child) {
        (Value::Object(p), Value::Object(c)) => {
            for (k, v) in c {
                if v.is_null() {
                    p.shift_remove(k);
                } else if let Some(pv) = p.get_mut(k) {
                    merge(pv, v);
                } else {
                    p.insert(k.clone(), strip_nulls(v));
                }
            }
        }
        (p, c) => *p = strip_nulls(c),
    }
}

fn strip_nulls(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.clone(), strip_nulls(v))).collect()),
        v => v.clone(),
    }
}

const WIDTH: usize = 120;
const INDENT: &str = "  ";

/// Pretty-print with small containers on one line: an object or array stays inline when it fits in the line
/// width and holds no objects (a hitbox, a keyframe, a point list), or when it's very short.
pub fn to_compact_string(v: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, v, 0, 0);
    out.push('\n');
    out
}

fn write_value(out: &mut String, v: &Value, depth: usize, prefix: usize) {
    match v {
        Value::Object(m) if !m.is_empty() => {
            let line = inline(v);
            if fits(v, &line, depth, prefix) {
                out.push_str(&line);
                return;
            }
            out.push('{');
            for (i, (k, val)) in m.iter().enumerate() {
                out.push('\n');
                push_indent(out, depth + 1);
                let key = Value::String(k.clone()).to_string();
                out.push_str(&key);
                out.push_str(": ");
                write_value(out, val, depth + 1, key.len() + 2);
                if i + 1 < m.len() {
                    out.push(',');
                }
            }
            out.push('\n');
            push_indent(out, depth);
            out.push('}');
        }
        Value::Array(a) if !a.is_empty() => {
            let line = inline(v);
            if fits(v, &line, depth, prefix) {
                out.push_str(&line);
                return;
            }
            out.push('[');
            for (i, val) in a.iter().enumerate() {
                out.push('\n');
                push_indent(out, depth + 1);
                write_value(out, val, depth + 1, 0);
                if i + 1 < a.len() {
                    out.push(',');
                }
            }
            out.push('\n');
            push_indent(out, depth);
            out.push(']');
        }
        v => out.push_str(&inline(v)),
    }
}

fn fits(v: &Value, line: &str, depth: usize, prefix: usize) -> bool {
    let len = depth * INDENT.len() + prefix + line.len() + 1;
    len <= 40 || (len <= WIDTH && !has_nested_object(v))
}

fn has_nested_object(v: &Value) -> bool {
    let kids: Box<dyn Iterator<Item = &Value>> = match v {
        Value::Object(m) => Box::new(m.values()),
        Value::Array(a) => Box::new(a.iter()),
        _ => return false,
    };
    kids.into_iter().any(|k| k.is_object() || has_nested_object(k))
}

/// One-line form with a space after `:` and `,`.
fn inline(v: &Value) -> String {
    match v {
        Value::Object(m) if m.is_empty() => "{}".into(),
        Value::Object(m) => {
            let items: Vec<String> = m.iter().map(|(k, v)| format!("{}: {}", Value::String(k.clone()), inline(v))).collect();
            format!("{{ {} }}", items.join(", "))
        }
        Value::Array(a) => format!("[{}]", a.iter().map(inline).collect::<Vec<_>>().join(", ")),
        v => v.to_string(),
    }
}

fn push_indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str(INDENT);
    }
}

/// 64-bit FNV-1a of `v`'s canonical text (keys sorted). Same data in any key order hashes the same.
pub fn content_hash(v: &Value) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |s: &str| {
        for b in s.bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    canonical(v, &mut feed);
    h
}

fn canonical(v: &Value, feed: &mut impl FnMut(&str)) {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            feed("{");
            for k in keys {
                feed(&Value::String(k.clone()).to_string());
                feed(":");
                canonical(&m[k], feed);
                feed(",");
            }
            feed("}");
        }
        Value::Array(a) => {
            feed("[");
            for x in a {
                canonical(x, feed);
                feed(",");
            }
            feed("]");
        }
        v => feed(&v.to_string()),
    }
}
