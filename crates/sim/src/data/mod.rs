//! Content: global tuning, stages and fighters, loaded from JSON strings. The optional `fs` feature adds
//! `sources` for reading content roots outside the sim core. See `docs/FORMAT.md` for the format.

pub mod audio;
pub mod camera;
pub mod common;
pub mod fighter;
pub mod global;
pub mod hitbox;
pub mod json;
pub mod load;
pub mod moves;
pub mod render;
pub mod rig;
pub mod schema;
pub mod stage;
pub mod unit;
pub mod visual;

pub use common::{Color, Num, Pt, Rect};
pub use fighter::{Fighter, Projectile, Stats};
pub use global::Global;
pub use hitbox::{HitType, Hitbox, LaunchDir};
#[cfg(feature = "fs")]
pub use load::sources;
pub use load::{ContentError, ContentKind, Source, load, migrate_kind, validate};
pub use moves::{ChargeNeed, ChargePool, Event, Move, MoveVariant, Moves, Phase, Transition, Trigger};
pub use stage::Stage;

/// Current stage format version; fighter/global version histories are independent.
pub const SCHEMA_VERSION: u32 = 2;

/// Everything a match reads. `hash` identifies the exact content, so online peers can check they match.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GameData {
    pub global: Global,
    pub stages: Vec<Stage>,
    /// Playable fighters (abstract packages excluded), sorted by id.
    pub fighters: Vec<Fighter>,
    pub hash: u64,
}

impl GameData {
    pub fn fighter(&self, id: &str) -> Option<&Fighter> {
        self.fighters.iter().find(|f| f.id == id)
    }
    pub fn stage(&self, id: &str) -> Option<&Stage> {
        self.stages.iter().find(|s| s.id == id)
    }
}
