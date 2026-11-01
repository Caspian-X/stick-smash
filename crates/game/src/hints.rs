//! Control hints: `[control] what` chips in a device's own controls (a round gamepad button or a keycap), and the
//! per-device clock that fades a player's hints in and out on character select.

use std::collections::BTreeMap;

use bevy::prelude::*;
use sim::data::render::HintLook;

use crate::input::{self, Act, Dev, Devices};
use crate::settings::Saved;
use crate::ui::*;

/// A control named in a hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ctl {
    Confirm,
    Back,
    Start,
    Options,
    Change,
}

/// How `dev` presses `c` in menus: the label, and whether it is a gamepad button (round) or a key.
pub fn cap(c: Ctl, dev: Dev, saved: &Saved, devs: &Devices) -> (String, bool) {
    // Scripted bots stand in for gamepads and the keyboard.
    let prof = match dev {
        Dev::Bot(n) if n % 2 == 1 => "keyboard".to_string(),
        _ => devs.profile(dev),
    };
    let pad = !prof.starts_with("keyboard");
    let b = input::binds(saved, &prof);
    let first = |a: Act| {
        let l = b[a as usize].first().map_or(a.label().to_string(), |c| input::control_label(c));
        l.split(" / ").next().unwrap_or_default().to_string()
    };
    // Enter, Esc and Backspace always work on keyboards (Enter and Esc not on the other half).
    let half = |l: Dev| matches!(dev, Dev::KeysL | Dev::KeysR) && dev != l;
    let s = match c {
        Ctl::Confirm | Ctl::Start if !pad && !half(Dev::KeysR) => "Enter".into(),
        Ctl::Start if pad => first(Act::Start),
        Ctl::Confirm | Ctl::Start => first(Act::Attack),
        Ctl::Back if !pad && !half(Dev::KeysL) => "Esc".into(),
        Ctl::Back if !pad => "Backspace".into(),
        Ctl::Back => first(Act::Special),
        Ctl::Options => first(Act::Grab),
        Ctl::Change if pad => "D-pad".into(),
        Ctl::Change if matches!(dev, Dev::KeysL) => format!("{} {}", first(Act::Left), first(Act::Right)),
        Ctl::Change => "← →".into(),
    };
    (s, pad)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FadePart {
    Text,
    Fill,
    Border,
}

/// A hint's color that fades with its player's hint opacity ([`HintClock`]).
#[derive(Component)]
pub struct Fade {
    pub dev: Dev,
    pub base: Color,
    pub part: FadePart,
}

/// `dev`'s hints in its controls, in a wrapping row at the bottom of its card (faded by [`HintClock`]).
pub fn hint_row(
    p: &mut ChildSpawnerCommands,
    f: &Fonts,
    dev: Dev,
    hints: &[(Ctl, &str)],
    ink: Color,
    fade: bool,
    (saved, devs): (&Saved, &Devices),
) {
    p.spawn(node(|n| {
        n.margin.top = Val::Auto;
        n.flex_direction = FlexDirection::Row;
        n.flex_wrap = FlexWrap::Wrap;
        n.align_items = AlignItems::Center;
        n.column_gap = Val::Px(8.);
        n.row_gap = Val::Px(2.);
    }))
    .with_children(|p| {
        for &(c, what) in hints {
            let (label, pad) = cap(c, dev, saved, devs);
            key_cap(p, f, &label, pad, what, fade.then_some(dev), ink);
        }
    });
}

/// One `[control] what` hint: a round gamepad button or a keycap. `fade`: faded with that device's hint opacity.
pub fn key_cap(
    p: &mut ChildSpawnerCommands,
    f: &Fonts,
    label: &str,
    pad: bool,
    what: &str,
    fade: Option<Dev>,
    ink: Color,
) {
    // Only fading hints get a `Fade`.
    let fade = |e: &mut EntityCommands, base: Color, part: FadePart| {
        if let Some(dev) = fade {
            e.insert(Fade { dev, base, part });
        }
    };
    p.spawn(node(|n| {
        n.flex_direction = FlexDirection::Row;
        n.align_items = AlignItems::Center;
        n.column_gap = Val::Px(4.);
    }))
    .with_children(|p| {
        let mut c = p.spawn(node(|n| {
            n.padding = UiRect::axes(Val::Px(if pad { 6. } else { 5. }), Val::Px(0.));
            n.min_width = Val::Px(17.);
            n.justify_content = JustifyContent::Center;
            n.border = UiRect::all(Val::Px(1.));
            n.border_radius = BorderRadius::all(Val::Px(if pad { 9. } else { 4. }));
        }));
        if pad {
            fade(c.insert(BackgroundColor(ink)), ink, FadePart::Fill);
        } else {
            fade(c.insert(BorderColor::all(ink)), ink, FadePart::Border);
        }
        let text = if pad { NIGHT } else { STAR };
        c.with_children(|p| fade(&mut p.spawn(txt(&f.body_bold, 11., text, label)), text, FadePart::Text));
        fade(&mut p.spawn(txt(&f.body, 11., ink, what)), ink, FadePart::Text);
    });
}

/// Small key / button hint chip for a whole menu: `[label] action`.
pub fn hint(p: &mut ChildSpawnerCommands, fonts: &Fonts, key: &str, action: &str) {
    p.spawn(node(|n| {
        n.flex_direction = FlexDirection::Row;
        n.align_items = AlignItems::Center;
        n.column_gap = Val::Px(6.);
    }))
    .with_children(|p| {
        p.spawn((
            node(|n| {
                n.padding = UiRect::axes(Val::Px(6.), Val::Px(1.));
                n.border = UiRect::all(Val::Px(1.));
                n.border_radius = BorderRadius::all(Val::Px(4.));
            }),
            BorderColor::all(FAINT),
        ))
        .with_child(txt(&fonts.body_bold, 13., DIM, key));
        p.spawn(txt(&fonts.body, 13., DIM, action));
    });
}

/// Each playing device's hint opacity, and when it joined and last gave input (seconds).
#[derive(Resource, Default)]
pub struct HintClock {
    seen: BTreeMap<Dev, (f32, f32)>,
    shown: BTreeMap<Dev, f32>,
}

impl HintClock {
    /// Forgets devices no longer `live`.
    pub fn retain(&mut self, live: impl Fn(&Dev) -> bool) {
        self.seen.retain(|d, _| live(d));
        self.shown.retain(|d, _| live(d));
    }
    /// Steps `dev`'s opacity toward shown when it just joined, has idled, or `want`s them anyway. `active`: it gave
    /// input this frame.
    pub fn step(&mut self, dev: Dev, active: bool, want: bool, (now, dt): (f32, f32), h: &HintLook) {
        let seen = self.seen.entry(dev).or_insert((now, now));
        if active {
            seen.1 = now;
        }
        let want = want || now - seen.0 < h.show_secs || now - seen.1 > h.idle_secs;
        let a = self.shown.entry(dev).or_insert(0.);
        *a = (*a + if want { 1. } else { -1. } * dt / h.fade_secs.max(0.01)).clamp(0., 1.);
    }
}

/// Applies each hint's opacity to its faded colors.
pub fn fade(
    clock: Res<HintClock>,
    mut q: Query<(&Fade, Option<&mut TextColor>, Option<&mut BackgroundColor>, Option<&mut BorderColor>)>,
) {
    for (fd, text, bg, border) in &mut q {
        let c = fd.base.with_alpha(fd.base.alpha() * clock.shown.get(&fd.dev).copied().unwrap_or(0.));
        match fd.part {
            FadePart::Text => {
                if let Some(mut t) = text.filter(|t| t.0 != c) {
                    t.0 = c;
                }
            }
            FadePart::Fill => {
                if let Some(mut b) = bg.filter(|b| b.0 != c) {
                    b.0 = c;
                }
            }
            FadePart::Border => {
                if let Some(mut b) = border.filter(|b| b.top != c) {
                    *b = BorderColor::all(c);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_use_each_devices_controls() {
        let (saved, devs) = (Saved::default(), Devices::default());
        assert_eq!(cap(Ctl::Confirm, Dev::Keys, &saved, &devs), ("Enter".into(), false));
        assert_eq!(cap(Ctl::Back, Dev::KeysR, &saved, &devs), ("Backspace".into(), false));
        assert_eq!(cap(Ctl::Confirm, Dev::KeysL, &saved, &devs), ("J".into(), false));
        assert_eq!(cap(Ctl::Back, Dev::Bot(0), &saved, &devs), ("B".into(), true));
        assert_eq!(cap(Ctl::Start, Dev::Bot(0), &saved, &devs), ("Start".into(), true));
    }
}
