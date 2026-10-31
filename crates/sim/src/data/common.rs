//! Small value types shared by every content file.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

/// Fighter-local point `[x, y]`: x = forward, y = up, origin at the feet.
pub type Pt = [f64; 2];

pub(crate) fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}
pub(crate) fn is_one(v: &f64) -> bool {
    *v == 1.0
}
pub(crate) fn one() -> f64 {
    1.0
}

/// A number, or `[min, max]` for a random value from the world RNG.
/// Ranges are drawn in field order (x, y, vx, vy, life, size), which matters for determinism.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Num {
    Fixed(f64),
    Range([f64; 2]),
}

impl Default for Num {
    fn default() -> Self {
        Num::Fixed(0.0)
    }
}

/// `"#rrggbb"`, or a player palette slot: `"player"`, `"playerDark"`, `"playerLight"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Color {
    Rgb(u32),
    #[default]
    Player,
    PlayerDark,
    PlayerLight,
}

impl TryFrom<String> for Color {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        match s.as_str() {
            "player" => Ok(Color::Player),
            "playerDark" => Ok(Color::PlayerDark),
            "playerLight" => Ok(Color::PlayerLight),
            _ => s
                .strip_prefix('#')
                .filter(|h| h.len() == 6)
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .map(Color::Rgb)
                .ok_or_else(|| {
                    format!("bad color {s:?}: expected \"#rrggbb\", \"player\", \"playerDark\" or \"playerLight\"")
                }),
        }
    }
}

impl From<Color> for String {
    fn from(c: Color) -> String {
        match c {
            Color::Rgb(v) => format!("#{v:06x}"),
            Color::Player => "player".into(),
            Color::PlayerDark => "playerDark".into(),
            Color::PlayerLight => "playerLight".into(),
        }
    }
}

impl JsonSchema for Color {
    fn schema_name() -> Cow<'static, str> {
        "Color".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "\"#rrggbb\", or a player palette slot: \"player\", \"playerDark\", \"playerLight\".",
            "type": "string",
            "pattern": "^(#[0-9a-fA-F]{6}|player|playerDark|playerLight)$"
        })
    }
}

/// Axis-aligned box in fighter-local space (x forward, y up from the feet).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rect {
    pub x1: f64,
    pub x2: f64,
    pub y1: f64,
    pub y2: f64,
}
