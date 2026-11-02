//! GameCube controller adapters (Nintendo WUP-028, and clones like Hyperkin's in Wii U mode). A reader
//! thread keeps the latest report; each frame turns the four ports into pads. The adapter needs
//! no driver: it is a HID device that starts reporting after a 0x13 output report.
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use bevy::input::gamepad::GamepadButton::*;
use bevy::prelude::*;

use super::{Bridge, NINTENDO, Sample, Slot};

const PRODUCT: u16 = 0x0337;
/// Report 0x21: id, then per port: status, buttons, buttons, stick x / y, c-stick x / y, L, R.
const REPORT: usize = 1 + 4 * PORT;
const PORT: usize = 9;
/// How far a GameCube stick reaches from its centre (of 127); the gate stops well short of the edge.
const STICK_REACH: f32 = 100.;

#[derive(Resource)]
pub struct Adapter {
    latest: Arc<Mutex<Option<[u8; REPORT]>>>,
    ports: [Option<Port>; 4],
}

struct Port {
    slot: Slot,
    /// Stick and c-stick rest positions read when the controller connects, like the console does.
    centre: [u8; 4],
}

impl Adapter {
    pub fn spawn() -> Self {
        let latest = Arc::new(Mutex::new(None));
        let shared = latest.clone();
        let reader = move || {
            loop {
                if let Err(e) = read_adapter(&shared) {
                    debug!("GameCube adapter: {e}");
                }
                *shared.lock().unwrap() = None;
                thread::sleep(Duration::from_secs(1));
            }
        };
        if let Err(e) = thread::Builder::new().name("gamecube-adapter".into()).spawn(reader) {
            warn!("GameCube adapter reader not started: {e}");
        }
        Self { latest, ports: Default::default() }
    }
}

/// Opens the adapter and copies reports until it goes away.
fn read_adapter(latest: &Mutex<Option<[u8; REPORT]>>) -> hidapi::HidResult<()> {
    let api = hidapi::HidApi::new()?;
    let dev = api.open(NINTENDO, PRODUCT)?;
    dev.write(&[0x13])?;
    let mut buf = [0u8; 64];
    loop {
        let n = dev.read_timeout(&mut buf, 1000)?;
        if n >= REPORT && buf[0] == 0x21 {
            *latest.lock().unwrap() = Some(buf[..REPORT].try_into().unwrap());
        }
    }
}

fn sample(p: &[u8], centre: [u8; 4]) -> Sample {
    let mut s = Sample::default();
    let bit = |byte: u8, mask: u8| if byte & mask != 0 { 1. } else { 0. };
    let axis = |i: usize| ((f32::from(p[3 + i]) - f32::from(centre[i])) / STICK_REACH).clamp(-1., 1.);
    // Analog L / R count as fully pressed at the click, partly before it.
    let trigger = |analog: u8, click: f32| click.max(f32::from(analog) / 255.);
    s.set(South, bit(p[1], 0x01));
    s.set(East, bit(p[1], 0x02));
    s.set(North, bit(p[1], 0x04));
    s.set(West, bit(p[1], 0x08));
    s.set(DPadLeft, bit(p[1], 0x10));
    s.set(DPadRight, bit(p[1], 0x20));
    s.set(DPadDown, bit(p[1], 0x40));
    s.set(DPadUp, bit(p[1], 0x80));
    s.set(Start, bit(p[2], 0x01));
    s.set(RightTrigger, bit(p[2], 0x02));
    s.set(RightTrigger2, trigger(p[8], bit(p[2], 0x04)));
    s.set(LeftTrigger2, trigger(p[7], bit(p[2], 0x08)));
    s.stick(false, axis(0), axis(1));
    s.stick(true, axis(2), axis(3));
    s
}

pub fn poll(mut adapter: ResMut<Adapter>, mut bridge: Bridge) {
    let report = *adapter.latest.lock().unwrap();
    for (i, port) in adapter.ports.iter_mut().enumerate() {
        // Status bits 4-5: 1 = wired controller, 2 = wireless (WaveBird).
        let p = report.as_ref().map(|r| &r[1 + i * PORT..1 + (i + 1) * PORT]).filter(|p| p[0] & 0x30 != 0);
        match (p, port.as_mut()) {
            (Some(p), Some(port)) => bridge.send(&mut port.slot, sample(p, port.centre)),
            (Some(p), None) => {
                let centre = [p[3], p[4], p[5], p[6]];
                let mut slot = bridge.connect(format!("GameCube {}", i + 1), Some(PRODUCT));
                bridge.send(&mut slot, sample(p, centre));
                *port = Some(Port { slot, centre });
            }
            (None, Some(old)) => {
                bridge.disconnect(&old.slot);
                *port = None;
            }
            (None, None) => {}
        }
    }
}
