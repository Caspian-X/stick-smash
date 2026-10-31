//! The editor's egui look: Barlow fonts and the game's ink-navy / gold palette (`game::ui`), plus editor-only UI
//! constants. Fonts are added as named families only, so the game's egui text keeps its own fonts.

use std::path::Path;
use std::sync::Arc;

use bevy_egui::egui::{self, Color32, CornerRadius, FontData, FontFamily, FontId, Stroke, Style, TextStyle};

pub const PREVIEW_ASPECT: f64 = 16. / 9.;

pub const NIGHT: Color32 = Color32::from_rgb(11, 13, 42);
pub const INK: Color32 = Color32::from_rgb(16, 19, 56);
pub const INK2: Color32 = Color32::from_rgb(24, 28, 74);
pub const LINE: Color32 = Color32::from_rgb(51, 59, 122);
pub const STAR: Color32 = Color32::from_rgb(237, 240, 255);
pub const DIM: Color32 = Color32::from_rgb(140, 148, 200);
pub const FAINT: Color32 = Color32::from_rgb(92, 99, 153);
pub const GOLD: Color32 = Color32::from_rgb(255, 211, 90);
pub const RED: Color32 = Color32::from_rgb(255, 120, 110);
pub const GREEN: Color32 = Color32::from_rgb(110, 230, 140);
pub const HOVER: Color32 = Color32::from_rgb(40, 46, 110);
pub const ERR_BG: Color32 = Color32::from_rgb(60, 14, 24);
pub const ERR_TEXT: Color32 = Color32::from_rgb(255, 200, 200);
pub const TEST: Color32 = Color32::from_rgb(40, 110, 80);
/// Onion skin: earlier and later frames.
pub const ONION_PAST: u32 = 0xff7a8a;
pub const ONION_NEXT: u32 = 0x6ee6a0;
pub const ONION_FRAMES: usize = 3;
/// Timeline phase colors, also used for the inspector's phase swatches.
pub const PHASE_COLORS: [Color32; 6] = [
    Color32::from_rgb(63, 182, 255),
    Color32::from_rgb(255, 138, 31),
    Color32::from_rgb(176, 112, 255),
    Color32::from_rgb(42, 212, 192),
    Color32::from_rgb(255, 93, 200),
    Color32::from_rgb(168, 196, 255),
];
/// Seconds a toast stays up.
pub const TOAST_SECS: f32 = 4.5;
pub const TOAST_ERR_SECS: f32 = 6.;
pub const TOAST_W: f32 = 320.;
/// Dims everything behind a modal.
pub const BACKDROP: Color32 = Color32::from_black_alpha(150);
/// Fill of cards inside the inspector (the selected hitbox), darker than fields so they stay visible.
pub const CARD: Color32 = NIGHT;
/// Outline of editable fields (drag values, text edits, checkboxes) at rest.
pub const FIELD_LINE: Color32 = Color32::from_rgb(44, 51, 112);
pub const BROWSER_W: f32 = 230.;
pub const BROWSER_MIN: f32 = 180.;
pub const BROWSER_MAX: f32 = 340.;
pub const INSPECTOR_W: f32 = 380.;
/// Inspector grid: label column and numeric field widths.
pub const LABEL_W: f32 = 104.;
pub const NUM_W: f32 = 58.;
/// Glyphs for icon buttons that exist in the loaded fonts (egui's emoji-icon-font and Hack).
pub const ICON_DUP: &str = "🗐";
pub const ICON_DEL: &str = "✖";
pub const ICON_UP: &str = "⏶";
pub const ICON_DOWN: &str = "⏷";

pub fn display() -> FontFamily {
    FontFamily::Name("display".into())
}
pub fn label() -> FontFamily {
    FontFamily::Name("label".into())
}
pub fn body() -> FontFamily {
    FontFamily::Name("body".into())
}
pub fn text(size: f32) -> FontId {
    FontId::new(size, body())
}

