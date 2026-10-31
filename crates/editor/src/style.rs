//! The editor's egui look: Barlow fonts and the game's ink-navy / gold palette (`game::ui`).

use std::path::Path;
use std::sync::Arc;

use bevy_egui::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Style, TextStyle,
};

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

pub fn display() -> FontFamily {
    FontFamily::Name("display".into())
}
pub fn label() -> FontFamily {
    FontFamily::Name("label".into())
}

/// Adds Barlow as named families (`display`, `label`) and as the proportional font, keeping egui's fonts as
/// fallbacks for symbols. Missing font files fall back to egui's defaults.
pub fn install_fonts(ctx: &egui::Context, assets: &Path) {
    let mut defs = FontDefinitions::default();
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
    defs.families.insert(FontFamily::Proportional, fam(body.then_some("barlow")));
    ctx.set_fonts(defs);
}

pub fn style(base: &Style) -> Style {
    let mut s = base.clone();
    s.text_styles = [
        (TextStyle::Heading, FontId::new(22., display())),
        (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
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
    let w = &mut v.widgets;
    for (st, bg, fg) in [
        (&mut w.noninteractive, INK, DIM),
        (&mut w.inactive, INK2, STAR),
        (&mut w.hovered, Color32::from_rgb(40, 46, 110), STAR),
        (&mut w.active, Color32::from_rgb(58, 64, 140), GOLD),
        (&mut w.open, INK2, STAR),
    ] {
        st.bg_fill = bg;
        st.weak_bg_fill = bg;
        st.fg_stroke = Stroke::new(1., fg);
        st.corner_radius = CornerRadius::same(5);
    }
    w.noninteractive.bg_stroke = Stroke::new(1., LINE);
    w.inactive.bg_stroke = Stroke::NONE;
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
