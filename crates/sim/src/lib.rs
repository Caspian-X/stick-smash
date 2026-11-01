//! Deterministic Stick Smash simulation: a pure function of `(World, GameData, inputs)`.
//!
//! Rules for this crate (see AGENTS.md):
//! - No Bevy, no core file/network I/O, no wall-clock time, no thread-local or OS randomness.
//!   The opt-in `fs` feature provides only the content-root reader `data::sources`.
//! - Randomness comes from [`rng::Rng`] stored on the world.
//! - Transcendental math goes through `libm` so results match across platforms (rollback netcode).
//! - Visual/audio side effects are emitted as [`Fx`] events for the game crate to consume.

pub mod ai;
pub mod combat;
pub mod data;
pub mod input;
pub mod moves;
pub mod physics;
pub mod projectiles;
pub mod rng;
pub mod states;
pub mod step;
pub mod world;

pub use data::GameData;
pub use input::Input;
pub use step::step_world;
pub use world::{Fx, HitRecord, MatchConfig, PlayerConfig, World};

/// Simulation ticks per second.
pub const TICK_HZ: u32 = 60;
