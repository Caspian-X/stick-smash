//! Gamepads Bevy's gilrs backend can't read, fed into Bevy's normal gamepad pipeline:
//! - macOS: the system driver takes over Switch controllers (Joy-Cons, Pro Controller), so gilrs sees
//!   them connected but silent; `mac.rs` reads them through Apple's GameController framework.
//! - GameCube controller adapters (Nintendo's WUP-028 and clones in Wii U mode) speak their own
//!   report format; `gcn.rs` reads all four ports.
use bevy::ecs::system::SystemParam;
use bevy::input::InputSystems;
use bevy::input::gamepad::{
    Gamepad, GamepadAxis, GamepadButton, GamepadConnection, GamepadConnectionEvent, RawGamepadAxisChangedEvent,
    RawGamepadButtonChangedEvent, RawGamepadEvent,
};
use bevy::prelude::*;

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod gcn;
#[cfg(target_os = "macos")]
mod mac;

/// Marks a gamepad entity driven by a native backend rather than gilrs.
#[derive(Component)]
pub struct Native;

const NINTENDO: u16 = 0x057e;

/// A gilrs pad that a native backend reads instead; it never reports input, so hide it.
pub fn shadowed(pad: &Gamepad, native: bool) -> bool {
    cfg!(target_os = "macos") && !native && pad.vendor_id() == Some(NINTENDO)
}

pub struct PadsPlugin;

impl Plugin for PadsPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(target_os = "macos")]
        app.insert_non_send(mac::Pads::default()).add_systems(PreUpdate, mac::poll.before(InputSystems));
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        app.insert_resource(gcn::Adapter::spawn()).add_systems(PreUpdate, gcn::poll.before(InputSystems));
        let _ = app;
    }
}

const BUTTONS: [GamepadButton; 17] = {
    use GamepadButton::*;
    [
        South,
        East,
        West,
        North,
        LeftTrigger,
        RightTrigger,
        LeftTrigger2,
        RightTrigger2,
        Start,
        Select,
        Mode,
        LeftThumb,
        RightThumb,
        DPadUp,
        DPadDown,
        DPadLeft,
        DPadRight,
    ]
};
const AXES: [GamepadAxis; 4] =
    [GamepadAxis::LeftStickX, GamepadAxis::LeftStickY, GamepadAxis::RightStickX, GamepadAxis::RightStickY];

/// One reading of a pad's buttons (0..1) and sticks (-1..1, up positive).
#[derive(Clone, Copy, PartialEq, Default)]
struct Sample {
    buttons: [f32; BUTTONS.len()],
    axes: [f32; AXES.len()],
}

impl Sample {
    fn set(&mut self, button: GamepadButton, value: f32) {
        if let Some(i) = BUTTONS.iter().position(|b| *b == button) {
            self.buttons[i] = value;
        }
    }
    fn stick(&mut self, right: bool, x: f32, y: f32) {
        let i = if right { 2 } else { 0 };
        self.axes[i] = x;
        self.axes[i + 1] = y;
    }
}

/// A connected native pad: its entity and the last sample sent for it.
struct Slot {
    entity: Entity,
    last: Sample,
}

/// Writes native pads into Bevy's gamepad events, sending only what changed.
#[derive(SystemParam)]
struct Bridge<'w, 's> {
    commands: Commands<'w, 's>,
    raw: MessageWriter<'w, RawGamepadEvent>,
    connections: MessageWriter<'w, GamepadConnectionEvent>,
}

impl Bridge<'_, '_> {
    fn connection(&mut self, e: GamepadConnectionEvent) {
        self.raw.write(e.clone().into());
        self.connections.write(e);
    }
    fn connect(&mut self, name: String, product_id: Option<u16>) -> Slot {
        let entity = self.commands.spawn(Native).id();
        let connection = GamepadConnection::Connected { name, vendor_id: Some(NINTENDO), product_id };
        self.connection(GamepadConnectionEvent::new(entity, connection));
        Slot { entity, last: Sample::default() }
    }
    fn disconnect(&mut self, slot: &Slot) {
        self.connection(GamepadConnectionEvent::new(slot.entity, GamepadConnection::Disconnected));
    }
    fn send(&mut self, slot: &mut Slot, s: Sample) {
        let e = slot.entity;
        for (i, b) in BUTTONS.iter().enumerate().filter(|(i, _)| s.buttons[*i] != slot.last.buttons[*i]) {
            self.raw.write(RawGamepadButtonChangedEvent::new(e, *b, s.buttons[i]).into());
        }
        for (i, a) in AXES.iter().enumerate().filter(|(i, _)| s.axes[*i] != slot.last.axes[*i]) {
            self.raw.write(RawGamepadAxisChangedEvent::new(e, *a, s.axes[i]).into());
        }
        slot.last = s;
    }
}