/// Adds Barlow as named families (`display`, `label`, `body`) on top of the context's fonts, keeping egui's fonts as
/// fallbacks for symbols. Missing font files fall back to egui's defaults. Idempotent.
pub fn install_fonts(ctx: &egui::Context, assets: &Path) {
    let mut defs = ctx.fonts(|f| f.definitions().clone());
    if defs.families.contains_key(&body()) {
        return;
    }
    let mut add = |name: &str, file: &str| match std::fs::read(assets.join("fonts").join(file)) {
        Ok(b) => {
            defs.font_data.insert(name.into(), Arc::new(FontData::from_owned(b)));
            true
        }
        Err(_) => false,
    };
    let body = add("barlow", "Barlow-Medium.ttf");
    let disp = add("barlow-display", "BarlowCondensed-BoldItalic.ttf");
    let lab = add("barlow-label", "BarlowCondensed-SemiBold.ttf");
    // egui's fonts, then Hack for arrows and other symbols (fighter blurbs use →).
    let mut fallback = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    fallback.extend(defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default());
    let fam = |first: Option<&str>| first.into_iter().map(String::from).chain(fallback.iter().cloned()).collect();
    defs.families.insert(display(), fam(disp.then_some("barlow-display")));
    defs.families.insert(label(), fam(lab.then_some("barlow-label")));
    defs.families.insert(self::body(), fam(body.then_some("barlow")));
    ctx.set_fonts(defs);
}

pub fn style(base: &Style) -> Style {
    let mut s = base.clone();
    s.text_styles = [
        (TextStyle::Heading, FontId::new(22., display())),
        (TextStyle::Body, text(13.5)),
        (TextStyle::Button, text(13.5)),
        (TextStyle::Small, text(11.5)),
        (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
    ]
    .into();
    s.spacing.item_spacing = egui::vec2(6., 5.);
    s.spacing.button_padding = egui::vec2(8., 3.);
    s.spacing.interact_size.y = 22.;
    let v = &mut s.visuals;
    *v = egui::Visuals::dark();
    v.override_text_color = None;
    v.panel_fill = INK;
    v.window_fill = INK;
    v.window_stroke = Stroke::new(1., LINE);
    v.extreme_bg_color = NIGHT;
    v.faint_bg_color = INK2;
    v.hyperlink_color = GOLD;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(255, 211, 90, 70);
    v.selection.stroke = Stroke::new(1., GOLD);
    v.window_corner_radius = CornerRadius::same(10);
    v.popup_shadow.color = Color32::from_black_alpha(120);
    s.interaction.tooltip_delay = 0.35;
    let w = &mut v.widgets;
    for (st, bg, fg) in [
        (&mut w.noninteractive, INK, DIM),
        (&mut w.inactive, INK2, STAR),
        (&mut w.hovered, HOVER, STAR),
        (&mut w.active, Color32::from_rgb(58, 64, 140), GOLD),
        (&mut w.open, INK2, STAR),
    ] {
        st.bg_fill = bg;
        st.weak_bg_fill = bg;
        st.fg_stroke = Stroke::new(1., fg);
        st.corner_radius = CornerRadius::same(5);
    }
    w.noninteractive.bg_stroke = Stroke::new(1., LINE);
    w.inactive.bg_stroke = Stroke::new(1., FIELD_LINE);
    w.hovered.bg_stroke = Stroke::new(1., Color32::from_rgb(90, 98, 180));
    w.active.bg_stroke = Stroke::new(1., GOLD);
    s
}

pub fn heading(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).family(display()).size(19.).color(STAR)
}
pub fn caps(text: &str) -> egui::RichText {
    egui::RichText::new(text.to_uppercase()).family(label()).size(12.5).color(FAINT)
}
pub fn faint(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).color(FAINT).small()
}
/// A keyboard key cap, for hints and the shortcut sheet.
pub fn key(text: &str) -> egui::RichText {
    egui::RichText::new(text).family(label()).size(13.5).color(GOLD)
}
pub fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}
