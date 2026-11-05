//! Unit tags for numeric content fields, written into the generated schemas as `x-unit` / `x-axis` so tools
//! (`tools/rescale.py`) can rescale content by unit. Use on a field: `#[schemars(transform = unit::v_x)]`.
//!
//! Units: `px` length / position (stage or fighter-local pixels), `px/tick` speed, `px/tick2` acceleration, `px/kb`
//! launch speed per reference knockback unit, `deg/px` rotation per pixel travelled, `tick2/px` frames per px/tick
//! of launch speed, `tick` frames (also per-charge-tick frame counts), `s` real seconds, `ratio` multipliers,
//! fractions and probabilities, `deg` / `rad` angles, `damage` percent and shield HP, `kb` reference knockback
//! units, `kb/tick`, `count`, `hz`, `uipx` logical UI / screen pixels (independent of the world scale), `none`
//! (ids, stick units, seeds, exponents, springs). A coefficient whose product is a non-spatial quantity takes that
//! quantity's unit (hitlag per damage → `tick`, shield HP per tick → `damage`, degrees per tick → `deg`).
//!
//! Axis (every unit that contains `px` except `uipx`): `x` horizontal, `y` vertical, `xy` isotropic (radii, sizes,
//! magnitudes along an arbitrary direction). Fixed tuples tag each element with an array (`[x, y]` points use
//! `["x", "y"]`; `null` where an element has no axis). A tag on an object-valued field overrides every tag inside it
//! (an event's `mulVel` is a `ratio` though `Vel` holds speeds); the outermost tag wins.

use schemars::Schema;
use serde_json::{Value, json};

/// Every `x-unit` value.
pub const UNITS: [&str; 18] = [
    "px", "px/tick", "px/tick2", "px/kb", "deg/px", "tick2/px", "tick", "s", "ratio", "deg", "rad", "damage", "kb",
    "kb/tick", "count", "hz", "uipx", "none",
];
/// Every `x-axis` value.
pub const AXES: [&str; 3] = ["x", "y", "xy"];

/// Does this unit need an `x-axis`?
pub fn spatial(unit: &str) -> bool {
    unit.contains("px") && unit != "uipx"
}

fn tag(s: &mut Schema, unit: Value, axis: Value) {
    s.insert("x-unit".into(), unit);
    if !axis.is_null() {
        s.insert("x-axis".into(), axis);
    }
}

macro_rules! units {
    ($($(#[$doc:meta])* $name:ident = $unit:tt $(, $axis:tt)?;)*) => {
        $($(#[$doc])* pub fn $name(s: &mut Schema) {
            tag(s, json!($unit), units!(@axis $($axis)?))
        })*
    };
    (@axis) => { Value::Null };
    (@axis $axis:tt) => { json!($axis) };
}

units! {
    px_x = "px", "x";
    px_y = "px", "y";
    px_xy = "px", "xy";
    px_pt = "px", ["x", "y"];
    v_x = "px/tick", "x";
    v_y = "px/tick", "y";
    v_xy = "px/tick", "xy";
    v_pt = "px/tick", ["x", "y"];
    a_x = "px/tick2", "x";
    a_y = "px/tick2", "y";
    a_xy = "px/tick2", "xy";
    px_kb = "px/kb", "xy";
    deg_px = "deg/px", "xy";
    tick2_px = "tick2/px", "xy";
    tick = "tick";
    s = "s";
    ratio = "ratio";
    deg = "deg";
    rad = "rad";
    damage = "damage";
    kb = "kb";
    kb_tick = "kb/tick";
    count = "count";
    hz = "hz";
    uipx = "uipx";
    none = "none";
    /// Hitbox track rows `[frame, x, y, r]`.
    hit_frame = ["tick", "px", "px", "px"], [null, "x", "y", "xy"];
    /// Attachment keys `[frame, degrees]`.
    key_deg = ["tick", "deg"];
    /// Attachment keys `[frame, x, y]`.
    key_offset = ["tick", "px", "px"], [null, "x", "y"];
    /// Attachment keys `[frame, scale]`.
    key_ratio = ["tick", "ratio"];
    /// `[x, y, w, h]`.
    px_box = "px", ["x", "y", "x", "y"];
    /// Stage platform shadow `[offset, height, alpha]`.
    shadow_band = ["px", "px", "ratio"], ["y", "y", null];
}
