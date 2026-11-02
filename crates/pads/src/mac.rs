#![allow(unsafe_code)]
//! Switch controllers through the GameController framework, sampled once per frame on the main thread.
use bevy::input::gamepad::GamepadButton::*;
use bevy::prelude::*;
use objc2::rc::Retained;
use objc2_game_controller::{GCController, GCControllerButtonInput, GCControllerDirectionPad, GCDevice};

use super::{Bridge, Sample, Slot};

/// Connected controllers and their slots.
#[derive(Default)]
pub struct Pads(Vec<(Retained<GCController>, Slot)>);

fn is_switch(c: &GCController) -> bool {
    let cat = unsafe { c.productCategory() }.to_string();
    cat.contains("Switch") || cat.contains("Joy-Con")
}

fn dpad(s: &mut Sample, d: &GCControllerDirectionPad) {
    unsafe {
        s.set(DPadUp, d.up().value());
        s.set(DPadDown, d.down().value());
        s.set(DPadLeft, d.left().value());
        s.set(DPadRight, d.right().value());
    }
}

fn read(c: &GCController) -> Option<Sample> {
    let value = |b: Option<Retained<GCControllerButtonInput>>| b.map_or(0., |b| unsafe { b.value() });
    let mut s = Sample::default();
    unsafe {
        if let Some(g) = c.extendedGamepad() {
            s.set(South, g.buttonA().value());
            s.set(East, g.buttonB().value());
            s.set(West, g.buttonX().value());
            s.set(North, g.buttonY().value());
            s.set(LeftTrigger, g.leftShoulder().value());
            s.set(RightTrigger, g.rightShoulder().value());
            s.set(LeftTrigger2, g.leftTrigger().value());
            s.set(RightTrigger2, g.rightTrigger().value());
            s.set(Start, g.buttonMenu().value());
            s.set(Select, value(g.buttonOptions()));
            s.set(Mode, value(g.buttonHome()));
            s.set(LeftThumb, value(g.leftThumbstickButton()));
            s.set(RightThumb, value(g.rightThumbstickButton()));
            dpad(&mut s, &g.dpad());
            let (l, r) = (g.leftThumbstick(), g.rightThumbstick());
            s.stick(false, l.xAxis().value(), l.yAxis().value());
            s.stick(true, r.xAxis().value(), r.yAxis().value());
        } else {
            // A lone sideways Joy-Con: its stick is the d-pad, turned to match how it's held.
            let g = c.microGamepad()?;
            g.setAllowsRotation(true);
            let pad = g.dpad();
            s.set(South, g.buttonA().value());
            s.set(West, g.buttonX().value());
            s.set(Start, g.buttonMenu().value());
            dpad(&mut s, &pad);
            s.stick(false, pad.xAxis().value(), pad.yAxis().value());
        }
    }
    Some(s)
}

pub fn poll(mut pads: NonSendMut<Pads>, mut bridge: Bridge) {
    let now: Vec<_> = unsafe { GCController::controllers() }.iter().filter(|c| is_switch(c)).collect();
    pads.0.retain(|(c, slot)| {
        let keep = now.iter().any(|n| Retained::as_ptr(n) == Retained::as_ptr(c));
        if !keep {
            bridge.disconnect(slot);
        }
        keep
    });
    for c in now {
        if !pads.0.iter().any(|(p, _)| Retained::as_ptr(p) == Retained::as_ptr(&c)) {
            let name =
                unsafe { c.vendorName() }.map_or_else(|| unsafe { c.productCategory() }.to_string(), |n| n.to_string());
            let slot = bridge.connect(name, None);
            pads.0.push((c, slot));
        }
    }
    for (c, slot) in &mut pads.0 {
        if let Some(s) = read(c) {
            bridge.send(slot, s);
        }
    }
}
