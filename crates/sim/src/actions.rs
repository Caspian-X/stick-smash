//! Accepted actions: what the sim actually did with a player's input, for the input display. Emitted where an
//! action starts (not inferred from state changes) and drained by the caller each tick, like `fx`.
use crate::World;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// A move by slot (`jab`, `fsmash`, `uspec`…).
    Move(String),
    /// A player-selected phase within a move.
    Followup {
        slot: String,
        phase: String,
    },
    Jump,
    ShortHop,
    FullHop,
    AirJump,
    Dash,
    Turn,
    Shield,
    ShieldDrop,
    Parry,
    Roll,
    SpotDodge,
    AirDodge,
    FastFall,
    /// A float started (`stats.float`).
    Float,
    PlatformDrop,
    LedgeGrab,
    LedgeTrump,
    /// Knocked off a ledge by another fighter.
    Trumped,
    /// Ledge getup by kind (`climb`, `attack`, `roll`, `jump`).
    Ledge(String),
    LedgeDrop,
    Getup,
    GetupAttack,
    Tech,
    TechRoll,
    Pummel,
    /// Throw by direction (`f`, `b`, `u`, `d`).
    Throw(String),
    /// Directional influence bent a launch by this many degrees (rounded).
    Influence(i32),
}
impl Action {
    /// Started by a button press or flick a buffered command could have asked for (not movement or reactions).
    pub fn command(&self) -> bool {
        !matches!(
            self,
            Action::Dash
                | Action::Turn
                | Action::ShortHop
                | Action::FullHop
                | Action::ShieldDrop
                | Action::FastFall
                | Action::Trumped
                | Action::LedgeDrop
                | Action::Influence(_)
        )
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Accepted {
    pub player: usize,
    pub action: Action,
    pub tick: u64,
    /// The press it came from, when that was earlier (buffered).
    pub pressed: Option<u64>,
}
pub fn accept(w: &mut World, player: usize, action: Action) {
    let tick = w.frame;
    w.acts.push(Accepted { player, action, tick, pressed: None });
}
